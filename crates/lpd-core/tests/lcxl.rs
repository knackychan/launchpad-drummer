use lpd_core::devices::lcxl::*;

#[test]
fn recognises_the_port_name() {
    assert!(is_lcxl_port("Launch Control XL"));
    assert!(is_lcxl_port("2- Launch Control XL"));
    assert!(is_lcxl_port("  MIDIIN2 (Launch Control XL) "));
    assert!(!is_lcxl_port("launch control xl"));
    assert!(!is_lcxl_port("Launchpad MK2"));
}

#[test]
fn decodes_faders_on_any_channel() {
    assert_eq!(
        decode(&[0xB0, 0x4D, 0x40]),
        Some(LcxlEvent::Fader { ch: 0, value: 64 })
    );
    assert_eq!(
        decode(&[0xB8, 0x54, 0x7F]),
        Some(LcxlEvent::Fader { ch: 7, value: 127 })
    );
}

#[test]
fn decodes_the_three_knob_rows() {
    assert_eq!(
        decode(&[0xB0, 0x0D, 0x00]),
        Some(LcxlEvent::Knob {
            row: 0,
            ch: 0,
            value: 0
        })
    );
    assert_eq!(
        decode(&[0xB0, 0x24, 0x7F]),
        Some(LcxlEvent::Knob {
            row: 1,
            ch: 7,
            value: 127
        })
    );
    assert_eq!(
        decode(&[0xB0, 0x38, 0x01]),
        Some(LcxlEvent::Knob {
            row: 2,
            ch: 7,
            value: 1
        })
    );
}

#[test]
fn decodes_the_two_button_rows() {
    assert_eq!(
        decode(&[0x90, 0x29, 0x7F]),
        Some(LcxlEvent::Button {
            row: 0,
            ch: 0,
            on: true
        })
    );
    assert_eq!(
        decode(&[0x90, 0x3C, 0x7F]),
        Some(LcxlEvent::Button {
            row: 0,
            ch: 7,
            on: true
        })
    );
    assert_eq!(
        decode(&[0x90, 0x59, 0x00]),
        Some(LcxlEvent::Button {
            row: 1,
            ch: 4,
            on: false
        })
    );
    assert_eq!(
        decode(&[0x80, 0x4C, 0x00]),
        Some(LcxlEvent::Button {
            row: 1,
            ch: 3,
            on: false
        })
    );
}

#[test]
fn every_button_note_maps_once() {
    let mut seen = std::collections::HashSet::new();
    for note in 0u8..128 {
        if let Some(LcxlEvent::Button { row, ch, .. }) = decode(&[0x90, note, 0x7F]) {
            assert!(seen.insert((row, ch)), "note {note} maps a second time");
        }
    }
    assert_eq!(seen.len(), 16);
}

#[test]
fn ignores_everything_else() {
    assert_eq!(decode(&[0xB0, 0x15, 0x10]), None); // CC 21 is unmapped
    assert_eq!(decode(&[0x90, 0x28, 0x7F]), None); // note 40
    assert_eq!(decode(&[0xE0, 0x00, 0x40]), None);
    assert_eq!(decode(&[0xB0, 0x4D]), None);
    assert_eq!(decode(&[0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7]), None);
    assert_eq!(decode(&[]), None);
}

#[test]
fn led_messages_match_the_protocol() {
    assert_eq!(
        led_message(0, 24, 0x3F),
        [0xF0, 0x00, 0x20, 0x29, 0x02, 0x11, 0x78, 0x00, 0x18, 0x3F, 0xF7]
    );
    assert_eq!(led_message(USER_TEMPLATE, 33, 0x3C)[8], 33);
    assert_eq!(led_index(0, 0), 24);
    assert_eq!(led_index(0, 7), 31);
    assert_eq!(led_index(1, 0), 32);
    assert_eq!(led_index(1, 7), 39);
    assert_eq!(led_value(LedColor::Off), 0x0C);
    assert_eq!(led_value(LedColor::Red), 0x0F);
    assert_eq!(led_value(LedColor::Green), 0x3C);
    assert_eq!(led_value(LedColor::Amber), 0x3F);
}

#[test]
fn fader_law() {
    assert_eq!(fader_db(0), -60.0);
    assert_eq!(fader_db(127), 6.0);
    // 0 dB sits at three quarters of the travel.
    assert!(fader_db(95).abs() < 0.2, "{}", fader_db(95));
    // Below 0 dB the law is 40·log10(p / 0.75), floored at −60.
    let p = 32.0_f32 / 127.0;
    assert!((fader_db(32) - 40.0 * (p / 0.75).log10()).abs() < 1e-4);
    assert!(fader_db(1) >= -60.0 && fader_db(1) < -30.0);
    // Monotonic.
    for v in 1..=127u8 {
        assert!(fader_db(v) >= fader_db(v - 1));
    }
}

#[test]
fn knob_laws() {
    assert_eq!(knob_pan(64), 0.0);
    assert_eq!(knob_pan(127), 1.0);
    assert_eq!(knob_pan(0), -1.0);
    assert_eq!(knob_pitch(64), 0.0);
    assert_eq!(knob_pitch(0), -12.0);
    assert_eq!(knob_pitch(127), 12.0);
    assert_eq!(knob_lowpass(0), 200.0);
    assert_eq!(knob_lowpass(127), 20000.0);
}
