# Cockpit 3D — Design Lock (v2.1 · Clean Pastel Modular Edition)

## 1. Core Identity & Grid Contract
- **1:1 Launchpad MK2 Mapping**: The entire screen (`576×324` logical grid, rendered at native high-DPI resolution) maps directly to the `9×9` Launchpad MK2 grid (`8×8` main square pads + `1` top row of round buttons `T1..T8` + `1` right column of round scene buttons `S8..S1` + `T9` status corner).
- **Logical Cell Dimensions**: `CW = 64`, `CH = 36` (`9 × 64 = 576`, `9 × 36 = 324`).
- **Regions**:
  - **Canopy Viewport (`Rows 8..4`, `Cols 1..8`)**: Widescreen 16:9 X-Wing style canopy looking out into deep 3D space. Enemy ships and asteroids spawn in the unobstructed central canopy window (`Rows 8..5`, `Cols B..G` primary) so 100% of every ship's hull and weak points stay clearly visible inside the glass.
  - **Aggregated Ship Equipment Dashboard (`Rows 3..1`, `Cols 1..8`)**: Instead of 22 repetitive mini-gadgets, the dashboard is organized into **7 unique, multi-cell 3D ship equipment modules** (plus the central Tactical Display and Flight Yoke), seated inside dark recessed foam bays. Clicking any cell covered by a multi-cell module routes to that module's anchor cell (`canonCell`).
  - **Side Command Rack (`Col 9`, `Rows 8..1` + `T9`)**: Grouped into 4 clean sub-racks (`SHIELD` on `S8`, `GUN A / B` on `S7..S6`, `WPN / SHD / ENG` Power Bus on `S5..S3`, and `DRONE / CHARGE` Utility Bay on `S2..S1`) below the `T9` Score/Wave monitor.

## 2. Aggregated Dashboard Modules (Zero Duplicate Systems)
| Module | Cells Covered | Anchor Cell | Type | Interaction |
|---|---|---|---|---|
| **COMMS Monitor** | `A3..B3` (`2×1`) | `A3` | `screen` | Press `A3` or `B3` to reset comms static |
| **VENT Louvre** | `A2..B2` (`2×1`) | `A2` | `vent` | Hold `A2` or `B2` to cycle jammed louvres |
| **BREAKER Unit** | `A1..B1` (`2×1`) | `A1` | `switch` | Press `A1` or `B1` to flip dual-pole breaker |
| **SHD LINK Bay** | `C3` → `C2` (`1×2`) | `C3` / `C2` | `wireA` / `sockA` | Grab loose cable at `C3`, plug into `C2` |
| **TACTICAL Core** | `D3..E2` (`2×2`) | — | `display` | Radar scope, Hull/Shield bars, Target lock & System bus |
| **FLIGHT Yoke** | `D1..E1` (`2×1`) | `D1` / `E1` | `stickL` / `stickR` | Bank/Roll left (`D1`) or right (`E1`) |
| **GUN FEED Bay** | `F3` → `F2` (`1×2`) | `F3` / `F2` | `wireB` / `sockB` | Grab loose cable at `F3`, plug into `F2` |
| **COOLANT Valve** | `G3..H3` (`2×1`) | `H3` | `valve` | Tap `G3` or `H3` `×3` to spin handwheel |
| **FUSE Canister** | `G2..H1` (`2×2`) | `G2` | `fuse` | Press `G2, H2, G1,` or `H1` to reseat reactor canister |

## 3. Clean Pastel Ligne-Claire Art Direction
- **High-Resolution Anti-Aliased Rendering**: `rtNormal`, `rtColor`, and `rtPost` scale dynamically to native high-DPI canvas resolution (`1152×648` minimum) with `LinearFilter`, eliminating low-res pixelation and Bayer dither crunch (`Crush: Off` and `CRT: Off` by default, tunable live in `#bar`).
- **Warm Pastel Industrial Palette**:
  - Coral terracotta hull (`#e05a47` / `#b84030`), warm sand/cream module casings (`#efe5ce` / `#d5c5a3`), soft sky-cyan (`#64c4ed` / `#3b96be`), pastel apricot orange (`#f59342`), butter yellow (`#f7d058`), pastel rose (`#f49cbb`), mint phosphor green (`#5ce096`), and dark matte foam recesses (`#303540`).
- **Crisp Ink Outlines**: 8-neighbor depth + normal + part-id (`oid`) edge pass with smooth anti-aliased ink blending (`compMat`) and `3×` resolution vector HUD badges (`ovc`).
