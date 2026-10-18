//! Launchpad MK2 protocol (Session layout). Values verified on hardware 2026-10-17,
//! see docs/superpowers/specs/2026-10-17-drum-sampler-design.md §2.2.

/// Port names are "Launchpad MK2" or "Launchpad MK2 <device id>"; Windows may add a "2- " prefix.
pub fn is_launchpad_port(name: &str) -> bool {
    let name = name.trim();
    let name = match name.split_once("- ") {
        Some((prefix, rest)) if prefix.parse::<u32>().is_ok() => rest,
        _ => name,
    };
    name == "Launchpad MK2" || name.starts_with("Launchpad MK2 ")
}

pub const DEVICE_INQUIRY: [u8; 6] = [0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7];
pub const SELECT_SESSION_LAYOUT: [u8; 9] = [0xF0, 0x00, 0x20, 0x29, 0x02, 0x18, 0x22, 0x00, 0xF7];
const SYSEX_HEADER: [u8; 6] = [0xF0, 0x00, 0x20, 0x29, 0x02, 0x18];
/// Max LEDs in one RGB SysEx message (Novation MK2 programmer's reference).
pub const MAX_LEDS_PER_MESSAGE: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LpEvent {
    /// x, y in 0..8; (0, 0) is bottom-left.
    PadDown {
        x: u8,
        y: u8,
    },
    PadUp {
        x: u8,
        y: u8,
    },
    /// Right-side round buttons, 0 = top (Volume) .. 7 = bottom (Record Arm).
    SideDown(u8),
    SideUp(u8),
    /// Top-row round buttons, 0 = ▲ .. 7 = Mixer.
    TopDown(u8),
    TopUp(u8),
}

pub fn decode(msg: &[u8]) -> Option<LpEvent> {
    let &[status, num, val] = msg else {
        return None;
    };
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
                    Some(if down {
                        LpEvent::PadDown { x, y }
                    } else {
                        LpEvent::PadUp { x, y }
                    })
                }
                9 => {
                    let i = 8 - row;
                    Some(if down {
                        LpEvent::SideDown(i)
                    } else {
                        LpEvent::SideUp(i)
                    })
                }
                _ => None,
            }
        }
        0xB0 if (104..=111).contains(&num) => {
            let i = num - 104;
            Some(if val > 0 {
                LpEvent::TopDown(i)
            } else {
                LpEvent::TopUp(i)
            })
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
