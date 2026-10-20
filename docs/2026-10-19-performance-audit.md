# Performance audit and proposed fixes

Date: 2026-10-19 (Asia/Taipei)
Status: Fixes 1-9 implemented 2026-10-19 (fix 8 only after its measurement gate was met). Evidence and remaining gaps: [completion ledger](superpowers/plans/2026-10-19-performance-fixes.md) and [startup measurements](performance/startup-baseline.md).

**Implementation guide:** Follow [the guided fix plan](superpowers/plans/2026-10-19-performance-fixes.md) task by task. It defines the startup contract, file changes, tests, and completion gates. This audit remains the evidence/reference document; the guide is the execution order. Neither document marks the fixes as shipped.

## Summary

Launchpad Drummer is a Tauri desktop app with a Svelte interface, Rust audio engine, MIDI integration, and games. This review assumes one desktop user and covers startup, sample loading, UI rendering, library browsing, and the integrated Horde path. The standalone Horde prototype is a separate entry point; the desktop findings below concern `launchpad-drummer`.

The leading suspect for a long wait before the app appears is the launcher: both normal and “release” launch use the development workflow. This is supported by code inspection, but native end-to-end startup has not been timed. Separately, synchronous set restoration can block initialization for larger kits. A loading screen should accompany background initialization so users never have to guess whether the app is working.

Implement fixes 1–4 first. They address launch overhead, blocking initialization, blank-window feedback, and frontend initialization ordering.

## Evidence and limits

| Check | Observed result |
| --- | --- |
| `npm run build`, including Svelte checks | 4.47 seconds; zero errors or warnings |
| Vite production build alone | 674 ms |
| Production JavaScript | 242.21 KB; 88.12 KB gzip |
| Production CSS | 44.11 KB; 10.59 KB gzip |
| Browser preview load event, six navigations | 69–433 ms |
| Main-thread long tasks in those navigations | 57–309 ms |
| Current default-set directory | Approximately 1.83 MB |
| Recent desktop log entries | Default set restored within the same one-second timestamp as setup logging; audio reported running within roughly a second afterward |
| Production output | Approximately 8.51 MB across 88 files |
| Public files matching versions, screenshots, design notes, READMEs, and sound bundles | Approximately 6.54 MB across 25 files; reference checks required before removal |

Browser measurements used the production preview in headless Chromium at 1920×1080 with device scale factors 1 and 2. They exclude Rust, WebView creation, MIDI, and real audio initialization. Repeated navigations are not independent cold starts. Paint entries were not consistently available in the short sampling window, so the load-event figures must not be presented as time-to-visible-content. No browser runtime exceptions occurred in those six navigations.

Desktop logs have one-second resolution and begin inside Tauri setup. They do not measure time spent launching development tools, compiling, or creating the WebView. Current default-set restoration does not explain a many-second delay by itself. CRT comparison runs were noisy and did not establish CRT as the sole cause of browser pauses.

No native cold-start benchmark, real MIDI/ASIO benchmark, or full hardware audio stress test was performed. The existing uncommitted `src/components/HordeGame.svelte` changes were preserved. The audit ran the frontend build, which regenerated ignored build output, but changed no application source.

## 1. Launch the built app directly

Priority: High. Likely largest improvement to ordinary launch.

Evidence: `run.bat:93` runs `npm run tauri dev`; `run.bat:107` runs `npm run tauri dev -- --release`. `src-tauri/tauri.conf.json` configures Vite as the development server. Dependency checks happen before routing to a launch mode.

Problem: Even “release” launch uses the development workflow. Startup involves Node, Vite, Cargo checks, and compilation when required. Missing dependencies can trigger installation before launch.

Proposed fix:

- Build the production executable as a separate operation, using the production frontend output.
- Make normal launch open that executable directly, without checking or starting Node, npm, Cargo, or Vite.
- Keep explicit development and build commands. Name them accurately.
- If the built executable is absent, show a clear build/setup instruction instead of silently compiling during ordinary launch.
- Keep dependency installation in setup/development, outside normal launch.

Verify: Launch the built app with no Vite server running. Confirm the process tree contains no development server or Cargo process. Compare cold and warm launch times with the current launcher. Confirm the production executable includes the current frontend, rather than pointing at an older artifact.

If skipped: Launch remains sensitive to toolchain state and rebuilds. A loading screen inside the app cannot cover compilation before the process exists.

## 2. Move set restoration out of blocking setup

Priority: High. Protects responsiveness as kits grow.

Evidence: `src-tauri/src/lib.rs:68` calls `restore()` inside setup. `src-tauri/src/state.rs:1138` restores configuration and the last kit. `src-tauri/src/store.rs:76` loads the kit and all samples; `crates/lpd-core/src/kit/decode.rs:75` decodes unique referenced samples sequentially. Audio startup is requested only after restore returns.

Problem: A large kit, compressed recordings, or slow storage extends blocking setup. The current default kit is small and restores quickly in existing logs; this is a concrete scaling problem rather than a measured explanation for the current long delay.

Proposed fix:

- Separate configuration reading from slow set/sample restoration.
- Read configuration first so audio preferences are available.
- Start sample restoration on a worker and let setup return promptly.
- Start audio independently after preferences are known; preserve safe sample installation and routing.
- Reuse the existing open-request ticket protection so a stale startup load cannot overwrite a newer set request. Allocate the startup request ticket before slow work starts.
- Keep a defined loading state until installation finishes; do not expose an empty placeholder set as successfully restored.
- Report failed samples and retain existing backup recovery behavior.

Start with background loading, not an unbounded parallel decoder or persistent cache. If later measurements show decoding itself dominates, evaluate bounded concurrency separately with memory limits.

Verify: Default kit, large kit, missing sample, corrupt set with valid backup, and slow storage. The window must remain responsive while loading. A newer open request must win over a still-running startup restoration. Audio must not reference incomplete sample state.

If skipped: Larger sets can stall initialization, regardless of whether a progress component exists.

## 3. Add a real startup/loading screen

Priority: High. Directly addresses the empty-window experience.

Evidence: `index.html:10` contains an empty app root. `src/App.svelte` initially assumes a loaded startup state; `loadInitialState()` logs failures to the console. `PowerOn.svelte` handles no-set/recovery choices, not loading progress.

Proposed fix:

- Add a minimal static loading fallback to HTML, using inline styling and system fonts. Keep it outside the Svelte mount target or explicitly remove it when the app is ready to replace it.
- Mount a lightweight startup view before optional screens and decorative work.
- Represent startup phases explicitly in Rust and the frontend, including loading, ready, and failed states.
- Store the latest progress snapshot in Rust and expose it through a command as well as events. Subscribe before fetching the snapshot and prevent an older snapshot from overwriting newer events, using a revision or equivalent ordering guard.
- Count unique sample paths, matching the existing decoder's deduplication. Count completed attempts, including failures, while reporting failures separately.
- Throttle progress emission, for example to at most 10 updates per second, with immediate phase and completion updates.
- Keep retry, open another set, and the existing first-run choices available when appropriate. Never leave a failed startup behind an indefinite spinner.

### Loading-screen behavior

| Stage | Text | Progress |
| --- | --- | --- |
| Web content starts | Starting Launchpad Drummer… | Immediate static fallback |
| Configuration | Reading settings… | Activity indicator |
| Default-kit creation, only when requested | Preparing default samples… | Completed files / total |
| Set restoration | Loading samples — 12 of 32 | Actual completed count |
| Audio initialization | Opening audio device… | Activity indicator |
| Optional SoundFont load | Loading instrument… | Activity indicator unless measurable progress is available |
| Ready | Main interface | Dismiss loading view |
| Failure | Specific reason and recovery action | Stop indeterminate progress |

The sample bar measures completed files, not an estimate of remaining time; files can differ greatly in size. Do not fabricate an overall percentage. Distinguish set-ready from audio-ready. An audio failure should leave the interface usable with an explicit unavailable-audio status and reconnect action.

First run with no previous set should lead to the existing New/Open/Default choices. Do not silently create a kit merely to give the loading screen something to do. When the user chooses Default Set, report preparation and loading progress for that operation.

Disconnected MIDI hardware must not block readiness. Game assets should load when their game is opened, with local progress or failure feedback if needed. Do not preload every game or tool at app startup. Avoid a mandatory animation delay, and respect reduced motion.

Limits: HTML cannot display before the WebView can render it. Fix 1 removes the development-build wait from ordinary launch; fix 2 lets the WebView and loading UI proceed during slow work. Measure any remaining pre-WebView blank period before deciding whether a separate native splash is justified.

Verify: Slow loading, zero samples, missing samples, failed configuration/set load, late event subscription, retry, unavailable audio, and first-run Default Set creation. Check that the fallback is replaced cleanly and that failures always leave a usable recovery action.

If skipped: Users cannot distinguish loading from failure.

## 4. Remove the initial-state request waterfall

Priority: High after startup state is made explicit.

Evidence: `src/App.svelte:461` awaits startup, missing samples, history, config, keys status, audio status, and MIDI devices in sequence before fetching set/bank/save state concurrently.

Problem: Each request adds latency, and failure in an earlier optional request prevents later core state from loading.

Proposed fix:

- Fetch independent state concurrently.
- Handle optional status failures independently from essential set data.
- Preserve event-versus-snapshot ordering guards for audio and MIDI, and extend equivalent protection to startup progress.
- With background restoration, fetch the authoritative set after the ready transition or use a consistent ready snapshot; do not treat pre-restoration placeholder state as final.
- Show essential initialization failures in the startup screen rather than only logging them.

Verify: Delay or fail each optional request and confirm the main set still loads. Deliver newer events during initial requests and confirm older responses do not overwrite them.

If skipped: Avoidable request latency and fragile initialization remain.

## 5. Move waveform scans to a worker and avoid unnecessary requests

Priority: Medium. Relevant to long recordings and editor responsiveness.

Evidence: `src-tauri/src/commands.rs:487` exposes synchronous `sample_peaks`; `src-tauri/src/state.rs:1099` scans every sample frame. The effect at `src/App.svelte:576` requests peaks without requiring the editor to be visible.

Proposed fix:

- Request peaks only while a waveform consumer is visible.
- Run the scan using the existing blocking-worker mechanism.
- Cache the resulting small envelope by loaded sample identity and bucket count, not path alone; replacement at the same path must invalidate it.
- Bound the cache to loaded samples and discard stale results when selection changes.

Verify: Select a long recording, repeat selection, replace a sample at the same path, close the editor, and rapidly change selections. Confirm no hidden-editor requests and no stale waveform display.

If skipped: Long samples can cause repeated scans and selection stalls.

## 6. Load optional screens on demand

Priority: Low to medium; measure after fixes 1–4.

Evidence: `src/App.svelte:11` onward statically imports Setup screens and Horde. The production build currently emits one JavaScript entry bundle of approximately 242 KB.

Proposed fix: Dynamically import optional screens when first opened, with local loading/error feedback. Keep Play and the startup shell immediately available. Preserve screen lifecycle cleanup when switching modes.

Verify: Compare initial bundle size and main-thread work. Open every deferred screen, exercise its controls, and test import failure/retry. Confirm game assets are not fetched during ordinary Play startup.

If skipped: Some avoidable parsing and evaluation remain, but bundle size alone does not explain a long native startup.

## 7. Consolidate and defer CRT mask work

Priority: Low to medium; rendering optimization candidate.

Evidence: `src/components/CrtLayers.svelte:9` builds a dense canvas path at physical display resolution. A reactive effect, ResizeObserver, and window resize listener can all request redraws.

Proposed fix:

- Cache the last rendered dimensions, device scale, and relevant settings; skip unchanged redraws.
- Consolidate resize scheduling into one animation-frame request.
- Defer the initial decorative mask until the startup view has painted.
- Preserve the physical-pixel appearance unless measurement justifies a resolution change.

Verify: Compare startup and resize traces at device scale factors 1 and 2, including maximized windows and display-scale changes. Check CRT off/low/medium and bend settings visually. Do not claim a gain based only on the noisy initial browser comparison.

If skipped: High-resolution decoration can add first-frame and resize work.

## 8. Limit rendered rows for large sample libraries

Priority: Conditional on large libraries; not a current startup cause.

Evidence: `src/components/SampleBrowser.svelte:60` renders every expanded row from the flattened tree.

Proposed fix: For large expanded lists, render the visible range with a small buffer. Preserve tree semantics, keyboard selection, focused-row visibility, and drag operations. Keep the existing lazy folder expansion.

Verify: A directory with thousands of entries; repeated expand/collapse, keyboard navigation, audition, drag, and scrolling. Compare DOM node count and interaction timings.

If skipped: Large libraries may make browsing sluggish even when app startup is fast.

## 9. Remove development-only material from production assets

Priority: Low. Distribution/build cleanup, not a primary startup fix.

Evidence: `public/games` contains archived versions, screenshots, design documents, and sound bundles. Approximately 6.54 MB matches those categories, out of 8.51 MB total build output.

Proposed fix:

- Search all source, HTML, CSS, scripts, and dynamic asset naming before deciding which files are unused.
- Move confirmed archives, screenshots, and documentation outside `public` while retaining their source/history.
- Keep runtime audio and any referenced bundles. Do not assume every file in the measured categories is removable.
- Rebuild and verify each game and its sounds.

If skipped: Production output remains larger than necessary. These assets are not all requested at startup, so byte reduction is not equivalent to launch-time reduction.

## Existing behavior worth preserving

- Audio device opening already runs in the background after configuration/restoration.
- Configured SoundFonts load on a background thread after the stream starts.
- Sample loading deduplicates repeated references.
- Ordinary open/import operations already use workers.
- Telemetry generation is gated by UI mode and visibility; activity animation sleeps at rest.
- Audio rendering uses preallocated buffers and bounded voices; a rewrite is not justified by this audit.
- Existing startup, set-transition, save, and audio tests provide useful correctness coverage, but do not replace startup timing measurements.

## Implementation order and completion checks

1. Record a native baseline: launcher invocation, process start, first visible content, set-ready, and audio-ready. Add monotonic elapsed timings with millisecond precision; current wall-clock log timestamps are too coarse.
2. Implement direct production launch (fix 1).
3. Implement background restoration and startup feedback together (fixes 2–3), including first-run preparation progress and recovery paths.
4. Parallelize independent initial requests with consistent readiness handling (fix 4).
5. Implement waveform work reduction (fix 5).
6. Re-measure. Apply deferred imports and CRT scheduling if their measured cost warrants it (fixes 6–7).
7. Address large-library rendering and distribution cleanup (fixes 8–9).

Benchmark cold and warm launches separately, with multiple runs and reported median/range. Test the default kit, a representative large kit, first run, missing/corrupt files, unavailable audio, disconnected MIDI, and high-DPI displays. Keep browser-preview timings separate from native timings.

Acceptance criteria:

- Normal launch requires no running development tools and performs no build/install work.
- Loading feedback is visible as soon as web content can render, and stays responsive during restoration.
- Every startup path reaches ready or an actionable error state; absent MIDI never holds the loading screen open.
- Measurable phases report real progress without flooding the UI.
- A stale background load or response cannot overwrite a newer user request or event.
- First-run and subsequent-run behavior preserve existing set, backup, and autosave safety.
- Compare launch timings against the baseline; do not promise a numerical speedup before measurement.
- Run focused regression tests for changed initialization and loading behavior, plus the frontend build/check and relevant existing Rust tests. Verify real audio/MIDI behavior when hardware is available.
