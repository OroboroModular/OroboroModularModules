//! PitchDelay: a delay whose echoes are transposed. Each repeat comes back
//! Pitch semitones higher (or lower) than the last, so with Feedback a
//! note climbs (or falls) in a trail of echoes: up a fifth and up again, or
//! down an octave into the dark. Time is the delay, Fine tunes the step,
//! Tone darkens what goes round, Mix sets dry against wet. Stereo: L and R
//! in (R takes L's sound while nothing is cabled into it), each side
//! shifted on its own, L and R out, and each side's echoes alone. Pitch's
//! input moves the step an octave a volt; the knobs take cables, each with
//! its own amount (a cable into Time sweeps it).
//!
//! A compiled module for Oroboro Modular (the SDK's docs/native-modules.md):
//!
//! ```text
//! oromod build . --out build --install
//! ```
//!
//! How it works: what comes in, and what goes round, is written into a
//! delay line. Two heads read it, each at a delay that slides across a
//! window (50 ms, or Time where that's shorter) at the speed that plays the
//! sound back at the pitch's ratio, and jumps back when it reaches the
//! window's end. The heads are half a window apart and fade in and out
//! (sin², which add up to one), so each jump is where its head is silent.
//! The echo is the two heads together, a window's half later than Time.
//! Reads between samples are cubic. Time glides to a new value, so turning
//! it doesn't jump. The two sides' heads move together. Signals are volts.

use std::f32::consts::PI;

use oroboro_module::{export_module, Module, Param, Spec};

/// Time's range, seconds.
const MIN_TIME: f32 = 0.02;
const MAX_TIME: f32 = 2.0;

/// The shifter's window, seconds (shorter where Time is).
const WINDOW: f32 = 0.05;

/// How fast Time glides to a new value: this share of the way a sample.
const GLIDE: f32 = 0.0005;

/// Knobs, in the spec's order.
const TIME: usize = 0;
const FEEDBACK: usize = 1;
const PITCH: usize = 2;
const FINE: usize = 3;
const TONE: usize = 4;
const MIX: usize = 5;

/// Inputs, in the spec's order: the two sides, and Pitch.
const L: usize = 0;
const R: usize = 1;
const IN_PITCH: usize = 2;
/// Outputs: the two sides, then each side's echoes alone.
const WET: usize = 2;

/// What goes round kept within ±10 V: as it is up to 8 V, then gently
/// limited.
fn soft(x: f32) -> f32 {
    match x.abs() {
        a if a <= 8.0 => x,
        a => x.signum() * (8.0 + 2.0 * ((a - 8.0) / 2.0).tanh()),
    }
}

pub struct PitchDelay {
    sample_rate: f32,
    /// Each side's delay line.
    buffers: [Vec<f32>; 2],
    /// Where the next sample goes.
    write: usize,
    /// The first head's place in the window, 0 to 1 (the second's is half
    /// a window on).
    phase: f32,
    /// The delay as it glides, in samples.
    delay: f32,
    time: f32,
    feedback: f32,
    pitch: f32,
    fine: f32,
    tone: f32,
    mix: f32,
    /// Each side's tone low-pass.
    low: [f32; 2],
    /// Whether R has a cable: without one it takes L's sound.
    r_patched: bool,
}

impl PitchDelay {
    /// Side `side`'s delay line `delay` samples back, between samples cubic.
    fn read(&self, side: usize, delay: f32) -> f32 {
        let buffer = &self.buffers[side];
        let n = buffer.len();
        let at = self.write as f32 - 1.0 - delay;
        let at = at.rem_euclid(n as f32);
        let i = at.floor() as usize;
        let t = at - at.floor();
        let x = |k: isize| buffer[(i as isize + k).rem_euclid(n as isize) as usize];
        let (y0, y1, y2, y3) = (x(-1), x(0), x(1), x(2));
        // Catmull-Rom
        let a = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
        let b = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
        let c = -0.5 * y0 + 0.5 * y2;
        ((a * t + b) * t + c) * t + y1
    }

    /// The delay Time asks for, in samples.
    fn target(&self) -> f32 {
        self.time * self.sample_rate
    }

    /// The tone's low-pass coefficient: 200 Hz to fully open.
    fn tone_coefficient(&self) -> f32 {
        if self.tone >= 0.999 {
            return 1.0;
        }
        let cutoff = (200.0 * (7.0 * self.tone.clamp(0.0, 1.0)).exp2()).min(0.45 * self.sample_rate);
        1.0 - (-2.0 * PI * cutoff / self.sample_rate).exp()
    }
}

impl Module for PitchDelay {
    fn spec() -> Spec {
        Spec::new("native/PitchDelay")
            .category("fx")
            .vendor("Oroboro")
            .input("L")
            .input("R")
            .input("Pitch")
            .output("L")
            .output("R")
            .output("Wet L")
            .output("Wet R")
            .param(Param::new("Time", MIN_TIME, MAX_TIME, 0.35).unit("s").exp())
            .param(Param::new("Feedback", 0.0, 0.95, 0.5))
            .param(Param::new("Pitch", -24.0, 24.0, 7.0).unit("st").steps())
            .param(Param::new("Fine", -100.0, 100.0, 0.0).unit("ct"))
            .param(Param::new("Tone", 0.0, 1.0, 0.7))
            .param(Param::new("Mix", 0.0, 1.0, 0.5))
    }

    fn new(sample_rate: f32) -> Self {
        let samples = ((MAX_TIME + WINDOW) * sample_rate) as usize + 8;
        PitchDelay {
            sample_rate,
            buffers: [vec![0.0; samples], vec![0.0; samples]],
            write: 0,
            phase: 0.0,
            delay: 0.35 * sample_rate,
            time: 0.35,
            feedback: 0.5,
            pitch: 7.0,
            fine: 0.0,
            tone: 0.7,
            mix: 0.5,
            low: [0.0; 2],
            r_patched: false,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            TIME => self.time = value.clamp(MIN_TIME, MAX_TIME),
            FEEDBACK => self.feedback = value,
            PITCH => self.pitch = value,
            FINE => self.fine = value,
            TONE => self.tone = value,
            MIX => self.mix = value,
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.buffers.iter_mut().for_each(|b| b.fill(0.0));
        (self.low, self.phase) = ([0.0; 2], 0.0);
        self.delay = self.target();
    }

    fn connected(&mut self, inputs: &[bool]) {
        self.r_patched = inputs.get(R).copied().unwrap_or(false);
    }

    fn tick(&mut self, inputs: &[f32], outputs: &mut [f32]) {
        let input = [inputs[L], if self.r_patched { inputs[R] } else { inputs[L] }];
        // Pitch's input is a volt an octave
        let semitones = (self.pitch + self.fine / 100.0 + 12.0 * inputs[IN_PITCH]).clamp(-48.0, 48.0);
        let ratio = (semitones / 12.0).exp2();
        self.delay += GLIDE * (self.target() - self.delay);

        // the heads slide across the window at the speed that plays at `ratio`
        let window = (WINDOW * self.sample_rate).min(self.delay).max(16.0);
        self.phase = (self.phase + (1.0 - ratio) / window).rem_euclid(1.0);
        let other = (self.phase + 0.5).fract();
        let tone = self.tone_coefficient();
        for side in [L, R] {
            let head = |phase: f32| {
                let w = (PI * phase).sin();
                self.read(side, self.delay + window * phase) * w * w
            };
            let shifted = head(self.phase) + head(other);
            self.low[side] += tone * (shifted - self.low[side]);
            let wet = self.low[side];
            self.buffers[side][self.write] = soft(input[side] + self.feedback * wet);
            outputs[side] = input[side] * (1.0 - self.mix) + wet * self.mix;
            outputs[WET + side] = wet;
        }
        self.write = (self.write + 1) % self.buffers[L].len();
    }
}

export_module!(PitchDelay);

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn delay(time: f32, pitch: f32, feedback: f32) -> PitchDelay {
        let mut d = PitchDelay::new(RATE);
        for (k, v) in [(TIME, time), (PITCH, pitch), (FEEDBACK, feedback), (TONE, 1.0), (MIX, 1.0)] {
            d.set_param(k, v);
        }
        d.reset();
        d
    }

    /// `samples` of it fed `input(n)` at L (R unpatched): its Wet L.
    fn play(d: &mut PitchDelay, samples: usize, mut input: impl FnMut(usize) -> f32) -> Vec<f32> {
        let mut o = [0.0; 4];
        (0..samples)
            .map(|n| {
                d.tick(&[input(n), 0.0, 0.0], &mut o);
                o[WET + L]
            })
            .collect()
    }

    /// Stereo: without a cable at R, both sides play L's sound; with one,
    /// each side echoes its own.
    #[test]
    fn its_two_sides() {
        let tone = |n: usize| 2.0 * (2.0 * PI * 220.0 * n as f32 / RATE).sin();
        let mut d = delay(0.1, 7.0, 0.3);
        let mut o = [0.0; 4];
        let mut both = Vec::new();
        for n in 0..20_000 {
            d.tick(&[tone(n), 0.0, 0.0], &mut o);
            both.push((o[L], o[R]));
        }
        assert!(both.iter().all(|(l, r)| l == r) && both.iter().any(|(l, _)| l.abs() > 0.1), "R as L");
        let mut d = delay(0.1, 7.0, 0.3);
        d.connected(&[true, true, false]);
        let mut sides = Vec::new();
        for n in 0..20_000 {
            d.tick(&[tone(n), 0.0, 0.0], &mut o);
            sides.push((o[L], o[R]));
        }
        assert!(sides.iter().all(|(_, r)| *r == 0.0) && sides.iter().any(|(l, _)| l.abs() > 0.1), "R its own: silent");
    }

    /// The power of `x` at `frequency` (Goertzel).
    fn power(x: &[f32], frequency: f32) -> f32 {
        let w = 2.0 * PI * frequency / RATE;
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &v in x {
            let s = v + 2.0 * w.cos() * s1 - s2;
            (s2, s1) = (s1, s);
        }
        (s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2) / (x.len() as f32).powi(2)
    }

    /// The power of `x` within 30 Hz of `frequency`: a shifter's output has
    /// sidebands at its window's rate around the tone.
    fn band(x: &[f32], frequency: f32) -> f32 {
        (-30..=30).map(|d| power(x, frequency + d as f32)).sum()
    }

    /// At no pitch it's a plain delay: a click comes back after Time and
    /// half a window.
    #[test]
    fn unshifted_its_a_delay() {
        let mut d = delay(0.1, 0.0, 0.0);
        let wet = play(&mut d, 9000, |n| if n == 100 { 1.0 } else { 0.0 });
        let at = (0..wet.len()).max_by(|&a, &b| wet[a].abs().total_cmp(&wet[b].abs())).unwrap();
        let expected = 100 + (0.1 * RATE) as usize + (WINDOW * RATE / 2.0) as usize;
        assert!(at.abs_diff(expected) <= 2, "at {at}, not {expected}");
        assert!((wet[at] - 1.0).abs() < 0.05, "whole: {}", wet[at]);
    }

    /// An octave up or down: a 220 Hz tone comes back at 440 or 110.
    #[test]
    fn echoes_are_transposed() {
        let tone = |n: usize| 2.0 * (2.0 * PI * 220.0 * n as f32 / RATE).sin();
        for (pitch, heard) in [(12.0, 440.0), (-12.0, 110.0), (7.0, 220.0 * 1.498_307)] {
            let mut d = delay(0.1, pitch, 0.0);
            let wet = play(&mut d, 48_000 * 2, tone);
            let settled = &wet[24_000..];
            let rms = (settled.iter().map(|x| x * x).sum::<f32>() / settled.len() as f32).sqrt();
            assert!(rms > 0.7, "{pitch} st: not much quieter than it came (1.41 V RMS): {rms}");
            let (there, source) = (band(settled, heard), band(settled, 220.0));
            assert!(there > 20.0 * source, "{pitch} st: {there} at {heard} Hz, {source} at 220");
        }
    }

    /// With feedback each repeat goes a step further: a fifth, then a ninth.
    #[test]
    fn feedback_climbs() {
        let mut d = delay(0.2, 7.0, 0.8);
        let burst = |n: usize| if n < 4800 { 2.0 * (2.0 * PI * 220.0 * n as f32 / RATE).sin() } else { 0.0 };
        let wet = play(&mut d, 48_000 * 2, burst);
        // the first repeat (about 0.2 s on) a fifth up, the second (0.4 s on) a ninth
        let first = &wet[(0.235 * RATE) as usize..(0.3 * RATE) as usize];
        let second = &wet[(0.46 * RATE) as usize..(0.52 * RATE) as usize];
        let fifth = 220.0 * 1.498_307;
        let ninth = fifth * 1.498_307;
        assert!(band(first, fifth) > 10.0 * band(first, ninth), "first: a fifth");
        assert!(band(second, ninth) > 10.0 * band(second, fifth), "second: a ninth");
    }

    #[test]
    fn it_stays_bounded() {
        let mut d = delay(0.02, -24.0, 0.95);
        d.set_param(TONE, 0.3);
        let mut seed = 3u32;
        let wet = play(&mut d, 48_000 * 4, |_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed as f32 / u32::MAX as f32 - 0.5) * 10.0
        });
        assert!(wet.iter().all(|x| x.is_finite() && x.abs() < 25.0));
    }
}
