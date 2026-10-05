//! Spectrum: a spectrum analyser for Oroboro Modular, on a panel in the
//! plugin's own look with a screen. What comes in (left and right, summed) goes through to
//! its outputs unchanged, and the screen shows how loud each frequency of
//! it is, from 20 Hz to 20 kHz, as it plays.
//!
//! The audio thread only keeps the last samples (in a ring the screen
//! reads); the screen does the rest on the plugin's window thread, each
//! frame: a Hann window, an FFT, decibels, smoothing, the peaks held.

use std::f32::consts::PI;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use oroboro_module::panel::MEDIUM;
use oroboro_module::{export_module, Align, Color, Drawing, Module, Panel, Param, Pointer, Screen, Spec};

/// The screen's box on the panel: left, top, width, height.
const SCREEN: (f32, f32, f32, f32) = (8.0, 22.0, 239.0, 132.0);

/// How many samples a picture looks at: a power of two.
const N: usize = 8192;

/// A sine of this many volts at its peak reads 0 dB (Oroboro's audio is
/// about ±5 V).
const FULL: f32 = 5.0;

/// The screen's top, in dB.
const CEILING: f32 = 6.0;

/// Knobs, in the spec's order.
const GAIN: usize = 0;
const FLOOR: usize = 1;
const SMOOTH: usize = 2;
const SCALE: usize = 3;
const HOLD: usize = 4;

/// The lowest and highest frequency the log scale shows.
const LOWEST: f32 = 20.0;
const HIGHEST: f32 = 20_000.0;

/// What the screen knows of the module: the samples it last had, and the
/// knobs the picture follows. Written by the audio thread (atomics),
/// read by the window's.
struct Shared {
    ring: Vec<AtomicU32>,
    /// Where the next sample goes (counts on: the ring is `% N`).
    written: AtomicUsize,
    sample_rate: f32,
    floor: AtomicU32,
    smooth: AtomicU32,
    log: AtomicU32,
    hold: AtomicU32,
}

impl Shared {
    fn get(cell: &AtomicU32) -> f32 {
        f32::from_bits(cell.load(Ordering::Relaxed))
    }

    fn set(cell: &AtomicU32, v: f32) {
        cell.store(v.to_bits(), Ordering::Relaxed);
    }
}

/// What the picture keeps from frame to frame (the window's thread's).
struct View {
    /// Each bin's level as shown, and as held (dB).
    shown: Vec<f32>,
    held: Vec<f32>,
    /// The pointer over the screen (panel pixels), for its readout.
    hover: Option<(f32, f32)>,
    /// A click on the screen stopped the picture.
    frozen: bool,
    // the FFT's room and tables
    re: Vec<f32>,
    im: Vec<f32>,
    window: Vec<f32>,
    window_sum: f32,
    cos: Vec<f32>,
    sin: Vec<f32>,
}

impl View {
    fn new() -> View {
        let window: Vec<f32> = (0..N).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / N as f32).cos()).collect();
        View {
            shown: vec![-200.0; N / 2],
            held: vec![-200.0; N / 2],
            hover: None,
            frozen: false,
            re: vec![0.0; N],
            im: vec![0.0; N],
            window_sum: window.iter().sum(),
            window,
            cos: (0..N / 2).map(|k| (2.0 * PI * k as f32 / N as f32).cos()).collect(),
            sin: (0..N / 2).map(|k| (2.0 * PI * k as f32 / N as f32).sin()).collect(),
        }
    }

    /// The last `N` samples, windowed, into the spectrum: each bin's level
    /// in dB, smoothed as it falls (`smooth`: 0 follows at once), held.
    fn take(&mut self, shared: &Shared, smooth: f32, hold: bool) {
        let end = shared.written.load(Ordering::Acquire);
        for i in 0..N {
            let at = end.wrapping_add(i).wrapping_sub(N) % N;
            self.re[i] = Shared::get(&shared.ring[at]) * self.window[i];
            self.im[i] = 0.0;
        }
        fft(&mut self.re, &mut self.im, &self.cos, &self.sin);
        let scale = 2.0 / (self.window_sum * FULL);
        for k in 0..N / 2 {
            let magnitude = (self.re[k] * self.re[k] + self.im[k] * self.im[k]).sqrt() * scale;
            let level = 20.0 * (magnitude + 1e-9).log10();
            let was = self.shown[k];
            // up at once, down as slowly as Smooth says
            self.shown[k] = if level > was { level } else { was * smooth + level * (1.0 - smooth) };
            self.held[k] = if hold { (self.held[k] - 0.35).max(self.shown[k]) } else { self.shown[k] };
        }
    }
}

/// An FFT in place (radix 2, `re` and `im` as long as twice the tables).
fn fft(re: &mut [f32], im: &mut [f32], cos: &[f32], sin: &[f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let step = n / len;
        let half = len / 2;
        for start in (0..n).step_by(len) {
            for k in 0..half {
                let (wr, wi) = (cos[k * step], -sin[k * step]);
                let (a, b) = (start + k, start + k + half);
                let (xr, xi) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
            }
        }
        len <<= 1;
    }
}

/// The screen: the spectrum, its grid, the pointer's readout.
pub struct SpectrumScreen {
    shared: Arc<Shared>,
    view: Mutex<View>,
}

impl SpectrumScreen {
    /// Where frequency `f` is across the screen, 0 to 1.
    fn across(&self, f: f32, log: bool) -> f32 {
        let nyquist = self.shared.sample_rate / 2.0;
        match log {
            true => (f.max(LOWEST) / LOWEST).ln() / (HIGHEST.min(nyquist) / LOWEST).ln(),
            false => f / HIGHEST.min(nyquist),
        }
    }

    /// The frequency at `t` across the screen (0 to 1).
    fn frequency(&self, t: f32, log: bool) -> f32 {
        let top = HIGHEST.min(self.shared.sample_rate / 2.0);
        match log {
            true => LOWEST * (top / LOWEST).powf(t),
            false => top * t,
        }
    }

    /// A list's level between frequencies `f0` and `f1`: the loudest bin
    /// between them, or between the bins about them where none is.
    fn level(&self, list: &[f32], f0: f32, f1: f32) -> f32 {
        let hz = self.shared.sample_rate / N as f32;
        let (b0, b1) = (f0 / hz, f1 / hz);
        let last = list.len() - 1;
        if b1 - b0 < 1.0 {
            let i = (b0.floor() as usize).min(last);
            let t = b0 - b0.floor();
            let next = (i + 1).min(last);
            list[i] + (list[next] - list[i]) * t
        } else {
            let (from, to) = ((b0.ceil() as usize).min(last), (b1.floor() as usize).min(last));
            list[from..=to.max(from)].iter().copied().fold(f32::MIN, f32::max)
        }
    }
}

/// A frequency as the readout says it.
fn hertz(f: f32) -> String {
    if f >= 1000.0 {
        format!("{:.2} kHz", f / 1000.0)
    } else {
        format!("{f:.0} Hz")
    }
}

impl Screen for SpectrumScreen {
    fn draw(&self, d: &mut Drawing) {
        let Ok(mut view) = self.view.lock() else { return };
        let shared = &self.shared;
        let floor = Shared::get(&shared.floor);
        let log = Shared::get(&shared.log) < 0.5;
        let hold = Shared::get(&shared.hold) >= 0.5;
        if !view.frozen {
            view.take(shared, Shared::get(&shared.smooth), hold);
        }
        let (x0, y0, w, h) = SCREEN;
        let y_of = |db: f32| y0 + h * (CEILING - db.clamp(floor, CEILING)) / (CEILING - floor);
        d.clip(x0, y0, x0 + w, y0 + h);

        // the grid: the decades (or each 5 kHz), and every 12 dB
        let grid = Color::rgb(0x8a, 0x92, 0x98).alpha(0.25);
        let label = Color::rgb(0x9a, 0xa2, 0xa8).alpha(0.8);
        let marks: &[(f32, &str)] = match log {
            true => &[(100.0, "100"), (1000.0, "1k"), (10_000.0, "10k")],
            false => &[(5000.0, "5k"), (10_000.0, "10k"), (15_000.0, "15k")],
        };
        for &(f, name) in marks {
            let x = x0 + w * self.across(f, log);
            d.line(&[(x, y0), (x, y0 + h)], 0.6, grid);
            d.text(x + 2.0, y0 + h - 5.0, name, 6.5, label, Align::Left);
        }
        let mut db = 0.0;
        while db > floor {
            let y = y_of(db);
            d.line(&[(x0, y), (x0 + w, y)], 0.6, grid);
            d.text(x0 + 3.0, y - 4.0, &format!("{db:.0}"), 6.5, label, Align::Left);
            db -= 12.0;
        }

        // the spectrum: a point a pixel, filled under, then its line
        let columns = w as usize;
        let at = |list: &[f32], px: usize| {
            let (t0, t1) = (px as f32 / w, (px as f32 + 1.0) / w);
            self.level(list, self.frequency(t0, log), self.frequency(t1, log))
        };
        let points: Vec<(f32, f32)> = (0..=columns).map(|px| (x0 + px as f32, y_of(at(&view.shown, px)))).collect();
        let accent = Color::rgb(0x6d, 0xd6, 0x8e);
        let mut area = points.clone();
        area.push((x0 + w, y0 + h));
        area.push((x0, y0 + h));
        d.fill_down(&area, accent.alpha(0.45), accent.alpha(0.03), y0, y0 + h);
        d.line(&points, 1.2, accent);
        if hold {
            let held: Vec<(f32, f32)> = (0..=columns).map(|px| (x0 + px as f32, y_of(at(&view.held, px)))).collect();
            d.line(&held, 0.8, Color::rgb(0xf0, 0xb4, 0x4a).alpha(0.85));
        }

        // where the pointer is: the frequency and its level there
        if let Some((px, _)) = view.hover {
            let t = ((px - x0) / w).clamp(0.0, 1.0);
            let f = self.frequency(t, log);
            let level = self.level(&view.shown, f, f);
            let pointer = Color::rgb(0xe6, 0xea, 0xec);
            d.line(&[(px, y0), (px, y0 + h)], 0.7, pointer.alpha(0.6));
            let (text, align, x) = match t < 0.6 {
                true => (format!("{}  {level:.0} dB", hertz(f)), Align::Left, px + 4.0),
                false => (format!("{}  {level:.0} dB", hertz(f)), Align::Right, px - 4.0),
            };
            d.text(x, y0 + 8.0, &text, 7.0, pointer, align);
        }
        if view.frozen {
            d.text(x0 + w - 4.0, y0 + h - 5.0, "FROZEN", 6.5, Color::rgb(0xf0, 0xb4, 0x4a), Align::Right);
        }
    }

    fn pointer(&self, event: Pointer) -> bool {
        let Ok(mut view) = self.view.lock() else { return false };
        let (x0, y0, w, h) = SCREEN;
        let inside = |x: f32, y: f32| x >= x0 && x <= x0 + w && y >= y0 && y <= y0 + h;
        match event {
            Pointer::Move { x, y, .. } => {
                view.hover = inside(x, y).then_some((x, y));
                false
            }
            Pointer::Leave => {
                view.hover = None;
                false
            }
            // a click on the screen stops the picture, and the next starts it
            Pointer::Press { x, y, button: 0, .. } if inside(x, y) => {
                view.frozen = !view.frozen;
                true
            }
            _ => false,
        }
    }
}

pub struct Spectrum {
    shared: Arc<Shared>,
    screen: Arc<SpectrumScreen>,
    gain: f32,
    /// R has a cable (else it's L's).
    right: bool,
    /// The input's level, for the light (V, falling).
    level: f32,
    fall: f32,
}

impl Module for Spectrum {
    fn spec() -> Spec {
        Spec::new("native/Spectrum")
            .category("io")
            .vendor("Oroboro")
            .input("L")
            .input("R")
            .output("L")
            .output("R")
            .param(Param::new("Gain", -24.0, 24.0, 0.0).unit("dB"))
            .param(Param::new("Floor", -120.0, -24.0, -96.0).unit("dB"))
            .param(Param::new("Smooth", 0.0, 0.95, 0.7))
            .param(Param::choice("Scale", &["Log", "Lin"], 0))
            .param(Param::choice("Hold", &["Off", "On"], 0))
    }

    fn new(sample_rate: f32) -> Self {
        let shared = Arc::new(Shared {
            ring: (0..N).map(|_| AtomicU32::new(0)).collect(),
            written: AtomicUsize::new(0),
            sample_rate,
            floor: AtomicU32::new((-96.0f32).to_bits()),
            smooth: AtomicU32::new(0.7f32.to_bits()),
            log: AtomicU32::new(0.0f32.to_bits()),
            hold: AtomicU32::new(0.0f32.to_bits()),
        });
        let screen = Arc::new(SpectrumScreen { shared: Arc::clone(&shared), view: Mutex::new(View::new()) });
        // the light falls by half in about a tenth of a second
        let fall = 0.5f32.powf(1.0 / (0.1 * sample_rate));
        Spectrum { shared, screen, gain: 1.0, right: true, level: 0.0, fall }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            GAIN => self.gain = 10f32.powf(value / 20.0),
            FLOOR => Shared::set(&self.shared.floor, value.min(-24.0)),
            SMOOTH => Shared::set(&self.shared.smooth, value.clamp(0.0, 0.95)),
            SCALE => Shared::set(&self.shared.log, value),
            HOLD => Shared::set(&self.shared.hold, value),
            _ => {}
        }
    }

    fn connected(&mut self, inputs: &[bool]) {
        self.right = inputs.get(1).copied().unwrap_or(true);
    }

    fn tick(&mut self, inputs: &[f32], outputs: &mut [f32]) {
        let l = inputs[0];
        let r = if self.right { inputs[1] } else { l };
        outputs[0] = l;
        outputs[1] = r;
        let x = 0.5 * (l + r);
        let at = self.shared.written.load(Ordering::Relaxed);
        Shared::set(&self.shared.ring[at % N], x * self.gain);
        self.shared.written.store(at.wrapping_add(1), Ordering::Release);
        self.level = (self.level * self.fall).max(l.abs().max(r.abs()));
    }

    /// In the plugin's own look, as its own modules are: the screen under
    /// the name, the knobs at the plugin's size over their names, the jacks
    /// under theirs.
    fn panel() -> Option<Panel> {
        let (x0, y0, w, h) = SCREEN;
        Some(
            Panel::stock(255.0, 250.0)
                .screen(x0, y0, w, h)
                .knob(GAIN, 32.0, 178.0, MEDIUM)
                .label("Gain")
                .knob(FLOOR, 78.0, 178.0, MEDIUM)
                .label("Floor")
                .knob(SMOOTH, 124.0, 178.0, MEDIUM)
                .label("Smooth")
                .selector(SCALE, 178.0, 176.0, 40.0)
                .label("Scale")
                .selector(HOLD, 226.0, 176.0, 36.0)
                .label("Hold")
                .line(8.0, 206.0, 247.0, 206.0)
                .input(0, 26.0, 230.0)
                .label("In L")
                .input(1, 62.0, 230.0)
                .label("In R")
                .light(0, 128.0, 228.0, 6.0, &["#6dd68e", "#ed4c3c"])
                .text(128.0, 241.0, "Level", 7.5, true)
                .output(0, 193.0, 230.0)
                .label("Out L")
                .output(1, 229.0, 230.0)
                .label("Out R"),
        )
    }

    /// Its light: green with the input's level, red past 0 dB.
    fn lights(&self, out: &mut [f32]) -> usize {
        let db = 20.0 * (self.level / FULL + 1e-9).log10();
        if out.len() >= 2 {
            out[0] = ((db + 48.0) / 48.0).clamp(0.0, 1.0);
            out[1] = if db > 0.0 { 1.0 } else { 0.0 };
        }
        2
    }

    fn screen(&self) -> Option<Arc<dyn Screen>> {
        Some(self.screen.clone() as Arc<dyn Screen>)
    }
}

export_module!(Spectrum, panel);

#[cfg(test)]
mod tests {
    use super::*;

    fn played(f: f32, volts: f32) -> Spectrum {
        let rate = 48_000.0;
        let mut m = Spectrum::new(rate);
        m.connected(&[true, false]);
        let mut out = [0.0; 2];
        for n in 0..N {
            let x = volts * (2.0 * PI * f * n as f32 / rate).sin();
            m.tick(&[x, 0.0], &mut out);
            assert_eq!(out, [x, x], "through, R as L without a cable");
        }
        m
    }

    /// A sine of 5 V reads 0 dB at its frequency, and far less an octave
    /// away; the screen draws its grid, its curve and its fill.
    #[test]
    fn a_sine_reads_0_db_at_its_frequency() {
        let m = played(1000.0, 5.0);
        let screen = &m.screen;
        let mut view = screen.view.lock().unwrap();
        view.take(&screen.shared, 0.0, false);
        let at = |f: f32| screen.level(&view.shown, f, f);
        assert!(at(1000.0).abs() < 1.5, "{}", at(1000.0));
        assert!(at(2000.0) < -40.0 && at(500.0) < -40.0, "{} {}", at(2000.0), at(500.0));
        drop(view);
        let mut d = Drawing::new();
        screen.draw(&mut d);
        assert!(d.numbers().len() > 1000);
        // its light: green, not over
        let mut lights = [0.0; 2];
        assert_eq!(m.lights(&mut lights), 2);
        assert!(lights[0] > 0.9 && lights[1] == 0.0, "{lights:?}");
    }

    /// The pointer over the screen gets a readout; a click there freezes
    /// the picture and is the screen's, one outside it isn't.
    #[test]
    fn the_screen_takes_the_pointer() {
        let m = played(440.0, 1.0);
        let screen = &m.screen;
        assert!(!screen.pointer(Pointer::Move { x: 100.0, y: 80.0, dx: 0.0, dy: 0.0 }));
        assert_eq!(screen.view.lock().unwrap().hover, Some((100.0, 80.0)));
        assert!(screen.pointer(Pointer::Press { x: 100.0, y: 80.0, button: 0, mods: 0 }));
        assert!(screen.view.lock().unwrap().frozen);
        assert!(!screen.pointer(Pointer::Press { x: 100.0, y: 260.0, button: 0, mods: 0 }), "the panel's own: it moves the module");
        screen.pointer(Pointer::Leave);
        assert_eq!(screen.view.lock().unwrap().hover, None);
    }
}
