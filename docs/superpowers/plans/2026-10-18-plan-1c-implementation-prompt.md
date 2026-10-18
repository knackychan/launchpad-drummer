# Prompt: implement Plan 1C

Paste everything below the line into a fresh Claude Code session started in
`D:\MY DOCS\MY TOOLS\LAUNCHBAND\launchpad-drummer`.

---

You are implementing Plan 1C of the Launchpad Drummer app: a Tauri 2 + Svelte 5 + Rust workspace
(`crates/lpd-core` = engine, kit and device codecs; `src-tauri` = the app: router, lights, MIDI bridges,
Tauri commands; `src` = the Svelte UI). The Play and Games screens already match the design. Your job is
every remaining Part 1 feature, exactly as the plan describes it.

## The contract

`docs/superpowers/plans/2026-10-18-plan-1c-ui-completion.md` is the contract. It was written to be
exhaustive so the implementation cannot drift from the design:

- Section 0 holds the global rules, the decisions already made (D1–D10), the shared name registry
  (every Tauri command with its JS argument keys, every event with its payload, the set-file
  additions) and the mockup line map. Section 0 wins over a task; the plan wins over the specs.
- Each task fixes its files, Rust signatures, TS types, CSS classes, user-visible strings, behaviour,
  tests with expected values, a "Do not" list and a "Done when" checklist.

Rules that follow from that:

1. Use the names, strings, payload shapes and file paths in the plan verbatim. Do not invent new
   commands, events, fields, CSS classes or texts. When the code needs a helper the plan does not
   name, keep it private to its module.
2. Do not reinterpret the design. The look is done: new UI copies markup and CSS from
   `docs/mockups/ui-mockup-scifi.html` at the line ranges the task names; existing parts are not
   restyled.
3. When the plan is ambiguous, contradicts itself, contradicts the code, or cannot be implemented as
   written: stop that task, report the exact conflict to the user with the two or three options
   you see and your recommendation, and wait. Do not choose silently. When the user decides, add the
   decision to section 0.3 as a new `D` row in the same commit as the code.
4. Do not revisit decisions D1–D10.

## Read first (in this order)

1. The plan, section 0 in full, then the task you are on.
2. `docs/superpowers/specs/2026-10-17-ui-design.md` and `docs/superpowers/specs/2026-10-17-drum-sampler-design.md`
   for background; the plan already restates what each task needs.
3. The files the task modifies, before changing them.

## Current state

- Branch `ui-sci-retro-skin`. Commits: `b688b3d` (Play/Games UI rebuild), `236324c` (first plan
  draft), `10835e4` (Task 1: undo history, single set owner, humanize), then the commit that adds
  the exhaustive plan and this prompt.
- **Task 1 is implemented but not reviewed.** Start by running the spec-compliance review and the
  code-quality review for Task 1 against its plan text (base `236324c`, head `10835e4`), fix what
  they find, then continue with Task 2.
- Baseline before Task 1: `cargo test --workspace` green, `npm test` 23/23, `npm run check` 0 errors.

## Workflow

Use the `superpowers:subagent-driven-development` skill, task by task, in plan order (2 → 18). Do not
run implementers in parallel; tasks share files.

For each task:

1. **Implementer subagent.** Its prompt contains, pasted in full: plan section 0 (all of it) and the
   full text of the task. Add the current HEAD SHA and the names created by earlier tasks if the code
   deviates from the plan anywhere (from the reviews). Tell it to follow
   `superpowers:test-driven-development`, to run the verification commands in section 0.1, to
   commit with the message the task gives, and to report status DONE / DONE_WITH_CONCERNS /
   BLOCKED / NEEDS_CONTEXT with the test output's summary lines and the commit SHA.
2. **Spec-compliance reviewer subagent.** Gets the same task text plus section 0 and the
   implementer's report. It reads the code (not the report) and checks every contract, string,
   payload key, test case and "Do not" item, and flags anything extra. It returns ✅ or a list of
   issues with `file:line`.
3. Fix loop: the implementer (same agent, via SendMessage) fixes the issues; the reviewer reviews
   again, until ✅.
4. **Code-quality reviewer subagent** (only after ✅): base SHA = commit before the task, head = after.
   Checks correctness, audio-thread safety (no allocation or frees in the callback, `no_alloc.rs`
   covers new engine paths), lock order (`router` → `history` → `kit_manager` → `painter`, never
   `emit` while holding `router`), Tauri camelCase args, the hardware-hit invariant, error handling,
   test quality, and file focus. Fix loop until approved.
5. Mark the task done and go straight to the next one. Do not stop to ask whether to continue.

Models: the implementer and the code-quality reviewer on the most capable model; the spec reviewer may
use a standard model.

Stop only when: a task is BLOCKED and more context or a stronger model does not unblock it; the plan
is ambiguous or wrong (rule 3 above); a verification command fails and the cause is outside the task;
or all tasks are done.

## Invariants the reviews must enforce on every task

- The UI never calls `trigger_pad` for hardware-sourced events (`source: 'hardware'`, `pad-hit`,
  sequencer steps). Only on-screen input sends pad triggers.
- Tauri v2: JS passes only camelCase argument keys, never both spellings.
- One set owner (the router); kit edits go through `AppState::edit`; mixer values are not undoable;
  every change calls `touch()`.
- Browser preview (`npm run dev`, no `window.__TAURI__`) still renders and throws nothing.
- No restyling of working parts; `prefers-reduced-motion` rules from section 0.2 hold.

## Verification

Per task (section 0.1): `cargo fmt --all -- --check`, `cargo test --workspace`, `npm test`,
`npm run check`, plus `npm run build` for UI tasks. Prepend
`$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"` in PowerShell. Where a task asks for a preview
check, run `npm run dev` and look at `http://127.0.0.1:1420` at the sizes it names (browser tools if
available); report what you saw, including anything you could not check.

Hardware is not required for any task. Note: the user's Launchpad has a loose USB-B socket, so
hangs or lost input on the real device come from the cable, not the code.

## Git

Commit on `ui-sci-retro-skin`, one commit per task plus review fix-ups. Message format from section
0.1, ending with a blank line and `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Do not
push, do not open pull requests, do not create branches.

## When everything is done

After Task 18, dispatch one final code reviewer over `236324c..HEAD`, fix what it finds, then report
to the user: what shipped per task, test totals, every deviation from the plan (with the D row that
records it), what was not verified (real hardware, real Explorer drops, real-time audio feel), and
the hardware checklist the user should run.
