# Guided implementation plan: startup and performance fixes

Date: 2026-10-19 (Asia/Taipei)
Status: Implemented 2026-10-19; see the completion ledger for verification and gaps.
Evidence: [Performance audit](../../2026-10-19-performance-audit.md).
Working directory for all commands: `launchpad-drummer`.

## Instructions for the implementing model

Implement one task at a time, in order. Read the named functions and their callers before editing. Check off a task only after its verification passes. Record the files changed, command results, and any limitations in the completion ledger at the end. Do not claim that a code change improved timing without measuring it.

This document supplies the intended decisions; do not replace them with a new framework, global job system, persistent decode cache, parallel decoder, or broad component rewrite. Keep existing APIs working where directed. The runtime contracts below are planned additions, not APIs that already exist.

Inspect `git status --short` first. At planning time, `src/components/HordeGame.svelte` has unrelated uncommitted work. Preserve it. Do not revert, format, or replace unrelated files. Do not modify the standalone `prototypes/horde/index.html` for desktop startup fixes.

No AGENTS.md, CLAUDE.md, or SPEC.md was found inside the app during planning. Check again before implementation. The existing UI and sampler design documents in `docs/superpowers/specs` remain references for current behavior; older plans contain historical “not done” statements that do not reflect today's code. Do not implement unrelated parked work from those plans.

## Fixed scope and task mapping

| Tasks | Audit fix | Completion rule |
| --- | --- | --- |
| 0 | Baseline | Record current timings and correctness checks |
| 1 | 1: launcher | Ordinary launch uses the production executable |
| 2–6 | 2–3: background restore and loading screen | Complete as one feature before calling startup fixed |
| 7 | 4: initial requests | Parallel, failure-isolated, stale-response-safe initialization |
| 8 | 5: waveforms | Worker, visibility gating, bounded cache |
| 9 | 6: optional screens | Load Setup and Horde on demand |
| 10 | 7: CRT work | Deduplicate and defer mask redraws |
| 11 | 8: large libraries | Measurement gate; implement windowed rows only when justified |
| 12 | 9: asset cleanup | Remove only verified development-only assets from production |
| 13 | Final validation | Report timings and every task's actual status |

Task 11 is explicitly conditional. All other tasks are in scope. An environment limitation is recorded as unverified, never as passed.

## Startup contract: use these decisions

### Separate three kinds of state

1. Keep the existing `Startup` enum as the outcome: loaded, firstRun, notFound, unreadable. Do not make a fake Loaded value stand for initialization.
2. Add a separate `LoadStatus` for operation progress, stored in AppState and serialized in camelCase. It covers initial restore and New/Open/Default operations. It does not replace `OpenResult`.
3. Keep existing `AudioStatus` and `KeysStatus`. Audio and SoundFont initialization run independently. Do not put hardware discovery into the set-loading percentage.

Proposed wire type, mirrored in `src/lib/model.ts`:

```ts
type LoadStatus = {
  requestId: number; // existing open-request ticket, not a second competing counter
  revision: number;  // globally increasing for this process, not reset per request
  phase: 'settings' | 'readingSet' | 'preparingSamples' | 'loadingSamples'
    | 'installing' | 'ready' | 'needsSet' | 'failed';
  completed: number;
  total: number | null; // null means indeterminate
  failed: number;       // failed sample attempts, not fatal operation errors
  error: string | null;
};
```

Use Rust enums for phases and serde camelCase. The wire shape is fixed; helper/type names may follow existing style. Initial state is request 0, revision 0, phase settings. Never send arbitrary raw file paths as the main user-facing label; phase labels are supplied by the UI, with detailed errors separately.

Add `get_load_status` and the `load-status` event. Add `retry_startup` for retrying the configured last set after a terminal startup outcome. It must not reread/overwrite user preferences or start a second configuration initialization. A retry during an active load returns a clear “already loading” error. Register new commands in `src-tauri/src/lib.rs` and preview handlers in `src/lib/api.ts`.

### Phase transitions

| Situation | Required transitions |
| --- | --- |
| No saved set | settings → needsSet; existing Startup::FirstRun |
| Saved set | settings → readingSet → loadingSamples → installing → ready |
| Missing/unreadable saved set | readingSet → needsSet; preserve existing notFound/unreadable outcome |
| User opens an existing set | readingSet → loadingSamples → installing → ready |
| User creates empty set | readingSet → installing → ready; skip nonexistent sample work |
| User chooses Default on first run | readingSet → preparingSamples → loadingSamples → installing → ready |
| Default already exists | ordinary existing-set path; never rewrite its samples or edits |
| Fatal operation error | active phase → failed with actionable error |
| Some samples fail decoding | continue counting; installing → ready; use existing missing-samples reporting |

No sample phase divides by zero. An empty set skips the sample phase or reports 0/0 without rendering a percentage. Configuration currently defaults on missing/broken JSON; preserve that behavior rather than introducing a new fatal-config policy.

### Concurrency and ownership rules

- Reserve the initial restore ticket before spawning its worker. Workers must not allocate a later ticket after decoding.
- New/Open/Default reserve their ticket at the start of the actual operation, after picker cancellation has been ruled out.
- Use the same ticket through preparation, decoding, `finish_open`, status updates, and terminal publication. Do not increment it again in nested helpers.
- Make request registration and status updates atomic relative to each other using the load-status mutex: allocate/register the new request while holding that mutex; update only if the status still belongs to the caller's request. A stale worker cannot publish ready, failed, or progress for the new operation.
- Treat this mutex as a leaf lock. Never hold it across decoding, filesystem access, router/config locking, or event emission. Copy the payload under the lock, release it, then emit. No Tauri handle belongs in `lpd-core`.
- Preserve `install()`'s existing request check under the router lock and the installed-sequence guard in `adopt()`. Do not replace the set directly to bypass these protections.
- Do not append an unconditional assignment to `state.startup` after a background restore: that could overwrite a newer operation's outcome. Non-loaded outcomes must also be guarded against supersession. Follow the existing lock order when publishing the outcome.
- Progress revisions increase under the status mutex. Events can arrive out of order because emission happens after unlocking; the frontend accepts only strictly newer revisions.
- Frontend subscribes and awaits listener registration, then fetches the snapshot. It uses the same revision acceptance function for both paths. Failure to subscribe produces recovery feedback, not a permanent spinner.
- Keep a frontend set-request generation counter. A response started before a later open/retry or ready notification cannot replace the current UI. Use this for `manageSet` command results as well as initial fetches.
- Stale operations may finish their disk read, but cannot become the active set or publish current progress. Do not implement forced thread cancellation. Keep existing file-creation rollback safeguards.

### Readiness and visible behavior

Show a minimal static HTML fallback as soon as WebView content can paint. Replace it with a mounted Svelte loading view. Use phase labels and actual file counts; audio uses an activity indicator/status, not fabricated percent completion.

The main interface becomes available when the set is ready, even if audio is still opening or has failed. Show the audio state and reconnect affordance. MIDI absence and optional SoundFont loading must not block the interface. First run leads to existing New/Open/Default choices; no automatic default-kit creation.

Initial loading owns the main content area. A later set operation uses a loading panel that retains the current set until replacement succeeds; do not clear it preemptively. Prevent accidental duplicate submissions, but retain an explicit Open another set recovery path for a slow restore. This path supersedes the old request; it is not thread cancellation.

The fallback cannot cover compilation before the process starts or WebView creation itself. Task 1 removes ordinary development-tool startup. A separate native splash is out of scope unless native timing later proves it necessary.

## Task 0 — Baseline and safe working state

Files: audit note, existing `run.bat`, `src-tauri/src/main.rs`, `lib.rs`, `logfile.rs`, `src/main.ts`.

- [ ] Record git status and inspect existing changes without modifying them.
- [ ] Run `npm run build`, `npm test`, and `cargo test --workspace`. Record existing failures before changing source. Do not fix unrelated failures under this plan.
- [ ] Add lightweight monotonic startup milestones: process entry, setup entry, configuration ready, restore start/end, audio running/failure. Do not perform filesystem logging on an audio callback.
- [ ] Add frontend performance marks for mount and initial set readiness. Record visible paint in a browser/native trace separately; mount is not proof of paint.
- [ ] Put the report in `docs/performance/startup-baseline.md`. Include launch method, build profile, kit size, display scale, first run versus later runs, and sample count. Measure at least five warm launches; record a cold launch separately if available. Do not call repeated browser navigations native cold starts.

Gate: baseline report distinguishes measured values from unmeasured stages. Existing checks are recorded. Do not start a second native app against the user's active data without isolation; use a disposable test set/config or close only a test process you started.

## Task 1 — Separate launch, development, and build commands

Files: `run.bat`, optionally `package.json` for a documented build alias.

- [ ] Route modes before Rust/Node/dependency checks.
- [ ] No argument and `release`/`--release`: run `%~dp0target\release\launchpad-drummer.exe` directly. Quote paths. Missing file: actionable message pointing to `run.bat build`, nonzero exit; no automatic compilation.
- [ ] `build`/`--build`: retain needed tool/dependency checks, then `call npm run tauri build`. Preserve `bundle.active: false`; do not add installers. This builds production assets and the executable together.
- [ ] `dev`/`--dev`: existing `npm run tauri dev` path.
- [ ] Preserve `probe`, `cli`, and `test` routes and CLI argument quoting. Keep checks specific to each route. Correct the Node requirement text to match package.json (>=22.18).
- [ ] Document custom `CARGO_TARGET_DIR` as unsupported by this simple launcher unless the project already configures it; do not silently run an obsolete executable from an unrelated location.

Verify: `run.bat build`, then `run.bat release` with Vite stopped. Inspect the process tree for absence of Cargo/Vite. Test a missing executable without deleting the real build (use an isolated launcher copy). Check a path with spaces and preserved CLI forwarding. Keep build duration separate from app launch duration.

Gate: direct launch works with no development server. Do not use `tauri dev --release` as the production verification.

## Task 2 — Implement and test progress state, without changing setup yet

Files: `src-tauri/src/startup.rs`, `state.rs`, `commands.rs`, `lib.rs`; `src/lib/model.ts`, `api.ts`; new `src/lib/startup.ts` and `startup.test.ts`; new `src-tauri/tests/startup_progress.rs`.

- [ ] Add LoadStatus, initial state, revision updates, and AppState snapshot access following the contract above.
- [ ] Add a notifier following the existing keys-status notifier pattern. It receives a copied status outside locks.
- [ ] Integrate request registration with `begin_open` so progress and set installation share one ticket. Inspect every `begin_open` caller, including tests.
- [ ] Add command/event wiring and a ready preview snapshot. Browser preview must keep its current demo behavior.
- [ ] In the frontend helper, implement accepting newer revisions, phase-to-label formatting, and safe progress math. Keep these functions pure for testing.

Tests: Rust serialization shape/camelCase; revisions increase; old request progress/error/ready ignored after a new request; notifier can read the snapshot without deadlocking. TypeScript: event revision 8 followed by snapshot revision 7 retains 8; duplicate revision ignored; total null/zero never yields NaN or fake percentage.

Run: `cargo test -p launchpad-drummer --test startup_progress`, `node --test src/lib/startup.test.ts`, `npm run check`.

Gate: these tests pass while existing restore/open behavior still works. Do not switch setup to background work until task 3 can report progress.

## Task 3 — Report sample preparation and decode progress

Files: `crates/lpd-core/src/kit/decode.rs`, `starter.rs`, `mod.rs`; `src-tauri/src/store.rs`, `state.rs`; `crates/lpd-core/tests/kit_decode.rs` and starter tests.

- [ ] Add `load_samples_with_progress` and `write_starter_samples_with_progress`. Keep existing functions as wrappers with a no-op callback so CLI/tests remain compatible.
- [ ] Callback receives completed count, total unique paths, and failure count where relevant. Gather unique paths in deterministic first-reference order. Preserve current sample path validation and decode error handling.
- [ ] Emit initial and final progress even for small kits. A failed decode increments completed and failed and leaves the rest loadable. Existing starter files count as completed checks; never overwrite them.
- [ ] Add the corresponding progress-aware KitManager load entry point, keeping `load_from_dir` for existing callers.
- [ ] Throttle Tauri emissions to one per 100 ms during a phase; phase transitions and terminal updates bypass throttling. Core callbacks remain independent of clocks and Tauri.

Tests: duplicate references count once; empty set completes; two files with one missing reach completed=2, failed=1; starter preparation preserves an existing file; failed generation returns an error; no callback holds application locks.

Run: `cargo test -p lpd-core`, `cargo test -p launchpad-drummer --test store`.

Gate: sample outputs and partial-failure semantics match current behavior. Do not optimize decoding itself in this task.

## Task 4 — Background restoration and operation lifecycle

Files: `src-tauri/src/lib.rs`, `state.rs`, `startup.rs`, `commands.rs`; existing store/set-transition/locking tests and new startup-progress tests.

- [ ] Split `restore` into configuration setup and a ticketed restore operation. Keep a synchronous compatibility wrapper for tests/callers, implemented through the same functions; do not duplicate the loading logic.
- [ ] Setup initializes logging/config, installs notifier/handle, reserves initial request, starts audio with loaded preferences, spawns restoration, starts existing MIDI/sequencer/housekeeping/telemetry, then returns. No sample decode or starter synthesis may run on setup's thread.
- [ ] Publish progress through the task 3 loader. Finish with the existing guarded install/adopt path. Publish ready only after successful adoption for the active request.
- [ ] Preserve FirstRun/NotFound/Unreadable and backup semantics. A superseded restore must not publish one of these over the newer set.
- [ ] Connect New/Open/Default to the same request lifecycle without allocating nested tickets. Existing default sets use ordinary loading; new default sets report preparation first.
- [ ] Add `retry_startup`, using the already-loaded config and an active-phase guard. Make retry/open command error paths publish terminal failure only for their own current request.
- [ ] Do not move decoding, file I/O, or logging into the audio callback. Preserve audio-controller ownership and existing router lock order.

Deterministic tests (use channels/barriers, not arbitrary sleeps):

1. Pause restore A after ticket allocation; complete open B; resume A. B remains current, including config, Startup outcome, and LoadStatus.
2. Pause failing restore A; complete B; release A's failure. B's UI state cannot become failed/needsSet.
3. Progress callbacks can read config/router state without deadlock.
4. Initial missing set leaves saving disabled and reports needsSet.
5. Backup restore still schedules recovery save.
6. Audio can initialize while a loader is paused; the eventual set installs into the current router/audio queues.
7. Existing default-set contents remain untouched on reopening.

Use a narrow injected loader callback at the restore boundary for tests, matching the existing `decide_startup` style; do not create a service abstraction solely for testing.

Run: `cargo test -p launchpad-drummer --test startup_progress`, `--test store`, `--test set_transition`, `--test locking`, `--test audio_controller` (each as a separate cargo test invocation if needed).

Gate: background restore is responsive and races are covered before adding frontend polish.

## Task 5 — Static fallback and Svelte loading/recovery view

Files: `index.html`, `src/main.ts`, `src/App.svelte`, `src/components/PowerOn.svelte`; new `src/components/StartupLoading.svelte`; `src/lib/startup.ts` and tests.

- [ ] Add a static fallback outside `#app`, with inline CSS, system font, dark background, and role=status. Do not depend on webfonts, images, or the main stylesheet to make it readable.
- [ ] Use a dynamic `import('./App.svelte')` in main.ts so bootstrap can show a failure message if the App import/mount fails. Remove fallback only after Svelte has mounted its replacement. A bootstrap failure offers Reload and visible text, not just console output.
- [ ] Replace the frontend's assumed Loaded default with an explicit initialization flag/load status. Do not render or edit the empty placeholder set as ready.
- [ ] Subscribe to `load-status`, await subscription, then get the snapshot. Pass both through the revision guard. Ensure teardown handles a listener whose registration resolves after disposal.
- [ ] Render phase labels and progress. Use `<progress>` without a value for indeterminate stages and with max/value for nonzero totals. Limit screen-reader announcements to phase changes or coarse progress changes.
- [ ] FirstRun/NotFound/Unreadable use existing PowerOn choices after authoritative outcome fetch. Fatal errors show retry/open recovery; retry submits once and returns to progress.
- [ ] Set-ready exits the blocking screen even if audio is opening or failed. Show existing audio status/reconnect; do not block on MIDI or SoundFont.
- [ ] For long-running steps, after 10 seconds show “Still loading” with the active step and Open another set where safe. This is feedback, not an automatic failure or a forced cancellation.
- [ ] Keep current set visible during later loading and keep prior set on failure. Disable duplicate New/Default submissions while their request is active.

Verify in browser with an injected preview delay/error seam, not production artificial sleeps: immediate fallback; slow decode progress; import failure; zero samples; error/retry; no previous set; audio still opening after set-ready; no MIDI. Confirm no forced animation duration and reduced-motion behavior. Do not add a UI library.

Run: `node --test src/lib/startup.test.ts`, `npm run build` and the focused browser checks.

Gate: every waiting/failure path has visible feedback and an exit/recovery path.

## Task 6 — First-run vertical integration checkpoint

Files: tests and completion ledger; fix only defects in tasks 2–5.

- [ ] With isolated first-run config, show New/Open/Default immediately after settings complete.
- [ ] Choose Default: observe preparation counts, loading counts, set-ready, and independent audio status. Confirm files are created only after this explicit choice.
- [ ] Relaunch: restore the same edited kit without regeneration.
- [ ] Inject a sample failure and a set failure; verify warnings versus blocking errors are distinct.
- [ ] Test an event emitted before frontend registration; snapshot restores the correct status.
- [ ] Test an event arriving between snapshot request and response; older snapshot cannot rewind progress.

Run: `cargo test --workspace`, `npm test`, `npm run build` once for this combined feature. Record native visual verification separately from preview checks.

Gate: tasks 2–6 form a complete working startup feature. Do not label only the progress bar as completion.

## Task 7 — Concurrent initial state with stale-response protection

Files: `src/App.svelte:loadInitialState`, `manageSet`; `src/lib/startup.ts` tests; preview handlers if necessary.

- [ ] Launch config, history, missing-sample, keys, audio, and MIDI fetches concurrently where independent. Handle optional failures individually or with `Promise.allSettled`; an optional device failure must not reject the entire initialization.
- [ ] Fetch set/bank/save only when ready. Tag the fetch with accepted requestId and a frontend generation captured before awaiting. Apply only if both are still current.
- [ ] Read load status again after the ready-state fetch if needed to detect a newer backend request whose event has not reached the frontend. Discard/retry the fetch if ownership changed.
- [ ] Apply the same generation guard to `manageSet` results. Keep existing audioEventSeen and midiEventSeen logic; preserve event precedence.
- [ ] Refresh missing samples/history after the new set becomes current, rather than accepting stale pre-load data.

Tests: one optional request rejects; required set request rejects; older initial set response arrives after user open; newer audio/MIDI event beats response; two rapid ready notifications. Test the extracted ordering helper and a browser integration with controllable promises.

Run: `node --test src/lib/startup.test.ts`, `npm test`, `npm run build`.

Gate: no sequential optional-request waterfall and no stale set overwrite.

## Task 8 — Waveform worker and bounded cache

Files: `src-tauri/src/commands.rs:sample_peaks`, `state.rs:sample_peaks`; `src/App.svelte` waveform effect; relevant sample tests.

- [ ] Make the command async and route the scan through the existing `worker` helper.
- [ ] Gate the frontend effect on actual editor visibility and selected sample. Preserve its stale-response cleanup.
- [ ] Use one cached envelope entry (last requested loaded sample plus bucket count) initially. Compare sample identity with `Arc::ptr_eq` and retain only a `Weak<Sample>` in the cache so old audio is not kept alive. This bounds cache size without a new dependency.
- [ ] Clone the sample under kit-manager lock, then scan with no application/cache lock held. Publish the small envelope afterward. A same-path replacement has different identity and misses the cache.

Tests: repeated request returns identical peaks; replacement at same path recomputes; bucket count change recomputes; sample removal does not retain PCM; hidden editor sends no request; late selection response ignored. Use a long synthetic recording for a responsiveness check, not a brittle millisecond unit-test assertion.

Run relevant Rust sample tests, `npm test`, `npm run build`.

## Task 9 — Defer optional screens

Files: `src/App.svelte`; existing Setup components and Horde import; optional new `SetupPanel.svelte` only if needed to group current setup markup without changing behavior.

- [ ] Replace eager Horde import with an on-demand import when Games selects Horde. Preserve the bound component's `padInput` API and event cleanup. Do not rewrite existing Horde internals.
- [ ] Load Setup content on first entry. A type-only import must not accidentally turn into a runtime eager import.
- [ ] Cache resolved components in component/module state. Show a local loading placeholder and error/retry UI. Guard import results against mode changes/unmount.
- [ ] Do not defer Play or the startup view.

Verify: production output has separate chunks; first Play navigation does not request optional chunks or game sounds; Setup/Horde work on first and repeated entry; mode change during import is safe; import failure recovers. Measure initial bytes and startup again; report actual delta.

Run `npm test` and `npm run build`, then targeted browser mode-switch checks.

## Task 10 — Deduplicate CRT mask redraws

Files: `src/components/CrtLayers.svelte`.

- [ ] Replace direct multiple redraw triggers with one scheduled animation-frame redraw.
- [ ] Cache width, height, device pixel ratio, level, and any geometry-affecting setting. Skip unchanged work; preserve redraw after switching off then on.
- [ ] Do not generate the decorative mask until the loading shell has had a paint opportunity. Use a frame boundary, not a fixed artificial delay.
- [ ] Keep ResizeObserver and scale-change detection as needed, but feed the same scheduler. Cancel pending work during cleanup.

Verify visually at 1280×720 and 1920×1080, scale 1 and 2, CRT off/low/medium, bent/flat, resizing and repeated mount/unmount. Profile redraw counts and long tasks. Do not reduce visual resolution or delete effects just to improve a metric.

Run `npm run build` and focused browser checks; no broad new unit-test suite for canvas implementation details.

## Task 11 — Large-library measurement gate and row windowing

Files: `src/components/SampleBrowser.svelte`, `src/lib/tree.ts`, `tree.test.ts`.

- [ ] Create a temporary library fixture outside user data with 5,000 supported file entries. Measure initial render, scrolling, and key navigation in the production frontend; dummy files need not decode successfully for this rendering check.
- [ ] If mounting/expanding the rows creates >50 ms main-thread tasks attributable to the list, or repeatable input stalls, implement the following. Otherwise mark this task deferred with measurements, not done-by-implementation.
- [ ] Keep the full flattened model for navigation; render the visible index range plus 10 rows of overscan each side. Use the existing fixed 22 px row height and top/bottom spacers.
- [ ] Keep IDs based on logical indexes, not sliced indexes. Scroll a keyboard target into the rendered range before resolving its DOM element. `aria-activedescendant` must refer to a rendered element.
- [ ] Preserve expansion, selection, audition, drag, and folder lazy loading. Clamp indexes after collapse. No virtualization dependency.

Tests: empty list, range boundaries, collapsed folder removing selection, keyboard navigation across the viewport, End-of-list range clamping. Browser check drag and audition wiring. Run `node --test src/lib/tree.test.ts` and `npm run build`.

## Task 12 — Production asset cleanup

Files: `public/games`, development documentation/archive destination outside public.

- [ ] Inventory candidate files and search references with `rg` across source, scripts, HTML, CSS, and JSON. Inspect computed URLs and iframe game routes manually.
- [ ] Move confirmed screenshots/design docs/archived versions to `docs/reference/game-archives/`, preserving relative structure. Do not recursively remove the public tree.
- [ ] Treat sfx-bundle.json as potentially live until each game's loader is checked. Keep current game HTML, referenced audio, fonts, and images.
- [ ] Update documentation links affected by moves. Rebuild and compare output file count/bytes.

Verify each game opens and sounds still load in production, including iframe games. Report verified removal bytes, not the audit's entire candidate total. Do not describe disk-size savings as measured startup improvement.

## Task 13 — Final acceptance and handoff

- [ ] Run `cargo test --workspace`, `npm test`, `npm run build` after the final source changes. Do not rerun unchanged suites repeatedly without a reason.
- [ ] Build and launch the production executable, verifying it contains the current frontend.
- [ ] Repeat the task 0 timing method on the same machine/kit. Report before/after median and range, separately for warm/cold and preview/native. Mark hardware checks unavailable when appropriate.
- [ ] Verify first run, subsequent restore, large kit, missing/corrupt samples, backup recovery, unavailable audio, disconnected MIDI, high DPI, startup retry, and superseded restore.
- [ ] Update the audit status with links to implementation evidence. Update only relevant durable behavior in existing design docs; do not rewrite unrelated specs or claim unverified features are shipped.
- [ ] List all deferred items and remaining limitations. Leave unrelated changes intact.

Release acceptance: no development toolchain on ordinary launch; no sample decoding on setup thread; immediate content once WebView can paint; progress based on real work; all waits terminate in ready or actionable recovery; no stale request overwrites; existing set/autosave/audio correctness preserved.

## Completion ledger

Update this table as implementation proceeds. Include exact test command/results or link a concise evidence file under `docs/performance/`. Do not pre-check tasks based on this plan's existence.

| Task | Status | Changed files / evidence / limitations |
| --- | --- | --- |
| 0 Baseline | Done | Baseline checks: `npm run build` pass, `npm test` 105 pass, `cargo test --workspace` 381 pass. Added `logfile::mark_process_start/milestone` (`src-tauri/src/logfile.rs`, used in `lib.rs`: setup entered, configuration ready, restore started/finished, audio status) and frontend marks `lpd:mounted` (`src/main.ts`) / `lpd:set-ready` (`App.svelte`). Report: [startup-baseline.md](../../performance/startup-baseline.md). Limits: warm launches only, no native cold start, no paint trace; `contentMs` is DOM presence. A transient "paging file is too small" compiler failure occurred once when two cargo jobs ran at once; the rerun with `-j 2` passed. |
| 1 Launcher | Done | `run.bat` rewritten: modes routed before tool checks; no arg/`release` runs `target\release\launchpad-drummer.exe` directly; `build` = `npm run tauri build`; `dev`; `probe`/`cli`/`test` kept; Node text now 22.18+; `LPD_NO_PAUSE` skips prompts. Verified: `run.bat build` 101 s (the earlier `tauri dev --release` exe was not a production build); `run.bat` launch has no cargo/node/npm/vite descendants and nothing on port 1420; missing exe (isolated copy in a folder with spaces) prints the build hint, exit 1; unknown mode exit 2; `cli` forwards a quoted path with spaces. Limit: custom `CARGO_TARGET_DIR` unsupported (documented in the script). |
| 2 Progress state | Done | `startup.rs` (`LoadPhase`, `LoadStatus`, `LoadStep`, `LoadTracker` with 100 ms throttle), `state.rs` (leaf `load` lock, notifier, `get_load_status`, ticket allocation under that lock), `commands.rs`/`lib.rs` (`get_load_status`, `retry_startup`, `load-status` event), `src/lib/model.ts`, `src/lib/startup.ts`, `api.ts` preview handlers. Tests: `src-tauri/tests/startup_progress.rs` (serialization, rising revisions, stale ignore, throttle, no deadlock) and `src/lib/startup.test.ts` (16). |
| 3 Sample progress | Done | `kit::load_samples_with_progress`, `unique_sample_paths`, `write_starter_samples_with_progress` (wrappers keep the old APIs), `KitManager::load_from_dir_with_progress`. Tests in `kit_decode.rs` (3) and `starter.rs` (2): unique paths once, empty = 0/0 once, failure counted, existing starter file kept, failed write errors. Decoding itself not optimized. |
| 4 Background restore | Done | `AppState::init_config/begin_restore/run_restore(_with)/restore_guarded/retry_startup`, `run_ticketed` (a failure or panic ends the own ticket as failed), guarded `set_startup_for`; `lib.rs` setup reserves the ticket, starts audio, spawns the `restore` thread. Race tests from the plan are in `startup_progress.rs` using channels (superseded success and failure, no deadlock from callbacks/notifier, missing set disables saving, backup recovery save, audio replaced while the loader is paused, default set not rewritten). Full workspace: 406 pass. |
| 5 Loading UI | Done | `index.html` static fallback, `main.ts` dynamic import with failure + Reload, `StartupLoading.svelte`, `PowerOn.svelte` (TRY AGAIN, busy), `App.svelte` (load-status subscription awaited before the snapshot, `hasSet`, overlay for later operations after 250 ms, 10 s "Still loading" with OPEN ANOTHER SET). Preview seam `?startup=slow|error|empty|firstrun|import-fail` (preview only). Browser-checked: fallback replaced, slow progress + still-loading, error + retry, first run + default (preparing then loading counts), import failure, normal preview unchanged. Limits: reduced-motion handling of the native `<progress>` indeterminate animation is best-effort CSS, not inspected visually; no real screen-reader test. |
| 6 First-run integration | Partly verified | Tests: first-run needsSet, nothing created before the explicit Default choice, prep/loading/ready counts, edits kept on reopen, partial sample failure ends ready with a `failed` count vs a fatal error ends failed. Native: restore from an isolated config; missing set shows the choices screen with TRY AGAIN, and retry reopens it. Event-before-subscription and snapshot-after-event are covered by the `acceptLoad` / `fetchReadyState` tests. **Not verified natively:** first-run Default creation (it would write into the real Documents folder) and injected failures. |
| 7 Initial requests | Done | `loadDeviceState` (allSettled), `fetchReadyState` (concurrent, required vs optional, ownership re-check), `setGate` generation guard also on `manageSet`/retry, `eventFirst` for audio/MIDI. Tests: optional failure, required failure, no waterfall, stale after user open, two rapid ready notices, ownership change, event beats snapshot. Native check: the independent requests are issued together and the set is fetched only after `get_load_status`. |
| 8 Waveforms | Done | `sample_peaks` is async via `worker`; one-entry cache keyed by `Arc::ptr_eq` + bucket count holding a `Weak<Sample>`; the frontend requests only while Play + editor drawer + selected sample (a string-derived value, so other pad edits do not re-request). Tests (3): cache hit / bucket miss / same-path replacement, no PCM retained after reclaim, a 5-minute recording scanned without blocking state reads. "Hidden editor sends no request" holds by construction (effect gate), not separately asserted. |
| 9 Deferred screens | Done | `SetupPanel.svelte` (groups the six Setup sections) and `HordeGame` are imported on demand with local loading/error/retry; `HordeGame.svelte` itself untouched. Build: initial JS about 185 KB (index 3.3 + client 48.1 + App 133.4) vs 242.3 KB before; Horde chunk 71 KB and Setup chunk 13 KB load on first entry. Browser: first Play load requests no game/Setup chunks; Games (all four) and Setup work. The import-failure retry UI exists but was not forced in a browser. Startup delta from this task alone not isolated. |
| 10 CRT redraws | Done | `CrtLayers.svelte`: single rAF scheduler, cache key (size, pixel ratio, level), first draw waits one extra frame, cleanup cancels. Preview check: 5 resize events cause no redraw; level switches redraw; mask drawn for low and medium. Not checked visually at 1280×720 / 1920×1080 at scale 1 and 2 or with bend; no profiler redraw counts. |
| 11 Large-library gate | Done (gate met) | 5,000-file fixture via preview `?files=5000`: mounting all rows produced long tasks of 369 ms and 294 ms (> 50 ms). Key/scroll timings were unreliable (background-tab rAF throttling) and are not claimed. Implemented windowing (`tree.ts` `visibleRange/revealScrollTop/clampIndex`, `SampleBrowser.svelte`): 50 rows in the DOM, no long tasks, ids by logical index, `aria-activedescendant` target always rendered (checked at list start and near row 4900). 6 new unit tests. Limits: drag/audition wiring not exercised in a browser; a pointer capture on a row that scrolls out of range mid-drag is not handled. |
| 12 Assets | Done | Moved 6.75 MB (screenshots, design lock, `versions/`, and the unreferenced cockpit `sfx/` including `sfx-bundle.json`: the cockpit embeds its sounds in `index.html`) to `docs/reference/game-archives/`. Kept each game's `index.html` and `horde/sfx/`. Production output 8.54 MB / 96 files → 1.79 MB / 40 files. Browser: chess, xiangqi and cockpit iframes return 200, Horde loads and its sfx return 200. Not heard: cockpit audio playback. Byte savings are not a startup claim. |
| 13 Final acceptance | Done with gaps | After the final source changes: `cargo test --workspace` 406 pass / 0 fail; `npm test` 127 pass; `npm run build` 0 errors; the production exe was rebuilt and confirmed to serve the current `App-*.js`. Timing: [startup-baseline.md](../../performance/startup-baseline.md). Not verified: cold start, native paint trace, high DPI, real MIDI/ASIO, native first-run Default, an unavailable-audio run, large kits beyond the 82 MB synthetic one. |
