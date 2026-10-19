//! MIDI device registry and the generic input poller (every port that is not a Launchpad or a
//! Launch Control XL).

use crate::state::AppState;
use lpd_core::devices::launchpad as lp;
use midir::{MidiInput, MidiInputConnection};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const RECHECK: Duration = Duration::from_secs(2);

/// `role`: "launchpad" | "lcxl" | "keys" | "other"; `status`: "connected" | "disconnected" |
/// "busy" | "bootloader".
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MidiDevice {
    pub port: String,
    pub role: String,
    pub status: String,
}

pub fn classify(port: &str, roles: &BTreeMap<String, String>) -> &'static str {
    if lp::is_launchpad_port(port) {
        return "launchpad";
    }
    if port.contains("Launch Control XL") {
        return "lcxl";
    }
    match roles.get(port).map(String::as_str) {
        Some("keys") => "keys",
        Some("other") => "other",
        _ if port.to_ascii_lowercase().contains("keystation") => "keys",
        _ => "other",
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MidiMsg {
    NoteOn { ch: u8, note: u8, vel: u8 },
    NoteOff { ch: u8, note: u8 },
    Cc { ch: u8, cc: u8, value: u8 },
    PitchBend { ch: u8, value: i16 },
}

/// Channel messages the app understands; anything else (SysEx, clock, running status, short
/// messages) is `None`.
pub fn parse(bytes: &[u8]) -> Option<MidiMsg> {
    let [status, a, b] = *bytes else {
        return None;
    };
    if a > 0x7F || b > 0x7F {
        return None;
    }
    let ch = status & 0x0F;
    match status & 0xF0 {
        0x90 if b > 0 => Some(MidiMsg::NoteOn {
            ch,
            note: a,
            vel: b,
        }),
        0x90 | 0x80 => Some(MidiMsg::NoteOff { ch, note: a }),
        0xB0 => Some(MidiMsg::Cc {
            ch,
            cc: a,
            value: b,
        }),
        0xE0 => Some(MidiMsg::PitchBend {
            ch,
            value: (i16::from(a) | i16::from(b) << 7) - 8192,
        }),
        _ => None,
    }
}

#[derive(Default)]
pub struct Registry {
    devices: Vec<MidiDevice>,
}

impl Registry {
    /// Add or update a device. True when the list changed.
    pub fn set(&mut self, port: &str, role: &str, status: &str) -> bool {
        match self.devices.iter_mut().find(|d| d.port == port) {
            Some(d) if d.role == role && d.status == status => false,
            Some(d) => {
                d.role = role.into();
                d.status = status.into();
                true
            }
            None => {
                self.devices.push(MidiDevice {
                    port: port.into(),
                    role: role.into(),
                    status: status.into(),
                });
                true
            }
        }
    }

    /// Forget generic inputs that are no longer plugged in. The Launchpad and the LCXL stay
    /// listed (as disconnected): their bridges own those rows.
    pub fn remove_absent(&mut self, present: &[String]) -> bool {
        let before = self.devices.len();
        self.devices
            .retain(|d| FIXED_ROLES.contains(&d.role.as_str()) || present.contains(&d.port));
        self.devices.len() != before
    }

    pub fn list(&self) -> Vec<MidiDevice> {
        self.devices.clone()
    }

    pub fn get(&self, port: &str) -> Option<&MidiDevice> {
        self.devices.iter().find(|d| d.port == port)
    }
}

const FIXED_ROLES: [&str; 2] = ["launchpad", "lcxl"];

/// The registry the pollers and bridges share (a leaf lock: take it, copy the list, release,
/// then emit).
pub type SharedRegistry = Arc<Mutex<Registry>>;

/// Record a device's state and tell the UI when the list changed.
pub fn report(app: &AppHandle, registry: &SharedRegistry, port: &str, role: &str, status: &str) {
    let changed = {
        let mut r = registry.lock().unwrap_or_else(PoisonError::into_inner);
        r.set(port, role, status).then(|| r.list())
    };
    if let Some(list) = changed {
        announce(app, list);
    }
}

fn announce(app: &AppHandle, list: Vec<MidiDevice>) {
    if let Err(error) = app.emit("midi-devices", list) {
        crate::logfile::log(format!("MIDI devices event failed: {error}"));
    }
}

/// Poll the input ports every 2 s and listen to every generic one (role `keys` or `other`).
/// Launchpad and LCXL ports are never opened here: their bridges own them.
pub fn start(app: AppHandle, state: AppState) {
    std::thread::spawn(move || {
        let mut connections: HashMap<String, (&'static str, MidiInputConnection<()>)> =
            HashMap::new();
        loop {
            poll(&app, &state, &mut connections);
            std::thread::sleep(RECHECK);
        }
    });
}

fn poll(
    app: &AppHandle,
    state: &AppState,
    connections: &mut HashMap<String, (&'static str, MidiInputConnection<()>)>,
) {
    let Ok(input) = MidiInput::new("launchpad-drummer-devices") else {
        return;
    };
    let names: Vec<String> = input
        .ports()
        .iter()
        .filter_map(|port| input.port_name(port).ok())
        .collect();
    let roles = state.get_config().midi_roles;
    // A vanished port, or one whose role changed, is closed; the loop below reopens it.
    connections.retain(|name, (role, _)| names.contains(name) && classify(name, &roles) == *role);

    let mut present = Vec::new();
    for name in &names {
        let role = classify(name, &roles);
        if FIXED_ROLES.contains(&role) {
            continue;
        }
        present.push(name.clone());
        let status = if connections.contains_key(name) {
            "connected"
        } else {
            match connect(app, state, name, role) {
                Ok(connection) => {
                    connections.insert(name.clone(), (role, connection));
                    "connected"
                }
                Err(error) => {
                    if state.midi_status(name) != Some("busy".to_string()) {
                        crate::logfile::log(format!("MIDI input {name}: {error}"));
                    }
                    "busy"
                }
            }
        };
        report(app, &state.midi, name, role, status);
    }
    let removed = {
        let mut r = state.midi.lock().unwrap_or_else(PoisonError::into_inner);
        r.remove_absent(&present).then(|| r.list())
    };
    if let Some(list) = removed {
        announce(app, list);
    }
}

fn connect(
    app: &AppHandle,
    state: &AppState,
    name: &str,
    role: &'static str,
) -> Result<MidiInputConnection<()>, String> {
    let input = MidiInput::new("launchpad-drummer-generic-in").map_err(|e| e.to_string())?;
    let port = input
        .ports()
        .into_iter()
        .find(|port| input.port_name(port).ok().as_deref() == Some(name))
        .ok_or("the port disappeared")?;
    let app = app.clone();
    let state = state.clone();
    let port_name = name.to_string();
    input
        .connect(
            &port,
            "launchpad-drummer-generic-in",
            move |_, bytes, _| {
                let Some(msg) = parse(bytes) else {
                    return;
                };
                // The router lock is released before anything is emitted.
                for event in state.on_midi(&port_name, role, msg) {
                    let _ = app.emit(event.name(), event.payload());
                }
            },
            (),
        )
        .map_err(|e| e.to_string())
}

/// Resolve `app.state()` once per bridge call site.
pub fn registry_of(app: &AppHandle) -> SharedRegistry {
    app.state::<AppState>().midi.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    #[test]
    fn classifies_by_port_name() {
        for name in ["Launchpad MK2", "Launchpad MK2 16", "2- Launchpad MK2"] {
            assert_eq!(classify(name, &none()), "launchpad", "{name}");
        }
        assert_eq!(classify("Launch Control XL", &none()), "lcxl");
        assert_eq!(classify("MIDIIN2 (Launch Control XL)", &none()), "lcxl");
        assert_eq!(classify("Keystation 49 MK3", &none()), "keys");
        assert_eq!(classify("keystation 61", &none()), "keys");
        assert_eq!(classify("Some Synth", &none()), "other");
    }

    #[test]
    fn user_roles_override_only_generic_ports() {
        let keys = BTreeMap::from([("Some Synth".to_string(), "keys".to_string())]);
        assert_eq!(classify("Some Synth", &keys), "keys");
        let other = BTreeMap::from([("Keystation 49 MK3".to_string(), "other".to_string())]);
        assert_eq!(classify("Keystation 49 MK3", &other), "other");
        // A fixed device keeps its role whatever the config says.
        let bogus = BTreeMap::from([
            ("Launchpad MK2".to_string(), "keys".to_string()),
            ("Launch Control XL".to_string(), "other".to_string()),
        ]);
        assert_eq!(classify("Launchpad MK2", &bogus), "launchpad");
        assert_eq!(classify("Launch Control XL", &bogus), "lcxl");
        // Anything but keys/other in the config is ignored.
        let junk = BTreeMap::from([("Some Synth".to_string(), "lcxl".to_string())]);
        assert_eq!(classify("Some Synth", &junk), "other");
    }

    #[test]
    fn parses_channel_messages() {
        assert_eq!(
            parse(&[0x90, 0x3C, 0x64]),
            Some(MidiMsg::NoteOn {
                ch: 0,
                note: 60,
                vel: 100
            })
        );
        assert_eq!(
            parse(&[0x93, 0x3C, 0x64]),
            Some(MidiMsg::NoteOn {
                ch: 3,
                note: 60,
                vel: 100
            })
        );
        assert_eq!(
            parse(&[0x90, 0x3C, 0x00]),
            Some(MidiMsg::NoteOff { ch: 0, note: 60 })
        );
        assert_eq!(
            parse(&[0x80, 0x3C, 0x40]),
            Some(MidiMsg::NoteOff { ch: 0, note: 60 })
        );
        assert_eq!(
            parse(&[0xB0, 0x01, 0x7F]),
            Some(MidiMsg::Cc {
                ch: 0,
                cc: 1,
                value: 127
            })
        );
    }

    #[test]
    fn parses_pitch_bend_centre_and_extremes() {
        let bend = |bytes: &[u8]| match parse(bytes) {
            Some(MidiMsg::PitchBend { value, .. }) => value,
            other => panic!("{other:?}"),
        };
        assert_eq!(bend(&[0xE0, 0x00, 0x40]), 0);
        assert_eq!(bend(&[0xE0, 0x7F, 0x7F]), 8191);
        assert_eq!(bend(&[0xE0, 0x00, 0x00]), -8192);
    }

    #[test]
    fn rejects_everything_else() {
        assert_eq!(parse(&[0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7]), None);
        assert_eq!(parse(&[0x3C, 0x64]), None, "running status");
        assert_eq!(parse(&[0x90, 0x3C]), None, "short message");
        assert_eq!(parse(&[]), None);
        assert_eq!(parse(&[0xC0, 0x05]), None, "program change");
        assert_eq!(parse(&[0xF8]), None, "clock");
    }

    #[test]
    fn registry_set_reports_only_real_changes() {
        let mut r = Registry::default();
        assert!(r.set("Keystation 49 MK3", "keys", "connected"));
        assert!(!r.set("Keystation 49 MK3", "keys", "connected"));
        assert!(r.set("Keystation 49 MK3", "keys", "busy"));
        assert!(r.set("Keystation 49 MK3", "other", "busy"));
        assert_eq!(r.list().len(), 1);
        assert_eq!(r.list()[0].role, "other");
    }

    #[test]
    fn remove_absent_drops_vanished_generic_ports_and_keeps_fixed_devices() {
        let mut r = Registry::default();
        r.set("Launchpad MK2", "launchpad", "disconnected");
        r.set("Launch Control XL", "lcxl", "connected");
        r.set("Keystation 49 MK3", "keys", "connected");
        r.set("Some Synth", "other", "busy");
        assert!(!r.remove_absent(&["Keystation 49 MK3".to_string(), "Some Synth".to_string()]));
        assert!(r.remove_absent(&["Some Synth".to_string()]));
        let ports: Vec<String> = r.list().into_iter().map(|d| d.port).collect();
        assert_eq!(ports, ["Launchpad MK2", "Launch Control XL", "Some Synth"]);
    }

    #[test]
    fn serialises_camel_case_for_the_ui() {
        let d = MidiDevice {
            port: "P".into(),
            role: "keys".into(),
            status: "connected".into(),
        };
        assert_eq!(
            serde_json::to_string(&d).unwrap(),
            r#"{"port":"P","role":"keys","status":"connected"}"#
        );
    }
}
