//! ReverseDelay: echoes that play backwards. What comes in is heard again
//! a moment later, reversed: a note swells in from its tail and ends on
//! its attack. Time is how long a reversed stretch is; Feedback sends the
//! reversed echo round again (so the next echo is forwards again,
//! reversed twice), Tone darkens it, Mix sets dry against wet. Stereo: L
//! and R in (R takes L's sound while nothing is cabled into it), each side
//! reversed on its own, L and R out, and each side's echo alone. Its knobs
//! take cables, each with its own amount (a cable into Time sweeps it).
//!
//! A compiled module for Oroboro Modular (the SDK's docs/native-modules.md):
//!
//! ```text
//! oromod build . --out build --install
//! ```
//!
//! How it works: what comes in is written into a delay line. Two heads read
//! it backwards, each over a stretch Time long that it starts at the newest
//! sample (so a head reads the last Time of sound, from its end to its
//! start, while that much new sound comes in). The heads are half a stretch
//! apart and fade in and out (sin², which add up to one), so there's no
//! click where one starts again. When Time changes, a head takes the new
//! length when it starts its next stretch, and the second head a stretch
//! that brings it back to halfway: nothing jumps. The two sides' heads
//! move together. Signals are volts.

use std::f32::consts::PI;

use oroboro_module::{export_module, Module, Param, Spec};

/// Time's range, seconds.
const MIN_TIME: f32 = 0.05;
const MAX_TIME: f32 = 2.0;

/// Knobs, in the spec's order.
const TIME: usize = 0;
const FEEDBACK: usize = 1;
const TONE: usize = 2;
const MIX: usize = 3;

/// Inputs and outputs, in the spec's order: the two sides, then (out)
/// each side's echo alone.
const L: usize = 0;
const R: usize = 1;
const WET: usize = 2;

/// A head reading backwards: from `start` (the newest sample when its
/// stretch began), `count` samples into a stretch `length` long.
#[derive(Clone, Copy)]
struct Head {
    start: usize,
    count: usize,
    length: usize,
}

impl Head {
    /// Its sample, faded by where it is in its stretch.
    fn read(&self, buffer: &[f32]) -> f32 {
        let n = buffer.len();
        let x = buffer[(self.start + n - self.count % n) % n];
        let w = (PI * self.count as f32 / self.length as f32).sin();
        x * w * w
    }

    fn advance(&mut self) {
        self.count += 1;
        if self.count >= self.length {
            self.count = 0;
        }
    }
}

/// A delay line read backwards by two heads, half a stretch apart.
pub struct Reverser {
    buffer: Vec<f32>,
    /// Where the next sample goes.
    write: usize,
    a: Head,
    b: Head,
}

impl Reverser {
    /// One for stretches up to `longest` samples.
    pub fn new(longest: usize) -> Self {
        let longest = longest.max(4);
        // a head's stretch reaches 2 stretches back; the second head's can be a quarter longer
        let buffer = vec![0.0; 3 * longest + 8];
        // (both start at their first read: the second with the stretch that
        // brings it halfway along the first's)
        let a = Head { start: 0, count: 0, length: longest };
        let b = Head { start: 0, count: 0, length: longest };
        Reverser { buffer, write: 0, a, b }
    }

    /// The reversed sound this sample, with stretches `length` long from
    /// the next one a head starts.
    pub fn read(&mut self, length: usize) -> f32 {
        let n = self.buffer.len();
        let length = length.clamp(4, (n - 8) / 3);
        let newest = (self.write + n - 1) % n;
        if self.a.count == 0 {
            (self.a.start, self.a.length) = (newest, length);
        }
        if self.b.count == 0 {
            // a stretch that ends where the first head is halfway
            let to_middle = (self.a.length / 2 + self.a.length - self.a.count) % self.a.length;
            let stretch = if to_middle < self.a.length / 4 { to_middle + self.a.length } else { to_middle };
            (self.b.start, self.b.length) = (newest, stretch.max(4));
        }
        let x = self.a.read(&self.buffer) + self.b.read(&self.buffer);
        self.a.advance();
        self.b.advance();
        x
    }

    pub fn write(&mut self, x: f32) {
        self.buffer[self.write] = x;
        self.write = (self.write + 1) % self.buffer.len();
    }

    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
    }
}

/// What goes round kept within ±10 V: as it is up to 8 V, then gently
/// limited.
fn soft(x: f32) -> f32 {
    match x.abs() {
        a if a <= 8.0 => x,
        a => x.signum() * (8.0 + 2.0 * ((a - 8.0) / 2.0).tanh()),
    }
}

pub struct ReverseDelay {
    sample_rate: f32,
    /// Each side's.
    reversers: [Reverser; 2],
    time: f32,
    feedback: f32,
    tone: f32,
    mix: f32,
    /// Each side's tone low-pass.
    low: [f32; 2],
    /// Whether R has a cable: without one it takes L's sound.
    r_patched: bool,
}

impl ReverseDelay {
    /// A stretch's length in samples.
    fn length(&self) -> usize {
        (self.time * self.sample_rate) as usize
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

impl Module for ReverseDelay {
    fn spec() -> Spec {
        Spec::new("native/ReverseDelay")
            .category("fx")
            .vendor("Oroboro")
            .input("L")
            .input("R")
            .output("L")
            .output("R")
            .output("Wet L")
            .output("Wet R")
            .param(Param::new("Time", MIN_TIME, MAX_TIME, 0.5).unit("s").exp())
            .param(Param::new("Feedback", 0.0, 0.95, 0.3))
            .param(Param::new("Tone", 0.0, 1.0, 0.8))
            .param(Param::new("Mix", 0.0, 1.0, 0.5))
    }

    fn new(sample_rate: f32) -> Self {
        let longest = (MAX_TIME * sample_rate) as usize;
        let reversers = [Reverser::new(longest), Reverser::new(longest)];
        ReverseDelay { sample_rate, reversers, time: 0.5, feedback: 0.3, tone: 0.8, mix: 0.5, low: [0.0; 2], r_patched: false }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            TIME => self.time = value.clamp(MIN_TIME, MAX_TIME),
            FEEDBACK => self.feedback = value,
            TONE => self.tone = value,
            MIX => self.mix = value,
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.reversers.iter_mut().for_each(Reverser::clear);
        self.low = [0.0; 2];
    }

    fn connected(&mut self, inputs: &[bool]) {
        self.r_patched = inputs.get(R).copied().unwrap_or(false);
    }

    fn tick(&mut self, inputs: &[f32], outputs: &mut [f32]) {
        let input = [inputs[L], if self.r_patched { inputs[R] } else { inputs[L] }];
        let (length, tone) = (self.length(), self.tone_coefficient());
        for side in [L, R] {
            let reversed = self.reversers[side].read(length);
            self.low[side] += tone * (reversed - self.low[side]);
            let wet = self.low[side];
            self.reversers[side].write(soft(input[side] + self.feedback * wet));
            outputs[side] = input[side] * (1.0 - self.mix) + wet * self.mix;
            outputs[WET + side] = wet;
        }
    }
}

export_module!(ReverseDelay);

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn delay(time: f32, feedback: f32, mix: f32) -> ReverseDelay {
        let mut d = ReverseDelay::new(RATE);
        for (k, v) in [(TIME, time), (FEEDBACK, feedback), (TONE, 1.0), (MIX, mix)] {
            d.set_param(k, v);
        }
        d
    }

    /// `samples` of it fed `input(n)` at L (R unpatched): its L and Wet L.
    fn play(d: &mut ReverseDelay, samples: usize, mut input: impl FnMut(usize) -> f32) -> (Vec<f32>, Vec<f32>) {
        let (mut out, mut wet) = (Vec::new(), Vec::new());
        let mut o = [0.0; 4];
        for n in 0..samples {
            d.tick(&[input(n), 0.0], &mut o);
            out.push(o[L]);
            wet.push(o[WET + L]);
        }
        (out, wet)
    }

    /// Stereo: without a cable at R, both sides play L's sound; with one,
    /// each side echoes its own.
    #[test]
    fn its_two_sides() {
        let mut d = delay(0.1, 0.0, 1.0);
        let mut o = [0.0; 4];
        let tone = |n: usize| (n as f32 * 0.01).sin();
        let mut both = Vec::new();
        for n in 0..20_000 {
            d.tick(&[tone(n), 0.0], &mut o);
            both.push((o[L], o[R]));
        }
        assert!(both.iter().all(|(l, r)| l == r) && both.iter().any(|(l, _)| l.abs() > 0.1), "R as L");
        let mut d = delay(0.1, 0.0, 1.0);
        d.connected(&[true, true]);
        let mut sides = Vec::new();
        for n in 0..20_000 {
            d.tick(&[tone(n), 0.0], &mut o);
            sides.push((o[L], o[R]));
        }
        assert!(sides.iter().all(|(_, r)| *r == 0.0) && sides.iter().any(|(l, _)| l.abs() > 0.1), "R its own: silent");
    }

    /// Two clicks, a loud one and then a soft one: the echo plays the soft
    /// one first, then the loud one as far after it as they were apart.
    #[test]
    fn echoes_play_backwards() {
        let mut d = delay(0.1, 0.0, 1.0);
        let (loud, soft) = (1000, 1100);
        let (_, wet) = play(&mut d, 3 * 4800, |n| match n {
            n if n == loud => 1.0,
            n if n == soft => 0.5,
            _ => 0.0,
        });
        let first = (soft + 1..wet.len()).find(|&n| wet[n].abs() > 0.01).expect("an echo");
        let later = first + (soft - loud);
        assert!(wet[first + 1..later - 1].iter().all(|x| x.abs() < 0.01), "nothing between them");
        let ratio = wet[later] / wet[first];
        assert!((1.4..3.0).contains(&ratio), "the soft click first, then the loud one: {} then {}", wet[first], wet[later]);
    }

    /// The heads' fades add up to one: a steady signal comes back steady.
    #[test]
    fn a_steady_signal_comes_back_steady() {
        let mut d = delay(0.2, 0.0, 1.0);
        let (_, wet) = play(&mut d, 4 * 9600, |_| 2.0);
        assert!(wet[3 * 9600..].iter().all(|x| (x - 2.0).abs() < 1e-3), "{:?}", &wet[3 * 9600..3 * 9600 + 4]);
    }

    /// A new Time comes in without a jump, and in time the echo is steady again.
    #[test]
    fn time_changes_smoothly() {
        let mut d = delay(0.2, 0.0, 1.0);
        let (_, before) = play(&mut d, 4 * 9600, |_| 2.0);
        d.set_param(TIME, 0.7);
        let (_, after) = play(&mut d, 6 * 33_600, |_| 2.0);
        let all: Vec<f32> = before.into_iter().chain(after.iter().copied()).collect();
        let jump = all.windows(2).skip(3 * 9600).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
        assert!(jump < 0.05, "no step: {jump}");
        assert!(after[after.len() - 1000..].iter().all(|x| (x - 2.0).abs() < 1e-3));
    }

    #[test]
    fn mix_and_feedback() {
        // dry alone is what comes in
        let mut d = delay(0.1, 0.5, 0.0);
        let (out, _) = play(&mut d, 2000, |n| (n as f32 * 0.01).sin());
        assert!(out.iter().enumerate().all(|(n, x)| (x - (n as f32 * 0.01).sin()).abs() < 1e-6));
        // feedback at its most, fed noise: it stays within bounds
        let mut d = delay(0.05, 0.95, 1.0);
        let mut seed = 1u32;
        let (out, _) = play(&mut d, 48_000 * 4, |_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed as f32 / u32::MAX as f32 - 0.5) * 10.0
        });
        assert!(out.iter().all(|x| x.is_finite() && x.abs() < 20.0));
    }
}
