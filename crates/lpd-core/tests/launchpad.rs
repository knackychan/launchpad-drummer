use lpd_core::devices::launchpad::*;

#[test]
fn decodes_messages_captured_from_hardware() {
    assert_eq!(
        decode(&[0x90, 0x0B, 0x7F]),
        Some(LpEvent::PadDown { x: 0, y: 0 })
    );
    assert_eq!(
        decode(&[0x90, 0x0B, 0x00]),
        Some(LpEvent::PadUp { x: 0, y: 0 })
    );
    assert_eq!(
        decode(&[0x90, 0x58, 0x7F]),
        Some(LpEvent::PadDown { x: 7, y: 7 })
    );
    assert_eq!(
        decode(&[0x90, 0x2C, 0x7F]),
        Some(LpEvent::PadDown { x: 3, y: 3 })
    );
    assert_eq!(decode(&[0x90, 0x59, 0x7F]), Some(LpEvent::SideDown(0)));
    assert_eq!(decode(&[0x90, 19, 0x7F]), Some(LpEvent::SideDown(7)));
    assert_eq!(decode(&[0x90, 0x59, 0x00]), Some(LpEvent::SideUp(0)));
    assert_eq!(decode(&[0xB0, 0x6C, 0x7F]), Some(LpEvent::TopDown(4)));
    assert_eq!(decode(&[0xB0, 0x6C, 0x00]), Some(LpEvent::TopUp(4)));
}

#[test]
fn note_off_is_a_release() {
    assert_eq!(
        decode(&[0x80, 0x2C, 0x40]),
        Some(LpEvent::PadUp { x: 3, y: 3 })
    );
}

#[test]
fn ignores_unknown_messages() {
    assert_eq!(decode(&[0x91, 11, 127]), None);
    assert_eq!(decode(&[0x90, 10, 127]), None);
    assert_eq!(decode(&[0x90, 99, 127]), None);
    assert_eq!(decode(&[0xB0, 1, 127]), None);
    assert_eq!(decode(&[0x90, 11]), None);
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
            assert_eq!(
                decode(&[0x90, grid_led(x, y), 127]),
                Some(LpEvent::PadDown { x, y })
            );
        }
    }
    for i in 0..8 {
        assert_eq!(
            decode(&[0x90, side_led(i), 127]),
            Some(LpEvent::SideDown(i))
        );
        assert_eq!(decode(&[0xB0, top_led(i), 127]), Some(LpEvent::TopDown(i)));
    }
}

#[test]
fn rgb_message_bytes() {
    let msgs = rgb_messages(&[(11, [63, 0, 0])]);
    assert_eq!(
        msgs,
        vec![vec![
            0xF0, 0x00, 0x20, 0x29, 0x02, 0x18, 0x0B, 11, 63, 0, 0, 0xF7
        ]]
    );
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
    assert_eq!(
        set_all(0),
        vec![0xF0, 0x00, 0x20, 0x29, 0x02, 0x18, 0x0E, 0x00, 0xF7]
    );
    assert_eq!(
        SELECT_SESSION_LAYOUT,
        [0xF0, 0x00, 0x20, 0x29, 0x02, 0x18, 0x22, 0x00, 0xF7]
    );
    assert_eq!(DEVICE_INQUIRY, [0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7]);
}

#[test]
fn matches_only_launchpad_mk2_ports() {
    assert!(is_launchpad_port("Launchpad MK2"));
    assert!(is_launchpad_port("Launchpad MK2 16"));
    assert!(is_launchpad_port("2- Launchpad MK2 16"));
    assert!(is_launchpad_port("  Launchpad MK2  "));
    assert!(!is_launchpad_port("Other Launchpad MK2"));
    assert!(!is_launchpad_port("Launchpad MK2X"));
    assert!(!is_launchpad_port("Launch Control XL"));
}

#[test]
fn matches_port_names_with_device_id_suffix() {
    assert!(is_launchpad_port("Launchpad MK2"));
    assert!(is_launchpad_port("Launchpad MK2 16"));
    assert!(is_launchpad_port("2- Launchpad MK2"));
    assert!(!is_launchpad_port("Launch Control XL"));
}
