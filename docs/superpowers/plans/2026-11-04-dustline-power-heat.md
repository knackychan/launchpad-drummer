# Dustline power, heat and alignment

Active request: implement inline, without subagents or commits. Preserve the existing working tree and archived releases. The three current 0.12 HTML files must remain byte-identical with CRLF.

Baseline: SHA-256 `4d3694b391bf2b8f0db6f4b19c992b7ba4d10ccb8c010707f7468d5509090e12`, saved in `.verification/dustline-power/index-before.html`. Existing 26 timing/input and 13 MIDI LED checks pass. Baseline measurements: `prototypes/Dustline/checks/power-baseline-v0.12.json`.

- [x] Shared 600/1200 km/h world-speed model, calibrated fader boost with hysteresis/rearming, independent engine heat and fair rival controls.
- [x] Permanent cooling/repair knob bindings, absolute alignment with fresh movement and dwell, safe scheduling, bounded rewards and real LCXL protocol feedback.
- [x] Twin vertical power/thermal instruments lower left; speed, charge and boost status lower right; actual controls and keyboard equivalents.
- [x] Focused logic, frame-rate, MIDI/LED, browser and app checks; measurements and visual inspection.
- [x] Rebuild using `build.bat`, inspect GAMES → DUSTLINE in the native release, document hardware limits and training checklist.

Scope includes simulation, MIDI, mixer, HUD/game wiring, engine signal handling, Rust output only where needed, tests, README and current release copies. Preserve route and craft/world assets. No dependencies. No physical-light or handling acceptance claims from synthetic tests. The screenshot mentioned in the pasted request was not attached; use its written composition.

## Completion evidence

- New focused coverage: `prototypes/Dustline/checks/power-heat.test.cjs`, `power-browser.py` and `power-scenarios.cjs`. Latest source hash and measurements live in `power-current-v0.12.json`; the preceding source/measurements remain in the baseline artifacts.
- 101 deterministic game checks, five browser suites, 168 app tests, Svelte checking and Rust workspace tests passed (two existing performance tests ignored). Production and rebuilt native-release smoke passed. Detailed logs: `.verification/dustline-power/`.
- Native registry observed a connected Launch Control XL before synthetic injection. Knob input → alignment → supported LED packet → Rust IPC path was exercised, including custom-cue priority and unknown-template fallback. This does not establish visible physical lights.
- Actual motion reaches 598.9 / 1199.9 km/h after three seconds. Two cooling holds through the normal scheduler extend boost from 3.67 to 5.54 seconds, ending on charge rather than heat. The benchmark includes reaction time and respects pacing; no direct event starts bypass its cooldown. Successful cooling now waits 1.5 seconds before another prompt, allowing attention to the other engine.
- README records current HUD/control map, exact tuning, speed/steering/braking measurements, build workflow, lighting trace and physical training checklist. Current copies remain identical CRLF; archived releases and unrelated scripts are protected by checks. No commits or subagents.

User has now confirmed visible knob blinking. Still open for human acceptance: clear target identification during driving, complete LED color/timing, fader/knob reach, subjective handling, latency and listening. Offline audio audition was not rerun; the graph is unchanged but its control signals now follow independent heat and continuous boost.


## Follow-up: knob discovery and button restoration

User confirmed blinking but could not readily identify the knob, and found Track Focus/Control vent/repair missing. Automatic alignment had replaced those event paths and `hitAction` ignored alignments.

- Added a 3×8 physical knob diagram and explicit TOP/MIDDLE LEFT/RIGHT KNOB labels; custom CC/channel bindings never imply an unknown physical location.
- During alignment, only the requested knob blinks; other knob meters stay dim and steady. The matching button is solid amber.
- Focus 1 selects the existing timed vent; Control 1 selects the existing two-hit repair. Release and subsequent timing hits are required. The initial selection gives no reward, preserves the affected engine and cannot refill charge. Existing button edges, wrong-button rejection, mappings and recovery remain in use.
- 58 focused checks, browser interaction/layout, host integration and rebuilt native-release smoke cover the change. Native smoke includes Focus 1 vent plus both stabilizer/power Control 1 repair sequences. No commits or subagents.
