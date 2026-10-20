# Cockpit — Version History

Each significant iteration of the Cockpit prototype is saved here as a self-contained single-file HTML snapshot (`three.min.js` r159 inlined for offline use). The active entry point `prototypes/cockpit/index.html` always mirrors the latest version.

## Versions

- **`cockpit-v2.1.html`** *(Current — Clean Pastel Modular 3D Edition)*
  - **Clean Pastel Ligne-Claire Rendering**: Removed low-res `576×324` nearest-neighbor pixelation and Bayer dither crunch (`Crush: Off`, `CRT: Off` by default). Render targets (`rtNormal`, `rtColor`, `rtPost`) now render at native high-DPI resolution (`1152×648+`) with smooth 3-tone pastel cel shading, anti-aliased ink outlines, and a `3×` resolution vector HUD overlay.
  - **Aggregated Ship Equipment Modules**: Consolidated the dashboard from 22 repetitive mini-gadgets into **7 unique, large multi-cell 3D ship equipment modules** seated inside dark recessed foam bays (`COMMS` diagnostic monitor on `A3..B3`, `VENT` louvre unit on `A2..B2`, `BREAKER` unit on `A1..B1`, `SHD LINK` patch bay on `C3..C2`, `GUN FEED` patch bay on `F3..F2`, `COOLANT` cryo regulator on `G3..H3`, and `FUSE` reactor canister on `G2..H1`), plus the central Hooded Tactical Monitor (`D3..E3`), Flush System Telemetry Strip (`D2..E2`), and Flight Yoke (`D1..E1`). Clicking any cell of a multi-cell module operates that module.
  - **Centered Canopy Enemy Framing**: Restricted enemy and asteroid spawning to the clear central canopy glass (`rows 8..5, cols B..G` primary) with clamped 3D hull bounds and camera-plane alignment so ships never appear off-screen or behind cockpit struts, and 2D target rings track the 3D weak points directly.

- **`cockpit-v2.0.html`**
  - Initial 3D Three.js port of the 9×9 Launchpad MK2 cockpit prototype with cel-shaded slabs, enemy ship footprints (`1×1` drone, `2×1` fighter, `3×2` cruiser, `4×3` capital), asteroids, repairables, Web MIDI MK2 SysEx LED output + hot-plug reconnect, and 2D HUD overlay.
