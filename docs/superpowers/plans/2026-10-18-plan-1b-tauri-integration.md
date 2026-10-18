# Plan 1B: Tauri App Integration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate the pure real-time audio engine, Launchpad MK2 hardware bridge, and kit store from `lpd-core` into the Tauri desktop application with the sci-retro Svelte UI. Sub-millisecond MIDI-to-audio dispatch without passing pad hits through the webview, 60 Hz reactive LED light painter with flash/play-state/ripple, 8-channel mixer, pad editor, and live status bar.

**Architecture:**
- **Audio Thread (CPAL)**: Real-time output stream running `lpd_core::audio::Engine` via `lpd_core::audio::host::process`. Zero allocation, lock-free SPSC queues (`rtrb`).
- **Router Thread**: Single owner of set state, bank selection, and held pad state. Direct bridge from `midir` Launchpad events to audio command queue.
- **Lights Engine**: 60 Hz tick running on the router thread: calculates base colors, flash animation, play-state brightness, expanding ripple rings, and missing-sample blink. Streams SysEx diffs to hardware and throttled (~30 Hz) frames to Tauri webview.
- **Worker / Background**: Asynchronous sample decoding (`lpd_core::kit::load_samples`) and atomic JSON set persistence (`lpd_core::kit::save`). Holds strong `Arc<Sample>` references so audio thread never deallocates memory.
- **Tauri / Webview**: UI display and kit editing only (Svelte 5 + TypeScript). Pad clicks and computer keyboard invoke Tauri commands that push straight to the router queue.

**Conventions:**
- Shell is PowerShell on Windows; repo root is `D:\MY DOCS\MY TOOLS\LAUNCHBAND\launchpad-drummer`.
- Prepend `$env:USERPROFILE\.cargo\bin` to PATH in commands.
- Run `cargo fmt` before every commit.
- End every commit message with a blank line and `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

---

## Task Breakdown

### Task 1: Tauri Dependencies and Audio Engine Host
- Add `cpal`, `rtrb`, `serde_json`, `anyhow` to `src-tauri/Cargo.toml`.
- Implement `src-tauri/src/audio.rs`: manage CPAL stream, command producer, and event consumer.
- Tests in `src-tauri/tests/audio_host.rs`: verify startup, stream creation, and clean shutdown.

### Task 2: High-Priority Router Thread
- Implement `src-tauri/src/router.rs`:
  - Owns `Set`, loaded samples `HashMap<String, Arc<Sample>>`, active bank index (0..7), held pad tracking.
  - Translates `LpEvent` to `audio::Command` and pushes to `rtrb::Producer`.
  - Listens to `audio::Event::VoiceStarted` / `VoiceEnded` for voice tracking.
  - Bank switching via side buttons, StopAll via top Mixer button (CC 111).
- Offline unit tests in `src-tauri/src/router.rs`.

### Task 3: 60 Hz Light Painter Engine
- Implement `src-tauri/src/lights.rs`:
  - 60 Hz calculation of 64 grid LEDs + 8 side buttons.
  - Flash: white on hit, decays back to base color over 150 ms.
  - Play-state: 100% brightness when sounding (voice counter > 0), 35% when idle.
  - Ripple: expanding concentric rings (1 pad per 40 ms, up to 4 rings).
  - Missing sample: blinking red at 2 Hz (500 ms period).
  - Side buttons: active bank white `[63, 63, 63]`, others off.
  - Diffs against previous frame to minimize SysEx bandwidth.
- Unit tests in `src-tauri/src/lights.rs`.

### Task 4: Kit Store & Asynchronous Sample Loading
- Implement `src-tauri/src/store.rs`:
  - Load set and decode samples on worker thread without blocking audio or router threads.
  - Safe sample handoff: router receives new sample map; previous samples dropped off-thread.
  - Atomic save and backup fallback.
  - Tauri commands: `get_current_set`, `load_set`, `new_set`, `save_set`.
- Unit tests in `src-tauri/src/store.rs`.

### Task 5: Tauri Application Wiring & Full State Bridge
- Update `src-tauri/src/lib.rs` and `src-tauri/src/launchpad.rs`:
  - Wire Launchpad MIDI in/out to router and lights engine.
  - Handle auto-reconnect every 2 s and port status events (`connected`, `disconnected`, `busy`, `bootloader`).
  - Register Tauri commands:
    - `trigger_pad(x, y)` / `release_pad(x, y)`
    - `select_bank(bank)`
    - `update_pad_params(bank, x, y, params)`
    - `update_group_params(group, params)`
    - `update_bank_lights(bank, lights)`
    - `stop_all()`
- Integration tests in `src-tauri/tests/app_integration.rs`.

### Task 6: Svelte UI: Bank Navigation, Pad Grid & Pad Editor
- Update `src/App.svelte` and create `src/components/PadEditor.svelte`:
  - Bank selector with 8 scribble strips (displays bank names, double-click rename, active bank highlight).
  - Bank lighting controls (flash, play-state, ripple checkboxes, humanize knob).
  - Pad editor drawer: sample name, volume slider/knob, pitch, mode (oneShot/hold), choke group, mix group, RGB swatch picker.
  - Live pad grid displaying lights emitted by Tauri router.

### Task 7: Svelte UI: 8-Channel Mixer, Status Bar & Verification
- Implement `src/components/MixerStrip.svelte` and integrate into Play stage:
  - 8 group strips: fader (volume), pan knob, pitch knob, lowpass cutoff knob, mute toggle, solo toggle.
  - Master strip with master fader.
  - Status bar updates: live audio device, sample rate, buffer size, dropout counter, Launchpad connection, and current set path.
- Final validation:
  - `cargo test --workspace`
  - `npm test`
  - `npm run check`
  - `npm run build`
