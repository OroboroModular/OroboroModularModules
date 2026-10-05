# Oroboro Modules

Oroboro's own compiled modules for Oroboro Modular made with the OroboroModularSDK. They are examples of how a module is made: in Rust with the SDK's `oroboro-module` crate, or in Faust.

| Project | Module | What it is |
|---|---|---|
| [`SpectrumAnalyser/`](SpectrumAnalyser/) | **Spectrum** (`native/Oroboro/Spectrum`) | A spectrum analyser on a panel of its own, with a screen you can point at and click. Rust. |
| [`ThroughZeroOscillator/`](ThroughZeroOscillator/) | **ThroughZero** (`native/Oroboro/ThroughZero`) | An oscillator for through-zero FM, in tune however deep. Rust. |
| [`PingPongDelay/`](PingPongDelay/) | **PingPong** (`native/Oroboro/PingPong`) | A stereo ping-pong delay, its repeats left, right, left. Faust DSP, built into a compiled module with `oromod faust build`. |
| [`PitchDelay/`](PitchDelay/) | **PitchDelay** (`native/Oroboro/PitchDelay`) | A stereo delay whose echoes climb or fall in pitch. Rust. |
| [`ReverseDelay/`](ReverseDelay/) | **ReverseDelay** (`native/Oroboro/ReverseDelay`) | Stereo echoes that play backwards. Rust. |
| [`ReverseReverb/`](ReverseReverb/) | **ReverseReverb** (`native/Oroboro/ReverseReverb`) | A stereo reverb heard backwards, swelling into the sound. Rust. |
| [`SwarmOscillator/`](SwarmOscillator/) | **Swarm** (`native/Oroboro/Swarm`) | Up to seven saws around one note, spread in pitch and across the stereo field. Rust. Not built into the plugin: build and install it yourself. |

These modules are already built into Oroboro Modular.

## Building

You need Rust, the Oroboro Modular SDK (its tool, `oromod`, and its
`oroboro-module` crate), and for PingPong Faust 2.88.

Each Rust project takes the SDK's crate, `oroboro-module`, as its
`Cargo.toml` says, and `cargo build --release` in its folder builds the
module's library.

Every build makes the module's file, `<Name>.oromodule`: its interface,
what it says of itself (version, description) and a build per platform
(`oromod build` for a Rust crate, `oromod faust build` for a Faust
source).

```bash
oromod build ThroughZeroOscillator --out build   # build/ThroughZero.oromodule
oromod check build/ThroughZero.oromodule         # its knobs and jacks, a second of sound, its cost a sample
oromod install build/ThroughZero.oromodule       # into native/Oroboro/ in the plugin's modules folder
```

The plugin loads a compiled module when the Community Library signed it,
or, with **Developer mode** on in its Settings, when it's a
build of your own.

## Publishing

```bash
oromod login
oromod publish build/<Name>.oromodule --summary "…"
```

The Community Library signs each build as it takes it, so the plugin
loads it for whoever downloads it, without Developer mode. Only makers
the library knows can publish compiled modules, under their own vendor
name. A build is for the platform it was made on: build on each into the
same module file, then publish that.
