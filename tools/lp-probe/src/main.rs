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
        .find(|p| {
            midi_in
                .port_name(p)
                .is_ok_and(|n| lp::is_launchpad_port(&n))
        })
        .context(
            "no Launchpad MK2 input port (plugged in? bootloader mode? used by another app?)",
        )?;
    let port_name = midi_in.port_name(&in_port)?;
    let (tx, rx) = mpsc::channel();
    let input = midi_in
        .connect(
            &in_port,
            "lp-probe-in",
            move |_, msg, _| {
                let _ = tx.send(msg.to_vec());
            },
            (),
        )
        .map_err(|e| anyhow!("open Launchpad input: {e}"))?;
    let midi_out = MidiOutput::new("lp-probe-out")?;
    let out_port = midi_out
        .ports()
        .into_iter()
        .find(|p| {
            midi_out
                .port_name(p)
                .is_ok_and(|n| lp::is_launchpad_port(&n))
        })
        .context("no Launchpad MK2 output port")?;
    let out = midi_out
        .connect(&out_port, "lp-probe-out")
        .map_err(|e| anyhow!("open Launchpad output: {e}"))?;
    Ok((
        port_name,
        Probe {
            out,
            rx,
            _input: input,
            rows: Vec::new(),
        },
    ))
}

impl Probe {
    fn send(&mut self, msg: &[u8]) -> Result<()> {
        self.out
            .send(msg)
            .map_err(|e| anyhow!("send {}: {e}", hex(msg)))
    }

    fn drain(&self) {
        while self.rx.try_recv().is_ok() {}
    }

    fn record(&mut self, check: &str, detail: impl Into<String>, ok: bool) {
        let result = if ok { "verified" } else { "FAILED" };
        println!("  [{result}] {check}");
        self.rows
            .push((check.to_string(), detail.into(), result.to_string()));
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
            let Ok(m) = self.rx.recv_timeout(left) else {
                break;
            };
            if m.starts_with(&[0xF0, 0x7E]) && m.get(3..5) == Some(&[0x06, 0x02]) {
                let ok = m.get(5..10) == Some(&[0x00, 0x20, 0x29, 0x69, 0x00]);
                self.record(
                    "Device Inquiry reply: Novation, family 0x69 (Launchpad MK2)",
                    hex(&m),
                    ok,
                );
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
        self.ask(
            "Palette static: Note On ch1, velocity = palette index",
            &m,
            "Is the pad in column 4, row 5 (counting from bottom-left) solid red?",
        )?;
        let m = [0x91, lp::grid_led(4, 4), 21];
        self.send(&m)?;
        self.ask(
            "Palette flash: Note On ch2",
            &m,
            "Is the pad to its right flashing green?",
        )?;
        let m = [0x92, lp::grid_led(5, 4), 45];
        self.send(&m)?;
        self.ask(
            "Palette pulse: Note On ch3",
            &m,
            "Is the next pad to the right pulsing blue?",
        )?;
        let m = lp::set_all(0);
        self.send(&m)?;
        self.ask(
            "Set all LEDs: SysEx 0E <palette index>",
            &m,
            "Did every LED turn off?",
        )?;

        let top: Vec<(u8, [u8; 3])> = (0..8).map(|i| (lp::top_led(i), [0, 0, 63])).collect();
        let msgs = lp::rgb_messages(&top);
        for m in &msgs {
            self.send(m)?;
        }
        self.ask(
            "RGB SysEx on top-row LEDs 104..111",
            &msgs[0],
            "Are all 8 top-row round buttons blue?",
        )?;

        let side: Vec<(u8, [u8; 3])> = (0..8).map(|i| (lp::side_led(i), [63, 0, 63])).collect();
        let msgs = lp::rgb_messages(&side);
        for m in &msgs {
            self.send(m)?;
        }
        self.ask(
            "RGB SysEx on side LEDs 89..19",
            &msgs[0],
            "Are all 8 right-side round buttons purple?",
        )?;

        let grid: Vec<(u8, [u8; 3])> = (0..8u8)
            .flat_map(|y| (0..8u8).map(move |x| (lp::grid_led(x, y), [x * 9, y * 9, 20])))
            .collect();
        let msgs = lp::rgb_messages(&grid);
        for m in &msgs {
            self.send(m)?;
        }
        self.ask(
            "RGB SysEx with 64 LEDs in one message",
            &msgs[0][..12],
            "Is the grid a smooth gradient (dark blue bottom-left to yellow top-right)?",
        )?;
        self.send(&lp::set_all(0))?;
        Ok(())
    }

    fn input_tests(&mut self) -> Result<()> {
        println!("\n== Input tests: press each control when asked (15 s timeout each) ==");
        let mut expected: Vec<(String, LpEvent)> = vec![
            (
                "bottom-left grid pad".into(),
                LpEvent::PadDown { x: 0, y: 0 },
            ),
            (
                "bottom-right grid pad".into(),
                LpEvent::PadDown { x: 7, y: 0 },
            ),
            ("top-left grid pad".into(), LpEvent::PadDown { x: 0, y: 7 }),
            ("top-right grid pad".into(), LpEvent::PadDown { x: 7, y: 7 }),
        ];
        let top = [
            "Up", "Down", "Left", "Right", "Session", "User 1", "User 2", "Mixer",
        ];
        for (i, n) in top.iter().enumerate() {
            expected.push((format!("top-row button {n}"), LpEvent::TopDown(i as u8)));
        }
        let side = [
            "Volume",
            "Pan",
            "Send A",
            "Send B",
            "Stop",
            "Mute",
            "Solo",
            "Record Arm",
        ];
        for (i, n) in side.iter().enumerate() {
            expected.push((format!("side button {n}"), LpEvent::SideDown(i as u8)));
        }
        for (name, want) in expected {
            self.drain();
            println!("Press: {name}");
            match self.wait_press(Duration::from_secs(15)) {
                Some(raw) => {
                    let got = lp::decode(&raw);
                    self.record(
                        &format!("Input: {name}"),
                        format!("{} -> {got:?}", hex(&raw)),
                        got == Some(want),
                    );
                }
                None => self.record(&format!("Input: {name}"), "timeout", false),
            }
        }
        self.drain();
        println!("Press and HOLD any grid pad for 1 second, then release it.");
        let down = self.wait_press(Duration::from_secs(15));
        let up = self.rx.recv_timeout(Duration::from_secs(15)).ok();
        let ok = matches!(
            up.as_deref().and_then(lp::decode),
            Some(LpEvent::PadUp { .. })
        );
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
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/launchpad-mk2-protocol.md"
        );
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
    m.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
