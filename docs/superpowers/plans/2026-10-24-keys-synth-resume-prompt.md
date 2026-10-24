# Resume prompt: keys synth, stopped during Task 13

Continue the keys synth implementation in `D:\MY DOCS\MY TOOLS\LAUNCHBAND\launchpad-drummer` on branch `keys-synth`.

The user explicitly stopped the previous session and requested this handoff. Resume implementation only when the user asks you to run this prompt. Work inline, in order, without subagents. Do not create another branch, push, or open a PR.

## Read first

1. `docs/superpowers/plans/2026-10-24-keys-synth-implementation-prompt.md` — original execution contract.
2. Section 0 of `docs/superpowers/plans/2026-10-24-keys-synth-plan.md` in full, including K21–K25.
3. The whole Task 13, then its touched files before editing. The plan wins over the spec; section 0 wins over tasks.
4. `docs/superpowers/specs/2026-10-24-keys-synth-design.md` for context as needed.

Apply the superpowers executing-plans and test-driven-development discipline and Ponytail full. The user requires inline work, overriding skill delegation suggestions. No redesign: existing console styling and the plan's component structure, classes, strings and tokens are the authority. Real plan conflicts require the user's decision; do not ask again about recorded decisions.

## Completed

Tasks 1–12 are complete and committed. DSP, FX, patches and set v2 migration, generated metadata, engine integration, commands and patch library, engine switch, LCXL mapping/LEDs, learned synth CCs and synth telemetry are implemented. Task 12 commit is `b818cd1` (`feat(app): LCXL synth knobs, learned synth CCs and the synth feed`). Its verification: 512 Rust tests pass, 2 debug timing tests ignored; 128 frontend tests pass; formatting and type checks pass with zero warnings.

Important recorded decisions:

- K21: retain `#[cfg_attr(debug_assertions, ignore)]`. Release timing tests run by name or without `--ignored` (that flag runs zero tests in release).
- K22: effects sleep only after their buffers/filter states drain; silent gaps before a delayed echo must not freeze effects.
- K23: the ping-pong second impulse with the specified 6 kHz feedback filter is approximately 0.272031; the test was corrected, not the DSP.
- K24: library filenames strictly reject `..`. A displayed patch named `A..B` saves as `A__B.lbpatch`.
- K25: user chose the supplied Task 13 solver and its passing size tests, accepting keybed/sequencer geometry changes. This overrides the task's preserve-place-and-size instruction. At 1920×1080 the row grows 64 px, keybed grows 7 px, sequencer shrinks 71 px.

Previous measured release checks: wavetable timing test passed in about 0.02 s; 10 s of worst-case synth rendering took 1.315 s (budget 2.5 s). Repeat the required final release checks in Task 19.

## Current checkpoint: Task 13 IN PROGRESS

A WIP checkpoint commit preserves the following implementation; it is not Task 13 completion:

- Replaced `src/lib/layout.ts` with the plan's complete solver verbatim.
- Rewrote `src/lib/layout.test.ts`: 11 prescribed layout tests. They first failed because the old solver lacked the new exports, and now all pass.
- Created `MixerSlim.svelte`, `ChannelPopover.svelte`, `SynthPlate.svelte`.
- Wired them into `App.svelte`; removed `MixerStrip.svelte` and its old rename/menu state. Kept other ContextMenu uses.
- Slim mixer has faders, mute/solo, channel chips and master; strip opens a fixed slim-mixer popover.
- Channel popover contains rename, pan/pitch/low-pass and the keys routing row; Escape/outside close.
- Synth shell shows patch name/modified marker, responsive page tabs with per-page-set localStorage, and SoundFont overlay with USE SYNTH.
- Recorded K25 in section 0.3.

The last implementation correction merged the mixer outside-pointer handler into App's existing `<svelte:window>` (Svelte permits only one) and made the bound mixer popover element `$state` to remove the checker warning.

Checkpoint checks: `cargo fmt --all -- --check` PASS; `npm test` PASS (131 tests); `npm run build` PASS, including `npm run check` (0 errors, 0 warnings). `cargo test --workspace` PASS (512 passed, 0 failed, 2 debug timing tests ignored).

Headless Edge browser preview rendered all three window sizes, 2560×1440, 1920×1080, 1280×720, with zero `pageerror` events. At 1280×720 the mixer is 40 px, synth 561×418; at 1920×1080 synth 599×666. The actual browser bay at 2560×1440 measured 2456×1348, differing from the plan's example bay 2455×1296; retain the supplied solver and fixed-input tests. Before/after screenshots and scripts exist locally under ignored `.verification/keys-synth-task13-*`. Screenshots were generated, but the after images and interactive controls still need inspection/testing. Do not claim the Task 13 visual or interaction review is complete.

## Next actions

1. Inspect current `git status`, checkpoint diff and files. Do not redo Tasks 1–12 or discard the Task 13 work.
2. Finish Task 13 browser verification at all three sizes: inspect screenshots; exercise faders, mute/solo, chip/popover knobs, rename Enter/Escape/blur, keys routing, strip expand/Escape/outside-close, responsive tabs and saved selection. Check focus, bounds and clipping. SoundFont overlay and USE SYNTH need appropriate verification; do not silently add preview APIs that Task 18 owns.
3. Run inline compliance and quality reviews against Task 13. Check the known pending interactions carefully, including a chip double-click while its first click opens a popover, Escape during rename, and nested channel/mixer popover behavior. These are review targets, not established failures. Fix actual defects with the smallest change.
4. Run all required checks: formatting, workspace Rust tests, npm tests, type checks and build. Read all results. Complete the Task 13 verification paragraph/checklists honestly, then commit `feat(ui): slim mixer, channel popover and the synth plate frame` with the required trailer.
5. Continue Tasks 14–18 sequentially, each with tests first, full checks and three-size previews. Then Task 19: final release performance, whole-diff review, spec status, hardware checklist, and mandatory listening pass with the user on all 16 factory patches. Ask and wait for the listening answers; do not tune by guess.

Task 18 likely has a prescribed test mismatch: it expects factory patches under all 8 category headings, but the 16 specified recipes cover 7 categories (no FX patch). Verify the actual test and recipes; prompt rule 2 permits the smallest test correction preserving specified behavior, documented in the task note. Do not invent a factory patch to populate an unused category.

## Environment and workflow

PowerShell on Windows. Before cargo: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"`. If stale duplicate `lpd_core` artifacts appear, follow section 0's targeted cargo clean instruction. No new dependencies are needed. Build outputs, `.verification`, user sets and secrets must remain uncommitted.

The dev server started by the previous session was stopped at wrap-up. Start `npm run dev` for preview at `http://127.0.0.1:1420`. Local browser scripts use headless Edge through a temporary npm-cached Playwright installation; discover its current path instead of relying on an old cache hash. No package dependency was added. Existing PadGrid runtime binding warnings and Vite target-file EBUSY watcher warnings remain; no new browser page exceptions were observed. No native app is intentionally left running by this work.

Audio thread: no allocations/frees/locks/I/O; boxes leave via trash, all new engine paths covered by no_alloc. Router owns the sole Set; synth edits are undoable/coalesced, mixer edits remain non-undoable. Lock order router → history → kit_manager → saver; emit and patch I/O outside router locks. Hardware events are mirrored, never re-sent by UI. Tauri JS arguments are camelCase.

Every task commit ends with a blank line and `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` as explicitly required by the original prompt. Append verification under that task and mark completion only after all its checks and reviews finish. Keep progress updates concise; proceed without asking whether to continue.

Hardware, real SF2 switching, listening, ASIO latency and the 30-minute session remain unverified. The user's Launchpad has a loose USB-B socket; do not misdiagnose cable hangs as code bugs.
