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
pub mod telemetry;

use lpd_core::kit::Set;
use state::AppState;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// Autosave, off every real-time path. Emits `save-status`. (A lost audio stream is noticed by the
/// telemetry thread.)
fn housekeeping(app: AppHandle, state: AppState) {
    loop {
        std::thread::sleep(Duration::from_millis(100));
        state.autosave_tick(Instant::now());
        // Save failures are logged by the save itself.
        if let Some(status) = state.take_save_change() {
            let _ = app.emit("save-status", status);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Placeholder queues: the audio owner thread installs the real ones once the stream is up.
    let (commands, _) = rtrb::RingBuffer::new(1024);
    let (_, events) = rtrb::RingBuffer::new(1024);
    let app_state = AppState::new(Set::new(), commands, events);
    // The app handle exists only once Tauri is up; every finished stream start is announced to the UI.
    let app_handle: Arc<OnceLock<AppHandle>> = Arc::default();
    let announce_to = app_handle.clone();
    let audio_controller = audio::AudioController::spawn(
        app_state.clone(),
        Box::new(move |status| {
            if let Some(app) = announce_to.get() {
                let _ = app.emit("audio-status", status);
            }
        }),
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(app_state.clone())
        .manage(audio_controller.clone())
        .setup({
            let app_state = app_state.clone();
            move |app| {
                if let Ok(dir) = app.path().app_log_dir() {
                    logfile::init(dir);
                }
                logfile::log("Launchpad Drummer started");
                match app.path().app_config_dir() {
                    Ok(dir) => {
                        app_state.restore(dir.join("config.json"));
                    }
                    Err(e) => logfile::log(format!("No app config folder: {e}")),
                }
                // The saved audio prefs decide the device, so the stream starts after the config
                // is read. It starts in the background (opening a device can take seconds); the
                // UI hears about it through `audio-status`, and a failure shows as "error" in SETUP.
                let _ = app_handle.set(app.handle().clone());
                audio_controller.start_detached(app_state.get_config().audio);
                launchpad::start(app.handle().clone(), app_state.clone());
                let handle = app.handle().clone();
                let telemetry_handle = handle.clone();
                let telemetry_state = app_state.clone();
                std::thread::spawn(move || housekeeping(handle, app_state));
                std::thread::spawn(move || telemetry::run(telemetry_handle, telemetry_state));
                Ok(())
            }
        })
        .invoke_handler(tauri::generate_handler![
            launchpad::send_led_frame,
            commands::get_audio_status,
            commands::audio_devices,
            commands::asio_available,
            commands::set_audio,
            commands::audio_reconnect,
            commands::set_telemetry,
            commands::reset_dropouts,
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
            commands::list_dir,
            commands::audition_file,
            commands::stop_audition,
            commands::load_into_pad,
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
