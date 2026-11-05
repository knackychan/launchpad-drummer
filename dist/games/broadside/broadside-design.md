# Broadside: Naval Warfare — Design Document (v2.0)
*Asymmetric 2-Player Co-op for Novation Launch Control XL and Launchpad MK2*

---

## 1. Executive Summary & Vision

**Broadside v2.0** is an asymmetric 2-player cooperative naval warfare simulator built around physical music performance hardware:
- **Player 1 (The Gunner) commands the Fleet Gunnery on the Novation Launch Control XL**:
  - Direct control of **8 naval cannon channels** across the entire 3-warship flotilla:
    - **Channels 1–4**: Heavy Main Batteries on the **Flagship (HMS Dreadnought)**.
    - **Channels 5–6**: Twin Forward & Aft mounts on **Escort Destroyer DD-1 Swift**.
    - **Channels 7–8**: Twin Forward & Aft mounts on **Escort Destroyer DD-2 Valor**.
  - Adjusts elevation angle, selects munitions (AP vs HE), calibrates power/charge faders, and discharges thunderous salvos.
  - **Co-op Dependency**: Turrets only have small ready-racks (3–4 rounds). Once spent, the gunner cannot fire until calling for ammunition, requiring the Engineer to hoist shells up from the deep magazine!
- **Player 2 (The Engineer) commands Damage Control, Fleet Logistics & Point Defense on the Novation Launchpad MK2**:
  - The 8×8 RGB matrix illuminates the internal compartments and structural blueprint of the active warship.
  - Switches between warships using top-row pads: **[FLG] HMS Dreadnought**, **[SWF] DD-1 Swift**, and **[VAL] DD-2 Valor**.
  - Dispatches damage control parties to extinguish spreading fires (crimson pads), weld hull breaches and run bilge pumps against flooding (aqua/blue pads), crank shell hoists (indigo/amber pads), and maintain boiler propulsion.
  - Issues fleet helm and maneuver orders via dedicated side buttons (Flank speed, Evasive weave, Emergency smoke).
  - **Tactical Radar & Point Defense (Key `RDR`)**: Switches the Launchpad to a real-time 8×8 sector radar to detect and intercept incoming **ballistic rockets**, **guided cruise missiles**, and **underwater torpedoes** using targeted **Flak Bursts** and **Depth Charges**!

---

## 2. Hardware Mapping Contracts

### A. Launch Control XL (Player 1: Gunnery Console)

#### Configurable Fleet Size & Fader 7 Hardware Safeguard:
Due to physical hardware variance (such as a faulty 7th fader on the Launch Control XL), the flotilla is **configurable via the top bar menu or URL query argument**:
- **Default Mode: 1 Boat (4 Turrets · CH 1–4)** (`?boats=1` or `?fleet=1`):
  - Focuses control strictly on the **HMS Dreadnought Flagship** (4 turrets on Channels 1–4).
  - Channels 5–8 (including Fader 7) are **completely bypassed, unlit, and ignored** in code, guaranteeing zero jitter or phantom inputs!
- **2-Boat Mode: 2 Boats (6 Turrets · CH 1–6)** (`?boats=2` or `?fleet=2`):
  - Controls Flagship (CH 1–4) + DD-1 Swift (CH 5–6). Total: 6 channels.
  - Channels 7 & 8 remain bypassed and disabled, keeping Fader 7 safely isolated.
- **3-Boat Mode: Full Fleet (8 Turrets · CH 1–8)** (`?boats=3` or `?fleet=3`):
  - Deploys all 3 warships across all 8 channels.

```
+-----------------------------------------------------------------------------------------+
| NOVATION LAUNCH CONTROL XL — FLEET GUNNERY DESK                                         |
| [ CH 1..4: HMS DREADNOUGHT (FLAG) ] | [ CH 5..6: DD-1 SWIFT ] | [ CH 7..8: DD-2 VALOR ] |
| (Default: CH 1-4 active, CH 5-8 bypassed for Fader 7 hardware safeguard)                |
+-----------------------------------------------------------------------------------------+
| KNOB ROW 1: Turret Elevation (5°–80°)                  (CC 13..20)                      |
| KNOB ROW 2: Munition Selector (AP / HE)                (CC 29..36)                      |
| KNOB ROW 3: Fine Trim & Convergence (-3.0° to +3.0°)   (CC 49..56)                      |
+-----------------------------------------------------------------------------------------+
| BTN ROW 1:  [CALL AMMO HOIST]                          (Notes 41..44, 57..60)           |
|             (Solid Green = Full, Amber = Low, Pulsing Red = Empty Rack)                 |
| BTN ROW 2:  [FIRE CANNON!]                             (Notes 73..76, 89..92)           |
|             (Green = Ready, Amber = Cycling, Red = Empty / Damaged)                     |
+-----------------------------------------------------------------------------------------+
| FADERS:     Propellant Powder Charge (30% to 100% Muzzle Velocity) (CC 77..84)          |
+-----------------------------------------------------------------------------------------+
| SIDE BTNS:  [SOLO: Escort Salvo / Aft Salvo]  /  [REC ARM: Flagship Salvo CH1-4]        |
+-----------------------------------------------------------------------------------------+
```

#### Gunnery Mechanics:
1. **Ready-Rack Limitation**: Each turret has a localized ready-rack holding 3 shells (flagship) or 4 shells (destroyers). Firing consumes ready rounds. When empty, the gunner clicks/presses the **CALL AMMO** button, which signals the Engineer with an alert tone and flashing hoist LED.
2. **Propellant Fader**: Modulates muzzle velocity, shaping ballistic arcs from flat armor-piercing trajectories to plunging mortar deck-fire.
3. **Ammo Types**:
   - **AP (Armor Piercing)**: High kinetic energy; pierces thick waterline belt armor and detonates interior machinery/citadels.
   - **HE (High Explosive)**: Explodes on deck impact; causes hull fragmentation and ignites raging compartment fires.

---

### B. Launchpad MK2 (Player 2: Damage Control, Fleet Helm & Point Defense)

```
Top Row (CC 104..111):
[ 104: FLG ]  [ 105: SWF ]  [ 106: VAL ]  [ 107: RDR ]  [ 108: D/C ]  [ 109: SMK ]  [ 110: REV ]  [ 111: AHD ]
 Flagship      Swift DD      Valor DD      Radar / PD    All-Hands DC  Smoke Screen  Full Astern   Full Ahead

8×8 Matrix View 1: Boat Blueprint (When FLG, SWF, or VAL active)
  Cols:  0 (Stern)   1          2         3 (Mid)       4          5          6      7 (Bow)
+------+-----------------------------------------------------------------------------+
| R0   |  .          .          .         [DIR] Range   [BRG] Helm .          .       .  | -> Superstructure
| R1   |  .         [T4]       [T3]       Superstruct   Superstr  [T2]       [T1]     .  | -> Main Turrets
| R2   |  Superstr  [H4] Hoist [H3] Hoist Superstruct   Superstr  [H2] Hoist [H1]    Sup | -> Munitions Hoists
| R3   |  Hull      [M4] Mag   [M3] Mag   Hull Armor    Hull      [M2] Mag   [M1]    Hul | -> Deep Magazines
| R4   |  Hull       Hull      [BLR]      [BLR] Boiler  [BLR]     [BLR]      Hull    Hul | -> Propulsion
| R5   |  Hull       Hull      [BLR]      [BLR] Boiler  [BLR]     [BLR]      Hull    Hul | -> Boilers
| R6   |  .          Hull       Hull       Hull          Hull      Hull       Hull    .  | -> Double Bottom
| R7   |  .          .         [PMP] Pump [PMP] Pump    [PMP]     [PMP]      .       .  | -> Bilge Keel
+------+-----------------------------------------------------------------------------+
Side Buttons (Notes 19..89):
[SPD+] Flank Speed | [SPD-] All Stop | [EVAD] Weave | [SMK] Smoke | [PUMP] All Bilge | [HST] Hoist All | [FLAK] Flak Barrage | [DPTH] Depth Charges
```

```
8×8 Matrix View 2: Tactical Radar & Point Defense (When RDR active)
  Cols:  0 (At Fleet)  1       2       3 (Midfield)  4       5       6       7 (Horizon)
+------+-----------------------------------------------------------------------------+
| R0..4| AIRSPACE SECTORS (Rows 0 to 4):                                             |
|      | • Red flashing blips = Incoming Ballistic Rockets (high arc, fast)          |
|      | • Magenta/Violet blips = Incoming Cruise Missiles (low altitude, guided)     |
|      | Tapping any air sector launches a POINT-DEFENSE FLAK BURST!                 |
+------+-----------------------------------------------------------------------------+
| R5..7| SEA DEPTH SECTORS (Rows 5 to 7):                                            |
|      | • Aqua/Cyan flashing blips = Incoming Torpedoes (waterline foam wake)        |
|      | Tapping any water sector drops an ANTI-TORPEDO DEPTH CHARGE!                |
+------+-----------------------------------------------------------------------------+
```

---

## 3. Co-op Interdependence & Combat Dynamics

| Condition | Cause | Consequence | Engineer Countermeasure |
|---|---|---|---|
| **Empty Ready-Rack** | Gunner fired 3–4 rounds | Turret cannot fire | Tap/crank Hoist `H#` or side `HST` to lift shells |
| **Spreading Fires** | Enemy HE shell or Rocket hit | Progressive hull HP drain | Tap burning pads (Red) to extinguish blaze |
| **Waterline Breach** | Torpedo or AP shell hit | Flood % rises, slows boat, risks sinking | Tap flooded pads (Blue) to weld hull, click `PMP` |
| **Boiler Damage** | Engine room hit | Fleet speed halved; cannot dodge torpedoes | Repair `BLR` pads |
| **Bridge Damage** | Conning tower hit | Helm orders disabled | Repair `BRG` pad |
| **Director Damage** | Mast / Rangefinder hit | Gunner shell dispersion triples | Repair `DIR` pad |
| **Incoming Rockets** | Rocket barge salvo | Starts multiple heavy fires across deck | Intercept on Radar with **Flak** before impact |
| **Cruise Missiles** | Armored cruiser / Boss launch | Pierces citadels and knocks out systems | Intercept on Radar with **Flak** before impact |
| **Incoming Torpedoes**| Torpedo boat launch | Catastrophic flooding and hull destruction | Intercept on Radar with **Depth Charges** |

---

## 4. Hardware Connectivity & Solo Testing

- **Bi-Directional MIDI Support**:
  - Auto-detects **Novation Launch Control XL** and **Novation Launchpad MK2** via Web MIDI API with SysEx enabled.
  - Full LED visual mirrors for reload status, fire readiness, fire damage, flooding, and radar threat blips.
- **Solo Playback Support**:
  - Full mouse drag and click interaction on all on-screen knobs, faders, and pads.
  - Keyboard shortcuts (`1–8` for gunnery, `Shift+1–8` for hoists, `Q/W/E` for boat blueprints, `R` for radar point defense, arrow keys for helm).
  - Built-in **AI Engineer Assist Mode (`🤖 Eng. Assist: ON`)** that autonomously cranks hoists, puts out fires, and intercepts threats for solo players.
