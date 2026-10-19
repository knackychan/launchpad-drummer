//! Launch Control XL MIDI bridge: controls drive the mixer, mute/solo light the button LEDs.

use lpd_core::devices::lcxl::{self, LedColor};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use serde::Serialize;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use crate::state::AppState;

const RECHECK: Duration = Duration::from_secs(2);
/// The device's row in the shared MIDI device registry (SETUP › MIDI DEVICES).
const REGISTRY_PORT: &str = "Launch Control XL";
/// `mixer-changed` goes out at most this often; the last value always follows.
const MIXER_EMIT_INTERVAL: Duration = Duration::from_millis(33);
/// Loop period: three ticks make one emit interval.
const TICK: Duration = Duration::from_millis(11);
/// The LEDs follow mute/solo within this long (the screen's own changes included).
const LED_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeviceStatus {
    Connected,
    Disconnected,
    Busy,
}

impl DeviceStatus {
    fn name(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
            Self::Busy => "busy",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::Connected => "Launch Control XL connected",
            Self::Disconnected => "Launch Control XL not connected — checking every 2 s",
            Self::Busy => "Launch Control XL port busy — close other MIDI apps",
        }
    }
}

type LedState = ([bool; 8], [bool; 8]);

struct Connection {
    input_name: String,
    output_name: String,
    output: MidiOutputConnection,
    _input: MidiInputConnection<()>,
    /// What the button LEDs were last told (`None`: nothing yet, send everything).
    leds: Option<LedState>,
}

/// `mixer-changed` payload (plan 0.4).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct MixerChanged {
    groups: Vec<lpd_core::kit::Group>,
    master_db: f32,
}

pub fn start(app: AppHandle, state: AppState) {
    std::thread::spawn(move || run(app, state));
}

/// The LED messages that bring the device from `last` to `(mute, solo)`: row 0 is amber while a
/// channel is muted, row 1 green while it is soloed. With no `last` all sixteen are sent.
fn led_updates(last: Option<LedState>, mute: [bool; 8], solo: [bool; 8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for ch in 0..8usize {
        if last.is_none_or(|(m, _)| m[ch] != mute[ch]) {
            let c = if mute[ch] {
                LedColor::Amber
            } else {
                LedColor::Off
            };
            out.push(lcxl::led_message(
                lcxl::USER_TEMPLATE,
                lcxl::led_index(0, ch as u8),
                lcxl::led_value(c),
            ));
        }
        if last.is_none_or(|(_, s)| s[ch] != solo[ch]) {
            let c = if solo[ch] {
                LedColor::Green
            } else {
                LedColor::Off
            };
            out.push(lcxl::led_message(
                lcxl::USER_TEMPLATE,
                lcxl::led_index(1, ch as u8),
                lcxl::led_value(c),
            ));
        }
    }
    out
}

/// A change waits until `MIXER_EMIT_INTERVAL` has passed since the previous emit, so a burst
/// becomes at most ~30 events/s and its last value is always sent (trailing edge).
fn emit_due(last_emit: Instant, now: Instant) -> bool {
    now.duration_since(last_emit) >= MIXER_EMIT_INTERVAL
}

fn run(app: AppHandle, state: AppState) {
    let mut connection: Option<Connection> = None;
    let mut status = None;
    let start = Instant::now();
    let mut next_check = start;
    let mut last_emit = start;
    let mut last_set_emit = start;
    let mut last_led = start;
    loop {
        let now = Instant::now();
        if now >= next_check {
            check_connection(&app, &state, &mut connection, &mut status);
            next_check = now + RECHECK;
        }
        if emit_due(last_emit, now) && state.take_mixer_change() {
            let mixer = state.mixer();
            let payload = MixerChanged {
                groups: mixer.groups,
                master_db: mixer.master_db,
            };
            if let Err(error) = app.emit("mixer-changed", payload) {
                crate::logfile::log(format!("Launch Control XL mixer event failed: {error}"));
            }
            last_emit = now;
        }
        if emit_due(last_set_emit, now) && state.take_set_change() {
            // A learned control changed the set (bank parameters): the screen takes it whole.
            if let Err(error) = app.emit("set-changed", state.get_current_set()) {
                crate::logfile::log(format!("set-changed event failed: {error}"));
            }
            last_set_emit = now;
        }
        if now.duration_since(last_led) >= LED_INTERVAL {
            last_led = now;
            if let Some(c) = connection.as_mut() {
                let (mute, solo) = state.mute_solo();
                let updates = led_updates(c.leds, mute, solo);
                let mut failed = None;
                for message in &updates {
                    if let Err(error) = c.output.send(message) {
                        failed = Some(error.to_string());
                        break;
                    }
                }
                match failed {
                    None => c.leds = Some((mute, solo)),
                    Some(error) => {
                        crate::logfile::log(format!("Launch Control XL LED send failed: {error}"));
                        connection = None;
                        report_status(&app, &mut status, DeviceStatus::Disconnected);
                    }
                }
            }
        }
        std::thread::sleep(TICK);
    }
}

fn check_connection(
    app: &AppHandle,
    state: &AppState,
    connection: &mut Option<Connection>,
    status: &mut Option<DeviceStatus>,
) {
    let (Ok(input), Ok(output)) = (
        MidiInput::new("launchpad-drummer-lcxl-in"),
        MidiOutput::new("launchpad-drummer-lcxl-out"),
    ) else {
        *connection = None;
        report_status(app, status, DeviceStatus::Disconnected);
        return;
    };
    let input_port = input.ports().into_iter().find_map(|port| {
        input
            .port_name(&port)
            .ok()
            .filter(|name| lcxl::is_lcxl_port(name))
            .map(|name| (port, name))
    });
    let output_port = output.ports().into_iter().find_map(|port| {
        output
            .port_name(&port)
            .ok()
            .filter(|name| lcxl::is_lcxl_port(name))
            .map(|name| (port, name))
    });
    let (Some((input_port, input_name)), Some((output_port, output_name))) =
        (input_port, output_port)
    else {
        *connection = None;
        report_status(app, status, DeviceStatus::Disconnected);
        return;
    };
    if connection
        .as_ref()
        .is_some_and(|c| c.input_name == input_name && c.output_name == output_name)
    {
        report_status(app, status, DeviceStatus::Connected);
        return;
    }
    *connection = None;
    let app_state = state.clone();
    let events = app.clone();
    let input_connection = match input.connect(
        &input_port,
        "launchpad-drummer-lcxl-in",
        move |_, msg, _| {
            if let Some(ev) = lcxl::decode(msg) {
                if let Some(ignored) = app_state.on_lcxl(ev) {
                    let _ = events.emit(ignored.name(), ignored.payload());
                }
            }
        },
        (),
    ) {
        Ok(input) => input,
        Err(error) => {
            if *status != Some(DeviceStatus::Busy) {
                crate::logfile::log(format!(
                    "Launch Control XL input connection failed: {error}"
                ));
            }
            report_status(app, status, DeviceStatus::Busy);
            return;
        }
    };
    let output_connection = match output.connect(&output_port, "launchpad-drummer-lcxl-out") {
        Ok(output) => output,
        Err(error) => {
            if *status != Some(DeviceStatus::Busy) {
                crate::logfile::log(format!(
                    "Launch Control XL output connection failed: {error}"
                ));
            }
            report_status(app, status, DeviceStatus::Busy);
            return;
        }
    };
    *connection = Some(Connection {
        input_name,
        output_name,
        output: output_connection,
        _input: input_connection,
        leds: None, // a new device gets all sixteen LEDs on the next LED tick
    });
    report_status(app, status, DeviceStatus::Connected);
}

fn report_status(app: &AppHandle, previous: &mut Option<DeviceStatus>, current: DeviceStatus) {
    if *previous != Some(current) {
        crate::logfile::log(format!("Launch Control XL: {}", current.message()));
        *previous = Some(current);
    }
    crate::devices::report(
        app,
        &crate::devices::registry_of(app),
        REGISTRY_PORT,
        "lcxl",
        current.name(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: [bool; 8] = [false; 8];

    #[test]
    fn a_new_device_gets_all_sixteen_leds() {
        let updates = led_updates(None, NONE, NONE);
        assert_eq!(updates.len(), 16);
        // Row 0 channel 0 is index 24 and off (0x0C); row 1 channel 7 is index 39 and off.
        assert_eq!(updates[0][8..10], [24, 0x0C]);
        assert_eq!(updates[15][8..10], [39, 0x0C]);
    }

    #[test]
    fn mute_is_amber_on_row_0_and_solo_green_on_row_1() {
        let mut mute = NONE;
        mute[2] = true;
        let mut solo = NONE;
        solo[5] = true;
        let updates = led_updates(Some((NONE, NONE)), mute, solo);
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0][8..10], [26, 0x3F]); // 24 + 2, amber
        assert_eq!(updates[1][8..10], [37, 0x3C]); // 32 + 5, green
    }

    #[test]
    fn only_changes_are_sent_and_nothing_when_equal() {
        let mut mute = NONE;
        mute[0] = true;
        assert!(led_updates(Some((mute, NONE)), mute, NONE).is_empty());
        let off = led_updates(Some((mute, NONE)), NONE, NONE);
        assert_eq!(off.len(), 1);
        assert_eq!(off[0][8..10], [24, 0x0C]);
    }

    #[test]
    fn mixer_events_are_spaced_by_33_ms() {
        let t0 = Instant::now();
        assert!(!emit_due(t0, t0 + Duration::from_millis(32)));
        assert!(emit_due(t0, t0 + Duration::from_millis(33)));
    }
}
