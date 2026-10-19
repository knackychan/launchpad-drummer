//! MIDI device registry and the generic input poller (every port that is not a Launchpad or a
//! Launch Control XL).

use crate::router::UiEvent;
use crate::state::AppState;
use lpd_core::devices::launchpad as lp;
use midir::{MidiInput, MidiInputConnection};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};
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
    // The bridge matches only the main port; secondary ports such as "MIDIIN2 (Launchpad MK2)"
    // are classified too, so nobody opens them as a generic input.
    if lp::is_launchpad_port(port) || port.contains("Launchpad MK2") {
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

/// Controller messages (pitch bend, CCs) reach the log at most this often per kind.
const LOG_INTERVAL: Duration = Duration::from_millis(100);
/// The emitter thread sends the newest wheel position this often (~60 Hz).
const WHEEL_INTERVAL: Duration = Duration::from_millis(16);

/// Decides which messages get a `midi-log` line: notes always, pitch bend and each CC number at
/// most once per `LOG_INTERVAL`. Only the log is thinned; every message is still processed.
pub struct LogThrottle {
    bend: Option<Instant>,
    cc: [Option<Instant>; 128],
}

impl LogThrottle {
    pub fn new() -> Self {
        Self {
            bend: None,
            cc: [None; 128],
        }
    }

    pub fn allow(&mut self, msg: &MidiMsg, now: Instant) -> bool {
        let last = match msg {
            MidiMsg::NoteOn { .. } | MidiMsg::NoteOff { .. } => return true,
            MidiMsg::PitchBend { .. } => &mut self.bend,
            MidiMsg::Cc { cc, .. } => &mut self.cc[usize::from(*cc & 0x7F)],
        };
        if last.is_some_and(|t| now.duration_since(t) < LOG_INTERVAL) {
            return false;
        }
        *last = Some(now);
        true
    }
}

impl Default for LogThrottle {
    fn default() -> Self {
        Self::new()
    }
}

/// Notes currently held on one input (a 128-bit set), shared by its callback and the poller so a
/// closing input can release them.
#[derive(Default)]
struct Held([AtomicU64; 2]);

impl Held {
    fn set(&self, note: u8, on: bool) {
        let bit = 1u64 << (note & 63);
        let word = &self.0[usize::from((note >> 6) & 1)];
        if on {
            word.fetch_or(bit, Ordering::Relaxed);
        } else {
            word.fetch_and(!bit, Ordering::Relaxed);
        }
    }

    fn take_all(&self) -> Vec<u8> {
        let mut notes = Vec::new();
        for (index, word) in self.0.iter().enumerate() {
            let bits = word.swap(0, Ordering::Relaxed);
            notes.extend(
                (0..64u8)
                    .filter(|n| (bits >> n) & 1 == 1)
                    .map(|n| n + 64 * index as u8),
            );
        }
        notes
    }
}

struct Open {
    role: &'static str,
    held: Arc<Held>,
    connection: MidiInputConnection<()>,
}

/// Close an input and tell the UI about what it leaves behind: notes still held are released
/// and the wheels return to rest, so nothing stays stuck.
fn release(app: &AppHandle, state: &AppState, open: Open) {
    let Open {
        role,
        held,
        connection,
    } = open;
    drop(connection);
    for note in held.take_all() {
        let off = UiEvent::KeysNote {
            note,
            velocity: 0,
            on: false,
        };
        let _ = app.emit(off.name(), off.payload());
    }
    if role == "keys" {
        state.keys_input_closed();
    }
}

/// Poll the input ports every 2 s and listen to every generic one (role `keys` or `other`).
/// Launchpad and LCXL ports are never opened here: their bridges own them.
pub fn start(app: AppHandle, state: AppState) {
    let (wheel_app, wheel_state) = (app.clone(), state.clone());
    std::thread::spawn(move || loop {
        std::thread::sleep(WHEEL_INTERVAL);
        if let Some(wheel) = wheel_state.take_wheel() {
            let _ = wheel_app.emit(wheel.name(), wheel.payload());
        }
    });
    std::thread::spawn(move || {
        let mut connections: HashMap<String, Open> = HashMap::new();
        loop {
            poll(&app, &state, &mut connections);
            std::thread::sleep(RECHECK);
        }
    });
}

fn poll(app: &AppHandle, state: &AppState, connections: &mut HashMap<String, Open>) {
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
    let stale: Vec<String> = connections
        .iter()
        .filter(|(name, open)| !names.contains(name) || classify(name, &roles) != open.role)
        .map(|(name, _)| name.clone())
        .collect();
    for name in stale {
        if let Some(open) = connections.remove(&name) {
            release(app, state, open);
        }
    }

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
                Ok(open) => {
                    connections.insert(name.clone(), open);
                    "connected"
                }
                Err(error) => {
                    if state.midi_status(name).as_deref() != Some("busy") {
                        crate::logfile::log(format!("MIDI input {name}: {error}"));
                    }
                    "busy"
                }
            }
        };
        // The player may have flipped the role while this round ran: report the newest one, so
        // the switch does not snap back (the next round reopens the port with it).
        let current = classify(name, &state.get_config().midi_roles);
        report(app, &state.midi, name, current, status);
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
) -> Result<Open, String> {
    let input = MidiInput::new("launchpad-drummer-generic-in").map_err(|e| e.to_string())?;
    let port = input
        .ports()
        .into_iter()
        .find(|port| input.port_name(port).ok().as_deref() == Some(name))
        .ok_or("the port disappeared")?;
    let app = app.clone();
    let state = state.clone();
    let port_name = name.to_string();
    let held = Arc::new(Held::default());
    let callback_held = held.clone();
    let mut throttle = LogThrottle::new();
    let connection = input
        .connect(
            &port,
            "launchpad-drummer-generic-in",
            move |_, bytes, _| {
                let Some(msg) = parse(bytes) else {
                    return;
                };
                if role == "keys" {
                    match msg {
                        MidiMsg::NoteOn { note, .. } => callback_held.set(note, true),
                        MidiMsg::NoteOff { note, .. } => callback_held.set(note, false),
                        _ => {}
                    }
                }
                let log = throttle.allow(&msg, Instant::now());
                // The router lock is released before anything is emitted.
                for event in state.on_midi(&port_name, role, msg, log) {
                    let _ = app.emit(event.name(), event.payload());
                }
            },
            (),
        )
        .map_err(|e| e.to_string())?;
    Ok(Open {
        role,
        held,
        connection,
    })
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
        for name in [
            "Launchpad MK2",
            "Launchpad MK2 16",
            "2- Launchpad MK2",
            "MIDIIN2 (Launchpad MK2)",
        ] {
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
    fn log_throttle_thins_controllers_but_never_notes() {
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        let mut t = LogThrottle::new();
        let bend = MidiMsg::PitchBend { ch: 0, value: 1 };
        let cc = |cc| MidiMsg::Cc {
            ch: 0,
            cc,
            value: 1,
        };
        assert!(t.allow(&bend, ms(0)));
        assert!(!t.allow(&bend, ms(50)));
        assert!(!t.allow(&bend, ms(99)));
        assert!(t.allow(&bend, ms(100)));
        // Each CC number has its own interval, and bend and CC do not share one.
        assert!(t.allow(&cc(1), ms(100)));
        assert!(t.allow(&cc(7), ms(100)));
        assert!(!t.allow(&cc(1), ms(150)));
        assert!(t.allow(&cc(1), ms(200)));
        let on = MidiMsg::NoteOn {
            ch: 0,
            note: 60,
            vel: 1,
        };
        for n in 0..5 {
            assert!(t.allow(&on, ms(200 + n)));
            assert!(t.allow(&MidiMsg::NoteOff { ch: 0, note: 60 }, ms(200 + n)));
        }
    }

    #[test]
    fn held_notes_are_remembered_and_released_once() {
        let held = Held::default();
        for note in [0, 60, 63, 64, 127] {
            held.set(note, true);
        }
        held.set(63, false);
        assert_eq!(held.take_all(), vec![0, 60, 64, 127]);
        assert!(held.take_all().is_empty());
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
