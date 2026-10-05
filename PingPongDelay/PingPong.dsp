// PingPong: a ping-pong delay for Oroboro Modular, written in Faust and
// built into a compiled module (README.md: `oromod faust build`).
//
// What goes in comes back from the left, then from the right, then from
// the left again, each repeat a little quieter, darker and thinner than
// the last. Two delay lines of one length feed each other: the left one's
// output goes into the right one, and the right one's back into the left.
// With Stereo off, L and R together go into the left line: the classic
// ping-pong, the dry sound staying where it came in. With Stereo on, L
// goes into the left line and R into the right: a stereo sound bounces
// both ways at once.

declare name "PingPong";              // "faust/PingPong" in saved patches
declare category "fx";                // its type: Effects
declare vendor "Oroboro";             // who makes it: where Add module lists it
declare version "2.0";
declare description "A ping-pong delay: each repeat comes from the other side, a little quieter, darker and thinner.";
declare input_names "L, R";
declare output_names "L, R";

import("stdfaust.lib");

// ---- the knobs (their numbers are their order, and a patch's) ------------
time   = hslider("[1]Time[unit:ms][scale:log]", 375, 10, 2000, 0.1);
fb     = hslider("[2]Feedback[unit:%]", 55, 0, 110, 1) / 100 : si.smoo;
tone   = hslider("[3]Tone[unit:Hz][scale:log]", 6000, 300, 18000, 1) : si.smoo;
lowcut = hslider("[4]Low Cut[unit:Hz][scale:log]", 80, 20, 2000, 1) : si.smoo;
width  = hslider("[5]Width[unit:%]", 100, 0, 100, 1) / 100 : si.smoo;
mix    = hslider("[6]Mix[unit:%]", 40, 0, 100, 1) / 100 : si.smoo;
stereo = checkbox("[7]Stereo");

// ---- signals are volts ---------------------------------------------------
// Audio is about ±5 V. Every knob takes a cable in the plugin, with an
// amount of its own (a cable into Time sweeps it).

// the longest line in samples, a power of two: 2.7 s at the engine's 96 kHz
N = 262144;

// one line's length in samples: a new time is reached over about 80 ms, so
// turning Time bends the repeats' pitch as tape does, without clicks
samples = time * ma.SR / 1000 : max(2) : min(N - 2) : si.smooth(ba.tau2pole(0.08));

// how much of a line's output goes on into the other one
amount = fb : max(0) : min(1.1);

// on its way into the other line a repeat loses its top (Tone) and its
// bottom (Low Cut), and is held in bounds: as it went in for small
// signals, bending towards ±10 V. Past 100 % the repeats build up to
// that and stay.
limit(x) = 10 * ma.tanh(x / 10);
across(g) = fi.lowpass(1, tone) : fi.highpass(1, lowcut) : *(g) : limit;

// the two lines. "~" is a feedback loop: what the lines give goes through
// the right-hand side and back into their first inputs, one sample later.
// The right-hand side crosses over: the left line's output (through
// `across`) into the right line, the right line's into the left.
lines(d) = de.fdelay(N, d), de.fdelay(N, d);
swap(a, b) = b, a;
echoes(d, g) = (ro.interleave(2, 2) : (+, +) : lines(d)) ~ (across(g), across(g) : swap);

// Width: how far apart the two sides' repeats are (0: both in the middle)
spread(l, r) = l * near + r * far, r * near + l * far
with {
    near = (1 + width) / 2;
    far = (1 - width) / 2;
};

// what went in and the repeats, for one side
side(dry, wet) = dry * (1 - mix) + wet * mix;

// into the lines: both sides into the left one (Stereo off: the right
// line gets them one delay later), or each into its own (on)
feed(l, r) = select2(stereo, (l + r) / 2, l), select2(stereo, 0, r);

// the module: L and R in, L and R out
process(l, r) = feed(l, r) : echoes(samples, amount) : spread : (side(l), side(r));
