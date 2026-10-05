# PitchDelay

PitchDelay

Written in Rust with the SDK's `oroboro-module` crate.

## The module

| Jack | |
|---|---|
| L, R (in) | What's echoed. Without a cable at R, R takes L's sound. |
| Pitch (in) | 1 V an octave, added to Pitch (between its semitone steps too). |
| L, R (out) | Dry and wet, as Mix sets them, each side shifted on its own. |
| Wet L, Wet R | The echoes alone. |

| Knob | |
|---|---|
| Time | 0.02 to 2 s (0.35): the delay (the echo comes 25 ms later than that: half the shifter's window). |
| Feedback | 0 to 0.95 (0.5): each echo sent round again, transposed again. |
| Pitch | −24 to 24 semitones (7), in steps. |
| Fine | −100 to 100 cents. |
| Tone | 0 to 1 (0.7): darkens what goes round. |
| Mix | 0 to 1 (0.5): dry against wet. |

## Building

```bash
cargo test --release                  # its tests
oromod build . --out build            # build/PitchDelay.oromodule: cargo build --release, and what Cargo.toml says

oromod check build/PitchDelay.oromodule    # its interface, a second of sound, its cost a sample
oromod install build/PitchDelay.oromodule  # into native/Oroboro/ in the plugin's modules folder
```