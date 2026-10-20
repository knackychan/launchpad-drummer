# Startup baseline and after-fix measurements

Date: 2026-10-19 (Asia/Taipei). Machine: the development PC (Windows 11, release builds, WebView2). Display scale 1.
Plan: [performance fixes](../superpowers/plans/2026-10-19-performance-fixes.md). Audit: [performance audit](../2026-10-19-performance-audit.md).

## Existing checks before any change (Task 0)

| Check | Result |
| --- | --- |
| `npm run build` (svelte-check + vite) | pass, 0 errors; one JS bundle 242.29 KB (88.15 KB gzip), CSS 44.11 KB |
| `npm test` | 105 pass, 0 fail |
| `cargo test --workspace` | 381 pass, 0 fail |

## Method

- **before**: the committed code (`git archive HEAD`, built with `npm run tauri build` in a separate folder). It lacks the uncommitted Horde edits, which do not affect startup.
- **after**: the working tree built with `run.bat build`.
- Both are release executables started **directly**; this measures the app, not the old launcher (which also ran Node, Vite and Cargo before the app existed; that wait is removed by Task 1 and is not in these numbers).
- A harness (`measure.mjs`, kept outside the repo) spawns the executable with a WebView2 remote-debugging port and a throw-away WebView profile, then polls the page every ~20 ms: time from process spawn to
  - **first page target** (WebView exists),
  - **content** (`#boot` fallback or the app root has children; DOM presence, **not** a paint measurement),
  - **set ready** (the set path appears in the status strip; in the old code this is when `get_startup` was answered).
- Isolation: the app's config folder cannot be redirected, so `config.json` was backed up, pointed at disposable copies of the Default Set under the session scratchpad, and restored afterwards (hash verified identical). The real Default Set was never opened.
- Kits: **small** = copy of the real Default Set (33 unique samples, ~1.8 MB). **heavy** = same set with every sample replaced by 15 s of stereo 16-bit 44.1 kHz audio (82 MB).
- 6 launches per cell, warm (files cached). The first launch of every series was slower (cold-ish disk cache) and is kept in the range. A true cold start (cleared cache/reboot) was **not** measured.
- Frontend marks `lpd:mounted` (~70–85 ms after navigation start) and `lpd:set-ready` (~215–260 ms) are recorded in the page. Mount is not paint.

## Results (milliseconds from process spawn)

| Series | content: median (range) | set ready: median (range) |
| --- | --- | --- |
| before, small kit | 539 (525–1254) | 575 (557–1307) |
| after, small kit | 476 (467–744) | 536 (529–804) |
| before, heavy kit | 718 (713–744) | 766 (748–791) |
| after, heavy kit | 480 (467–507) | 544 (530–588) |

Backend milestones from `lpd.log` (after): setup entered ≈ +285 ms after `run()` starts, configuration ready +0.4 ms later, audio running ≈ +407 ms, restore finished ≈ +496–505 ms for the heavy kit (restore ≈ 210 ms, on its own thread).

## What the numbers say

- Content now appears at about the same time whatever the kit size (≈ 470–480 ms) because decoding is off the setup thread. Before, content waited for the restore: +180 ms for the 82 MB kit on this machine.
- For the small default kit the gain is ≈ 60 ms; the audit's expectation that the default kit alone does not explain a long wait holds. The large saving for ordinary use comes from Task 1 (no dev toolchain on launch), which these direct-launch numbers deliberately exclude.
- Decoding is fast here (82 MB in ≈ 0.2 s), so slower storage or compressed samples would widen the gap. Not measured.
- "Set ready" after the change includes the extra round trip of fetching state once the ready event arrives; it is still below the old value for the heavy kit and ≈ 40 ms below for the small kit.

## Not measured

- Native cold start; visible-paint time in a native trace; slow storage; high-DPI/scale 2 native; real MIDI hardware; ASIO.
- Time spent before the process exists (launcher) and WebView creation beyond "first target".
