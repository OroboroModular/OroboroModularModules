//! ThroughZero: an oscillator made for through-zero FM. A signal at FM
//! moves its frequency up and down by the same number of hertz (linear
//! FM), and past zero it doesn't stop: the frequency turns negative and the
//! wave runs backwards. So deep FM keeps its pitch, as a DX7 operator's
//! does, instead of drifting sharp the way exponential FM makes it.
//!
//! A compiled module for Oroboro Modular (the SDK's docs/native-modules.md):
//!
//! ```text
//! oromod build . --out build --install
//! ```
//!
//! How it works: the note (V/Oct, Tune, Fine, and the Exp input's octaves)
//! gives a frequency f0; the FM input moves it to f0 * (1 + Index * FM / 5 V).
//! A phase runs at that frequency, forwards or backwards, and the four
//! waves are read from it: sine and triangle as they are, saw and square
//! with their steps smoothed (polyBLEP, the step's direction followed
//! when the wave runs backwards). In Lin mode the frequency stops at zero
//! instead (the classic linear FM of analogue oscillators). Sync restarts
//! the cycle on a rising edge. Signals are volts: ±5 V out, 1 V an octave in.

use std::f32::consts::TAU;

use oroboro_module::{export_module, Module, Param, Spec};

/// C4, the note 0 V is.
const C4: f32 = 261.625_58;

/// FM's input at full swing (an audio signal's peak): at Index 1 it moves
/// the frequency by f0 either way.
const FULL: f32 = 5.0;

/// The deepest Index.
const MAX_INDEX: f32 = 5.0;

/// Where Sync's rising edge is.
const HIGH: f32 = 1.0;

/// Knobs, in the spec's order.
const TUNE: usize = 0;
const FINE: usize = 1;
const INDEX: usize = 2;
const EXP: usize = 3;
const MODE: usize = 4;

/// Inputs, in the spec's order.
const IN_PITCH: usize = 0;
const IN_FM: usize = 1;
const IN_INDEX: usize = 2;
const IN_EXP: usize = 3;
const IN_SYNC: usize = 4;

pub struct ThroughZero {
    sample_rate: f32,
    /// Where the cycle is, 0 to 1.
    phase: f32,
    tune: f32,
    fine: f32,
    index: f32,
    exp: f32,
    /// Through zero (else the frequency stops at zero).
    through: bool,
    /// Sync was high the sample before.
    synced: bool,
}

/// What takes the step out of a wave's edge at phase 0: a polynomial over
/// the sample before and the sample after it (`t` the phase, `dt` its step
/// a sample, positive).
fn blep(t: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        0.0
    } else if t < dt {
        let x = t / dt;
        2.0 * x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + 2.0 * x + 1.0
    } else {
        0.0
    }
}

impl ThroughZero {
    /// The frequency this sample, in hertz: the note, moved by Exp, then
    /// by linear FM; negative when FM takes it through zero (Lin: held at
    /// zero), and never past 45% of the sample rate either way.
    fn frequency(&self, inputs: &[f32]) -> f32 {
        let octaves = inputs[IN_PITCH] + (self.tune + self.fine / 100.0) / 12.0 + self.exp * inputs[IN_EXP];
        let base = C4 * octaves.clamp(-12.0, 8.0).exp2();
        // 10 V at Index is the knob's whole way
        let index = (self.index + inputs[IN_INDEX] / 10.0 * MAX_INDEX).clamp(0.0, MAX_INDEX);
        let f = base * (1.0 + index * inputs[IN_FM] / FULL);
        let limit = 0.45 * self.sample_rate;
        match self.through {
            true => f.clamp(-limit, limit),
            false => f.clamp(0.0, limit),
        }
    }
}

impl Module for ThroughZero {
    fn spec() -> Spec {
        Spec::new("native/ThroughZero")
            .category("osc")
            .vendor("Oroboro")
            .input("V/Oct")
            .input("FM")
            .input("Index")
            .input("Exp")
            .input("Sync")
            .output("Sine")
            .output("Tri")
            .output("Saw")
            .output("Square")
            .param(Param::new("Tune", -48.0, 48.0, 0.0).unit("st"))
            .param(Param::new("Fine", -100.0, 100.0, 0.0).unit("ct"))
            .param(Param::new("Index", 0.0, MAX_INDEX, 1.0))
            .param(Param::new("Exp", 0.0, 1.0, 0.0))
            .param(Param::choice("Mode", &["TZ", "Lin"], 0))
    }

    fn new(sample_rate: f32) -> Self {
        ThroughZero { sample_rate, phase: 0.0, tune: 0.0, fine: 0.0, index: 1.0, exp: 0.0, through: true, synced: false }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            TUNE => self.tune = value,
            FINE => self.fine = value,
            INDEX => self.index = value,
            EXP => self.exp = value,
            MODE => self.through = value.round() < 0.5,
            _ => {}
        }
    }

    fn reset(&mut self) {
        (self.phase, self.synced) = (0.0, false);
    }

    fn tick(&mut self, inputs: &[f32], outputs: &mut [f32]) {
        // a rising edge at Sync restarts the cycle
        let high = inputs[IN_SYNC] >= HIGH;
        if high && !self.synced {
            self.phase = 0.0;
        }
        self.synced = high;

        let dt = self.frequency(inputs) / self.sample_rate;
        let (step, direction) = (dt.abs(), if dt < 0.0 { -1.0 } else { 1.0 });
        let t = self.phase;
        let sine = (TAU * t).sin();
        // in phase with the sine: 0 at the start, up first
        let tri = 4.0 * ((t + 0.75).fract() - 0.5).abs() - 1.0;
        // a saw steps down where the phase wraps going forwards, up going backwards
        let saw = 2.0 * t - 1.0 - direction * blep(t, step);
        let square = if t < 0.5 { 1.0 } else { -1.0 } + direction * (blep(t, step) - blep((t + 0.5).fract(), step));
        self.phase = (t + dt).rem_euclid(1.0);
        if !self.phase.is_finite() {
            self.phase = 0.0;
        }

        outputs[0] = 5.0 * sine;
        outputs[1] = 5.0 * tri;
        outputs[2] = 5.0 * saw;
        outputs[3] = 5.0 * square;
    }
}

export_module!(ThroughZero);

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    /// A second of it, with `inputs` at every sample: its four waves.
    fn play(osc: &mut ThroughZero, inputs: [f32; 5]) -> [Vec<f32>; 4] {
        let mut waves: [Vec<f32>; 4] = Default::default();
        let mut out = [0.0; 4];
        for _ in 0..RATE as usize {
            osc.tick(&inputs, &mut out);
            for (wave, x) in waves.iter_mut().zip(out) {
                wave.push(x);
            }
        }
        waves
    }

    /// How many times a second a wave climbs through zero.
    fn frequency(wave: &[f32]) -> f32 {
        let rising = wave.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        rising as f32 * RATE / wave.len() as f32
    }

    /// The share of samples a wave goes down from the one before.
    fn falling(wave: &[f32]) -> f32 {
        wave.windows(2).filter(|w| w[1] < w[0]).count() as f32 / (wave.len() - 1) as f32
    }

    #[test]
    fn it_plays_the_note() {
        let mut osc = ThroughZero::new(RATE);
        let [sine, tri, saw, square] = play(&mut osc, [0.0; 5]);
        for wave in [&sine, &tri, &saw, &square] {
            assert!((frequency(wave) - C4).abs() < 1.5, "0 V is C4: {}", frequency(wave));
            assert!(wave.iter().all(|x| x.abs() <= 5.6), "±5 V");
        }
        // a volt up is an octave up
        let [sine, ..] = play(&mut osc, [1.0, 0.0, 0.0, 0.0, 0.0]);
        assert!((frequency(&sine) - 2.0 * C4).abs() < 1.5, "{}", frequency(&sine));
        // a forward saw rises, but where it steps down
        assert!(falling(&saw) < 0.05, "{}", falling(&saw));
    }

    /// Past zero the wave runs backwards at the frequency it's taken to
    /// (a saw that falls); in Lin mode it stops there.
    #[test]
    fn linear_fm_goes_through_zero() {
        let mut osc = ThroughZero::new(RATE);
        // Index 1 and -10 V at FM: f0 * (1 - 2) = -f0
        let [sine, _, saw, _] = play(&mut osc, [0.0, -10.0, 0.0, 0.0, 0.0]);
        assert!((frequency(&sine) - C4).abs() < 1.5, "as fast as forwards: {}", frequency(&sine));
        assert!(falling(&saw) > 0.95, "backwards, the saw falls: {}", falling(&saw));
        // the same at -5 V is zero hertz
        let [sine, ..] = play(&mut osc, [0.0, -5.0, 0.0, 0.0, 0.0]);
        assert!(frequency(&sine) < 1.5, "{}", frequency(&sine));
        // Lin: held at zero
        osc.set_param(MODE, 1.0);
        let [sine, ..] = play(&mut osc, [0.0, -10.0, 0.0, 0.0, 0.0]);
        let first = sine[0];
        assert!(sine.iter().all(|x| (x - first).abs() < 1e-4), "it stops");
    }

    /// Deep FM by a modulator at the note's own frequency keeps the pitch:
    /// the wave's period stays the note's (zero crossings at f0 and its
    /// multiples only).
    #[test]
    fn deep_fm_keeps_the_pitch() {
        // 300 Hz: a period of 160 samples exactly
        let (note, period) = (300.0, 160);
        let pitch = (note / C4).log2();
        let mut osc = ThroughZero::new(RATE);
        osc.set_param(INDEX, 3.0);
        let mut out = [0.0; 4];
        let mut cycle = Vec::new();
        for n in 0..(period * 40) {
            let modulator = 5.0 * (TAU * note * n as f32 / RATE).sin();
            osc.tick(&[pitch, modulator, 0.0, 0.0, 0.0], &mut out);
            cycle.push(out[0]);
            assert!(out.iter().all(|x| x.is_finite()));
        }
        // the last cycles repeat each other: periodic at the note
        let n = cycle.len();
        let difference: f32 = (n - 4 * period..n).map(|k| (cycle[k] - cycle[k - period]).abs()).sum::<f32>() / (4 * period) as f32;
        assert!(difference < 0.05, "periodic at the note's frequency: {difference}");
    }

    #[test]
    fn sync_restarts_the_cycle() {
        let mut osc = ThroughZero::new(RATE);
        let mut out = [0.0; 4];
        for _ in 0..1000 {
            osc.tick(&[0.0; 5], &mut out);
        }
        osc.tick(&[0.0, 0.0, 0.0, 0.0, 5.0], &mut out);
        assert!(out[0].abs() < 1e-3, "the sine starts again at zero: {}", out[0]);
        // held high: no more restarts
        osc.tick(&[0.0, 0.0, 0.0, 0.0, 5.0], &mut out);
        assert!(out[0] > 0.0);
    }

    #[test]
    fn wild_inputs_stay_finite() {
        let mut osc = ThroughZero::new(RATE);
        osc.set_param(INDEX, MAX_INDEX);
        osc.set_param(EXP, 1.0);
        let mut out = [0.0; 4];
        for n in 0..48_000 {
            let x = if n % 2 == 0 { 1e6 } else { -1e6 };
            osc.tick(&[x, x, x, x, x], &mut out);
            assert!(out.iter().all(|y| y.is_finite() && y.abs() <= 6.0), "{out:?}");
        }
    }
}
