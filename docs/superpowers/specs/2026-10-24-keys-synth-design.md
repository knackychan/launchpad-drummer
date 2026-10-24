# Launchpad Drummer — Keys Synth Design (semi-modular wavetable synth on PLAY)

Date: 2026-10-24
Status: design approved section by section in the brainstorming session of 2026-10-24; written spec awaiting review
Parent specs: `2026-10-17-drum-sampler-design.md` (engine; section 14 roadmap), `2026-10-17-ui-design.md`
(PLAY layout section 4, keys plate 4.6, mixer 7.1, SETUP section 8)
Replaces: the built-in keys synth of plan 1C Task 15 (`BasicKeys`, the built-in half of decision D1)

This document locks the design of the keys synthesizer before its implementation plan is written. Where it
and the parent specs disagree, this document wins for the keys, the mixer on PLAY and the Launch Control XL
knobs. Section 16 lists every change to the parent specs.

## 1. Decisions (from the brainstorming session)

| Topic | Decision |
|---|---|
| Architecture | **Semi-modular**: a fixed voice path (oscillators → filter → amp), like a Moog or Serum, plus a modulation matrix in which any source can drive any continuous voice control. No patch cables. |
| Placement | **Everything stays on PLAY.** The keys plate and the sequencer keep their place and size. The plate right of the Launchpad is shared: a **slim mixer** on the left and the **synth** on the right, both visible at once. |
| Mixer | Slim: 8 strips (fader, meter, mute, solo) plus master. Pan, pitch and low-pass move into a channel popover. |
| Launch Control XL | The **24 knobs drive 24 fixed synth controls**. Faders and buttons keep mixer volume, mute and solo. Mixer pan, pitch and low-pass become screen-only (still MIDI-learnable from other controllers). |
| Old sounds | The six keys sounds (PIANO … SYNTH BASS) become **editable factory patches**. The keys plate's sound LCD becomes the patch picker. A loaded `.sf2` stays available as an alternative engine; while it plays, the synth is dimmed. |
| v1 scope | **Wavetable oscillators** (8 built-in tables), **unison** up to 7 copies, **FX chain** (drive, chorus, delay, reverb). WAV import comes later (section 13). |
| Modulation UI | **Drag a source badge onto a knob** (Serum style): coloured arcs show the depth, a live dot shows the modulated value. A MATRIX page lists every route. |
| Implementation | **Own Rust DSP** in `lpd-core`, behind the existing `Instrument` trait, on the same audio path as the pads (ASIO / WASAPI, mixer routing). |
| Visuals | Wavetable view, filter response curve over the live spectrum, envelope and LFO shapes with live dots, knob rings, a synth-only scope. Fed by the existing telemetry thread. |
| Keyboard | Already works (MIDI role KEYS). Velocity, both wheels and a new sustain pedal (CC 64) feed the synth. |

## 2. Architecture and data flow

### 2.1 Code layout

New module `crates/lpd-core/src/synth/`, one job per file:

| File | Job |
|---|---|
| `mod.rs` | `Synth`: the 16 voices, global LFO state, FX chain; implements `Instrument` |
| `params.rs` | `ParamId`, the parameter table (id, label, range, taper, default, unit, flags; section 3), `SynthParams` (Copy: every value and the 16 routes, no strings) |
| `patch.rs` | `Patch` (serde: `patchVersion`, name, category, `SynthParams`), validation, conversion to `SynthParams` |
| `wavetable.rs` | `Wavetable` (frames plus one band-limited copy per octave), the 8 built-in generators, the registry built at startup |
| `osc.rs` | wavetable oscillator with unison and warp |
| `filter.rs` | ladder (LP 24) and state-variable (LP 12, HP 12, BP 12, NOTCH) filters, plus the analytic magnitude response used by the display |
| `env.rs` | ADSR envelope |
| `lfo.rs` | LFO shapes, free rate or tempo sync, modes |
| `matrix.rs` | routes, source values, destination maths |
| `voice.rs` | one note: oscillators, sub, noise, filter, amp, its envelopes and per-voice LFOs |
| `fx/drive.rs`, `fx/chorus.rs`, `fx/delay.rs`, `fx/reverb.rs` | the four FX units |
| `factory.rs` | the 16 factory patches (section 8.3) |
| `monitor.rs` | `SynthMonitor`: what the UI shows live |

Changes elsewhere:

- `instrument/basic.rs` is deleted. `Engine::build` creates a `Synth` as the default keys instrument.
- `instrument/soundfont.rs` stays; it keeps the six GM programs (`KeysSound`, `gm_program`).
- The `Instrument` trait gains `set_param`, `set_route`, `load_params` and `sustain`, each with a default that
  does nothing. The SoundFont engine implements only `sustain` (CC 64 to rustysynth).
- `audio/fft.rs` gains a general band function (N log-spaced bands over a frequency range). The existing
  28-band function for the telemetry column stays as it is.
- `kit/model.rs`: `SET_VERSION` becomes 2 and `KeysSettings` changes (section 8.2).
- `kit/learn.rs`: parameter targets `synth.<id>` (section 10.3).
- `devices/lcxl.rs`: the knob → synth map and knob LED indices (section 10.2). The knob laws for pan, pitch
  and low-pass (`knob_pan`, `knob_pitch`, `knob_lowpass`) are removed.

### 2.2 One edit, end to end

1. A screen control, an LCXL knob or a learned CC changes a synth control.
2. The router updates `set.keys.patch`, marks the patch modified, and records history with the key
   `synth.<id>` (the existing History merges records with the same key within 1 s).
3. The router pushes `Command::SynthParam(ParamId, f32)` (Copy) on the audio command ring.
4. The engine calls `instrument.set_param`; the synth smooths the value (section 11).
5. Screen edits are optimistic: the UI already shows its own value and gets nothing back. LCXL and learned-CC
   edits emit `synth-params` (batched, at most every 33 ms, the last value always follows) so the screen
   follows the hardware.

Routes travel as `Command::SynthRoute { slot, route }` (Copy). A whole patch (load, revert, undo, redo, a new
audio engine) travels as `Command::SynthLoad(Box<SynthParams>)`: the engine fades every voice out over 5 ms,
copies the values, clears the FX buffers, and returns the box through a trash queue so nothing is freed on the
audio thread. When the audio engine is rebuilt (device change, reconnect), the router sends the current patch
together with the existing keys route and volume (`send_keys`).

### 2.3 Keyboard path

The route is unchanged: a KEYS-role input → router `keys_event` → `KeysNoteOn` / `KeysNoteOff`, `KeysPitch`,
`KeysMod`. Added: `KeysSustain(bool)` from CC 64 (value ≥ 64 = down). `KeysPitch` now carries the normalized
wheel position (−1…+1); each instrument scales it (the synth by its BEND RANGE, the SoundFont by ±2 semitones
as today). `KeysMod` carries the mod wheel (0…1) and becomes the synth's WHEEL source.

### 2.4 Visual feed

- The engine owns a `SynthTap`, lock-free like the existing taps: the synth's own output (mono sum, after FX,
  before the keys volume) for every frame, and one `SynthMonitor` per rendered block. A full queue drops data;
  the audio thread never waits.
- `SynthMonitor` holds, for the newest sounding voice: the value of every source, the live (modulated) value
  of every destination, the level and stage of each envelope, the value and phase of each LFO, the live
  cutoff, resonance and filter type; plus the active voice count, the synth's render time over the block time
  (CPU), and a counter of voices reset for instability (section 11). With no voice sounding, the live values
  are the knob values.
- The telemetry thread (16 ms loop, running only while the feed is on: PLAY visible, same switch as the
  `telemetry` event) drains the tap, runs the 2048-point FFT on the synth samples, computes the filter
  response at 128 frequencies from the live filter values, and emits `synth-telemetry`.
- Wavetable display frames are fetched once per table with `synth_table`.
- Rust is the single source of truth for parameter metadata. A cargo test writes `src/lib/synth-params.json`
  (ids, labels, ranges, tapers, defaults, units, flags, the destination order, the LCXL map) and fails if the
  committed file differs. The UI and the browser preview read that file, so they cannot drift from the engine.

### 2.5 Threads and rules

No new threads. The audio thread never allocates, locks or frees (enforced by the `no_alloc` test).
Wavetables are built at startup on a worker thread before audio starts and live for the whole run. Patch
library file I/O runs in async commands outside the router lock; the lock order stays router → history →
kit_manager → saver.

### 2.6 Command and event registry

| Name | Kind | Payload → result |
|---|---|---|
| `synth_param` | command | `{ id: string, value: number }` → nothing |
| `synth_route` | command | `{ slot: number /* 0–15 */, route: Route \| null }` → nothing (`null` deletes) |
| `synth_patches` | command | → `{ factory: PatchRef[], user: PatchRef[] }` |
| `synth_load_patch` | command | `{ source: { factory: string } \| { user: string } \| { init: true } }` → `Set` |
| `synth_save_patch` | command | `{ name: string, overwrite: boolean }` → `PatchRef` (error `exists` when the name is taken and `overwrite` is false) |
| `synth_rename_patch` | command | `{ file: string, name: string }` → `PatchRef` |
| `synth_delete_patch` | command | `{ file: string }` → nothing |
| `synth_revert` | command | → `Set` |
| `synth_table` | command | `{ table: string }` → `{ frames: number[][] }` (up to 64 frames × 128 points, display only) |
| `set_keys_engine` | command | `{ engine: 'synth' \| 'soundfont' }` → `KeysStatus` |
| `synth-params` | event | `{ values: [string, number][] }` (LCXL and learned-CC edits) |
| `synth-telemetry` | event | below |
| `keys-status` | event (existing) | `engine` is now `'synth' \| 'soundfont'` (was `'builtin'`) |
| `set-changed` | event (existing) | also after patch loads, revert, undo and redo |

`PatchRef` is `{ id: string, name: string, category: Category }`: `id` is the factory id or the user file name.
`synth_param` values are in the control's units, as stored in the patch (Hz, seconds, dB, semitones, 0–1
positions); keys send 0 or 1; selectors send the index of the value in the control's value list
(`synth-params.json` lists them).

```ts
// synth-telemetry, every 16 ms while the feed is on
{
  scope: number[];         // 256 samples of the synth output, trigger-aligned like the telemetry scope
  spectrum: number[];      // 128 values, dBFS at 128 log-spaced frequencies from 20 Hz to 20 kHz
  curve: number[];         // 128 values, filter response in dB at the same frequencies
  sources: number[];       // value of every source (section 4.1 order), newest voice, in the source's range
  live: number[];          // live value of every destination (section 4.2 order), normalized 0–1
  env: [number, number][]; // per envelope: level 0–1, stage (0 idle, 1 attack, 2 decay, 3 sustain, 4 release)
  lfo: [number, number][]; // per LFO: value −1…1, phase 0–1
  cutoffHz: number;
  voices: number;          // sounding voices, 0–16
  cpu: number;             // synth render time / block time, 0–1
  resets: number;          // voices reset for instability since start
}
```

## 3. Sound engine

### 3.1 Signal path

```
OSC A ─┐                     ┌── (→ FILTER off: skips the filter) ──┐
OSC B ─┤                     │                                      ▼
SUB   ─┼─► per-source ───────┴─► FILTER ─► AMP ───────────────────► (+) ─► sum of voices × 0.25
NOISE ─┘   "→ FILTER" switch                                                   │
                         FX: DRIVE → CHORUS → DELAY → REVERB ◄─────────────────┘
                            └─► VOLUME (patch) ─► keys volume ─► master bus or one mixer channel
```

- **16 voices**, one per held or releasing note. Each voice runs OSC A and OSC B (each up to 7 unison
  copies), SUB and NOISE, its filter and amp, its three envelopes and its per-voice LFOs.
- The sum of voices is scaled by 0.25 (−12 dB headroom, as today), then goes through the FX chain, the patch
  VOLUME and the keys volume, then to the master bus or one mixer channel exactly as today (decision D5 still
  holds: the channel's pitch does not apply to the keys).
- **Control rate**: envelopes, LFOs and the matrix are evaluated every 16 samples (about 0.33 ms at 48 kHz).
  Cutoff, levels and pans are linearly interpolated across those 16 samples.
- A source whose ON key is off is not computed, except an oscillator that is the modulator of the other
  oscillator's FM or RM warp: it runs (silently when its level is 0).

**Tapers** used in the tables: `lin` (min + n·(max − min)), `log` (min·(max/min)ⁿ), `cubic`
(max·n³, for times that start at 0), `int` (rounded `lin`). `n` is the normalized knob position 0–1; the LCXL
sends `n = value / 127`. Levels shown in % use gain = n². `mod` marks a modulation destination (section 4.2).

### 3.2 Oscillators A and B (ids `oscA.*`, `oscB.*`)

| Id | Label | Range / values | Default A / B | Taper / unit | Notes |
|---|---|---|---|---|---|
| `on` | ON | off, on | on / off | key | |
| `table` | TABLE | `basic`, `pwm`, `harmonics`, `vowels`, `fmSweep`, `syncSweep`, `organ`, `digital` | `basic` | menu | section 3.3 |
| `pos` | POS | 0–1 | 0.667 (saw) | lin, % | mod; scans frames, crossfading neighbours |
| `oct` | OCT | −3…+3 | 0 | int | |
| `semi` | SEMI | −12…+12 | 0 | int, st | |
| `fine` | FINE | −100…+100 | 0 | lin, cents | mod |
| `warp` | WARP | `off`, `sync`, `bend`, `pwm`, `fm`, `rm` | `off` | menu | below |
| `warpAmt` | WARP AMT | 0–1 | 0 | lin, % | mod |
| `unison` | UNISON | 1–7 | 1 | int | |
| `detune` | DETUNE | 0–1 | 0.25 | lin, % | mod; outer copies at ±100·d² cents, the rest evenly between |
| `blend` | BLEND | 0–1 | 0.75 | lin, % | mod; level of the outer copies relative to the centre one (0: centre only, 1: all equal) |
| `width` | WIDTH | 0–1 | 0.5 | lin, % | mod; stereo spread of the copies |
| `phaseRand` | PHASE RAND | 0–1 | 1 | lin, % | 0: every copy starts at phase 0 on note-on; 1: random start phases |
| `level` | LEVEL | 0–1 | 0.75 | lin, % (gain n²) | mod |
| `pan` | PAN | −1…+1 | 0 | lin, L50–C–R50 | mod |
| `toFilter` | → FILTER | off, on | on | key | |

Destination `oscA.pitch` / `oscB.pitch` has no knob: it adds semitones (section 4.3).

**Unison**: n copies are spaced evenly from −spread to +spread (for n = 1, a single copy at the note). The
"centre" copy is the middle one; with an even count, the inner copy just below the note. Copies are panned
evenly across ±WIDTH and their level is normalized so that changing UNISON keeps the loudness about the same.

**Warp modes** (amount 0–1):

- `sync`: window sync. The frame is read at `phase × ratio` (wrapping) and restarts with the main phase;
  ratio = 16^amount (1–16×).
- `bend`: phase distortion `p′ = p^(1 + 4·amount)`; the cycle is squeezed towards its end.
- `pwm`: the frame is squeezed into the first `1 − 0.98·amount` of the cycle; the rest of the cycle is 0.
- `fm`: phase modulation by the other oscillator's centre copy, index 0–8 radians.
- `rm`: ring modulation with the other oscillator's centre copy, crossfaded from dry (0) to fully ringed (1).

The band-limited copy is chosen from the effective frequency (for `sync`, the frequency × ratio), so
playback, `sync`, `bend` and `pwm` stay clean. `fm` and `rm` can alias at high amounts; that is accepted, like
Serum without its HQ mode.

### 3.3 Wavetables

- A table is 4–64 frames of 2048 samples. For each table and frame, one band-limited copy per octave is built
  (harmonics above the octave's Nyquist limit removed with an inverse FFT; lower octaves keep more samples,
  so a table takes about 1 MB). The 8 tables take about 8 MB and are built in 200 ms or less at startup.
- Frames are normalized to a peak of 1.0.
- The 8 built-in tables (generated in code, no files):

| Id | Label | Frames | Content |
|---|---|---|---|
| `basic` | BASIC | 4 | sine → triangle → saw → square (POS 0, 0.333, 0.667, 1) |
| `pwm` | PWM | 32 | pulse, width 50 % → 2 % |
| `harmonics` | HARMONICS | 32 | additive saw spectrum, 1 → 32 partials |
| `vowels` | VOWELS | 5 | formant spectra A, E, I, O, U on a saw source |
| `fmSweep` | FM SWEEP | 32 | 2-operator FM (ratio 1:2), index 0 → 10 |
| `syncSweep` | SYNC SWEEP | 32 | hard-synced saw, ratio 1 → 8 |
| `organ` | ORGAN | 8 | drawbar mixes from a pure 8′ to full registration |
| `digital` | DIGITAL | 16 | saw reduced from 8 bits to 2 bits and from 64 to 8 steps per cycle |

### 3.4 Sub and noise (ids `sub.*`, `noise.*`)

| Id | Label | Range / values | Default | Taper / unit | Notes |
|---|---|---|---|---|---|
| `sub.on` | ON | off, on | off | key | |
| `sub.shape` | SHAPE | `sine`, `triangle`, `saw`, `square` | `sine` | menu | polyBLEP for saw and square |
| `sub.oct` | OCT | −1, −2 | −1 | menu | below the note |
| `sub.level` | LEVEL | 0–1 | 0.6 | lin, % (gain n²) | mod |
| `sub.toFilter` | → FILTER | off, on | on | key | |
| `noise.on` | ON | off, on | off | key | |
| `noise.color` | COLOR | `white`, `pink` | `white` | menu | |
| `noise.level` | LEVEL | 0–1 | 0.3 | lin, % (gain n²) | mod |
| `noise.toFilter` | → FILTER | off, on | on | key | |

The sub plays the note (with glide, bend, DRIFT and `osc.pitch`) one or two octaves down; it ignores the
oscillators' OCT, SEMI and FINE. Noise is unpitched.

### 3.5 Filter (ids `filter.*`, one filter per voice)

| Id | Label | Range / values | Default | Taper / unit | Notes |
|---|---|---|---|---|---|
| `on` | ON | off, on | on | key | off = sources go straight to the amp |
| `type` | TYPE | `lp24`, `lp12`, `hp12`, `bp12`, `notch` | `lp24` | menu | |
| `cutoff` | CUTOFF | 20–20 000 Hz | 12 000 Hz | log, Hz / kHz | mod |
| `reso` | RESO | 0–1 | 0 | lin, % | mod |
| `drive` | DRIVE | 0–1 | 0 | lin, 0–24 dB | mod |
| `keyTrack` | KEY | 0–1 | 0 | lin, % | cutoff + keyTrack × (note − 60) / 12 octaves |
| `envAmt` | ENV AMT | −1…+1 | 0 | lin, ±8 oct | cutoff + envAmt × 8 × ENV2 octaves |

- `lp24` is a Moog-style ladder: four one-pole stages with tanh saturation, resonance feedback scaled so that
  RESO 1.0 self-oscillates, and half of the passband loss from resonance made up.
- `lp12`, `hp12`, `bp12` and `notch` are a topology-preserving state-variable filter; Q runs from 0.5 to 25
  (exponential in RESO).
- DRIVE is a pre-gain of 0–24 dB into a tanh stage, followed by half that gain as makeup.
- Order of cutoff maths, in octaves from the knob value: matrix routes, then ENV AMT, then KEY TRACK, then
  DRIFT; the result is clamped to 20 Hz – 20 kHz (and below 0.45 × the sample rate).
- `filter.rs` exposes the linear magnitude response of each type for the display (drive changes only the
  level of the drawn curve).

### 3.6 Amp (ids `amp.*`)

| Id | Label | Range | Default | Taper / unit | Notes |
|---|---|---|---|---|---|
| `level` | LEVEL | 0–1 | 0.8 | lin, % (gain n²) | mod |
| `velSens` | VEL | 0–1 | 0.7 | lin, % | gain × ((1 − s) + s·v²), v = velocity / 127 |

ENV1 always drives the amp.

### 3.7 Envelopes (ids `env1.*`, `env2.*`, `env3.*`)

ENV1 is the amp envelope, ENV2 the filter envelope (ENV AMT) and also a free source, ENV3 is free.

| Id | Label | Range | Default ENV1 / ENV2 / ENV3 | Taper / unit |
|---|---|---|---|---|
| `attack` | A | 0–10 s | 2 ms | cubic, ms / s (never under 0.5 ms) |
| `decay` | D | 0–10 s | 500 ms | cubic |
| `sustain` | S | 0–1 | 1.0 / 0 / 0 | lin, % |
| `release` | R | 0–20 s | 150 ms | cubic (never under 0.5 ms) |

- The attack rises convex (aims at 1.3 and stops at 1.0, so it reaches 1.0 in ATTACK). Decay and release are
  exponential and reach 1 % of the way to their target in DECAY or RELEASE (the same definition as today's
  synth).
- A new note on a busy voice restarts the attack from the current level, so it never clicks (as today).
- On screen each envelope is four faders (section 6.3).

### 3.8 LFOs (ids `lfo1.*`, `lfo2.*`)

| Id | Label | Range / values | Default LFO1 / LFO2 | Taper / unit | Notes |
|---|---|---|---|---|---|
| `shape` | SHAPE | `sine`, `triangle`, `sawUp`, `sawDown`, `square`, `sampleHold`, `smoothRandom` | `sine` / `triangle` | menu | |
| `rate` | RATE | 0.01–50 Hz | 5.5 Hz / 0.5 Hz | log, Hz | mod (rules in 4.3) |
| `sync` | SYNC | off, on | off | key | rate follows `set.pattern.bpm` |
| `syncDiv` | DIV | `1/32`, `1/16T`, `1/16`, `1/16D`, `1/8T`, `1/8`, `1/8D`, `1/4T`, `1/4`, `1/4D`, `1/2`, `1/1`, `2/1`, `4/1`, `8/1` | `1/4` | menu | while SYNC is on, the RATE knob steps through these |
| `mode` | MODE | `trig`, `free`, `oneShot` | `trig` / `free` | menu | below |
| `delay` | DELAY | 0–5 s | 0 | cubic, s | fade-in after note-on |
| `phase` | PHASE | 0–1 | 0 | lin, 0–360° | start phase |
| `polarity` | POL | `bi`, `uni` | `bi` | key | bi: −1…+1, uni: 0…1 |

- `trig`: one instance per voice, restarted on each note.
- `free`: one shared instance that runs whether or not notes play (DELAY does not apply).
- `oneShot`: per voice, runs one cycle from note-on, then holds its last value.
- Tempo sync follows the sequencer's BPM (`set.pattern.bpm`) even while the sequencer is stopped.

### 3.9 Voice control (ids `voice.*`)

| Id | Label | Range / values | Default | Taper / unit | Notes |
|---|---|---|---|---|---|
| `mode` | MODE | `poly`, `mono`, `legato` | `poly` | menu | |
| `glide` | GLIDE | 0–2 s | 0 | cubic, ms / s | portamento time |
| `glideMode` | GLIDE MODE | `always`, `legato` | `always` | menu | mono and legato only |
| `bendRange` | BEND | 0–24 | 2 | int, st | same up and down |
| `drift` | DRIFT | 0–1 | 0 | lin, % | analog wander |
| `volumeDb` | VOLUME | −60…+6 dB | 0 dB | lin, dB | −60 = silence; after the FX |

- **POLY**: 16 voices. With GLIDE above 0, a new note glides from the last note played.
- **MONO**: one voice, last-note priority; every new note retriggers the envelopes. Releasing the newest note
  returns to the previous held note.
- **LEGATO**: like MONO, but a note played while another is held changes pitch without retriggering.
- **GLIDE MODE** `legato`: glide only between overlapping notes.
- **Voice stealing**: the oldest released voice first, then the oldest held voice, faded out over 3 ms
  (as today).
- **Sustain pedal**: while it is down, note-offs are held back; pedal up releases every held-back note that is
  not still pressed. A note struck again while it sustains retriggers its own voice.
- **DRIFT**: each voice has two slow random walks (0.1–0.3 Hz) that move its pitch by up to ±8 cents × DRIFT and
  its cutoff by up to ±0.3 octave × DRIFT.

## 4. Modulation

### 4.1 Sources

| Id | Label | Range | Scope | Colour |
|---|---|---|---|---|
| `env1` | ENV1 | 0…1 | per voice | `#ff5b1f` |
| `env2` | ENV2 | 0…1 | per voice | `#ffb21a` |
| `env3` | ENV3 | 0…1 | per voice | `#f2e15b` |
| `lfo1` | LFO1 | −1…+1 (bi) or 0…1 (uni) | per voice (`trig`, `oneShot`) or global (`free`) | `#41c8ff` |
| `lfo2` | LFO2 | as LFO1 | as LFO1 | `#7f8cff` |
| `vel` | VEL | 0…1 (velocity / 127) | per voice | `#5cff9d` |
| `note` | NOTE | −1…+1 ((note − 60) / 60, clamped) | per voice | `#3fe0c5` |
| `rand` | RAND | −1…+1, new value at note-on | per voice | `#d77bff` |
| `wheel` | WHEEL | 0…1 (mod wheel) | global | `#ff7fb0` |
| `bend` | BEND | −1…+1 (pitch wheel) | global | `#c9d1d9` |

### 4.2 Destinations

27 destinations, in this order (the order of `live` in `synth-telemetry`):

`oscA.pos`, `oscA.pitch`, `oscA.fine`, `oscA.warpAmt`, `oscA.detune`, `oscA.blend`, `oscA.width`,
`oscA.level`, `oscA.pan`, the same nine for `oscB`, `osc.pitch` (both oscillators and the sub), `sub.level`,
`noise.level`, `filter.cutoff`, `filter.reso`, `filter.drive`, `amp.level`, `lfo1.rate`, `lfo2.rate`.

Not destinations in v1: selectors (tables, warp modes, filter type, modes), envelope times, voice controls and
every FX control (FX is global while voices are per note).

### 4.3 Maths

- For a knob destination: `live = clamp01(n + Σ amount × source × aux)`, where `n` is the knob position,
  `aux` is 1 for a route without one, and the result is mapped through the knob's taper. So +100 % sweeps the
  whole knob range, and cutoff moves in log space.
- For the pitch destinations (`oscA.pitch`, `oscB.pitch`, `osc.pitch`): semitones = 24 × Σ amount × source ×
  aux (100 % = ±24 st), added to OCT, SEMI and FINE; the total is clamped to ±48 st.
- ENV AMT and KEY TRACK act on cutoff in octaves after the matrix (section 3.5).
- A FREE LFO's rate can only be moved by global sources (WHEEL, BEND, the other LFO when it is FREE). Other
  routes to it are drawn greyed with the tooltip "A per-note source can't move a FREE LFO" and ignored.
- While an LFO is in SYNC, routes to its RATE are greyed and ignored.

### 4.4 Matrix

- 16 slots, a fixed array (no allocation). A route is
  `{ source, dest, amount (−1…+1), aux (none or a source), bypass }`. `dest` may be none (a fresh row from
  `+ ADD ROUTE`); such a route does nothing until a destination is picked.
- AUX scales the amount by a second source. The classic example: LFO1 → `osc.pitch` with AUX WHEEL is
  mod-wheel vibrato, which is how today's wheel behaviour is kept (section 8.3).
- A source may drive several destinations and a destination may take several routes.

### 4.5 Interaction

- **Source strip** in the synth header, visible on every page: `ENV1 ENV2 ENV3 LFO1 LFO2 VEL NOTE RAND WHEEL
  BEND`, each badge in its colour with a ≡ grip and a tiny live meter. The ENV and LFO section titles are grips
  too.
- **Drop a badge on a knob** that is a destination: a route is created at +25 % and becomes the knob's selected
  route. Knobs that are not destinations do not accept the drop (no highlight). A 17th route shows the notice
  "Matrix full (16 routes)".
- **Arcs**: each route draws an arc inside the knob's ring, in its source's colour, from the knob value to the
  value plus the amount (negative amounts go the other way). Up to 3 arcs are drawn, newest outermost; more
  routes show a `+n` tag.
- **Live dot**: rides the knob's track at the live value (from `live`) while notes play.
- **Depth**: Alt+drag on the knob changes the depth of its selected route (the same drag law as the knob,
  Shift for fine). Chips under a knob with routes list them; clicking a chip selects that route.
- **Right-click a knob**: a menu with its routes (source and amount; × deletes one), then Learn MIDI CC and
  Reset (the existing items).
- **MATRIX page**: one row per route: source ▾, destination ▾ (grouped by section), amount LCD (drag; shown in
  units, e.g. `+0.40 st`, `+1.5 oct`, `+25 %`), aux ▾ (— or a source), BYPASS key, × delete, and the source's
  live meter. An `+ ADD ROUTE` key adds an empty row (source ENV2, no destination until one is picked).

## 5. FX chain (ids `fx.drive.*`, `fx.chorus.*`, `fx.delay.*`, `fx.reverb.*`)

Global and stereo, after the voices, in the fixed order DRIVE → CHORUS → DELAY → REVERB. Every unit has an ON
key (default off) and a MIX knob.

| Unit | Id | Label | Range / values | Default | Taper / unit |
|---|---|---|---|---|---|
| DRIVE | `type` | TYPE | `soft`, `tube`, `hard`, `fold` | `soft` | menu |
| | `amount` | DRIVE | 0–1 | 0.3 | lin, 0–36 dB |
| | `tone` | TONE | 500–20 000 Hz | 20 000 Hz | log (low-pass after the shaper) |
| | `mix` | MIX | 0–1 | 1.0 | lin, % |
| CHORUS | `rate` | RATE | 0.05–5 Hz | 0.6 Hz | log |
| | `depth` | DEPTH | 0–1 | 0.5 | lin, % |
| | `delay` | DELAY | 5–30 ms | 12 ms | lin |
| | `feedback` | FDBK | 0–0.9 | 0 | lin, % (high values give a flanger) |
| | `mix` | MIX | 0–1 | 0.5 | lin, % |
| DELAY | `time` | TIME | 1–2000 ms | 375 ms | log |
| | `sync` | SYNC | off, on | off | key (tempo = `set.pattern.bpm`) |
| | `syncDiv` | DIV | `1/32` … `1/2` (the LFO list up to `1/2`) | `1/8D` | menu; longer than 2 s is clamped to 2 s |
| | `feedback` | FDBK | 0–0.95 | 0.35 | lin, % |
| | `pingPong` | PING-PONG | off, on | off | key |
| | `tone` | TONE | 500–20 000 Hz | 6 000 Hz | log (damping inside the feedback loop) |
| | `mix` | MIX | 0–1 | 0.25 | lin, % |
| REVERB | `size` | SIZE | 0–1 | 0.6 | lin, % |
| | `decay` | DECAY | 0.2–20 s | 2.5 s | log |
| | `damp` | DAMP | 0–1 | 0.5 | lin, % |
| | `preDelay` | PRE | 0–200 ms | 10 ms | lin |
| | `mix` | MIX | 0–1 | 0.25 | lin, % |

- DRIVE runs at 2× oversampling to keep aliasing down. CHORUS is two modulated delay lines per side. REVERB is
  an 8-line feedback delay network (Hadamard mixing, slowly modulated lines) for a dense, smooth tail.
- A unit that is off is skipped (no cost). A unit that is on stops processing once its input is silent and its
  tail has fallen below −100 dB, and resumes on the next non-silent input.
- A patch load clears every FX buffer after the voices have faded (5 ms).
- FX controls are not modulation destinations in v1. The LCXL (DELAY MIX, REVERB MIX) and MIDI learn can still
  move them.

## 6. PLAY layout

### 6.1 Size rules

```
┌ LAUNCHPAD ─────┐┌ MIX ───────┐┌ SYNTH ─────────────────────────────────┐┌TELE┐
│ ▢▢▢▢▢▢▢▢ ○     ││1 2 3 4 … M ││ header · source strip · scope          ││    │
│ ▢▢▢▢▢▢▢▢ ○     ││┃ ┃ ┃ ┃   ┃ ││ OSC A │ OSC B │ SUB · NOISE            ││    │
│ ▢▢▢▢▢▢▢▢ ○     ││M M M M     ││ FILTER (curve) │ AMP │ VOICE           ││    │
│ ▢▢▢▢▢▢▢▢ ○     ││S S S S     ││ ENV1 ENV2 ENV3 │ LFO1 │ LFO2           ││    │
└────────────────┘└────────────┘└────────────────────────────────────────┘└────┘
[KEYS: wheels · patch LCD · 88-key keybed: same place and size as today        ]
[SEQUENCER: unchanged                                                          ]
```

The solver (`src/lib/layout.ts`) applies these rules in order:

1. **Vertical split from the height alone.** The pad size the window height allows (`pH`, today's height
   term) sets the height of the Launchpad row, the keybed (`kh = clamp(round(0.9 × pH), 44, 110)`) and the
   sequencer, exactly as today. The keys plate and sequencer therefore keep their place and size; when the
   pads end up smaller than `pH` (rule 2), the Launchpad plate is centred vertically in its row.
2. **The synth is never narrower than 480 px.** To keep that, in this order: the pads shrink (floor 36 px);
   a docked drawer floats instead; the telemetry column folds to its 30 px strip; the slim mixer collapses to
   a 40 px meter strip whose click opens the full slim mixer as a popover.
3. **The synth grows to 760 px** (the single-page layout) while the pads stay at 56 px or more.
4. **Any width left** goes to the pads (up to `pH`), then to the synth.

Expected results (to be pinned by solver tests):

| Window | Drawer | Pads | Synth |
|---|---|---|---|
| 2560×1440 | floating | 94 px (as today) | ≈ 840 px, single page |
| 2560×1440 | docked | ≈ 65 px | ≈ 760 px, single page |
| 1920×1080 | floating | 56 px | ≈ 590 px, two pages |
| 1280×720 | floating | 36 px; telemetry and mixer collapsed | ≈ 550 px |

### 6.2 Slim mixer (replaces `MixerStrip` on PLAY)

- About 300 px wide: 8 channel strips plus MASTER, as tall as the Launchpad row.
- A strip, top to bottom: the channel number chip in its group colour (the name in its tooltip; a dot in the
  keys colour when the keys play through the channel, as today), a tall fader with its level fill, the dB
  value, MUTE and SOLO keys (`M` / `S`).
- Clicking a chip opens the **channel popover**: the name (click to rename, as today), the PAN, PITCH and
  LOW-PASS knobs (same ranges and learn ids as today: `group.<i>.pan`, `group.<i>.pitch`,
  `group.<i>.lowpass`), then today's channel menu items (pads in the group, ✓ Keys, Rename…). Esc or a click
  outside closes it.
- Faders, mute and solo keep their learn ids and the LCXL mapping. Level fills follow the engine as today.
- The collapsed form (rule 2) is a 40 px strip of 8 level dots and the master meter; a click opens the slim
  mixer as a popover.

### 6.3 Synth plate

**Header** (always visible, two lines):

- The patch LCD (name, `*` when modified, coloured by category) with ◀ ▶ (previous and next patch in the menu
  order), a SAVE key and a ▾ menu (section 8.4).
- The page keys.
- The source strip (section 4.5).
- A mini scope (synth output), `VOICES n/16`, `CPU n %` and the engine lamp (SYNTH or SOUNDFONT).

**Pages** (the plate picks a page set from its size; the chosen page is remembered per page set in
`localStorage` as a convenience):

| Page set | When | Pages |
|---|---|---|
| wide | synth ≥ 760 px wide and plate ≥ 600 px tall | MAIN (oscillators; filter, amp and voice; envelopes and LFOs), FX, MATRIX |
| narrow | otherwise, plate ≥ 480 px tall | OSC (oscillators; filter, amp, voice), MOD (envelopes and LFOs, larger displays), FX, MATRIX |
| compact | plate under 480 px tall | OSC (oscillators), FILTER (filter, amp, voice), MOD, FX, MATRIX |

Every control of a page must be visible without scrolling at 2560×1440, 1920×1080 and 1280×720. Knobs scale
between 24 and 48 px with the plate.

**Sections** (MAIN page, top to bottom):

- **OSC A, OSC B**: display, ON, TABLE LCD (menu), POS (large), WARP LCD (menu) and WARP AMT, UNISON stepper,
  DETUNE, BLEND, WIDTH, PHASE RAND, OCT / SEMI steppers, FINE, LEVEL, PAN, → FILTER key.
- **SUB · NOISE**: ON, SHAPE LCD, OCT LCD, LEVEL, → FILTER; ON, COLOR LCD, LEVEL, → FILTER.
- **FILTER**: display, ON, TYPE LCD, CUTOFF (large), RESO, DRIVE, KEY, ENV AMT.
- **AMP**: LEVEL, VEL. **VOICE**: MODE LCD, GLIDE, GLIDE MODE, BEND stepper, DRIFT, VOLUME.
- **ENV1 · AMP, ENV2 · FILTER, ENV3**: display, then A, D, S and R as four vertical faders (a new linear
  `ParamFader` component; the dB-law `Fader` stays for the mixer).
- **LFO1, LFO2**: display, SHAPE LCD, RATE (shows the division when SYNC is on), SYNC key, MODE LCD, DELAY,
  PHASE, POL key.
- **FX page**: four units side by side (two by two in the narrow and compact sets), each with its ON key.
- **MATRIX page**: section 4.5.

All synth knobs and faders are `role="slider"` with `aria-valuetext` in units, keyboard-operable like today's
knobs (arrows, Shift fine, Home / End), double-click resets to the default, and are MIDI-learnable.

### 6.4 Keys plate changes

- The sound LCD becomes the **patch LCD**: the patch name in its category colour; click or Enter opens the same
  patch menu as the synth header. Held keys light in the category colour (learned keys keep their pad's colour).
- While the SoundFont engine plays, the LCD shows and picks the six GM programs, as today.
- Everything else (wheels, volume, octave, output, keybed, fold) is unchanged. The MOD wheel keeps its learn
  id `keys.mod` and now feeds the WHEEL source.

### 6.5 Category colours (LED 0–63 triples, like today's sound colours)

| Category | Colour | Category | Colour |
|---|---|---|---|
| PIANO | [63, 63, 63] | LEAD | [0, 63, 25] |
| KEYS | [63, 40, 0] | BASS | [0, 30, 63] |
| ORGAN | [63, 20, 0] | PLUCK | [63, 63, 0] |
| PAD | [25, 0, 63] | FX | [63, 0, 40] |

## 7. Visualizers

| Display | Shows | Data |
|---|---|---|
| OSC (A and B) | the table's frames stacked in pseudo-3D, the frame at POS bright; SYNC, BEND and PWM warp previewed on that frame; a live dot on the frame stack follows modulated POS | frames from `synth_table`; live POS from `live` |
| FILTER | the response curve (20 Hz–20 kHz, dB grid) over the live spectrum of the synth output; with no note playing the curve uses the knob values. Dragging on the display sets CUTOFF (horizontal) and RESO (vertical) | `curve`, `spectrum`, `cutoffHz` |
| ENV (1–3) | the ADSR curve drawn from the faders (curves as in 3.7), with a dot at the newest voice's stage and level | params; `env` |
| LFO (1–2) | one cycle of the shape with DELAY and PHASE applied, with a moving phase dot; random shapes show a row of sampled steps | params; `lfo` |
| Knob rings | route arcs and the live dot (section 4.5) | routes; `live` |
| Header scope | the synth's output waveform, trigger-aligned | `scope` |
| Source strip and MATRIX rows | a tiny live meter per source | `sources` |

- Style: the phosphor look of the telemetry column (pixel crush and glow), drawn by one shared animation loop
  for all synth canvases. It stops while the feed is off (window hidden or another mode); the displays then
  show STANDBY, like the telemetry column.
- With reduced motion, dots, rings and meters update at 10 Hz instead of every frame.
- The telemetry column keeps showing the master output, unchanged.

## 8. Patches and storage

### 8.1 Patch format

A patch is JSON (serde, camelCase), the same shape in the set file and in a user patch file:

```json
{
  "patchVersion": 1,
  "name": "MOOG BASS",
  "category": "bass",
  "params": {
    "oscA": { "on": true, "table": "basic", "pos": 0.667, "oct": 0, "semi": 0, "fine": 0,
              "warp": "off", "warpAmt": 0, "unison": 1, "detune": 0.25, "blend": 0.75, "width": 0.5,
              "phaseRand": 1, "level": 0.75, "pan": 0, "toFilter": true },
    "oscB": { "...": "same fields" },
    "sub": { "on": true, "shape": "sine", "oct": -1, "level": 0.5, "toFilter": true },
    "noise": { "on": false, "color": "white", "level": 0.3, "toFilter": true },
    "filter": { "on": true, "type": "lp24", "cutoff": 300, "reso": 0.3, "drive": 0.4, "keyTrack": 0, "envAmt": 0.45 },
    "amp": { "level": 0.8, "velSens": 0.7 },
    "env": [ { "attack": 0.002, "decay": 0.5, "sustain": 1, "release": 0.15 }, { "...": "" }, { "...": "" } ],
    "lfo": [ { "shape": "sine", "rate": 5.5, "sync": false, "syncDiv": "1/4", "mode": "trig",
               "delay": 0, "phase": 0, "polarity": "bi" }, { "...": "" } ],
    "voice": { "mode": "mono", "glide": 0.06, "glideMode": "legato", "bendRange": 2, "drift": 0.1, "volumeDb": 0 },
    "routes": [ { "source": "lfo1", "dest": "osc.pitch", "amount": 0.021, "aux": "wheel", "bypass": false } ],
    "fx": { "drive": { "on": false, "type": "soft", "amount": 0.3, "tone": 20000, "mix": 1 },
            "chorus": { "...": "" }, "delay": { "...": "" }, "reverb": { "...": "" } }
  }
}
```

`"..."` entries abbreviate objects that repeat the fields shown above them.

Values are stored in their units (Hz, seconds, dB, semitones; levels and other 0–1 controls as their 0–1
value), not as knob positions. Names are stored
uppercase, at most 24 characters. Missing fields take their defaults and unknown fields are ignored. A patch
whose `patchVersion` is newer than the app knows is refused with "Patch made by a newer version". Values out of
range are clamped on load.

### 8.2 Set file v2 and migration

`KeysSettings` becomes:

| Field | Type | Meaning |
|---|---|---|
| `patch` | Patch | the current synth patch |
| `patchSource` | `{ factory: id }` \| `{ user: file }` \| `init` | where it came from (REVERT, SAVE) |
| `modified` | bool | edited since it was loaded or saved (the `*`) |
| `gmSound` | `KeysSound` | the GM program the SoundFont engine plays (today's `sound`) |
| `volumeDb` | f32 | unchanged |
| `output` | `Option<u8>` | unchanged |

`SET_VERSION` becomes 2. A version 1 set is migrated when it is opened: `sound` becomes both `gmSound` and the
factory patch of the same name (`patchSource` = that factory id, `modified` = false); `volumeDb` and `output`
are kept. The set is saved in version 2 on its next save. A new set starts with the E.PIANO factory patch (the
current default sound).

### 8.3 Factory patches

Built in and read-only. The recipes are starting points, tuned by ear during implementation; the user approves
the final sounds. Every factory patch keeps today's mod-wheel vibrato: a route from a 5.5 Hz sine LFO in
`trig` mode to `osc.pitch` at 2.1 % (±0.5 st) with AUX WHEEL, on whichever LFO the patch does not otherwise use.

| Id | Name | Category | Recipe |
|---|---|---|---|
| `piano` | PIANO | PIANO | A HARMONICS pos 0.15; B BASIC triangle +12 st level 0.3; LP 12 3 kHz, KEY 50 %, ENV AMT +30 %; ENV1 2 ms / 1.2 s / 0 / 250 ms; ENV2 1 ms / 0.8 s / 0; VEL → CUTOFF +20 %; reverb 15 % |
| `ePiano` | E.PIANO | KEYS | A BASIC sine, WARP FM 0.1; B on, BASIC sine +12 st, level 0 (FM source only); ENV3 1 ms / 0.4 s / 0 → A WARP AMT +30 % (bright attack that mellows); ENV1 2 ms / 1.5 s / 30 % / 400 ms; chorus 30 % |
| `organ` | ORGAN | ORGAN | A ORGAN pos 0.5; filter off; ENV1 5 ms / 0 / 100 % / 5 ms; chorus rate 5.5 Hz depth 0.7, 40 % |
| `synthPad` | SYNTH PAD | PAD | A BASIC saw, unison 3, detune 0.26 (±7 cents), width 0.8; LP 12 2.5 kHz; ENV1 300 ms / 0 / 100 % / 1.2 s; LFO2 free triangle 0.3 Hz → CUTOFF +10 %; chorus 30 %; reverb 35 %, decay 4 s |
| `synthLead` | SYNTH LEAD | LEAD | A BASIC square; B BASIC saw level 0.55; LP 24 4 kHz, reso 20 %; ENV1 5 ms / 200 ms / 70 % / 150 ms; LEGATO, glide 40 ms; delay 1/8D 20 % |
| `synthBass` | SYNTH BASS | BASS | A BASIC saw −1 oct; LP 24 800 Hz; ENV1 3 ms / 300 ms / 60 % / 80 ms |
| `moogBass` | MOOG BASS | BASS | A BASIC saw; B BASIC square −1 oct, fine −7; SUB sine −1 level 0.5; LP 24 300 Hz, reso 30 %, drive 40 %, ENV AMT +45 %; ENV2 0 / 350 ms / 0 / 120 ms; MONO, glide 60 ms legato; DRIFT 10 % |
| `acidBass` | ACID BASS | BASS | A BASIC saw; LP 24 400 Hz, reso 75 %, drive 30 %, ENV AMT +60 %; ENV2 1 ms / 200 ms / 0; VEL → CUTOFF +25 %; MONO, glide 80 ms legato; FX drive tube 30 % |
| `wobbleBass` | WOBBLE BASS | BASS | A BASIC saw, unison 3, detune 0.2; B BASIC square −1 oct; LP 24 250 Hz, reso 40 %; LFO1 sync 1/8 sine → CUTOFF +55 %; FX drive fold 25 % |
| `supersaw` | SUPERSAW | LEAD | A BASIC saw, unison 7, detune 0.45, width 1, blend 0.8; B BASIC saw +12 st, unison 7, detune 0.3, level 0.5; LP 12 6 kHz; ENV1 5 ms / 0 / 100 % / 300 ms; delay 1/4 18 %; reverb 25 % |
| `syncLead` | SYNC LEAD | LEAD | A BASIC saw, WARP SYNC 0.3, ENV2 → A WARP AMT +40 %; B BASIC square −12 st level 0.4; LP 24 5 kHz; MONO, glide 30 ms; delay 15 % |
| `pluck` | PLUCK | PLUCK | A BASIC saw, unison 3, detune 0.15; B HARMONICS pos 0.6 level 0.4; LP 24 600 Hz, ENV AMT +55 %; ENV1 1 ms / 450 ms / 0 / 300 ms; ENV2 1 ms / 250 ms / 0; reverb 20 % |
| `brass` | BRASS | LEAD | A BASIC saw; B BASIC saw fine +8; LP 24 900 Hz, ENV AMT +35 %; ENV2 80 ms / 400 ms / 40 %; ENV1 40 ms / 0 / 90 % / 200 ms; reverb 15 % |
| `strings` | STRINGS | PAD | A BASIC saw, unison 5, detune 0.22, width 0.9; B PWM pos 0.3, unison 3; LP 12 3.5 kHz; ENV1 450 ms / 0 / 100 % / 900 ms; chorus 40 %; reverb 35 % |
| `vowelPad` | VOWEL PAD | PAD | A VOWELS pos 0.5, unison 3; LFO2 free triangle 0.15 Hz → A POS +50 % (sweeps all five vowels); LP 12 4 kHz; ENV1 600 ms / 0 / 100 % / 1.5 s; reverb 45 % |
| `fmBell` | FM BELL | KEYS | A BASIC sine, WARP FM 0.15; B on, BASIC sine at ratio 3.5 (+1 oct, +9 st, +69 cents), level 0 (FM source only); ENV3 1 ms / 1.5 s / 0 → A WARP AMT +45 %; ENV1 1 ms / 2.5 s / 0 / 1.5 s; reverb 30 % |

INIT is every parameter at its default (a plain saw into an LP 24 at 12 kHz, no FX, no routes).

### 8.4 User library and patch menu

- Folder: `<Documents>\Launchpad Drummer\Patches\`, created on first save. One `<NAME>.lbpatch` file (JSON,
  section 8.1) per patch; the file name is the name with characters invalid on Windows replaced by `_`.
- **Patch menu** (synth header ▾, and the keys plate LCD): FACTORY ▸ (by category), USER ▸ (alphabetical),
  SAVE, SAVE AS…, RENAME…, DELETE, INIT, REVERT, SHOW FOLDER.
  - SAVE overwrites the user file the patch came from; for a factory or INIT patch it acts as SAVE AS.
  - SAVE AS asks for a name inline. If the name exists, the field says "Exists — SAVE again to replace" and a
    second SAVE overwrites.
  - RENAME and DELETE apply to user patches only (greyed otherwise). DELETE leaves the current sound playing
    and turns its source into `init`.
  - REVERT reloads the source. If a user file has gone, the notice "Patch file not found" appears and nothing
    changes.
  - SHOW FOLDER uses the existing `reveal_path`.
- ◀ ▶ step through the menu order (factory then user) and load directly; they never ask to confirm.
- An unreadable or invalid file shows the notice "Patch not loaded — reason"; the current patch stays.

### 8.5 Undo

Synth edits are undo steps, unlike mixer moves (`restore_keeping_mixer` keeps the mixer, not the keys).
Knob and fader drags, LCXL turns and learned-CC moves merge per control within 1 s; adding, changing or
deleting a route and loading a patch are one step each. Undo and redo send the whole patch to the engine.

## 9. Keys engine switch (SoundFont)

- SETUP › KEYS gets **ENGINE: SYNTH | SOUNDFONT**. SOUNDFONT is selectable only with a `.sf2` loaded
  (LOAD… and CLEAR as today). The choice is stored in the app config (`keysEngine`, next to `soundfont`).
  Config migration: a config that already names a SoundFont starts in SOUNDFONT, so nothing changes for an
  existing setup.
- The rack also shows the patches folder with a SHOW key.
- While SOUNDFONT plays: the synth plate is dimmed under the overlay "SOUNDFONT ACTIVE · USE SYNTH" (the key
  switches back); its controls are inert, the LCXL knobs do nothing (notice "LCXL knobs drive the synth —
  SoundFont active") and their LEDs are off; the patch stays untouched in the set.
- `KeysStatus.engine` is `'synth'` or `'soundfont'`; `name` is the patch name or the SoundFont name.

## 10. Hardware

### 10.1 Keyboard (KEYS role, e.g. the Keystation 49 MK3)

- Notes play voices; velocity feeds VEL and the amp's VEL SENS.
- The pitch wheel bends by the patch's BEND RANGE and feeds BEND.
- The mod wheel (CC 1) feeds WHEEL; it is no longer hard-wired to vibrato (factory patches route it, 8.3).
- New: the CC 64 sustain pedal (section 3.9), for both engines.
- A keys input that disconnects releases its notes and the pedal, and the wheels return to rest (today's
  `reset_keys_wheels`, extended to the pedal).

### 10.2 Launch Control XL

- Faders 1–8: mixer channel volume. Track focus / control buttons: mute (amber) and solo (green). Unchanged.
- The 24 knobs follow a fixed synth map:

| Row | Knob 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|
| 1 (top) | `oscA.pos` | `oscA.warpAmt` | `oscA.detune` | `oscA.level` | `oscB.pos` | `oscB.warpAmt` | `oscB.detune` | `oscB.level` |
| 2 | `filter.cutoff` | `filter.reso` | `filter.drive` | `filter.envAmt` | `lfo1.rate` | `lfo2.rate` | `fx.delay.mix` | `fx.reverb.mix` |
| 3 | `env1.attack` | `env1.decay` | `env1.sustain` | `env1.release` | `env2.attack` | `env2.decay` | `env2.sustain` | `env2.release` |

- Value law: `n = value / 127` through the control's taper; bipolar controls are centred at 64. An LFO rate in
  SYNC steps through the divisions.
- The knobs are absolute: a knob's first move jumps its control to the knob's position, as the mixer does
  today. Pickup is not included (section 13).
- Knob LEDs (indices 0–23 in the existing LED SysEx, template 0): row 1 amber, row 2 red, row 3 green while the
  synth engine plays; off while the SoundFont plays.
- On screen, each synth control on the map carries a small `XL` tag; its tooltip names the row and knob
  ("LCXL row 2 · knob 1").
- Mixer pan, pitch and low-pass are no longer on the LCXL: screen-only in the channel popover, still learnable
  from other controllers.
- LCXL messages are still ignored by MIDI learn (the map is fixed).

### 10.3 MIDI learn

Every continuous synth control and every ON / SYNC / PING-PONG key is learnable from other controllers, with
the id `synth.<id>` (e.g. `synth.filter.cutoff`, `synth.env1.attack`, `synth.fx.reverb.mix`,
`synth.oscB.on`). Keys toggle on the CC crossing 64, like today's toggles. Selectors (menus) and route amounts
are not learnable in v1. Bindings live in the set, as today, and appear in SETUP › MIDI LEARN.

## 11. Real-time safety and errors

- **Allocation**: `Synth::new` allocates everything the audio thread will touch: voices, unison copies, filter,
  envelope and LFO state, FX buffers (delay up to 2 s and reverb lines, sized for 96 kHz), oversampling
  buffers. Commands are Copy except `SynthLoad(Box<SynthParams>)`, whose box goes back through a trash queue.
  Table changes send an index into the startup registry.
- **No clicks**: levels, cutoff and pan are smoothed over 10 ms; patch loads fade voices for 5 ms first;
  voice stealing fades over 3 ms.
- **Denormals**: flush-to-zero and denormals-are-zero are set on the audio thread.
- **Instability**: a voice whose output turns NaN or infinite is reset and silenced, and `resets` counts it;
  the UI shows "Synth voice reset (unstable settings)" once per session. An FX unit is reset the same way.
- **CPU budget**: the worst-case patch (16 notes, 2 × 7 unison, LP 24 with drive, all four FX on) renders in
  under 25 % of real time on one core at 48 kHz with 128-frame blocks, in a release build. The header shows
  live CPU %. Overload in practice shows in the existing dropout counter.
- **Startup**: the wavetable build adds at most 200 ms (measured against `docs/performance/startup-baseline.md`).
- **Errors**: patch files (8.4); a set that fails to migrate opens with the INIT patch and a notice; no LCXL
  connected means the screen still works; with the feed off the displays show STANDBY.

## 12. Testing

**lpd-core (cargo test)**

- Wavetables: each band-limited copy has no energy above its Nyquist limit; frames are normalized; the build
  takes 200 ms or less in release.
- Oscillator: pitch within ±0.1 cent at A4 = 440 Hz and across the range; unison spread symmetric with the
  outer copies at ±100·d² cents; every warp mode stays within ±1.
- Filter: the measured magnitude of each type is within ±1 dB of the analytic curve (linear range, drive 0);
  the ladder self-oscillates at RESO 1.0; no blow-up under audio-rate cutoff modulation.
- Envelopes within ±1 ms of their stage times; LFO rates, tempo sync and the three modes.
- Voices: allocation, stealing order, POLY / MONO / LEGATO, glide, sustain pedal.
- Matrix: amount × source × aux, clamping, log cutoff mapping, ±24 st pitch, the FREE-LFO and SYNC rules.
- FX: delay time exact to the sample, feedback bounded, reverb decay (RT60) within ±20 %, drive bounded.
- Patches: serde round trip; defaults for missing fields; clamping; a newer `patchVersion` refused; set v1 → v2
  migration; every factory patch renders a C major chord that is non-silent, finite and below 1.0 peak.
- `no_alloc`: note on and off, sustain, param and route changes, patch loads, all FX on, full taps.
- `synth-params.json` is up to date (section 2.4).
- Performance (release only, `#[ignore]` by default): the CPU budget of section 11.

**src-tauri**

- The LCXL knob map reaches the synth while faders still drive the mixer; knob LEDs follow the engine.
- Synth edits, route edits and patch loads undo and redo; LCXL synth turns are undoable.
- Engine switch and its config migration; the SoundFont plays GM programs and ignores synth commands.
- The patch library commands (save, overwrite rule, rename, delete, revert, missing file).

**TypeScript (node:test, as today)**

- Layout solver: the four rows of 6.1, the collapse order of rule 2, keybed and sequencer heights unchanged.
- Patch menu model; arc and ring maths; unit formatters; the LCXL tag map read from `synth-params.json`.

**Browser preview**: it has no engine, so the synth plate shows a demo patch and its displays stay in STANDBY;
it is used to check layout at the three window sizes.

**Hardware checks for the user** (like plan 1C's open items): Keystation velocity, wheels and sustain pedal;
the LCXL knob map and LEDs; ASIO latency with a heavy patch; a 30-minute session with a SUPERSAW chord and FX
and zero dropouts.

## 13. Not included (ask before adding)

WAV import and a wavetable editor; more than 2 LFOs or 3 envelopes; a second filter; modulating FX controls;
reordering FX; LCXL pickup / soft takeover; a user-editable LCXL knob map; aftertouch, MPE or macro knobs;
an arpeggiator; recording the keys into the sequencer; an HQ (oversampled) voice mode; patch tags, search or
favourites; randomize; per-voice FX.

## 14. Risks and open items

- **CPU**: 16 × 2 × 7 oscillator copies plus a nonlinear ladder per voice is the heaviest load in the app; the
  performance test runs early in the plan so the budget can be tuned (fewer unison copies at high polyphony is
  the fallback, decided with the user).
- **LCXL protocol** is still unverified on the unit (plan 1C decision D6), including knob LED indices 0–23.
- **Sound quality** of the factory patches needs the user's ears; piano and organ are approximations.
- **Live displays** can only be checked in the desktop app (the browser preview has no engine).
- **Startup time**: the wavetable build must stay within its 200 ms.

## 15. Build order (for the implementation plan)

1. DSP core: wavetables, oscillator, filter, envelopes, LFOs, matrix, voice, `Synth` with `no_alloc` and the
   performance test.
2. FX chain.
3. Parameters, patches, factory patches, set v2 migration, `synth-params.json`.
4. Router, commands, events, telemetry feed, LCXL map and LEDs, keyboard sustain, engine switch, undo.
5. Layout solver and slim mixer.
6. Synth plate: header, pages, sections, drag-and-drop and matrix.
7. Visualizers.
8. Patch tuning by ear with the user, then the hardware checks.

The plan may split this into two plans (engine: steps 1–4; interface: steps 5–8) if one plan gets too long.

## 16. Changes to existing specs

- **UI spec section 1** (Decisions): "Mixer — always visible … a 1:1 Launch Control XL mirror" becomes the slim
  mixer of 6.2; the "1:1 mirror" of the LCXL columns no longer holds for the knobs.
- **UI spec section 4** (Play layout and its size table): replaced by 6.1 for the Launchpad row; keys plate and
  sequencer rules unchanged.
- **UI spec 4.6** (keys plate): the sound LCD becomes the patch LCD (6.4); "What a key plays" is the synth
  patch; the mod wheel feeds WHEEL instead of fixed vibrato.
- **UI spec 7.1** (mixer strip): replaced on PLAY by the slim mixer and channel popover (6.2).
- **UI spec section 8** (SETUP › KEYS): the ENGINE switch and patches folder (section 9).
- **UI spec section 8** (MIDI learn): `synth.*` ids added (10.3); LCXL still ignored while learning.
- **Engine spec 2.3 / plan 1C Task 11** (LCXL mapping): knob rows now drive the synth (10.2).
- **Plan 1C decision D1**: the built-in synth is replaced by this synth; the SoundFont engine stays.
  D5 still holds.
- **Engine spec section 14** (Part 4, Band mode): unaffected; this synth becomes one of its instruments later.
