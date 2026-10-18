use launchpad_drummer_lib::state::AppState;
use lpd_core::audio::Command;
use lpd_core::kit::{Pad, Set};

#[test]
fn app_state_triggers_pad_and_selects_bank() {
    let mut set = Set::new();
    set.banks[0].set_pad(Pad::new(0, 0, "samples/kick.wav", [63, 0, 0]));
    set.banks[1].set_pad(Pad::new(0, 0, "samples/snare.wav", [0, 63, 0]));
    let (tx_cmd, mut rx_cmd) = rtrb::RingBuffer::new(64);
    let (_tx_evt, rx_evt) = rtrb::RingBuffer::new(64);

    let state = AppState::new(set, tx_cmd, rx_evt);
    while rx_cmd.pop().is_ok() {}

    // Trigger pad (0,0) on Bank 0
    state.trigger_pad(0, 0);
    // Since sample wasn't loaded in empty sample map, empty pad won't trigger audio command
    assert!(rx_cmd.pop().is_err());

    // Switch bank to 1
    state.select_bank(1);
    assert_eq!(state.current_bank(), 1);

    // Stop all
    state.stop_all();
    let cmd = rx_cmd.pop().expect("command popped");
    assert!(matches!(cmd, Command::StopAll));
}

#[test]
fn humanize_is_clamped_and_reported_in_current_set() {
    let (tx_cmd, _rx_cmd) = rtrb::RingBuffer::new(64);
    let (_tx_evt, rx_evt) = rtrb::RingBuffer::new(64);
    let state = AppState::new(Set::new(), tx_cmd, rx_evt);

    state.update_bank_humanize(2, 3.5);
    state.update_bank_humanize(3, 40.0);

    let set = state.get_current_set();
    assert_eq!(set.banks[2].humanize_db, 3.5);
    assert_eq!(set.banks[3].humanize_db, 6.0);
}

#[test]
fn sample_peaks_reduce_loaded_sample_to_buckets() {
    let (tx_cmd, _rx_cmd) = rtrb::RingBuffer::new(64);
    let (_tx_evt, rx_evt) = rtrb::RingBuffer::new(64);
    let state = AppState::new(Set::new(), tx_cmd, rx_evt);
    assert!(state.sample_peaks("samples/none.wav", 32).is_none());

    // 64 stereo frames at 1 kHz; frame i has amplitude i / 64 on the right channel.
    let data = (0..64).flat_map(|i| [0.0, -(i as f32) / 64.0]).collect();
    let sample = std::sync::Arc::new(lpd_core::audio::Sample { rate: 1000, data });
    state
        .kit_manager
        .lock()
        .unwrap()
        .samples
        .insert("samples/ramp.wav".into(), sample);

    let peaks = state.sample_peaks("samples/ramp.wav", 16).expect("loaded");
    assert_eq!(peaks.peaks.len(), 16);
    assert_eq!(peaks.rate, 1000);
    assert_eq!(peaks.length_ms, 64.0);
    assert_eq!(peaks.peaks[0], 3.0 / 64.0);
    assert_eq!(peaks.peaks[15], 63.0 / 64.0);
}
