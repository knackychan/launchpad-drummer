> Follow-up on 2026-11-04: the user requested in-app LED output, fixed Vent/Repair buttons on Track Focus 1 / Track Control 1, and blinking knob lights. The input-only restriction and MIDI-script preservation below are superseded by [the controller-lights plan](2026-11-04-dustline-controller-lights.md). Archived 0.11 remains unchanged; the current 0.12 release receives the enhancement.

Make Dustline playable inside the Launchpad Drummer app (GAMES tab) with a physical Launch Control XL. The standalone browser build must keep working exactly as it does now. Workspace: D:\MY DOCS\MY TOOLS\LAUNCHBAND. Read prototypes/Dustline/README.md first.

DECISIONS ALREADY MADE (do not reopen)
1. Sound stays in the game. Dustline 0.11's Web Audio engine is not ported to Rust, and the app's keys synth is not driven live. Every app game makes its own sound inside its frame.
2. prototypes/Dustline stays the only source, with its tests and version files. The app gets a byte-identical copy at launchpad-drummer/public/games/dustline/index.html.
3. Release this as Dustline 0.12: prototypes/Dustline/dustline-rift-run-v0.12.html == prototypes/Dustline/index.html == launchpad-drummer/public/games/dustline/index.html. Never modify the 0.11, 0.10 or 0.9 files or the v0.2 archive.
4. In the app, the LCXL belongs to the game. While GAMES shows a frame game, the app forwards raw LCXL MIDI to the game and does not apply it to the drummer's mixer, synth or autosave.
5. No new dependencies. Dustline stays one offline HTML file. Its sound, visuals, HUD, camera, handling and rival AI do not change.

FACTS CHECKED ON 2026-11-03 (line numbers approximate; re-check them)
App (launchpad-drummer, branch main):
- Frame games are loaded at src/App.svelte ~1290 as <iframe sandbox="allow-scripts" src="/games/${game}/index.html">. The frame's origin is opaque, so Web MIDI fails with SecurityError and localStorage throws.
- Bridge code is at App.svelte ~578-620 (toProto, onProtoMessage; protoActive = mode==='games' && game!=='horde').
  - Messages from the app to the game today: {lpd:'status',state} for the Launchpad, and {lpd:'midi',data}, which carries Launchpad pads as 0x90 notes and the top row as CC 104-111.
  - Messages from the game to the app: {lpd:'req_status'} and {lpd:'leds',leds:[[note,r,g,b]]}.
  - No LCXL traffic reaches games.
- The game bar is the {#each [...]} list at App.svelte ~1270. The hub registry is ALL_GAMES in public/games/shared/launchband-menu.js, and public/games/index.html is the standalone hub.
- LCXL path in Rust: the midir input callback in src-tauri/src/lcxl.rs check_connection() calls lpd_core::devices::lcxl::decode(), then AppState::on_lcxl (src-tauri/src/state.rs ~706), then router.on_lcxl.
  - That path changes mixer group gain, mute and solo, and synth knob params. These changes are undoable and autosaved.
  - decode() keeps only faders (CC 77-84), knobs (CC 13-20, 29-36, 49-56) and track buttons (notes 41-44, 57-60, 73-76, 89-92). Everything else is dropped.
  - Nothing on this path checks InputMode, so today the LCXL faders move the drummer's mixer while a game is open.
- InputMode is set by AppState::set_input_mode() and read by input_mode() (state.rs ~789-811). App.svelte switches to 'game' when entering GAMES (~1034). launchpad.rs already branches on InputMode::Game; follow that pattern.
- LCXL connection state in the UI comes from lcxlStatus (App.svelte ~523, from the device registry).

Dustline (prototypes/Dustline, 0.11):
- Script layout: eight inline scripts. Script 3 is the MIDI module DustlineMIDI. Script 7 is the game; it builds midi=new DustlineMIDI({...}) at ~4963 and connects only from #connectMidiBtn (~5003).
- Connecting: connect() calls navigator.requestMIDIAccess({sysex:true}), falling back to {sysex:false}. _chooseInput() prefers isOriginalXL (by name and manufacturer), then assigns input.onmidimessage. access.onstatechange drives the disconnect and reconnect logic.
- LED output: _attachOutput and _sendLED drive LCXL LEDs. Knob LEDs need SysEx. The README says the integration is "input-only, no LED path". Check which is true and fix the README.
- Persistence:
  - Bindings persist through localStorage, read by _load() in the constructor (~2102) and written by _save() (~2125), both inside try/catch.
  - In the sandbox they silently fail to persist, so learned buttons (vent/repair, boost, airbrake, shunt) would be lost on every launch.
- Existing fake to copy: checks/timing-browser.py already fakes navigator.requestMIDIAccess with a port {name:'Launch Control XL', manufacturer:'Novation', state, onmidimessage} and an access object {inputs: Map, onstatechange}. A host adapter with the same shape lets DustlineMIDI stay unchanged.
- Twin faders: the left fader is CC 77 and the right is CC 84.
- Tests pin the scripts:
  - timing-events.test.cjs requires scripts 0-6 to equal 0.10 and only script 7 to differ.
  - rival-ai.test.cjs compares scripts 0, 1, 4, 5 and 6 with 0.9.
  - apply-sfx.py and the tests expect exactly 8 scripts.

TASKS
1. Rust: gate the LCXL and forward raw MIDI.
   - In InputMode::Game, the LCXL callback emits the raw message bytes as a new event, lcxl-midi, and skips on_lcxl. Sampler mode is unchanged.
   - Never drop a note on or off. CC bursts may coalesce to the latest value per controller, because a fader drag sends hundreds of messages a second. The final value must always arrive.
   - Put the routing decision somewhere testable, such as an AppState method. Cargo tests: game mode leaves the mixer, synth and autosave untouched and forwards the message; sampler mode behaves as before.
2. App bridge.
   - While protoActive(), forward lcxl-midi to the frame as {lpd:'lcxl', data}.
   - Send {lpd:'lcxl-status', state} on frame load, on registry changes and on req_status.
   - Add Dustline to the game bar (id 'dustline', title 'DUSTLINE', desc 'twin-engine racer') and to ALL_GAMES (requiredHardware ['Launch Control XL']).
   - Leave the Launchpad bridge as it is.
3. Generic storage bridge for frame games.
   - The app keeps small per-game values in its own storage under lpd.game.<id>.<key>, with a cap of 32 KB per game.
   - The game needs them synchronously at boot. Recommended: pass them in the iframe src fragment (#lpd-store=<encodeURIComponent(JSON)>) and update them with {lpd:'store-set', key, value}.
   - Compute the src once per game load, not reactively from the store, so the frame never reloads mid-race.
   - If you find a cleaner synchronous path, propose it to the user before building it.
4. Dustline 0.12 host adapter.
   - Put it in a marked block at the start of script 7: /* @dustline-host-bridge:begin */ ... /* @dustline-host-bridge:end */.
   - It is active only when window.parent !== window. Opened directly, the page behaves exactly as 0.11.
   - It provides a navigator.requestMIDIAccess that returns a Web MIDI-shaped access object: one input {name:'Launch Control XL', manufacturer:'Novation', state, onmidimessage}, no outputs, and sysexEnabled false.
   - Only {lpd:'lcxl'} messages feed onmidimessage({data: Uint8Array}). Launchpad {lpd:'midi'} traffic must never reach DustlineMIDI.
   - lcxl-status updates the port state and fires access.onstatechange, so the existing disconnect and reconnect logic runs.
   - It connects automatically once the bridge reports the LCXL connected.
   - It provides a localStorage shim filled from the boot store; setItem posts store-set.
   - Keep scripts 0-6 byte-identical and keep 8 scripts. Bump the version to '0.12.0' and update the title, topbar and footer strings the way 0.11 did.
   - Update the Dustline tests for 0.12: 0.12 equals index.html; 0.11, 0.10 and 0.9 are preserved by SHA-256; scripts 0-6 still equal 0.10.
   - Update apply-sfx.py and the README (0.12, how it runs in the app, the truth about LEDs).
   - Copy the build to public/games/dustline/index.html and add a check that the copy equals prototypes/Dustline/index.html.
5. Verification.
   - Run every Dustline check: open-circuit, rival-ai, timing-events and audio-engine (node); timing-browser, audio-browser and audio_audition (python).
   - Add checks/host-bridge.py. It serves the folder on 127.0.0.1 with no network access and loads the build in <iframe sandbox="allow-scripts"> on a harness page that plays the app's role. Assert:
     - CC 77 and CC 84 move the left and right engines;
     - a learned button works, and the binding survives a reload through the store;
     - a disconnect pauses an active MIDI session, and reconnecting requires fader acquisition;
     - Launchpad messages are ignored;
     - opened directly, the page still uses real Web MIDI;
     - no page errors.
   - In the app, run cargo test (PowerShell: $env:PATH = "$env:USERPROFILE\.cargo\bin;" + $env:PATH), npm test and npm run check.
   - If feasible, run the app (npm run tauri dev, or a release build with --no-bundle) and smoke-test GAMES > DUSTLINE over CDP (WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<p>). Check keyboard focus inside the frame, full screen, and the layout in the game plate.
6. Final report: what changed, every check with its result, and a hardware checklist for the user. Do not claim any of these yourself:
   - the LCXL faders drive the engines in the app while the drummer mixer stays put;
   - learned buttons persist across an app restart;
   - unplugging and replugging recovers;
   - latency feels right;
   - game audio and drums come out of the intended device (WASAPI vs ASIO).

RULES
- Work inline, with no subagents.
- The launchpad-drummer working tree has uncommitted sector-defense work in these places:
  - public/games/index.html
  - public/games/shared/launchband-menu.js
  - src/App.svelte
  - public/games/sector/
  - src/lib/sector.test.ts
  - tools/test-sector-browser.py
  - docs/superpowers/plans/2026-11-03-sector-defense.md
  Before editing anything, run git status and ask the user whether that work is committed. Never revert, reformat or commit it.
- Create the branch dustline-in-app from main, and commit only when the user says so.
- Dustline's HTML uses uniform CRLF. Normalise to LF, edit, write back as CRLF, and assert the script bytes. Long Python heredocs fail in the Bash tool, so write the script to a file and run it.
- Ask when anything is ambiguous, and record each decision in the Dustline README.
