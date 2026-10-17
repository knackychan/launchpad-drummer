# Launchpad Drummer — Part 1: Drum Sampler (Design)

Date: 2026-10-17
Status: draft, awaiting user review

## 1. Goal

A Windows desktop app for finger drumming on a Novation Launchpad MK2, with a Novation
Launch Control XL as mixer. Each of the 64 grid pads plays a user-chosen sample, pad LEDs
are fully controlled by the app, and the app's own UI is used to build and edit kits.
End-to-end latency target: under 10 ms with ASIO.

This document covers **Part 1** only. Later parts are listed in section 14 so the
architecture leaves room for them.

## 2. Hardware and protocol

### 2.1 Step 0: verify the protocol on the real units

Both devices are USB class-compliant MIDI; no vendor driver is required. Novation's
published programmer references describe the protocol. Before any app code, a CLI tool
`lp-probe` verifies every message below on the user's units and writes the result to
`docs/launchpad-mk2-protocol.md` and `docs/launch-control-xl-protocol.md`. App code uses
only verified values. Where the probe disagrees with this section, the probe wins and
this spec gets updated.

`lp-probe` does, per device:
1. List MIDI in/out ports, find the device by port name.
2. Send Device Inquiry `F0 7E 7F 06 01 F7`, log the reply (model, firmware version).
3. Log raw input bytes while the user presses/moves every control; compare to the
   expected map below and report mismatches.
4. Send each LED command type and ask the user to confirm the visible result (y/n).
5. Write the protocol table document.

### 2.2 Launchpad MK2 (USB `VID_1235 PID_1069` observed on user's unit)

Observed 2026-10-17: enumerates with Microsoft class driver (`usbaudio`, no Novation
driver needed); WinMM ports `IN 0: Launchpad MK2`, `OUT 1: Launchpad MK2`. Note On ch1
to notes 11/18/81/88 with palette velocities sent without error (visual result to be
confirmed by user).

- SysEx header: `F0 00 20 29 02 18`.
- Select Session layout on connect: `F0 00 20 29 02 18 22 00 F7`.
- Grid pads: Note On, channel 1, note = `row*10 + col` (row 1–8 bottom to top,
  col 1–8 left to right), i.e. 11–88. Velocity 127 on press, Note Off / velocity 0 on
  release. **Pads are not velocity-sensitive.**
- Right-side round buttons: notes 19, 29, …, 89.
- Top row round buttons: CC 104–111, value 127 press / 0 release.
- LED RGB: `F0 00 20 29 02 18 0B <led> <r> <g> <b> F7`, r/g/b 0–63. Several LEDs
  can be packed into one message by repeating `<led> <r> <g> <b>`.
- LED palette: Note On ch1 (static), ch2 (flash), ch3 (pulse), velocity = palette index.
- Top-row LEDs addressed as 104–111.

### 2.3 Launch Control XL (expected, USB `VID_1235 PID_0061`)

- Uses templates; app assumes User template 1 (MIDI channel 1). Probe confirms which
  template/channel the unit is on.
- Faders: CC 77–84. Knob rows (top to bottom): CC 13–20, 29–36, 49–56.
- Button row 1 ("track focus"): notes 41–44, 57–60. Button row 2 ("track control"):
  notes 73–76, 89–92.
- LED: `F0 00 20 29 02 11 78 <template> <index> <value> F7`, value encodes red/green
  brightness (amber = both).

### 2.4 Windows MIDI port sharing

Classic Windows MIDI (WinMM) ports are single-client: if Novation Components, a DAW, or a
second copy of this app holds a port, opening it fails. The app shows "port busy — close
other MIDI apps" instead of "not found".

## 3. Architecture

Tauri 2 desktop app. Real-time work in Rust; Svelte + TypeScript UI for editing only.
A pad hit never passes through the UI.

```
 MIDI devices ──► [midi_io] ──events──► [router] ──commands──► [audio] ──► ASIO / WASAPI
 (Launchpad, LCXL,   ▲                    │  ▲                      │
  keyboards, …)      │                    │  └── voice start/end ◄──┘
                     └── LED SysEx ◄── [lights]   (Launchpad + LCXL LEDs)
                                          │
                              Tauri events (~30 Hz) ▼ ▲ commands
                                       [Svelte UI]
```

### 3.1 Rust modules

| Module | Responsibility |
|---|---|
| `midi_io` | Enumerate all MIDI inputs/outputs (`midir`), poll for changes every 2 s, open/close ports, tag every incoming message with a device id. Recognise Launchpad MK2 and Launch Control XL by port name. |
| `devices::launchpad` | Pure functions: bytes ↔ `PadDown(x,y)`, `PadUp(x,y)`, `SideButton(n)`, `TopButton(n)`; LED frame → SysEx messages. |
| `devices::lcxl` | Pure functions: bytes ↔ `Fader(ch, v)`, `Knob(row, ch, v)`, `Button(row, ch, on)`; LED state → SysEx. |
| `learn` | MIDI-learn table: (device id, message kind, number) → target (pad trigger or parameter). |
| `router` | Owns the current set state. Turns input events into audio commands and light state; handles bank switch, mixer controls, learn mode; pushes UI updates. |
| `audio` | Real-time engine on its own thread (`cpal`). Voices, per-pad playback params, choke, 8-channel mixer, master. Commands in / voice events out via lock-free SPSC queues (`rtrb`). No allocation, no locks in the callback. |
| `instrument` | `Instrument` trait; `DrumKit` is the only implementation in Part 1. Each instrument feeds one mixer channel set. Exists so Part 4 can add `SoundFont` without touching the engine. |
| `kit` | Set/bank/pad data model, JSON load/save, sample import (copy into set folder), decode (`symphonia`), resample to device rate (`rubato`). |
| `lights` | ~60 Hz painter: from pad colours + light modes + voice state + time, compute all 81 Launchpad LEDs and LCXL button LEDs; diff against last sent frame; send only changes. |

### 3.2 Threads

- MIDI input callbacks (from `midir`) → push to router queue.
- Router thread: single owner of state; sends to audio queue, updates lights, emits UI
  events at ~30 Hz.
- Audio callback thread: consumes commands, renders, pushes voice start/end events.
- Lights: a 60 Hz tick on the router thread (no separate thread).
- Kit loading (decode/resample) on a worker thread; finished sample buffers handed to the
  audio engine as `Arc<[f32]>` swaps, never decoded in the callback.

## 4. Data model and files

### 4.1 Set folder

```
MySet/
  set.json        8 banks × 64 pads + mixer + settings
  set.json.bak    previous good save
  samples/        imported audio files (copied on drop)
```

Dropped files are copied into `samples/` (name collision → suffix `-2`, `-3`, …). Paths
in `set.json` are relative to the set folder. Autosave 1 s after the last change.
Saves are atomic: write `set.json.tmp`, rename current to `.bak`, rename tmp to
`set.json`.

### 4.2 `set.json` shape

```json
{
  "version": 1,
  "banks": [
    {
      "name": "Bank 1",
      "lights": { "flash": true, "playState": true, "ripple": false },
      "humanizeDb": 0.0,
      "pads": [
        {
          "x": 0, "y": 0,
          "sample": "samples/kick.wav",
          "color": [63, 10, 0],
          "volumeDb": 0.0,
          "pitchSemitones": 0.0,
          "trimStartMs": 0, "trimEndMs": null,
          "mode": "oneShot",
          "chokeGroup": null,
          "mixGroup": 0
        }
      ]
    }
  ],
  "mixer": {
    "groups": [
      { "name": "Kick", "volumeDb": 0.0, "pan": 0.0, "pitchSemitones": 0.0,
        "lowpassHz": 20000, "mute": false, "solo": false }
    ],
    "masterDb": 0.0
  },
  "learn": [
    { "device": "Some Keyboard", "kind": "note", "number": 36, "target": { "pad": [0, 0] } }
  ]
}
```

Empty pads are omitted from `pads`. `mixGroup` defaults to the pad's row (`y`).
`chokeGroup` is `null` or 1–8. `mode` is `"oneShot"` or `"hold"`.

## 5. Playback behaviour

- **Re-hit while playing:** restart; the previous voice of that pad fades out over 3 ms.
  One voice per pad.
- **Choke:** triggering a pad in choke group N fades out all other voices in group N
  within 5 ms.
- **Hold mode:** pad release fades the voice out over 10 ms. One-shot ignores release.
- **Voice limit:** 32. When full, oldest voice fades out (3 ms) to make room.
- **Pitch:** playback-rate change (pitch and length together), ±24 semitones; pad pitch
  plus mix-group pitch offset.
- **Trim:** playback starts at `trimStartMs`, ends at `trimEndMs` (or sample end) with a
  2 ms fade.
- **Humanize:** per hit, random gain offset uniformly in ±`humanizeDb/2`; 0 = off.
- **Mixer:** voice → mix group (volume, pan, low-pass filter, mute/solo) → master.
  Solo on any group mutes all non-solo groups. Low-pass is one biquad per group per
  channel; 20 000 Hz = bypassed. Master stage: master gain then a `tanh` soft clipper,
  so output never exceeds full scale.
- **Banks:** Launchpad side buttons 1–8 select the active bank (button lit). Sounding
  voices continue across bank switches.
- **Top row:** `Mixer` button (CC 111) = stop all voices (5 ms fade). Others unused in
  Part 1.
- **Empty pad:** dark, silent.

## 6. Lights

Per bank, three independent toggles:

- **Flash:** on hit, pad goes white then fades back to its colour over 150 ms.
- **Play-state:** while the pad's voice is sounding, pad shows its colour at full
  brightness; idle pads show their colour at ~35 % brightness.
- **Ripple:** on hit, a ring expands from the pad at ~1 pad per 40 ms, lightening pads
  it passes, for 4 rings.

Layer order (lowest first): base colour → play-state → ripple → flash. Missing-sample
pads blink red (500 ms period) on top. Active bank side button lit white; others off.
LCXL: mute buttons lit amber when muted, solo buttons lit green when soloed.

## 7. Other MIDI devices and MIDI learn

- All connected MIDI inputs are listened to. Events carry their device id.
- **Pad learn:** in the pad editor, "Learn" → next Note On from any non-Launchpad device
  is bound to that pad. Bindings are per set, stored in `learn`.
- **Parameter learn:** right-click any knob/slider in the UI → "Learn" → next CC from any
  device controls it (0–127 mapped to the parameter range).
- Launch Control XL has the fixed default mapping below; learn can add more bindings but
  does not override the defaults in Part 1.

### 7.1 Launch Control XL default mapping (channels 1–8 = mix groups 1–8)

| Control | Function |
|---|---|
| Faders 1–8 | Group volume (−∞ to +6 dB) |
| Knob row 1 | Group pan |
| Knob row 2 | Group pitch offset (±12 st) |
| Knob row 3 | Group low-pass cutoff (200 Hz – 20 kHz, log) |
| Button row 1 | Group mute (toggle) |
| Button row 2 | Group solo (toggle) |

## 8. User interface (Svelte)

- **Grid view:** 8×8 mirror of the Launchpad with live LED colours; click = play.
  Computer-keyboard fallback for the bottom 4 rows: `1–8`, `Q–I`, `A–K`, `Z–,`.
- **Pad editor** (selected pad): sample name + waveform with draggable trim handles,
  colour picker, volume, pitch, mode, choke group, mix group, Learn button.
- **Drag and drop:**
  - Explorer file → pad: import sample.
  - Pad → pad: move; onto occupied pad = swap; Ctrl = copy. Hover a bank tab while
    dragging to switch bank.
  - Sample browser sidebar: folder tree rooted at a user-chosen library folder; click
    to audition, drag to pad.
- **Bank tabs** 1–8 with names, light-mode toggles, humanize amount.
- **Mixer panel:** 8 group strips (volume, pan, pitch, low-pass, mute, solo) + master.
- **Settings:** audio backend (ASIO / WASAPI), device, buffer size, measured latency,
  dropout counter, MIDI device list with connection status.
- **Status bar:** Launchpad / LCXL connected or not, current set path.

## 9. Error handling

| Situation | Behaviour |
|---|---|
| No Launchpad at startup | App runs; UI shows disconnected; on-screen grid playable. Auto-reconnect via 2 s port poll. |
| Device unplugged mid-play | Send error → mark disconnected; audio continues. On reconnect: re-select Session layout, repaint all LEDs. |
| Port busy | Status shows "port busy — close other MIDI apps". |
| Audio device lost/changed | Stream stops, UI warning with Reconnect button. |
| ASIO unavailable | Fall back to WASAPI with a notice. |
| Audio dropouts | Counter in Settings, hint to raise buffer size. |
| Sample missing / undecodable | Pad blinks red, warning in UI, entry in log. Rest of set works. |
| `set.json` unreadable | Load `set.json.bak`, notify user. If both fail, offer to start a new set; never overwrite the broken file. |
| Logging | Rotating log file in the app data folder. |

## 10. Testing

### 10.1 Automated (`cargo test`, no hardware, no sound card)

- `devices::launchpad`, `devices::lcxl`: bytes ↔ events for every message type; LED
  frames → expected SysEx bytes.
- `kit`: JSON round-trip; atomic save; `.bak` fallback; import name collisions.
- `router`: choke, bank switch, restart on re-hit, hold release, 32-voice stealing,
  mute/solo, learn bindings — with fake audio and LED sinks.
- `lights`: (state, time) → expected 81-LED frame for each mode; diff emits only changes.
- `audio` offline render into a buffer:
  - choke reaches silence (< −60 dB) within 5 ms;
  - restart has no click (max sample-to-sample step below threshold);
  - 32 voices at 0 dB do not exceed full scale after the master soft clipper;
  - low-pass at 1 kHz attenuates a 5 kHz sine by the expected amount.
- Debug builds wrap the audio callback with `assert_no_alloc`; any allocation fails.

### 10.2 Latency

- Settings shows measured MIDI-receive → sample-in-buffer time plus device-reported
  output latency.
- Real-world check: phone records finger tap + speaker output; target < 10 ms with ASIO.

### 10.3 Manual hardware checklist

Hot-plug both devices, bank switching, all light modes, LCXL faders/knobs/buttons and
their LEDs, every drag-and-drop path, MIDI learn with a keyboard, 30-minute session with
zero dropouts.

## 11. Tech stack and prerequisites

- Rust stable (to install), Node 26 (present).
- Tauri 2, Svelte 5 + TypeScript, Vite.
- Crates: `midir`, `cpal` (feature `asio`), `rtrb`, `symphonia`, `rubato`, `serde`,
  `serde_json`, `assert_no_alloc` (dev/debug), `tracing`.
- ASIO build needs the Steinberg ASIO SDK (free download, not redistributed) and
  LLVM/clang. WASAPI build needs neither; ASIO is a cargo feature so the app builds
  without it.
- User needs an ASIO driver (audio interface driver or ASIO4ALL) for the < 10 ms target.

## 12. Repository layout

```
launchpad-drummer/
  docs/                      specs, plans, verified protocol tables
  tools/lp-probe/            Step 0 CLI (Rust)
  src-tauri/                 Rust app (modules from 3.1)
  src/                       Svelte UI
  starter-sets/              (later) example sets
```

## 13. Out of scope for Part 1

Looper/recorder, pitched instruments, SoundFonts, training mode, mini-games, effects
other than the per-group low-pass, velocity curves (pads send fixed 127), macOS/Linux.

## 14. Roadmap (later parts — notes only, each gets its own design)

**Part 2 — Training (Melodics style).** Horizontal lanes, one per pad used in the lesson,
coloured like the pad, labelled with sample name and a mini 8×8 pad map. Notes scroll
right → left toward a playhead about ⅓ from the left; bar lines shown. Upcoming notes
coloured by suggested hand (blue left, yellow right); hit notes turn green; misses
marked. Launchpad shows approach lights (pad brightens until its hit time), green/red
hit feedback. Modes: Performance, Practice (wait mode, loop a bar range, tempo ramp
from 70 %), Memory (guide notes fade over passes). Sidebar volumes: metronome, your
notes, guide notes, backing track; Back / Pause / Restart. Judgments Perfect / Early /
Late / Miss in ms, using a one-time tap calibration (16 clicks) for output latency.
Gamification: streak toasts, multiplier, accuracy %, 1–3 stars, XP and levels, daily
streak, history. Content: drum MIDI import (GM note → pad mapping, editable; hand
assignment by heuristic, overridable per lane) + ~10 built-in graded lessons with hands
set.

**Part 3 — Mini-games.** Launchpad grid is the game screen. First game: objects fall
down the 8 LED columns; hitting a column's bottom pad on time destroys them; each
destroy plays the next note of a melody, so good play assembles the tune. Explosions and
combo effects on the grid; UI mirrors it larger.

**Part 4 — Band mode.** Keyboards and other controllers play pitched instruments
(SoundFont `.sf2`), each device on its own mixer channel (same 8 channels the LCXL
already controls), plus a looper/recorder to stack parts.

Order: Part 1 first (everything depends on its engine and light painter). Order of
Parts 2–4 to be decided after Part 1.

## 15. Risks and open items

- Launchpad USB enumeration failure (Code 43) resolved by replug on 2026-10-17.
  Launch Control XL not yet connected.
- Expected protocol values (2.2, 2.3) are unverified until `lp-probe` runs.
- `cpal` ASIO setup on Windows is the most likely build friction; WASAPI fallback keeps
  the project unblocked.
