# ReverseReverb

Reverse Reverb

Written in Rust with the SDK's `oroboro-module` crate.

## The module

| Jack | |
|---|---|
| L, R (in) | What's reverberated. Without a cable at R, R takes L's sound. |
| L, R (out) | Dry and wet, as Mix sets them. |

| Knob | |
|---|---|
| Length | 0.1 to 2 s (0.8): how long a reversed swell is. |
| Decay | 0.3 to 10 s (3): how long the reverb rings (60 dB down). |
| Damp | 0 to 1 (0.4): darkens the reverb. |
| Mix | 0 to 1 (0.6): dry against wet. |

Every knob takes a cable in the plugin (drop one on it), with an amount
of its own: an LFO into Time sweeps it, an envelope into Mix opens it.

How it works: the input goes through two allpass diffusers into a
feedback delay network (four delay lines mixed by a Hadamard matrix, each
damped and scaled for Decay), two pairs of lines making left and right.
Each side then goes into a reverser: two heads read it backwards over
stretches Length long, half a stretch apart and faded in and out. So each
stretch of reverb is heard from its quiet end to its loud start.

## Building

```bash
cargo test --release                  # its tests
oromod build . --out build            # build/ReverseReverb.oromodule: cargo build --release, and what Cargo.toml says

oromod check build/ReverseReverb.oromodule    # its interface, a second of sound, its cost a sample
oromod install build/ReverseReverb.oromodule  # into native/Oroboro/ in the plugin's modules folder
```