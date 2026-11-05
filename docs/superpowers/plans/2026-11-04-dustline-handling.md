# Dustline driving feel

Active request: tune the current 0.12 inline, without subagents or commits. This supersedes the older handling/script-preservation restrictions for the active release only. Preserve archived releases, controller acquisition/reconnect, app routing, bindings and reactive LEDs, and unrelated uncommitted work.

- [x] Record the unmodified source and repeatable acceleration/lift, steering at several speeds, sustained turn, reversal, brake–turn–accelerate, unequal-thrust, boost and damage measurements.
- [x] Tune the existing mixer and shared simulation; compare against the baseline before iterating. Check input timing, keyboard consistency and rival behavior.
- [x] Align engine/camera feedback with actual control and motion; keep the road and timing events readable.
- [x] Run focused handling, existing simulation/MIDI/audio/browser and app integration checks. Keep the standalone, versioned and app files byte-identical CRLF.
- [x] Document measured changes, remaining limitations and a physical LCXL playtest checklist in the Dustline README.

Scope: simulation script 2, mixer script 6, game/input/render script 7, MIDI script 3 only if timestamp handling requires it, related tests and documentation, and the two release copies. Route/assets/audio synthesis and archived releases stay intact. No new dependency or feature menu. Physical feel remains subject to hardware playtesting.

Implemented: one-step differential dead zone and progressive gain, stronger/faster yaw with reduced release overshoot, a shared square-root power curve and slower coast, simulation-clock keyboard ramps, timestamped input edges, accurate engine/brake visuals and restrained speed-dependent camera feedback. Initial tuning passed circuit/rival checks; the follow-up corrected input timing and feedback wiring. No extra smoothing or assistance was added.

Evidence: baseline/current JSON and the repeatable runner in `prototypes/Dustline/checks/handling-*`; 84 Node checks; handling/timing/host/audio browser checks; 168 app tests, clean Svelte check/build and production CSP smoke test. All five rivals finish three laps across six seeds with no recovery loops or large steering reversals. The timing-browser fixture now resets existing presentation hit-stop between independent cases. Hardware latency and subjective feel are not established by these results. Rust/native build and offline audition were not repeated because their implementations are unchanged.

Final three-copy SHA-256: `4d3694b391bf2b8f0db6f4b19c992b7ba4d10ccb8c010707f7468d5509090e12`. The pre-tune source is retained in `.verification/dustline-handling/index-before.html`; the source README has parameter changes, measurements and the physical checklist. No commits or subagents.

Launcher follow-up: the user opened `run.bat` and saw no DUSTLINE tab. It was launching the November 3 release executable; the prior web build had not updated that embedded bundle. Ran `build.bat` successfully (Svelte check: zero errors/warnings; frontend and optimized native release passed). `target/release/launchpad-drummer.exe` now contains the current app and Dustline. No launcher logic change was needed; documented build-versus-launch behavior in the README. Native menu reopening and physical-controller testing were not performed in this follow-up.
