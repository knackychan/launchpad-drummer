# Dustline — Ship Engine Sound Design (gears, Keys synth tone, BAKED and LIVE SYNTH)

Date: 2026-11-05
Status: design approved section by section in the brainstorming session of 2026-11-05; written spec awaiting review
Review: checked against the code on 2026-11-05 and 2026-11-06; factual errors were fixed in place (listed in the plan, section 0.3)
Revision: **tone revision R1** (2026-11-06), after the user failed checkpoint 1 ("it sound like an electric razer, the previous
sound was more matching the feel"). It changes sections 1, 2, 3.1, 3.2, 3.4–3.7, 4.1, 5.1, 6, 7 and 8 where marked "R1";
section 10 records what it supersedes and why. Everything else stands.
Game: `prototypes/Dustline` 0.12.0 (engine audio `DustlineEngineAudio` 1.2.0), shipped as three byte-identical CRLF copies
Synth: the Keys synth in `launchpad-drummer/crates/lpd-core/src/synth` (spec `2026-10-24-keys-synth-design.md`)
Listening tool: earshot (`D:\MY DOCS\MY TOOLS\earshot`, skill `skills/earshot/SKILL.md`)

This document fixes the design of a new propulsion sound for Dustline before its implementation plan is written.
The goal is a bassy engine whose pitch follows speed, with a clear zingy top. The character is an F1 racer mixed
with a fighter jet and a spaceship. The design is built so that later ships (ship selection is not part of this
work) are new data, not new code.

## 1. Decisions

| Topic | Decision |
|---|---|
| Synth role | **Both backends, switchable for A/B.** **BAKED**: the Keys synth patch is rendered offline to loops that the game pitches and crossfades. **LIVE SYNTH**: the same patch is played in real time by a dedicated Rust `Synth` instance in the app. |
| Shared layers | Approach A: one ship profile and one gear model drive both backends. Only the **tonal core** differs between them. The ElevenLabs **texture layers** and one-shots play in the game's Web Audio in both modes, so the A/B compares baked against live synth and nothing else. R1: so do the restored 1.2.0 propulsion layers (section 3.7). |
| Pitch | **Gears + boost lift.** F1-style upshifts while accelerating; past the last gear, boost keeps raising the pitch and a jet layer takes over. |
| Scope | **One ship (Kestrel, the current craft) + the ship profile format.** Ship selection comes later. |
| What is replaced | **R1:** the 1.2.0 **afterburner** and the 1.2.0 **spool** (the gear model now sets the pitch of everything). The 1.2.0 mechanical core, turbine, exhaust and jet hiss are **restored** and driven by the gear model (section 3.7); the Keys synth tone sits under them. **Kept from 1.2.0**: airflow/wind, heat strain, damage misfires, vent steam, rival engines, the master compressor and limiter, and the ducking rules. (Before R1: all propulsion was replaced, core, turbine, exhaust and afterburner.) |
| Mode switch | Pause → Sound → **ENGINE: BAKED / LIVE SYNTH**, remembered for the session. LIVE SYNTH appears only inside the app once the bridge reports ready. |
| Your synth patch | LIVE SYNTH prefers a patch named **"Dustline Kestrel"** from the user's Keys patch folder, if one exists, so a tweak saved on PLAY is heard without a rebuild. The bake tool can read the same file. (The app stores patch names in capitals: the file is `DUSTLINE KESTREL.lbpatch`.) |
| Asset sources | Synth loops: rendered by the real Rust `Synth`. Textures and one-shots: ElevenLabs sound effects, screened with earshot by a fixed rule. |
| Versions | Dustline **0.13.0**, engine audio **2.0.0**. `dustline-rift-run-v0.12.html` stays frozen as the "before" reference. |
| Commits | None unless the user asks. |

## 2. Architecture

```
simulation frame (speed, faders, brake, boost, heat, faults, events)
        │
        ▼
  DustlineEngineAudio 2.0  ──  gear model (shared, deterministic)
        │                         │ gear, rpm, fundamental Hz, load, shift events
        ├── tone backend ─────────┤
        │     BAKED: 8 synth loops per engine, crossfaded + playbackRate
        │     LIVE SYNTH: a frame per control step (~40 Hz) → postMessage → App.svelte → Tauri → GameEngineVoice (Rust Synth)
        ├── R1: restored 1.2.0 propulsion (core, turbine, exhaust, jet hiss), spool from the gear rpm, both modes
        ├── texture layers (ElevenLabs loops, pitched by rpm)
        ├── one-shots (upshift pop, downshift blip, boost ignite/release, launch)
        └── kept 1.2.0 layers (airflow, strain, misfire, vent) → existing master (glue comp, limiter, 0.75)
```

### 2.1 Interface

The new engine keeps the 1.2.0 interface, so the game wiring changes little:
`DustlineEngineAudio.create(ctx, {output}) → {update(frame, dt, now), event(name, data, now), setActive(on, now), reset(now), dispose()}`
and `DustlineEngineAudio.createMaster(ctx, dest)`. The frame built by `fillAudioFrame()` is unchanged except for one
new field, `engineMode` (`'baked'` or `'live'`). The game events already routed to the engine (`ignite`,
`release`, `pad`, `launch`, `cue`, `vent`, `restore`, `duck`) keep their names. Two new internal events, `upshift`
and `downshift`, come from the gear model, not from the game.

The gear model is exported as `DustlineEngineAudio.gears` and the profiles as `DustlineEngineAudio.ships`, so the
Node checks can load them from the marked block (`/* @dustline-engine-audio:begin */ … end */`), as the current
checks do.

### 2.2 Ship profile format

A ship profile is plain data. A future ship is a new profile plus its assets.

| Field | Kestrel value / meaning |
|---|---|
| `id`, `name` | `'kestrel'`, `'KESTREL'` |
| `gears.tops` | Gear top speeds in km/h: `[80, 150, 220, 300, 390, 490, 700]` |
| `gears.idleHz`, `gears.redlineHz` | 41, 124 (core fundamental) |
| `gears.boostExponent` | 0.8 (lift past the last gear, section 3.1) |
| `gears.upMin`, `gears.downMin` | 0.22 s, 0.09 s (minimum time between shifts) |
| `gears.overRev` | 1.03 (rpm cap while an upshift waits) |
| `gears.pace` | 250 km/h per second (the fastest the paced speed rises in gears 1–6, section 3.1) |
| `patch` | The Keys synth patch, embedded: the `.lbpatch` JSON `{patchVersion: 1, name, category, params}` that `synth_save_patch` writes |
| `patchName` | `'DUSTLINE KESTREL'` (patch names are stored in capitals; LIVE SYNTH looks for `DUSTLINE KESTREL.lbpatch` in the user's patch folder) |
| `liveNote` | 48 (MIDI note held by LIVE SYNTH; it sounds twice the fundamental, section 5.1; bend range forced to 24) |
| `baked` | 8 entries `{id, anchorHz, load: 'on'|'off', loopStart, loopEnd}`; anchors 48 / 75 / 117 / 182 Hz |
| `textures` | Entries `{id, layer: 'f1'|'jet'|'drone', refRpm, baseHz, loopStart, loopEnd, weight}` |
| `oneShots` | `{upshift: [3 ids], downshift: [2 ids], ignite, release, launch}` |
| `mix` | Layer levels in dB, shift cut depth, pan of the two engines (±0.34), right-engine pitch offset (+0.35 %) |

Assets are stored as embedded base64 Ogg Opus, in a bundle beside the existing 29-sound bundle (`DUSTLINE_SFX_B64`).

## 3. Sound design

### 3.1 Gear model

Speed `v` is in km/h (`sim.state.speed × 3.6`). Gear `g` runs from 1 to 7, plus a neutral state used on the
menu and during the countdown.

- **Paced speed** (user decision in the 2026-11-05 review): in gears 1–6 the rules below read a paced speed `vp`
  instead of `v`. `vp` rises at most `gears.pace` (250 km/h per second) and falls instantly (`vp = v` whenever
  `v ≤ vp`). In 7th, at GO, on recovery and on restart, `vp = v`. Effect at launch: upshifts at 0.19, 0.54 and
  0.93 s, each with a full sweep and an audible drop, never at the over-rev cap; the pitch lags real speed by up to
  23 % during the first second.
- **Gear from speed**: the highest gear whose downshift point lies below `v`: start at 1st and step up while
  `g < 7` and `v > 0.92 × tops[g − 1]` (245 km/h → 4th, 432 km/h → 6th, 600 km/h → 7th). Neither shift rule then
  fires on the next step. (The earlier rule, "the lowest gear with `v ≤ 0.85 × tops[g]`", gave 7th at 432 km/h,
  which the downshift rule dropped to 6th at once.)

- **Within a gear** pitch is proportional to speed: `r = v / tops[g]`, fundamental `f = redlineHz × r`, never
  below `idleHz` (gear 1 slips below 41 Hz). `rpm = f / redlineHz`.
- **Cruise** (600 km/h, no boost) sits at r = 0.857 in 7th: off the limiter, so speed changes stay audible.
- **Boost lift**: in 7th above 700 km/h, `f = 124 × (v / 700)^0.8`. It is continuous at 700 km/h and reaches about
  191 Hz at 1200 km/h. There are no more shifts.
- **Upshift**: when `r ≥ 0.97`, the craft is accelerating (speed rising, or throttle ≥ 0.3), `g < 7`, and at
  least 0.22 s have passed since the last shift. While an upshift waits, `r` is capped at 1.03 (pitch holds
  instead of following speed). The 0.22 s minimum exists because acceleration is steep: from a standstill the craft
  reaches 526 km/h in one second, and a race starts at 245 km/h at GO (`LAUNCH_SPEED` 68 m/s × throttle,
  `index.html:1739`) and passes 500 km/h 0.6 s later; without it the shifts would blur into a buzz.
- **Downshift**: when `g > 1`, `v ≤ 0.92 × tops[g − 1]`, and at least 0.09 s have passed since the last shift.
- **Shift shaping** (applied to the tone and textures, and from R1 to the restored 1.2.0 layers, not to the reported `f`):
  - upshift: 40 ms ignition cut (body, sub and zing −9 dB), then the pitch glides down to the new gear over 60 ms
    (linear in log pitch), a crackle pop one-shot and a short zing blip;
  - downshift: an 80 ms +3 dB throttle blip and a burble pop.
- **Recovery and restart**: the gear is set from speed, with no shift events.
- **Neutral** (menu, countdown): `f = idleHz + (redlineHz − idleHz) × 0.8 × throttle^0.8`, chasing with a 0.15 s
  time constant, for grid revs. GO sets the gear from speed (4th at full throttle) with the launch one-shot; GO
  does not count as a shift for the 0.22 s minimum.
- **Load** per engine follows its own fader thrust within about 55 ms up and 110 ms down (the 1.2.0 load
  constants). Frames carry fader positions (`thrL`, `thrR`); full thrust is at 0.75 (`FADER_ZERO`,
  `index.html:1174`), so thrust = min(1, fader / 0.75), as the simulation computes it (`index.html:1181`). Braking scales it by `1 − 0.45 × brake`. Off-load means load < 0.25.

The model is deterministic and independent of the frame rate. It advances on the `dt` the engine receives and
uses no random numbers.

### 3.2 Tone: the Keys synth patch (`kestrel-engine.lbpatch`)

Voice mode LEGATO with the bend range at 24, which LIVE SYNTH forces whatever the patch says. The held note sounds
twice the fundamental (section 5.1), so SUB and OSC A sit one octave below it.

**R1.** The tone is now the bass body under the restored 1.2.0 layers (section 3.7), which carry the mechanical
character, the turbine movement and the noise. The patch keeps its file, name, voice, SUB, filter type, envelopes and
FX DRIVE TUBE; it loses the sync warp and OSC B, which made the periodic, harmonically dense buzz the user heard as an
electric razor, and it gets darker and quieter:

| Layer | Patch section | Role |
|---|---|---|
| SUB | `sub`: sine, oct −1 (= the fundamental), through the filter | Bass weight at the fundamental (41–191 Hz). The drive adds a 2nd harmonic that carries on small speakers. |
| BODY | OSC A: BASIC table at saw (`pos` 0.6666667), **warp off** (R1; was SYNC), oct −1 (= the fundamental); LP 24 at 301 Hz with filter DRIVE; FX DRIVE type TUBE on the whole patch with its tone filter at **2.5 kHz** (R1; was 9 kHz) | A dark saw growl at the fundamental. The cutoff runs from about 400 Hz off-load to 980 Hz on-load (R1, by the wheel route; was 500 Hz–2.6 kHz). Off-load drops drive and level, so a lift sounds hollow. (R1 first lowered the cutoff to 220 Hz; the low-pass's phase shift at the fundamental then changed so much between anchors and loads that two equal-gain crossfade midpoints of section 4.2's loop screening fell to −1.37 and −1.91 dB, past the −1 dB limit. At 301 Hz the worst is −0.76 dB.) |
| ~~ZING~~ | OSC B **off** (R1) | Superseded: the restored turbine (section 3.7) and the shift zing blip (S7) carry the top. OSC B's other settings stay in the file, unused. |

Level: `voice.volumeDb` **−4.7 dB** (R1; was 0), applied after the FX, so the tone at cruise sits 6 dB under the restored
layers (measured on the audition frames at cruise: the tone alone −26.5 LUFS, the restored layers alone −20.8 LUFS;
the rejected phase-1 tone measured −13.6 LUFS there).

Routes: **WHEEL** → filter cutoff (**+0.2**, R1; was +0.367), filter drive (+0.55), OSC A level (+0.6, the BODY's
off-load level drop). R1 removes the OSC A warp, OSC B warp and OSC B level routes. In BAKED the wheel is fixed per
loop (0.85 on-load, 0.2 off-load). In LIVE SYNTH the game sends it every frame (section 5.3).

### 3.3 Texture layers (ElevenLabs)

Each texture loop is a steady-RPM recording, pitched by `playbackRate = (f / redlineHz) / refRpm`, clamped to
0.5–2.0. `baseHz` (its measured fundamental) is recorded for verification.

| Layer | Loops | Kestrel weight and behaviour |
|---|---|---|
| F1 | V10 onboard at mid RPM (refRpm ≈ 0.65), at high RPM (≈ 0.95), off-load burble | 40 %. Full in gears 3–7, 0.6 in gears 1–2. Mid/high crossfaded by rpm, on/off by load. |
| Jet | Turbine cruise roar, afterburner burn | 35 %. `0.4 + 0.6 × boostIntensity`; cruise → afterburner crossfaded by boost intensity. |
| Drone | Spaceship ion drone, one octave under the fundamental | 25 %. Constant "not a car" layer, `0.6 + 0.4 × load`, with slow phasing shimmer. |

One-shots: upshift crackle pop (3 variants, picked with the engine's private seeded generator), downshift blip
(2 variants), boost ignite, boost release, launch. They replace the 1.2.0 afterburner ignition, burn, release and
the `pad` ignition (a shorter ignite).

### 3.4 Twin engines

- **BAKED**: two engines panned ±0.34. Each one's load follows its own fader. Pitch is shared (speed-based),
  with the right engine 0.35 % higher. Each engine runs its own 8 synth loops.
- **LIVE SYNTH**: one centred voice with unison width. The wheel carries the higher fader's load plus boost.
  Textures and one-shots stay twin and panned as in BAKED.
- **R1**: the restored 1.2.0 layers are twin in both modes: each engine has its own, at 1.2.0's per-engine values
  (the right one fires 1.6 % faster and its turbine sits 0.35 % higher, with its own partial ratios, intake and
  exhaust resonances and noise offsets). From the misfire dip on, each engine is one channel, so the ±0.34 pan is
  1.2.0's equal-power pan of a mono engine. (Before R1 the stereo loops took the panner's stereo law: the near
  channel at full level plus half the far one, about 5 dB louder than an equal-power pan and wider.) The phase-2
  texture loops take their own stereo path per engine (section 10, answered): they keep their width and the shift
  shaping, and skip the misfire dips.

### 3.5 Kept 1.2.0 layers

Airflow/wind, heat strain, damage misfires, vent steam and the ducking rules stay. Two need new connections
because the sources they hung on are removed:
- **Strain**: its metallic resonators are excited by a pulse train at the new fundamental. The pulse train goes
  to the resonators only, never to the output.
- **Misfires** on a damage fault act on the new body and zing gains (the same short shallow dips), plus a 0.6 %
  detune. Thrust stays audible. R1: the dip also takes the restored layers, as it did in 1.2.0, and the detune moves
  their firing rate and turbine pitch.

R1: the airflow layer's intake suction reads the restored spool (section 3.7) and 1.2.0's load again, as it did in
1.2.0; phase 1 had moved it to the gear rpm and load, which made it about 6 dB louder and 0.4 kHz higher at cruise.

Rivals keep the 1.2.0 procedural engine in both modes.

### 3.6 Measured targets

Measured by meters and earshot on the audition renders (section 7). They are evidence, not a verdict: the user's
listening decides.

| Target | Measure |
|---|---|
| Bassy | At cruise, at least 35 % of the propulsion energy below 200 Hz |
| Zing present | A clear tonal peak between 1 and 4 kHz that rises with speed |
| Pitch follows speed | Measured fundamental within 3 % of the gear model's `f` (pacing included) |
| Layers recognisable | Each texture reads as its description in earshot `match`: z ≥ 2.5 |
| Loudness | Propulsion bus about −16 LUFS at cruise; one-shots and ducking at their current levels |
| R1: tone pitch | The tone alone (the restored layers, textures and the other layers muted) within 3 % of the gear model's `f` on at least 90 % of the probes away from gear changes. The pitch row above stays measured on the whole propulsion, where the irregular restored firing pulse now dominates; its miss is reported, not tuned away. |
| R1: turbine follows the gears | The restored turbine whine: a spectral peak within 3 % of the engine's turbine frequency (the gear rpm through 1.2.0's curve, section 3.7), 6 dB above its ±25 % neighbourhood, on at least as many probes as 0.12's turbine against its own spool on the same frames |

R1 keeps 1.2.0's absolute levels for the restored layers, because that is the level the user preferred, so the
loudness row is expected to miss (the R1 prototype measured −19.8 LUFS at cruise); the miss is reported.

### 3.7 Restored 1.2.0 propulsion (R1)

Each engine gets back 1.2.0's propulsion layers, built as 1.2.0 built them (same nodes, buffers, filters, modulators,
per-engine values and level formulas):

- **Mechanical core** (mixer layer `growl`): the irregular firing pulse train and the combustion body, low-passed,
  driven into saturation, with restrained irregular movement.
- **Turbine** (`turbine`): a whine with related partials, a weak inharmonic stage tone, blade noise, intake
  resonance and slow drift.
- **Exhaust** (`exhaust`): pink noise opened by effort, body and mid resonances, roughened, crackling with the
  firing pulses; and the **jet hiss**, the high band of the jet, opening with effort.

What drives them:

- **Spool from the gear model**, in place of 1.2.0's spool: `n = 0.746 × rpm` (`rpm = tone / redlineHz`, the shift
  glide included), reduced by up to 7 % by damage with 1.2.0's slow surge. Full thrust in top gear (rpm 0.857) sits
  at 1.2.0's full-thrust spool, 0.639. An upshift drops `n` with the tone and boost lifts it with the boost lift.
  1.2.0's curves give the turbine pitch `120 + 780 × n^1.15` Hz and the firing rate `26 + 44 × n` Hz, each times the
  engine's own pitch factor (the right engine, the misfire detune, the re-light).
- **Effort as in 1.2.0**: each engine's restored layers read 1.2.0's throttle, the raw fader (on the grid
  `0.1 + 0.62 × fader`), through 1.2.0's load smoothing and effort formula, so a given game state works them as hard
  as 1.2.0 did. The gear model's load (section 3.1) keeps driving the tone.
- **Levels**: 1.2.0's, including its side level `0.92 × (0.55 + 0.45 × effort^0.9)`. The restored layers enter each
  engine at the shift cut, after their own two 46 Hz high-passes (1.2.0's bus had them; the tone joins after the
  bus's), so they take the ignition cut, the throttle blip, the zing blip and the misfire dips. Their level divides
  out the ship's `toneDb`, which the side level carries (S33), so their absolute level stays 1.2.0's.
- They sit outside the tone layer and play in BAKED and LIVE SYNTH alike. Their 69 nodes per engine are built once.

## 4. Asset pipeline

All ship assets live in `prototypes/Dustline/audio/ships/kestrel/`: `kestrel-engine.lbpatch`, `raw/`, `loops/`,
`LOG.md` and `picks.json`.

### 4.1 Synth patch

`kestrel-engine.lbpatch` is a normal Keys synth patch file (`.lbpatch` JSON, read with `Patch::from_json`, which
clamps every value to its range). The first version is written by hand to section 3.2. PLAY lists only the factory
patches and the user's patch folder (`Documents\Launchpad Drummer\Patches`, `commands.rs:255–260`), so the file is
copied into that folder to be loaded. The user can then load it on PLAY, hold note 48 (one octave below middle C),
use the bend and mod wheels as speed and load, tweak it and save it as "Dustline Kestrel" (stored as
`DUSTLINE KESTREL.lbpatch`). R1 revises the file in place (section 3.2); the copy in the user's patch folder is
replaced only if the user asks.

### 4.2 Synth bake (`crates/lpd-core/examples/bake_engine.rs`)

- Runs the real `Synth` offline at 48 kHz; no app change and no new dependency (WAV output through `hound`, already
  a dev-dependency of `lpd-core`, which examples may use).
- **Loop mode**: for each anchor (48 / 75 / 117 / 182 Hz, 0.64 octave apart) and each load (wheel 0.85, 0.2),
  hold note 48 with the bend set to the anchor (section 5.3 formula), render 3.25 s from the note start, drop the
  first 1.0 s and keep 2 s stereo. The seam gets a 150 ms crossfade that keeps the power constant for the measured
  correlation of the two ends (equal-power when they are uncorrelated, equal-gain when they are in phase). Result:
  8 loops.
- **Phase alignment**: with OSC A and SUB starting at phase 0 (`phaseRand` 0) and integer anchors, 1.0 s and 2 s
  are whole numbers of periods for every anchor, so every loop starts at the same phase of its fundamental. The
  0.5 s drop would leave the 75 and 117 Hz loops half a period off, and the anchor crossfade would cancel the
  fundamental. The engine starts all eight sources of an engine together at their loop start, and because the
  loops are in phase, its anchor and load crossfades are equal-gain (a power crossfade would add up to 3 dB).
- **Scenario mode**: drives the patch from a recorded game frame log (bend, wheel, gain at 40 Hz, ramped as
  section 5.1 does). This is how LIVE SYNTH is auditioned offline.
- Input: `--patch <path>` (the profile's file or the user's saved "Dustline Kestrel").

### 4.3 ElevenLabs generation (`prototypes/Dustline/generate-ship-sfx.py`)

- Follows `generate-sfx.py`: the key comes from `LAUNCHBAND/.env` (or `ELEVENLABS_API_KEY`) and is never
  printed or written anywhere else.
- Every loop prompt asks for a steady RPM: no gear changes, no Doppler, no music.
- 3 candidates per sound (two phrasings, plus one prompt-influence change); about 42 generations, using the
  user's ElevenLabs credits. The generation runs only after this spec is approved.

| Sound | Takes |
| --- | --- |
| F1 V10 onboard, steady RPM | mid RPM, high RPM, off-load burble |
| Jet turbine | cruise roar, afterburner burn |
| Spaceship ion drone | 1 loop |
| Upshift crackle pop | 3 variants |
| Downshift blip | 2 variants |
| Boost ignite, boost release, launch | 1 each |

### 4.4 earshot screening (`prototypes/Dustline/checks/ship_screen.py`)

One Python script in the earshot venv loads the models once (`from earshot import api`).

1. **Fit**: `rank` and `match` each candidate against its description with realistic confusions:
   F1 against a motorcycle, a chainsaw and a vacuum cleaner; jet against wind and a hair dryer; drone against a
   fridge hum and mains hum.
2. **Clean trims**: `find` the steadiest window for loops (`kind texture`) and a clean trim for one-shots (head
   and tail at −30 dB or lower; no 'head cut' or 'tail cut').
3. **Steadiness**: a pitch tracker rejects any loop whose pitch drifts more than 2 % and measures `baseHz`.
4. **Pick rule, fixed before results are read**: among the takes that pass steadiness and clean seams, the
   highest `match` z wins. `realness` is recorded as information, not used as a gate.
5. The 8 synth loops get `describe` and `measure` (band shares, brightness) against the section 3.6 targets.
6. Every number goes into `LOG.md`; the picks and their trims into `picks.json`.

### 4.5 Encode and embed

- Ogg Opus, 96 kbps (stereo for loops). WebView2 and Chromium decode it gaplessly (checked on 2026-11-05 in Edge
  156: the decoded length is sample-exact and starts with no offset, so the Opus pre-skip and end padding are
  trimmed); MP3 encoder padding would click at the seam.
- Loop points are stored in samples at 48 kHz (the encode rate) and set in seconds. A 44.1 kHz context resamples
  the decoded buffer, so the seam is verified on the **browser-decoded** buffer at both 48 and 44.1 kHz (section 7).
- `apply-sfx.py` gains the ship bundle: it validates all three copies, embeds the assets and keeps CRLF.
- Size: about 470 KB of audio, about 630 KB as base64. The page grows from about 1.46 MB (1,455,469 bytes) to
  about 2.1 MB.

## 5. LIVE SYNTH bridge (app only)

### 5.1 Rust (`lpd-core`)

- New `GameEngineVoice` (in `crates/lpd-core/src/synth/`) wraps a **second `Synth` instance**. The user's Keys
  synth, its patch and its routing are never touched.
- New engine commands: `GameSynthLoad(Box<GameEngineVoice>)`, `GameSynthFrame { on, bend, wheel, gain }`,
  `GameSynthClear`. The voice is built off the audio thread. A replaced voice returns through the existing trash
  queue, so the audio thread still never allocates.
- Output is added to the master mix where the Keys synth goes when it has no mixer channel: before the master
  gain and the soft clipper (`engine.rs:458–467`). The app's engine has no limiter.
- **Smoothing**: bend and the wheel glide linearly to each frame over 40 ms and the gain ramps over 25 ms, stepped
  every 16-sample synth tick. The game sends one frame per engine control step (`CTRL` 1/40 s, run on whole display
  frames): 30 Hz at a 60 Hz display, 36 Hz at 144 Hz, 40 Hz in the offline scenario. The 40 ms glide is longer than
  any of these gaps, so the pitch never stalls between frames (a 25 ms ramp would hold still for 8 ms of every 33 ms
  at 60 Hz, a 30 Hz flutter during gear sweeps); the shorter gain ramp keeps the shift cuts crisp. The synth holds
  pitch constant within a tick (`env.rs:8`, `voice.rs:325–329`) and smooths neither bend nor the wheel itself
  (`mod.rs:464–470`); an audio block can hold up to 1024 samples, so stepping once per block would not be fine enough.
- **Pitch**: hold note 48 (130.8 Hz), which sounds twice the fundamental, with bend ±24 semitones: the fundamental
  covers 16–262 Hz and the whole curve (41–191 Hz, bend −8.1 to +18.6 semitones) fits without a synth-core change.
  SUB and OSC A play one octave below the note, OSC B two octaves above it (8 × the fundamental, at most 42.6
  semitones above the note). Note 36 cannot work: SUB cannot go above oct −1 (`params.rs:891`, clamped on load),
  and OSC B at +36 semitones would hit the voice's ±48-semitone limit (`voice.rs:328–329`) above a 131 Hz
  fundamental, freezing the zing during boost. Voice mode LEGATO and bend range 24 are forced. (R1 turns OSC B off;
  note 48 stays.)
- **R1 in LIVE SYNTH**: the revised patch plays live too (unless the user's `DUSTLINE KESTREL.lbpatch` exists, which
  wins as before), and the restored 1.2.0 layers keep playing in Web Audio, so the A/B still compares only the tone.
  The frames (section 5.3) are unchanged; the wheel now moves the cutoff, the filter drive and the BODY level only.
- **Watchdog**: with no frame for 250 ms, the voice fades to silence.

### 5.2 Tauri and `App.svelte`

- Commands: `game_engine_start { patch }`, `game_engine_frame { on, bend, wheel, gain }`, `game_engine_stop`, and
  `game_engine_status` (frames received; used by the smoke test). `start` prefers the user's
  `DUSTLINE KESTREL.lbpatch` (in `Documents\Launchpad Drummer\Patches`) if it exists, else the embedded patch.
  Every value is clamped and checked as finite. The commands follow the lock order in `state.rs` (router →
  history → kit_manager → saver).
- A restarted output stream gets a new voice: `Router::replace_audio` resends only the mixer, keys, patch and
  wheels (`router.rs:342–350`), so the router keeps the started game patch and the voice is rebuilt for the new
  stream's sample rate.
- Bridge, as a pure validator in `src/lib/engineBridge.ts` used by `App.svelte`:
  - accepts `{lpd: 'engine-synth', op: 'start' | 'frame' | 'stop', …}` only from the active game frame with
    `dataset.game === 'dustline'` (the `lcxl-leds` check);
  - drops malformed messages and caps the rate at 60 messages per second;
  - replies `{lpd: 'engine-synth-status', state: 'ready' | 'unavailable' | 'error'}`;
  - stops the voice on game switch, leaving GAMES, frame unload and window hidden.

### 5.3 Game side

- LIVE SYNTH appears in Pause → Sound only after `ready`. On `error`, or with no host, the game uses BAKED and
  shows "LIVE SYNTH UNAVAILABLE".
- Each frame (one per engine control step, section 5.1) carries:
  - `bend = (12 × log2(f / 65.41)) / 24`, with `f` including the shift glide (note 48 sounds 2f, so the bend is
    measured from 65.41 Hz);
  - `wheel = clamp(0.15 + 0.6 × max(loadLeft, loadRight) + 0.25 × boostIntensity, 0, 1)`;
  - `gain = master × mute × pause × duck`.
- Switching mode mid-race crossfades the two backends over 150 ms.

### 5.4 Known limits (stated in the README)

- Tone and textures come out of two audio paths (WebView2 and the app's native output). Expect an offset of
  roughly 20–60 ms between them; a shift pop may land slightly off the pitch drop.
- LIVE SYNTH twin engines are one centred voice.
- Rivals use the 1.2.0 procedural engine in both modes.
- The real native output cannot be recorded by the automated checks; the Rust scenario render is the closest
  stand-in.

## 6. Files

| Area | Files |
|---|---|
| Game (three copies) | `prototypes/Dustline/index.html`, new `dustline-rift-run-v0.13.html`, `launchpad-drummer/public/games/dustline/index.html`: engine block (2.0, gear model, profile), game script (frame field, mode switch, bridge client, Pause → Sound), ship asset bundle |
| Game tools | `generate-ship-sfx.py`, `apply-sfx.py` (ship bundle), `checks/ship_screen.py`, `checks/ship-gears.test.cjs`, `checks/audio-engine.test.cjs`, `checks/audio-browser.py`, `checks/audio_audition.py`, `README.md` |
| Assets | `prototypes/Dustline/audio/ships/kestrel/` |
| R1 | `tools/engine2/step_r1_mech.py` and `tools/engine2/pieces/mech.js` (the restored layers), `tools/engine2/step_r1b_intake.py` (the intake suction, section 3.5), `kestrel-engine.lbpatch` (section 3.2), `checks/audio-engine.test.cjs`, `checks/ship-profile.test.cjs`, `checks/audio_audition.py` (`--rejected`, the R1 targets), `checks/ship_screen.py` (the rejected A/B), `README.md`; audition in `audio/audition-v0.13-p1-r1/` |
| Rust | `crates/lpd-core/src/synth/` (new `GameEngineVoice`), `crates/lpd-core/src/audio/engine.rs` (commands), `crates/lpd-core/examples/bake_engine.rs`, `crates/lpd-core/tests/no_alloc.rs` |
| App | `src-tauri/src/commands.rs`, `state.rs`, `router.rs` (voice rebuilt after a stream restart), `lib.rs` (registration), `src/lib/engineBridge.ts` (+ test), `src/App.svelte`, `tools/test-dustline-browser.py` |

Before each write of a game copy, compare the three hashes. If one differs from the last written state, diff and
merge instead of overwriting. Edit the CRLF copies with byte-preserving tools; Git Bash `sed -i` converts them to
LF.

## 7. Testing

- **`checks/ship-gears.test.cjs`**: shift points and hysteresis; minimum shift spacing (0.22 s up, 0.09 s down);
  over-rev cap 1.03; idle clamp; the boost lift formula; the paced speed (rise limit, instant fall, GO in 4th,
  launch upshifts at 0.19, 0.54 and 0.93 s); pitch continuous except at shifts; downshift on brake;
  neutral revs; the same gear sequence, with shift times within 1/120 s (GO counted from the first racing frame),
  at 30, 60, 120 and 144 Hz and with
  irregular frames (the engine updates on a variable step of about 1/40 s, so bit-identical results are not
  possible).
- **`checks/audio-engine.test.cjs`** (rewritten for 2.0): a fixed node count built once with no allocation per
  frame; pause, mute and restart lifecycle; the backend crossfade; fallback when native is unavailable; frame bend
  and wheel match the gear model; NaN and out-of-range inputs clamped. R1: the restored layers follow `0.746 × rpm`
  on 1.2.0's curves and 1.2.0's effort, join each engine at its shift and outside its tone sum, mute with the growl,
  turbine and exhaust layers but not with the tone layer, drop at an upshift, and each engine is one channel from
  its misfire dip on.
- **Assets** (`checks/audio-browser.py`, headless Chromium): every loop and one-shot decodes; a seam click test on
  the decoded buffers; each loop's `baseHz` matches `picks.json`.
- **Rust**: `GameEngineVoice` spreads each frame's bend and wheel change evenly over 40 ms and its gain change over
  25 ms, in 16-sample ticks (a 375-cent change moves the pitch 3.125 cents per tick at 48 kHz); the watchdog fades after
  250 ms; NaN and out-of-range values are clamped; legato and bend 24 are forced; the user's Keys synth params are
  unchanged after a game engine start and stop.
- **App**: `engineBridge.ts` tests (wrong source, malformed frames, rate cap, auto-stop), `npm test`,
  `npm run check`, `cargo test --workspace`.
- **Native smoke**: `tools/test-dustline-browser.py --cdp` checks that `ready` arrives and that frames reach Rust
  (`game_engine_status`).
- **Audition** (`checks/audio_audition.py`, output `audio/audition-v0.13/`): scenarios launch from GO (245 → 600
  km/h) through the gears, cruise, lift and brake downshifts, boost to 1200, release. Their frames come from the
  real 0.12 simulation (`checks/audio_scenarios.py` still uses the 0.11 speed law). BAKED is rendered as played; LIVE SYNTH as the
  Rust scenario render plus the same textures. earshot `timeline` and `ab` against `audition-v0.12`, plus
  spectrograms. The section 3.6 targets go into `metrics.json`. R1: the revised phase 1 renders to
  `audio/audition-v0.13-p1-r1/` on the same frames as frozen 0.12 and the rejected phase 1 (a snapshot of its engine
  and loops in `rejected/`), with as-played and −20 LUFS renders of all three and a three-way per-section A/B.
- **Regression**: the 131 existing deterministic checks in 10 `checks/*.test.cjs` files (the engine-audio ones as
  rewritten above), the 8 browser harnesses and `node checks/ai-pace.cjs index.html --verify` (run from
  `prototypes/Dustline`) pass. Scripts 0–6 (Three.js, route, simulation, MIDI, craft, world, mixer) are
  byte-identical to 0.12; the engine block and the game wiring are both in script 7. The three copies are
  byte-identical with uniform CRLF.

## 8. Work order

1. Gear model, ship profile format, the Keys patch and the Rust bake tool; BAKED with the 8 synth loops only.
   **Checkpoint 1**: the user listens to the bass and zing character and can tweak the patch on PLAY.
   R1: checkpoint 1 failed; tone revision R1 (section 10) is followed by checkpoint 1 again, before phase 2.
2. ElevenLabs generation, earshot screening and the texture layers; full BAKED.
   **Checkpoint 2**: the user listens to the blend.
3. The native bridge and LIVE SYNTH; the A/B switch in Pause → Sound. Rebuild the exe
   (`npm run tauri -- build --no-bundle`).

## 9. Done

- Every check in section 7 passes, and the section 3.6 targets are met or their misses are reported with numbers.
- The three game copies are byte-identical, v0.12 is unchanged, and the exe is rebuilt.
- The README documents the engine, the profile format, the bridge and the limits in section 5.4.
- The user has listened to both modes. Reports give measurements and earshot readings; they never claim the
  engine "sounds good".

## 10. Tone revision R1 (2026-11-06)

**Why.** Checkpoint 1 failed. The user's verdict, which is binding: "it sound like an electric razer, the previous
sound was more matching the feel". Approved direction: restore the previous engine's mechanical body, turbine
movement and noise texture while keeping the new speed/gear behaviour, and produce another loudness-matched A/B.
The previous engine (0.12, 1.2.0) is the character reference.

**Evidence** (supporting, not a diagnosis). Source: 1.2.0's propulsion is an irregular firing pulse train, a drifting
turbine with blade noise and noise exhausts; phase 1 replaced all of it with phase-locked loops of one periodic patch,
a sync-warped saw plus a 3-voice OSC B at 8 × the fundamental, with no noise and no irregularity. earshot read the
phase-1 on-load loops as "error buzz" (z +3.1 to +3.7), "electrical hum" and "phone vibration buzz", and the
audition timeline as "synth lead / electrical hum / dubstep".

**Superseded** (exactly these tone choices):
- section 1, "What is replaced": the 1.2.0 core, turbine, exhaust and jet hiss are restored, not replaced;
- section 3.2: OSC A warp SYNC (warp amount 0.1) → off (0); OSC B on → off; FX drive tone 9000 → 2500 Hz (the
  cutoff stays at 301 Hz: a 220 Hz cutoff failed the loop crossfade check); volume 0 → −4.7 dB; the wheel routes
  to OSC A warp (+0.45), OSC B warp (+0.6) and OSC B level (+0.3) are removed, and the cutoff route goes from +0.367
  to +0.2;
- section 3.4: the stereo-law pan of the stereo loops becomes an equal-power pan of a mono engine;
- section 3.5: the intake suction goes back to 1.2.0's inputs (the spool, now restored, and 1.2.0's load);
- section 3.6: two targets added (tone pitch, turbine); the old ones stay and are reported as measured.

**Kept**: the gear model and its numbers, paced sweeps, GO, shift timing and shaping, load, the twin engines' pan
positions and pitch offset, the BAKED loop set (anchors, loads, layout, bake procedure), the 1.2.0 afterburner in
phase 1, phases 2 and 3 as designed, engine version 2.0.0, and every decision outside section 3.2's tone choices.

**LIVE SYNTH downstream.** The revised patch is what the native voice plays (unless the user's own file exists);
the restored layers play in both modes; the frames and the native voice are unchanged; the LIVE trim measures
the tone alone, with the restored layers muted.

**Open for the user, before phase 2 starts.** The phase-2 textures (F1, jet, drone) and their weights were designed
to sit on the phase-1 tone. With R1 the restored 1.2.0 turbine, exhaust and jet hiss already fill part of that role.
Whether the phase-2 texture set and weights still stand is the user's call after hearing R1. Also part of that question:
from the misfire dip on, R1 makes each engine one channel (section 3.4), so phase 2's stereo texture loops and shift pops,
which enter each engine before that dip, would be downmixed to that engine's mono channel unless phase 2 gives them their own
stereo path.

**Answered (2026-11-06, revised checkpoint 1 passed: "yes its good").** Phase 2 keeps all six textures and their weights
as designed (section 3.3); CHECKPOINT 2 checks them against R1. The textures get their **own stereo path** per engine:
texture sum → its own shift cut/blip → its own zing blip → its own level (`toneDb`, as the side level gave them before
R1) → its own ±0.34 pan (the panner's stereo law, as phase 1 designed their level for) → `propMix`. So the textures keep
their width and take the shift shaping, but not the misfire dips. The shift pops are mono one-shots and stay in the
engine's mono channel.

**Takes picked by ear (2026-11-06, plan S27-E).** The screening rule (section 4.4) picked 5 of the 14 sounds; the user
chose to pick every take by ear instead of new takes or a rule change. The ear picks change three details of section 3.3:
the up-shift crackle has **four** variants and the down-shift blip **three** (24 ship assets, 10 one-shots); the drone
pick measures unpitched, so instead of sitting an octave under the fundamental it plays at its own speed at cruise rpm
(refRpm 0.857, baseHz 0) and follows the rpm like the jet loops. Loops use the screened 3 s window of the picked take;
one-shots use the whole take less silence.
