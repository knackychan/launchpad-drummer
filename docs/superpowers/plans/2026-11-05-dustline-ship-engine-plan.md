# Dustline Ship Engine (engine audio 2.0) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task, **inline in one session** (the user asked for no subagents and no workflows). Steps use checkbox (`- [ ]`) syntax for tracking. **Stop at CHECKPOINT 1 (after Task 9) and at CHECKPOINT 2 (after Task 16)** and wait for the user's listening verdict. **Task 11 (42 ElevenLabs generations) runs only when the user says go.** No git commits unless the user asks.

**Goal:** Replace Dustline's procedural propulsion sound with engine audio 2.0 and ship it as Dustline 0.13: a seven-gear model whose pitch follows speed, a tone from a Keys synth patch played either as baked loops in the game (BAKED) or live on the app's native synth (LIVE SYNTH), and ElevenLabs texture loops and one-shots.

**Architecture:** The game (`prototypes/Dustline`, three byte-identical CRLF copies) keeps all of it in inline script 7. The engine block gains a ship profile (`DustlineEngineAudio.ships.kestrel`), a pure gear model (`DustlineEngineAudio.gears`), a BAKED player for eight phase-locked synth loops per engine (rendered offline by the app's own Rust synth through the `bake_engine` example), texture loops and one-shots that the game decodes and hands over with `attach()`, and a LIVE sink that streams `{on, bend, wheel, gain}` frames. In the app (`launchpad-drummer`), `src/lib/engineBridge.ts` validates and rate-caps those frames from the Dustline frame, Tauri commands pass them through the router, and a private `GameEngineVoice` (a second `Synth`) plays them on the master bus. Every change to the game goes through byte-preserving step scripts that check the three copies' recorded hash first.

**Tech Stack:** Vanilla JS and Web Audio (the game); Python 3.14 with numpy and Playwright driving headless Microsoft Edge (tools and browser harnesses); Node 26 (`checks/*.test.cjs`, `node --test`); Rust (lpd-core, Tauri 2; WAV through the existing `hound` dev-dependency); Svelte 5 and TypeScript; ffmpeg with libopus; earshot (listening measurements); the ElevenLabs sound-generation API. No new crates or npm packages.

**Spec:** `../specs/2026-11-05-dustline-ship-engine-design.md` (the "spec" below; "spec 3.1" means its section 3.1). It was corrected during planning; section 0.3 lists the corrections.

**How to read this plan.** Section 0 holds the environment and workflow, the invariants, the spec fixes, the decisions made while planning (S1–S54), the shared names, the commands the tasks use and the task map. Tasks 1–24 follow in execution order, in three phases. Every new file is given in full ("Create"). Every change to an existing file other than the game copies is an exact Edit: replace the first text with the second. Each "Replace" text occurs exactly once in its file at that moment, so apply the Edits of a file in the order given. The game copies are changed only by the scripts this plan creates. All paths are relative to `D:\MY DOCS\MY TOOLS\LAUNCHBAND` unless a command changes directory. Expected outputs that depend on rendered or generated audio are the measurements of the planning runs on scratch copies; they may differ slightly, and each task says which differences matter.

---

## 0. Global rules, decisions and shared names

### 0.1 Environment and workflow

- Root: `D:\MY DOCS\MY TOOLS\LAUNCHBAND`. `prototypes/` is not a git repository. `launchpad-drummer/` is git (branch `main`, `core.autocrlf=true`, so its working-tree files are CRLF). It already holds unrelated uncommitted changes under `dist/`, `public/` and `target/`; leave them alone. **No commits, branches or pushes** unless the user asks.
- **Inline execution only**: no subagents and no workflows.
- Shells: Git Bash (the Bash tool) for `python`, `node` and `npm`; PowerShell for cargo, after `$env:PATH = "$env:USERPROFILE\.cargo\bin;" + $env:PATH` (Git Bash has no cargo). If cargo reports "multiple different versions of crate `lpd_core`", run `cargo clean -p lpd-core -p launchpad-drummer` and repeat the command.
- Python: the system `python` (3.14, with numpy and Playwright; the harnesses launch headless Microsoft Edge with `channel="msedge"`) runs every tool except `checks/ship_screen.py`, which runs in the earshot venv. In Git Bash: `PY="/d/MY DOCS/MY TOOLS/earshot/.venv/Scripts/python.exe"`, then `"$PY" -I prototypes/Dustline/checks/ship_screen.py …`. earshot's notes are in `D:\MY DOCS\MY TOOLS\earshot\skills\earshot\SKILL.md`. earshot's `measure` reports no f0 below 50 Hz (a 48 Hz loop reads 50), so pitch comes from `checks/pitch.py`.
- ffmpeg and ffprobe are on the PATH (`D:\MY DOCS\MY TOOLS\FFMPEG\bin`).
- **The ElevenLabs key** is in `D:\MY DOCS\MY TOOLS\LAUNCHBAND\.env` (`ELEVENLABS_API_KEY=`). Only `generate-ship-sfx.py --go` reads it. Never print, copy, echo or open that file, and never put the key on a command line.
- **Line endings.** Keep every file's line endings as they are. The game copies, the Dustline README, some Dustline checks and the whole launchpad-drummer working tree are CRLF; the other Dustline checks are LF (each task's Files list says which). Never use Git Bash `sed -i`: it turns CRLF into LF. Make every Edit with the Edit tool (it keeps the file's line endings) and run the line-ending check (0.6) on the file afterwards. The Write tool writes LF, both when it creates a file and when it overwrites one, so every file this plan creates is LF (Python, Node, rustc, rustfmt and serde all read it). The one CRLF file a task replaces whole, `apply-sfx.py` (Task 7), goes back to CRLF with the CRLF restore command (0.6) right after the Write.
- **The three game copies** (from Task 1 on): `prototypes/Dustline/index.html`, `prototypes/Dustline/dustline-rift-run-v0.13.html` and `launchpad-drummer/public/games/dustline/index.html`, byte-identical, uniform CRLF. Another session may edit one of them at any time. Never touch them with the Edit or Write tool. Only these write them: `tools/start_013.py` (once, Task 1), the step scripts `tools/engine2/step*.py`, `tools/engine2/profile_assets.py`, `tools/engine2/profile_patch.py`, `tools/engine2/live_trim.py` and `apply-sfx.py`. Each first checks that the three copies equal the hash recorded in `prototypes/Dustline/tools/copies.sha256` and stops otherwise. When one stops with "Changed since the last recorded write" or "Refusing to overwrite a copy changed elsewhere": run `python prototypes/Dustline/tools/sync_copies.py diff`, merge the other session's change into `index.html` by hand (never overwrite it), run `python prototypes/Dustline/tools/sync_copies.py push`, tell the user what you merged, then rerun the script that stopped.
- **Checkpoints.** After Task 9 and after Task 16, stop: report the measurements and the files to listen to, then wait for the user's listening verdict. Do not continue on your own. If the verdict asks for changes, make them (asking first where they touch a decision) before going on.
- **Task 11** spends the user's ElevenLabs credits (42 generations). It runs only when the user says go.
- **Reporting.** Never say the engine sounds good (or bad). Report earshot numbers and measurements; the user's listening decides.
- **Decisions.** Do not change a decision in 0.3 or 0.4 without asking the user. If this plan and the code disagree, stop and ask.
- **Memory.** The user's memory notes live in `C:\Users\DEROK137\.claude\projects\d--MY-DOCS-MY-TOOLS-LAUNCHBAND\memory\`. `dustline-ship-engine-spec.md` and `dustline-three-copies.md` concern this work; Task 24 updates them.

### 0.2 Invariants (never break)

1. The three game copies stay byte-identical and uniform CRLF, and only the scripts named in 0.1 write them.
2. Inline scripts 0–6 of the page stay byte-identical to 0.12; the engine and its game wiring live in script 7. The output stage (`createMaster`: glue compressor −11 dB 3.8:1, limiter −1.5 dB 20:1, master gain 0.75) does not change; `timing-events.test.cjs` compares its source with 0.12 (S39).
3. The engine never decodes audio: the game decodes the ship's assets and hands each buffer over with `attach()` (S11). The engine uses no `Math.random` and never draws from the simulation's random stream; variation comes from its private seeded generator.
4. The engine builds its nodes once (213 nodes with 60 sources with the eight loops of phase 1; 222 with 66 once all 22 assets are in, Task 15; plus at most six one-shot voices) and per frame only writes parameters. Tone revision R1 adds 69 nodes and 20 sources per engine: 351 with 100 in phase 1, 360 with 106 from Task 15.
5. Rust audio thread: no allocation, no lock, no I/O and no free in `Engine::handle`, `Engine::render` and everything they call, the game voice included. Boxes leave through the trash queue (S20). `crates/lpd-core/tests/no_alloc.rs` covers the game voice's paths.
6. The game voice is a second, private `Synth`. It never touches the Keys synth's patch, routing or wheels.
7. Lock order in `src-tauri`: `router` → `history` → `kit_manager` → `saver`. `AppState::game_engine_start` builds the voice with no lock held.
8. Tauri v2 arguments: the JS side passes camelCase keys (here the single words `patch`, `on`, `bend`, `wheel`, `gain`); Rust parameters are snake_case.
9. No Rust test reads `prototypes/` (S44).
10. The archives `dustline-rift-run-v0.12.html` and older stay untouched; `timing-events.test.cjs` checks their hashes.
11. Measurements only: the user's listening decides. ElevenLabs runs only on the user's go.

### 0.3 Spec fixes made while planning

The spec was checked against the code on 2026-11-05 and 2026-11-06 and corrected in place (its line 5 points to this table). Line numbers refer to the code at that date.

| # | Fix |
|---|---|
| 1 | The patch file is `.lbpatch` JSON `{patchVersion:1,name,category,params}`, read by `Patch::from_json`, which clamps every value. PLAY lists only the factory patches and `Documents\Launchpad Drummer\Patches` (`commands.rs:255–260`). |
| 2 | Patch names are stored in capitals: `DUSTLINE KESTREL.lbpatch` (`patchName` `'DUSTLINE KESTREL'`). |
| 3 | The held note is 48, not 36. SUB is limited to oct −1 (`params.rs:891`), and OSC B at +36 would hit the voice's ±48-semitone limit (`voice.rs:328–329`). Note 48 sounds 2f, so the bend is measured from 65.41 Hz. |
| 4 | Patch layers: SUB oct −1 (was 0); LEGATO (was "mono/legato"); the filter's drive has no type, so the drive is FX DRIVE TUBE; OSC B `toFilter` off. |
| 5 | `createMaster(ctx)` becomes `createMaster(ctx, dest)`. |
| 6 | The SFX bundle holds 29 sounds, not 28. |
| 7 | The app engine has no limiter. The voice is added before the master gain and the soft clipper (`engine.rs:458–467`). |
| 8 | Smoothing happens per 16-sample synth tick, not per audio block (`env.rs:8`, `voice.rs:325–329`, `mod.rs:464–470`). The test reads "5 cents per tick". |
| 9 | Bake: render 3.25 s and drop the first 1.0 s, so every loop is phase-aligned. The seam crossfade depends on the measured correlation; anchor and load crossfades are equal-gain. WAV is written through the `hound` dev-dependency, with no new dependency. |
| 10 | Opus decodes gaplessly (verified in Edge 156). Loop points are set in seconds, and seams are checked at 48 and 44.1 kHz. |
| 11 | GO sets the gear from speed (4th at full throttle; `LAUNCH_SPEED` 68 m/s × throttle, `index.html:1739`). GO is not a shift. |
| 12 | Gear-from-speed rule rewritten. The old rule gave 7th at 432 km/h. |
| 13 | Load: frames carry fader positions, and thrust = min(1, fader/0.75) (`index.html:1174`, `:1181`). |
| 14 | Frame-rate test: the same gear sequence, with shift times within 1/120 s. Bit-identical results are not possible. |
| 15 | Regression: 131 checks in 10 files (was 129). Scripts 0–6 stay identical (was 0–5); script 7 holds both the engine and the wiring. ai-pace runs from `prototypes/Dustline`. |
| 16 | Audition frames come from the real 0.12 simulation (`audio_scenarios.py` uses the 0.11 speed law). Launch goes from 245 to 600 km/h. |
| 17 | `replace_audio` resends only the mixer, keys, patch and wheels (`router.rs:342–350`), so the voice must be rebuilt after a stream restart. `router.rs` and `no_alloc.rs` were added to the files table. |
| 18 | The explanation of the 0.22 s minimum shift time is corrected. |
| 19 | The pitch target is measured against the gear model's `f`, pacing included. |
| 20 | LIVE frames arrive once per engine control step on whole display frames: 30 Hz at a 60 Hz display, 36 Hz at 144 Hz, 40 Hz offline (`CTRL` = 1/40 s). `game.rs` therefore glides bend and wheel over 40 ms (`GAME_GLIDE_S`), longer than the longest gap between frames, while the gain keeps its 25 ms ramp so shift cuts stay crisp (S49; the user may veto). |

**Question answered during planning: gear pacing → paced sweeps.** `gears.pace` is 250 km/h per second, so the launch upshifts come 0.19, 0.54 and 0.93 s after GO. Recorded in spec 2.2 and 3.1.

### 0.4 Decisions made while planning (do not revisit without the user)

| # | Decision |
|---|---|
| S1 | **Paced sweeps.** In gears 1–6 the shift rules and the pitch read a paced speed `vp` that rises at most `gears.pace` = 250 km/h per second and falls with the real speed at once. 7th and the boost lift read the real speed. `pace` is a profile value, tunable after listening. |
| S2 | **GO.** On the first racing frame the gear is set from speed (S3): 4th at full throttle, since the launch gives 68 m/s × throttle ≈ 245 km/h. GO is not a shift: no shift event, no cut, no blip, and `upMin` does not hold back the first upshift. The launch one-shot marks GO (from Task 14). On the grid (menu, countdown) the pitch follows the throttle: `idleHz + (redlineHz − idleHz) × 0.8 × thr^0.8`, eased with a 0.15 s time constant. |
| S3 | **Gear from speed:** the highest gear whose downshift point lies below v: `g = 1; while (g < 7 && v > 0.92 × tops[g−1]) g++`. Neither shift rule fires at once afterwards. Used at GO and during recovery. |
| S4 | **Load.** Thrust per side = min(1, fader / 0.75) (0.75 is the fader's 0 dB mark; the boost zone adds no load) × (1 − 0.45 × brake). Load eases up over 55 ms and down over 110 ms (`loadStep`). Below a load of 0.25 the tone moves to the off-load loops (S15). |
| S5 | **Substeps.** The gear model advances in fixed 1/120 s steps (`GEAR_DT`), interpolating the speed linearly across each update, and records at most 4 events per update in a preallocated array. Shift times agree within 1/120 s at 30, 60 and 144 Hz updates (fix 14). |
| S6 | **Shift shaping.** BAKED schedules it as AudioParam automation at the event time: an upshift cuts the tone by `mix.cutDb` (−9 dB) for 40 ms, glides the pitch down from the old value over 60 ms and blips the zing (S7); a downshift blips the throttle by `mix.blipDb` (+3 dB) over 80 ms with a 40 ms glide. LIVE carries the same shapes inside its frames (S16). |
| S7 | **Zing blip:** a peaking filter at 8 × f, Q 1.4, `mix.zingBlipDb` (+6 dB) on each upshift. LIVE adds 0.15 to the wheel for the blip instead. |
| S8 | **`propMix`.** The ship's propulsion (the tone voices; from Task 14 also the textures and one-shots) joins the 1.2.0 bus at the cabin stage, after the two 46 Hz high-passes, so the 41–48 Hz fundamental survives. Cabin and view shaping, cue space, saturation, duck, level and the master stage still apply. R1: the restored 1.2.0 layers go through their own two 46 Hz high-passes, then join each engine at its shift and reach `propMix` with the tone (R1-4). |
| S9 | **Strain exciter.** The 1.2.0 strain resonators ring from a pulse wave at the fundamental, which reaches the resonators only and is never heard directly. |
| S10 | Phase 1 keeps the 1.2.0 afterburner (ignition, burn, sputter, release) and its controls. Step 5 (Task 14) removes them when the one-shots and the jet textures arrive. |
| S11 | **`attach(id, buffer)`** returns true when the engine takes the buffer, and false for an id the profile does not name or one already attached. The tone banks are built once all eight baked loops are there; their 16 sources (8 per engine) start on the same frame at their loop start, so loops rendered in phase stay in phase. Textures start as they arrive; one-shots are kept for their events. |
| S12 | **Ship bundle:** `const DUSTLINE_SHIP_B64 = {…};` on one line beside `DUSTLINE_SFX_B64`, keys `kestrel:<id>`, values base64 Ogg Opus. Step 4 adds it empty; only `apply-sfx.py` fills it. |
| S13 | **Loop layout** at 48 kHz: baked loops 2400 lead-in + 96000 loop + 2400 tail samples (100800); textures 2400 + 144000 + 2400. The tail repeats the loop start, so the seam plays continuous audio. `loopStart` and `loopEnd` are stored in samples and set on the source in seconds (sample / 48000), which holds at a 44.1 kHz context too. |
| S14 | **Seam crossfade:** the last 150 ms (7200 samples) of the loop is crossfaded into the 150 ms before the loop start with gains g·cos θ and g·sin θ, g = 1/√(cos²θ + sin²θ + 2ρ·cos θ·sin θ), where ρ is the measured correlation of the two stretches (textures clamp ρ at −0.5). |
| S15 | **Tone mix.** The loops are phase-locked, so every crossfade is equal-gain: linear in log frequency between the two nearest anchors (48, 75, 117, 182 Hz) and a smoothstep on load. Every loop of an engine plays at `playbackRate = f / anchor`; `clampRate` limits f to [0.05 × 182, 4 × 48] Hz. |
| S16 | **LIVE frames**, one per control step: `bend = 12 × log2(f / 65.41) / 24` (the shift glide inside f); `wheel = 0.15 + 0.6 × max(loadL, loadR) + 0.25 × boost` (+0.15 during the zing blip); `gain = master × mute × pause × tone layer × mix.toneDb × mix.liveTrimDb × duck × shift shape × misfire dip × sin(modeMix × π/2)`. Bend is clamped to −1…1, wheel and gain to 0…1. |
| S17 | **BAKED ↔ LIVE:** the frame's `engineMode` moves `modeMix` between 0 and 1 over 0.15 s; the baked tone follows cos(modeMix × π/2) and the LIVE gain sin(modeMix × π/2) (equal power). Textures and one-shots play in both modes. |
| S18 | **Voice constants** (`game.rs`): note 48, velocity 127, bend range 24 and LEGATO forced, gain ramp 25 ms, bend and wheel glide 40 ms (S49), watchdog 250 ms, fade 50 ms, `GAME_ENGINES_GAIN` 2.0 (S50), all stepped per 16-sample tick (the gain also moves per sample inside a tick). |
| S19 | **Frame flags.** `on: false` fades the voice over one ramp (25 ms) and releases the note. With no frame for 250 ms it fades over 50 ms and releases; the next frame restarts the note. `stop()` fades over 50 ms, then the voice reports finished and ignores later frames. |
| S20 | **Engine commands:** `Command::GameSynthLoad(Box<GameEngineVoice>)`, `Command::GameSynthFrame { on, bend, wheel, gain }`, `Command::GameSynthClear`. A replaced or cleared voice leaves through `Trash::Game`; if the trash queue is full, the box is leaked with `mem::forget` rather than dropped on the audio thread. The voice renders into the engine's `game_l` / `game_r` buffers and is added to the master bus before the master gain and the soft clipper. |
| S21 | **Stream restart.** The router keeps `GameSpec { params, source, name, generation }`, because `replace_audio` does not resend the voice (fix 17). `AppState::restart_game_engine()`, called in `install_audio` after the status is set, builds a voice for the new rate off the audio thread and hands it to `Router::game_engine_rebuild(generation, voice)`, which drops it if a newer start happened meanwhile. |
| S22 | **Start.** `game_engine_start(patch)` refuses patch JSON over 16384 bytes (`GAME_PATCH_MAX`) and anything that is not a patch. The user's `DUSTLINE KESTREL.lbpatch` in the patch folder wins (source `'user'`); otherwise the game's JSON plays (source `'game'`). `Patch::from_json` clamps, LEGATO and bend 24 are forced, and the voice is built with no lock held for the current output rate. The shipped patch is named KESTREL ENGINE, so it never shadows the user's file. |
| S23 | **Bridge** (`engineBridge.ts`): messages count only from the active frame showing Dustline; malformed ones are dropped; frames are capped at 60 per second with a burst of 10. Replies are `{lpd: 'engine-synth-status', state: 'ready' \| 'unavailable' \| 'error'}`; outside the app the game hears `unavailable`. Leaving Dustline stops and forgets the voice; a hidden window stops it until the next sounding frame; a stop while the voice is being built stops it as soon as it exists. |
| S24 | **Pause → Sound → ENGINE** (`#pauseEngine`): BAKED, and LIVE SYNTH, which stays disabled and reads LIVE SYNTH UNAVAILABLE until the app reports ready. The choice is kept for the session only. Anything but ready puts the engine back on BAKED. |
| S25 | The native voice is started once, when Dustline's audio starts (`engineHost.start(JSON.stringify(engine.ship.patch))`), and idles until LIVE SYNTH is chosen: BAKED sends no frames, and leaving LIVE sends an `on: false` frame. Pausing sends an off frame. |
| S26 | **ElevenLabs:** 14 sounds × 3 takes = 42 generations. Take a = phrasing 1 at prompt influence 0.6, b = phrasing 2 at 0.6, c = phrasing 1 at 0.3. Loops are generated 6 s long (a 3 s window is picked), one-shots 1, 2 or 2.5 s; output `mp3_44100_128` to `audio/ships/kestrel/raw/`. Existing files are never regenerated. |
| S27 | **Pick rule:** among the takes that pass steadiness and a clean seam (loops) or a clean trim (one-shots), the highest earshot `match` z wins; ties go to take order a, b, c. `realness` and `rank` are information only. A sound without a passing take gets no pick, and new takes need the user's go. |
| S28 | **Mastering** (`tools/ship_assets.py`): loops are the picked 3.0 s window plus 0.05 s on each side, stereo, 48 kHz, seam crossfade S14, −18 dBFS RMS over the loop, lowered if the peak would pass −1 dBFS. One-shots are mono, the picked trim, 3 ms fade-in, 20 ms fade-out, peak −1 dBFS. 32-bit float WAV masters. |
| S29 | **Texture fields:** `{id, layer, role, refRpm, baseHz, loopStart, loopEnd, weight}`. Roles and refRpm: f1-mid `mid` 0.65, f1-high `high` 0.95, f1-off `off` 0.65, jet-cruise `cruise` 0.857, jet-burn `burn` 1.25; the drone (`drone`) has refRpm = 2 × baseHz / redlineHz, an octave under the fundamental. Each texture plays at `rpm / refRpm`, clamped to 0.5–2. |
| S30 | **Layer weights:** F1 0.4 (full in gears 3–7, 0.6 in gears 1–2), jet 0.35 × (0.4 + 0.6 × boost), drone 0.25 × (0.6 + 0.4 × load) with a slow shimmer at a different phase on each side. Crossfades inside a layer are equal-power. |
| S31 | **Mix defaults:** `toneDb −6, textureDb −6, shotDb −6, cutDb −9, blipDb 3, zingBlipDb 6, pan 0.34, rightPitch 1.0035, liveTrimDb 0`. |
| S32 | **One-shots:** at most six at once (the oldest stops first). An upshift plays one of three crackles and a downshift one of two burbles, each engine a different variant chosen by the seeded generator; boost ignition, boost release and the launch have their own sounds; a boost pad plays the ignition shortened with a 40 ms fade. They replace the 1.2.0 afterburner ignition, burn and release voices. |
| S33 | A `'texture'` layer joins the engine's layer list. Per side, the side level `vl` carries `mix.toneDb` and `toneSum` carries the tone layer's level, so muting the tone layer (or LIVE's crossfade) silences the loops without silencing the textures. R1 makes the `vl`/`toneSum` split in phase 1 (R1-7); Task 14 adds only the `'texture'` layer. |
| S34 | **Opus** at 96 kbps and 48 kHz through ffmpeg/libopus, with the master's channel count. The browser seam test decodes every loop in 48 and 44.1 kHz contexts (limits S51). |
| S35 | **Five version strings** change in Task 1: `DUSTLINE v0.13`, `Rift Run 0.13 / On the Limit`, `TIMING RUN 0.13`, `Circuit 0.13`, `version:'0.13.0'`. The engine's `VERSION` becomes `'2.0.0'` in Task 6. |
| S36 | **`tools/sync_copies.py`** keeps the hash of the last tool write in `tools/copies.sha256`: `check` (all three copies equal the record), `push` (after `index.html` was edited: the other two must still equal the record; `index.html` is copied over them and recorded), `diff` (which copies differ, with a short line diff). |
| S37 | **Step scripts** in `tools/engine2/`: `step1_ship.py` (Task 2), `step2_gears.py` (3), `step3_baked.py` (6), `step4_ship_bundle.py` (7), `step5_textures.py` (14), `step6_live.py` (22), `step7_live_game.py` (23), plus the re-runnable `profile_assets.py` (15), `live_trim.py` (23) and `profile_patch.py` (created in Task 2, run only when the user wants a patch tweak in BAKED). Each edits `index.html` as text with exact-count replacements (LF during the edit, CRLF again after it), writes it atomically and pushes it; a step whose anchors are missing stops before writing, so no step runs twice. JavaScript pieces live in `tools/engine2/pieces/`. R1 adds `step_r1_mech.py` with `pieces/mech.js`, run once between Task 9 and Task 10. |
| S38 | **Check totals:** new `checks/ship-profile.test.cjs` (7) and `checks/ship-gears.test.cjs` (17); `checks/audio-engine.test.cjs` is replaced (16 checks, 20 after Task 14, 22 after Task 22). Totals: 131 (Task 1), 138 (Task 2), 155 (Task 3), 161 (Tasks 6–9), 165 (Tasks 14–16), 167 (Tasks 22–24). R1 adds one audio-engine check (17, then 21 after Task 14, 23 after Task 22): 162 (R1), 166 (Tasks 14–16), 168 (Tasks 22–24). |
| S39 | Screenshots of the 0.13 harnesses go to `checks/renders-v0.13/`. `timing-events.test.cjs` requires scripts 0–6 to equal 0.12's and the master stage (the source of `createMaster`, 817 characters, asserted longer than 500) to equal 0.12's. |
| S40 | **`audio/ships/kestrel/profile-assets.json`** (written by `tools/ship_assets.py`) holds the profile's `textures` and `oneShots`; `tools/engine2/profile_assets.py` copies them into the build and `ship-profile.test.cjs` compares them. |
| S41 | **Audition frames** come from the real simulation (`checks/ship-scenarios.cjs`): 1458 frames at 60 Hz, 24.3 s plus a 1.5 s tail. 0.12's engine (1.2.0) is rendered on the same frames for comparison, and both are also gain-matched to −20 LUFS. |
| S42 | **Measured targets** in `metrics.json` (spec 3.6): bassy (at cruise at least 35 % of the propulsion energy below 200 Hz), zing (a tonal peak at 1–4 kHz that rises with speed), pitch (within 3 % of the gear model's f on at least 90 % of the probes away from shifts), layers (each texture's earshot `match` z ≥ 2.5, from Task 16), loudness (propulsion about −16 LUFS at cruise). R1 adds `pitchTone` (the tone alone, same pitch rule) and `turbine` (the restored turbine follows the gear model at least as often as 0.12's follows its spool). |
| S43 | `bake_engine check --patch <file>` parses the patch as the app does and fails if any value had to be clamped. |
| S44 | No Rust test reads `prototypes/`: `tests/synth_game.rs` builds its own test patch. |
| S45 | **Pitch tracker** (`checks/pitch.py`, numpy only): 100 ms frames every 50 ms; McLeod's normalized square difference over 30–1500 Hz; the period is the first local maximum within 90 % of the highest; a frame is periodic when that maximum reaches 0.6. |
| S46 | `game_engine_status` returns `{running, frames, source, patch, rate}` (camelCase); the native smoke test reads `frames`. |
| S47 | `Dustline.audio` gains `ship {id, decoded, total}` (Task 7), `engineMode` and `live` (the bridge state, `'none'` outside the app) (Task 23). The engine's debug report gains `tone` and `attached`. |
| S48 | **`decodeShip()`** (game) decodes every `DUSTLINE_SHIP_B64` entry once audio starts and calls `engine.attach(id, buffer)` for each. |
| S49 | LIVE bend and wheel glide over 40 ms (fix 20). **The user may veto.** |
| S50 | `GAME_ENGINES_GAIN = 2.0`: one voice stands for both engines, so it plays at twice its frame gain. Without it LIVE measured 8.2 dB under BAKED at cruise, with it 2.2 dB under (trim suggestion +2 dB). **The user may veto.** |
| S51 | **Seam limits per kind:** 0.1 for baked loops, 0.25 for textures (Opus coding noise alone reads about 0.10–0.15 on broadband loops). **The user may veto.** **Changed by the user (2026-11-06, Task 15 Step 6): textures 0.3.** The ear-picked jet-cruise (take b) decoded at 44.1 kHz read 0.265 (0.246 at 48 kHz) while its WAV master's seam reads 0.005: Opus noise where the 64 samples at the seam are 1.48 × the loop's RMS. Baked loops stay 0.1. |
| S52 | **Steadiness of unpitched loop takes:** level range ≤ 3 dB and spectral-centroid drift (Welch, 100 Hz–16 kHz) ≤ 10 %, over 0.5 s frames. Pitched takes: pitch drift ≤ 2 %. (A 5 % limit over 100 ms frames failed every noise loop.) **The user may veto.** |
| S53 | **Audition segments:** grid revs 2 s, launch 6 s, cruise 3 s, lift 2 s, brake 0.3 s (346 → 125 km/h), accelerate 3 s, boost 5 s (must reach at least 1150 km/h; it reaches 1200), release 3 s, tail 1.5 s. |
| S54 | The phase-1 audition goes to `audio/audition-v0.13-p1/`; phases 2 and 3 write `audio/audition-v0.13/`. |

### 0.5 Shared name registry

**Game (`prototypes/Dustline`, engine block in script 7)**

- `DustlineEngineAudio.VERSION` / the engine's `version`: `'2.0.0'` (from Task 6).
- `DustlineEngineAudio.ships.kestrel`: `id 'kestrel'`, `name`, `gears {tops:[80,150,220,300,390,490,700], idleHz:41, redlineHz:124, boostExponent:.8, upMin:.22, downMin:.09, overRev:1.03, pace:250}`, `patch` (the `.lbpatch` JSON as an object), `patchName 'DUSTLINE KESTREL'`, `liveNote 48`, `baked` (eight `{id, anchorHz, load, loopStart:2400, loopEnd:98400}`, ids `core-048-on`, `core-048-off`, `core-075-on`, … `core-182-off`), `textures` (empty until Task 15, then six entries, S29), `oneShots {upshift:[…], downshift:[…], ignite, release, launch}` (empty until Task 15, then `up-1…3`, `down-1…2`, `ignite`, `release`, `launch`), `mix` (S31).
- `DustlineEngineAudio.gears`: `{create: createGears, fromSpeed: gearFromSpeed, GEAR_DT}`; constants `GEAR_DT = 1/120`, `CUT_S = .04`, `GLIDE_S = .06`, `DOWN_GLIDE_S = .04`, `BLIP_S = .08`.
- `DustlineEngineAudio.create(ctx, {output, live})`: `live` (optional, Task 22) is the sink called with `{on, bend, wheel, gain}` once per control step.
- `engine.attach(id, buffer) → boolean` (S11); `engine.ship` (the profile).
- Frame field `engineMode: 'baked' | 'live'` (Task 23).
- `Dustline.audio.ship {id, decoded, total}`, `Dustline.audio.engine.tone`, `Dustline.audio.engine.attached`, `Dustline.audio.engineMode`, `Dustline.audio.live` (S47).
- Host bridge client `DustlineHostBridge.engineSynth {state, onStatus, start(patch), frame(on, bend, wheel, gain), stop()}`; messages `{lpd:'engine-synth', op:'start'|'frame'|'stop'}` out, `{lpd:'engine-synth-status', state}` in.
- `#pauseEngine` select: options `baked` (BAKED) and `live` (LIVE SYNTH / LIVE SYNTH UNAVAILABLE).
- `const DUSTLINE_SHIP_B64 = {…};` (S12); `decodeShip()` (S48).

**Dustline tools** (paths under `prototypes/Dustline`)

- `tools/sync_copies.py` (`check | push | diff`), `tools/copies.sha256`, `tools/start_013.py`, `tools/ship_encode.py`, `tools/ship_assets.py`.
- `tools/engine2/common.py` (`Build`: `rep`, `between`, `piece`, `engine`, `save`), `tools/engine2/step1_ship.py` … `step7_live_game.py`, `tools/engine2/profile_assets.py`, `tools/engine2/profile_patch.py`, `tools/engine2/live_trim.py <dB>`, `tools/engine2/pieces/{kestrel,gears,baked-side,baked-player}.js`.
- `apply-sfx.py` (replaced), `generate-ship-sfx.py [--go]`.
- `checks/ship-profile.test.cjs`, `checks/ship-gears.test.cjs`, `checks/audio-engine.test.cjs` (replaced), `checks/ship-scenarios.cjs`, `checks/pitch.py`, `checks/ship_screen.py loops | takes | audition DIR`, `checks/audio_audition.py [--engine] [--previous] [--out] [--live] [--patch]` (replaced), `checks/audio_lab.py` (gains `ship_bundle()` and attaches the ship in `RENDER_JS`).
- Assets under `audio/ships/kestrel/`: `kestrel-engine.lbpatch`, `loops/*.wav` (+ `bake.json`, `analysis.json`), `raw/` (takes, `prompts.json`, `wav/`), `picks.json`, `shots/*.wav`, `profile-assets.json`, `opus/*.opus`, `LOG.md`.

**Rust (`launchpad-drummer/crates/lpd-core`)**

- `examples/bake_engine.rs`: modes `check --patch <file>`, `loops --patch <file> --out <dir>`, `scenario --patch <file> --frames <json> --out <wav>` (Task 18).
- `src/synth/game.rs`: `GAME_NOTE 48`, `GAME_VELOCITY 127`, `GAME_BEND_RANGE 24`, `GAME_RAMP_S 0.025`, `GAME_GLIDE_S 0.040`, `GAME_WATCHDOG_S 0.25`, `GAME_ENGINES_GAIN 2.0`, `GAME_FADE_S 0.05`; `GameFrame {on, bend, wheel, gain}`; `GameEngineVoice::new(sample_rate, &SynthParams) -> Box<GameEngineVoice>`, `params()`, `current()`, `finished()`, `frame(GameFrame)`, `stop()`, `render_add(l, r)`.
- `src/audio/engine.rs`: `Command::GameSynthLoad(Box<GameEngineVoice>)`, `Command::GameSynthFrame { on, bend, wheel, gain }`, `Command::GameSynthClear`, `Trash::Game(Box<GameEngineVoice>)`, fields `game`, `game_l`, `game_r`, `trash_game()`, `game_voice()`.

**App (`launchpad-drummer/src-tauri`, `src`)**

- `router.rs`: `GameSpec { params, source: &'static str, name, generation }`, `GameEngineStatus { running, frames, source, patch, rate }` (serde camelCase); `Router::game_engine_start(…) -> u64`, `game_engine_frame(on, bend, wheel, gain) -> bool`, `game_engine_stop()`, `game_spec()`, `game_engine_rebuild(generation, voice) -> bool`, `game_engine_status(rate)`.
- `state.rs`: `GAME_PATCH_NAME "DUSTLINE KESTREL"`, `GAME_PATCH_MAX 16384`; `AppState::game_engine_start(game_patch, dir) -> Result<GameEngineStatus, String>`, `game_engine_frame`, `game_engine_stop`, `game_engine_status`, `restart_game_engine()`.
- `commands.rs`: `game_engine_start(patch)` (async, through `worker`, reads `patches_dir`), `game_engine_frame(on, bend, wheel, gain) -> bool`, `game_engine_stop()`, `game_engine_status() -> GameEngineStatus`; registered in `lib.rs` after `commands::keys_mod,`.
- `src/lib/engineBridge.ts`: `ENGINE_PATCH_MAX 16384`, `ENGINE_RATE 60`, `ENGINE_BURST 10`, `EngineMessage`, `EngineStatus`, `parseEngineMessage`, `rateCap`, `EngineHost`, `EngineLink` (`receive(data, fromDustline)`, `stop(forget)`).

### 0.6 Commands used by the tasks

Run from the root in Git Bash unless the block says PowerShell.

**Hash check of the three copies** (Task 1 on; every game-writing script also runs it):

```bash
python prototypes/Dustline/tools/sync_copies.py check
```

Expected: `ok <sha256>`.

**Line-ending check** (after every Edit; prints the file's kind, fails on mixed endings):

```bash
python -c "import sys;d=open(sys.argv[1],'rb').read();n=d.count(b'\r\n');l=d.count(b'\n')-n;assert not(n and l),'mixed line endings';print('CRLF' if n else 'LF',sys.argv[1])" <file>
```

Expected: the kind the task's Files list gives for that file.

**CRLF restore** (only after the Write tool replaced a CRLF file):

```bash
python -c "import sys;p=sys.argv[1];d=open(p,'rb').read().replace(b'\r\n',b'\n');open(p,'wb').write(d.replace(b'\n',b'\r\n'));print('CRLF',p)" <file>
```

**Regression** (all deterministic checks and the AI pace check):

```bash
cd "/d/MY DOCS/MY TOOLS/LAUNCHBAND/prototypes/Dustline"
for f in checks/*.test.cjs; do out=$(node "$f" 2>&1) && echo "${f#checks/}: ${out##*$'\n'}" || { echo "FAIL $f"; printf '%s\n' "$out" | tail -25; }; done
out=$(node checks/ai-pace.cjs index.html --verify 2>&1) && printf '%s\n' "$out" | tail -2 || { echo "FAIL ai-pace"; printf '%s\n' "$out" | tail -25; }
cd "/d/MY DOCS/MY TOOLS/LAUNCHBAND"
```

Expected at 0.12/Task 1 (131): `audio-engine.test.cjs: 10 engine audio checks passed.`, `easy-mode.test.cjs: 4 Easy-mode checks passed`, `handling.test.cjs: 9 handling checks passed.`, `hud-model.test.cjs: 11 HUD model checks passed`, `midi-leds.test.cjs: 13 MIDI LED checks passed.`, `open-circuit.test.cjs: 11 open-circuit checks passed.`, `power-heat.test.cjs: 21 power/heat/alignment checks passed.`, `rival-ai.test.cjs: 18 rival AI checks passed.`, `terrain-ground.test.cjs: 3 terrain ground checks passed.`, `timing-events.test.cjs: 31 timing/input checks passed.`, then `PASS straight pace regression: every rival exceeds 90% of the player ceiling and averages above cruise` and `PASS headless AI pace: all five drivers finish across three seeds; finite speeds within the shared ceiling`. Later tasks add `ship-profile.test.cjs: ship profile: 7 passed`, `ship-gears.test.cjs: ship gears: 17 passed`, and `audio-engine.test.cjs` reads `16 engine audio checks passed.` (then 20, then 22). No line may start with `FAIL`.

**Browser harnesses** (headless Edge; all eight took about 90 s in planning, `audio-browser` about 30 s of it):

```bash
cd "/d/MY DOCS/MY TOOLS/LAUNCHBAND/prototypes/Dustline"
for h in audio-browser desert-browser handling-browser host-bridge; do out=$(python checks/$h.py 2>&1) && echo "PASS $h: $(printf '%s' "${out##*$'\n'}" | cut -c1-240)" || { echo "FAIL $h"; printf '%s\n' "$out" | tail -30; }; done
for h in pause-browser power-browser qte-browser timing-browser; do out=$(python checks/$h.py 2>&1) && echo "PASS $h: $(printf '%s' "${out##*$'\n'}" | cut -c1-240)" || { echo "FAIL $h"; printf '%s\n' "$out" | tail -30; }; done
cd "/d/MY DOCS/MY TOOLS/LAUNCHBAND"
```

Expected: eight `PASS` lines. `audio-browser` ends with a JSON line (`engineNodes`, `engineSources`, `worstSeam`, …) and `host-bridge` with a JSON line (`checks`); the others end with their last `PASS …` line.

**Cargo** (PowerShell):

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;" + $env:PATH
Set-Location "D:\MY DOCS\MY TOOLS\LAUNCHBAND\launchpad-drummer"
cargo test -p lpd-core --example bake_engine
```

**App checks** (Task 19 on): `cargo fmt --all -- --check` and `cargo test --workspace` in PowerShell from `launchpad-drummer`; `npm test` and `npm run check` in Git Bash from `launchpad-drummer`.

**earshot venv** (Git Bash): `PY="/d/MY DOCS/MY TOOLS/earshot/.venv/Scripts/python.exe"`, then `"$PY" -I prototypes/Dustline/checks/ship_screen.py <mode>`.

### 0.7 Task map

| Phase | Task | What it delivers | Checks after it |
|---|---|---|---|
| 1 BAKED with synth loops | 1 Start 0.13 | three 0.13 copies, `sync_copies.py`, checks pointed at 0.13 | 131 |
| | 2 Ship profile | `ships.kestrel`, `kestrel-engine.lbpatch` | 138 |
| | 3 Gear model | `gears` | 155 |
| | 4 Bake tool | `bake_engine` example: `check`, `loops` | 5 Rust tests |
| | 5 Bake and screen the loops | eight loops, `pitch.py`, `ship_screen.py loops` | loop checks |
| | 6 BAKED engine | engine 2.0.0 on the synth loops | 161 |
| | 7 Game wiring and ship bundle | `decodeShip`, Opus bundle, seams in Edge | 161, audio-browser |
| | 8 Audition | `ship-scenarios.cjs`, audition, `ship_screen.py audition` | measurements |
| | 9 Regression, patch copy, README | | 161, 8 harnesses, ai-pace |
| | **CHECKPOINT 1** | the user listens: failed 2026-11-06 | |
| | R1 Tone revision | restored 1.2.0 layers on the gear model, revised patch, audition `audition-v0.13-p1-r1/` | 162, 8 harnesses, ai-pace |
| | **CHECKPOINT 1 (revised)** | the user listens: passed 2026-11-06 | |
| 2 Textures and one-shots | 10 Generation script | dry run of the 42 generations | |
| | 11 Generation run | **only on the user's go** | |
| | 12 Screen the takes | `ship_screen.py takes`, `picks.json` | 14 picks |
| | 13 Master the picks | `ship_assets.py`, `profile-assets.json` | |
| | 14 Textures and one-shots in the engine | step 5 | 165 (R1: 166) |
| | 15 Profile assets, encode, embed | 22 assets, page about 2.2 MB | 165 (R1: 166), audio-browser |
| | 16 Audition, regression, README | | 165 (R1: 166), 8 harnesses, ai-pace |
| | **CHECKPOINT 2** | the user listens | |
| 3 LIVE SYNTH | 17 `GameEngineVoice` | `synth/game.rs` | 8 Rust tests |
| | 18 Engine commands, master mix, bake scenario | `engine.rs`, `bake_engine scenario` | 29 + 6 + 5 Rust tests |
| | 19 App Rust layer | router, state, commands | `cargo test --workspace` |
| | 20 `engineBridge.ts` | | 9 tests |
| | 21 `App.svelte` | | `npm run check`, `npm test` |
| | 22 LIVE in the engine | step 6 | 167 (R1: 168) |
| | 23 LIVE game wiring and LIVE audition | step 7, `live_trim.py` | host-bridge, pause-browser |
| | 24 Exe rebuild, native smoke, final regression, README | | everything |

---

## Phase 1 — BAKED with the synth loops

Phase 1 replaces the 1.2.0 core, turbine and exhaust with the Kestrel's tone: the gear model plays eight synth loops per engine, baked by the app's own Rust synth. The 1.2.0 afterburner stays until Task 14 (S10). It ends at CHECKPOINT 1 with an audition for the user to hear.

### Task 1: Start 0.13

**Purpose.** Dustline 0.13 starts from the frozen 0.12 build: the five version strings change (S35), `index.html`, the new `dustline-rift-run-v0.13.html` and the app copy carry 0.13, and `tools/copies.sha256` records their hash for `tools/sync_copies.py` (S36). The checks and harnesses move to 0.13 (fix 15, S39). Nothing else in the page changes.

**Files:**
- Create: `prototypes/Dustline/tools/sync_copies.py`, `prototypes/Dustline/tools/start_013.py`
- Written by `start_013.py`: `prototypes/Dustline/index.html`, `prototypes/Dustline/dustline-rift-run-v0.13.html` (new), `launchpad-drummer/public/games/dustline/index.html`, `prototypes/Dustline/tools/copies.sha256` (new)
- Modify: `prototypes/Dustline/checks/timing-events.test.cjs` (CRLF), `checks/open-circuit.test.cjs` (CRLF), `checks/timing-browser.py` (LF), `checks/desert-browser.py` (CRLF), `checks/handling-browser.py` (LF), `checks/host-bridge.py` (LF), `checks/pause-browser.py` (CRLF), `checks/power-browser.py` (CRLF), `checks/qte-browser.py` (CRLF)

- [ ] **Step 1: Check the baseline.**

```bash
sha256sum prototypes/Dustline/index.html prototypes/Dustline/dustline-rift-run-v0.12.html launchpad-drummer/public/games/dustline/index.html
ls prototypes/Dustline/dustline-rift-run-v0.13.html prototypes/Dustline/tools
```

Expected: all three hashes are `2301504ed0be27444d126cdeecb65e6f9985fb91032d01c3e1f86b5cbf22e3a3`, and `ls` reports that neither `dustline-rift-run-v0.13.html` nor `tools` exists. If a hash differs, another session changed that copy: stop and ask the user (show `cmp` or a short diff of the copies).

- [ ] **Step 2: Point the checks at 0.13 (the failing test).** Apply these Edits, then run the line-ending check (0.6) on each of the nine files.

Edit `prototypes/Dustline/checks/timing-events.test.cjs`. Replace:

```js
test('0.12 copies are byte-identical CRLF; prior releases, renderer and craft are preserved', () => {
  const bytes = fs.readFileSync(path.join(root, 'index.html'));
  assert.deepEqual(bytes, fs.readFileSync(path.join(root, 'dustline-rift-run-v0.12.html')));
  assert.deepEqual(bytes, fs.readFileSync(path.join(root, '../../launchpad-drummer/public/games/dustline/index.html')));
  assert.equal(html.replaceAll('\r\n', '').includes('\n'), false);
  const sha = file => crypto.createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex');
  assert.equal(sha('dustline-rift-run-v0.11.html'), 'dbbc0f048d49ebbfd28d8b12b7548f971434635157e74e7fcbf2d306cc73c9a0');
  assert.equal(sha('dustline-rift-run-v0.10.html'), 'fd404efeef0035bd8e37f19b3fbb59bdb1a40a430446cb23bcd5075b46277e23');
  assert.equal(sha('dustline-rift-run-v0.9.html'), '0b6f690a6d51bf3268573944d94c403d8264257496d38f08d6d24a0b5eea066b');
  const ref = [...fs.readFileSync(path.join(root, 'dustline-rift-run-v0.11.html'), 'utf8').matchAll(/<script[^>]*>([\s\S]*?)<\/script>/g)].map(m => m[1]);
  assert.equal(scripts.length, 8);
  for (const i of [0, 4]) assert.equal(scripts[i], ref[i], `inline script ${i} must match 0.11`);
  const engine = code => code.slice(code.indexOf('/* @dustline-engine-audio:begin */'), code.indexOf('/* @dustline-engine-audio:end */'));
  const graph = code => engine(code).slice(engine(code).indexOf('  function createMaster('), engine(code).indexOf('    function player('));
  assert.equal(graph(scripts[7]), graph(ref[7]), 'procedural audio graph unchanged; signal mapping is intentionally updated');
```

with:

```js
test('0.13 copies are byte-identical CRLF; prior releases, scripts 0-6 and the master stage are preserved', () => {
  const bytes = fs.readFileSync(path.join(root, 'index.html'));
  assert.deepEqual(bytes, fs.readFileSync(path.join(root, 'dustline-rift-run-v0.13.html')));
  assert.deepEqual(bytes, fs.readFileSync(path.join(root, '../../launchpad-drummer/public/games/dustline/index.html')));
  assert.equal(html.replaceAll('\r\n', '').includes('\n'), false);
  const sha = file => crypto.createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex');
  assert.equal(sha('dustline-rift-run-v0.12.html'), '2301504ed0be27444d126cdeecb65e6f9985fb91032d01c3e1f86b5cbf22e3a3');
  assert.equal(sha('dustline-rift-run-v0.11.html'), 'dbbc0f048d49ebbfd28d8b12b7548f971434635157e74e7fcbf2d306cc73c9a0');
  assert.equal(sha('dustline-rift-run-v0.10.html'), 'fd404efeef0035bd8e37f19b3fbb59bdb1a40a430446cb23bcd5075b46277e23');
  assert.equal(sha('dustline-rift-run-v0.9.html'), '0b6f690a6d51bf3268573944d94c403d8264257496d38f08d6d24a0b5eea066b');
  const scriptsOf = file => [...fs.readFileSync(path.join(root, file), 'utf8').matchAll(/<script[^>]*>([\s\S]*?)<\/script>/g)].map(m => m[1]);
  const v11 = scriptsOf('dustline-rift-run-v0.11.html'), v12 = scriptsOf('dustline-rift-run-v0.12.html');
  assert.equal(scripts.length, 8);
  for (const i of [0, 4]) assert.equal(scripts[i], v11[i], `inline script ${i} must match 0.11`);
  for (let i = 0; i < 7; i++) assert.equal(scripts[i], v12[i], `inline script ${i} must match 0.12`);
  // The engine block changes in 0.13; its master stage (glue compressor, limiter, make-up gain) does not.
  const engine = code => code.slice(code.indexOf('/* @dustline-engine-audio:begin */'), code.indexOf('/* @dustline-engine-audio:end */'));
  const master = code => engine(code).slice(engine(code).indexOf('  function createMaster('), engine(code).indexOf('  // The two engines differ'));
  assert(master(scripts[7]).length > 500, 'master stage found');
  assert.equal(master(scripts[7]), master(v12[7]), 'master stage unchanged since 0.12');
```

Edit `prototypes/Dustline/checks/open-circuit.test.cjs`. Replace:

```js
assert.equal(html, fs.readFileSync(path.join(dir, 'dustline-rift-run-v0.12.html'), 'utf8'));
```

with:

```js
assert.equal(html, fs.readFileSync(path.join(dir, 'dustline-rift-run-v0.13.html'), 'utf8'));
```

Edit `prototypes/Dustline/checks/timing-browser.py` (1 of 2). Replace:

```python
OUT = ROOT / "checks" / "renders-v0.12"
```

with:

```python
OUT = ROOT / "checks" / "renders-v0.13"
```

Edit `prototypes/Dustline/checks/timing-browser.py` (2 of 2). Replace:

```python
    shipped.goto((ROOT / "dustline-rift-run-v0.12.html").as_uri())
    shipped.wait_for_function("window.Dustline && document.getElementById('loading').hidden")
    assert shipped.evaluate("Dustline.version") == "0.12.0"
```

with:

```python
    shipped.goto((ROOT / "dustline-rift-run-v0.13.html").as_uri())
    shipped.wait_for_function("window.Dustline && document.getElementById('loading').hidden")
    assert shipped.evaluate("Dustline.version") == "0.13.0"
```

Edit `prototypes/Dustline/checks/desert-browser.py`. Replace:

```python
OUT = ROOT / 'checks/renders-v0.12/desert'
```

with:

```python
OUT = ROOT / 'checks/renders-v0.13/desert'
```

Edit `prototypes/Dustline/checks/handling-browser.py`. Replace:

```python
OUT = ROOT / 'checks/renders-v0.12/handling'
```

with:

```python
OUT = ROOT / 'checks/renders-v0.13/handling'
```

Edit `prototypes/Dustline/checks/host-bridge.py`. Replace:

```python
OUT = ROOT / 'checks/renders-v0.12'
```

with:

```python
OUT = ROOT / 'checks/renders-v0.13'
```

Edit `prototypes/Dustline/checks/pause-browser.py`. Replace:

```python
OUT=ROOT/'checks/renders-v0.12/pause'
```

with:

```python
OUT=ROOT/'checks/renders-v0.13/pause'
```

Edit `prototypes/Dustline/checks/power-browser.py`. Replace:

```python
OUT=ROOT/'checks/renders-v0.12/power'
```

with:

```python
OUT=ROOT/'checks/renders-v0.13/power'
```

Edit `prototypes/Dustline/checks/qte-browser.py`. Replace:

```python
OUT=ROOT/'checks/renders-v0.12/qte'
```

with:

```python
OUT=ROOT/'checks/renders-v0.13/qte'
```

- [ ] **Step 3: Run the two node checks to see them fail.**

```bash
cd prototypes/Dustline && node checks/timing-events.test.cjs; node checks/open-circuit.test.cjs; cd ../..
```

Expected: both stop with `Error: ENOENT: no such file or directory, open '…\prototypes\Dustline\dustline-rift-run-v0.13.html'`.

- [ ] **Step 4: Create the copy tools.** `mkdir -p prototypes/Dustline/tools/engine2/pieces prototypes/Dustline/audio/ships/kestrel`, then create the two files.

Create `prototypes/Dustline/tools/sync_copies.py`:

```python
"""Keep the three Dustline copies byte-identical without overwriting an edit made elsewhere.

    python prototypes/Dustline/tools/sync_copies.py check   all three copies equal each other and the recorded hash
    python prototypes/Dustline/tools/sync_copies.py push    after editing index.html: the other two still equal the
                                                            record; index.html is copied over them and recorded
    python prototypes/Dustline/tools/sync_copies.py diff    which copies differ from the record, with a short line diff

A copy that differs from the record was changed by another session. The tool stops; merge that change into
index.html by hand (never overwrite it), then run push. Bytes are copied as they are, so CRLF survives.
"""
import difflib
import hashlib
import sys
from pathlib import Path

DIR = Path(__file__).resolve().parents[1]
VERSION = "0.13"
INDEX = DIR / "index.html"
RELEASE = DIR / f"dustline-rift-run-v{VERSION}.html"
APP = DIR.parents[1] / "launchpad-drummer" / "public" / "games" / "dustline" / "index.html"
COPIES = [INDEX, RELEASE, APP]
RECORD = Path(__file__).resolve().parent / "copies.sha256"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def recorded():
    return RECORD.read_text(encoding="ascii").strip()


def record(digest):
    RECORD.write_text(digest + "\n", encoding="ascii")


def changed(paths=COPIES):
    want = recorded()
    return [p for p in paths if sha(p) != want]


def diff_lines(a, b, limit=40):
    """A short unified diff of two copies, by line, with the long bundle lines cut to 160 characters."""
    cut = lambda p: [line[:160] for line in p.read_bytes().decode("utf-8", "replace").split("\r\n")]
    return list(difflib.unified_diff(cut(a), cut(b), str(a.name), str(b), lineterm="", n=1))[:limit]


def check():
    bad = changed()
    if bad:
        raise SystemExit("Changed since the last recorded write: " + ", ".join(str(p) for p in bad) + "\nRun diff, merge by hand, then push.")
    print("ok", recorded())


def push():
    bad = changed([RELEASE, APP])
    if bad:
        raise SystemExit("Refusing to overwrite a copy changed elsewhere: " + ", ".join(str(p) for p in bad))
    data = INDEX.read_bytes()
    assert b"\r\n" in data and b"\n" not in data.replace(b"\r\n", b""), "index.html must be uniform CRLF"
    for target in (RELEASE, APP):
        tmp = target.with_suffix(".html.tmp")
        tmp.write_bytes(data)
        tmp.replace(target)
    digest = hashlib.sha256(data).hexdigest()
    record(digest)
    assert not changed(), "copies differ after push"
    print("pushed", digest, len(data), "bytes")


def diff():
    want = recorded()
    for p in COPIES:
        print(("same  " if sha(p) == want else "CHANGED ") + str(p))
    for p in changed([RELEASE, APP]):
        print("\n".join(diff_lines(INDEX, p)))


if __name__ == "__main__":
    {"check": check, "push": push, "diff": diff}[sys.argv[1] if len(sys.argv) > 1 else "check"]()
```

Create `prototypes/Dustline/tools/start_013.py`:

```python
"""Start 0.13 from the frozen 0.12 build, once: bump the five version strings, write the v0.13 copy and the app copy, and
record their hash for sync_copies.py. It refuses when 0.13 exists or the three 0.12 copies differ from the reviewed build."""
import hashlib

import sync_copies as sc

V12 = sc.DIR / "dustline-rift-run-v0.12.html"
V12_SHA = "2301504ed0be27444d126cdeecb65e6f9985fb91032d01c3e1f86b5cbf22e3a3"
assert not sc.RELEASE.exists(), "0.13 has already been started"
data = sc.INDEX.read_bytes()
assert data == V12.read_bytes() == sc.APP.read_bytes(), "the three 0.12 copies must match: diff and merge first"
assert hashlib.sha256(data).hexdigest() == V12_SHA, "index.html is not the reviewed 0.12 build"
for old, new in [(b"DUSTLINE v0.12", b"DUSTLINE v0.13"), (b"Rift Run 0.12 / On the Limit", b"Rift Run 0.13 / On the Limit"),
                 (b"TIMING RUN 0.12", b"TIMING RUN 0.13"), (b"Circuit 0.12", b"Circuit 0.13"),
                 (b"version:'0.12.0'", b"version:'0.13.0'")]:
    assert data.count(old) == 1, old
    data = data.replace(old, new)
for target in (sc.INDEX, sc.RELEASE, sc.APP):
    tmp = target.with_suffix(".html.tmp")
    tmp.write_bytes(data)
    tmp.replace(target)
sc.record(hashlib.sha256(data).hexdigest())
print("0.13 started:", sc.recorded(), len(data), "bytes")
```

- [ ] **Step 5: Start 0.13.**

```bash
python prototypes/Dustline/tools/start_013.py
python prototypes/Dustline/tools/sync_copies.py check
```

Expected: `0.13 started: a2dec869da0d6a4ca464200ffe0b65514372f5c56489754354cf79abe640bc8d 1455469 bytes`, then `ok a2dec869da0d6a4ca464200ffe0b65514372f5c56489754354cf79abe640bc8d`. `start_013.py` refuses to run twice ("0.13 has already been started").

- [ ] **Step 6: Run the regression (0.6).** Expected: the 131-check list of 0.6 (`timing-events.test.cjs: 31 timing/input checks passed.`, `open-circuit.test.cjs: 11 open-circuit checks passed.`) and both ai-pace `PASS` lines.

- [ ] **Step 7: Run the browser harnesses (0.6).** Expected: eight `PASS` lines; the screenshots now go to `checks/renders-v0.13/`.

### Task 2: Ship profile

**Purpose.** The ship profile format and the Kestrel profile, exported as `DustlineEngineAudio.ships.kestrel` (spec 2.2, 3.2, 4.1; fixes 1–4; S29, S31). The profile embeds the Keys synth patch; the same JSON is the patch file `audio/ships/kestrel/kestrel-engine.lbpatch` that PLAY loads and the bake reads. `tools/engine2/common.py` is the byte-preserving editor every step script uses (S37). The test checks the profile's fields, that it is deeply frozen, that the embedded patch equals the file, the patch choices of spec 3.2 (LEGATO, bend 24, SUB and BODY at the fundamental, ZING two octaves up around the filter), the wheel routes, and the baked set (four integer anchors, on and off load).

**Files:**
- Create: `prototypes/Dustline/checks/ship-profile.test.cjs`, `prototypes/Dustline/audio/ships/kestrel/kestrel-engine.lbpatch`, `prototypes/Dustline/tools/engine2/common.py`, `prototypes/Dustline/tools/engine2/pieces/kestrel.js`, `prototypes/Dustline/tools/engine2/step1_ship.py`, `prototypes/Dustline/tools/engine2/profile_patch.py` (not run now; it brings a patch tweak into BAKED, see CHECKPOINT 1)
- Modify (by `step1_ship.py`): the three game copies

- [ ] **Step 1: Write the failing test.** Create `prototypes/Dustline/checks/ship-profile.test.cjs`:

```js
'use strict';
// Ship profile checks (spec 2.2, 3.2, 4.2): the Kestrel profile's fields, its embedded Keys synth patch against
// audio/ships/kestrel/kestrel-engine.lbpatch, the patch choices of section 3.2, and the baked loop set.
// These are data checks. They say nothing about how the engine sounds.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const root = path.resolve(__dirname, '..');
const html = fs.readFileSync(path.join(root, 'index.html'), 'utf8');
const BEGIN = '/* @dustline-engine-audio:begin */', END = '/* @dustline-engine-audio:end */';
const source = html.slice(html.indexOf(BEGIN) + BEGIN.length, html.indexOf(END));
// objects from the sandbox have their own prototypes: compare them as JSON or field by field
const sandbox = vm.createContext({ Math, Float32Array, Float64Array, Int32Array, Uint8Array, Map, WeakMap, Set, Array, Object, Number, JSON });
vm.runInContext(source, sandbox);
const E = sandbox.DustlineEngineAudio, K = E.ships.kestrel;
const PATCH_FILE = path.join(root, 'audio', 'ships', 'kestrel', 'kestrel-engine.lbpatch');
let passed = 0;
function test(name, fn) { fn(); passed++; console.log('PASS', name); }

test('Kestrel profile: identity, live note, gears and mix as the spec sets them', () => {
  assert.equal(K.id, 'kestrel'); assert.equal(K.name, 'KESTREL');
  assert.equal(K.patchName, 'DUSTLINE KESTREL'); assert.equal(K.liveNote, 48);
  assert.equal(JSON.stringify(K.gears), JSON.stringify({ tops: [80, 150, 220, 300, 390, 490, 700], idleHz: 41, redlineHz: 124, boostExponent: .8,
    upMin: .22, downMin: .09, overRev: 1.03, pace: 250 }));
  assert.equal(JSON.stringify(K.mix), JSON.stringify({ toneDb: -6, textureDb: -6, shotDb: -6, cutDb: -9, blipDb: 3, zingBlipDb: 6, pan: .34, rightPitch: 1.0035, liveTrimDb: 0 }));
});

test('the profile is deeply frozen', () => {
  for (const o of [K, K.gears, K.gears.tops, K.patch, K.patch.params, K.patch.params.oscA, K.patch.params.routes, K.patch.params.routes[0], K.baked, K.baked[0], K.mix, K.oneShots])
    assert(Object.isFrozen(o));
});

test('the embedded patch is the .lbpatch file, field for field', () => {
  const file = JSON.parse(fs.readFileSync(PATCH_FILE, 'utf8'));
  assert.equal(JSON.stringify(K.patch), JSON.stringify(file));
  assert.equal(file.patchVersion, 1); assert.equal(file.category, 'fx'); assert.equal(file.name, 'KESTREL ENGINE');
});

test('patch choices of section 3.2: legato, bend 24, SUB and BODY at the fundamental, ZING two octaves up around the filter', () => {
  const p = K.patch.params;
  assert.equal(p.voice.mode, 'legato'); assert.equal(p.voice.bendRange, 24); assert.equal(p.voice.glide, 0); assert.equal(p.voice.drift, 0);
  assert.deepEqual([p.sub.on, p.sub.shape, p.sub.oct, p.sub.toFilter], [true, 'sine', -1, true]);
  assert.deepEqual([p.oscA.on, p.oscA.table, p.oscA.warp, p.oscA.oct, p.oscA.unison, p.oscA.toFilter], [true, 'basic', 'sync', -1, 1, true]);
  assert.deepEqual([p.oscB.on, p.oscB.table, p.oscB.warp, p.oscB.oct, p.oscB.unison, p.oscB.toFilter], [true, 'basic', 'sync', 2, 3, false]);
  assert.equal(p.oscA.phaseRand, 0, 'OSC A starts at phase 0, so the baked loops line up');
  assert.deepEqual([p.filter.on, p.filter.type, p.filter.keyTrack, p.filter.envAmt], [true, 'lp24', 0, 0]);
  assert(p.filter.drive > 0, 'filter drive');
  assert.deepEqual([p.fx.drive.on, p.fx.drive.type], [true, 'tube']);
  assert.equal(p.noise.on, false);
  for (const fx of ['chorus', 'delay', 'reverb']) assert.equal(p.fx[fx].on, false, fx + ' off: a loop must not carry a tail');
  assert.equal(JSON.stringify(p.env[0]), JSON.stringify({ attack: .005, decay: .5, sustain: 1, release: .15 }), 'the amp envelope holds the note');
});

test('the wheel drives warp, cutoff, drive and both oscillator levels, and nothing else', () => {
  const r = K.patch.params.routes;
  assert(r.every(x => x.source === 'wheel' && x.aux === null && !x.bypass && x.amount > 0));
  assert.deepEqual([...r].map(x => x.dest).sort(), ['filter.cutoff', 'filter.drive', 'oscA.level', 'oscA.warpAmt', 'oscB.level', 'oscB.warpAmt']);
});

test('baked set: four integer anchors about 0.64 octave apart, on and off load each, 2 s loops after 0.05 s', () => {
  assert.equal(K.baked.length, 8);
  assert.deepEqual([...K.baked].map(b => b.id), ['core-048-on', 'core-048-off', 'core-075-on', 'core-075-off', 'core-117-on', 'core-117-off', 'core-182-on', 'core-182-off']);
  const anchors = [...new Set(K.baked.map(b => b.anchorHz))];
  assert.deepEqual(anchors, [48, 75, 117, 182]);
  for (const a of anchors) assert(Number.isInteger(a), 'an integer anchor fits whole periods in 1 s and 2 s');
  for (let i = 1; i < anchors.length; i++) { const oct = Math.log2(anchors[i] / anchors[i - 1]); assert(oct > .63 && oct < .65, 'spacing ' + oct); }
  assert(anchors[0] * 4 >= 191.5 && anchors[3] * .05 <= 41 * .994, 'every loop stays inside playback rates 0.05-4 over the whole tone range');
  for (const b of K.baked) {
    assert.equal(b.loopStart, 2400); assert.equal(b.loopEnd, 98400);
    assert.equal((b.loopEnd - b.loopStart) % 48000, 0);
  }
});

test('phase 1 has no textures or one-shots yet', () => {
  assert.equal(K.textures.length, 0);
  assert.deepEqual(JSON.parse(JSON.stringify(K.oneShots)), { upshift: [], downshift: [], ignite: '', release: '', launch: '' });
});

console.log(`ship profile: ${passed} passed`);
```

- [ ] **Step 2: Run it to see it fail.**

```bash
node prototypes/Dustline/checks/ship-profile.test.cjs
```

Expected: `TypeError: Cannot read properties of undefined (reading 'kestrel')`.

- [ ] **Step 3: Create the patch file, the editor, the profile piece, step 1 and the patch tool.**

Create `prototypes/Dustline/audio/ships/kestrel/kestrel-engine.lbpatch`:

```json
{
  "patchVersion": 1,
  "name": "KESTREL ENGINE",
  "category": "fx",
  "params": {
    "oscA": {"on": true, "table": "basic", "pos": 0.6666667, "oct": -1, "semi": 0, "fine": 0.0, "warp": "sync", "warpAmt": 0.1, "unison": 1, "detune": 0.0, "blend": 0.75, "width": 0.0, "phaseRand": 0.0, "level": 0.35, "pan": 0.0, "toFilter": true},
    "oscB": {"on": true, "table": "basic", "pos": 0.6666667, "oct": 2, "semi": 0, "fine": 0.0, "warp": "sync", "warpAmt": 0.1, "unison": 3, "detune": 0.12, "blend": 0.5, "width": 0.6, "phaseRand": 1.0, "level": 0.1, "pan": 0.0, "toFilter": false},
    "sub": {"on": true, "shape": "sine", "oct": -1, "level": 0.8, "toFilter": true},
    "noise": {"on": false, "color": "white", "level": 0.3, "toFilter": true},
    "filter": {"on": true, "type": "lp24", "cutoff": 301.0, "reso": 0.2, "drive": 0.15, "keyTrack": 0.0, "envAmt": 0.0},
    "amp": {"level": 0.8, "velSens": 0.0},
    "env": [
      {"attack": 0.005, "decay": 0.5, "sustain": 1.0, "release": 0.15},
      {"attack": 0.0, "decay": 0.35, "sustain": 0.0, "release": 0.12},
      {"attack": 0.002, "decay": 0.5, "sustain": 0.0, "release": 0.15}
    ],
    "lfo": [
      {"shape": "sine", "rate": 5.5, "sync": false, "syncDiv": "1/4", "mode": "trig", "delay": 0.0, "phase": 0.0, "polarity": "bi"},
      {"shape": "triangle", "rate": 0.5, "sync": false, "syncDiv": "1/4", "mode": "free", "delay": 0.0, "phase": 0.0, "polarity": "bi"}
    ],
    "voice": {"mode": "legato", "glide": 0.0, "glideMode": "legato", "bendRange": 24, "drift": 0.0, "volumeDb": 0.0},
    "routes": [
      {"source": "wheel", "dest": "oscA.warpAmt", "amount": 0.45, "aux": null, "bypass": false},
      {"source": "wheel", "dest": "oscB.warpAmt", "amount": 0.6, "aux": null, "bypass": false},
      {"source": "wheel", "dest": "filter.cutoff", "amount": 0.367, "aux": null, "bypass": false},
      {"source": "wheel", "dest": "filter.drive", "amount": 0.55, "aux": null, "bypass": false},
      {"source": "wheel", "dest": "oscA.level", "amount": 0.6, "aux": null, "bypass": false},
      {"source": "wheel", "dest": "oscB.level", "amount": 0.3, "aux": null, "bypass": false}
    ],
    "fx": {
      "drive": {"on": true, "type": "tube", "amount": 0.3, "tone": 9000.0, "mix": 0.6},
      "chorus": {"on": false, "rate": 0.6, "depth": 0.5, "delay": 0.012, "feedback": 0.0, "mix": 0.5},
      "delay": {"on": false, "time": 0.375, "sync": false, "syncDiv": "1/8D", "feedback": 0.35, "pingPong": false, "tone": 6000.0, "mix": 0.25},
      "reverb": {"on": false, "size": 0.6, "decay": 2.5, "damp": 0.5, "preDelay": 0.01, "mix": 0.25}
    }
  }
}
```

Create `prototypes/Dustline/tools/engine2/common.py`:

```python
"""Byte-preserving edits of the Dustline build for engine audio 2.0, one step script per plan task.

A step checks that the three copies still equal the recorded hash (tools/sync_copies.py), edits index.html as text with
exact-count replacements (CRLF becomes LF for the edit and CRLF again after it), writes index.html atomically and pushes it
to the other two copies. A step whose anchors are missing stops before anything is written, so a step never runs twice.
Pieces of JavaScript live in tools/engine2/pieces/ and are inserted as they are.
"""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import sync_copies as sc  # noqa: E402

BEGIN, END = "/* @dustline-engine-audio:begin */", "/* @dustline-engine-audio:end */"


class Build:
    def __init__(self):
        sc.check()
        raw = sc.INDEX.read_bytes()
        assert b"\n" not in raw.replace(b"\r\n", b""), "index.html must be uniform CRLF"
        self.t = raw.decode("utf-8").replace("\r\n", "\n")

    def rep(self, old, new, count=1):
        n = self.t.count(old)
        assert n == count, f"expected {count} x {old[:90]!r}, found {n}"
        self.t = self.t.replace(old, new)

    def between(self, start, end, new):
        """Replace from `start` (unique) through the first `end` after it, both included."""
        assert self.t.count(start) == 1, f"expected 1 x {start[:90]!r}"
        i = self.t.index(start)
        j = self.t.index(end, i) + len(end)
        self.t = self.t[:i] + new + self.t[j:]

    @staticmethod
    def piece(name):
        return (HERE / "pieces" / name).read_text(encoding="utf-8").rstrip("\n") + "\n"

    def engine(self):
        return self.t[self.t.index(BEGIN):self.t.index(END)]

    def save(self):
        data = self.t.replace("\n", "\r\n").encode("utf-8")
        tmp = sc.INDEX.with_suffix(".html.tmp")
        tmp.write_bytes(data)
        tmp.replace(sc.INDEX)
        sc.push()
```

Create `prototypes/Dustline/tools/engine2/pieces/kestrel.js`:

```js
  /* ---- ship profiles (spec 2.2). A ship is data: gears, the Keys synth patch, the baked loop set, texture loops, one-shots
   * and the mix. create(ctx,{ship}) picks one; KESTREL is the default and, for now, the only ship. ---- */
  const deepFreeze=o=>{if(o&&typeof o==='object'&&!Object.isFrozen(o)){Object.freeze(o);for(const k of Object.keys(o))deepFreeze(o[k]);}return o;};
  // The Keys synth patch, the same object as audio/ships/kestrel/kestrel-engine.lbpatch (checked by checks/ship-profile.test.cjs).
  const KESTREL_PATCH={"patchVersion":1,"name":"KESTREL ENGINE","category":"fx","params":{
    "oscA":{"on":true,"table":"basic","pos":0.6666667,"oct":-1,"semi":0,"fine":0.0,"warp":"sync","warpAmt":0.1,"unison":1,"detune":0.0,"blend":0.75,"width":0.0,"phaseRand":0.0,"level":0.35,"pan":0.0,"toFilter":true},
    "oscB":{"on":true,"table":"basic","pos":0.6666667,"oct":2,"semi":0,"fine":0.0,"warp":"sync","warpAmt":0.1,"unison":3,"detune":0.12,"blend":0.5,"width":0.6,"phaseRand":1.0,"level":0.1,"pan":0.0,"toFilter":false},
    "sub":{"on":true,"shape":"sine","oct":-1,"level":0.8,"toFilter":true},
    "noise":{"on":false,"color":"white","level":0.3,"toFilter":true},
    "filter":{"on":true,"type":"lp24","cutoff":301.0,"reso":0.2,"drive":0.15,"keyTrack":0.0,"envAmt":0.0},
    "amp":{"level":0.8,"velSens":0.0},
    "env":[{"attack":0.005,"decay":0.5,"sustain":1.0,"release":0.15},{"attack":0.0,"decay":0.35,"sustain":0.0,"release":0.12},{"attack":0.002,"decay":0.5,"sustain":0.0,"release":0.15}],
    "lfo":[{"shape":"sine","rate":5.5,"sync":false,"syncDiv":"1/4","mode":"trig","delay":0.0,"phase":0.0,"polarity":"bi"},{"shape":"triangle","rate":0.5,"sync":false,"syncDiv":"1/4","mode":"free","delay":0.0,"phase":0.0,"polarity":"bi"}],
    "voice":{"mode":"legato","glide":0.0,"glideMode":"legato","bendRange":24,"drift":0.0,"volumeDb":0.0},
    "routes":[{"source":"wheel","dest":"oscA.warpAmt","amount":0.45,"aux":null,"bypass":false},{"source":"wheel","dest":"oscB.warpAmt","amount":0.6,"aux":null,"bypass":false},{"source":"wheel","dest":"filter.cutoff","amount":0.367,"aux":null,"bypass":false},{"source":"wheel","dest":"filter.drive","amount":0.55,"aux":null,"bypass":false},{"source":"wheel","dest":"oscA.level","amount":0.6,"aux":null,"bypass":false},{"source":"wheel","dest":"oscB.level","amount":0.3,"aux":null,"bypass":false}],
    "fx":{"drive":{"on":true,"type":"tube","amount":0.3,"tone":9000.0,"mix":0.6},"chorus":{"on":false,"rate":0.6,"depth":0.5,"delay":0.012,"feedback":0.0,"mix":0.5},"delay":{"on":false,"time":0.375,"sync":false,"syncDiv":"1/8D","feedback":0.35,"pingPong":false,"tone":6000.0,"mix":0.25},"reverb":{"on":false,"size":0.6,"decay":2.5,"damp":0.5,"preDelay":0.01,"mix":0.25}}}};
  // Eight loops per engine: four pitch anchors, each rendered on-load (wheel 0.85) and off-load (wheel 0.2). Loop points are
  // in samples at 48 kHz, the encode rate: 0.05 s of lead-in, then 2 s of loop.
  const bakedSet=anchors=>anchors.flatMap(hz=>['on','off'].map(load=>({id:'core-'+String(hz).padStart(3,'0')+'-'+load,anchorHz:hz,load,loopStart:2400,loopEnd:98400})));
  const KESTREL=deepFreeze({id:'kestrel',name:'KESTREL',
    gears:{tops:[80,150,220,300,390,490,700],idleHz:41,redlineHz:124,boostExponent:.8,upMin:.22,downMin:.09,overRev:1.03,pace:250},
    patch:KESTREL_PATCH,patchName:'DUSTLINE KESTREL',liveNote:48,
    baked:bakedSet([48,75,117,182]),
    textures:[],
    oneShots:{upshift:[],downshift:[],ignite:'',release:'',launch:''},
    // levels and cuts in dB; pan of the two engines; the right engine's pitch offset (+0.35 %); LIVE SYNTH trim (section 5)
    mix:{toneDb:-6,textureDb:-6,shotDb:-6,cutDb:-9,blipDb:3,zingBlipDb:6,pan:.34,rightPitch:1.0035,liveTrimDb:0}});
```

Create `prototypes/Dustline/tools/engine2/step1_ship.py`:

```python
"""Engine 2.0, step 1 (plan Task 2): the ship profile format and the Kestrel profile, exported as DustlineEngineAudio.ships."""
from common import Build

b = Build()
b.rep("  const loadStep=(l,target,dt)=>ease(l,target,dt,.055,.11);\n",
      "  const loadStep=(l,target,dt)=>ease(l,target,dt,.055,.11);\n\n" + b.piece("kestrel.js"))
b.rep("loadStep,VMAX,CTRL}),buffersFor});\n",
      "loadStep,VMAX,CTRL}),\n    ships:Object.freeze({kestrel:KESTREL}),buffersFor});\n")
b.save()
print("step 1: the Kestrel profile is in the engine block")
```

Create `prototypes/Dustline/tools/engine2/profile_patch.py` (re-runnable; it replaces the profile's `KESTREL_PATCH` statement with the patch file, so the ship-profile test keeps comparing the two; do not run it now):

```python
"""Engine 2.0: put audio/ships/kestrel/kestrel-engine.lbpatch into the Kestrel profile (the KESTREL_PATCH object), for
example once the user saved a tweak on PLAY and asked for it in BAKED. Re-runnable: it rewrites the whole statement.
    python prototypes/Dustline/tools/engine2/profile_patch.py
Then bake the loops again (bake_engine loops), encode them (tools/ship_encode.py) and embed them (apply-sfx.py)."""
import json
from pathlib import Path

from common import Build

SHIP = Path(__file__).resolve().parents[2] / "audio" / "ships" / "kestrel"
patch = json.loads((SHIP / "kestrel-engine.lbpatch").read_text(encoding="utf-8"))
assert patch.get("patchVersion") == 1 and isinstance(patch.get("params"), dict), "not a Keys synth patch file"
assert patch.get("name") == "KESTREL ENGINE", "the shipped patch keeps the name KESTREL ENGINE (it must not shadow the user's file)"
END = "\n  // Eight loops per engine:"
b = Build()
b.between("  const KESTREL_PATCH=", END, "  const KESTREL_PATCH=" + json.dumps(patch, separators=(",", ":")) + ";" + END)
b.save()
print("profile: the Kestrel patch is kestrel-engine.lbpatch")
```

- [ ] **Step 4: Run step 1.**

```bash
python prototypes/Dustline/tools/engine2/step1_ship.py
```

Expected: `pushed <sha256> <bytes> bytes`, then `step 1: the Kestrel profile is in the engine block`.

- [ ] **Step 5: Run the test to see it pass.**

```bash
node prototypes/Dustline/checks/ship-profile.test.cjs
```

Expected: seven `PASS` lines, then `ship profile: 7 passed`.

- [ ] **Step 6: Run the regression (0.6).** Expected: 138 checks (the 131 plus `ship-profile.test.cjs: ship profile: 7 passed`) and both ai-pace `PASS` lines.

### Task 3: Gear model

**Purpose.** The pure, deterministic gear model both tone backends read, exported as `DustlineEngineAudio.gears` (spec 3.1; fixes 11, 12, 14, 18, 19; S1–S7). Seven gears, paced sweeps, GO from speed, the boost lift past 700 km/h, shift shaping, per-side load, 1/120 s substeps with no allocation per step. The 17 tests cover gear-from-speed, neutral revs, GO and the paced launch upshifts at 0.19, 0.54 and 0.93 s, pacing, frame-rate independence (30, 60, 120, 144 Hz and irregular frames within 1/120 s), brake downshifts, `upMin`, the over-rev hold, idle, the boost lift (190.85 Hz at 1200 km/h), hysteresis, smoothness, the cut/glide/blip timing, recovery, reset, load and bad input.

**Files:**
- Create: `prototypes/Dustline/checks/ship-gears.test.cjs`, `prototypes/Dustline/tools/engine2/pieces/gears.js`, `prototypes/Dustline/tools/engine2/step2_gears.py`
- Modify (by `step2_gears.py`): the three game copies

- [ ] **Step 1: Write the failing test.** Create `prototypes/Dustline/checks/ship-gears.test.cjs`:

```js
'use strict';
// Gear model checks (spec section 3.1): gear from speed, the paced speed, shift points and spacing, the over-rev cap, the idle
// clamp, the boost lift, shift shaping, neutral revs, recovery, loads, bad input, and the same shifts at any frame rate.
// Speed curves follow the 0.12 speed law (index.html:1880): v += (desired - v) * (1 - exp(-k / 120)) every 1/120 s.
// These are model checks. They say nothing about how the engine sounds.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const root = path.resolve(__dirname, '..');
const html = fs.readFileSync(path.join(root, 'index.html'), 'utf8');
const BEGIN = '/* @dustline-engine-audio:begin */', END = '/* @dustline-engine-audio:end */';
const source = html.slice(html.indexOf(BEGIN) + BEGIN.length, html.indexOf(END));
const sandbox = vm.createContext({ Math, Float32Array, Float64Array, Int32Array, Uint8Array, Map, WeakMap, Set, Array, Object, Number, JSON });
vm.runInContext(source, sandbox);
const E = sandbox.DustlineEngineAudio;
const K = E.ships.kestrel.gears, STEP = E.gears.GEAR_DT;

let passed = 0;
function test(name, fn) { fn(); passed++; console.log('PASS', name); }
const near = (a, b, tol, what) => assert(Math.abs(a - b) <= tol, `${what}: ${a} vs ${b} (±${tol})`);
const input = over => Object.assign({ phase: 'racing', speed: 0, thrL: 1, thrR: 1, brake: 0, recovery: false }, over);

// Speed in km/h on the simulation's 1/120 s grid, from v0 toward `desired` with response k (2.1 accelerating, 3.4 braking).
function curve(v0, seconds, desired, k) {
  const c = [v0];
  for (let i = 0; i < Math.round(seconds * 120); i++) c.push(c[i] + (desired - c[i]) * (1 - Math.exp(-k / 120)));
  return c;
}
const at = (c, t) => { const x = t * 120, i = Math.min(c.length - 2, Math.floor(x)); return c[i] + (c[i + 1] - c[i]) * (x - i); };
const LAUNCH = curve(68 * 3.6, 3, 600, 2.1);   // GO at full throttle: LAUNCH_SPEED 68 m/s (index.html:1121, 1739)
const BRAKE = curve(600, 1.5, 108, 3.4);       // brake at zero throttle: target 30 m/s (index.html:1874), response 3.4
const RATES = { 30: () => 1 / 30, 60: () => 1 / 60, 120: () => 1 / 120, 144: () => 1 / 144,
  irregular: i => [1 / 30, 1 / 144, 1 / 60, 1 / 90, 1 / 240, 1 / 45][i % 6] };

// A countdown frame, a GO frame at the curve's first speed, then frames of step(i) seconds. Log times are seconds after GO.
// GO reaches the model at the same instant at every rate: in the game it arrives with the first racing frame.
function race(c, seconds, step, over = {}, gears = K, each) {
  const g = E.gears.create(gears), log = [];
  g.advance(1 / 60, input({ phase: 'countdown' }));
  g.advance(0, input(Object.assign({ speed: at(c, 0) }, over)));
  const t0 = g.state.t;
  for (let i = 0, t = 0; t < seconds - 1e-9; i++) {
    const dt = Math.min(step(i), seconds - t); t += dt;
    g.advance(dt, input(Object.assign({ speed: at(c, t) }, over)));
    for (let k = 0; k < g.state.eventCount; k++) {
      const e = g.state.events[k]; log.push({ type: e.type, gear: e.gear, t: e.t - t0 });
    }
    if (each) each(g.state, t);
  }
  return { g, log };
}
const shifts = log => log.map(e => `${e.type} ${e.gear}`);

test('gear from speed: the highest gear whose downshift point (0.92 x the lower top) lies below v, so no shift follows at once', () => {
  const speeds = [0, 73.55, 73.65, 137.95, 138.05, 202.35, 202.45, 275.95, 276.05, 358.75, 358.85, 450.75, 450.85, 600, 1200];
  assert.deepEqual(speeds.map(v => E.gears.fromSpeed(K.tops, v)), [1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 7]);
  for (let v = 0; v <= 1200; v += .5) {
    const g = E.gears.create(K);
    g.advance(0, input({ speed: v }));
    g.advance(1 / 60, input({ speed: v }));
    assert.equal(g.state.eventCount, 0, 'no shift right after GO at ' + v);
  }
});

test('neutral revs chase 41 + 83 x 0.8 x throttle^0.8 with a 0.15 s time constant', () => {
  const g = E.gears.create(K);
  g.advance(.15, input({ phase: 'countdown' }));
  near(g.state.f, 41 + 66.4 * (1 - Math.exp(-1)), 1e-9, 'after 0.15 s at full throttle');
  for (let i = 0; i < 8; i++) g.advance(.25, input({ phase: 'countdown' }));
  near(g.state.f, 107.4, .01, 'full throttle');
  for (let i = 0; i < 8; i++) g.advance(.25, input({ phase: 'menu', thrL: .5, thrR: 0 }));
  near(g.state.f, 41 + 66.4 * Math.pow(.5, .8), .01, 'half throttle on the menu');
  assert.equal(g.state.gear, 0); assert.equal(g.state.eventCount, 0); assert.equal(g.state.tone, g.state.f);
});

test('GO takes the gear from speed with no event, and the paced launch upshifts at 0.19, 0.54 and 0.93 s', () => {
  const g = E.gears.create(K);
  g.advance(1 / 60, input({ phase: 'countdown' }));
  g.advance(0, input({ speed: at(LAUNCH, 0) }));
  assert.equal(g.state.gear, 4); near(g.state.f, 124 * 244.8 / 300, 1e-9, 'GO pitch'); assert.equal(g.state.eventCount, 0);
  const { log } = race(LAUNCH, 1.5, RATES[60]);
  assert.deepEqual(shifts(log), ['upshift 5', 'upshift 6', 'upshift 7']);
  [23, 65, 111].forEach((k, i) => near(log[i].t, k / 120, 1e-9, `upshift to ${i + 5}`));   // 23/120 s < 0.22 s: GO is not a shift
});

test('the paced speed rises at most 250 km/h per second in gears 1-6 and falls at once', () => {
  const g = E.gears.create(K);
  g.advance(1 / 60, input({ phase: 'countdown' }));
  g.advance(0, input({ speed: 245 }));
  g.advance(.1, input({ speed: 400 }));
  near(g.state.vp, 245 + 25, 1e-9, 'paced speed after 0.1 s'); assert.equal(g.state.v, 400);
  near(g.state.f, 124 * 270 / 300, 1e-9, 'pitch follows the paced speed'); assert.equal(g.state.gear, 4);
  g.advance(STEP, input({ speed: 200 }));
  assert.equal(g.state.vp, 200);
  assert.deepEqual([g.state.gear, g.state.eventCount, g.state.events[0].type], [3, 1, 'downshift']);
});

test('the same shifts, within 1/120 s, at 30, 60, 120 and 144 Hz and with irregular frames', () => {
  for (const [c, over] of [[LAUNCH, {}], [BRAKE, { thrL: 0, thrR: 0, brake: 1 }]]) {
    const ref = race(c, 1.5, RATES[120], over).log;
    for (const [name, step] of Object.entries(RATES)) {
      const { log } = race(c, 1.5, step, over);
      assert.deepEqual(shifts(log), shifts(ref), name);
      log.forEach((e, i) => near(e.t, ref[i].t, STEP + 1e-9, `${name} ${e.type} ${e.gear}`));
    }
  }
});

test('braking from cruise downshifts at least 0.09 s apart and ends in 2nd', () => {
  const { g, log } = race(BRAKE, 1.5, RATES[60], { thrL: 0, thrR: 0, brake: 1 });
  assert.deepEqual(shifts(log), ['downshift 6', 'downshift 5', 'downshift 4', 'downshift 3', 'downshift 2']);
  [13, 24, 38, 59, 99].forEach((k, i) => near(log[i].t, k / 120, 1e-9, `downshift to ${6 - i}`));   // 0.11, 0.20, 0.32, 0.49, 0.83 s
  for (let i = 1; i < log.length; i++) assert(log[i].t - log[i - 1].t >= K.downMin - 1e-9, 'down spacing');
  assert.equal(g.state.gear, 2); near(g.state.f, 91.76, .01, 'pitch at 111 km/h in 2nd');
});

test('without pacing, the 0.22 s minimum spaces the upshifts', () => {
  const { log } = race(LAUNCH, 1.5, RATES[60], {}, Object.assign({}, K, { pace: 1e6 }));
  assert.deepEqual(shifts(log), ['upshift 5', 'upshift 6', 'upshift 7']);
  [8, 35, 62].forEach((k, i) => near(log[i].t, k / 120, 1e-9, `unpaced upshift to ${i + 5}`));   // 0.07, 0.29, 0.52 s
  for (let i = 1; i < log.length; i++) assert(log[i].t - log[i - 1].t >= K.upMin - 1e-9, 'up spacing');
});

test('while an upshift waits, the pitch holds at 1.03 x redline', () => {
  const jump = [0].concat(Array(400).fill(300));
  const { g, log } = race(jump, 2, RATES[60], {}, Object.assign({}, K, { upMin: 10 }));
  assert.deepEqual(shifts(log), ['upshift 2']);
  near(g.state.f, 124 * 1.03, 1e-9, 'over-rev cap');
});

test('gear 1 never drops the pitch below idle', () => {
  const g = E.gears.create(K);
  g.advance(0, input({ speed: 10 }));
  assert.equal(g.state.gear, 1); assert.equal(g.state.f, 41);
});

test('boost lift past 700 km/h in 7th: continuous at 700, 190.85 Hz at 1200', () => {
  const s = v => { const g = E.gears.create(K); g.advance(0, input({ speed: v })); return g.state; };
  assert.equal(s(700).gear, 7); near(s(700).f, 124, 1e-9, '700 km/h');
  near(s(699.9).f, s(700.1).f, .05, 'continuous at 700 km/h');
  near(s(1200).f, 124 * Math.pow(1200 / 700, .8), 1e-9, '1200 km/h'); near(s(1200).f, 190.85, .01, '1200 km/h value');
});

test('speed wobbling around a shift point shifts once', () => {
  const wob = Array.from({ length: 3.5 * 120 + 1 }, (_, i) => { const t = i / 120; return t < .5 ? 245 + 110 * t : 290 + 10 * Math.sin(4 * Math.PI * (t - .5)); });
  const { log } = race(wob, 3.5, RATES[60]);
  assert.deepEqual(shifts(log), ['upshift 5']);
});

test('the reported pitch moves smoothly except at shifts', () => {
  for (const [c, over] of [[LAUNCH, {}], [BRAKE, { thrL: 0, thrR: 0, brake: 1 }]]) {
    let prev = null, worst = 0;
    race(c, 1.5, RATES[120], over, K, st => {
      if (prev !== null) {
        const step = Math.abs(Math.log(st.f / prev));
        if (st.eventCount) assert(step > Math.log(1.15), `a shift moves the pitch by ${step}`);
        else worst = Math.max(worst, step);
      }
      prev = st.f;
    });
    assert(worst < Math.log(1.03), `smooth step ${worst}`);
  }
});

test('upshift: 40 ms cut at the old pitch, then a 60 ms log glide; downshift: 80 ms blip and a 40 ms glide', () => {
  const rows = [];
  race(LAUNCH, .5, RATES[120], {}, K, st => rows.push({ ev: st.eventCount ? st.events[0].type : '', cut: st.cut, blip: st.blip, tone: st.tone, f: st.f }));
  const s = rows.findIndex(r => r.ev === 'upshift'), from = rows[s - 1].tone;
  for (let j = s; j < s + 5; j++) { assert.equal(rows[j].cut, 1, 'cut ' + j); assert.equal(rows[j].tone, from, 'held ' + j); }
  assert.equal(rows[s + 5].cut, 0);
  for (let j = s + 5; j < s + 12; j++) assert(rows[j].tone < rows[j - 1].tone && rows[j].tone > rows[j].f, 'glide ' + j);
  assert.equal(rows[s + 12].tone, rows[s + 12].f);
  const down = [];
  race(BRAKE, .5, RATES[120], { thrL: 0, thrR: 0, brake: 1 }, K, st => down.push({ ev: st.eventCount ? st.events[0].type : '', blip: st.blip, tone: st.tone, f: st.f }));
  const d = down.findIndex(r => r.ev === 'downshift'), dfrom = down[d - 1].tone;
  for (let j = d; j < d + 10; j++) assert.equal(down[j].blip, 1, 'blip ' + j);
  assert.equal(down[d + 10].blip, 0);
  assert.equal(down[d].tone, dfrom);
  for (let j = d + 1; j < d + 5; j++) assert(down[j].tone > down[j - 1].tone && down[j].tone < down[j].f, 'down glide ' + j);
  assert.equal(down[d + 5].tone, down[d + 5].f);
});

test('recovery sets the gear from speed with no shift events', () => {
  const g = E.gears.create(K);
  g.advance(0, input({ speed: 600 })); assert.equal(g.state.gear, 7);
  g.advance(.1, input({ speed: 150, recovery: true }));
  assert.deepEqual([g.state.gear, g.state.eventCount, g.state.tone], [3, 0, g.state.f]); near(g.state.vp, 150, 1e-9, 'paced speed');
});

test('reset returns to the state of a new model', () => {
  const { g } = race(LAUNCH, 1, RATES[60]); g.reset();
  const fresh = E.gears.create(K).state, keys = ['gear', 'f', 'tone', 'vp', 'v', 'vIn', 'carry', 't', 'cut', 'blip', 'eventCount', 'shiftDir'];
  assert.deepEqual(keys.map(k => g.state[k]), keys.map(k => fresh[k])); assert.deepEqual([...g.state.load], [0, 0]);
});

test('each load follows its own fader (55 ms up, 110 ms down); braking scales it by 1 - 0.45 x brake', () => {
  const g = E.gears.create(K);
  g.advance(0, input({ speed: 300 }));
  g.advance(6 / 120, input({ speed: 300, thrL: 1, thrR: 0 }));
  near(g.state.load[0], 1 - Math.exp(-(6 / 120) / .055), 1e-9, 'left rising'); assert.equal(g.state.load[1], 0);
  for (let i = 0; i < 8; i++) g.advance(.25, input({ speed: 300, thrL: 1, thrR: 0, brake: 1 }));
  near(g.state.load[0], .55, 1e-6, 'braked');
  g.advance(12 / 120, input({ speed: 300, thrL: 0, thrR: 0 }));
  near(g.state.load[0], .55 * Math.exp(-(12 / 120) / .11), 1e-6, 'falling'); assert(g.state.load[0] < .25, 'off-load');
});

test('bad input keeps the state finite and the pitch between idle and the lift at 1500 km/h', () => {
  const g = E.gears.create(K), odd = [NaN, Infinity, -Infinity, -5, 1e9, undefined, null, 'x', {}];
  let seed = 7; const r = () => (seed = (seed * 1103515245 + 12345) >>> 0) / 4294967296;
  const pick = scale => r() < .3 ? odd[Math.floor(r() * odd.length)] : r() * scale;
  for (let i = 0; i < 3000; i++) {
    g.advance([1 / 60, 0, NaN, -1, 5, 1 / 144][i % 6], i % 97 ? { phase: ['racing', 'countdown', 'menu', 'finished', undefined, 7][i % 6],
      speed: pick(1300), thrL: pick(1.2), thrR: pick(1.2), brake: pick(1.2), recovery: r() < .05 } : null);
    for (const k of ['f', 'tone', 'vp', 'v', 't', 'carry']) assert(Number.isFinite(g.state[k]), k);
    assert(g.state.f >= 41 - 1e-9 && g.state.f <= 124 * Math.pow(1500 / 700, .8) + 1e-9, 'f ' + g.state.f);
    assert(g.state.gear >= 0 && g.state.gear <= 7 && g.state.load.every(l => l >= 0 && l <= 1));
  }
});

console.log(`ship gears: ${passed} passed`);
```

- [ ] **Step 2: Run it to see it fail.**

```bash
node prototypes/Dustline/checks/ship-gears.test.cjs
```

Expected: `TypeError: Cannot read properties of undefined (reading 'GEAR_DT')`.

- [ ] **Step 3: Create the gear piece and step 2.**

Create `prototypes/Dustline/tools/engine2/pieces/gears.js`:

```js
  /* ---- 2.0 gear model: seven gears, a paced speed and the boost lift. Pure and deterministic, with no allocation per step;
   * both tone backends read it. Speed in km/h. It advances in fixed 1/120 s substeps and interpolates the speed across each
   * update, so shift times do not depend on the frame rate. ---- */
  const GEAR_DT=1/120,CUT_S=.04,GLIDE_S=.06,DOWN_GLIDE_S=.04,BLIP_S=.08;
  // Gear from speed: the highest gear whose downshift point lies below v, so neither shift rule fires at once.
  const gearFromSpeed=(tops,v)=>{let g=1;while(g<tops.length&&v>.92*tops[g-1])g++;return g;};
  function createGears(gp){
    const tops=gp.tops,N=tops.length,TOP=tops[N-1];
    const events=[0,1,2,3].map(()=>({type:'',t:0,gear:0}));
    const st={gear:0,f:gp.idleHz,tone:gp.idleHz,vp:0,v:0,vIn:null,carry:0,t:0,lastShift:-1e9,shiftAt:-1e9,shiftDir:0,
      shiftFrom:gp.idleHz,cut:0,blip:0,load:[0,0],events,eventCount:0};
    const push=type=>{if(st.eventCount<events.length){const e=events[st.eventCount++];e.type=type;e.t=st.t;e.gear=st.gear;}};
    const pitch=(g,vp)=>g===N&&vp>TOP?gp.redlineHz*Math.pow(vp/TOP,gp.boostExponent):
      Math.max(gp.idleHz,gp.redlineHz*Math.min(vp/tops[g-1],g<N?gp.overRev:1));
    function sub(v,inp){
      st.t+=GEAR_DT;
      const thrL=sat(fin(inp.thrL,0)),thrR=sat(fin(inp.thrR,0)),brake=sat(fin(inp.brake,0)),thr=Math.max(thrL,thrR);
      st.load[0]=loadStep(st.load[0],thrL*(1-.45*brake),GEAR_DT);st.load[1]=loadStep(st.load[1],thrR*(1-.45*brake),GEAR_DT);
      const rising=v>st.v+1e-9;st.v=v;
      if(inp.phase==='menu'||inp.phase==='countdown'){
        const target=gp.idleHz+(gp.redlineHz-gp.idleHz)*.8*Math.pow(thr,.8);
        st.gear=0;st.vp=v;st.lastShift=st.shiftAt=-1e9;st.shiftDir=0;st.cut=st.blip=0;
        st.f+=(target-st.f)*(1-Math.exp(-GEAR_DT/.15));st.tone=st.f;return;
      }
      if(inp.recovery){st.gear=gearFromSpeed(tops,v);st.vp=v;st.shiftDir=0;}
      else{
        st.vp=st.gear<N&&v>st.vp?Math.min(v,st.vp+gp.pace*GEAR_DT):v;
        const since=st.t-st.lastShift;
        if(st.gear<N&&st.vp>=.97*tops[st.gear-1]&&(rising||thr>=.3)&&since>=gp.upMin-1e-9){
          st.shiftFrom=st.tone;st.gear++;if(st.gear===N)st.vp=v;
          st.lastShift=st.shiftAt=st.t;st.shiftDir=1;push('upshift');
        }else if(st.gear>1&&st.vp<=.92*tops[st.gear-2]&&since>=gp.downMin-1e-9){
          st.shiftFrom=st.tone;st.gear--;st.lastShift=st.shiftAt=st.t;st.shiftDir=-1;push('downshift');
        }
      }
      st.f=pitch(st.gear,st.vp);
      // Shift shaping: the tone holds through the ignition cut, then glides in log pitch. The reported f never glides.
      const age=st.t-st.shiftAt,up=st.shiftDir>0,lf=Math.log(st.f),l0=Math.log(st.shiftFrom);
      st.cut=up&&age<CUT_S-1e-9?1:0;st.blip=st.shiftDir<0&&age<BLIP_S-1e-9?1:0;
      st.tone=st.cut?st.shiftFrom:up&&age<CUT_S+GLIDE_S-1e-9?Math.exp(l0+(lf-l0)*(age-CUT_S)/GLIDE_S):
        st.shiftDir<0&&age<DOWN_GLIDE_S-1e-9?Math.exp(l0+(lf-l0)*age/DOWN_GLIDE_S):st.f;
    }
    return{state:st,
      advance(dt,inp){
        inp=inp||{};st.eventCount=0;
        dt=clamp(fin(dt,0),0,.25);
        const v1=clamp(fin(inp.speed,0),0,1500),neutral=inp.phase==='menu'||inp.phase==='countdown';
        if(st.vIn===null)st.vIn=st.v=v1;
        // GO: the gear comes from the first racing frame's speed. The countdown standstill is not blended into the launch,
        // and GO does not count as a shift.
        if(!neutral&&st.gear===0){st.gear=gearFromSpeed(tops,v1);st.vIn=st.v=st.vp=v1;st.shiftDir=0;st.f=st.tone=pitch(st.gear,v1);}
        if(dt<=0)return st;
        const v0=st.vIn;let x=GEAR_DT-st.carry,n=0;
        while(x<=dt+1e-9){sub(v0+(v1-v0)*Math.min(1,x/dt),inp);x+=GEAR_DT;n++;}
        st.carry=Math.max(0,dt+st.carry-n*GEAR_DT);st.vIn=v1;
        return st;
      },
      reset(){
        Object.assign(st,{gear:0,f:gp.idleHz,tone:gp.idleHz,vp:0,v:0,vIn:null,carry:0,t:0,lastShift:-1e9,shiftAt:-1e9,shiftDir:0,
          shiftFrom:gp.idleHz,cut:0,blip:0,eventCount:0});
        st.load[0]=st.load[1]=0;
      }};
  }
```

Create `prototypes/Dustline/tools/engine2/step2_gears.py`:

```python
"""Engine 2.0, step 2 (plan Task 3): the gear model, exported as DustlineEngineAudio.gears. Nothing plays it yet."""
from common import Build

b = Build()
b.rep("  const loadStep=(l,target,dt)=>ease(l,target,dt,.055,.11);\n\n",
      "  const loadStep=(l,target,dt)=>ease(l,target,dt,.055,.11);\n\n" + b.piece("gears.js") + "\n")
b.rep("    ships:Object.freeze({kestrel:KESTREL}),buffersFor});\n",
      "    gears:Object.freeze({create:createGears,fromSpeed:gearFromSpeed,GEAR_DT}),ships:Object.freeze({kestrel:KESTREL}),buffersFor});\n")
b.save()
print("step 2: the gear model is in the engine block")
```

- [ ] **Step 4: Run step 2.**

```bash
python prototypes/Dustline/tools/engine2/step2_gears.py
```

Expected: `pushed <sha256> <bytes> bytes`, then `step 2: the gear model is in the engine block`.

- [ ] **Step 5: Run the test to see it pass.**

```bash
node prototypes/Dustline/checks/ship-gears.test.cjs
```

Expected: seventeen `PASS` lines, then `ship gears: 17 passed`.

- [ ] **Step 6: Run the regression (0.6).** Expected: 155 checks (`ship-gears.test.cjs: ship gears: 17 passed` added) and both ai-pace `PASS` lines.

### Task 4: Bake tool

**Purpose.** `crates/lpd-core/examples/bake_engine.rs` renders the patch with the app's own `Synth`, offline (spec 4.2; fix 9; S13, S14, S43). `check` parses the patch as the app does and fails if a value had to be clamped. `loops` renders the eight phase-locked loops: note 48 held in LEGATO with bend 24, the bend putting the fundamental on each anchor (measured from 65.41 Hz), 3.25 s rendered and the first 1.0 s dropped, the seam crossfaded for the measured correlation, 32-bit float stereo WAV at 48 kHz, 100800 samples each, plus `bake.json`. Its five unit tests cover the bend, a seamless periodic loop, power through the crossfade, the correlation and a rendered note on its anchor. Task 18 adds the `scenario` mode.

**Files:**
- Create: `launchpad-drummer/crates/lpd-core/examples/bake_engine.rs`

- [ ] **Step 1: See that the example does not exist yet** (PowerShell, from `launchpad-drummer`, cargo PATH set as in 0.1).

```powershell
cargo test -p lpd-core --example bake_engine
```

Expected: `` error: no example target named `bake_engine` ``.

- [ ] **Step 2: Create the bake tool and its tests.** Create `launchpad-drummer/crates/lpd-core/examples/bake_engine.rs`:

```rust
//! Dustline ship engine bake tool (spec `2026-11-05-dustline-ship-engine-design.md`, section 4.2).
//!
//!   cargo run -p lpd-core --release --example bake_engine -- check --patch <file.lbpatch>
//!   cargo run -p lpd-core --release --example bake_engine -- loops --patch <file.lbpatch> --out <dir>
//!
//! `check` parses the patch the way the app does (`Patch::from_json`) and fails if any value had to be clamped.
//! `loops` renders the eight phase-locked tone loops (4 anchors x on/off load) as 32-bit float stereo WAV files at
//! 48 kHz, 100800 samples each: 0.05 s lead-in, the 2 s loop, then 0.05 s of what follows the loop end, and
//! writes `bake.json` beside them.

use anyhow::{bail, Context, Result};
use lpd_core::instrument::Instrument;
use lpd_core::synth::params::{SynthParams, VoiceMode};
use lpd_core::synth::patch::Patch;
use lpd_core::synth::Synth;
use std::path::{Path, PathBuf};

const RATE: u32 = 48_000;
/// The held note, its velocity and the forced bend range (the LIVE SYNTH voice uses the same, spec 5.1).
const GAME_NOTE: u8 = 48;
const GAME_VELOCITY: u8 = 127;
const GAME_BEND_RANGE: u8 = 24;
const BLOCK: usize = 256;
/// Frequency of the held note's fundamental at bend 0: note 48 sounds twice it.
const BEND_ZERO_HZ: f64 = 65.406_391_325_149_66;
const ANCHORS: [u32; 4] = [48, 75, 117, 182];
const LOADS: [(&str, f32); 2] = [("on", 0.85), ("off", 0.2)];
const RENDER_S: f64 = 3.25;
/// Dropped from the note start: whole periods of every integer anchor, so all loops start in phase.
const DROP: usize = 48_000;
const LOOP: usize = 96_000;
const MARGIN: usize = 2_400;
const SEAM: usize = 7_200;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn need(args: &[String], name: &str) -> Result<String> {
    arg(args, name).with_context(|| format!("missing {name}"))
}

/// The patch as LIVE SYNTH plays it: parsed like the app, LEGATO and a 24-semitone bend forced.
fn load_patch(path: &Path) -> Result<(Patch, SynthParams)> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let patch = Patch::from_json(&text)?;
    let mut params = patch.params;
    params.voice.mode = VoiceMode::Legato;
    params.voice.bend_range = GAME_BEND_RANGE;
    Ok((patch, params))
}

/// Bend (−1…1 of 24 semitones) that puts the fundamental at `hz` (spec 5.3).
fn bend_for(hz: f64) -> f32 {
    (12.0 * (hz / BEND_ZERO_HZ).log2() / f64::from(GAME_BEND_RANGE)) as f32
}

fn check(args: &[String]) -> Result<()> {
    let path = PathBuf::from(need(args, "--patch")?);
    let text = std::fs::read_to_string(&path)?;
    let raw: Patch = serde_json::from_str(&text).context("not a patch file")?;
    let clean = Patch::from_json(&text)?;
    let p = (&raw.params, &clean.params);
    let mut changed = Vec::new();
    macro_rules! section {
        ($($f:ident),*) => { $( if p.0.$f != p.1.$f { changed.push(stringify!($f)); } )* };
    }
    section!(osc_a, osc_b, sub, noise, filter, amp, env, lfo, voice, routes, fx);
    if raw.name != clean.name {
        changed.push("name");
    }
    if !changed.is_empty() {
        bail!("values out of range in: {}", changed.join(", "));
    }
    if clean.params.voice.mode != VoiceMode::Legato
        || clean.params.voice.bend_range != GAME_BEND_RANGE
    {
        println!("note: LIVE SYNTH and the bake force LEGATO and a 24-semitone bend");
    }
    println!(
        "patch ok: {} ({}), nothing clamped",
        clean.name,
        path.display()
    );
    Ok(())
}

fn render_note(params: &SynthParams, hz: f64, wheel: f32, seconds: f64) -> (Vec<f32>, Vec<f32>) {
    let mut synth = Synth::new(RATE);
    synth.load_params(params);
    let (mut l, mut r) = (vec![0.0f32; BLOCK], vec![0.0f32; BLOCK]);
    for _ in 0..8 {
        synth.render_add(&mut l, &mut r); // the 5 ms load fade, before the note
    }
    synth.pitch_bend(bend_for(hz));
    synth.modulation(wheel);
    synth.note_on(GAME_NOTE, GAME_VELOCITY);
    let n = (seconds * f64::from(RATE)).round() as usize;
    let (mut outl, mut outr) = (vec![0.0f32; n], vec![0.0f32; n]);
    for (cl, cr) in outl.chunks_mut(BLOCK).zip(outr.chunks_mut(BLOCK)) {
        synth.render_add(cl, cr);
    }
    (outl, outr)
}

/// Correlation of two stereo segments (both channels together).
fn correlation(a: (&[f32], &[f32]), b: (&[f32], &[f32])) -> f64 {
    let (mut ab, mut aa, mut bb) = (0.0f64, 0.0f64, 0.0f64);
    for (x, y) in a.0.iter().zip(b.0).chain(a.1.iter().zip(b.1)) {
        let (x, y) = (f64::from(*x), f64::from(*y));
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    if aa == 0.0 || bb == 0.0 {
        0.0
    } else {
        ab / (aa * bb).sqrt()
    }
}

/// The loop file: lead-in, the loop with its end crossfaded into what precedes the loop start, then the loop start
/// again (what follows the seam). The crossfade keeps the power constant for the measured correlation `rho`.
fn make_loop(x: &[f32], rho: f64) -> Vec<f32> {
    let a = DROP;
    let b = a + LOOP;
    let mut body = x[a..b].to_vec();
    for k in 0..SEAM {
        let th = (k as f64 + 0.5) / SEAM as f64 * std::f64::consts::FRAC_PI_2;
        let (we, wp) = (th.cos(), th.sin());
        let g = 1.0 / (we * we + wp * wp + 2.0 * rho * we * wp).sqrt();
        let i = LOOP - SEAM + k;
        body[i] = (g * (we * f64::from(x[a + i]) + wp * f64::from(x[a - SEAM + k]))) as f32;
    }
    let mut out = Vec::with_capacity(LOOP + 2 * MARGIN);
    out.extend_from_slice(&x[a - MARGIN..a]);
    out.extend_from_slice(&body);
    out.extend_from_slice(&x[a..a + MARGIN]);
    out
}

fn write_wav(path: &Path, l: &[f32], r: &[f32]) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for (a, b) in l.iter().zip(r) {
        w.write_sample(*a)?;
        w.write_sample(*b)?;
    }
    w.finalize()?;
    Ok(())
}

fn loops(args: &[String]) -> Result<()> {
    let (patch, params) = load_patch(Path::new(&need(args, "--patch")?))?;
    let out = PathBuf::from(need(args, "--out")?);
    std::fs::create_dir_all(&out)?;
    let mut manifest = Vec::new();
    for hz in ANCHORS {
        for (load, wheel) in LOADS {
            let (l, r) = render_note(&params, f64::from(hz), wheel, RENDER_S);
            let (a, b) = (DROP, DROP + LOOP);
            let rho = correlation(
                (&l[b - SEAM..b], &r[b - SEAM..b]),
                (&l[a - SEAM..a], &r[a - SEAM..a]),
            );
            let (ll, rr) = (make_loop(&l, rho), make_loop(&r, rho));
            let id = format!("core-{hz:03}-{load}");
            write_wav(&out.join(format!("{id}.wav")), &ll, &rr)?;
            let peak = ll.iter().chain(&rr).fold(0.0f32, |m, v| m.max(v.abs()));
            println!("{id}: {hz} Hz, wheel {wheel}, seam correlation {rho:.3}, peak {peak:.3}");
            manifest.push(serde_json::json!({
                "id": id, "anchorHz": hz, "load": load, "wheel": wheel, "bend": bend_for(f64::from(hz)),
                "rate": RATE, "samples": ll.len(), "loopStart": MARGIN, "loopEnd": MARGIN + LOOP,
                "seamCorrelation": rho, "peak": peak
            }));
        }
    }
    let doc = serde_json::json!({ "patch": patch.name, "loops": manifest });
    std::fs::write(
        out.join("bake.json"),
        serde_json::to_string_pretty(&doc)? + "\n",
    )?;
    println!(
        "wrote {} loops to {}",
        ANCHORS.len() * LOADS.len(),
        out.display()
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check") => check(&args),
        Some("loops") => loops(&args),
        _ => bail!("usage: bake_engine check|loops --patch <file> [--out <dir>]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f64, seconds: f64) -> Vec<f32> {
        let n = (seconds * f64::from(RATE)) as usize;
        (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * hz * i as f64 / f64::from(RATE)).sin() as f32)
            .collect()
    }

    #[test]
    fn the_bend_puts_the_fundamental_on_the_anchor() {
        assert!(bend_for(BEND_ZERO_HZ).abs() < 1e-7);
        for hz in [41.0, 48.0, 75.0, 117.0, 182.0, 191.0] {
            let back = BEND_ZERO_HZ
                * 2f64.powf(f64::from(bend_for(hz)) * f64::from(GAME_BEND_RANGE) / 12.0);
            assert!((back / hz - 1.0).abs() < 1e-6, "{hz}");
            assert!(bend_for(hz).abs() <= 1.0);
        }
    }

    #[test]
    fn a_periodic_signal_loops_unchanged_and_seamless() {
        let x = sine(75.0, RENDER_S);
        let (a, b) = (DROP, DROP + LOOP);
        let rho = correlation(
            (&x[b - SEAM..b], &x[b - SEAM..b]),
            (&x[a - SEAM..a], &x[a - SEAM..a]),
        );
        assert!(rho > 0.999, "{rho}");
        let out = make_loop(&x, rho);
        assert_eq!(out.len(), LOOP + 2 * MARGIN);
        for (i, v) in out.iter().enumerate() {
            assert!((v - x[DROP - MARGIN + i]).abs() < 1e-3, "sample {i}");
        }
        // across the seam the loop continues the sine: the step equals the step inside the loop
        let step = |p: f32, q: f32| (q - p).abs();
        let max_step = out.windows(2).map(|w| step(w[0], w[1])).fold(0.0, f32::max);
        assert!(step(out[MARGIN + LOOP - 1], out[MARGIN]) <= max_step * 1.01);
        // the margins: what precedes the loop start, and the loop start again after the loop end
        assert_eq!(&out[LOOP + MARGIN..], &out[MARGIN..2 * MARGIN]);
    }

    #[test]
    fn uncorrelated_ends_keep_their_power_through_the_crossfade() {
        let mut seed = 7u32;
        let x: Vec<f32> = (0..(RENDER_S * f64::from(RATE)) as usize)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5
            })
            .collect();
        let (a, b) = (DROP, DROP + LOOP);
        let rho = correlation(
            (&x[b - SEAM..b], &x[b - SEAM..b]),
            (&x[a - SEAM..a], &x[a - SEAM..a]),
        );
        assert!(rho.abs() < 0.05, "{rho}");
        let out = make_loop(&x, rho);
        let rms = |s: &[f32]| {
            (s.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / s.len() as f64).sqrt()
        };
        let body = rms(&out[MARGIN..MARGIN + LOOP - SEAM]);
        let mid = rms(&out[MARGIN + LOOP - SEAM / 2 - 1200..MARGIN + LOOP - SEAM / 2 + 1200]);
        assert!(
            (20.0 * (mid / body).log10()).abs() < 0.5,
            "crossfade {mid} body {body}"
        );
    }

    #[test]
    fn correlation_is_one_for_equal_and_minus_one_for_inverted() {
        let x = sine(48.0, 0.5);
        let y: Vec<f32> = x.iter().map(|v| -v).collect();
        assert!((correlation((&x, &x), (&x, &x)) - 1.0).abs() < 1e-9);
        assert!((correlation((&x, &x), (&y, &y)) + 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_rendered_note_sits_on_its_anchor() {
        let mut p = SynthParams::default();
        p.osc_a.on = false;
        p.osc_b.on = false;
        p.sub.on = true;
        p.sub.oct = -1;
        p.voice.mode = VoiceMode::Legato;
        p.voice.bend_range = GAME_BEND_RANGE;
        let (l, _) = render_note(&p, 75.0, 0.5, RENDER_S);
        let body = &l[DROP..DROP + LOOP];
        let crossings = body
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count();
        assert!(
            (149..=151).contains(&crossings),
            "{crossings} upward crossings in 2 s"
        );
    }
}
```

- [ ] **Step 3: Run its tests.**

```powershell
cargo test -p lpd-core --example bake_engine
```

Expected: `test result: ok. 5 passed; 0 failed` (`the_bend_puts_the_fundamental_on_the_anchor`, `a_periodic_signal_loops_unchanged_and_seamless`, `uncorrelated_ends_keep_their_power_through_the_crossfade`, `correlation_is_one_for_equal_and_minus_one_for_inverted`, `a_rendered_note_sits_on_its_anchor`).

- [ ] **Step 4: Check the Kestrel patch.**

```powershell
cargo run -p lpd-core --release --example bake_engine -- check --patch ..\prototypes\Dustline\audio\ships\kestrel\kestrel-engine.lbpatch
```

Expected: `patch ok: KESTREL ENGINE (..\prototypes\Dustline\audio\ships\kestrel\kestrel-engine.lbpatch), nothing clamped`.

- [ ] **Step 5: Format check.**

```powershell
cargo fmt --all -- --check
```

Expected: no output.

### Task 5: Bake and screen the loops

**Purpose.** Render the eight loops and measure them before the engine uses them (spec 4.2, 4.4; S13, S14, S45). `checks/pitch.py` is the pitch tracker (earshot reports no f0 under 50 Hz). `checks/ship_screen.py loops` measures layout, pitch, drift, seam, seam correlation, peak, band shares, loudness, brightness, the crossfade midpoints and earshot `describe`, writes `audio/ships/kestrel/LOG.md` and `loops/analysis.json`, and fails when a loop is more than 1 % off its anchor, a seam reaches 0.02 or a crossfade midpoint dips below −1 dB. Task 8 adds the `audition` mode and Task 12 the `takes` mode.

**Files:**
- Create: `prototypes/Dustline/checks/pitch.py`, `prototypes/Dustline/checks/ship_screen.py`
- Generated: `prototypes/Dustline/audio/ships/kestrel/loops/core-{048,075,117,182}-{on,off}.wav`, `loops/bake.json`, `loops/analysis.json`, `audio/ships/kestrel/LOG.md`

- [ ] **Step 1: Create the pitch tracker and run its self-check.** Create `prototypes/Dustline/checks/pitch.py`:

```python
"""Pitch track of a mono signal (ship engine plan, S45). numpy only; used by ship_screen.py and audio-browser.py.

Frames of 100 ms every 50 ms; in each, the normalized square difference function (McLeod) over 30-1500 Hz. The period is the
first local maximum within 90 % of the highest one; a frame is periodic when that highest maximum reaches 0.6.
    python prototypes/Dustline/checks/pitch.py     self-check on synthetic signals
"""
import numpy as np


def track(x, sr, fmin=30.0, fmax=1500.0, frame=0.1, hop=0.05, threshold=0.6):
    """[(time s, f0 Hz or None, clarity 0..1)] for each frame."""
    x = np.asarray(x, dtype=np.float64)
    n, h = int(round(frame * sr)), int(round(hop * sr))
    lo, hi = max(2, int(sr / fmax)), min(int(sr / fmin), n - 2)
    k = np.arange(hi + 2)
    out = []
    for i in range(0, len(x) - n + 1, h):
        s = x[i:i + n] - x[i:i + n].mean()
        c = np.concatenate([[0.0], np.cumsum(s * s)])
        if c[-1] <= 1e-12:
            out.append((i / sr, None, 0.0))
            continue
        r = np.fft.irfft(np.abs(np.fft.rfft(s, 2 * n)) ** 2)[:hi + 2]
        d = 2 * r / np.maximum(c[n - k] + (c[n] - c[k]), 1e-12)
        seg = d[lo:hi + 2]
        peaks = [j for j in range(1, len(seg) - 1) if seg[j] >= seg[j - 1] and seg[j] > seg[j + 1]]
        if not peaks:
            out.append((i / sr, None, 0.0))
            continue
        top = max(seg[j] for j in peaks)
        j = next(j for j in peaks if seg[j] >= .9 * top)
        a, b, e = seg[j - 1], seg[j], seg[j + 1]
        bend = a - 2 * b + e
        lag = lo + j + (0.5 * (a - e) / bend if bend else 0.0)
        out.append((i / sr, sr / lag if top >= threshold else None, float(top)))
    return out


def summary(x, sr, **kw):
    """Median f0 of the periodic frames, its drift (the larger distance from the median to the 5th or 95th percentile, as a
    fraction of the median) and the periodic fraction of frames. f0 0 and drift None when no frame is periodic."""
    t = track(x, sr, **kw)
    f = np.array([f0 for _, f0, _ in t if f0])
    if len(f) == 0:
        return {"f0": 0.0, "drift": None, "periodic": 0.0}
    med = float(np.median(f))
    p5, p95 = np.percentile(f, [5, 95])
    return {"f0": med, "drift": float(max(p95 - med, med - p5) / med), "periodic": len(f) / len(t)}


if __name__ == "__main__":
    sr = 48000
    t = np.arange(sr * 2) / sr
    rng = np.random.default_rng(7)
    for hz in (41.0, 48.0, 75.0, 117.0, 191.0, 650.0):
        x = np.sin(2 * np.pi * hz * t) + .6 * np.sin(4 * np.pi * hz * t) + .3 * np.sign(np.sin(6 * np.pi * hz * t))
        s = summary(x + .05 * rng.standard_normal(len(t)), sr)
        assert abs(s["f0"] / hz - 1) < .005 and s["drift"] < .005 and s["periodic"] > .95, (hz, s)
    glide = np.sin(2 * np.pi * np.cumsum(100 * (1 + .1 * t / t[-1])) / sr)
    assert summary(glide, sr)["drift"] > .03, "a 10 % glide drifts"
    assert summary(rng.standard_normal(len(t)), sr)["periodic"] < .2, "noise is not periodic"
    print("pitch self-check passed")
```

```bash
python prototypes/Dustline/checks/pitch.py
```

Expected: `pitch self-check passed`.

- [ ] **Step 2: Bake the loops** (PowerShell, from `launchpad-drummer`).

```powershell
cargo run -p lpd-core --release --example bake_engine -- loops --patch ..\prototypes\Dustline\audio\ships\kestrel\kestrel-engine.lbpatch --out ..\prototypes\Dustline\audio\ships\kestrel\loops
```

Expected: eight lines of the form `core-048-on: 48 Hz, wheel 0.85, seam correlation 0.99…, peak 0.3…` (wheel 0.85 on load, 0.2 off load), then `wrote 8 loops to ..\prototypes\Dustline\audio\ships\kestrel\loops`.

- [ ] **Step 3: Create the screening tool.** Create `prototypes/Dustline/checks/ship_screen.py`:

```python
"""earshot screening of the Kestrel's assets (ship engine spec 4.4). Runs in the earshot venv; the models load once per run.

    $PY prototypes/Dustline/checks/ship_screen.py loops           the 8 baked synth loops: layout, pitch, seams, phase, bands, earshot
    $PY prototypes/Dustline/checks/ship_screen.py takes           the 42 ElevenLabs takes: fit, trims, steadiness, the pick rule
    $PY prototypes/Dustline/checks/ship_screen.py audition DIR    earshot timeline and A/B of an audition folder (audio_audition.py)

$PY is "D:\\MY DOCS\\MY TOOLS\\earshot\\.venv\\Scripts\\python.exe" -I. Every number goes into audio/ships/kestrel/LOG.md (one section
per mode, replaced on each run) and a JSON file: loops/analysis.json, picks.json, or DIR/earshot.json.
Measurements only. They do not say how anything sounds; the user's listening decides.
"""
import json
import re
import subprocess
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pitch  # noqa: E402

DIR = HERE.parent
SHIP = DIR / "audio" / "ships" / "kestrel"
LOOPS, RAW, LOG = SHIP / "loops", SHIP / "raw", SHIP / "LOG.md"
SR = 48000
ANCHORS = (48, 75, 117, 182)


def earshot():
    from earshot import api
    return api


def pcm(path, channels=2):
    """Any audio file as float64 (n, channels) at 48 kHz, decoded by ffmpeg."""
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", str(path), "-f", "f32le", "-ac", str(channels), "-ar", str(SR), "-"],
                         capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype="<f4").reshape(-1, channels).astype(np.float64)


def rms(x):
    return float(np.sqrt(np.mean(np.square(x)) + 1e-24))


def db(v):
    return float(20 * np.log10(max(v, 1e-12)))


def shares(mono):
    """Energy shares below 200 Hz and between 1 and 4 kHz."""
    spec = np.abs(np.fft.rfft(mono * np.hanning(len(mono)))) ** 2
    f = np.fft.rfftfreq(len(mono), 1 / SR)
    total = spec.sum() + 1e-24
    return float(spec[f < 200].sum() / total), float(spec[(f >= 1000) & (f < 4000)].sum() / total)


def log_section(title, body):
    """Replace or append the '## title' section of LOG.md."""
    text = LOG.read_text(encoding="utf-8") if LOG.exists() else "# Kestrel asset log\n\nMeasurements only. They do not say how anything sounds.\n"
    block = f"## {title}\n\n{body.rstrip()}\n"
    pat = re.compile(rf"(?ms)^## {re.escape(title)}\n.*?(?=^## |\Z)")
    text = pat.sub(lambda _: block + "\n", text) if pat.search(text) else text.rstrip() + "\n\n" + block
    LOG.write_text(text, encoding="utf-8")


def loops():
    api = earshot()
    man = json.loads((LOOPS / "bake.json").read_text(encoding="utf-8"))
    rows = {}
    for e in man["loops"]:
        path = LOOPS / f"{e['id']}.wav"
        x = pcm(path)
        s, t, hz = e["loopStart"], e["loopEnd"], e["anchorHz"]
        assert x.shape == (100800, 2) and (s, t) == (2400, 98400), (e["id"], x.shape, s, t)
        body = x[s:t].mean(axis=1)
        p = pitch.summary(body, SR)
        # the loop end must continue as the loop start (the tail is a copy of it): same measure as audio-browser.py
        seam = max(rms(x[t - 32:t + 32, c] - x[s - 32:s + 32, c]) / rms(x[s:t, c]) for c in (0, 1))
        one = body[:SR]  # one second holds whole periods of every integer anchor
        z = np.sum(one * np.exp(-2j * np.pi * hz * np.arange(SR) / SR)) * 2 / SR
        low, zing = shares(body)
        m = api.measure(str(path), start=s / SR, end=t / SR)
        d = api.describe(str(path), start=s / SR, end=t / SR, top=5)
        rows[e["id"]] = {"anchorHz": hz, "load": e["load"], "f0": round(p["f0"], 2), "drift": p["drift"], "seam": round(seam, 4),
                         "rho": round(e["seamCorrelation"], 4), "peak": round(e["peak"], 3), "amp": float(abs(z)),
                         "phaseDeg": round(float(np.degrees(np.angle(z))), 1), "below200": round(low, 3), "zing1to4k": round(zing, 4),
                         "lufs": m["lufs"], "brightnessHz": m["brightness_hz"], "centroidHz": m["centroid_hz"],
                         "describe": d.get("summary", "")}
    # The engine crossfades neighbouring anchors (same load) and on/off (same anchor) with equal gain: the fundamental's
    # level at the midpoint shows whether the loops are in phase (0 dB) or cancel.
    def midpoint(a, b):
        A, B = rows[a], rows[b]
        dphi = np.radians(B["phaseDeg"] - A["phaseDeg"])
        return round(db(abs(.5 * A["amp"] + .5 * B["amp"] * np.exp(1j * dphi)) / (.5 * A["amp"] + .5 * B["amp"])), 2)
    mids = {f"{a}>{b}": midpoint(a, b) for a, b in
            [(f"core-{x:03d}-{l}", f"core-{y:03d}-{l}") for l in ("on", "off") for x, y in zip(ANCHORS, ANCHORS[1:])] +
            [(f"core-{h:03d}-on", f"core-{h:03d}-off") for h in ANCHORS]}
    (LOOPS / "analysis.json").write_text(json.dumps({"loops": rows, "midpointsDb": mids}, indent=1), encoding="utf-8")
    lines = ["| loop | f0 Hz | drift | seam | rho | peak | <200 Hz | 1-4 kHz | LUFS | brightness Hz | earshot |", "|" + "---|" * 11]
    for k, r in rows.items():
        lines.append(f"| {k} | {r['f0']} | {r['drift']} | {r['seam']} | {r['rho']} | {r['peak']} | {r['below200']} | {r['zing1to4k']} | "
                     f"{r['lufs']} | {r['brightnessHz']} | {r['describe']} |")
    lines += ["", "Equal-gain crossfade midpoints of the fundamental (dB): " + ", ".join(f"{k} {v}" for k, v in mids.items())]
    log_section("Synth loops", "\n".join(lines))
    bad = [k for k, r in rows.items() if abs(r["f0"] / r["anchorHz"] - 1) > .01 or r["seam"] >= .02]
    bad += [k for k, v in mids.items() if v < -1.0]
    print(json.dumps({"loops": len(rows), "worstSeam": max(r["seam"] for r in rows.values()),
                      "worstMidpointDb": min(mids.values()), "failed": bad}))
    if bad:
        raise SystemExit("loop checks failed: " + ", ".join(bad))


if __name__ == "__main__":
    mode = sys.argv[1] if len(sys.argv) > 1 else ""
    if mode == "loops":
        loops()
    else:
        raise SystemExit(__doc__)
```

- [ ] **Step 4: Screen the loops** (Git Bash, earshot venv; the models take about a minute to load).

```bash
PY="/d/MY DOCS/MY TOOLS/earshot/.venv/Scripts/python.exe"
"$PY" -I prototypes/Dustline/checks/ship_screen.py loops
```

Expected: `{"loops": 8, "worstSeam": 0.0003, "worstMidpointDb": -0.57, "failed": []}`. The planning run measured, in `LOG.md`: f0 48.0, 75.0, 117.0 and 182.0 Hz on and off load (181.99 for `core-182-on`); drift under 0.0001; seam correlation 0.9965–1.0; peaks 0.15–0.34; energy below 200 Hz 0.51–0.74 on load and 0.96–0.98 off load; energy at 1–4 kHz 0.031–0.075 on load; crossfade midpoints −0.02 to −0.57 dB; LUFS −17.4 to −14.4 on load; earshot `describe` on load "an error buzz / an electrical hum / a singing bowl" (z 2.4–3.7). The bake is deterministic, so these should repeat; if a loop fails, stop and report the `LOG.md` table.

### Task 6: BAKED engine

**Purpose.** Engine audio 2.0.0 with the synth loops only (spec 2.1, 3.2, 3.4, 3.5; S4, S6–S9, S11, S15). The 1.2.0 mechanical core, turbine and exhaust go. Each engine gets one tone voice: the eight loops (`toneSum`) → shift (cut or blip) → zing (peaking at 8 f) → sum → misfire → level → pan, joining the bus at `propMix` after the high-passes (S8). The strain resonators ring from a pulse wave at the fundamental. `attach()` takes the loops and builds both banks once all eight are there. The 1.2.0 afterburner stays (S10). The replaced `audio-engine.test.cjs` (16 checks) runs the engine block in a sandbox with fake Web Audio nodes: one build, banks and synchronous starts, tone rates and weights, faders as positions, neutral revs and GO, shift scheduling, misfire detune, fuzzed frames, dispose, pause/reset, rivals, Doppler, twin engines, damage and seeded variation.

**Files:**
- Replace: `prototypes/Dustline/checks/audio-engine.test.cjs` (LF before and after)
- Create: `prototypes/Dustline/tools/engine2/pieces/baked-side.js`, `prototypes/Dustline/tools/engine2/pieces/baked-player.js`, `prototypes/Dustline/tools/engine2/step3_baked.py`
- Modify (by `step3_baked.py`): the three game copies

- [ ] **Step 1: Write the failing test.** Replace the whole content of `prototypes/Dustline/checks/audio-engine.test.cjs` with:

```js
'use strict';
// Engine audio 2.0 checks against a recording mock of the Web Audio API: every parameter write finite and bounded, node lifecycle,
// repeated start/pause/reset/dispose cycles, rival voice limits, the BAKED tone (attach, phase-locked banks, proportional rates,
// equal-gain weights, scheduled shift shaping), fader thrust, misfire detune, and independence from Math.random.
// These are structural checks. They say nothing about how the engine sounds.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const root = path.resolve(__dirname, '..');
const html = fs.readFileSync(path.join(root, 'index.html'), 'utf8');
const BEGIN = '/* @dustline-engine-audio:begin */', END = '/* @dustline-engine-audio:end */';
assert.equal(html.split(BEGIN).length, 2, 'one engine block');
const source = html.slice(html.indexOf(BEGIN) + BEGIN.length, html.indexOf(END));
assert(!/Math\.random\s*\(/.test(source), 'engine variation must not call Math.random');

let passed = 0;
function test(name, fn) { fn(); passed++; console.log('PASS', name); }
const near = (a, b, tol, what) => assert(Math.abs(a - b) <= tol, `${what || 'value'} ${a} not within ${tol} of ${b}`);

// ---- mock Web Audio ---------------------------------------------------------------------------------------------------------
const LIMITS = { gain: [0, 2.01], frequency: [0, 24000], detune: [-1200, 1200], Q: [0, 40], pan: [-1, 1], playbackRate: [0.05, 4],
  threshold: [-100, 0], knee: [0, 40], ratio: [1, 20], attack: [0, 1], release: [0, 1] };
function makeContext() {
  const ctx = { sampleRate: 48000, currentTime: 0, state: 'running', nodes: [], writes: 0, problems: [], connections: 0, trace: false };
  class Param {
    constructor(owner, name, v) { this.owner = owner; this.name = name; this.value = v; this.log = []; }
    check(v, t, kind) {
      ctx.writes++;
      if (ctx.trace) this.log.push({ kind, v, t });
      // a gain feeding an oscillator's detune is a modulation depth in cents, not a level
      const cents = this.owner.kind === 'gain' && [...this.owner.out].some(o => o instanceof Param && o.name === 'detune');
      const [lo, hi] = cents ? [0, 100] : LIMITS[this.name] || [-Infinity, Infinity];
      // peaking/shelf gains are in dB
      const dbGain = this.name === 'gain' && this.owner.kind === 'biquad';
      if (!Number.isFinite(v) || !Number.isFinite(t) || t < 0) ctx.problems.push(`${kind} ${this.owner.kind}.${this.name} ${v} @${t}`);
      else if (!dbGain && (v < lo || v > hi)) ctx.problems.push(`${kind} ${this.owner.kind}.${this.name} out of range ${v}`);
      else if (dbGain && (v < -24 || v > 24)) ctx.problems.push(`biquad gain out of range ${v}`);
    }
    setTargetAtTime(v, t, tc) { this.check(v, t, 'target'); if (!(tc > 0)) ctx.problems.push('time constant ' + tc); this.value = v; }
    setValueAtTime(v, t) { this.check(v, t, 'set'); this.value = v; }
    linearRampToValueAtTime(v, t) { this.check(v, t, 'ramp'); this.value = v; }
    exponentialRampToValueAtTime(v, t) { this.check(v, t, 'exp'); if (v <= 0) ctx.problems.push('exp ramp to ' + v); this.value = v; }
    cancelScheduledValues(t) { if (!Number.isFinite(t)) ctx.problems.push('cancel ' + t); }
  }
  class Node {
    constructor(kind, params) {
      this.kind = kind; this.out = new Set(); ctx.nodes.push(this);
      for (const [k, v] of Object.entries(params || {})) this[k] = new Param(this, k, v);
    }
    connect(dest) {
      if (!dest || !(dest instanceof Node || dest instanceof Param)) throw new Error('bad connect target');
      this.out.add(dest); ctx.connections++; return dest;
    }
    disconnect(dest) { if (dest) this.out.delete(dest); else this.out.clear(); }
  }
  class Source extends Node {
    start(when = 0, offset = 0) {
      if (this.started) throw new Error('started twice');
      if (!Number.isFinite(when) || !Number.isFinite(offset) || offset < 0) ctx.problems.push(`start ${when} ${offset}`);
      if (this.buffer && offset >= this.buffer.duration) ctx.problems.push(`offset ${offset} beyond ${this.buffer.duration}`);
      this.started = true; this.when = when; this.offset = offset;
    }
    stop() { if (!this.started) throw new Error('stop before start'); this.stopped = true; }
  }
  Object.assign(ctx, {
    destination: new Node('destination'),
    createGain: () => new Node('gain', { gain: 1 }),
    createBiquadFilter: () => Object.assign(new Node('biquad', { frequency: 350, Q: 1, gain: 0, detune: 0 }), { type: 'lowpass' }),
    createOscillator: () => Object.assign(new Source('osc', { frequency: 440, detune: 0 }), { type: 'sine', setPeriodicWave(w) { if (!w || !w.periodic) throw new Error('wave'); } }),
    createBufferSource: () => Object.assign(new Source('buffer', { playbackRate: 1, detune: 0 }), { buffer: null, loop: false }),
    createWaveShaper: () => Object.assign(new Node('shaper'), { curve: null, oversample: 'none' }),
    createStereoPanner: () => new Node('panner', { pan: 0 }),
    createDynamicsCompressor: () => new Node('compressor', { threshold: -24, knee: 30, ratio: 12, attack: .003, release: .25 }),
    createPeriodicWave: (re, im) => { assert.equal(re.length, im.length); return { periodic: true }; },
    createBuffer: (ch, len, rate) => {
      const data = Array.from({ length: ch }, () => new Float32Array(len));
      return { numberOfChannels: ch, length: len, sampleRate: rate, duration: len / rate, getChannelData: i => data[i] };
    }
  });
  return ctx;
}

const sandbox = vm.createContext({ Math, Float32Array, Float64Array, Int32Array, Uint8Array, Map, WeakMap, Set, Array, Object, Number, JSON });
vm.runInContext(source, sandbox);
const E = sandbox.DustlineEngineAudio;
assert(E && typeof E.create === 'function', 'engine API');
const SHIP = E.ships.kestrel, BAKED = [...SHIP.baked], MIX = SHIP.mix, GP = SHIP.gears;

function frame(over) {
  return Object.assign({ phase: 'racing', countdown: 0, speed: 120, thrL: .75, thrR: .75, brake: 0, steer: 0, boost: false, burst: 0, padBurst: 0,
    charge: .8, heat: .3, hull: 100, fault: '', draft: 0, recovery: false, view: 'chase', blocked: false, timing: null, ticks: false, rivals: [] }, over);
}
function drive(ctx, eng, seconds, f, dt = 1 / 60, each) {
  for (let i = 0; i < Math.round(seconds / dt); i++) {
    ctx.currentTime += dt; eng.update(typeof f === 'function' ? f(i * dt) : f, dt, ctx.currentTime);
    if (each) each(eng.debug(), i * dt);
  }
}
// the eight loops a game hands over after decoding the ship bundle: 2.1 s stereo at 48 kHz
const attachAll = (ctx, eng) => BAKED.map(b => eng.attach(b.id, ctx.createBuffer(2, 100800, 48000)));
const bankOf = ctx => ctx.nodes.filter(n => n.kind === 'buffer' && n.loopEnd > 0);
const next = n => [...n.out][0];
const reachable = (ctx, from) => {
  const seen = new Set(), stack = [from];
  while (stack.length) { const n = stack.pop(); if (seen.has(n)) continue; seen.add(n); for (const o of n.out || []) stack.push(o instanceof Object && o.owner ? o.owner : o); }
  return seen;
};
const sources = ctx => ctx.nodes.filter(n => n.kind === 'osc' || n.kind === 'buffer');
const smooth = (a, b, v) => { const t = Math.min(1, Math.max(0, (v - a) / (b - a))); return t * t * (3 - 2 * t); };

test('builds once, every source starts once, and buffers are shared per context', () => {
  const ctx = makeContext(), master = E.createMaster(ctx), eng = E.create(ctx, { output: master.input });
  const st = eng.stats();
  assert(st.nodes > 100 && st.nodes < 260, 'node count ' + st.nodes);
  assert.equal(st.sources, sources(ctx).length);
  assert(sources(ctx).every(s => s.started && !s.stopped));
  assert.equal(E.buffersFor(ctx), E.buffersFor(ctx));
  assert(reachable(ctx, eng.out).has(ctx.destination), 'engine reaches the destination through the master stage');
  assert.equal(st.tone, false, 'no tone before the loops are attached');
  assert.equal(E.version, '2.0.0');
  const before = ctx.nodes.length;
  drive(ctx, eng, 3, frame());
  assert.equal(ctx.nodes.length, before, 'updates allocate no audio nodes');
  assert.deepEqual(ctx.problems, []);
});

test('attach takes each of the eight loops once and builds both banks, all sixteen sources starting together at their loop start', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input });
  drive(ctx, eng, .2, frame());
  const n0 = ctx.nodes.length, s0 = eng.stats().sources;
  assert.equal(eng.attach('core-999-on', ctx.createBuffer(2, 100800, 48000)), false, 'unknown id');
  assert.equal(eng.attach(BAKED[0].id, ctx.createBuffer(2, 48000, 48000)), false, 'shorter than the loop end');
  assert.equal(eng.attach(BAKED[0].id, null), false, 'no buffer');
  assert.equal(eng.attach(7, ctx.createBuffer(2, 100800, 48000)), false, 'id must be a string');
  const ok = attachAll(ctx, eng);
  assert.deepEqual(ok, BAKED.map(() => true));
  assert.equal(eng.attach(BAKED[3].id, ctx.createBuffer(2, 100800, 48000)), false, 'each loop once');
  const st = eng.stats();
  assert.equal(st.tone, true); assert.equal(st.attached, 8);
  assert.equal(ctx.nodes.length - n0, 32, 'sixteen sources and sixteen weights');
  assert.equal(st.sources - s0, 16);
  const bank = bankOf(ctx);
  assert.equal(bank.length, 16);
  for (const s of bank) {
    assert(s.loop && s.started && !s.stopped);
    near(s.loopStart, 2400 / 48000, 1e-12, 'loopStart'); near(s.loopEnd, 98400 / 48000, 1e-12, 'loopEnd');
    assert.equal(s.offset, s.loopStart, 'starts at the loop start');
    assert.equal(s.when, bank[0].when, 'all sources start on the same frame');
  }
  const n1 = ctx.nodes.length;
  drive(ctx, eng, 2, frame());
  assert.equal(ctx.nodes.length, n1, 'no nodes after attach');
  eng.dispose();
  assert.equal(eng.attach(BAKED[0].id, ctx.createBuffer(2, 100800, 48000)), false, 'disposed');
  assert.deepEqual(ctx.problems, []);
});

test('tone rates follow the gear model: every loop of an engine at tone x pitch / anchor, the eight rates exactly proportional', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input });
  attachAll(ctx, eng);
  drive(ctx, eng, 2, frame({ speed: 120 }));
  const d = eng.debug(), bank = bankOf(ctx);
  assert.equal(d.gear, 6, '432 km/h is 6th');
  near(d.f, GP.redlineHz * 432 / 490, 1e-9, 'f');
  near(d.toneL, d.tone, 1e-12, 'left engine at the model tone');
  near(d.toneR / d.toneL, MIX.rightPitch, 1e-12, 'right engine 0.35 % higher');
  bank.forEach((s, i) => near(s.playbackRate.value * BAKED[i % 8].anchorHz, i < 8 ? d.toneL : d.toneR, 1e-9, 'rate x anchor ' + i));
  // a slow speed change rewrites all eight rates of an engine at once, never some of them
  drive(ctx, eng, 1, t => frame({ speed: 120 + 10 * t }));
  const e = eng.debug();
  bank.forEach((s, i) => near(s.playbackRate.value * BAKED[i % 8].anchorHz, i < 8 ? e.toneL : e.toneR, e.tone * .0005, 'rate x anchor after the change ' + i));
  const L = bank.slice(0, 8).map((s, i) => s.playbackRate.value * BAKED[i].anchorHz);
  assert(L.every(x => Math.abs(x - L[0]) <= L[0] * 1e-12), 'the eight rates of an engine stay exactly proportional');
  assert.deepEqual(ctx.problems, []);
});

test('weights: the two nearest anchors share the tone linearly in log frequency, the load splits on/off by a smoothstep, all sum to one', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input });
  attachAll(ctx, eng);
  const bank = bankOf(ctx), w = i => next(bank[i]).gain.value;
  const expect = (tone, load) => BAKED.map(b => {
    const anchors = [48, 75, 117, 182]; let j = 0; while (j < 2 && tone > anchors[j + 1]) j++;
    const lo = anchors[j], hi = anchors[j + 1], x = Math.min(1, Math.max(0, Math.log(tone / lo) / Math.log(hi / lo))), on = smooth(.1, .4, load);
    return (b.anchorHz === lo ? 1 - x : b.anchorHz === hi ? x : 0) * (b.load === 'on' ? on : 1 - on);
  });
  for (const [speed, thr] of [[120, .75], [120, 0], [60, .75], [200, .75], [30, .2]]) {
    drive(ctx, eng, 2, frame({ speed, thrL: thr, thrR: thr }));
    const d = eng.debug(), want = expect(d.tone, d.loadL);
    let sum = 0;
    for (let i = 0; i < 8; i++) { near(w(i), want[i], .002 * Math.max(1, want[i]) + 1e-4, `weight ${BAKED[i].id} at ${speed} m/s, thrust ${thr}`); sum += w(i); }
    near(sum, 1, .005, 'weights sum');
  }
  assert.deepEqual(ctx.problems, []);
});

test('faders are positions: 0.75 is full thrust, half way is half load, the boost zone adds no load', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input });
  drive(ctx, eng, 1.5, frame({ thrL: .75, thrR: .375 }));
  let d = eng.debug();
  near(d.loadL, 1, .005, 'full thrust'); near(d.loadR, .5, .005, 'half thrust');
  drive(ctx, eng, 1.5, frame({ thrL: 1, thrR: .9 }));
  d = eng.debug();
  near(d.loadL, 1, .005, 'boost zone'); near(d.loadR, 1, .005, 'boost zone');
  drive(ctx, eng, 1.5, frame({ thrL: 0, thrR: 0, brake: 1 }));
  assert(eng.debug().loadL < .01, 'lift');
});

test('neutral revs on the grid follow the throttle; GO picks the gear from the first racing speed', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input });
  drive(ctx, eng, 2, frame({ phase: 'countdown', speed: 0, thrL: .75, thrR: .75 }));
  let d = eng.debug();
  assert.equal(d.gear, 0);
  near(d.tone, GP.idleHz + (GP.redlineHz - GP.idleHz) * .8, .05, 'full-throttle grid revs');
  drive(ctx, eng, 2 / 60, frame({ speed: 68 })); // the engine steps its model every 1/40 s: two 60 Hz frames
  d = eng.debug();
  assert.equal(d.gear, 4, '245 km/h at GO is 4th');
  near(d.f, GP.redlineHz * 244.8 / 300, 1e-9, 'f at GO');
});

test('gear shifts schedule the ignition cut, the zing blip and the held-then-gliding loop rates at the shift', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input });
  attachAll(ctx, eng);
  drive(ctx, eng, .2, frame({ phase: 'countdown', speed: 0 }));
  ctx.trace = true;
  drive(ctx, eng, 1.5, t => frame({ speed: 68 + 40 * t }));
  const bank = bankOf(ctx), src = bank[0], toneSum = next(next(src)), shift = next(toneSum), zing = next(shift);
  const log = src.playbackRate.log, i = log.findIndex(e => e.kind === 'exp');
  assert(i >= 2, 'an upshift ramp');
  const [a, h, x] = log.slice(i - 2, i + 1);
  assert.equal(a.kind, 'set'); assert.equal(h.kind, 'set'); assert.equal(a.v, h.v, 'the rate holds through the cut');
  near(h.t - a.t, .04, 1e-9, 'hold'); near(x.t - a.t, .1, 1e-9, 'cut + glide');
  assert(x.v < a.v / 1.15, 'an upshift drops the pitch');
  for (const [k, s] of bank.entries()) {
    const e = s.playbackRate.log.find(e => e.kind === 'exp' && Math.abs(e.t - x.t) < 1e-9);
    assert(e, 'every loop ramps at the same moment ' + k);
    near(e.v * BAKED[k % 8].anchorHz, x.v * BAKED[0].anchorHz * (k < 8 ? 1 : MIX.rightPitch), 1e-9, 'proportional ramp ' + k);
  }
  const cut = shift.gain.log.filter(e => e.t >= a.t - 1e-9 && e.t <= a.t + .05);
  near(cut[0].v, Math.pow(10, MIX.cutDb / 20), 1e-9, 'cut depth'); near(cut[0].t, a.t, 1e-9);
  assert.equal(cut[1].v, 1); near(cut[1].t, a.t + .04, 1e-9, 'cut length');
  const z = zing.gain.log.filter(e => e.t >= a.t - 1e-9 && e.t <= a.t + .1);
  assert.equal(z[0].v, MIX.zingBlipDb); near(z[0].t, a.t + .04, 1e-9, 'zing after the cut');
  assert.equal(z[1].v, 0); near(z[1].t, a.t + .09, 1e-9);
  // downshift: brake from 600 km/h
  const ctx2 = makeContext(), eng2 = E.create(ctx2, { output: E.createMaster(ctx2).input });
  attachAll(ctx2, eng2);
  drive(ctx2, eng2, .5, frame({ speed: 600 / 3.6 }));
  ctx2.trace = true;
  drive(ctx2, eng2, 1.5, t => frame({ speed: Math.max(30, 600 / 3.6 - 120 * t), thrL: 0, thrR: 0, brake: 1 }));
  const b2 = bankOf(ctx2), sh2 = next(next(next(b2[0]))), l2 = b2[0].playbackRate.log, j = l2.findIndex(e => e.kind === 'exp');
  assert(j >= 1, 'a downshift ramp');
  const [da, dx] = l2.slice(j - 1, j + 1);
  assert.equal(da.kind, 'set'); near(dx.t - da.t, .04, 1e-9, 'down-glide'); assert(dx.v > da.v, 'a downshift raises the pitch');
  const blip = sh2.gain.log.filter(e => e.t >= da.t - 1e-9 && e.t <= da.t + .09);
  near(blip[0].v, Math.pow(10, MIX.blipDb / 20), 1e-9, 'blip level'); assert.equal(blip[1].v, 1); near(blip[1].t, da.t + .08, 1e-9, 'blip length');
  assert.deepEqual(ctx.problems, []); assert.deepEqual(ctx2.problems, []);
});

test('a misfire detunes its own engine 0.6 % for the dip; the other engine keeps its pitch', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input });
  attachAll(ctx, eng);
  let detuned = 0, clean = 0;
  drive(ctx, eng, .5, frame()); // misfires are timed from the moment the fault appears
  drive(ctx, eng, 6, frame({ fault: 'hull', hull: 40 }), 1 / 60, d => {
    near(d.toneR / d.tone, MIX.rightPitch, 1e-12, 'right engine untouched by a left misfire');
    const r = d.toneL / d.tone;
    if (Math.abs(r - .994) < 1e-12) detuned++; else { near(r, 1, 1e-12, 'left engine between misfires'); clean++; }
  });
  assert(detuned > 0 && clean > detuned, `detuned ${detuned} of ${detuned + clean} frames`);
  assert.deepEqual(ctx.problems, []);
});

test('fuzzed frames and events keep every parameter write finite and inside its range', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input });
  attachAll(ctx, eng);
  let seed = 12345; const r = () => (seed = (seed * 1103515245 + 12345) >>> 0) / 4294967296;
  const odd = [NaN, Infinity, -Infinity, -5, 1e9, undefined, null, 'x', {}, 0, 1, .5];
  const pick = () => r() < .25 ? odd[Math.floor(r() * odd.length)] : r() * 1.2;
  for (let i = 0; i < 3000; i++) {
    const f = frame({ speed: pick() * 400, thrL: pick(), thrR: pick(), brake: pick(), steer: pick() * 2 - 1, boost: r() < .3, burst: pick(), padBurst: pick(),
      charge: pick(), heat: pick(), hull: pick() * 100, fault: ['', 'hull', 'coolant', 'stabilizer', 7][i % 5], draft: pick(), recovery: r() < .1,
      phase: ['racing', 'countdown', 'menu', 'finished', undefined][Math.floor(r() * 5)], countdown: pick() * 3, view: r() < .5 ? 'cockpit' : 'chase',
      blocked: r() < .1, timing: r() < .3 ? { kind: 'vent', delta: pick() - .6, good: pick(), perfect: pick() } : null, ticks: r() < .5,
      rivals: Array.from({ length: 5 }, (_, id) => ({ id, dx: pick() * 60 - 30, dz: pick() * 400 - 200, speed: pick() * 170, thr: pick(), boost: r() < .2, brake: pick(), active: r() < .9 })) });
    const dt = [1 / 60, 1 / 144, 1 / 30, 0, .3, NaN, -1][i % 7];
    ctx.currentTime += Number.isFinite(dt) && dt > 0 ? Math.min(dt, .3) : 0;
    eng.update(f, dt, ctx.currentTime);
    if (i % 37 === 0) eng.event(['ignite', 'release', 'launch', 'pad', 'vent', 'restore', 'cue', 'duck', 'nonsense'][i % 9],
      { strength: pick(), cue: ['open', 'tick', 'hit', 'perfect', 'good', 'fail', 'warn', 'trip', 'zzz'][i % 9], kind: 'vent', depth: pick(), hold: pick(), band: -pick() * 8 }, ctx.currentTime);
    if (i % 401 === 0) eng.reset(ctx.currentTime);
  }
  assert.deepEqual(ctx.problems.slice(0, 5), []);
  assert.equal(eng.stats().bad, 0);
  assert(eng.stats().voices <= 3);
});

test('dispose stops and disconnects everything; repeated create/dispose cycles leave nothing running', () => {
  const ctx = makeContext(), master = E.createMaster(ctx);
  for (let k = 0; k < 20; k++) {
    const eng = E.create(ctx, { output: master.input, seed: k });
    if (k % 2) attachAll(ctx, eng);
    drive(ctx, eng, .5, frame({ rivals: [{ id: 1, dx: 4, dz: 10, speed: 120, thr: 1 }] }));
    eng.dispose(); eng.dispose();
    eng.update(frame(), 1 / 60, ctx.currentTime); eng.event('ignite', {}, ctx.currentTime); eng.setActive(true);
  }
  assert(sources(ctx).every(s => s.stopped), 'all sources stopped');
  const engineNodes = ctx.nodes.filter(n => n.kind !== 'compressor' && n.kind !== 'destination' && !master.input.out.has(n));
  assert(engineNodes.filter(n => n !== master.gain).every(n => n.out.size === 0), 'all engine nodes disconnected');
  master.dispose();
  assert.equal(master.gain.out.size, 0);
  assert.deepEqual(ctx.problems, []);
});

test('pause / resume / reset cycles reuse the same graph and park the engine while inactive', () => {
  const ctx = makeContext(), master = E.createMaster(ctx), eng = E.create(ctx, { output: master.input });
  attachAll(ctx, eng);
  drive(ctx, eng, .5, frame());
  const n = ctx.nodes.length;
  for (let k = 0; k < 50; k++) {
    eng.setActive(false, ctx.currentTime);
    drive(ctx, eng, .5, frame());
    assert(eng.parked && !reachable(ctx, eng.out).has(ctx.destination), 'parked engine is off the graph');
    eng.setActive(true, ctx.currentTime);
    assert(!eng.parked && reachable(ctx, eng.out).has(ctx.destination));
    drive(ctx, eng, .2, frame({ boost: k % 2 === 0, speed: 60 + k }));
    if (k % 5 === 0) { eng.reset(ctx.currentTime); assert.equal(eng.gear.gear, 0, 'reset returns the gear model to neutral'); }
  }
  assert.equal(ctx.nodes.length, n, 'no nodes created by cycles');
  assert(sources(ctx).every(s => s.started && !s.stopped));
  assert.deepEqual(ctx.problems, []);
});

test('rival voices: at most three, nearest first, distant racers silent and disconnected, reset clears them', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input });
  const near3 = Array.from({ length: 5 }, (_, id) => ({ id, dx: id * 3 - 6, dz: 15 + id * 20, speed: 120, thr: 1 }));
  drive(ctx, eng, 1, frame({ rivals: near3 }));
  assert.equal(eng.stats().voices, 3);
  assert.equal(eng.debug().rivals.filter(Boolean).map(v => v.id).sort().join(), '0,1,2');
  drive(ctx, eng, 2, frame({ rivals: near3.map(r => ({ ...r, dz: 400 + r.dz })) }));
  assert.equal(eng.stats().voices, 0, 'out-of-range rivals free their voices');
  drive(ctx, eng, 1, frame({ rivals: near3 }));
  assert.equal(eng.stats().voices, 3);
  eng.reset(ctx.currentTime);
  assert.equal(eng.stats().voices, 0);
  drive(ctx, eng, 1, frame({ phase: 'finished', rivals: near3 }));
  assert.equal(eng.stats().voices, 0, 'finished race has no rival voices');
});

test('rival Doppler follows measured range rate: approaching is higher, receding lower, both within 8 %', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input });
  let dz = -120;
  drive(ctx, eng, 2, () => { dz += 40 / 60; return frame({ rivals: [{ id: 1, dx: 6, dz, speed: 160, thr: 1 }] }); });
  const closing = eng.debug().rivals.find(Boolean).vr;
  assert(closing < -30 && closing > -50, 'closing range rate ' + closing);
  dz = 20;
  drive(ctx, eng, 2, () => { dz += 40 / 60; return frame({ rivals: [{ id: 1, dx: 6, dz, speed: 160, thr: 1 }] }); });
  const opening = eng.debug().rivals.find(Boolean).vr;
  assert(opening > 30 && opening < 50, 'opening range rate ' + opening);
});

test('left and right engines follow their own faders: load, effort and pan move apart, the pitch stays shared', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input });
  attachAll(ctx, eng);
  drive(ctx, eng, 3, frame({ thrL: .75, thrR: .15 }));
  const d = eng.debug();
  assert(d.loadL > .95 && d.loadR < .25, `loads ${d.loadL} ${d.loadR}`);
  assert(d.effL > d.effR + .6, 'effort follows each fader');
  near(d.toneR / d.toneL, MIX.rightPitch, 1e-12, 'one shared pitch, the right engine 0.35 % higher');
  const bank = bankOf(ctx), on = i => next(bank[i]).gain.value + next(bank[i + 2]).gain.value + next(bank[i + 4]).gain.value + next(bank[i + 6]).gain.value;
  near(on(0), smooth(.1, .4, d.loadL), .005, 'on-load share, left');
  near(on(8), smooth(.1, .4, d.loadR), .005, 'on-load share, right');
  assert(on(0) > .99 && on(8) < .3, 'each engine crossfades on its own load');
});

test('damage and heat impair the sound without stopping thrust', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input });
  attachAll(ctx, eng);
  drive(ctx, eng, 4, frame({ hull: 40, fault: 'hull', heat: .97 }));
  const d = eng.debug();
  assert(d.dmg > .5 && d.heat > .9);
  assert(d.loadL > .95 && d.loadR > .95, 'load stays at full thrust');
  assert.equal(d.gear, 6);
  assert.deepEqual(ctx.problems, []);
});

test('variation is private and seeded: Math.random is never called, equal seeds give equal parameter streams', () => {
  const real = Math.random;
  Math.random = () => { throw new Error('Math.random used'); };
  try {
    // every write is traced: the seed moves misfire timing and length, sputter and afterfire pops
    const runs = [1, 1, 2].map(seed => {
      const ctx = makeContext(), log = [];
      ctx.trace = true;
      const eng = E.create(ctx, { output: E.createMaster(ctx).input, seed });
      attachAll(ctx, eng);
      drive(ctx, eng, 4.5, t => frame(t < .5 ? {} : { fault: 'hull', hull: 50, heat: .9, boost: t > 2.5, charge: .05 }));
      for (const n of ctx.nodes) for (const k of Object.keys(n)) if (n[k] && n[k].constructor && n[k].constructor.name === 'Param') for (const e of n[k].log) log.push(e.kind, e.v, e.t);
      return log.join(',');
    });
    assert.equal(runs[0], runs[1]);
    assert.notEqual(runs[0], runs[2]);
  } finally { Math.random = real; }
});

console.log(`${passed} engine audio checks passed.`);
```

- [ ] **Step 2: Run it to see it fail.**

```bash
node prototypes/Dustline/checks/audio-engine.test.cjs
```

Expected: `AssertionError [ERR_ASSERTION]: node count 302`.

- [ ] **Step 3: Create the BAKED pieces and step 3.**

Create `prototypes/Dustline/tools/engine2/pieces/baked-side.js`:

```js
    /* ---- player engines: one tone voice per side (spec 3.4) ----
     * toneSum (the baked loops) -> shift (ignition cut / throttle blip) -> zing (a peaking blip at 8 f) -> sum. The strain resonators
     * ring from a pulse train at the fundamental, which reaches the resonators only. sum -> fault (misfire dips) -> level -> pan. */
    const TONE=dbGain(MIX.toneDb),CUT=dbGain(MIX.cutDb),BLIP=dbGain(MIX.blipDb);
    function buildSide(o){
      const sum=T.gain(1),toneSum=T.gain(1),shift=shot(T.gain(1),1),zing=shot(T.bq('peaking',800,1.4,0),0);
      T.chain(toneSum,shift,zing,sum);
      const px=T.osc('sine',60,B.pulse),sb1=T.bq('bandpass',1150*o.metal,14),sb2=T.bq('bandpass',1740*o.metal,18),sx=T.bq('bandpass',620*o.metal,9),gst=T.gain(0);
      px.connect(sb1);px.connect(sb2);px.connect(sx);sb1.connect(gst);sb2.connect(gst);sx.connect(gst);gst.connect(sum);
      const fault=shot(T.gain(1),1),vl=T.gain(0),panBase=o.side*MIX.pan,pan=T.pan(panBase);
      T.chain(sum,fault,vl,pan,propMix);
      return{o,pitch:o.side>0?MIX.rightPitch:1,panBase,load:0,eff:0,kick:0,hz:0,detuneUntil:0,rampUntil:0,rate:0,bank:null,toneSum,shift,zing,px,gst,fault,vl,pan};
    }
    const sides=SIDES.map(buildSide);
```

Create `prototypes/Dustline/tools/engine2/pieces/baked-player.js`:

```js
    /* ---- BAKED tone (spec 3.2, 4.2): the ship's eight synth loops per engine, phase-locked ---- */
    // The game decodes the ship's assets and hands each one over with attach(). The banks are built once, when every loop is
    // there: all sixteen sources start on the same frame at their loop start, so loops rendered in phase stay in phase.
    const assets=new Map();
    function attach(id,buffer){
      if(disposed||typeof id!=='string'||!buffer||!(buffer.duration>0)||assets.has(id))return false;
      const e=SHIP.baked.find(b=>b.id===id);
      if(!e||buffer.duration*LOOP_RATE<e.loopEnd-1)return false;
      assets.set(id,buffer);
      if(!sides[0].bank&&SHIP.baked.every(b=>assets.has(b.id)))buildBanks();
      return true;
    }
    function buildBanks(){
      const t0=ctx.currentTime+.02;
      for(const s of sides){
        s.bank=SHIP.baked.map(b=>{
          const src=T.loop(assets.get(b.id),b.loopStart/LOOP_RATE,b.loopEnd/LOOP_RATE,t0),w=T.gain(0);
          src.connect(w);w.connect(s.toneSum);return{b,src,w};
        });
        s.rate=0;
      }
    }
    // Every loop of an engine plays at tone / anchor. The rates are written together and only all at once, never through S()'s
    // per-parameter skip, so their ratio, and with it the loops' phase lock, never changes.
    function toneRates(s,hz,tc){
      hz=clampRate(hz);
      if(Math.abs(hz-s.rate)<=s.rate*.0004)return;
      s.rate=hz;writes+=s.bank.length;
      for(const k of s.bank)k.src.playbackRate.setTargetAtTime(hz/k.b.anchorHz,NOW,tc);
    }
    // Equal-gain weights (the loops are in phase): linear in log frequency between the two nearest anchors, a smoothstep on load.
    function toneWeights(s,tone,load){
      let j=0;while(j<ANCHORS.length-2&&tone>ANCHORS[j+1])j++;
      const lo=ANCHORS[j],hi=ANCHORS[j+1],x=clamp(Math.log(tone/lo)/Math.log(hi/lo),0,1),on=ss(.1,.4,load);
      for(const k of s.bank){
        const a=k.b.anchorHz===lo?1-x:k.b.anchorHz===hi?x:0;
        S(k.w.gain,a*(k.b.load==='on'?on:1-on),.03,0,1);
      }
    }
    // Shift shaping (spec 3.1), scheduled at the event so the 40 ms cut, 60 ms glide, 80 ms blip and 40 ms down-glide keep their
    // length at any control rate. Control steps leave the loop rates alone until the glide has ended.
    function shiftTone(type,t,g){
      const up=type==='upshift',hold=up?.04:0,glide=up?.06:.04;
      for(const s of sides){
        const p=s.shift.gain;p.cancelScheduledValues(t);
        if(up){p.setTargetAtTime(CUT,t,.004);p.setTargetAtTime(1,t+.04,.008);}
        else{p.setTargetAtTime(BLIP,t,.006);p.setTargetAtTime(1,t+.08,.02);}
        if(up){const z=s.zing.gain;z.cancelScheduledValues(t);z.setTargetAtTime(MIX.zingBlipDb,t+.04,.01);z.setTargetAtTime(0,t+.09,.05);}
        if(!s.bank)continue;
        const a=clampRate(g.shiftFrom*s.pitch),b=clampRate(g.f*s.pitch);
        for(const k of s.bank){
          const r=k.src.playbackRate;
          r.cancelScheduledValues(t);r.setValueAtTime(a/k.b.anchorHz,t);
          if(hold)r.setValueAtTime(a/k.b.anchorHz,t+hold);
          r.exponentialRampToValueAtTime(b/k.b.anchorHz,t+hold+glide);
        }
        s.rate=b;s.rampUntil=t+hold+glide;writes+=3*s.bank.length;
      }
    }

    /* ---- per-tick update of the player's engines and shared layers ---- */
    function player(f,dt,speed){
      const phase=f.phase||'racing',racing=phase==='racing',v=speed/VMAX;
      // fader positions to thrust: 0.75 is full thrust, the boost zone above it adds nothing here
      const fl=fin(f.thrL,FADER_FULL);
      let thrL=sat(fl/FADER_FULL),thrR=sat(fin(f.thrR,fl)/FADER_FULL);
      if(phase==='finished')thrL=thrR=0;
      const recovering=!!f.recovery&&racing;
      if(recovering){thrL*=.5;thrR*=.5;}
      const brake=sat(fin(f.brake,0)),boost=racing?sat(fin(f.boostIntensity,f.boost?1:0)):0,charge=sat(fin(f.charge,1));
      // the gear model: gear, tone (with the shift glide), loads and shift events
      gin.phase=phase;gin.speed=speed*3.6;gin.thrL=thrL;gin.thrR=thrR;gin.brake=brake;gin.recovery=recovering;
      const g=gears.advance(dt,gin);
      for(let k=0;k<g.eventCount;k++)shiftTone(g.events[k].type,NOW,g);
      // afterburner presence: quick attack, slower release; the ignition burst follows the simulation's own timer
      const burnT=racing?Math.max(boost,.5*sat(fin(f.padBurst,0))):0;
      M.burnA=ease(M.burnA,burnT,dt,.08,.32);
      M.burst=racing?sat(fin(f.burst,0)):0;
      M.brakeA=ease(M.brakeA,brake,dt,.05,.15);
      const heat=sat(fin(f.heat,0)),hull=clamp(fin(f.hull,100),0,100),fault=typeof f.fault==='string'?f.fault:'';
      M.heat=ease(M.heat,heat,dt,.3,.3);
      M.draft=ease(M.draft,sat(fin(f.draft,0)),dt,.12,.12);
      M.faultA=ease(M.faultA,fault?1:0,dt,.12,.5);
      M.dmg=ease(M.dmg,Math.max(ss(0,.75,1-hull/100)*.55,M.faultA),dt,.4,.4);
      const dmg=M.dmg,bA=M.burnA,bu=M.burst,rpm=g.tone/GP.redlineHz;
      let sumLoad=0;
      for(let i=0;i<2;i++){
        const s=sides[i],heatS=ss(.55,.98,sat(fin(i?f.heatRight:f.heatLeft,heat)));
        s.load=g.load[i];sumLoad+=s.load;
        // effort: load, much less under the airbrake, a little more in boost
        const eff=clamp(s.load*(1-.7*M.brakeA)+.1*bA,0,1.1);
        s.eff=eff;
        if(s.kick>0)s.kick=Math.max(0,s.kick-dt*.9);
        // a misfire detunes its engine 0.6 % for the dip; a restored engine re-lights 1.5 % sharp and settles
        s.hz=g.tone*s.pitch*(NOW<s.detuneUntil?.994:1)*(1+.015*s.kick);
        if(s.bank){if(NOW>=s.rampUntil)toneRates(s,s.hz,.012);toneWeights(s,g.tone,s.load);}
        S(s.zing.frequency,8*s.hz,.03,100,4000);
        S(s.px.frequency,s.hz,.03,20,400);
        S(s.gst.gain,LV.strain*(.06*Math.pow(heatS,1.3)+.1*dmg)*(.5+.5*eff),.12,0,.25);
        S(s.vl.gain,LV.tone*TONE,.05,0,1.2);
      }
      const asym=clamp(sides[1].eff-sides[0].eff,-1,1),loadAvg=sumLoad*.5;
      for(const s of sides)S(s.pan.pan,s.panBase+.1*asym,.08,-.6,.6);
      S(bbp.frequency,450+250*bA+350*bu,.06,100,6000);S(bl.gain,LV.burn*.17*Math.pow(bA,1.1)*(1+.3*bu),.05,0,.9);
      S(bam.depth.gain,.35+.35*bu+.25*sat(1-charge*5)*bA,.05,0,.8);S(bam.node.gain,.65-.35*bu,.05,0,1);
      S(bll.gain,LV.burn*.11*bA,.08,0,.5);S(bhg.gain,LV.burn*.03*bA*(.6+.4*bu),.06,0,.3);
      // airflow follows vehicle speed, not engine effort
      const cock=view==='cockpit'?.62:1,steerV=Math.abs(fin(f.steer,0))*v*.1,wind=LV.air*.2*Math.pow(v,2.15)*(1-.3*M.draft)*cock;
      S(wl.gain,wind,.1,0,.6);S(wl2.gain,wind,.1,0,.6);
      S(wbpA.frequency,380+2900*Math.pow(v,1.3),.1,100,7000);S(wbpB.frequency,420+2800*Math.pow(v,1.3),.1,100,7000);
      S(bfl.gain,LV.air*(.045+.17*Math.pow(v,2.4)+.08*bA+steerV+.06*M.draft*v)*cock,.12,0,.5);
      // intake suction follows the engine's rpm
      S(il.gain,LV.air*.07*Math.pow(clamp(rpm,0,1.2),1.7)*(.4+.6*loadAvg)*(racing?1:.5),.08,0,.2);S(ibp.frequency,900+1700*sat(rpm),.1,200,5000);
      S(kl.gain,LV.air*.24*Math.pow(M.draft,1.2),.1,0,.5);S(kbp.frequency,650+500*M.draft,.1,200,3000);
      S(brl.gain,LV.air*.11*M.brakeA*Math.pow(v,.8),.06,0,.3);S(brbp.frequency,1800+1400*v,.1,400,6000);
      // events derived from state changes
      const t=NOW;
      if(racing){
        if(M.lastThr-loadAvg>.5&&rpm>.5&&t>M.flutterAt){M.flutterAt=t+.6;blowoff(t,clamp(rpm,.3,1));}
        if(M.lastDraft-M.draft>.012&&M.lastDraft>.3&&t>M.flutterAt-.3)puff(t,M.lastDraft);
        if(heat>=.84&&!M.heatWarned){M.heatWarned=true;M.lastWarnAt=t;cue('warn',null,t);}else if(heat<.74)M.heatWarned=false;
        if(heat>=.95&&t-M.lastWarnAt>1.6){M.lastWarnAt=t;cue('warn',null,t);}
        if(f.blocked&&!M.wasBlocked)cue('trip',null,t);
        M.wasBlocked=!!f.blocked;
        if(M.lastFault&&!fault)for(const s of sides)s.kick=1;
        M.lastFault=fault;
        if(M.wasRecovering&&!recovering){for(const s of sides)s.kick=.7;puff(t,.6);}
        // sputtering: which side coughs is cosmetic; thrust and pitch stay readable because dips are short and shallow
        if(dmg>.3||fault){
          for(let i=0;i<2;i++){
            if(fault==='hull'&&i||fault==='coolant'&&!i)continue;
            if(t>=M.misfireAt[i]){
              if(M.misfireAt[i]<1e8)misfire(i,t,.28+.3*Math.min(1,dmg),.05+.07*rnd());
              M.misfireAt[i]=t+(fault?.45+.9*rnd():(2.4+3*rnd())/dmg);
            }
          }
        }else M.misfireAt[0]=M.misfireAt[1]=t+1;
        if(boost&&charge<.12){
          if(t>=M.sputterAt){sputter(t,.25+.35*sat(1-charge/.12),.03+.05*rnd());M.sputterAt=t+.08+.25*charge/.12+.1*rnd();}
        }else M.sputterAt=t;
      }
      M.wasRecovering=recovering;
      M.lastThr=loadAvg;M.lastDraft=M.draft;
      if(f.timing&&racing){
        const d=f.timing,delta=fin(d.delta,9),good=fin(d.good,.24),perfect=fin(d.perfect,.075);
        if(f.ticks!==false&&Math.abs(delta)<=good&&t>=M.timingTickAt){M.timingTickAt=t+.19;cue('tick',{perfect:Math.abs(delta)<=perfect},t);}
        S(space.gain,-3.5,.05,-12,6);
      }else S(space.gain,0,.2,-12,6);
    }
```

Create `prototypes/Dustline/tools/engine2/step3_baked.py`:

```python
"""Engine 2.0, step 3 (plan Task 6): BAKED with the synth loops only. The 1.2.0 mechanical core, turbine and exhaust go; the
tone is the Kestrel patch's eight phase-locked loops per engine, played by the gear model. The 1.2.0 afterburner stays until step 5 (Task 14)."""
from common import Build

b = Build()
# A: header
b.between("/* DUSTLINE twin-turbine engine audio.", " * Layers: mechanical core, turbine, exhaust, afterburner, airflow, strain, rival voices, timing cues.\n */\n",
"""/* DUSTLINE ship engine audio 2.0. A shared gear model (seven gears, a paced speed, boost lift) drives the ship's tone. BAKED plays
 * the ship's Keys synth patch rendered offline as eight phase-locked loops per engine. Airflow, strain, misfires, vent steam, rival
 * engines and timing cues are the 1.2.0 layers. No Math.random: variation comes from a private seeded generator, never from the
 * simulation's random state. The engine never decodes audio: the game decodes the ship's loops and hands them over with attach().
 */
""")
# B: constants
b.rep("""  const VMAX=1200/3.6,CTRL=1/40,VERSION='1.2.0',FIRE_LOOP=4,FIRE_RATE=40;
  const LAYERS=['growl','turbine','exhaust','strain','burn','air','rival','cue'];
""", """  const VMAX=1200/3.6,CTRL=1/40,VERSION='2.0.0',FIRE_LOOP=4,FIRE_RATE=40;
  // Frames carry fader positions: 0.75 is full thrust (the simulation's FADER_ZERO); above it is the boost zone.
  const FADER_FULL=.75,LOOP_RATE=48000,dbGain=db=>Math.pow(10,db/20);
  const LAYERS=['tone','strain','burn','air','rival','cue'];
""")
# D: pulse wave for the strain exciter
b.rep("    const B={white,pink,brown,combustion,", """    // Strain exciter: 40 equal harmonics, a band-limited pulse train at the fundamental.
    const pim=new Float32Array(41);for(let h=1;h<41;h++)pim[h]=1;
    const pulse=ctx.createPeriodicWave(new Float32Array(41),pim);
    const B={pulse,white,pink,brown,combustion,""")
# E: tracker loop
BUF = "      buf:(b,rate=1,off=0)=>{const s=reg(ctx.createBufferSource());s.buffer=b;s.loop=true;s.playbackRate.value=rate;s.start(0,off%b.duration);srcs.push(s);return s;},\n"
b.rep(BUF, BUF +
    "      // a loop started at a set time at its loop start, so loops started together stay in phase\n"
    "      loop:(b,start,end,when)=>{const s=reg(ctx.createBufferSource());s.buffer=b;s.loop=true;s.loopStart=start;s.loopEnd=end;s.start(when,start);srcs.push(s);return s;},\n")
# SIDES
b.between("  // The two engines differ in pitch, firing rate, spool inertia,",
        "    {name:'R',pan:.34,pitch:1.0035,fire:1.016,rB:1.5032,rD:1.997,rE:2.74,intake:690,body:176,metal:1.035,spool:1.07,off:2.77,lfoOff:1.7,tone:.88}\n  ];\n",
"""  // The two engines differ in pan, pitch (the right one 0.35 % higher, from the ship's mix) and strain resonance.
  const SIDES=[{name:'L',side:-1,metal:1},{name:'R',side:1,metal:1.035}];
""")
# F: ship, gear model and rate clamp in create()
b.rep("    const dest=opt.output||null;\n", """    const dest=opt.output||null;
    const SHIP=opt.ship||KESTREL,GP=SHIP.gears,MIX=SHIP.mix,ANCHORS=[...new Set(SHIP.baked.map(b=>b.anchorHz))].sort((a,b)=>a-b);
    const gears=createGears(GP),gin={phase:'menu',speed:0,thrL:0,thrR:0,brake:0,recovery:false};
    // Loop rates stay inside 0.05-4: the tone is clamped once, before it is divided by each anchor, so the rates of an engine's
    // loops stay exactly proportional.
    const RATE_LO=.05*ANCHORS[ANCHORS.length-1],RATE_HI=4*ANCHORS[0],clampRate=hz=>clamp(hz,RATE_LO,RATE_HI);
""")
# G: propulsion mix after the high-passes
CH = "    T.chain(engineMix,hp1,hp2,cabin,viewLP,space,dip,busPre,busSat,duck,level,master);\n"
b.rep(CH, CH +
    "    // The ship's tone joins after the 46 Hz high-passes: its fundamental runs from 41 Hz.\n"
    "    const propMix=T.gain(1);propMix.connect(cabin);\n")
# H: buildSide
b.between("    /* ---- player engines: one voice per side ---- */", "    const sides=SIDES.map(buildSide);\n", b.piece("baked-side.js"))
# surge went with the turbine
b.rep("    const surgeTable=Array.from({length:64},()=>rnd()*2-1);\n", "")
b.between("    const surge=t=>{", "};\n", "")
# I: misfire detune
MF = "      g.setValueAtTime(1-depth,t+.018+len);g.linearRampToValueAtTime(1,t+.018+len+.07);\n"
b.rep(MF, MF + "      sides[side].detuneUntil=t+.018+len+.07;\n")
# J+K: baked tone functions and the 2.0 player
b.between("    /* ---- per-tick update of the player's engines and shared layers ---- */", "      }else S(space.gain,0,.2,-12,6);\n    }\n", b.piece("baked-player.js"))
# reset, dispose, api
b.rep("      for(const s of sides){s.n=.16;s.load=0;s.kick=0;s.eff=0;}\n", """      for(const s of sides){
        s.load=0;s.kick=0;s.eff=0;s.hz=0;s.detuneUntil=0;s.rampUntil=0;s.rate=0;
        if(s.bank)for(const k of s.bank)k.src.playbackRate.cancelScheduledValues(t);
      }
      gears.reset();
""")
b.rep("      T.dispose();last.clear();rivalState.clear();\n", "      T.dispose();last.clear();rivalState.clear();assets.clear();\n")
b.rep("    const api={update,event,setActive,reset,dispose,rnd,out,layers:LV,\n", "    const api={update,event,setActive,reset,dispose,attach,rnd,out,layers:LV,ship:SHIP,\n")
b.rep("      get disposed(){return disposed;},get active(){return active;},get parked(){return parked;},\n",
    "      get disposed(){return disposed;},get active(){return active;},get parked(){return parked;},get gear(){return gears.state;},\n")
b.rep("voices:rivalVoices.filter(v=>v.connected).length};},\n", "voices:rivalVoices.filter(v=>v.connected).length,tone:!!sides[0].bank,attached:assets.size};},\n")
b.rep("""      debug(){return{nL:sides[0].nEff,nR:sides[1].nEff,loadL:sides[0].load,loadR:sides[1].load,effL:sides[0].eff,effR:sides[1].eff,burn:M.burnA,dmg:M.dmg,heat:M.heat,draft:M.draft,
        turbineL:turbineHz(sides[0].nEff)*SIDES[0].pitch,turbineR:turbineHz(sides[1].nEff)*SIDES[1].pitch,accel:M.accel,
""", """      debug(){const g=gears.state;return{gear:g.gear,f:g.f,tone:g.tone,vp:g.vp,rpm:g.tone/GP.redlineHz,toneL:sides[0].hz,toneR:sides[1].hz,
        loadL:sides[0].load,loadR:sides[1].load,effL:sides[0].eff,effR:sides[1].eff,burn:M.burnA,dmg:M.dmg,heat:M.heat,draft:M.draft,accel:M.accel,
""")
b.save()
print("step 3: BAKED (synth loops) is in the engine block, version 2.0.0")
```

- [ ] **Step 4: Run step 3.**

```bash
python prototypes/Dustline/tools/engine2/step3_baked.py
```

Expected: `pushed <sha256> <bytes> bytes`, then `step 3: BAKED (synth loops) is in the engine block, version 2.0.0`.

- [ ] **Step 5: Run the test to see it pass.**

```bash
node prototypes/Dustline/checks/audio-engine.test.cjs
```

Expected: sixteen `PASS` lines, then `16 engine audio checks passed.`

- [ ] **Step 6: Run the regression (0.6).** Expected: 161 checks (`audio-engine.test.cjs: 16 engine audio checks passed.`) and both ai-pace `PASS` lines.

### Task 7: Game wiring and the ship bundle

**Purpose.** The game decodes the ship's assets and hands each one to the engine (spec 2.1, 4.5; fix 10; S11, S12, S34, S48). Step 4 adds an empty `DUSTLINE_SHIP_B64` bundle beside the SFX bundle, `decodeShip()` and `Dustline.audio.ship`. `tools/ship_encode.py` turns the WAV masters into Ogg Opus; the new `apply-sfx.py` embeds the 29-sound SFX bundle and the ship bundle in all three copies. `ship-profile.test.cjs` gains the check that the bundle holds exactly the profile's assets; `audio-browser.py` checks in Edge that every asset decodes and attaches, that the tone runs, and that every loop has the right length and a clean seam at 48 and 44.1 kHz (limits S51).

**Files:**
- Create: `prototypes/Dustline/tools/engine2/step4_ship_bundle.py`, `prototypes/Dustline/tools/ship_encode.py`
- Replace: `prototypes/Dustline/apply-sfx.py` (CRLF; restored after the Write, Step 5)
- Modify: `prototypes/Dustline/checks/ship-profile.test.cjs` (LF, from Task 2), `prototypes/Dustline/checks/audio-browser.py` (CRLF)
- Modify (by `step4_ship_bundle.py` and `apply-sfx.py`): the three game copies
- Generated: `prototypes/Dustline/audio/ships/kestrel/opus/core-*.opus` (8)

- [ ] **Step 1: Write the failing test.** Apply this Edit, then run the line-ending check (expected `LF`).

Edit `prototypes/Dustline/checks/ship-profile.test.cjs`. Replace:

```js
test('phase 1 has no textures or one-shots yet', () => {
  assert.equal(K.textures.length, 0);
  assert.deepEqual(JSON.parse(JSON.stringify(K.oneShots)), { upshift: [], downshift: [], ignite: '', release: '', launch: '' });
```

with:

```js
test('the ship bundle holds exactly the profile assets; phase 1 has no textures or one-shots yet', () => {
  assert.equal(K.textures.length, 0);
  assert.deepEqual(JSON.parse(JSON.stringify(K.oneShots)), { upshift: [], downshift: [], ignite: '', release: '', launch: '' });
  const bundle = JSON.parse(html.match(/const DUSTLINE_SHIP_B64 = (\{[^\r\n]*\});/)[1]);
  const ids = [...K.baked.map(b => b.id), ...K.textures.map(x => x.id), ...K.oneShots.upshift, ...K.oneShots.downshift,
    K.oneShots.ignite, K.oneShots.release, K.oneShots.launch].filter(Boolean);
  assert.deepEqual(Object.keys(bundle).sort(), ids.map(id => 'kestrel:' + id).sort());
```

- [ ] **Step 2: Run it to see it fail.**

```bash
node prototypes/Dustline/checks/ship-profile.test.cjs
```

Expected: six `PASS` lines, then `TypeError: Cannot read properties of null (reading '1')` (the page has no ship bundle yet).

- [ ] **Step 3: Create and run step 4.** Create `prototypes/Dustline/tools/engine2/step4_ship_bundle.py`:

```python
"""Engine 2.0, step 4 (plan Task 7): the game decodes the ship's assets and hands each one to the engine. Adds an empty
DUSTLINE_SHIP_B64 bundle beside the SFX bundle (apply-sfx.py fills it), decodeShip(), and Dustline.audio.ship."""
from common import Build

b = Build()
b.rep("  // Procedural twin-turbine engine (DustlineEngineAudio) + ElevenLabs one-shots, with a WebAudio synth fallback for one-shots.\n",
      "  // Ship engine (DustlineEngineAudio 2.0: gear model, the ship's synth loops) + ElevenLabs one-shots, with a WebAudio synth fallback\n"
      "  // for one-shots.\n")
b.rep("  let audio=null,sound=true,soundMode='sfx',",
      "  // The ship's assets as Ogg Opus, keyed '<ship>:<id>'. apply-sfx.py writes them from audio/ships/<ship>/opus/.\n"
      "  const DUSTLINE_SHIP_B64 = {};\n"
      "  let shipDecoded=0,shipDecoding=false;\n"
      "  let audio=null,sound=true,soundMode='sfx',")
b.rep("""  function audioInit(){
    if(audio)return;
""", """  // Each decoded asset goes straight to the engine, which starts an engine's tone once all eight of its loops are in.
  function decodeShip(){
    if(shipDecoding||!audio||!audio.ctx)return;
    shipDecoding=true;
    const prefix=audio.engine.ship.id+':';
    for(const [key,b64] of Object.entries(DUSTLINE_SHIP_B64)){
      if(!key.startsWith(prefix))continue;
      audio.ctx.decodeAudioData(b64ToArrayBuffer(b64))
        .then(buf=>{if(audio&&audio.engine.attach(key.slice(prefix.length),buf))shipDecoded++;}).catch(()=>{});
    }
  }
  function audioInit(){
    if(audio)return;
""")
b.rep("    decodeAllSfx();\n  }\n", "    decodeAllSfx();decodeShip();\n  }\n")
b.rep("decodedCount:sfxDecodedCount,totalCount:Object.keys(DUSTLINE_SFX_B64).length,",
      "decodedCount:sfxDecodedCount,totalCount:Object.keys(DUSTLINE_SFX_B64).length,"
      "ship:{id:audio?.engine.ship.id||'',decoded:shipDecoded,total:Object.keys(DUSTLINE_SHIP_B64).length},")
b.save()
print("step 4: the game decodes the ship bundle and attaches it to the engine")
```

```bash
python prototypes/Dustline/tools/engine2/step4_ship_bundle.py
```

Expected: `pushed <sha256> <bytes> bytes`, then `step 4: the game decodes the ship bundle and attaches it to the engine`.

- [ ] **Step 4: Create the encoder and encode the loops.** Create `prototypes/Dustline/tools/ship_encode.py`:

```python
"""Encode the Kestrel's WAV masters to Ogg Opus for the ship bundle (spec 4.5).

    python prototypes/Dustline/tools/ship_encode.py

loops/<id>.wav (the baked loops and the texture loops) and shots/<id>.wav (the one-shots) become opus/<id>.opus at 96 kbps
and 48 kHz, with the master's channel count. Masters must be 48 kHz: loop points are stored in samples at 48 kHz. Run
apply-sfx.py afterwards to embed the files. Ogg Opus decodes gaplessly in WebView2 and Chromium (spec 4.5); MP3 would not.
"""
import subprocess
from pathlib import Path

SHIP = Path(__file__).resolve().parents[1] / "audio" / "ships" / "kestrel"
OUT = SHIP / "opus"


def sample_rate(path):
    out = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "a:0", "-show_entries", "stream=sample_rate",
                          "-of", "csv=p=0", str(path)], capture_output=True, text=True, check=True).stdout
    return int(out.strip())


def main():
    masters = sorted((SHIP / "loops").glob("*.wav")) + sorted((SHIP / "shots").glob("*.wav"))
    assert masters, f"no WAV masters in {SHIP / 'loops'} or {SHIP / 'shots'}"
    OUT.mkdir(exist_ok=True)
    keep = {p.stem for p in masters}
    stale = sorted(p.name for p in OUT.glob("*.opus") if p.stem not in keep)
    assert not stale, f"opus/ holds files with no master, delete them by hand first: {stale}"
    for src in masters:
        assert sample_rate(src) == 48000, f"{src.name}: masters must be 48 kHz"
        subprocess.run(["ffmpeg", "-y", "-v", "error", "-i", str(src), "-c:a", "libopus", "-b:a", "96k", "-ar", "48000",
                        "-map_metadata", "-1", str(OUT / f"{src.stem}.opus")], check=True)
    total = sum((OUT / f"{p.stem}.opus").stat().st_size for p in masters)
    print(f"encoded {len(masters)} assets into {OUT}: {total} bytes of Opus")


if __name__ == "__main__":
    main()
```

```bash
python prototypes/Dustline/tools/ship_encode.py
```

Expected: `encoded 8 assets into …\prototypes\Dustline\audio\ships\kestrel\opus: 290693 bytes of Opus` (the planning run; a few bytes either way is fine).

- [ ] **Step 5: Replace `apply-sfx.py` and embed.** Replace the whole content of `prototypes/Dustline/apply-sfx.py` with:

```python
"""Embed the SFX bundle and the ship bundle in the 0.13 build (all three copies) without touching its code.

    python prototypes/Dustline/apply-sfx.py

SFX: sfx/sfx-bundle.json less the five propulsion recordings the engine voices itself (29 sounds).
Ship: every audio/ships/kestrel/opus/<id>.opus (tools/ship_encode.py), keyed "kestrel:<id>". checks/ship-profile.test.cjs
checks that the keys are exactly the Kestrel profile's assets.
The three copies must equal the hash tools/sync_copies.py recorded; index.html is written and pushed to the other two.
"""
import base64
import json
import re
import sys
from pathlib import Path

DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(DIR / "tools"))
import sync_copies as sc  # noqa: E402

BUNDLE_PATH = DIR / "sfx" / "sfx-bundle.json"
SHIP_DIR = DIR / "audio" / "ships" / "kestrel" / "opus"
ENGINE_VOICED = {"turbineLoop", "afterburnerLoop", "slipstreamLoop", "boostIgnite", "boostCooldown"}


def embed(html, name, bundle):
    # Escape HTML delimiters so even an edited bundle cannot close the inline script.
    payload = json.dumps(bundle, separators=(",", ":")).replace("<", "\\u003c").encode("ascii")
    out, count = re.subn(rb"(const " + name + rb" = )\{[^\r\n]*\};", lambda m: m[1] + payload + b";", html)
    assert count == 1, f"expected exactly one {name.decode()}"
    return out


sc.check()
# Bytes throughout, so the page's CRLF line endings survive on any platform.
html = sc.INDEX.read_bytes()
sfx = json.loads(BUNDLE_PATH.read_text(encoding="utf-8"))
assert isinstance(sfx, dict) and len(sfx) == 34, "Expected the 34-sound SFX bundle (33 generated + repairWarnShort, cut from repairWarn)"
assert all(isinstance(k, str) and isinstance(v, str) for k, v in sfx.items())
assert ENGINE_VOICED <= sfx.keys()
sfx = {k: v for k, v in sfx.items() if k not in ENGINE_VOICED}
ship = {f"kestrel:{p.stem}": base64.b64encode(p.read_bytes()).decode("ascii") for p in sorted(SHIP_DIR.glob("*.opus"))}
assert ship, f"no encoded ship assets in {SHIP_DIR}: run tools/ship_encode.py"
assert b"version:'0.13.0'" in html, "Expected the active 0.13 build"
assert html.count(b"/* @dustline-host-bridge:begin */") == html.count(b"/* @dustline-host-bridge:end */") == 1
assert html.count(b"/* @dustline-engine-audio:begin */") == html.count(b"/* @dustline-engine-audio:end */") == 1
updated = embed(embed(html, b"DUSTLINE_SFX_B64", sfx), b"DUSTLINE_SHIP_B64", ship)
assert len(re.findall(rb"<script[ >]", updated)) == 8
opus = sum(len(v) * 3 // 4 for v in ship.values())
if updated == html:
    print(f"All three 0.13 copies already carry these {len(sfx)} sounds and {len(ship)} ship assets.")
    raise SystemExit(0)
temporary = sc.INDEX.with_suffix(".html.tmp")
temporary.write_bytes(updated)
temporary.replace(sc.INDEX)
sc.push()
print(f"Embedded {len(sfx)} sounds and {len(ship)} ship assets (about {opus} bytes of Opus) in all three 0.13 copies; "
      f"page {len(updated)} bytes.")
```

Restore its CRLF line endings (0.6), then embed:

```bash
python -c "import sys;p=sys.argv[1];d=open(p,'rb').read().replace(b'\r\n',b'\n');open(p,'wb').write(d.replace(b'\n',b'\r\n'));print('CRLF',p)" prototypes/Dustline/apply-sfx.py
python prototypes/Dustline/apply-sfx.py
```

Expected: `CRLF prototypes/Dustline/apply-sfx.py`, `pushed <sha256> <bytes> bytes`, then `Embedded 29 sounds and 8 ship assets (about 290… bytes of Opus) in all three 0.13 copies; page 1853068 bytes.` (about 1.85 MB).

- [ ] **Step 6: Run the test to see it pass.**

```bash
node prototypes/Dustline/checks/ship-profile.test.cjs
```

Expected: `ship profile: 7 passed`.

- [ ] **Step 7: Extend the browser check.** Apply these Edits to `prototypes/Dustline/checks/audio-browser.py`, then run the line-ending check (expected `CRLF`).

Edit `prototypes/Dustline/checks/audio-browser.py` (1 of 4). Replace:

```python
"""Live audio smoke test of the shipped build in Edge (offline): unlock, engine and grid rivals running, timing cues in both sound
modes, repeated pause / mute / hidden-tab / restart cycles without a second engine, new nodes or errors, and the engine's frame cost.
```

with:

```python
"""Live audio smoke test of the shipped build in Edge (offline): unlock, the ship's assets decoded and attached, engine and grid
rivals running, timing cues in both sound modes, repeated pause / mute / hidden-tab / restart cycles without a second engine, new
nodes or errors, and the engine's frame cost. Then every loop of the ship bundle is decoded again in OfflineAudioContexts at 48 and
44.1 kHz: its length must be sample-exact and its loop seam continuous.
```

Edit `prototypes/Dustline/checks/audio-browser.py` (2 of 4). Replace:

```python
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
```

with:

```python
import re
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
SHIP_RE = re.compile(r"const DUSTLINE_SHIP_B64 = (\{[^\r\n]*\});")
# Opus coding noise alone reads about 0.1-0.15 on broadband textures at 96 kbps; a seam that does not continue reads about 1.4.
SEAM_LIMIT = {"baked": 0.1, "texture": 0.25}
# Each loop of the Kestrel profile, decoded as the game decodes it. Seam: the RMS difference between the 64 samples around the
# loop end and the 64 samples around the loop start (they must continue each other), relative to the loop's RMS.
SEAMS = """async ([assets, rate]) => {
  const K = DustlineEngineAudio.ships.kestrel, out = [];
  const ctx = new OfflineAudioContext(2, 1, rate);
  for (const e of [...K.baked, ...K.textures]) {
    const kind = K.baked.includes(e) ? 'baked' : 'texture';
    const bin = atob(assets['kestrel:' + e.id]), u8 = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i);
    const buf = await ctx.decodeAudioData(u8.buffer), k = buf.sampleRate / 48000;
    const s = e.loopStart * k, t = e.loopEnd * k;
    let seam = 0;
    for (let c = 0; c < buf.numberOfChannels; c++) {
      const x = buf.getChannelData(c), at = p => { const i = Math.floor(p), f = p - i; return x[i] + (x[i + 1] - x[i]) * f; };
      let rms = 0; for (let i = Math.ceil(s); i < t; i++) rms += x[i] * x[i];
      rms = Math.sqrt(rms / (t - s));
      let d = 0; for (let j = -32; j < 32; j++) d += (at(t + j) - at(s + j)) ** 2;
      seam = Math.max(seam, Math.sqrt(d / 64) / rms);
    }
    out.push({ id: e.id, kind, length: buf.length, expect: Math.round(e.loopEnd * k + 2400 * k), sampleRate: buf.sampleRate, seam });
  }
  return out;
}"""
```

Edit `prototypes/Dustline/checks/audio-browser.py` (3 of 4). Replace:

```python
        page.wait_for_function("Dustline.audio.state==='running' && Dustline.audio.decodedCount===29")
```

with:

```python
        page.wait_for_function("Dustline.audio.state==='running' && Dustline.audio.decodedCount===29")
        page.wait_for_function("Dustline.audio.ship.total>0 && Dustline.audio.ship.decoded===Dustline.audio.ship.total"
                               " && Dustline.audio.engine.tone===true", timeout=10000)
        ship = audio()["ship"]
        assert ship["id"] == "kestrel" and engine()["attached"] == ship["total"], (ship, engine())
```

Edit `prototypes/Dustline/checks/audio-browser.py` (4 of 4). Replace:

```python
        print(json.dumps({"browser": browser.version, "offline": True, "engineCreates": upd["creates"], "engineNodes": nodes,
                          "engineSources": sources, "timing": results, "updates": upd["n"], "meanUpdateMs": round(mean, 4),
                          "worstUpdateMs": round(upd["worst"], 3), "writes": e["writes"], "skipped": e["skipped"], "errors": errors,
                          "checks": "unlock, grid voices, steer and boost, vent and repair cues in sfx and synth, pause park x3, "
                                    "mute suspend/resume x6, hidden tab, 12 restarts with mode and pause cycles"}))
```

with:

```python
        assets = json.loads(SHIP_RE.search((ROOT / "index.html").read_text(encoding="utf-8"))[1])
        seams = [r for rate in (48000, 44100) for r in page.evaluate(SEAMS, [assets, rate])]
        for r in seams:
            assert abs(r["length"] - r["expect"]) <= 1, r
            assert r["seam"] < SEAM_LIMIT[r["kind"]], r
        print(json.dumps({"browser": browser.version, "offline": True, "ship": ship, "engineCreates": upd["creates"], "engineNodes": nodes,
                          "worstSeam": max(seams, key=lambda r: r["seam"]),
                          "engineSources": sources, "timing": results, "updates": upd["n"], "meanUpdateMs": round(mean, 4),
                          "worstUpdateMs": round(upd["worst"], 3), "writes": e["writes"], "skipped": e["skipped"], "errors": errors,
                          "checks": "unlock, ship assets attached and tone on, grid voices, steer and boost, vent and repair cues in "
                                    "sfx and synth, pause park x3, mute suspend/resume x6, hidden tab, 12 restarts with mode and pause "
                                    "cycles, loop lengths and seams at 48 and 44.1 kHz"}))
```

- [ ] **Step 8: Run the browser check.**

```bash
python prototypes/Dustline/checks/audio-browser.py
```

Expected: exit code 0 and one JSON line with `"ship": {"id": "kestrel", "decoded": 8, "total": 8}`, `"engineNodes": 213`, `"engineSources": 60`, a `worstSeam` entry whose `seam` is about 0.044 (limit 0.1), `"errors": []`, and timing results all `PERFECT`. The planning run measured a mean update of 0.0295 ms and a worst of 0.9 ms.

- [ ] **Step 9: Run the regression (0.6).** Expected: 161 checks and both ai-pace `PASS` lines.

### Task 8: Audition

**Purpose.** Render what the user will hear at the checkpoint, from the real simulation, and measure it (spec 3.6, 7; fix 16; S41, S42, S53, S54). `checks/ship-scenarios.cjs` drives the 0.13 simulation on an empty flat track (grid revs, GO launch, cruise, lift, brake downshifts, a second acceleration, boost to 1200 km/h, release) and records 60 Hz frames that mirror `fillAudioFrame`. `checks/audio_lab.py` learns to attach the ship's assets in its offline render. The new `checks/audio_audition.py` renders the engine as played, a propulsion-only pass, and 0.12's engine (1.2.0) on the same frames, gain-matches both to −20 LUFS, and writes `metrics.json` with the targets of S42. `ship_screen.py audition DIR` adds an earshot timeline and per-section A/B comparisons.

**Files:**
- Create: `prototypes/Dustline/checks/ship-scenarios.cjs`
- Modify: `prototypes/Dustline/checks/audio_lab.py` (CRLF), `prototypes/Dustline/checks/ship_screen.py` (LF, from Task 5)
- Replace: `prototypes/Dustline/checks/audio_audition.py` (LF before and after)
- Generated: `prototypes/Dustline/audio/audition-v0.13-p1/` (`engine-as-played.wav`, `engine-matched.wav`, `propulsion.wav`, `previous-as-played.wav`, `previous-matched.wav`, `ab-matched.wav`, `spectrogram-*.png`, `metrics.json`, `earshot.json`), a new section in `audio/ships/kestrel/LOG.md`

- [ ] **Step 1: Create the scenario script and check its frames.** Create `prototypes/Dustline/checks/ship-scenarios.cjs`:

```js
'use strict';
// Audition frames from the real simulation (ship engine spec 7): a race start through the gears, cruise, a lift, brake
// downshifts, a second acceleration, a boost to 1200 km/h and its release, on an empty flat track. Each 60 Hz frame mirrors
// fillAudioFrame (index.html) for a player on the twin faders; the events mirror the game's mapping (GO -> launch,
// boost-start -> ignite, boost-end -> release, boost-pad -> pad). No timing cue can open (eventReadyAt is pushed out).
//     node checks/ship-scenarios.cjs [index.html]   prints {source, sha256, dt, seconds, frames, events, marks, speeds}
const { load } = require('./handling-scenarios.cjs');

const DT = 1 / 60, STEPS = 2, TAIL = 1.5;
// name, seconds, fader (both sides; a number or a function of the time into the segment), brake, set-up at the segment start
const SEGMENTS = [
  { name: 'grid revs', seconds: 2, fader: t => t < .4 ? 0 : t < .9 ? .75 : t < 1.4 ? .1 : .75 },
  { name: 'launch', seconds: 6, fader: .75 },
  { name: 'cruise', seconds: 3, fader: .75 },
  { name: 'lift', seconds: 2, fader: .1 },
  { name: 'brake', seconds: .3, fader: 0, brake: true },
  { name: 'accelerate', seconds: 3, fader: .75 },
  { name: 'boost', seconds: 5, fader: 1, setup: s => Object.assign(s, { charge: 1, heat: 0, heatLeft: 0, heatRight: 0 }) },
  { name: 'release', seconds: 3, fader: .75 },
];

function scenario(file) {
  const { ctx, sha256 } = load(file);
  const sim = new ctx.DustlineSimulation(7);
  sim.reset('race');
  Object.assign(sim.state, { countdown: SEGMENTS[0].seconds, heading: 0, opponents: [], eventReadyAt: 1e9 });
  sim.sampleTrack = d => ({ halfWidth: 1e6, metric: 1, headingRate: 0, heading: 0, curvature: 0, bank: 0, loopX: 0, loopZ: d });
  sim.route.featuresNear = () => [];
  const rivals = Array.from({ length: 5 }, (_, id) => ({ id, dx: 0, dz: 1e6, speed: 0, thr: 1, boost: false, brake: 0, active: false }));
  const frames = [], events = [], marks = [], speeds = {};
  const kmh = () => +(sim.state.speed * 3.6).toFixed(1);
  let i = 0, racing = false, peak = 0;
  for (const seg of SEGMENTS) {
    marks.push([+(i * DT).toFixed(4), seg.name]);
    if (seg.setup) seg.setup(sim.state);
    const from = kmh(), n = Math.round(seg.seconds / DT);
    for (let k = 0; k < n; k++, i++) {
      const t = i * DT, s = sim.state;
      const fader = typeof seg.fader === 'function' ? seg.fader(k * DT) : seg.fader;
      // what the game does on this display frame: sound events from the last steps, then the engine frame
      for (const e of sim.drainEvents()) {
        if (e.type === 'boost-start') events.push([t, 'ignite', { strength: s.boostIntensity }]);
        else if (e.type === 'boost-end') events.push([t, 'release', { strength: s.boostExhausted ? .7 : .9 }]);
        else if (e.type === 'boost-pad') events.push([t, 'pad']);
      }
      if (!racing && s.countdown <= 0) { racing = true; events.push([t, 'launch']); }
      frames.push({ phase: s.finished ? 'finished' : s.countdown > 0 ? 'countdown' : 'racing', countdown: s.countdown, speed: s.speed,
        brake: seg.brake ? 1 : 0, steer: 0, thrL: fader, thrR: fader, boost: s.boostActive, boostIntensity: s.boostIntensity,
        heatLeft: s.heatLeft, heatRight: s.heatRight, burst: s.boostBurst, padBurst: s.padBurstTimer > 0 ? s.padBurstTimer / .65 : 0,
        charge: s.charge, heat: s.heat, hull: s.hull, fault: s.penaltyTime > 0 ? s.penaltyKind || '' : '', draft: s.draft,
        recovery: s.recovery > 0, view: 'chase', blocked: !!s.boostExhausted, timing: null, ticks: false, engineMode: 'baked',
        rivals: rivals.map(r => ({ ...r })) });
      const input = { ...ctx.DustlineDrive.mix(fader, fader), twinDrive: true, boostReady: true, brake: !!seg.brake };
      for (let j = 0; j < STEPS; j++) sim.step(DT / STEPS, input);
      peak = Math.max(peak, kmh());
    }
    speeds[seg.name] = [from, kmh()];
  }
  const boost = speeds.boost[1];
  if (boost < 1150) throw new Error(`the boost segment reaches ${boost} km/h, not about 1200: lengthen it`);
  return { source: file, sha256, dt: DT, seconds: +(frames.length * DT + TAIL).toFixed(4), frames, events, marks, speeds, peakKmh: peak };
}

module.exports = { scenario, SEGMENTS };
if (require.main === module) {
  const path = require('node:path');
  process.stdout.write(JSON.stringify(scenario(process.argv[2] || path.join(__dirname, '../index.html'))));
}
```

```bash
node -e "const r=JSON.parse(require('child_process').execFileSync('node',['prototypes/Dustline/checks/ship-scenarios.cjs','prototypes/Dustline/index.html'],{maxBuffer:1<<28}));console.log('frames',r.frames.length,'seconds',r.seconds,'events',r.events.length);console.log('speeds',JSON.stringify(r.speeds))"
```

Expected: `frames 1458 seconds 25.8 events 3` and `speeds {"grid revs":[0,244.8],"launch":[244.8,600],"cruise":[600,600],"lift":[600,345.9],"brake":[345.9,124.7],"accelerate":[124.7,599.1],"boost":[599.1,1200],"release":[1200,715.2]}`. The boost must reach at least 1150 km/h (S53).

- [ ] **Step 2: Let the offline render attach the ship.** Apply these Edits to `prototypes/Dustline/checks/audio_lab.py`, then run the line-ending check (expected `CRLF`).

Edit `prototypes/Dustline/checks/audio_lab.py` (1 of 5). Replace:

```python
RENDER_JS = """
```

with:

```python
def ship_bundle(path=None):
    """The embedded ship bundle of an HTML build, '<ship>:<id>' -> base64 Ogg Opus. Empty for a build before 0.13."""
    text = Path(path or ROOT / "index.html").read_text(encoding="utf-8")
    m = re.search(r"const DUSTLINE_SHIP_B64 = (\{[^\r\n]*\});", text)
    return json.loads(m.group(1)) if m else {}


RENDER_JS = """
```

Edit `prototypes/Dustline/checks/audio_lab.py` (2 of 5). Replace:

```python
  const eng = E.create(ctx, { output: master.input });
  Object.assign(eng.layers, spec.layers || {});
```

with:

```python
  // LIVE SYNTH frames the engine sends, stamped with the time of the update that sent them (spec 5.3)
  const live = [];
  let now = 0;
  const eng = E.create(ctx, spec.live ? { output: master.input, live: f => live.push([now, f.on, f.bend, f.wheel, f.gain]) } : { output: master.input });
  Object.assign(eng.layers, spec.layers || {});
  // the ship's assets are decoded and attached before rendering, as decodeShip() does in the game (0.13 engines only)
  let attached = 0;
  if (spec.ship && eng.attach) {
    const prefix = eng.ship.id + ':';
    for (const [key, b64] of Object.entries(spec.ship)) {
      if (!key.startsWith(prefix)) continue;
      const bin = atob(b64), u8 = new Uint8Array(bin.length);
      for (let k = 0; k < bin.length; k++) u8[k] = bin.charCodeAt(k);
      if (eng.attach(key.slice(prefix.length), await ctx.decodeAudioData(u8.buffer))) attached++;
    }
  }
```

Edit `prototypes/Dustline/checks/audio_lab.py` (3 of 5). Replace:

```python
      }
```

with:

```python
      }
      now = t;
```

Edit `prototypes/Dustline/checks/audio_lab.py` (4 of 5). Replace:

```python
    stats: eng.stats(), debug: eng.debug(), probes };
```

with:

```python
    stats: eng.stats(), debug: eng.debug(), probes, attached, live };
```

Edit `prototypes/Dustline/checks/audio_lab.py` (5 of 5). Replace:

```python
    def render(self, frames, events=(), seconds=None, layers=None, dt=1 / 60, sr=SR, probe=0):
        seconds = seconds or len(frames) * dt + 1.2
        r = self.page.evaluate(RENDER_JS + "\n", {"frames": frames, "events": [list(e) for e in events], "seconds": seconds,
                                                  "layers": layers or {}, "dt": dt, "sampleRate": sr, "probe": probe})
        info = {k: r[k] for k in ("renderMs", "worstUpdateMs", "meanUpdateMs", "stats", "debug", "probes")}
```

with:

```python
    def render(self, frames, events=(), seconds=None, layers=None, dt=1 / 60, sr=SR, probe=0, ship=None, live=False):
        """ship: a ship bundle (ship_bundle()) to attach before rendering; live: record the LIVE SYNTH frames the engine sends."""
        seconds = seconds or len(frames) * dt + 1.2
        r = self.page.evaluate(RENDER_JS + "\n", {"frames": frames, "events": [list(e) for e in events], "seconds": seconds,
                                                  "layers": layers or {}, "dt": dt, "sampleRate": sr, "probe": probe,
                                                  "ship": ship or {}, "live": live})
        info = {k: r[k] for k in ("renderMs", "worstUpdateMs", "meanUpdateMs", "stats", "debug", "probes", "attached", "live")}
```

- [ ] **Step 3: Replace the audition script.** Replace the whole content of `prototypes/Dustline/checks/audio_audition.py` with:

```python
"""Render and measure the ship engine audition (ship engine spec 3.6 and 7), and compare it with 0.12's engine (1.2.0) driven by
the same frames, at matched loudness.

    python prototypes/Dustline/checks/audio_audition.py [--engine BUILD.html] [--previous BUILD.html] [--out DIR] [--live]

The frames come from the real simulation (checks/ship-scenarios.cjs): GO at 245 km/h through the gears to 600, cruise, a lift,
brake downshifts, a second acceleration, a boost to 1200 km/h and its release. Writes, into DIR (default
prototypes/Dustline/audio/audition-v0.13):
  engine-as-played.wav     the engine through the game's output stage, at in-game level (BAKED)
  engine-matched.wav       the same, gain-matched to -20 LUFS
  propulsion.wav           tone, textures and afterburner only (air, strain, rival and cue muted), at in-game level
  previous-as-played.wav   0.12's engine (1.2.0) driven by the same frames
  previous-matched.wav     the same, gain-matched to -20 LUFS
  ab-matched.wav           per section: previous, then new, both at -20 LUFS
  --live adds (phase 3; runs cargo): live-frames.json (the LIVE SYNTH frames resampled to the 40 Hz log bake_engine reads),
  live-tone.wav (bake_engine's Rust render of it), live-as-played.wav (the engine on LIVE SYNTH: everything but the tone from
  the browser, the tone from Rust), live-matched.wav and ab-baked-live.wav (per section: BAKED, then LIVE SYNTH)
  spectrogram-*.png, metrics.json (sections, gear changes, the spec 3.6 targets and, with --live, the LIVE trim suggestion)
Automated measurement only. It does not establish how the engine sounds; the user's listening decides.
"""
from pathlib import Path
import argparse
import json
import math
import os
import re
import subprocess
import sys

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import audio_lab as lab  # noqa: E402
import pitch  # noqa: E402

ROOT = HERE.parent
APP = ROOT.parents[1] / "launchpad-drummer"
SHIP = ROOT / "audio" / "ships" / "kestrel"
USER_PATCH = Path.home() / "Documents" / "Launchpad Drummer" / "Patches" / "DUSTLINE KESTREL.lbpatch"
TARGET = -20.0
BANDS = [(20, 60), (60, 250), (250, 1000), (1000, 4000), (4000, 10000), (10000, 20000)]
AB = [("grid revs", 0.0, 2.0), ("launch", -0.3, 4.5), ("lift", -0.5, 2.5), ("brake", -0.3, 2.0), ("boost", -0.5, 4.5),
      ("release", 0.0, 3.0)]
PROPULSION = {"air": 0, "strain": 0, "rival": 0, "cue": 0}
TONE_ONLY = {"texture": 0, "strain": 0, "burn": 0, "air": 0, "rival": 0, "cue": 0}
PROBE = 3  # one engine probe every 3 frames (20 Hz)
SHIFT_GUARD = 0.12  # the pitch target skips probes this close to a gear change (cut and glide)


def scenario(build):
    """The audition frames from the real simulation (checks/ship-scenarios.cjs)."""
    out = subprocess.run(["node", str(HERE / "ship-scenarios.cjs"), str(build)], capture_output=True, text=True, check=True,
                         encoding="utf-8").stdout
    return json.loads(out)


def sections(marks, end):
    return [(name, t0, nxt[0]) for (t0, name), nxt in zip(marks, marks[1:] + [(end, "")])]


def cut(audio, a, b):
    return audio[:, int(a * lab.SR):int(b * lab.SR)]


def measure(audio, a, b):
    x = cut(audio, a, b)
    mono = x.mean(axis=0)
    corr = float(np.dot(x[0], x[1]) / (np.linalg.norm(x[0]) * np.linalg.norm(x[1]) + 1e-12))
    stereo = 10 * math.log10(np.mean(x[0] ** 2 + x[1] ** 2) / 2 + 1e-18)
    monod = 10 * math.log10(np.mean(mono ** 2) + 1e-18)
    # integrated loudness needs one 0.4 s gating block: a shorter section (the brake) has none
    return {"lufs": round(lab.lufs(x), 2) if x.shape[1] >= .4 * lab.SR else None, "rms": round(lab.rms_db(x), 2), "peak": round(lab.peak_db(x), 2),
            "bands": [round(lab.band_db(mono, lo, hi), 1) for lo, hi in BANDS], "corr": round(corr, 3), "monoLoss": round(stereo - monod, 2)}


def below200(audio, a, b):
    """Energy share below 200 Hz of the section's mono mix."""
    x = cut(audio, a, b).mean(axis=0)
    spec = np.abs(np.fft.rfft(x * np.hanning(len(x)))) ** 2
    f = np.fft.rfftfreq(len(x), 1 / lab.SR)
    return float(spec[f < 200].sum() / (spec.sum() + 1e-24))


def spectrum(mono, t, n):
    i = int(t * lab.SR)
    seg = mono[i - n // 2:i + n // 2]
    if len(seg) < n:
        return None, None
    return np.fft.rfftfreq(n, 1 / lab.SR), np.abs(np.fft.rfft(seg * np.hanning(n)))


def salience(audio, probes, a, b):
    """Tone level against the local spectral median (±1/3 octave), at each engine's own frequency (debug toneL / toneR)."""
    mono = audio.mean(axis=0)
    vals = []
    for t, d in probes:
        if not a + .2 <= t < b - .2:
            continue
        f, spec = spectrum(mono, t, 16384)
        if f is None:
            continue
        for ft in (d["toneL"], d["toneR"]):
            near = (f > ft * .97) & (f < ft * 1.03)
            band = (f > ft * 2 ** (-1 / 3)) & (f < ft * 2 ** (1 / 3))
            if near.any() and band.any():
                vals.append(20 * math.log10((spec[near].max() + 1e-12) / (np.median(spec[band]) + 1e-12)))
    return round(float(np.median(vals)), 1) if vals else None


def zing(audio, probes, a, b):
    """At each probe, the strongest peak between 1 and 4 kHz: (tone Hz, peak Hz, prominence in dB over the ±1/3-octave median)."""
    mono = audio.mean(axis=0)
    rows = []
    for t, d in probes:
        if not a + .1 <= t < b - .1:
            continue
        f, spec = spectrum(mono, t, 8192)
        if f is None:
            continue
        band = np.flatnonzero((f >= 1000) & (f < 4000))
        k = band[np.argmax(spec[band])]
        near = (f > f[k] * 2 ** (-1 / 3)) & (f < f[k] * 2 ** (1 / 3))
        hi = (f >= 1000) & (f < 4000)
        centroid = float((f[hi] * spec[hi] ** 2).sum() / ((spec[hi] ** 2).sum() + 1e-24))
        rows.append((d["tone"], float(f[k]), 20 * math.log10((spec[k] + 1e-12) / (np.median(spec[near]) + 1e-12)), centroid))
    return rows


def gear_changes(probes):
    return [[round(t, 2), p["gear"], d["gear"]] for (t, d), (_, p) in zip(probes[1:], probes) if d["gear"] != p["gear"]]


def pitch_follow(audio, probes):
    """Probes away from gear changes whose tracked fundamental (pitch.py) is within 3 % of the gear model's f."""
    track = pitch.track(audio.mean(axis=0), lab.SR)  # 100 ms frames every 50 ms: frame j is centred on j * .05 + .05
    shifts = [t for t, _, _ in gear_changes(probes)]
    hits, n, misses = 0, 0, []
    for t, d in probes:
        if t < .2 or any(abs(t - s) <= SHIFT_GUARD for s in shifts):
            continue
        j = int(round((t - .05) / .05))
        if not 0 <= j < len(track):
            continue
        n += 1
        f0 = track[j][1]
        if f0 and abs(f0 / d["f"] - 1) <= .03:
            hits += 1
        elif len(misses) < 12:
            misses.append([round(t, 2), round(d["f"], 1), round(f0, 1) if f0 else None])
    return {"probes": n, "share": round(hits / max(n, 1), 3), "firstMisses": misses}


def targets(prop, probes, secs, picks):
    """The spec 3.6 measured targets, on the propulsion render. Evidence, not a verdict."""
    at = {n: (a, b) for n, a, b in secs}
    rows = [r for n, a, b in secs if n != "grid revs" for r in zing(prop, probes, a, b)]
    tone, peak, prom, centroid = (np.array([r[i] for r in rows]) for i in range(4))
    corr = lambda a, b: round(float(np.corrcoef(a, b)[0, 1]), 3) if len(a) > 2 and a.std() > 0 and b.std() > 0 else None
    follow = pitch_follow(prop, probes)
    low = below200(prop, *at["cruise"])
    loud = lab.lufs(cut(prop, *at["cruise"]))
    tex = {k: v["z"] for k, v in picks.items() if v.get("kind") == "loop"}
    return {
        "bassy": {"target": "at cruise, at least 35 % of the propulsion energy below 200 Hz", "measured": round(low, 3), "meets": low >= .35},
        "zing": {"target": "a clear tonal peak between 1 and 4 kHz that rises with speed",
                 "medianProminenceDb": round(float(np.median(prom)), 1), "medianPeakHz": round(float(np.median(peak))),
                 "medianPeakOverTone": round(float(np.median(peak / tone)), 2), "peakCorrelationWithTone": corr(tone, peak),
                 "bandCentroidCorrelationWithTone": corr(tone, centroid)},
        "pitch": {"target": "fundamental within 3 % of the gear model's f on at least 90 % of the probes away from gear changes",
                  **follow, "meets": follow["share"] >= .9},
        "layers": {"target": "each texture reads as its description in earshot match: z >= 2.5",
                   "measured": tex or "n/a: no texture in the profile yet", "meets": all(z >= 2.5 for z in tex.values()) if tex else None},
        "loudness": {"target": "propulsion about -16 LUFS at cruise", "measured": round(loud, 2), "deltaDb": round(loud + 16, 2)},
    }


def ab_file(out, name, marks, pairs):
    """For each AB section, each (label, audio, gain) excerpt in turn, 0.4 s apart and 1 s between sections."""
    at = {n: t for t, n in marks}
    fade = int(.02 * lab.SR)
    parts, ab_marks, cursor = [], [], 0.0
    for section, off, length in AB:
        a = max(0.0, at[section] + off)
        for k, (label, src, g) in enumerate(pairs):
            clip = cut(src, a, a + length) * g
            env = np.ones(clip.shape[1])
            env[:fade], env[-fade:] = np.linspace(0, 1, fade), np.linspace(1, 0, fade)
            parts.append(clip * env)
            ab_marks.append((cursor, f"{section}: {label}"))
            pause = .4 if k < len(pairs) - 1 else 1.0
            cursor += clip.shape[1] / lab.SR + pause
            parts.append(np.zeros((2, int(pause * lab.SR))))
    ab = np.concatenate(parts, axis=1)
    lab.write_wav(out / f"{name}.wav", ab)
    lab.spectrogram_png(out / f"spectrogram-{name}.png", ab.mean(axis=0), marks=[(t, l.split(": ")[1][0].upper()) for t, l in ab_marks])
    return [{"t": round(t, 2), "label": l} for t, l in ab_marks]


def live_log(sent, seconds):
    """The LIVE SYNTH frames as the 40 Hz log bake_engine's scenario mode reads: at each 1/40 s tick, the latest frame the engine
    sent at or before it. The engine sends one frame per control step, on whole display frames (spec 5.1)."""
    out, j, cur = [], 0, [False, 0.0, 0.0, 0.0]
    for k in range(int(round(seconds * 40))):
        while j < len(sent) and sent[j][0] <= k / 40 + 1e-9:
            cur = sent[j][1:]
            j += 1
        out.append([bool(cur[0]), round(cur[1], 6), round(cur[2], 6), round(cur[3], 6)])
    return {"rate": 40, "frames": out}


def bake_live(patch, frames, wav):
    """bake_engine's scenario mode: the real GameEngineVoice plays the frame log (cargo, from launchpad-drummer)."""
    env = dict(os.environ, PATH=str(Path.home() / ".cargo" / "bin") + os.pathsep + os.environ.get("PATH", ""))
    subprocess.run(["cargo", "run", "-q", "-p", "lpd-core", "--release", "--example", "bake_engine", "--", "scenario",
                    "--patch", str(patch), "--frames", str(frames), "--out", str(wav)], cwd=APP, env=env, check=True)


def read_wav(path, n):
    """Any WAV as (2, n) float64 at 48 kHz (decoded by ffmpeg), cut or padded to n samples."""
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", str(path), "-f", "f32le", "-ac", "2", "-ar", str(lab.SR), "-"],
                         capture_output=True, check=True).stdout
    x = np.frombuffer(raw, dtype="<f4").reshape(-1, 2).T.astype(np.float64)
    return np.pad(x, ((0, 0), (0, max(0, n - x.shape[1]))))[:, :n]


def match_gain(audio):
    return 10 ** ((TARGET - lab.lufs(audio)) / 20)


def overall(audio, info=None):
    r = {"lufs": round(lab.lufs(audio), 2), "truePeak": round(lab.true_peak_db(audio), 2), "peak": round(lab.peak_db(audio), 2),
         "nonFinite": int(np.count_nonzero(~np.isfinite(audio))), "discontinuities": lab.discontinuities(audio)[:12],
         "hfBursts": lab.hf_bursts(audio.mean(axis=0))[:12]}
    if info:
        r.update(renderMs=round(info["renderMs"]), worstUpdateMs=round(info["worstUpdateMs"], 2), meanUpdateMs=round(info["meanUpdateMs"], 3),
                 stats=info["stats"])
    return r


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--engine", default=str(ROOT / "index.html"))
    ap.add_argument("--previous", default=str(ROOT / "dustline-rift-run-v0.12.html"))
    ap.add_argument("--out", default=str(ROOT / "audio" / "audition-v0.13"))
    ap.add_argument("--live", action="store_true", help="also render LIVE SYNTH (phase 3; runs bake_engine through cargo)")
    ap.add_argument("--patch", default="", help="LIVE SYNTH patch; default the user's DUSTLINE KESTREL patch, else the profile's")
    args = ap.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    sc = scenario(args.engine)
    frames, events, marks, seconds = sc["frames"], [tuple(e) for e in sc["events"]], [tuple(m) for m in sc["marks"]], sc["seconds"]
    secs = sections(marks, len(frames) * sc["dt"])
    ship = lab.ship_bundle(args.engine)
    with lab.Lab(args.engine) as L:
        new, info = L.render(frames, events, seconds=seconds, probe=PROBE, ship=ship)
        prop, _ = L.render(frames, events, seconds=seconds, layers=PROPULSION, ship=ship)
        if args.live:
            rest, linfo = L.render([dict(f, engineMode="live") for f in frames], events, seconds=seconds, ship=ship, live=True)
            tone_only, _ = L.render(frames, events, seconds=seconds, layers=TONE_ONLY, ship=ship)
        errors = list(L.errors)
    assert info["attached"] >= 8 and info["stats"]["tone"], f"the ship did not start its tone (attached {info['attached']})"
    with lab.Lab(args.previous) as P:
        old, _ = P.render(frames, events, seconds=seconds)
        errors += P.errors
    probes = [(t, d) for t, d in info["probes"]]
    picks_file = SHIP / "picks.json"
    picks = json.loads(picks_file.read_text(encoding="utf-8"))["picks"] if picks_file.exists() else {}
    report = {"seconds": round(new.shape[1] / lab.SR, 2), "frames": len(frames), "framesSeconds": round(len(frames) * sc["dt"], 3),
              "errors": errors, "attached": info["attached"],
              "scenario": {"sha256": sc["sha256"], "speedsKmh": sc["speeds"], "events": sc["events"]},
              "gearChanges": gear_changes(probes),
              "engine": dict(overall(new, info), sections={n: dict(measure(new, a, b), toneSalience=salience(new, probes, a, b)) for n, a, b in secs}),
              "propulsion": dict(overall(prop), sections={n: dict(measure(prop, a, b), below200=round(below200(prop, a, b), 3)) for n, a, b in secs}),
              "previous": dict(overall(old), sections={n: measure(old, a, b) for n, a, b in secs}),
              "targets": targets(prop, probes, secs, picks),
              "marks": [{"t": round(t, 2), "label": n} for t, n in marks]}
    g_new, g_old = match_gain(new), match_gain(old)
    report["matchGainDb"] = {"engine": round(20 * math.log10(g_new), 2), "previous": round(20 * math.log10(g_old), 2)}
    lab.write_wav(out / "engine-as-played.wav", new)
    lab.write_wav(out / "engine-matched.wav", new, gain=g_new)
    lab.write_wav(out / "propulsion.wav", prop)
    lab.write_wav(out / "previous-as-played.wav", old)
    lab.write_wav(out / "previous-matched.wav", old, gain=g_old)
    for name, audio in (("engine", new), ("propulsion", prop), ("previous", old)):
        lab.spectrogram_png(out / f"spectrogram-{name}.png", audio.mean(axis=0), marks=marks)
    report["ab"] = ab_file(out, "ab-matched", marks, [("previous", old, g_old), ("new", new, g_new)])
    if args.live:
        sent = linfo["live"]
        if not sent:
            raise SystemExit("the engine sent no LIVE SYNTH frame: LIVE needs the phase-3 engine (Task 22)")
        patch = Path(args.patch) if args.patch else USER_PATCH if USER_PATCH.exists() else SHIP / "kestrel-engine.lbpatch"
        (out / "live-frames.json").write_text(json.dumps(live_log(sent, seconds)), encoding="utf-8")
        bake_live(patch, out / "live-frames.json", out / "live-tone.wav")
        tone = read_wav(out / "live-tone.wav", new.shape[1])
        live = rest + tone
        g_live = match_gain(live)
        cruise = next((a, b) for n, a, b in secs if n == "cruise")
        baked_tone, live_tone = lab.lufs(cut(tone_only, *cruise)), lab.lufs(cut(tone, *cruise))
        m = re.search(r"liveTrimDb:(-?\d+(?:\.\d+)?)", Path(args.engine).read_text(encoding="utf-8"))
        trim = float(m.group(1)) if m else 0.0
        racing = [s for s in sent if s[0] >= marks[1][0]]
        report["live"] = dict(overall(live), patch=str(patch), sent=len(sent),
                              sentHz=round((len(racing) - 1) / (racing[-1][0] - racing[0][0]), 1) if len(racing) > 1 else None,
                              bakedToneCruiseLufs=round(baked_tone, 2), liveToneCruiseLufs=round(live_tone, 2), trimNowDb=trim,
                              trimSuggestionDb=round(2 * (trim + baked_tone - live_tone)) / 2,
                              sections={n: measure(live, a, b) for n, a, b in secs})
        report["matchGainDb"]["live"] = round(20 * math.log10(g_live), 2)
        lab.write_wav(out / "live-as-played.wav", live)
        lab.write_wav(out / "live-matched.wav", live, gain=g_live)
        lab.spectrogram_png(out / "spectrogram-live.png", live.mean(axis=0), marks=marks)
        report["abLive"] = ab_file(out, "ab-baked-live", marks, [("baked", new, g_new), ("live", live, g_live)])
    (out / "metrics.json").write_text(json.dumps(report, indent=1), encoding="utf-8")
    e = report["engine"]
    print(json.dumps({k: e[k] for k in ("lufs", "truePeak", "peak", "nonFinite", "renderMs", "worstUpdateMs", "meanUpdateMs", "stats")}))
    print("attached", info["attached"], "discontinuities", len(e["discontinuities"]), "hfBursts", len(e["hfBursts"]), "errors", errors)
    print("speeds km/h", json.dumps(sc["speeds"]))
    print("gear changes", json.dumps(report["gearChanges"]))
    print(f"{'section':12s} {'LUFS':>6s} {'peak':>6s} {'corr':>5s} {'sal':>5s} {'<200':>5s}  bands <60 60-250 250-1k 1-4k 4-10k 10k+")
    for n, m in e["sections"].items():
        p, o = report["propulsion"]["sections"][n], report["previous"]["sections"][n]
        print(f"{n:12s} {str(m['lufs']):>6s} {m['peak']:6.1f} {m['corr']:5.2f} {str(m['toneSalience']):>5s} {p['below200']:5.2f}  {m['bands']}")
        print(f"{'  previous':12s} {str(o['lufs']):>6s} {o['peak']:6.1f} {o['corr']:5.2f} {'':>5s} {'':>5s}  {o['bands']}")
    print("targets", json.dumps(report["targets"]))
    if args.live:
        lv = report["live"]
        print("live", json.dumps({k: lv[k] for k in ("lufs", "truePeak", "sent", "sentHz", "bakedToneCruiseLufs", "liveToneCruiseLufs",
                                                     "trimNowDb", "trimSuggestionDb", "patch")}))


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Render the phase-1 audition** (about 20 s of rendering in Edge).

```bash
python prototypes/Dustline/checks/audio_audition.py --out prototypes/Dustline/audio/audition-v0.13-p1
```

Expected (the planning run on a phase-1 copy): a JSON line with `"lufs": -12.8` and `"truePeak": -3.71`; `attached 8 discontinuities 0 …`; the speeds of Step 1; `gear changes [[2.05, 0, 4], [2.2, 4, 5], [2.55, 5, 6], [2.95, 6, 7], [11.95, 7, 6], …]` (GO into 4th at 2.05 s, then 5th, 6th and 7th); a section table; and `targets` with bassy 0.452 (meets), pitch share 0.924 of 422 probes (meets), zing median peak 1172 Hz at 39.5 dB prominence with a tone correlation of 0.18, layers "n/a: no texture in the profile yet", and cruise propulsion −13.61 LUFS against about −16 (2.4 dB hot). Report the numbers as measured; do not tune anything to them without the user.

- [ ] **Step 5: Add the audition mode to the screening tool.** Apply this Edit to `prototypes/Dustline/checks/ship_screen.py`, then run the line-ending check (expected `LF`).

Edit `prototypes/Dustline/checks/ship_screen.py`. Replace:

```python
if __name__ == "__main__":
    mode = sys.argv[1] if len(sys.argv) > 1 else ""
    if mode == "loops":
        loops()
```

with:

```python
def audition(folder):
    """earshot on an audition folder (checks/audio_audition.py): a timeline of the as-played render with the section marks as
    cues, and per section an A/B of the previous engine against the new one at matched loudness, plus BAKED against LIVE SYNTH
    when the folder holds a LIVE render. Sections shorter than 1 s are heard for 1 s from their start."""
    api = earshot()
    m = json.loads((folder / "metrics.json").read_text(encoding="utf-8"))
    marks = [(x["t"], x["label"]) for x in m["marks"]]
    secs = [(n, a, max(b, a + 1.0)) for (a, n), (b, _) in zip(marks, marks[1:] + [(m["framesSeconds"], "")])]
    tl = api.timeline(str(folder / "engine-as-played.wav"), cues=[{"t": t, "kind": "cue", "id": n} for t, n in marks])
    out = {"timeline": {"summary": tl["summary"], "sections": tl.get("sections"), "events": (tl.get("events") or [])[:60]}, "ab": {}}
    lines = [f"Folder `{folder.name}`.", "", f"Timeline of engine-as-played.wav: {tl['summary']}", ""]
    pairs = [("previous > new", "previous-matched.wav", "engine-matched.wav")]
    if (folder / "live-matched.wav").exists():
        pairs.append(("baked > live", "engine-matched.wav", "live-matched.wav"))
    for label, a, b in pairs:
        rows = out["ab"][label] = {}
        lines += [f"A/B {label}, both at -20 LUFS:", ""]
        for n, t0, t1 in secs:
            r = api.ab(str(folder / a), str(folder / b), a_start=t0, a_end=t1, b_start=t0, b_end=t1)
            rows[n] = {"cosine": r["cosine_ab"], "differences": r["differences"], "summary": r["summary"]}
            lines.append(f"- {n} ({t0:.2f}-{t1:.2f} s): {r['summary']}")
        lines.append("")
    old = DIR / "audio" / "audition-v0.12" / "engine-matched.wav"
    if old.exists():
        r = api.ab(str(old), str(folder / "engine-matched.wav"))
        out["abV012"] = {"cosine": r["cosine_ab"], "differences": r["differences"], "summary": r["summary"]}
        lines.append(f"Whole file against audition-v0.12/engine-matched.wav (0.12's own scenarios, not the same frames): {r['summary']}")
    (folder / "earshot.json").write_text(json.dumps(out, indent=1), encoding="utf-8")
    log_section(f"Audition {folder.name}", "\n".join(lines))
    print(json.dumps({"timeline": tl["summary"], "ab": {k: {n: v["cosine"] for n, v in rows.items()} for k, rows in out["ab"].items()}}))


if __name__ == "__main__":
    mode = sys.argv[1] if len(sys.argv) > 1 else ""
    if mode == "loops":
        loops()
    elif mode == "audition" and len(sys.argv) > 2:
        audition(Path(sys.argv[2]))
```

- [ ] **Step 6: Screen the audition** (earshot venv).

```bash
PY="/d/MY DOCS/MY TOOLS/earshot/.venv/Scripts/python.exe"
"$PY" -I prototypes/Dustline/checks/ship_screen.py audition prototypes/Dustline/audio/audition-v0.13-p1
```

Expected: one JSON line with an earshot timeline summary (the planning run: "synthwave / synth lead … electrical hum") and, under `"ab"` → `"previous > new"`, one cosine per section (the planning run: 0.42–0.79). It writes `audition-v0.13-p1/earshot.json` and the "Audition audition-v0.13-p1" section of `LOG.md`.

### Task 9: Regression, patch copy and README

**Purpose.** Close phase 1: everything passes, the patch is on PLAY for the user, and the README describes the phase-1 engine (spec 4.1, 9).

**Files:**
- Modify: `prototypes/Dustline/README.md` (CRLF)
- Copy: `prototypes/Dustline/audio/ships/kestrel/kestrel-engine.lbpatch` → `C:\Users\DEROK137\Documents\Launchpad Drummer\Patches\kestrel-engine.lbpatch`

- [ ] **Step 1: Run the regression (0.6).** Expected: 161 checks (`audio-engine` 16, `ship-gears` 17, `ship-profile` 7 and the other 121) and both ai-pace `PASS` lines.

- [ ] **Step 2: Run the browser harnesses (0.6).** Expected: eight `PASS` lines.

- [ ] **Step 3: Put the patch on PLAY.**

```bash
ls "/c/Users/DEROK137/Documents/Launchpad Drummer/Patches/"
cp -n prototypes/Dustline/audio/ships/kestrel/kestrel-engine.lbpatch "/c/Users/DEROK137/Documents/Launchpad Drummer/Patches/"
cmp prototypes/Dustline/audio/ships/kestrel/kestrel-engine.lbpatch "/c/Users/DEROK137/Documents/Launchpad Drummer/Patches/kestrel-engine.lbpatch" && echo copied
```

Expected: `copied`. If a `kestrel-engine.lbpatch` was already there and differs, `cp -n` keeps it and `cmp` reports the difference: ask the user before replacing it.

- [ ] **Step 4: README.** Replace the whole `## Engine audio` section of `prototypes/Dustline/README.md` (from that heading up to, not including, `## Verification and rebuilding`) with the text below. Do it with a byte-preserving replacement: write the text with the Write tool to `prototypes/Dustline/readme-section.tmp`, then run

```bash
python -c "import sys;p,s=sys.argv[1:3];d=open(p,'rb').read().decode('utf-8');n=open(s,encoding='utf-8').read().replace('\r\n','\n').strip('\n').replace('\n','\r\n');a=d.index('\r\n## Engine audio\r\n')+2;b=d.index('\r\n## Verification and rebuilding\r\n')+2;assert a<b;open(p,'wb').write((d[:a]+n+'\r\n\r\n'+d[b:]).encode('utf-8'))" prototypes/Dustline/README.md prototypes/Dustline/readme-section.tmp && rm prototypes/Dustline/readme-section.tmp
```

The section (if the measurements of Tasks 5–8 differ from the planning run, use the measured numbers: nodes, sources, seam, page size):

```markdown
## Engine audio

The engine (`DustlineEngineAudio` 2.0.0) is driven by a ship profile and a gear model. The Kestrel is the only ship for now; a later ship is a new profile plus its assets, not new code. Airflow, heat strain, damage misfires, vent steam, rival engines, the timing cues, the ducking rules and the output stage (glue compressor −11 dB 3.8:1, limiter −1.5 dB 20:1, master gain 0.75) are the 1.2.0 layers. The ElevenLabs one-shots (contact, gates, laps, countdown, hazards, timing cues) remain: the page carries 29 of them.

**Gear model.** Seven gears with top speeds 80, 150, 220, 300, 390, 490 and 700 km/h. Within a gear the fundamental is `124 Hz × v / top`, never below 41 Hz. An upshift comes at 0.97 of the gear's top while the craft accelerates, at least 0.22 s after the last shift; while it waits, the pitch holds at 1.03 × redline. A downshift comes when the speed falls to 0.92 × the lower gear's top, at least 0.09 s after the last shift. In gears 1–6 the rules read a paced speed that rises at most 250 km/h per second, so each launch upshift is a full sweep with an audible drop (0.19, 0.54 and 0.93 s after GO). Past 700 km/h in 7th, boost keeps lifting the pitch: `124 × (v / 700)^0.8`, about 191 Hz at 1200 km/h. Cruise (600 km/h) sits at 0.857 of 7th, off the limiter. On the grid the pitch follows the throttle (neutral revs); GO picks the gear from speed (4th at full throttle) with the launch sound and is not a shift. An upshift cuts the tone 9 dB for 40 ms, glides down over 60 ms and blips the zing (+6 dB at 8 × the fundamental); a downshift blips the throttle (+3 dB for 80 ms). The model is deterministic and does not depend on the frame rate: it advances in 1/120 s substeps.

**Tone.** The Keys synth patch `audio/ships/kestrel/kestrel-engine.lbpatch` (KESTREL ENGINE) holds the sound: SUB and a sync-warped OSC A at the fundamental through a driven 24 dB low-pass, a 3-voice OSC B two octaves up that bypasses the filter for the zing, and FX DRIVE TUBE; the mod wheel opens both warps, the cutoff, the filter drive and both oscillator levels. The app's own Rust synth renders it offline (`crates/lpd-core/examples/bake_engine.rs`) into eight 2 s loops: four pitch anchors (48, 75, 117 and 182 Hz), each on-load (wheel 0.85) and off-load (wheel 0.2). The loops are phase-locked, so the engine starts all eight of an engine together and crossfades them at equal gain: linearly in log frequency between the two nearest anchors, and by a smoothstep on load. Every loop of an engine plays at `fundamental / anchor`.

**Twin engines.** The left engine sits at pan −0.34, the right at +0.34 and 0.35 % higher. Each engine's load follows its own fader: full thrust is at the fader's 0 dB mark (0.75), the boost zone adds no load, and braking takes 45 % off. Load rises within about 55 ms and falls within 110 ms; below 0.25 the tone moves to the hollow off-load loops.

**Kept layers.** The strain resonators now ring from a pulse wave at the fundamental, which reaches the resonators only. A misfire dips the tone briefly and detunes its own engine 0.6 %; thrust stays audible. In this first 2.0 step the 1.2.0 afterburner (ignition, burn, sputter, release) still plays; the ship's own boost and shift sounds follow in the next step. Rivals keep the 1.2.0 procedural engine.

**Assets and lifecycle.** The ship's loops are Ogg Opus at 96 kbps and 48 kHz, which WebView2 and Chromium decode gaplessly. They are embedded as `DUSTLINE_SHIP_B64`, one line beside the SFX bundle, keyed `kestrel:<id>`. The game decodes them and hands each one to the engine (`attach`); the engine itself never decodes audio. The engine builds 213 nodes (60 sources) once and only writes parameters per frame. Pause, mute, hidden tab and restart behave as in 1.2.0. Variation comes from a private seeded generator, never `Math.random` or the simulation's random stream.

**Ship profile.** `DustlineEngineAudio.ships.kestrel` holds `id`, `name`, `gears` (`tops`, `idleHz` 41, `redlineHz` 124, `boostExponent` 0.8, `upMin` 0.22, `downMin` 0.09, `overRev` 1.03, `pace` 250), `patch` (the `.lbpatch` JSON), `patchName` (`DUSTLINE KESTREL`), `liveNote` 48, `baked` (eight `{id, anchorHz, load, loopStart, loopEnd}`, in samples at 48 kHz: 0.05 s lead-in, then the 2 s loop), `textures`, `oneShots` and `mix` (levels in dB, shift cut and blips, pan, right-engine pitch, LIVE trim). The gear model is `DustlineEngineAudio.gears`.

**Playing the patch.** `kestrel-engine.lbpatch` is copied to `Documents\Launchpad Drummer\Patches`, so PLAY lists it as KESTREL ENGINE. Hold note 48 (one octave below middle C) and use the pitch wheel as speed and the mod wheel as load. Save a tweak as "Dustline Kestrel" (stored as `DUSTLINE KESTREL.lbpatch`). To hear it in the game, copy its `params` into `kestrel-engine.lbpatch`, run `python prototypes/Dustline/tools/engine2/profile_patch.py`, then bake, encode and embed (Tools below).

**Tools** (paths from `D:\MY DOCS\MY TOOLS\LAUNCHBAND`):

- `python prototypes/Dustline/tools/sync_copies.py check|diff|push` compares the three copies with the recorded hash (`tools/copies.sha256`). `tools/engine2/step*.py` are the byte-preserving edits that built 2.0, one per step.
- Bake, from `launchpad-drummer`: `cargo run -p lpd-core --release --example bake_engine -- loops --patch <file.lbpatch> --out ..\prototypes\Dustline\audio\ships\kestrel\loops` (`check --patch <file>` reports any value the app would clamp). Then `python prototypes/Dustline/tools/ship_encode.py` and `python prototypes/Dustline/apply-sfx.py`.
- earshot screening, in the earshot venv: `checks/ship_screen.py loops` writes `audio/ships/kestrel/LOG.md` and `loops/analysis.json` (pitch with `checks/pitch.py`, seams, phase, band shares, earshot `measure` and `describe`).
- Audition: `python prototypes/Dustline/checks/audio_audition.py --out prototypes/Dustline/audio/audition-v0.13-p1`, then `checks/ship_screen.py audition <folder>` in the earshot venv. The frames come from the real simulation (`checks/ship-scenarios.cjs`), and 0.12's engine is rendered on the same frames for comparison.

**Checks.** 161 deterministic checks (`audio-engine` 16, `ship-gears` 17, `ship-profile` 7 and the other 121), the 8 browser harnesses and `ai-pace.cjs --verify`. The baked loops measure within 0.01 % of their anchors; the worst decoded seam is 0.044 (limit 0.1, at 48 and 44.1 kHz). The page is about 1.85 MB. The phase-1 audition is in `audio/audition-v0.13-p1/` (`metrics.json`, `earshot.json`); these are measurements, and the listening verdict is the user's.
```

Then apply these Edits and run the line-ending check (expected `CRLF`):

Edit `prototypes/Dustline/README.md` (1 of 4). Replace:

```markdown
# Dustline — On the Limit 0.12
```

with:

```markdown
# Dustline — On the Limit 0.13
```

Edit `prototypes/Dustline/README.md` (2 of 4). Replace:

```markdown
The three current files are byte-identical, with uniform CRLF:
```

with:

```markdown
**0.13 replaces the propulsion sound** with engine audio 2.0 (see Engine audio below). Driving, boost, heat, scoring, bindings and MIDI protections are unchanged, and `dustline-rift-run-v0.12.html` joins the untouched archives as the engine's "before" reference.

The three current files are byte-identical, with uniform CRLF:
```

Edit `prototypes/Dustline/README.md` (3 of 4). Replace:

```markdown
- `prototypes/Dustline/dustline-rift-run-v0.12.html`
```

with:

```markdown
- `prototypes/Dustline/dustline-rift-run-v0.13.html`
```

Edit `prototypes/Dustline/README.md` (4 of 4). Replace:

```markdown
The HTML still contains eight inline scripts: Three.js, route, simulation, MIDI, craft, world, mixer and game/input/rendering. `DustlineSimulation.controls` is the shared mixer/calibration source. The game audio/host bridge markers remain stable for helpers. `apply-sfx.py` validates all three copies, refreshes only the current 28-sound bundle and preserves CRLF. `window.Dustline.version` remains `0.12.0`; audio engine version is `1.2.0`.
```

with:

```markdown
The HTML still contains eight inline scripts: Three.js, route, simulation, MIDI, craft, world, mixer and game/input/rendering. `DustlineSimulation.controls` is the shared mixer/calibration source. The game audio/host bridge markers remain stable for helpers. `apply-sfx.py` validates all three copies, refreshes the 29-sound bundle and the ship bundle (`DUSTLINE_SHIP_B64`) and preserves CRLF. `window.Dustline.version` is `0.13.0`; audio engine version is `2.0.0`.
```

- [ ] **Step 5: Hash check (0.6).** Expected: `ok <sha256>`.

- [ ] **Step 6: Report, then CHECKPOINT 1.** Report to the user, as measurements only:
  - checks: 161 deterministic, 8 harnesses, ai-pace, 5 bake-tool tests;
  - the loops (Task 5 JSON and the `LOG.md` ranges);
  - Edge: assets decoded and attached, nodes and sources, worst seam, update cost, page size;
  - the audition numbers of Task 8 (LUFS, true peak, gear-change times, the S42 targets, cruise propulsion against −16 LUFS) and the earshot timeline and A/B cosines;
  - what to listen to: `prototypes/Dustline/audio/audition-v0.13-p1/engine-as-played.wav` (as played), `propulsion.wav` (propulsion only), `ab-matched.wav` (per section: 0.12's engine, then the new one, both at −20 LUFS) and the spectrograms; the game itself (`prototypes/Dustline/index.html` in Edge); and the patch on PLAY (KESTREL ENGINE: hold note 48, pitch wheel as speed, mod wheel as load).

**CHECKPOINT 1. Stop here and wait for the user's listening verdict. Do not start Task 10 on your own.**

**If the verdict brings a patch tweak.** When the user saved a tweak on PLAY as "Dustline Kestrel" and wants it in BAKED:

1. Check it (PowerShell, from `launchpad-drummer`): `cargo run -p lpd-core --release --example bake_engine -- check --patch "C:\Users\DEROK137\Documents\Launchpad Drummer\Patches\DUSTLINE KESTREL.lbpatch"`. Expected: `patch ok: DUSTLINE KESTREL (…), nothing clamped`.
2. Copy its `params` into the shipped patch, which keeps `patchVersion`, `name` ("KESTREL ENGINE") and `category` (Git Bash, from the root):

```bash
python -c "import json,sys;u=json.load(open(sys.argv[1],encoding='utf-8'));p=sys.argv[2];k=json.load(open(p,encoding='utf-8'));k['params']=u['params'];open(p,'w',encoding='utf-8',newline='\n').write(json.dumps(k,indent=2)+'\n')" "/c/Users/DEROK137/Documents/Launchpad Drummer/Patches/DUSTLINE KESTREL.lbpatch" prototypes/Dustline/audio/ships/kestrel/kestrel-engine.lbpatch
```

3. Put it in the profile: `python prototypes/Dustline/tools/engine2/profile_patch.py`. Expected: `pushed …`, then `profile: the Kestrel patch is kestrel-engine.lbpatch`.
4. `node prototypes/Dustline/checks/ship-profile.test.cjs`. Expected: `ship profile: 7 passed`. If a check of spec 3.2's patch choices fails (the tweak changed LEGATO, the bend range, an oscillator's octave, the filter around ZING or the wheel routes), show the user which one and ask: those choices are spec decisions.
5. Run again, without recreating any file: the check of Task 4 Step 4, the bake and the screening of Task 5 Steps 2 and 4, the encode and embed commands of Task 7 Steps 4 and 5, the browser check of Task 7 Step 8, the audition render and screening of Task 8 Steps 4 and 6, then the regression (0.6) and the hash check. Copy the new `kestrel-engine.lbpatch` to the patch folder only if the user asks (it would replace KESTREL ENGINE there). Report again, as in Task 9 Step 6, and wait for the verdict.

---

## Tone revision R1 (after the failed CHECKPOINT 1)

**Status.** Tasks 1–9 were done as written above; their steps and expectations stay as the record of the original phase 1. CHECKPOINT 1 failed on 2026-11-06. The user's verdict, which is binding: "it sound like an electric razer, the previous sound was more matching the feel". The approved direction: restore the previous engine's mechanical body, turbine movement and noise texture while keeping the new speed/gear behaviour, and produce another loudness-matched A/B. The spec records the revision in its section 10 and in every section marked "R1". R1 is one revision of phase 1, not a new phase: it ends at a revised CHECKPOINT 1, and Task 10 waits for that verdict. No ElevenLabs generation; `generate-ship-sfx.py --go` is not part of R1.

**Evidence** (supporting, not a diagnosis; 0.12 is the character reference). 1.2.0's propulsion is an irregular firing pulse train with a saturated combustion body, a drifting turbine with blade noise and intake resonance, and noise exhausts that crackle with the firing pulses. Phase 1 replaced all of it with phase-locked loops of one periodic patch, a sync-warped saw plus a 3-voice OSC B two octaves up, with no noise and no irregularity. earshot read the phase-1 on-load loops as "error buzz" (z +3.1 to +3.7), "electrical hum" and "phone vibration buzz", and the audition timeline as "synth lead / electrical hum / dubstep". The R1 prototype (scratch, before this amendment) rebuilt 1.2.0's layers on the gear model and found three level effects that the design below handles: the gear load is 1 at cruise where 1.2.0's load was 0.75 (so R1 reads 1.2.0's own throttle), the full-thrust spool must land on 1.2.0's 0.639 (so `n = 0.746 × rpm`), and the stereo loops put each engine through the stereo panner's stereo law, about 5 dB louder than 1.2.0's equal-power pan of a mono engine (so each engine is one channel from its misfire dip on).

**Superseded** (exactly these; everything else in 0.2–0.5 and in Tasks 1–24 stands):
- spec 1 "What is replaced": the 1.2.0 core, turbine, exhaust and jet hiss are restored on the gear model (spec 3.7), not replaced; the 1.2.0 spool is still replaced by the gear model.
- spec 3.2 tone choices: OSC A warp `sync` (amount 0.1) → `off` (0); OSC B on → off (its other settings stay in the file, unused); FX drive tone 9000 → 2500 Hz; `voice.volumeDb` 0 → −4.7; the wheel routes to `oscA.warpAmt` (+0.45), `oscB.warpAmt` (+0.6) and `oscB.level` (+0.3) are removed; the cutoff route goes from +0.367 to +0.2.
- spec 3.4: the stereo-law pan of the stereo loops becomes an equal-power pan of a mono engine.
- S33 is done in R1, not in Task 14 (Task 14's amendment below drops its copy).
- S38 totals, S42 (two targets added), invariant 4 (node counts) and the counts of Tasks 14–16 and 22–24, as amended below.

**Kept** (not touched by R1): the gear model and every number of it (S1–S5, pacing, GO, shift timing), shift shaping S6–S7, the load S4, the twin engines' pan positions and right-engine pitch S31, the BAKED loop set and its bake (anchors, loads, layout, seam crossfade: S13–S15), the 1.2.0 afterburner in phase 1 (S10), the phase-2 and phase-3 designs, `VERSION` `'2.0.0'`, the output stage, the archives, and every other decision.

**R1 decisions** (new; the user may veto any of them at the revised CHECKPOINT 1):

| # | Decision |
|---|---|
| R1-1 | **Restored layers.** Per engine, 1.2.0's mechanical core (firing pulse train `B.firing`, combustion body, LP, drive, saturation, irregular AM), turbine (5 partials, blade noise, two band-passes, intake and 1480 Hz peaks, LP, drift and vibrato), exhaust (pink noise, LP, body and mid peaks, drive, AM, firing crackle, soft saturation) and jet hiss (white noise, HP, LP, AM), built from 1.2.0's code with 1.2.0's buffers, per-engine values and level formulas: 69 nodes and 20 sources per engine, built once. |
| R1-2 | **Spool from the gear model:** `n = 0.746 × tone / redlineHz × (1 − 0.07 × dmg × (0.5 + 0.5 × surge))`, clamped to 0–1.3; full thrust in 7th (rpm 0.857) gives 1.2.0's full-thrust spool 0.639. Turbine `turbineHz(n)` and firing `fireHz(n)` (1.2.0's curves, `E.model`) times the engine's own pitch (right engine, misfire detune, re-light). The surge table comes from its own generator `rng32(seed ^ 0x5129E)`, so the engine's other seeded draws are unchanged. |
| R1-3 | **Effort as in 1.2.0:** the restored layers read 1.2.0's throttle per side, the raw fader (countdown `0.1 + 0.62 × fader`, finished 0, recovering × 0.5), through `loadStep` and 1.2.0's effort `load × (0.78 + 0.22 × accel/22) × (1 − 0.7 × brake) + 0.1 × burn`, clamped to 0–1.1. The gear model's load keeps driving the tone. |
| R1-4 | **Join and level:** mech sum → two 46 Hz high-passes (Q 0.62) → level → the engine's `shift`, so the restored layers take the ignition cut, the throttle blip, the zing blip and the misfire dips, outside `toneSum`. Level `MECH × (0.55 + 0.45 × effort^0.9)` with `MECH = 0.92 / TONE`: the side level carries `TONE`, so the restored layers keep 1.2.0's absolute side level `0.92 × lvl`. |
| R1-5 | **Mono engines:** `fault.channelCount = 1`, `channelCountMode = 'explicit'` on each engine, so the ±0.34 pan is the equal-power pan of a mono engine. R1's loops are exactly L = R, so the downmix loses nothing. |
| R1-6 | **Mixer layers** `growl`, `turbine`, `exhaust` return (1.2.0's names), between `tone` and `strain`. |
| R1-7 | **S33 early:** `vl` carries `TONE`, `toneSum` carries `LV.tone`. Muting the tone layer (and later LIVE's crossfade) silences the loops and leaves the restored layers. |
| R1-8 | **Patch** (spec 3.2 R1): the edits listed under Superseded. At cruise the tone then sits about 6 dB under the restored layers. |
| R1-9 | **Intake suction on the restored spool** (added at R1 Step 10, see the finding there): the 1.2.0 air layer's intake suction reads the mean restored spool `(nL + nR) / 2` and the mean restored 1.2.0 load, as 1.2.0 read its spool and load, instead of phase 1's gear rpm and gear load. The lift flutter (`blowoff`) keeps phase 1's inputs: its trigger needs a load drop over 0.5 within one control step, which `loadStep` never allows at the game's frame rates, in 0.12 or in phase 1. |

**Files and callers:**
- Modify: `prototypes/Dustline/audio/ships/kestrel/kestrel-engine.lbpatch` (LF). Read by `tools/engine2/profile_patch.py` (into `ships.kestrel.patch` in the three copies), by `bake_engine check|loops` (Tasks 4–5), by `checks/ship-profile.test.cjs` tests 4 and 5 (through the profile), and in phase 3 by the game's `engineHost.start(JSON.stringify(engine.ship.patch))` (S25) when the user has no `DUSTLINE KESTREL.lbpatch`.
- Create: `prototypes/Dustline/tools/engine2/step_r1b_intake.py` (LF, R1-9, added at Step 10): a second R1 game writer, built the same way.
- Create: `prototypes/Dustline/tools/engine2/pieces/mech.js`, `prototypes/Dustline/tools/engine2/step_r1_mech.py` (both LF). `step_r1_mech.py` is the first additional game writer of R1: it uses `common.Build` (hash check against `tools/copies.sha256`, exact-count replacements, CRLF write, `sync_copies` push), and stops before writing when an anchor is missing, so it runs once.
- Modify (by `tools/engine2/profile_patch.py`, `tools/engine2/step_r1_mech.py`, `tools/engine2/step_r1b_intake.py` and `apply-sfx.py` only): the three game copies.
- Regenerate: `audio/ships/kestrel/loops/*.wav`, `loops/bake.json`, `loops/analysis.json`, `audio/ships/kestrel/LOG.md`, `audio/ships/kestrel/opus/*.opus`, the ship bundle line in the three copies.
- Modify: `prototypes/Dustline/checks/audio-engine.test.cjs` (LF), `prototypes/Dustline/checks/ship-profile.test.cjs` (LF), `prototypes/Dustline/checks/audio_audition.py` (LF), `prototypes/Dustline/checks/ship_screen.py` (LF), `prototypes/Dustline/README.md` (CRLF).
- Create: `prototypes/Dustline/audio/audition-v0.13-p1-r1/` (the R1 audition). Its `rejected/` folder holds the snapshot of the rejected phase 1, taken before R1: `engine.js` (the phase-1 engine block with its ship bundle line, from the copies at hash `a04a36ae…`; sha256 `2941503f889a198977bc61f4bdce0ece6367756b4935f83224c943b097528b9d`), `kestrel-engine.lbpatch`, `loops-bake.json`, `loops-analysis.json` and `LOG-synth-loops.md`. `audio/audition-v0.13-p1/` stays untouched.
- Not touched: `dustline-rift-run-v0.12.html` and older archives, `apply-sfx.py`, `tools/ship_encode.py`, `checks/audio_lab.py`, `checks/audio-browser.py`, the Rust bake tool, `launchpad-drummer/`, the user's patch folder.

**Expected counts, fixed before implementation** (each must match exactly):
- `audio-engine.test.cjs`: 17 (16 + the new R1 test); its test 1 node range becomes 280–340 (R1 build: 319 before the loops arrive; 298 after Task 14).
- `ship-profile.test.cjs`: 7 (tests 4 and 5 rewritten for the superseded patch choices only).
- Deterministic total: 162 (`audio-engine` 17, `ship-gears` 17, `ship-profile` 7 and the other 121); both ai-pace `PASS` lines.
- `audio-browser.py`: `"decoded": 8, "total": 8`, `"engineNodes": 351`, `"engineSources": 100` (213 + 138 and 60 + 40), worst seam below 0.1, `"errors": []`.
- `bake_engine` example tests: 5 passed; the screening: 8 loops, `"failed": []`.
- Eight browser harness `PASS` lines; `sync_copies.py check` prints `ok <sha256>`.
- Only these assertions change, each because it encodes a superseded tone choice or a superseded count: audio-engine test 1's node range; ship-profile test 4 (oscillator, filter, drive and volume choices) and test 5 (wheel routes). No technical test is weakened.

- [ ] **R1 Step 1: Write the failing tests.** Apply these Edits, then run the line-ending check on each file (expected `LF`).

Edit `prototypes/Dustline/checks/audio-engine.test.cjs` (1 of 2). Replace:

```js
  assert(st.nodes > 100 && st.nodes < 260, 'node count ' + st.nodes);
```

with:

```js
  // tone revision R1 restores 1.2.0's propulsion layers, 69 nodes per engine: 319 nodes before the loops arrive (298 from Task 14)
  assert(st.nodes > 280 && st.nodes < 340, 'node count ' + st.nodes);
```

Edit `prototypes/Dustline/checks/audio-engine.test.cjs` (2 of 2). Replace:

```js
console.log(`${passed} engine audio checks passed.`);
```

with:

```js
test('tone revision R1: the 1.2.0 core, turbine, exhaust and jet hiss play on each engine at the gear rpm, join it at the shift, outside the tone layer', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input });
  attachAll(ctx, eng);
  const Mo = E.model, SPOOL = .746, firing = E.buffersFor(ctx).firing;
  const into = node => ctx.nodes.filter(n => n.out && n.out.has(node));
  const bank = bankOf(ctx), toneSum = [next(next(bank[0])), next(next(bank[8]))], shift = toneSum.map(next);
  const level = [], layers = [];
  for (const [i, sh] of shift.entries()) {
    // the firing pulse train of each engine reaches its own shift, never the tone sum or the other engine
    const own = ctx.nodes.filter(n => n.kind === 'buffer' && n.buffer === firing && reachable(ctx, n).has(sh));
    assert.equal(own.length, 1, 'one firing source per engine');
    assert(!reachable(ctx, own[0]).has(toneSum[i]) && !reachable(ctx, own[0]).has(shift[1 - i]), 'outside the tone sum, own engine only');
    const lv = into(sh).filter(n => n !== toneSum[i]);
    assert.equal(lv.length, 1, 'the restored layers enter the shift through one level');
    const sum = into(into(into(lv[0])[0])[0])[0];
    level.push(lv[0]); layers.push(into(sum));
    assert.equal(layers[i].length, 4, 'core, turbine, exhaust and jet hiss');
    // from the misfire dip on, each engine is one channel, so its pan is an equal-power pan
    const fault = next(next(next(sh)));
    assert.deepEqual([fault.channelCount, fault.channelCountMode], [1, 'explicit']);
  }
  // full thrust at cruise in 7th: 1.2.0's full-thrust spool on its curves; 1.2.0's effort from the raw fader
  drive(ctx, eng, 3, frame({ speed: 600 / 3.6 }));
  let d = eng.debug();
  assert.equal(d.gear, 7);
  near(d.nL, SPOOL * d.tone / GP.redlineHz, 1e-9, 'spool = 0.746 x rpm'); near(d.nL, .639, .001, "1.2.0's full-thrust spool");
  near(d.turbineL, Mo.turbineHz(d.nL), 1e-9, "1.2.0's turbine curve"); near(d.turbineR, d.turbineL * MIX.rightPitch, 1e-9, 'right engine 0.35 % higher');
  near(d.fireL, Mo.fireHz(d.nL), 1e-9, "1.2.0's firing curve"); near(d.fireR, d.fireL * 1.016, 1e-9, "1.2.0's right engine fires 1.6 % faster");
  near(d.effL, 1, .005, 'the gear load is full thrust'); near(d.mechEffL, .75 * .78, .002, "1.2.0's effort at the fader's 0.75");
  assert(layers.flat().every(g => g.gain.value > 0) && level.every(g => g.gain.value > 1), 'all four layers sound');
  // R1-9 (added at R1 Step 10): the intake suction follows the restored spool, as in 1.2.0
  const intake = 900 + 1700 * (d.nL + d.nR) / 2;
  assert.equal(ctx.nodes.filter(n => n.kind === 'biquad' && n.type === 'bandpass' && Math.abs(n.frequency.value - intake) < 1).length, 1, 'intake band-pass at 900 + 1700 n');
  // the tone layer mutes the loops only; the growl, turbine and exhaust layers mute the restored layers
  eng.layers.tone = 0;
  drive(ctx, eng, .5, frame({ speed: 600 / 3.6 }));
  assert(toneSum.every(g => g.gain.value === 0) && layers.flat().every(g => g.gain.value > 0) && level.every(g => g.gain.value > 1), 'tone layer');
  eng.layers.tone = 1; eng.layers.growl = eng.layers.turbine = eng.layers.exhaust = 0;
  drive(ctx, eng, .5, frame({ speed: 600 / 3.6 }));
  assert(toneSum.every(g => g.gain.value === 1) && layers.flat().every(g => g.gain.value === 0), 'growl, turbine and exhaust layers');
  // through a launch the spool follows the tone, shift glide included, so the turbine drops at an upshift
  const c2 = makeContext(), e2 = E.create(c2, { output: E.createMaster(c2).input }), seen = [];
  drive(c2, e2, .2, frame({ phase: 'countdown', speed: 0 }));
  drive(c2, e2, 1.5, t => frame({ speed: 68 + 40 * t }), 1 / 60, x => seen.push(x));
  for (const x of seen) if (x.gear) near(x.nL, SPOOL * x.tone / GP.redlineHz, 1e-9, 'spool follows the tone');
  const k = seen.findIndex((x, j) => j && seen[j - 1].gear && x.gear > seen[j - 1].gear);
  assert(k > 0, 'an upshift');
  const before = seen[k - 1].turbineL, after = Math.min(...seen.slice(k, k + 12).map(x => x.turbineL));
  assert(after < before / 1.15, `the turbine drops at the upshift: ${before} -> ${after}`);
  assert.deepEqual(ctx.problems, []); assert.deepEqual(c2.problems, []);
});

console.log(`${passed} engine audio checks passed.`);
```

Edit `prototypes/Dustline/checks/ship-profile.test.cjs` (1 of 2). Replace:

```js
test('patch choices of section 3.2: legato, bend 24, SUB and BODY at the fundamental, ZING two octaves up around the filter', () => {
  const p = K.patch.params;
  assert.equal(p.voice.mode, 'legato'); assert.equal(p.voice.bendRange, 24); assert.equal(p.voice.glide, 0); assert.equal(p.voice.drift, 0);
  assert.deepEqual([p.sub.on, p.sub.shape, p.sub.oct, p.sub.toFilter], [true, 'sine', -1, true]);
  assert.deepEqual([p.oscA.on, p.oscA.table, p.oscA.warp, p.oscA.oct, p.oscA.unison, p.oscA.toFilter], [true, 'basic', 'sync', -1, 1, true]);
  assert.deepEqual([p.oscB.on, p.oscB.table, p.oscB.warp, p.oscB.oct, p.oscB.unison, p.oscB.toFilter], [true, 'basic', 'sync', 2, 3, false]);
  assert.equal(p.oscA.phaseRand, 0, 'OSC A starts at phase 0, so the baked loops line up');
  assert.deepEqual([p.filter.on, p.filter.type, p.filter.keyTrack, p.filter.envAmt], [true, 'lp24', 0, 0]);
  assert(p.filter.drive > 0, 'filter drive');
  assert.deepEqual([p.fx.drive.on, p.fx.drive.type], [true, 'tube']);
```

with:

```js
test('patch choices of section 3.2 (tone revision R1): legato, bend 24, SUB and a plain saw BODY at the fundamental, dark, OSC B off', () => {
  const p = K.patch.params;
  assert.equal(p.voice.mode, 'legato'); assert.equal(p.voice.bendRange, 24); assert.equal(p.voice.glide, 0); assert.equal(p.voice.drift, 0);
  assert.deepEqual([p.sub.on, p.sub.shape, p.sub.oct, p.sub.toFilter], [true, 'sine', -1, true]);
  assert.deepEqual([p.oscA.on, p.oscA.table, p.oscA.pos, p.oscA.warp, p.oscA.oct, p.oscA.unison, p.oscA.toFilter], [true, 'basic', .6666667, 'off', -1, 1, true]);
  assert.equal(p.oscB.on, false, 'no OSC B: the restored 1.2.0 turbine carries the top');
  assert.equal(p.oscA.phaseRand, 0, 'OSC A starts at phase 0, so the baked loops line up');
  assert.deepEqual([p.filter.on, p.filter.type, p.filter.keyTrack, p.filter.envAmt], [true, 'lp24', 0, 0]);
  assert(p.filter.drive > 0, 'filter drive');
  assert.deepEqual([p.fx.drive.on, p.fx.drive.type, p.fx.drive.tone], [true, 'tube', 2500]);
  assert.equal(p.voice.volumeDb, -4.7, 'at cruise the tone sits 6 dB under the restored layers');
```

Edit `prototypes/Dustline/checks/ship-profile.test.cjs` (2 of 2). Replace:

```js
test('the wheel drives warp, cutoff, drive and both oscillator levels, and nothing else', () => {
  const r = K.patch.params.routes;
  assert(r.every(x => x.source === 'wheel' && x.aux === null && !x.bypass && x.amount > 0));
  assert.deepEqual([...r].map(x => x.dest).sort(), ['filter.cutoff', 'filter.drive', 'oscA.level', 'oscA.warpAmt', 'oscB.level', 'oscB.warpAmt']);
});
```

with:

```js
test('the wheel drives the cutoff (+0.2), the filter drive and the BODY level, and nothing else (tone revision R1: no warp)', () => {
  const r = K.patch.params.routes;
  assert(r.every(x => x.source === 'wheel' && x.aux === null && !x.bypass && x.amount > 0));
  assert.deepEqual([...r].map(x => x.dest).sort(), ['filter.cutoff', 'filter.drive', 'oscA.level']);
  assert.equal(r.find(x => x.dest === 'filter.cutoff').amount, .2);
});
```

- [ ] **R1 Step 2: Run them to see them fail.**

```bash
node prototypes/Dustline/checks/audio-engine.test.cjs
node prototypes/Dustline/checks/ship-profile.test.cjs
```

Expected: `AssertionError [ERR_ASSERTION]: node count 181` and `AssertionError [ERR_ASSERTION]: Expected values to be strictly deep-equal:`.

- [ ] **R1 Step 3: Revise the patch.** Apply these six Edits to `prototypes/Dustline/audio/ships/kestrel/kestrel-engine.lbpatch`, then run the line-ending check (expected `LF`):
  1. `"warp": "sync", "warpAmt": 0.1, "unison": 1,` → `"warp": "off", "warpAmt": 0.0, "unison": 1,` (OSC A only; OSC B's line has `"unison": 3`).
  2. `"oscB": {"on": true,` → `"oscB": {"on": false,`.
  3. `"volumeDb": 0.0}` → `"volumeDb": -4.7}`.
  4. Remove the two lines `{"source": "wheel", "dest": "oscA.warpAmt", "amount": 0.45, …},` and `{"source": "wheel", "dest": "oscB.warpAmt", "amount": 0.6, …},`, and change `"dest": "filter.cutoff", "amount": 0.367,` to `"dest": "filter.cutoff", "amount": 0.2,`.
  5. Remove the last route `{"source": "wheel", "dest": "oscB.level", "amount": 0.3, …}` and the comma before it, so the `oscA.level` route closes the list.
  6. `"tone": 9000.0, "mix": 0.6}` → `"tone": 2500.0, "mix": 0.6}` (FX drive).

  The cutoff stays at 301 Hz; see the finding under Step 5.

Then check it (PowerShell, from `launchpad-drummer`, cargo PATH of 0.6):

```powershell
cargo run -p lpd-core --release --example bake_engine -- check --patch ..\prototypes\Dustline\audio\ships\kestrel\kestrel-engine.lbpatch
```

Expected: `patch ok: KESTREL ENGINE (..\prototypes\Dustline\audio\ships\kestrel\kestrel-engine.lbpatch), nothing clamped`.

- [ ] **R1 Step 4: Put the patch in the profile.**

```bash
python prototypes/Dustline/tools/engine2/profile_patch.py
node prototypes/Dustline/checks/ship-profile.test.cjs
```

Expected: `pushed <sha256> <bytes> bytes`, `profile: the Kestrel patch is kestrel-engine.lbpatch`, then `ship profile: 7 passed`.

- [ ] **R1 Step 5: Bake and screen the loops.** PowerShell, from `launchpad-drummer`:

```powershell
cargo run -p lpd-core --release --example bake_engine -- loops --patch ..\prototypes\Dustline\audio\ships\kestrel\kestrel-engine.lbpatch --out ..\prototypes\Dustline\audio\ships\kestrel\loops
cargo test -p lpd-core --example bake_engine
```

Expected: eight `core-…` lines, `wrote 8 loops to ..\prototypes\Dustline\audio\ships\kestrel\loops`, then `test result: ok. 5 passed; 0 failed`. Then, in the earshot venv (Git Bash): `"$PY" -I prototypes/Dustline/checks/ship_screen.py loops`. Expected: `{"loops": 8, …, "failed": []}`, f0 on each anchor within 0.01 %. (The rejected loops' screening is kept in `audition-v0.13-p1-r1/rejected/`.)

  **Finding during R1 (2026-11-06), resolved inside the amendment.** The first R1 patch also lowered the cutoff from 301 to 220 Hz (the R1 prototype's value). Its bake passed every Rust test, but the screening failed two equal-gain crossfade midpoints of the fundamental, `core-117-off>core-182-off` −1.37 dB and `core-182-on>core-182-off` −1.91 dB (limit −1 dB, unchanged): with the off-load cutoff near 290 Hz, the low-pass's phase shift at the fundamental moved too far between anchors and between loads. Scratch bakes (same patch otherwise, `ship_screen.py`'s midpoint formula): cutoff 260 Hz −1.15 dB (fails), LP 12 at 220 Hz −0.39 dB (passes, brighter above the cutoff), 300 Hz −0.77 dB (passes). The cutoff therefore stays at 301 Hz, not superseded; the lower cutoff route alone darkens the on-load loops (about 400 Hz off-load to 980 Hz on-load, was 500 Hz–2.6 kHz); the 1–4 kHz energy of the on-load loops stays under 0.3 % (rejected 3–7.5 %). The test of Step 1 keeps the original filter assertion, without the cutoff. Re-bake, re-run `profile_patch.py` and re-screen; expected `"failed": []`, worst midpoint about −0.76 dB.

- [ ] **R1 Step 6: Create the restored layers and run the R1 step.** Create `prototypes/Dustline/tools/engine2/pieces/mech.js`:

```js
    /* ---- the 1.2.0 propulsion layers, restored by tone revision R1 (spec 3.7) ----
     * Per engine, 1.2.0's mechanical core (the firing pulse train and the combustion body, low-passed, driven into saturation,
     * moving irregularly), turbine (a whine with related partials, a weak inharmonic stage tone, blade noise, intake resonance and
     * slow drift), exhaust (pink noise opened by effort, body and mid resonances, crackling with the firing pulses) and jet hiss.
     * The gear model drives them in place of 1.2.0's spool. They play in both tone modes, outside the tone layer, and join each
     * engine at its shift cut: mech sum -> two 46 Hz high-passes (1.2.0's bus had them) -> level -> shift. The level takes back the
     * side level's toneDb, so these layers keep 1.2.0's level (0.92 x lvl) whatever the ship's tone level. From the misfire dip on,
     * each engine is one channel: the pan is then 1.2.0's equal-power pan of a mono engine, where a stereo input would take the
     * panner's stereo law, 5 dB louder and wider. R1's loops are mono (L = R), so the downmix loses nothing. */
    const MECH=.92/TONE,SPOOL_RPM=.746;
    // 1.2.0's slow surge of a damaged turbine, from its own seeded generator, so the engine's other random draws stay as they were
    const surgeRnd=rng32((opt.seed==null?0x5EED:opt.seed>>>0)^0x5129E),surgeTable=Array.from({length:64},()=>surgeRnd()*2-1);
    const surge=t=>{const x=(t*1.9)%64,i=Math.floor(x),f=x-i,a=surgeTable[i],b=surgeTable[(i+1)%64];return lerp(a,b,f*f*(3-2*f));};
    function buildMech(s){
      const o=s.o,sum=T.gain(1);
      const gs=T.buf(B.firing,1,o.off),glp=T.bq('lowpass',420,.9),gdrive=T.gain(1.6),gsh=T.shaper(B.sat),gam=T.am('lfoMid',o.lfoOff,.3),gl=T.gain(0);
      T.chain(gs,glp,gdrive,gsh,gam.node,gl,sum);
      const fo=T.osc('sine',50,B.combustion),fhp=T.bq('highpass',72,.7),fog=T.gain(.28),fvib=T.buf(B.lfoMid,1,o.lfoOff*1.3+.5),fvg=T.gain(9);
      T.chain(fo,fhp,fog,glp);fvib.connect(fvg);fvg.connect(fo.detune);
      const genv=T.buf(B.firingEnv,1,o.off);
      const oA=T.osc('sine',300,B.wave),oB=T.osc('sine',450),oC=T.osc('triangle',150),oD=T.osc('sine',600),oE=T.osc('sine',800);
      const gA=T.gain(1),gB=T.gain(.3),gC=T.gain(.3),gD=T.gain(.07),gE=T.gain(.035),tsum=T.gain(1);
      oA.connect(gA);oB.connect(gB);oC.connect(gC);oD.connect(gD);oE.connect(gE);
      for(const g of[gA,gB,gC,gD,gE])g.connect(tsum);
      const blade=T.buf(B.white,1,o.off*1.7),bbp=T.bq('bandpass',700,9),gbl=T.gain(0),bbp0=T.bq('bandpass',350,16),gbl0=T.gain(0);
      T.chain(blade,bbp,gbl,tsum);T.chain(blade,bbp0,gbl0,tsum);
      const vib=T.buf(B.lfoMid,1,o.lfoOff*2.3),vg=T.gain(4);vib.connect(vg);for(const x of[oA,oB,oC,oD,oE])vg.connect(x.detune);
      const drift=T.buf(B.lfoDrift,1,o.lfoOff*3.1),dg=T.gain(7);drift.connect(dg);for(const x of[oA,oB,oC,oD,oE])dg.connect(x.detune);
      const driftB=T.buf(B.lfoSlow,1,o.lfoOff*4.7+1),dgB=T.gain(5);driftB.connect(dgB);dgB.connect(oB.detune);dgB.connect(oE.detune);
      const tpk=T.bq('peaking',o.intake,3.2,5),tpk2=T.bq('peaking',1480,4,2),tlp=T.bq('lowpass',2200,.6),tam=T.am('lfoSlow',o.lfoOff*1.4,.12),gt=T.gain(0);
      T.chain(tsum,tpk,tpk2,tlp,tam.node,gt,sum);
      const es=T.buf(B.pink,1,o.off*1.3),elp=T.bq('lowpass',300,.7),ebody=T.bq('peaking',o.body,.9,4),emid=T.bq('peaking',560,.9,2.5),edrive=T.gain(2.2),
        eam=T.am('lfoFast',o.lfoOff*.6,.3),eam2=T.am('lfoMid',o.lfoOff*1.9,.18),ecr=T.amBy(genv,.15),esh=T.shaper(B.satSoft),ge=T.gain(0);
      T.chain(es,elp,ebody,emid,edrive,eam.node,eam2.node,ecr.node,esh,ge,sum);
      const hs=T.buf(B.white,1,o.off*3.7),hhp=T.bq('highpass',2600,.6),hlp=T.bq('lowpass',9000,.5),ham=T.am('lfoFast',o.lfoOff*2.2,.35),gh=T.gain(0);
      T.chain(hs,hhp,hlp,ham.node,gh,sum);
      const hpA=T.bq('highpass',46,.62),hpB=T.bq('highpass',46,.62),lv=T.gain(0);
      T.chain(sum,hpA,hpB,lv,s.shift);
      s.fault.channelCount=1;s.fault.channelCountMode='explicit';
      s.m={load:0,eff:0,n:0,ft:0,fire:0,gs,genv,fo,fog,glp,gl,gam,oA,oB,oC,oD,oE,vg,dg,bbp,gbl,bbp0,gbl0,tlp,gt,elp,ge,eam,ecr,gh,hhp,lv};
    }
    for(const s of sides)buildMech(s);
    // Per control step. The spool is SPOOL_RPM x rpm: full thrust in top gear (rpm 0.857) sits at 1.2.0's full-thrust spool 0.639,
    // an upshift drops it and boost lifts it. 1.2.0's curves give the firing rate and the turbine pitch, which follow the engine's
    // own pitch (the right engine 0.35 % higher and firing 1.6 % faster, the misfire detune, the re-light). The effort is 1.2.0's,
    // from 1.2.0's throttle (thr, the raw fader), so the same game state works these layers as hard as 1.2.0 did; the gear model's
    // load keeps driving the tone. Damage surge and every level are 1.2.0's.
    function mech(s,heatS,tone,thr,dt){
      const o=s.o,m=s.m,ev=s.hz/(tone*s.pitch),dmg=M.dmg,bA=M.burnA,accN=sat(M.accel/22),sg=surge(tClock);
      m.load=loadStep(m.load,thr,dt);
      const eff=clamp(m.load*(.78+.22*accN)*(1-.7*M.brakeA)+.1*bA,0,1.1);
      m.eff=eff;
      const n=clamp(SPOOL_RPM*tone/GP.redlineHz*(1-.07*dmg*(.5+.5*sg)),0,1.3),ng=Math.pow(sat(n/1.1),1.4);
      const ft=turbineHz(n)*s.pitch*ev,fr=fireHz(n)*o.fire*ev/FIRE_RATE;
      m.n=n;m.ft=ft;m.fire=fr*FIRE_RATE;
      S(m.gs.playbackRate,fr,.045,.4,2.6);S(m.genv.playbackRate,fr,.045,.4,2.6);S(m.fo.frequency,fr*FIRE_RATE,.045,10,200);
      S(m.fog.gain,.2+.14*eff-.1*dmg,.1,0,.6);
      S(m.glp.frequency,190+430*eff+300*bA,.05,60,2400);
      S(m.gl.gain,LV.growl*.11*(.45+.55*eff)*(1-.18*bA),.04,0,1);
      S(m.gam.depth.gain,.3+.22*dmg,.2,0,.8);S(m.gam.node.gain,.7-.22*dmg,.2,0,1);
      S(m.oA.frequency,ft,.04,40,6000);S(m.oB.frequency,ft*o.rB,.04,40,6000);S(m.oC.frequency,ft*.5,.04,20,3000);
      S(m.oD.frequency,ft*o.rD,.04,40,8000);S(m.oE.frequency,ft*o.rE,.04,40,9000);
      S(m.bbp.frequency,ft*2.04,.05,100,9000);S(m.gbl.gain,.016*ng*(.5+eff),.06,0,.1);
      S(m.bbp0.frequency,ft,.04,60,6000);S(m.gbl0.gain,.03*ng*(.6+.6*eff),.06,0,.15);
      S(m.vg.gain,3+20*heatS+26*dmg,.15,0,60);S(m.dg.gain,7+6*heatS,.3,0,30);
      S(m.tlp.frequency,ft*(1.9+1.3*sat(n)),.05,300,9000);
      S(m.gt.gain,LV.turbine*.029*(.34+.66*ng)*(.6+.4*eff)*o.tone,.05,0,.4);
      S(m.elp.frequency,150+1500*Math.pow(eff,1.3)+800*bA,.035,60,7000);
      S(m.ge.gain,LV.exhaust*.14*(.3+.7*Math.pow(eff,1.15))*(1+.22*bA),.03,0,.7);
      S(m.eam.depth.gain,.3+.28*dmg+.12*heatS,.15,0,.7);S(m.eam.node.gain,.7-.28*dmg-.12*heatS,.15,0,1);
      // crackle opens with effort and gets ragged with damage
      const cr=.1+.32*Math.pow(eff,1.4)+.12*dmg;
      S(m.ecr.depth.gain,cr,.06,0,.6);S(m.ecr.node.gain,1-cr,.06,0,1);
      S(m.gh.gain,LV.exhaust*.022*Math.pow(eff,1.6)*(.6+.4*sat(n))*(1+.5*bA),.05,0,.2);S(m.hhp.frequency,2200+1400*sat(n),.1,800,8000);
      S(m.lv.gain,MECH*(.55+.45*Math.pow(clamp(eff,0,1),.9)),.05,0,2);
    }
```

Create `prototypes/Dustline/tools/engine2/step_r1_mech.py`:

```python
"""Engine 2.0, tone revision R1 (plan: Tone revision R1, after the failed CHECKPOINT 1): restore the 1.2.0 mechanical core,
turbine, exhaust and jet hiss on each engine, driven by the gear model, at 1.2.0's levels and effort. Each engine becomes one
channel from its misfire dip on (1.2.0's equal-power pan), the side level carries the ship's tone level and toneSum the tone
layer (S33, early), and the restored layers get their own mixer layers (growl, turbine, exhaust) as in 1.2.0."""
from common import Build

b = Build()
# A: header comment
b.rep("""/* DUSTLINE ship engine audio 2.0. A shared gear model (seven gears, a paced speed, boost lift) drives the ship's tone. BAKED plays
 * the ship's Keys synth patch rendered offline as eight phase-locked loops per engine. Airflow, strain, misfires, vent steam, rival
 * engines and timing cues are the 1.2.0 layers. No Math.random: variation comes from a private seeded generator, never from the
 * simulation's random state. The engine never decodes audio: the game decodes the ship's loops and hands them over with attach().
 */""", """/* DUSTLINE ship engine audio 2.0. A shared gear model (seven gears, a paced speed, boost lift) drives the ship's tone. BAKED plays
 * the ship's Keys synth patch rendered offline as eight phase-locked loops per engine. Tone revision R1 restores the 1.2.0
 * mechanical core, turbine, exhaust and jet hiss on each engine, driven by the same gear model. Airflow, strain, misfires, vent
 * steam, rival engines and timing cues are the 1.2.0 layers. No Math.random: variation comes from a private seeded generator, never
 * from the simulation's random state. The engine never decodes audio: the game decodes the ship's loops and hands them over with
 * attach().
 */""")
# B: the restored layers keep their 1.2.0 mixer layers
b.rep("  const LAYERS=['tone','strain','burn','air','rival','cue'];\n",
      "  const LAYERS=['tone','growl','turbine','exhaust','strain','burn','air','rival','cue'];\n")
# C: 1.2.0's per-engine values for the restored layers
b.rep("""  // The two engines differ in pan, pitch (the right one 0.35 % higher, from the ship's mix) and strain resonance.
  const SIDES=[{name:'L',side:-1,metal:1},{name:'R',side:1,metal:1.035}];
""", """  // The two engines differ in pan, pitch (the right one 0.35 % higher, from the ship's mix) and strain resonance. The restored 1.2.0
  // layers (tone revision R1) keep 1.2.0's firing rate offset, turbine partial ratios, intake and exhaust body resonances, buffer
  // and modulator offsets and turbine level per engine.
  const SIDES=[{name:'L',side:-1,metal:1,fire:1,rB:1.4985,rD:2.003,rE:2.71,intake:620,body:190,off:.31,lfoOff:.2,tone:1},
    {name:'R',side:1,metal:1.035,fire:1.016,rB:1.5032,rD:1.997,rE:2.74,intake:690,body:176,off:2.77,lfoOff:1.7,tone:.88}];
""")
# D: the restored layers, built once per engine after the sides
b.rep("    const sides=SIDES.map(buildSide);\n", "    const sides=SIDES.map(buildSide);\n" + b.piece("mech.js"))
# E: 1.2.0's throttle for the restored layers
b.rep("      if(recovering){thrL*=.5;thrR*=.5;}\n", """      if(recovering){thrL*=.5;thrR*=.5;}
      // 1.2.0's throttle, for its restored layers (tone revision R1): the raw fader, revved against the clamps on the grid
      let mThrL=sat(fin(f.thrL,1)),mThrR=sat(fin(f.thrR,mThrL));
      if(phase==='countdown'){mThrL=.1+.62*mThrL;mThrR=.1+.62*mThrR;}else if(phase==='finished')mThrL=mThrR=0;
      if(recovering){mThrL*=.5;mThrR*=.5;}
""")
# F: per engine and control step; the side level carries the ship's tone level, the tone layer sits on toneSum (S33)
b.rep("        S(s.vl.gain,LV.tone*TONE,.05,0,1.2);\n",
      "        mech(s,heatS,g.tone,i?mThrR:mThrL,dt);\n        S(s.vl.gain,TONE,.05,0,1.2);S(s.toneSum.gain,LV.tone,.05,0,1);\n")
# G: reset
b.rep("        s.load=0;s.kick=0;s.eff=0;s.hz=0;s.detuneUntil=0;s.rampUntil=0;s.rate=0;\n",
      "        s.load=0;s.kick=0;s.eff=0;s.hz=0;s.detuneUntil=0;s.rampUntil=0;s.rate=0;s.m.load=s.m.eff=0;\n")
# H: debug
b.rep("toneL:sides[0].hz,toneR:sides[1].hz,\n", "toneL:sides[0].hz,toneR:sides[1].hz,\n"
      "        mechEffL:sides[0].m.eff,mechEffR:sides[1].m.eff,nL:sides[0].m.n,nR:sides[1].m.n,turbineL:sides[0].m.ft,turbineR:sides[1].m.ft,"
      "fireL:sides[0].m.fire,fireR:sides[1].m.fire,\n")
e = b.engine()
assert e.count("function buildMech(s){") == 1 and e.count("mech(s,heatS,g.tone,i?mThrR:mThrL,dt);") == 1, "restored layers"
assert "LV.tone*TONE" not in e, "the tone layer sits on toneSum"
b.save()
print("step R1: the 1.2.0 core, turbine, exhaust and jet hiss are restored in the engine block")
```

```bash
python prototypes/Dustline/tools/engine2/step_r1_mech.py
node prototypes/Dustline/checks/audio-engine.test.cjs
```

Expected: `pushed <sha256> <bytes> bytes`, `step R1: the 1.2.0 core, turbine, exhaust and jet hiss are restored in the engine block`, then `17 engine audio checks passed.`

- [ ] **R1 Step 7: Encode and embed** (Task 7 Steps 4 and 5, without recreating any file).

```bash
python prototypes/Dustline/tools/ship_encode.py
python prototypes/Dustline/apply-sfx.py
```

Expected: `encoded 8 assets into …opus: <bytes> bytes of Opus`, then `pushed …` and `Embedded 29 sounds and 8 ship assets (…) in all three 0.13 copies; page <bytes> bytes.`

- [ ] **R1 Step 8: Browser check.** `python prototypes/Dustline/checks/audio-browser.py`. Expected: exit 0, `"ship": {"id": "kestrel", "decoded": 8, "total": 8}`, `"engineNodes": 351`, `"engineSources": 100`, a worst decoded seam below 0.1 (48 and 44.1 kHz), `"errors": []`, timing results all `PERFECT`.

- [ ] **R1 Step 9: Audition tooling.** Apply these Edits, then run the line-ending check on both files (expected `LF`). In `prototypes/Dustline/checks/audio_audition.py`:
  1. Usage line: `[--previous BUILD.html] [--out DIR] [--live]` → `[--previous BUILD.html] [--rejected BUILD] [--out DIR] [--live]`.
  2. After the docstring line `  ab-matched.wav           per section: previous, then new, both at -20 LUFS` insert:
     ```
       previous-propulsion.wav  0.12's propulsion (air, strain, rival and cue muted), at in-game level
       --rejected adds (tone revision R1): rejected-as-played.wav and rejected-matched.wav (a rejected build or engine .js, with its
       own ship bundle, driven by the same frames) and ab3-matched.wav (per section: previous, rejected, new, all at -20 LUFS)
     ```
  3. `import json` gets `import hashlib` before it.
  4. `TONE_ONLY = {"texture": 0, "strain": 0, …}` → preceded by the comment `# the ship's tone alone: textures, the restored 1.2.0 layers (tone revision R1) and every kept layer muted` and with `"growl": 0, "turbine": 0, "exhaust": 0` after `"texture": 0`.
  5. Before `def targets(prop, probes, secs, picks):` insert `turbine_follow(audio, probes)`: for each probe at least 0.2 s in and more than `SHIFT_GUARD` from a gear change (gear changes only when the probes carry `gear`; 0.12's do not), take a 100 ms Hann window at the probe, zero-padded 8 ×; the probe hits when the strongest peak within ±6 % of the debug `turbineL` lies within 3 % of it and at least 6 dB above the median of the ±25 % neighbourhood; return `{probes, share, medianProminenceDb}`. The signature becomes `def targets(prop, probes, secs, picks, alone=None, previous=None):` (`alone` is the tone-only render; not `tone`, which `targets` already uses for the zing rows).
  6. The `targets` result gains `**r1_targets(prop, probes, alone, previous)` after `"loudness"`, and a new `r1_targets` adds `pitchTone` (`pitch_follow` on the tone-only render; meets at share ≥ 0.9) and `turbine` (`turbine_follow` on the propulsion against `turbine_follow` on 0.12's propulsion with 0.12's own probes; meets when the share is at least 0.12's).
  7. `--rejected` argument (default empty) before `--live`.
  8. After `secs = …`: `frames_sha = sha256(json.dumps([frames, sc["events"]], sort_keys=True, separators=(",", ":")))`.
  9. The tone-only render moves out of `if args.live:` and always runs.
  10. After 0.12's render: `old_prop, oinfo = P.render(…, layers=PROPULSION, probe=PROBE)`; then, with `--rejected`, `lab.Lab(args.rejected)` renders the rejected engine with `lab.ship_bundle(args.rejected)` on the same frames and events, asserting at least 8 attached loops; `sha` hashes a file.
  11. `report["inputs"] = {framesSha256, engine, previous[, rejected]}` (file sha256), after `"scenario"`.
  12. `"targets": targets(prop, probes, secs, picks, tone_only, (old_prop, 0.12's probes))`.
  13. After the `ab-matched` line: write `previous-propulsion.wav`; with `--rejected`, `report["rejected"]` (overall plus sections), `matchGainDb.rejected`, `rejected-as-played.wav`, `rejected-matched.wav`, `spectrogram-rejected.png`, and `report["ab3"] = ab_file(out, "ab3-matched", marks, [("previous", …), ("rejected", …), ("new", …)])`; print the `inputs` line and, with `--rejected`, the rejected LUFS, true peak, stats and attached count.

  In `prototypes/Dustline/checks/ship_screen.py` `audition()`: after `pairs = [("previous > new", "previous-matched.wav", "engine-matched.wav")]` add the pair `("rejected > new", "rejected-matched.wav", "engine-matched.wav")` when the folder holds `rejected-matched.wav`, and say so in its docstring.

- [ ] **R1 Step 10: Render and screen the R1 audition.** The same simulation frames drive 0.12, the rejected phase 1 and R1 (one `scenario()` run; `metrics.json` `inputs.framesSha256` records them; the R1 prototype measured `cdba6e377e344591680cc81e9649c1117dd1851e8c4122a1e76cff6d958fa6ac`, 1458 frames).

```bash
python prototypes/Dustline/checks/audio_audition.py --rejected prototypes/Dustline/audio/audition-v0.13-p1-r1/rejected/engine.js --out prototypes/Dustline/audio/audition-v0.13-p1-r1
"$PY" -I prototypes/Dustline/checks/ship_screen.py audition prototypes/Dustline/audio/audition-v0.13-p1-r1
```

Expected: `attached 8 … errors []`, `nonFinite` 0, the `inputs` line with `framesSha256`, `rejected … attached 8`; then the earshot JSON line with `"previous > new"` and `"rejected > new"` cosines per section, and `earshot.json` in the folder. Inspect `spectrogram-engine.png`, `spectrogram-previous.png`, `spectrogram-rejected.png` and `spectrogram-ab3-matched.png`. Report every S42 and R1 target with its number, met or missed; expected misses from the prototype (reported, not tuned away): propulsion pitch (the firing pulse dominates the tracker), loudness (R1 keeps 1.2.0's levels; the prototype measured about −19.8 LUFS at cruise), and possibly the zing correlation.

  **Finding during R1 Step 10 (2026-11-06): the intake suction (R1-9).** The first R1 render put the propulsion within 0.1–0.7 dB of 0.12's in every band at cruise, but the whole engine stayed 4.7 dB above 0.12 at 4–10 kHz. Rendering each kept layer alone traced it to the air layer (cruise −31.41 LUFS against 0.12's −33.11; 1–4 kHz +3.8 dB, 4–10 kHz +5.4 dB): phase 1 had pointed the intake suction at the gear rpm (0.857 at cruise) and the gear load (1), where 1.2.0 read its spool (0.639) and its load (0.75), which raised it about 5.8 dB and moved its band from 1.99 to 2.36 kHz. A scratch build with R1-9 measured the air layer at −33.11 LUFS at cruise, and the engine's 1–4, 4–10 and 10–20 kHz bands within 0.5 dB of 0.12's.

- [ ] **R1 Step 10b: Put the intake suction on the restored spool (R1-9).**
  1. Add the R1-9 lines to the R1 test (Step 1's code above, after `'all four layers sound'`), run the line-ending check (`LF`), then `node prototypes/Dustline/checks/audio-engine.test.cjs`. Expected: `AssertionError [ERR_ASSERTION]: intake band-pass at 900 + 1700 n`.
  2. Create `prototypes/Dustline/tools/engine2/step_r1b_intake.py`:

```python
"""Engine 2.0, tone revision R1, decision R1-9 (plan: Tone revision R1, finding at R1 Step 10): the intake suction follows the
restored spool and 1.2.0's load again, as in 1.2.0. Phase 1 had pointed it at the gear rpm and the gear load when the spool went;
on the audition frames that made the air layer 1.7 dB louder at cruise than 0.12's, and 5.4 dB louder between 4 and 10 kHz."""
from common import Build

b = Build()
b.rep("""      // intake suction follows the engine's rpm
      S(il.gain,LV.air*.07*Math.pow(clamp(rpm,0,1.2),1.7)*(.4+.6*loadAvg)*(racing?1:.5),.08,0,.2);S(ibp.frequency,900+1700*sat(rpm),.1,200,5000);
""", """      // intake suction follows the spool and 1.2.0's load, as in 1.2.0 (tone revision R1: the restored spool, 0.746 x rpm)
      const nAvg=(sides[0].m.n+sides[1].m.n)*.5,mLoad=(sides[0].m.load+sides[1].m.load)*.5;
      S(il.gain,LV.air*.07*Math.pow(clamp(nAvg,0,1.2),1.7)*(.4+.6*mLoad)*(racing?1:.5),.08,0,.2);S(ibp.frequency,900+1700*sat(nAvg),.1,200,5000);
""")
b.save()
print("step R1b: the intake suction follows the restored spool")
```

  3. `python prototypes/Dustline/tools/engine2/step_r1b_intake.py`. Expected: `pushed <sha256> <bytes> bytes`, `step R1b: the intake suction follows the restored spool`. Then the R1 test: `17 engine audio checks passed.`
  4. Rerun Step 8 (expected unchanged: 351 nodes, 100 sources, 8/8, worst seam below 0.1) and Step 10 (render and screening), and report Step 10's numbers from this second render.

- [ ] **R1 Step 11: Regression, harnesses and hashes** (0.6). Expected: 162 checks (`audio-engine` 17, `ship-gears` 17, `ship-profile` 7 and the other 121), both ai-pace `PASS` lines, eight harness `PASS` lines, `ok <sha256>`.

- [ ] **R1 Step 12: README, progress record, memory.** Replace the `## Engine audio` section of `prototypes/Dustline/README.md` with the byte-preserving command of Task 9 Step 4, keeping the Task 9 text except: **Tone** describes the R1 patch (SUB and a plain saw OSC A at the fundamental through a driven 24 dB low-pass at 301 Hz, FX DRIVE TUBE with its tone filter at 2.5 kHz, −4.7 dB, OSC B off; the wheel opens the cutoff, the filter drive and the OSC A level); a new **Restored 1.2.0 layers** paragraph (R1-1 to R1-6 in plain words); **Twin engines** adds the mono equal-power pan; **Kept layers** says a misfire also dips the restored layers; **Assets** gives the measured nodes and sources (351 and 100); **Tools** adds `step_r1_mech.py` and the R1 audition command of Step 10; **Checks** gives 162 (`audio-engine` 17) and both audition folders. Run the line-ending check (expected `CRLF`). Update `plans/2026-11-05-dustline-ship-engine-progress.md` (this amendment, commands, counts, measurements, and the line `original last finished task: Task 9; revised CHECKPOINT 1: awaiting listening verdict`) and the memory note `dustline-ship-engine-spec.md`.

- [ ] **R1 Step 13: Report, then the revised CHECKPOINT 1.** Measurements only: the checks; the loops; Edge (decoded, nodes, sources, worst seam, update cost, page size); the audition numbers (LUFS, true peak, the S42 and R1 targets with their misses, the earshot timeline and the A/B cosines). What to listen to, in `prototypes/Dustline/audio/audition-v0.13-p1-r1/`: `ab3-matched.wav` (per section: 0.12, the rejected phase 1, R1, all at −20 LUFS), `ab-matched.wav` (0.12 then R1), `engine-as-played.wav`, `rejected-as-played.wav`, `previous-as-played.wav`, `propulsion.wav` and `previous-propulsion.wav`; the game in Edge. The user's patch folder is not touched; copy the revised patch there only if the user asks.

**REVISED CHECKPOINT 1. Stop here and wait for the user's listening verdict. Do not start Task 10 on your own.**

**Downstream amendments** (apply when the task runs; the original text above and below stays as the record):
- **Task 14 Step 3, `step5_textures.py` rep A**: its old text is now the R1 header (`step_r1_mech.py` rep A's new text), and its new text is:
  ```
  /* DUSTLINE ship engine audio 2.0. A shared gear model (seven gears, a paced speed, boost lift) drives the ship's tone. BAKED plays
   * the ship's Keys synth patch rendered offline as eight phase-locked loops per engine; ElevenLabs texture loops (F1, jet, drone)
   * follow the same rpm, and one-shots mark the shifts, boost ignition and release, and the launch. Tone revision R1 restores the
   * 1.2.0 mechanical core, turbine, exhaust and jet hiss on each engine, driven by the same gear model. Airflow, strain, misfires,
   * vent steam, rival engines and timing cues are the 1.2.0 layers. No Math.random: variation comes from a private seeded generator,
   * never from the simulation's random state. The engine never decodes audio: the game decodes the ship's assets and hands them over
   * with attach().
   */
  ```
- **Task 14 rep B** becomes `b.rep("const LAYERS=['tone','growl','turbine','exhaust','strain','burn','air','rival','cue'];", "const LAYERS=['tone','texture','growl','turbine','exhaust','strain','burn','air','rival','cue'];")`.
- **Task 14 rep L**: drop its fourth replacement (`S(s.vl.gain,LV.tone*TONE,…)` → `…toneSum…`); R1 already made it (R1-7).
- **Task 14 counts**: Step 4 expects `21 engine audio checks passed.`, Step 5 166 checks. Test 1's node range (280–340) holds the Task 14 count, 298 (306 with R1-10).
- **Task 14, mono engines (R1-5)**: from Task 14 the stereo texture loops and the shift pops enter each engine before its misfire dip, so they are downmixed to that engine's mono channel and panned ±0.34 with the rest. Whether phase 2 keeps that, or gives the textures their own stereo path, is part of the open phase-2 question (spec 10); ask before Task 14.
- **Answered at the revised CHECKPOINT 1 (2026-11-06; verdict "yes its good")**: all six textures and their weights as planned (Tasks 10–13 unchanged), and **R1-10, a texture stereo path** (spec 10): in Task 14 rep C each engine's `texSum` feeds its own `texShift` (`shot(T.gain(1),1)`) → `texZing` (`shot(T.bq('peaking',800,1.4,0),0)`) → `texVl` (`T.gain(0)`) → `texPan` (`T.pan(panBase)`) → `propMix`, instead of `shift`; `jetCut` and `popIn` stay as planned (pops are mono and keep the misfire dip). `shiftTone` schedules the cut, the blip and the zing blip on both pairs (`shift`/`zing` and `texShift`/`texZing`); each control step sets `texZing.frequency` like `zing.frequency`, `texVl.gain` to `TONE` like `vl.gain`, and `texPan.pan` like `pan.pan`. Eight more nodes: expected 306 before the loops arrive (test 1's 280–340 range holds) and 368 nodes with 106 sources from Task 15. The exact step 5 replacements and test lines are in "Task 14 as amended by R1 and R1-10" below (dry-run on scratch copies 2026-11-06).
- **PLAY patch**: the user asked to replace it; the R1 `kestrel-engine.lbpatch` was copied over `Documents\Launchpad Drummer\Patches\kestrel-engine.lbpatch` on 2026-11-06 (`cmp` equal).
- **Task 15 Step 6** expects `"engineNodes": 360`, `"engineSources": 106` (with R1-10: 368 and 106); Step 7 166 checks.
- **Task 16**: 166 checks; its README text keeps R1's Tone, Restored 1.2.0 layers and Twin engines paragraphs and the R1 node counts; its audition reports the two R1 targets.
- **Task 22**: its rep on `S(s.vl.gain,TONE,.05,0,1.2);S(s.toneSum.gain,LV.tone,.05,0,1);` still finds the line once (R1 put `mech(…)` on the line before it). Step 4 expects `23 engine audio checks passed.`, Step 5 168 checks.
- **Task 23 (LIVE SYNTH)**: the native voice plays the revised patch (unless the user's `DUSTLINE KESTREL.lbpatch` exists); frames, bend, wheel and gain are unchanged (S16). The restored layers play in both modes, outside LIVE's crossfade. `TONE_ONLY` now also mutes the restored layers, so the trim compares the baked tone alone with the LIVE tone alone, as designed. Expected 168 checks.
- **Task 24**: 168 checks; its README text follows the Task 16 rule above.
- **Task 21, Edit 2: changed by the user (2026-11-06, answer "1" at Task 24 Step 4).** `protoLoaded()` does not call `engineLink.stop(true)`; the line becomes the comment `// No engineLink stop here: Dustline asks for its voice at boot, before this load event fires; a reloaded` / `// game's own start replaces the voice, and leaving the game stops it (the effect above).` Why: Dustline runs `audioInit()` at boot, so its `engine-synth` start reaches the app during the frame's load and the iframe `load` event (about 7 ms later) stopped the voice being built; `begin()` then sent `game_engine_stop` and never replied `ready`, and the native smoke timed out on `Dustline.audio.live==="ready"`. Leaving GAMES → DUSTLINE still stops and forgets the voice (the `$effect`), a hidden window still stops it, and a reloaded Dustline sends its own start, which replaces the voice in Rust. Then rerun Task 21 Step 2 and Task 24 Steps 2–4.

**Task 14 as amended by R1 and R1-10.** Written and dry-run on 2026-11-06 on scratch copies of `index.html` and `audio-engine.test.cjs`, with the hash check and the push disabled; the game copies were not written. Run Task 14 with these changes; everything else in it stays as written.

- **Step 1**: the four Edits as written, then these two (line-ending check after each, expected `LF`).

Edit `prototypes/Dustline/checks/audio-engine.test.cjs` (5 of 6). Replace:

```js
  // tone revision R1 restores 1.2.0's propulsion layers, 69 nodes per engine: 319 nodes before the loops arrive (298 from Task 14)
```

with:

```js
  // tone revision R1 restores 1.2.0's propulsion layers, 69 nodes per engine: 319 nodes before the loops arrive (306 from Task 14,
  // with the textures' own stereo path, R1-10)
```

Edit `prototypes/Dustline/checks/audio-engine.test.cjs` (6 of 6), the end of test 17. Replace:

```js
  assert.deepEqual(ctx.problems, []); assert.deepEqual(c2.problems, []);
});
```

with:

```js
  // R1-10 (Task 14): each engine's texture loops have their own stereo path, with that engine's shift shaping, tone level and
  // pan, but not its misfire dip, from which the rest of the engine is one channel
  const c3 = makeContext(), e3 = E.create(c3, { output: E.createMaster(c3).input, ship: TEX_SHIP });
  attachAll(c3, e3); attachTextures(c3, e3);
  const b3 = bankOf(c3), tex = b3.slice(16), CUTG = Math.pow(10, MIX.cutDb / 20), BLIPG = Math.pow(10, MIX.blipDb / 20);
  drive(c3, e3, .2, frame({ phase: 'countdown', speed: 0 }));
  c3.trace = true;
  drive(c3, e3, 1.5, t => frame({ speed: 68 + 40 * t }));
  drive(c3, e3, 1.5, t => frame({ speed: Math.max(30, 108 - 60 * t), thrL: 0, thrR: 0, brake: 1 }));
  for (const i of [0, 1]) {
    const texSum = next(next(tex[i])), texShift = next(texSum), texZing = next(texShift), texVl = next(texZing), texPan = next(texVl);
    const sh = next(next(next(b3[8 * i]))), zg = next(sh), fault = next(next(zg)), vl = next(fault), pan = next(vl);
    assert.deepEqual([texShift.kind, texZing.kind, texZing.type, texVl.kind, texPan.kind], ['gain', 'biquad', 'peaking', 'gain', 'panner']);
    assert.equal(next(texPan), next(pan), 'both paths of an engine meet at propMix');
    assert(next(next(next(tex[6 + i]))) === texSum && next(next(next(tex[10 + i]))) === texSum, 'the jet loops (through their cut) and the drone (through its shimmer) join it');
    const reach = reachable(c3, texSum);
    assert(![sh, zg, fault, vl, pan].some(n => reach.has(n)), 'the textures skip the tone path and the misfire dip');
    assert([texSum, texShift, texZing, texVl, texPan].every(n => n.channelCountMode !== 'explicit'), 'stereo all the way');
    assert(sh.gain.log.some(e => Math.abs(e.v - CUTG) < 1e-9) && sh.gain.log.some(e => Math.abs(e.v - BLIPG) < 1e-9), 'an upshift and a downshift');
    assert(zg.gain.log.length > 0, 'a zing blip');
    assert.deepEqual(texShift.gain.log, sh.gain.log, 'the same cut and blip'); assert.deepEqual(texZing.gain.log, zg.gain.log, 'the same zing blip');
    assert.deepEqual([texZing.frequency.value, texVl.gain.value, texPan.pan.value], [zg.frequency.value, vl.gain.value, pan.pan.value]);
    near(texVl.gain.value, Math.pow(10, MIX.toneDb / 20), 1e-12, 'the ship tone level');
  }
  assert.deepEqual(ctx.problems, []); assert.deepEqual(c2.problems, []); assert.deepEqual(c3.problems, []);
});
```

- **Step 2**: expected as written (`AssertionError [ERR_ASSERTION]: The expression evaluated to a falsy value:`, from the fuzz test's `shots <= 6`).
- **Step 3**: create `step5_textures.py` from the code block of Step 3 with five changes: rep A and rep B as in the amendments above; rep C replaced by the first block below; rep L without its fourth replacement; reps R, S and T (the second block) inserted before the closing `assert not any(…)` line. Expected output as written.

```python
# C: each engine gets a texture input with its own stereo path (R1-10; the jet through its own flicker cut) and a pop input after the shift cut
b.between("    /* ---- player engines: one tone voice per side (spec 3.4) ----", "    const sides=SIDES.map(buildSide);\n", """    /* ---- player engines: one tone voice per side (spec 3.4) ----
     * toneSum (the baked loops) -> shift (ignition cut / throttle blip) -> zing (a peaking blip at 8 f) -> sum. Shift pops join at
     * sum (popIn), after the cut. The strain resonators ring from a pulse train at the fundamental, which reaches the resonators
     * only. sum -> fault (misfire dips) -> level -> pan. The texture loops have their own stereo path (tone revision R1, R1-10):
     * texSum (the jet ones through jetCut, the flame-out flicker) -> texShift (the same cut and blip) -> texZing (the same zing
     * blip) -> texVl (the ship's tone level) -> texPan (the same pan) -> propMix. They keep their width and skip the misfire
     * dips, from which the rest of each engine is one channel. */
    const TONE=dbGain(MIX.toneDb),TEX=dbGain(MIX.textureDb),SHOT=dbGain(MIX.shotDb),CUT=dbGain(MIX.cutDb),BLIP=dbGain(MIX.blipDb);
    function buildSide(o){
      const sum=T.gain(1),toneSum=T.gain(1),texSum=T.gain(1),jetCut=shot(T.gain(1),1),popIn=T.gain(1),shift=shot(T.gain(1),1),zing=shot(T.bq('peaking',800,1.4,0),0);
      T.chain(toneSum,shift,zing,sum);popIn.connect(sum);
      const px=T.osc('sine',60,B.pulse),sb1=T.bq('bandpass',1150*o.metal,14),sb2=T.bq('bandpass',1740*o.metal,18),sx=T.bq('bandpass',620*o.metal,9),gst=T.gain(0);
      px.connect(sb1);px.connect(sb2);px.connect(sx);sb1.connect(gst);sb2.connect(gst);sx.connect(gst);gst.connect(sum);
      const fault=shot(T.gain(1),1),vl=T.gain(0),panBase=o.side*MIX.pan,pan=T.pan(panBase);
      T.chain(sum,fault,vl,pan,propMix);
      const texShift=shot(T.gain(1),1),texZing=shot(T.bq('peaking',800,1.4,0),0),texVl=T.gain(0),texPan=T.pan(panBase);
      T.chain(jetCut,texSum,texShift,texZing,texVl,texPan,propMix);
      return{o,pitch:o.side>0?MIX.rightPitch:1,panBase,load:0,eff:0,kick:0,hz:0,detuneUntil:0,rampUntil:0,rate:0,bank:null,tex:[],toneSum,texSum,jetCut,popIn,shift,zing,texShift,texZing,texVl,texPan,px,gst,fault,vl,pan};
    }
    const sides=SIDES.map(buildSide);
""")
```

```python
# R (R1-10): a shift shapes the tone path and the textures' own stereo path alike
b.rep("""      for(const s of sides){
        const p=s.shift.gain;p.cancelScheduledValues(t);
        if(up){p.setTargetAtTime(CUT,t,.004);p.setTargetAtTime(1,t+.04,.008);}
        else{p.setTargetAtTime(BLIP,t,.006);p.setTargetAtTime(1,t+.08,.02);}
        if(up){const z=s.zing.gain;z.cancelScheduledValues(t);z.setTargetAtTime(MIX.zingBlipDb,t+.04,.01);z.setTargetAtTime(0,t+.09,.05);}
""", """      for(const s of sides){
        // the tone path, then the textures' own stereo path (R1-10): the same cut or blip and the same zing blip
        for(let k=0;k<2;k++){
          const p=(k?s.texShift:s.shift).gain;p.cancelScheduledValues(t);
          if(up){p.setTargetAtTime(CUT,t,.004);p.setTargetAtTime(1,t+.04,.008);}
          else{p.setTargetAtTime(BLIP,t,.006);p.setTargetAtTime(1,t+.08,.02);}
          if(up){const z=(k?s.texZing:s.zing).gain;z.cancelScheduledValues(t);z.setTargetAtTime(MIX.zingBlipDb,t+.04,.01);z.setTargetAtTime(0,t+.09,.05);}
        }
""")
# S (R1-10): per control step the texture path follows the zing and takes the ship's tone level
b.rep("        S(s.zing.frequency,8*s.hz,.03,100,4000);\n",
    "        S(s.zing.frequency,8*s.hz,.03,100,4000);S(s.texZing.frequency,8*s.hz,.03,100,4000);S(s.texVl.gain,TONE,.05,0,1.2);\n")
# T (R1-10): and the same pan
b.rep("      for(const s of sides)S(s.pan.pan,s.panBase+.1*asym,.08,-.6,.6);\n",
    "      for(const s of sides){S(s.pan.pan,s.panBase+.1*asym,.08,-.6,.6);S(s.texPan.pan,s.panBase+.1*asym,.08,-.6,.6);}\n")
```

- **Step 4**: `21 engine audio checks passed.` **Step 5**: 166 checks and both ai-pace `PASS` lines.
- **Dry run (2026-11-06)**: RED as expected; after step 5, `21 engine audio checks passed.`; 306 nodes and 76 sources before the loops arrive. With R1's amendments but without R1-10 (rep C as first written), the amended tests fail in test 17 with `the restored layers enter the shift through one level`: that rep C feeds `texSum` into the shift as a second input. R1-10 removes that conflict too.

**Task 12 pick by ear (S27-E, the user's word, 2026-11-06).** The screening picked 5 of the 14 sounds (jet-cruise a, jet-burn a, up-1 c, down-1 b, down-2 b; the three one-shot picks are earshot "no match"). No take of f1-mid, f1-high, f1-off, drone, up-2, up-3, ignite, release or launch passed. Offered new takes (a go), a rule change, or picking by ear, the user chose to pick by ear. So:
- **S27-E**: the user names one take per sound and that pick wins; a rule pick stands only where the user keeps it. The screening rows in `picks.json` and the "Takes" table stay as information; `picks.json` records the rule as S27-E, marks each pick `by: "ear"` and keeps the rule's picks under `rulePicks`.
- **Loops** use the picked take's screened 3 s window (the `start`/`end` in its row: the steadiest window `find` returned), mastered as S28.
- **One-shots** use the picked take whole, less silence at either end, so nothing heard is cut: 10 ms RMS frames of the mono take; the trim starts 10 ms before the first frame within 40 dB of the loudest frame (not before 0) and ends 50 ms after the last such frame (not after the take's end); then S28's 3 ms fade-in, 20 ms fade-out and −1 dBFS peak.
- **The drone pick must be pitched** (`ship_assets.py` stops otherwise): only takes a (395 Hz) and b (71 Hz) measure pitched. With S29's refRpm (an octave under the fundamental), take a's rate would stay clamped at 0.5 and not follow the rpm; take b's follows it.
- **The user's picks (2026-11-06)**: f1-mid c, f1-high c, f1-off a, jet-cruise b, jet-burn a, drone c; up-shift crackles up-1 a, up-1 c, up-2 c and up-3 a; down-shift blips down-1 a, down-1 c and down-2 a; ignite b, release a, launch a. So the up-shifts get **four** variants (`up-1` … `up-4`; `up-4` is up-1 c) and the down-shifts **three** (`down-1` … `down-3`; `down-3` is down-1 c); the variants of a shift share one prompt. With the user's "ok" to the default offered, the drone is take c, which measures unpitched: its refRpm is 0.857 (it plays at its own speed at cruise, 7th at full thrust, and follows the rpm like the jet loops) and its baseHz 0.
- **Step 3 of Task 12 (the picks)**: add this `ear` mode to `checks/ship_screen.py` (LF) before its `if __name__` block, and the `ear` branch after the `takes` branch:

```python
# S27-E (plan "Task 12 pick by ear"): the user's ear picks, written from the screening rows of takes(); no earshot model runs
EAR_RULE = ("S27-E (the user's word, 2026-11-06): the user picks one take per slot by ear and that pick wins. Loops use the picked "
            "take's screened 3 s window; one-shots the whole take less silence. The screening rows stay as information.")


def whole_trim(x):
    """S27-E trim of a mono one-shot take: from 10 ms before the first 10 ms frame within 40 dB of the loudest frame to 50 ms after
    the last such frame, inside the take."""
    n = SR // 100
    env = np.array([rms(x[i:i + n]) for i in range(0, len(x) - n + 1, n)])
    on = np.nonzero(env >= env.max() * 10 ** (-40 / 20))[0]
    return round(max(0, (on[0] - 1) * n) / SR, 3), round(min(len(x), (on[-1] + 6) * n) / SR, 3)


def ear(args):
    """picks.json from the user's ear picks: `<slot>=<take>` picks that take of the slot's own sound, `<slot>=<sound>-<take>` a take
    of another sound of the same kind (the shift variants share one prompt each)."""
    man = json.loads((RAW / "prompts.json").read_text(encoding="utf-8"))
    old = json.loads((SHIP / "picks.json").read_text(encoding="utf-8"))
    rows, picks, lines = old["takes"], {}, ["| slot | take | kind | window or trim s | z | baseHz |", "|" + "---|" * 6]
    for arg in args:
        slot, _, src = arg.partition("=")
        src = src if "-" in src else f"{slot}-{src}"
        sid = src.rsplit("-", 1)[0]
        assert slot not in picks and src in rows, f"bad pick {arg}"
        kind = man["sounds"][sid]["kind"]
        assert man["sounds"].get(slot, {"kind": kind})["kind"] == kind, f"{slot} and {src} differ in kind"
        r = rows[src]
        start, end = (r["start"], r["end"]) if kind == "loop" else whole_trim(pcm(RAW / "wav" / f"{src}.wav").mean(axis=1))
        picks[slot] = {"kind": kind, "take": src.rsplit("-", 1)[1], "source": src, "wav": f"raw/wav/{src}.wav", "start": start,
                       "end": end, "z": r["z"], "baseHz": r["baseHz"] if r.get("pitched") else 0, "by": "ear"}
        lines.append(f"| {slot} | {src} | {kind} | {start}-{end} | {r['z']} | {picks[slot]['baseHz']} |")
    (SHIP / "picks.json").write_text(json.dumps({"rule": EAR_RULE, "picks": picks, "rulePicks": old.get("rulePicks", old["picks"]),
                                                 "takes": rows}, indent=1), encoding="utf-8")
    log_section("Picks by ear", EAR_RULE + "\n\n" + "\n".join(lines))
    print(json.dumps({"picked": len(picks), "by": "ear"}))
```

```python
    elif mode == "takes":
        takes()
    elif mode == "ear":
        ear(sys.argv[2:])
```

Run it (no earshot model loads):

```bash
python prototypes/Dustline/checks/ship_screen.py ear f1-mid=c f1-high=c f1-off=a jet-cruise=b jet-burn=a drone=c up-1=a up-2=c up-3=a up-4=up-1-c down-1=a down-2=a down-3=down-1-c ignite=b release=a launch=a
```

Expected: `{"picked": 16, "by": "ear"}`; `picks.json` and a "Picks by ear" section of `LOG.md` written.
- **Task 13** creates `ship_assets.py` from its code block with these replacements (old → new, each found once):

```python
# old
REDLINE_HZ = 124  # the profile's gears.redlineHz
# new
REDLINE_HZ = 124  # the profile's gears.redlineHz
DRONE_UNPITCHED_RPM = .857  # S27-E: an unpitched drone (the user's ear pick) plays at its own speed at cruise rpm (7th, full thrust)
```

```python
# old
SHOTS = {"upshift": ["up-1", "up-2", "up-3"], "downshift": ["down-1", "down-2"],
# new
SHOTS = {"upshift": ["up-1", "up-2", "up-3", "up-4"], "downshift": ["down-1", "down-2", "down-3"],
```

```python
# old
"refRpm": ref if ref else round(2 * base / REDLINE_HZ, 4),
# new
"refRpm": ref if ref else round(2 * base / REDLINE_HZ, 4) if base > 0 else DRONE_UNPITCHED_RPM,
```

```python
# old
print(f"{tid}: take {p['take']}, window
# new
print(f"{tid}: take {p.get('source', p['take'])}, window
```

```python
# old
print(f"{sid}: take {p['take']}, {len(x) / SR:.3f} s")
# new
print(f"{sid}: take {p.get('source', p['take'])}, {len(x) / SR:.3f} s")
```

```python
# old
    if any(t["layer"] == "drone" and t["baseHz"] <= 0 for t in textures):
        raise SystemExit("the drone pick has no baseHz: it must be pitched")
# new
(removed)
```

Expected: six texture lines, **ten** one-shot lines, then `wrote 6 loops, 10 one-shots and …profile-assets.json`.
- **Task 15**: the test's one-shot ids are `upshift: ['up-1', 'up-2', 'up-3', 'up-4'], downshift: ['down-1', 'down-2', 'down-3']` and the bundle holds **24** assets (`ids.length` 24); its drone line becomes `assert.ok(drone.baseHz === 0 && drone.refRpm === .857, 'the unpitched drone (S27-E) plays at its own speed at cruise rpm')`. `profile_assets.py` prints `profile: 6 textures and 10 one-shots`; Step 4 expects `encoded 24 assets` and `Embedded 29 sounds and 24 ship assets`; Step 6 expects `"decoded": 24, "total": 24`, 368 nodes, 106 sources, and `texturePitch` for the three F1 loops only (the drone has no baseHz). The F1 pitch reads jump an octave on part of the frames (f1-off a: 47 %), so if Step 6's 3 % pitch check fails on an octave, stop and ask.
- **Task 16**: `attached 24`; the README says four up-shift and three down-shift variants, 24 assets (10 one-shots), a drone at its own speed at cruise rpm, and that the takes were picked by ear (S27-E) after the rule picked 5 of 14.
- **Task 24** (added 2026-11-06 while running it): the smoke test's comment and wait read the ship's **24** assets (`Dustline.audio.ship.total===24 && Dustline.audio.ship.decoded===24`); its README text follows the Task 16 rule of the R1 amendment and this one.

---

## Phase 2 — Textures and one-shots (full BAKED)

Phase 2 adds the ElevenLabs layers: six texture loops that follow the engine's rpm (F1 V10 mid, high and off-load; jet cruise and afterburner; a spaceship drone) and eight one-shots (three upshift crackles, two downshift burbles, boost ignition, boost release, launch). They replace the 1.2.0 afterburner voices. It starts only after the user's verdict at CHECKPOINT 1, and the generation itself (Task 11) only on the user's go.

### Task 10: Generation script

**Purpose.** `generate-ship-sfx.py` lists and makes the 42 ElevenLabs generations (spec 3.3, 4.3; S26): 14 sounds × 3 takes, steady-RPM loop prompts (no gear changes, no Doppler, no music), raw MP3 into `audio/ships/kestrel/raw/`, and `raw/prompts.json` for the screening. The dry run reads no key and calls nothing; `--go` makes only the missing takes.

**Files:**
- Create: `prototypes/Dustline/generate-ship-sfx.py`
- Generated by the dry run: `prototypes/Dustline/audio/ships/kestrel/raw/prompts.json`

- [ ] **Step 1: Create the script.** Create `prototypes/Dustline/generate-ship-sfx.py`:

```python
"""Generate the Kestrel's ElevenLabs takes (ship engine spec 3.3 and 4.3): 14 sounds x 3 takes = 42 generations.

    python prototypes/Dustline/generate-ship-sfx.py          dry run: lists the 42 generations and which files already exist
    python prototypes/Dustline/generate-ship-sfx.py --go     generates only the missing takes, with the user's ElevenLabs credits

Takes: a = phrasing 1, b = phrasing 2, c = phrasing 1 with the prompt influence lowered by 0.3. Every loop prompt asks for a
steady RPM: no gear changes, no Doppler, no music. Raw MP3 (mp3_44100_128) goes to audio/ships/kestrel/raw/<sound>-<take>.mp3;
raw/prompts.json records the prompts, the earshot descriptions and the files for checks/ship_screen.py takes. Existing files are
never overwritten. The API key comes from LAUNCHBAND/.env (or ELEVENLABS_API_KEY) and is never printed or written anywhere else.
"""
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
ENV_PATH = HERE.parents[1] / ".env"
RAW = HERE / "audio" / "ships" / "kestrel" / "raw"
URL = "https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_128"
INFLUENCE, LOWER, LOOP_S = 0.6, 0.3, 6.0
STEADY = ", constant pitch, no gear changes, no Doppler, no music"


def loop(description, p1, p2, pitched=False):
    return {"kind": "loop", "seconds": LOOP_S, "description": description, "pitched": pitched, "phrasings": [p1 + STEADY, p2 + STEADY]}


def shot(seconds, description, p1, p2):
    return {"kind": "shot", "seconds": seconds, "description": description, "pitched": False,
            "phrasings": [p1 + ", one-shot, no music", p2 + ", one-shot, no music"]}


UP = shot(1.0, "a car exhaust popping and crackling during a gear change",
          "Single racing car upshift: a sharp exhaust crackle and pop as the gear changes, short and dry",
          "Race engine gearshift cut: one quick bang and crackle from the exhaust, short")
DOWN = shot(1.0, "a racing car engine blipping the throttle on a downshift",
            "Single racing car downshift throttle blip: a quick rising engine rev with exhaust burble, short",
            "Race car downshift: one fast throttle blip and pop from the exhaust, short")
SOUNDS = {
    "f1-mid": loop("a Formula 1 V10 racing car engine revving at a steady mid RPM",
                   "Onboard recording of a Formula 1 V10 racing engine held at a steady mid RPM, raw high-revving scream with exhaust rasp",
                   "Steady-state V10 race car engine at constant medium revs recorded from the cockpit, mechanical whine and exhaust bark"),
    "f1-high": loop("a Formula 1 V10 racing car engine screaming at a steady high RPM",
                    "Onboard recording of a Formula 1 V10 racing engine held at a steady very high RPM near the limiter, piercing scream",
                    "Steady-state V10 race car engine at constant maximum revs recorded from the cockpit, shrill continuous howl"),
    "f1-off": loop("a racing car engine burbling and crackling off the throttle",
                   "Onboard recording of a Formula 1 V10 engine off the throttle at a steady RPM, overrun burble with exhaust pops and crackles",
                   "V10 race car engine coasting on overrun at constant revs from the cockpit, lumpy burbling exhaust with light crackle"),
    "jet-cruise": loop("a jet turbine engine roaring steadily",
                       "Jet turbine engine running at a steady cruise power, continuous deep roar with smooth compressor whine",
                       "Steady turbofan engine at constant thrust heard close up, even broadband roar and turbine whine"),
    "jet-burn": loop("a jet engine afterburner roaring",
                     "Jet engine with the afterburner lit at a steady full power, continuous thundering roar with crackling flame",
                     "Military jet afterburner burning steadily at maximum thrust heard close up, sustained rumbling blast"),
    "drone": loop("a spaceship engine humming",
                  "Science-fiction spaceship ion drive drone, steady pitched electric hum with a soft harmonic shimmer",
                  "Futuristic spacecraft engine hum at constant power, smooth tonal electric drone with gently pulsing harmonics",
                  pitched=True),
    "up-1": UP, "up-2": UP, "up-3": UP,
    "down-1": DOWN, "down-2": DOWN,
    "ignite": shot(2.0, "a jet afterburner igniting with a whoomp",
                   "Jet afterburner igniting: a deep whoomp of flame followed by a building roar",
                   "Rocket booster ignition burst: a punchy thump of combustion swelling into thrust"),
    "release": shot(2.0, "a jet engine afterburner cutting out with a hiss",
                    "Jet afterburner cutting out: the roar collapses with a hissing exhale and fading crackles",
                    "Booster shutdown: a release of pressure, a whoosh and a fading turbine spin-down"),
    "launch": shot(2.5, "a racing car accelerating hard from a standstill",
                   "Race car launch from a standing start: the engine revs hard and it surges away",
                   "Hover racer launch: a surge of engine power and a blast of thrust pulling away"),
}
TAKES = {"a": (0, INFLUENCE), "b": (1, INFLUENCE), "c": (0, INFLUENCE - LOWER)}


def generations():
    return [(sid, take, s["phrasings"][k], s["seconds"], round(infl, 2), f"{sid}-{take}.mp3")
            for sid, s in SOUNDS.items() for take, (k, infl) in TAKES.items()]


def load_api_key():
    if os.environ.get("ELEVENLABS_API_KEY"):
        return os.environ["ELEVENLABS_API_KEY"].strip()
    if ENV_PATH.exists():
        for line in ENV_PATH.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line.startswith("ELEVENLABS_API_KEY="):
                return line.split("=", 1)[1].strip()
    raise SystemExit("ELEVENLABS_API_KEY not found in LAUNCHBAND/.env or the environment")


def generate(key, prompt, seconds, influence, path):
    body = json.dumps({"text": prompt, "duration_seconds": seconds, "prompt_influence": influence}).encode("utf-8")
    req = urllib.request.Request(URL, data=body, method="POST", headers={"xi-api-key": key, "Content-Type": "application/json"})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                data = resp.read()
            part = path.with_suffix(".part")
            part.write_bytes(data)
            part.replace(path)
            return
        except urllib.error.HTTPError as e:
            print(f"  HTTP {e.code} for {path.name} (attempt {attempt + 1}): {e.read().decode('utf-8', errors='ignore')[:300]}")
        except (urllib.error.URLError, TimeoutError) as e:
            print(f"  {type(e).__name__} for {path.name} (attempt {attempt + 1}): {e}")
        if attempt == 2:
            raise SystemExit(f"{path.name} failed three times; the takes made so far are kept")
        time.sleep(2.0 * (attempt + 1))


def main():
    go = "--go" in sys.argv[1:]
    rows = generations()
    assert len(rows) == 42, len(rows)
    RAW.mkdir(parents=True, exist_ok=True)
    manifest = {"format": "mp3_44100_128", "sounds": {sid: {"kind": s["kind"], "seconds": s["seconds"], "description": s["description"],
                                                          "pitched": s["pitched"], "takes": {}} for sid, s in SOUNDS.items()}}
    for sid, take, prompt, _, infl, name in rows:
        manifest["sounds"][sid]["takes"][take] = {"prompt": prompt, "influence": infl, "file": name}
    (RAW / "prompts.json").write_text(json.dumps(manifest, indent=1), encoding="utf-8")
    missing = [r for r in rows if not (RAW / r[5]).exists()]
    for sid, take, prompt, seconds, infl, name in rows:
        print(f"{'have' if (RAW / name).exists() else 'make'}  {name:15s} {seconds:3.1f} s  influence {infl:.1f}  {prompt[:72]}")
    print(f"{len(rows)} generations in the plan: {len(rows) - len(missing)} already exist, {len(missing)} to make")
    if not go:
        print("Dry run: nothing was generated. Run with --go to generate the missing takes.")
        return
    key = load_api_key()
    for i, (sid, take, prompt, seconds, infl, name) in enumerate(missing, 1):
        print(f"[{i}/{len(missing)}] {name}")
        generate(key, prompt, seconds, infl, RAW / name)
    print(f"Generated {len(missing)} takes into {RAW}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Dry run.**

```bash
python prototypes/Dustline/generate-ship-sfx.py
```

Expected: 42 lines of the form `make  f1-mid-a.mp3     6.0 s  influence 0.6  Onboard recording of a Formula 1 V10 racing engine held at a steady mid…` (loops 6.0 s; one-shots 1.0, 2.0 or 2.5 s; take c at influence 0.3), then `42 generations in the plan: 0 already exist, 42 to make` and `Dry run: nothing was generated. Run with --go to generate the missing takes.`

### Task 11: Generation run (only on the user's go)

**Purpose.** Make the 42 takes with the user's ElevenLabs credits (S26).

**Files:**
- Generated: `prototypes/Dustline/audio/ships/kestrel/raw/<sound>-<a|b|c>.mp3` (42)

- [ ] **Step 1: Ask for the go.** Tell the user: "Task 11 makes 42 ElevenLabs generations (14 sounds × 3 takes; loops 6 s, one-shots 1–2.5 s, mp3_44100_128) with your credits. Go?" Wait for an explicit yes. Do not run Step 2 without it.

- [ ] **Step 2: Generate.**

```bash
python prototypes/Dustline/generate-ship-sfx.py --go
```

Expected: the 42-line list, then `[1/42] f1-mid-a.mp3` … `[42/42] launch-c.mp3` and `Generated 42 takes into …\prototypes\Dustline\audio\ships\kestrel\raw`. If it stops with `<file> failed three times; the takes made so far are kept`, report the HTTP lines it printed (they never contain the key) and ask the user before running `--go` again; a rerun makes only the missing takes.

- [ ] **Step 3: Confirm the set.**

```bash
python prototypes/Dustline/generate-ship-sfx.py
```

Expected: `42 generations in the plan: 42 already exist, 0 to make`.

### Task 12: Screen the takes

**Purpose.** earshot screens every take and picks one per sound by rule S27 (spec 4.4; S27, S52). Loops: the steadiest 3 s window (`find`, kind texture), steadiness (pitched: drift ≤ 2 %; unpitched: level range ≤ 3 dB and centroid drift ≤ 10 % over 0.5 s frames), seam level (first and last 100 ms within 1.5 dB), the drone must be pitched. One-shots: a clean trim (no cut, head and tail at −30 dB or lower). `match` against realistic confusions (F1: motorcycle, chainsaw, vacuum cleaner; jet: strong wind, hair dryer; drone: refrigerator hum, mains hum; one-shots: gunshot, firecracker, motorcycle revving, car horn, explosion, gas burner, gust of wind, sigh, rocket launch, car crash). Writes `audio/ships/kestrel/picks.json` and the "Takes" section of `LOG.md`.

**Files:**
- Modify: `prototypes/Dustline/checks/ship_screen.py` (LF, from Task 5)
- Generated: `prototypes/Dustline/audio/ships/kestrel/raw/wav/*.wav` (42), `audio/ships/kestrel/picks.json`

- [ ] **Step 1: Add the takes mode.** Apply this Edit, then run the line-ending check (expected `LF`).

Edit `prototypes/Dustline/checks/ship_screen.py`. Replace:

```python
if __name__ == "__main__":
    mode = sys.argv[1] if len(sys.argv) > 1 else ""
    if mode == "loops":
        loops()
```

with:

```python
# Realistic confusions for earshot match (spec 4.4), by sound family (the id before its first '-')
LOOP_ALTS = {"f1": ["motorcycle engine", "chainsaw", "vacuum cleaner"], "jet": ["strong wind", "hair dryer"],
             "drone": ["refrigerator hum", "mains electrical hum"]}
SHOT_ALTS = {"up": ["gunshot", "firecracker"], "down": ["motorcycle revving", "car horn"],
             "ignite": ["explosion", "gas burner lighting"], "release": ["gust of wind", "sigh"], "launch": ["rocket launch", "car crash"]}
WIN = 3.0  # a texture loop: 3.0 s, 144000 samples at 48 kHz (plan S13)


def spread(v):
    """The larger distance from the median to the 5th or 95th percentile, as a fraction of the median."""
    med = float(np.median(v))
    p5, p95 = np.percentile(v, [5, 95])
    return float(max(p95 - med, med - p5) / med)


def centroid(x, n=2048):
    """Spectral centroid over 100 Hz-16 kHz of the averaged (Welch) power spectrum: sub-bass wander does not move it."""
    spec = np.mean([np.abs(np.fft.rfft(x[i:i + n] * np.hanning(n))) ** 2 for i in range(0, len(x) - n + 1, n // 2)], axis=0)
    f = np.fft.rfftfreq(n, 1 / SR)
    m = (f >= 100) & (f < 16000)
    return float((f[m] * spec[m]).sum() / (spec[m].sum() + 1e-24))


def screen_loop(api, wav, s, alts):
    """A loop take: fit, the steadiest 3 s window, steadiness (pitched: drift <= 2 %; unpitched: level range <= 3 dB and
    centroid drift <= 10 %, over 0.5 s frames) and the seam level (first and last 100 ms within 1.5 dB)."""
    m = api.match(str(wav), s["description"], alts=alts)
    x = pcm(wav)
    f = api.find(str(wav), s["description"], win=WIN, hop=.25, top=3, start=.25, end=len(x) / SR - .1, kind="texture")
    a = f["segments"][0]["start"]
    body = x[int(a * SR):int(a * SR) + int(WIN * SR)].mean(axis=1)
    p = pitch.summary(body, SR)
    pitched = p["periodic"] >= .6
    frames = [body[i:i + SR // 2] for i in range(0, len(body) - SR // 2 + 1, SR // 2)]
    levels = [db(rms(v)) for v in frames]
    level_range = float(np.percentile(levels, 95) - np.percentile(levels, 5))
    cdrift = spread([centroid(v) for v in frames])
    steady = p["drift"] <= .02 if pitched else level_range <= 3 and cdrift <= .1
    seam = abs(db(rms(body[:SR // 10])) - db(rms(body[-SR // 10:])))
    real = api.realness(str(wav), start=a, end=a + WIN)
    return {"z": m["z"], "verdict": m["verdict"], "pVsAlts": m.get("p_vs_alts"), "start": round(a, 3), "end": round(a + WIN, 3),
            "pitched": pitched, "baseHz": round(p["f0"], 2) if pitched else 0, "periodic": round(p["periodic"], 2),
            "drift": None if p["drift"] is None else round(p["drift"], 4), "levelRangeDb": round(level_range, 2),
            "centroidDrift": round(cdrift, 4), "seamDb": round(seam, 2), "realness": real.get("p_real"),
            "pass": bool(steady and seam <= 1.5 and (pitched or not s.get("pitched")))}


def screen_shot(api, wav, s, alts):
    """A one-shot take: fit and a clean trim (no head or tail cut, head and tail at -30 dB or lower)."""
    m = api.match(str(wav), s["description"], alts=alts)
    segs = api.find(str(wav), s["description"], top=3, kind="event")["segments"]
    clean = [g for g in segs if not g["cut"] and g["head_db"] <= -30 and g["tail_db"] <= -30]
    g = (clean or segs or [None])[0]
    real = api.realness(str(wav), start=g["start"], end=g["end"]) if g else {}
    return {"z": m["z"], "verdict": m["verdict"], "pVsAlts": m.get("p_vs_alts"), "start": round(g["start"], 3) if g else None,
            "end": round(g["end"], 3) if g else None, "cut": g["cut"] if g else "none found", "headDb": g["head_db"] if g else None,
            "tailDb": g["tail_db"] if g else None, "realness": real.get("p_real"), "pass": bool(clean)}


def takes():
    """Screen every take in raw/prompts.json (generate-ship-sfx.py) and pick one per sound by rule S27."""
    api = earshot()
    man = json.loads((RAW / "prompts.json").read_text(encoding="utf-8"))
    (RAW / "wav").mkdir(exist_ok=True)
    rows, picks, missing = {}, {}, []
    for sid, s in man["sounds"].items():
        alts = (LOOP_ALTS if s["kind"] == "loop" else SHOT_ALTS)[sid.split("-")[0]]
        wavs = {}
        for take, t in sorted(s["takes"].items()):
            mp3, wav = RAW / t["file"], RAW / "wav" / (Path(t["file"]).stem + ".wav")
            if not mp3.exists():
                continue
            if not wav.exists() or wav.stat().st_mtime < mp3.stat().st_mtime:
                subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(mp3), "-ar", str(SR), "-ac", "2", "-c:a", "pcm_f32le", str(wav)],
                               check=True)
            wavs[take] = wav
        if not wavs:
            missing.append(sid)
            continue
        ranked = api.rank([str(w) for w in wavs.values()], s["description"])
        order = {Path(r["path"]).stem.rsplit("-", 1)[1]: i + 1 for i, r in enumerate(ranked["results"])}
        for take, wav in wavs.items():
            rows[f"{sid}-{take}"] = dict((screen_loop if s["kind"] == "loop" else screen_shot)(api, wav, s, alts), rank=order.get(take))
        ok = [(take, rows[f"{sid}-{take}"]) for take in wavs if rows[f"{sid}-{take}"]["pass"]]
        if not ok:
            missing.append(sid)
            continue
        take, r = max(ok, key=lambda kv: kv[1]["z"])  # max keeps the first of equal z: take order a, b, c
        picks[sid] = {"kind": s["kind"], "take": take, "wav": f"raw/wav/{wavs[take].name}", "start": r["start"], "end": r["end"],
                      "z": r["z"], "baseHz": r.get("baseHz", 0)}
    rule = ("S27: among the takes that pass steadiness and a clean seam (loops) or a clean trim (one-shots), the highest earshot "
            "match z wins; ties go to take order a, b, c. realness and rank are information only.")
    (SHIP / "picks.json").write_text(json.dumps({"rule": rule, "picks": picks, "takes": rows}, indent=1), encoding="utf-8")
    lines = [rule, "", "| take | z | verdict | rank | window s | pitched f0 Hz / drift | level range dB / centroid drift | seam dB | realness | pass |",
             "|" + "---|" * 10]
    for k, r in rows.items():
        if "seamDb" in r:
            lines.append(f"| {k} | {r['z']} | {r['verdict']} | {r['rank']} | {r['start']}-{r['end']} | "
                         f"{r['baseHz'] if r['pitched'] else 'unpitched'} / {r['drift']} | {r['levelRangeDb']} / {r['centroidDrift']} | "
                         f"{r['seamDb']} | {r['realness']} | {r['pass']} |")
    lines += ["", "| take | z | verdict | rank | trim s | cut | head / tail dB | realness | pass |", "|" + "---|" * 9]
    for k, r in rows.items():
        if "seamDb" not in r:
            lines.append(f"| {k} | {r['z']} | {r['verdict']} | {r['rank']} | {r['start']}-{r['end']} | {r['cut'] or '-'} | "
                         f"{r['headDb']} / {r['tailDb']} | {r['realness']} | {r['pass']} |")
    lines += ["", "Picks: " + ", ".join(f"{k} {v['take']} (z {v['z']})" for k, v in picks.items()),
              "No pick: " + (", ".join(missing) or "none")]
    log_section("Takes", "\n".join(lines))
    print(json.dumps({"picked": len(picks), "missing": missing}))
    if missing:
        raise SystemExit("no pick for: " + ", ".join(missing) + ". New takes need the user's go (generate-ship-sfx.py).")


if __name__ == "__main__":
    mode = sys.argv[1] if len(sys.argv) > 1 else ""
    if mode == "loops":
        loops()
    elif mode == "takes":
        takes()
```

- [ ] **Step 2: Screen** (earshot venv; several minutes for 42 takes).

```bash
PY="/d/MY DOCS/MY TOOLS/earshot/.venv/Scripts/python.exe"
"$PY" -I prototypes/Dustline/checks/ship_screen.py takes
```

Expected: `{"picked": 14, "missing": []}`. If it ends with `no pick for: … New takes need the user's go (generate-ship-sfx.py).`, stop: show the user the "Takes" table of `LOG.md` for those sounds. New takes, or any change to the rule, need the user's word.

- [ ] **Step 3: Note the picks.** From `audio/ships/kestrel/picks.json`, list each sound's take, window or trim and z for the report.

### Task 13: Master the picks

**Purpose.** `tools/ship_assets.py` turns the picks into the Kestrel's masters (spec 3.3, 4.5; S13, S14, S28, S29, S30, S40): six texture loops (2400 + 144000 + 2400 samples, stereo, 48 kHz, seam crossfade, −18 dBFS RMS, peak ≤ −1 dBFS) and eight one-shots (mono, trimmed, faded, peak −1 dBFS), plus `profile-assets.json` with the profile's `textures` and `oneShots`.

**Files:**
- Create: `prototypes/Dustline/tools/ship_assets.py`
- Generated: `prototypes/Dustline/audio/ships/kestrel/loops/{f1-mid,f1-high,f1-off,jet-cruise,jet-burn,drone}.wav`, `audio/ships/kestrel/shots/{up-1,up-2,up-3,down-1,down-2,ignite,release,launch}.wav`, `audio/ships/kestrel/profile-assets.json`

- [ ] **Step 1: Create the mastering tool.** Create `prototypes/Dustline/tools/ship_assets.py`:

```python
"""Master the picked ElevenLabs takes into the Kestrel's texture loops and one-shots (ship engine spec 3.3 and 4.5; plan S13,
S28, S29, S30, S40).

    python prototypes/Dustline/tools/ship_assets.py      reads audio/ships/kestrel/picks.json (checks/ship_screen.py takes)

Loops: the picked 3.0 s window with 0.05 s on each side, 2400 + 144000 + 2400 samples, stereo, 48 kHz. The last 0.15 s of the
loop is crossfaded into the audio before the loop start, with gains that keep the power constant for the measured correlation
of the two (as the bake does), so the loop end continues as the loop start; the 0.05 s after the loop end is a copy of the loop
start. Level: -18 dBFS RMS over the loop, lowered if the peak would pass -1 dBFS.
One-shots: mono, the picked trim, 3 ms fade-in, 20 ms fade-out, peak -1 dBFS.
Writes 32-bit float WAV masters to loops/ and shots/ (ffmpeg), and profile-assets.json: the profile's textures and oneShots
(tools/engine2/profile_assets.py puts them in the build). Then run tools/ship_encode.py and apply-sfx.py.
"""
import json
import subprocess
from pathlib import Path

import numpy as np

SHIP = Path(__file__).resolve().parents[1] / "audio" / "ships" / "kestrel"
SR, MARGIN, LOOP, SEAM = 48000, 2400, 144000, 7200
REDLINE_HZ = 124  # the profile's gears.redlineHz
# id: layer, role, refRpm (spec 3.3; the drone's refRpm puts it an octave under the fundamental: 2 * baseHz / redlineHz)
TEXTURES = {"f1-mid": ("f1", "mid", .65), "f1-high": ("f1", "high", .95), "f1-off": ("f1", "off", .65),
            "jet-cruise": ("jet", "cruise", .857), "jet-burn": ("jet", "burn", 1.25), "drone": ("drone", "drone", None)}
WEIGHTS = {"f1": .4, "jet": .35, "drone": .25}
SHOTS = {"upshift": ["up-1", "up-2", "up-3"], "downshift": ["down-1", "down-2"], "ignite": "ignite", "release": "release", "launch": "launch"}


def pcm(path):
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", str(path), "-f", "f32le", "-ac", "2", "-ar", str(SR), "-"],
                         capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype="<f4").reshape(-1, 2).astype(np.float64)


def write(path, x):
    """x: (n, channels) float. 32-bit float WAV at 48 kHz."""
    x = np.ascontiguousarray(x, dtype="<f4")
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "f32le", "-ar", str(SR), "-ac", str(x.shape[1]), "-i", "-",
                    "-c:a", "pcm_f32le", str(path)], input=x.tobytes(), check=True)


def make_loop(x, a):
    """The loop master from source x (n, 2) with the loop starting at sample a; returns it and the seam correlation."""
    b = a + LOOP
    assert a >= SEAM and b <= len(x), "the picked window needs 0.15 s before it inside the take"
    end, pre = x[b - SEAM:b], x[a - SEAM:a]
    rho = float(np.sum(end * pre) / (np.sqrt(np.sum(end ** 2) * np.sum(pre ** 2)) + 1e-24))
    th = (np.arange(SEAM) + .5) / SEAM * np.pi / 2
    we, wp = np.cos(th), np.sin(th)
    g = 1 / np.sqrt(we ** 2 + wp ** 2 + 2 * max(rho, -.5) * we * wp)
    body = x[a:b].copy()
    body[-SEAM:] = (g * we)[:, None] * end + (g * wp)[:, None] * pre
    return np.concatenate([x[a - MARGIN:a], body, x[a:a + MARGIN]]), rho


def main():
    picks = json.loads((SHIP / "picks.json").read_text(encoding="utf-8"))["picks"]
    need = list(TEXTURES) + [i for v in SHOTS.values() for i in (v if isinstance(v, list) else [v])]
    assert set(need) <= set(picks), f"picks.json has no pick for {sorted(set(need) - set(picks))}"
    (SHIP / "loops").mkdir(exist_ok=True)
    (SHIP / "shots").mkdir(exist_ok=True)
    textures = []
    for tid, (layer, role, ref) in TEXTURES.items():
        p = picks[tid]
        x = pcm(SHIP / p["wav"])
        out, rho = make_loop(x, int(round(p["start"] * SR)))
        loop = out[MARGIN:MARGIN + LOOP]
        gain = min(10 ** (-18 / 20) / (np.sqrt(np.mean(loop ** 2)) + 1e-24), 10 ** (-1 / 20) / (np.max(np.abs(out)) + 1e-24))
        write(SHIP / "loops" / f"{tid}.wav", out * gain)
        base = round(float(p["baseHz"]), 2)
        textures.append({"id": tid, "layer": layer, "role": role, "refRpm": ref if ref else round(2 * base / REDLINE_HZ, 4),
                         "baseHz": base, "loopStart": MARGIN, "loopEnd": MARGIN + LOOP, "weight": WEIGHTS[layer]})
        print(f"{tid}: take {p['take']}, window {p['start']}-{p['end']} s, seam correlation {rho:.3f}, gain {20 * np.log10(gain):+.1f} dB")
    for sid in need[len(TEXTURES):]:
        p = picks[sid]
        x = pcm(SHIP / p["wav"]).mean(axis=1)[int(round(p["start"] * SR)):int(round(p["end"] * SR))]
        fi, fo = int(.003 * SR), int(.02 * SR)
        x[:fi] *= np.linspace(0, 1, fi)
        x[-fo:] *= np.linspace(1, 0, fo)
        write(SHIP / "shots" / f"{sid}.wav", (x * 10 ** (-1 / 20) / (np.max(np.abs(x)) + 1e-24))[:, None])
        print(f"{sid}: take {p['take']}, {len(x) / SR:.3f} s")
    if any(t["layer"] == "drone" and t["baseHz"] <= 0 for t in textures):
        raise SystemExit("the drone pick has no baseHz: it must be pitched")
    (SHIP / "profile-assets.json").write_text(json.dumps({"textures": textures, "oneShots": SHOTS}, indent=1), encoding="utf-8")
    print(f"wrote {len(textures)} loops, {len(need) - len(textures)} one-shots and {SHIP / 'profile-assets.json'}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Master.**

```bash
python prototypes/Dustline/tools/ship_assets.py
```

Expected: six lines `<texture>: take <a|b|c>, window <start>-<end> s, seam correlation <ρ>, gain <±x.x> dB`, eight lines `<one-shot>: take <a|b|c>, <seconds> s`, then `wrote 6 loops, 8 one-shots and …\prototypes\Dustline\audio\ships\kestrel\profile-assets.json`.

### Task 14: Textures and one-shots in the engine

**Purpose.** Full BAKED (spec 3.3, 3.5; S10, S29, S30, S32, S33). Textures follow the engine's rpm on both engines (`rpm / refRpm`, clamped 0.5–2) with the S30 weights and equal-power crossfades inside a layer; the jet textures go through `jetCut` (running dry in boost flickers them); one-shots mark the shifts, boost ignition and release, the pad and the launch, at most six at once; the 1.2.0 afterburner and ignition voices go; a `'texture'` layer joins the layer list. The four new tests (and an extended fuzz test) build a test profile with synthetic textures and one-shots, since the real ones reach the profile only in Task 15.

**Files:**
- Modify: `prototypes/Dustline/checks/audio-engine.test.cjs` (LF)
- Create: `prototypes/Dustline/tools/engine2/step5_textures.py`
- Modify (by `step5_textures.py`): the three game copies

- [ ] **Step 1: Write the failing tests.** Apply these Edits, then run the line-ending check (expected `LF`).

Edit `prototypes/Dustline/checks/audio-engine.test.cjs` (1 of 4). Replace:

```js
const smooth = (a, b, v) => { const t = Math.min(1, Math.max(0, (v - a) / (b - a))); return t * t * (3 - 2 * t); };
```

with:

```js
const smooth = (a, b, v) => { const t = Math.min(1, Math.max(0, (v - a) / (b - a))); return t * t * (3 - 2 * t); };
// Phase 2: a test ship, the Kestrel with three F1 loops, two jet loops, a drone and seven one-shots, ids as the bundle names them.
// Every texture loop is a 2.1 s stereo buffer with its loop between 0.05 s and 2.05 s; one-shots are 0.5 s.
const TEX_SHIP = Object.assign({}, SHIP, {
  textures: [
    { id: 'f1-mid', layer: 'f1', role: 'mid', refRpm: .65, baseHz: 90, loopStart: 2400, loopEnd: 98400, weight: .4 },
    { id: 'f1-high', layer: 'f1', role: 'high', refRpm: .95, baseHz: 130, loopStart: 2400, loopEnd: 98400, weight: .4 },
    { id: 'f1-off', layer: 'f1', role: 'off', refRpm: .65, baseHz: 88, loopStart: 2400, loopEnd: 98400, weight: .4 },
    { id: 'jet-cruise', layer: 'jet', role: 'cruise', refRpm: .8, baseHz: 0, loopStart: 2400, loopEnd: 98400, weight: .35 },
    { id: 'jet-burn', layer: 'jet', role: 'burn', refRpm: .9, baseHz: 0, loopStart: 2400, loopEnd: 98400, weight: .35 },
    { id: 'drone', layer: 'drone', role: 'drone', refRpm: .9, baseHz: 55.8, loopStart: 2400, loopEnd: 98400, weight: .25 }],
  oneShots: { upshift: ['up-1', 'up-2', 'up-3'], downshift: ['down-1', 'down-2'], ignite: 'ignite', release: 'release', launch: 'launch' } });
const SHOT_IDS = ['up-1', 'up-2', 'up-3', 'down-1', 'down-2', 'ignite', 'release', 'launch'];
const attachTextures = (ctx, eng) => TEX_SHIP.textures.map(x => eng.attach(x.id, ctx.createBuffer(2, 100800, 48000)));
const attachShots = (ctx, eng) => Object.fromEntries(SHOT_IDS.map(id => { const b = ctx.createBuffer(2, 24000, 48000); eng.attach(id, b); return [id, b]; }));
const loops = ctx => ctx.nodes.filter(n => n.kind === 'buffer' && n.loopEnd > 0);
```

Edit `prototypes/Dustline/checks/audio-engine.test.cjs` (2 of 4). Replace:

```js
test('fuzzed frames and events keep every parameter write finite and inside its range', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input });
  attachAll(ctx, eng);
```

with:

```js
test('fuzzed frames and events keep every parameter write finite and inside its range', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input, ship: TEX_SHIP });
  attachAll(ctx, eng); attachTextures(ctx, eng); attachShots(ctx, eng);
```

Edit `prototypes/Dustline/checks/audio-engine.test.cjs` (3 of 4). Replace:

```js
  assert(eng.stats().voices <= 3);
```

with:

```js
  assert(eng.stats().voices <= 3);
  assert(eng.stats().shots <= 6);
```

Edit `prototypes/Dustline/checks/audio-engine.test.cjs` (4 of 4). Replace:

```js
console.log(`${passed} engine audio checks passed.`);
```

with:

```js
test('textures play on both engines from their loop start, pitched by rpm / refRpm inside 0.5-2; unknown and repeated ids are refused', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input, ship: TEX_SHIP });
  attachAll(ctx, eng);
  assert.deepEqual(attachTextures(ctx, eng), [true, true, true, true, true, true]);
  assert.equal(eng.attach('f1-mid', ctx.createBuffer(2, 100800, 48000)), false, 'once only');
  assert.equal(eng.attach('f1-low', ctx.createBuffer(2, 100800, 48000)), false, 'unknown id');
  const tex = loops(ctx).slice(16);
  assert.equal(tex.length, 12, 'six textures on each engine');
  assert.equal(eng.stats().textures, 6);
  for (const n of tex) { near(n.loopStart, .05, 1e-12); near(n.loopEnd, 2.05, 1e-12); assert.equal(n.offset, n.loopStart); assert(n.started); }
  for (const [speed, boost] of [[120, 0], [20, 0], [330, 1]]) {
    drive(ctx, eng, 1.5, frame({ speed, boost: boost > 0, boostIntensity: boost }));
    const d = eng.debug();
    TEX_SHIP.textures.forEach((x, i) => {
      for (const [side, hz] of [[0, d.toneL], [1, d.toneR]])
        near(tex[2 * i + side].playbackRate.value, Math.min(2, Math.max(.5, hz / GP.redlineHz / x.refRpm)), .005, `${x.id} at ${speed} m/s`);
    });
  }
  assert.deepEqual(ctx.problems, []);
});

test('texture weights: F1 mid/high by rpm and on/off by load (equal power, 0.6 in gears 1-2), the jet by the afterburner, the drone by load', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input, ship: TEX_SHIP });
  attachTextures(ctx, eng);
  const left = loops(ctx).filter((_, i) => i % 2 === 0).map(n => next(n).gain);
  const TEX = Math.pow(10, MIX.textureDb / 20), q = Math.PI / 2;
  for (const [speed, thr, boost] of [[120, .75, 0], [120, .1, 0], [30, .75, 0], [160, .75, 1], [100, .4, .5]]) {
    drive(ctx, eng, 2.5, frame({ speed, thrL: thr, thrR: thr, boost: boost > 0, boostIntensity: boost }));
    const d = eng.debug(), r = d.toneL / GP.redlineHz, on = smooth(.1, .4, d.loadL), x = Math.min(1, Math.max(0, (r - .65) / .3));
    const f1 = d.gear > 0 && d.gear < 3 ? .6 : 1, b = d.burn;
    const want = [f1 * Math.cos(x * q) * Math.sqrt(on), f1 * Math.sin(x * q) * Math.sqrt(on), f1 * Math.sqrt(1 - on),
      (.4 + .6 * b) * Math.cos(b * q), (.4 + .6 * b) * Math.sin(b * q), .6 + .4 * d.loadL];
    TEX_SHIP.textures.forEach((t, i) => near(left[i].value, TEX * t.weight * want[i], .003, `${t.id} at ${speed} m/s, fader ${thr}`));
    if (speed === 30) assert.equal(d.gear, 2, 'F1 at 0.6 in 2nd');
  }
  assert.deepEqual(ctx.problems, []);
});

test('one-shots: each shift pops a different variant on each engine, seeded; ignition, release and launch play once in the centre', () => {
  // the variants heard through a launch from GO (three upshifts) and a hard brake (downshifts), in the order they start
  const variants = seed => {
    const c = makeContext(), e = E.create(c, { output: E.createMaster(c).input, ship: TEX_SHIP, seed });
    const name = new Map(Object.entries(attachShots(c, e)).map(([k, v]) => [v, k]));
    drive(c, e, .5, frame({ phase: 'countdown', speed: 0 }));
    drive(c, e, 1.2, t => frame({ speed: (245 + 280 * t) / 3.6 }));
    drive(c, e, 1.5, t => frame({ speed: Math.max(20, 600 - 400 * t) / 3.6, thrL: 0, thrR: 0, brake: 1 }));
    return c.nodes.filter(n => n.kind === 'buffer' && name.has(n.buffer)).map(n => ({ id: name.get(n.buffer), when: n.when, rate: n.playbackRate.value, to: n.out.size ? next(next(n)) : null }));
  };
  const v = variants(7);
  assert.equal(JSON.stringify(v.map(p => p.id)), JSON.stringify(variants(7).map(p => p.id)), 'equal seeds pick the same variants');
  assert(v.length >= 14 && v.length % 2 === 0, 'two pops per shift: ' + v.length);
  for (let i = 0; i < v.length; i += 2) {
    const [l, r] = [v[i], v[i + 1]], kind = i < 6 ? 'up-' : 'down-';
    assert(l.id.startsWith(kind) && r.id.startsWith(kind) && l.id !== r.id, `${l.id} / ${r.id}`);
    assert.equal(l.when, r.when, 'both engines pop at the shift');
    if (l.to && r.to) assert(l.to !== r.to, 'each through its own engine');
    near(l.rate, 1, 1e-12); near(r.rate, MIX.rightPitch, 1e-12);
  }
  assert(v.at(-1).to && v.at(-2).to, 'the latest pops are still playing (older ones were stopped by the cap of six)');
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input, ship: TEX_SHIP });
  const bufs = attachShots(ctx, eng), SHOT = Math.pow(10, MIX.shotDb / 20);
  const played = () => ctx.nodes.filter(n => n.kind === 'buffer' && SHOT_IDS.some(id => n.buffer === bufs[id]));
  for (const name of ['ignite', 'release', 'launch']) eng.event(name, { strength: 1 }, ctx.currentTime);
  const [ig, rel, la] = played();
  assert.deepEqual([ig.buffer, rel.buffer, la.buffer], [bufs.ignite, bufs.release, bufs.launch]);
  assert(next(next(ig)) === next(next(rel)) && next(next(la)) === next(next(ig)), 'one centre input');
  for (const s of [ig, rel, la]) near(next(s).gain.value, SHOT, 1e-12);
  ctx.trace = true;
  ctx.currentTime = 3;
  eng.event('pad', {}, ctx.currentTime);
  const pad = played().at(-1), pg = next(pad).gain;
  assert.equal(pad.buffer, bufs.ignite, 'the pad plays the ignite, shorter');
  assert(pg.log.some(e => e.kind === 'target' && e.v === 0 && Math.abs(e.t - 3.25) < 1e-9) && pad.stopped, 'fades from 0.25 s and stops');
  for (let i = 0; i < 10; i++) eng.event('release', {}, ctx.currentTime);
  assert.equal(eng.stats().shots, 6, 'at most six at once, the oldest stopped first');
  const live = played().filter(s => !s.stopped);
  assert.equal(live.length, 6);
  live[0].onended();
  assert.equal(eng.stats().shots, 5);
  assert.equal(live[0].out.size, 0, 'an ended one-shot is disconnected');
  eng.reset(ctx.currentTime);
  assert.equal(eng.stats().shots, 0);
  assert(played().every(s => s.out.size === 0), 'reset stops and disconnects every one-shot');
  assert.deepEqual(ctx.problems, []);
});

test('the 1.2.0 afterburner is gone: running dry in boost flickers the jet textures only; without assets nothing plays or is built', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input, ship: TEX_SHIP });
  attachTextures(ctx, eng);
  const cuts = loops(ctx).map(n => next(next(n)));
  assert(cuts[6] === cuts[8] && cuts[7] === cuts[9] && cuts[6] !== cuts[7], 'the two jet loops of an engine share its flicker cut');
  ctx.trace = true;
  drive(ctx, eng, 2, frame({ boost: true, boostIntensity: 1, charge: .03 }));
  for (const cut of [cuts[6], cuts[7]]) assert(cut.gain.log.some(e => e.kind === 'ramp' && e.v < .8), 'flicker');
  assert(cuts.filter((_, i) => i < 6 || i > 9).every(n => n.gain.log.length === 0), 'F1 and drone untouched');
  const c2 = makeContext(), e2 = E.create(c2, { output: E.createMaster(c2).input }), n0 = c2.nodes.length;
  for (const name of ['ignite', 'release', 'launch', 'pad']) e2.event(name, { strength: 1 }, 0);
  drive(c2, e2, 2, t => frame({ speed: (245 + 280 * t) / 3.6 }));
  assert.equal(c2.nodes.length, n0, 'without attached assets nothing plays and nothing is built');
  assert.equal(e2.stats().shots, 0);
  assert.deepEqual(ctx.problems, []);
});

console.log(`${passed} engine audio checks passed.`);
```

- [ ] **Step 2: Run them to see them fail.**

```bash
node prototypes/Dustline/checks/audio-engine.test.cjs
```

Expected: `AssertionError [ERR_ASSERTION]: The expression evaluated to a falsy value:`.

- [ ] **Step 3: Create and run step 5.** Create `prototypes/Dustline/tools/engine2/step5_textures.py`:

```python
"""Engine 2.0, step 5 (plan Task 14): full BAKED. ElevenLabs texture loops (F1, jet, drone) follow the engine's rpm, one-shots
mark the shifts, boost ignition and release, the pad and the launch, and the 1.2.0 afterburner and ignition voices go."""
from common import Build

b = Build()
# A: header comment
b.rep("""/* DUSTLINE ship engine audio 2.0. A shared gear model (seven gears, a paced speed, boost lift) drives the ship's tone. BAKED plays
 * the ship's Keys synth patch rendered offline as eight phase-locked loops per engine. Airflow, strain, misfires, vent steam, rival
 * engines and timing cues are the 1.2.0 layers. No Math.random: variation comes from a private seeded generator, never from the
 * simulation's random state. The engine never decodes audio: the game decodes the ship's loops and hands them over with attach().
 */""", """/* DUSTLINE ship engine audio 2.0. A shared gear model (seven gears, a paced speed, boost lift) drives the ship's tone. BAKED plays
 * the ship's Keys synth patch rendered offline as eight phase-locked loops per engine; ElevenLabs texture loops (F1, jet, drone)
 * follow the same rpm, and one-shots mark the shifts, boost ignition and release, and the launch. Airflow, strain, misfires, vent
 * steam, rival engines and timing cues are the 1.2.0 layers. No Math.random: variation comes from a private seeded generator, never
 * from the simulation's random state. The engine never decodes audio: the game decodes the ship's assets and hands them over with
 * attach().
 */""")
# B: a mixer layer for the textures
b.rep("const LAYERS=['tone','strain','burn','air','rival','cue'];", "const LAYERS=['tone','texture','strain','burn','air','rival','cue'];")
# C: each engine gets a texture input (the jet through its own flicker cut) and a pop input after the shift cut
b.between("    /* ---- player engines: one tone voice per side (spec 3.4) ----", "    const sides=SIDES.map(buildSide);\n", """    /* ---- player engines: one tone voice per side (spec 3.4) ----
     * toneSum (the baked loops) and texSum (the texture loops; the jet ones through jetCut, the flame-out flicker) -> shift
     * (ignition cut / throttle blip) -> zing (a peaking blip at 8 f) -> sum. Shift pops join at sum (popIn), after the cut. The
     * strain resonators ring from a pulse train at the fundamental, which reaches the resonators only. sum -> fault (misfire dips)
     * -> level -> pan. */
    const TONE=dbGain(MIX.toneDb),TEX=dbGain(MIX.textureDb),SHOT=dbGain(MIX.shotDb),CUT=dbGain(MIX.cutDb),BLIP=dbGain(MIX.blipDb);
    function buildSide(o){
      const sum=T.gain(1),toneSum=T.gain(1),texSum=T.gain(1),jetCut=shot(T.gain(1),1),popIn=T.gain(1),shift=shot(T.gain(1),1),zing=shot(T.bq('peaking',800,1.4,0),0);
      T.chain(toneSum,shift,zing,sum);T.chain(jetCut,texSum,shift);popIn.connect(sum);
      const px=T.osc('sine',60,B.pulse),sb1=T.bq('bandpass',1150*o.metal,14),sb2=T.bq('bandpass',1740*o.metal,18),sx=T.bq('bandpass',620*o.metal,9),gst=T.gain(0);
      px.connect(sb1);px.connect(sb2);px.connect(sx);sb1.connect(gst);sb2.connect(gst);sx.connect(gst);gst.connect(sum);
      const fault=shot(T.gain(1),1),vl=T.gain(0),panBase=o.side*MIX.pan,pan=T.pan(panBase);
      T.chain(sum,fault,vl,pan,propMix);
      return{o,pitch:o.side>0?MIX.rightPitch:1,panBase,load:0,eff:0,kick:0,hz:0,detuneUntil:0,rampUntil:0,rate:0,bank:null,tex:[],toneSum,texSum,jetCut,popIn,shift,zing,px,gst,fault,vl,pan};
    }
    const sides=SIDES.map(buildSide);
""")
# D: the 1.2.0 afterburner and ignition voices go (spec 3.3); the misfire crackle stays; one-shots in the centre join propMix
b.between("    /* ---- afterburner: pressure, rasp and a persistent ignition voice scheduled per event ---- */",
        "T.chain(iOsc,iOscG,iSum);T.chain(iNs,iBp,iNg,iSum);T.chain(iCs,iHp,iCg,iSum);iSum.connect(engineMix);\n",
        """    /* ---- one-shots in the centre (boost ignition, release, launch) and the misfire crackle ---- */
    const shotIn=T.gain(1);shotIn.connect(propMix);
    const iCs=T.buf(B.white,1,5.5),iHp=T.bq('highpass',2300,.7),iCg=shot(T.gain(0));T.chain(iCs,iHp,iCg,engineMix);
""")
# E: the 1.2.0 release thump goes
b.rep("    const rS=T.osc('sine',80),rSg=shot(T.gain(0));T.chain(rS,rSg,engineMix);\n", "")
# F: ignition and release become one-shots; shift pops
b.between("    function ignite(t,k){\n", "      for(const dt of[.07+rnd()*.03,.17+rnd()*.05]){iCg.gain.setValueAtTime(0,t+dt);iCg.gain.linearRampToValueAtTime(.09*k,t+dt+.004);iCg.gain.setTargetAtTime(0,t+dt+.01,.016);}\n    }\n",
        """    // One-shots (spec 3.3) play a decoded buffer once. Web Audio cannot restart a source, so each play makes one source and one
    // gain, kept out of the fixed graph's counts, disconnected when it ends and stopped on reset and dispose. At most six play at
    // once: the oldest is stopped first. `len` cuts a one-shot short with a 40 ms fade (the boost pad's shorter ignite).
    const shots=new Set(),SHOT_MAX=6;
    function playShot(id,dest,t,gain,rate,len){
      const buf=id&&assets.get(id);
      if(!buf||!(gain>0))return false;
      if(shots.size>=SHOT_MAX)shots.values().next().value.stop();
      const src=ctx.createBufferSource(),g=ctx.createGain();
      src.buffer=buf;src.playbackRate.value=clamp(fin(rate,1),.5,2);g.gain.value=clamp(gain,0,2);
      src.connect(g);g.connect(dest);
      const s={stop(){try{src.stop();}catch(_){}s.end();},end(){if(!shots.delete(s))return;try{src.disconnect();g.disconnect();}catch(_){}}};
      src.onended=s.end;shots.add(s);
      src.start(t);
      if(len){g.gain.setValueAtTime(clamp(gain,0,2),t+len);g.gain.setTargetAtTime(0,t+len,.04);src.stop(t+len+.3);}
      return true;
    }
    function ignite(t,k,len){
      k=clamp(fin(k,1),.2,1.2);
      playShot(SHIP.oneShots.ignite,shotIn,t,LV.burn*SHOT*k,1,len);
      duckBus(t,.22*k,.12);
    }
    function release(t,k){
      playShot(SHIP.oneShots.release,shotIn,t,LV.burn*SHOT*clamp(fin(k,1),.2,1.2),1);
    }
    // Shift pops (spec 3.1): each engine plays its own variant, the first picked by the seeded generator, at its own pitch and pan.
    function pops(ids,t){
      if(!ids.length)return;
      const a=Math.floor(rnd()*ids.length);
      for(let i=0;i<2;i++)playShot(ids[(a+i)%ids.length],sides[i].popIn,t,LV.texture*SHOT*Math.SQRT1_2,sides[i].pitch);
    }
""")
# H: the flame-out flicker acts on the jet textures
b.rep("""    function sputter(t,depth,len){
      const g=burnCut.gain;
      g.cancelScheduledValues(t);g.setValueAtTime(1,t);g.linearRampToValueAtTime(1-depth,t+.012);
      g.setValueAtTime(1-depth,t+.012+len);g.linearRampToValueAtTime(1,t+.012+len+.04);
    }
""", """    function sputter(t,depth,len){
      for(const s of sides){
        const g=s.jetCut.gain;
        g.cancelScheduledValues(t);g.setValueAtTime(1,t);g.linearRampToValueAtTime(1-depth,t+.012);
        g.setValueAtTime(1-depth,t+.012+len);g.linearRampToValueAtTime(1,t+.012+len+.04);
      }
    }
""")
# I: attach takes textures and one-shots; a texture plays as soon as it arrives
b.rep("""    // The game decodes the ship's assets and hands each one over with attach(). The banks are built once, when every loop is
    // there: all sixteen sources start on the same frame at their loop start, so loops rendered in phase stay in phase.
    const assets=new Map();
    function attach(id,buffer){
      if(disposed||typeof id!=='string'||!buffer||!(buffer.duration>0)||assets.has(id))return false;
      const e=SHIP.baked.find(b=>b.id===id);
      if(!e||buffer.duration*LOOP_RATE<e.loopEnd-1)return false;
      assets.set(id,buffer);
      if(!sides[0].bank&&SHIP.baked.every(b=>assets.has(b.id)))buildBanks();
      return true;
    }
""", """    // The game decodes the ship's assets and hands each one over with attach(). The banks are built once, when every loop is
    // there: all sixteen sources start on the same frame at their loop start, so loops rendered in phase stay in phase. A texture
    // plays as soon as it arrives; one-shots wait for their event.
    const assets=new Map();
    const SHOT_IDS=new Set([...SHIP.oneShots.upshift,...SHIP.oneShots.downshift,SHIP.oneShots.ignite,SHIP.oneShots.release,SHIP.oneShots.launch].filter(Boolean));
    function attach(id,buffer){
      if(disposed||typeof id!=='string'||!buffer||!(buffer.duration>0)||assets.has(id))return false;
      const e=SHIP.baked.find(b=>b.id===id)||SHIP.textures.find(x=>x.id===id);
      if(e?buffer.duration*LOOP_RATE<e.loopEnd-1:!SHOT_IDS.has(id))return false;
      assets.set(id,buffer);
      if(!sides[0].bank&&SHIP.baked.every(b=>assets.has(b.id)))buildBanks();
      if(e&&e.layer)buildTexture(e,buffer);
      return true;
    }
    // A texture loop plays on both engines (spec 3.3, 3.4); the drone gets a slow shimmer, at a different phase on each side.
    function buildTexture(e,buffer){
      for(const s of sides){
        const src=T.loop(buffer,e.loopStart/LOOP_RATE,e.loopEnd/LOOP_RATE,ctx.currentTime+.02),w=T.gain(0);
        src.connect(w);
        if(e.layer==='drone'){const am=T.am('lfoSlow',s.o.side<0?.7:2.9,.25);w.connect(am.node);am.node.connect(s.texSum);}
        else w.connect(e.layer==='jet'?s.jetCut:s.texSum);
        s.tex.push({e,src,w});
      }
    }
""")
# J: texture rates and weights
b.rep("""    // Shift shaping (spec 3.1), scheduled at the event""", """    // Texture rates follow the engine's rpm (spec 3.3), clamped to 0.5-2. Weights: F1 mid/high by rpm and on/off by load (0.6 in
    // gears 1-2), the jet by the afterburner's presence, the drone by load. Different recordings never sum coherently, so the
    // crossfades inside a layer are equal-power.
    function textures(s,gear,load,burn){
      if(!s.tex.length)return;
      const r=s.hz/GP.redlineHz,on=ss(.1,.4,load),x=clamp((r-.65)/.3,0,1),f1=gear>0&&gear<3?.6:1,q=Math.PI/2;
      for(const k of s.tex){
        const e=k.e;
        S(k.src.playbackRate,r/e.refRpm,.03,.5,2);
        const w=e.layer==='f1'?f1*(e.role==='mid'?Math.cos(x*q)*Math.sqrt(on):e.role==='high'?Math.sin(x*q)*Math.sqrt(on):Math.sqrt(1-on))
          :e.layer==='jet'?(.4+.6*burn)*(e.role==='burn'?Math.sin(burn*q):Math.cos(burn*q)):.6+.4*load;
        S(k.w.gain,LV.texture*TEX*e.weight*w,.05,0,1);
      }
    }
    // Shift shaping (spec 3.1), scheduled at the event""")
# K: shift pops
b.rep("""        s.rate=b;s.rampUntil=t+hold+glide;writes+=3*s.bank.length;
      }
    }
""", """        s.rate=b;s.rampUntil=t+hold+glide;writes+=3*s.bank.length;
      }
      pops(up?SHIP.oneShots.upshift:SHIP.oneShots.downshift,t);
    }
""")
# L: per-frame textures; the side level carries the ship's tone level, the tone layer sits on toneSum
b.rep("      M.burst=racing?sat(fin(f.burst,0)):0;\n", "")
b.rep("      const dmg=M.dmg,bA=M.burnA,bu=M.burst,rpm=g.tone/GP.redlineHz;\n", "      const dmg=M.dmg,bA=M.burnA,rpm=g.tone/GP.redlineHz;\n")
b.rep("""        if(s.bank){if(NOW>=s.rampUntil)toneRates(s,s.hz,.012);toneWeights(s,g.tone,s.load);}
""", """        if(s.bank){if(NOW>=s.rampUntil)toneRates(s,s.hz,.012);toneWeights(s,g.tone,s.load);}
        textures(s,g.gear,s.load,bA);
""")
b.rep("        S(s.vl.gain,LV.tone*TONE,.05,0,1.2);\n", "        S(s.vl.gain,TONE,.05,0,1.2);S(s.toneSum.gain,LV.tone,.05,0,1);\n")
# M: the 1.2.0 afterburner controls go
b.rep("""      S(bbp.frequency,450+250*bA+350*bu,.06,100,6000);S(bl.gain,LV.burn*.17*Math.pow(bA,1.1)*(1+.3*bu),.05,0,.9);
      S(bam.depth.gain,.35+.35*bu+.25*sat(1-charge*5)*bA,.05,0,.8);S(bam.node.gain,.65-.35*bu,.05,0,1);
      S(bll.gain,LV.burn*.11*bA,.08,0,.5);S(bhg.gain,LV.burn*.03*bA*(.6+.4*bu),.06,0,.3);
""", "")
# N: events
b.rep("""        case'ignite':ignite(t,d.strength||1);break;
        case'launch':ignite(t,.6);break;
        case'release':release(t,d.strength||1);break;
        case'pad':ignite(t,.4);break;
""", """        case'ignite':ignite(t,d.strength||1);break;
        case'launch':playShot(SHIP.oneShots.launch,shotIn,t,LV.burn*SHOT,1);break;
        case'release':release(t,d.strength||1);break;
        case'pad':ignite(t,.6,.25);break;
""")
# O, P: reset and dispose stop the one-shots
b.rep("""      const t=now==null?ctx.currentTime:now;NOW=t;
      for(const[g,rest]of oneShots){""", """      const t=now==null?ctx.currentTime:now;NOW=t;
      for(const s of[...shots])s.stop();
      for(const[g,rest]of oneShots){""")
b.rep("""      if(disposed)return;disposed=true;
      T.dispose();""", """      if(disposed)return;disposed=true;
      for(const s of[...shots])s.stop();
      T.dispose();""")
# Q: stats
b.rep(",tone:!!sides[0].bank,attached:assets.size};},\n", ",tone:!!sides[0].bank,attached:assets.size,textures:sides[0].tex.length,shots:shots.size};},\n")
assert not any(k in b.engine() for k in ("burnSum", "burnCut", "iOsc", "rSg", "bhg", "bll", "bam.")), "1.2.0 afterburner left"
b.save()
print("step 5: textures and one-shots are in the engine block")
```

```bash
python prototypes/Dustline/tools/engine2/step5_textures.py
```

Expected: `pushed <sha256> <bytes> bytes`, then `step 5: textures and one-shots are in the engine block`.

- [ ] **Step 4: Run the tests to see them pass.**

```bash
node prototypes/Dustline/checks/audio-engine.test.cjs
```

Expected: `20 engine audio checks passed.`

- [ ] **Step 5: Run the regression (0.6).** Expected: 165 checks (`audio-engine.test.cjs: 20 engine audio checks passed.`) and both ai-pace `PASS` lines.

### Task 15: Profile assets, encode and embed

**Purpose.** The real textures and one-shots reach the profile and the page (spec 2.2, 4.5; S12, S29, S40). `tools/engine2/profile_assets.py` copies `profile-assets.json` into the Kestrel profile (re-runnable). The test checks the profile against the file, the six textures' layers, roles, weights, loop points and refRpm, the drone an octave under the fundamental, the one-shot ids, and that the bundle holds exactly the 22 assets. `audio-browser.py` also checks each pitched texture's decoded pitch against its `baseHz`.

**Files:**
- Modify: `prototypes/Dustline/checks/ship-profile.test.cjs` (LF, from Task 2), `prototypes/Dustline/checks/audio-browser.py` (CRLF)
- Create: `prototypes/Dustline/tools/engine2/profile_assets.py`
- Modify (by `profile_assets.py` and `apply-sfx.py`): the three game copies
- Generated: `prototypes/Dustline/audio/ships/kestrel/opus/*.opus` (22)

- [ ] **Step 1: Write the failing test.** Apply this Edit, then run the line-ending check (expected `LF`).

Edit `prototypes/Dustline/checks/ship-profile.test.cjs`. Replace:

```js
test('the ship bundle holds exactly the profile assets; phase 1 has no textures or one-shots yet', () => {
  assert.equal(K.textures.length, 0);
  assert.deepEqual(JSON.parse(JSON.stringify(K.oneShots)), { upshift: [], downshift: [], ignite: '', release: '', launch: '' });
  const bundle = JSON.parse(html.match(/const DUSTLINE_SHIP_B64 = (\{[^\r\n]*\});/)[1]);
  const ids = [...K.baked.map(b => b.id), ...K.textures.map(x => x.id), ...K.oneShots.upshift, ...K.oneShots.downshift,
    K.oneShots.ignite, K.oneShots.release, K.oneShots.launch].filter(Boolean);
```

with:

```js
test('textures and one-shots are audio/ships/kestrel/profile-assets.json; the ship bundle holds exactly the profile assets', () => {
  const man = JSON.parse(fs.readFileSync(path.join(root, 'audio', 'ships', 'kestrel', 'profile-assets.json'), 'utf8'));
  assert.deepEqual(JSON.parse(JSON.stringify(K.textures)), man.textures);
  assert.deepEqual(JSON.parse(JSON.stringify(K.oneShots)), man.oneShots);
  // arrays made in the sandbox have its prototypes: spread them into this realm before deepEqual
  assert.deepEqual([...K.textures.map(x => `${x.id} ${x.layer}/${x.role}`)],
    ['f1-mid f1/mid', 'f1-high f1/high', 'f1-off f1/off', 'jet-cruise jet/cruise', 'jet-burn jet/burn', 'drone drone/drone']);
  for (const x of K.textures) {
    assert.equal(x.weight, { f1: .4, jet: .35, drone: .25 }[x.layer], x.id);
    assert.equal(x.loopStart, 2400, x.id);
    assert.equal(x.loopEnd, 2400 + 144000, x.id);
    assert.ok(x.refRpm > 0 && x.baseHz >= 0, x.id);
  }
  assert.deepEqual([...K.textures.filter(x => x.layer !== 'drone').map(x => x.refRpm)], [.65, .95, .65, .857, 1.25]);
  const drone = K.textures.find(x => x.layer === 'drone');
  assert.ok(drone.baseHz > 0 && Math.abs(drone.refRpm - 2 * drone.baseHz / K.gears.redlineHz) < 1e-4, 'the drone sits an octave under the fundamental');
  assert.deepEqual(JSON.parse(JSON.stringify(K.oneShots)),
    { upshift: ['up-1', 'up-2', 'up-3'], downshift: ['down-1', 'down-2'], ignite: 'ignite', release: 'release', launch: 'launch' });
  const bundle = JSON.parse(html.match(/const DUSTLINE_SHIP_B64 = (\{[^\r\n]*\});/)[1]);
  const ids = [...K.baked.map(b => b.id), ...K.textures.map(x => x.id), ...K.oneShots.upshift, ...K.oneShots.downshift,
    K.oneShots.ignite, K.oneShots.release, K.oneShots.launch];
  assert.equal(ids.length, 22);
```

- [ ] **Step 2: Run it to see it fail.**

```bash
node prototypes/Dustline/checks/ship-profile.test.cjs
```

Expected: `AssertionError [ERR_ASSERTION]: Expected values to be strictly deep-equal:` (the profile has no textures yet).

- [ ] **Step 3: Create and run the profile tool.** Create `prototypes/Dustline/tools/engine2/profile_assets.py`:

```python
"""Engine 2.0, plan Task 15: the Kestrel profile takes its textures and one-shots from audio/ships/kestrel/profile-assets.json
(tools/ship_assets.py). Re-runnable: it rewrites the profile's two lines, whatever they hold."""
import json
import re
from pathlib import Path

from common import Build

SHIP = Path(__file__).resolve().parents[2] / "audio" / "ships" / "kestrel"
man = json.loads((SHIP / "profile-assets.json").read_text(encoding="utf-8"))
b = Build()
pat = re.compile(r"\n    textures:\[[^\n]*\],\n    oneShots:\{[^\n]*\},\n")
assert len(pat.findall(b.t)) == 1, "expected the Kestrel profile's textures and oneShots lines once"
js = lambda v: json.dumps(v, separators=(",", ":"))
b.t = pat.sub(lambda _: f"\n    textures:{js(man['textures'])},\n    oneShots:{js(man['oneShots'])},\n", b.t)
b.save()
print(f"profile: {len(man['textures'])} textures and {sum(len(v) if isinstance(v, list) else 1 for v in man['oneShots'].values())} one-shots")
```

```bash
python prototypes/Dustline/tools/engine2/profile_assets.py
```

Expected: `pushed <sha256> <bytes> bytes`, then `profile: 6 textures and 8 one-shots`.

- [ ] **Step 4: Encode and embed all 22 assets.**

```bash
python prototypes/Dustline/tools/ship_encode.py
python prototypes/Dustline/apply-sfx.py
```

Expected: `encoded 22 assets into …\opus: <bytes> bytes of Opus`, then `pushed …` and `Embedded 29 sounds and 22 ship assets (about <bytes> bytes of Opus) in all three 0.13 copies; page <bytes> bytes.` The page is about 2.2 MB (2,200,322 bytes with the planning stand-ins; the spec's "about 2.1 MB" was an estimate).

- [ ] **Step 5: Run the test to see it pass.**

```bash
node prototypes/Dustline/checks/ship-profile.test.cjs
```

Expected: `ship profile: 7 passed`.

- [ ] **Step 6: Check the texture pitch in the browser.** Apply these Edits to `prototypes/Dustline/checks/audio-browser.py`, then run the line-ending check (expected `CRLF`).

Edit `prototypes/Dustline/checks/audio-browser.py` (1 of 3). Replace:

```python
44.1 kHz: its length must be sample-exact and its loop seam continuous.

The page runs in real time; test hooks are injected here, never shipped. Nothing here judges how the engine sounds.
"""
from pathlib import Path
import json
import re
```

with:

```python
44.1 kHz: its length must be sample-exact and its loop seam continuous. Each pitched texture's decoded loop must measure within
3 % of its profile baseHz (checks/pitch.py).

The page runs in real time; test hooks are injected here, never shipped. Nothing here judges how the engine sounds.
"""
from pathlib import Path
import base64
import json
import re
import sys

import numpy as np
```

Edit `prototypes/Dustline/checks/audio-browser.py` (2 of 3). Replace:

```python
SEAM_LIMIT = {"baked": 0.1, "texture": 0.25}
```

with:

```python
SEAM_LIMIT = {"baked": 0.1, "texture": 0.25}
sys.path.insert(0, str(Path(__file__).resolve().parent))
import pitch  # noqa: E402
# The loop region of each pitched texture, decoded at 48 kHz and mixed to mono, as base64 float32 for pitch.summary.
TEXTURE_PITCH = """async assets => {
  const K = DustlineEngineAudio.ships.kestrel, out = [];
  const ctx = new OfflineAudioContext(2, 1, 48000);
  for (const e of K.textures.filter(x => x.baseHz > 0)) {
    const bin = atob(assets['kestrel:' + e.id]), u8 = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i);
    const buf = await ctx.decodeAudioData(u8.buffer), mono = new Float32Array(e.loopEnd - e.loopStart);
    for (let c = 0; c < buf.numberOfChannels; c++) {
      const x = buf.getChannelData(c);
      for (let i = 0; i < mono.length; i++) mono[i] += x[e.loopStart + i] / buf.numberOfChannels;
    }
    const u = new Uint8Array(mono.buffer); let s = '';
    for (let i = 0; i < u.length; i += 32768) s += String.fromCharCode.apply(null, u.subarray(i, i + 32768));
    out.push({ id: e.id, baseHz: e.baseHz, mono: btoa(s) });
  }
  return out;
}"""
```

Edit `prototypes/Dustline/checks/audio-browser.py` (3 of 3). Replace:

```python
        print(json.dumps({"browser": browser.version, "offline": True, "ship": ship, "engineCreates": upd["creates"], "engineNodes": nodes,
                          "worstSeam": max(seams, key=lambda r: r["seam"]),
                          "engineSources": sources, "timing": results, "updates": upd["n"], "meanUpdateMs": round(mean, 4),
                          "worstUpdateMs": round(upd["worst"], 3), "writes": e["writes"], "skipped": e["skipped"], "errors": errors,
                          "checks": "unlock, ship assets attached and tone on, grid voices, steer and boost, vent and repair cues in "
                                    "sfx and synth, pause park x3, mute suspend/resume x6, hidden tab, 12 restarts with mode and pause "
                                    "cycles, loop lengths and seams at 48 and 44.1 kHz"}))
```

with:

```python
        pitched = []
        for r in page.evaluate(TEXTURE_PITCH, assets):
            f0 = pitch.summary(np.frombuffer(base64.b64decode(r["mono"]), dtype="<f4").astype(np.float64), 48000)["f0"]
            assert abs(f0 / r["baseHz"] - 1) <= .03, (r["id"], f0, r["baseHz"])
            pitched.append({"id": r["id"], "baseHz": r["baseHz"], "decodedHz": round(f0, 2)})
        print(json.dumps({"browser": browser.version, "offline": True, "ship": ship, "engineCreates": upd["creates"], "engineNodes": nodes,
                          "worstSeam": max(seams, key=lambda r: r["seam"]), "texturePitch": pitched,
                          "engineSources": sources, "timing": results, "updates": upd["n"], "meanUpdateMs": round(mean, 4),
                          "worstUpdateMs": round(upd["worst"], 3), "writes": e["writes"], "skipped": e["skipped"], "errors": errors,
                          "checks": "unlock, ship assets attached and tone on, grid voices, steer and boost, vent and repair cues in "
                                    "sfx and synth, pause park x3, mute suspend/resume x6, hidden tab, 12 restarts with mode and pause "
                                    "cycles, loop lengths and seams at 48 and 44.1 kHz, texture baseHz"}))
```

```bash
python prototypes/Dustline/checks/audio-browser.py
```

Expected: exit code 0 and one JSON line with `"ship": {"id": "kestrel", "decoded": 22, "total": 22}`, `"engineNodes": 222`, `"engineSources": 66`, a `worstSeam` below its kind's limit (the planning stand-ins: a texture at 0.114 against 0.25), `texturePitch` with the drone's `decodedHz` within 3 % of its `baseHz` (planning: within 0.04 %), and `"errors": []`.

- [ ] **Step 7: Run the regression (0.6).** Expected: 165 checks and both ai-pace `PASS` lines.

### Task 16: Audition with textures, regression and README

**Purpose.** Close phase 2: the full BAKED audition for the user, everything passing, the README describing the full BAKED engine (spec 3.6, 7, 9; S42, S54).

**Files:**
- Modify: `prototypes/Dustline/README.md` (CRLF)
- Generated: `prototypes/Dustline/audio/audition-v0.13/` (the files of Task 8), new `LOG.md` section

- [ ] **Step 1: Render the audition.**

```bash
python prototypes/Dustline/checks/audio_audition.py
```

Expected: `attached 22 discontinuities 0 …`, the speeds and gear changes of Task 8, and `targets` now with `layers` measured (each texture's pick z from `picks.json`; the target is z ≥ 2.5). Report every number as measured.

- [ ] **Step 2: Screen the audition** (earshot venv).

```bash
PY="/d/MY DOCS/MY TOOLS/earshot/.venv/Scripts/python.exe"
"$PY" -I prototypes/Dustline/checks/ship_screen.py audition prototypes/Dustline/audio/audition-v0.13
```

Expected: one JSON line with the timeline summary and the per-section `previous > new` cosines; `audition-v0.13/earshot.json` written.

- [ ] **Step 3: Run the regression and the browser harnesses (0.6).** Expected: 165 checks, both ai-pace `PASS` lines and eight harness `PASS` lines.

- [ ] **Step 4: README.** Replace the whole `## Engine audio` section of `prototypes/Dustline/README.md` with the text below, using the byte-preserving replacement of Task 9 Step 4 (write the text to `prototypes/Dustline/readme-section.tmp`, run the same `python -c` command, which also deletes the file). Use the measured nodes, sources and page size if they differ.

```markdown
## Engine audio

The engine (`DustlineEngineAudio` 2.0.0) is driven by a ship profile and a gear model. The Kestrel is the only ship for now; a later ship is a new profile plus its assets, not new code. Airflow, heat strain, damage misfires, vent steam, rival engines, the timing cues, the ducking rules and the output stage (glue compressor −11 dB 3.8:1, limiter −1.5 dB 20:1, master gain 0.75) are the 1.2.0 layers. The ElevenLabs one-shots (contact, gates, laps, countdown, hazards, timing cues) remain: the page carries 29 of them.

**Gear model.** Seven gears with top speeds 80, 150, 220, 300, 390, 490 and 700 km/h. Within a gear the fundamental is `124 Hz × v / top`, never below 41 Hz. An upshift comes at 0.97 of the gear's top while the craft accelerates, at least 0.22 s after the last shift; while it waits, the pitch holds at 1.03 × redline. A downshift comes when the speed falls to 0.92 × the lower gear's top, at least 0.09 s after the last shift. In gears 1–6 the rules read a paced speed that rises at most 250 km/h per second, so each launch upshift is a full sweep with an audible drop (0.19, 0.54 and 0.93 s after GO). Past 700 km/h in 7th, boost keeps lifting the pitch: `124 × (v / 700)^0.8`, about 191 Hz at 1200 km/h. Cruise (600 km/h) sits at 0.857 of 7th, off the limiter. On the grid the pitch follows the throttle (neutral revs); GO picks the gear from speed (4th at full throttle) with the launch sound and is not a shift. An upshift cuts the tone 9 dB for 40 ms, glides down over 60 ms and blips the zing (+6 dB at 8 × the fundamental); a downshift blips the throttle (+3 dB for 80 ms). The model is deterministic and does not depend on the frame rate: it advances in 1/120 s substeps.

**Tone.** The Keys synth patch `audio/ships/kestrel/kestrel-engine.lbpatch` (KESTREL ENGINE) holds the sound: SUB and a sync-warped OSC A at the fundamental through a driven 24 dB low-pass, a 3-voice OSC B two octaves up that bypasses the filter for the zing, and FX DRIVE TUBE; the mod wheel opens both warps, the cutoff, the filter drive and both oscillator levels. The app's own Rust synth renders it offline (`crates/lpd-core/examples/bake_engine.rs`) into eight 2 s loops: four pitch anchors (48, 75, 117 and 182 Hz), each on-load (wheel 0.85) and off-load (wheel 0.2). The loops are phase-locked, so the engine starts all eight of an engine together and crossfades them at equal gain: linearly in log frequency between the two nearest anchors, and by a smoothstep on load. Every loop of an engine plays at `fundamental / anchor`.

**Textures.** Six ElevenLabs loops, 3 s each, follow the same rpm on both engines, each at `playbackRate = rpm / refRpm` (clamped to 0.5–2): F1 V10 at mid RPM, at high RPM and off-load burble (40 %, full in gears 3–7 and 0.6 in gears 1–2; mid and high crossfaded by rpm, on and off by load); jet turbine cruise roar and afterburner burn (35 %, `0.4 + 0.6 × boost`, cruise to burn by boost intensity); and a spaceship drone an octave under the fundamental (25 %, `0.6 + 0.4 × load`, with a slow shimmer at a different phase on each side). Different recordings never sum coherently, so the crossfades inside a layer are equal-power.

**One-shots.** An upshift pops a crackle (three variants: each engine plays a different one, chosen by the seeded generator), a downshift a burble blip (two variants), and boost ignition, boost release and the launch each have their own sound; a boost pad plays a shortened ignition. At most six play at once (the oldest stops first). They replace the 1.2.0 afterburner ignition, burn and release voices.

**Twin engines.** The left engine sits at pan −0.34, the right at +0.34 and 0.35 % higher. Each engine's load follows its own fader: full thrust is at the fader's 0 dB mark (0.75), the boost zone adds no load, and braking takes 45 % off. Load rises within about 55 ms and falls within 110 ms; below 0.25 the tone moves to the hollow off-load loops.

**Kept layers.** The strain resonators now ring from a pulse wave at the fundamental, which reaches the resonators only. A misfire dips the tone briefly and detunes its own engine 0.6 %; thrust stays audible. Running dry in boost flickers the jet textures. Rivals keep the 1.2.0 procedural engine.

**Assets and lifecycle.** The ship's 22 assets (8 baked loops, 6 textures, 8 one-shots) are Ogg Opus at 96 kbps and 48 kHz, which WebView2 and Chromium decode gaplessly. They are embedded as `DUSTLINE_SHIP_B64`, one line beside the SFX bundle, keyed `kestrel:<id>`. The game decodes them and hands each one to the engine (`attach`); the engine itself never decodes audio. The engine builds 222 nodes (66 sources) once, plus at most six one-shot voices, and only writes parameters per frame. Pause, mute, hidden tab and restart behave as in 1.2.0. Variation comes from a private seeded generator, never `Math.random` or the simulation's random stream. The page is about 2.2 MB.

**Ship profile.** `DustlineEngineAudio.ships.kestrel` holds `id`, `name`, `gears` (`tops`, `idleHz` 41, `redlineHz` 124, `boostExponent` 0.8, `upMin` 0.22, `downMin` 0.09, `overRev` 1.03, `pace` 250), `patch` (the `.lbpatch` JSON), `patchName` (`DUSTLINE KESTREL`), `liveNote` 48, `baked` (eight `{id, anchorHz, load, loopStart, loopEnd}`, in samples at 48 kHz: 0.05 s lead-in, then the 2 s loop), `textures` (six `{id, layer, role, refRpm, baseHz, loopStart, loopEnd, weight}`: 0.05 s lead-in, then the 3 s loop), `oneShots` (`{upshift: [3 ids], downshift: [2 ids], ignite, release, launch}`) and `mix` (levels in dB, shift cut and blips, pan, right-engine pitch, LIVE trim). The gear model is `DustlineEngineAudio.gears`.

**Playing the patch.** `kestrel-engine.lbpatch` is copied to `Documents\Launchpad Drummer\Patches`, so PLAY lists it as KESTREL ENGINE. Hold note 48 (one octave below middle C) and use the pitch wheel as speed and the mod wheel as load. Save a tweak as "Dustline Kestrel" (stored as `DUSTLINE KESTREL.lbpatch`). To hear it in the game, copy its `params` into `kestrel-engine.lbpatch`, run `python prototypes/Dustline/tools/engine2/profile_patch.py`, then bake, encode and embed (Tools below).

**Tools** (paths from `D:\MY DOCS\MY TOOLS\LAUNCHBAND`):

- `python prototypes/Dustline/tools/sync_copies.py check|diff|push` compares the three copies with the recorded hash (`tools/copies.sha256`). `tools/engine2/step*.py` are the byte-preserving edits that built 2.0, one per step.
- Bake, from `launchpad-drummer`: `cargo run -p lpd-core --release --example bake_engine -- loops --patch <file.lbpatch> --out ..\prototypes\Dustline\audio\ships\kestrel\loops` (`check --patch <file>` reports any value the app would clamp). Then `python prototypes/Dustline/tools/ship_encode.py` and `python prototypes/Dustline/apply-sfx.py`.
- Textures and one-shots: `python prototypes/Dustline/generate-ship-sfx.py` lists the 42 ElevenLabs generations (14 sounds × 3 takes) and `--go` makes the missing ones with the user's credits (key from `.env`, never printed). `checks/ship_screen.py takes` (earshot venv) screens every take and picks one per sound by a fixed rule (the highest earshot `match` z among the takes that pass steadiness and a clean seam or trim) into `audio/ships/kestrel/picks.json`. `python prototypes/Dustline/tools/ship_assets.py` masters the picks into `loops/` and `shots/` and writes `profile-assets.json`; `python prototypes/Dustline/tools/engine2/profile_assets.py` puts them in the profile. Then encode and embed as above.
- earshot screening, in the earshot venv: `checks/ship_screen.py loops` writes `audio/ships/kestrel/LOG.md` and `loops/analysis.json` (pitch with `checks/pitch.py`, seams, phase, band shares, earshot `measure` and `describe`).
- Audition: `python prototypes/Dustline/checks/audio_audition.py`, then `checks/ship_screen.py audition prototypes/Dustline/audio/audition-v0.13` in the earshot venv. The frames come from the real simulation (`checks/ship-scenarios.cjs`), and 0.12's engine is rendered on the same frames for comparison.

**Checks.** 165 deterministic checks (`audio-engine` 20, `ship-gears` 17, `ship-profile` 7 and the other 121), the 8 browser harnesses and `ai-pace.cjs --verify`. `audio-browser.py` decodes every loop at 48 and 44.1 kHz (seam limits 0.1 for baked loops and 0.25 for textures, whose Opus coding noise alone reads about 0.1–0.15) and checks each pitched texture against its `baseHz`. The synth-only audition is in `audio/audition-v0.13-p1/`, the full one in `audio/audition-v0.13/` (`metrics.json`, `earshot.json`); these are measurements, and the listening verdict is the user's.
```

Then run the line-ending check on the README (expected `CRLF`).

- [ ] **Step 5: Hash check (0.6).** Expected: `ok <sha256>`.

- [ ] **Step 6: Report, then CHECKPOINT 2.** Report, as measurements only: 165 checks, 8 harnesses, ai-pace; the picks (take and z per sound) and the mastering lines; Edge (22 assets, nodes, sources, worst seam per kind, texture pitch, page size); the audition numbers and targets (layers z, bassy, pitch, zing, cruise propulsion against −16 LUFS); the earshot timeline and A/B cosines. What to listen to: `prototypes/Dustline/audio/audition-v0.13/engine-as-played.wav`, `propulsion.wav`, `ab-matched.wav`; the masters in `audio/ships/kestrel/loops/` and `shots/`; the game in Edge.

**CHECKPOINT 2. Stop here and wait for the user's listening verdict. Do not start Task 17 on your own.**

---

## Phase 3 — LIVE SYNTH

Phase 3 lets the app play the ship's tone live on its native synth: a private `GameEngineVoice` in `lpd-core`, engine commands and the master mix, the router and Tauri commands, `engineBridge.ts` and `App.svelte`, then the LIVE sink in the engine and the ENGINE select in the game. It starts after the user's verdict at CHECKPOINT 2. Cargo runs in PowerShell from `launchpad-drummer` with the PATH of 0.1; npm and node in Git Bash from `launchpad-drummer`.

### Task 17: `GameEngineVoice`

**Purpose.** A second, private `Synth` that holds note 48 while the game streams frames (spec 5.1; fixes 3, 8, 20; S18, S19, S49, S50). LEGATO and bend 24 are forced, the rest of the patch is kept; bend and wheel glide to each frame over 40 ms and the gain ramps over 25 ms, in 16-sample ticks; silent until the first frame; `on: false` fades and releases; the watchdog fades after 250 ms without frames; `stop()` fades over 50 ms and finishes; NaN and out-of-range values are clamped or ignored; it plays at twice its frame gain (`GAME_ENGINES_GAIN`). The Keys synth is a different `Synth` and is never touched. The 8 tests build their own patch (S44).

**Files:**
- Create: `launchpad-drummer/crates/lpd-core/tests/synth_game.rs`, `launchpad-drummer/crates/lpd-core/src/synth/game.rs`
- Modify: `launchpad-drummer/crates/lpd-core/src/synth/mod.rs` (CRLF)

- [ ] **Step 1: Write the failing tests.** Create `launchpad-drummer/crates/lpd-core/tests/synth_game.rs`:

```rust
//! GameEngineVoice (Dustline ship engine spec, sections 5.1 and 7).
use lpd_core::synth::game::*;
use lpd_core::synth::params::*;

const RATE: u32 = 48_000;
const TICK: usize = 16;

fn frame(bend: f32, wheel: f32, gain: f32) -> GameFrame {
    GameFrame {
        on: true,
        bend,
        wheel,
        gain,
    }
}

fn render(v: &mut GameEngineVoice, samples: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (vec![0.0; samples], vec![0.0; samples]);
    for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        v.render_add(cl, cr);
    }
    (l, r)
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0, |m, v| m.max(v.abs()))
}

fn engine_patch() -> SynthParams {
    let mut p = SynthParams::default();
    p.osc_a.on = true;
    p.sub.on = true;
    p
}

#[test]
fn legato_and_a_24_semitone_bend_are_forced_and_the_rest_of_the_patch_is_kept() {
    let mut p = engine_patch();
    p.voice.mode = VoiceMode::Poly;
    p.voice.bend_range = 2;
    p.filter.cutoff = 900.0;
    let v = GameEngineVoice::new(RATE, &p);
    assert_eq!(v.params().voice.mode, VoiceMode::Legato);
    assert_eq!(v.params().voice.bend_range, 24);
    let mut want = p;
    want.voice.mode = VoiceMode::Legato;
    want.voice.bend_range = 24;
    assert_eq!(*v.params(), want);
}

#[test]
fn silent_until_the_first_frame_then_the_note_fades_in_over_25_ms() {
    let mut v = GameEngineVoice::new(RATE, &engine_patch());
    assert_eq!(peak(&render(&mut v, 4800).0), 0.0);
    assert!(!v.current().on);
    v.frame(frame(0.0, 0.5, 1.0));
    let (l, _) = render(&mut v, 4800);
    assert!(v.current().on);
    assert!(peak(&l[..48]) < peak(&l[2400..]) * 0.1, "fades in");
    assert!(peak(&l[2400..]) > 0.01);
    assert_eq!(v.current().gain, 1.0);
}

#[test]
fn bend_and_wheel_glide_over_40_ms_and_the_gain_ramps_over_25_ms_in_16_sample_ticks() {
    let mut v = GameEngineVoice::new(RATE, &engine_patch());
    v.frame(frame(0.0, 0.2, 1.0));
    render(&mut v, 4800);
    // 375 cents in one frame: 3.125 cents per tick, the target reached after 120 ticks (40 ms at 48 kHz); the gain
    // lands after 75 ticks (25 ms)
    v.frame(frame(3.75 / 24.0, 0.85, 0.5));
    let (mut l, mut r) = ([0.0f32; TICK], [0.0f32; TICK]);
    let mut last = v.current();
    for tick in 1..=125 {
        v.render_add(&mut l, &mut r);
        let now = v.current();
        let cents = (now.bend - last.bend) * 24.0 * 100.0;
        if tick <= 120 {
            assert!((cents - 3.125).abs() < 0.01, "tick {tick}: {cents} cents");
            assert!(now.wheel > last.wheel);
        } else {
            assert_eq!(cents, 0.0, "settled after 120 ticks");
        }
        assert_eq!(
            now.gain == 0.5,
            tick >= 75,
            "tick {tick}: gain {}",
            now.gain
        );
        last = now;
    }
    assert_eq!(last.bend, 3.75 / 24.0);
    assert_eq!(last.wheel, 0.85);
}

#[test]
fn the_watchdog_fades_out_after_250_ms_without_frames_and_a_new_frame_restarts_the_note() {
    let mut v = GameEngineVoice::new(RATE, &engine_patch());
    v.frame(frame(0.0, 0.5, 1.0));
    let (l, _) = render(&mut v, 12_000); // 250 ms
    assert!(peak(&l[9_600..]) > 0.01, "still sounding at 200-250 ms");
    let (l, _) = render(&mut v, 2_400 + 512); // the 50 ms fade, then silence
    assert!(peak(&l[2_400 + 256..]) == 0.0, "silent after the fade");
    assert!(!v.current().on, "the note is released");
    v.frame(frame(0.0, 0.5, 1.0));
    let (l, _) = render(&mut v, 4_800);
    assert!(
        v.current().on && peak(&l[2_400..]) > 0.01,
        "a new frame restarts the note"
    );
}

#[test]
fn on_false_fades_out_over_one_ramp_and_releases_the_note() {
    let mut v = GameEngineVoice::new(RATE, &engine_patch());
    v.frame(frame(0.0, 0.5, 1.0));
    render(&mut v, 4_800);
    v.frame(GameFrame {
        on: false,
        bend: 0.0,
        wheel: 0.5,
        gain: 1.0,
    });
    let (l, _) = render(&mut v, 1_200 + 256);
    assert!(peak(&l[1_200 + 16..]) == 0.0);
    assert!(!v.current().on);
}

#[test]
fn nan_and_out_of_range_values_are_clamped_or_ignored() {
    let mut v = GameEngineVoice::new(RATE, &engine_patch());
    v.frame(frame(0.25, 0.5, 0.5));
    render(&mut v, 4_800);
    v.frame(frame(f32::NAN, 7.0, -3.0));
    render(&mut v, 4_800);
    let c = v.current();
    assert_eq!((c.bend, c.wheel, c.gain), (0.25, 1.0, 0.0));
    v.frame(frame(2.0, f32::INFINITY, f32::NAN));
    render(&mut v, 4_800);
    let c = v.current();
    assert_eq!((c.bend, c.wheel, c.gain), (1.0, 1.0, 0.0));
    let (l, r) = render(&mut v, 4_800);
    assert!(l.iter().chain(&r).all(|s| s.is_finite()));
}

#[test]
fn stop_fades_out_over_50_ms_then_finishes_and_ignores_later_frames() {
    let mut v = GameEngineVoice::new(RATE, &engine_patch());
    v.frame(frame(0.0, 0.5, 1.0));
    render(&mut v, 4_800);
    v.stop();
    assert!(!v.finished());
    render(&mut v, 2_400);
    assert!(v.finished());
    v.frame(frame(0.0, 0.5, 1.0));
    assert_eq!(peak(&render(&mut v, 4_800).0), 0.0);
}

#[test]
fn glides_take_40_ms_at_44_1_khz_too() {
    let mut v = GameEngineVoice::new(44_100, &engine_patch());
    v.frame(frame(0.0, 0.2, 1.0));
    render(&mut v, 4_410);
    v.frame(frame(0.5, 0.2, 1.0));
    let (mut l, mut r) = ([0.0f32; TICK], [0.0f32; TICK]);
    let mut ticks = 0;
    while v.current().bend < 0.5 {
        v.render_add(&mut l, &mut r);
        ticks += 1;
    }
    assert_eq!(ticks, 110, "round(0.040 x 44100 / 16)");
}
```

- [ ] **Step 2: Run them to see them fail.**

```powershell
cargo test -p lpd-core --test synth_game
```

Expected: `` error[E0432]: unresolved import `lpd_core::synth::game` ``.

- [ ] **Step 3: Create the voice and register the module.** Create `launchpad-drummer/crates/lpd-core/src/synth/game.rs`:

```rust
//! Dustline's live ship engine voice (spec `2026-11-05-dustline-ship-engine-design.md`, section 5.1).
//!
//! A second, private `Synth` holds one note while the game streams frames, one per engine control step (about
//! 1/40 s: 30 Hz at a 60 Hz display, 36 Hz at 144 Hz, 40 Hz offline): `bend` (−1…1 of a forced 24-semitone range),
//! `wheel` (0…1, the patch's WHEEL source) and `gain`. Bend and wheel glide to each frame evenly over 40 ms, longer
//! than any gap between frames, so the pitch never stalls between them; the gain ramps over 25 ms, so shift cuts stay
//! crisp. Both step once per 16-sample synth tick, and the gain moves per sample inside a tick.
//! With no frame for 250 ms the voice fades out over 50 ms and releases its note. Built off the audio thread; on it,
//! it never allocates. The user's Keys synth is a different `Synth` and is never touched.

use super::env::TICK;
use super::params::{SynthParams, VoiceMode};
use super::Synth;
use crate::instrument::Instrument;

/// The held note. It sounds twice the ship's fundamental, so the game measures its bend from 65.41 Hz (MIDI 36).
pub const GAME_NOTE: u8 = 48;
pub const GAME_VELOCITY: u8 = 127;
pub const GAME_BEND_RANGE: u8 = 24;
pub const GAME_RAMP_S: f32 = 0.025;
/// Bend and wheel glide time: longer than the longest gap between frames (1/30 s at a 60 Hz display).
pub const GAME_GLIDE_S: f32 = 0.040;
pub const GAME_WATCHDOG_S: f32 = 0.25;
/// One voice stands for both of the ship's engines (spec 5.4): BAKED sums two engine voices, each at the profile's tone
/// level, so the voice plays at twice its frame gain (+6 dB) to match them at the same gain.
pub const GAME_ENGINES_GAIN: f32 = 2.0;
pub const GAME_FADE_S: f32 = 0.05;

/// One frame from the game. `on: false` fades the voice out over one ramp and releases the note.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameFrame {
    pub on: bool,
    pub bend: f32,
    pub wheel: f32,
    pub gain: f32,
}

/// A value that moves to its target in equal steps, one per tick, and lands on it exactly.
#[derive(Debug, Clone, Copy)]
struct Ramp {
    value: f32,
    target: f32,
    step: f32,
    left: u32,
}

impl Ramp {
    fn at(v: f32) -> Ramp {
        Ramp {
            value: v,
            target: v,
            step: 0.0,
            left: 0,
        }
    }
    fn to(&mut self, target: f32, ticks: u32) {
        self.target = target;
        self.left = ticks.max(1);
        self.step = (target - self.value) / self.left as f32;
    }
    fn tick(&mut self) -> f32 {
        if self.left > 0 {
            self.left -= 1;
            self.value = if self.left == 0 {
                self.target
            } else {
                self.value + self.step
            };
        }
        self.value
    }
    fn settled(&self) -> bool {
        self.left == 0
    }
}

/// A finite value clamped to `lo..=hi`; anything else keeps `keep`.
fn clean(v: f32, lo: f32, hi: f32, keep: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        keep
    }
}

pub struct GameEngineVoice {
    synth: Synth,
    ramp_ticks: u32,
    glide_ticks: u32,
    fade_ticks: u32,
    watchdog: usize,
    bend: Ramp,
    wheel: Ramp,
    gain: Ramp,
    /// 1 while frames arrive; the watchdog and `stop` take it to 0.
    fade: Ramp,
    /// The output gain reached at the end of the last tick (gain × fade).
    level: f32,
    since_frame: usize,
    want_on: bool,
    held: bool,
    stopping: bool,
    finished: bool,
    l: [f32; TICK],
    r: [f32; TICK],
}

impl std::fmt::Debug for GameEngineVoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GameEngineVoice")
    }
}

impl GameEngineVoice {
    /// Builds the voice and applies `patch` with LEGATO and a 24-semitone bend range forced. Allocates: call it off
    /// the audio thread.
    pub fn new(sample_rate: u32, patch: &SynthParams) -> Box<GameEngineVoice> {
        let rate = sample_rate.max(1) as f32;
        let mut params = *patch;
        params.voice.mode = VoiceMode::Legato;
        params.voice.bend_range = GAME_BEND_RANGE;
        let mut synth = Synth::new(sample_rate);
        synth.load_params(&params);
        // Play out the 5 ms load fade now, so the first note is not held back.
        let (mut l, mut r) = ([0.0f32; 256], [0.0f32; 256]);
        for _ in 0..4 {
            synth.render_add(&mut l, &mut r);
        }
        let ticks = |s: f32| ((s * rate / TICK as f32).round() as u32).max(1);
        Box::new(GameEngineVoice {
            synth,
            ramp_ticks: ticks(GAME_RAMP_S),
            glide_ticks: ticks(GAME_GLIDE_S),
            fade_ticks: ticks(GAME_FADE_S),
            watchdog: (GAME_WATCHDOG_S * rate) as usize,
            bend: Ramp::at(0.0),
            wheel: Ramp::at(0.0),
            gain: Ramp::at(0.0),
            fade: Ramp::at(0.0),
            level: 0.0,
            since_frame: 0,
            want_on: false,
            held: false,
            stopping: false,
            finished: false,
            l: [0.0; TICK],
            r: [0.0; TICK],
        })
    }

    /// The patch as the voice plays it.
    pub fn params(&self) -> &SynthParams {
        self.synth.params()
    }

    /// The ramped values at the end of the last tick (`on`: the note is held).
    pub fn current(&self) -> GameFrame {
        GameFrame {
            on: self.held,
            bend: self.bend.value,
            wheel: self.wheel.value,
            gain: self.level,
        }
    }

    /// The voice has faded out after `stop` and can go back through the trash.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// One 40 Hz frame. Non-finite values keep the previous target; the rest is clamped.
    pub fn frame(&mut self, f: GameFrame) {
        if self.stopping {
            return;
        }
        self.since_frame = 0;
        let bend = clean(f.bend, -1.0, 1.0, self.bend.target);
        let wheel = clean(f.wheel, 0.0, 1.0, self.wheel.target);
        let gain = clean(f.gain, 0.0, 1.0, 0.0);
        self.want_on = f.on;
        if f.on && !self.held {
            // A new note starts at the frame's pitch and wheel and fades in over one ramp.
            self.bend = Ramp::at(bend);
            self.wheel = Ramp::at(wheel);
            self.gain = Ramp::at(0.0);
            self.gain.to(gain, self.ramp_ticks);
            self.fade = Ramp::at(1.0);
            self.synth.pitch_bend(bend);
            self.synth.modulation(wheel);
            self.synth.note_on(GAME_NOTE, GAME_VELOCITY);
            self.held = true;
            return;
        }
        self.bend.to(bend, self.glide_ticks);
        self.wheel.to(wheel, self.glide_ticks);
        self.gain.to(if f.on { gain } else { 0.0 }, self.ramp_ticks);
        if f.on && self.fade.target < 1.0 {
            self.fade.to(1.0, self.ramp_ticks);
        }
    }

    /// Fade out over 50 ms, release the note and finish.
    pub fn stop(&mut self) {
        if !self.stopping {
            self.stopping = true;
            self.fade.to(0.0, self.fade_ticks);
        }
    }

    /// Adds into `l` and `r` (same length, at most `MAX_BLOCK`). Never allocates.
    pub fn render_add(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        if self.finished {
            return;
        }
        let mut at = 0;
        while at < n {
            let len = TICK.min(n - at);
            self.synth.pitch_bend(self.bend.tick());
            self.synth.modulation(self.wheel.tick());
            let g0 = self.level;
            let g1 = self.gain.tick() * self.fade.tick();
            self.l[..len].fill(0.0);
            self.r[..len].fill(0.0);
            self.synth
                .render_add(&mut self.l[..len], &mut self.r[..len]);
            for j in 0..len {
                let g = (g0 + (g1 - g0) * (j + 1) as f32 / len as f32) * GAME_ENGINES_GAIN;
                l[at + j] += self.l[j] * g;
                r[at + j] += self.r[j] * g;
            }
            self.level = g1;
            at += len;
        }
        self.since_frame = self.since_frame.saturating_add(n);
        if self.since_frame >= self.watchdog && self.fade.target > 0.0 {
            self.fade.to(0.0, self.fade_ticks);
        }
        let silent = self.level == 0.0 && self.gain.settled() && self.fade.settled();
        if self.held && silent && (!self.want_on || self.fade.target == 0.0) {
            self.synth.note_off(GAME_NOTE);
            self.held = false;
        }
        if self.stopping && self.fade.settled() {
            self.synth.all_notes_off();
            self.held = false;
            self.finished = true;
        }
    }
}
```

Apply this Edit, then run the line-ending check (expected `CRLF`):

Edit `launchpad-drummer/crates/lpd-core/src/synth/mod.rs`. Replace:

```rust
pub mod factory;
```

with:

```rust
pub mod factory;
pub mod game;
```

- [ ] **Step 4: Run the tests to see them pass.**

```powershell
cargo test -p lpd-core --test synth_game
```

Expected: `test result: ok. 8 passed; 0 failed` (`legato_and_a_24_semitone_bend_are_forced_and_the_rest_of_the_patch_is_kept`, `silent_until_the_first_frame_then_the_note_fades_in_over_25_ms`, `bend_and_wheel_glide_over_40_ms_and_the_gain_ramps_over_25_ms_in_16_sample_ticks`, `the_watchdog_fades_out_after_250_ms_without_frames_and_a_new_frame_restarts_the_note`, `on_false_fades_out_over_one_ramp_and_releases_the_note`, `nan_and_out_of_range_values_are_clamped_or_ignored`, `stop_fades_out_over_50_ms_then_finishes_and_ignores_later_frames`, `glides_take_40_ms_at_44_1_khz_too`).

- [ ] **Step 5: Format check.** `cargo fmt --all -- --check` → no output.

### Task 18: Engine commands, master mix and the bake scenario

**Purpose.** The audio engine plays the game voice (spec 5.1; fix 7; S20). `Command::GameSynthLoad`, `GameSynthFrame` and `GameSynthClear`; a replaced or cleared voice returns through `Trash::Game` (leaked with `mem::forget` if the queue is full); the voice renders into `game_l` / `game_r` and joins the master bus before the master gain and the soft clipper. Three `audio_engine` tests (the voice on the master bus obeying the master gain; cleared and replaced voices through the trash; a start and stop leaving the Keys synth untouched) and one `no_alloc` test (the game voice paths never allocate or free). `bake_engine` gains `scenario --patch <file> --frames <json> --out <wav>`, which plays a 40 Hz frame log through the real `GameEngineVoice` (the LIVE audition in Task 23 uses it).

**Files:**
- Modify: `launchpad-drummer/crates/lpd-core/tests/audio_engine.rs` (CRLF), `launchpad-drummer/crates/lpd-core/tests/no_alloc.rs` (CRLF), `launchpad-drummer/crates/lpd-core/src/audio/engine.rs` (CRLF), `launchpad-drummer/crates/lpd-core/examples/bake_engine.rs` (LF, from Task 4)

- [ ] **Step 1: Write the failing tests.** Apply these Edits, then run the line-ending check on both files (expected `CRLF`).

Edit `launchpad-drummer/crates/lpd-core/tests/audio_engine.rs`. Replace:

```rust
    assert!((ratio - 2f32.powf(2.0 / 12.0)).abs() < 0.01, "{ratio}");
}
```

with:

```rust
    assert!((ratio - 2f32.powf(2.0 / 12.0)).abs() < 0.01, "{ratio}");
}

fn game_voice() -> Box<lpd_core::synth::game::GameEngineVoice> {
    let mut p = lpd_core::synth::params::SynthParams::default();
    p.sub.on = true;
    lpd_core::synth::game::GameEngineVoice::new(SR, &p)
}

fn game_frame(engine: &mut Engine) {
    engine.handle(Command::GameSynthFrame {
        on: true,
        bend: 0.0,
        wheel: 0.5,
        gain: 1.0,
    });
}

#[test]
fn the_game_voice_plays_on_the_master_bus_and_obeys_the_master_gain() {
    let (mut engine, _) = Engine::new(SR);
    for group in 0..8 {
        engine.handle(Command::SetGroup {
            group,
            params: GroupParams {
                mute: true,
                ..Default::default()
            },
        });
    }
    engine.handle(Command::GameSynthLoad(game_voice()));
    assert_eq!(
        peak(&render(&mut engine, 2400)),
        0.0,
        "silent until the first frame"
    );
    game_frame(&mut engine);
    assert!(
        peak(&render(&mut engine, 4800)) > 0.01,
        "on the master even with every channel muted"
    );
    engine.handle(Command::SetMasterGain(0.0));
    game_frame(&mut engine);
    assert_eq!(peak(&render(&mut engine, 2400)), 0.0);
}

#[test]
fn a_cleared_or_replaced_game_voice_returns_through_the_trash() {
    let (mut engine, _) = Engine::new(SR);
    let mut trash = engine.take_trash().unwrap();
    engine.handle(Command::GameSynthLoad(game_voice()));
    engine.handle(Command::GameSynthLoad(game_voice()));
    assert!(
        matches!(trash.pop(), Ok(Trash::Game(_))),
        "the replaced voice came back"
    );
    game_frame(&mut engine);
    render(&mut engine, 4800);
    engine.handle(Command::GameSynthClear);
    assert!(engine.game_voice().is_some());
    let fade = render(&mut engine, 2400 + 1024);
    assert!(engine.game_voice().is_none(), "gone after the 50 ms fade");
    assert!(matches!(trash.pop(), Ok(Trash::Game(_))));
    assert_eq!(peak(&fade[2 * (2400 + 64)..]), 0.0);
    // Frames without a voice are ignored.
    game_frame(&mut engine);
    assert_eq!(peak(&render(&mut engine, 2400)), 0.0);
}

#[test]
fn a_game_voice_start_and_stop_leaves_the_keys_synth_untouched() {
    let run = |game: bool| {
        let (mut engine, _) = Engine::new(SR);
        engine.handle(Command::KeysNoteOn {
            note: 60,
            velocity: 110,
        });
        if game {
            engine.handle(Command::GameSynthLoad(game_voice()));
            game_frame(&mut engine);
        }
        render(&mut engine, 4800);
        if game {
            engine.handle(Command::GameSynthClear);
        }
        render(&mut engine, 4800);
        render(&mut engine, 4800)
    };
    assert_eq!(
        run(true),
        run(false),
        "the same keys output once the game voice has left"
    );
}
```

Edit `launchpad-drummer/crates/lpd-core/tests/no_alloc.rs`. Replace:

```rust
            engine.handle(Command::SetInstrument(slot.take().unwrap()));
        }
    });
}
```

with:

```rust
            engine.handle(Command::SetInstrument(slot.take().unwrap()));
        }
    });
}

#[test]
fn game_voice_paths_never_allocate_or_free() {
    use lpd_core::synth::{game::GameEngineVoice, params::SynthParams};
    let (mut engine, _) = Engine::new(48_000);
    let mut trash = engine.take_trash().unwrap();
    // Built before the guard, as the host does.
    let first = GameEngineVoice::new(48_000, &SynthParams::default());
    let second = GameEngineVoice::new(48_000, &SynthParams::default());
    let mut out = [0.0; 2048];
    assert_no_alloc(|| {
        engine.handle(Command::GameSynthLoad(first));
        engine.handle(Command::GameSynthLoad(second));
        for k in 0..40 {
            engine.handle(Command::GameSynthFrame {
                on: k < 30,
                bend: k as f32 / 40.0,
                wheel: 0.5,
                gain: 0.8,
            });
            engine.render(&mut out);
        }
        engine.handle(Command::GameSynthFrame {
            on: true,
            bend: f32::NAN,
            wheel: 9.0,
            gain: -1.0,
        });
        engine.render(&mut out);
        engine.handle(Command::GameSynthClear);
        for _ in 0..4 {
            engine.render(&mut out);
        }
    });
    // Both voices came back, to be dropped here, off the audio thread.
    assert!(matches!(trash.pop(), Ok(Trash::Game(_))));
    assert!(matches!(trash.pop(), Ok(Trash::Game(_))));
}
```

- [ ] **Step 2: Run them to see them fail.**

```powershell
cargo test -p lpd-core --test audio_engine
```

Expected: compile errors: `Command` has no variant `GameSynthLoad`, `GameSynthFrame` or `GameSynthClear` yet.

- [ ] **Step 3: Add the commands and the master mix.** Apply these Edits, then run the line-ending check (expected `CRLF`).

Edit `launchpad-drummer/crates/lpd-core/src/audio/engine.rs` (1 of 8). Replace:

```rust
use crate::synth::{
```

with:

```rust
use crate::synth::{
    game::{GameEngineVoice, GameFrame},
```

Edit `launchpad-drummer/crates/lpd-core/src/audio/engine.rs` (2 of 8). Replace:

```rust
    SetInstrument(Box<dyn Instrument>),
```

with:

```rust
    SetInstrument(Box<dyn Instrument>),
    /// Dustline's live ship engine voice, built off the audio thread. A voice it replaces goes back through the trash.
    GameSynthLoad(Box<GameEngineVoice>),
    /// One game frame (one per engine control step); bend and wheel glide to it over 40 ms, the gain
    /// over 25 ms.
    GameSynthFrame {
        on: bool,
        bend: f32,
        wheel: f32,
        gain: f32,
    },
    /// Fade the game voice out over 50 ms; it then goes back through the trash.
    GameSynthClear,
```

Edit `launchpad-drummer/crates/lpd-core/src/audio/engine.rs` (3 of 8). Replace:

```rust
    Params(Box<SynthParams>),
```

with:

```rust
    Params(Box<SynthParams>),
    Game(Box<GameEngineVoice>),
```

Edit `launchpad-drummer/crates/lpd-core/src/audio/engine.rs` (4 of 8). Replace:

```rust
    keys_r: Vec<f32>,
```

with:

```rust
    keys_r: Vec<f32>,
    game: Option<Box<GameEngineVoice>>,
    game_l: Vec<f32>,
    game_r: Vec<f32>,
```

Edit `launchpad-drummer/crates/lpd-core/src/audio/engine.rs` (5 of 8). Replace:

```rust
            keys_r: vec![0.0; MAX_BLOCK],
```

with:

```rust
            keys_r: vec![0.0; MAX_BLOCK],
            game: None,
            game_l: vec![0.0; MAX_BLOCK],
            game_r: vec![0.0; MAX_BLOCK],
```

Edit `launchpad-drummer/crates/lpd-core/src/audio/engine.rs` (6 of 8). Replace:

```rust
                    // Nobody is collecting: leak it rather than free it on the audio thread.
                    std::mem::forget(old);
                }
            }
```

with:

```rust
                    // Nobody is collecting: leak it rather than free it on the audio thread.
                    std::mem::forget(old);
                }
            }
            Command::GameSynthLoad(voice) => {
                if let Some(old) = self.game.replace(voice) {
                    self.trash_game(old);
                }
            }
            Command::GameSynthFrame {
                on,
                bend,
                wheel,
                gain,
            } => {
                if let Some(game) = self.game.as_deref_mut() {
                    game.frame(GameFrame {
                        on,
                        bend,
                        wheel,
                        gain,
                    });
                }
            }
            Command::GameSynthClear => {
                if let Some(game) = self.game.as_deref_mut() {
                    game.stop();
                }
            }
```

Edit `launchpad-drummer/crates/lpd-core/src/audio/engine.rs` (7 of 8). Replace:

```rust
                self.audition_attack[slot] = 0;
            }
        }
```

with:

```rust
                self.audition_attack[slot] = 0;
            }
        }
    }

    /// A game voice leaves the audio thread through the trash; with nobody collecting, it is leaked, never freed here.
    fn trash_game(&mut self, voice: Box<GameEngineVoice>) {
        if let Err(rtrb::PushError::Full(old)) = self.trash.push(Trash::Game(voice)) {
            std::mem::forget(old);
        }
    }

    /// The game voice is playing (tests and status).
    pub fn game_voice(&self) -> Option<&GameEngineVoice> {
        self.game.as_deref()
```

Edit `launchpad-drummer/crates/lpd-core/src/audio/engine.rs` (8 of 8). Replace:

```rust
                out[2 * i + 1] += self.keys_r[i] * self.keys_gain;
            }
        }
```

with:

```rust
                out[2 * i + 1] += self.keys_r[i] * self.keys_gain;
            }
        }
        if let Some(game) = self.game.as_deref_mut() {
            // The game voice joins the master bus too, before master gain and the clipper; the game sets its level.
            self.game_l[..n].fill(0.0);
            self.game_r[..n].fill(0.0);
            game.render_add(&mut self.game_l[..n], &mut self.game_r[..n]);
            for i in 0..n {
                out[2 * i] += self.game_l[i];
                out[2 * i + 1] += self.game_r[i];
            }
            if game.finished() {
                if let Some(old) = self.game.take() {
                    self.trash_game(old);
                }
            }
        }
```

- [ ] **Step 4: Run the engine tests to see them pass.**

```powershell
cargo test -p lpd-core --test audio_engine
cargo test -p lpd-core --test no_alloc
```

Expected: `test result: ok. 29 passed; 0 failed`, then `test result: ok. 6 passed; 0 failed`.

- [ ] **Step 5: Add the scenario mode to the bake tool.** Apply these Edits, then run the line-ending check (expected `LF`).

Edit `launchpad-drummer/crates/lpd-core/examples/bake_engine.rs` (1 of 4). Replace:

```rust
//!
//! `check` parses the patch the way the app does (`Patch::from_json`) and fails if any value had to be clamped.
//! `loops` renders the eight phase-locked tone loops (4 anchors x on/off load) as 32-bit float stereo WAV files at
//! 48 kHz, 100800 samples each: 0.05 s lead-in, the 2 s loop, then 0.05 s of what follows the loop end, and
//! writes `bake.json` beside them.

use anyhow::{bail, Context, Result};
use lpd_core::instrument::Instrument;
```

with:

```rust
//!   cargo run -p lpd-core --release --example bake_engine -- scenario --patch <file.lbpatch> --frames <frames.json> --out <file.wav>
//!
//! `check` parses the patch the way the app does (`Patch::from_json`) and fails if any value had to be clamped.
//! `loops` renders the eight phase-locked tone loops (4 anchors x on/off load) as 32-bit float stereo WAV files at
//! 48 kHz, 100800 samples each: 0.05 s lead-in, the 2 s loop, then 0.05 s of what follows the loop end, and
//! writes `bake.json` beside them. `scenario` plays a recorded 40 Hz game frame log through `GameEngineVoice`, the
//! LIVE SYNTH voice, so LIVE can be auditioned offline.

use anyhow::{bail, Context, Result};
use lpd_core::instrument::Instrument;
use lpd_core::synth::game::{
    GameEngineVoice, GameFrame, GAME_BEND_RANGE, GAME_NOTE, GAME_VELOCITY,
};
```

Edit `launchpad-drummer/crates/lpd-core/examples/bake_engine.rs` (2 of 4). Replace:

```rust
const RATE: u32 = 48_000;
/// The held note, its velocity and the forced bend range (the LIVE SYNTH voice uses the same, spec 5.1).
const GAME_NOTE: u8 = 48;
const GAME_VELOCITY: u8 = 127;
const GAME_BEND_RANGE: u8 = 24;
```

with:

```rust
const RATE: u32 = 48_000;
```

Edit `launchpad-drummer/crates/lpd-core/examples/bake_engine.rs` (3 of 4). Replace:

```rust
fn main() -> Result<()> {
```

with:

```rust
/// Frames: `{"rate": 40, "frames": [[on, bend, wheel, gain], ...]}`, one every 1/40 s from t = 0.
fn scenario(args: &[String]) -> Result<()> {
    let (_, params) = load_patch(Path::new(&need(args, "--patch")?))?;
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(need(args, "--frames")?)?)?;
    if doc["rate"].as_u64() != Some(40) {
        bail!("frames must be at 40 Hz");
    }
    let frames = doc["frames"].as_array().context("no frames")?;
    let per = RATE as usize / 40;
    let mut voice = GameEngineVoice::new(RATE, &params);
    let (mut l, mut r) = (
        vec![0.0f32; frames.len() * per + RATE as usize / 2],
        vec![0.0f32; frames.len() * per + RATE as usize / 2],
    );
    for (i, f) in frames.iter().enumerate() {
        let v = |k: usize| f[k].as_f64().unwrap_or(f64::NAN) as f32;
        voice.frame(GameFrame {
            on: f[0].as_bool().unwrap_or(false),
            bend: v(1),
            wheel: v(2),
            gain: v(3),
        });
        voice.render_add(
            &mut l[i * per..(i + 1) * per],
            &mut r[i * per..(i + 1) * per],
        );
    }
    let tail = frames.len() * per;
    voice.render_add(&mut l[tail..], &mut r[tail..]);
    let out = PathBuf::from(need(args, "--out")?);
    write_wav(&out, &l, &r)?;
    println!(
        "wrote {} ({} frames, {:.2} s)",
        out.display(),
        frames.len(),
        l.len() as f64 / f64::from(RATE)
    );
    Ok(())
}

fn main() -> Result<()> {
```

Edit `launchpad-drummer/crates/lpd-core/examples/bake_engine.rs` (4 of 4). Replace:

```rust
        _ => bail!("usage: bake_engine check|loops --patch <file> [--out <dir>]"),
```

with:

```rust
        Some("scenario") => scenario(&args),
        _ => bail!("usage: bake_engine check|loops|scenario --patch <file> [--out <dir|file>] [--frames <json>]"),
```

- [ ] **Step 6: Run the bake tool's tests and the format check.**

```powershell
cargo test -p lpd-core --example bake_engine
cargo fmt --all -- --check
```

Expected: `test result: ok. 5 passed; 0 failed`, then no output.

### Task 19: App Rust layer

**Purpose.** The router, the app state and the Tauri commands for LIVE SYNTH (spec 5.2; fix 17; S21, S22, S46). The router keeps the `GameSpec` and the frame count and rebuilds the voice after a stream restart (only for the same start); `AppState::game_engine_start` prefers the user's `DUSTLINE KESTREL.lbpatch`, refuses patches over 16384 bytes or that are not patches, and builds the voice with no lock held; `game_engine_frame`, `game_engine_stop` and `game_engine_status` pass through; `restart_game_engine()` runs in `install_audio`. Four tests: start, feed and stop through the engine queue; a restarted stream keeps the patch and takes a rebuilt voice only for the same start; the user's patch wins; a too-large or invalid patch is refused.

**Files:**
- Modify: `launchpad-drummer/src-tauri/src/router.rs` (CRLF), `launchpad-drummer/src-tauri/src/state.rs` (CRLF), `launchpad-drummer/src-tauri/src/commands.rs` (CRLF), `launchpad-drummer/src-tauri/src/lib.rs` (CRLF)

- [ ] **Step 1: Write the failing tests.** Apply these two Edits (each adds tests at the start of the file's `#[cfg(test)]` module), then run the line-ending check on both files (expected `CRLF`).

Edit `launchpad-drummer/src-tauri/src/router.rs`. Replace:

```rust
        v
    }

    #[test]
```

with:

```rust
        v
    }

    fn game_voice() -> Box<GameEngineVoice> {
        GameEngineVoice::new(48_000, &SynthParams::default())
    }

    #[test]
    fn the_game_engine_voice_is_started_fed_and_stopped_through_the_engine_queue() {
        let (mut router, mut rx) = router_with_pad(0.0, 0.0);
        drain(&mut rx);
        assert!(
            !router.game_engine_frame(true, 0.0, 0.5, 0.5),
            "no frame before a start"
        );
        let generation = router.game_engine_start(
            SynthParams::default(),
            "game",
            "KESTREL ENGINE".into(),
            game_voice(),
        );
        assert_eq!(generation, 1);
        assert!(router.game_engine_frame(true, 2.0, -1.0, 0.5));
        assert!(!router.game_engine_frame(true, f32::NAN, 0.5, 0.5));
        assert_eq!(router.game_engine_status(48_000).frames, 1);
        router.game_engine_stop();
        let sent = drain(&mut rx);
        assert!(
            matches!(
                sent.as_slice(),
                [
                    Command::GameSynthLoad(_),
                    Command::GameSynthFrame { on: true, bend, wheel, gain },
                    Command::GameSynthClear
                ] if *bend == 1.0 && *wheel == 0.0 && *gain == 0.5
            ),
            "{sent:?}"
        );
        assert_eq!(
            router.game_engine_status(44_100),
            GameEngineStatus {
                running: false,
                frames: 1,
                source: String::new(),
                patch: String::new(),
                rate: 44_100,
            }
        );
        router.game_engine_stop();
        assert!(drain(&mut rx).is_empty(), "a second stop sends nothing");
    }

    #[test]
    fn a_restarted_stream_keeps_the_game_patch_and_takes_a_voice_rebuilt_for_the_same_start_only() {
        let (mut router, _old_rx) = router_with_pad(0.0, 0.0);
        let first = router.game_engine_start(
            SynthParams::default(),
            "user",
            "DUSTLINE KESTREL".into(),
            game_voice(),
        );
        let (new_tx, mut new_rx) = RingBuffer::new(1024);
        let (_evt_tx, new_events) = RingBuffer::new(16);
        router.replace_audio(new_tx, new_events);
        assert_eq!(
            router
                .game_spec()
                .map(|g| (g.generation, g.source, g.name.as_str())),
            Some((first, "user", "DUSTLINE KESTREL"))
        );
        assert!(
            !drain(&mut new_rx)
                .iter()
                .any(|c| matches!(c, Command::GameSynthLoad(_))),
            "replace_audio sends no voice itself"
        );
        assert!(!router.game_engine_rebuild(first + 1, game_voice()));
        assert!(router.game_engine_rebuild(first, game_voice()));
        assert!(matches!(
            drain(&mut new_rx).as_slice(),
            [Command::GameSynthLoad(_)]
        ));
        router.game_engine_stop();
        assert!(
            !router.game_engine_rebuild(first, game_voice()),
            "a stopped start takes no rebuild"
        );
    }

    #[test]
```

Edit `launchpad-drummer/src-tauri/src/state.rs`. Replace:

```rust
        assert_eq!(set.keys.patch.params.filter.cutoff, 300.0);
    }

    #[test]
```

with:

```rust
        assert_eq!(set.keys.patch.params.filter.cutoff, 300.0);
    }

    #[test]
    fn the_game_engine_prefers_the_users_dustline_kestrel_patch() {
        let (state, mut rx) = app();
        let dir = tempfile::tempdir().unwrap();
        let mut game = factory("moogBass").unwrap();
        game.name = "KESTREL ENGINE".into();
        let json = game.to_json();
        let status = state.game_engine_start(&json, Some(dir.path())).unwrap();
        assert_eq!(
            (
                status.running,
                status.source.as_str(),
                status.patch.as_str()
            ),
            (true, "game", "KESTREL ENGINE")
        );
        let mut user = game.clone();
        user.name = GAME_PATCH_NAME.into();
        user.params.filter.cutoff = 777.0;
        patches::write(dir.path(), &user, false).unwrap();
        let status = state.game_engine_start(&json, Some(dir.path())).unwrap();
        assert_eq!(
            (status.source.as_str(), status.patch.as_str()),
            ("user", "DUSTLINE KESTREL")
        );
        assert!(std::iter::from_fn(|| rx.pop().ok()).any(
            |c| matches!(c, Command::GameSynthLoad(ref v) if v.params().filter.cutoff == 777.0)
        ));
        assert!(state.game_engine_frame(true, 0.1, 0.5, 0.5));
        assert_eq!(state.game_engine_status().frames, 1);
        state.game_engine_stop();
        assert!(!state.game_engine_status().running);
        // The user's keys patch is never touched.
        assert_eq!(state.get_current_set().keys.patch, Set::new().keys.patch);
    }

    #[test]
    fn a_game_engine_patch_that_is_too_large_or_not_a_patch_is_refused() {
        let (state, _rx) = app();
        let big = "x".repeat(GAME_PATCH_MAX + 1);
        assert!(state
            .game_engine_start(&big, None)
            .unwrap_err()
            .starts_with("Engine patch too large"));
        assert!(state
            .game_engine_start("not json", None)
            .unwrap_err()
            .starts_with("Engine patch not loaded"));
        assert!(!state.game_engine_status().running);
    }

    #[test]
```

- [ ] **Step 2: Run them to see them fail.**

```powershell
cargo test -p launchpad-drummer --lib game_
```

Expected: compile errors: no method named `game_engine_start` (and the other new names) yet.

- [ ] **Step 3: Router.** Apply these Edits to `router.rs`:

Edit `launchpad-drummer/src-tauri/src/router.rs` (1 of 5). Replace:

```rust
use lpd_core::synth::params::{ParamId, Route};
```

with:

```rust
use lpd_core::synth::game::GameEngineVoice;
use lpd_core::synth::params::{ParamId, Route, SynthParams};
```

Edit `launchpad-drummer/src-tauri/src/router.rs` (2 of 5). Replace:

```rust
pub struct Router {
```

with:

```rust
/// The game patch LIVE SYNTH plays (ship engine spec 5.2). The router keeps it because a restarted
/// output stream needs a new voice for its sample rate, and `replace_audio` resends only the mixer,
/// keys, patch and wheels.
#[derive(Debug, Clone, PartialEq)]
pub struct GameSpec {
    pub params: SynthParams,
    /// "user": the user's `DUSTLINE KESTREL.lbpatch`; "game": the patch the game sent.
    pub source: &'static str,
    pub name: String,
    /// Bumped by every start; a voice rebuilt for an older start is dropped.
    pub generation: u64,
}

/// What `game_engine_status` reports (the native smoke test reads `frames`).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameEngineStatus {
    pub running: bool,
    pub frames: u64,
    pub source: String,
    pub patch: String,
    pub rate: u32,
}

pub struct Router {
```

Edit `launchpad-drummer/src-tauri/src/router.rs` (3 of 5). Replace:

```rust
    trash: Option<Consumer<Trash>>,
```

with:

```rust
    trash: Option<Consumer<Trash>>,
    /// LIVE SYNTH: the started game patch, the start counter and the frames since the start.
    game: Option<GameSpec>,
    game_generation: u64,
    game_frames: u64,
```

Edit `launchpad-drummer/src-tauri/src/router.rs` (4 of 5). Replace:

```rust
            trash: None,
```

with:

```rust
            trash: None,
            game: None,
            game_generation: 0,
            game_frames: 0,
```

Edit `launchpad-drummer/src-tauri/src/router.rs` (5 of 5). Replace:

```rust
        self.send_mixer();
        self.send_keys();
        self.send_patch();
        self.send_wheels();
```

with:

```rust
        self.send_mixer();
        self.send_keys();
        self.send_patch();
        self.send_wheels();
    }

    /// LIVE SYNTH start: keep the patch for stream restarts and hand the voice (built off the audio
    /// thread for the current rate) to the engine. Returns the start's generation.
    pub fn game_engine_start(
        &mut self,
        params: SynthParams,
        source: &'static str,
        name: String,
        voice: Box<GameEngineVoice>,
    ) -> u64 {
        self.game_generation += 1;
        self.game = Some(GameSpec {
            params,
            source,
            name,
            generation: self.game_generation,
        });
        self.game_frames = 0;
        // A full queue hands the box back; it is dropped here, on the router thread.
        let _ = self.audio_tx.push(Command::GameSynthLoad(voice));
        self.game_generation
    }

    /// One 40 Hz game frame. Nothing is sent (false) before a start or with a non-finite value; the
    /// rest is clamped.
    pub fn game_engine_frame(&mut self, on: bool, bend: f32, wheel: f32, gain: f32) -> bool {
        if self.game.is_none() || !(bend.is_finite() && wheel.is_finite() && gain.is_finite()) {
            return false;
        }
        self.game_frames += 1;
        let _ = self.audio_tx.push(Command::GameSynthFrame {
            on,
            bend: bend.clamp(-1.0, 1.0),
            wheel: wheel.clamp(0.0, 1.0),
            gain: gain.clamp(0.0, 1.0),
        });
        true
    }

    /// Fade the game voice out and forget its patch.
    pub fn game_engine_stop(&mut self) {
        if self.game.take().is_some() {
            let _ = self.audio_tx.push(Command::GameSynthClear);
        }
    }

    /// The started game patch, if any.
    pub fn game_spec(&self) -> Option<&GameSpec> {
        self.game.as_ref()
    }

    /// After a stream restart: a voice rebuilt for the new rate goes to the engine only while the
    /// start it was built for is still the running one.
    pub fn game_engine_rebuild(&mut self, generation: u64, voice: Box<GameEngineVoice>) -> bool {
        if self.game.as_ref().map(|g| g.generation) != Some(generation) {
            return false;
        }
        let _ = self.audio_tx.push(Command::GameSynthLoad(voice));
        true
    }

    pub fn game_engine_status(&self, rate: u32) -> GameEngineStatus {
        GameEngineStatus {
            running: self.game.is_some(),
            frames: self.game_frames,
            source: self.game.as_ref().map_or("", |g| g.source).into(),
            patch: self
                .game
                .as_ref()
                .map(|g| g.name.clone())
                .unwrap_or_default(),
            rate,
        }
```

- [ ] **Step 4: App state.** Apply these Edits to `state.rs` (lock order 0.2 item 7: the voice is built with no lock held):

Edit `launchpad-drummer/src-tauri/src/state.rs` (1 of 5). Replace:

```rust
use crate::router::{LearnTarget, Router, UiEvent};
```

with:

```rust
use crate::router::{GameEngineStatus, LearnTarget, Router, UiEvent};
```

Edit `launchpad-drummer/src-tauri/src/state.rs` (2 of 5). Replace:

```rust
    params::{ParamId, Route, ROUTES},
    patch::{clean_name, Patch, PatchSource},
```

with:

```rust
    game::GameEngineVoice,
    params::{ParamId, Route, ROUTES},
    patch::{clean_name, file_name, Patch, PatchSource},
```

Edit `launchpad-drummer/src-tauri/src/state.rs` (3 of 5). Replace:

```rust
#[derive(serde::Deserialize, Debug, Clone, PartialEq)]
```

with:

```rust
/// LIVE SYNTH (ship engine spec 5.2): a user patch saved on PLAY under this name replaces the game's.
pub const GAME_PATCH_NAME: &str = "DUSTLINE KESTREL";
/// The largest patch text the game may send.
pub const GAME_PATCH_MAX: usize = 16384;

#[derive(serde::Deserialize, Debug, Clone, PartialEq)]
```

Edit `launchpad-drummer/src-tauri/src/state.rs` (4 of 5). Replace:

```rust
        self.set_audio_stats(stats);
```

with:

```rust
        self.set_audio_stats(stats);
        // After the status: the voice is rebuilt for the new stream's rate.
        self.restart_game_engine();
```

Edit `launchpad-drummer/src-tauri/src/state.rs` (5 of 5). Replace:

```rust
        lock(&self.router).set_trash(trash);
```

with:

```rust
        lock(&self.router).set_trash(trash);
    }

    /// LIVE SYNTH start (ship engine spec 5.2). The user's `DUSTLINE KESTREL.lbpatch` in `dir` wins
    /// over the patch the game sent, so a tweak saved on PLAY is heard without a rebuild. The voice is
    /// built here, with no lock held and off the audio thread, for the current output rate. Slow:
    /// call it off the UI thread.
    pub fn game_engine_start(
        &self,
        game_patch: &str,
        dir: Option<&Path>,
    ) -> Result<GameEngineStatus, String> {
        if game_patch.len() > GAME_PATCH_MAX {
            return Err(format!(
                "Engine patch too large ({} bytes, at most {GAME_PATCH_MAX})",
                game_patch.len()
            ));
        }
        let user = dir.and_then(|d| patches::read(d, &file_name(GAME_PATCH_NAME)).ok());
        let (patch, source) = match user {
            Some(p) => (p, "user"),
            None => (
                Patch::from_json(game_patch)
                    .map_err(|e| format!("Engine patch not loaded — {e}"))?,
                "game",
            ),
        };
        let rate = self.sample_rate();
        let voice = GameEngineVoice::new(rate, &patch.params);
        let mut r = lock(&self.router);
        r.game_engine_start(patch.params, source, patch.name, voice);
        Ok(r.game_engine_status(rate))
    }

    /// One 40 Hz frame from the game (false: no voice started, or a non-finite value).
    pub fn game_engine_frame(&self, on: bool, bend: f32, wheel: f32, gain: f32) -> bool {
        lock(&self.router).game_engine_frame(on, bend, wheel, gain)
    }

    pub fn game_engine_stop(&self) {
        lock(&self.router).game_engine_stop();
    }

    pub fn game_engine_status(&self) -> GameEngineStatus {
        let rate = self.sample_rate();
        lock(&self.router).game_engine_status(rate)
    }

    /// A new output stream: the started game patch gets a voice for the new rate, built with no lock
    /// held. A start or stop in between wins (the generation no longer matches).
    fn restart_game_engine(&self) {
        let Some(spec) = lock(&self.router).game_spec().cloned() else {
            return;
        };
        let voice = GameEngineVoice::new(self.sample_rate(), &spec.params);
        lock(&self.router).game_engine_rebuild(spec.generation, voice);
```

- [ ] **Step 5: Commands and registration.** Apply these Edits, then run the line-ending check on all four files (expected `CRLF`):

Edit `launchpad-drummer/src-tauri/src/commands.rs` (1 of 2). Replace:

```rust
use crate::router::LearnTarget;
```

with:

```rust
use crate::router::{GameEngineStatus, LearnTarget};
```

Edit `launchpad-drummer/src-tauri/src/commands.rs` (2 of 2). Replace:

```rust
    state.keys_mod(value);
```

with:

```rust
    state.keys_mod(value);
}

/// LIVE SYNTH (ship engine spec 5.2): build Dustline's engine voice from `patch` (the game's
/// `.lbpatch` JSON) or the user's `DUSTLINE KESTREL.lbpatch`, off the command thread.
#[tauri::command]
pub(crate) async fn game_engine_start(
    patch: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<GameEngineStatus, String> {
    let dir = patches_dir(&app).ok();
    worker(&state, move |s| s.game_engine_start(&patch, dir.as_deref())).await
}

/// One frame of the game engine voice (one per engine control step, about 40 Hz: 30 Hz at a 60 Hz
/// display); values are clamped, non-finite frames dropped.
#[tauri::command]
pub(crate) fn game_engine_frame(
    on: bool,
    bend: f32,
    wheel: f32,
    gain: f32,
    state: State<'_, AppState>,
) -> bool {
    state.game_engine_frame(on, bend, wheel, gain)
}

#[tauri::command]
pub(crate) fn game_engine_stop(state: State<'_, AppState>) {
    state.game_engine_stop();
}

/// Running state and frames received since the start (the native smoke test reads it).
#[tauri::command]
pub(crate) fn game_engine_status(state: State<'_, AppState>) -> GameEngineStatus {
    state.game_engine_status()
```

Edit `launchpad-drummer/src-tauri/src/lib.rs`. Replace:

```rust
            commands::keys_mod,
```

with:

```rust
            commands::keys_mod,
            commands::game_engine_start,
            commands::game_engine_frame,
            commands::game_engine_stop,
            commands::game_engine_status,
```

- [ ] **Step 6: Run the tests to see them pass.**

```powershell
cargo test -p launchpad-drummer --lib game_
cargo test --workspace
cargo fmt --all -- --check
```

Expected: the four new tests pass (`the_game_engine_voice_is_started_fed_and_stopped_through_the_engine_queue`, `a_restarted_stream_keeps_the_game_patch_and_takes_a_voice_rebuilt_for_the_same_start_only`, `the_game_engine_prefers_the_users_dustline_kestrel_patch`, `a_game_engine_patch_that_is_too_large_or_not_a_patch_is_refused`); the whole workspace passes (the app library: 154 tests, 150 before plus these 4; `lpd-core` with `audio_engine` 29, `synth_game` 8, `no_alloc` 6; the `bake_engine` example 5); no format output. Clippy is not a gate (`lpd-core` had 6 clippy errors before this work).

### Task 20: `engineBridge.ts`

**Purpose.** The app side of the game's messages (spec 5.2; S23): `parseEngineMessage` keeps only well-formed `{lpd: 'engine-synth', op}` messages and clamps their values; `rateCap` allows a burst of 10, then 60 per second; `EngineLink` accepts messages only when the caller says they came from the active Dustline frame, starts the voice and replies `ready`, `unavailable` (outside the app) or `error`, drops frames before ready, stops and forgets the voice when Dustline is left, stops it while the window is hidden until the next sounding frame, and handles a stop that arrives while the voice is being built. Nine `node --test` tests.

**Files:**
- Create: `launchpad-drummer/src/lib/engineBridge.test.ts`, `launchpad-drummer/src/lib/engineBridge.ts`

- [ ] **Step 1: Write the failing tests.** Create `launchpad-drummer/src/lib/engineBridge.test.ts`:

```ts
import assert from 'node:assert/strict';
import test from 'node:test';
import { ENGINE_PATCH_MAX, EngineLink, parseEngineMessage, rateCap, type EngineHost, type EngineStatus } from './engineBridge.ts';

const frame = (over: Record<string, unknown> = {}) => ({ lpd: 'engine-synth', op: 'frame', on: true, bend: .1, wheel: .5, gain: .8, ...over });
const start = { lpd: 'engine-synth', op: 'start', patch: '{"patchVersion":1}' };

function host(native = true) {
  const calls: string[] = [];
  let release: (() => void) | null = null, fail: Error | null = null, hold = false;
  const h: EngineHost & { calls: string[]; replies: EngineStatus[]; hold(): void; release(): void; fail(e: Error): void } = {
    native, calls, replies: [],
    start(patch) {
      calls.push('start ' + patch.length);
      if (fail) return Promise.reject(fail);
      return hold ? new Promise<void>(r => { release = r; }) : Promise.resolve();
    },
    frame(on, bend, wheel, gain) { calls.push(`frame ${on} ${bend} ${wheel} ${gain}`); },
    stop() { calls.push('stop'); },
    reply(state) { h.replies.push(state); },
    hold() { hold = true; },
    release() { hold = false; release?.(); },
    fail(e) { fail = e; },
  };
  return h;
}
const clock = () => { let t = 0; const now = () => t; return { now, add(ms: number) { t += ms; } }; };

test('parse: well-formed messages pass, values are clamped', () => {
  assert.deepEqual(parseEngineMessage(start), { op: 'start', patch: start.patch });
  assert.deepEqual(parseEngineMessage({ lpd: 'engine-synth', op: 'stop' }), { op: 'stop' });
  assert.deepEqual(parseEngineMessage(frame({ bend: -3, wheel: 2, gain: -1 })), { op: 'frame', on: true, bend: -1, wheel: 1, gain: 0 });
});

test('parse: malformed messages are dropped', () => {
  for (const bad of [null, 7, 'x', [], { lpd: 'leds' }, { lpd: 'engine-synth' }, { lpd: 'engine-synth', op: 'boom' },
    frame({ on: 1 }), frame({ bend: NaN }), frame({ wheel: Infinity }), frame({ gain: '1' }), frame({ bend: undefined }),
    { ...start, patch: '' }, { ...start, patch: 42 }, { ...start, patch: 'x'.repeat(ENGINE_PATCH_MAX + 1) }]) {
    assert.equal(parseEngineMessage(bad), null, JSON.stringify(bad));
  }
  assert.notEqual(parseEngineMessage({ ...start, patch: 'x'.repeat(ENGINE_PATCH_MAX) }), null);
});

test('rate cap: a burst of 10, then 60 a second; 40 Hz frames always pass', () => {
  const c = clock(), allow = rateCap(60, 10, c.now);
  for (let i = 0; i < 10; i++) assert.equal(allow(), true);
  assert.equal(allow(), false);
  c.add(100);
  let passed = 0;
  for (let i = 0; i < 10; i++) if (allow()) passed++;
  assert.equal(passed, 6);
  const d = clock(), steady = rateCap(60, 10, d.now);
  for (let i = 0; i < 400; i++) { d.add(25); assert.equal(steady(), true); }
});

test('messages from anywhere but the Dustline frame are ignored', async () => {
  const h = host(), link = new EngineLink(h);
  await link.receive(start, false);
  await link.receive(frame(), false);
  assert.deepEqual(h.calls, []);
  assert.deepEqual(h.replies, []);
});

test('start builds the voice, replies ready, then frames pass; frames before ready are dropped', async () => {
  const h = host(), link = new EngineLink(h);
  await link.receive(frame(), true);
  assert.deepEqual(h.calls, [], 'no voice yet');
  await link.receive(start, true);
  assert.deepEqual(h.replies, ['ready']);
  assert.equal(link.active, true);
  await link.receive(frame({ bend: 2 }), true);
  await link.receive(frame({ gain: NaN }), true);
  assert.deepEqual(h.calls, ['start 18', 'frame true 1 0.5 0.8']);
});

test('outside the app the game hears unavailable; a failed start replies error', async () => {
  const h = host(false), link = new EngineLink(h);
  await link.receive(start, true);
  assert.deepEqual(h.replies, ['unavailable']);
  assert.deepEqual(h.calls, []);
  const f = host(), broken = new EngineLink(f);
  f.fail(new Error('no audio output'));
  await broken.receive(start, true);
  assert.deepEqual(f.replies, ['error']);
  await broken.receive(frame(), true);
  assert.equal(broken.active, false);
});

test('auto-stop: leaving Dustline forgets the voice; a hidden window stops it until the next sounding frame', async () => {
  const h = host(), link = new EngineLink(h);
  await link.receive(start, true);
  link.stop(false);
  assert.equal(h.calls.at(-1), 'stop');
  await link.receive(frame({ on: false }), true);
  assert.equal(h.calls.at(-1), 'stop', 'a silent frame does not restart');
  await link.receive(frame(), true);
  assert.deepEqual(h.calls.slice(-1), ['start 18'], 'a sounding frame restarts the voice');
  assert.deepEqual(h.replies, ['ready', 'ready']);
  link.stop(true);
  await link.receive(frame(), true);
  assert.equal(h.calls.at(-1), 'stop', 'after leaving, frames need a new start');
  await link.receive({ lpd: 'engine-synth', op: 'stop' }, true);
  assert.equal(h.calls.filter(c => c === 'stop').length, 2, 'stop is sent once per running voice');
});

test('a stop while the voice is being built stops it as soon as it exists, with no ready reply', async () => {
  const h = host(), link = new EngineLink(h);
  h.hold();
  const pending = link.receive(start, true);
  link.stop(true);
  h.release();
  await pending;
  assert.deepEqual(h.calls, ['start 18', 'stop']);
  assert.deepEqual(h.replies, []);
  assert.equal(link.active, false);
});

test('a flood is capped before it reaches the app', async () => {
  const c = clock(), h = host(), link = new EngineLink(h, c.now);
  await link.receive(start, true);
  for (let i = 0; i < 100; i++) await link.receive(frame(), true);
  assert.equal(h.calls.filter(x => x.startsWith('frame')).length, 9, 'the burst, less the start');
});
```

- [ ] **Step 2: Run them to see them fail** (Git Bash, from `launchpad-drummer`).

```bash
node --test src/lib/engineBridge.test.ts
```

Expected: `Error [ERR_MODULE_NOT_FOUND]: Cannot find module '…\src\lib\engineBridge.ts'`.

- [ ] **Step 3: Create the bridge.** Create `launchpad-drummer/src/lib/engineBridge.ts`:

```ts
// Dustline LIVE SYNTH bridge (ship engine spec 5.2). The game frame asks for a native synth voice with
// `{lpd: 'engine-synth', op: 'start' | 'frame' | 'stop'}` messages; this module validates them, caps their rate and
// drives the app's game engine voice through an injected host, so it is tested without Tauri or a frame.

export const ENGINE_PATCH_MAX = 16384;
/** Messages per second, and how many may arrive at once (frames come at 40 Hz). */
export const ENGINE_RATE = 60;
export const ENGINE_BURST = 10;

export type EngineMessage =
  | { op: 'start'; patch: string }
  | { op: 'frame'; on: boolean; bend: number; wheel: number; gain: number }
  | { op: 'stop' };
export type EngineStatus = 'ready' | 'unavailable' | 'error';

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
const finite = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);

/** The message, cleaned, or null when it is not a well-formed engine-synth message. */
export function parseEngineMessage(data: unknown): EngineMessage | null {
  if (!data || typeof data !== 'object') return null;
  const m = data as Record<string, unknown>;
  if (m.lpd !== 'engine-synth') return null;
  if (m.op === 'stop') return { op: 'stop' };
  if (m.op === 'start') {
    return typeof m.patch === 'string' && m.patch.length > 0 && m.patch.length <= ENGINE_PATCH_MAX ? { op: 'start', patch: m.patch } : null;
  }
  if (m.op === 'frame') {
    if (typeof m.on !== 'boolean' || !finite(m.bend) || !finite(m.wheel) || !finite(m.gain)) return null;
    return { op: 'frame', on: m.on, bend: clamp(m.bend, -1, 1), wheel: clamp(m.wheel, 0, 1), gain: clamp(m.gain, 0, 1) };
  }
  return null;
}

/** A token bucket: `perSecond` tokens a second, at most `burst` saved up. Returns whether one more message may pass. */
export function rateCap(perSecond = ENGINE_RATE, burst = ENGINE_BURST, now: () => number = () => performance.now()): () => boolean {
  let tokens = burst, last = now();
  return () => {
    const t = now();
    tokens = Math.min(burst, tokens + Math.max(0, t - last) * perSecond / 1000);
    last = t;
    if (tokens < 1) return false;
    tokens -= 1;
    return true;
  };
}

export interface EngineHost {
  /** Inside the desktop app: the native voice exists. */
  native: boolean;
  /** `game_engine_start`: resolves once the voice is built and handed to the audio engine. */
  start(patch: string): Promise<unknown>;
  frame(on: boolean, bend: number, wheel: number, gain: number): void;
  stop(): void;
  /** Reply to the game frame with `{lpd: 'engine-synth-status', state, message}`. */
  reply(state: EngineStatus, message?: string): void;
}

/** Messages from the Dustline frame in, voice commands out. One per app. */
export class EngineLink {
  private host: EngineHost;
  private allow: () => boolean;
  private patch: string | null = null;
  private running = false;
  private starting = false;
  private generation = 0;

  constructor(host: EngineHost, now?: () => number) {
    this.host = host;
    this.allow = rateCap(ENGINE_RATE, ENGINE_BURST, now);
  }

  get active(): boolean {
    return this.running;
  }

  /** A message from the prototype frame; `fromDustline`: it came from the active frame and that frame shows Dustline. */
  async receive(data: unknown, fromDustline: boolean): Promise<void> {
    if (!fromDustline || !this.allow()) return;
    const m = parseEngineMessage(data);
    if (!m) return;
    if (m.op === 'stop') return this.stop(true);
    if (m.op === 'start') {
      this.patch = m.patch;
      return this.begin();
    }
    if (this.running) this.host.frame(m.on, m.bend, m.wheel, m.gain);
    // A voice the app stopped while the window was hidden comes back with the next sounding frame.
    else if (m.on && this.patch !== null && !this.starting) await this.begin();
  }

  /** Game switch, leaving GAMES, frame reload: stop and forget the patch. Window hidden: stop only (`forget` false). */
  stop(forget: boolean): void {
    this.generation++;
    this.starting = false;
    if (forget) this.patch = null;
    if (this.running) {
      this.running = false;
      this.host.stop();
    }
  }

  private async begin(): Promise<void> {
    if (!this.host.native) {
      this.host.reply('unavailable');
      return;
    }
    const patch = this.patch;
    if (patch === null) return;
    const generation = ++this.generation;
    this.starting = true;
    try {
      await this.host.start(patch);
      if (generation !== this.generation) {
        if (!this.starting) this.host.stop(); // stopped while the voice was being built
        return;
      }
      this.running = true;
      this.host.reply('ready');
    } catch (error) {
      if (generation === this.generation) this.host.reply('error', error instanceof Error ? error.message : String(error));
    } finally {
      if (generation === this.generation) this.starting = false;
    }
  }
}
```

- [ ] **Step 4: Run the tests to see them pass.**

```bash
node --test src/lib/engineBridge.test.ts
```

Expected: `# tests 9`, `# pass 9`, `# fail 0`.

### Task 21: `App.svelte`

**Purpose.** Wire the bridge into the app (spec 5.2; S23): one `EngineLink` whose host calls the four Tauri commands and replies to the game frame; messages with `lpd: 'engine-synth'` go to it with the "active Dustline frame" flag; leaving GAMES → DUSTLINE stops and forgets the voice, a hidden window stops it, and a reloaded game starts again. The behaviour is tested in Task 20; this task is checked by `svelte-check` and the test suite, and natively in Task 24.

**Files:**
- Modify: `launchpad-drummer/src/App.svelte` (CRLF)

- [ ] **Step 1: Wire the bridge.** Apply these Edits, then run the line-ending check (expected `CRLF`).

Edit `launchpad-drummer/src/App.svelte` (1 of 3). Replace:

```svelte
  import { readGameStore, setGameStore } from './lib/gameStore.ts';
```

with:

```svelte
  import { readGameStore, setGameStore } from './lib/gameStore.ts';
  import { EngineLink } from './lib/engineBridge.ts';
```

Edit `launchpad-drummer/src/App.svelte` (2 of 3). Replace:

```svelte
  function flushProtoLeds() {
    if (tauri) void invoke('send_led_leds', { leds: [...protoLeds] }).catch(() => {});
  }
  function protoLoaded() {
```

with:

```svelte
  // LIVE SYNTH (ship engine spec 5.2): Dustline asks for the app's native engine voice. The bridge
  // takes messages only from the active frame showing Dustline, drops malformed ones, caps their rate
  // and replies with the voice's state; the voice stops when the game is left or the window hides.
  const engineLink = new EngineLink({
    native: tauri,
    start: (patch) => call('game_engine_start', { patch }),
    frame: (on, bend, wheel, gain) => { void call('game_engine_frame', { on, bend, wheel, gain }).catch(() => {}); },
    stop: () => { void call('game_engine_stop').catch(() => {}); },
    reply: (state, message) => toProto({ lpd: 'engine-synth-status', state, message }),
  });
  $effect(() => {
    if (!(mode === 'games' && game === 'dustline')) engineLink.stop(true);
  });
  $effect(() => {
    if (!windowVisible) engineLink.stop(false);
  });
  function flushProtoLeds() {
    if (tauri) void invoke('send_led_leds', { leds: [...protoLeds] }).catch(() => {});
  }
  function protoLoaded() {
    engineLink.stop(true); // a reloaded game starts its voice again
```

Edit `launchpad-drummer/src/App.svelte` (3 of 3). Replace:

```svelte
    const data = e.data as { lpd?: string; leds?: unknown; messages?: unknown; key?: unknown; value?: unknown };
```

with:

```svelte
    const data = e.data as { lpd?: string; leds?: unknown; messages?: unknown; key?: unknown; value?: unknown };
    if (data?.lpd === 'engine-synth') {
      void engineLink.receive(data, protoFrame.dataset.game === 'dustline' && game === 'dustline');
      return;
    }
```

- [ ] **Step 2: Check and test** (Git Bash, from `launchpad-drummer`).

```bash
npm run check
npm test
```

Expected: `svelte-check found 0 errors and 0 warnings`, and every test passes (`# fail 0`), `engineBridge` included.

### Task 22: LIVE in the engine

**Purpose.** With a LIVE sink (`create(ctx, {live})`), the gear model also plays the app's voice through one frame per control step (spec 5.3; S16, S17, S25): bend, wheel and gain per S16, the BAKED ↔ LIVE equal-power crossfade over 0.15 s on the frame's `engineMode`, an off frame on pause, and no frames while BAKED. Without a sink the engine stays on BAKED. Two new tests and an extended fuzz test (every LIVE frame finite and in range).

**Files:**
- Modify: `prototypes/Dustline/checks/audio-engine.test.cjs` (LF)
- Create: `prototypes/Dustline/tools/engine2/step6_live.py`
- Modify (by `step6_live.py`): the three game copies

- [ ] **Step 1: Write the failing tests.** Apply these Edits, then run the line-ending check (expected `LF`).

Edit `prototypes/Dustline/checks/audio-engine.test.cjs` (1 of 3). Replace:

```js
test('fuzzed frames and events keep every parameter write finite and inside its range', () => {
  const ctx = makeContext(), eng = E.create(ctx, { output: E.createMaster(ctx).input, ship: TEX_SHIP });
```

with:

```js
test('fuzzed frames and events keep every parameter write and every LIVE frame finite and inside its range', () => {
  const ctx = makeContext();
  const live = f => { if (typeof f.on !== 'boolean' || !(f.bend >= -1 && f.bend <= 1 && f.wheel >= 0 && f.wheel <= 1 && f.gain >= 0 && f.gain <= 1)) ctx.problems.push('live frame ' + JSON.stringify(f)); };
  const eng = E.create(ctx, { output: E.createMaster(ctx).input, ship: TEX_SHIP, live });
```

Edit `prototypes/Dustline/checks/audio-engine.test.cjs` (2 of 3). Replace:

```js
      rivals: Array.from({ length: 5 }, (_, id) => ({ id, dx: pick() * 60 - 30, dz: pick() * 400 - 200, speed: pick() * 170, thr: pick(), boost: r() < .2, brake: pick(), active: r() < .9 })) });
    const dt = [1 / 60, 1 / 144, 1 / 30, 0, .3, NaN, -1][i % 7];
    ctx.currentTime += Number.isFinite(dt) && dt > 0 ? Math.min(dt, .3) : 0;
    eng.update(f, dt, ctx.currentTime);
```

with:

```js
      rivals: Array.from({ length: 5 }, (_, id) => ({ id, dx: pick() * 60 - 30, dz: pick() * 400 - 200, speed: pick() * 170, thr: pick(), boost: r() < .2, brake: pick(), active: r() < .9 })),
      engineMode: ['baked', 'live', 'live', undefined, 7][i % 5] });
    const dt = [1 / 60, 1 / 144, 1 / 30, 0, .3, NaN, -1][i % 7];
    ctx.currentTime += Number.isFinite(dt) && dt > 0 ? Math.min(dt, .3) : 0;
    eng.update(f, dt, ctx.currentTime);
    if (i % 113 === 0) eng.setActive(i % 226 !== 0, ctx.currentTime);
```

Edit `prototypes/Dustline/checks/audio-engine.test.cjs` (3 of 3). Replace:

```js
console.log(`${passed} engine audio checks passed.`);
```

with:

```js
test('LIVE SYNTH: an equal-power crossfade over 0.15 s, frames that follow the gear model, an off frame on pause', () => {
  const ctx = makeContext(), sent = [], eng = E.create(ctx, { output: E.createMaster(ctx).input, live: f => sent.push({ ...f }) });
  attachAll(ctx, eng);
  const toneGain = next(next(bankOf(ctx)[0])).gain, base = Math.pow(10, MIX.toneDb / 20) * Math.pow(10, MIX.liveTrimDb / 20);
  drive(ctx, eng, 1, frame({ engineMode: 'baked' }));
  assert.equal(sent.length, 0, 'no frames while BAKED');
  let steps = 0;
  drive(ctx, eng, .3, frame({ engineMode: 'live' }), 1 / 60, d => {
    if (sent.length === steps) return;
    steps = sent.length;
    const f = sent[sent.length - 1], mix = d.mode;
    near(toneGain.value, Math.cos(mix * Math.PI / 2), 1e-3, 'BAKED tone gain');
    near(f.gain, base * Math.sin(mix * Math.PI / 2), 1e-9, 'LIVE gain');
    near(toneGain.value ** 2 + (f.gain / base) ** 2, 1, 3e-3, 'equal power');
    near(f.bend, 12 * Math.log2(d.tone / 65.40639132514966) / 24, 1e-12, 'bend from the tone');
    near(f.wheel, Math.min(1, .15 + .6 * Math.max(d.loadL, d.loadR)), 1e-12, 'wheel from the load');
    assert.equal(f.on, true);
  });
  assert(steps >= 6 && steps <= 13, 'one frame per control step: ' + steps);
  assert.equal(eng.debug().mode, 1);
  assert(toneGain.value < 1e-3, 'BAKED tone silent in LIVE');
  eng.setActive(false, ctx.currentTime);
  const off = sent[sent.length - 1];
  assert.equal(off.on, false); assert.equal(off.gain, 0);
  const n = sent.length;
  drive(ctx, eng, .5, frame({ engineMode: 'live' }));
  assert.equal(sent.length, n, 'no frames while paused');
  eng.setActive(true, ctx.currentTime);
  drive(ctx, eng, .5, frame({ engineMode: 'baked' }));
  assert.equal(eng.debug().mode, 0);
  const last = sent[sent.length - 1];
  assert.equal(last.on, false, 'the last frame after the crossfade back turns the voice off');
  const m = sent.length;
  drive(ctx, eng, .5, frame({ engineMode: 'baked' }));
  assert.equal(sent.length, m, 'no frames once BAKED again');
  assert.deepEqual(ctx.problems, []);
});

test('LIVE frames carry the ignition cut and the zing wheel lift; without a sink the engine stays on BAKED', () => {
  const ctx = makeContext(), sent = [], eng = E.create(ctx, { output: E.createMaster(ctx).input, live: f => sent.push({ ...f, t: ctx.currentTime }) });
  drive(ctx, eng, .5, frame({ phase: 'countdown', speed: 0, engineMode: 'live' }));
  sent.length = 0;
  drive(ctx, eng, 1.5, t => frame({ speed: 68 + 40 * t, engineMode: 'live' }));
  const base = Math.pow(10, MIX.toneDb / 20) * Math.pow(10, MIX.liveTrimDb / 20), cut = Math.pow(10, MIX.cutDb / 20);
  assert(sent.some(f => Math.abs(f.gain - base * cut) < 1e-9), 'a frame inside the ignition cut');
  assert(sent.some(f => Math.abs(f.wheel - Math.min(1, .15 + .6 + .15)) < 1e-9), 'a frame inside the zing blip');
  const ctx2 = makeContext(), eng2 = E.create(ctx2, { output: E.createMaster(ctx2).input });
  attachAll(ctx2, eng2);
  drive(ctx2, eng2, 1, frame({ engineMode: 'live' }));
  assert.equal(eng2.debug().mode, 0, 'no native voice: BAKED');
  assert.equal(next(next(bankOf(ctx2)[0])).gain.value, 1);
});

console.log(`${passed} engine audio checks passed.`);
```

- [ ] **Step 2: Run them to see them fail.**

```bash
node prototypes/Dustline/checks/audio-engine.test.cjs
```

Expected: an `AssertionError` with the message `one frame per control step: 0`.

- [ ] **Step 3: Create and run step 6.** Create `prototypes/Dustline/tools/engine2/step6_live.py`:

```python
"""Engine 2.0, step 6 (plan Task 22): LIVE SYNTH. With a sink (create(ctx,{live})), the gear model also plays the app's native
voice through one frame per control step; BAKED and LIVE crossfade over 0.15 s on the frame's engineMode."""
from common import Build

b = Build()
# create(): the sink, the trim and the crossfade state
b.rep("""    const RATE_LO=.05*ANCHORS[ANCHORS.length-1],RATE_HI=4*ANCHORS[0],clampRate=hz=>clamp(hz,RATE_LO,RATE_HI);
""", """    const RATE_LO=.05*ANCHORS[ANCHORS.length-1],RATE_HI=4*ANCHORS[0],clampRate=hz=>clamp(hz,RATE_LO,RATE_HI);
    // LIVE SYNTH (spec 5): the same gear model plays the app's native voice through opt.live(frame), one frame per control
    // step. The held note sounds twice the fundamental, so the bend is measured from an octave below it. Without a sink the
    // engine stays on BAKED, whatever the frames ask.
    const liveSink=typeof opt.live==='function'?opt.live:null,LIVE_HZ=440*Math.pow(2,(SHIP.liveNote-12-69)/12),LIVE_TRIM=dbGain(MIX.liveTrimDb);
    const lf={on:false,bend:0,wheel:0,gain:0};
    let modeMix=0,liveOn=false,liveFrames=0;
""")
# per-side misfire dip state
b.rep("detuneUntil:0,rampUntil:0,rate:0,bank:null,", "detuneUntil:0,rampUntil:0,rate:0,dip:0,dipUntil:0,bank:null,")
# model state: phase level, the engine-bus duck and the zing window, for LIVE frames
b.rep("rivalsHeard:0,wasRecovering:false,sputterAt:0};\n",
    "rivalsHeard:0,wasRecovering:false,sputterAt:0,\n      lv:1,duckTo:1,duckUntil:0,zingFrom:0,zingUntil:0};\n")
b.rep("""      g.cancelScheduledValues(t);g.setTargetAtTime(1-d,t,.012);g.setTargetAtTime(1,t+hold,.14);
    }
""", """      g.cancelScheduledValues(t);g.setTargetAtTime(1-d,t,.012);g.setTargetAtTime(1,t+hold,.14);
      if(!r){M.duckTo=1-d;M.duckUntil=t+hold;}
    }
""")
b.rep("      sides[side].detuneUntil=t+.018+len+.07;\n",
    "      sides[side].detuneUntil=t+.018+len+.07;sides[side].dip=depth;sides[side].dipUntil=t+.018+len;\n")
b.rep("""      const up=type==='upshift',hold=up?.04:0,glide=up?.06:.04;
""", """      const up=type==='upshift',hold=up?.04:0,glide=up?.06:.04;
      if(up){M.zingFrom=t+.04;M.zingUntil=t+.09;}
""")
# LIVE frames
b.rep("""    /* ---- per-tick update of the player's engines and shared layers ---- */
""", """    // One LIVE SYNTH frame (spec 5.3): bend from the tone (shift glide included), the wheel from the higher load, boost and the
    // zing blip, the gain from the phase level, the engine-bus duck, the shift cut or blip, misfire dips and the crossfade.
    // At 40 Hz the 40 ms cut and the blip land in one or two frames: the native voice smooths them over its 25 ms ramp.
    function sendLive(g,boost){
      const dip=1-.5*((NOW<sides[0].dipUntil?sides[0].dip:0)+(NOW<sides[1].dipUntil?sides[1].dip:0));
      const shape=g.cut?CUT:g.blip?BLIP:1,duckG=NOW<M.duckUntil?M.duckTo:1,zing=NOW>=M.zingFrom&&NOW<M.zingUntil?1:0;
      lf.on=modeMix>0;
      lf.bend=clamp(12*Math.log2(g.tone/LIVE_HZ)/24,-1,1);
      lf.wheel=clamp(.15+.6*Math.max(sides[0].load,sides[1].load)+.25*boost+.15*zing,0,1);
      lf.gain=lf.on?clamp(M.lv*LV.tone*TONE*LIVE_TRIM*duckG*shape*dip*Math.sin(modeMix*Math.PI/2),0,1):0;
      liveOn=lf.on;liveFrames++;
      try{liveSink(lf);}catch(_){}
    }
    function liveOff(){
      if(!liveSink||!liveOn)return;
      lf.on=false;lf.gain=0;liveOn=false;liveFrames++;
      try{liveSink(lf);}catch(_){}
    }

    /* ---- per-tick update of the player's engines and shared layers ---- */
""")
# the crossfade: computed before the engines, the BAKED tone weighted by cos, LIVE by sin; textures stay in both modes
b.rep("""      for(let k=0;k<g.eventCount;k++)shiftTone(g.events[k].type,NOW,g);
""", """      for(let k=0;k<g.eventCount;k++)shiftTone(g.events[k].type,NOW,g);
      // BAKED <-> LIVE SYNTH: an equal-power crossfade over 0.15 s (the two paths never sum coherently). Textures stay in both.
      modeMix=clamp(modeMix+(f.engineMode==='live'&&liveSink?dt:-dt)/.15,0,1);
""")
b.rep("        S(s.vl.gain,TONE,.05,0,1.2);S(s.toneSum.gain,LV.tone,.05,0,1);\n",
    "        S(s.vl.gain,TONE,.05,0,1.2);S(s.toneSum.gain,LV.tone*Math.cos(modeMix*Math.PI/2),.012,0,1);\n")
b.rep("""      const asym=clamp(sides[1].eff-sides[0].eff,-1,1),loadAvg=sumLoad*.5;
""", """      if(liveSink&&(modeMix>0||liveOn))sendLive(g,boost);
      const asym=clamp(sides[1].eff-sides[0].eff,-1,1),loadAvg=sumLoad*.5;
""")
b.rep("""      S(level.gain,LEVEL*lv,.25,0,2);S(rlevel.gain,RIVAL_LEVEL*lv,.25,0,2);
""", """      S(level.gain,LEVEL*lv,.25,0,2);S(rlevel.gain,RIVAL_LEVEL*lv,.25,0,2);M.lv=lv;
""")
b.rep("""      }else{master.gain.setTargetAtTime(.0001,t,.03);parkAt=t+.35;}
""", """      }else{master.gain.setTargetAtTime(.0001,t,.03);parkAt=t+.35;liveOff();}
""")
b.rep("""      if(disposed)return;disposed=true;
      for(const s of[...shots])s.stop();""", """      if(disposed)return;liveOff();disposed=true;
      for(const s of[...shots])s.stop();""")
b.rep(",textures:sides[0].tex.length,shots:shots.size};},\n", ",textures:sides[0].tex.length,shots:shots.size,live:modeMix,liveFrames};},\n")
b.rep("""        loadL:sides[0].load,loadR:sides[1].load,effL:sides[0].eff,effR:sides[1].eff,burn:M.burnA,dmg:M.dmg,heat:M.heat,draft:M.draft,accel:M.accel,
""", """        loadL:sides[0].load,loadR:sides[1].load,effL:sides[0].eff,effR:sides[1].eff,burn:M.burnA,dmg:M.dmg,heat:M.heat,draft:M.draft,accel:M.accel,
        mode:modeMix,liveFrame:{on:lf.on,bend:lf.bend,wheel:lf.wheel,gain:lf.gain},
""")
b.save()
print("step 6: LIVE SYNTH frames and the BAKED/LIVE crossfade are in the engine block")
```

```bash
python prototypes/Dustline/tools/engine2/step6_live.py
```

Expected: `pushed <sha256> <bytes> bytes`, then `step 6: LIVE SYNTH frames and the BAKED/LIVE crossfade are in the engine block`.

- [ ] **Step 4: Run the tests to see them pass.**

```bash
node prototypes/Dustline/checks/audio-engine.test.cjs
```

Expected: `22 engine audio checks passed.`

- [ ] **Step 5: Run the regression (0.6).** Expected: 167 checks (`audio-engine` 22, `ship-gears` 17, `ship-profile` 7 and the other 121) and both ai-pace `PASS` lines.

### Task 23: LIVE game wiring and the LIVE audition

**Purpose.** The game side of LIVE SYNTH (spec 5.3, 5.4, 7; S24, S25, S47, S50) and its audition. The host bridge gains the `engineSynth` client; audio start asks the app for the voice once; Pause → Sound gets the ENGINE select, which offers LIVE SYNTH only once the app reports ready and falls back to BAKED on anything else; the frame carries `engineMode`; `Dustline.audio` reports `engineMode` and `live`. `host-bridge.py` plays the app's part: it answers `ready`, checks that no frame goes out before LIVE is chosen, that LIVE frames are in range, and that an `error` puts the engine back on BAKED. The LIVE audition renders the same frames through the real `GameEngineVoice` (`bake_engine scenario`) and suggests a `liveTrimDb`; `tools/engine2/live_trim.py` sets it, only with the user's value.

**Files:**
- Modify: `prototypes/Dustline/checks/host-bridge.py` (LF)
- Create: `prototypes/Dustline/tools/engine2/step7_live_game.py`, `prototypes/Dustline/tools/engine2/live_trim.py`
- Modify (by `step7_live_game.py`): the three game copies
- Generated: `live-frames.json`, `live-tone.wav`, `live-as-played.wav`, `live-matched.wav`, `ab-baked-live.wav` in `prototypes/Dustline/audio/audition-v0.13/`

- [ ] **Step 1: Write the failing check.** Apply these Edits, then run the line-ending check (expected `LF`).

Edit `prototypes/Dustline/checks/host-bridge.py` (1 of 3). Replace:

```python
window.hardwareState='disconnected';window.loads=0;window.writes=0;window.ledMessages=[];
```

with:

```python
window.hardwareState='disconnected';window.loads=0;window.writes=0;window.ledMessages=[];
window.engineStarts=0;window.engineFrames=[];
```

Edit `prototypes/Dustline/checks/host-bridge.py` (2 of 3). Replace:

```python
  if(m?.lpd==='lcxl-leds')ledMessages.push(...m.messages);
```

with:

```python
  if(m?.lpd==='lcxl-leds')ledMessages.push(...m.messages);
  if(m?.lpd==='engine-synth'&&m.op==='start'){engineStarts++;send({lpd:'engine-synth-status',state:'ready'});}
  if(m?.lpd==='engine-synth'&&m.op==='frame')engineFrames.push(m);
```

Edit `prototypes/Dustline/checks/host-bridge.py` (3 of 3). Replace:

```python
            frame.wait_for_function('Dustline.audio.decodedCount===29 && Dustline.audio.state==="running"')
```

with:

```python
            frame.wait_for_function('Dustline.audio.decodedCount===29 && Dustline.audio.state==="running"')
            # LIVE SYNTH (ship engine spec 5.3): audio start asks the host for the native voice once; frames go out only after
            # the host reported ready and LIVE SYNTH was chosen; an error puts the engine back on BAKED.
            page.wait_for_function('engineStarts===1')
            frame.wait_for_function('Dustline.audio.live==="ready" && !document.getElementById("pauseEngine").options[1].disabled')
            assert frame.evaluate('Dustline.audio.engineMode') == 'baked' and page.evaluate('engineFrames.length') == 0
            frame.evaluate("const s=document.getElementById('pauseEngine');s.value='live';s.dispatchEvent(new Event('change'))")
            frame.wait_for_function('Dustline.audio.engineMode==="live"')
            page.wait_for_function('engineFrames.some(f=>f.on===true&&f.gain>0&&f.bend>=-1&&f.bend<=1&&f.wheel>=0&&f.wheel<=1)')
            page.evaluate("send({lpd:'engine-synth-status',state:'error'})")
            frame.wait_for_function('Dustline.audio.engineMode==="baked" && document.getElementById("pauseEngine").options[1].textContent==="LIVE SYNTH UNAVAILABLE"')
```

- [ ] **Step 2: Run it to see it fail.**

```bash
python prototypes/Dustline/checks/host-bridge.py
```

Expected: a Playwright `TimeoutError` while waiting for `engineStarts===1` (the game does not ask for the voice yet).

- [ ] **Step 3: Create and run step 7.** Create `prototypes/Dustline/tools/engine2/step7_live_game.py`:

```python
"""Engine 2.0, step 7 (plan Task 23): the game side of LIVE SYNTH. The host bridge gains an engineSynth client, Pause gets
the ENGINE select, the frame carries engineMode, and the engine gets the bridge as its LIVE sink."""
from common import Build

b = Build()
# A: the host bridge's client for the app's native voice (spec 5.2, 5.3)
b.rep("""  let midi;
  window.addEventListener('message', event => {
""", """  // LIVE SYNTH (ship engine spec 5.3): the app plays the ship's tone on its native synth. The game asks with
  // {lpd:'engine-synth', op:'start'|'frame'|'stop'}; the app answers {lpd:'engine-synth-status', state}. Frames go out only
  // once the app reported ready.
  const engineSynth = { state: 'idle', onStatus: null,
    start(patch) { if (typeof patch === 'string') post({ lpd: 'engine-synth', op: 'start', patch }); },
    frame(on, bend, wheel, gain) { if (this.state === 'ready') post({ lpd: 'engine-synth', op: 'frame', on, bend, wheel, gain }); },
    stop() { post({ lpd: 'engine-synth', op: 'stop' }); } };
  let midi;
  window.addEventListener('message', event => {
""")
b.rep("""      input.onmidimessage({ data: new Uint8Array(message.data) });
    }
  });
  post({ lpd: 'req_status' });
""", """      input.onmidimessage({ data: new Uint8Array(message.data) });
    } else if (message.lpd === 'engine-synth-status' && ['ready', 'unavailable', 'error'].includes(message.state)) {
      engineSynth.state = message.state;
      if (engineSynth.onStatus) engineSynth.onStatus(message.state);
    }
  });
  post({ lpd: 'req_status' });
""")
b.rep("  return { connect(client) { midi = client;if (input.state === 'connected') void midi.connect(); } };\n",
      "  return { connect(client) { midi = client;if (input.state === 'connected') void midi.connect(); }, engineSynth };\n")
# B: Pause -> Sound gets the ENGINE select; LIVE SYNTH is disabled until the app reports ready
b.rep("""</select></label>
        <label for="pauseMotion">""", """</select></label>
        <label for="pauseEngine">Engine<select id="pauseEngine"><option value="baked">BAKED</option><option value="live" disabled>LIVE SYNTH UNAVAILABLE</option></select></label>
        <label for="pauseMotion">""")
# C: the mode, remembered for the session
b.rep("  let audio=null,sound=true,soundMode='sfx',", """  // ENGINE (spec 5.3): BAKED, or LIVE SYNTH once the app's native voice reports ready. Remembered for the session.
  let engineMode='baked';
  const engineHost=DustlineHostBridge?DustlineHostBridge.engineSynth:null;
  const liveReady=()=>!!engineHost&&engineHost.state==='ready';
  let audio=null,sound=true,soundMode='sfx',""")
b.rep("fault:'',draft:0,recovery:false,view:'chase',blocked:false,timing:null,ticks:false,rivals:audioRivals};",
      "fault:'',draft:0,recovery:false,view:'chase',blocked:false,timing:null,ticks:false,engineMode:'baked',rivals:audioRivals};")
# D: the engine's LIVE sink is the bridge; the native voice is built once, at audio start, and idles until LIVE is chosen
b.rep("    const engine=DustlineEngineAudio.create(ctx,{output:mix});\n",
      "    const engine=DustlineEngineAudio.create(ctx,{output:mix,live:engineHost?f=>engineHost.frame(f.on,f.bend,f.wheel,f.gain):undefined});\n")
b.rep("    decodeAllSfx();decodeShip();\n  }\n", """    decodeAllSfx();decodeShip();
    // The app builds the native voice once (it prefers the user's DUSTLINE KESTREL patch); it idles until LIVE SYNTH is chosen.
    if(engineHost){engineHost.onStatus=syncEngineUI;engineHost.start(JSON.stringify(engine.ship.patch));}
  }
""")
# E: the frame field
b.rep("    f.view=view;f.blocked=!!s.boostExhausted;f.ticks=soundMode==='synth';\n",
      "    f.view=view;f.blocked=!!s.boostExhausted;f.ticks=soundMode==='synth';f.engineMode=engineMode==='live'&&liveReady()?'live':'baked';\n")
# F: the select follows the bridge state; anything but ready falls back to BAKED
b.rep("""    $('pauseCamera').value=view;$('pauseSound').value=soundMode;$('pauseMotion').value=reduced?'reduced':'full';
  }
""", """    $('pauseCamera').value=view;$('pauseSound').value=soundMode;$('pauseMotion').value=reduced?'reduced':'full';syncEngineUI();
  }
  // LIVE SYNTH is offered once the app's voice reports ready; unavailable or an error keeps or puts the engine on BAKED (spec 5.3).
  function syncEngineUI(){
    const ready=liveReady(),select=$('pauseEngine'),live=select.options[1];
    live.disabled=!ready;live.textContent=ready?'LIVE SYNTH':'LIVE SYNTH UNAVAILABLE';
    if(!ready)engineMode='baked';
    select.value=engineMode;
  }
""")
b.rep("  $('pauseSound').onchange=()=>setSoundMode($('pauseSound').value);\n",
      "  $('pauseSound').onchange=()=>setSoundMode($('pauseSound').value);\n"
      "  $('pauseEngine').onchange=()=>{engineMode=$('pauseEngine').value==='live'&&liveReady()?'live':'baked';syncEngineUI();};\n")
# G: Dustline.audio reports the mode and the bridge state
b.rep("return {mode:soundMode,enabled:sound,", "return {mode:soundMode,enabled:sound,engineMode,live:engineHost?engineHost.state:'none',")
b.save()
print("step 7: the game offers LIVE SYNTH through the host bridge")
```

```bash
python prototypes/Dustline/tools/engine2/step7_live_game.py
```

Expected: `pushed <sha256> <bytes> bytes`, then `step 7: the game offers LIVE SYNTH through the host bridge`.

- [ ] **Step 4: Run the bridge and pause harnesses and the regression.**

```bash
python prototypes/Dustline/checks/host-bridge.py
python prototypes/Dustline/checks/pause-browser.py
```

Expected: `host-bridge.py` exits 0 with its JSON `checks` line; `pause-browser.py` prints its `PASS` lines (the last: `PASS modal focus/Tab/Escape, frozen race, nested settings, camera/audio/motion, full restart, Easy auto-maintenance and Standard QTE return`). Then run the regression (0.6): 167 checks and both ai-pace `PASS` lines.

- [ ] **Step 5: Create the trim tool (do not run it yet).** Create `prototypes/Dustline/tools/engine2/live_trim.py`:

```python
"""Engine 2.0, plan Task 23: set the Kestrel's LIVE SYNTH trim (mix.liveTrimDb, in dB) once the user has heard the LIVE
audition.     python prototypes/Dustline/tools/engine2/live_trim.py 2
It rewrites the value in the profile (all three copies) and the value checks/ship-profile.test.cjs expects. Above +3 dB the
LIVE frame gain reaches its ceiling of 1 at full level (tone -6 dB, blip +3 dB), and the loudest moments stop growing."""
import re
import sys
from pathlib import Path

from common import Build

db = round(float(sys.argv[1]), 1)
assert -12 <= db <= 6, "the trim must be between -12 and +6 dB"
b = Build()
pat = re.compile(r"liveTrimDb:-?\d+(?:\.\d+)?\}")
assert len(pat.findall(b.t)) == 1, "expected the profile's liveTrimDb once"
b.t = pat.sub(f"liveTrimDb:{db:g}}}", b.t)
b.save()
test = Path(__file__).resolve().parents[2] / "checks" / "ship-profile.test.cjs"
raw = test.read_bytes().decode("utf-8")
tp = re.compile(r"liveTrimDb: -?\d+(?:\.\d+)? \}")
assert len(tp.findall(raw)) == 1, "expected liveTrimDb once in checks/ship-profile.test.cjs"
test.write_bytes(tp.sub(f"liveTrimDb: {db:g} }}", raw).encode("utf-8"))
print(f"liveTrimDb {db:g} dB in the profile and in checks/ship-profile.test.cjs")
```

- [ ] **Step 6: Render the LIVE audition.** The script adds `~/.cargo/bin` to the PATH itself and runs `bake_engine scenario` through cargo (the first build takes a few minutes). It plays the user's `DUSTLINE KESTREL.lbpatch` if one exists in the patch folder, else the profile's patch.

```bash
python prototypes/Dustline/checks/audio_audition.py --live
```

Expected: the lines of Task 16, plus a `live {…}` line. The planning run (phase-3 copy, phase-1 assets): `"sent": 730`, `"sentHz": 30.0`, LIVE −15.38 LUFS, true peak −4.9 dB, no discontinuities, `"bakedToneCruiseLufs": -13.61`, `"liveToneCruiseLufs": -15.83`, `"trimSuggestionDb": 2.0`. If cargo reports "multiple different versions of crate `lpd_core`", run the `cargo clean` of 0.1 in PowerShell and repeat.

- [ ] **Step 7: Screen the audition** (earshot venv).

```bash
PY="/d/MY DOCS/MY TOOLS/earshot/.venv/Scripts/python.exe"
"$PY" -I prototypes/Dustline/checks/ship_screen.py audition prototypes/Dustline/audio/audition-v0.13
```

Expected: the JSON line now has a `"baked > live"` block of per-section cosines beside `"previous > new"`.

- [ ] **Step 8: Ask about the LIVE trim, without waiting.** Tell the user the measured `trimSuggestionDb`, the two cruise levels and the files `audition-v0.13/live-as-played.wav` and `ab-baked-live.wav` (per section: BAKED, then LIVE). Ask whether to set a LIVE trim. `mix.liveTrimDb` stays 0 unless the user answers with a value; continue with Task 24 meanwhile. When the user gives a value `<dB>` (between −12 and +6):

```bash
python prototypes/Dustline/tools/engine2/live_trim.py <dB>
node prototypes/Dustline/checks/ship-profile.test.cjs
```

Expected: `pushed …`, `liveTrimDb <dB> dB in the profile and in checks/ship-profile.test.cjs`, then `ship profile: 7 passed`. Then rerun the regression (0.6) and, if Task 24 already built the app, rebuild it (Task 24 Step 2) and repeat its smoke steps.

### Task 24: Exe rebuild, native smoke test, final regression and README

**Purpose.** Ship 0.13 in the app and prove LIVE SYNTH natively (spec 7, 9; S46). The smoke test learns 0.13 (version, 29 decoded sounds, the ship's 22 assets decoded and attached, the tone on) and, over CDP, that LIVE SYNTH reports ready, can be chosen, and that its frames reach Rust (`game_engine_status`). Then the final regression, the README's final engine section, and the memory notes.

**Files:**
- Modify: `launchpad-drummer/tools/test-dustline-browser.py` (CRLF), `prototypes/Dustline/README.md` (CRLF)
- Build: `launchpad-drummer/dist/`, `launchpad-drummer/target/release/launchpad-drummer.exe`
- Update: the memory notes `dustline-ship-engine-spec.md` and `dustline-three-copies.md` (0.1)

- [ ] **Step 1: Point the smoke test at 0.13.** Apply these Edits, then run the line-ending check (expected `CRLF`).

Edit `launchpad-drummer/tools/test-dustline-browser.py` (1 of 2). Replace:

```python
                assert frame.evaluate('Dustline.version') == '0.12.0'
                frame.locator('#startBtn').click()
                wait(frame, 'Dustline.audio.decodedCount===28 && Dustline.audio.state==="running"')
                assert frame.evaluate('document.activeElement.id') == 'game', 'Start focuses the racing frame'
                page.keyboard.press('p');wait(frame, 'Dustline.paused')
                page.keyboard.press('p');wait(frame, '!Dustline.paused')
```

with:

```python
                assert frame.evaluate('Dustline.version') == '0.13.0'
                frame.locator('#startBtn').click()
                wait(frame, 'Dustline.audio.decodedCount===29 && Dustline.audio.state==="running"')
                # the ship's 22 assets are decoded and attached, and the tone runs (ship engine spec 2.2 and 4.5)
                wait(frame, 'Dustline.audio.ship.total===22 && Dustline.audio.ship.decoded===22 && Dustline.audio.engine.tone===true')
                assert frame.evaluate('document.activeElement.id') == 'game', 'Start focuses the racing frame'
                page.keyboard.press('p');wait(frame, 'Dustline.paused')
                page.keyboard.press('p');wait(frame, '!Dustline.paused')
                if args.cdp:
                    # LIVE SYNTH (ship engine spec 5.3 and 7): the native voice reports ready, LIVE is chosen, frames reach Rust
                    wait(frame, 'Dustline.audio.live==="ready" && !document.getElementById("pauseEngine").options[1].disabled')
                    frame.evaluate("const s=document.getElementById('pauseEngine');s.value='live';s.dispatchEvent(new Event('change'))")
                    wait(frame, 'Dustline.audio.engineMode==="live"')
                    page.wait_for_timeout(500)
                    live = page.evaluate("__TAURI__.core.invoke('game_engine_status')")
                    assert live['running'] and live['frames'] > 0 and live['source'] in ('game', 'user'), live
                    frame.evaluate("const s=document.getElementById('pauseEngine');s.value='baked';s.dispatchEvent(new Event('change'))")
                    wait(frame, 'Dustline.audio.engineMode==="baked"')
```

Edit `launchpad-drummer/tools/test-dustline-browser.py` (2 of 2). Replace:

```python
                print(json.dumps({'mode': label, 'checks': 'sandbox boot, Web Audio, keyboard focus/pause, host storage and remount, source rejection, parent and in-frame fullscreen, game plate bounds' + (', synthetic Tauri fader boost/brake, knob alignment, Focus 1 vent and Control 1 repairs, native LED command delivery/validation and game-switch cleanup' if args.cdp else ''), 'frame': box, 'actualDevicesBeforeInjection':actual_devices, 'errors': errors}))
```

with:

```python
                print(json.dumps({'mode': label, 'checks': 'sandbox boot, Web Audio, ship assets and tone, keyboard focus/pause, host storage and remount, source rejection, parent and in-frame fullscreen, game plate bounds' + (', LIVE SYNTH ready and frames in Rust, synthetic Tauri fader boost/brake, knob alignment, Focus 1 vent and Control 1 repairs, native LED command delivery/validation and game-switch cleanup' if args.cdp else ''), 'frame': box, 'actualDevicesBeforeInjection':actual_devices, 'errors': errors}))
```

- [ ] **Step 2: Rebuild the app** (PowerShell, from `launchpad-drummer`, cargo PATH set). If Launchpad Drummer is running, ask the user to close it first.

```powershell
npm run tauri -- build --no-bundle
```

Expected: the frontend build, then cargo's `` Finished `release` profile `` and the path of `target\release\launchpad-drummer.exe`.

- [ ] **Step 3: Production smoke test** (Git Bash, from the root; it serves `launchpad-drummer/dist`).

```bash
python launchpad-drummer/tools/test-dustline-browser.py
```

Expected: one JSON line with `"mode": "production"`, the checks list including "ship assets and tone", and `"errors": []`.

- [ ] **Step 4: Native smoke test.** Start the release with a WebView2 debugging port (PowerShell; run the three lines in one call, since the variable does not survive between calls):

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9417"
Start-Process "D:\MY DOCS\MY TOOLS\LAUNCHBAND\launchpad-drummer\target\release\launchpad-drummer.exe"
Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
```

Wait until `curl -s http://127.0.0.1:9417/json/version` answers (Git Bash), then:

```bash
python launchpad-drummer/tools/test-dustline-browser.py --cdp http://127.0.0.1:9417
```

Expected: one JSON line with `"mode": "native"`, the checks list including "LIVE SYNTH ready and frames in Rust", and `"errors": []`. Close the app afterwards (`Stop-Process -Name launchpad-drummer` in PowerShell, or ask the user if it was theirs).

- [ ] **Step 5: Final regression.**
  - Dustline regression (0.6): 167 checks and both ai-pace `PASS` lines.
  - Browser harnesses (0.6): eight `PASS` lines.
  - PowerShell, from `launchpad-drummer`: `cargo fmt --all -- --check` (no output) and `cargo test --workspace` (all pass).
  - Git Bash, from `launchpad-drummer`: `npm test` (`# fail 0`) and `npm run check` (`svelte-check found 0 errors and 0 warnings`).
  - Hash check (0.6): `ok <sha256>`.

- [ ] **Step 6: README.** Replace the whole `## Engine audio` section of `prototypes/Dustline/README.md` with the text below, using the byte-preserving replacement of Task 9 Step 4. Use the measured numbers where they differ, and the user's `liveTrimDb` if Task 23 set one.

```markdown
## Engine audio

The engine (`DustlineEngineAudio` 2.0.0) is driven by a ship profile and a gear model. The Kestrel is the only ship for now; a later ship is a new profile plus its assets, not new code. The tone has two backends, chosen in Pause → Sound → **ENGINE: BAKED / LIVE SYNTH** and kept for the session: BAKED plays synth loops in the game, LIVE SYNTH plays the same patch live on the app's native synth. Airflow, heat strain, damage misfires, vent steam, rival engines, the timing cues, the ducking rules and the output stage (glue compressor −11 dB 3.8:1, limiter −1.5 dB 20:1, master gain 0.75) are the 1.2.0 layers. The ElevenLabs one-shots (contact, gates, laps, countdown, hazards, timing cues) remain: the page carries 29 of them.

**Gear model.** Seven gears with top speeds 80, 150, 220, 300, 390, 490 and 700 km/h. Within a gear the fundamental is `124 Hz × v / top`, never below 41 Hz. An upshift comes at 0.97 of the gear's top while the craft accelerates, at least 0.22 s after the last shift; while it waits, the pitch holds at 1.03 × redline. A downshift comes when the speed falls to 0.92 × the lower gear's top, at least 0.09 s after the last shift. In gears 1–6 the rules read a paced speed that rises at most 250 km/h per second, so each launch upshift is a full sweep with an audible drop (0.19, 0.54 and 0.93 s after GO). Past 700 km/h in 7th, boost keeps lifting the pitch: `124 × (v / 700)^0.8`, about 191 Hz at 1200 km/h. Cruise (600 km/h) sits at 0.857 of 7th, off the limiter. On the grid the pitch follows the throttle (neutral revs); GO picks the gear from speed (4th at full throttle) with the launch sound and is not a shift. An upshift cuts the tone 9 dB for 40 ms, glides down over 60 ms and blips the zing (+6 dB at 8 × the fundamental); a downshift blips the throttle (+3 dB for 80 ms). The model is deterministic and does not depend on the frame rate: it advances in 1/120 s substeps.

**Tone.** The Keys synth patch `audio/ships/kestrel/kestrel-engine.lbpatch` (KESTREL ENGINE) holds the sound: SUB and a sync-warped OSC A at the fundamental through a driven 24 dB low-pass, a 3-voice OSC B two octaves up that bypasses the filter for the zing, and FX DRIVE TUBE; the mod wheel opens both warps, the cutoff, the filter drive and both oscillator levels. **BAKED:** the app's own Rust synth renders it offline (`crates/lpd-core/examples/bake_engine.rs`) into eight 2 s loops: four pitch anchors (48, 75, 117 and 182 Hz), each on-load (wheel 0.85) and off-load (wheel 0.2). The loops are phase-locked, so the engine starts all eight of an engine together and crossfades them at equal gain: linearly in log frequency between the two nearest anchors, and by a smoothstep on load. Every loop of an engine plays at `fundamental / anchor`.

**LIVE SYNTH** (app only). The game asks the app for a native voice once audio starts, and offers LIVE SYNTH only after the app answers ready; otherwise the option reads LIVE SYNTH UNAVAILABLE and BAKED plays. On LIVE, the engine sends one frame per control step (30 Hz at a 60 Hz display, 36 Hz at 144 Hz): `bend = 12 × log2(f / 65.41) / 24` with the shift glide in `f`, `wheel = 0.15 + 0.6 × the higher load + 0.25 × boost` (+0.15 during a zing blip), and `gain` = master × mute × pause × duck × the shift cut or blip, all clamped. The frames travel by postMessage to the app, where `src/lib/engineBridge.ts` takes them only from the active frame showing Dustline, drops malformed ones and caps them at 60 per second (burst 10), then to the Tauri commands `game_engine_start`, `game_engine_frame`, `game_engine_stop` and `game_engine_status`. In Rust, `GameEngineVoice` is a second, private `Synth` (the Keys synth's own patch and routing are never touched): it holds note 48 in LEGATO with the bend range forced to 24, glides bend and wheel to each frame over 40 ms and the gain over 25 ms in 16-sample steps, fades out 250 ms after the last frame, and joins the master bus before the master gain and the soft clipper (the app has no limiter). One voice stands for both engines, so it plays at twice its frame gain. It prefers the user's `DUSTLINE KESTREL.lbpatch` from the patch folder, else the profile's patch; a restarted output stream gets a rebuilt voice. Switching backends mid-race crossfades them over 0.15 s at equal power. The profile's `mix.liveTrimDb` sets the LIVE level (`python prototypes/Dustline/tools/engine2/live_trim.py <dB>`, after the LIVE audition).

**Textures.** Six ElevenLabs loops, 3 s each, follow the same rpm on both engines in both modes, each at `playbackRate = rpm / refRpm` (clamped to 0.5–2): F1 V10 at mid RPM, at high RPM and off-load burble (40 %, full in gears 3–7 and 0.6 in gears 1–2; mid and high crossfaded by rpm, on and off by load); jet turbine cruise roar and afterburner burn (35 %, `0.4 + 0.6 × boost`, cruise to burn by boost intensity); and a spaceship drone an octave under the fundamental (25 %, `0.6 + 0.4 × load`, with a slow shimmer at a different phase on each side). Different recordings never sum coherently, so the crossfades inside a layer are equal-power.

**One-shots.** An upshift pops a crackle (three variants: each engine plays a different one, chosen by the seeded generator), a downshift a burble blip (two variants), and boost ignition, boost release and the launch each have their own sound; a boost pad plays a shortened ignition. At most six play at once (the oldest stops first). They replace the 1.2.0 afterburner ignition, burn and release voices.

**Twin engines.** The left engine sits at pan −0.34, the right at +0.34 and 0.35 % higher. Each engine's load follows its own fader: full thrust is at the fader's 0 dB mark (0.75), the boost zone adds no load, and braking takes 45 % off. Load rises within about 55 ms and falls within 110 ms; below 0.25 the tone moves to the hollow off-load loops. On LIVE SYNTH the tone is one centred voice; textures and one-shots stay twin and panned.

**Kept layers.** The strain resonators now ring from a pulse wave at the fundamental, which reaches the resonators only. A misfire dips the tone briefly and detunes its own engine 0.6 %; thrust stays audible. Running dry in boost flickers the jet textures. Rivals keep the 1.2.0 procedural engine.

**Known limits.** On LIVE SYNTH the tone and the textures come out of two audio paths (WebView2 and the app's native output); expect an offset of roughly 20–60 ms between them, so a shift pop may land slightly off the pitch drop. The LIVE twin engines are one centred voice. Rivals use the 1.2.0 procedural engine in both modes. The automated checks cannot record the native output; the Rust scenario render (`bake_engine scenario`) is the closest stand-in. Frames arrive at 30–36 Hz, so the voice's 40 ms glide is what keeps the pitch moving between them.

**Assets and lifecycle.** The ship's 22 assets (8 baked loops, 6 textures, 8 one-shots) are Ogg Opus at 96 kbps and 48 kHz, which WebView2 and Chromium decode gaplessly. They are embedded as `DUSTLINE_SHIP_B64`, one line beside the SFX bundle, keyed `kestrel:<id>`. The game decodes them and hands each one to the engine (`attach`); the engine itself never decodes audio. The engine builds 222 nodes (66 sources) once, plus at most six one-shot voices, and only writes parameters per frame. Pause, mute, hidden tab and restart behave as in 1.2.0; pausing sends LIVE SYNTH an off frame. Variation comes from a private seeded generator, never `Math.random` or the simulation's random stream. The page is about 2.2 MB.

**Ship profile.** `DustlineEngineAudio.ships.kestrel` holds `id`, `name`, `gears` (`tops`, `idleHz` 41, `redlineHz` 124, `boostExponent` 0.8, `upMin` 0.22, `downMin` 0.09, `overRev` 1.03, `pace` 250), `patch` (the `.lbpatch` JSON), `patchName` (`DUSTLINE KESTREL`), `liveNote` 48, `baked` (eight `{id, anchorHz, load, loopStart, loopEnd}`, in samples at 48 kHz: 0.05 s lead-in, then the 2 s loop), `textures` (six `{id, layer, role, refRpm, baseHz, loopStart, loopEnd, weight}`: 0.05 s lead-in, then the 3 s loop), `oneShots` (`{upshift: [3 ids], downshift: [2 ids], ignite, release, launch}`) and `mix` (levels in dB, shift cut and blips, pan, right-engine pitch, `liveTrimDb`). The gear model is `DustlineEngineAudio.gears`.

**Playing the patch.** `kestrel-engine.lbpatch` is copied to `Documents\Launchpad Drummer\Patches`, so PLAY lists it as KESTREL ENGINE. Hold note 48 (one octave below middle C) and use the pitch wheel as speed and the mod wheel as load. Save a tweak as "Dustline Kestrel" (stored as `DUSTLINE KESTREL.lbpatch`): LIVE SYNTH loads it the next time Dustline starts its audio, without a rebuild. For BAKED, copy its `params` into `kestrel-engine.lbpatch`, run `python prototypes/Dustline/tools/engine2/profile_patch.py`, then bake, encode and embed (Tools below).

**Tools** (paths from `D:\MY DOCS\MY TOOLS\LAUNCHBAND`):

- `python prototypes/Dustline/tools/sync_copies.py check|diff|push` compares the three copies with the recorded hash (`tools/copies.sha256`). `tools/engine2/step*.py` are the byte-preserving edits that built 2.0, one per step.
- Bake, from `launchpad-drummer`: `cargo run -p lpd-core --release --example bake_engine -- loops --patch <file.lbpatch> --out ..\prototypes\Dustline\audio\ships\kestrel\loops` (`check --patch <file>` reports any value the app would clamp; `scenario --patch <file> --frames <json> --out <wav>` renders a 40 Hz frame log through `GameEngineVoice`). Then `python prototypes/Dustline/tools/ship_encode.py` and `python prototypes/Dustline/apply-sfx.py`.
- Textures and one-shots: `python prototypes/Dustline/generate-ship-sfx.py` lists the 42 ElevenLabs generations (14 sounds × 3 takes) and `--go` makes the missing ones with the user's credits (key from `.env`, never printed). `checks/ship_screen.py takes` (earshot venv) screens every take and picks one per sound by a fixed rule (the highest earshot `match` z among the takes that pass steadiness and a clean seam or trim) into `audio/ships/kestrel/picks.json`. `python prototypes/Dustline/tools/ship_assets.py` masters the picks into `loops/` and `shots/` and writes `profile-assets.json`; `python prototypes/Dustline/tools/engine2/profile_assets.py` puts them in the profile. Then encode and embed as above.
- earshot screening, in the earshot venv: `checks/ship_screen.py loops` writes `audio/ships/kestrel/LOG.md` and `loops/analysis.json` (pitch with `checks/pitch.py`, seams, phase, band shares, earshot `measure` and `describe`).
- Audition: `python prototypes/Dustline/checks/audio_audition.py --live` (runs cargo for the LIVE render), then `checks/ship_screen.py audition prototypes/Dustline/audio/audition-v0.13` in the earshot venv. The frames come from the real simulation (`checks/ship-scenarios.cjs`), 0.12's engine is rendered on the same frames for comparison, and `metrics.json` suggests a `liveTrimDb`.
- Native smoke: `python launchpad-drummer/tools/test-dustline-browser.py --cdp http://127.0.0.1:9417` against a release started with that WebView2 debugging port; it also checks that LIVE SYNTH reports ready and that frames reach Rust (`game_engine_status`).

**Checks.** 167 deterministic checks (`audio-engine` 22, `ship-gears` 17, `ship-profile` 7 and the other 121), the 8 browser harnesses and `ai-pace.cjs --verify`. In the app: `cargo test --workspace` (with `synth_game` 8, `audio_engine` 29 and `no_alloc` 6 in `lpd-core`, and the router and state tests in the app), `npm test` (with `engineBridge` 9) and `npm run check`. `audio-browser.py` decodes every loop at 48 and 44.1 kHz (seam limits 0.1 for baked loops and 0.25 for textures, whose Opus coding noise alone reads about 0.1–0.15) and checks each pitched texture against its `baseHz`. The synth-only audition is in `audio/audition-v0.13-p1/`, the full one with the LIVE SYNTH render in `audio/audition-v0.13/` (`metrics.json`, `earshot.json`); these are measurements, and the listening verdict is the user's.
```

Then apply this Edit and run the line-ending check (expected `CRLF`):

Edit `prototypes/Dustline/README.md`. Replace:

```markdown
Historical implementation checks from earlier November 4 passes (current QTE results are above and in `checks/qte-report.txt`):
```

with:

```markdown
0.13 (engine audio 2.0) was rebuilt with `npm run tauri -- build --no-bundle`; the native smoke (`--cdp`) also checks that LIVE SYNTH reports ready and that its frames reach Rust (`game_engine_status`).

Historical implementation checks from earlier November 4 passes (current QTE results are above and in `checks/qte-report.txt`):
```

- [ ] **Step 7: List what changed (no commit).**

```bash
git -C launchpad-drummer status --short
python prototypes/Dustline/tools/sync_copies.py check
```

Expected: the files of Tasks 4, 17–21 and 24 and the app's game copy among the changes (plus the unrelated ones that were there before); `ok <sha256>`. Do not commit unless the user asks.

- [ ] **Step 8: Update the memory notes.** In `C:\Users\DEROK137\.claude\projects\d--MY-DOCS-MY-TOOLS-LAUNCHBAND\memory\`: `dustline-three-copies.md`: the three copies are now `index.html`, `dustline-rift-run-v0.13.html` and the app copy, and `prototypes/Dustline/tools/sync_copies.py check|diff|push` guards them. `dustline-ship-engine-spec.md`: engine audio 2.0 (0.13) is implemented, with the checkpoint verdicts, the LIVE trim and anything the user still has to hear. Update their lines in `MEMORY.md`.

- [ ] **Step 9: Final report.** Measurements only: the check counts (167, 8 harnesses, ai-pace, the Rust and npm suites), the native smoke result, `game_engine_status` frames, page size, the LIVE numbers and the trim state, and the files to listen to. Ask the user to listen to both modes in the app (GAMES → DUSTLINE; in Pause, the Engine select under Sound: BAKED, then LIVE SYNTH): spec 9 counts the work done only once the user has heard both. The listening verdict is the user's.

---

## Self-review (spec coverage)

Each section of the spec and the tasks that carry it.

| Spec section | Tasks |
|---|---|
| 1 Decisions | 0.3 (the review's fixes and the answered question), 0.4 (S1–S54); each task names the rows it implements |
| 2 Architecture, 2.1 Interface | 2 (profile), 3 (gear model), 6 (engine 2.0.0 behind the same interface), 7 (game wiring, ship bundle), 22 (LIVE sink), 23 (LIVE game wiring) |
| 2.2 Ship profile format | 2 (identity, gears, mix, patch, baked set), 15 (textures and one-shots from `profile-assets.json`) |
| 3.1 Gear model | 3 |
| 3.2 Tone patch | 2 (the file and its section 3.2 checks), 4 (`check`), 5 (bake and screening), 6 (the anchors in BAKED); CHECKPOINT 1 (a tweak into BAKED) |
| 3.3 Texture layers | 10 (prompts), 11 (generation), 12 (screening and picks), 13 (masters), 14 (engine), 15 (profile and embed) |
| 3.4 Twin engines | 6 (two BAKED engines, eight loops each), 14 (textures and one-shots on both), 22 (one LIVE voice, the wheel from the higher load) |
| 3.5 Kept 1.2.0 layers | 6 (strain from a pulse wave at the fundamental, misfires on the new gains with detune, rivals on 1.2.0), 14 (the 1.2.0 afterburner and ignition voices go, S10) |
| 3.6 Measured targets | 8 (phase-1 audition), 16 (full BAKED, layers included), 23 (LIVE) |
| 4.1 Synth patch | 2, 9 (on PLAY) |
| 4.2 Synth bake | 4 (`check`, `loops`), 5 (the run), 18 (`scenario`) |
| 4.3 ElevenLabs generation | 10, 11 (42 generations, only on the user's go) |
| 4.4 earshot screening | 5 (`loops`), 8 (`audition`), 12 (`takes`), 16, 23 |
| 4.5 Encode and embed | 7 (the eight loops), 15 (all 22 assets) |
| 5.1 Rust (`lpd-core`) | 17 (`GameEngineVoice`), 18 (engine commands, master mix, no-allocation tests) |
| 5.2 Tauri and `App.svelte` | 19 (router, state, commands, registration), 20 (`engineBridge.ts`), 21 (`App.svelte`) |
| 5.3 Game side | 22 (frames, BAKED/LIVE crossfade), 23 (host bridge client, Pause → Sound → Engine, fallback to BAKED) |
| 5.4 Known limits | 24 (README section) |
| 6 Files | each task's Files list; 0.5 |
| 7 Testing | each task's failing test first; 24 (smoke tests, final regression) |
| 8 Work order | the three phases; CHECKPOINT 1 after Task 9, CHECKPOINT 2 after Task 16; Task 11 only on go |
| 9 Done | 24: every check, the copies' hash check, the archive hashes (`timing-events.test.cjs`, Task 1), the rebuilt exe, the README, and the request to hear both modes |

Where the plan meets a spec line in its own way:
- Spec 7 asks that each loop's `baseHz` match `picks.json`. `audio-browser.py` measures each pitched texture's decoded fundamental against its profile `baseHz` (within 3 %), and Task 15's test checks that the profile equals `profile-assets.json`, which `ship_assets.py` builds from `picks.json` (Task 13).
- Spec 7 asks for earshot `ab` against `audition-v0.12`. The audition renders 0.12's engine (1.2.0) on the same frames (`ab-matched.wav`), and `ship_screen.py audition` compares the two per section.

Placeholder scan: no "TBD", "TODO" or "fill in". The remaining angle-bracket slots are values the run supplies: `<sha256>` (the hash check prints it), `<dB>` (only the user's answer, Task 23 Step 8), and the `<file>`, `<dir>` and `<wav>` arguments in usage lines.

Name consistency: every name in 0.5 occurs in the code the tasks create.
