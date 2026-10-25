# Prompt: implement the keys synth plan

Paste everything below the line into a fresh Claude Code session started in
`D:\MY DOCS\MY TOOLS\LAUNCHBAND\launchpad-drummer`.

---

You are implementing the keys synth of the Launchpad Drummer app: a Tauri 2 + Svelte 5 + Rust workspace
(`crates/lpd-core` = engine, kit and device codecs; `src-tauri` = the app: router, lights, MIDI bridges,
Tauri commands; `src` = the Svelte UI). The app works today (Part 1 is complete). Your job is to replace
the built-in keys synth with a semi-modular wavetable synthesizer — engine, FX, patches, Launch Control XL
knobs, the synth plate on the PLAY screen, the slim mixer and the visualizers — exactly as the plan
describes it.

## The contract

`docs/superpowers/plans/2026-10-24-keys-synth-plan.md` is the contract. It was written to be exhaustive so
the implementation cannot drift from the design:

- Section 0 holds the environment and workflow (0.1), the invariants (0.2), the decisions made while
  planning (K1–K20, 0.3), the shared name registry (0.4: every Rust item, Tauri command with its JS
  argument keys, event with its payload, set-file and config change, TypeScript file and component) and
  the phase list (0.5). Section 0 wins over a task; the plan wins over the spec.
- Each task fixes its files, its contracts, its behaviour, the code of every DSP kernel and pure helper,
  its tests with expected values, a "Do not" list and a "Done when" checklist.
- The spec it implements is `docs/superpowers/specs/2026-10-24-keys-synth-design.md`; read it for the
  why, but follow the plan for the what.

Rules that follow from that:

1. Use the names, strings, payload shapes, ids and file paths in the plan verbatim. Do not invent new
   commands, events, fields, CSS classes or texts. When the code needs a helper the plan does not name,
   keep it private to its module.
2. Copy the plan's code where it gives code. If it does not compile or a test the plan gives fails, fix
   the smallest thing that makes it right, keep the plan's behaviour, and write what you changed in the
   task's verification paragraph.
3. When the plan is ambiguous, contradicts itself, contradicts the code, or cannot be implemented as
   written: stop that task, report the exact conflict to the user with the two or three options you see
   and your recommendation, and wait. Do not choose silently. When the user decides, add the decision to
   section 0.3 as a new `K` row in the same commit as the code.
4. Do not revisit decisions K1–K20 or the spec's section 1 decisions.

## How to work

The user wants the work done **inline in this session — do not use subagents, the Agent tool or
workflows.** Use the `superpowers:executing-plans` skill and go task by task in order (1 → 19); tasks
share files, so never run two at once.

For each task:

1. Read section 0 in full (once per session, and again if the conversation was summarized), then the
   whole task, then the files the task modifies before changing them.
2. Follow `superpowers:test-driven-development`: write the tests the task lists, run them and see them
   fail for the expected reason, implement, see them pass.
3. Run the verification of section 0.1 (`cargo fmt --all -- --check`, `cargo test --workspace`,
   `npm test`, `npm run check`, and `npm run build` for UI tasks). In PowerShell prepend
   `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"`. Release-only tests:
   `cargo test --release -p lpd-core -- --ignored` when the task says so.
4. Review your own work inline, in two passes, before committing:
   - **Spec compliance:** re-read the task and check every contract, id, string, payload key, test case
     and "Do not" item against the code you wrote (read the code, not your memory of it); flag anything
     extra.
   - **Code quality:** correctness, audio-thread safety (no allocation, lock, I/O or free inside
     `Engine::handle` / `render` and everything they call; `no_alloc.rs` covers every new engine path),
     lock order (`router` → `history` → `kit_manager` → `saver`; never `emit` while holding `router`;
     no lock across patch-file I/O), Tauri camelCase arguments, undo scope (0.2 item 3), error handling,
     test quality, and file focus.
   Fix what you find, rerun the verification.
5. Append a "Task N verification (date): …" paragraph under the task in the plan file (tests written
   first and seen failing, test totals, what you checked in the preview or the app, every deviation and
   why, what you could not verify), then commit with the message the task gives, ending with a blank
   line and `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
6. Go straight on to the next task. Do not stop to ask whether to continue.

Stop only when: the plan is ambiguous or wrong (rule 3); a verification command fails and the cause is
outside the task; Task 8's CPU budget test fails (report the measured time — the fallback is the user's
decision, spec 14); Task 19 needs the user's ears; or all tasks are done.

## Current state

- Branch `keys-synth` (created from `main` at `52b5af1`) holds the spec (`ba09b2b`). The spec
  correction made while planning, the plan and this prompt may still be uncommitted: if `git status`
  shows them, commit them first as `docs: keys synth implementation plan and prompt` (the three files
  only). Nothing of the plan is implemented yet.
- Before Task 1, run the full verification of section 0.1 on the branch as it is and write the totals
  down as the baseline (plan 1C left everything green). If anything is red before you start, report it
  and stop.
- The user's screen is 2560×1440 at 100 % scaling (the maximized app bay is about 2455×1296); check UI work
  at that size first, then 1920×1080 and 1280×720.

## Invariants the reviews must enforce on every task

- The audio thread never allocates, locks, frees or does I/O; boxes go back through the trash queue;
  wavetables are built off the audio thread.
- One set owner (the router); synth edits go through `AppState::edit` / `edit_at` / `edit_in`; synth
  edits are undoable, mixer values are not; every change calls `touch()`.
- Hardware-sourced events (Keystation notes, LCXL moves, learned CCs) are never re-sent by the UI; the UI
  only mirrors them (`keys-note`, `synth-params`, `mixer-changed`).
- Tauri v2: JS passes only camelCase argument keys.
- The keys plate and the sequencer keep their place and size; the telemetry column keeps its look;
  existing parts are not restyled.
- The browser preview (`npm run dev`, no `window.__TAURI__`) still renders and throws nothing.
- `prefers-reduced-motion`: synth displays, rings and meters update at 10 Hz.

## Preview and hardware

Where a task asks for a preview check, run `npm run dev` and look at `http://127.0.0.1:1420` at the sizes
it names (browser tools if available; otherwise a scripted headless browser as plan 1C did); report what
you saw, including anything you could not check. The desktop app runs with `npm run tauri dev` (or
`run.bat`); use it where a task says "desktop app" if you can, and say so when you cannot.

No hardware is needed for any task. The user's Launchpad has a loose USB-B socket: hangs or lost input on
the real device come from the cable, not the code. Hardware checks (Keystation, LCXL, ASIO latency, a
30-minute session) are listed for the user in Task 19.

## Git

Commit on `keys-synth`, one commit per task plus review fix-ups. Do not push, do not open pull requests,
do not create other branches. Never commit `.env`, build output or the user's sets.

## When everything is done

Task 19 is the finish: the release performance numbers, a listening pass with the user on the 16 factory
patches (ask, then wait for the answers), a whole-diff review of `main..HEAD` (inline), the spec status
section, and the final report: what shipped per task, test totals, performance, every deviation (with the
K row that records it), what was not verified, and the hardware checklist the user should run.
