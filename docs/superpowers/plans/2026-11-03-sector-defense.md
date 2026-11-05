# Sector Defense

Add a central-station defense game inspired by the supplied CRT space-grid reference.
Use the MKII's actual 8×8 pads; the station sits between the four center pads.
Press a target's cell to shoot or collect repairs. Volume fires a charged pulse;
Session starts/pauses, and Pan pauses. Mouse and arrow keys + Enter provide fallback.

Scope: new standalone game/rules/rendering files, desktop Games tab, both arcade
hub entries and shared-menu registries, focused Node and browser checks. Reuse
the existing sandbox MIDI bridge and shared menu; no Rust/audio-engine changes,
new dependencies, multiplayer, or persistent progression.

- [x] Rules, waves, scoring, damage, repair, pulse, loss/restart.
- [x] CRT renderer, accessible controls, shared menu, audio, MKII LED/input mapping.
- [x] Desktop and browser launch entries, matching prototype delivery.
- [x] Rules/MIDI regression tests, production build, browser visual/interaction checks.

Completed 2026-11-03. Canonical game files are in `public/games/sector/`, mirrored
to the workspace's `prototypes/sector/` for the existing standalone launcher.
No new dependencies or external assets are needed.

Validation:
- `npm test`: 164 passed, including 10 Sector Defense checks.
- `npm run build`: zero Svelte errors/warnings; production assets built.
- `python tools/test-sector-browser.py`: offline standalone and production page;
  mouse/keyboard, pause/resume, loss/restart; simulated Web MIDI RGB SysEx,
  Session layout initialization, palette fallback and reconnect; actual app
  sandbox under production CSP, parent-message validation, corner pad mapping,
  releases, pulse, pause, disconnect/reconnect and LED cleanup on tab change.
- Visual inspection at 1280×720 and 1440×900 in the app, 1280×800 standalone,
  and a narrow 390 px viewport. Screenshots: `.verification/sector/`.
- `npm run tauri build`: release executable rebuilt successfully, so `run.bat`
  opens the version with Sector Defense.

Physical MKII input/LED latency and audible output were not verified on hardware.
