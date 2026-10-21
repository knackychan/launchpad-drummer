# Chess and Xiangqi AI repairs — 2026-10-21

The current prototype entry points and `public/games` copies use the same rules and engine files. Changes were copied only after checking that the starting copies matched; the regression suite checks their equality. Historical HTML versions remain unchanged. No dependencies were added to the app. A concurrent prototype shared-menu update was preserved; its new storage lookup received the same narrow guard without copying its unrelated UI changes into the app.

| Issue from the repair brief | Resolution | Regression evidence |
|---|---|---|
| Main-thread search / unused worker handler | Pages create real Blob workers containing the already-loaded rules and engine. No worker imports or network requests. Progress and results carry the search ID. Reset, valid undo, mode/difficulty changes and page disposal terminate work. A watchdog and error handlers release control to two-player mode with a visible banner. | Browser tests: strongest-level progress, heartbeat, input lock, legal result, cancellation, stale IDs, empty-history undo, disposal, startup failure, worker search error, syntax error and retry. |
| Horizon mate, stalemate and check handling | Generate legal moves and adjudicate terminal positions before static evaluation. In check, search all evasions, including quiet moves; no stand-pat or delta pruning. Chess stalemate draws; Xiangqi stalemate loses. | Depth-zero/depth-one mate and stalemate fixtures, actual single quiet evasion in each game, synthetic capture/check continuation. Chess initial `perft(4)` remains **197281**. |
| Chess table orientation | White flips ranks (`sq ^ 56` equivalent); Black indexes directly, for both phase tables. Comments corrected. | Advanced-pawn fixture, explicit MG/EG square assertions, color-swapped vertical symmetry. |
| Recursive deadlines / incomplete results | Every recursive node, including quiescence continuations, checks the deadline. An interruption unwinds with `finally` and discards the unfinished root iteration. Return the last complete iteration or a legal depth-zero fallback. | Opening and tactical deadline fixtures at 0/20/80 ms, quiescence visitation assertions, deterministic scripted-clock interruption and completed-result comparisons. |
| Repetition, counters and cache safety | Rules and draw adjudication are shared with the UI. Search appends/restores canonical text position keys along each branch, including root children and quiescence. Chess en-passant identity includes only a legally available capture. Cache stores ordering moves only, never reusable scores. | Root repetition, search-created repetition, branch restoration after exceptions, near-counter root moves, identical boards with different histories, actual Xiangqi checking repetition. |
| Unsupported evaluation / strength claims | Five unrated settings: Easy, Relaxed, Standard, Hard, Strongest. Removed Elo/master/calibration claims and nonexistent dynamic Xiangqi evaluation claims. | Implemented behavior described below; no rating inferred from timing or depth. |

Two pre-existing integration faults surfaced in browser tests and were repaired: menu script loading after its initializer, and an unguarded menu `localStorage` read that threw inside the app's `sandbox="allow-scripts"` iframe. The shared menu now uses its configured defaults when storage is denied. Tauri production and development CSPs explicitly permit Blob workers, without changing iframe permissions.

## Implemented evaluation and draw rules

Chess evaluates material plus PeSTO middlegame/endgame piece-square tables, blended by material phase. It does not implement separate pawn-structure, bishop-pair or king-shield terms. Xiangqi evaluates material and piece-square tables, with central-cannon and friendly-pawn-free chariot-file bonuses. Horse-leg and elephant-eye restrictions are move rules, not separate dynamic evaluation penalties.

Both UIs automatically adjudicate their draw thresholds; search follows the same policy. Chess uses threefold repetition, 100 half-moves, and the existing insufficient-material detector. Xiangqi deliberately retains **simplified threefold repetition as a draw, including repeated checks**, 120 half-moves without capture, and no attacking pieces. This is **not** a tournament perpetual-check/chase adjudicator. No claim of full Xiangqi federation rules is made.

To bound exceptional continuations, search caps lines at 96 plies. A nonchecked position at that cap uses static evaluation after terminal/draw checks. A checked position aborts the iteration, retaining the previous completed result. The cache holds ordering hints only; this sacrifices score-cache speed to avoid history-dependent score reuse. Search deadlines are scheduling targets, not hard real-time guarantees: one move-generation call can run between checks.

## Reproduce

From the workspace root (the directory containing `prototypes` and `launchpad-drummer`):

```powershell
node --test test-chess-ai.js
node test-menu-system.js
npm --prefix launchpad-drummer test
npm --prefix launchpad-drummer run build
python test-chess-browser.py
```

The engine suite uses Node's built-in test runner and VM, with no new packages. The optional browser suite uses the environment's existing Python Playwright and installed Microsoft Edge. Run the build first: it tests the production `dist` app as well as source copies. It starts local HTTP servers with the configured production CSP, opens both games in the actual sandboxed app frame, and tests prototype/app source pages over HTTP and direct `file:///` with browser networking disabled. Browser tests do not connect a physical MIDI device.

## Results and limits

Measured on Windows x64, Intel Core i9-13980HX, Node **v26.2.0**, headless Microsoft Edge **155.0.4283.62**. Engine regression suite: **19 passed**. Existing app suite: **127 passed**. Menu suite passed, including denied-storage fallback. Production build and Svelte checks passed with **0 errors / 0 warnings**.

Source browser matrix: both games × prototype/app copy × HTTP/offline-file = **8 passing combinations**. The production build also ran both games inside the app's existing opaque-origin sandbox under its production CSP, including worker error/retry paths, with no uncaught page errors. After the concurrent prototype menu update, the four prototype HTTP/offline cases were rechecked successfully through the Keyboard & Mouse Start flow.

In a production-frame initial-position search at the Strongest setting's 2500 ms budget, Chess completed depth 6 with 93,640 nodes; Xiangqi completed depth 4 with 44,787 nodes. Both reported 2500 ms. The source-browser matrix's 20 ms UI heartbeat had maximum gaps around 48–233 ms across the recorded runs. A later production-frame run completed Chess depth 5 / 37,346 nodes and Xiangqi depth 4 / 31,280 nodes at the same budget, illustrating scheduling/load variation. These are responsiveness/search measurements, **not** playing-strength ratings or promises of performance on other hardware. Node timing fixtures permit budget + 250 ms to accommodate scheduling; recorded 20/80 ms fixtures were approximately 19–20 / 79–80 ms.

Not performed: native Tauri/WebView2 runtime launch, physical Launchpad operation, listening/audio-device checks, exhaustive move-generator certification, rated matches or Elo calibration. Presentation/audio/MIDI code was preserved apart from the described menu integration repairs; browser and synthetic tests do not establish physical hardware behavior.
