Review the Dustline ship engine spec, then write its implementation plan. Do not implement anything.

Spec: `D:\MY DOCS\MY TOOLS\LAUNCHBAND\launchpad-drummer\docs\superpowers\specs\2026-11-05-dustline-ship-engine-design.md`
It was approved section by section in a brainstorming session on 2026-11-05. Its decisions (section 1) are mine:
both backends (BAKED synth loops and LIVE SYNTH via a dedicated second Rust `Synth`), F1 gears plus boost lift,
one ship (Kestrel) plus the profile format, all propulsion replaced, rivals and airflow/strain/misfire/vent kept.
Do not change a decision without asking me.

Read first:
- the spec, in full
- `prototypes/Dustline/README.md` (sections "Engine audio" and "Verification and rebuilding")
- the engine block in `prototypes/Dustline/index.html` between `/* @dustline-engine-audio:begin */` and `end */`,
  and the game wiring between `// @dustline-game-audio:begin` and `end` (`fillAudioFrame`, `updateAudio`)
- `prototypes/Dustline/generate-sfx.py`, `apply-sfx.py`, `checks/audio-engine.test.cjs`, `checks/audio-browser.py`,
  `checks/audio_audition.py`
- `launchpad-drummer/crates/lpd-core/src/synth/` (`mod.rs`, `params.rs`, `voice.rs`, `patch.rs`),
  `crates/lpd-core/src/audio/engine.rs`, `src-tauri/src/commands.rs`, `state.rs`, `lib.rs`, and the `lcxl-leds`
  handler in `src/App.svelte`
- the earshot skill: `D:\MY DOCS\MY TOOLS\earshot\skills\earshot\SKILL.md`
- the existing plan `launchpad-drummer/docs/superpowers/plans/2026-10-24-keys-synth-plan.md`, as a model for task
  size and decision rows

Step 1: review the spec against the code. Check every assumption it makes and report each problem with
file:line evidence. At least verify these:
- `Synth` pitch bend: range −1…1, `voice.bendRange` max 24, whether bend and the WHEEL source are smoothed, and
  whether MONO/LEGATO holds one note cleanly while bend moves.
- The patch fields used in spec section 3.2 exist with those ranges: sub at oct 0, OSC B `toFilter` off, SYNC
  warp, WHEEL routes to warp, cutoff, drive and OSC B level.
- A second `Synth` can be built off the audio thread, boxed and swapped in through the engine `Command` enum and
  the `Trash` queue without allocating on the audio thread. Check where the master bus and any limiter are in
  `engine.rs`.
- The patch file format, where the user's Keys patch folder is, and how to find "Dustline Kestrel" in it.
- An offline render from `crates/lpd-core/examples/` needs no new dependencies (a WAV writer can be hand-written).
- ffmpeg with libopus is installed, and WebView2/Chromium `decodeAudioData` decodes Ogg Opus gaplessly.
- Which of the eight inline scripts hold the engine block and the game wiring. The spec says scripts 0–5 stay
  byte-identical; confirm the engine block is not inside them.
- How `audio-engine.test.cjs` loads the engine block, how `apply-sfx.py` embeds the bundle, how many deterministic
  checks there are now (the spec says 129), and the exact commands for the 8 browser harnesses and
  `ai-pace.cjs --verify`.
- The gear numbers against the real simulation: `sim.state.speed` units, 600 km/h cruise, 1200 km/h cap,
  acceleration from rest.
- The earshot Python API names the spec relies on (`rank`, `match`, `find`, `describe`, `measure`, `realness`,
  `timeline`, `ab`), and that its venv runs: `"D:\MY DOCS\MY TOOLS\earshot\.venv\Scripts\python.exe" -m earshot doctor`.

Fix factual errors in the spec directly, and list each fix. For anything that is a choice rather than a fact,
ask me, one question at a time, before you change it.

Step 2: write the plan with the superpowers:writing-plans skill, to
`launchpad-drummer/docs/superpowers/plans/2026-11-05-dustline-ship-engine-plan.md`:
- Follow the spec's work order: phase 1 (gear model, profile format, patch, bake tool, BAKED with synth loops
  only), then checkpoint 1; phase 2 (ElevenLabs generation, earshot screening, textures, full BAKED), then
  checkpoint 2; phase 3 (native bridge, LIVE SYNTH, A/B switch, exe rebuild).
- Small tasks, each with the exact files, the code for every non-trivial piece (gear model, `GameEngineVoice`,
  bridge validator, crossfade logic, the patch JSON), the test written first, the command to run it and the
  expected output.
- A decisions table (rows S1, S2, …) for every choice the plan makes beyond the spec.
- At each checkpoint, stop and wait for my listening verdict. Do not continue past it on your own.
- The ElevenLabs generation task states the number of generations and runs only when I say go.

Constraints:
- Work inline. No subagents or workflows.
- No git commits unless I ask.
- Never print or copy API keys. The ElevenLabs key is in `D:\MY DOCS\MY TOOLS\LAUNCHBAND\.env`; scripts read it
  from there.
- Dustline has three byte-identical CRLF copies: `prototypes/Dustline/index.html`, the versioned
  `dustline-rift-run-v0.1x.html`, and `launchpad-drummer/public/games/dustline/index.html`. Another session may
  edit one of them. Before any write, compare all three hashes; if one changed, diff and merge, never overwrite.
  Git Bash `sed -i` converts CRLF to LF, so the plan must use byte-preserving edits.
- Run cargo from PowerShell with `$env:PATH = "$env:USERPROFILE\.cargo\bin;" + $env:PATH`. If cargo reports
  "multiple different versions of crate `lpd_core`", run `cargo clean -p lpd-core -p launchpad-drummer`.
- Lock order in `src-tauri/src/state.rs`: router → history → kit_manager → saver.
- Never say the engine sounds good. Report earshot numbers and measurements; my listening decides.

When you finish, give me a short list of the spec fixes you made, the questions I answered, and the plan's path
and task count.
