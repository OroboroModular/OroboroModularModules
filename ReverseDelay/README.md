# ReverseDelay

Reverse Delay effect

Written in Rust with the SDK's `oroboro-module` crate.

## The module

| Jack | |
|---|---|
| L, R (in) | What's echoed. Without a cable at R, R takes L's sound. |
| L, R (out) | Dry and wet, as Mix sets them, each side reversed on its own. |
| Wet L, Wet R | The reversed echo alone. |

| Knob | |
|---|---|
| Time | 0.05 to 2 s (0.5): how long a reversed stretch is. |
| Feedback | 0 to 0.95 (0.3): the echo sent round again (so the next one is reversed twice: forwards). |
| Tone | 0 to 1 (0.8): darkens the echo, and what goes round. |
| Mix | 0 to 1 (0.5): dry against wet. |


## Building

```bash
cargo test --release                  # its tests
oromod build . --out build            # build/ReverseDelay.oromodule: cargo build --release, and what Cargo.toml says

oromod check build/ReverseDelay.oromodule    # its interface, a second of sound, its cost a sample
oromod install build/ReverseDelay.oromodule  # into native/Oroboro/ in the plugin's modules folder
```