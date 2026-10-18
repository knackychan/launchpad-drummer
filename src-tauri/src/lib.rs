pub mod audio;
pub mod commands;
mod launchpad;
pub mod lights;
pub mod router;
pub mod state;
pub mod store;

use lpd_core::kit::Set;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let default_set = Set::new();
    let (stream, commands, events, status) = match audio::start(false, None) {
        Ok(sys) => (Some(sys.stream), sys.commands, sys.events, sys.status),
        Err(e) => {
            eprintln!("Audio device init warning: {e}");
            let (tx, _) = rtrb::RingBuffer::new(1024);
            let (_, rx) = rtrb::RingBuffer::new(1024);
            (
                None,
                tx,
                rx,
                audio::AudioStatus {
                    state: "error".into(),
                    device_name: "none".into(),
                    sample_rate: 48_000,
                    buffer_size: None,
                    channels: 2,
                },
            )
        }
    };
    let app_state = AppState::new(default_set, commands, events);
    app_state.set_audio_status(status);
    let _audio_stream = stream;

    tauri::Builder::default()
        .manage(app_state.clone())
        .setup({
            let app_state = app_state.clone();
            move |app| {
                launchpad::start(app.handle().clone(), app_state);
                Ok(())
            }
        })
        .invoke_handler(tauri::generate_handler![
            launchpad::send_led_frame,
            commands::get_audio_status,
            commands::trigger_pad,
            commands::release_pad,
            commands::select_bank,
            commands::get_current_bank,
            commands::get_current_set,
            commands::load_set_dir,
            commands::new_set_dir,
            commands::save_set_dir,
            commands::update_pad,
            commands::remove_pad,
            commands::update_group,
            commands::update_master_db,
            commands::update_bank_lights,
            commands::rename_bank,
            commands::stop_all,
            commands::import_sample,
            commands::update_bank_humanize,
            commands::sample_peaks
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Launchpad Drummer");
}
