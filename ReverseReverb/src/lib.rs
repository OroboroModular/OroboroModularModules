//! ReverseReverb: a reverb heard backwards. A sound's tail swells up out of
//! nothing and stops where the sound itself is, the effect of a reversed
//! tape of a reverb, here as it plays. Length is how long a reversed swell
//! is; Decay how long the reverb rings; Damp darkens it; Mix sets dry
//! against wet. Stereo: L and R in (R takes L's sound while nothing is
//! cabled into it), L and R out. Its knobs take cables, each with its own
//! amount.
//!
//! A compiled module for Oroboro Modular (the SDK's docs/native-modules.md):
//!
//! ```text
//! oromod build . --out build --install
//! ```
//!
//! How it works: each side goes through two allpass diffusers of its own
//! into a feedback delay network (four delay lines, mixed by a Hadamard
//! matrix, each damped by a low-pass and scaled for Decay's time to fall
//! 60 dB): the left into the two lines that make the left, the right into
//! the two that make the right, and the matrix spreads each to the other. Each side's reverb then
//! goes into a reverser: a delay line read backwards by two heads, each
//! over a stretch Length long that it starts at the newest sample, half a
//! stretch apart and faded in and out (sin², which add up to one). So each
//! stretch of reverb is heard from its quiet end to its loud start: the
//! swell. Signals are volts.

use std::f32::consts::PI;

use oroboro_module::{export_module, Module, Param, Spec};

/// Length's range, seconds.
const MIN_LENGTH: f32 = 0.1;
const MAX_LENGTH: f32 = 2.0;

/// The delay lines, milliseconds: lengths without a common measure, so the
/// echoes never line up.
const LINES_MS: [f32; 4] = [29.7, 37.1, 41.1, 43.7];
/// The diffusers, milliseconds, and how much each feeds back.
const DIFFUSERS_MS: [f32; 2] = [4.77, 1.73];
const DIFFUSION: f32 = 0.6;

/// How loud the reverb comes out against what goes in.
const LEVEL: f32 = 0.6;

/// Knobs, in the spec's order.
const LENGTH: usize = 0;
const DECAY: usize = 1;
const DAMP: usize = 2;
const MIX: usize = 3;

/// Inputs and outputs, in the spec's order.
const L: usize = 0;
const R: usize = 1;

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

/// A delay line read backwards by two heads, half a stretch apart. When
/// the length changes, a head takes it as it starts its next stretch, and
/// the second one a stretch that brings it back to halfway: nothing jumps.
struct Reverser {
    buffer: Vec<f32>,
    write: usize,
    a: Head,
    b: Head,
}

impl Reverser {
    fn new(longest: usize) -> Self {
        let longest = longest.max(4);
        // a head's stretch reaches 2 stretches back; the second head's can be a quarter longer
        let head = Head { start: 0, count: 0, length: longest };
        Reverser { buffer: vec![0.0; 3 * longest + 8], write: 0, a: head, b: head }
    }

    /// The reversed sound this sample, with stretches `length` long from
    /// the next one a head starts.
    fn read(&mut self, length: usize) -> f32 {
        let n = self.buffer.len();
        let length = length.clamp(4, (n - 8) / 3);
        let newest = (self.write + n - 1) % n;
        if self.a.count == 0 {
            (self.a.start, self.a.length) = (newest, length);
        }
        if self.b.count == 0 {
            let to_middle = (self.a.length / 2 + self.a.length - self.a.count) % self.a.length;
            let stretch = if to_middle < self.a.length / 4 { to_middle + self.a.length } else { to_middle };
            (self.b.start, self.b.length) = (newest, stretch.max(4));
        }
        let x = self.a.read(&self.buffer) + self.b.read(&self.buffer);
        self.a.advance();
        self.b.advance();
        x
    }

    fn write(&mut self, x: f32) {
        self.buffer[self.write] = x;
        self.write = (self.write + 1) % self.buffer.len();
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
    }
}

/// A delay line of a fixed number of samples.
struct Line {
    buffer: Vec<f32>,
    at: usize,
}

impl Line {
    fn new(samples: usize) -> Self {
        Line { buffer: vec![0.0; samples.max(1)], at: 0 }
    }

    /// What went in `len` samples ago (the oldest sample).
    fn oldest(&self) -> f32 {
        self.buffer[self.at]
    }

    fn push(&mut self, x: f32) {
        self.buffer[self.at] = x;
        self.at = (self.at + 1) % self.buffer.len();
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
    }
}

/// A Schroeder allpass: it smears without colouring.
struct Diffuser {
    line: Line,
}

impl Diffuser {
    fn tick(&mut self, x: f32) -> f32 {
        let delayed = self.line.oldest();
        let v = x + DIFFUSION * delayed;
        self.line.push(v);
        delayed - DIFFUSION * v
    }
}

pub struct ReverseReverb {
    sample_rate: f32,
    /// Each side's.
    diffusers: [[Diffuser; 2]; 2],
    lines: [Line; 4],
    /// Each line's damping low-pass.
    lows: [f32; 4],
    sides: [Reverser; 2],
    length: f32,
    decay: f32,
    damp: f32,
    mix: f32,
    /// Whether R has a cable: without one it takes L's sound.
    r_patched: bool,
}

impl ReverseReverb {
    /// A stretch's length in samples.
    fn stretch(&self) -> usize {
        (self.length * self.sample_rate) as usize
    }

    /// The forward reverb this sample of `left` and `right`: left and right.
    fn reverb(&mut self, left: f32, right: f32) -> (f32, f32) {
        let [l, r] = &mut self.diffusers;
        let left = l.iter_mut().fold(left, |x, d| d.tick(x));
        let right = r.iter_mut().fold(right, |x, d| d.tick(x));
        // (lines 0 and 2 make the left, 1 and 3 the right)
        let fed = [left, right, left, right];
        let damping = 1.0 - 0.9 * self.damp.clamp(0.0, 1.0);
        let mut outs = [0.0; 4];
        for (k, line) in self.lines.iter().enumerate() {
            self.lows[k] += damping * (line.oldest() - self.lows[k]);
            outs[k] = self.lows[k];
        }
        // Hadamard: every line feeds every other, the energy kept
        let [a, b, c, d] = outs;
        let mixed = [a + b + c + d, a - b + c - d, a + b - c - d, a - b - c + d].map(|m| 0.5 * m);
        for (k, line) in self.lines.iter_mut().enumerate() {
            // so that a line's sound falls 60 dB in Decay's time
            let seconds = line.buffer.len() as f32 / self.sample_rate;
            let gain = 10f32.powf(-3.0 * seconds / self.decay.max(0.1));
            line.push(fed[k] + gain * mixed[k]);
        }
        (LEVEL * (outs[0] + outs[2]), LEVEL * (outs[1] + outs[3]))
    }
}

impl Module for ReverseReverb {
    fn spec() -> Spec {
        Spec::new("native/ReverseReverb")
            .category("fx")
            .vendor("Oroboro")
            .input("L")
            .input("R")
            .output("L")
            .output("R")
            .param(Param::new("Length", MIN_LENGTH, MAX_LENGTH, 0.8).unit("s").exp())
            .param(Param::new("Decay", 0.3, 10.0, 3.0).unit("s").exp())
            .param(Param::new("Damp", 0.0, 1.0, 0.4))
            .param(Param::new("Mix", 0.0, 1.0, 0.6))
    }

    fn new(sample_rate: f32) -> Self {
        let samples = |ms: f32| (ms / 1000.0 * sample_rate) as usize;
        let longest = (MAX_LENGTH * sample_rate) as usize;
        ReverseReverb {
            sample_rate,
            diffusers: [0, 1].map(|_| DIFFUSERS_MS.map(|ms| Diffuser { line: Line::new(samples(ms)) })),
            lines: LINES_MS.map(|ms| Line::new(samples(ms))),
            lows: [0.0; 4],
            sides: [Reverser::new(longest), Reverser::new(longest)],
            length: 0.8,
            decay: 3.0,
            damp: 0.4,
            mix: 0.6,
            r_patched: false,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            LENGTH => self.length = value.clamp(MIN_LENGTH, MAX_LENGTH),
            DECAY => self.decay = value,
            DAMP => self.damp = value,
            MIX => self.mix = value,
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.diffusers.iter_mut().flatten().for_each(|d| d.line.clear());
        self.lines.iter_mut().for_each(Line::clear);
        self.sides.iter_mut().for_each(Reverser::clear);
        self.lows = [0.0; 4];
    }

    fn connected(&mut self, inputs: &[bool]) {
        self.r_patched = inputs.get(R).copied().unwrap_or(false);
    }

    fn tick(&mut self, inputs: &[f32], outputs: &mut [f32]) {
        let input = [inputs[L], if self.r_patched { inputs[R] } else { inputs[L] }];
        let stretch = self.stretch();
        let (left, right) = self.reverb(input[L], input[R]);
        let wet = [left, right];
        for (k, side) in self.sides.iter_mut().enumerate() {
            let reversed = side.read(stretch);
            side.write(wet[k]);
            outputs[k] = input[k] * (1.0 - self.mix) + reversed * self.mix;
        }
    }
}

export_module!(ReverseReverb);

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn reverb(length: f32, decay: f32) -> ReverseReverb {
        let mut r = ReverseReverb::new(RATE);
        for (k, v) in [(LENGTH, length), (DECAY, decay), (DAMP, 0.3), (MIX, 1.0)] {
            r.set_param(k, v);
        }
        r
    }

    /// `samples` of it fed `input(n)`: left and right.
    fn play(r: &mut ReverseReverb, samples: usize, mut input: impl FnMut(usize) -> f32) -> (Vec<f32>, Vec<f32>) {
        let (mut left, mut right) = (Vec::new(), Vec::new());
        let mut o = [0.0; 2];
        for n in 0..samples {
            r.tick(&[input(n), 0.0], &mut o);
            left.push(o[0]);
            right.push(o[1]);
        }
        (left, right)
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
    }

    /// A click comes back as a swell: within a stretch, the reversed tail
    /// grows louder towards its end (the click itself).
    #[test]
    fn a_click_swells_up() {
        let length = 0.5;
        let stretch = (length * RATE) as usize;
        let mut r = reverb(length, 3.0);
        let (left, right) = play(&mut r, 3 * stretch, |n| if n == 10 { 5.0 } else { 0.0 });
        for side in [&left, &right] {
            // the stretch that holds the click's tail, read backwards, in 20 blocks
            let blocks: Vec<f32> = side[stretch..2 * stretch].chunks(stretch / 20).map(rms).collect();
            let early = blocks[1..5].iter().sum::<f32>() / 4.0;
            let late = blocks[10..16].iter().cloned().fold(0.0, f32::max);
            assert!(late > 3.0 * early && late > 0.01, "it swells: {early} then {late} ({blocks:?})");
            assert!(side.iter().all(|x| x.is_finite() && x.abs() < 20.0));
        }
        assert!(left.iter().zip(&right).any(|(l, r)| (l - r).abs() > 1e-3), "two sides of their own");
    }

    /// The forward reverb rings for about Decay: 60 dB down by then.
    #[test]
    fn decay_is_how_long_it_rings() {
        let mut r = reverb(0.5, 1.0);
        let mut energy = Vec::new();
        for n in 0..(2.0 * RATE) as usize {
            let x = if n == 0 { 5.0 } else { 0.0 };
            let (l, _) = r.reverb(x, x);
            energy.push(l);
        }
        let start = rms(&energy[1000..5800]);
        let after = rms(&energy[48_000..52_800]);
        let fallen = 20.0 * (after / start).log10();
        assert!((-75.0..-40.0).contains(&fallen), "about 60 dB in a second: {fallen} dB");
    }

    /// With a cable at R, a sound at L alone rings in both sides (the
    /// matrix spreads it), louder in the left.
    #[test]
    fn a_side_spreads_to_the_other() {
        let mut r = reverb(0.5, 3.0);
        r.connected(&[true, true]);
        let (mut left, mut right) = (Vec::new(), Vec::new());
        for n in 0..(0.5 * RATE) as usize {
            let (l, rr) = r.reverb(if n == 10 { 5.0 } else { 0.0 }, 0.0);
            left.push(l);
            right.push(rr);
        }
        let (l, rr) = (rms(&left[..4800]), rms(&right[..4800]));
        assert!(rr > 0.0 && l > rr, "left {l}, right {rr}");
    }

    #[test]
    fn dry_is_the_input_and_loud_noise_stays_bounded() {
        let mut r = reverb(0.3, 10.0);
        r.set_param(MIX, 0.0);
        let (left, right) = play(&mut r, 1000, |n| (n as f32 * 0.02).sin());
        assert!(left.iter().zip(&right).enumerate().all(|(n, (l, r))| (l - (n as f32 * 0.02).sin()).abs() < 1e-6 && l == r));
        let mut r = reverb(0.1, 10.0);
        let mut seed = 7u32;
        let (left, _) = play(&mut r, 48_000 * 4, |_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed as f32 / u32::MAX as f32 - 0.5) * 10.0
        });
        assert!(left.iter().all(|x| x.is_finite() && x.abs() < 60.0));
    }
}
