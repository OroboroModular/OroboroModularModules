//! Swarm: up to seven saws around one note, spread in pitch and across the
//! stereo field. The sound of big leads and pads: one oscillator that is a
//! crowd.


use std::f32::consts::FRAC_PI_4;
use std::sync::atomic::{AtomicU32, Ordering};

use oroboro_module::{export_module, Module, Param, Spec};

/// Maximum number of saws.
const SAWS: usize = 7;

/// C4, the note 0 V is.
const C4: f32 = 261.625_58;

/// Each saw's place in the spread, from −1 to 1: the centre first, then
/// pairs outwards. Uneven on purpose (the beats between them never line up).
const PLACE: [f32; SAWS] = [0.0, -0.19, 0.2, -0.57, 0.55, -1.0, 0.97];

/// Where each sits between left (−1) and right (1) at full Spread: a pair's
/// two saws on opposite sides, the pairs alternating so that neither side
/// is the sharp one.
const PAN: [f32; SAWS] = [0.0, -0.4, 0.4, 0.75, -0.75, -1.0, 1.0];

/// The spread's width in semitones at full Detune: the outer saws this far
/// from the note.
const WIDTH: f32 = 2.0;

/// The share of the power the saws beside the centre have at full Mix.
const BESIDE: f32 = 0.8;

/// Knobs, in the spec's order.
const TUNE: usize = 0;
const FINE: usize = 1;
const FM: usize = 2;
const DETUNE: usize = 3;
const MIX: usize = 4;
const SPREAD: usize = 5;
const COUNT: usize = 6;

/// Inputs, in the spec's order.
const IN_PITCH: usize = 0;
const IN_FM: usize = 1;
const IN_DETUNE: usize = 2;
const IN_SPREAD: usize = 3;

/// Counts the instances made, so each starts its saws somewhere else.
static MADE: AtomicU32 = AtomicU32::new(0);

pub struct Swarm {
    sample_rate: f32,
    phase: [f32; SAWS],
    tune: f32,
    fine: f32,
    fm: f32,
    detune: f32,
    mix: f32,
    spread: f32,
    count: usize,
}

/// The width of the spread, in semitones, for a Detune of 0 to 1: fine at
/// the low end (most of the knob is the lush part), wide at the top.
fn width(detune: f32) -> f32 {
    let d = detune.clamp(0.0, 1.0);
    WIDTH * (0.2 * d + 0.8 * d * d * d)
}

/// What takes the step out of a saw's edge: a polynomial over the sample
/// before and the sample after it (`t` the phase, `dt` its step a sample).
fn blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        2.0 * x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + 2.0 * x + 1.0
    } else {
        0.0
    }
}

impl Swarm {
    /// The centre saw's gain and each other saw's, for `mix` and `count`
    /// saws: together they keep the power of one saw.
    fn gains(mix: f32, count: usize) -> (f32, f32) {
        if count <= 1 {
            return (1.0, 0.0);
        }
        let beside = BESIDE * mix.clamp(0.0, 1.0);
        ((1.0 - beside).sqrt(), (beside / (count - 1) as f32).sqrt())
    }
}

impl Module for Swarm {
    fn spec() -> Spec {
        Spec::new("native/Swarm")
            .vendor("Oroboro")
            .category("osc")
            .input("V/Oct")
            .input("FM")
            .input("Detune")
            .input("Spread")
            .output("L")
            .output("R")
            .param(Param::new("Tune", -36.0, 36.0, 0.0).unit("st"))
            .param(Param::new("Fine", -100.0, 100.0, 0.0).unit("ct"))
            .param(Param::new("FM", 0.0, 1.0, 0.0))
            .param(Param::new("Detune", 0.0, 1.0, 0.35))
            .param(Param::new("Mix", 0.0, 1.0, 0.7))
            .param(Param::new("Spread", 0.0, 1.0, 0.8))
            .param(Param::new("Saws", 1.0, SAWS as f32, SAWS as f32).steps())
    }

    fn new(sample_rate: f32) -> Self {
        // each instance (each voice) starts its saws at phases of its own
        let mut seed = 0x9e37_79b9u32.wrapping_mul(MADE.fetch_add(1, Ordering::Relaxed).wrapping_add(1)) | 1;
        let mut phase = [0.0; SAWS];
        for p in &mut phase {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *p = (seed >> 8) as f32 / (1u32 << 24) as f32;
        }
        Swarm { sample_rate, phase, tune: 0.0, fine: 0.0, fm: 0.0, detune: 0.35, mix: 0.7, spread: 0.8, count: SAWS }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            TUNE => self.tune = value,
            FINE => self.fine = value,
            FM => self.fm = value,
            DETUNE => self.detune = value,
            MIX => self.mix = value,
            SPREAD => self.spread = value,
            COUNT => self.count = (value.round() as usize).clamp(1, SAWS),
            _ => {}
        }
    }

    fn tick(&mut self, inputs: &[f32], outputs: &mut [f32]) {
        // the note: the key or a cable, the knobs, and FM (a volt an octave at full FM)
        let octaves = inputs[IN_PITCH] + (self.tune + self.fine / 100.0) / 12.0 + self.fm * inputs[IN_FM];
        // 10 V at an input is its knob's whole way
        let detune = self.detune + inputs[IN_DETUNE] / 10.0;
        let spread = (self.spread + inputs[IN_SPREAD] / 10.0).clamp(0.0, 1.0);
        let width = width(detune) / 12.0;
        let (centre, beside) = Swarm::gains(self.mix, self.count);
        let nyquist = 0.45 * self.sample_rate;

        let (mut left, mut right) = (0.0, 0.0);
        for k in 0..self.count {
            let frequency = (C4 * (octaves + PLACE[k] * width).exp2()).clamp(0.0, nyquist);
            let dt = frequency / self.sample_rate;
            let t = self.phase[k];
            let saw = 2.0 * t - 1.0 - blep(t, dt);
            self.phase[k] = (t + dt).fract();
            let gain = if k == 0 { centre } else { beside };
            // equal power between the sides; in the middle both have it whole
            let angle = (PAN[k] * spread + 1.0) * FRAC_PI_4;
            left += saw * gain * angle.cos() * std::f32::consts::SQRT_2;
            right += saw * gain * angle.sin() * std::f32::consts::SQRT_2;
        }
        outputs[0] = 5.0 * left;
        outputs[1] = 5.0 * right;
    }
}

export_module!(Swarm);