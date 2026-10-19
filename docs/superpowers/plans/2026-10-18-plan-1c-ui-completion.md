# Plan 1C: Complete the Part 1 Console — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build everything the UI spec (`../specs/2026-10-17-ui-design.md`) and the engine spec (`../specs/2026-10-17-drum-sampler-design.md`) describe for Part 1 that the app does not do yet: persistence, notices, undo, drag and drop, sample browser, telemetry feed, audio and MIDI setup, Launch Control XL, MIDI learn, step sequencer, keys instrument.

**Architecture:** The router (Rust) owns the single `Set`. Every kit edit goes through `AppState::edit`, which records undo, marks the set dirty for autosave and leaves the router holding the new set. Pure logic (history, kit ops, learn, LCXL codec, sequencer timing, FFT, synth) lives in `lpd-core` with integration tests. `src-tauri` wires threads, devices and Tauri commands. The Svelte UI only edits and draws. Hardware pad hits never pass through the UI.

**Tech Stack:** Rust (lpd-core, Tauri 2, cpal 0.15, midir 0.10, rtrb 0.3, symphonia 0.5, rustysynth 1, tauri-plugin-dialog 2, tauri-plugin-opener 2), Svelte 5 + TypeScript 5.9, Vite 8, `node --test` for `src/lib/*.test.ts`.

**How to read this plan.** Every task is self-contained: it restates the spec text it implements (so nobody has to interpret a section number), names the mockup lines to copy the look and behaviour from, fixes every new name (Rust items, Tauri commands with their exact JS argument keys, events with payload shapes, TS types, CSS classes, user-visible strings), lists the tests with expected values, and ends with a "Do not" list and a "Done when" checklist. Section 0 holds the rules and the shared name registry that all tasks use. **If a task and section 0 disagree, section 0 wins; if this plan and the specs disagree, this plan wins (it records decisions made after the specs). Report every disagreement you find instead of choosing silently.**

---

## 0. Global rules, decisions and shared names

### 0.1 Environment and workflow

- Repo root: `D:\MY DOCS\MY TOOLS\LAUNCHBAND\launchpad-drummer`. Branch: `ui-sci-retro-skin` (commit on it; do not create other branches, do not push).
- Shell: PowerShell 7 on Windows (Git Bash also available). Before cargo: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"`.
- Verification before every commit (all must pass; paste the summary lines into the task report):
  ```powershell
  cargo fmt --all -- --check
  cargo test --workspace
  npm test
  npm run check
  ```
  UI tasks also run `npm run build` (it runs `svelte-check`, then `vite build`).
- One commit per task (fix-up commits after review are fine). Message: conventional type + scope, e.g. `feat(core): …`, then a blank line, then `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- TDD: write the listed tests first, run them, see them fail for the expected reason, implement, see them pass.
- Browser preview: `npm run dev` serves `http://127.0.0.1:1420`. Without `window.__TAURI__` the app shows `createPreviewSet()`. Every feature must render and not throw there: commands resolve `undefined`, unless the task names a preview fallback.

### 0.2 Invariants (never break)

1. **Hardware-sourced pad events are never re-triggered from the UI.** `padInput({ …, source: 'hardware' })` updates selection and activity only. Only `source: 'screen'` calls `trigger_pad` / `release_pad`. The same holds for router events about learned notes (`pad-hit`) and sequencer steps.
2. **Tauri v2 argument names:** Rust command parameters are snake_case; JS passes **only** the camelCase key (`{ groupData }`, `{ humanizeDb }`, `{ fromBank }`). Never pass both spellings. Rust structs crossing the bridge use `#[serde(rename_all = "camelCase")]`.
3. **The look is done.** Do not restyle existing parts. New parts copy markup and CSS from `docs/mockups/ui-mockup-scifi.html` (line refs per task) and reuse existing classes from `src/styles/app.css` / `tokens.css`: `.plate` (+ `data-label` legend), `.screen`, `.lcd`, `.lcd-dim`, `.silk`, `.key`, `.key.on`, `.key.small`, `.key.round`, `.key.blank`, `.switch`, `.lamp` (+ `.ok`/`.warn`/`.error`), `.rack-unit`. Before adding CSS, grep `src/styles/app.css` for the selector; reuse a rule that is already ported. New CSS goes in the component's `<style>` unless several components share the class (then `app.css`). Use the tokens (`--orange`, `--warn`, `--ok`, `--error`, `--lcd`, `--lcd-dim`, `--silk`, `--vec`, `--vec-dim`, `--line`, `--line-hi`, `--glass`, `--focus`, fonts `--f-lcd`, `--f-silk`). Add a raw colour only when the mockup line uses one.
4. **No allocation, locks or I/O in the audio callback.** `crates/lpd-core/tests/no_alloc.rs` wraps engine work in `assert_no_alloc`; every new engine code path gets a case there. Nothing may be freed on the audio thread: send it back through a queue.
5. **Single set owner:** the router's `Set` is the only copy. `KitManager` holds `dir`, `samples`, `failed_samples` only. Readers clone from the router.
6. **Undo scope** (UI spec 7.3): pad params, sample replace, move/swap/copy, clear, bank names, light modes, humanize, group **names**, learn bindings, pattern edits, keys settings. **Not** mixer values (volume, pan, pitch, low-pass, mute, solo, master), whether from the screen or the LCXL. 50 steps. Undo restores the snapshot but keeps the current mixer values (`restore_keeping_mixer`).
7. **Autosave:** any change to the set (undoable or not, incl. mixer values) calls `AppState::touch()`; the saver writes 1 s after the last change.
8. Lock order in `src-tauri`: `router` → `history` → `kit_manager` → `saver`. `painter` (and `assigns`) are separate leaves: nothing else is taken while holding them; `save_gate` is taken first, only to order disk writes. Never hold `router` while calling `emit`: clone what you need, drop the guard, then emit.
9. `prefers-reduced-motion`: no key transitions, learn blink steady (2 px `--warn` ring), missing-sample blink steady red + `!`, no scope afterglow, CRT layers still. Hardware LEDs unaffected.
10. Accessibility: focus ring 2 px `--focus` at 2 px offset on `:focus-visible` only (already global). Knobs/faders `role="slider"` + `aria-valuetext`; latching keys `aria-pressed`; choke/mix/mode/step-count rows `role="radiogroup"`; telemetry and CRT layers `aria-hidden`.
11. Text uses the true minus `−`, multiplication `×`, ellipsis `…`, middle dot `·` and arrow `→` exactly as written in this plan.

### 0.3 Decisions already made (do not revisit)

| # | Decision |
|---|---|
| D1 | Keys sound engine: a built-in Rust synth (`BasicKeys`) so the keys sound out of the box; SETUP › KEYS can load a user `.sf2`, which `SoundFontKeys` (rustysynth) then plays. No `.sf2` is downloaded or bundled. (User, 2026-10-18.) |
| D2 | The sequencer and the keys plate ship in Part 1 (the brief says "everything still missing"). |
| D3 | The sequencer clock runs on a dedicated router-side thread with sleep + 1 ms spin, jitter ≤ 1 ms; not sample-accurate (UI spec 4.7 asked for sample-accurate; recorded deviation). |
| D4 | Pad learn targets carry the bank: `{"pad":{"bank":0,"x":0,"y":0}}` (the engine spec 4.2 example had no bank; recorded deviation). |
| D5 | The group pitch offset does not apply to the keys instrument routed into that group (fader, pan, low-pass, mute and solo do). |
| D6 | LCXL protocol values are the engine spec's "expected" values (2.3), unverified until `lp-probe` runs on the unit. |
| D7 | MIDI roles: Launchpad MK2 and Launch Control XL are fixed by port name. Every other input port is `keys` (if the name contains `Keystation` or the user set it) or `other`. Only `keys` ports play the keys instrument; `other` ports act only through learn bindings. Roles are app config (per machine), not set data. |
| D8 | File and folder pickers, reveal-in-Explorer and open-folder run in Rust commands (tauri-plugin-dialog / tauri-plugin-opener Rust APIs). The JS side never imports plugin packages, so capabilities stay `core:default`. |
| D9 | CRT level and BEND are per-user localStorage settings (CRT already is); the keys octave window and fold states are UI state in localStorage. Library folder, `.sf2` path, audio prefs and MIDI roles live in the app config file. Everything else lives in `set.json`. |
| D10 | Task 1 outcome (implemented in commit `10835e4`): `edit` records an undo step only when the set actually changed; a group rename that also changes values is split (name via `edit`, values via `Router::update_group` without history); `Router::release_pad_in(bank, x, y)`, `Router::install_set(set)`, `Router::send_mixer()` and `router::mixer_audio_differs(a, b)` exist; holds are tracked by pad id. |
| D11 | Audit commit `0ef8f5e` (and the Task 1 review fix-ups): lock order is `router` → `history` → `kit_manager` → `saver`, with `painter` a separate leaf (see 0.2 item 8). `update_pad` already takes `key` (Task 7 only wires it in the UI). `AppState::edit_at(key, now, f)` (injected time) and `edit_in(&mut Router, key, now, f)` (inside a held router lock) exist; `update_group` uses `edit_in` for the rename and `Router::update_group` for values, under one lock, one `touch()`. `update_master_db` and group volumes go through `fader_gain` (the −60 dB floor = silence). `update_pad` never changes an existing pad's `sample`; use `assign_sample`. `KitManager` has no `save` / `set_dir` / `import_sample_file`: saving is `AppState::save_set`, importing is `AppState::import_sample` / `store::prepare_sample`. |
| D12 | Task 9: `audio_reconnect`, the RECONNECT key and the `audioLost` notice also apply to state `error` (a failed start would otherwise have no way back); status strip shows `AUDIO ERROR`. |

### 0.4 Shared name registry

Every name below is created by the task in the "Task" column and used unchanged by later tasks. Payloads are JSON as the UI sees them (camelCase).

**Tauri commands** (Rust name; JS args object; return):

| Command | JS args | Returns | Task |
|---|---|---|---|
| `undo` / `redo` | — | `Set \| null` | 1 (done) |
| `history_state` | — | `{ canUndo, canRedo }` | 1 (done) |
| `update_pad` | `{ bank, pad, key? }` | — | exists; `key` added in 7 |
| `get_startup` | — | `Startup` | 2 |
| `new_set` / `open_set` | `{ dir }` | `OpenResult` | 2 |
| `pick_folder` | `{ title, defaultDir? }` | `string \| null` | 2 |
| `pick_file` | `{ title, extensions }` | `string \| null` | 2 |
| `reveal_path` | `{ path }` | — | 2 |
| `open_log_folder` | — | — | 2 |
| `default_sets_dir` | — | `string` | 2 |
| `get_config` | — | `AppConfig` | 2 |
| `set_library_dir` | `{ dir }` (string or null) | — | 2 |
| `set_soundfont` | `{ path }` (string or null) | `KeysStatus` | 2 (stub), 15 (real) |
| `missing_samples` | — | `MissingPad[]` | 2 |
| `save_now` | — | — (error string) | 2 (renames `save_set_dir`) |
| `drop_files` | `{ bank, x, y, paths }` | `DropReport` | 4 |
| `move_pad` | `{ fromBank, fromX, fromY, toBank, toX, toY, copy }` | `Set \| null` | 4 |
| `clear_pad` | `{ bank, x, y }` | `Set` | 4 (replaces `remove_pad`) |
| `locate_sample` | `{ bank, x, y, path }` | `Set` | 4 |
| `list_dir` | `{ path }` | `DirEntry[]` | 5 |
| `audition_file` / `stop_audition` | `{ path }` / — | — | 5 |
| `load_into_pad` | `{ bank, x, y, path }` | `DropReport` | 5 |
| `set_telemetry` | `{ on }` | — | 8 |
| `reset_dropouts` | — | — | 8 |
| `audio_devices` | `{ backend }` | `AudioDevice[]` | 9 |
| `asio_available` | — | `{ available, reason }` | 9 |
| `set_audio` | `{ prefs }` | `AudioStatus` | 9 |
| `audio_reconnect` | — | `AudioStatus` | 9 |
| `midi_devices` | — | `MidiDevice[]` | 10 |
| `set_midi_role` | `{ port, role }` | — | 10 |
| `arm_learn` | `{ target }` (`{ pad: { bank, x, y } }` or `{ param: id }`) | — | 12 |
| `cancel_learn` | — | — | 12 |
| `unbind` | `{ index }` | `Set` | 12 |
| `seq_play` / `seq_rec` | `{ play }` / `{ armed }` | — | 13 |
| `seq_update` | `{ pattern, key? }` | `Set` | 13 |
| `seq_add_lane` | `{ bank, x, y }` | `Set` (error string) | 13 |
| `seq_audition_step` | `{ bank, x, y }` | — | 13 |
| `keys_note` | `{ note, velocity, on }` | — | 15 |
| `keys_pitch` / `keys_mod` | `{ value }` | — | 15 |
| `set_keys` | `{ settings }` | `Set` | 15 |
| `keys_status` | — | `KeysStatus` | 15 |
| `trigger_pad_in` / `release_pad_in` | `{ bank, x, y }` | — | 16 |

**Events** (backend → UI):

| Event | Payload | Task |
|---|---|---|
| `launchpad-status` | `{ state, message }` (exists) | — |
| `launchpad-pad` | `{ note, pressed }` (exists) | — |
| `launchpad-frame` | `[r, g, b][64]` (exists) | — |
| `save-status` | `{ state: 'idle'\|'pending'\|'saved'\|'failed', reason: string \| null }` | 2 |
| `telemetry` | `{ scope: number[1024], bands: number[28], peakHz, master: [l, r], groups: number[8] }` | 8 |
| `audio-status` | `AudioStatus` | 8 / 9 |
| `midi-devices` | `MidiDevice[]` | 10 |
| `midi-log` | `{ text }` | 10 |
| `keys-note` | `{ note, velocity, on }` | 10 |
| `keys-wheel` | `{ pitch, mod }` | 10 |
| `mixer-changed` | `{ groups: Group[], masterDb }` | 11 |
| `learn-bound` | `{ binding: Binding }` | 12 |
| `learn-ignored` | `{ reason: string }` | 12 |
| `pad-hit` | `{ bank, x, y }` | 12 |
| `set-changed` | `Set` | 12 / 13 |
| `seq-position` | `{ playing, rec, step, length, bpm, msIntoStep }` | 13 |
| `keys-status` | `KeysStatus` | 15 |

**Set file additions** (all `#[serde(default)]` so old `set.json` files load): `learn: Vec<Binding>` (Task 12), `pattern: Pattern` (Task 13), `keys: KeysSettings` (Task 15). `SET_VERSION` stays 1.

**TS types** in `src/lib/model.ts` mirror every Rust struct above with the same field names; each task adds the types it introduces.

### 0.5 Mockup map (`docs/mockups/ui-mockup-scifi.html`)

CSS: shell + notice 85–108 · drawer, tree, pads, drop states, editor 114–180 · knobs/faders + learn dot/blink 181–197 · mixer 198–213 · keys 214–245, 389–398 · sequencer 246–275, 399–407 · setup racks 276–290 · power-on 291–301 · menus, popover, ghost 334–344, 416–417 · telemetry 421–445 · bend 446–486 · reduced motion 497–502.
JS: undo 644–650 · notices 725–775 · menus 776–789 · learn 790–809 · knob 810–836 · fader 837–857 · status strip 881–901 · grid 902–947 · banks + pad menu 948–979 · drag and drop 980–1064 · editor 1066–1133 · browser 1134–1170 · layout 1171–1225 · keys 1226–1352 · sequencer 1353–1451 · mixer + channel menu + LCXL sim 1452–1518 · setup 1519–1546 · power-on 1547–1562 · telemetry 1661–1823 · CRT/bend 1898–1936 · keyboard 1937–1965 · markup 504–566.
Copy the markup structure, class names and CSS; re-implement the behaviour in Svelte (the mockup's DOM-building `h()` code is a reference, not code to paste). Where the mockup and this plan differ, this plan wins.

---

## Phase A — Foundation

### Task 1: One set owner, edit pipeline, undo history, humanize — DONE (`10835e4`, review pending)

Implemented. The reviewers check it against this text; later tasks rely on these names.

- `crates/lpd-core/src/kit/history.rs`: `UNDO_STEPS = 50`; `History::{new, record(before, key, now), undo(current), redo(current), can_undo, can_redo, clear}`; `restore_keeping_mixer(snapshot, &current) -> Set`. The same `key` within 1 s of the previous record coalesces (and refreshes the time); `None` never coalesces; record clears redo; cap 50; undo/redo already apply `restore_keeping_mixer`.
- `AppState::edit<R>(key: Option<&str>, f: impl FnOnce(&mut Set) -> R) -> R`: applies `f` to the router set, records history only if the set changed, re-sends mixer audio only if `mixer_audio_differs`, calls `touch()`.
- `AppState::{undo, redo, history_state}`; commands `undo`, `redo`, `history_state` registered.
- Router: `trigger_pad_in(bank, x, y, velocity_gain)` is the only trigger path. Humanize = `gain × velocity_gain × db_to_gain(u × humanize_db / 2)`, `u` uniform in [−1, 1) from a xorshift32 (`set_seed` for tests). `release_pad_in`, `install_set`, `send_mixer`, `mixer_audio_differs`. Holds tracked by pad id.
- `KitManager { dir, samples, failed_samples }` (no `save`: see D11).

### Task 2: App config, startup decision, autosave, save status, log file, pickers — DONE

> **Status 2026-10-18: DONE** (the saver part in audit commit `0ef8f5e`, the rest in the commit after `4479281`). The saver keeps the audit's shape (`reset`/`due`/`finish`/`take_change`, driven by `AppState::autosave_tick`/`run_save`) instead of the `tick(now, save)` contract below; behaviour 3 is implemented by that path, not by `km.save` (D11). `AppState::restore(config_path)` runs the startup decision in `setup()`; `new_set`/`open_set` keep the audit's guards (`ensure_new_set_dir`, request tickets, sample retirement, worker threads). `config` and `startup` are leaf locks; `config` is held across its own small file write (it must not share `save_gate`, or opening a set would wait for a set save in flight). Local times use `chrono` (added dependency). TS mirrors for `Startup`, `OpenResult`, `AppConfig`, `AudioPrefs`, `MissingPad`, `KeysStatus` are in `src/lib/model.ts` (0.4). See [the remediation note](2026-10-18-code-audit-and-remediation.md#implementation-record-2026-10-18) for the saver.

**Purpose.** The app must reopen the last set, save by itself, report save failures, and offer folder and file pickers. Today nothing is remembered between runs and saving is a manual key.

**Files:**
- Create: `src-tauri/src/config.rs`, `src-tauri/src/saver.rs`, `src-tauri/src/logfile.rs`, `src-tauri/src/startup.rs`
- Modify: `src-tauri/Cargo.toml` (add `tauri-plugin-dialog = "2"`, `tauri-plugin-opener = "2"`), `src-tauri/src/lib.rs`, `state.rs`, `commands.rs`, `store.rs`, `launchpad.rs`, `audio.rs` (logging only), `src/App.svelte` (only `promptSetManagement`, see 11)

**Contracts:**
```rust
// config.rs — file: <app_config_dir>/config.json (app.path().app_config_dir()), atomic write (config.json.tmp + rename)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub last_set: Option<String>,
    pub library_dir: Option<String>,
    pub soundfont: Option<String>,
    pub audio: AudioPrefs,
    pub midi_roles: BTreeMap<String, String>, // port name → "keys" | "other"
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AudioPrefs { pub backend: String /* "wasapi" | "asio"; default "wasapi" */, pub device: Option<String>, pub buffer_frames: Option<u32> }
pub fn load(path: &Path) -> AppConfig;                 // missing or broken file → AppConfig::default()
pub fn save(path: &Path, c: &AppConfig) -> anyhow::Result<()>;

// startup.rs
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Startup {
    Loaded { path: String, name: String, from_backup: bool, backup_saved_at: Option<String> },
    FirstRun,
    NotFound { path: String },
    Unreadable { path: String, name: String },
}
/// `load` is KitManager::load_from_dir in the app, a fake in tests.
pub fn decide_startup(last: Option<&Path>, load: impl Fn(&Path) -> anyhow::Result<LoadedKit>) -> (Startup, Option<LoadedKit>);
pub fn set_name(dir: &Path) -> String;                // folder name
pub fn backup_time(dir: &Path) -> Option<String>;     // set.json.bak mtime as local "HH:MM"

// saver.rs
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SaveStatus { pub state: String /* "idle"|"pending"|"saved"|"failed" */, pub reason: Option<String> }
pub struct Saver { /* deadline: Option<Instant>, retry_at: Option<Instant>, status: SaveStatus, enabled: bool */ }
impl Saver {
    pub fn new() -> Self;
    pub fn touch(&mut self, now: Instant);                // deadline = now + SAVE_DELAY; state "pending" (only when enabled)
    pub fn set_enabled(&mut self, on: bool);              // off while no set dir is open; turning off clears the deadline
    /// Runs the save when due. Returns Some(new status) when the status changed.
    pub fn tick(&mut self, now: Instant, save: impl FnOnce() -> anyhow::Result<()>) -> Option<SaveStatus>;
}
pub const SAVE_DELAY: Duration = Duration::from_secs(1);
pub const RETRY_DELAY: Duration = Duration::from_secs(2);

// logfile.rs
pub fn init(dir: PathBuf);                               // app.path().app_log_dir()
pub fn log(line: impl AsRef<str>);                       // "[YYYY-MM-DD HH:MM:SS] line\n" appended to lpd.log; no-op before init
pub const MAX_BYTES: u64 = 1_000_000;                    // rotate: lpd.2.log→lpd.3.log, lpd.1.log→lpd.2.log, lpd.log→lpd.1.log
```

**Behaviour:**
1. `setup()` in `lib.rs`: register `.plugin(tauri_plugin_dialog::init()).plugin(tauri_plugin_opener::init())`; init the log; load the config; run `decide_startup(config.last_set)`:
   - `last_set` is `None` → `FirstRun`.
   - The path does not exist → `NotFound { path }`.
   - Load ok → `Loaded`; the router gets the set and samples (`router.set_current_set`), `km.dir` is set, the saver is enabled. `from_backup` comes from `kit::load`; `backup_saved_at = backup_time(dir)`.
   - Load fails (set.json and backup both unreadable) → `Unreadable { path, name }`. Files stay untouched (`kit::load` renames set.json aside only when the backup *was* readable — keep that).
   - Not `Loaded` → the router keeps `Set::new()`, `km.dir = None`, the saver is disabled.
   - Log the outcome and every failed sample (`{path}: {error}`).
2. Keep the decision in `AppState.startup: Arc<Mutex<Startup>>`; `get_startup` returns it.
3. `AppState::touch()` (currently a no-op) → `saver.lock().touch(Instant::now())`; it emits nothing. The saver thread (spawned in `setup`, 100 ms tick) calls `tick(now, || km.save(&set))` with `set` cloned from the router **before** locking the kit manager (lock order). Emit `save-status` on each returned change; log failures with the reason. After a failure the saver retries every 2 s until a save succeeds (state stays `failed` meanwhile); a new `touch` while failed keeps `failed` showing but moves the deadline.
4. `new_set(dir)`: create the folder, `kit::save(dir, &Set::new())`, install it, `km.dir = dir`, clear samples and failed list, clear history, enable the saver, set `config.last_set = dir` and save the config; startup becomes `Loaded`. Returns `OpenResult`.
5. `open_set(dir)`: `KitManager::load_from_dir`; on success the same as `new_set` with the loaded set and samples; on failure return `Err("Couldn't open {name} — set.json and backup are unreadable. Files left untouched.")` and change nothing.
   ```rust
   #[derive(Serialize)] #[serde(rename_all = "camelCase")]
   pub struct OpenResult { pub set: Set, pub path: String, pub name: String, pub from_backup: bool, pub backup_saved_at: Option<String> }
   ```
6. `missing_samples() -> Vec<MissingPad>` with `#[serde(rename_all = "camelCase")] MissingPad { bank: usize, x: u8, y: u8, sample: String }`: every pad in every bank whose `sample` is in `km.failed_samples` (by path) or not in `router.samples()`. Order: bank, then y descending (top row first), then x.
7. Pickers (async commands; body in `tauri::async_runtime::spawn_blocking`): `pick_folder(title, default_dir)` = `app.dialog().file().set_title(title)` (+ `.set_directory(dir)` when given) `.blocking_pick_folder()`; `pick_file(title, extensions)` adds `.add_filter(name, &extensions)` with name `"SoundFont"` when the list is `["sf2"]`, else `"Audio"`, then `.blocking_pick_file()`. Return the path as a string (`FilePath::into_path()`), `None` on cancel.
8. `reveal_path(path)`: relative paths resolve against `km.dir`; `app.opener().reveal_item_in_dir(path)`. `open_log_folder()`: `app.opener().open_path(log_dir, None::<&str>)`.
9. `default_sets_dir()` = `<Documents>\Launchpad Drummer` (`app.path().document_dir()`), created if missing.
10. `get_config`; `set_library_dir(dir)` (saves the config); `set_soundfont(path)` (saves the config; returns `KeysStatus { engine: "builtin", name: "BUILT-IN", error: None }` until Task 15 makes it real). Define `#[serde(rename_all = "camelCase")] KeysStatus { engine: String, name: String, error: Option<String> }` in `state.rs`.
11. Rename the command `save_set_dir` → `save_now` (immediate save through the saver path; emits `save-status`). Remove the `load_set_dir` and `new_set_dir` commands **and** `AppState::load_set_dir/new_set_dir`, replaced by `open_set`/`new_set`. Update `promptSetManagement` in `src/App.svelte` to call `new_set`/`open_set` and `save_now`, so the app keeps working until Task 3 replaces that code.
12. Replace `eprintln!` in `lib.rs`, `launchpad.rs`, `audio.rs`, `state.rs` with `logfile::log` (keep the `eprintln!` too under `cfg(debug_assertions)`). Also log every Launchpad status change.

**Tests:**
- `config.rs` unit: missing file → default; save + load round-trip (with one `midi_roles` entry); broken JSON → default; unknown fields are ignored.
- `saver.rs` unit (no sleeping; pass `Instant`s): touch at t0, tick at t0+0.9 s → no save; tick at t0+1.1 s → one save, status `saved`; touches at t0 and t0+0.5 s → one save, at ≥ t0+1.5 s; a failing save → `failed` with the reason; tick 1 s later → no retry; 2 s later → retry; success → `saved`; a disabled saver never saves.
- `logfile.rs` unit: temp dir with an internal test hook `init_with_limit(dir, 64)`: writing past the limit creates `lpd.1.log`; four rotations leave at most `lpd.log` + 3 older files.
- `startup.rs` unit with `tempfile`: `None` → FirstRun; missing dir → NotFound; valid set (`kit::save`) → Loaded with `from_backup: false`; broken set.json + valid .bak → Loaded with `from_backup: true` and `backup_saved_at` `Some`; both broken → Unreadable, and both files are byte-for-byte unchanged.

**Do not:** add UI for any of this (Task 3); store the CRT level in the config; save the config on every touch; hold the router lock while writing to disk (clone the set, release, then write).

**Done when:**
- [ ] Tests written first and failing for the expected reason, then passing.
- [ ] `cargo build -p launchpad-drummer` succeeds.
- [ ] `promptSetManagement` uses `new_set`/`open_set`/`save_now`; old commands removed; `npm run check` clean.
- [ ] Full verification; commit `feat(tauri): app config, startup decision, autosave, log file and pickers`.

### Task 3: Shell UI — notices, set menu, power-on, undo/redo keys, status strip, keyboard map — DONE

**Purpose.** UI spec 3, 7.1, 7.2 and 10: the top strip, the notice LCD, the power-on screen and the keyboard become real.

**Files:**
- Create: `src/lib/api.ts`, `src/lib/notices.ts` + `src/lib/notices.test.ts`, `src/lib/keymap.ts` + `src/lib/keymap.test.ts`, `src/lib/format.ts` + `src/lib/format.test.ts`, `src/lib/esc.ts`, `src/components/NoticeLcd.svelte`, `src/components/SetMenu.svelte`, `src/components/PowerOn.svelte`, `src/components/ContextMenu.svelte`
- Modify: `src/App.svelte`, `src/tauri.d.ts`, `src/lib/model.ts`, `src/components/LaunchpadPanel.svelte` (bindable `renaming` for F2), `src/styles/app.css` (menu/popover classes if not present)

**Contracts:**
```ts
// api.ts — the only module that touches window.__TAURI__
export const isTauri: boolean;
export function invoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T>; // preview: resolves PREVIEW[cmd]?.(args) ?? undefined
export function listen<T>(event: string, fn: (payload: T) => void): Promise<() => void>;   // preview: resolves a no-op unlisten
// PREVIEW map: get_startup → { kind: 'loaded', path: 'PREVIEW', name: 'PREVIEW KIT', fromBackup: false, backupSavedAt: null },
// history_state → { canUndo: false, canRedo: false }, missing_samples → [],
// get_config → { lastSet: null, libraryDir: null, soundfont: null, audio: { backend: 'wasapi', device: null, bufferFrames: null }, midiRoles: {} }.

// notices.ts — UI spec 7.1, pure
export type Level = 'error' | 'warn' | 'info' | 'ok';
export interface NoticeAction { label: string; id: 'reconnect' | 'dismiss' | 'howto' | 'show' | 'setup' }
export interface Notice { id: string; prio: number; level: Level; text: string; action?: NoticeAction }
export interface Confirmation { id: string; text: string; level?: Level }   // default level 'ok'
export interface NoticeInputs {
  learnArmed: null | 'cc' | 'note';
  audioLost: boolean;
  backup: null | { savedAt: string | null };
  saveFailed: string | null;
  launchpad: 'connected' | 'disconnected' | 'busy' | 'bootloader';
  lcxl: 'connected' | 'disconnected' | 'busy';
  missing: number;
  asioFallback: boolean;
  dropoutRecent: boolean;
  confirmations: Confirmation[];
  keysAbsent: boolean;            // Task 16 wires it; false until then
  dismissed: ReadonlySet<string>;
}
export function activeNotices(i: NoticeInputs): Notice[];   // sorted by prio ascending; dismissed ids removed
export const ICONS: Record<Level, string> = { error: '✖', warn: '⚠', info: '•', ok: '✓' };

// format.ts
/** Keep the last path segment whole when it fits; put "…" in the middle; result length ≤ max. */
export function middleTruncate(s: string, max: number): string;

// keymap.ts — UI spec 10
export function padForKey(key: string): { x: number; y: number } | null;
// "1".."8" → y 3, x 0..7; "q w e r t y u i" → y 2; "a s d f g h j k" → y 1; "z x c v b n m ," → y 0. Case-insensitive. Anything else → null.

// esc.ts — Esc chain (UI spec 10: drag → learn → rename → menu/popover; if nothing, STOP ALL)
export function pushEsc(handler: () => void): () => void;  // returns remove(); the last pushed runs first
export function runEsc(): boolean;                          // pops and runs the top handler; false when empty
```

**Notice table** (exact text; `{…}` = substitution; prio = row):

| prio | id | when | level | text | action |
|---|---|---|---|---|---|
| 1 | `learn` | `learnArmed` | warn (styled `.notice.learn`) | cc: `Move a control on any MIDI device · Esc cancels` / note: `Hit a key or pad on any MIDI device · Esc cancels` | — |
| 2 | `audio-lost` | `audioLost` | error | `Audio device lost` | RECONNECT |
| 3 | `backup` | `backup` | warn | `set.json unreadable — loaded backup (saved {savedAt})`; `savedAt` null → `(saved earlier)` | DISMISS |
| 4 | `save-failed` | `saveFailed` | error | `Couldn't save set — {reason}. Retrying` | — |
| 5 | `lp-bootloader` | `launchpad === 'bootloader'` | error | `Launchpad in bootloader — run Novation's MK2 updater, then replug` | HOW TO FIX |
| 6 | `lp-busy` | `launchpad === 'busy'` | warn | `Launchpad port busy — close Components, DAWs or another copy of this app` | — |
| 7 | `missing` | `missing > 0` | warn | `1 sample missing` / `{n} samples missing` | SHOW |
| 8 | `asio` | `asioFallback` | warn | `ASIO not available — using WASAPI` | DISMISS |
| 9 | `dropout` | `dropoutRecent` | warn | `Audio dropout — raise buffer size` | SETUP |
| 10 | `lcxl-busy` | `lcxl === 'busy'` | warn | `Launch Control XL port busy — close other MIDI apps` | — |
| 11 | `ok:{id}` | each confirmation | its level (default ok) | `{text}` | — |
| 12 | `lp-off` | `launchpad === 'disconnected'` | info | `Launchpad not connected — checking every 2 s` | — |
| 13 | `lcxl-off` | `lcxl === 'disconnected'` | info | `Launch Control XL not connected — checking every 2 s` | — |
| 14 | `keys-off` | `keysAbsent` | info | `Keystation 49 not connected — on-screen keys still play` | — |

Confirmations: `confirm(text, level = 'ok')` in `App.svelte` pushes `{ id: crypto.randomUUID(), text, level }` and removes it after 3000 ms. Texts used across tasks: `Undone`, `Redone`, `Pad cleared`, `Imported {n}`, `Learned {label}`, `Set saved`, `Lane added`.

**Components:**
- `NoticeLcd.svelte` (mockup CSS 85–108, JS 746–771): props `{ notices: Notice[]; idle: string; onaction: (n: Notice) => void }`. Shows `notices[0]`: `<span class="ico">` (`ICONS`) + `<span class="txt">` coloured by level (`.notice.error|warn|info|ok`; `.notice.learn` for prio 1), its action as `.key.small` on the right, and a `+n` `.key.small` when more notices exist, which opens a `.popover` (fixed, 460 px, below the LCD) listing every notice (icon, text, action). Empty list → the `idle` text in `.lcd-dim`. `role="status"`, `aria-live="polite"`; `aria-live="assertive"` when `notices[0].level === 'error'`. The popover closes on outside click and via `pushEsc`. `idle` = today's idle texts (`Launchpad MK2 connected` when connected; preview `Browser preview — demo kit, no audio engine`).
- HOW TO FIX opens a popover with three lines: `1. Run Novation's launchpad-mk2-updater.exe.` `2. Unplug the Launchpad.` `3. Plug it back in. The app reconnects within 2 s.`
- SHOW selects the next missing pad (cycling through `missing_samples()` in order), switches the bank if needed and opens the drawer on the editor (the one automatic drawer open, UI spec 4). SETUP → `setMode('setup')`. RECONNECT → `audio_reconnect` (Task 9; until then `invoke` returns nothing). DISMISS → add the id to `dismissed`.
- `SetMenu.svelte` replaces the set LCD button. The existing `.set-lcd` markup (`SET` silk, name LCD, `▾`) opens a `ContextMenu` with `New set…` (shortcut `Ctrl+N`), `Open set…` (`Ctrl+O`), separator, `Show in Explorer`.
  - New → `pick_folder({ title: 'New set — choose an empty folder', defaultDir: await invoke('default_sets_dir') })` → `new_set({ dir })`.
  - Open → `pick_folder({ title: 'Open set folder', defaultDir })` → `open_set({ dir })`. An error string shows in the power-on status line when the power-on screen is up, otherwise as an error confirmation.
  - After either: replace `currentSet`, `setName = result.name`, `setPath = result.path`, backup notice if `fromBackup`, refresh missing samples and history state.
  - Show in Explorer → `reveal_path({ path: setPath })`; disabled without a set.
- `ContextMenu.svelte` (mockup CSS 335–338 + 416–417, JS 777–789): props `{ x, y, items: MenuItem[]; onclose }`, `MenuItem = { label: string; shortcut?: string; disabled?: boolean; checked?: boolean; info?: boolean; sep?: boolean; action?: () => void }`. Fixed `.menu.screen`, min-width 190 px, clamped inside the window; the first enabled item gets focus; ↑/↓ move, Enter/Space activate, Esc (via `pushEsc`) and outside click close; `role="menu"`, items `role="menuitem"` (`role="menuitemcheckbox"` + `aria-checked` + a `✓ ` prefix when `checked` is defined); `info` rows are dim and not focusable. Tasks 7, 12 and 16 reuse it.
- `PowerOn.svelte` (mockup CSS 291–301, JS 1547–1562): shown in the bay instead of the Play screen when `startup.kind !== 'loaded'` and no set has been opened since. Content: title `LAUNCHPAD DRUMMER` (30 px / 700 / +0.18 em, with the mockup's chromatic-fringe class); status line: FirstRun → `No set yet. Create one or open a set folder.`; NotFound → `Last set not found: {path}`; Unreadable → `Couldn't open {name} — set.json and backup are unreadable. Files left untouched.`; lamps LAUNCHPAD / LCXL / AUDIO in the same states as the status strip; keys `NEW SET` and `OPEN SET` (same flows as the set menu). PLAY shows it; SETUP and GAMES still work; the rail stays.
- Undo/redo keys (exist in the top strip): unlit with `aria-disabled` per `history_state`; click / Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z → `undo`/`redo`; a non-null result replaces `currentSet` and confirms `Undone` / `Redone`. `refreshHistory()` (debounced 100 ms) runs after every edit command and after undo/redo.
- Status strip (mockup JS 881–901): remove the `SAVE` key. Order: LAUNCHPAD lamp, LCXL lamp, AUDIO lamp, audio LCD, `DROPS {n}` LCD (`audioStatus.dropouts ?? 0`; Task 8 fills it), set path LCD `middleTruncate(setPath, 48)` with the full path as `title`, save LED (`.lamp`: dim = idle/pending, `.ok` with a 600 ms `blink` class on `saved`, `.error` on `failed`; `aria-label` `Saved` / `Saving…` / `Save failed`). Every lamp is a button that opens SETUP (exists).
- Keyboard (`App.svelte`: one `keydown` and one `keyup` on `window`; ignore events whose target is an input, textarea, select or contenteditable; ignore `event.repeat` for pad keys):

  | Key | Action | Where |
  |---|---|---|
  | pad keys (`padForKey`) | press: `padInput({ note, pressed: true, source: 'screen' })` and select; keyup: release | Play only; not while the browser tree has focus |
  | Space | play the selected pad (press/release like a pad key) | Play; grid focused or `document.activeElement === document.body` |
  | Arrows | move the selection (clamped 0..7) | grid focused or nothing focused |
  | PageUp / PageDown | bank − / + (clamped) | Play |
  | Alt+1…8 | go to bank | Play |
  | F2 | rename the active bank (via `LaunchpadPanel`'s bindable `renaming`); if focus is inside the mixer, rename that group (MixerStrip's existing rename) | Play |
  | Delete | clear the selected pad (`clear_pad` from Task 4; until Task 4 lands, `remove_pad`) | Play |
  | Ctrl+E / Ctrl+B | toggle the drawer on the editor / browser (browser: Task 5) | Play |
  | Ctrl+K | fold the keys plate (Task 16) | Play |
  | Ctrl+1 / Ctrl+, | PLAY / SETUP (Ctrl+2 TRAIN does nothing: blank cap) | everywhere |
  | Ctrl+N / Ctrl+O | new / open set | everywhere |
  | Ctrl+Z / Ctrl+Y, Ctrl+Shift+Z | undo / redo | everywhere |
  | Esc | `if (!runEsc())`: full-screen game → exit; else on Play → STOP ALL | everywhere |

  `preventDefault()` for every handled combination. Components register Esc handlers (menus, popovers, rename inputs, drag, learn) with `pushEsc`.
- `App.svelte` refactor: replace the inline `tauri`/`invoke` with `api.ts`; delete `promptSetManagement`, `window.prompt` and `window.alert`. Add state: `startup`, `setPath`, `saveStatus`, `missing: MissingPad[]`, `history: { canUndo, canRedo }`, `confirmations`, `dismissed`. Listen to `save-status`. Load `get_startup`, `missing_samples` and `history_state` on mount.

**Tests:**
- `notices.test.ts`: (a) all inputs active → ids in the prio order of the table; (b) `dismissed: new Set(['backup'])` removes the backup row; (c) `missing: 1` → `1 sample missing`, `missing: 3` → `3 samples missing`; (d) backup text with `savedAt: '14:32'` and with `null`; (e) two confirmations keep insertion order at prio 11; (f) `launchpad: 'connected'`, `lcxl: 'connected'` and nothing else → `[]`.
- `keymap.test.ts`: all 32 keys map to the right `{x, y}` (table-driven); `'Q'` equals `'q'`; `'9'`, `'p'`, `'.'` → null.
- `format.test.ts`: a short path unchanged; a long path ≤ max with `…`; the last segment kept when it fits; a single long segment truncated in the middle.

**Do not:** redesign the top strip; bring back a SAVE key; open the drawer on a pad press; use `window.confirm/alert/prompt` anywhere.

**Done when:**
- [x] Tests first → PASS.
- [x] Preview at 1280×720: Play renders; the notice LCD shows the idle preview text; undo/redo unlit; the set menu opens and closes with Esc; keyboard keys light pads; no console errors.
- [x] Full verification + `npm run build`; commit `feat(ui): notices, set menu, power-on, undo keys and keyboard map`.

Task 3 verification (2026-10-19): helper tests failed for absent modules, then 4 passed. `cargo fmt --all -- --check`: exit 0; `cargo test --workspace`: exit 0, all passing; `npm test`: 36 passed; `npm run check`: 0 errors, 0 warnings; `npm run build`: passed. Edge preview at 1280×720: no page errors or horizontal overflow; set menu/Esc, Q pad selection, arrow movement and disabled undo checked. Power-on native picker flows await the Task 18 desktop smoke test. Ctrl+B/Ctrl+K remain with Tasks 5/16 as specified.

---

## Phase B — Editing

### Task 4: Kit operations backend (drops, move / swap / copy, clear, locate) — DONE

**Purpose.** The UI spec 5 rules, implemented once in Rust so every drop is one undo step.

**Files:**
- Create: `crates/lpd-core/src/kit/ops.rs`, `crates/lpd-core/tests/kit_ops.rs`
- Modify: `crates/lpd-core/src/kit/mod.rs` (export `ops`), `src-tauri/src/state.rs`, `commands.rs`, `lib.rs`, `src/App.svelte` (`remove_pad` → `clear_pad`)

**Contracts:**
```rust
pub const AUDIO_EXTENSIONS: [&str; 6] = ["wav", "aif", "aiff", "flac", "mp3", "ogg"];
pub fn is_audio_path(p: &Path) -> bool;                   // extension in AUDIO_EXTENSIONS, case-insensitive
pub const PRESETS: [[u8; 3]; 16] = [[63,0,0],[63,20,0],[63,40,0],[63,63,0],[30,63,0],[0,63,0],[0,63,25],[0,63,63],[0,30,63],[0,0,63],[25,0,63],[45,0,63],[63,0,63],[63,10,30],[63,63,63],[63,45,30]];
/// Row defaults (UI spec 2.1): row y (0 = bottom) gets preset number [1,2,4,6,8,10,12,14][y] (1-based) → PRESETS[n-1].
pub fn row_default_color(y: u8) -> [u8; 3];
/// The target pad first (filled or empty), then the next EMPTY pads in reading order:
/// x+1.. along row y, then row y+1 from x = 0, and so on up to row 7. At most n positions.
pub fn drop_positions(bank: &Bank, x: u8, y: u8, n: usize) -> Vec<(u8, u8)>;
/// Filled pad: replace sample, trim_start_ms = 0, trim_end_ms = None; keep colour, volume, pitch, mode, choke, mix.
/// Empty pad: Pad::new(x, y, sample, row_default_color(y)) — 0 dB, 0 st, one-shot, no choke, mix = row.
pub fn assign_sample(bank: &mut Bank, x: u8, y: u8, sample: String);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)] #[serde(rename_all = "camelCase")]
pub enum PadMove { Move, Swap, Copy }
/// copy = Ctrl held: copy onto the target (overwrites a filled target); the source stays.
/// Not copy: empty target → Move; filled target → Swap. x/y of moved pads are rewritten to their new spot.
/// None when the source is empty or source == target.
pub fn move_pad(set: &mut Set, from: (usize, u8, u8), to: (usize, u8, u8), copy: bool) -> Option<PadMove>;
pub fn clear_pad(set: &mut Set, bank: usize, x: u8, y: u8) -> bool;   // false when already empty
```
```rust
// src-tauri
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct DropReport { pub set: Set, pub imported: usize, pub leftover: usize, pub rejected: usize }
```

**Behaviour:**
- `AppState::drop_files(bank, x, y, paths)`: split `paths` into audio (`is_audio_path`) and rejected; `positions = drop_positions(bank, x, y, audio.len())`; `leftover = audio.len() − positions.len()`; import the first `positions.len()` files with `km.import_sample_file` (copies into `samples/` with the `-2`, `-3`… collision suffix — already in `kit::import_sample`) and insert each decoded `Arc<Sample>` into `router.samples_mut()`; then one `edit(None, |set| …assign_sample for each (position, relative path)…)`. A decode failure counts as rejected and is logged. Returns the new set. Nothing imported → no history step.
- `move_pad` command → `edit(None, |set| ops::move_pad(...))`; returns `Some(set)` when something moved, else `None`.
- `clear_pad` → `edit(None, …)`; returns the set.
- `locate_sample(bank, x, y, path)`: import the file, then `edit(None, …)` replacing that pad's `sample` with the new relative path and **keeping the trim** (it is the same sound found again); remove the old path from `km.failed_samples`. Empty pad → `Err("Pad is empty")`.
- Remove the `remove_pad` command and `AppState::remove_pad`. `App.svelte` calls `clear_pad({ bank, x, y })` and uses the returned set.
- Undo never deletes imported files (UI spec 5: "an undone import leaves the copied file in samples/").

**Tests (`kit_ops.rs`):**
- `drop_positions` on an empty bank from (6, 0), n = 4 → `[(6,0),(7,0),(0,1),(1,1)]`.
- With (7,0) and (0,1) filled: from (6,0), n = 3 → `[(6,0),(1,1),(2,1)]`.
- A filled target is still first: target (2,2) filled, n = 2 → `[(2,2),(3,2)]`.
- From (7,7), n = 3 → `[(7,7)]` (bank end).
- `row_default_color(0) == [63,0,0]`, `(1) == [63,20,0]`, `(2) == [63,63,0]`, `(7) == [63,10,30]`.
- `assign_sample` on a filled pad with trim 100/400, colour [0,0,63], volume −6, pitch 3, hold, choke 2, mix 5 → only the sample changes and the trim resets to 0/None.
- `move_pad` to an empty pad → Move (source gone, target has the pad with the new x/y); onto a filled pad → Swap (both rewritten); `copy: true` onto a filled pad → Copy (both equal except x/y); cross-bank move; empty source → None; same spot → None.
- `clear_pad` → true, then false.
- `is_audio_path("A.WAV")` true, `"a.txt"` false, `"noext"` false.
- `src-tauri` state test (style of `src-tauri/tests/app_state.rs`): temp set dir, three 10 ms WAVs written with `hound`, bank where (2,0) is the only empty pad after the target (1,0) and everything from (3,0) up is filled → dropping 3 files on (1,0) → imported 2, leftover 1; one undo restores the bank.

**Do not:** build the drag UI here; handle learn bindings (Task 12 extends these ops); delete files on undo.

**Done when:** [x] tests first → PASS; [x] commands registered, `remove_pad` gone; [x] full verification; commit `feat(core): pad drop, move, swap, copy and clear operations`.

Task 4 verification (2026-10-19): 3 core operations tests and 2 state tests RED→GREEN; fmt clean, workspace tests passed, npm 36 passed, Svelte 0 errors/warnings, build passed. D11: imports use prepare_sample and edit_in with request/epoch checks; changed targets reject a stale batch. Moving preserves mix routing (the initial test expected the destination row incorrectly and was corrected).

### Task 5: Sample browser and audition — DONE

**Purpose.** UI spec 4.4, plus 7.3: "Audition plays a file once through the master bus, not through a pad."

**Files:**
- Create: `src/components/SampleBrowser.svelte`, `src/lib/tree.ts` + `src/lib/tree.test.ts`
- Modify: `crates/lpd-core/src/audio/engine.rs`, `crates/lpd-core/src/audio/mod.rs`, `crates/lpd-core/tests/audio_engine.rs`, `crates/lpd-core/tests/no_alloc.rs`, `src-tauri/src/router.rs`, `state.rs`, `commands.rs`, `lib.rs`, `src/App.svelte`

**Contracts:**
```rust
// engine.rs
pub const AUDITION_PAD: u16 = u16::MAX;
// new Command variants:
Command::Audition { sample: Arc<Sample> },   // plays the whole sample once, unity gain, centred
Command::StopAudition,
```
- The engine keeps two audition `Voice` slots outside `voices`, so the fade-out of the old audition (3 ms) and the new start overlap without allocation. Audition voices render into a dedicated stereo bus that is added to the output **after** the group sum and **before** `master_gain` and the soft clipper. They emit no `VoiceStarted`/`VoiceEnded`. `StopAll` and `StopAudition` fade them over 5 ms.
- `Router::audition(sample: Arc<Sample>)` pushes the command and keeps the last two auditioned `Arc`s alive in a field, so the audio thread never drops the last reference.
```rust
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct DirEntry { pub name: String, pub path: String, pub is_dir: bool }
```
- `list_dir(path)`: `std::fs::read_dir`; folders first, then audio files (`ops::is_audio_path`), each group sorted case-insensitively; skip names starting with `.` and entries with the Windows hidden or system attribute (`MetadataExt::file_attributes() & (0x2 | 0x4)`); errors → `Err(message)`.
- `audition_file(path)`: decode on a `std::thread::spawn` worker (`kit::decode_file`), then `router.audition(...)`; a decode error → `Err` (the UI shows the error confirmation `Can't play {name}`).
- `load_into_pad(bank, x, y, path)` = `drop_files(bank, x, y, vec![path])`.

```ts
// tree.ts
export interface TreeNode { name: string; path: string; isDir: boolean; depth: number; open?: boolean; children?: TreeNode[] }
export function flatten(nodes: TreeNode[]): TreeNode[];                  // visible rows, depth-first; children only under open folders
export type TreeAction = 'open' | 'close' | 'toggle' | 'audition' | 'load' | 'parent';
export function keyNav(rows: TreeNode[], index: number, key: string): { index: number; action?: TreeAction };
// ArrowDown / ArrowUp: ±1 clamped. ArrowRight: closed folder → 'open'; open folder → index of its first child; file → nothing.
// ArrowLeft: open folder → 'close'; otherwise → index of the parent row with action 'parent'. Space: folder → 'toggle', file → 'audition'.
// Enter: folder → 'toggle', file → 'load'. Other keys: index unchanged, no action.
```

**UI (`SampleBrowser.svelte`; mockup markup 539, CSS 114–133 + 370, JS 1134–1170):**
- Props: `{ libraryDir: string | null; selected: { bank: number; x: number; y: number } | null; onclose: () => void; onload: (path: string) => void; ondragfile: (path: string, e: PointerEvent) => void }`.
- Header: `×` close key (`aria-label="Close browser"`), library path LCD (`middleTruncate(dir, 34)`, full path in `title`; `NO LIBRARY FOLDER` when null), `FOLDER…` key → `pick_folder({ title: 'Choose sample library folder', defaultDir: libraryDir })` → `set_library_dir({ dir })`.
- Tree: `role="tree"`, `aria-activedescendant`, `tabindex="0"`; `.tree` with `.ti.folder` / `.ti.file` rows (22 px, mono 12 px; file rows show the extension in `.ext`); folder rows have `aria-expanded` and a `▸`/`▾` glyph; children load lazily with `list_dir` on the first open and are cached; `.ti.active` marks the keyboard cursor; `.ti.playing` (♪) marks the file last auditioned for 1.5 s or until another audition.
- Click a file = audition it and make it active. Click a folder = toggle. Keyboard per `keyNav` (Space auditions, Enter loads into the selected pad via `onload`).
- `pointerdown` on a file row calls `ondragfile(path, e)` (Task 6 turns it into a drag after 4 px; a click without movement still auditions).
- No library folder: the text `Choose a library folder to browse samples.` and the FOLDER… key.
- `App.svelte`: drawer state becomes `'editor' | 'browser' | null`; the BROWSER tab (`.dtab`, now a blank cap) becomes live: `class:on`, `role="tab"`, `aria-selected`, title `Sample browser (Ctrl+B)`. Ctrl+B toggles. One panel at a time. Esc closes the drawer (via `pushEsc` while it is open). `onload(path)` → `load_into_pad` for the selected pad → replace the set, confirm `Imported 1` (or the bank-full text from Task 6), refresh history. No selected pad → error confirmation `Select a pad first`.
- Preview fallback: `list_dir` returns a fake tree with folders `Kicks`, `Snares`, `FX` and the preview set's sample names.

**Tests:**
- `audio_engine.rs`: (a) all 8 groups muted, an audition of a 0.5-amplitude 100-frame DC sample renders non-zero output (it bypasses groups); (b) master gain 0 → silent (the audition sits before the master); (c) a second audition while the first plays: no sample-to-sample step above the existing click-test threshold; (d) the event queue stays empty.
- `no_alloc.rs`: handling `Audition` and rendering 2 blocks with an audition active does not allocate.
- `tree.test.ts`: flatten with nested open and closed folders; keyNav for each key at the first row, the last row, a nested file (ArrowLeft → parent index), a closed folder and an open folder.

**Do not:** route auditions through a pad or group; add a search box (spec: ask before adding); scan the library recursively up front.

**Done when:** [x] tests first → PASS; [x] preview: the BROWSER tab opens the browser, the keyboard moves the cursor, Space marks ♪, Enter with no pad selected shows the error confirmation; [x] full verification + build; commit `feat: sample browser with audition`.

Task 5 verification (2026-10-19): listed engine/no-allocation/tree tests RED→GREEN; directory filter/sort test also RED→GREEN. fmt/workspace tests passed; npm 38 passed; Svelte 0 errors/warnings; build passed. Edge 1280×720: browser folder navigation, Space audition marker, Enter with no selection error, Esc close, Ctrl+B reopen; no runtime errors/overflow. Callback uses two audition voices and a 3 ms attack to satisfy the click threshold. D11 retirement also retains older queued auditions beyond the two latest references. No pad is preselected at startup so loading requires the player’s choice.

### Task 6: Drag and drop — DONE

**Purpose.** UI spec 5 in the UI: pad drags, browser file drags, Explorer file drops.

**Files:**
- Create: `src/lib/dnd.ts` + `src/lib/dnd.test.ts`, `src/components/DragGhost.svelte`
- Modify: `src/components/PadGrid.svelte`, `LaunchpadPanel.svelte`, `SampleBrowser.svelte`, `PadEditor.svelte`, `src/App.svelte`, `src/styles/app.css` (port `.pad.drop`, `.pad.nodrop`, `.tag`, `.scrib.drop`, `.editor.drop`, `.ghost` from mockup lines 150–165 and 341–342 if missing)

**Contracts:**
```ts
export interface Rect { left: number; top: number; width: number; height: number }
export type DropTarget = { kind: 'pad'; x: number; y: number } | { kind: 'bank'; bank: number } | { kind: 'editor' } | null;
/** Pads first, then banks, then the editor. Left/top edges inclusive, right/bottom exclusive. */
export function hitTest(px: number, py: number, pads: { x: number; y: number; rect: Rect }[], banks: { bank: number; rect: Rect }[], editor: Rect | null): DropTarget;
export type DropTag = 'IMPORT' | `IMPORT ×${number}` | 'REPLACE' | 'MOVE' | 'SWAP' | 'COPY';
export function dropTag(source: 'files' | 'pad', targetFilled: boolean, ctrl: boolean, count: number): DropTag;
// files: count > 1 → `IMPORT ×${count}`; else filled → 'REPLACE', empty → 'IMPORT'.
// pad: ctrl → 'COPY'; filled → 'SWAP'; empty → 'MOVE'.
export const DRAG_THRESHOLD = 4;     // px; below = click
export const BANK_HOVER_MS = 400;
export function physicalToCss(p: { x: number; y: number }, dpr: number): { x: number; y: number };
export function isAudioName(name: string): boolean;     // same extension list as Rust AUDIO_EXTENSIONS
export function passedThreshold(dx: number, dy: number): boolean; // Math.hypot(dx, dy) >= DRAG_THRESHOLD
```

**Behaviour (UI spec 5, exact):**

| Drag | Drop on | Result | Tag |
|---|---|---|---|
| Explorer/browser file | empty pad | copy to `samples/` (clash → `-2`, `-3`…); defaults: row colour, 0 dB, 0 st, ONE-SHOT, no choke, mix = row | IMPORT |
| Explorer/browser file | filled pad | replace sample, reset trim; keep colour, volume, pitch, mode, choke, mix | REPLACE |
| several files | pad | target pad, then the next **empty** pads in reading order; leftovers dropped with the notice `{n} files not imported — bank full` | IMPORT ×n |
| pad | empty pad | move (all params) | MOVE |
| pad | filled pad | swap | SWAP |
| pad + Ctrl | any pad | copy (overwrites a filled target) | COPY |
| anything | side button / scribble strip held 400 ms | switch to that bank, the drag continues | — |
| non-audio file or folder | anywhere | not-allowed cursor, red bezel on the hovered pad, notice `Not an audio file` | — |
| anything | outside targets, or Esc | cancel (no drag-to-delete) | — |

- Pointer events only (no HTML5 DnD). Pad drag: the pad plays on `pointerdown` (exists). The drag starts when the pointer moves ≥ 4 px while held. At the start call `release_pad` for that pad (hold voices are released) and mark the press as consumed so `pointerup` sends no second release. The ghost (`DragGhost.svelte`: `.ghost.screen`, fixed, follows the pointer, `pointer-events: none`) shows a 22 px swatch in the source colour with a glow and the sample label. The hovered target pad gets `.drop` (dashed white bezel) and a `.tag` LCD with `dropTag(...)`. Ctrl is read live from each pointer event (`e.ctrlKey`), so pressing Ctrl mid-drag switches MOVE/SWAP → COPY. A drop on a pad → `move_pad({ fromBank, fromX, fromY, toBank: activeBank, toX, toY, copy })`; a non-null result replaces `currentSet`; the target pad gets selected.
- Hovering a side button or scribble strip for 400 ms (`BANK_HOVER_MS`) shows `.scrib.drop` while waiting, then `chooseBank(i)`; the drag continues and the pad rects are re-read.
- Browser drag: `SampleBrowser` calls `ondragfile(path, e)`; same machinery with source `files`, count 1, ghost label = file name. A drop on a pad → `drop_files({ bank: activeBank, x, y, paths: [path] })`; a drop on the editor (`.editor.drop` while hovered) targets the selected pad. A browser press without 4 px of movement stays a click (audition).
- Explorer drops (Tauri 2 webview events): `listen('tauri://drag-enter', { paths, position })`, `listen('tauri://drag-over', { position })`, `listen('tauri://drag-drop', { paths, position })`, `listen('tauri://drag-leave')`. `position` is in physical px: convert with `physicalToCss(position, devicePixelRatio)`. Remember `paths` on enter. If none is audio (`isAudioName`): the hovered pad gets `.nodrop` (red bezel), the body gets `cursor: not-allowed`, and a drop shows the error confirmation `Not an audio file`. Otherwise the hovered pad shows `.drop` + the tag (`IMPORT ×n` for several files). Drop → `drop_files` → replace the set; confirm `Imported {imported}`; `leftover > 0` → error confirmation `{leftover} files not imported — bank full` (singular `1 file not imported — bank full`); `rejected > 0` and nothing imported → `Not an audio file`. Bank hover works for Explorer drags too.
- Rects: `PadGrid` exposes `rects(): { x, y, rect }[]` (`getBoundingClientRect` of the pad elements, read at drag start and after a bank switch); `LaunchpadPanel` exposes bank rects (side button and scribble strip both count as bank `i`); `PadEditor` exposes its root rect while the drawer shows the editor.
- Esc during a drag cancels (`pushEsc` while dragging); a drop outside every target cancels; nothing is deleted.
- After each successful drop: `refreshHistory()` and `refreshMissing()`.
- Preview: drags show the ghost and tags; drops call `invoke` (resolves `undefined`) and change nothing.

**Tests (`dnd.test.ts`):** `hitTest` returns the pad for a point inside a pad rect, the bank for a scribble rect, the editor, and null outside; edges: left/top inclusive, right/bottom exclusive. `dropTag` for every row of the table incl. `IMPORT ×3`. `physicalToCss({x: 300, y: 150}, 1.5)` → `{x: 200, y: 100}`. `isAudioName('A.FLAC')` true, `'readme.md'` false. `passedThreshold(3, 0)` false, `(3, 3)` true.

**Do not:** use HTML5 `draggable`; add drag-to-delete; open the drawer on drop; call `trigger_pad` during a drag.

**Done when:** [x] tests first → PASS; [x] preview: dragging a pad shows the ghost and the MOVE/SWAP/COPY tags, Esc cancels, bank hover switches banks after 400 ms; [x] full verification + build; commit `feat(ui): drag and drop for pads, browser files and Explorer files`.

Task 6 verification (2026-10-19): `dnd.test.ts` (hitTest order and half-open edges, every dropTag row incl. `IMPORT ×3`, physicalToCss, isAudioName, passedThreshold) written first. fmt clean; cargo test --workspace all passed; npm test 41 passed; npm run check 0 errors/0 warnings; npm run build passed. Browser preview (synthetic pointer events): pad drag shows the ghost with the pad label and the MOVE tag on an empty pad, SWAP on a filled one, COPY when Ctrl is held mid-drag; Esc removes ghost and tags; hovering a scribble strip shows `.scrib.drop` and switches bank after 400 ms. Not verified here: real Explorer drops (Tauri drag events), browser-file drags with a real library, editor drop in the desktop app. Small additions: dropping a pad on itself is a no-op; the editor drop also tags the selected pad; the bank hover outline clears once that bank is active. Review fix-ups: Ctrl is also read from the pointerup event and from Control keydown/keyup while dragging; a click that follows an Esc-cancelled drag is suppressed like a completed drag's (preview: tag SWAP→COPY→SWAP on Control, click after Esc swallowed, later click passes).

### Task 7: Pad editor completion, pad context menu, grid keyboard and labels — DONE

**Purpose.** UI spec 4.1, 4.3 and 11.

**Files:**
- Modify: `src/components/PadEditor.svelte`, `PadGrid.svelte`, `LaunchpadPanel.svelte`, `src/App.svelte`, `src/lib/model.ts` + `model.test.ts`, `src-tauri/src/commands.rs` + `state.rs` (`update_pad` gains `key`)

**Contracts:**
- `update_pad(bank: usize, pad: Pad, key: Option<String>)`; JS `{ bank, pad, key }`; Rust passes `key.as_deref()` to `edit`. Knob and trim drags send `key: \`pad:${bank}:${x}:${y}:${field}\`` so one drag = one undo step; swatch clicks and mode/choke/mix keys send no key.
- `model.ts`: `export function padAriaLabel(pad: Pad | null, x: number, y: number, groups: Group[], missing: boolean): string` → `Row 4, column 2, KICK 808, mix 4, choke off` / `Row 1, column 1, empty` / `Row 2, column 3, SNARE, mix 2, choke 1, sample missing` (row = y + 1, column = x + 1, name = `padLabel(sample).toUpperCase()`, mix = mixGroup + 1).

**Pad editor (mockup JS 1066–1133, CSS 160–180), top to bottom — exact:**
1. Header: `R{y+1}·C{x+1}` (dim LCD), sample name 16 px (`padLabel(sample).toUpperCase()`, or `EMPTY`), `{length s} · {rate kHz}` (`0.45 s · 44.1 kHz`) when the peaks are known, `CLEAR` key (`clear_pad`, confirmation `Pad cleared`).
2. Missing sample (the pad's sample is in `missing`): `.errbox` red box `Sample missing: {sample}` + a `LOCATE…` key → `pick_file({ title: 'Locate sample', extensions: ['wav','aif','aiff','flac','mp3','ogg'] })` → `locate_sample({ bank, x, y, path })` → replace the set, refresh missing and peaks.
3. SAMPLE · TRIM: the waveform (exists; keep the drawing code); handles drag with the history key; min 10 ms apart; a click elsewhere auditions (exists). LCD `START x ms   END y ms`.
4. COLOUR (exists: readout, 16 swatches 8×2, white ring on the current one, `+RGB` with three 0–63 knobs).
5. VOLUME knob (−60…+6; −60 shows `−∞`), PITCH knob (±24 st, 1 st steps, Shift 0.1), MODE switch ONE-SHOT | HOLD (exists).
6. CHOKE row OFF, 1–8; MIX GROUP row 1–8 with the group legend on the right (exists).
7. MIDI: LEARN key with `aria-disabled="true"` and title `MIDI learn — not wired yet` until Task 12; no binding LCD yet.
- **Remove** the free-text SAMPLE path input and `pathDraft`.
- Empty pad: header `EMPTY` and exactly `Empty pad. Drop a sample here, drag one from the browser, or press Enter on a file in the browser.` The editor root is a drop target (Task 6).
- Props gain `bank: number`, `missing: boolean`, `onlocate: () => void`, `onclear: () => void`; `onupdate(pad, key?)`.

**Pad grid:**
- `role="grid"` container with a single `tabindex="0"` stop and `aria-activedescendant` = the selected pad's element id (`pad-{x}-{y}`); rows `role="row"`, pads `role="gridcell"` with `aria-label={padAriaLabel(...)}` and `aria-selected`.
- Arrows inside the grid move the selection (App's handler does it when the grid is focused); Space plays the selected pad.
- Missing pad: a `!` glyph (`.miss`, top-right, white with a dark shadow) in addition to the LED blink; with reduced motion the glyph stays and the pad shows steady red (`@media (prefers-reduced-motion: reduce)` on `.pad.missing`).
- Empty pad: a faint `+` centred on hover (`.pad.empty:hover::after { content: '+' }` in `--lcd-dim` at 40 %).
- Right-click a pad (or Shift+F10 / ContextMenu key while the grid is focused; the menu then opens at the selected pad's rect): `ContextMenu` items `Learn MIDI note…` (disabled until Task 12), `Clear pad` with shortcut `Del` (filled pads only), `Show sample in Explorer` (filled pads only → `reveal_path({ path: pad.sample })`). The right-click selects the pad but does not play it.
- The Delete key and the CLEAR key → `clear_pad`.

**Tests:** `model.test.ts`: the three `padAriaLabel` examples above.

**Do not:** bring back a typed path input; play a pad on right-click; change the waveform drawing.

**Done when:** [x] test first → PASS; [x] preview: the right-click menu opens at the pad and closes with Esc; Shift+F10 on the focused grid opens it at the selected pad; arrows move the bezel; [x] full verification + build; commit `feat(ui): pad editor missing-sample box, pad menu and grid keyboard`.

Task 7 verification (2026-10-19): `padAriaLabel` test (the three plan examples) written first, RED (missing export) then GREEN. fmt clean; cargo test --workspace all passed; npm test 42 passed; npm run check 0 errors/0 warnings; npm run build passed. Browser preview (Chrome 1280×720): right-click opens the menu at the pointer (Learn MIDI note… disabled, Clear pad Del, Show sample in Explorer) and selects without playing, Esc closes; ArrowUp/ArrowRight then Shift+F10 on the focused grid moved `aria-activedescendant` pad-0-0 → pad-1-1 and opened the menu at that pad's centre; 64 `gridcell` in 8 `row` elements with the plan's labels; with a temporary preview `missing_samples` entry the pad showed the `!` glyph and the editor the `Sample missing: …` errbox with LOCATE…; empty pad shows the exact empty text; no console errors (the temporary entry was reverted). D11: `update_pad` already took `key` and `padEditKey` already sends the `pad:{bank}:{x}:{y}:{field}` keys for knob/trim drags, so no Rust change was needed. Notes: the typed-path input, `pathDraft`, the editor's `onassign`/`onremove` props and App's now-unused `assignSample` were removed; the existing `.pad.empty:hover .glyph::before` `+` rule was kept instead of adding a second one (no restyling of existing parts); pads are `tabindex="-1"` and the grid is the single tab stop (mousedown focuses the grid, Enter on the grid presses the selected pad); the reduced-motion steady-red `.pad.missing` rule and the locate flow (pick_file, locate_sample, peaks refetch) could not be exercised here (no Tauri, no OS media query).

---

## Phase C — Audio and telemetry

### Task 8: Engine taps, FFT, telemetry stream, dropout counter — DONE

**Purpose.** UI spec 4.8 and 7.3 "Telemetry stream"; engine spec 9 dropouts.

**Files:**
- Create: `crates/lpd-core/src/audio/taps.rs`, `crates/lpd-core/src/audio/fft.rs`, `crates/lpd-core/tests/audio_taps.rs`, `crates/lpd-core/tests/audio_fft.rs`, `src-tauri/src/telemetry.rs`
- Modify: `crates/lpd-core/src/audio/{engine,mod}.rs`, `crates/lpd-core/tests/no_alloc.rs`, `src-tauri/src/audio.rs`, `state.rs`, `lib.rs`, `commands.rs`, `src/components/Telemetry.svelte`, `src/components/MixerStrip.svelte` (fills only), `src/lib/activity.ts`, `src/App.svelte`, `src/lib/model.ts`

**Contracts:**
```rust
// taps.rs
pub const SCOPE_CAPACITY: usize = 8192;
pub const LEVELS_CAPACITY: usize = 512;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Levels { pub master: [f32; 2], pub groups: [f32; 8] }   // peak |x| over one rendered block
pub struct Taps { pub scope: rtrb::Consumer<f32>, pub levels: rtrb::Consumer<Levels> }
// engine.rs
impl Engine { pub fn with_taps(sample_rate: u32) -> (Engine, rtrb::Consumer<Event>, Taps); }   // `new` unchanged, no taps
```
- In `render_block`, when taps exist: per group, the peak of its output contribution after low-pass, pan and gain (0 when muted or silenced by solo); master L/R peak after the soft clipper; push one `Levels` per block (ignore a full queue); push the mono sum `(l + r) / 2` of every output frame into the scope queue (stop pushing when full). No allocation.
```rust
// fft.rs
pub const FFT_SIZE: usize = 2048;
pub const BANDS: usize = 28;
pub struct Fft { /* twiddles, Hann window, bit-reversal table, work buffers — all allocated in new() */ }
impl Fft { pub fn new() -> Self; pub fn magnitudes(&mut self, input: &[f32; FFT_SIZE]) -> &[f32; FFT_SIZE / 2]; } // Hann-windowed, scaled |X|/(N/4) so a full-scale sine ≈ 1.0
/// 28 log-spaced bands from 40 Hz to 16 kHz; each band = max magnitude of its bins in dBFS (floor −100);
/// a band narrower than one bin takes the nearest bin. Second value: frequency of the loudest bin overall (0 when silent).
pub fn bands(mags: &[f32; FFT_SIZE / 2], sample_rate: u32) -> ([f32; BANDS], f32);
/// First index i in 1..buf.len()/2 with buf[i-1] < 0 <= buf[i]; 0 if none.
pub fn trigger_index(buf: &[f32]) -> usize;
pub fn to_dbfs(x: f32) -> f32;   // 20·log10(x), floor −100
```
- `src-tauri/src/telemetry.rs`: a thread spawned in `setup()` with a 16 ms period. It owns an `Fft` and a 2048-sample ring (latest samples). Each tick: drain `taps.scope` into the ring; drain `taps.levels`, keeping the per-channel max since the last frame; if `AppState.telemetry_on` (AtomicBool) is set, emit `telemetry` `{ scope, bands, peakHz, master, groups }` where `scope` = 1024 samples of the ring starting at `trigger_index(ring)`, rounded to 3 decimals; `bands` = dBFS ×28; `master` = `[dBFS L, dBFS R]`; `groups` = linear peaks 0..1. The taps live in `AppState.taps: Arc<Mutex<Option<Taps>>>` so Task 9 can swap them.
- Command `set_telemetry(on: bool)`. The UI sends `true` while `mode === 'play'` and `document.visibilityState === 'visible'`, otherwise `false` (UI spec: nothing is sent while the window is hidden).
- Dropouts (`audio.rs`): the output callback records `Instant::now()`; an interval between callbacks longer than `1.8 × frames / rate` is a dropout. The pure rule is `pub fn is_dropout(interval: Duration, frames: usize, rate: u32) -> bool`. The count (`AtomicU32`) and `last_dropout_ms` (`AtomicU64`, ms since stream start) live in an `Arc<AudioStats>` shared with `AppState`. The first callback never counts. `AudioStatus` gains `dropouts: u32` and `last_dropout_ago_ms: Option<u64>` (JSON `dropouts`, `lastDropoutAgoMs`), filled when read. The telemetry thread emits `audio-status` whenever the dropout count changes. `reset_dropouts()` zeroes the counter.
- `audio.rs::start` uses `Engine::with_taps`; `AudioSystem` gains `taps: Taps` and `stats: Arc<AudioStats>`.

**UI:**
- `Telemetry.svelte` (mockup JS 1661–1823, CSS 421–445): keep the existing raster/bit-crush pipeline, fonts and layout; replace only the data:
  - Head: orange pill `SIGNAL`; clock `T+mm:ss.t` since app start; 4 beat lamps (Task 14 drives them; dark until then); fold ▸ (exists).
  - Oscilloscope: trace from `scope` (near-white, 1.5 px with glow) plus 3 fading afterglow traces from the previous frames (none with reduced motion); readouts `CH1 · MASTER` and `{x} ms/div` with `x = 1024 / sampleRate × 1000 / 8`, one decimal (2.7 ms/div at 48 kHz; sample rate from `audioStatus`). Silent = master below −90 dBFS for 500 ms → flat trace + `STANDBY · PLAY A PAD`.
  - Spectrum: 28 bars from `bands`, −72…0 dBFS mapped to 0…1 of the height; 4 px segments with 2 px gaps (raster); teal below 60 % of the height, amber to 85 %, red above; bars fall ~12 % of the height per frame; cream peak caps hold 400 ms then fall 1 % per frame; scale `100 / 1k / 10k`; readout `PEAK {f}` (`94 Hz`, `1.2 kHz`).
  - Orbit radar: channel energy from `groups` (keeps the 110 ms half-life decay in `activity.ts`); outer ring and planet: Task 14.
  - Master L/R: 36-segment meters −48…+3 dB (green < −9, amber to 0, red above), falling 36 dB/s, 1.2 s peak hold; scale −40 / −24 / −12 / −6 / 0; readout `PK {x} dB` (the larger hold value, one decimal, true minus).
  - Folded strip (30 px): vertical L/R meters from the same data.
  - Browser preview (no feed): keep today's STANDBY drawing.
- `activity.ts`: add `export function setLevels(groups: number[]): void` — `energy[i] = max(energy[i], groups[i])`. The hit-based `hitGroup` stays for the preview and as the fallback when no feed arrived within 1 s; `App.svelte` stops calling `hitGroup` in `padInput` while a feed is live.
- Mixer level fills (`.f-fill[data-group]`, exist): `--m` = `dbToPos` of the channel level in dB, i.e. on the fader's own scale; MASTER gets two fills (L left of the slot, R right) from `master` (mockup CSS 375–388). Colours come from the existing CSS (green to 0 dB, amber to +4, red above).
- Top-strip chips (exist) keep reading `energy`.
- Status strip `DROPS {n}` from `audioStatus.dropouts`; the AUDIO lamp is amber while `lastDropoutAgoMs < 10000` (re-evaluated every second); the notice input `dropoutRecent` uses the same rule.

**Tests:**
- `audio_fft.rs`: a 1 kHz full-scale sine at 48 kHz → `peakHz` within ±25 Hz of 1000; the band containing 1 kHz is the loudest and > −6 dBFS; silence → all bands −100 and `peakHz == 0`; `trigger_index` on a sine that starts at phase π/2 returns the first sample after the negative half-cycle; an all-positive buffer → 0; `to_dbfs(1.0) == 0`, `to_dbfs(0.0) == −100`.
- `audio_taps.rs`: a DC sample on group 3 → `levels.groups[3] > 0` and every other group 0; the same with group 3 muted → 0; master peak > 0; after rendering 256 frames the scope consumer holds 256 values.
- `no_alloc.rs`: a `with_taps` engine, trigger + render 4 blocks inside `assert_no_alloc`, also with the scope queue full.
- `audio.rs` unit: `is_dropout(12 ms, 256, 48000)` true (threshold 9.6 ms), `is_dropout(6 ms, 256, 48000)` false.

**Do not:** compute the FFT on the audio thread; send telemetry while the window is hidden or on another screen; change the telemetry look.

**Done when:** [x] tests first → PASS; [x] full verification + build; commit `feat: telemetry stream from engine taps, FFT and dropout counter`.

Task 8 verification (2026-10-19): written first and RED (missing items): `audio_fft.rs` (5), `audio_taps.rs` (6), `no_alloc.rs` tapped case incl. both queues full, `is_dropout` unit test, `src-tauri/tests/telemetry.rs` (7: frame shape, 3-decimal rounding, feed off drains and resets peaks, peak max-then-reset, dropout counter/reset/age), TS `telemetry.test.ts`, `activity.test.ts` (setLevels, master fall) and `status.test.ts` (dropoutRecent). fmt clean; cargo test --workspace all passed; npm test 52 passed; npm run check 0 errors/0 warnings; npm run build passed. Browser preview (Chrome, synthetic `pushFrame` calls at 60 Hz, since there is no engine in the preview): scope trace with afterglow, 28 coloured bars with caps, `PEAK 1.2 kHz`, orbit radar, master meters with `PK −2.0 dB`, group and master mixer fills all move; with zero frames the scope shows `STANDBY · PLAY A PAD` after 500 ms, meters fall to nothing and `PK −∞`; reload without a feed shows the old STANDBY drawing, no console errors. Deviations/notes: `trigger_index` searches `1..len/2` as written in the contract (so 1024 samples always follow it); the DC bin is ignored for `peakHz`; `AudioStats` also exposes `dropouts()`, `record_dropout(now)`, `reset()` and `last_dropout_ago_ms(now)` (None when the count is 0); `AppState` gained `taps`, `set_taps`, `set_audio_stats`, `set_telemetry`, `reset_dropouts`; the lib-side feed state lives in `src/lib/telemetry.ts` (subscribed in App so the mixer fills work while the telemetry column is hidden), `activity.ts` gained `setLevels`, `setMaster`, `masterDb`, `feedLive`; spectrum bars fall by ×0.88 per frame (mockup law) for "~12 % per frame". `DROPS {n}` already existed in the status strip; the AUDIO lamp is amber while `lastDropoutAgoMs` + time since the status arrived is under 10 s (re-evaluated every second), and the same flag feeds `dropoutRecent`. Task 9 still has to swap `taps` and `stats` on a stream restart and send `audio-status` with `state: "lost"`. Not verified here: real audio device (dropout counting, taps under real callbacks, telemetry events through Tauri), `set_telemetry` toggling in the desktop app, the reduced-motion afterglow rule (no OS media query), feel of the meters.

### Task 9: Audio controller, SETUP › AUDIO, SETUP as racks — DONE

**Purpose.** UI spec 8 item 1 and 7.1 rows 2, 8, 9; engine spec 9 (device lost, ASIO fallback).

**Files:**
- Create: `src/components/SetupAudio.svelte`, `src/components/RackUnit.svelte` (wraps `Plate` with four registration marks and the orange pill title; mockup CSS 276–290, JS 1519–1546)
- Modify: `src-tauri/src/audio.rs`, `lib.rs`, `state.rs`, `router.rs`, `commands.rs`, `config.rs`, `telemetry.rs`, `src/App.svelte`, `src/components/SetupDisplay.svelte` (DISPLAY rack only), `src/lib/model.ts`

**Contracts:**
- **Audio owner thread.** `cpal::Stream` is not `Send`, so one thread owns it: `pub struct AudioController { tx: mpsc::Sender<AudioRequest> }`, `enum AudioRequest { Start(AudioPrefs, mpsc::Sender<Result<AudioStatus, String>>), Stop }`. On `Start`: drop the old stream, build a new `Engine::with_taps`, start the stream, call `router.replace_audio(tx, rx)`, swap `AppState.taps` and `AppState.stats`, reply with the status. `lib.rs` starts it with `config.audio` at startup instead of calling `audio::start` directly (removes today's `_audio_stream` in `run()`).
- `Router::replace_audio(&mut self, tx: Producer<Command>, rx: Consumer<Event>)`: swap the queues, clear voice counts and holds, re-send the mixer (`send_mixer`) and the keys settings (Task 15 fills a call site marked `// keys: Task 15`), nothing else.
- Device lost: the stream error callback sets `AudioStats.lost` (AtomicBool) and logs it; the telemetry thread sees the flag and emits `audio-status` with `state: "lost"`. `audio_reconnect()` restarts with the current prefs, only when the state is `lost` or `stopped`; otherwise it returns the current status.
- ASIO: prefs `backend == "asio"` but `asio_host()` fails → start WASAPI, set `asioFallback: true`, log it.
```rust
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct AudioDevice { pub name: String, pub is_default: bool, pub buffer_sizes: Vec<u32> }
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct AsioInfo { pub available: bool, pub reason: Option<String> }   // reason: "Built without ASIO support" | "No ASIO driver found"
pub fn buffer_choices(min: u32, max: u32) -> Vec<u32>;   // powers of two in 32..=2048 within [min, max]
pub fn latency_ms(frames: u32, rate: u32) -> f32;        // frames / rate × 1000, rounded to one decimal
```
- `AudioStatus` (extended; camelCase): `state` (`"running" | "stopped" | "lost" | "error"`), `backend` (`"WASAPI" | "ASIO"`), `deviceName`, `sampleRate`, `bufferSize: Option<u32>`, `channels`, `dropouts`, `lastDropoutAgoMs`, `asioFallback: bool`, `latency: { midiToBufferMs, outputMs, totalMs }`. `midiToBufferMs = latency_ms(buffer, rate)` (buffer unknown → the callback's measured frame count); `outputMs` from `OutputCallbackInfo::timestamp()` (`playback.duration_since(&callback)`, stored as µs in `AudioStats.output_us`); `totalMs` = the sum.
- Commands: `audio_devices(backend)` (WASAPI = `cpal::default_host()`, ASIO = `asio_host()`; each output device; `SupportedBufferSize::Range { min, max }` → `buffer_choices`, `Unknown` → `[]`), `asio_available()`, `set_audio(prefs)` (saves `config.audio`, restarts, returns the status), `audio_reconnect()`, `get_audio_status` (exists, now with the new fields). All `async`, waiting on the reply channel with a 5 s timeout.

**UI:**
- SETUP becomes a single column of rack units (max 860 px, scrolls), in this order: AUDIO (this task), MIDI DEVICES (Task 10), KEYS (Task 16), MIDI LEARN (Task 12), DISPLAY (exists), LOG (Task 17). A rack appears when its task lands.
- `RackUnit.svelte`: props `{ title: string; children }` → `.plate.rack-unit` with four registration marks (circle + crosshair in `--vec-dim`) and an `<h1>` with the orange pill marker (mockup).
- `SetupAudio.svelte` (mockup 1519–1530), rows with a silk label on the left:
  - `BACKEND`: two-position switch `ASIO | WASAPI`; ASIO `aria-disabled` with the title `asio_available().reason` when not available.
  - `DEVICE`: LCD select (`select.lcd.screen`) of `audio_devices(backend)`; the default device is marked `(default)`.
  - `BUFFER`: LCD select of the device's `bufferSizes` plus `AUTO` (null); empty list → only `AUTO`.
  - `LATENCY`: LCD `MIDI→BUFFER {a} · OUTPUT {b} · TOTAL {c} ms`.
  - `DROPOUTS`: readout LCD `{n}`, `RESET` key (`reset_dropouts`), hint `Raise buffer size` (`.lcd-dim`) when n > 0.
  - `STREAM`: lamp (green running / amber recent dropout / red stopped or lost) + state text (`RUNNING`, `STOPPED`, `DEVICE LOST`) + `RECONNECT` key, `aria-disabled` unless lost or stopped.
  - Any change → `set_audio({ prefs })` → the returned status replaces `audioStatus`.
- Status strip audio LCD: `{backend} · {bufferSize ?? 'AUTO'} · {totalMs} ms` (e.g. `WASAPI · 256 · 5.3 ms`); stopped → `AUDIO STOPPED`; lost → `AUDIO LOST`.
- Notices: `audioLost = audioStatus.state === 'lost'`; `asioFallback = audioStatus.asioFallback` (DISMISS adds `asio` to `dismissed`).
- Listen to `audio-status`.
- Preview: demo status `{ state: 'running', backend: 'WASAPI', deviceName: 'Speakers (Preview)', sampleRate: 48000, bufferSize: 256, channels: 2, dropouts: 0, lastDropoutAgoMs: null, asioFallback: false, latency: { midiToBufferMs: 5.3, outputMs: 10, totalMs: 15.3 } }` and two fake devices.

**Tests:** `buffer_choices(64, 1024) == [64, 128, 256, 512, 1024]`; `buffer_choices(1, 4096) == [32, 64, …, 2048]`; `buffer_choices(100, 120) == []`; `latency_ms(256, 48000) == 5.3`. Router: after `replace_audio`, a trigger lands in the new queue, and the new queue starts with 8 `SetGroup` + 1 `SetMasterGain`.

**Do not:** keep the stream on `run()`'s stack; restart the stream from the router thread; block the UI while restarting.

**Done when:** [x] tests first → PASS; [x] app builds; [x] preview SETUP shows the AUDIO and DISPLAY racks with registration marks; [x] full verification + build; commit `feat: audio device selection, reconnect and setup audio rack`.

Task 9 verification (2026-10-19): written first: router `replace_audio_swaps_queues_and_resends_the_mixer` (new queue starts with 8 `SetGroup` + 1 `SetMasterGain`, a trigger lands in the new queue only, voice counts and holds cleared), `audio.rs` units (`buffer_choices`, `latency_ms`, status JSON names, per-stream error flag), `tests/audio_controller.rs` (6, fake starter: start swaps queues/taps/status and resends the mixer, restart, failed start gives `error` and no taps, stop, reconnect only when lost/stopped/error, `apply` saves prefs first), `tests/audio_health.rs` rewritten for `lost`, `telemetry.rs` lost tick, TS `audio.test.ts` and `status.test.ts` (`audioLabel`). fmt clean; cargo test --workspace all passed; npm test 57 passed; npm run check 0 errors/0 warnings; npm run build passed. Browser preview (Chrome): SETUP shows AUDIO, MIDI DEVICES (still the old rack, Task 10 replaces it) and DISPLAY as rack units with registration marks; BUFFER 128 changes the status strip to `WASAPI · 128 · 12.7 ms` and the LATENCY LCD; ASIO is `aria-disabled` with the title `Built without ASIO support`; RECONNECT disabled while running; no console errors. Deviations/notes: `AudioStats` also carries `frames`/`output_us`/`lost`; the earlier `StreamHealth` and `poll_stream_health` were folded into `AudioStats.lost` + `AppState::poll_stream_lost` (state `lost`, noticed by the telemetry thread, which logs it); `AudioSystem.stream` is `Option` and `AudioController::spawn_with(state, starter)` takes the stream builder so the controller is testable without a device; `AudioStatus` lost `Eq` (f32 latency); a failed start leaves state `error` (the plan lists the value without a rule) and RECONNECT / `audio_reconnect` / the `audioLost` notice also apply to `error` (D12); `set_audio` saves the prefs before restarting, so a failing device is remembered for the retry; in the preview `get_audio_status` now answers the demo status, so the Telemetry `ms/div` readout shows `2.7` there. The MIDI DEVICES plate stays in `SetupDisplay.svelte` (moved above DISPLAY to match the final order); `.setup` got a flex column with a 24 px gap for the stacked racks. `// keys: Task 15` marks the call site in `Router::replace_audio`. Review fix-ups (`fix: task 9 review fix-ups`): a start that fails with a fixed buffer is retried once with AUTO; a failed start clears the router's voice counts and holds and invalidates the LED painter (also after a successful install); the boot start is detached (does not block `setup()`); every finished start is announced as `audio-status` even after the 5 s reply timeout; `install_audio` sets the status before the stats; the owner thread catches panics (error status, thread stays alive); `get_audio_status` is async; telemetry clears its ring and emits no frame while there are no taps; the unused `Stop` request was removed. Not verified here: real audio devices (WASAPI/ASIO start, device list and buffer ranges, unplug -> `lost` -> RECONNECT, driver-reported output latency, ASIO fallback), the 5 s timeout path, the desktop app build running the audio start in `setup()`.

---

## Phase D — MIDI

### Task 10: Device registry, generic MIDI inputs, SETUP › MIDI DEVICES — DONE

**Purpose.** Engine spec 7 ("All connected MIDI inputs are listened to"), UI spec 8 item 2, status lamps.

**Files:**
- Create: `src-tauri/src/devices.rs`, `src/components/SetupMidi.svelte`
- Modify: `src-tauri/src/launchpad.rs` (status into the registry), `state.rs`, `router.rs`, `lib.rs`, `commands.rs`, `config.rs`, `src/App.svelte`, `src/components/SetupDisplay.svelte` (move the MIDI section out), `src/lib/model.ts`

**Contracts:**
```rust
#[derive(Serialize, Clone, Debug, PartialEq)] #[serde(rename_all = "camelCase")]
pub struct MidiDevice { pub port: String, pub role: String, pub status: String }
// role: "launchpad" | "lcxl" | "keys" | "other"; status: "connected" | "disconnected" | "busy" | "bootloader"
pub fn classify(port: &str, roles: &BTreeMap<String, String>) -> &'static str;
// lpd_core::devices::launchpad port match → "launchpad"; name contains "Launch Control XL" → "lcxl";
// roles[port] == "keys" | "other" → that; name contains "Keystation" (case-insensitive) → "keys"; else "other".
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MidiMsg { NoteOn { ch: u8, note: u8, vel: u8 }, NoteOff { ch: u8, note: u8 }, Cc { ch: u8, cc: u8, value: u8 }, PitchBend { ch: u8, value: i16 } }
pub fn parse(bytes: &[u8]) -> Option<MidiMsg>;
// 0x9n vel > 0 → NoteOn; 0x9n vel 0 or 0x8n → NoteOff; 0xBn → Cc; 0xEn → PitchBend ((lsb | msb << 7) − 8192);
// anything else (SysEx, running status, short messages) → None
pub struct Registry { /* devices: Vec<MidiDevice> */ }
impl Registry {
    pub fn set(&mut self, port: &str, role: &str, status: &str) -> bool;   // true = changed
    pub fn remove_absent(&mut self, present: &[String]) -> bool;
    pub fn list(&self) -> Vec<MidiDevice>;
}
```
- `devices.rs` thread (2 s poll, modelled on `launchpad.rs`): enumerate input ports with `midir`; connect every port classified `keys` or `other` that is not connected yet (connect error → status `busy`); keep the `MidiInputConnection`s in a map. Callback: `parse(bytes)` → `AppState::on_midi(&port, role, msg)`. Ports that disappeared leave the map and the registry. Launchpad and LCXL ports are **not** opened here; their bridges report their status into the shared `Arc<Mutex<Registry>>` (the Launchpad bridge reports `bootloader` when it detects it). Emit `midi-devices` with the full list whenever `set`/`remove_absent` report a change.
- `AppState::on_midi(port, role, msg)` → `Router::on_midi(port, role, msg) -> Vec<UiEvent>`; the caller emits the events after releasing the lock. In this task, role `keys`: NoteOn/NoteOff → `UiEvent::KeysNote { note, velocity, on }` (event `keys-note`); PitchBend → `UiEvent::KeysWheel { pitch: value / 8192, mod }`; CC 1 → `UiEvent::KeysWheel { pitch, mod: value / 127 }` (the router keeps the last pitch and mod so each event carries both; event `keys-wheel`). Role `other`: nothing yet (Task 12). Every message also produces `UiEvent::MidiLog { text: "{port}: {msg:?}" }` (event `midi-log`).
- Commands: `midi_devices()`, `set_midi_role(port, role)` (accepts `keys`/`other`, saves the config; the poll thread re-classifies on its next round).

**UI:**
- `SetupMidi.svelte` (mockup 1531–1536): rack `MIDI DEVICES` with a table: lamp · port · role · status. Lamp: green connected, off disconnected, amber busy, red bootloader. Role: fixed text `LAUNCHPAD` / `LCXL` for those; a two-position switch `KEYS | OTHER` for the rest (`set_midi_role`). Status text: `connected`, `disconnected`, `port busy`, `bootloader — run updater`. Empty list: `No MIDI devices found. Plug one in; the list refreshes every 2 s.` Below the table: the existing `MIDI EVENT LOG` list, moved unchanged from `SetupDisplay.svelte` and now also fed by `midi-log` (keep the 40-entry cap; the game prototypes' diagnostics rely on this log). The old `RETRY FRAME` key and the Launchpad-only status line go away.
- Status strip LAUNCHPAD and LCXL lamps from the registry (`ok` connected, plain when disconnected or absent, `warn` busy, `error` bootloader). Notices keep `deviceState` from `launchpad-status`; the `lcxl` input comes from the registry (absent = `disconnected`).
- `keysAbsent` notice input stays false here (Task 16 wires it).
- Preview: registry demo `[{ port: 'Launchpad MK2', role: 'launchpad', status: 'connected' }, { port: 'Launch Control XL', role: 'lcxl', status: 'disconnected' }, { port: 'Keystation 49 MK3', role: 'keys', status: 'connected' }]`.

**Tests:** `classify` for `Launchpad MK2`, `Launchpad MK2 16`, `2- Launchpad MK2`, `Launch Control XL`, `Keystation 49 MK3`, `Some Synth` (→ other) and `Some Synth` with a `keys` override; `parse` for `90 3C 64`, `90 3C 00`, `80 3C 40`, `B0 01 7F`, `E0 00 40` (→ 0), `E0 7F 7F` (→ 8191), `E0 00 00` (→ −8192), `F0 … F7` → None, `[0x3C, 0x64]` → None; `Registry::set` returns false for an identical update.

**Do not:** open Launchpad or LCXL ports in `devices.rs`; play the keys instrument (Task 15); bind learn (Task 12).

**Done when:** [x] tests first → PASS; [x] full verification + build; commit `feat: MIDI device registry, roles and setup MIDI rack`.

Task 10 verification (2026-10-19): written first (failing on `unimplemented!()` stubs): `devices.rs` units (`classify` for all listed names plus overrides and fixed-role protection, `parse` for every listed message incl. pitch bend 0 / 8191 / −8192, SysEx, running status and short messages → None, `Registry::set` false for an identical update, `remove_absent` keeps the Launchpad and LCXL rows, camelCase JSON), `router.rs` units (`on_midi`: keys NoteOn/NoteOff → `KeysNote`, wheel events carry both last values, CC 1 → mod, other CCs and role `other` only log, MIDI never queues an audio command, `UiEvent` names and payloads), `state.rs` units (`set_midi_role` saves and updates the row, refuses fixed devices and unknown roles; `on_midi` hands events back), TS `midi.test.ts` (lamps, status texts, fixed role text, `roleStatus`, 40-entry log cap). fmt clean; cargo test --workspace all passed; npm test 62 passed; npm run check 0 errors/0 warnings; npm run build passed. Browser preview (Chrome): SETUP shows AUDIO, MIDI DEVICES (table with lamps, fixed LAUNCHPAD / LCXL text, KEYS | OTHER switch that moves on click, event log below) and DISPLAY; status strip LAUNCHPAD lamp green and LCXL plain from the demo registry; no console errors. Deviations/notes: `UiEvent` lives in `router.rs` (`KeysWheel` has a `r#mod` field; JSON key `mod`); `AppState.midi` holds the shared `Arc<Mutex<Registry>>` (leaf lock) and `devices::report` updates it and emits `midi-devices`; the Launchpad bridge reports under the fixed row name `Launchpad MK2` (not the Windows port name) from `report_status`; `remove_absent` keeps launchpad/lcxl rows (their bridges own them; no LCXL bridge exists until Task 11, so that row is absent = disconnected); `set_midi_role` also updates the registry row at once and emits `midi-devices` (the poller then reopens the port on its next round, reconnecting when the role changed); `set_midi_role` refuses Launchpad/LCXL ports; the UI switch is optimistic and reloads the list on error; the `keys-note` / `keys-wheel` events are emitted but nothing in the UI listens yet (Tasks 15/16); `midi-log` and the existing `launchpad-status` / `launchpad-pad` lines feed the one log (40 cap, `pushLog`). The old `RETRY FRAME` key was removed from the UI (the `refresh_leds` command stays). `PowerOn` gets the LCXL state from the registry. Not verified here: real MIDI hardware (port enumeration and connect on Windows, busy detection with another app holding a port, hot-plug, Keystation notes and wheels reaching the log, bootloader row, role persistence across restarts). Review fix-ups (`fix: task 10 review fix-ups`): recorded deviation from "every message produces `MidiLog`": every message is still processed by the router (wheel tracking, later learn), but only the `midi-log` line is thinned: notes always, pitch bend and each CC number at most once per 100 ms (`LogThrottle`, state in the midir callback). `keys-wheel` is coalesced: the router's newest wheel event is kept in `AppState` and an emitter thread sends it every 16 ms (about 60 Hz, the last value always arrives). `Router::on_midi(role, msg)` now returns `Option<UiEvent>` (plain data, no allocation) instead of `Vec<UiEvent>`, and `AppState::on_midi(port, role, msg, log)` builds the log line after the router lock is released. Closing a keys input or changing its role emits note-offs for the notes still held (a 128-bit set shared by callback and poller) and resets the wheels. The poller reports the freshest role so the KEYS/OTHER switch does not snap back. `classify` treats any port containing `Launchpad MK2` (e.g. `MIDIIN2 (Launchpad MK2)`) as launchpad; `lpd_core::is_launchpad_port` is unchanged because the bridge must pick only the main port. SetupMidi: empty log copy `Waiting for MIDI events.`, `aria-live="off"`, hidden-text header for the lamp column, lamp `aria-hidden` (status text is the label), no `set_midi_role` call when the selected role is clicked. Tests added: `LogThrottle`, held-note set, wheel coalescing and reset, secondary Launchpad port. Totals after the fix: lib 59 passed, npm test 62 passed.

### Task 11: Launch Control XL — DONE

**Purpose.** Engine spec 2.3, 6 (LCXL LEDs) and 7.1 mapping; UI spec 6: "Hardware LCXL moves update the screen at ~30 Hz … the value jumps to the hardware (no pickup)."

**Files:**
- Create: `crates/lpd-core/src/devices/lcxl.rs`, `crates/lpd-core/tests/lcxl.rs`, `src-tauri/src/lcxl.rs`
- Modify: `crates/lpd-core/src/devices/mod.rs`, `src-tauri/src/router.rs`, `state.rs`, `lib.rs`, `src/App.svelte`

**Contracts:**
```rust
pub const PORT_NAME: &str = "Launch Control XL";
pub fn is_lcxl_port(name: &str) -> bool;              // trimmed name contains PORT_NAME (case-sensitive)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LcxlEvent { Fader { ch: u8, value: u8 }, Knob { row: u8, ch: u8, value: u8 }, Button { row: u8, ch: u8, on: bool } } // ch, row 0-based
/// Any MIDI channel. CC 77–84 → Fader ch 0–7; CC 13–20 → Knob row 0; 29–36 → row 1; 49–56 → row 2.
/// Note 41–44 → Button row 0 ch 0–3, 57–60 → row 0 ch 4–7; 73–76 → row 1 ch 0–3, 89–92 → row 1 ch 4–7;
/// Note On vel > 0 = on; Note On vel 0 or Note Off = off. Anything else → None.
pub fn decode(msg: &[u8]) -> Option<LcxlEvent>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum LedColor { Off, Red, Green, Amber }
pub fn led_value(c: LedColor) -> u8;                  // Off 0x0C, Red 0x0F, Green 0x3C, Amber 0x3F
pub fn led_index(row: u8, ch: u8) -> u8;              // row 0 (track focus) 24 + ch; row 1 (track control) 32 + ch
pub fn led_message(template: u8, index: u8, value: u8) -> Vec<u8>; // F0 00 20 29 02 11 78 <template> <index> <value> F7
pub fn fader_db(value: u8) -> f32;    // p = value/127; p ≥ .75 → (p − .75)/.25 × 6; 0 < p < .75 → max(−60, 40·log10(p/.75)); p = 0 → −60
pub fn knob_pan(value: u8) -> f32;    // (value − 64) / 63 clamped to −1..1 (0 → −1, 64 → 0, 127 → 1)
pub fn knob_pitch(value: u8) -> f32;  // round((value − 64) / 63 × 12) clamped to ±12
pub fn knob_lowpass(value: u8) -> f32; // 200 × 100^(value/127): 0 → 200, 127 → 20000
pub const USER_TEMPLATE: u8 = 0;
```
- `src-tauri/src/lcxl.rs`: a bridge thread modelled on `launchpad.rs` (2 s reconnect, input + output found by `is_lcxl_port`, `busy` on a connect error, status into the registry and `logfile::log`). Input → `AppState::on_lcxl(ev)` → `Router::on_lcxl(ev) -> bool` (changed): Fader → group `volume_db = fader_db`; Knob row 0 → `pan`, row 1 → `pitch_semitones`, row 2 → `lowpass_hz`; Button row 0 `on` → toggle `mute`; row 1 `on` → toggle `solo`; releases ignored. Uses `Router::update_group` (no history) + `touch()`.
- LEDs: on (re)connect and whenever mute/solo differs from the last sent state (the bridge compares every 16 ms tick, so screen changes show too), send for ch 0–7: row 0 = Amber if muted else Off; row 1 = Green if soloed else Off, with `led_message(USER_TEMPLATE, led_index(row, ch), led_value(c))`.
- `mixer-changed` `{ groups, masterDb }`: the bridge emits it at most every 33 ms after a hardware change, trailing edge (the last value is always sent). `App.svelte` replaces `currentSet.mixer` with it.

**Tests (`lcxl.rs`):** decode `B0 4D 40` → Fader ch 0 value 64; `B8 54 7F` (MIDI channel 9) → Fader ch 7; `B0 0D 00` → Knob row 0 ch 0; `B0 24 7F` → Knob row 1 ch 7; `B0 38 01` → Knob row 2 ch 7; `90 29 7F` → Button row 0 ch 0 on; `90 3C 7F` → Button row 0 ch 7 on; `90 59 00` → Button row 1 ch 4 off; `80 4C 00` → Button row 1 ch 3 off; `B0 15 10` → None (CC 21 unmapped); `led_message(0, 24, 0x3F) == [0xF0,0x00,0x20,0x29,0x02,0x11,0x78,0x00,0x18,0x3F,0xF7]`; `fader_db(0) == −60`, `fader_db(127) == 6`; `knob_pan(64) == 0`, `knob_pan(127) == 1`, `knob_pan(0) == −1`; `knob_pitch(0) == −12`; `knob_lowpass(127) == 20000`, `knob_lowpass(0) == 200`. Router unit: a Fader event sets the group volume and pushes one `SetGroup`; two Button row 0 presses → mute true, then false; a release changes nothing.

**Do not:** let LCXL messages reach MIDI learn (Task 12 ignores them); add pickup / soft takeover (spec: ask before adding); record LCXL moves in history.

**Done when:** [x] tests first → PASS; [x] full verification + build; commit `feat: Launch Control XL mixer mapping and LEDs`.

Task 11 verification (2026-10-19): `lcxl.rs` codec tests were RED (module missing) then GREEN (9 tests). The router, state and bridge tests were written together with the code, so they were not seen RED first. fmt clean; `cargo test --workspace` all passed (68 lib tests); `npm test` 62 passed; `npm run check` 0 errors, 0 warnings; `npm run build` passed. Implementation notes: `Router::on_lcxl` returns whether the mixer changed (a release or the same value again changes nothing) and goes through `update_group`, so no history. `AppState::on_lcxl` sets a dirty flag and calls `touch()`; the bridge thread (11 ms tick) emits `mixer-changed` when the flag is set and ≥ 33 ms passed since the last emit (trailing edge: the flag stays until emitted), and diffs mute/solo against the last LED state every 16 ms, so screen changes light the LEDs too; a new connection sends all 16 LEDs. The bridge reports its row `Launch Control XL` (role `lcxl`) into the registry. LCXL input is not offered to learn yet (Task 12 adds the `LearnIgnored` hook). Not verified: a real Launch Control XL (D6: protocol values are the spec's expected ones), hot-plug, LED colours, how the faders feel.

### Task 12: MIDI learn — DONE

**Purpose.** UI spec 8.1, 8 item 3, 4.3 item 7 and 4.5 (right-click Learn / Reset); engine spec 7.

**Files:**
- Create: `crates/lpd-core/src/kit/learn.rs`, `crates/lpd-core/tests/kit_learn.rs`, `src/components/SetupLearn.svelte`, `src/lib/learn.ts` + `src/lib/learn.test.ts`
- Modify: `crates/lpd-core/src/kit/{model,mod,ops}.rs`, `src-tauri/src/router.rs`, `state.rs`, `commands.rs`, `lib.rs`, `src/components/{Knob,Fader,MixerStrip,LaunchpadPanel,PadEditor,PadGrid}.svelte`, `src/App.svelte`, `src/lib/model.ts`

**Contracts:**
```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] #[serde(rename_all = "camelCase")]
pub struct Binding { pub device: String, pub kind: LearnKind, pub number: u8, pub target: Target }
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)] #[serde(rename_all = "camelCase")]
pub enum LearnKind { Note, Cc }
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] #[serde(rename_all = "camelCase")]
pub enum Target { Pad { bank: u8, x: u8, y: u8 }, Param(String) }
// JSON: {"pad":{"bank":0,"x":0,"y":0}} or {"param":"group.2.volume"}
// Set gains: #[serde(default)] pub learn: Vec<Binding>
pub fn is_param(id: &str) -> bool;  // the ids in the table below
pub enum ParamValue { Continuous(f32), Toggle }
/// CC value → parameter value, per the table below.
pub fn param_value(id: &str, cc: u8) -> Option<ParamValue>;
/// Removes any binding with the same (device, kind, number) AND any binding with the same target, then pushes b.
pub fn bind(learn: &mut Vec<Binding>, b: Binding);
pub fn unbind(learn: &mut Vec<Binding>, index: usize) -> bool;
pub fn retarget_pad(learn: &mut Vec<Binding>, from: (u8, u8, u8), to: Option<(u8, u8, u8)>); // (bank, x, y); None = remove
```
Param ids and mappings (exact):

| id | value |
|---|---|
| `group.{0-7}.volume` | dB, `lcxl::fader_db` law |
| `group.{0-7}.pan` | `lcxl::knob_pan` |
| `group.{0-7}.pitch` | `lcxl::knob_pitch` |
| `group.{0-7}.lowpass` | `lcxl::knob_lowpass` |
| `group.{0-7}.mute`, `group.{0-7}.solo` | Toggle |
| `master` | dB, fader law |
| `bank.{0-7}.humanize` | 0..6 dB, step 0.5 |
| `bank.{0-7}.flash`, `.playState`, `.ripple` | Toggle |
| `keys.volume` | −40..+6 dB, linear |
| `keys.mod` | 0..1 |
| `seq.play` | Toggle |
| `seq.bpm` | 60..200, integer |
| `seq.swing` | 50..75, integer |
| `stopAll` | Toggle |

Toggle: the router fires when the CC value crosses from < 64 to ≥ 64 (it keeps the last value per binding).
- Ops (Task 4 functions extended): `move_pad` Move → `retarget_pad(from → to)`; Swap → exchange the two pads' bindings; Copy → unchanged (a copy does not duplicate the binding); `clear_pad` → `retarget_pad(pad, None)`.
- Router state `learn_armed: Option<LearnTarget>` with `LearnTarget = Pad { bank, x, y } | Param(String)`. `arm_learn` replaces any armed target. While armed, from a `keys`/`other` device: a Note On (pad target) or a CC (param target) binds via `edit(None, |set| bind(...))`, disarms, and returns `UiEvent::LearnBound(binding)`. Launchpad/LCXL input while armed is not bound and yields `UiEvent::LearnIgnored(reason)` once per arm, reason `LCXL controls are fixed in Part 1` (LCXL) or `Launchpad pads are fixed` (Launchpad); the Launchpad pad still plays and the LCXL still moves the mixer. A note while a param is armed, or a CC while a pad is armed, is ignored silently.
- Not armed: a Note On matching a `Note` binding to a pad → `trigger_pad_in(bank, x, y, vel / 127)`, the painter flash when the bank is active, and `UiEvent::PadHit { bank, x, y }` (the UI selects the pad when `bank === activeBank` and **never** calls `trigger_pad`); Note Off → `release_pad_in`. A note bound to a pad does not reach the keys instrument. A CC matching a param binding → applied through the same functions the UI commands use (group values via `update_group` without history; bank/keys/seq params via `edit` with key `learn:{id}` so a sweep coalesces), then `UiEvent::MixerChanged` or `UiEvent::SetChanged`, each throttled to 33 ms.
- Commands: `arm_learn(target: LearnTargetDto)` with `#[serde(rename_all = "camelCase")] enum LearnTargetDto { Pad { bank: u8, x: u8, y: u8 }, Param(String) }` (JS `{ target: { pad: { bank, x, y } } }` or `{ target: { param: 'group.2.volume' } }`), `cancel_learn()`, `unbind(index) -> Set` (via `edit`, undoable).

**UI:**
- `learn.ts`: `paramLabel(id, set)` → `3 TOMS · VOLUME` (groups: `groupLegend` + `VOLUME|PAN|PITCH|LPF|MUTE|SOLO`), `MASTER`, `BANK 2 · HUMANIZE` (and `· FLASH|PLAY|RIPPLE`), `KEYS · VOLUME`, `KEYS · MOD`, `SEQ · PLAY|BPM|SWING`, `STOP ALL`; `targetLabel(target, set)` (pads: `B{bank+1} R{y+1}·C{x+1} · {SAMPLE}`); `bindingLabel(b, set)` → `{device} · {NOTE|CC} {number} → {targetLabel}`; `boundIds(set): Set<string>` (param ids and `pad:{bank}:{x}:{y}`); `noteBoundTo(set, note): { bank: number; x: number; y: number } | null` (Task 16 uses it for key dots; matches any device).
- `Knob`, `Fader` and latching keys that map to a param get a `learnId?: string` prop. Right-click (`contextmenu` with `preventDefault`) or Shift+F10 / ContextMenu key while focused → `ContextMenu` items `Learn MIDI CC` and `Reset` (Reset = the double-click default; keys have no Reset). Learn → `arm_learn({ target: { param: learnId } })`; the control gets `.learning` (mockup CSS 188: amber outline blinking with `steps(1)` every 0.5 s; reduced motion: a steady 2 px `--warn` ring) until bound or cancelled. Bound controls get `.learned` (a 6 px amber dot top-right, mockup CSS 187); give the control `position: relative`.
- `learnId`s to wire: mixer PAN/PITCH/LPF knobs (`group.{i}.pan|pitch|lowpass`), faders (`group.{i}.volume`; MASTER `master`), MUTE/SOLO keys (`group.{i}.mute|solo`), bank strip FLASH/PLAY/RIPPLE keys (`bank.{b}.flash|playState|ripple`), HUMANIZE knob (`bank.{b}.humanize`), the Launchpad panel's STOP ALL key (`stopAll`). The keys plate (Task 16) and the sequencer (Task 14) wire theirs.
- Pad learn: the pad editor's `LEARN` key becomes live (lit amber while waiting) and the pad menu's `Learn MIDI note…` is enabled; both call `arm_learn({ target: { pad: { bank, x, y } } })`. When bound, the editor MIDI row shows `.learn-lcd` `NOTE {n} · {device}` and a `×` key (`aria-label="Clear MIDI binding"`) → `unbind({ index })`.
- Notice row 1 (`learnArmed: 'cc' | 'note'`) while armed; Esc cancels (`pushEsc` → `cancel_learn`). `learn-bound` → clear the armed state, replace the set, confirm `Learned {bindingLabel}`, refresh history. `learn-ignored` → error confirmation with the reason.
- `pad-hit` → treated like a hardware hit: when `bank === activeBank`, `padInput({ note: (y + 1) * 10 + x + 1, pressed: true, source: 'hardware' })` (selects + activity), then a matching release after 120 ms for the screen's pressed state.
- `set-changed` → replace `currentSet`.
- `SetupLearn.svelte` (rack `MIDI LEARN`, mockup 1537–1541): one row per binding, `{device} · {NOTE|CC} {n}` → `{targetLabel}`, with a `×` key (`unbind`). Empty text exactly: `No bindings. Right-click any knob, fader or key → Learn MIDI CC. Pads: LEARN in the pad editor.`

**Tests:**
- `kit_learn.rs`: serde of both target kinds matches the JSON above; an old set.json without `learn` loads with `learn == []`; `bind` with a reused (device, kind, number) moves it (one binding left, with the new target); `bind` to an already-bound target replaces the old source; `param_value("group.0.volume", 127)` = Continuous(6.0); `("group.0.pan", 0)` = Continuous(−1.0); `("seq.bpm", 127)` = Continuous(200.0); `("group.3.mute", 100)` = Toggle; `is_param("group.8.volume")` false; ops: Move retargets, Swap exchanges, Copy leaves one binding, clear removes it.
- Router unit: armed Param + CC from an `other` device → binding added, disarmed, `LearnBound`; armed + an LCXL fader → `LearnIgnored` and no binding; not armed + a bound Note On → a Trigger in the audio queue + `PadHit`; a bound CC on `group.1.volume` changes the group; toggle fires only on the rising edge (values 0, 100, 120, 10, 90 → two toggles).
- `learn.test.ts`: `paramLabel` for a group, master and bank humanize; `bindingLabel` for a CC binding and a pad binding.

**Do not:** let learn override the LCXL defaults; bind from the Launchpad; bind anything in the browser preview (arming shows the notice; nothing binds).

**Done when:** [x] tests first → PASS; [x] preview: right-click a mixer knob → menu → Learn shows the notice and the blinking outline; Esc cancels; [x] full verification + build; commit `feat: MIDI learn for pads and on-screen controls`.

Task 12 verification (2026-10-19): `kit_learn.rs` (18 tests) and `learn.test.ts` were written before the code (RED: module missing). The router and state learn tests were written together with the implementation. fmt clean; `cargo test --workspace` all passed (86 lib tests); `npm test` 67 passed; `npm run check` 0 errors, 0 warnings; `npm run build` passed. Edge/Chrome preview: right-click on a mixer knob opens `Learn MIDI CC` / `Reset`, Learn shows the notice and `.learning`, Esc clears both; the pad menu `Learn MIDI note…` is enabled and arms `Hit a key or pad on any MIDI device · Esc cancels`; SETUP lists AUDIO, MIDI DEVICES, MIDI LEARN, DISPLAY with the exact empty text; 54 controls carry `data-learn`. Design notes: the router returns a `MidiOutcome` (binding to add, pad hit, parameter changes) and `AppState::on_midi` finishes it under the same router lock through `edit_in` (bindings are undoable, `bank.*` parameters coalesce under `learn:{id}`, group and master parameters are mixer changes without history). `mixer-changed` and a new `set-changed` event are emitted by the LCXL bridge thread (≤ 1 per 33 ms, trailing edge); `on_lp_event` and `on_lcxl` now return the `learn-ignored` event for the caller to emit. The `LearnTarget` type serves as the command DTO. The parameters `keys.volume`, `keys.mod`, `seq.play`, `seq.bpm` and `seq.swing` are valid ids and show in the UI, but their CC values are not applied yet: Tasks 13 and 15 add that to `apply_learned_cc`. Not verified: a real controller, hot-plug during learn, `pad-hit` against hardware. The user's uncommitted `PadEditor.svelte` CSS edit was kept out of the commit.

---

## Phase E — Sequencer and keys

### Task 13: Sequencer engine (model, timing, clock, record) — DONE

**Purpose.** UI spec 4.7 and 7.3: the pattern lives in set.json, the router runs the clock, the UI gets the position at ~30 Hz, recording follows the nearest-step rule.

**Files:**
- Create: `crates/lpd-core/src/seq.rs`, `crates/lpd-core/tests/seq.rs`, `src-tauri/src/sequencer.rs`
- Modify: `crates/lpd-core/src/lib.rs`, `crates/lpd-core/src/kit/{model,ops}.rs`, `src-tauri/src/router.rs`, `state.rs`, `commands.rs`, `lib.rs`, `launchpad.rs` (the Mixer top button also stops the sequencer), `src/lib/model.ts`

**Contracts:**
```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] #[serde(rename_all = "camelCase", default)]
pub struct Pattern { pub lanes: Vec<Lane>, pub length: u8, pub bpm: f32, pub swing: f32 }
impl Default for Pattern { fn default() -> Self { Pattern { lanes: vec![], length: 16, bpm: 100.0, swing: 50.0 } } }
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] #[serde(rename_all = "camelCase")]
pub struct Lane { pub bank: u8, pub x: u8, pub y: u8, pub mute: bool, pub steps: Vec<u8> }   // always MAX_STEPS values: 0 off, 1 on, 2 accent
pub const MAX_STEPS: usize = 32;
pub const VEL_ON: u8 = 90;
pub const VEL_ACCENT: u8 = 127;
pub const LENGTHS: [u8; 4] = [4, 8, 16, 32];
pub fn step_duration(bpm: f32) -> f64;                       // 16th notes: 60 / bpm / 4 seconds
/// Start of step i (0-based) from the loop start: i × d, plus (swing − 50) / 50 × d for odd i (swing 75 → odd steps half a step late).
pub fn step_time(i: usize, bpm: f32, swing: f32) -> f64;
pub fn loop_duration(length: usize, bpm: f32) -> f64;        // length × d
/// Record rule (UI spec 4.7). i = the last step whose time is ≤ t (t in [0, loop)). If t − step_time(i) is more than
/// half a step, the hit belongs to the next step: returns (i + 1, true), wrapping to (0, true) after the last step;
/// `true` = "already played live", the clock must not fire that step on this pass. Otherwise (i, false): step i has
/// already fired this pass, so nothing is doubled.
pub fn nearest_step(t: f64, length: usize, bpm: f32, swing: f32) -> (usize, bool);
/// Steps whose time falls in (prev, now] (seconds since loop start; now may pass the loop end): in order, wrapping.
pub fn due_steps(prev: f64, now: f64, length: usize, bpm: f32, swing: f32) -> Vec<usize>;
/// Growing: in every lane whose steps from the old length on are all 0, fill the new steps with repeats of the old
/// length; other lanes keep their steps. Shrinking only changes `length` (hidden steps are kept).
pub fn set_length(p: &mut Pattern, new_len: u8);
pub fn add_lane(p: &mut Pattern, bank: u8, x: u8, y: u8) -> Result<(), &'static str>; // Err("Lane already there")
pub fn clear_steps(p: &mut Pattern);                          // all steps 0, lanes kept
// Set gains: #[serde(default)] pub pattern: Pattern
```
- Ops: `move_pad` Move/Swap retarget lanes like learn bindings (a lane follows its sound); Copy adds no lane; `clear_pad` keeps the lane (the UI shows `EMPTY`; the clock skips it because the pad has no sample).
- Clock thread (`sequencer.rs`), shared state `AppState.seq: Arc<Mutex<SeqState>>` with `SeqState { playing: bool, rec: bool, started: Instant, last_t: f64, skip: HashSet<(usize, usize)> /* (lane, step) */, releases: Vec<(Instant, usize, u8, u8)> /* (when, bank, x, y) */ }`:
  - On start: `started = now`, `last_t = −1e-9` (so step 0 fires immediately).
  - While playing, loop: `now_t = started.elapsed()` in seconds, taken modulo the loop (fold `started` forward by whole loops); for each step in `due_steps(last_t, now_t, …)` and each unmuted lane whose step value is non-zero and `(lane, step)` is not in `skip`: `router.trigger_pad_in(bank, x, y, vel / 127)` with vel `VEL_ON` or `VEL_ACCENT`; when `bank == router.bank()` also `painter.on_pad_hit(x, y, now)`; hold-mode pads get a release scheduled at 90 % of a step. After a step has passed, remove its `skip` entries. Sleep until 1 ms before the next step time, then spin (`std::hint::spin_loop`) — D3, comment the deviation in code.
  - Emit `seq-position` `{ playing, rec, step, length, bpm, msIntoStep }` on every step and at least every 33 ms while playing; when stopped, one event with `playing: false, step: 0, msIntoStep: 0`.
  - Tempo, swing and length edits while playing apply from the next step: recompute `started` so the current step index and its progress are preserved.
- Record (`rec && playing`): every pad trigger that is **not** from the clock (screen, Launchpad, computer keys, learned notes) passes through `Router::trigger_pad_in`, which reports `(bank, x, y)` to a record hook installed by the sequencer: `t` = loop position, `(step, already) = nearest_step(t, …)`; lane = the existing lane for that pad, else a new one (`add_lane`); `steps[step] = max(steps[step], 1)` (an accent stays; recording never lowers or removes steps); if `already`, insert `(lane, step)` into `skip`. The take is one undo step: arming REC calls a new `AppState::begin_take()` that runs `history.record(&router.set().clone(), None, now)` directly (`edit` would record nothing, because the set has not changed yet); later writes go through `router.set_mut()` + `touch()`; emit `set-changed` throttled to 33 ms.
- Stop paths: `stop_all` (screen ■, Esc, Launchpad Mixer CC 111) also stops the sequencer (`playing = false`; REC stays armed).
- Commands:
  - `seq_play(play)`: start from step 0 / stop.
  - `seq_rec(armed)`: arming starts playback if stopped and records the take's history step; disarming leaves playback running.
  - `seq_update(pattern, key)`: validate (length in `LENGTHS`, bpm clamped 60–200, swing clamped 50–75, every lane exactly 32 steps with values ≤ 2, lanes point at valid bank/x/y), then `edit(key, |set| set.pattern = pattern)`; return the set. The UI sends keys `seq:bpm`, `seq:swing`, `seq:paint` for drags.
  - `seq_add_lane(bank, x, y)`: `Err("Pad is empty")` when the pad has no sample; `Err("Lane already there")`; else `edit(None, add_lane)`.
  - `seq_audition_step(bank, x, y)`: `trigger_pad_in(bank, x, y, VEL_ON / 127)` only when stopped (no-op while playing); not recorded.

**Tests (`seq.rs`):** `step_duration(120.0) == 0.125`; `step_time(1, 120, 50) == 0.125`; `step_time(1, 120, 75) == 0.1875`; `step_time(2, 120, 75) == 0.25`; `nearest_step(0.01, 16, 120, 50) == (0, false)`; `nearest_step(0.07, 16, 120, 50) == (1, true)`; `nearest_step(0.124, 16, 120, 50) == (1, true)`; `nearest_step(0.126, 16, 120, 50) == (1, false)`; `nearest_step(1.99, 16, 120, 50) == (0, true)` (loop 2.0 s, step 15 at 1.875, 0.115 s late → next pass's step 0); `due_steps(-1e-9, 0.26, 16, 120, 50) == [0, 1, 2]`; `due_steps(0.0, 0.26, …) == [1, 2]`; `due_steps(1.9, 2.01, …)` → `[0]` with the window crossing the loop end (normalise: pass `now` beyond the loop and expect wrapping); `set_length` 16→32 copies steps 0–15 into 16–31 when 16–31 are all 0 and keeps them otherwise; 32→16→32 keeps the hidden steps; `add_lane` twice → Err; an old set.json without `pattern` loads the default; ops: Move/Swap retarget lanes, clear keeps the lane.

**Do not:** run the clock in the UI; fire hidden steps (index ≥ length); remove steps while recording; record auditions or sequencer-fired hits.

**Done when:** [x] tests first → PASS; [x] full verification; commit `feat: step sequencer clock, pattern model and record`.

Task 13 verification (2026-10-19): `crates/lpd-core/tests/seq.rs` (13 tests, the plan's values) was written first; the clock, router and state tests were written with the code. fmt clean; `cargo test --workspace` all passed (109 lib tests); `npm test` 67 passed; `npm run check` 0 errors, 0 warnings. Implementation notes: the clock state (`SeqState`, in `sequencer.rs`) is created by the router and shared with `AppState.seq`; it is a leaf lock below the router (the record hook takes it while the router is held; the clock thread never holds it while taking the router). `SeqState::tick` is a pure function of `Instant`s: it returns the due steps, folds the loop reference, and on a tempo, swing or length change moves the reference so the current step and its progress are kept. `Router::trigger_pad_in` records (non-clock hits: screen, Launchpad, computer keys, learned notes, later the keys-bound pads); `trigger_pad_quiet` (clock, step audition) does not; a hit on a pad without a sample is not recorded. `Router::stop_all` stops the clock (so the screen ■, Esc, Launchpad Mixer CC 111 and a learned `stopAll` all do). The learned parameters `seq.play`, `seq.bpm` and `seq.swing` now work (`seq.play` toggles the clock; tempo and swing are `learn:{id}` edits). Recording marks the set dirty through the clock thread, which makes the LCXL bridge thread emit `set-changed` (≤ 1 per 33 ms). Added `lpd_core::seq::validate` (length in `LENGTHS`, tempo and swing clamped, 32 steps of 0..2, valid unique lanes) used by `seq_update`. Moving a pad onto an empty pad that carries a lane keeps only the moved pad's lane (one lane per pad); the plan did not say. Known limit: `seq_update` sends the whole pattern, so an edit made while REC is writing steps can overwrite steps recorded since the UI's last `set-changed`. Not verified: the clock's timing on a real machine (jitter ≤ 1 ms is a D3 target, not measured), recording against real hardware.

### Task 14: Sequencer UI, fold strips, telemetry beat lamps and orbit — DONE

**Purpose.** UI spec 4.7 (header, lanes, steps, playhead, fold), 4 (height split with folded plates) and 4.8 (beat lamps, orbit ring).

**Files:**
- Modify: `src/components/SequencerPlate.svelte` (functional; remove `unfitted` and the `NOT FITTED · PART 2` text), `src/App.svelte`, `src/lib/layout.ts` + `layout.test.ts`, `src/components/Telemetry.svelte`, `src/lib/model.ts`
- Create: `src/lib/seq.ts` + `src/lib/seq.test.ts`, `src/components/NumLcd.svelte` (mockup JS 1376–1386: LCD value; drag up/down, wheel, arrows, double-click resets; `role="slider"`, `aria-valuetext`, optional `learnId`)

**Contracts:**
```ts
// seq.ts (pure)
export type StepValue = 0 | 1 | 2;
export function clickValue(current: StepValue, accent: boolean): StepValue; // click: 0 → 1, 1 | 2 → 0; accent (right-click or Shift+click): 2 → 0, else → 2
export function paint(steps: StepValue[], index: number, value: StepValue): StepValue[]; // a copy with steps[index] = value
export function foldSummary(lanes: number, length: number, bpm: number): string;        // "5 lanes · 16 steps · 100 BPM" ("1 lane")
export function laneLabel(bank: number, sample: string | null): string;                  // (0, 'samples/kick_808.wav') → "1·KICK_808"; null → "EMPTY"
export const BPM = { min: 60, max: 200, def: 100 };
export const SWING = { min: 50, max: 75, def: 50 };
/** Same rule as Rust seq::set_length: growing fills a lane's new steps with repeats of the old length only when
 *  that lane's steps from the old length on are all 0; shrinking only changes `length`. Returns a new pattern. */
export function setLength(p: Pattern, n: 4 | 8 | 16 | 32): Pattern;
```
- `layout.ts`: `PlayLayoutInput` gains `keysFolded: boolean` and `seqFolded: boolean`. A folded plate is 28 px tall and its saved height goes back to the pads (pads still ≤ 112 px). `lanes` = the pattern's lane count (min 1). Spec figures at 1280×720: sequencer folded → pads 51; keys and sequencer folded → pads 55. Write the tests with the same bay size the existing `layout.test.ts` uses for 1280×720. If the existing solver's open-plate result already differs from the spec table, keep the same offset for the folded cases, name it in the test, and report it; do not retune the open case.

**UI (mockup markup 533, CSS 246–275 + 399–407, JS 1353–1451):**
- Header (22 px; left block 223 px): PLAY/STOP `.key.round` (▶; green ■ while playing; `aria-pressed`; `learnId="seq.play"`), REC `.key.round` ● (red when armed; `aria-pressed`), `BPM` `NumLcd` 60–200 (default 100, `learnId="seq.bpm"`), `SWING` `NumLcd` 50–75 shown as `50%` (default 50, `learnId="seq.swing"`); `STEPS` `.switch` 4 | 8 | 16 | 32 (`role="radiogroup"`; applies `setLength` from `seq.ts`, then `seq_update` with no key); `+ LANE` (selected pad → `seq_add_lane`; errors as error confirmations; success `Lane added`); `CLEAR` (all steps 0 via `seq_update` with no key; undoable); spacer; fold ▾.
- Folded: a 28 px strip `▸ SEQUENCER  {foldSummary}`; clicking it or ▸ unfolds.
- Lanes: colour bar (`.sw`, pad colour via `ledToCss`), LCD `laneLabel` (bright `--lcd` when the lane's pad is the selected pad; `EMPTY` in dim italics when the pad is gone); clicking the LCD = `chooseBank(lane.bank)` + select the pad; `M` key (amber `.on` when muted; `seq_update`), `×` key (remove the lane; `seq_update`).
- Steps: `length` cells sharing the width, 2 px gaps, every 4th cell `.beat` (lighter). On = pad colour at 48 % over ink with a 1 px outline in the pad colour; accent = full colour + glow (mockup classes). Click → `clickValue(v, e.shiftKey)`; right-click → `clickValue(v, true)`; press + drag paints the value chosen on the first cell across the cells under the pointer (pointer capture on the lane body, hit-test cells by x); one `seq_update` per changed cell with key `seq:paint` (one stroke = one undo step). A step switched on while stopped → `seq_audition_step`.
- Playhead: `.st.ph` (orange outline + glow; lit cells brighten) on column `step` from `seq-position`.
- Lane heights from `--lh` (14–24 px); when the lanes need more height than the body has, the body scrolls (`overflow-y: auto`).
- All edits are optimistic: update local `currentSet.pattern`, call the command, replace with the returned set.
- `App.svelte`: listen to `seq-position` → `seqPos` state. Fold state `seqFolded` in localStorage `lpd.seq.folded`.
- Telemetry: beat lamps = quarter notes of the running sequencer (`Math.floor(step / 4) % 4`), downbeat lamp cream, others orange, all dark when stopped. Orbit outer ring: one tick per step of `length` (beat ticks longer; ticks of steps where any lane has a hit in amber); playhead planet (cream dot, orange tail) at angle `(step + msIntoStep / stepMs) / length × 2π` from the top, interpolated per frame from the last event; stopped: planet parked at the top, readout `IDLE · {bpm} BPM`; playing: `STEP {step + 1}/{length} · {bpm} BPM`.
- Preview: the plate edits a local pattern (starts empty; `+ LANE` adds the selected pad when it has a sample); PLAY does nothing.

**Tests:** `seq.test.ts`: `clickValue` table (0→1, 1→0, 2→0; accent 0→2, 1→2, 2→0); `paint` does not mutate its input; `foldSummary(1, 8, 120) == "1 lane · 8 steps · 120 BPM"`; `laneLabel(0, 'samples/kick_808.wav') == "1·KICK_808"`, `laneLabel(2, null) == "EMPTY"`; `setLength` 16→32 copies steps 0–15 into 16–31 for an empty tail and keeps a non-empty tail, 32→16→32 keeps hidden steps, input not mutated. `layout.test.ts`: the two fold cases.

**Do not:** keep the `unfitted` class or the `NOT FITTED · PART 2` text; run a UI timer to advance steps; send a command per pixel of mouse movement (only when the cell changes).

**Done when:** [x] tests first → PASS; [x] preview at 1280×720: add lanes, paint steps and accents, fold/unfold, 32 steps; [x] full verification + build; commit `feat(ui): working step sequencer plate`.

Task 14 verification (2026-10-19): `seq.test.ts` (9 tests) was RED (module missing), then GREEN; the layout fold tests were written with the solver change. fmt clean; `cargo test --workspace` all passed; `npm test` 80 passed; `npm run check` 0 errors, 0 warnings; `npm run build` passed. Chrome preview: the plate shows no `NOT FITTED` text; the selected pad's lane is added (`Lane added`); click, right-click (accent) and a drag across cells paint `x...A...xxx.....`; clicking again clears; STEPS 32 repeats the 16 into an empty tail; fold gives the 28 px strip `▸ SEQUENCER 1 lane · 32 steps · 100 BPM`, stores `lpd.seq.folded` and gives the height back to the pads (68 px vs 64 px at this window); no console errors. **Layout deviation (reported as the plan asks):** the spec table says 1280×720 pads 39 open, 51 with the sequencer folded, 55 with both folded; in the existing test bay (1176×628) the open solver gives 37 (a pre-existing offset of 2, not retuned), the folded cases give 48 and 51: the folded gain is 11 and 14 px against the spec's 12 and 16. The solver follows the mockup's `layoutPad` formula exactly (`rest − 28`), so the difference comes from the spec's own measurement. The keys plate gets `keysFolded` in the solver now (Task 16 wires the UI). Notes: edits are optimistic through `updatePattern` (a returned set is applied only if no newer edit is in flight; an error re-reads the set); the playhead planet is interpolated from the last `seq-position` event and redrawn at ~30 fps while playing (~4 with reduced motion); the clock thread also emits a position when REC changes while stopped. `laneLabel(2, null)` is `EMPTY` (the mockup's `3·EMPTY` is not used). Not verified: the plate against the real clock and recording in the desktop app; playhead smoothness.

### Task 15: Keys instrument engine (built-in synth + SoundFont) — DONE

**Purpose.** UI spec 4.6 (engine part) and 7.3 (keys); engine spec 3.1 `instrument`; decisions D1 and D5.

**Files:**
- Create: `crates/lpd-core/src/instrument/mod.rs`, `basic.rs`, `soundfont.rs`, `crates/lpd-core/tests/instrument_basic.rs`
- Modify: `Cargo.toml` (workspace dep `rustysynth = "1"`), `crates/lpd-core/Cargo.toml`, `crates/lpd-core/src/lib.rs`, `crates/lpd-core/src/audio/engine.rs`, `crates/lpd-core/tests/no_alloc.rs`, `crates/lpd-core/tests/audio_engine.rs`, `crates/lpd-core/src/kit/model.rs`, `src-tauri/src/{router,state,commands,lib,audio}.rs`

**Contracts:**
```rust
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)] #[serde(rename_all = "camelCase")]
pub enum KeysSound { Piano, #[default] EPiano, Organ, SynthPad, SynthLead, SynthBass }
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)] #[serde(rename_all = "camelCase", default)]
pub struct KeysSettings { pub sound: KeysSound, pub volume_db: f32, pub output: Option<u8> }   // default EPiano, −6.0, None (= master)
// Set gains: #[serde(default)] pub keys: KeysSettings
pub trait Instrument: Send {
    fn note_on(&mut self, note: u8, velocity: u8);
    fn note_off(&mut self, note: u8);
    fn pitch_bend(&mut self, semitones: f32);    // −2..2, applies to held notes live
    fn modulation(&mut self, depth: f32);        // 0..1 → vibrato depth up to ±0.5 st at 5.5 Hz
    fn set_sound(&mut self, sound: KeysSound);
    fn all_notes_off(&mut self);                 // fast 5 ms release
    /// Adds into l/r (same length ≤ MAX_BLOCK). Never allocates.
    fn render_add(&mut self, l: &mut [f32], r: &mut [f32]);
}
pub const KEYS_VOICES: usize = 16;
pub struct BasicKeys { /* 16 voices, preallocated */ }
impl BasicKeys { pub fn new(sample_rate: u32) -> Self; pub fn active_voices(&self) -> usize; }
pub struct SoundFontKeys { /* rustysynth::Synthesizer + two scratch buffers of MAX_BLOCK */ }
impl SoundFontKeys { pub fn load(path: &Path, sample_rate: u32) -> anyhow::Result<Self>; }
pub fn gm_program(sound: KeysSound) -> u8;       // Piano 0, EPiano 4, Organ 16, SynthPad 89, SynthLead 81, SynthBass 38
```
`BasicKeys` voice recipes (per voice: phase accumulators and an ADSR; velocity gain = (vel/127)²; output × 0.25 headroom; the same signal on both sides):

| Sound | Oscillator | Envelope A / D / S / R | Notes |
|---|---|---|---|
| Piano | sines at 1×, 2×, 3× (amps 1, 0.5, 0.25) | 2 ms / 1.2 s to 0 / 0 / 250 ms | decays while held |
| EPiano | 2-op FM: carrier sine, modulator at 1× with index 1.5 decaying to 0.2 over 400 ms | 2 ms / 1.5 s / 0.3 / 400 ms | |
| Organ | sines 1×, 2×, 3×, 4× (amps 1, 0.7, 0.4, 0.25) | 5 ms / 0 / 1 / 5 ms | stops at once on note off |
| SynthPad | 3 saws detuned −7, 0, +7 cents (PolyBLEP) | 300 ms / 0 / 1 / 1.2 s | rings on |
| SynthLead | square (PolyBLEP) + 0.3 × saw | 5 ms / 200 ms / 0.7 / 150 ms | |
| SynthBass | saw one octave below the note through a one-pole low-pass at 800 Hz | 3 ms / 300 ms / 0.6 / 80 ms | |

Voice stealing: when all 16 voices are busy, the oldest one is reused (its envelope restarts from its current level). Re-hitting a held note reuses that note's voice.
- `SoundFontKeys`: `SoundFont::new(&mut File)` + `Synthesizer::new(&Arc<SoundFont>, &SynthesizerSettings::new(rate as i32))` with `maximum_polyphony = 32`. On load and on `set_sound`: program change `gm_program` on channel 0 (`process_midi_message(0, 0xC0, program, 0)`). Pitch bend range ±2 via RPN 0 (CC 101 = 0, CC 100 = 0, CC 6 = 2, CC 38 = 0). `pitch_bend(st)` → `0xE0` with value `8192 + st / 2 × 8191`; `modulation(d)` → CC 1 = `d × 127`; `render_add` renders into the scratch buffers (`synth.render(&mut sl[..n], &mut sr[..n])`) and adds them.
- Engine: commands `KeysNoteOn { note: u8, velocity: u8 }`, `KeysNoteOff { note: u8 }`, `KeysPitch(f32)`, `KeysMod(f32)`, `KeysSound(KeysSound)`, `KeysAllOff`, `KeysRoute { gain: f32, group: Option<u8> }`, `SetInstrument(Box<dyn Instrument>)`. `Engine` holds `instrument: Box<dyn Instrument>` (default `BasicKeys::new(rate)` in `new` and `with_taps`), `keys_gain` (default `db_to_gain(−6.0)`), `keys_group: Option<u8>`, and `trash: Producer<Box<dyn Instrument>>` (capacity 4) whose `Consumer` the host takes once with `Engine::take_trash(&mut self) -> Option<Consumer<Box<dyn Instrument>>>`. `SetInstrument(new)`: `let mut old = mem::replace(&mut self.instrument, new); old.all_notes_off();` then push `old` into the trash (queue full → `mem::forget(old)` rather than freeing it on the audio thread). Render: the instrument renders into `keys_l`/`keys_r` (preallocated `MAX_BLOCK`), scaled by `keys_gain`. `keys_group = Some(g)` → added into `groups[g].buf_l/buf_r` **before** group processing (the channel's low-pass, pan, gain, mute, solo and level tap apply; D5: the group pitch does not); `None` → added to the master sum after the groups and before master gain and clip, like auditions.
- Router: `on_midi` for role `keys`: a note not bound to a pad (Task 12 check) → `KeysNoteOn`/`KeysNoteOff` + the `keys-note` event; PitchBend → `KeysPitch(value / 8192 × 2)`; CC 1 → `KeysMod(value / 127)` (and `keys-wheel`). `send_keys()` pushes `KeysSound(set.keys.sound)` + `KeysRoute { gain: db_to_gain(set.keys.volume_db), group: set.keys.output }`; it is called from `set_current_set`, `install_set`, `replace_audio` (the Task 9 call site) and after `set_keys`. The router drains the trash in `poll_audio_events` (the drop happens on the router thread).
- SoundFont lifecycle (`state.rs`): `set_soundfont(path)` (the Task 2 command, now real) saves the config. `None` → `SetInstrument(BasicKeys::new(rate))`, status `{ engine: "builtin", name: "BUILT-IN", error: null }`. `Some(p)` → a worker thread runs `SoundFontKeys::load(p, rate)` → `SetInstrument`, status `{ engine: "soundfont", name: <file stem uppercased>, error: null }`; a load error keeps the current instrument, status `{ engine: "builtin", name: "BUILT-IN", error: "Couldn't load {file}: {err}" }`. Emit `keys-status`. At startup, when `config.soundfont` is set, load it the same way after audio starts; after an audio restart (Task 9) set the instrument again (reload the `.sf2` for the new sample rate).
- Commands: `keys_note(note, velocity, on)` (screen keys; also emits `keys-note` so the mirror shows it), `keys_pitch(value)` (−1..1 → ±2 st), `keys_mod(value)` (0..1), `set_keys(settings)` → `edit(Some("keys"), |set| set.keys = settings)` then `send_keys()`, returns the set; `keys_status()`.

**Tests (`instrument_basic.rs`; render with a helper that calls `render_add` into zeroed buffers at 48 kHz):**
- Every sound: note 60, velocity 100 → RMS of the first 50 ms > 0.01.
- Piano held for 2 s → RMS of the last 50 ms < 10 % of the first 50 ms.
- Organ: 5 ms after note off, RMS < 1 % of the held level.
- SynthPad: 200 ms after note off, still > 10 % of the held level.
- 17 note-ons (60..76) → `active_voices() <= 16`, and note 60's voice was stolen.
- Pitch bend +2 st on a held Organ note raises the zero-crossing rate by 12.2 % ± 1 %.
- Velocity 127 is louder than velocity 30.
- `audio_engine.rs`: keys routed to group 2 with group 2 muted → silent; routed to the master with all groups muted → audible; `KeysRoute` gain 0 → silent; `SetInstrument` puts the old box into the trash consumer.
- `no_alloc.rs`: `KeysNoteOn`, `KeysPitch`, 4 rendered blocks with `BasicKeys`, and `SetInstrument` (box created outside the guard) — no allocation inside the guard.
- `SoundFontKeys::load("does-not-exist.sf2", 48000)` → Err. No network, no `.sf2` fixtures.
- Router unit: a `keys` NoteOn not bound to a pad → `KeysNoteOn` in the audio queue; bound to a pad → a Trigger instead.

**Do not:** allocate or drop boxes on the audio thread; bundle or download a SoundFont; route keys through pads; apply the group pitch to the keys.

**Done when:** [x] tests first → PASS; [x] full verification; commit `feat(core): keys instrument with built-in synth and SoundFont support`.

Task 15 verification (2026-10-19): `instrument_basic.rs` (15 tests) was written first (RED: module missing); the engine, `no_alloc`, router and state tests were written with the code. fmt clean; `cargo test --workspace` all passed (119 lib tests); no compiler warnings. `no_alloc.rs` gained `keys_paths_never_allocate_or_free` (note on/off, pitch, mod, sound, route, all-off, `SetInstrument`, 4+ rendered blocks) and `a_full_trash_queue_leaks_instead_of_freeing`. Implementation notes: `BasicKeys` follows the plan's recipe table; envelopes are linear for attack and release and exponential for decay (the decay time is the time to reach 1 % of the way to the sustain level); the synth pad uses saws of amplitude 0.9 so that its slow attack still clears the plan's RMS > 0.01 in the first 50 ms. `BasicKeys::is_sounding(note)` exists for the stealing test. `Engine::take_trash` hands the host the consumer of replaced instruments; the router drains it in `drain_trash`, which `AppState::maintain` calls (the box is dropped after the router lock is released, not inside `poll_audio_events` as the plan wrote). `AudioSystem` carries the trash consumer; the audio owner thread passes it to the router (`install_trash`) before `install_audio`. The `// keys: Task 15` marker in `replace_audio` now calls `send_keys` and re-sends the wheel positions; `Router::new`, `replace_set` and `install_set` send the keys settings too (`replace_set` needs 2 more free slots). A reload of the configured `.sf2` runs in a background thread after every stream start (`install_audio`), and a notifier emits `keys-status`. A SoundFont load error keeps the current instrument and reports `Couldn't load {file}: {error}`; the path stays in the config (CLEAR removes it). Learned parameters: `keys.volume` is a set edit (`learn:keys.volume`) followed by `send_keys`, `keys.mod` moves the mod wheel directly. The logfile test now logs only lines from its own thread (parallel tests that log used to be able to break it). Not verified: a real `.sf2` (none is bundled, none is downloaded), `SoundFontKeys` on the audio thread (rustysynth's allocation behaviour is not asserted), the built-in sounds by ear, the SoundFont reload after a device change.

### Task 16: Keys plate UI, mixer channel inputs, SETUP › KEYS — DONE

**Purpose.** UI spec 4.6 (plate), 6 "Channel inputs", and the SETUP part of D1.

**Files:**
- Modify: `src/components/KeysPlate.svelte` (functional; remove `unfitted`, `aria-hidden` on interactive parts, and the `NOT FITTED` / `PART 2` texts), `src/components/MixerStrip.svelte`, `src/App.svelte`, `src/lib/model.ts`, `src-tauri/src/commands.rs` + `state.rs` + `lib.rs` (thin `trigger_pad_in` / `release_pad_in` commands)
- Create: `src/lib/keys.ts` + `src/lib/keys.test.ts`, `src/components/Wheel.svelte`, `src/components/SetupKeys.svelte`

**Contracts:**
```ts
// keys.ts
export const SOUNDS = ['piano', 'ePiano', 'organ', 'synthPad', 'synthLead', 'synthBass'] as const;
export type KeysSound = typeof SOUNDS[number];
export const SOUND_LABELS: Record<KeysSound, string> = { piano: 'PIANO', ePiano: 'E.PIANO', organ: 'ORGAN', synthPad: 'SYNTH PAD', synthLead: 'SYNTH LEAD', synthBass: 'SYNTH BASS' };
export const SOUND_COLORS: Record<KeysSound, [number, number, number]> = { piano: [63,63,63], ePiano: [63,40,0], organ: [63,20,0], synthPad: [25,0,63], synthLead: [0,63,25], synthBass: [0,30,63] };
export function noteName(n: number): string;               // 60 → "C4", 61 → "C#4", 21 → "A0", 108 → "C8"
export function velocityFromY(y: number, height: number): number; // y from the key top: top → 30, bottom → 127, linear, rounded, clamped
export function windowRange(octave: number): [number, number];     // [36 + 12·oct, 84 + 12·oct], octave clamped to −1…+2
export function rangeLabel(octave: number): string;                 // 0 → "C2–C6", −1 → "C1–C5", 2 → "C4–C8"
export function outputLabel(output: number | null, groups: Group[]): string; // null → "→ MASTER"; 7 → "→ 8 VOX" (index + 1 and the group name, max 8 characters)
```
- Commands `trigger_pad_in(bank, x, y)` / `release_pad_in(bank, x, y)`: `AppState` calls `Router::trigger_pad_in(bank, x, y, 1.0)` (+ the painter hit when `bank` is the active bank) / `Router::release_pad_in`.

**Keys plate (mockup markup 532, CSS 214–245 + 389–398, JS 1226–1352):**
- Keybed: 88 keys A0–C8; white keys share the width (52), black keys 60 % of a white key wide and 62 % tall (exists); `C n` label on every C (exists). The lit 49-key window is `windowRange(octave)`, marked by the bright orange bar in the felt strip (exists; now moves with `octave`); keys outside are dimmed but playable.
- Mouse: `pointerdown` on a key → `keys_note({ note, velocity: velocityFromY(...), on: true })`; sliding onto another key while held (pointer capture on the keybed; hit-test by x and the black-key area) → note off the old one, note on the new one (glissando); `pointerup`/`pointercancel` → note off. A key bound to a pad (`noteBoundTo(set, note)` from Task 12) plays the pad instead: `trigger_pad_in({ bank, x, y })` on press and `release_pad_in({ bank, x, y })` on release, and selects the pad when `bank === activeBank`; it does not call `keys_note`.
- Pressed keys light in `SOUND_COLORS[sound]` (bound keys: in the pad colour). Keys bound to a pad carry a small dot in the pad colour near the bottom. Hardware notes from `keys-note` light the same way.
- Controls (176 px column):
  - Row 1: status lamp (green when a `keys` device is connected); sound LCD in the sound colour (`SOUND_LABELS`; click / Enter / Space opens a `ContextMenu` with the six sounds, the current one ticked → `set_keys`); volume `NumLcd` (−40…+6 dB, default −6, shows `−6`; `learnId="keys.volume"`); fold `▾` (Ctrl+K).
  - Row 2: octave `−` / `+` keys with the range LCD (`rangeLabel`); output LCD `outputLabel` (click = menu `MASTER` + the 8 channels by `groupLegend`, current ticked → `set_keys`).
  - Row 3: `LAST` silk + LCD `{noteName} · VEL {v}` of the last note (screen or hardware); hidden when the keybed is under 68 px (`short`, exists).
- Wheels (`Wheel.svelte`, mockup JS 1269–1281): PITCH is spring-loaded (drag up/down; on release it returns to centre with a 120 ms ease; arrow keys nudge it and it springs back on keyup; sends `keys_pitch({ value })` −1..1 at most every 16 ms); MOD stays where it is put (`keys_mod({ value })` 0..1, `learnId="keys.mod"`). Both `role="slider"` with `aria-valuetext` (`+1.2 st`, `MOD 40 %`). Hardware `keys-wheel` moves both thumbs.
- Fold: a 28 px strip `▸ KEYS ● KEYSTATION 49` (lamp inline); `keysFolded` in localStorage `lpd.keys.folded`; feeds `solvePlayLayout`.
- `keysAbsent` notice input = no connected `keys` device AND the plate not folded AND not the preview.
- No computer-keyboard playing of the keys.

**Mixer channel inputs (UI spec 6; mockup JS 1494–1511):**
- A single click or a right-click on a channel's scribble strip opens a `ContextMenu`: info row `Pads: {n} in group {k} (pad editor › MIX GROUP)` (`n` = pads in all banks with `mixGroup === i`, `k = i + 1`; `info: true`); checkbox row `Keys: {SOUND_LABEL}` (`checked = keys.output === i`; when the keys go to another channel the label is `Keys: on {groupLegend(other)}`) → `set_keys({ settings: { ...keys, output: checked ? null : i } })`; separator; `Rename…` with shortcut `F2` (the existing inline rename). Double-click still renames.
- A channel carrying the keys shows a dot in the sound colour before its name; the scribble's `title` lists its inputs (`Pads: 6 · Keys: E.PIANO`).

**SETUP › KEYS (`SetupKeys.svelte`, rack after MIDI DEVICES):** row `ENGINE` LCD (`BUILT-IN SYNTH` or `SOUNDFONT · {name}`); row `SOUNDFONT` LCD with the path (`middleTruncate`; `NONE`) + `LOAD…` (`pick_file({ title: 'Choose a SoundFont', extensions: ['sf2'] })` → `set_soundfont({ path })`) + `CLEAR` (`set_soundfont({ path: null })`); an error line in `--error` when `keysStatus.error` is set; hint `The built-in synth plays the six sounds; a General MIDI SoundFont (.sf2) replaces it.` Listen to `keys-status`; load `keys_status` on mount.

**Tests:** `keys.test.ts`: `noteName` (21, 60, 61, 108); `velocityFromY(0, 100) == 30`, `(100, 100) == 127`, `(50, 100) == 79`; `windowRange(0) == [36, 84]`; `windowRange(3)` clamps to `[60, 108]`; `rangeLabel(-1) == "C1–C5"`; `outputLabel(null, g) == "→ MASTER"`; `outputLabel(7, groups with name 'VOX') == "→ 8 VOX"`. `notices.test.ts`: the `keysAbsent` row text and prio 14.

**Do not:** play the keys from the computer keyboard; restyle the keybed; leave `aria-hidden` on controls that are now interactive.

**Done when:** [x] tests first → PASS; [x] preview: keys light on click and glissando, menus open, fold works with Ctrl+K and gives the height back to the pads; [x] full verification + build; commit `feat(ui): playable keys plate, mixer channel inputs and setup keys rack`.

Task 16 verification (2026-10-19): `keys.test.ts` (6 tests) was RED (module missing), then GREEN; the notices test for the Keystation row was added. fmt clean; `cargo test --workspace` all passed; `npm test` 87 passed; `npm run check` 0 errors, 0 warnings; `npm run build` passed. Headless Edge via CDP at 1280×720 (the Chrome extension disconnected during this task, so the preview checks were scripted): the plate has no `NOT FITTED` text and shows 88 keys; a press lights key 60 (`LAST C4 · VEL 117`), sliding onto 62 releases 60 and lights 62 (glissando), release lights none; the sound menu lists the six sounds with the current one ticked, choosing ORGAN shows `ORGAN`; the output menu lists MASTER and the eight channels, choosing channel 8 shows `→ 8 VOX`; octave + gives `C3–C7`; clicking the channel 8 scribble opens `Pads: 1 in group 8 (pad editor › MIX GROUP)`, `✓ Keys: ORGAN`, `Rename…  F2`, and the channel shows the keys dot; Ctrl+K folds to `▸ KEYS  KEYSTATION 49 MK3`, stores `lpd.keys.folded`, and gives the pads 37 → 40 px; unfolding restores the plate; SETUP lists AUDIO, MIDI DEVICES, KEYS, MIDI LEARN, DISPLAY with the KEYS rack texts of the plan; no console errors. Notes: the keys plate plays unbound keys through `keys_note` (the engine's `keys-note` event lights the keys on every mirror) and keys bound to a pad (`noteBoundTo`) through `trigger_pad_in` / `release_pad_in`, selecting the pad when its bank is active; `Wheel.svelte` sends at most one value per 16 ms (the last one always) and the pitch wheel springs back in 120 ms (instantly with reduced motion); `keysAbsent` is `tauri && no connected keys device && plate not folded`. The mixer channel menu and the learn menu share one `ContextMenu` instance in `App.svelte`. Not verified: the plate with a real Keystation or SoundFont, the sound of the six built-in voices, the wheels against hardware.

---

## Phase F — Finish

### Task 17: SETUP › DISPLAY bend, SETUP › LOG, rack polish — DONE

**Files:** `src/components/SetupDisplay.svelte`, `src/components/CrtLayers.svelte`, `src/lib/display.ts` + `src/lib/display.test.ts`, `src/components/SetupLog.svelte`, `src/App.svelte`

- `display.ts`: `export interface DisplayPrefs { level: CrtLevel; bend: 'flat' | 'bent' }`; `readDisplay(storage): DisplayPrefs` reads the existing key `launchpad-drummer.display.crt` (the old string format stays valid) and a new key `launchpad-drummer.display.bend`; `saveDisplay(storage, prefs)`. Update the callers.
- DISPLAY rack: `CRT EFFECTS` three-position switch OFF | LOW | MEDIUM (exists) and `BEND` two-position switch FLAT | BENT (default FLAT; `aria-disabled` and dimmed while CRT is OFF). Hint text exactly (replaces the two current lines): `Glow, curved pixel mask, grain, vignette and a scan line that now and then sweeps down (sometimes stalling mid-screen). BENT also bulges the picture itself (text near the edges gets softer). Reduced motion keeps the mask and grain still.`
- BENT (UI spec 2.5 "Real bend"): port the mockup's SVG filter (`#crt-bend` / `#bend-map` markup 557–558, JS `bendSize` / `setBend` 1921–1923): a radial displacement map, 4.5 px at the edge middles, 9 px in the corners, + 0.4 px blur, applied to `.console` with `filter: url(#crt-bend)` only when `data-bend="on"` and CRT is not OFF; a resize regenerates the map.
- LOG rack (`SetupLog.svelte`): `OPEN LOG FOLDER` key → `open_log_folder`; hint `Rotating log: lpd.log + 3 older files, 1 MB each.`
- Check that every rack uses `RackUnit` (registration marks + orange pill): AUDIO, MIDI DEVICES, KEYS, MIDI LEARN, DISPLAY, LOG; single column, max 860 px, scrolls.

**Tests:** `display.test.ts`: storage with only `medium` → `{ level: 'medium', bend: 'flat' }`; round-trip `{ level: 'low', bend: 'bent' }`; garbage → `{ level: 'low', bend: 'flat' }`; throwing storage → defaults.

**Done when:** [x] tests first → PASS; [x] preview SETUP: the BEND switch toggles the bulge and is inactive at OFF; [x] full verification + build; commit `feat(ui): display bend option and log rack`.

Task 17 verification (2026-10-19): `display.test.ts` (4 tests) was written first and failed against the old signature, then passed. fmt clean; `cargo test --workspace` all passed; `npm test` 89 passed; `npm run check` 0 errors, 0 warnings; `npm run build` passed. Headless Edge via CDP at 1280×720: SETUP lists AUDIO, MIDI DEVICES, KEYS, MIDI LEARN, DISPLAY, LOG (every rack is a `RackUnit`; the column is 860 px wide and scrolls); the DISPLAY hint is the plan's exact text; BEND → BENT sets `data-bend="on"`, the console gets `filter: url(#crt-bend) …`, the displacement map is the generated 512 px PNG sized to the console (1256 × 696), both preferences are stored (`low`, `bent`); at CRT OFF the bend is off, the BEND fieldset is dimmed, and clicking FLAT does nothing (the radio does not flip); back at LOW the bend returns; a screenshot shows the edges bulging. The old `launchpad-drummer.display.crt` string stays valid. The OPEN LOG FOLDER key calls `open_log_folder` (not exercised here: no Tauri). Not verified: the bend on a real monitor (softness of text near the edges), the scan-line behaviour (unchanged, already shipped).

### Task 18: Final verification and spec status

- [ ] `cargo fmt --all -- --check`, `cargo test --workspace`, `npm test`, `npm run check`, `npm run build` all green; paste the summary lines in the report.
- [ ] Browser preview at 1280×720, 1440×900 and 1920×1080 (drawer docked and floating; keys and sequencer open and folded; SETUP; the power-on screen cannot appear in the preview — skip it): no console errors, no overlap, no horizontal scroll.
- [ ] `cargo build -p launchpad-drummer`; start `npm run tauri dev` once; confirm the window opens, the power-on screen (no config) or the last set shows, `lpd.log` gets the startup line, and nothing panics.
- [ ] Update `docs/superpowers/specs/2026-10-17-ui-design.md` §13 and `2026-10-17-drum-sampler-design.md` §15: what shipped in plan 1C; decisions D1–D7 as recorded deviations; open items: the user's hardware checklist (engine spec 10.3: hot-plug, bank switching, light modes, LCXL controls + LEDs, every drag-and-drop path incl. a real Explorer drop, MIDI learn with a keyboard, a 30-minute session with zero dropouts, the ASIO latency check).
- [ ] Commit `docs: plan 1C status and remaining hardware checks`.

---

## Self-review (spec coverage)

| Spec item | Task |
|---|---|
| UI 1 undo/redo 50 steps, kit edits only | 1, 3 |
| UI 1 MIDI LEARN list, New/Open set UI, couldn't-save notice | 12, 3, 2 + 3 |
| UI 3 set LCD menu, undo keys, notice LCD, chips; status lamps, audio LCD, DROPS, path, save LED | 3, 8, 9, 10 |
| UI 4 drawer browser/editor (docked/floating exists), auto open only on SHOW | 5, 3 |
| UI 4.1 pad menu, missing `!`, empty `+`, Stop all (exists), scribble rename (exists) | 7 |
| UI 4.2 humanize applied | 1 |
| UI 4.3 missing box + LOCATE, LEARN, empty text, editor drop target, coalesced drags | 7, 12, 6 |
| UI 4.4 browser | 5 |
| UI 4.5 right-click Learn / Reset | 12 |
| UI 4.6 keys plate + engine | 15, 16 |
| UI 4.7 sequencer | 13, 14 |
| UI 4.8 telemetry feed, beat lamps, orbit playhead, folded strip | 8, 14 |
| UI 5 drag and drop | 4, 6 |
| UI 6 channel inputs, level fills, LCXL updates | 16, 8, 11 |
| UI 7.1 notices 1–13 (+ Keystation info row) | 3 (inputs from 2, 8, 9, 10, 11, 12, 16) |
| UI 7.2 power-on | 2, 3 |
| UI 7.3 backend requirements | 1, 2, 5, 8, 13, 15 |
| UI 8 SETUP racks | 9, 10, 12, 16, 17 |
| UI 8.1 learn interaction | 12 |
| UI 10 keyboard | 3 (+ 5 Ctrl+B, 16 Ctrl+K) |
| UI 11 accessibility | 3, 7, 12, 14, 16 |
| Engine 4.1 autosave 1 s | 2 |
| Engine 7 other MIDI devices + learn, LCXL mapping/LEDs | 10, 11, 12 |
| Engine 9 error handling (port busy, audio lost, ASIO fallback, dropouts, missing, unreadable set, log file) | 2, 8, 9, 10 |
| Engine 3.1 `instrument` trait | 15 |
