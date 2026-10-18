# Launchpad Drummer — Code audit and remediation plan

Date: 2026-10-18 (Asia/Taipei)  
Audited revision: `10835e4`  
Status: review complete. Findings 1–13 implemented and verified in software on 2026-10-18 (see [Implementation record](#implementation-record-2026-10-18)); hardware, ASIO, latency and WebView2 checks remain open.

## Scope and conclusion

Review the code delivered through Plans 1A, 1B, and the implemented foundation of 1C. Assume one performer using eight banks, up to 32 simultaneous voices, and live editing on Windows.

The core audio engine has ample rendering headroom in the isolated benchmark. The urgent issues are in integration: opposite mutex acquisition order, sample lifetime during set replacement, destructive set creation, sample assignment without decoding, and storage work holding the MIDI routing lock. Fix these before expanding the sequencer or device threads.

This note preserves the audit findings, evidence, plan coverage, performance measurements, and appropriate fixes. The audit did not change repository source files. Temporary reproduction programs were run outside the repository; they are evidence, not committed regression tests.

Related documents:

- [Plan 1A: core and CLI](2026-10-17-plan-1a-core-and-cli.md)
- [Plan 1A session handoff](2026-10-18-plan-1a-session-handoff.md)
- [Plan 1B: Tauri integration](2026-10-18-plan-1b-tauri-integration.md)
- [Plan 1C: UI completion](2026-10-18-plan-1c-ui-completion.md)
- [Drum sampler design](../specs/2026-10-17-drum-sampler-design.md)
- [UI design](../specs/2026-10-17-ui-design.md)
- [Recorded Launchpad protocol verification](../../launchpad-mk2-protocol.md)

Source line references below identify the audited revision and may move during implementation.

## Implementation order

- [x] First: finding 1, remove the deadlock and establish a consistent locking rule.
- [x] Next: findings 2 and 5, make set transitions, sample retirement, and storage handoffs safe.
- [x] Next: findings 3 and 4, protect existing kits and make sample assignment playable.
- [x] Next: finding 8 with Plan 1C task 2, provide visible save status and persistence. (Only the saver/status/autosave part of task 2; see deviations.)
- [x] Then: findings 6 and 7 together, establish one owner for hardware input and LED output. (Software-verified with a fake MIDI sink; not on hardware.)
- [x] Then: findings 9–13, finish control semantics, launcher routing, history, idle rendering, and audio health reporting.
- [ ] Reconcile plan/spec status, then resume the remaining Plan 1C features.

Keep fixes small and preserve the existing core/integration/UI separation. Do not rewrite the DSP engine or introduce a general framework to solve these issues. Each implementation task should add the focused regression checks described below and update this checklist only after verification.

## Must fix

### 1. P1 — Pad input and LED painting can deadlock

**Location:** `src-tauri/src/state.rs:123,160,170–185`; `src-tauri/src/lights.rs`.

**Problem:** Input locks `router` and then `painter`. `paint_frame` locks `painter`, then its sounding-state callback reacquires `router`. Two threads can each hold the lock the other needs. A concurrent reproduction left both workers stuck, with both locks held and no progress between the 500 ms and 1,000 ms observations.

**Fix:**

- Under one short router lock, capture the active bank, its pad/light data, loaded-sample flags, and all 64 sounding flags.
- Release the router lock before acquiring the painter lock.
- Make painting consume that snapshot; the sounding callback must never acquire a lock.
- Audit the other router/painter call sites for the same rule. Avoid cloning all eight banks or reacquiring the router separately for each pad.

**Verification:** Add a focused concurrent input/paint regression with a bounded completion check, plus a deterministic check that painting can consume its snapshot without accessing the router. Do not leave a deadlocked test blocking the test runner indefinitely; use a bounded child process if needed. Preserve existing light-mode tests.

**If skipped:** MIDI input, LED updates, and application commands can freeze during ordinary playing.

- [x] Implement and verify finding 1. Done 2026-10-18. Router lock is released before the painter is taken; painting consumes a `PaintSnapshot` (active bank only). Tests: `src-tauri/tests/locking.rs`.

### 2. P1 — Set changes leave old audio playing and free samples in the audio callback

**Location:** `src-tauri/src/router.rs:127–132`; `src-tauri/src/state.rs:315–344`; `crates/lpd-core/src/audio/voice.rs:125`; `crates/lpd-core/tests/no_alloc.rs`.

**Problem:** Set replacement drops the old sample caches and clears held-pad and voice tracking without stopping existing voices. Those voices may hold the last strong sample reference. The reproduction found zero stop/release commands, continued audible playback, one remaining strong reference, and a sample-buffer deallocation inside rendering. The existing allocation test keeps an external sample reference alive, so it does not exercise this failure.

**Fix:**

- Queue a guaranteed stop/fade of the old voices before accepting triggers for the new set. Preserve the existing short stop fade; do not assume sending `StopAll` means the fade has already finished.
- Move old sample ownership into a retirement collection before removing the active caches. A non-audio worker retains those references until neither queued commands nor voices own the samples, then frees them there. A retained `Arc` with strong count 1 can be reclaimed once it is no longer reachable from the active caches.
- Do not clear voice counters while old start/end events remain in flight. Keep the counters balanced through the transition; only introduce a generation identifier if immediate independent counters for the new set are required.
- Clear held-pad tracking only after the stop request has been accepted. Handle a full command queue explicitly so set replacement cannot silently lose the stop operation.
- Apply the same transition procedure to both New and Open. Keep the old set active if preparing the replacement fails.

**Verification:** Trigger a long hold sample, switch sets, release the original pad, and verify the old sound fades out. Count both allocations and deallocations around rendering while dropping the old caches. Verify that old voice events cannot leave a new pad permanently marked sounding, and that reclamation happens outside rendering. Exercise a full command queue.

**If skipped:** Set changes can leave unwanted audio sounding, lose release control, and cause callback stalls from sample destruction.

- [x] Implement and verify finding 2. Done 2026-10-18. `Router::replace_set` queues StopAll + mixer commands only when the whole transition fits, retires old samples, keeps voice counters; `AppState::maintain` frees them off the audio thread. Tests: `src-tauri/tests/set_transition.rs` (counting allocator), mutation-checked.

### 3. P1 — New set can overwrite an existing kit

**Location:** `src-tauri/src/state.rs:330–334`; compare `crates/lpd-cli/src/main.rs`.

**Problem:** The GUI immediately saves an empty set into the supplied directory. Unlike the CLI, it does not reject an existing set. The reproduction replaced a named kit with the default empty kit. A subsequent save can also replace the recovery backup.

**Fix:** Before any filesystem or active-state mutation, reject a destination containing `set.json` or `set.json.bak`. Preserve broken/recovery artifacts as well; direct the user to Open or a different folder. Return an actionable error rather than silently replacing a kit. Use the same creation guard for the GUI and CLI where practical.

**Verification:** Attempt creation in folders containing a valid main file, only a backup, or an unreadable main file. Assert that file contents and the active in-memory set are unchanged. Confirm that creation in a fresh directory still works.

**If skipped:** A mistyped destination can replace the user's kit and eventually its recovery copy.

- [x] Implement and verify finding 3. Done 2026-10-18. `kit::ensure_new_set_dir` (set.json, .bak, .broken, .tmp) used by GUI and CLI before any mutation; a refused or failed creation leaves folder and active set untouched. Tests: `set_transition.rs`, `crates/lpd-core/tests/kit_store.rs`.

### 4. P1 — Assigning a sample does not load it

**Location:** `src/components/PadEditor.svelte:89`; `src-tauri/src/state.rs:219`; `src-tauri/src/store.rs`; `src/App.svelte`.

**Problem:** The sample path field only changes pad metadata. It does not decode the file or install it in the sample caches, and the existing import command has no frontend caller. A valid WAV decoded successfully in isolation but remained absent from the cache after assignment.

**Fix:**

- Give sample assignment an explicit backend operation, distinct from ordinary parameter updates. Capture the target bank, coordinates, set identity, and path when the request starts.
- Resolve a set-relative path against the active set directory, decode on a worker, then install the sample and commit the undoable pad edit together.
- For an external file, use the import/copy path before assignment; do not require the user to manually copy files into the set folder.
- Keep the existing pad playable if decoding fails and show the reason. If the active set changed while decoding, reject the stale result. Return the authoritative updated set/pad to the UI.
- Reuse this operation for Plan 1C's drop and locate flows rather than creating separate assignment implementations.

**Verification:** Assign a valid sample that was not previously referenced and immediately trigger it without reloading. Check waveform availability, invalid files, a set switch during decoding, and two assignments whose workers finish out of order. A late result must not overwrite a newer assignment.

**If skipped:** The editor appears to configure a pad successfully, but the pad remains silent until save/reload.

- [x] Implement and verify finding 4. Done 2026-10-18. New `assign_sample` command: set-relative path or external file (copied in), decoded on a worker, installed and committed as one undoable edit; stale set / older request rejected. Tests: `src-tauri/tests/sample_assign.rs`.

### 5. P1 — Storage work blocks MIDI routing

**Location:** `src-tauri/src/state.rs:315,347–356`; `src-tauri/src/commands.rs:37,47,87,97`; `src-tauri/src/store.rs`.

**Problem:** Saving holds the router mutex throughout serialization and disk writes. MIDI presses and releases need that mutex. Set loading and decoding also run synchronously instead of using the worker handoff promised by Plan 1B.

**Fix:**

- Capture a matching set snapshot and destination path under a brief, consistent locking boundary, then release application locks before serialization or filesystem access.
- Run loading, importing, and other substantial decoding work through blocking workers exposed by async Tauri commands. Commit completed results under a short lock only after validating their target set/request identity.
- Keep save operations ordered so two writes cannot race on the fixed temporary and backup filenames. The saver from Plan 1C task 2 should become the shared path for manual and automatic saves.
- Do not report a set as saved if it changed after the saved snapshot was captured; leave newer edits pending.

**Verification:** Inject a save operation that waits on a test barrier and verify that pad trigger/release and painting continue while it is blocked. Test edits during an in-flight save and a set switch during loading/saving, ensuring that data is written to the correct directory and newer dirty state is preserved.

**If skipped:** Slow disks, antivirus scans, or large kits delay input and undermine the low-latency architecture.

- [x] Implement and verify finding 5. Done 2026-10-18. Save, open, new, import and assign run on blocking workers; `run_save` captures set, folder, set identity and revision under brief locks, writes with none held, serialises writes, and acknowledges only the saved revision. Tests: `src-tauri/tests/save.rs`.

### 6. P2 — LED diff state survives hardware resets and external frames

**Location:** `src-tauri/src/launchpad.rs:137–173,302–306`; `src-tauri/src/lights.rs`.

**Problem:** Reconnection clears all LEDs but retains the painter's previous-frame cache. Manual game frames also bypass that cache. The next unchanged sampler frame has an empty diff, so pads can stay dark or retain game colours. The cache reproduction returned zero updates for an unchanged frame after a simulated device reset.

**Fix:** Add an explicit cache invalidation/full-refresh operation. Invoke it on connection loss, successful reconnect, failed output sends, and changes of LED owner. After reconnect, send the current owner's complete frame. Update the sent-frame cache only for successfully delivered output, and stop treating the last manual frame as the authoritative sampler frame.

**Verification:** With a fake MIDI sink, send a stable kit frame, disconnect/reconnect, and verify a full repaint without any pad hit or edit. Send a game frame, switch back to Play, and verify a complete sampler repaint. Cover failed sends and retries.

**If skipped:** The hardware display disagrees with the UI and can remain incorrect until unrelated LEDs change.

- [x] Implement and verify finding 6. Done 2026-10-18 (software only). `LightPainter::invalidate` / generation-checked `mark_sent`; `LedOutput` invalidates on connect, loss, failed send and owner change. Tests: `src-tauri/tests/led_output.rs`. Not verified on a Launchpad.

### 7. P2 — Game mode still controls the sampler

**Location:** `src-tauri/src/launchpad.rs:273`; `src/App.svelte:166,256`; `src/components/HordeGame.svelte`.

**Problem:** The active mode lives only in Svelte. Every hardware event reaches the sampler before the UI routes it to Horde. Game side buttons therefore also change the sampler bank; the UI's early return can leave its displayed bank stale.

**Fix:**

- Add a small backend routing mode for sampler versus game ownership, and switch it through one command when entering/leaving Games. Setup can retain sampler routing if that is the intended editing behavior.
- In game mode, forward game input to the UI without invoking sampler triggers or bank selection. Ensure previously held sampler notes receive their stop/release before ownership changes.
- Gate sampler LED painting while the game owns output. On return to Play, synchronize the current bank and force the full repaint from finding 6.
- Use a bounded/latest-frame handoff for game LEDs rather than accumulating stale visual frames if producers outrun MIDI output.

**Verification:** Load a sounding kit, enter Horde, press grid and side buttons, and assert zero sampler trigger/bank changes. Switch modes while a hold pad is down; verify it stops and both UI and hardware restore the sampler state.

**If skipped:** Game actions unexpectedly play drums, alter banks, and compete with sampler LED output.

- [x] Implement and verify finding 7. Done 2026-10-18 (software only). Backend `InputMode` (`set_input_mode`), sampler input ignored while a game owns the pads, held pads released on entry, latest-frame game LED slot, full repaint on return. Tests: `src-tauri/tests/input_mode.rs`, `led_output.rs`. Not verified on a Launchpad.

### 8. P2 — Save failures are not visible

**Location:** `src/App.svelte:247–252`; `src-tauri/src/state.rs:85`; Plan 1C tasks 2–3.

**Problem:** SAVE errors only reach `console.warn`. There is no visible failure or dirty-state indicator. Autosave is also not implemented: `touch()` is intentionally a placeholder for the next planned task.

**Fix:** Use Plan 1C's shared `pending`/`saved`/`failed` status for manual and automatic saves. Display an actionable error, preserve dirty state on failure, and only acknowledge the revision actually written. Finish the one-second debounced autosave and retry behavior in task 2, using the worker and ordering rules from finding 5. Until that full task ships, manual-save errors still need immediate visible feedback.

**Verification:** Inject a write failure and check that the notice remains visible and dirty state remains pending. Retry successfully and verify it clears only for the saved revision. Cover edits made while saving and saving before a set directory exists.

**If skipped:** Users can close the app believing changes were saved when they were not.

- [x] Implement and verify finding 8. Done 2026-10-18. `saver.rs` (pending/saved/failed, 1 s debounce, 2 s retry), `save-status` event, notice + SAVE badge in the UI. Tests: `saver.rs` unit tests, `save.rs`, `src/lib/status.test.ts`.

### 9. P2 — The fader's −∞ position is not silent

**Location:** `src/lib/mix.ts:12–26`; `crates/lpd-core/src/audio/mod.rs:14`; `crates/lpd-core/src/kit/params.rs`.

**Problem:** The UI encodes the bottom position as −60 dB and displays −∞. Rust's ordinary dB conversion produces gain `0.001`, not zero.

**Fix:** Define the fader-floor convention explicitly at the mixer parameter boundary: a group or master value at/below the UI floor means zero linear gain. Preserve ordinary mathematical `db_to_gain` behavior for other uses such as humanize. Reuse the same mixer conversion in desktop and CLI paths, and keep UI labels and stored values consistent. Define pad-volume floor behavior separately if it also displays −∞.

**Verification:** Render a nonzero sample with group and master faders at the floor and require exact silence. Check that values above the floor still use the intended dB curve and that humanize behavior remains unchanged. Add a matching frontend boundary test.

**If skipped:** Controls labelled −∞ still leak audio.

- [x] Implement and verify finding 9. Done 2026-10-18. `audio::fader_gain` / `FADER_FLOOR_DB` (−60 dB = exact silence) used for group, master (desktop and CLI) and pad volume; `db_to_gain` unchanged for humanize. Tests: `kit_params.rs`, `audio_engine.rs`, `src/lib/mix.test.ts`.

### 10. P2 — The batch launcher passes the `cli` routing token to Rust

**Location:** `run.bat:115–121`.

**Problem:** `shift` changes numbered batch arguments but does not change `%*`. A batch reproduction confirmed that `cli new-set example` remains unchanged after `shift`. The launcher therefore invokes the Rust CLI with `cli play ...` or `cli new-set ...`, which it rejects.

**Fix:** Forward the arguments after the launcher routing token explicitly, preserving quoted paths. The CLI currently has a small, bounded argument shape; avoid a general-purpose argument parser or unsafe string reconstruction. Preserve the actual child exit code.

**Verification:** Exercise both `cli` and `--cli`, New and Play, paths containing spaces, and `--buffer`/`--asio`. Use an argument-echoing stub for hardware-dependent routes and a temporary directory for a real `new-set` smoke check. Confirm nonzero exit codes survive the launcher.

**If skipped:** The advertised CLI launcher route is unusable even though direct Cargo commands work.

- [x] Implement and verify finding 10. Done 2026-10-18. `run.bat` collects the arguments after the routing token one at a time and keeps the child exit code. Tests: `tools/test-run-bat.ps1` (stubbed cargo plus a real `new-set` smoke).

## Should fix

### 11. Continuous edits exhaust undo history

**Location:** `src-tauri/src/state.rs:219,276`; `crates/lpd-core/src/kit/history.rs`; relevant editor commands.

**Problem:** The history module can coalesce edits, but pad and humanize update paths always use `edit(None, ...)`. Seventy consecutive parameter updates consumed all 50 history slots; undoing all retained entries stopped at gain 1.9 rather than restoring the original value.

**Fix:** Pass stable keys for continuous changes, including bank, pad coordinates, and parameter identity, for example `pad:0:2:3:volume` or `bank:0:humanize`. Keep discrete actions such as clear, assignment, and rename as separate undo entries. Where the command currently replaces the whole pad, add the minimum parameter/edit-key information needed to avoid coalescing unrelated edits. Keep mixer value changes outside kit undo as specified.

**Verification:** Through AppState/command-level operations, make more than 50 updates to one control within the coalescing window and verify one undo restores its starting value. A different control, a discrete action, or a pause beyond the window must create a separate step. Preserve redo invalidation behavior.

**If skipped:** One drag discards useful edit history before the undo UI is even connected.

- [x] Implement and verify finding 11. Done 2026-10-18. `update_pad` takes an optional coalescing `key` (UI sends `pad:<bank>:<x>:<y>:<field>` for volume, pitch, trim and RGB knobs); humanize keys itself. Tests: `src-tauri/tests/edit_history.rs`, `src/lib/status.test.ts`.

### 12. Standby telemetry performs unnecessary continuous rendering

**Location:** `src/components/Telemetry.svelte:69,153–165`; `src/lib/activity.ts`.

**Problem:** Static scope, spectrum, and meter displays redraw on every animation frame. Each presentation reads pixels back, processes them, and applies a blur. In a three-second headless run at 1920×1080, the open telemetry view performed approximately 1,204 readbacks over 12.5 million pixels despite having no engine level feed.

**Fix:** Cache the static grids, labels, and standby panels; regenerate only on resize or a relevant display change. Draw orbit activity only while its values change, and bound future live telemetry rendering to the intended feed rate instead of monitor refresh rate. Avoid pixel readback for unchanged frames. Respect reduced motion without changing displayed state correctness.

**Verification:** Repeat the same idle measurements with telemetry open/folded and reduced motion. Record frame/readback counts and main-thread time. Confirm that standby work falls substantially, resizing redraws correctly, and activity still decays visibly. Profile actual WebView2 separately before making GPU performance claims.

**If skipped:** Idle visuals consume CPU and compositing resources that could serve interaction and real telemetry.

- [x] Implement and verify finding 12. Done 2026-10-18. Telemetry draws on size change and while orbit activity decays; the activity loop sleeps at rest. Measured in headless Edge, see the performance table. Not measured in WebView2.

### 13. Audio status only reflects startup

**Location:** `src-tauri/src/audio.rs:94`; `src-tauri/src/lib.rs:36`; `src/App.svelte:96`; Plan 1C task 9.

**Problem:** Stream errors only print to stderr. Stored status is set at startup and the frontend fetches it once. A later device failure can leave the AUDIO indicator reporting a healthy running stream.

**Fix:** Forward a lightweight failure signal to the audio owner/controller, update application status there, and emit a typed status event consumed by the UI. Do not perform stream reconstruction, blocking UI emission, or filesystem logging in the audio render callback. Implement the reconnect action with the audio-controller work in Plan 1C task 9; a visible error state should work before that larger feature is complete.

**Verification:** Inject a stream error without hardware, verify the status transitions from running to error and reaches the UI, then simulate a successful restart. Confirm repeated errors do not flood the UI and stale events from a replaced stream cannot mark the new stream failed.

**If skipped:** The app provides misleading health information when playback stops.

- [x] Implement and verify finding 13. Done 2026-10-18. Stream error callback only sets a per-stream flag; `housekeeping` thread moves it into `AudioStatus` once and emits `audio-status`; the UI shows an error lamp and notice. Tests: `src-tauri/tests/audio_health.rs`. No real device failure was provoked; reconnect stays with Plan 1C task 9.

## Coverage against the plans

These are unimplemented or incomplete planned features, separate from defects in delivered behavior.

| Scope | Audited state | Next action |
|---|---|---|
| 1A core, protocol, CLI | Main implementation exists. The committed Launchpad report records 29 successful hardware checks. | Fix the affected shared behavior above; perform the remaining hardware/ASIO acceptance checks. |
| 1B desktop integration | Router, painter, mixer, editor, and commands exist. Worker loading, safe sample retirement, and live audio health are incomplete. | Findings 1–7 and 13 close the principal integration gaps. |
| 1C task 1 | Single set ownership, history APIs, and humanize exist. Continuous-edit keys are unwired. | Finding 11; task 3 still owns connecting the undo/redo UI. |
| 1C tasks 2–3 | Config, startup restoration, autosave, rotating logs, native pickers, notices, set menu, and full keyboard handling remain outstanding. | Complete persistence/status first, integrating findings 3, 5, and 8. |
| 1C tasks 4–7 | Browser/audition, drag-and-drop, move/swap/copy, locate-missing workflow, and context menus remain outstanding. Basic edit/clear controls exist. | Reuse finding 4's assignment transaction across these features. |
| 1C tasks 8–12 | Engine telemetry, FFT, dropout/latency reporting, audio setup, general MIDI registry, LCXL, and learn remain outstanding. | Establish lifecycle/ownership rules first; incorporate findings 12–13. |
| 1C tasks 13–18 | Sequencer, playable keys/SoundFont engine, remaining setup work, and final acceptance remain outstanding. | Keys and sequencer are inert displays at the audited revision; retain that distinction until functional. |

### Documentation reconciliation

- [x] Mark the Plan 1A session handoff as historical/superseded. It still says Task 3 has not started, while the implementation and protocol report exist.
- [ ] Update plan task status from verified code and checks, not from the existence of a visual placeholder.
- [ ] Reconcile the older engine-spec scope statements with the later Plan 1C allocation of sequencer and keys work. Distinguish agreed scope from shipped behavior.
- [ ] Preserve the recorded Launchpad protocol results, but do not present them as a fresh hardware test from this audit or as verification of LCXL.
- [ ] Keep ASIO latency, dropout endurance, and real WebView2 performance explicitly pending until measured.

## Architecture assessment

Keep the useful separation already present:

- `lpd-core`: device codecs, kit model/history, parameter conversion, and hardware-independent audio rendering.
- `src-tauri`: device ownership, storage workers, sample lifetime, command routing, and UI events.
- Svelte: editing and display, with authoritative state returned from the backend.

The integration currently differs from the planned single-owner router thread: it shares state through several mutexes and dispatches directly from MIDI callbacks. The immediate repair is to make lock ordering explicit, use short consistent snapshots, and remove disk work and sample destruction from timing-sensitive paths. A dedicated router owner can follow if required by the upcoming MIDI/sequencer work; it is not necessary to rewrite the engine to fix the current defects.

Rules to preserve while implementing:

1. Audio rendering performs no allocation, deallocation of sample buffers, blocking I/O, or mutex acquisition.
2. No worker result commits into a different set or overwrites a newer request.
3. One mode owns Launchpad input and LED output at a time.
4. Saving captures a matching set/path and acknowledges only the saved revision.
5. Hardware input remains independent of webview round trips in sampler mode.

## Performance evidence

### Offline audio rendering

Optimized build; 48 kHz output; 32 active voices distributed across eight groups; all eight low-pass filters enabled at 1 kHz. Each case used 100 warm-up blocks and 2,000 measured blocks. The voices remained active through the measurement.

| Buffer frames | Median render time | 99th percentile | Maximum observed | Buffer duration |
|---|---:|---:|---:|---:|
| 64 | 0.0143 ms | 0.0209 ms | 0.0998 ms | 1.333 ms |
| 128 | 0.0290 ms | 0.0591 ms | 0.2740 ms | 2.667 ms |
| 256 | 0.0584 ms | 0.1137 ms | 0.2709 ms | 5.333 ms |
| 512 | 0.1177 ms | 0.1564 ms | 0.3105 ms | 10.667 ms |

The benchmark also exercised idle and eight-voice cases. The results support keeping the current DSP implementation. They measure `Engine::render`, not the full CPAL callback, OS scheduling, MIDI dispatch, device latency, or a real-time dropout guarantee. Synthetic samples also do not represent every possible signal/parameter workload.

### Browser idle work

Headless Chromium browser preview; 1920×1080; device scale factor 1; three-second observation per state. Main-thread and script times are cumulative CDP metrics over that interval; script time is part of main-thread work, not additional to it.

| State | Main-thread work | Script time | Canvas readbacks | Pixels read |
|---|---:|---:|---:|---:|
| Telemetry open, normal motion | 466.54 ms | 346.41 ms | 1,204 | 12,491,500 |
| Telemetry folded, normal motion | 205.30 ms | 124.60 ms | 301 | 1,435,770 |
| Telemetry open, reduced motion | 81.65 ms | 44.48 ms | 44 | 456,500 |

The observations identify avoidable standby work. They are not a controlled GPU benchmark or measurements of the desktop WebView2 runtime. Folding changes rendered content as well as work volume.

The frontend production bundle built successfully: JavaScript approximately 108.76 kB / 40.66 kB gzip; CSS approximately 30.22 kB / 7.93 kB gzip. Bundle size was not the principal issue found in this review.

## Validation and remaining test gaps

Completed against the audited checkout:

- `cargo test --workspace`: **101 tests passed**.
- `cargo fmt --all -- --check`: passed.
- `npm test`: **23 tests passed**.
- `npm run build`, including `npm run check`: passed; zero Svelte errors or warnings.
- Browser previews at 1280×720, 1440×900, and 1920×1080: zero runtime exceptions and no measured outer-layout overflow in the inspected main panels.
- Temporary targeted reproductions confirmed the deadlock, set-switch sample deallocation, overwritten set, uncached valid sample assignment, undo exhaustion, fader-floor mismatch, and batch argument behavior. LED cache behavior was exercised without real MIDI hardware.

The passing suite does not establish concurrency or lifecycle safety. In particular, the no-allocation test retains a sample owner throughout rendering, and the audio-system test checks status serialization rather than device startup/loss/recovery. Add the targeted regressions above rather than relying on more repetitions of the existing suite.

After implementing the fixes:

- [x] Run the focused regression tests for each finding.
- [x] Run workspace tests, formatting, frontend tests, Svelte checking, and the production frontend build.
- [x] Build and smoke-test the actual desktop application. (Release exe built; driven over WebView2 remote debugging: IPC and UI save-failure notice. No Launchpad attached.)
- [ ] Repeat UI mode changes, set changes during playback, save failure/retry, and hardware reconnect checks. (UI mode change and save failure checked in the app; set change during playback and retry are covered by tests only; hardware reconnect not done.)
- [x] Repeat the audio and idle-rendering measurements after relevant changes, documenting methodology and limits.
- [ ] Complete the engine spec's manual hardware checklist, including a sustained session and measured latency/dropouts.

Not checked by this audit: physical MIDI/audio operation, ASIO setup, end-to-end latency, prolonged dropout behavior, WebView2 GPU cost, or the internal code of archived HTML game prototypes. Their integration with the current app was inspected where relevant.

## Implementation record (2026-10-18)

Implemented against the working tree on top of `10835e4`. Not committed.

### Verification

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | clean |
| `cargo test --workspace` | 157 passed, 0 failed (101 at the audited revision) |
| `npm test` | 32 passed (23 before) |
| `npm run build` (includes `svelte-check`) | 0 errors, 0 warnings; JS 111.56 kB / 41.77 gzip, CSS 30.35 kB / 7.95 gzip |
| `pwsh tools/test-run-bat.ps1` | all launcher checks passed |
| `npm run tauri -- build --no-bundle` | release exe built; smoke-tested over WebView2 remote debugging |

Mutation check: replacing sample retirement with a plain drop makes `set_transition.rs` fail (a deallocation inside rendering), so the test does detect finding 2.

The baseline `cargo test --workspace` first failed to compile the `src-tauri` integration tests ("multiple different versions of crate `lpd_core`"). The cause was stale mixed build artifacts in `target/`; `cargo clean -p lpd-core -p launchpad-drummer` fixed it. This is a build-cache issue, not a source change.

### Performance (repeated)

Idle browser work, same method as above (headless Edge, 1920×1080, DPR 1, three seconds, CDP `TaskDuration`/`ScriptDuration`, `getImageData` counted by a page hook). The "before" column was rebuilt from `HEAD` (bundle 108.76 kB, as audited) and reproduces the audit's counts.

| State | Before: main / script / readbacks / pixels | After: main / script / readbacks / pixels |
|---|---|---|
| Telemetry open, normal motion | 454.95 ms / 341.11 ms / 1,204 / 12,491,500 | 47.11 ms / 9.56 ms / 0 / 0 |
| Telemetry folded, normal motion | 182.63 ms / 110.74 ms / 301 / 1,435,770 | 18.44 ms / 7.43 ms / 0 / 0 |
| Telemetry open, reduced motion | 60.45 ms / 32.50 ms / 48 / 498,000 | 4.27 ms / 0.33 ms / 0 / 0 |

After the change the panels still draw on load, redraw on resize (checked 1920→1800 wide) and the orbit changes on a pad hit and returns to its exact idle image after the decay. Remaining main-thread time is the 10 Hz clock text and compositing. Limits: headless Chromium/Edge, not WebView2; no GPU claims.

Offline engine render (`Engine::render`, release, 48 kHz, 32 voices over 8 groups, all low-passes at 1 kHz, 100 warm-up + 2,000 measured blocks; throw-away program outside the repo):

| Buffer frames | Median | 99th percentile | Maximum | Buffer duration |
|---|---:|---:|---:|---:|
| 64 | 0.0168 ms | 0.0187 ms | 0.0263 ms | 1.333 ms |
| 128 | 0.0334 ms | 0.0363 ms | 0.0854 ms | 2.667 ms |
| 256 | 0.0666 ms | 0.0709 ms | 0.1280 ms | 5.333 ms |
| 512 | 0.1355 ms | 0.1465 ms | 0.2207 ms | 10.667 ms |

The DSP code is unchanged (only the gain conversion at the mixer boundary), so this checks for regressions; it is on a different run and sample content than the audit's table (about 15 % higher medians, same order of magnitude). It is still `Engine::render` only, not the CPAL callback, device latency or a dropout guarantee.

### Deviations and decisions

- **Plan 1C task 2 is only partly done.** Implemented: the saver, `save-status` event, autosave (1 s debounce, 2 s retry) and `get_save_status`. Not done: config file, startup restore of the last set, log file, native pickers, `new_set`/`open_set`/`OpenResult`, and the `save_set_dir`→`save_now` rename. The existing `load_set_dir`, `new_set_dir` and `save_set_dir` commands stay (now async workers); the set menu is still the `prompt()` placeholder, but errors now go to the notice LCD instead of `alert`.
- **Plan 1C task 3 is not done.** Undo/redo buttons are still inert; the notice LCD only gained save/audio/action messages.
- **Sample assignment needs a set folder.** With no folder (first launch, before New/Open) `assign_sample` fails with "Create or open a set first" instead of storing an unreachable path. `update_pad` still accepts a `sample` field (kept for existing callers); the editor no longer uses it for samples.
- **Pad volume has the same silent floor** as the faders, because its knob also displays −∞.
- **`RETRY FRAME` in Setup now calls `refresh_leds`** (full repaint) instead of re-sending the last frame.
- **Game frames in sampler mode are dropped** by `send_led_frame`; only the owner may draw.
- **Audio failure reporting** has no message text (the status keeps its fields, `state` becomes `error`) and the reconnect action is still Plan 1C task 9.

### Still open

- Opening or creating a set within the 1 s autosave window does not flush the old set first; up to one second of edits to the previous set can be lost. (Manual save before switching is unaffected.)
- A superseded or rejected assignment leaves its imported copy in the set's `samples/` folder.
- Not performed: Launchpad hardware checks (LED repaint after real unplug/replug, game↔sampler on the device, the engine spec's manual checklist), ASIO, latency and dropout measurement, sustained session, WebView2 performance, a real audio-device failure and restart, and LCXL.
- The desktop smoke test drove the real IPC and two UI paths over remote debugging with no Launchpad attached; it was not a full manual pass.
- Documentation reconciliation items above remain, except the handoff and Plan 1C status notes added with this change.
