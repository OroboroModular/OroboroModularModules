# PingPong

PingPong Delay

Written in Faust DSP (`PingPong.dsp`, the whole module) and built into a
compiled/native module with the Oroboro Modular SDK, so it plays without Faust
installed and at a compiled module's cost.

## The module

| Jack | |
|---|---|
| L, R (in) | Audio, about ±5 V. With Stereo off both go into the left line; with it on, each into its own. A mono sound: cable it into both. |
| L, R (out) | Left and right: what went in, where it came in, and the repeats on their sides. |

| Knob | |
|---|---|
| Time | 10 ms to 2 s (375 ms): the gap between a repeat on one side and the next on the other. The same side again comes after twice that. |
| Feedback | 0 to 110 % (55 %): how much of each repeat goes on to the other side. Past 100 % the repeats build up, to a level they're held at, and stay. |
| Tone | 300 Hz to 18 kHz (6 kHz): each repeat loses what's above it. |
| Low Cut | 20 Hz to 2 kHz (80 Hz): each repeat loses what's below it. |
| Width | 0 to 100 % (100 %): how far apart the two sides' repeats are. At 0 both are in the middle. |
| Mix | 0 to 100 % (40 %): from what went in alone to the repeats alone. |
| Stereo | Off (the ping-pong: L and R together into the left line, the repeats bouncing left, right, left …) or On (L into the left line, R into the right: a stereo sound bounces both ways at once). |

## Building

```bash
oromod check PingPong.dsp                    # the source: the interface the plugin would give it
oromod faust build PingPong.dsp --out build  # build/PingPong.oromodule: Faust, then cargo
oromod check build/PingPong.oromodule        # the build: its interface, a second of sound, its cost a sample
oromod install build/PingPong.oromodule      # into native/Oroboro/ in the plugin's modules folder
```

Building needs Faust 2.88 (`FAUST`, `PATH` or `~/tools/faust-*`) and
Rust. The plugin has PingPong built in; a build of your own in its
modules folder plays instead, loaded with **Developer mode** on in its
Settings, about a second after it's installed.

`PingPong.dsp` in the modules folder itself plays too, as a script
(`faust/PingPong`, with Faust installed): a module of its own beside the
compiled one.