# Swarm

Swarm Oscillator (Up to 7 Saws)

## The module

| Jack | |
|---|---|
| V/Oct (in) | The note, 1 V an octave, 0 V = C4. Without a cable it's the key played. |
| FM (in) | Moves the pitch, as far as the FM knob says (a volt an octave at full FM). |
| Detune (in) | Adds to the Detune knob (10 V is its whole way). |
| Spread (in) | Adds to the Spread knob (10 V is its whole way). |
| L, R (out) | Left and right, about ±5 V. |

| Knob | |
|---|---|
| Tune | −36 to 36 semitones. |
| Fine | −100 to 100 cents. |
| FM | How far the FM input moves the pitch. |
| Detune | How far apart the saws are: fine at the low end, two semitones at the top. |
| Mix | From the centre saw alone to mostly the saws beside it. The level stays. |
| Spread | How far left and right the saws beside the centre sit. |
| Saws | 1 to 7. |

Each saw runs free from a phase of its own, and each voice starts differently, so a chord doesn't begin with every saw at the same point.

## Building

```bash
cargo test --release                  # its tests
oromod build . --out build            # build/Swarm.oromodule: cargo build --release, and what Cargo.toml says

oromod check build/Swarm.oromodule    # its interface, a second of sound, its cost a sample
oromod install build/Swarm.oromodule  # into native/Oroboro/ in the plugin's modules folder
```

## Publishing

```bash
oromod login                          # once: a code to confirm at oroboro.space
oromod publish build/Swarm.oromodule --summary "Up to seven saws, spread in pitch and in stereo"
```