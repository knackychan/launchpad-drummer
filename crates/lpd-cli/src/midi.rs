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
            self.conn
                .send(&m)
                .map_err(|e| anyhow!("LED send failed: {e}"))?;
        }
        Ok(())
    }
}

pub fn connect_launchpad() -> Result<(LaunchpadIn, LaunchpadOut)> {
    let midi_in = MidiInput::new("lpd-cli-in")?;
    let in_port = midi_in
        .ports()
        .into_iter()
        .find(|p| {
            midi_in
                .port_name(p)
                .is_ok_and(|n| lp::is_launchpad_port(&n))
        })
        .context("Launchpad MK2 not found (plugged in? used by another app? bootloader mode?)")?;
    let (tx, rx) = mpsc::channel();
    let conn_in = midi_in
        .connect(
            &in_port,
            "lpd-cli-in",
            move |_, msg, _| {
                if let Some(ev) = lp::decode(msg) {
                    let _ = tx.send(ev);
                }
            },
            (),
        )
        .map_err(|e| anyhow!("open Launchpad input: {e}"))?;
    let midi_out = MidiOutput::new("lpd-cli-out")?;
    let out_port = midi_out
        .ports()
        .into_iter()
        .find(|p| {
            midi_out
                .port_name(p)
                .is_ok_and(|n| lp::is_launchpad_port(&n))
        })
        .context("Launchpad MK2 output port not found")?;
    let mut conn = midi_out
        .connect(&out_port, "lpd-cli-out")
        .map_err(|e| anyhow!("open Launchpad output: {e}"))?;
    conn.send(&lp::SELECT_SESSION_LAYOUT)
        .map_err(|e| anyhow!("{e}"))?;
    conn.send(&lp::set_all(0)).map_err(|e| anyhow!("{e}"))?;
    Ok((
        LaunchpadIn {
            events: rx,
            _conn: conn_in,
        },
        LaunchpadOut { conn },
    ))
}
