//! Launchpad MK2 MIDI bridge.
// shortcut: protocol lives here until Plan 1A's lpd-core is built; then use its codec.

use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use serde::Serialize;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

const SESSION_LAYOUT: [u8; 9] = [0xF0, 0, 0x20, 0x29, 2, 0x18, 0x22, 0, 0xF7];
const ALL_OFF: [u8; 9] = [0xF0, 0, 0x20, 0x29, 2, 0x18, 0x0E, 0, 0xF7];
const RECHECK: Duration = Duration::from_secs(2);

pub struct FrameSender(Sender<Vec<u8>>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeviceStatus {
    Connected,
    Disconnected,
    Busy,
    Bootloader,
}

impl DeviceStatus {
    fn payload(self) -> StatusEvent {
        match self {
            Self::Connected => StatusEvent { state: "connected", message: "Launchpad MK2 connected" },
            Self::Disconnected => StatusEvent { state: "disconnected", message: "Launchpad not connected — checking every 2 s" },
            Self::Busy => StatusEvent { state: "busy", message: "Launchpad port busy — close Components, DAWs or another copy of this app" },
            Self::Bootloader => StatusEvent { state: "bootloader", message: "Launchpad in bootloader — run Novation's MK2 updater, then replug" },
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
    output: MidiOutputConnection,
}

pub fn start(app: AppHandle) {
    let (tx, rx) = mpsc::channel();
    app.manage(FrameSender(tx));
    std::thread::spawn(move || run(app, rx));
}

/// `frame` has 64 RGB triplets in visual order (top-left to bottom-right), channels 0..=63.
#[tauri::command]
pub fn send_led_frame(frame: Vec<[u8; 3]>, sender: State<'_, FrameSender>) -> Result<(), String> {
    let message = rgb_message(&frame)?;
    sender.0.send(message).map_err(|_| "Launchpad MIDI worker stopped".to_string())
}

fn is_launchpad_port(name: &str) -> bool {
    let name = name.trim();
    let name = match name.split_once("- ") {
        Some((prefix, rest)) if prefix.parse::<u32>().is_ok() => rest,
        _ => name,
    };
    name == "Launchpad MK2" || name.starts_with("Launchpad MK2 ")
}

fn decode_pad(msg: &[u8]) -> Option<PadEvent> {
    let &[status, note, velocity] = msg else { return None };
    let row = note / 10;
    let column = note % 10;
    if !(1..=8).contains(&row) || !(1..=9).contains(&column) {
        return None;
    }
    match status {
        0x90 => Some(PadEvent { note, pressed: velocity > 0 }),
        0x80 => Some(PadEvent { note, pressed: false }),
        _ => None,
    }
}

fn rgb_message(frame: &[[u8; 3]]) -> Result<Vec<u8>, String> {
    if frame.len() != 64 || frame.iter().flatten().any(|value| *value > 63) {
        return Err("LED frame requires 64 pads with RGB channels from 0 to 63".to_string());
    }
    let mut message = Vec::with_capacity(8 + 64 * 4);
    message.extend_from_slice(&[0xF0, 0, 0x20, 0x29, 2, 0x18, 0x0B]);
    for (index, rgb) in frame.iter().enumerate() {
        let row = 8 - index / 8;
        let column = index % 8 + 1;
        message.extend_from_slice(&[(row * 10 + column) as u8, rgb[0], rgb[1], rgb[2]]);
    }
    message.push(0xF7);
    Ok(message)
}

fn run(app: AppHandle, rx: Receiver<Vec<u8>>) {
    let mut connection: Option<Connection> = None;
    let mut status = None;
    let mut latest_frame = None;
    let mut next_check = Instant::now();
    loop {
        if Instant::now() >= next_check {
            check_connection(&app, &mut connection, &mut status, latest_frame.as_deref());
            next_check = Instant::now() + RECHECK;
        }
        match rx.recv_timeout(next_check.saturating_duration_since(Instant::now())) {
            Ok(frame) => {
                latest_frame = Some(frame);
                if let (Some(device), Some(frame)) = (connection.as_mut(), latest_frame.as_deref()) {
                    if let Err(error) = device.output.send(frame) {
                        eprintln!("Launchpad LED send failed: {error}");
                        connection = None;
                        report_status(&app, &mut status, DeviceStatus::Disconnected);
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {},
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn check_connection(
    app: &AppHandle,
    connection: &mut Option<Connection>,
    status: &mut Option<DeviceStatus>,
    latest_frame: Option<&[u8]>,
) {
    match bootloader_present() {
        Ok(true) => {
            *connection = None;
            report_status(app, status, DeviceStatus::Bootloader);
            return;
        }
        Err(error) => {
            if *status != Some(DeviceStatus::Disconnected) {
                eprintln!("Launchpad bootloader detection failed (CfgMgr32 code {error})");
            }
        },
        Ok(false) => {},
    }

    let input = match MidiInput::new("launchpad-drummer-in") {
        Ok(input) => input,
        Err(error) => {
            *connection = None;
            if *status != Some(DeviceStatus::Disconnected) {
                eprintln!("Launchpad input scan failed: {error}");
            }
            report_status(app, status, DeviceStatus::Disconnected);
            return;
        }
    };
    let output = match MidiOutput::new("launchpad-drummer-out") {
        Ok(output) => output,
        Err(error) => {
            *connection = None;
            if *status != Some(DeviceStatus::Disconnected) {
                eprintln!("Launchpad output scan failed: {error}");
            }
            report_status(app, status, DeviceStatus::Disconnected);
            return;
        }
    };
    let input_port = input.ports().into_iter().find_map(|port| {
        input.port_name(&port).ok().filter(|name| is_launchpad_port(name)).map(|name| (port, name))
    });
    let output_port = output.ports().into_iter().find_map(|port| {
        output.port_name(&port).ok().filter(|name| is_launchpad_port(name)).map(|name| (port, name))
    });
    let (Some((input_port, input_name)), Some((output_port, output_name))) = (input_port, output_port) else {
        *connection = None;
        report_status(app, status, DeviceStatus::Disconnected);
        return;
    };
    if connection.as_ref().is_some_and(|device| device.input_name == input_name && device.output_name == output_name) {
        report_status(app, status, DeviceStatus::Connected);
        return;
    }
    *connection = None;
    let receiver = app.clone();
    let input_connection = match input.connect(&input_port, "launchpad-drummer-in", move |_, msg, _| {
        if let Some(pad) = decode_pad(msg) {
            if let Err(error) = receiver.emit("launchpad-pad", pad) {
                eprintln!("Launchpad pad event failed: {error}");
            }
        }
    }, ()) {
        Ok(input) => input,
        Err(error) => {
            if *status != Some(DeviceStatus::Busy) {
                eprintln!("Launchpad input connection failed: {error}");
            }
            report_status(app, status, DeviceStatus::Busy);
            return;
        }
    };
    let mut output_connection = match output.connect(&output_port, "launchpad-drummer-out") {
        Ok(output) => output,
        Err(error) => {
            if *status != Some(DeviceStatus::Busy) {
                eprintln!("Launchpad output connection failed: {error}");
            }
            report_status(app, status, DeviceStatus::Busy);
            return;
        }
    };
    if let Err(error) = output_connection.send(&SESSION_LAYOUT)
        .and_then(|_| output_connection.send(&ALL_OFF))
        .and_then(|_| latest_frame.map_or(Ok(()), |frame| output_connection.send(frame)))
    {
        eprintln!("Launchpad initialization failed: {error}");
        report_status(app, status, DeviceStatus::Disconnected);
        return;
    }
    *connection = Some(Connection { input_name, output_name, _input: input_connection, output: output_connection });
    report_status(app, status, DeviceStatus::Connected);
}

fn report_status(app: &AppHandle, previous: &mut Option<DeviceStatus>, current: DeviceStatus) {
    if *previous != Some(current) {
        eprintln!("Launchpad: {}", current.payload().message);
        *previous = Some(current);
    }
    // Repeat the current state quietly so a UI listener mounted after startup receives it.
    if let Err(error) = app.emit("launchpad-status", current.payload()) {
        eprintln!("Launchpad status event failed: {error}");
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
        fn CM_Get_Device_ID_ListW(filter: *const u16, buffer: *mut u16, len: u32, flags: u32) -> u32;
    }
    const FLAGS: u32 = 0x0000_0101; // present devices from the USB enumerator
    let filter: Vec<u16> = "USB\0".encode_utf16().collect();
    let mut len = 0;
    let result = unsafe { CM_Get_Device_ID_List_SizeW(&mut len, filter.as_ptr(), FLAGS) };
    if result != 0 { return Err(result); }
    if len == 0 { return Ok(false); }
    let mut ids = vec![0u16; len as usize];
    let result = unsafe { CM_Get_Device_ID_ListW(filter.as_ptr(), ids.as_mut_ptr(), len, FLAGS) };
    if result != 0 { return Err(result); }
    Ok(ids.split(|unit| *unit == 0)
        .filter(|id| !id.is_empty())
        .any(|id| is_bootloader_id(&String::from_utf16_lossy(id))))
}

#[cfg(not(windows))]
fn bootloader_present() -> Result<bool, u32> { Ok(false) }


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_only_launchpad_mk2_ports() {
        assert!(is_launchpad_port("Launchpad MK2"));
        assert!(is_launchpad_port("Launchpad MK2 16"));
        assert!(is_launchpad_port("2- Launchpad MK2 16"));
        assert!(!is_launchpad_port("Other Launchpad MK2"));
        assert!(!is_launchpad_port("Launchpad MK2X"));
        assert!(!is_launchpad_port("Launch Control XL"));
    }

    #[test]
    fn decodes_grid_and_side_button_press_and_release_only() {
        assert_eq!(decode_pad(&[0x90, 11, 127]), Some(PadEvent { note: 11, pressed: true }));
        assert_eq!(decode_pad(&[0x90, 88, 0]), Some(PadEvent { note: 88, pressed: false }));
        assert_eq!(decode_pad(&[0x80, 44, 64]), Some(PadEvent { note: 44, pressed: false }));
        assert_eq!(decode_pad(&[0x90, 89, 127]), Some(PadEvent { note: 89, pressed: true }));
        assert_eq!(decode_pad(&[0x80, 79, 64]), Some(PadEvent { note: 79, pressed: false }));
        for bytes in [&[0x91, 11, 127][..], &[0x90, 99, 127], &[0xB0, 104, 127], &[0x90, 11]] {
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
    fn recognises_bootloader_pid_without_matching_other_usb_ids() {
        assert!(is_bootloader_id("USB\\VID_1235&PID_1069\\1234"));
        assert!(!is_bootloader_id("USB\\VID_1235&PID_1068\\1234"));
        assert!(!is_bootloader_id("USB\\VID_9999&PID_1069\\1234"));
    }
}
