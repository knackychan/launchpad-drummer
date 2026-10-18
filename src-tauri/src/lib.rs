pub mod audio;
pub mod commands;
pub mod config;
mod launchpad;
pub mod lights;
pub mod logfile;
pub mod router;
pub mod saver;
pub mod startup;
pub mod state;
pub mod store;

use lpd_core::kit::Set;
use state::AppState;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// Autosave and audio health, off every real-time path. Emits `save-status` and `audio-status`.
fn housekeeping(app: AppHandle, state: AppState, health: Arc<audio::StreamHealth>) {
    loop {
        std::thread::sleep(Duration::from_millis(100));
        state.autosave_tick(Instant::now());
        // Save failures are logged by the save itself.
        if let Some(status) = state.take_save_change() {
            let _ = app.emit("save-status", status);
        }
        if let Some(status) = audio::poll_stream_health(&health, &state) {
            logfile::log("audio stream error: output stopped");
            let _ = app.emit("audio-status", status);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let default_set = Set::new();
    let mut audio_warning = None;
    let (stream, commands, events, status, health) = match audio::start(false, None) {
        Ok(sys) => (
            Some(sys.stream),
            sys.commands,
            sys.events,
            sys.status,
            sys.health,
        ),
        Err(e) => {
            // The log file opens in `setup`; the warning is written there.
            audio_warning = Some(format!("Audio device init warning: {e}"));
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
                Arc::new(audio::StreamHealth::default()),
            )
        }
    };
    let app_state = AppState::new(default_set, commands, events);
    app_state.set_audio_status(status);
    let _audio_stream = stream;

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(app_state.clone())
        .setup({
            let app_state = app_state.clone();
            move |app| {
                if let Ok(dir) = app.path().app_log_dir() {
                    logfile::init(dir);
                }
                logfile::log("Launchpad Drummer started");
                match audio_warning {
                    Some(warning) => logfile::log(warning),
                    None => logfile::log(format!("Audio: {:?}", app_state.get_audio_status())),
                }
                match app.path().app_config_dir() {
                    Ok(dir) => {
                        app_state.restore(dir.join("config.json"));
                    }
                    Err(e) => logfile::log(format!("No app config folder: {e}")),
                }
                launchpad::start(app.handle().clone(), app_state.clone());
                let handle = app.handle().clone();
                std::thread::spawn(move || housekeeping(handle, app_state, health));
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
            commands::get_startup,
            commands::new_set,
            commands::open_set,
            commands::save_now,
            commands::get_save_status,
            commands::missing_samples,
            commands::get_config,
            commands::set_library_dir,
            commands::set_soundfont,
            commands::pick_folder,
            commands::pick_file,
            commands::reveal_path,
            commands::open_log_folder,
            commands::default_sets_dir,
            commands::assign_sample,
            commands::set_input_mode,
            commands::refresh_leds,
            commands::update_pad,
            commands::clear_pad,
            commands::move_pad,
            commands::drop_files,
            commands::locate_sample,
            commands::update_group,
            commands::update_master_db,
            commands::update_bank_lights,
            commands::rename_bank,
            commands::stop_all,
            commands::import_sample,
            commands::update_bank_humanize,
            commands::sample_peaks,
            commands::undo,
            commands::redo,
            commands::history_state
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Launchpad Drummer");
}
