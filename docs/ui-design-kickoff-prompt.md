I want to lock the UI design for my finger-drumming app before any UI code is written. This session is design only: no app code, no scaffolding.

## Context (read these first)

Repo: `D:\MY DOCS\MY TOOLS\REVERSE-ENGINEERING-ANYTHING\launchpad-drummer`

- `docs/superpowers/specs/2026-10-17-drum-sampler-design.md`: approved Part 1 spec. Section 8 lists the UI parts, section 14 the later parts (training, mini-games, band).
- `docs/superpowers/plans/2026-10-17-plan-1a-core-and-cli.md`: Plan 1A (Rust core engine + CLI player, no UI). Not started yet. The UI comes in Plan 1B.
- `docs/reference/melodics-pad-lesson.png`: Melodics screenshot I want the future training mode (Part 2) to resemble.

What it is: a Windows desktop app (Tauri 2 + Svelte 5 + TypeScript) for finger drumming on a Novation Launchpad MK2 (8×8 RGB pads, 8 side buttons, 8 top buttons). A Novation Launch Control XL is the mixer (8 faders, 24 knobs, 16 buttons). Rust handles all real-time audio and MIDI, so the UI is never in the path of a pad hit. It only edits the kit and shows state, which arrives as events at about 30 Hz.

Facts that constrain the design:
- 8 banks × 64 pads. Each pad has: sample, colour, volume, pitch (±24 st), trim start/end, one-shot/hold, choke group (none or 1–8), mix group (1–8).
- Pad colours are Launchpad RGB with 0–63 per channel. The colour picker should only offer what the hardware can show.
- The pads are not velocity-sensitive (always 127). Per bank: light modes (flash / play-state / ripple toggles) and a humanize amount.
- Mixer: 8 groups (volume, pan, pitch offset, low-pass, mute, solo) plus master. It mirrors the Launch Control XL 1:1.
- Drag and drop: Explorer file → pad (the file gets copied into the set). Pad → pad moves, dropping on a filled pad swaps, Ctrl copies. Hovering a bank tab while dragging switches bank. A sample browser sidebar lets me audition sounds and drag them onto pads.
- States to design: device connected / disconnected / port busy / Launchpad in bootloader ("run the updater"), missing sample (that pad blinks red), audio dropouts, set loaded from backup.
- Settings: audio backend (ASIO/WASAPI), device, buffer size, measured latency, dropout counter, MIDI device list. MIDI learn: right-click any control → Learn.
- Later parts need a place in the navigation from day one. Part 2 is the Melodics-style training mode: right-to-left lanes, approach lights on the pads, gamification. Part 3 is mini-games drawn on the Launchpad grid. Part 4 is band mode (keyboards, SoundFonts, looper).

## What I want from this session

1. Use the superpowers brainstorming process. Ask one question at a time and offer the visual companion for mockups.
2. Settle the open decisions:
   - visual direction (a studio-hardware feel, dark by default?);
   - overall layout and navigation (single window? tabs or a side rail for Play / Mixer / Train / Games / Settings?);
   - how the 8×8 grid mirror looks and behaves (live LED state vs. the pad's set colour);
   - where the pad editor lives (side panel, inspector, popover);
   - bank switching UI;
   - mixer placement;
   - where the sample browser goes;
   - empty/onboarding state when no Launchpad or no set exists;
   - keyboard shortcuts;
   - how the Part 2 training screen fits the same design language.
3. Produce clickable HTML mockups of the main screens (Play/grid + pad editor, Mixer, Settings, a rough Training screen) so I can judge them.
4. Write the result as a UI design spec at `docs/superpowers/specs/<date>-ui-design.md` and commit it. It should cover the design tokens (colour, type, spacing), the screen layouts, components and their states, interactions, drag-and-drop rules, and accessibility basics. Then ask me to review it.

Constraints: desktop only, minimum window about 1280×800. Keep it lean: no features beyond the spec unless you ask me first.
