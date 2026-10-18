use launchpad_drummer_lib::state::AppState;
use lpd_core::audio::Command;
use lpd_core::kit::{Group, LightModes, Pad, PadMode, Set};
use tempfile::tempdir;

#[test]
fn test_app_state_integration_and_commands() {
    let (tx_cmd, mut rx_cmd) = rtrb::RingBuffer::new(64);
    let (_tx_evt, rx_evt) = rtrb::RingBuffer::new(64);

    let state = AppState::new(Set::new(), tx_cmd, rx_evt);
    while rx_cmd.pop().is_ok() {}

    // Verify initial state
    assert_eq!(state.current_bank(), 0);
    assert_eq!(state.get_audio_status().state, "stopped");
    let set = state.get_current_set();
    assert_eq!(set.banks.len(), 8);
    assert_eq!(set.mixer.groups.len(), 8);

    // Test bank selection
    assert!(state.select_bank(3));
    assert_eq!(state.current_bank(), 3);
    assert!(!state.select_bank(99));

    // Test updating a pad
    let pad = Pad {
        x: 2,
        y: 2,
        sample: "samples/snare.wav".into(),
        color: [0, 63, 0],
        volume_db: -3.0,
        pitch_semitones: 2.0,
        trim_start_ms: 10.0,
        trim_end_ms: Some(250.0),
        mode: PadMode::OneShot,
        choke_group: Some(1),
        mix_group: 2,
    };
    state.update_pad(3, pad.clone(), None);
    let current_set = state.get_current_set();
    let retrieved_pad = current_set.banks[3].pad(2, 2).expect("pad found");
    assert_eq!(retrieved_pad.sample, "samples/snare.wav");
    assert_eq!(retrieved_pad.color, [0, 63, 0]);

    // Test bank lights and renaming
    state.rename_bank(3, "Hyper Snare".into());
    state.update_bank_lights(
        3,
        LightModes {
            flash: false,
            play_state: true,
            ripple: true,
        },
    );
    let current_set = state.get_current_set();
    assert_eq!(current_set.banks[3].name, "Hyper Snare");
    assert!(current_set.banks[3].lights.ripple);
    assert!(!current_set.banks[3].lights.flash);

    // Test updating a mixer group generates Command::SetGroup
    let group = Group {
        name: "Percussion".into(),
        volume_db: -6.0,
        pan: 0.5,
        pitch_semitones: -1.0,
        lowpass_hz: 5000.0,
        mute: false,
        solo: false,
    };
    state.update_group(2, group.clone());
    let current_set = state.get_current_set();
    assert_eq!(current_set.mixer.groups[2].name, "Percussion");
    let cmd = rx_cmd.pop().expect("audio command sent for group update");
    match cmd {
        Command::SetGroup { group: g, params } => {
            assert_eq!(g, 2);
            assert_eq!(params.lowpass_hz, 5000.0);
        }
        other => panic!("expected SetGroup, got {other:?}"),
    }

    // Test master gain update generates Command::SetMasterGain
    state.update_master_db(-12.0);
    let cmd = rx_cmd.pop().expect("audio command sent for master update");
    assert!(matches!(cmd, Command::SetMasterGain(_)));

    // Test removing a pad
    state.clear_pad(3, 2, 2);
    let current_set = state.get_current_set();
    assert!(current_set.banks[3].pad(2, 2).is_none());

    // Test saving and loading a set dir
    let temp = tempdir().unwrap();
    let set_dir = temp.path().join("test_set");
    let created_set = state.new_set(set_dir.to_str().unwrap()).unwrap();
    assert_eq!(created_set.set.banks.len(), 8);

    // Modify and save
    state.rename_bank(0, "Acoustic".into());
    state.save_set().unwrap();

    // Reload
    let loaded = state.open_set(set_dir.to_str().unwrap()).unwrap();
    assert_eq!(loaded.set.banks[0].name, "Acoustic");
    assert_eq!(loaded.name, "test_set");
    assert!(!loaded.from_backup);
    assert!(
        loaded.backup_saved_at.is_some(),
        "the save rotated a backup"
    );
}
