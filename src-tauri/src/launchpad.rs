//! Launchpad MK2 MIDI bridge.

use lpd_core::devices::launchpad::{self as lp, LpEvent};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use serde::Serialize;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

const RECHECK: Duration = Duration::from_secs(2);
/// The Launchpad's row in the shared MIDI device registry (SETUP › MIDI DEVICES).
const REGISTRY_PORT: &str = "Launchpad MK2";

/// Latest game LED frame, replaced (never queued) by each new one.
#[derive(Clone, Default)]
pub struct FrameSender(Arc<Mutex<Option<Vec<u8>>>>);

impl FrameSender {
    fn take(&self) -> Option<Vec<u8>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).take()
    }
}

impl LedSink for MidiOutputConnection {
    fn send(&mut self, message: &[u8]) -> Result<(), String> {
        MidiOutputConnection::send(self, message).map_err(|e| e.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeviceStatus {
    Connected,
    Disconnected,
    Busy,
    Bootloader,
}

impl DeviceStatus {
    fn name(self) -> &'static str {
        self.payload().state
    }

    fn payload(self) -> StatusEvent {
        match self {
            Self::Connected => StatusEvent {
                state: "connected",
                message: "Launchpad MK2 connected",
            },
            Self::Disconnected => StatusEvent {
                state: "disconnected",
                message: "Launchpad not connected — checking every 2 s",
            },
            Self::Busy => StatusEvent {
                state: "busy",
                message: "Launchpad port busy — close Components, DAWs or another copy of this app",
            },
            Self::Bootloader => StatusEvent {
                state: "bootloader",
                message: "Launchpad in bootloader — run Novation's MK2 updater, then replug",
            },
        }
    }
}

#[derive(Clone, Copy, Serialize)]
struct StatusEvent {
    state: &'static str,
    message: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
struct PadEvent {
    note: u8,
    pressed: bool,
}

struct Connection {
    input_name: String,
    output_name: String,
    _input: MidiInputConnection<()>,
}

use crate::lights::{LedOutput, LedSink};
use crate::state::{AppState, InputMode};

pub fn start(app: AppHandle, state: AppState) {
    let slot = FrameSender::default();
    app.manage(slot.clone());
    std::thread::spawn(move || run(app, state, slot));
}

/// `frame` has 64 RGB triplets in visual order (top-left to bottom-right), channels 0..=63.
/// Only a game that owns the pads may draw; frames arriving in sampler mode are dropped.
#[tauri::command]
pub fn send_led_frame(
    frame: Vec<[u8; 3]>,
    sender: State<'_, FrameSender>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let message = rgb_message(&frame)?;
    if state.input_mode() == InputMode::Game {
        *sender.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(message);
    }
    Ok(())
}

fn decode_pad(msg: &[u8]) -> Option<PadEvent> {
    match lp::decode(msg) {
        Some(LpEvent::PadDown { x, y }) => Some(PadEvent {
            note: lp::grid_led(x, y),
            pressed: true,
        }),
        Some(LpEvent::PadUp { x, y }) => Some(PadEvent {
            note: lp::grid_led(x, y),
            pressed: false,
        }),
        Some(LpEvent::SideDown(i)) => Some(PadEvent {
            note: lp::side_led(i),
            pressed: true,
        }),
        Some(LpEvent::SideUp(i)) => Some(PadEvent {
            note: lp::side_led(i),
            pressed: false,
        }),
        Some(LpEvent::TopDown(_)) | Some(LpEvent::TopUp(_)) | None => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
struct TopEvent {
    /// 0 = leftmost round button above the grid .. 7 = rightmost.
    index: u8,
    pressed: bool,
}

fn decode_top(msg: &[u8]) -> Option<TopEvent> {
    match lp::decode(msg) {
        Some(LpEvent::TopDown(index)) => Some(TopEvent { index, pressed: true }),
        Some(LpEvent::TopUp(index)) => Some(TopEvent { index, pressed: false }),
        _ => None,
    }
}

fn rgb_message(frame: &[[u8; 3]]) -> Result<Vec<u8>, String> {
    if frame.len() != 64 || frame.iter().flatten().any(|value| *value > 63) {
        return Err("LED frame requires 64 pads with RGB channels from 0 to 63".to_string());
    }
    let leds: Vec<(u8, [u8; 3])> = frame
        .iter()
        .enumerate()
        .map(|(index, rgb)| (lp::grid_led((index % 8) as u8, (7 - index / 8) as u8), *rgb))
        .collect();
    let msgs = lp::rgb_messages(&leds);
    Ok(msgs.into_iter().next().expect("exactly one message"))
}

/// One RGB SysEx for an arbitrary set of LEDs: grid and side buttons (`row * 10 + col`, rows 1..=8,
/// cols 1..=9) and the top row (104..=111). A prototype game running in the sandboxed frame draws
/// all 80 of them through this, since it cannot open the MIDI port itself.
fn leds_message(leds: &[(u8, [u8; 3])]) -> Result<Vec<u8>, String> {
    let valid = |led: u8| {
        ((1..=8).contains(&(led / 10)) && (1..=9).contains(&(led % 10))) || (104..=111).contains(&led)
    };
    if leds.is_empty()
        || leds.len() > lp::MAX_LEDS_PER_MESSAGE
        || leds.iter().any(|(led, rgb)| !valid(*led) || rgb.iter().any(|value| *value > 63))
    {
        return Err(
            "LED list needs 1..=80 valid Launchpad LEDs with RGB channels from 0 to 63".to_string(),
        );
    }
    Ok(lp::rgb_messages(leds).into_iter().next().expect("exactly one message"))
}

/// Like `send_led_frame`, for a game that also lights the side and top buttons.
#[tauri::command]
pub fn send_led_leds(
    leds: Vec<(u8, [u8; 3])>,
    sender: State<'_, FrameSender>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let message = leds_message(&leds)?;
    if state.input_mode() == InputMode::Game {
        *sender.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(message);
    }
    Ok(())
}

fn run(app: AppHandle, state: AppState, game_frames: FrameSender) {
    let mut connection: Option<Connection> = None;
    let mut led: LedOutput<MidiOutputConnection> = LedOutput::default();
    let mut status = None;
    let mut next_check = Instant::now();
    let mut tick_counter: u32 = 0;
    let tick_interval = Duration::from_millis(16);
    loop {
        let now = Instant::now();
        if now >= next_check {
            check_connection(&app, &state, &mut connection, &mut led, &mut status);
            next_check = now + RECHECK;
        }

        state.maintain();

        let sent = match state.input_mode() {
            InputMode::Game => match game_frames.take() {
                Some(frame) => led.send_game(&state.painter, frame),
                None => Ok(()),
            },
            InputMode::Sampler => {
                led.clear_game();
                game_frames.take();
                let full_frame = state.paint_frame(now);
                let sent = led.send_sampler(&state.painter, &full_frame);

                tick_counter = tick_counter.wrapping_add(1);
                if tick_counter % 2 == 0 {
                    let mut screen_frame = [[0u8; 3]; 64];
                    for &(led_id, color) in &full_frame {
                        let row_1_based = led_id / 10;
                        let col_1_based = led_id % 10;
                        if (1..=8).contains(&row_1_based) && (1..=8).contains(&col_1_based) {
                            let y = row_1_based - 1;
                            let x = col_1_based - 1;
                            let screen_idx = (7 - y as usize) * 8 + x as usize;
                            screen_frame[screen_idx] = color;
                        }
                    }
                    let _ = app.emit("launchpad-frame", screen_frame.to_vec());
                }
                sent
            }
        };
        if let Err(error) = sent {
            crate::logfile::log(format!("Launchpad LED send failed: {error}"));
            connection = None;
            report_status(&app, &mut status, DeviceStatus::Disconnected);
        }

        std::thread::sleep(tick_interval);
    }
}

/// Forget the device: no connection, and the painter must repaint from scratch next time.
fn drop_device(
    connection: &mut Option<Connection>,
    led: &mut LedOutput<MidiOutputConnection>,
    state: &AppState,
) {
    *connection = None;
    led.disconnect(&state.painter);
}

fn check_connection(
    app: &AppHandle,
    state: &AppState,
    connection: &mut Option<Connection>,
    led: &mut LedOutput<MidiOutputConnection>,
    status: &mut Option<DeviceStatus>,
) {
    match bootloader_present() {
        Ok(true) => {
            drop_device(connection, led, state);
            report_status(app, status, DeviceStatus::Bootloader);
            return;
        }
        Err(error) => {
            if *status != Some(DeviceStatus::Disconnected) {
                crate::logfile::log(format!(
                    "Launchpad bootloader detection failed (CfgMgr32 code {error})"
                ));
            }
        }
        Ok(false) => {}
    }

    let input = match MidiInput::new("launchpad-drummer-in") {
        Ok(input) => input,
        Err(error) => {
            drop_device(connection, led, state);
            if *status != Some(DeviceStatus::Disconnected) {
                crate::logfile::log(format!("Launchpad input scan failed: {error}"));
            }
            report_status(app, status, DeviceStatus::Disconnected);
            return;
        }
    };
    let output = match MidiOutput::new("launchpad-drummer-out") {
        Ok(output) => output,
        Err(error) => {
            drop_device(connection, led, state);
            if *status != Some(DeviceStatus::Disconnected) {
                crate::logfile::log(format!("Launchpad output scan failed: {error}"));
            }
            report_status(app, status, DeviceStatus::Disconnected);
            return;
        }
    };
    let input_port = input.ports().into_iter().find_map(|port| {
        input
            .port_name(&port)
            .ok()
            .filter(|name| lp::is_launchpad_port(name))
            .map(|name| (port, name))
    });
    let output_port = output.ports().into_iter().find_map(|port| {
        output
            .port_name(&port)
            .ok()
            .filter(|name| lp::is_launchpad_port(name))
            .map(|name| (port, name))
    });
    let (Some((input_port, input_name)), Some((output_port, output_name))) =
        (input_port, output_port)
    else {
        drop_device(connection, led, state);
        report_status(app, status, DeviceStatus::Disconnected);
        return;
    };
    if connection
        .as_ref()
        .is_some_and(|device| device.input_name == input_name && device.output_name == output_name)
    {
        report_status(app, status, DeviceStatus::Connected);
        return;
    }
    drop_device(connection, led, state);
    let receiver = app.clone();
    let app_state = state.clone();
    let input_connection = match input.connect(
        &input_port,
        "launchpad-drummer-in",
        move |_, msg, _| {
            if let Some(ev) = lp::decode(msg) {
                if let Some(ignored) = app_state.on_lp_event(ev) {
                    let _ = receiver.emit(ignored.name(), ignored.payload());
                }
            }
            if let Some(pad) = decode_pad(msg) {
                if let Err(error) = receiver.emit("launchpad-pad", pad) {
                    crate::logfile::log(format!("Launchpad pad event failed: {error}"));
                }
            }
            // The round top-row buttons only matter to a game (the sampler ignores them).
            if app_state.input_mode() == InputMode::Game {
                if let Some(top) = decode_top(msg) {
                    let _ = receiver.emit("launchpad-top", top);
                }
            }
        },
        (),
    ) {
        Ok(input) => input,
        Err(error) => {
            if *status != Some(DeviceStatus::Busy) {
                crate::logfile::log(format!("Launchpad input connection failed: {error}"));
            }
            report_status(app, status, DeviceStatus::Busy);
            return;
        }
    };
    let output_connection = match output.connect(&output_port, "launchpad-drummer-out") {
        Ok(output) => output,
        Err(error) => {
            if *status != Some(DeviceStatus::Busy) {
                crate::logfile::log(format!("Launchpad output connection failed: {error}"));
            }
            report_status(app, status, DeviceStatus::Busy);
            return;
        }
    };
    // A new device shows nothing: the painter repaints in full, and a game that owns the pads
    // gets its last frame back.
    let initialised = led
        .connect(output_connection, &state.painter)
        .and_then(|_| match state.input_mode() {
            InputMode::Game => led.resend_game(&state.painter),
            InputMode::Sampler => Ok(()),
        });
    if let Err(error) = initialised {
        crate::logfile::log(format!("Launchpad initialization failed: {error}"));
        drop_device(connection, led, state);
        report_status(app, status, DeviceStatus::Disconnected);
        return;
    }
    *connection = Some(Connection {
        input_name,
        output_name,
        _input: input_connection,
    });
    report_status(app, status, DeviceStatus::Connected);
}

fn report_status(app: &AppHandle, previous: &mut Option<DeviceStatus>, current: DeviceStatus) {
    if *previous != Some(current) {
        crate::logfile::log(format!("Launchpad: {}", current.payload().message));
        *previous = Some(current);
    }
    crate::devices::report(
        app,
        &crate::devices::registry_of(app),
        REGISTRY_PORT,
        "launchpad",
        current.name(),
    );
    // Repeat the current state quietly so a UI listener mounted after startup receives it.
    if let Err(error) = app.emit("launchpad-status", current.payload()) {
        crate::logfile::log(format!("Launchpad status event failed: {error}"));
    }
}

fn is_bootloader_id(id: &str) -> bool {
    let id = id.to_ascii_uppercase();
    id.starts_with("USB\\VID_1235&PID_1069\\") || id.starts_with("USB\\VID_1235&PID_1069&")
}

#[cfg(windows)]
fn bootloader_present() -> Result<bool, u32> {
    #[link(name = "Cfgmgr32")]
    extern "system" {
        fn CM_Get_Device_ID_List_SizeW(len: *mut u32, filter: *const u16, flags: u32) -> u32;
        fn CM_Get_Device_ID_ListW(
            filter: *const u16,
            buffer: *mut u16,
            len: u32,
            flags: u32,
        ) -> u32;
    }
    const FLAGS: u32 = 0x0000_0101; // present devices from the USB enumerator
    let filter: Vec<u16> = "USB\0".encode_utf16().collect();
    let mut len = 0;
    let result = unsafe { CM_Get_Device_ID_List_SizeW(&mut len, filter.as_ptr(), FLAGS) };
    if result != 0 {
        return Err(result);
    }
    if len == 0 {
        return Ok(false);
    }
    let mut ids = vec![0u16; len as usize];
    let result = unsafe { CM_Get_Device_ID_ListW(filter.as_ptr(), ids.as_mut_ptr(), len, FLAGS) };
    if result != 0 {
        return Err(result);
    }
    Ok(ids
        .split(|unit| *unit == 0)
        .filter(|id| !id.is_empty())
        .any(|id| is_bootloader_id(&String::from_utf16_lossy(id))))
}

#[cfg(not(windows))]
fn bootloader_present() -> Result<bool, u32> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_only_launchpad_mk2_ports() {
        assert!(lp::is_launchpad_port("Launchpad MK2"));
        assert!(lp::is_launchpad_port("Launchpad MK2 16"));
        assert!(lp::is_launchpad_port("2- Launchpad MK2 16"));
        assert!(!lp::is_launchpad_port("Other Launchpad MK2"));
        assert!(!lp::is_launchpad_port("Launchpad MK2X"));
        assert!(!lp::is_launchpad_port("Launch Control XL"));
    }

    #[test]
    fn decodes_grid_and_side_button_press_and_release_only() {
        assert_eq!(
            decode_pad(&[0x90, 11, 127]),
            Some(PadEvent {
                note: 11,
                pressed: true
            })
        );
        assert_eq!(
            decode_pad(&[0x90, 88, 0]),
            Some(PadEvent {
                note: 88,
                pressed: false
            })
        );
        assert_eq!(
            decode_pad(&[0x80, 44, 64]),
            Some(PadEvent {
                note: 44,
                pressed: false
            })
        );
        assert_eq!(
            decode_pad(&[0x90, 89, 127]),
            Some(PadEvent {
                note: 89,
                pressed: true
            })
        );
        assert_eq!(
            decode_pad(&[0x80, 79, 64]),
            Some(PadEvent {
                note: 79,
                pressed: false
            })
        );
        for bytes in [
            &[0x91, 11, 127][..],
            &[0x90, 99, 127],
            &[0xB0, 104, 127],
            &[0x90, 11],
        ] {
            assert_eq!(decode_pad(bytes), None);
        }
    }

    #[test]
    fn encodes_screen_order_as_hardware_rows_and_validates_frame() {
        let mut frame = vec![[0, 0, 0]; 64];
        frame[0] = [63, 1, 2];
        frame[63] = [3, 4, 5];
        let message = rgb_message(&frame).unwrap();
        assert_eq!(&message[..7], &[0xF0, 0, 0x20, 0x29, 2, 0x18, 0x0B]);
        assert_eq!(&message[7..11], &[81, 63, 1, 2]);
        assert_eq!(&message[7 + 63 * 4..7 + 64 * 4], &[18, 3, 4, 5]);
        assert_eq!(message.last(), Some(&0xF7));
        assert!(rgb_message(&frame[..63]).is_err());
        frame[2][0] = 64;
        assert!(rgb_message(&frame).is_err());
    }

    #[test]
    fn decodes_top_row_buttons_only() {
        assert_eq!(decode_top(&[0xB0, 104, 127]), Some(TopEvent { index: 0, pressed: true }));
        assert_eq!(decode_top(&[0xB0, 111, 0]), Some(TopEvent { index: 7, pressed: false }));
        for bytes in [&[0xB0, 103, 127][..], &[0xB0, 112, 127], &[0x90, 11, 127], &[0xB0, 104]] {
            assert_eq!(decode_top(bytes), None);
        }
    }

    #[test]
    fn encodes_grid_side_and_top_leds_and_rejects_bad_input() {
        let leds = [(11, [1, 2, 3]), (89, [4, 5, 6]), (104, [63, 0, 0]), (111, [0, 63, 0])];
        let message = leds_message(&leds).unwrap();
        assert_eq!(&message[..7], &[0xF0, 0, 0x20, 0x29, 2, 0x18, 0x0B]);
        assert_eq!(&message[7..11], &[11, 1, 2, 3]);
        assert_eq!(&message[15..19], &[104, 63, 0, 0]);
        assert_eq!(message.last(), Some(&0xF7));
        let all: Vec<(u8, [u8; 3])> = (1..=8)
            .flat_map(|row| (1..=9).map(move |col| (row * 10 + col, [0, 0, 0])))
            .chain((104..=111).map(|led| (led, [0, 0, 0])))
            .collect();
        assert_eq!(all.len(), 80);
        assert!(leds_message(&all).is_ok());
        assert!(leds_message(&[]).is_err());
        assert!(leds_message(&[(10, [0, 0, 0])]).is_err());
        assert!(leds_message(&[(90, [0, 0, 0])]).is_err());
        assert!(leds_message(&[(100, [0, 0, 0])]).is_err());
        assert!(leds_message(&[(112, [0, 0, 0])]).is_err());
        assert!(leds_message(&[(11, [64, 0, 0])]).is_err());
        let mut too_many = all;
        too_many.push((11, [0, 0, 0]));
        assert!(leds_message(&too_many).is_err());
    }

    #[test]
    fn recognises_bootloader_pid_without_matching_other_usb_ids() {
        assert!(is_bootloader_id("USB\\VID_1235&PID_1069\\1234"));
        assert!(!is_bootloader_id("USB\\VID_1235&PID_1068\\1234"));
        assert!(!is_bootloader_id("USB\\VID_9999&PID_1069\\1234"));
    }
}
