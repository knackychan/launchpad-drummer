# Plan 1A: Core Library, lp-probe and CLI Player — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Play a drum kit from the Launchpad MK2 through a low-latency Rust audio engine, with no UI yet: `lpd-cli new-set` builds a set from a folder of samples, `lpd-cli play` plays it.

**Architecture:** A Rust workspace. `crates/lpd-core` holds pure, hardware-free logic (Launchpad protocol encode/decode, the real-time audio engine, kit files) and is fully unit-tested offline. `tools/lp-probe` verifies the protocol on the real device and writes a protocol table. `crates/lpd-cli` wires `lpd-core` to real hardware with `midir` (MIDI) and `cpal` (audio). Plan 1B will reuse `lpd-core` inside a Tauri app.

**Tech Stack:** Rust stable (edition 2021), `midir 0.10`, `cpal 0.15`, `rtrb 0.3`, `symphonia 0.5`, `serde`/`serde_json`, `anyhow`; dev: `hound`, `tempfile`, `assert_no_alloc`.

**Spec:** `docs/superpowers/specs/2026-10-17-drum-sampler-design.md` (sections 2, 3, 4, 5, 10).

**Conventions for every task:**
- Shell is PowerShell on Windows; run commands from the repo root `launchpad-drummer/`.
- End every commit message with a blank line and `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Run `cargo fmt` before each commit.

---

## File map

| File | Responsibility |
|---|---|
| `Cargo.toml` | Workspace + shared dependency versions |
| `crates/lpd-core/src/lib.rs` | Module list |
| `crates/lpd-core/src/devices/launchpad.rs` | Launchpad MK2 bytes ↔ events, LED messages |
| `crates/lpd-core/src/audio/sample.rs` | `Sample` (decoded stereo audio) |
| `crates/lpd-core/src/audio/voice.rs` | One playing sound: interpolation, fades, trim |
| `crates/lpd-core/src/audio/biquad.rs` | Low-pass filter |
| `crates/lpd-core/src/audio/engine.rs` | Voices, choke, voice limit, 8-group mixer, master |
| `crates/lpd-core/src/audio/host.rs` | One audio callback: commands → render → device channels |
| `crates/lpd-core/src/kit/model.rs` | `Set`/`Bank`/`Pad`/`Mixer` + JSON |
| `crates/lpd-core/src/kit/store.rs` | Atomic save, backup, load fallback, sample import |
| `crates/lpd-core/src/kit/decode.rs` | Audio file → `Sample` |
| `crates/lpd-core/src/kit/params.rs` | `Pad` → engine `PlayParams`, `Group` → `GroupParams` |
| `crates/lpd-core/tests/*.rs` | Offline tests per module |
| `tools/lp-probe/src/main.rs` | Interactive hardware protocol check |
| `crates/lpd-cli/src/main.rs` | `new-set` / `play` commands |
| `crates/lpd-cli/src/player.rs` | Launchpad events → engine commands + LED frames (unit-tested) |
| `crates/lpd-cli/src/audio_out.rs` | `cpal` stream (WASAPI / ASIO) |
| `crates/lpd-cli/src/midi.rs` | `midir` Launchpad connection |

---

### Task 0: Install the Rust toolchain

Nothing is installed yet: no MSVC build tools, no Rust (checked 2026-10-17). `winget` is available.

- [ ] **Step 1: Install Visual Studio 2022 Build Tools with the C++ workload** (UAC prompt; ~10 min)

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

- [ ] **Step 2: Install rustup**

```powershell
winget install --id Rustlang.Rustup -e
```

- [ ] **Step 3: Open a new terminal, then verify**

```powershell
rustup default stable
cargo --version
rustc --version
```
Expected: both print a version (1.80 or newer).

---

### Task 1: Workspace scaffold

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `crates/lpd-core/Cargo.toml`, `crates/lpd-core/src/lib.rs`

- [ ] **Step 1: Create `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = ["crates/lpd-core"]

[workspace.dependencies]
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
midir = "0.10"
cpal = "0.15"
rtrb = "0.3"
symphonia = { version = "0.5", features = ["mp3"] }
hound = "3.5"
tempfile = "3"
assert_no_alloc = "1.1"
```

- [ ] **Step 2: Create `.gitignore`**

```
/target
```

- [ ] **Step 3: Create `crates/lpd-core/Cargo.toml`**

```toml
[package]
name = "lpd-core"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
rtrb = { workspace = true }
symphonia = { workspace = true }

[dev-dependencies]
hound = { workspace = true }
tempfile = { workspace = true }
assert_no_alloc = { workspace = true }
```

- [ ] **Step 4: Create `crates/lpd-core/src/lib.rs`**

```rust
//! Hardware-free core of Launchpad Drummer: device protocols, audio engine, kit files.
```

- [ ] **Step 5: Build**

Run: `cargo build`
Expected: `Finished` with no errors.

- [ ] **Step 6: Commit**

```powershell
cargo fmt
git add Cargo.toml .gitignore crates/lpd-core Cargo.lock
git commit -m "chore: add Rust workspace with lpd-core crate"
```

---

### Task 2: Launchpad MK2 protocol module

Values below are verified on the user's unit (spec §2.2): pads `row*10+col` (11..88), side buttons notes 19..89, top row CC 104..111, release = Note On velocity 0, RGB SysEx `F0 00 20 29 02 18 0B <led> <r> <g> <b> F7`.

**Files:**
- Create: `crates/lpd-core/src/devices/mod.rs`, `crates/lpd-core/src/devices/launchpad.rs`
- Modify: `crates/lpd-core/src/lib.rs`
- Test: `crates/lpd-core/tests/launchpad.rs`

- [ ] **Step 1: Write the failing test** `crates/lpd-core/tests/launchpad.rs`

```rust
use lpd_core::devices::launchpad::*;

#[test]
fn decodes_messages_captured_from_hardware() {
    assert_eq!(decode(&[0x90, 0x0B, 0x7F]), Some(LpEvent::PadDown { x: 0, y: 0 }));
    assert_eq!(decode(&[0x90, 0x0B, 0x00]), Some(LpEvent::PadUp { x: 0, y: 0 }));
    assert_eq!(decode(&[0x90, 0x58, 0x7F]), Some(LpEvent::PadDown { x: 7, y: 7 }));
    assert_eq!(decode(&[0x90, 0x2C, 0x7F]), Some(LpEvent::PadDown { x: 3, y: 3 }));
    assert_eq!(decode(&[0x90, 0x59, 0x7F]), Some(LpEvent::SideDown(0)));
    assert_eq!(decode(&[0x90, 19, 0x7F]), Some(LpEvent::SideDown(7)));
    assert_eq!(decode(&[0x90, 0x59, 0x00]), Some(LpEvent::SideUp(0)));
    assert_eq!(decode(&[0xB0, 0x6C, 0x7F]), Some(LpEvent::TopDown(4)));
    assert_eq!(decode(&[0xB0, 0x6C, 0x00]), Some(LpEvent::TopUp(4)));
}

#[test]
fn note_off_is_a_release() {
    assert_eq!(decode(&[0x80, 0x2C, 0x40]), Some(LpEvent::PadUp { x: 3, y: 3 }));
}

#[test]
fn ignores_unknown_messages() {
    assert_eq!(decode(&[0x91, 11, 127]), None); // other channel
    assert_eq!(decode(&[0x90, 10, 127]), None); // column 0
    assert_eq!(decode(&[0x90, 99, 127]), None); // row 9
    assert_eq!(decode(&[0xB0, 1, 127]), None); // unrelated CC
    assert_eq!(decode(&[0x90, 11]), None); // too short
    assert_eq!(decode(&[0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7]), None);
}

#[test]
fn led_addresses_round_trip_through_decode() {
    assert_eq!(grid_led(0, 0), 11);
    assert_eq!(grid_led(7, 7), 88);
    assert_eq!(side_led(0), 89);
    assert_eq!(side_led(7), 19);
    assert_eq!(top_led(0), 104);
    assert_eq!(top_led(7), 111);
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(decode(&[0x90, grid_led(x, y), 127]), Some(LpEvent::PadDown { x, y }));
        }
    }
    for i in 0..8 {
        assert_eq!(decode(&[0x90, side_led(i), 127]), Some(LpEvent::SideDown(i)));
        assert_eq!(decode(&[0xB0, top_led(i), 127]), Some(LpEvent::TopDown(i)));
    }
}

#[test]
fn rgb_message_bytes() {
    let msgs = rgb_messages(&[(11, [63, 0, 0])]);
    assert_eq!(msgs, vec![vec![0xF0, 0x00, 0x20, 0x29, 0x02, 0x18, 0x0B, 11, 63, 0, 0, 0xF7]]);
}

#[test]
fn rgb_components_are_clamped_to_63() {
    let msgs = rgb_messages(&[(11, [200, 64, 63])]);
    assert_eq!(&msgs[0][8..11], &[63, 63, 63]);
}

#[test]
fn rgb_messages_split_at_80_leds() {
    let leds: Vec<(u8, [u8; 3])> = (0..81).map(|i| (i as u8, [1, 2, 3])).collect();
    let msgs = rgb_messages(&leds);
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].len(), 8 + 80 * 4);
    assert_eq!(msgs[1].len(), 8 + 4);
}

#[test]
fn fixed_sysex_messages() {
    assert_eq!(set_all(0), vec![0xF0, 0x00, 0x20, 0x29, 0x02, 0x18, 0x0E, 0x00, 0xF7]);
    assert_eq!(SELECT_SESSION_LAYOUT, [0xF0, 0x00, 0x20, 0x29, 0x02, 0x18, 0x22, 0x00, 0xF7]);
    assert_eq!(DEVICE_INQUIRY, [0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7]);
}

#[test]
fn matches_port_names_with_device_id_suffix() {
    assert!(is_launchpad_port("Launchpad MK2"));
    assert!(is_launchpad_port("Launchpad MK2 16"));
    assert!(is_launchpad_port("2- Launchpad MK2"));
    assert!(!is_launchpad_port("Launch Control XL"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lpd-core --test launchpad`
Expected: compile error `could not find devices in lpd_core`.

- [ ] **Step 3: Implement**

`crates/lpd-core/src/lib.rs`:
```rust
//! Hardware-free core of Launchpad Drummer: device protocols, audio engine, kit files.

pub mod devices;
```

`crates/lpd-core/src/devices/mod.rs`:
```rust
pub mod launchpad;
```

`crates/lpd-core/src/devices/launchpad.rs`:
```rust
//! Launchpad MK2 protocol (Session layout). Values verified on hardware 2026-10-17,
//! see docs/superpowers/specs/2026-10-17-drum-sampler-design.md §2.2.

/// Port names are "Launchpad MK2" or "Launchpad MK2 <device id>"; Windows may add a "2- " prefix.
pub fn is_launchpad_port(name: &str) -> bool {
    name.contains("Launchpad MK2")
}

pub const DEVICE_INQUIRY: [u8; 6] = [0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7];
pub const SELECT_SESSION_LAYOUT: [u8; 9] = [0xF0, 0x00, 0x20, 0x29, 0x02, 0x18, 0x22, 0x00, 0xF7];
const SYSEX_HEADER: [u8; 6] = [0xF0, 0x00, 0x20, 0x29, 0x02, 0x18];
/// Max LEDs in one RGB SysEx message (Novation MK2 programmer's reference).
pub const MAX_LEDS_PER_MESSAGE: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LpEvent {
    /// x, y in 0..8; (0, 0) is bottom-left.
    PadDown { x: u8, y: u8 },
    PadUp { x: u8, y: u8 },
    /// Right-side round buttons, 0 = top (Volume) .. 7 = bottom (Record Arm).
    SideDown(u8),
    SideUp(u8),
    /// Top-row round buttons, 0 = ▲ .. 7 = Mixer.
    TopDown(u8),
    TopUp(u8),
}

pub fn decode(msg: &[u8]) -> Option<LpEvent> {
    let &[status, num, val] = msg else { return None };
    match status {
        0x90 | 0x80 => {
            let down = status == 0x90 && val > 0;
            let (row, col) = (num / 10, num % 10);
            if !(1..=8).contains(&row) {
                return None;
            }
            match col {
                1..=8 => {
                    let (x, y) = (col - 1, row - 1);
                    Some(if down { LpEvent::PadDown { x, y } } else { LpEvent::PadUp { x, y } })
                }
                9 => {
                    let i = 8 - row;
                    Some(if down { LpEvent::SideDown(i) } else { LpEvent::SideUp(i) })
                }
                _ => None,
            }
        }
        0xB0 if (104..=111).contains(&num) => {
            let i = num - 104;
            Some(if val > 0 { LpEvent::TopDown(i) } else { LpEvent::TopUp(i) })
        }
        _ => None,
    }
}

pub fn grid_led(x: u8, y: u8) -> u8 {
    (y + 1) * 10 + x + 1
}

pub fn side_led(i: u8) -> u8 {
    (8 - i) * 10 + 9
}

pub fn top_led(i: u8) -> u8 {
    104 + i
}

/// RGB SysEx messages for `(led, [r, g, b])`, components 0..=63 (higher values clamp).
pub fn rgb_messages(leds: &[(u8, [u8; 3])]) -> Vec<Vec<u8>> {
    leds.chunks(MAX_LEDS_PER_MESSAGE)
        .map(|chunk| {
            let mut m = Vec::with_capacity(SYSEX_HEADER.len() + 2 + chunk.len() * 4);
            m.extend_from_slice(&SYSEX_HEADER);
            m.push(0x0B);
            for &(led, [r, g, b]) in chunk {
                m.extend_from_slice(&[led, r.min(63), g.min(63), b.min(63)]);
            }
            m.push(0xF7);
            m
        })
        .collect()
}

/// Set every LED to palette colour `index` (0 = off).
pub fn set_all(index: u8) -> Vec<u8> {
    let mut m = SYSEX_HEADER.to_vec();
    m.extend_from_slice(&[0x0E, index & 0x7F, 0xF7]);
    m
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p lpd-core --test launchpad`
Expected: `test result: ok. 9 passed`

- [ ] **Step 5: Commit**

```powershell
cargo fmt
git add crates/lpd-core
git commit -m "feat(core): Launchpad MK2 protocol decode and LED messages"
```

---

### Task 3: `lp-probe` hardware verification tool

Interactive; it has no automated tests. It checks every message the app depends on and writes `docs/launchpad-mk2-protocol.md`.

**Files:**
- Create: `tools/lp-probe/Cargo.toml`, `tools/lp-probe/src/main.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Add the member** — in `Cargo.toml` change the members line to:

```toml
members = ["crates/lpd-core", "tools/lp-probe"]
```

- [ ] **Step 2: Create `tools/lp-probe/Cargo.toml`**

```toml
[package]
name = "lp-probe"
version = "0.1.0"
edition = "2021"

[dependencies]
lpd-core = { path = "../../crates/lpd-core" }
anyhow = { workspace = true }
midir = { workspace = true }
```

- [ ] **Step 3: Create `tools/lp-probe/src/main.rs`**

```rust
//! lp-probe: verifies the Launchpad MK2 protocol on real hardware and writes
//! docs/launchpad-mk2-protocol.md. Run: cargo run -p lp-probe

use anyhow::{anyhow, bail, Context, Result};
use lpd_core::devices::launchpad::{self as lp, LpEvent};
use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use std::io::{self, BufRead, Write};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

struct Probe {
    out: MidiOutputConnection,
    rx: Receiver<Vec<u8>>,
    _input: MidiInputConnection<()>,
    /// (check, bytes/detail, result)
    rows: Vec<(String, String, String)>,
}

fn main() -> Result<()> {
    let (port, mut probe) = connect()?;
    println!("Connected to '{port}'. Close other MIDI apps if this fails.");
    probe.identify()?;
    probe.send(&lp::SELECT_SESSION_LAYOUT)?;
    probe.send(&lp::set_all(0))?;
    probe.led_tests()?;
    probe.input_tests()?;
    probe.send(&lp::set_all(0))?;
    let path = probe.write_report(&port)?;
    println!("\nReport written to {path}");
    Ok(())
}

fn connect() -> Result<(String, Probe)> {
    let mut midi_in = MidiInput::new("lp-probe-in")?;
    midi_in.ignore(Ignore::None);
    let in_port = midi_in
        .ports()
        .into_iter()
        .find(|p| midi_in.port_name(p).is_ok_and(|n| lp::is_launchpad_port(&n)))
        .context("no Launchpad MK2 input port (plugged in? bootloader mode? used by another app?)")?;
    let port_name = midi_in.port_name(&in_port)?;
    let (tx, rx) = mpsc::channel();
    let input = midi_in
        .connect(&in_port, "lp-probe-in", move |_, msg, _| {
            let _ = tx.send(msg.to_vec());
        }, ())
        .map_err(|e| anyhow!("open Launchpad input: {e}"))?;
    let midi_out = MidiOutput::new("lp-probe-out")?;
    let out_port = midi_out
        .ports()
        .into_iter()
        .find(|p| midi_out.port_name(p).is_ok_and(|n| lp::is_launchpad_port(&n)))
        .context("no Launchpad MK2 output port")?;
    let out = midi_out
        .connect(&out_port, "lp-probe-out")
        .map_err(|e| anyhow!("open Launchpad output: {e}"))?;
    Ok((port_name, Probe { out, rx, _input: input, rows: Vec::new() }))
}

impl Probe {
    fn send(&mut self, msg: &[u8]) -> Result<()> {
        self.out.send(msg).map_err(|e| anyhow!("send {}: {e}", hex(msg)))
    }

    fn drain(&self) {
        while self.rx.try_recv().is_ok() {}
    }

    fn record(&mut self, check: &str, detail: impl Into<String>, ok: bool) {
        let result = if ok { "verified" } else { "FAILED" };
        println!("  [{result}] {check}");
        self.rows.push((check.to_string(), detail.into(), result.to_string()));
    }

    fn ask(&mut self, check: &str, msg: &[u8], question: &str) -> Result<()> {
        let ok = yes_no(question)?;
        self.record(check, hex(msg), ok);
        Ok(())
    }

    fn identify(&mut self) -> Result<()> {
        self.drain();
        self.send(&lp::DEVICE_INQUIRY)?;
        let deadline = Instant::now() + Duration::from_secs(2);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            let Ok(m) = self.rx.recv_timeout(left) else { break };
            if m.starts_with(&[0xF0, 0x7E]) && m.get(3..5) == Some(&[0x06, 0x02]) {
                let ok = m.get(5..10) == Some(&[0x00, 0x20, 0x29, 0x69, 0x00]);
                self.record("Device Inquiry reply: Novation, family 0x69 (Launchpad MK2)", hex(&m), ok);
                return Ok(());
            }
        }
        self.record("Device Inquiry reply", "no reply within 2 s", false);
        Ok(())
    }

    fn led_tests(&mut self) -> Result<()> {
        println!("\n== LED tests: answer y/n for what you see ==");
        let m = [0x90, lp::grid_led(3, 4), 5];
        self.send(&m)?;
        self.ask("Palette static: Note On ch1, velocity = palette index", &m,
            "Is the pad in column 4, row 5 (counting from bottom-left) solid red?")?;
        let m = [0x91, lp::grid_led(4, 4), 21];
        self.send(&m)?;
        self.ask("Palette flash: Note On ch2", &m, "Is the pad to its right flashing green?")?;
        let m = [0x92, lp::grid_led(5, 4), 45];
        self.send(&m)?;
        self.ask("Palette pulse: Note On ch3", &m, "Is the next pad to the right pulsing blue?")?;
        let m = lp::set_all(0);
        self.send(&m)?;
        self.ask("Set all LEDs: SysEx 0E <palette index>", &m, "Did every LED turn off?")?;

        let top: Vec<(u8, [u8; 3])> = (0..8).map(|i| (lp::top_led(i), [0, 0, 63])).collect();
        let msgs = lp::rgb_messages(&top);
        for m in &msgs {
            self.send(m)?;
        }
        self.ask("RGB SysEx on top-row LEDs 104..111", &msgs[0], "Are all 8 top-row round buttons blue?")?;

        let side: Vec<(u8, [u8; 3])> = (0..8).map(|i| (lp::side_led(i), [63, 0, 63])).collect();
        let msgs = lp::rgb_messages(&side);
        for m in &msgs {
            self.send(m)?;
        }
        self.ask("RGB SysEx on side LEDs 89..19", &msgs[0], "Are all 8 right-side round buttons purple?")?;

        let grid: Vec<(u8, [u8; 3])> = (0..8u8)
            .flat_map(|y| (0..8u8).map(move |x| (lp::grid_led(x, y), [x * 9, y * 9, 20])))
            .collect();
        let msgs = lp::rgb_messages(&grid);
        for m in &msgs {
            self.send(m)?;
        }
        self.ask("RGB SysEx with 64 LEDs in one message", &msgs[0][..12],
            "Is the grid a smooth gradient (dark blue bottom-left to yellow top-right)?")?;
        self.send(&lp::set_all(0))?;
        Ok(())
    }

    fn input_tests(&mut self) -> Result<()> {
        println!("\n== Input tests: press each control when asked (15 s timeout each) ==");
        let mut expected: Vec<(String, LpEvent)> = vec![
            ("bottom-left grid pad".into(), LpEvent::PadDown { x: 0, y: 0 }),
            ("bottom-right grid pad".into(), LpEvent::PadDown { x: 7, y: 0 }),
            ("top-left grid pad".into(), LpEvent::PadDown { x: 0, y: 7 }),
            ("top-right grid pad".into(), LpEvent::PadDown { x: 7, y: 7 }),
        ];
        let top = ["Up", "Down", "Left", "Right", "Session", "User 1", "User 2", "Mixer"];
        for (i, n) in top.iter().enumerate() {
            expected.push((format!("top-row button {n}"), LpEvent::TopDown(i as u8)));
        }
        let side = ["Volume", "Pan", "Send A", "Send B", "Stop", "Mute", "Solo", "Record Arm"];
        for (i, n) in side.iter().enumerate() {
            expected.push((format!("side button {n}"), LpEvent::SideDown(i as u8)));
        }
        for (name, want) in expected {
            self.drain();
            println!("Press: {name}");
            match self.wait_press(Duration::from_secs(15)) {
                Some(raw) => {
                    let got = lp::decode(&raw);
                    self.record(&format!("Input: {name}"), format!("{} -> {got:?}", hex(&raw)), got == Some(want));
                }
                None => self.record(&format!("Input: {name}"), "timeout", false),
            }
        }
        self.drain();
        println!("Press and HOLD any grid pad for 1 second, then release it.");
        let down = self.wait_press(Duration::from_secs(15));
        let up = self.rx.recv_timeout(Duration::from_secs(15)).ok();
        let ok = matches!(up.as_deref().and_then(lp::decode), Some(LpEvent::PadUp { .. }));
        let detail = format!(
            "down {} / up {}",
            down.as_deref().map(hex).unwrap_or_default(),
            up.as_deref().map(hex).unwrap_or_default()
        );
        self.record("Release decodes as PadUp", detail, ok);
        Ok(())
    }

    /// Next message that decodes as a press; releases and unknown messages are skipped.
    fn wait_press(&self, timeout: Duration) -> Option<Vec<u8>> {
        let deadline = Instant::now() + timeout;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            let m = self.rx.recv_timeout(left).ok()?;
            if matches!(
                lp::decode(&m),
                Some(LpEvent::PadDown { .. } | LpEvent::SideDown(_) | LpEvent::TopDown(_))
            ) {
                return Some(m);
            }
        }
        None
    }

    fn write_report(&self, port: &str) -> Result<String> {
        let mut s = format!(
            "# Launchpad MK2 protocol — verified on hardware\n\nGenerated by `lp-probe` on port `{port}`.\n\n| Check | Bytes / detail | Result |\n|---|---|---|\n"
        );
        for (check, detail, result) in &self.rows {
            s += &format!("| {check} | `{detail}` | {result} |\n");
        }
        let failed = self.rows.iter().filter(|r| r.2 == "FAILED").count();
        s += &format!("\n{} checks, {failed} failed.\n", self.rows.len());
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/launchpad-mk2-protocol.md");
        std::fs::write(path, s).with_context(|| format!("write {path}"))?;
        Ok(path.to_string())
    }
}

fn yes_no(question: &str) -> Result<bool> {
    loop {
        print!("{question} [y/n] ");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().lock().read_line(&mut line)? == 0 {
            bail!("stdin closed");
        }
        match line.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => {}
        }
    }
}

fn hex(m: &[u8]) -> String {
    m.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}
```

- [ ] **Step 4: Build**

Run: `cargo build -p lp-probe`
Expected: `Finished`, no errors.

- [ ] **Step 5: Run on the hardware (user at the Launchpad)**

Run: `cargo run -p lp-probe`
Expected: `Connected to 'Launchpad MK2 16'`, every check prints `[verified]`, ends with `Report written to …docs/launchpad-mk2-protocol.md`.
If any check FAILS: stop and report the failing row; update `devices/launchpad.rs` and spec §2.2 to match what the hardware does before continuing.

- [ ] **Step 6: Commit (code + generated report)**

```powershell
cargo fmt
git add Cargo.toml Cargo.lock tools/lp-probe docs/launchpad-mk2-protocol.md
git commit -m "feat(probe): lp-probe hardware protocol check and verified report"
```

---

### Task 4: Kit data model and JSON

**Files:**
- Create: `crates/lpd-core/src/kit/mod.rs`, `crates/lpd-core/src/kit/model.rs`
- Modify: `crates/lpd-core/src/lib.rs`
- Test: `crates/lpd-core/tests/kit_model.rs`

- [ ] **Step 1: Write the failing test** `crates/lpd-core/tests/kit_model.rs`

```rust
use lpd_core::kit::*;

#[test]
fn new_set_has_8_banks_and_8_groups_with_defaults() {
    let set = Set::new();
    assert_eq!(set.version, 1);
    assert_eq!(set.banks.len(), 8);
    assert_eq!(set.banks[0].name, "Bank 1");
    assert_eq!(set.banks[7].name, "Bank 8");
    assert!(set.banks[0].lights.flash && set.banks[0].lights.play_state && !set.banks[0].lights.ripple);
    assert!(set.banks[0].pads.is_empty());
    assert_eq!(set.mixer.groups.len(), 8);
    assert_eq!(set.mixer.groups[0].lowpass_hz, 20_000.0);
}

#[test]
fn new_pad_defaults_mix_group_to_row() {
    let pad = Pad::new(2, 5, "samples/a.wav", [1, 2, 3]);
    assert_eq!(pad.mix_group, 5);
    assert_eq!(pad.mode, PadMode::OneShot);
    assert_eq!(pad.choke_group, None);
}

#[test]
fn json_round_trip() {
    let mut set = Set::new();
    let mut pad = Pad::new(0, 0, "samples/kick.wav", [63, 10, 0]);
    pad.mode = PadMode::Hold;
    pad.choke_group = Some(2);
    pad.trim_end_ms = Some(120.0);
    set.banks[0].set_pad(pad);
    let back = Set::from_json(&set.to_json()).unwrap();
    assert_eq!(back, set);
}

#[test]
fn json_uses_spec_field_names() {
    let mut set = Set::new();
    set.banks[0].set_pad(Pad::new(0, 0, "samples/kick.wav", [63, 10, 0]));
    let json = set.to_json();
    for key in ["\"volumeDb\"", "\"pitchSemitones\"", "\"trimStartMs\"", "\"chokeGroup\"", "\"mixGroup\"",
        "\"oneShot\"", "\"playState\"", "\"humanizeDb\"", "\"lowpassHz\"", "\"masterDb\""] {
        assert!(json.contains(key), "missing {key} in {json}");
    }
}

#[test]
fn rejects_wrong_version_and_shape() {
    let mut set = Set::new();
    set.version = 2;
    assert!(Set::from_json(&set.to_json()).is_err());
    let mut set = Set::new();
    set.banks.pop();
    assert!(Set::from_json(&set.to_json()).is_err());
    assert!(Set::from_json("not json").is_err());
}

#[test]
fn set_pad_replaces_pad_at_same_position() {
    let mut bank = Set::new().banks.remove(0);
    bank.set_pad(Pad::new(1, 1, "samples/a.wav", [0, 0, 0]));
    bank.set_pad(Pad::new(1, 1, "samples/b.wav", [0, 0, 0]));
    assert_eq!(bank.pads.len(), 1);
    assert_eq!(bank.pad(1, 1).unwrap().sample, "samples/b.wav");
    assert!(bank.pad(0, 0).is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lpd-core --test kit_model`
Expected: compile error `could not find kit in lpd_core`.

- [ ] **Step 3: Implement**

`crates/lpd-core/src/lib.rs` — add the line:
```rust
pub mod kit;
```

`crates/lpd-core/src/kit/mod.rs`:
```rust
//! Sets, banks and pads: data model and files.

mod model;

pub use model::*;
```

`crates/lpd-core/src/kit/model.rs`:
```rust
use serde::{Deserialize, Serialize};

pub const BANKS: usize = 8;
pub const GROUPS: usize = 8;
pub const SET_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Set {
    pub version: u32,
    pub banks: Vec<Bank>,
    pub mixer: Mixer,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bank {
    pub name: String,
    pub lights: LightModes,
    pub humanize_db: f32,
    /// Only pads with a sample; empty pads are omitted.
    pub pads: Vec<Pad>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LightModes {
    pub flash: bool,
    pub play_state: bool,
    pub ripple: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pad {
    /// 0..8, (0, 0) = bottom-left.
    pub x: u8,
    pub y: u8,
    /// Path relative to the set folder, forward slashes.
    pub sample: String,
    /// RGB, each 0..=63 (Launchpad LED range).
    pub color: [u8; 3],
    pub volume_db: f32,
    pub pitch_semitones: f32,
    pub trim_start_ms: f32,
    pub trim_end_ms: Option<f32>,
    pub mode: PadMode,
    /// None or 1..=8.
    pub choke_group: Option<u8>,
    /// 0..8.
    pub mix_group: u8,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PadMode {
    OneShot,
    Hold,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Mixer {
    pub groups: Vec<Group>,
    pub master_db: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub name: String,
    pub volume_db: f32,
    /// -1 = left, 0 = centre, 1 = right.
    pub pan: f32,
    pub pitch_semitones: f32,
    /// 20 000 = filter bypassed.
    pub lowpass_hz: f32,
    pub mute: bool,
    pub solo: bool,
}

impl Pad {
    /// A pad with default settings; mix group = row (spec §4.2).
    pub fn new(x: u8, y: u8, sample: impl Into<String>, color: [u8; 3]) -> Pad {
        Pad {
            x,
            y,
            sample: sample.into(),
            color,
            volume_db: 0.0,
            pitch_semitones: 0.0,
            trim_start_ms: 0.0,
            trim_end_ms: None,
            mode: PadMode::OneShot,
            choke_group: None,
            mix_group: y,
        }
    }
}

impl Bank {
    pub fn pad(&self, x: u8, y: u8) -> Option<&Pad> {
        self.pads.iter().find(|p| p.x == x && p.y == y)
    }

    /// Insert, or replace the pad at the same position.
    pub fn set_pad(&mut self, pad: Pad) {
        self.pads.retain(|p| !(p.x == pad.x && p.y == pad.y));
        self.pads.push(pad);
    }
}

impl Set {
    pub fn new() -> Set {
        let banks = (1..=BANKS)
            .map(|i| Bank {
                name: format!("Bank {i}"),
                lights: LightModes { flash: true, play_state: true, ripple: false },
                humanize_db: 0.0,
                pads: Vec::new(),
            })
            .collect();
        let groups = (1..=GROUPS)
            .map(|i| Group {
                name: format!("Group {i}"),
                volume_db: 0.0,
                pan: 0.0,
                pitch_semitones: 0.0,
                lowpass_hz: 20_000.0,
                mute: false,
                solo: false,
            })
            .collect();
        Set { version: SET_VERSION, banks, mixer: Mixer { groups, master_db: 0.0 } }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("Set always serializes")
    }

    pub fn from_json(s: &str) -> anyhow::Result<Set> {
        let set: Set = serde_json::from_str(s)?;
        anyhow::ensure!(set.version == SET_VERSION, "unsupported set version {}", set.version);
        anyhow::ensure!(
            set.banks.len() == BANKS && set.mixer.groups.len() == GROUPS,
            "a set needs {BANKS} banks and {GROUPS} mix groups"
        );
        Ok(set)
    }
}

impl Default for Set {
    fn default() -> Self {
        Set::new()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p lpd-core --test kit_model`
Expected: `test result: ok. 6 passed`

- [ ] **Step 5: Commit**

```powershell
cargo fmt
git add crates/lpd-core
git commit -m "feat(core): kit data model with spec JSON format"
```

---

### Task 5: Set folder storage (atomic save, backup, import)

Spec §4.1 and §9: write `set.json.tmp` → rotate `set.json` to `set.json.bak` → rename tmp. If `set.json` is unreadable, load the backup and move the broken file to `set.json.broken` (never overwrite it). Imported samples are copied into `samples/` with `-2`, `-3`… on name collision.

**Files:**
- Create: `crates/lpd-core/src/kit/store.rs`
- Modify: `crates/lpd-core/src/kit/mod.rs`
- Test: `crates/lpd-core/tests/kit_store.rs`

- [ ] **Step 1: Write the failing test** `crates/lpd-core/tests/kit_store.rs`

```rust
use lpd_core::kit::*;
use std::fs;

fn set_named(name: &str) -> Set {
    let mut set = Set::new();
    set.banks[0].name = name.to_string();
    set
}

#[test]
fn save_then_load_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), &set_named("A")).unwrap();
    let loaded = load(dir.path()).unwrap();
    assert_eq!(loaded.set.banks[0].name, "A");
    assert!(!loaded.from_backup);
    assert!(!dir.path().join("set.json.tmp").exists());
}

#[test]
fn second_save_keeps_previous_version_as_backup() {
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), &set_named("A")).unwrap();
    save(dir.path(), &set_named("B")).unwrap();
    let bak = fs::read_to_string(dir.path().join(BACKUP_FILE)).unwrap();
    assert_eq!(Set::from_json(&bak).unwrap().banks[0].name, "A");
    assert_eq!(load(dir.path()).unwrap().set.banks[0].name, "B");
}

#[test]
fn corrupt_set_loads_backup_and_keeps_broken_file() {
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), &set_named("A")).unwrap();
    save(dir.path(), &set_named("B")).unwrap();
    fs::write(dir.path().join(SET_FILE), "{ broken").unwrap();
    let loaded = load(dir.path()).unwrap();
    assert!(loaded.from_backup);
    assert_eq!(loaded.set.banks[0].name, "A");
    assert_eq!(fs::read_to_string(dir.path().join(BROKEN_FILE)).unwrap(), "{ broken");
    assert!(!dir.path().join(SET_FILE).exists());
}

#[test]
fn load_fails_when_nothing_is_readable() {
    let dir = tempfile::tempdir().unwrap();
    assert!(load(dir.path()).is_err());
    fs::write(dir.path().join(SET_FILE), "x").unwrap();
    fs::write(dir.path().join(BACKUP_FILE), "y").unwrap();
    assert!(load(dir.path()).is_err());
    assert_eq!(fs::read_to_string(dir.path().join(SET_FILE)).unwrap(), "x");
}

#[test]
fn import_copies_into_samples_with_collision_suffix() {
    let set_dir = tempfile::tempdir().unwrap();
    let src_dir = tempfile::tempdir().unwrap();
    let src = src_dir.path().join("kick.wav");
    fs::write(&src, b"RIFF").unwrap();
    assert_eq!(import_sample(set_dir.path(), &src).unwrap(), "samples/kick.wav");
    assert_eq!(import_sample(set_dir.path(), &src).unwrap(), "samples/kick-2.wav");
    assert_eq!(import_sample(set_dir.path(), &src).unwrap(), "samples/kick-3.wav");
    assert_eq!(fs::read(set_dir.path().join("samples/kick-2.wav")).unwrap(), b"RIFF");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lpd-core --test kit_store`
Expected: compile error `cannot find function save`.

- [ ] **Step 3: Implement**

`crates/lpd-core/src/kit/mod.rs` — replace the whole file with:
```rust
//! Sets, banks and pads: data model and files.

mod model;
mod store;

pub use model::*;
pub use store::{import_sample, load, save, Loaded, BACKUP_FILE, BROKEN_FILE, SAMPLES_DIR, SET_FILE};
```

`crates/lpd-core/src/kit/store.rs`:
```rust
use super::Set;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub const SET_FILE: &str = "set.json";
pub const BACKUP_FILE: &str = "set.json.bak";
pub const BROKEN_FILE: &str = "set.json.broken";
pub const SAMPLES_DIR: &str = "samples";
const TMP_FILE: &str = "set.json.tmp";

#[derive(Debug)]
pub struct Loaded {
    pub set: Set,
    /// true when set.json was unreadable and set.json.bak was used.
    pub from_backup: bool,
}

pub fn load(dir: &Path) -> Result<Loaded> {
    let main = dir.join(SET_FILE);
    let main_err = match read_set(&main) {
        Ok(set) => return Ok(Loaded { set, from_backup: false }),
        Err(e) => e,
    };
    let set = read_set(&dir.join(BACKUP_FILE))
        .with_context(|| format!("set unreadable ({main_err:#}) and backup unreadable"))?;
    if main.exists() {
        fs::rename(&main, dir.join(BROKEN_FILE)).context("move broken set.json aside")?;
    }
    Ok(Loaded { set, from_backup: true })
}

fn read_set(path: &Path) -> Result<Set> {
    let s = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Set::from_json(&s).with_context(|| format!("parse {}", path.display()))
}

/// Atomic save: tmp file, rotate the previous set.json to set.json.bak, rename tmp.
pub fn save(dir: &Path, set: &Set) -> Result<()> {
    fs::create_dir_all(dir)?;
    let tmp = dir.join(TMP_FILE);
    let main = dir.join(SET_FILE);
    fs::write(&tmp, set.to_json()).with_context(|| format!("write {}", tmp.display()))?;
    if main.exists() {
        fs::rename(&main, dir.join(BACKUP_FILE)).context("rotate backup")?;
    }
    fs::rename(&tmp, &main).context("move new set.json into place")?;
    Ok(())
}

/// Copy `src` into the set's samples folder; returns the set-relative path.
pub fn import_sample(dir: &Path, src: &Path) -> Result<String> {
    let samples = dir.join(SAMPLES_DIR);
    fs::create_dir_all(&samples)?;
    let stem = src.file_stem().and_then(|s| s.to_str()).context("sample name is not valid UTF-8")?;
    let ext = src.extension().and_then(|s| s.to_str()).unwrap_or("");
    let name_for = |n: u32| {
        let stem = if n == 1 { stem.to_string() } else { format!("{stem}-{n}") };
        if ext.is_empty() { stem } else { format!("{stem}.{ext}") }
    };
    let mut n = 1;
    while samples.join(name_for(n)).exists() {
        n += 1;
    }
    let name = name_for(n);
    fs::copy(src, samples.join(&name)).with_context(|| format!("copy {}", src.display()))?;
    Ok(format!("{SAMPLES_DIR}/{name}"))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p lpd-core --test kit_store`
Expected: `test result: ok. 5 passed`

- [ ] **Step 5: Commit**

```powershell
cargo fmt
git add crates/lpd-core
git commit -m "feat(core): atomic set save with backup fallback and sample import"
```

---

### Task 6: Sample type and audio file decoding

**Files:**
- Create: `crates/lpd-core/src/audio/mod.rs`, `crates/lpd-core/src/audio/sample.rs`, `crates/lpd-core/src/kit/decode.rs`, `crates/lpd-core/tests/common/mod.rs`
- Modify: `crates/lpd-core/src/lib.rs`, `crates/lpd-core/src/kit/mod.rs`
- Test: `crates/lpd-core/tests/kit_decode.rs`

- [ ] **Step 1: Write the test helpers** `crates/lpd-core/tests/common/mod.rs`

```rust
#![allow(dead_code)]
use std::path::Path;

/// Mono 16-bit WAV where sample i = i * step.
pub fn write_wav_mono_ramp(path: &Path, rate: u32, frames: usize, step: i16) {
    let spec = hound::WavSpec { channels: 1, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..frames {
        w.write_sample((i as i16).wrapping_mul(step)).unwrap();
    }
    w.finalize().unwrap();
}

/// Stereo 32-bit float WAV with constant left/right values.
pub fn write_wav_stereo(path: &Path, rate: u32, frames: usize, left: f32, right: f32) {
    let spec = hound::WavSpec { channels: 2, sample_rate: rate, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for _ in 0..frames {
        w.write_sample(left).unwrap();
        w.write_sample(right).unwrap();
    }
    w.finalize().unwrap();
}
```

- [ ] **Step 2: Write the failing test** `crates/lpd-core/tests/kit_decode.rs`

```rust
mod common;

use lpd_core::kit::*;
use std::fs;

#[test]
fn decodes_mono_wav_to_stereo() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ramp.wav");
    common::write_wav_mono_ramp(&path, 48_000, 480, 50);
    let s = decode_file(&path).unwrap();
    assert_eq!(s.rate, 48_000);
    assert_eq!(s.frames(), 480);
    let (l, r) = s.frame(100);
    assert!((l - 5000.0 / 32768.0).abs() < 1e-4, "got {l}");
    assert_eq!(l, r);
}

#[test]
fn decodes_stereo_wav_keeping_channels() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("st.wav");
    common::write_wav_stereo(&path, 44_100, 100, 0.25, -0.5);
    let s = decode_file(&path).unwrap();
    assert_eq!(s.rate, 44_100);
    assert_eq!(s.frames(), 100);
    assert_eq!(s.frame(10), (0.25, -0.5));
}

#[test]
fn frame_outside_sample_is_silent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("st.wav");
    common::write_wav_stereo(&path, 44_100, 10, 0.25, 0.25);
    let s = decode_file(&path).unwrap();
    assert_eq!(s.frame(-1), (0.0, 0.0));
    assert_eq!(s.frame(10), (0.0, 0.0));
}

#[test]
fn rejects_non_audio_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.wav");
    fs::write(&path, b"definitely not audio").unwrap();
    assert!(decode_file(&path).is_err());
}

#[test]
fn load_samples_reports_failures_and_loads_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("samples")).unwrap();
    common::write_wav_stereo(&dir.path().join("samples/ok.wav"), 48_000, 10, 0.1, 0.1);
    let mut set = Set::new();
    set.banks[0].set_pad(Pad::new(0, 0, "samples/ok.wav", [0, 0, 0]));
    set.banks[1].set_pad(Pad::new(0, 0, "samples/ok.wav", [0, 0, 0]));
    set.banks[0].set_pad(Pad::new(1, 0, "samples/missing.wav", [0, 0, 0]));
    let (ok, failed) = load_samples(dir.path(), &set);
    assert_eq!(ok.len(), 1);
    assert!(ok.contains_key("samples/ok.wav"));
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].0, "samples/missing.wav");
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p lpd-core --test kit_decode`
Expected: compile error `cannot find function decode_file`.

- [ ] **Step 4: Implement**

`crates/lpd-core/src/lib.rs` — final content:
```rust
//! Hardware-free core of Launchpad Drummer: device protocols, audio engine, kit files.

pub mod audio;
pub mod devices;
pub mod kit;
```

`crates/lpd-core/src/audio/mod.rs`:
```rust
//! Real-time audio engine. Nothing in here touches a sound card.

mod sample;

pub use sample::Sample;
```

`crates/lpd-core/src/audio/sample.rs`:
```rust
/// Decoded audio: interleaved stereo f32 at `rate` Hz.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub rate: u32,
    pub data: Vec<f32>,
}

impl Sample {
    pub fn frames(&self) -> usize {
        self.data.len() / 2
    }

    /// Left/right of frame `i`; silence outside the sample.
    #[inline]
    pub fn frame(&self, i: isize) -> (f32, f32) {
        if i < 0 || i as usize >= self.frames() {
            return (0.0, 0.0);
        }
        let j = i as usize * 2;
        (self.data[j], self.data[j + 1])
    }
}
```

`crates/lpd-core/src/kit/decode.rs`:
```rust
use super::Set;
use crate::audio::Sample;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decode an audio file (WAV, FLAC, MP3, OGG) to stereo f32. Mono is duplicated,
/// channels beyond the first two are dropped.
pub fn decode_file(path: &Path) -> Result<Sample> {
    let file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .with_context(|| format!("unrecognised audio format: {}", path.display()))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .context("no audio track")?;
    let track_id = track.id;
    let mut rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut data = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        };
        let spec = *decoded.spec();
        rate = spec.rate;
        let channels = spec.channels.count();
        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buf.copy_interleaved_ref(decoded);
        for frame in buf.samples().chunks(channels) {
            let l = frame[0];
            let r = if channels > 1 { frame[1] } else { l };
            data.push(l);
            data.push(r);
        }
    }
    anyhow::ensure!(!data.is_empty(), "no audio decoded from {}", path.display());
    Ok(Sample { rate, data })
}

/// Decode every sample the set references. Failures are returned per path; the rest load.
pub fn load_samples(dir: &Path, set: &Set) -> (HashMap<String, Arc<Sample>>, Vec<(String, anyhow::Error)>) {
    let mut ok = HashMap::new();
    let mut failed: Vec<(String, anyhow::Error)> = Vec::new();
    for pad in set.banks.iter().flat_map(|b| &b.pads) {
        if ok.contains_key(&pad.sample) || failed.iter().any(|(p, _)| p == &pad.sample) {
            continue;
        }
        match decode_file(&dir.join(&pad.sample)) {
            Ok(s) => {
                ok.insert(pad.sample.clone(), Arc::new(s));
            }
            Err(e) => failed.push((pad.sample.clone(), e)),
        }
    }
    (ok, failed)
}
```

`crates/lpd-core/src/kit/mod.rs` — replace the whole file with:
```rust
//! Sets, banks and pads: data model and files.

mod decode;
mod model;
mod store;

pub use decode::{decode_file, load_samples};
pub use model::*;
pub use store::{import_sample, load, save, Loaded, BACKUP_FILE, BROKEN_FILE, SAMPLES_DIR, SET_FILE};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p lpd-core --test kit_decode`
Expected: `test result: ok. 5 passed`

- [ ] **Step 6: Commit**

```powershell
cargo fmt
git add crates/lpd-core
git commit -m "feat(core): decode audio files to stereo samples"
```

---

### Task 7: Voice (playback, interpolation, fades, trim)

A voice plays one sample at `rate_ratio` source frames per output frame (sample-rate conversion and pitch in one), using 4-point Hermite interpolation. It fades out linearly on request and fades over the last 2 ms before the trim end so it never clicks.

**Files:**
- Create: `crates/lpd-core/src/audio/voice.rs`
- Modify: `crates/lpd-core/src/audio/mod.rs`
- Test: `crates/lpd-core/tests/audio_voice.rs`

- [ ] **Step 1: Write the failing test** `crates/lpd-core/tests/audio_voice.rs`

```rust
use lpd_core::audio::*;
use std::sync::Arc;

fn constant(frames: usize, value: f32) -> Arc<Sample> {
    Arc::new(Sample { rate: 48_000, data: vec![value; frames * 2] })
}

fn params(start: usize, end: usize) -> PlayParams {
    PlayParams { gain: 1.0, pitch_semitones: 0.0, start_frame: start, end_frame: end, choke_group: None, mix_group: 0, hold: false }
}

fn render(v: &mut Voice, frames: usize) -> (Vec<f32>, bool) {
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    let alive = v.render_add(&mut l, &mut r);
    assert_eq!(l, r);
    (l, alive)
}

#[test]
fn plays_with_gain() {
    let mut v = Voice::idle();
    let mut p = params(0, 1000);
    p.gain = 0.5;
    v.start(1, constant(1000, 1.0), p, 1.0, 0);
    let (out, alive) = render(&mut v, 100);
    assert!(alive);
    assert!(out.iter().all(|&x| (x - 0.5).abs() < 1e-6));
}

#[test]
fn finishes_at_end_of_range() {
    let mut v = Voice::idle();
    v.start(1, constant(100, 1.0), params(0, 100), 1.0, 0);
    let (out, alive) = render(&mut v, 200);
    assert!(!alive);
    assert!(!v.is_active());
    assert!(out[99] > 0.0);
    assert!(out[100..].iter().all(|&x| x == 0.0));
}

#[test]
fn rate_ratio_two_plays_twice_as_fast() {
    let mut v = Voice::idle();
    v.start(1, constant(100, 1.0), params(0, 100), 2.0, 0);
    let (out, _) = render(&mut v, 200);
    assert!(out[49] > 0.0);
    assert!(out[50..].iter().all(|&x| x == 0.0));
}

#[test]
fn trim_start_begins_inside_the_sample() {
    let data: Vec<f32> = (0..1000).flat_map(|i| [i as f32, i as f32]).collect();
    let mut v = Voice::idle();
    v.start(1, Arc::new(Sample { rate: 48_000, data }), params(10, 500), 1.0, 0);
    let (out, _) = render(&mut v, 3);
    assert_eq!(&out[..3], &[10.0, 11.0, 12.0]);
}

#[test]
fn end_fade_avoids_a_click() {
    let mut v = Voice::idle();
    v.start(1, constant(1000, 1.0), params(0, 1000), 1.0, 0);
    let (out, _) = render(&mut v, 1000);
    assert!(out[999] < 0.05, "last frame {}", out[999]);
    assert!(out[800] > 0.99);
}

#[test]
fn fade_out_reaches_silence_and_stops() {
    let mut v = Voice::idle();
    v.start(1, constant(10_000, 1.0), params(0, 10_000), 1.0, 0);
    v.fade_out(48);
    assert!(v.is_fading());
    let (out, alive) = render(&mut v, 100);
    assert!(!alive);
    assert!(out.windows(2).take(48).all(|w| w[1] <= w[0]));
    assert!(out[49..].iter().all(|&x| x == 0.0));
}

#[test]
fn a_slower_fade_never_overrides_a_faster_one() {
    let mut v = Voice::idle();
    v.start(1, constant(10_000, 1.0), params(0, 10_000), 1.0, 0);
    v.fade_out(10);
    v.fade_out(1000);
    let (_, alive) = render(&mut v, 20);
    assert!(!alive);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lpd-core --test audio_voice`
Expected: compile error `cannot find type Voice`.

- [ ] **Step 3: Implement**

`crates/lpd-core/src/audio/mod.rs` — replace the whole file with:
```rust
//! Real-time audio engine. Nothing in here touches a sound card.

mod sample;
mod voice;

pub use sample::Sample;
pub use voice::{PlayParams, Voice};

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}
```

`crates/lpd-core/src/audio/voice.rs`:
```rust
use super::Sample;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayParams {
    /// Linear gain.
    pub gain: f32,
    pub pitch_semitones: f32,
    /// Playback range in source frames: start_frame <= end_frame <= sample.frames().
    pub start_frame: usize,
    pub end_frame: usize,
    pub choke_group: Option<u8>,
    /// 0..8
    pub mix_group: u8,
    /// Hold mode: releasing the pad fades the voice out.
    pub hold: bool,
}

/// Fade before the end of the playback range.
const END_FADE_MS: f64 = 2.0;

pub struct Voice {
    pub pad: u16,
    pub params: PlayParams,
    /// Trigger order; lower = older.
    pub age: u64,
    sample: Option<Arc<Sample>>,
    pos: f64,
    step: f64,
    end_fade_frames: f64,
    env: f32,
    env_step: f32,
}

impl Voice {
    pub fn idle() -> Voice {
        Voice {
            pad: 0,
            params: PlayParams { gain: 0.0, pitch_semitones: 0.0, start_frame: 0, end_frame: 0, choke_group: None, mix_group: 0, hold: false },
            age: 0,
            sample: None,
            pos: 0.0,
            step: 0.0,
            end_fade_frames: 1.0,
            env: 0.0,
            env_step: 0.0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.sample.is_some()
    }

    pub fn is_fading(&self) -> bool {
        self.env_step < 0.0
    }

    /// Start playing. `rate_ratio` = sample rate / output rate × pitch factor.
    pub fn start(&mut self, pad: u16, sample: Arc<Sample>, params: PlayParams, rate_ratio: f64, age: u64) {
        self.end_fade_frames = (END_FADE_MS / 1000.0 * sample.rate as f64).max(1.0);
        self.pad = pad;
        self.params = params;
        self.age = age;
        self.pos = params.start_frame as f64;
        self.step = rate_ratio;
        self.env = 1.0;
        self.env_step = 0.0;
        self.sample = Some(sample);
    }

    /// Fade to silence over `frames` output frames, then stop. A slower fade never
    /// replaces a faster one already running.
    pub fn fade_out(&mut self, frames: usize) {
        if !self.is_active() {
            return;
        }
        let step = -self.env / frames.max(1) as f32;
        if step < self.env_step {
            self.env_step = step;
        }
    }

    /// Mix this voice into `l`/`r`. Returns false once the voice has finished (it is then idle).
    pub fn render_add(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let Some(sample) = self.sample.as_ref() else { return false };
        let end = self.params.end_frame as f64;
        let mut finished = false;
        for (ol, or) in l.iter_mut().zip(r.iter_mut()) {
            if self.pos >= end || self.env <= 0.0 {
                finished = true;
                break;
            }
            let i = self.pos.floor();
            let t = (self.pos - i) as f32;
            let i = i as isize;
            let (al, ar) = sample.frame(i - 1);
            let (bl, br) = sample.frame(i);
            let (cl, cr) = sample.frame(i + 1);
            let (dl, dr) = sample.frame(i + 2);
            let tail = ((end - self.pos) / self.end_fade_frames).min(1.0) as f32;
            let g = self.params.gain * self.env * tail;
            *ol += hermite(al, bl, cl, dl, t) * g;
            *or += hermite(ar, br, cr, dr, t) * g;
            self.pos += self.step;
            self.env = (self.env + self.env_step).max(0.0);
        }
        if finished {
            self.sample = None;
            self.env_step = 0.0;
        }
        !finished
    }
}

/// 4-point, 3rd-order Hermite interpolation between x0 and x1 at t in [0, 1).
#[inline]
fn hermite(xm1: f32, x0: f32, x1: f32, x2: f32, t: f32) -> f32 {
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * t + c2) * t + c1) * t + x0
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p lpd-core --test audio_voice`
Expected: `test result: ok. 7 passed`

- [ ] **Step 5: Commit**

```powershell
cargo fmt
git add crates/lpd-core
git commit -m "feat(core): voice playback with Hermite interpolation and fades"
```

---

### Task 8: Low-pass filter

**Files:**
- Create: `crates/lpd-core/src/audio/biquad.rs`
- Modify: `crates/lpd-core/src/audio/mod.rs`
- Test: `crates/lpd-core/tests/audio_biquad.rs`

- [ ] **Step 1: Write the failing test** `crates/lpd-core/tests/audio_biquad.rs`

```rust
use lpd_core::audio::LowPass;

const FS: f32 = 48_000.0;

/// Gain in dB of a sine through the filter, measured after it settles.
fn gain_db(lp: &mut LowPass, freq: f32) -> f32 {
    let n = 48_000;
    let (mut sum_in, mut sum_out) = (0.0f64, 0.0f64);
    for i in 0..n {
        let x = (2.0 * std::f32::consts::PI * freq * i as f32 / FS).sin();
        let y = lp.process(x);
        if i >= 4800 {
            sum_in += (x * x) as f64;
            sum_out += (y * y) as f64;
        }
    }
    10.0 * (sum_out / sum_in).log10() as f32
}

#[test]
fn bypassed_at_20khz() {
    let mut lp = LowPass::new();
    lp.set_cutoff(20_000.0, FS);
    assert_eq!(lp.process(0.3), 0.3);
    assert_eq!(lp.process(-0.7), -0.7);
}

#[test]
fn passes_low_frequencies() {
    let mut lp = LowPass::new();
    lp.set_cutoff(1_000.0, FS);
    assert!(gain_db(&mut lp, 100.0).abs() < 0.5);
}

#[test]
fn minus_3db_at_cutoff() {
    let mut lp = LowPass::new();
    lp.set_cutoff(1_000.0, FS);
    let g = gain_db(&mut lp, 1_000.0);
    assert!((g + 3.0).abs() < 0.5, "got {g}");
}

#[test]
fn attenuates_high_frequencies() {
    let mut lp = LowPass::new();
    lp.set_cutoff(1_000.0, FS);
    let g = gain_db(&mut lp, 5_000.0);
    assert!(g < -24.0, "got {g}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lpd-core --test audio_biquad`
Expected: compile error `no LowPass in audio`.

- [ ] **Step 3: Implement**

`crates/lpd-core/src/audio/mod.rs` — replace the whole file with:
```rust
//! Real-time audio engine. Nothing in here touches a sound card.

mod biquad;
mod sample;
mod voice;

pub use biquad::LowPass;
pub use sample::Sample;
pub use voice::{PlayParams, Voice};

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}
```

`crates/lpd-core/src/audio/biquad.rs`:
```rust
use std::f32::consts::{FRAC_1_SQRT_2, PI};

/// RBJ-cookbook 2nd-order low-pass (Q = 1/√2), transposed direct form II.
#[derive(Debug, Clone, Copy)]
pub struct LowPass {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
    bypass: bool,
}

impl LowPass {
    pub fn new() -> LowPass {
        LowPass { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0, z1: 0.0, z2: 0.0, bypass: true }
    }

    /// At or above 20 kHz (or 0.45 × sample rate) the filter is bypassed.
    pub fn set_cutoff(&mut self, hz: f32, sample_rate: f32) {
        if hz >= 20_000.0 || hz >= 0.45 * sample_rate {
            self.bypass = true;
            return;
        }
        let w0 = 2.0 * PI * hz.max(20.0) / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * FRAC_1_SQRT_2);
        let a0 = 1.0 + alpha;
        self.b0 = (1.0 - cos) / 2.0 / a0;
        self.b1 = (1.0 - cos) / a0;
        self.b2 = self.b0;
        self.a1 = -2.0 * cos / a0;
        self.a2 = (1.0 - alpha) / a0;
        if self.bypass {
            self.z1 = 0.0;
            self.z2 = 0.0;
        }
        self.bypass = false;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        if self.bypass {
            return x;
        }
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

impl Default for LowPass {
    fn default() -> Self {
        LowPass::new()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p lpd-core --test audio_biquad`
Expected: `test result: ok. 4 passed`

- [ ] **Step 5: Commit**

```powershell
cargo fmt
git add crates/lpd-core
git commit -m "feat(core): RBJ low-pass filter"
```

---

### Task 9: Engine (voices, choke, limit, mixer, master)

Behaviour from spec §5: restart fades the old voice of the same pad over 3 ms; choke fades same-group voices over 5 ms; hold release fades over 10 ms; max 32 non-fading voices (oldest fades over 3 ms); StopAll fades over 5 ms; 8 mix groups with gain, equal-power pan (centre = unity), low-pass, mute, solo, pitch offset; master gain + soft clipper (linear to ±0.8, tanh knee, never above 1.0). Events `VoiceStarted`/`VoiceEnded` go out through a lock-free queue.

**Files:**
- Create: `crates/lpd-core/src/audio/engine.rs`
- Modify: `crates/lpd-core/src/audio/mod.rs`
- Test: `crates/lpd-core/tests/audio_engine.rs`

- [ ] **Step 1: Write the failing test** `crates/lpd-core/tests/audio_engine.rs`

```rust
use lpd_core::audio::*;
use std::sync::Arc;

const SR: u32 = 48_000;

fn constant(frames: usize, value: f32) -> Arc<Sample> {
    Arc::new(Sample { rate: SR, data: vec![value; frames * 2] })
}

fn sine(freq: f32, frames: usize, amp: f32) -> Arc<Sample> {
    let data = (0..frames)
        .flat_map(|i| {
            let x = amp * (2.0 * std::f32::consts::PI * freq * i as f32 / SR as f32).sin();
            [x, x]
        })
        .collect();
    Arc::new(Sample { rate: SR, data })
}

fn params(sample: &Sample) -> PlayParams {
    PlayParams { gain: 1.0, pitch_semitones: 0.0, start_frame: 0, end_frame: sample.frames(), choke_group: None, mix_group: 0, hold: false }
}

fn trigger(e: &mut Engine, pad: u16, sample: &Arc<Sample>, f: impl FnOnce(&mut PlayParams)) {
    let mut p = params(sample);
    f(&mut p);
    e.handle(Command::Trigger { pad, sample: sample.clone(), params: p });
}

fn render(e: &mut Engine, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * 2];
    e.render(&mut out);
    out
}

fn left(out: &[f32]) -> Vec<f32> {
    out.iter().step_by(2).copied().collect()
}

fn events(rx: &mut rtrb::Consumer<Event>) -> Vec<Event> {
    let mut v = Vec::new();
    while let Ok(e) = rx.pop() {
        v.push(e);
    }
    v
}

#[test]
fn trigger_plays_at_unity_and_reports_start() {
    let (mut e, mut rx) = Engine::new(SR);
    let s = constant(48_000, 0.5);
    trigger(&mut e, 1, &s, |_| {});
    let out = render(&mut e, 64);
    assert!(out.iter().all(|&x| (x - 0.5).abs() < 1e-4));
    assert_eq!(events(&mut rx), vec![Event::VoiceStarted { pad: 1 }]);
}

#[test]
fn choke_silences_same_group_within_5ms() {
    let (mut e, mut rx) = Engine::new(SR);
    let s = constant(48_000, 0.5);
    let silent = constant(48_000, 0.0);
    trigger(&mut e, 1, &s, |p| p.choke_group = Some(1));
    trigger(&mut e, 3, &s, |p| p.choke_group = Some(2));
    render(&mut e, 100);
    trigger(&mut e, 2, &silent, |p| p.choke_group = Some(1));
    let out = left(&render(&mut e, 480));
    // pad 3 (other choke group) keeps playing at 0.5; pad 1 is gone after 240 frames
    assert!(out[245..].iter().all(|&x| (x - 0.5).abs() < 1e-4), "{}", out[245]);
    assert!(events(&mut rx).contains(&Event::VoiceEnded { pad: 1 }));
}

#[test]
fn restart_does_not_click() {
    let (mut e, _rx) = Engine::new(SR);
    let s = sine(200.0, 48_000, 0.5);
    trigger(&mut e, 1, &s, |_| {});
    let mut out = left(&render(&mut e, 1000));
    trigger(&mut e, 1, &s, |_| {});
    out.extend(left(&render(&mut e, 1000)));
    let max_step = out.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
    // An abrupt cut would jump ~0.43 here; two overlapping 200 Hz sines plus the fade stay well under 0.05.
    assert!(max_step < 0.05, "max step {max_step}");
}

#[test]
fn hold_release_fades_within_10ms_and_one_shot_ignores_release() {
    let (mut e, _rx) = Engine::new(SR);
    let s = constant(48_000, 0.5);
    trigger(&mut e, 1, &s, |p| p.hold = true);
    trigger(&mut e, 2, &s, |_| {});
    render(&mut e, 100);
    e.handle(Command::Release { pad: 1 });
    e.handle(Command::Release { pad: 2 });
    render(&mut e, 500);
    assert_eq!(e.active_voices(), 1);
}

#[test]
fn voice_limit_fades_oldest() {
    let (mut e, mut rx) = Engine::new(SR);
    let s = constant(48_000, 0.01);
    for pad in 0..33 {
        trigger(&mut e, pad, &s, |_| {});
    }
    render(&mut e, 200);
    assert_eq!(e.active_voices(), MAX_VOICES);
    assert!(events(&mut rx).contains(&Event::VoiceEnded { pad: 0 }));
}

#[test]
fn soft_clipper_never_exceeds_full_scale() {
    let (mut e, _rx) = Engine::new(SR);
    let s = constant(48_000, 0.5);
    for pad in 0..32 {
        trigger(&mut e, pad, &s, |_| {});
    }
    let out = render(&mut e, 64);
    assert!(out.iter().all(|&x| x <= 1.0 && x > 0.95));
}

#[test]
fn mute_and_solo() {
    let (mut e, _rx) = Engine::new(SR);
    trigger(&mut e, 1, &constant(48_000, 0.1), |p| p.mix_group = 0);
    trigger(&mut e, 2, &constant(48_000, 0.2), |p| p.mix_group = 1);
    e.handle(Command::SetGroup { group: 1, params: GroupParams { solo: true, ..GroupParams::default() } });
    assert!((render(&mut e, 8)[0] - 0.2).abs() < 1e-4);
    e.handle(Command::SetGroup { group: 1, params: GroupParams { mute: true, ..GroupParams::default() } });
    assert!((render(&mut e, 8)[0] - 0.1).abs() < 1e-4);
}

#[test]
fn hard_left_pan() {
    let (mut e, _rx) = Engine::new(SR);
    trigger(&mut e, 1, &constant(48_000, 0.5), |_| {});
    e.handle(Command::SetGroup { group: 0, params: GroupParams { pan: -1.0, ..GroupParams::default() } });
    let out = render(&mut e, 8);
    assert!((out[0] - 0.5 * std::f32::consts::SQRT_2).abs() < 1e-4);
    assert!(out[1].abs() < 1e-6);
}

#[test]
fn group_pitch_offset_changes_speed() {
    let (mut e, mut rx) = Engine::new(SR);
    e.handle(Command::SetGroup { group: 0, params: GroupParams { pitch_semitones: 12.0, ..GroupParams::default() } });
    trigger(&mut e, 1, &constant(100, 0.5), |_| {});
    let out = left(&render(&mut e, 100));
    assert!(out[10] > 0.0);
    assert_eq!(out[55], 0.0);
    assert!(events(&mut rx).contains(&Event::VoiceEnded { pad: 1 }));
}

#[test]
fn resamples_to_output_rate() {
    let (mut e, _rx) = Engine::new(SR);
    let s = Arc::new(Sample { rate: 44_100, data: vec![0.5; 4410 * 2] });
    trigger(&mut e, 1, &s, |_| {});
    let out = left(&render(&mut e, 4900));
    assert!(out[4790] > 0.0);
    assert_eq!(out[4805], 0.0);
}

#[test]
fn stop_all_silences_within_5ms() {
    let (mut e, _rx) = Engine::new(SR);
    let s = constant(48_000, 0.5);
    for pad in 0..4 {
        trigger(&mut e, pad, &s, |_| {});
    }
    e.handle(Command::StopAll);
    render(&mut e, 300);
    assert_eq!(e.active_voices(), 0);
}

#[test]
fn group_lowpass_is_applied() {
    let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
    let s = sine(5_000.0, 48_000, 0.5);
    let (mut dry, _rx1) = Engine::new(SR);
    trigger(&mut dry, 1, &s, |_| {});
    let dry_rms = rms(&left(&render(&mut dry, 4800))[480..]);
    let (mut wet, _rx2) = Engine::new(SR);
    wet.handle(Command::SetGroup { group: 0, params: GroupParams { lowpass_hz: 1_000.0, ..GroupParams::default() } });
    trigger(&mut wet, 1, &s, |_| {});
    let wet_rms = rms(&left(&render(&mut wet, 4800))[480..]);
    assert!(wet_rms < dry_rms * 0.1, "dry {dry_rms} wet {wet_rms}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lpd-core --test audio_engine`
Expected: compile error `cannot find type Engine`.

- [ ] **Step 3: Implement**

`crates/lpd-core/src/audio/mod.rs` — replace the whole file with:
```rust
//! Real-time audio engine. Nothing in here touches a sound card.

mod biquad;
mod engine;
mod sample;
mod voice;

pub use biquad::LowPass;
pub use engine::{Command, Engine, Event, GroupParams, GROUPS, MAX_BLOCK, MAX_VOICES};
pub use sample::Sample;
pub use voice::{PlayParams, Voice};

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}
```

`crates/lpd-core/src/audio/engine.rs`:
```rust
use super::{LowPass, PlayParams, Sample, Voice};
use rtrb::{Consumer, Producer, RingBuffer};
use std::f32::consts::{FRAC_PI_4, SQRT_2};
use std::sync::Arc;

pub const MAX_VOICES: usize = 32;
/// Extra slots so voices that are fading out do not count against MAX_VOICES.
const SLOTS: usize = MAX_VOICES + 8;
pub const GROUPS: usize = 8;
/// Largest block rendered in one pass; longer buffers are split.
pub const MAX_BLOCK: usize = 1024;
const RESTART_FADE_MS: f32 = 3.0;
const CHOKE_FADE_MS: f32 = 5.0;
const RELEASE_FADE_MS: f32 = 10.0;
const STEAL_FADE_MS: f32 = 3.0;
const STOP_FADE_MS: f32 = 5.0;
/// Soft clipper is linear below this level.
const CLIP_KNEE: f32 = 0.8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupParams {
    /// Linear gain.
    pub gain: f32,
    /// -1 = left, 0 = centre, 1 = right.
    pub pan: f32,
    pub pitch_semitones: f32,
    pub lowpass_hz: f32,
    pub mute: bool,
    pub solo: bool,
}

impl Default for GroupParams {
    fn default() -> Self {
        GroupParams { gain: 1.0, pan: 0.0, pitch_semitones: 0.0, lowpass_hz: 20_000.0, mute: false, solo: false }
    }
}

#[derive(Debug)]
pub enum Command {
    /// `pad` identifies the pad across banks (bank * 64 + y * 8 + x).
    Trigger { pad: u16, sample: Arc<Sample>, params: PlayParams },
    Release { pad: u16 },
    StopAll,
    SetGroup { group: u8, params: GroupParams },
    SetMasterGain(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    VoiceStarted { pad: u16 },
    VoiceEnded { pad: u16 },
}

struct GroupState {
    params: GroupParams,
    lp_l: LowPass,
    lp_r: LowPass,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
}

/// Owned by the audio thread. All buffers are allocated in `new`; `handle` and
/// `render` never allocate.
pub struct Engine {
    rate: f32,
    voices: Vec<Voice>,
    groups: Vec<GroupState>,
    master_gain: f32,
    next_age: u64,
    events: Producer<Event>,
}

impl Engine {
    pub fn new(sample_rate: u32) -> (Engine, Consumer<Event>) {
        let (events, rx) = RingBuffer::new(1024);
        let groups = (0..GROUPS)
            .map(|_| GroupState {
                params: GroupParams::default(),
                lp_l: LowPass::new(),
                lp_r: LowPass::new(),
                buf_l: vec![0.0; MAX_BLOCK],
                buf_r: vec![0.0; MAX_BLOCK],
            })
            .collect();
        let engine = Engine {
            rate: sample_rate as f32,
            voices: (0..SLOTS).map(|_| Voice::idle()).collect(),
            groups,
            master_gain: 1.0,
            next_age: 0,
            events,
        };
        (engine, rx)
    }

    pub fn sample_rate(&self) -> u32 {
        self.rate as u32
    }

    /// Voices currently producing sound, including ones fading out.
    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.is_active()).count()
    }

    fn frames_for(&self, ms: f32) -> usize {
        (ms / 1000.0 * self.rate) as usize
    }

    pub fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::Trigger { pad, sample, params } => self.trigger(pad, sample, params),
            Command::Release { pad } => {
                let frames = self.frames_for(RELEASE_FADE_MS);
                for v in &mut self.voices {
                    if v.is_active() && v.pad == pad && v.params.hold {
                        v.fade_out(frames);
                    }
                }
            }
            Command::StopAll => {
                let frames = self.frames_for(STOP_FADE_MS);
                for v in &mut self.voices {
                    v.fade_out(frames);
                }
            }
            Command::SetGroup { group, params } => {
                if let Some(g) = self.groups.get_mut(group as usize) {
                    g.params = params;
                    g.lp_l.set_cutoff(params.lowpass_hz, self.rate);
                    g.lp_r.set_cutoff(params.lowpass_hz, self.rate);
                }
            }
            Command::SetMasterGain(gain) => self.master_gain = gain,
        }
    }

    fn trigger(&mut self, pad: u16, sample: Arc<Sample>, params: PlayParams) {
        let restart = self.frames_for(RESTART_FADE_MS);
        let choke = self.frames_for(CHOKE_FADE_MS);
        let steal = self.frames_for(STEAL_FADE_MS);
        for v in self.voices.iter_mut().filter(|v| v.is_active()) {
            if v.pad == pad {
                v.fade_out(restart);
            } else if params.choke_group.is_some() && v.params.choke_group == params.choke_group {
                v.fade_out(choke);
            }
        }
        let sounding = self.voices.iter().filter(|v| v.is_active() && !v.is_fading()).count();
        if sounding >= MAX_VOICES {
            if let Some(oldest) = self.voices.iter_mut().filter(|v| v.is_active() && !v.is_fading()).min_by_key(|v| v.age) {
                oldest.fade_out(steal);
            }
        }
        let slot = match self.voices.iter().position(|v| !v.is_active()) {
            Some(i) => i,
            None => {
                // Every slot is busy fading: cut the oldest immediately.
                let i = self.voices.iter().enumerate().min_by_key(|(_, v)| v.age).map(|(i, _)| i).unwrap_or(0);
                let _ = self.events.push(Event::VoiceEnded { pad: self.voices[i].pad });
                i
            }
        };
        let group = (params.mix_group as usize).min(GROUPS - 1);
        let semitones = (params.pitch_semitones + self.groups[group].params.pitch_semitones) as f64;
        let ratio = sample.rate as f64 / self.rate as f64 * 2f64.powf(semitones / 12.0);
        let age = self.next_age;
        self.next_age += 1;
        self.voices[slot].start(pad, sample, params, ratio, age);
        let _ = self.events.push(Event::VoiceStarted { pad });
    }

    /// Render interleaved stereo into `out` (length = 2 × frames).
    pub fn render(&mut self, out: &mut [f32]) {
        for chunk in out.chunks_mut(MAX_BLOCK * 2) {
            self.render_block(chunk);
        }
    }

    fn render_block(&mut self, out: &mut [f32]) {
        let n = out.len() / 2;
        for g in &mut self.groups {
            g.buf_l[..n].fill(0.0);
            g.buf_r[..n].fill(0.0);
        }
        for v in &mut self.voices {
            if !v.is_active() {
                continue;
            }
            let pad = v.pad;
            let g = &mut self.groups[(v.params.mix_group as usize).min(GROUPS - 1)];
            if !v.render_add(&mut g.buf_l[..n], &mut g.buf_r[..n]) {
                let _ = self.events.push(Event::VoiceEnded { pad });
            }
        }
        let any_solo = self.groups.iter().any(|g| g.params.solo);
        out.fill(0.0);
        for g in &mut self.groups {
            let p = g.params;
            if p.mute || (any_solo && !p.solo) {
                continue;
            }
            let angle = (p.pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
            let gl = angle.cos() * SQRT_2 * p.gain;
            let gr = angle.sin() * SQRT_2 * p.gain;
            for i in 0..n {
                out[2 * i] += g.lp_l.process(g.buf_l[i]) * gl;
                out[2 * i + 1] += g.lp_r.process(g.buf_r[i]) * gr;
            }
        }
        for s in out.iter_mut() {
            *s = soft_clip(*s * self.master_gain);
        }
    }
}

/// Linear up to ±CLIP_KNEE, tanh knee above; output magnitude never exceeds 1.0.
#[inline]
fn soft_clip(x: f32) -> f32 {
    let a = x.abs();
    if a <= CLIP_KNEE {
        x
    } else {
        let range = 1.0 - CLIP_KNEE;
        (CLIP_KNEE + range * ((a - CLIP_KNEE) / range).tanh()).copysign(x)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p lpd-core --test audio_engine`
Expected: `test result: ok. 12 passed`

- [ ] **Step 5: Run the whole core suite**

Run: `cargo test -p lpd-core`
Expected: all test files pass.

- [ ] **Step 6: Commit**

```powershell
cargo fmt
git add crates/lpd-core
git commit -m "feat(core): audio engine with choke, voice limit, 8-group mixer and soft clip"
```

---

### Task 10: Kit → engine parameters

**Files:**
- Create: `crates/lpd-core/src/kit/params.rs`
- Modify: `crates/lpd-core/src/kit/mod.rs`
- Test: `crates/lpd-core/tests/kit_params.rs`

- [ ] **Step 1: Write the failing test** `crates/lpd-core/tests/kit_params.rs`

```rust
use lpd_core::audio::Sample;
use lpd_core::kit::*;

fn one_second() -> Sample {
    Sample { rate: 48_000, data: vec![0.0; 48_000 * 2] }
}

#[test]
fn pad_settings_convert_to_play_params() {
    let mut pad = Pad::new(2, 3, "s.wav", [0, 0, 0]);
    pad.volume_db = -6.0;
    pad.pitch_semitones = 2.5;
    pad.trim_start_ms = 10.0;
    pad.trim_end_ms = Some(20.0);
    pad.mode = PadMode::Hold;
    pad.choke_group = Some(4);
    let p = play_params(&pad, &one_second());
    assert!((p.gain - 0.501).abs() < 0.001);
    assert_eq!(p.pitch_semitones, 2.5);
    assert_eq!((p.start_frame, p.end_frame), (480, 960));
    assert!(p.hold);
    assert_eq!(p.choke_group, Some(4));
    assert_eq!(p.mix_group, 3);
}

#[test]
fn trim_is_clamped_to_the_sample() {
    let mut pad = Pad::new(0, 0, "s.wav", [0, 0, 0]);
    pad.trim_end_ms = Some(5_000.0);
    let p = play_params(&pad, &one_second());
    assert_eq!((p.start_frame, p.end_frame), (0, 48_000));
    pad.trim_start_ms = 500.0;
    pad.trim_end_ms = Some(100.0);
    let p = play_params(&pad, &one_second());
    assert_eq!((p.start_frame, p.end_frame), (24_000, 24_000));
}

#[test]
fn group_converts_to_group_params() {
    let mut g = Set::new().mixer.groups.remove(0);
    g.volume_db = -6.0;
    g.pan = 0.5;
    g.pitch_semitones = -3.0;
    g.lowpass_hz = 2_000.0;
    g.mute = true;
    let p = group_params(&g);
    assert!((p.gain - 0.501).abs() < 0.001);
    assert_eq!((p.pan, p.pitch_semitones, p.lowpass_hz, p.mute, p.solo), (0.5, -3.0, 2_000.0, true, false));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lpd-core --test kit_params`
Expected: compile error `cannot find function play_params`.

- [ ] **Step 3: Implement**

`crates/lpd-core/src/kit/params.rs`:
```rust
use super::{Group, Pad, PadMode};
use crate::audio::{db_to_gain, GroupParams, PlayParams, Sample, GROUPS};

pub fn play_params(pad: &Pad, sample: &Sample) -> PlayParams {
    let frames = sample.frames();
    let to_frame = |ms: f32| ((ms.max(0.0) / 1000.0) * sample.rate as f32) as usize;
    let start = to_frame(pad.trim_start_ms).min(frames);
    let end = pad.trim_end_ms.map(to_frame).unwrap_or(frames).clamp(start, frames);
    PlayParams {
        gain: db_to_gain(pad.volume_db),
        pitch_semitones: pad.pitch_semitones,
        start_frame: start,
        end_frame: end,
        choke_group: pad.choke_group,
        mix_group: pad.mix_group.min(GROUPS as u8 - 1),
        hold: pad.mode == PadMode::Hold,
    }
}

pub fn group_params(g: &Group) -> GroupParams {
    GroupParams {
        gain: db_to_gain(g.volume_db),
        pan: g.pan,
        pitch_semitones: g.pitch_semitones,
        lowpass_hz: g.lowpass_hz,
        mute: g.mute,
        solo: g.solo,
    }
}
```

`crates/lpd-core/src/kit/mod.rs` — replace the whole file with:
```rust
//! Sets, banks and pads: data model and files.

mod decode;
mod model;
mod params;
mod store;

pub use decode::{decode_file, load_samples};
pub use model::*;
pub use params::{group_params, play_params};
pub use store::{import_sample, load, save, Loaded, BACKUP_FILE, BROKEN_FILE, SAMPLES_DIR, SET_FILE};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p lpd-core --test kit_params`
Expected: `test result: ok. 3 passed`

- [ ] **Step 5: Commit**

```powershell
cargo fmt
git add crates/lpd-core
git commit -m "feat(core): convert pad and group settings to engine parameters"
```

---

### Task 11: Audio callback host + no-allocation guarantee

`host::process` is the whole audio callback: apply queued commands, render, copy to the device's channel layout. A test runs it under `assert_no_alloc`, which aborts the test binary if anything allocates or frees memory.

**Files:**
- Create: `crates/lpd-core/src/audio/host.rs`
- Modify: `crates/lpd-core/src/audio/mod.rs`
- Test: `crates/lpd-core/tests/audio_host.rs`, `crates/lpd-core/tests/no_alloc.rs`

- [ ] **Step 1: Write the failing tests**

`crates/lpd-core/tests/audio_host.rs`:
```rust
use lpd_core::audio::*;
use std::sync::Arc;

fn setup() -> (Engine, rtrb::Producer<Command>, rtrb::Consumer<Command>, Arc<Sample>) {
    let (engine, _events) = Engine::new(48_000);
    let (tx, rx) = rtrb::RingBuffer::new(64);
    let sample = Arc::new(Sample { rate: 48_000, data: [0.2, 0.6].repeat(48_000) });
    (engine, tx, rx, sample)
}

fn trigger(tx: &mut rtrb::Producer<Command>, s: &Arc<Sample>) {
    let params = PlayParams { gain: 1.0, pitch_semitones: 0.0, start_frame: 0, end_frame: s.frames(), choke_group: None, mix_group: 0, hold: false };
    tx.push(Command::Trigger { pad: 0, sample: s.clone(), params }).unwrap();
}

#[test]
fn stereo_device_gets_left_and_right() {
    let (mut e, mut tx, mut rx, s) = setup();
    trigger(&mut tx, &s);
    let mut scratch = host::scratch();
    let mut out = vec![0.0; 16];
    host::process(&mut e, &mut rx, &mut scratch, &mut out, 2);
    assert!((out[0] - 0.2).abs() < 1e-4 && (out[1] - 0.6).abs() < 1e-4);
}

#[test]
fn mono_device_gets_mixdown() {
    let (mut e, mut tx, mut rx, s) = setup();
    trigger(&mut tx, &s);
    let mut scratch = host::scratch();
    let mut out = vec![0.0; 8];
    host::process(&mut e, &mut rx, &mut scratch, &mut out, 1);
    assert!((out[0] - 0.4).abs() < 1e-4);
}

#[test]
fn extra_channels_are_silent() {
    let (mut e, mut tx, mut rx, s) = setup();
    trigger(&mut tx, &s);
    let mut scratch = host::scratch();
    let mut out = vec![9.0; 4 * 8];
    host::process(&mut e, &mut rx, &mut scratch, &mut out, 4);
    assert!((out[0] - 0.2).abs() < 1e-4 && (out[1] - 0.6).abs() < 1e-4);
    assert_eq!((out[2], out[3]), (0.0, 0.0));
}

#[test]
fn buffers_larger_than_scratch_are_filled() {
    let (mut e, mut tx, mut rx, s) = setup();
    trigger(&mut tx, &s);
    let mut scratch = host::scratch();
    let mut out = vec![0.0; 2 * (MAX_BLOCK * 3 + 7)];
    host::process(&mut e, &mut rx, &mut scratch, &mut out, 2);
    assert!((out[out.len() - 2] - 0.2).abs() < 1e-4);
}
```

`crates/lpd-core/tests/no_alloc.rs`:
```rust
use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use lpd_core::audio::*;
use std::sync::Arc;

#[global_allocator]
static ALLOC: AllocDisabler = AllocDisabler;

#[test]
fn audio_callback_never_allocates() {
    let (mut engine, mut events) = Engine::new(48_000);
    let (mut tx, mut rx) = rtrb::RingBuffer::new(256);
    let mut scratch = host::scratch();
    let mut out = vec![0.0f32; 512 * 2];
    // Kept alive here so the audio thread only ever decrements the refcount.
    let sample = Arc::new(Sample { rate: 44_100, data: vec![0.25; 44_100 * 2] });
    for pad in 0..40u16 {
        let params = PlayParams {
            gain: 1.0,
            pitch_semitones: 0.0,
            start_frame: 0,
            end_frame: sample.frames(),
            choke_group: if pad % 2 == 0 { Some(1) } else { None },
            mix_group: (pad % 8) as u8,
            hold: pad % 3 == 0,
        };
        tx.push(Command::Trigger { pad, sample: sample.clone(), params }).unwrap();
    }
    tx.push(Command::SetGroup { group: 2, params: GroupParams { lowpass_hz: 1_000.0, ..GroupParams::default() } }).unwrap();
    tx.push(Command::Release { pad: 3 }).unwrap();
    tx.push(Command::SetMasterGain(0.8)).unwrap();
    assert_no_alloc(|| {
        for _ in 0..200 {
            host::process(&mut engine, &mut rx, &mut scratch, &mut out, 2);
        }
    });
    tx.push(Command::StopAll).unwrap();
    assert_no_alloc(|| {
        for _ in 0..20 {
            host::process(&mut engine, &mut rx, &mut scratch, &mut out, 2);
        }
    });
    while events.pop().is_ok() {}
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p lpd-core --test audio_host --test no_alloc`
Expected: compile error `could not find host in audio`.

- [ ] **Step 3: Implement**

`crates/lpd-core/src/audio/mod.rs` — replace the whole file with:
```rust
//! Real-time audio engine. Nothing in here touches a sound card.

mod biquad;
mod engine;
pub mod host;
mod sample;
mod voice;

pub use biquad::LowPass;
pub use engine::{Command, Engine, Event, GroupParams, GROUPS, MAX_BLOCK, MAX_VOICES};
pub use sample::Sample;
pub use voice::{PlayParams, Voice};

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}
```

`crates/lpd-core/src/audio/host.rs`:
```rust
//! The body of the audio callback, shared by every audio backend.

use super::{Command, Engine, MAX_BLOCK};
use rtrb::Consumer;

/// Stereo scratch buffer for `process`. Allocate once, outside the callback.
pub fn scratch() -> Vec<f32> {
    vec![0.0; MAX_BLOCK * 2]
}

/// Apply queued commands, render, and write `out` (interleaved, `channels` per frame).
/// Mono devices get (L + R) / 2; channels beyond 2 are silent. Never allocates.
pub fn process(engine: &mut Engine, commands: &mut Consumer<Command>, scratch: &mut [f32], out: &mut [f32], channels: usize) {
    while let Ok(cmd) = commands.pop() {
        engine.handle(cmd);
    }
    if channels == 0 {
        return;
    }
    let frames_per_chunk = scratch.len() / 2;
    for chunk in out.chunks_mut(frames_per_chunk * channels) {
        let frames = chunk.len() / channels;
        let stereo = &mut scratch[..frames * 2];
        engine.render(stereo);
        for (f, frame) in chunk.chunks_mut(channels).enumerate() {
            let (l, r) = (stereo[2 * f], stereo[2 * f + 1]);
            if channels == 1 {
                frame[0] = 0.5 * (l + r);
            } else {
                frame[0] = l;
                frame[1] = r;
                frame[2..].fill(0.0);
            }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p lpd-core --test audio_host --test no_alloc`
Expected: `audio_host: 4 passed`, `no_alloc: 1 passed`. If `no_alloc` aborts with a "memory allocation" message, find the allocating line in `engine.rs`/`voice.rs`/`host.rs` and remove the allocation; do not weaken the test.

- [ ] **Step 5: Commit**

```powershell
cargo fmt
git add crates/lpd-core
git commit -m "feat(core): audio callback host with no-allocation test"
```

---

### Task 12: CLI player logic (events → commands + LEDs)

Pure logic, unit-tested without hardware. Pad ids are unique across banks (`bank * 64 + y * 8 + x`), so voices keep playing across bank switches. Held pads remember which id they triggered, so releasing after a bank switch still releases the right voice. Pads whose sample failed to load show red.

**Files:**
- Create: `crates/lpd-cli/Cargo.toml`, `crates/lpd-cli/src/main.rs` (temporary stub), `crates/lpd-cli/src/player.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Add the member** — in `Cargo.toml` change the members line to:

```toml
members = ["crates/lpd-core", "crates/lpd-cli", "tools/lp-probe"]
```

- [ ] **Step 2: Create `crates/lpd-cli/Cargo.toml`**

```toml
[package]
name = "lpd-cli"
version = "0.1.0"
edition = "2021"

[features]
asio = ["cpal/asio"]

[dependencies]
lpd-core = { path = "../lpd-core" }
anyhow = { workspace = true }
cpal = { workspace = true }
midir = { workspace = true }
rtrb = { workspace = true }
```

- [ ] **Step 3: Create a temporary `crates/lpd-cli/src/main.rs`** (replaced in Task 13)

```rust
mod player;

fn main() {}
```

- [ ] **Step 4: Write `crates/lpd-cli/src/player.rs` with failing tests first** — create the file with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use lpd_core::audio::{Command, Sample};
    use lpd_core::devices::launchpad::{self as lp, LpEvent};
    use lpd_core::kit::{Pad, Set};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn player() -> Player {
        let mut set = Set::new();
        set.banks[0].set_pad(Pad::new(0, 0, "samples/a.wav", [63, 0, 0]));
        set.banks[0].set_pad(Pad::new(1, 0, "samples/missing.wav", [0, 63, 0]));
        let mut hold = Pad::new(2, 0, "samples/a.wav", [0, 0, 63]);
        hold.mode = lpd_core::kit::PadMode::Hold;
        set.banks[0].set_pad(hold);
        set.banks[1].set_pad(Pad::new(0, 0, "samples/b.wav", [0, 0, 63]));
        let mut samples = HashMap::new();
        for name in ["samples/a.wav", "samples/b.wav"] {
            samples.insert(name.to_string(), Arc::new(Sample { rate: 48_000, data: vec![0.0; 200] }));
        }
        Player::new(set, samples)
    }

    fn triggered_pad(actions: &[Action]) -> Option<u16> {
        actions.iter().find_map(|a| match a {
            Action::Audio(Command::Trigger { pad, .. }) => Some(*pad),
            _ => None,
        })
    }

    #[test]
    fn pad_down_triggers_its_sample() {
        let mut p = player();
        assert_eq!(triggered_pad(&p.on_event(LpEvent::PadDown { x: 0, y: 0 })), Some(0));
    }

    #[test]
    fn empty_or_missing_pads_do_nothing() {
        let mut p = player();
        assert!(p.on_event(LpEvent::PadDown { x: 5, y: 5 }).is_empty());
        assert!(p.on_event(LpEvent::PadDown { x: 1, y: 0 }).is_empty());
    }

    #[test]
    fn side_button_switches_bank_and_repaints() {
        let mut p = player();
        let actions = p.on_event(LpEvent::SideDown(1));
        assert_eq!(p.bank(), 1);
        let Some(Action::Leds(leds)) = actions.first() else { panic!("expected LEDs") };
        assert!(leds.contains(&(lp::side_led(1), [63, 63, 63])));
        assert!(leds.contains(&(lp::side_led(0), [0, 0, 0])));
        assert_eq!(triggered_pad(&p.on_event(LpEvent::PadDown { x: 0, y: 0 })), Some(64));
    }

    #[test]
    fn release_after_bank_switch_targets_the_original_voice() {
        let mut p = player();
        p.on_event(LpEvent::PadDown { x: 2, y: 0 });
        p.on_event(LpEvent::SideDown(1));
        let actions = p.on_event(LpEvent::PadUp { x: 2, y: 0 });
        assert!(matches!(actions.as_slice(), [Action::Audio(Command::Release { pad: 2 })]));
    }

    #[test]
    fn mixer_button_stops_all() {
        let mut p = player();
        assert!(matches!(p.on_event(LpEvent::TopDown(7)).as_slice(), [Action::Audio(Command::StopAll)]));
    }

    #[test]
    fn paint_shows_colours_missing_red_empty_off_and_bank() {
        let p = player();
        let leds = p.paint();
        assert_eq!(leds.len(), 72);
        assert!(leds.contains(&(lp::grid_led(0, 0), [63, 0, 0])));
        assert!(leds.contains(&(lp::grid_led(1, 0), [63, 0, 0])));
        assert!(leds.contains(&(lp::grid_led(5, 5), [0, 0, 0])));
        assert!(leds.contains(&(lp::side_led(0), [63, 63, 63])));
    }

    #[test]
    fn mixer_commands_configure_all_groups_and_master() {
        let p = player();
        let cmds = p.mixer_commands();
        assert_eq!(cmds.iter().filter(|c| matches!(c, Command::SetGroup { .. })).count(), 8);
        assert!(matches!(cmds.last(), Some(Command::SetMasterGain(_))));
    }
}
```

Note: in `paint_shows_…`, pad (1,0) has sample `missing.wav` which is not in the samples map, so it shows red `[63, 0, 0]` instead of its green colour.

- [ ] **Step 5: Run tests to verify they fail**

Run: `cargo test -p lpd-cli`
Expected: compile error `cannot find type Player`.

- [ ] **Step 6: Implement** — add above the test module in `crates/lpd-cli/src/player.rs`:

```rust
//! Launchpad events → engine commands and LED frames. No I/O here.

use lpd_core::audio::{db_to_gain, Command, Sample};
use lpd_core::devices::launchpad::{self as lp, LpEvent};
use lpd_core::kit::{self, Set};
use std::collections::HashMap;
use std::sync::Arc;

pub enum Action {
    Audio(Command),
    Leds(Vec<(u8, [u8; 3])>),
}

const BANK_LED_ON: [u8; 3] = [63, 63, 63];
const MISSING: [u8; 3] = [63, 0, 0];
const OFF: [u8; 3] = [0, 0, 0];
/// Top-row index of the Mixer button: stop all sounds.
const TOP_MIXER: u8 = 7;

pub struct Player {
    set: Set,
    samples: HashMap<String, Arc<Sample>>,
    bank: usize,
    /// Pads currently held → the pad id their press triggered.
    held: HashMap<(u8, u8), u16>,
}

impl Player {
    pub fn new(set: Set, samples: HashMap<String, Arc<Sample>>) -> Player {
        Player { set, samples, bank: 0, held: HashMap::new() }
    }

    pub fn bank(&self) -> usize {
        self.bank
    }

    /// Pad id unique across banks, so voices survive bank switches.
    fn pad_id(bank: usize, x: u8, y: u8) -> u16 {
        (bank * 64 + y as usize * 8 + x as usize) as u16
    }

    /// Mixer setup from the set; send once before playing.
    pub fn mixer_commands(&self) -> Vec<Command> {
        let mut cmds: Vec<Command> = self
            .set
            .mixer
            .groups
            .iter()
            .enumerate()
            .map(|(i, g)| Command::SetGroup { group: i as u8, params: kit::group_params(g) })
            .collect();
        cmds.push(Command::SetMasterGain(db_to_gain(self.set.mixer.master_db)));
        cmds
    }

    /// Full LED frame: pad colours (missing samples red, empty off) and the active bank button.
    pub fn paint(&self) -> Vec<(u8, [u8; 3])> {
        let bank = &self.set.banks[self.bank];
        let mut leds = Vec::with_capacity(72);
        for y in 0..8 {
            for x in 0..8 {
                let colour = match bank.pad(x, y) {
                    Some(p) if self.samples.contains_key(&p.sample) => p.color,
                    Some(_) => MISSING,
                    None => OFF,
                };
                leds.push((lp::grid_led(x, y), colour));
            }
        }
        for i in 0..8u8 {
            leds.push((lp::side_led(i), if i as usize == self.bank { BANK_LED_ON } else { OFF }));
        }
        leds
    }

    pub fn on_event(&mut self, ev: LpEvent) -> Vec<Action> {
        match ev {
            LpEvent::PadDown { x, y } => {
                let Some(pad) = self.set.banks[self.bank].pad(x, y) else { return vec![] };
                let Some(sample) = self.samples.get(&pad.sample) else { return vec![] };
                let id = Self::pad_id(self.bank, x, y);
                self.held.insert((x, y), id);
                let params = kit::play_params(pad, sample);
                vec![Action::Audio(Command::Trigger { pad: id, sample: sample.clone(), params })]
            }
            LpEvent::PadUp { x, y } => match self.held.remove(&(x, y)) {
                Some(id) => vec![Action::Audio(Command::Release { pad: id })],
                None => vec![],
            },
            LpEvent::SideDown(i) if (i as usize) < self.set.banks.len() => {
                self.bank = i as usize;
                vec![Action::Leds(self.paint())]
            }
            LpEvent::TopDown(TOP_MIXER) => vec![Action::Audio(Command::StopAll)],
            _ => vec![],
        }
    }
}
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p lpd-cli`
Expected: `test result: ok. 7 passed` (a `dead_code` warning for the stub `main` is fine).

- [ ] **Step 8: Commit**

```powershell
cargo fmt
git add Cargo.toml Cargo.lock crates/lpd-cli
git commit -m "feat(cli): player maps Launchpad events to engine commands and LEDs"
```

---

### Task 13: CLI hardware wiring (`new-set`, `play`)

**Files:**
- Create: `crates/lpd-cli/src/audio_out.rs`, `crates/lpd-cli/src/midi.rs`
- Modify: `crates/lpd-cli/src/main.rs` (replace the stub)

- [ ] **Step 1: Create `crates/lpd-cli/src/audio_out.rs`**

```rust
//! cpal output stream driving the lpd-core engine (WASAPI by default, ASIO with --asio).

use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use lpd_core::audio::{host, Command, Engine};

pub struct AudioOut {
    pub stream: cpal::Stream,
    pub commands: rtrb::Producer<Command>,
    pub device_name: String,
    pub sample_rate: u32,
    pub buffer: cpal::BufferSize,
}

pub fn start(asio: bool, buffer_frames: Option<u32>) -> Result<AudioOut> {
    let audio_host = if asio { asio_host()? } else { cpal::default_host() };
    let device = audio_host.default_output_device().context("no audio output device")?;
    let device_name = device.name().unwrap_or_else(|_| "unknown".into());
    let supported = device.default_output_config()?;
    let format = supported.sample_format();
    let mut config: cpal::StreamConfig = supported.into();
    if let Some(frames) = buffer_frames {
        config.buffer_size = cpal::BufferSize::Fixed(frames);
    }
    let sample_rate = config.sample_rate.0;
    let (engine, _events) = Engine::new(sample_rate);
    let (commands, rx) = rtrb::RingBuffer::new(1024);
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, engine, rx)?,
        cpal::SampleFormat::I32 => build::<i32>(&device, &config, engine, rx)?,
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, engine, rx)?,
        other => bail!("unsupported device sample format {other:?}"),
    };
    stream.play()?;
    Ok(AudioOut { stream, commands, device_name, sample_rate, buffer: config.buffer_size })
}

fn build<T>(device: &cpal::Device, config: &cpal::StreamConfig, mut engine: Engine, mut rx: rtrb::Consumer<Command>) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut scratch = host::scratch();
    // f32 staging buffer; a whole number of frames.
    let mut staging = vec![0.0f32; 4096 * channels];
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            for chunk in data.chunks_mut(staging.len()) {
                let buf = &mut staging[..chunk.len()];
                host::process(&mut engine, &mut rx, &mut scratch, buf, channels);
                for (d, s) in chunk.iter_mut().zip(buf.iter()) {
                    *d = T::from_sample(*s);
                }
            }
        },
        |err| eprintln!("audio stream error: {err}"),
        None,
    )?;
    Ok(stream)
}

#[cfg(feature = "asio")]
fn asio_host() -> Result<cpal::Host> {
    cpal::host_from_id(cpal::HostId::Asio).map_err(|e| anyhow::anyhow!("ASIO host unavailable: {e}"))
}

#[cfg(not(feature = "asio"))]
fn asio_host() -> Result<cpal::Host> {
    bail!("built without ASIO support; rebuild with `--features asio`")
}
```

- [ ] **Step 2: Create `crates/lpd-cli/src/midi.rs`**

```rust
//! Launchpad MK2 connection via midir.

use anyhow::{anyhow, Context, Result};
use lpd_core::devices::launchpad::{self as lp, LpEvent};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use std::sync::mpsc::{self, Receiver};

pub struct LaunchpadIn {
    pub events: Receiver<LpEvent>,
    _conn: MidiInputConnection<()>,
}

pub struct LaunchpadOut {
    conn: MidiOutputConnection,
}

impl LaunchpadOut {
    pub fn send_leds(&mut self, leds: &[(u8, [u8; 3])]) -> Result<()> {
        for m in lp::rgb_messages(leds) {
            self.conn.send(&m).map_err(|e| anyhow!("LED send failed: {e}"))?;
        }
        Ok(())
    }
}

pub fn connect_launchpad() -> Result<(LaunchpadIn, LaunchpadOut)> {
    let midi_in = MidiInput::new("lpd-cli-in")?;
    let in_port = midi_in
        .ports()
        .into_iter()
        .find(|p| midi_in.port_name(p).is_ok_and(|n| lp::is_launchpad_port(&n)))
        .context("Launchpad MK2 not found (plugged in? used by another app? bootloader mode?)")?;
    let (tx, rx) = mpsc::channel();
    let conn_in = midi_in
        .connect(&in_port, "lpd-cli-in", move |_, msg, _| {
            if let Some(ev) = lp::decode(msg) {
                let _ = tx.send(ev);
            }
        }, ())
        .map_err(|e| anyhow!("open Launchpad input: {e}"))?;
    let midi_out = MidiOutput::new("lpd-cli-out")?;
    let out_port = midi_out
        .ports()
        .into_iter()
        .find(|p| midi_out.port_name(p).is_ok_and(|n| lp::is_launchpad_port(&n)))
        .context("Launchpad MK2 output port not found")?;
    let mut conn = midi_out
        .connect(&out_port, "lpd-cli-out")
        .map_err(|e| anyhow!("open Launchpad output: {e}"))?;
    conn.send(&lp::SELECT_SESSION_LAYOUT).map_err(|e| anyhow!("{e}"))?;
    conn.send(&lp::set_all(0)).map_err(|e| anyhow!("{e}"))?;
    Ok((LaunchpadIn { events: rx, _conn: conn_in }, LaunchpadOut { conn }))
}
```

- [ ] **Step 3: Replace `crates/lpd-cli/src/main.rs`**

```rust
mod audio_out;
mod midi;
mod player;

use anyhow::{bail, Context, Result};
use lpd_core::audio::Command;
use lpd_core::kit::{self, Pad, Set};
use player::{Action, Player};
use std::path::{Path, PathBuf};

const USAGE: &str = "usage:
  lpd-cli new-set <set-dir> <samples-folder>
  lpd-cli play <set-dir> [--asio] [--buffer <frames>]";

/// Colours for new sets (Launchpad 0..63 RGB), one per row.
const ROW_COLOURS: [[u8; 3]; 8] = [
    [63, 0, 0], [63, 20, 0], [63, 50, 0], [0, 63, 0],
    [0, 63, 40], [0, 30, 63], [30, 0, 63], [63, 0, 40],
];

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("new-set") if args.len() == 3 => new_set(Path::new(&args[1]), Path::new(&args[2])),
        Some("play") if args.len() >= 2 => {
            let asio = args.iter().any(|a| a == "--asio");
            let buffer = match args.iter().position(|a| a == "--buffer") {
                Some(i) => Some(args.get(i + 1).context("--buffer needs a value")?.parse::<u32>().context("--buffer must be a number")?),
                None => None,
            };
            play(Path::new(&args[1]), asio, buffer)
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

fn new_set(dir: &Path, samples: &Path) -> Result<()> {
    if dir.join(kit::SET_FILE).exists() {
        bail!("{} already contains a set", dir.display());
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(samples)
        .with_context(|| format!("read {}", samples.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            matches!(ext.as_str(), "wav" | "mp3" | "flac" | "ogg")
        })
        .collect();
    files.sort();
    let mut set = Set::new();
    for (i, file) in files.iter().take(64).enumerate() {
        let (x, y) = ((i % 8) as u8, (i / 8) as u8);
        let rel = kit::import_sample(dir, file)?;
        set.banks[0].set_pad(Pad::new(x, y, rel, ROW_COLOURS[y as usize]));
    }
    kit::save(dir, &set)?;
    println!("Created {} with {} pads in bank 1.", dir.display(), files.len().min(64));
    Ok(())
}

fn play(dir: &Path, asio: bool, buffer: Option<u32>) -> Result<()> {
    let loaded = kit::load(dir)?;
    if loaded.from_backup {
        println!("set.json was unreadable: loaded set.json.bak (broken file kept as set.json.broken).");
    }
    let (samples, failed) = kit::load_samples(dir, &loaded.set);
    for (path, err) in &failed {
        println!("Sample not loaded, pad shows red: {path}: {err:#}");
    }
    let mut player = Player::new(loaded.set, samples);
    let audio_out::AudioOut { stream: _stream, commands: mut audio_tx, device_name, sample_rate, buffer } = audio_out::start(asio, buffer)?;
    println!("Audio: {device_name} @ {sample_rate} Hz, buffer {buffer:?}");
    for cmd in player.mixer_commands() {
        push(&mut audio_tx, cmd);
    }
    let (lp_in, mut lp_out) = midi::connect_launchpad()?;
    lp_out.send_leds(&player.paint())?;
    println!("Playing. Side buttons = banks, Mixer = stop all, Ctrl+C = quit.");
    for ev in lp_in.events.iter() {
        for action in player.on_event(ev) {
            match action {
                Action::Audio(cmd) => push(&mut audio_tx, cmd),
                Action::Leds(leds) => lp_out.send_leds(&leds)?,
            }
        }
    }
    Ok(())
}

fn push(tx: &mut rtrb::Producer<Command>, cmd: Command) {
    if tx.push(cmd).is_err() {
        eprintln!("audio command queue full; command dropped");
    }
}
```

- [ ] **Step 4: Build and run all tests**

Run: `cargo build -p lpd-cli; cargo test`
Expected: build `Finished`; every test suite `ok`.

- [ ] **Step 5: Commit**

```powershell
cargo fmt
git add crates/lpd-cli
git commit -m "feat(cli): new-set and play commands on real Launchpad and audio device"
```

---

### Task 14: Hardware session (user at the Launchpad)

No code. Needs a folder with a few drum samples (WAV/MP3/FLAC/OGG).

- [ ] **Step 1: Build a set**

```powershell
cargo run --release -p lpd-cli -- new-set "$env:USERPROFILE\Music\lpd-sets\first" "<folder with drum samples>"
```
Expected: `Created … with N pads in bank 1.`; `samples/` and `set.json` exist in the set folder.

- [ ] **Step 2: Play** (close other MIDI apps first)

```powershell
cargo run --release -p lpd-cli -- play "$env:USERPROFILE\Music\lpd-sets\first"
```
Expected: `Audio: … @ 48000 Hz`, pads light in row colours, side button 1 white.

- [ ] **Step 3: Check, and note results**
  - Each lit pad plays its sound; empty pads silent and dark.
  - Hitting the same pad fast restarts the sound without clicks.
  - Side button 2 switches to an empty bank (grid dark, button 2 lit); a sound still ringing from bank 1 continues.
  - Mixer button stops all sounds.
  - Hand-edit `set.json`: give two pads `"chokeGroup": 1`; restart `play`; the second pad cuts the first.
  - Hand-edit one pad to `"mode": "hold"`; it stops shortly after release.
  - Rename one sample file; restart `play`; that pad is red and the app still runs.
  - Play continuously for 10 minutes: no crackles or dropouts.

- [ ] **Step 4: Rough latency check**
  Record with a phone placed between the Launchpad and the speaker while tapping a pad sharply. In an audio editor, measure tap click → sound onset. Record the number in `docs/latency-notes.md` with the audio device and buffer used. Retry with `--buffer 128` and compare.

- [ ] **Step 5: Commit the notes**

```powershell
git add docs/latency-notes.md
git commit -m "docs: first hardware session and latency notes"
```

---

### Task 15: ASIO build (optional, for the < 10 ms target)

Needs an ASIO driver on the PC (audio interface driver, or ASIO4ALL).

- [ ] **Step 1: Install LLVM** (bindgen needs libclang)

```powershell
winget install --id LLVM.LLVM -e
```

- [ ] **Step 2: Get the ASIO SDK**: download "ASIO SDK" from steinberg.net/developers, extract to `C:\SDK\asiosdk`, then:

```powershell
[Environment]::SetEnvironmentVariable("CPAL_ASIO_DIR", "C:\SDK\asiosdk", "User")
[Environment]::SetEnvironmentVariable("LIBCLANG_PATH", "C:\Program Files\LLVM\bin", "User")
```
Open a new terminal.

- [ ] **Step 3: Build and run**

```powershell
cargo build --release -p lpd-cli --features asio
cargo run --release -p lpd-cli --features asio -- play "$env:USERPROFILE\Music\lpd-sets\first" --asio --buffer 64
```
Expected: `Audio: <ASIO driver name> @ … Hz, buffer Fixed(64)`; pads play. Repeat the phone latency check and add the result to `docs/latency-notes.md`.

- [ ] **Step 4: Commit**

```powershell
git add docs/latency-notes.md
git commit -m "docs: ASIO latency measurement"
```

---

## Not in this plan (covered by plans 1B / 1C)

- 1B: Tauri + Svelte app, router thread (MIDI callback straight to the audio queue), light modes (flash / play-state counter / ripple) and blinking missing pads, hot-plug and reconnect, humanize, drag and drop, pad editor, sample browser, autosave, settings with latency and dropout counter, logging, safe sample unloading when a set changes (old `Arc<Sample>` must not be freed on the audio thread).
- 1C: Launch Control XL mapping and LEDs (probe first), other MIDI devices, MIDI learn.
