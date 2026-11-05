# Dustline controller lights

Continue the existing `dustline-in-app` working tree without committing or changing the sector work. The 2026-11-03 input-only decision is superseded by this request. Keep archived 0.11/0.10/0.9 unchanged; ship the enhancement in the three identical 0.12 files.

- [x] Send validated LCXL LED messages from the sandbox through the app's existing MIDI output; coalesce LED values and clear them on game exit/switch, then restore sampler lights.
- [x] Add stock button defaults: Track Focus 1 Vent, Track Control 1 Repair (user confirmed); Focus 2/7 shunts, Focus 8 Boost, Control 8 Airbrake. Retain custom learning and legacy shared QTE bindings.
- [x] Blink the required QTE button and knob target; show hit window/results, fader acquisition, heat and input feedback. Keep timing/handling/audio unchanged.
- [x] Extend protocol, MIDI and sandbox tests; run app checks and browser smoke tests. Update the source README with controls and remaining physical checks.

Scope: Rust LCXL output worker and command registration, Svelte frame bridge, Dustline MIDI/game wiring, existing tests and release copies. MIDI input must continue bypassing sampler edits/autosave in game mode. Only LED writes and template selection are allowed through the output boundary. No new dependencies.

Validation completed 2026-11-04: 75 Dustline Node checks; timing-browser, host-bridge, audio-browser and audio_audition; cargo test --workspace (two existing performance tests ignored), then 16 focused LCXL checks after final edits; 168 app JavaScript tests; Svelte check/build; native debug build; production and native WebView2 smoke tests. Native network observations confirmed successful LED/template IPC and cleanup; inputs were synthetic. The three 0.12 copies have identical SHA-256 `39f4fdacb2ede015c37a8be6bf1ef98b1065c9c60a4ead6b7ae8790082e8753c`. Previous releases retain their pinned hashes.

Physical acceptance remains: visible LED colors/blinks, real fader/button latency, unplug/replug, app-restart persistence and intended audio device. See `prototypes/Dustline/README.md` for the control map and full check results. No commits made. Test app/server processes were closed.
