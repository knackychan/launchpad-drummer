# Plan 1C: Complete the Part 1 Console — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build everything the UI spec (`../specs/2026-10-17-ui-design.md`) and the engine spec (`../specs/2026-10-17-drum-sampler-design.md`) still describe but the app does not do yet: persistence, notices, undo, editing by drag and drop, browser, telemetry feed, audio and MIDI setup, Launch Control XL, MIDI learn, sequencer and keys.

**Architecture:** The router (Rust) owns the one `Set`; every kit edit goes through one `AppState::edit` path that records undo, marks the set dirty for autosave and returns the new set. Pure logic (history, kit ops, LCXL codec, sequencer timing, FFT, synth) lives in `lpd-core` with integration tests; `src-tauri` wires threads, devices and Tauri commands; the Svelte UI only edits and draws. Hardware pad hits never pass through the UI.

**Tech Stack:** Rust (lpd-core, Tauri 2, cpal 0.15, midir 0.10, rtrb, symphonia, rustysynth, tauri-plugin-dialog, tauri-plugin-opener), Svelte 5 + TypeScript, `node --test` for `src/lib/*.test.ts`.

**Format note:** this plan follows plan 1B's format (task-level contracts, not full code listings): the scope is ~18 tasks over three layers. Each task fixes the interfaces (Rust signatures, Tauri command names and camelCase args, event names and payloads, TS types) and the tests to write; later tasks rely on those names exactly. The implementer writes the code, TDD, inside those contracts.

---

## Conventions (every task)

- Shell: PowerShell on Windows. Repo root `D:\MY DOCS\MY TOOLS\LAUNCHBAND\launchpad-drummer`, branch `ui-sci-retro-skin`.
- Prepend cargo: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"`.
- **Tauri v2 arg naming:** Rust command args are snake_case; JS passes **only** camelCase keys (`{ groupData }`, `{ humanizeDb }`), never both.
- **Never call `trigger_pad` for hardware-sourced events** (`padInput` with `source: 'hardware'`); the router already played them.
- **Do not restyle working parts.** Look comes from `src/styles/app.css` + `tokens.css` and the mockup `docs/mockups/ui-mockup-scifi.html`; reuse its classes (`.plate`, `.screen`, `.lcd`, `.lcd-dim`, `.silk`, `.key`, `.key.on`, `.key.small`, `.key.round`, `.switch`, `.lamp`, `.rack-unit`). New UI parts copy the mockup's markup and CSS for that part. Remove the `unfitted` class only when a plate becomes functional.
- Browser preview (no `window.__TAURI__`) must keep working: every new feature has a harmless no-op or demo path there (`invoke` already resolves `undefined`).
- Rust: `cargo fmt` before every commit; no allocation added to the audio callback (`crates/lpd-core/tests/no_alloc.rs` covers the engine; extend it when the engine changes).
- Verification before each commit (all must pass):
  ```
  cargo test --workspace
  npm test
  npm run check
  ```
  and `npm run build` at the end of every UI task.
- Commit per task: `git add <files>; git commit -m "<type>(<scope>): <summary>"` with a blank line and `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` at the end.
- Keep the existing tests green; extend `src-tauri` unit tests with the fake-queue pattern already used in `router.rs` / `lights.rs`.

## File map (new files)

| File | Responsibility |
|---|---|
| `crates/lpd-core/src/kit/history.rs` | Undo/redo stack of `Set` snapshots (pure) |
| `crates/lpd-core/src/kit/ops.rs` | Pure set edits: drop files onto pads, move / swap / copy pads, clear, learn bindings follow |
| `crates/lpd-core/src/kit/learn.rs` | Learn binding model + param target ids + CC→value mapping |
| `crates/lpd-core/src/seq.rs` | Pattern model helpers, step timing with swing, nearest-step for record, grow rule |
| `crates/lpd-core/src/devices/lcxl.rs` | Launch Control XL bytes ↔ events, LED SysEx |
| `crates/lpd-core/src/audio/taps.rs` | Lock-free master/channel taps for telemetry |
| `crates/lpd-core/src/audio/fft.rs` | Radix-2 FFT + 28 log bands |
| `crates/lpd-core/src/instrument/{mod,basic,soundfont}.rs` | `Instrument` trait, built-in keys synth, rustysynth wrapper |
| `src-tauri/src/config.rs` | App config file (last set, library folder, sf2, audio prefs, MIDI roles) |
| `src-tauri/src/saver.rs` | Autosave thread + save status |
| `src-tauri/src/logfile.rs` | Rotating log file |
| `src-tauri/src/telemetry.rs` | 60 Hz telemetry frame builder/emitter |
| `src-tauri/src/devices.rs` | MIDI device registry (all ports, roles, status) + generic MIDI inputs |
| `src-tauri/src/lcxl.rs` | LCXL bridge thread |
| `src-tauri/src/sequencer.rs` | Sequencer clock thread |
| `src/lib/notices.ts` | Notice priority resolver (pure) |
| `src/lib/keymap.ts` | Computer-keyboard map (pure) |
| `src/lib/dnd.ts` | Drag hit-testing + drag state helpers (pure) |
| `src/lib/api.ts` | Typed `invoke` / `listen` wrappers + preview fallbacks (moves the inline `invoke` out of App.svelte) |
| `src/components/NoticeLcd.svelte`, `PowerOn.svelte`, `SetMenu.svelte`, `ContextMenu.svelte`, `SampleBrowser.svelte`, `SetupAudio.svelte`, `SetupMidi.svelte`, `SetupLearn.svelte`, `SetupLog.svelte` | UI parts |

---

## Phase A — Foundation

### Task 1: One set owner, edit pipeline, undo history, humanize

Today `AppState` keeps two copies of the set (router + `KitManager.set`), edits are not undoable and `humanizeDb` is stored but never applied.

**Files:**
- Create: `crates/lpd-core/src/kit/history.rs`, test `crates/lpd-core/tests/kit_history.rs`
- Modify: `crates/lpd-core/src/kit/mod.rs` (export), `src-tauri/src/store.rs` (drop `KitManager.set`), `src-tauri/src/state.rs`, `src-tauri/src/router.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`

**Contracts:**
```rust
// lpd-core kit::history
pub const UNDO_STEPS: usize = 50;
pub struct History { /* undo: VecDeque<Set>, redo: Vec<Set>, last_key: Option<String>, last_at: Option<Instant> */ }
impl History {
    pub fn new() -> Self;
    /// Call BEFORE applying an edit with the set as it was. Same `key` within 1 s of the
    /// previous record = coalesced (no new step; e.g. one knob drag). `None` never coalesces.
    pub fn record(&mut self, before: &Set, key: Option<&str>, now: Instant);
    pub fn undo(&mut self, current: &Set) -> Option<Set>;
    pub fn redo(&mut self, current: &Set) -> Option<Set>;
    pub fn can_undo(&self) -> bool; pub fn can_redo(&self) -> bool;
    pub fn clear(&mut self);
}
/// Mixer values are not covered by undo (UI spec 7.3): the restored set keeps `current`'s
/// group volume/pan/pitch/lowpass/mute/solo and master, but takes group *names* from `snapshot`.
pub fn restore_keeping_mixer(snapshot: Set, current: &Set) -> Set;
```
- `record` drops the redo stack; caps at 50 (oldest dropped).
- `AppState::edit<R>(&self, key: Option<&str>, f: impl FnOnce(&mut Set) -> R) -> R`: locks router, records history, applies `f` to `router.set_mut()`, re-sends mixer commands only if the mixer changed, marks dirty (Task 2 hooks `saver.touch()`; in this task a no-op `fn touch(&self)` on AppState), emits nothing. All existing edit commands (`update_pad`, `remove_pad`, `update_bank_lights`, `rename_bank`, `update_bank_humanize`, group *rename*) go through it. Group value changes (`update_group` without a name change) and `update_master_db` do **not** record history but still `touch()`.
- `KitManager` keeps `dir`, `samples`, `failed_samples` only; `save` takes `&Set` (`km.save(router.set())`).
- Commands: `undo() -> Option<Set>`, `redo() -> Option<Set>`, `history_state() -> HistoryState { canUndo, canRedo }` (serde camelCase). `load_set_dir` / `new_set_dir` clear history.
- Humanize: `Router::trigger_pad_in(bank, x, y, velocity_gain: f32)` is the one trigger path (`trigger_pad(x,y)` = current bank, gain 1.0). It multiplies `params.gain` by `velocity_gain × db_to_gain(u × humanize_db/2)`, `u` uniform in [−1, 1] from a xorshift32 field on the router (seeded from time; tests seed via `Router::set_seed`).

**Tests (write first):**
- `kit_history.rs`: record+undo returns the earlier set; redo returns the later; new record clears redo; 51 records keep 50; same key within 1 s coalesces, after 1 s does not, `None` never coalesces; `restore_keeping_mixer` keeps current volumes but snapshot names.
- `router.rs` unit: with `humanize_db = 6` and a fixed seed, ten triggers produce gains inside `[db_to_gain(-3), db_to_gain(3)]` and not all equal; `humanize_db = 0` gives exactly the pad gain.
- `state.rs` unit (construct `AppState` with `RingBuffer` queues as `lib.rs` does): `edit` then `undo` restores the pad; `update_group` volume change is not undoable.

- [ ] Write failing tests → run `cargo test -p lpd-core --test kit_history` (FAIL: missing module) → implement → PASS
- [ ] Router humanize test → implement → PASS; state tests → refactor `AppState`/`KitManager` → PASS
- [ ] Register `undo`, `redo`, `history_state` in `lib.rs`; full verification; commit `feat(core): undo history, single set owner and humanize`

### Task 2: App config, startup, autosave, save status, log file, dialogs

**Files:**
- Create: `src-tauri/src/config.rs`, `src-tauri/src/saver.rs`, `src-tauri/src/logfile.rs`
- Modify: `src-tauri/Cargo.toml` (+ `tauri-plugin-dialog = "2"`, `tauri-plugin-opener = "2"`, `dirs` not needed: use `app.path()`), `src-tauri/src/lib.rs`, `state.rs`, `commands.rs`, `store.rs`, `src-tauri/capabilities/default.json` (add `dialog:default`, `opener:default` only if JS calls them — the plan calls them from Rust, so `core:default` stays enough)

**Contracts:**
```rust
// config.rs — stored at app.path().app_config_dir()/config.json, atomic write (tmp + rename)
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub last_set: Option<String>,
    pub library_dir: Option<String>,
    pub soundfont: Option<String>,
    pub audio: AudioPrefs,           // { backend: "wasapi"|"asio", device: Option<String>, bufferFrames: Option<u32> }
    pub midi_roles: BTreeMap<String, String>, // port name → "keys" | "other"
}
pub fn load(path: &Path) -> AppConfig;          // missing/broken → default
pub fn save(path: &Path, c: &AppConfig) -> Result<()>;

// Startup decision (UI spec 7.2), computed once in setup():
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Startup {
    Loaded { path: String, name: String, from_backup: bool, backup_saved_at: Option<String> /* "hh:mm" of set.json.bak mtime */ },
    FirstRun,
    NotFound { path: String },
    Unreadable { path: String, name: String },
}
```
- Startup: `config.last_set` → `KitManager::load_from_dir`; ok → `Loaded`; dir missing → `NotFound`; both files unreadable → `Unreadable` (never overwrite: `kit::load` already renames nothing — verify and keep it so). No last set → `FirstRun`. While not `Loaded` the router keeps an empty set with **no** dir, and autosave is off.
- Saver thread (`saver.rs`): `Saver::touch()` sets `deadline = now + 1 s`, state `pending`; thread ticks every 100 ms; on deadline saves `router.set().clone()` via `km.save`; ok → state `saved`; err → state `failed` with reason and retries every 2 s. Emits `save-status` `{ state: "pending"|"saved"|"failed", reason: Option<String> }` on each change. `save_set_dir` command saves immediately (Ctrl+S not in spec; keep the existing command for the SAVE key, which the UI drops in Task 3).
- Missing samples: `missing_samples() -> Vec<MissingPad { bank, x, y, sample }>` from `km.failed_samples` × set pads.
- Commands (all registered in `lib.rs`):
  - `get_startup() -> Startup`
  - `new_set(dir: String) -> Result<Set,String>` / `open_set(dir: String) -> Result<OpenResult,String>` where `OpenResult { set, path, name, fromBackup, backupSavedAt }`; both update `config.last_set`, clear history, restart autosave. Keep `load_set_dir`/`new_set_dir` as aliases calling the same code until the UI no longer uses them, then delete them in Task 3.
  - `pick_folder(title: String, default_dir: Option<String>) -> Option<String>` and `pick_file(title: String, extensions: Vec<String>) -> Option<String>` via `tauri_plugin_dialog::DialogExt` blocking pickers, run inside `tauri::async_runtime::spawn_blocking` (make these `async` commands).
  - `reveal_path(path: String)` via `tauri_plugin_opener::OpenerExt::reveal_item_in_dir`; relative paths resolve against the set dir.
  - `default_sets_dir() -> String` = `Documents\Launchpad Drummer` (`app.path().document_dir()`).
  - `open_log_folder()`.
  - `get_config() -> AppConfig`, `set_library_dir(dir: Option<String>)`, `set_soundfont(path: Option<String>)` (Task 15 uses it).
- `logfile.rs`: `log(line)` appends `[yyyy-mm-dd hh:mm:ss] line` to `app_log_dir/lpd.log`; rotates at 1 MB to `lpd.1.log`…`lpd.3.log`. Log: startup outcome, failed samples, save errors, device status changes. Replace `eprintln!` in `lib.rs`, `launchpad.rs`, `audio.rs` with it (keep `eprintln!` too in debug builds).

**Tests:** `config.rs` unit: default on missing file, round-trip, broken JSON → default. `saver.rs` unit with a closure-based save function (inject `Fn(&Set) -> Result<()>`): touch then 1.1 s later saves once; two touches 0.5 s apart save once; failure → `failed` then retry succeeds → `saved`. Use an injectable clock (`fn tick(&mut self, now: Instant)`) instead of sleeping. `logfile.rs`: rotation at a tiny limit in a temp dir. Startup decision as a pure fn `decide_startup(last: Option<&Path>, load: impl Fn(&Path)->Result<LoadedKit>) -> Startup` tested for the four cases (use `tempfile` + `kit::save`).

- [ ] Tests first for config, saver, logfile, decide_startup → implement → PASS
- [ ] Wire plugins (`.plugin(tauri_plugin_dialog::init()).plugin(tauri_plugin_opener::init())`), commands, startup in `setup()`; full verification; commit `feat(tauri): app config, startup, autosave, log file and pickers`

### Task 3: Shell UI — notices, set menu, power-on, undo/redo keys, status strip, keyboard

**Files:**
- Create: `src/lib/api.ts`, `src/lib/notices.ts` (+ `notices.test.ts`), `src/lib/keymap.ts` (+ `keymap.test.ts`), `src/components/NoticeLcd.svelte`, `src/components/SetMenu.svelte`, `src/components/PowerOn.svelte`
- Modify: `src/App.svelte`, `src/tauri.d.ts`, `src/lib/model.ts` (types `Startup`, `OpenResult`, `SaveStatus`, `HistoryState`, `MissingPad`)

**Contracts:**
```ts
// notices.ts — UI spec 7.1 table. Pure.
export type Level = 'error' | 'warn' | 'info' | 'ok';
export interface Notice { id: string; prio: number; level: Level; text: string; action?: { label: string; id: string }; sticky?: boolean }
export interface NoticeInputs {
  learnArmed: null | 'cc' | 'note'; audioLost: boolean; backup: null | { savedAt: string | null }; saveFailed: string | null;
  launchpad: 'connected' | 'disconnected' | 'busy' | 'bootloader'; lcxl: 'connected' | 'disconnected' | 'busy' | 'absent';
  missing: number; asioFallback: boolean; dropoutRecent: boolean; confirmations: { id: string; text: string }[];
  dismissed: ReadonlySet<string>;
}
export function activeNotices(i: NoticeInputs): Notice[]; // sorted by prio (1 = highest), dismissed removed
```
Texts exactly as the spec table (rows 1–13). Actions: `RECONNECT` (audio), `DISMISS` (backup, asio), `HOW TO FIX` (bootloader popover with steps: 1 run Novation's `launchpad-mk2-updater.exe`, 2 unplug, 3 replug), `SHOW` (missing), `SETUP` (dropout). Confirmations last 3 s (App owns the timers: `confirm(text)` helper).
- `NoticeLcd.svelte`: one slot showing `notices[0]` with icon (✖ ⚠ • ✓) coloured by level, its action key, and `+n` key opening a popover listing all; `aria-live` polite, `assertive` when the top one is an error. Empty → the existing idle text.
- `SetMenu.svelte`: the set LCD becomes a menu button `SET ▸ {name} ▾`: New set… (Ctrl+N), Open set… (Ctrl+O), Show in Explorer. New: `pick_folder('New set', default_sets_dir)` → `new_set`. Open: `pick_folder` → `open_set` → backup notice if `fromBackup`.
- `PowerOn.svelte` (mockup `#poweron`): centre bay when startup is not `Loaded` and no set is open: title, status line (`Last set not found: {path}` / `Couldn't open {name} — set.json and backup are unreadable. Files left untouched.` / first-run text), lamps LAUNCHPAD / LCXL / AUDIO, keys NEW SET and OPEN SET. PLAY shows it; SETUP still works; pads and mixer hidden. Browser preview: never shown (preview set is open).
- Undo/redo keys: lit when `history_state` says so; click / Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z call `undo`/`redo`, replace `currentSet` with the result, show confirmation `✓ Undone` / `✓ Redone`. Refresh `history_state` after every edit call (debounced 100 ms).
- Status strip: drop the SAVE key; add save LED (dim = pending, green blink 600 ms on `saved`, red on `failed`), `DROPS n` LCD (0 until Task 8), set path middle-truncated (`D:\Sets\…\My Kit`, helper `middleTruncate(path, max)` in `notices.ts` or a small `format.ts`, tested), LCXL lamp from the device registry (until Task 10: off).
- `keymap.ts` (UI spec 10): `padForKey(key: string): { x: number; y: number } | null` — rows `1–8` → y=3, `Q–I` → y=2, `A–K` → y=1, `Z–,` → y=0 (x = position). Tested for every key incl. `,`.
- `App.svelte` keyboard handler (one `keydown` + `keyup`), never while typing in inputs, Play screen only for pad keys: letters/digits play+select (`source: 'screen'`, release on keyup), Space plays selected pad, arrows move selection (grid focused or nothing focused), PageUp/PageDown bank −/+, Alt+1–8 bank, F2 rename active bank (focus in mixer → that group), Delete clear selected pad (undoable), Ctrl+E drawer editor, Ctrl+B drawer browser (Task 5 adds the panel; until then no-op), Ctrl+K fold keys (Task 16), Ctrl+1 PLAY, Ctrl+, SETUP, Ctrl+N/O, Ctrl+Z/Y, Esc chain: drag → learn → rename → menu/popover → full-screen game → STOP ALL. Expose the chain as a small stack (`escStack.push(fn)` / pop) so components register cancel handlers.
- `api.ts`: `invoke`, `listen` wrappers moved from `App.svelte`, typed per command; preview fallbacks (`get_startup` → `{kind:'loaded', name:'PREVIEW KIT', …}`).

**Tests:** `notices.test.ts`: priority order for a mix of inputs (learn beats audio lost beats backup …); dismissed backup disappears; LCXL busy text; missing count pluralisation (`1 sample missing` / `3 samples missing`); confirmation ordering. `keymap.test.ts` as above.

- [ ] Tests first (notices, keymap, middleTruncate) → implement → PASS
- [ ] Components + App wiring; remove `promptSetManagement` / `window.prompt` / `alert`; delete the `load_set_dir`/`new_set_dir` aliases from Rust
- [ ] Preview check: `npm run dev`, open `http://127.0.0.1:1420`, Play renders, notice LCD shows the preview text, undo keys unlit, no console errors
- [ ] Full verification + `npm run build`; commit `feat(ui): notices, set menu, power-on, undo keys and keyboard map`

---

## Phase B — Editing

### Task 4: Kit operations backend (drops, move / swap / copy, clear, locate)

**Files:**
- Create: `crates/lpd-core/src/kit/ops.rs`, test `crates/lpd-core/tests/kit_ops.rs`
- Modify: `crates/lpd-core/src/kit/mod.rs`, `crates/lpd-core/src/kit/model.rs` (no `learn` field yet: Task 12 adds it and extends these ops so bindings follow moved pads), `src-tauri/src/state.rs`, `commands.rs`, `lib.rs`

**Contracts:**
```rust
pub const AUDIO_EXTENSIONS: [&str; 6] = ["wav", "aif", "aiff", "flac", "mp3", "ogg"];
pub fn is_audio_path(p: &Path) -> bool;
/// Target pad, then the next EMPTY pads in reading order: right along the row, then the row above
/// (y + 1), from x = 0. Stops at the top-right pad. Returns positions for `n` files (≤ n).
pub fn drop_positions(bank: &Bank, x: u8, y: u8, n: usize) -> Vec<(u8, u8)>;
pub fn row_default_color(y: u8) -> [u8; 3];     // presets 1,2,4,6,8,10,12,14 (1-based) of the 16 swatches, row 0 = bottom
/// Filled pad: replace sample, reset trim, keep the rest. Empty pad: Pad::new with row colour, mix = row.
pub fn assign_sample(bank: &mut Bank, x: u8, y: u8, sample: String);
pub enum PadMove { Move, Swap, Copy }
/// from/to may be in different banks. Empty target + no Ctrl = Move; filled target + no Ctrl = Swap;
/// Ctrl = Copy (overwrites). Returns the kind applied, or None when from is empty or from == to.
pub fn move_pad(set: &mut Set, from: (usize, u8, u8), to: (usize, u8, u8), copy: bool) -> Option<PadMove>;
pub fn clear_pad(set: &mut Set, bank: usize, x: u8, y: u8) -> bool;
```
- `AppState::drop_files(bank, x, y, paths: Vec<String>) -> Result<DropReport, String>`: rejects non-audio paths (count them), imports each via `km.import_sample_file` (collision → `-2`, `-3`, already in `kit::import_sample`), assigns via `drop_positions` / `assign_sample` (first file onto the target replaces if filled; following files only onto empty pads), inserts decoded samples into router, **one** history record (key `None`), `touch()`. `DropReport { set: Set, imported: usize, leftover: usize, rejected: usize }` (serde camelCase).
- Commands: `drop_files(bank: usize, x: u8, y: u8, paths: Vec<String>) -> Result<DropReport,String>`, `move_pad(from_bank: usize, from_x: u8, from_y: u8, to_bank: usize, to_x: u8, to_y: u8, copy: bool) -> Option<Set>` (JS: `{ fromBank, fromX, fromY, toBank, toX, toY, copy }`), `clear_pad(bank, x, y) -> Set` (replaces `remove_pad`; update the UI call), `locate_sample(bank, x, y, path) -> Result<Set,String>` (import the picked file and replace the pad's sample keeping trim — LOCATE is a re-import of the same sound).
- Hold voices on drag start: UI calls `release_pad` itself (Task 6); nothing here.

**Tests (`kit_ops.rs`):** reading-order positions across a row end and around filled pads; bank full → fewer positions; row default colours for y=0 and y=7; assign to filled keeps colour/volume/pitch/mode/choke/mix and resets trim; move to empty; swap; copy overwrites and leaves the source; cross-bank move; from empty → None; `is_audio_path` case-insensitive.

- [ ] Tests first → implement ops → PASS; AppState methods + state unit test (drop of 3 files onto a bank with 1 free pad after the target → imported 2, leftover 1; use `tempfile` set dir and tiny WAVs written with `hound`)
- [ ] Commands registered; full verification; commit `feat(core): pad drop, move, swap, copy and clear operations`

### Task 5: Sample browser and audition

**Files:**
- Create: `src/components/SampleBrowser.svelte`, `src/lib/tree.ts` (+ `tree.test.ts`)
- Modify: `crates/lpd-core/src/audio/engine.rs` (+ audition bus), `crates/lpd-core/tests/audio_engine.rs`, `crates/lpd-core/tests/no_alloc.rs`, `src-tauri/src/router.rs`, `state.rs`, `commands.rs`, `lib.rs`, `src/App.svelte`

**Contracts:**
- Engine: `Command::Audition { sample: Arc<Sample> }` plays the whole sample once at unity, centred, **summed into the master before the master gain/clipper, bypassing groups**; a new audition stops the previous (3 ms fade); `Command::StopAudition`. Uses one dedicated voice slot outside the 32-voice pool; pad id `AUDITION_PAD = u16::MAX` (no `VoiceStarted`/`VoiceEnded` events for it, so lights never react).
- Router keeps the last auditioned `Arc<Sample>` (so the audio thread never frees it) and replaces it on the next audition.
- Commands: `list_dir(path: String) -> Result<Vec<DirEntry>, String>` (`DirEntry { name, path, isDir }`, folders first then files, case-insensitive name order, only audio files by `is_audio_path`, hidden/system entries skipped); `audition_file(path: String) -> Result<(), String>` (decode on a worker via `std::thread::spawn`, then push); `stop_audition()`; `load_into_pad(bank, x, y, path) -> Result<DropReport,String>` = `drop_files` with one path.
- Library folder: `get_config().libraryDir`; FOLDER… key → `pick_folder` → `set_library_dir`.
- `tree.ts` (pure): `interface Node { name; path; isDir; depth; open?: boolean; children?: Node[] }`; `flatten(root: Node[]): Node[]` (visible rows), `keyNav(rows, activeIndex, key): { index, action?: 'open'|'close'|'audition'|'toggle'|'load' }` implementing ↑ ↓ → ← Space Enter (UI spec 4.4).
- `SampleBrowser.svelte` (mockup browser markup): header × key, library path LCD, FOLDER… key; `role="tree"` list, 22 px rows, mono 12 px, ♪ marker on the auditioned file; click file = audition; folders lazy-load children via `list_dir`; keyboard per `keyNav`; Enter = `load_into_pad` on the selected pad (notice if none / result counts); rows are pointer-drag sources for Task 6 (expose `ondragstartfile(path, event)` prop).
- Drawer: BROWSER tab live (Ctrl+B), one panel at a time with the editor (UI spec 4 side drawer). Preview: a fake tree (`samples/` names from the preview set).

**Tests:** engine: audition output non-zero with all groups muted (proves bypass), a second audition fades the first (no step > threshold, reuse the click test helper), no events emitted; `no_alloc` covers `Command::Audition` handling + render. `tree.test.ts`: flatten with open/closed folders, keyNav for each key at edges.

- [ ] Engine tests first → implement → PASS; tree tests → PASS
- [ ] Commands + component + drawer tab; preview check (browser opens, keyboard moves, Enter shows notice); full verification + build; commit `feat: sample browser with audition`

### Task 6: Drag and drop

**Files:**
- Create: `src/lib/dnd.ts` (+ `dnd.test.ts`)
- Modify: `src/components/PadGrid.svelte`, `LaunchpadPanel.svelte`, `SampleBrowser.svelte`, `PadEditor.svelte`, `src/App.svelte`, `src/styles/app.css` (ghost + dashed bezel + tag classes, copied from the mockup's drag styles)

**Contracts (UI spec 5):**
```ts
export interface Rect { left: number; top: number; width: number; height: number }
export type DropTarget = { kind: 'pad'; x: number; y: number } | { kind: 'bank'; bank: number } | { kind: 'editor' } | null;
export function hitTest(px: number, py: number, pads: { x: number; y: number; rect: Rect }[], banks: { bank: number; rect: Rect }[], editor: Rect | null): DropTarget;
export function dropTag(source: 'files' | 'pad', target: DropTarget, targetFilled: boolean, ctrl: boolean, count: number): 'IMPORT' | `IMPORT ×${number}` | 'REPLACE' | 'MOVE' | 'SWAP' | 'COPY' | null;
export const DRAG_THRESHOLD = 4; export const BANK_HOVER_MS = 400;
export function physicalToCss(p: { x: number; y: number }, dpr: number): { x: number; y: number };
```
- Pointer-based drags only (no HTML5 DnD). Pad drag starts after 4 px movement **after** the press sounded; on start call `release_pad` for hold voices; ghost = small pad in source colour + name following the pointer; target gets dashed white bezel + LCD tag (`dropTag`); Ctrl at drop = copy; drop calls `move_pad`; Esc or drop outside = cancel. Browser row drag: same ghost (file name), drop calls `drop_files` with one path; onto the editor = selected pad.
- Hovering a side button / scribble strip 400 ms switches bank (`chooseBank`), drag continues.
- Explorer drops: `listen('tauri://drag-over', { position })`, `listen('tauri://drag-drop', { paths, position })`, `listen('tauri://drag-leave')` (Tauri 2 window events; `position` is physical px → `physicalToCss`). Non-audio (by extension, `isAudioName` in `dnd.ts` mirroring `AUDIO_EXTENSIONS`) → red bezel on hovered pad + notice `Not an audio file`. Several files → `drop_files`; report `n files not imported — bank full` when `leftover > 0`; confirmation `✓ Imported n`.
- Pad rects come from `PadGrid` (`bind:this` + `getBoundingClientRect` on drag start, refreshed on bank switch).
- Every drop = one undo step (backend does it); after a drop replace `currentSet` with the returned set and refresh history state.

**Tests (`dnd.test.ts`):** hitTest on pad/bank/editor/outside; dropTag table for all rows of UI spec 5 incl. `IMPORT ×3`; physicalToCss at dpr 1.5.

- [ ] Tests first → implement → PASS
- [ ] Wire pad drags, browser drags, Explorer drops, bank hover; preview check: drag a pad onto an empty pad in the preview set (preview applies the move locally using a TS port of `move_pad` rules from `dnd.ts`? **No** — keep it simple: preview shows ghost and tag, and drops are no-ops there)
- [ ] Full verification + build; commit `feat(ui): drag and drop for pads, browser files and Explorer files`

### Task 7: Pad editor completion, pad context menu, grid accessibility

**Files:**
- Create: `src/components/ContextMenu.svelte` (generic: items `{ label, shortcut?, disabled?, checked?, action }`, opens at pointer or at the focused control for Shift+F10 / Menu key, Esc closes, arrow keys move)
- Modify: `src/components/PadEditor.svelte`, `PadGrid.svelte`, `LaunchpadPanel.svelte`, `src/App.svelte`

**Contracts (UI spec 4.1, 4.3, 11):**
- Pad editor: replace the free-text SAMPLE path input with the spec layout: header `R4·C2`, name 16 px, `0.45 s · 44.1 kHz`; **missing sample box** (red `Sample missing: samples/…` + LOCATE… → `pick_file('Locate sample', AUDIO_EXTENSIONS)` → `locate_sample`) when the pad's sample is in `missing_samples()`; empty-pad text exactly: "Empty pad. Drop a sample here, drag one from the browser, or press Enter on a file in the browser."; editor is a drop target (Task 6). Trim: min 10 ms apart (already), clicks elsewhere audition (already). Knob drags send `update_pad` with history key `pad:{bank}:{x}:{y}:{field}` so a drag coalesces — add an optional `key` argument to the `update_pad` command (`update_pad(bank, pad, key: Option<String>)`; JS `{ bank, pad, key }`).
- MIDI section: LEARN key (Task 12 makes it live; here render it with `aria-disabled` until Task 12) and the binding LCD placeholder hidden.
- Pad grid: `role="grid"`, one Tab stop, arrows move selection, `aria-activedescendant`, pad labels per spec 11 ("Row 4, column 2, KICK_808, mix 4, choke off" / "…, empty" / "…, sample missing"); missing pad shows `!` glyph (steady with reduced motion); empty pad faint `+` on hover; selection bezel (exists).
- Right-click pad (or Shift+F10 / Menu): Learn MIDI note… (Task 12; disabled until then), Clear pad (`clear_pad`), Show sample in Explorer (`reveal_path`).
- Delete key / CLEAR key use `clear_pad` (undoable, confirmation `✓ Pad cleared`).
- Hardware pad hit selects the pad (exists; keep).

**Tests:** extend `model.test.ts` with `padAriaLabel(pad | null, x, y, groups, missing)` (pure helper in `model.ts`).

- [ ] Test first → helper → PASS; components; preview check (right-click menu opens/closes with Esc, arrows move selection); full verification + build; commit `feat(ui): pad editor missing-sample box, pad menu and grid keyboard`

---

## Phase C — Audio and telemetry

### Task 8: Engine taps, FFT and the telemetry stream

**Files:**
- Create: `crates/lpd-core/src/audio/taps.rs`, `crates/lpd-core/src/audio/fft.rs`, tests `crates/lpd-core/tests/audio_taps.rs`, `crates/lpd-core/tests/audio_fft.rs`, `src-tauri/src/telemetry.rs`
- Modify: `crates/lpd-core/src/audio/{engine,mod}.rs`, `no_alloc.rs`, `src-tauri/src/audio.rs`, `state.rs`, `lib.rs`, `commands.rs`, `src/components/Telemetry.svelte`, `src/lib/activity.ts`, `src/App.svelte`

**Contracts:**
```rust
// taps.rs
pub const SCOPE_CAPACITY: usize = 8192;
#[derive(Clone, Copy, Debug, Default)]
pub struct Levels { pub master: [f32; 2], pub groups: [f32; 8] } // peak |x| since the last block, post-fader
pub struct TapsTx { /* Producer<f32> mono, Producer<Levels> */ }
pub struct Taps { pub scope: Consumer<f32>, pub levels: Consumer<Levels> }
// engine.rs
impl Engine { pub fn with_taps(sample_rate: u32) -> (Engine, Consumer<Event>, Taps); } // `new` keeps its signature (no taps)
```
- After the master stage, each block pushes the mono sum `(l+r)/2` of every frame into the scope queue (drop on full — `push` error ignored) and one `Levels` (master L/R peak, per-group post-fader/pan peak, 0 for muted/solo-silenced groups). No allocation.
```rust
// fft.rs
pub struct Fft2048; // precomputed twiddles + Hann window, allocated in new()
impl Fft2048 { pub fn new() -> Self; pub fn magnitudes(&mut self, input: &[f32; 2048]) -> &[f32; 1024]; }
pub const BANDS: usize = 28;
/// 28 log-spaced bands 40 Hz–16 kHz: max magnitude per band in dBFS (−100 floor); plus the loudest bin's frequency.
pub fn bands(mags: &[f32; 1024], sample_rate: u32) -> ([f32; BANDS], f32);
/// Index of the first rising zero crossing in the first half of `buf`, so a 1024 window fits after it.
pub fn trigger_index(buf: &[f32]) -> usize;
```
- `telemetry.rs`: a 60 Hz thread (spawned in `setup`) drains `Taps` into a 2048 ring, keeps max levels since the last frame, and — only while `AppState.telemetry_on` (AtomicBool) — emits `telemetry`:
  `{ scope: number[1024] (from trigger_index, rounded to 1e-3), bands: number[28] (dBFS), peakHz: number, master: [l, r] (dBFS, −100 floor), groups: number[8] (linear peak 0..1) }`.
  Command `set_telemetry(on: bool)`; the UI turns it on while the Play screen is shown **and** the window is visible (`document.visibilitychange`).
- Dropouts (engine spec 9; UI notice 9): in `audio.rs`'s callback, measure the interval between callbacks with `Instant`; an interval > 1.8 × the buffer duration counts as a dropout (`AtomicU32` count + `AtomicU64` last-dropout ms since start). `get_audio_status` gains `dropouts: u32` and `lastDropoutAgoMs: Option<u64>`; the telemetry thread emits `audio-status` whenever the count changes. Command `reset_dropouts()`.
- `audio.rs` uses `Engine::with_taps`; `AudioSystem` gains `taps: Taps`.
- UI `Telemetry.svelte`: replace the STANDBY stand-ins with the real feed (scope trace + 3 afterglow traces, none with reduced motion; spectrum segments with falling bars ~12 %/frame and slow cream peak caps; `PEAK f` readout; L/R 36-segment meters −48…+3, falling 36 dB/s, 1.2 s hold, `PK x dB`); silent (master < −90 dBFS for 0.5 s) → flat trace + `STANDBY · PLAY A PAD`. Keep the raster/bit-crush drawing code; only the data source changes. Session clock `T+mm:ss.t`. The folded 30 px strip draws vertical L/R meters.
- `activity.ts`: `setLevels(groups: number[])` feeds `energy[]` from the real per-channel peaks when a feed is present (hit-based estimate stays for the browser preview). Mixer `.f-fill` and MASTER L/R fills use the same feed (`--m` on the fader's own scale via `dbToPos`).

**Tests:** `audio_fft.rs`: a 1 kHz sine at 48 kHz → loudest band contains 1 kHz and `peakHz` within ±25 Hz; silence → all bands −100; `trigger_index` on a sine finds a rising crossing. `audio_taps.rs`: render a sample through group 3 → `levels.groups[3] > 0`, others 0; muted group → 0; scope receives `frames` values. `no_alloc.rs`: `with_taps` engine render does not allocate.

- [ ] Rust tests first → implement taps/fft → PASS; telemetry thread + dropout counter (unit-test the dropout rule as a pure fn `is_dropout(interval, buffer_frames, rate)`)
- [ ] UI feed; preview keeps STANDBY; full verification + build; commit `feat: telemetry stream from engine taps, FFT and dropout counter`

### Task 9: Audio controller and SETUP › AUDIO

**Files:**
- Create: `src/components/SetupAudio.svelte`
- Modify: `src-tauri/src/audio.rs`, `src-tauri/src/lib.rs`, `state.rs`, `router.rs`, `commands.rs`, `config.rs` (AudioPrefs), `src/components/SetupDisplay.svelte` → split: keep DISPLAY there; `src/App.svelte` setup screen becomes rack list AUDIO, MIDI DEVICES, MIDI LEARN, DISPLAY, LOG (UI spec 8)

**Contracts:**
- `cpal::Stream` is not `Send`: an **audio owner thread** holds the stream and receives `AudioRequest::{Start(AudioPrefs), Stop}` over `std::sync::mpsc`; replies with `Result<AudioStatus>`. On start it builds a new `Engine::with_taps`, hands the new `Producer<Command>` / `Consumer<Event>` to the router (`Router::replace_audio(tx, rx)`: re-sends mixer + keys settings, clears voice counts) and the new `Taps` to the telemetry thread (`Mutex<Option<Taps>>` swap).
- Stream error callback (device lost) → status `stopped` + event `audio-status` with `state: "lost"`; notice 2 "Audio device lost" with RECONNECT → `audio_reconnect()` (restart with current prefs; live only when stopped).
- ASIO: when prefs say `asio` but `asio_host()` fails, start WASAPI and set `asioFallback: true` in status (notice 8, DISMISS).
- Commands: `audio_devices(backend: String) -> Vec<AudioDevice { name, isDefault, bufferSizes: Vec<u32> }>` (buffer sizes = powers of two 32…2048 inside the device's `SupportedBufferSize::Range`, or `[]` for Unknown = "auto" only), `asio_available() -> AsioInfo { available: bool, reason: Option<String> }` (not built / no driver), `set_audio(prefs: AudioPrefs) -> Result<AudioStatus,String>` (saves prefs to config), `audio_reconnect() -> Result<AudioStatus,String>`.
- `AudioStatus` gains `backend`, `latency: { midiToBufferMs, outputMs, totalMs }` (midi→buffer = buffer / rate; output from `OutputCallbackInfo::timestamp().playback - callback`, stored as an AtomicU32 µs), `asioFallback`.
- `SetupAudio.svelte` (mockup rack AUDIO): BACKEND switch ASIO | WASAPI (ASIO disabled with reason), DEVICE LCD select, BUFFER LCD select, LATENCY LCD `MIDI→BUFFER x · OUTPUT y · TOTAL z ms`, DROPOUTS counter + RESET + hint "Raise buffer size" when > 0, STREAM lamp + state + RECONNECT.
- Status strip AUDIO LCD shows `backend · buffer · latency` (`WASAPI · 256 · 5.3 ms`); AUDIO lamp amber while a dropout happened in the last 10 s.

**Tests:** pure helpers unit-tested in `audio.rs`: `buffer_choices(min, max) -> Vec<u32>`; `latency_ms(buffer, rate)`. Router `replace_audio` test: after replace, commands go to the new queue and mixer commands were re-sent.

- [ ] Tests first → implement owner thread + commands → PASS
- [ ] Setup racks; preview shows the racks with demo data; full verification + build; commit `feat: audio device selection, reconnect and setup audio rack`

---

## Phase D — MIDI

### Task 10: Device registry, generic MIDI inputs, SETUP › MIDI DEVICES

**Files:**
- Create: `src-tauri/src/devices.rs`, `src/components/SetupMidi.svelte`
- Modify: `src-tauri/src/launchpad.rs` (report into the registry), `state.rs`, `router.rs`, `lib.rs`, `commands.rs`, `config.rs`, `src/App.svelte`, `src/components/SetupDisplay.svelte` (move the MIDI event log into SetupMidi; keep it — memory: the event log is shared with the game prototypes' diagnostics)

**Contracts:**
```rust
#[serde(rename_all = "camelCase")]
pub struct MidiDevice { pub port: String, pub role: String /* "launchpad"|"lcxl"|"keys"|"other" */, pub status: String /* "connected"|"disconnected"|"busy"|"bootloader" */ }
pub fn classify(port: &str, roles: &BTreeMap<String, String>) -> &'static str; // Launchpad MK2 prefix → launchpad; "Launch Control XL" → lcxl; config role or "Keystation" in the name → keys; else other
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MidiMsg { NoteOn { ch: u8, note: u8, vel: u8 }, NoteOff { ch: u8, note: u8 }, Cc { ch: u8, cc: u8, value: u8 }, PitchBend { ch: u8, value: i16 /* −8192..8191 */ } }
pub fn parse(bytes: &[u8]) -> Option<MidiMsg>; // Note On vel 0 = NoteOff
```
- A 2 s poll thread opens every input whose role is `keys` or `other` (Launchpad and LCXL ports belong to their own bridges), forwards parsed messages to `AppState::on_midi(port, role, msg)` (router queue via the existing router mutex; keep it short), and keeps the registry (`Arc<Mutex<Vec<MidiDevice>>>`) of **all** ports incl. Launchpad/LCXL status reported by their bridges. Emits `midi-devices` (full list) on change. Busy = connect error.
- Router `on_midi`: in this task only `keys` role notes are forwarded as `keys-note` UI events `{ note, velocity, on }` (the synth comes in Task 15) and pitch bend / CC 1 as `keys-wheel` `{ pitch: −1..1, mod: 0..1 }`; learn hooks come in Task 12.
- Commands: `midi_devices() -> Vec<MidiDevice>`, `set_midi_role(port: String, role: String)` (`keys` | `other`, saved in config).
- `SetupMidi.svelte` (mockup rack MIDI DEVICES): table lamp · port · role (select for keys/other; fixed text for LAUNCHPAD / LCXL) · status (connected / disconnected / port busy / bootloader — run updater), plus the existing MIDI EVENT LOG below.
- Status strip LAUNCHPAD and LCXL lamps from the registry (green connected / off absent / amber busy / red bootloader).

**Tests:** `classify` for each name family incl. `Launchpad MK2 16`, `Launch Control XL`, `Keystation 49 MK3`, a config override; `parse` for note on/off (incl. vel 0), CC, pitch bend centre/extremes, running status not supported → None, sysex → None.

- [ ] Tests first → implement → PASS; UI rack; full verification + build; commit `feat: MIDI device registry, roles and setup MIDI rack`

### Task 11: Launch Control XL

**Files:**
- Create: `crates/lpd-core/src/devices/lcxl.rs`, test `crates/lpd-core/tests/lcxl.rs`, `src-tauri/src/lcxl.rs`
- Modify: `crates/lpd-core/src/devices/mod.rs`, `src-tauri/src/router.rs`, `state.rs`, `lib.rs`, `src/App.svelte`

**Contracts (engine spec 2.3, 7.1 — expected values, unverified until lp-probe):**
```rust
pub const PORT_PREFIX: &str = "Launch Control XL";
pub fn is_lcxl_port(name: &str) -> bool;
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LcxlEvent { Fader { ch: u8, value: u8 }, Knob { row: u8, ch: u8, value: u8 }, Button { row: u8, ch: u8, on: bool } } // ch/row 0-based
pub fn decode(msg: &[u8]) -> Option<LcxlEvent>; // CC 77–84 faders; CC 13–20 / 29–36 / 49–56 knob rows 0/1/2; notes 41–44,57–60 row 0; 73–76,89–92 row 1; any MIDI channel
pub enum LedColor { Off, Red, Green, Amber }
pub fn led_value(c: LedColor) -> u8;            // Off 0x0C, Red 0x0F, Green 0x3C, Amber 0x3F
pub fn led_index(row: u8, ch: u8) -> u8;        // row 0 (focus) 24 + ch, row 1 (control) 40 + ch
pub fn led_message(template: u8, index: u8, value: u8) -> Vec<u8>; // F0 00 20 29 02 11 78 tt ii vv F7
// mapping helpers (pure, in the same module)
pub fn fader_db(value: u8) -> f32;   // p = value/127, UI fader law (mix.ts posToDb): p ≥ .75 → (p−.75)/.25×6; p < .75 → 40·log10(p/.75); 0 → −60 (−∞)
pub fn knob_pan(value: u8) -> f32;   // −1..1, 64 → 0
pub fn knob_pitch(value: u8) -> f32; // −12..12 rounded to 1 st, 64 → 0
pub fn knob_lowpass(value: u8) -> f32; // log 200 Hz–20 kHz, 127 → 20000
```
- Bridge thread (2 s reconnect like `launchpad.rs`; status into the registry): decoded events → `Router::on_lcxl(ev)`: fader → group volume; knob rows → pan / pitch / lowpass; button row 0 press → toggle mute, row 1 press → toggle solo (release ignored). Mixer values: no history, `touch()` for autosave. LEDs: after each change and on connect, mute buttons amber when muted (else off), solo buttons green when soloed; template 0 (User 1).
- UI: event `mixer-changed` `{ groups, masterDb }` throttled to ~30 Hz by the bridge (send at most every 33 ms, last value wins); `App.svelte` replaces `currentSet.mixer` on it (screen jumps to hardware; no pickup).
- MIDI learn ignores LCXL messages ("LCXL controls are fixed in Part 1" — Task 12 shows it).

**Tests (`lcxl.rs`):** decode every control family incl. channel 9 and an unknown CC → None; LED message bytes; fader law at 0, 95 (≈0 dB), 127 (+6); pan/pitch/lowpass endpoints. Router unit: a fader event changes group volume and sends `SetGroup`; button press toggles mute twice → back to false.

- [ ] Tests first → implement → PASS; bridge + UI listener; full verification + build; commit `feat: Launch Control XL mixer mapping and LEDs`

### Task 12: MIDI learn

**Files:**
- Create: `crates/lpd-core/src/kit/learn.rs`, test `crates/lpd-core/tests/kit_learn.rs`, `src/components/SetupLearn.svelte`, `src/lib/learn.ts` (+ test)
- Modify: `crates/lpd-core/src/kit/{model,mod,ops}.rs` (Set gains `#[serde(default)] pub learn: Vec<Binding>`; `move_pad`/`clear_pad` keep bindings correct), `src-tauri/src/router.rs`, `state.rs`, `commands.rs`, `lib.rs`, `src/components/{Knob,Fader,MixerStrip,LaunchpadPanel,PadEditor,ContextMenu}.svelte`, `src/App.svelte`

**Contracts:**
```rust
#[serde(rename_all = "camelCase")]
pub struct Binding { pub device: String, pub kind: LearnKind /* "note"|"cc" */, pub number: u8, pub target: Target }
#[serde(rename_all = "camelCase")]
pub enum Target { Pad { bank: u8, x: u8, y: u8 }, Param(String) }
// serialises as {"pad":{"bank":0,"x":0,"y":0}} or {"param":"group.2.volume"} (parent spec 4.2 example extended with the bank)
pub const PARAMS: … // ids: group.{0-7}.{volume,pan,pitch,lowpass,mute,solo}, master, bank.{0-7}.{humanize,flash,playState,ripple}, keys.volume, keys.mod, seq.play, seq.bpm, seq.swing, stopAll
pub fn param_value(id: &str, cc: u8) -> Option<ParamValue>; // Continuous(f32 in the param's own unit: dB via fader law, pan −1..1, …) | Toggle (cc ≥ 64 rising edge = toggle)
pub fn bind(learn: &mut Vec<Binding>, b: Binding); // one binding per source (device,kind,number): re-using moves it; one source per pad target too
pub fn unbind(learn: &mut Vec<Binding>, index: usize);
```
- `Target::Pad` follows the sound on move/swap (`ops::move_pad` rewrites targets); copy does not duplicate; clear removes the pad's binding.
- Router: `arm_learn(target: LearnTarget)` where `LearnTarget = Pad{…} | Param(String)`; `cancel_learn()`. While armed, the next Note On (pad target) or CC (param target) from a `keys`/`other` device binds (history record key `None`, `touch()`), emits `learn-bound` `{ binding }` and disarms; LCXL/Launchpad messages are ignored while armed and emit `learn-ignored` once (`LCXL controls are fixed in Part 1`). Not armed: notes matching a pad binding → `trigger_pad_in(bank, …)` (select on screen if in the current bank: emit `pad-hit` `{ bank, x, y }` which the UI treats as a hardware hit — **no** `trigger_pad` from the UI), note off → release; CCs matching a param binding → apply (same code paths as the UI commands; param changes emit `mixer-changed` / `set-changed` throttled 30 Hz so the screen follows).
- Commands: `arm_learn(target: LearnTargetDto)` (JS `{ target: { pad: { bank, x, y } } }` or `{ target: { param: 'group.2.volume' } }`), `cancel_learn()`, `unbind(index: usize) -> Set`.
- UI: right-click (or Shift+F10 / Menu) on any Knob / Fader / key with a param id (`learnId` prop) → `ContextMenu` with Learn MIDI CC / Reset; armed control outline blinks amber (`.learning`, steady with reduced motion); bound controls show a 6 px amber dot (`.bound`); notice row 1 while armed ("Move a control on any MIDI device · Esc cancels" / pads "Hit a key or pad…"); Esc cancels (esc stack). Pad editor LEARN key lit amber while waiting; when bound LCD `NOTE 36 · device` + × (unbind). Pad context menu "Learn MIDI note…" enabled. Confirmation `✓ Learned NOTE 36 → R1·C1`.
- `learn.ts`: `bindingLabel(b, set)` → `"Keystation 49 · CC 21 → 3 TOMS · VOLUME"`, `boundIds(set): Set<string>` (param ids + `pad:b:x:y`).
- `SetupLearn.svelte`: list `source → target` with × per row; empty text exactly: "No bindings. Right-click any knob, fader or key → Learn MIDI CC. Pads: LEARN in the pad editor."

**Tests:** `kit_learn.rs`: serde round-trip of both target kinds; `bind` moves a re-used source; `param_value` for volume (127 → +6), pan (0 → −1), toggles; ops: move/swap rewrites pad targets, copy does not, clear removes; an old set.json without `learn` still loads. Router: armed + CC from `other` binds and disarms; CC from LCXL while armed is ignored; bound note triggers the pad. `learn.test.ts`: labels.

- [ ] Tests first → implement → PASS; UI; full verification + build; commit `feat: MIDI learn for pads and on-screen controls`

---

## Phase E — Sequencer and keys

### Task 13: Sequencer engine (model, timing, clock, record)

**Files:**
- Create: `crates/lpd-core/src/seq.rs`, test `crates/lpd-core/tests/seq.rs`, `src-tauri/src/sequencer.rs`
- Modify: `crates/lpd-core/src/lib.rs`, `crates/lpd-core/src/kit/model.rs` (`#[serde(default)] pub pattern: Pattern`), `crates/lpd-core/src/kit/ops.rs` (clear/move keep lanes pointing at pads: lanes follow move/swap like learn bindings; cleared pad → lane stays, shows EMPTY), `src-tauri/src/router.rs`, `state.rs`, `commands.rs`, `lib.rs`

**Contracts (UI spec 4.7, 7.3):**
```rust
#[serde(rename_all = "camelCase", default)]
pub struct Pattern { pub lanes: Vec<Lane>, pub length: u8 /* 4|8|16|32 */, pub bpm: f32 /* 60–200 */, pub swing: f32 /* 50–75 % */ }
#[serde(rename_all = "camelCase")]
pub struct Lane { pub bank: u8, pub x: u8, pub y: u8, pub mute: bool, pub steps: Vec<u8> /* 32 values: 0 off, 1 on (vel 90), 2 accent (vel 127) */ }
impl Default for Pattern { /* no lanes, 16, 100, 50 */ }
pub fn step_duration(bpm: f32) -> f64;                    // 16th notes: 60 / bpm / 4 seconds
/// Start time of step `i` (0-based) from the loop start, swing delaying odd steps by (swing−50)/50 × step.
pub fn step_time(i: usize, bpm: f32, swing: f32) -> f64;
/// Record rule: (step to write, fire_now). A hit more than half a step after step i's time writes i+1 and the
/// clock must not fire step i+1 again on this pass (fire_now = false means "already played live").
pub fn nearest_step(t_in_loop: f64, length: usize, bpm: f32, swing: f32) -> (usize, bool);
pub fn set_length(p: &mut Pattern, new_len: u8); // growing fills the new steps with repeats of the old length when they are all empty; shrinking keeps hidden steps
pub fn add_lane(p: &mut Pattern, bank: u8, x: u8, y: u8) -> Result<(), &'static str>; // "already there"
pub const VEL_ON: u8 = 90; pub const VEL_ACCENT: u8 = 127;
```
- Clock thread (`sequencer.rs`): while playing, sleeps until 1 ms before the next step time then spins; at each step fires every unmuted lane with a step value via `Router::trigger_pad_in(bank, x, y, vel/127)` and `painter.on_pad_hit` **only if that bank is the active bank** (LEDs show the active bank); hold-mode pads get `release` after 90 % of a step; position event `seq-position` `{ playing, step, length, bpm, stepStartedMs }` at ~30 Hz and on every step. Timing deviation from "sample-accurate" (UI spec 4.7): ≤ 1 ms jitter from the spin-wait; documented in a code comment.
- Record: `rec` armed + playing; every live pad hit (screen, Launchpad, computer keys, learned notes — i.e. every `trigger_pad_in` that is not from the clock) is written via `nearest_step` (accent kept, never removes steps; pad without a lane gets one); the whole take is one history record (record on arm-start; subsequent writes don't record); emits `set-changed` (throttled 30 Hz) so the UI redraws lanes.
- Esc / STOP ALL / Launchpad Mixer button also stop the sequencer.
- Commands: `seq_play(play: bool)`, `seq_rec(armed: bool)` (arming starts playback), `seq_update(pattern: Pattern, key: Option<String>) -> Set` (all pattern edits; history key for coalescing BPM/swing drags; `CLEAR` = all steps 0, undoable), `seq_add_lane(bank, x, y) -> Result<Set,String>` (errors "Pad is empty" / "Lane already there"), `seq_audition_step(bank, x, y)` = trigger while stopped.

**Tests (`seq.rs`):** step_duration at 120 BPM = 0.125 s; swing 50 → even spacing, swing 75 → odd steps delayed by half a step; nearest_step early/late/wrap at loop end; set_length 16→32 copies, 32→16 keeps steps; add_lane duplicate error; serde: old set.json without `pattern` loads with default. Clock: pure fn `due_steps(prev_t, now_t, …) -> Vec<usize>` tested for a jump spanning two steps and the loop wrap.

- [ ] Tests first → implement → PASS; clock + record + commands; full verification; commit `feat: step sequencer clock, pattern model and record`

### Task 14: Sequencer UI and telemetry tie-ins

**Files:**
- Modify: `src/components/SequencerPlate.svelte` (functional; drop `unfitted`), `src/App.svelte`, `src/lib/layout.ts` (lanes from the pattern, folded sequencer = 28 px giving height back to pads; keys fold too — UI spec 4 height split), `src/lib/layout.test.ts`, `src/components/Telemetry.svelte` (beat lamps, orbit step ring + playhead planet, `STEP n/len · BPM` / `IDLE · BPM`)
- Create: `src/lib/seq.ts` (+ `seq.test.ts`)

**Contracts:**
- Plate per UI spec 4.7 / mockup `#seq`: PLAY/STOP round key (green ■ while playing), REC ● (red when armed), BPM LCD 60–200 and SWING LCD 50–75 % (drag up/down, wheel, arrows, double-click resets 100 / 50 %), STEPS switch 4|8|16|32, `+ LANE` (selected pad; notices from the error), `CLEAR`, fold ▾ → 28 px strip `▸ SEQUENCER  5 lanes · 16 steps · 100 BPM`.
- Lanes: colour bar in pad colour, LCD `bank·SAMPLE` (bright when it is the selected pad; click = go to that bank and select), `M` mute (amber), `×` remove; cleared pad → `EMPTY` dim italics. Steps: click on/off, right-click or Shift+click accent, drag paints the same value; switched on while stopped → `seq_audition_step`. Playhead orange outline from `seq-position`, interpolated with `stepStartedMs` for the orbit planet. Lanes 14–24 px; under 14 px the body scrolls.
- All edits go through `seq_update` (optimistic local update, server set replaces on return).
- `seq.ts`: `paint(steps, from, value)`, `cycleOnClick(v, accent)`, `foldSummary(pattern) -> "5 lanes · 16 steps · 100 BPM"`.
- `layout.ts`: input gains `keysFolded`, `seqFolded`; a folded plate costs 28 px and returns the rest to the pads; tests for 1280×720 with the sequencer folded → pads 51, both folded → 55 (UI spec 4 figures; adjust only if the existing solver's open-plate numbers also differ from the spec today — then match the current solver's behaviour and note it).

**Tests:** `seq.test.ts`, extended `layout.test.ts`.

- [ ] Tests first → implement → PASS; preview: sequencer edits work locally (preview applies `seq_update` locally since invoke resolves undefined — keep the optimistic local state), playhead does not move in preview; full verification + build; commit `feat(ui): working step sequencer plate`

### Task 15: Keys instrument engine (built-in synth + SoundFont)

**Files:**
- Create: `crates/lpd-core/src/instrument/{mod,basic,soundfont}.rs`, tests `crates/lpd-core/tests/instrument_basic.rs`
- Modify: `Cargo.toml` (workspace dep `rustysynth = "1"`), `crates/lpd-core/Cargo.toml`, `crates/lpd-core/src/lib.rs`, `crates/lpd-core/src/audio/engine.rs`, `no_alloc.rs`, `crates/lpd-core/src/kit/model.rs` (`#[serde(default)] pub keys: KeysSettings`), `src-tauri/src/{router,state,commands,lib,config}.rs`

**Contracts:**
```rust
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum KeysSound { Piano, EPiano, Organ, SynthPad, SynthLead, SynthBass }
#[serde(rename_all = "camelCase", default)]
pub struct KeysSettings { pub sound: KeysSound /* EPiano */, pub volume_db: f32 /* −6, range −40…+6 */, pub output: Option<u8> /* None = master, Some(ch) */ }
pub trait Instrument: Send {
    fn note_on(&mut self, note: u8, velocity: u8);
    fn note_off(&mut self, note: u8);
    fn pitch_bend(&mut self, semitones: f32);   // ±2 st
    fn modulation(&mut self, depth: f32);       // 0..1 vibrato
    fn set_sound(&mut self, sound: KeysSound);
    fn all_notes_off(&mut self);
    /// Add into l/r (len ≤ MAX_BLOCK). Must not allocate.
    fn render_add(&mut self, l: &mut [f32], r: &mut [f32]);
}
pub struct BasicKeys; // 16-voice poly, preallocated; per sound: Piano = decaying additive (1,2,3 harmonics) fast decay; EPiano = 2-op FM sine, bell attack; Organ = drawbar sines 1/2/3/4, no release (stops at once); SynthPad = 3 detuned saws, slow attack 300 ms, release 1.2 s; SynthLead = square + slight saw, release 150 ms; SynthBass = saw one octave down through a one-pole low-pass, release 80 ms. Velocity → level. Oldest voice stolen at 16.
impl BasicKeys { pub fn new(sample_rate: u32) -> Self; }
pub struct SoundFontKeys; // rustysynth::Synthesizer; GM programs: Piano 0, EPiano 4, Organ 16, SynthPad 89, SynthLead 81, SynthBass 38; channel 0; pitch bend range ±2 (RPN 0)
impl SoundFontKeys { pub fn load(path: &Path, sample_rate: u32) -> anyhow::Result<Self>; }
```
- Engine: `Command::KeysNoteOn{note, vel}`, `KeysNoteOff{note}`, `KeysPitch(f32)`, `KeysMod(f32)`, `KeysSound(KeysSound)`, `KeysRoute { gain: f32, group: Option<u8> }`, `SetInstrument(Box<dyn Instrument>)`. The engine owns `Option<Box<dyn Instrument>>`; `SetInstrument` swaps and **returns the old box through a `trash` queue** (`Producer<Box<dyn Instrument>>`, capacity 4) so the audio thread never frees it; the router drains and drops it. Keys render into a keys buffer × gain, then add into the chosen group's buffers **before** group processing (so the channel's fader/pan/pitch?/low-pass/mute/solo apply — pitch offset of a group does not apply to the synth; note it), or into the master sum when `group == None`. `Engine::with_taps` creates a `BasicKeys` by default (allocation at construction is fine). Channel energy: keys routed to a group count in `Levels.groups` automatically.
- Router: on `keys` role notes not bound to a pad → keys commands; pitch bend → `KeysPitch(value/8192 × 2)`; CC 1 → `KeysMod(v/127)`; sends `KeysRoute`/`KeysSound` from `set.keys` on load / change / `replace_audio`.
- SoundFont: `config.soundfont` set → load on a worker thread at startup and on `set_soundfont` (Task 2 command) → `SetInstrument`; load error → notice text via event `keys-status` `{ engine: "builtin"|"soundfont", name, error? }`; cleared → back to `BasicKeys`.
- Commands: `keys_note(note: u8, velocity: u8, on: bool)` (screen keys; router also emits `keys-note` for the mirror of hardware notes in Task 10), `keys_pitch(value: f32)`, `keys_mod(value: f32)`, `set_keys(settings: KeysSettings) -> Set` (history key `keys`), `keys_status() -> KeysStatus`.

**Tests (`instrument_basic.rs`):** each sound renders non-silent after note_on and decays; Organ silent within 5 ms of note_off; SynthPad still sounding 200 ms after note_off; 17 notes steal the oldest (16 voices max); pitch bend +2 st raises a held note's measured period (zero-crossing count) by ≈12 %; `no_alloc.rs`: note_on/off + render with BasicKeys inside `assert_no_alloc`. Engine: keys routed to a muted group are silent; routed to master are audible with all groups muted; `SetInstrument` sends the old box to the trash queue. SoundFont: no `.sf2` on the machine — test only that `load` of a missing path returns an error (no network fetches in tests).

- [ ] Tests first → implement BasicKeys + engine wiring → PASS; SoundFontKeys; router + commands; full verification; commit `feat(core): keys instrument with built-in synth and SoundFont support`

### Task 16: Keys plate UI, mixer channel inputs, SETUP keys sound

**Files:**
- Modify: `src/components/KeysPlate.svelte` (functional; drop `unfitted`), `src/components/MixerStrip.svelte` (channel menu, keys dot), `src/App.svelte`, `src/components/SetupAudio.svelte` (or a small KEYS rack inside SetupMidi: SOUNDFONT LCD + LOAD… / CLEAR keys, engine status line — put it as rack **KEYS** after MIDI DEVICES)
- Create: `src/lib/keys.ts` (+ `keys.test.ts`)

**Contracts (UI spec 4.6, 6 channel inputs):**
- Keybed: 88 keys A0–C8, `C n` labels, lit 49-key window per octave (UI state `octave` −1…+2, persisted in localStorage `lpd.keys.octave`), outside keys dimmed but playable; mouse press = note on (lower on the key = louder, velocity 30–127), slide = glissando, release = note off; pressed keys light in the sound's colour (learned keys: the pad's colour + a dot on keys learned to a pad: `boundIds` from Task 12 gives the notes). Hardware notes mirrored from `keys-note`.
- Controls (176 px): status lamp (Keystation connected from the registry), sound LCD in the sound's colour (click / Enter / Space = menu with the six sounds, current ticked), volume LCD dB (−40…+6, default −6; drag, wheel, arrows, double-click resets), fold ▾ (Ctrl+K) → 28 px strip `▸ KEYS ● KEYSTATION 49`; row 2 octave − / + with range LCD (`C2–C6`), output LCD `→ MASTER` / `→ 8 VOX` (menu MASTER + 8 channels); row 3 `LAST` LCD `E4 · VEL 90` (hidden under 68 px keybed). Keystation unplugged → info notice "Keystation 49 not connected — on-screen keys still play" (add as an info row to `notices.ts`, lowest priority, with a test).
- Wheels: PITCH spring-loaded (returns to centre on release, ±2 st, `keys_pitch`), MOD stays (`keys_mod`, learnable id `keys.mod`); draggable + arrow keys; mirror `keys-wheel`.
- Sound colours (`keys.ts`): `SOUND_COLORS` per sound (pick from `LAUNCHPAD_PALETTE`), `SOUND_LABELS` (`PIANO`, `E.PIANO`, `ORGAN`, `SYNTH PAD`, `SYNTH LEAD`, `SYNTH BASS`), `noteName(n)` (`C4` = 60, `C#4`), `velocityFromY(y, h)` (30–127), `windowRange(octave)` → `[36 + 12·oct, 84 + 12·oct]`, `rangeLabel(octave)`.
- Mixer channel scribble click / right-click → menu: `Pads: n in group k (pad editor › MIX GROUP)` (info, disabled), `✓ Keys: E.PIANO` (toggle routing to this channel; if routed elsewhere show `Keys: on 3 TOMS`), `Rename… F2`. Double-click still renames. Channel carrying the keys shows a dot in the sound's colour; tooltip lists inputs.
- Layout: `keysFolded` from Task 14's solver input.

**Tests:** `keys.test.ts` for noteName (21 → A0, 60 → C4, 108 → C8, 61 → C#4), velocityFromY bounds, windowRange/rangeLabel at −1 and +2; `notices.test.ts` Keystation row.

- [ ] Tests first → implement → PASS; preview: keys play nothing but light, menus open; full verification + build; commit `feat(ui): playable keys plate and mixer channel inputs`

---

## Phase F — Finish

### Task 17: SETUP DISPLAY bend, LOG rack, telemetry fold strip polish

**Files:** `src/components/SetupDisplay.svelte`, `src/components/CrtLayers.svelte`, `src/lib/display.ts` (+ test), `src/components/SetupLog.svelte`, `src/App.svelte`

- DISPLAY: CRT EFFECTS OFF | LOW | MEDIUM (exists) + BEND FLAT | BENT (default FLAT, inactive when CRT is OFF), stored like the CRT level (localStorage key next to the existing one; `readDisplay`/`saveDisplay` extended to `{ level, bend }` with backwards-compatible reading of the old value); BENT applies the SVG `feDisplacementMap` from the mockup (radial map, 4.5 px edge middles, 9 px corners, + 0.4 px blur) to the console. Hint text exactly as UI spec 8 item 4.
- LOG: OPEN LOG FOLDER key → `open_log_folder`.
- Rack units carry four registration marks and an orange pill before the title (mockup `.rack-unit`); check every rack (AUDIO, MIDI DEVICES, KEYS, MIDI LEARN, DISPLAY, LOG) uses them; single column max 860 px, scrolls.

**Tests:** `display.test.ts`: old stored value `"medium"` reads as `{ level: 'medium', bend: 'flat' }`; round-trip.

- [ ] Test first → implement → PASS; full verification + build; commit `feat(ui): display bend option and log rack`

### Task 18: Final verification and spec status

- [ ] `cargo fmt --all -- --check`, `cargo test --workspace`, `npm test`, `npm run check`, `npm run build` — all green; paste the summary lines into the commit message body.
- [ ] Browser preview pass at 1280×720, 1440×900, 1920×1080 (drawer docked / floating, sequencer and keys folded / open, SETUP): no console errors, nothing overlapping; screenshots by eye.
- [ ] `cargo build -p launchpad-drummer` (app compiles); start `npm run tauri dev` once and confirm the window opens, power-on or the last set shows, no panic in the log.
- [ ] Update `docs/superpowers/specs/2026-10-17-ui-design.md` §13 and `2026-10-17-drum-sampler-design.md` §15: what shipped, the deviations (sequencer clock jitter ≤ 1 ms instead of sample-accurate; pad learn target carries the bank; group pitch offset does not apply to the keys; LCXL protocol values unverified until `lp-probe` runs on the unit), and the manual hardware checklist (engine spec 10.3) still to run by the user.
- [ ] Commit `docs: plan 1C status and remaining hardware checks`

---

## Self-review (spec coverage)

| Spec item | Task |
|---|---|
| UI 1 undo/redo 50 steps, kit edits only | 1, 3 |
| UI 1 MIDI LEARN list, New/Open set UI, couldn't-save notice, sample-name label, hardware hit selects | 12, 3, 2–3, exists, exists |
| UI 3 set LCD menu, undo keys, notice LCD, chips; status strip lamps, audio LCD, DROPS, path, save LED | 3, 8, 9, 10 |
| UI 4 drawer browser/editor, docked/floating | 5 (browser tab), exists |
| UI 4.1 pad menu, missing `!`, Stop all, scribble rename (exists) | 7 |
| UI 4.2 humanize applied | 1 |
| UI 4.3 missing box + LOCATE, LEARN, empty text, editor drop target | 7, 12, 6 |
| UI 4.4 browser | 5 |
| UI 4.5 knob right-click learn/reset | 12 |
| UI 4.6 keys plate + engine | 15, 16 |
| UI 4.7 sequencer | 13, 14 |
| UI 4.8 telemetry real feed, beat lamps, orbit playhead | 8, 14 |
| UI 5 drag and drop | 4, 6 |
| UI 6 mixer channel inputs, level fills, LCXL updates | 16, 8, 11 |
| UI 7.1 notices 1–13 | 3 (+ inputs from 2, 8, 9, 10, 11, 12) |
| UI 7.2 power-on | 2, 3 |
| UI 7.3 backend requirements | 1, 2, 5, 8, 13, 15 |
| UI 8 SETUP racks | 9, 10, 12, 16, 17 |
| UI 8.1 learn interaction | 12 |
| UI 10 keyboard | 3 (+ 5, 16 for Ctrl+B / Ctrl+K) |
| UI 11 accessibility | 3, 7, 12 (roles, labels, focus) |
| Engine 4.1 autosave 1 s, atomic save (exists) | 2 |
| Engine 7 other MIDI devices + learn, LCXL mapping/LEDs | 10, 11, 12 |
| Engine 9 error handling (port busy, audio lost, ASIO fallback, dropouts, missing, unreadable set, log file) | 2, 8, 9, 10 |
| Engine 3.1 `instrument` trait | 15 |
