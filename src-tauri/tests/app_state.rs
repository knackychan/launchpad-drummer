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
