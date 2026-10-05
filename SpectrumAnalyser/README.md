# Spectrum

Spectrum Analyzer

It's written in Rust with the SDK's `oroboro-module` crate, and shows
what a module of one's own can have: a panel laid out its own way in the
plugin's look (`Module::panel`, `Panel::stock`: its knobs, jacks, light
and captions where it puts them), a screen on it (`Module::screen`:
drawn by the plugin each frame, and the pointer there), and a light
(`Module::lights`).

## The module

| Jack | |
|---|---|
| L, R (in) | What's looked at, summed. R without a cable is L. |
| L, R (out, THRU) | What came in, unchanged. |

| Knob | |
|---|---|
| Gain | −24 to 24 dB: lifts or lowers the picture, not the sound. |
| Floor | −120 to −24 dB (−96): the screen's bottom. Its top is +6 dB. |
| Smooth | 0 to 0.95 (0.7): how slowly the picture falls after a peak (it rises at once). |
| Scale | Log (20 Hz to 20 kHz, evenly by octaves) or Lin (0 to 20 kHz). |
| Hold | Off or On: a line over the picture that keeps the peaks and falls slowly. |

A sine of 5 V at its peak (Oroboro's audio level) reads 0 dB. The light
is green with the input's level and red past 0 dB.

**The screen:** point at it to read the frequency and its level there;
click it to stop the picture, and again to go on. Drag the panel anywhere
else to move the module.


## Building

```bash
cargo test --release                  # its tests
oromod build . --out build            # build/Spectrum.oromodule: cargo build --release, and what Cargo.toml says

oromod check build/Spectrum.oromodule    # its interface, a second of sound, its panel, its screen
oromod install build/Spectrum.oromodule  # into native/Oroboro/ in the plugin's modules folder
```