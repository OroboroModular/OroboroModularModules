# ThroughZero

ThroughZero Oscillator

Written in Rust with the SDK's `oroboro-module` crate.

## The module

| Jack | |
|---|---|
| V/Oct | The note: 1 V an octave, 0 V is C4 (the key's, while nothing's patched). |
| FM | Linear FM: at Index 1, ±5 V moves the frequency by the note's own either way. |
| Index (on its knob) | 10 V is the knob's whole way. |
| Exp | Exponential FM: Exp's knob in octaves a volt. |
| Sync | A rising edge (over 1 V) starts the cycle again. |
| Sine, Tri, Saw, Square (out) | ±5 V. Saw and square have their steps smoothed (polyBLEP), going either way. |

| Knob | |
|---|---|
| Tune | −48 to 48 semitones. |
| Fine | −100 to 100 cents. |
| Index | 0 to 5 (1): how deep FM goes. Past 1, a full swing takes the frequency through zero. |
| Exp | 0 to 1: the Exp input's octaves a volt. |
| Mode | TZ (through zero) or Lin (the frequency stops at zero, as in analogue linear FM). |

How it works: the note gives a frequency f0, and FM makes it
f0 · (1 + Index · FM / 5 V). A phase runs at that frequency, forwards or
backwards; the waves are read from it.

## Building

```bash
cargo test --release                  # its tests
oromod build . --out build            # build/ThroughZero.oromodule: cargo build --release, and what Cargo.toml says

oromod check build/ThroughZero.oromodule    # its interface, a second of sound, its cost a sample
oromod install build/ThroughZero.oromodule  # into native/Oroboro/ in the plugin's modules folder
```