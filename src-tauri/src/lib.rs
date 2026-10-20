pub mod audio;
pub mod commands;
pub mod config;
pub mod devices;
mod launchpad;
pub mod lcxl;
pub mod lights;
pub mod logfile;
pub mod router;
pub mod saver;
pub mod sequencer;
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
    logfile::mark_process_start();
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
            logfile::milestone(&format!("audio {}", status.state));
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
                logfile::milestone("setup entered");
                // The UI hears about audio, keys and set loading through these hooks.
                let _ = app_handle.set(app.handle().clone());
                let keys_app = app.handle().clone();
                app_state.set_keys_notifier(Arc::new(move |status| {
                    let _ = keys_app.emit("keys-status", status.clone());
                }));
                let load_app = app.handle().clone();
                app_state.set_load_notifier(Arc::new(move |status| {
                    let _ = load_app.emit("load-status", status.clone());
                }));
                // Preferences first (quick), then the restore's ticket, before anything slow starts.
                match app.path().app_config_dir() {
                    Ok(dir) => {
                        app_state.init_config(dir.join("config.json"));
                    }
                    Err(e) => logfile::log(format!("No app config folder: {e}")),
                }
                logfile::milestone("configuration ready");
                let restore_ticket = app_state.begin_restore();
                // The saved audio prefs decide the device. The stream starts in the background
                // (opening a device can take seconds) and independently of the set; the UI hears
                // about it through `audio-status`, and a failure shows as "error" in SETUP.
                audio_controller.start_detached(app_state.get_config().audio);
                // Decoding the last set's samples can take long: never on the setup thread.
                let restore_state = app_state.clone();
                let spawned = std::thread::Builder::new()
                    .name("restore".into())
                    .spawn(move || {
                        logfile::milestone("restore started");
                        restore_state.restore_guarded(restore_ticket);
                        logfile::milestone("restore finished");
                    });
                if let Err(e) = spawned {
                    app_state.fail_load(restore_ticket, &format!("Couldn't start loading: {e}"));
                }
                launchpad::start(app.handle().clone(), app_state.clone());
                devices::start(app.handle().clone(), app_state.clone());
                lcxl::start(app.handle().clone(), app_state.clone());
                sequencer::start(app.handle().clone(), app_state.clone());
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
            launchpad::send_led_leds,
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
            commands::get_load_status,
            commands::retry_startup,
            commands::new_set,
            commands::open_default_set,
            commands::open_set,
            commands::save_now,
            commands::get_save_status,
            commands::missing_samples,
            commands::get_config,
            commands::set_library_dir,
            commands::set_soundfont,
            commands::midi_devices,
            commands::set_midi_role,
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
            commands::history_state,
            commands::arm_learn,
            commands::cancel_learn,
            commands::unbind,
            commands::seq_play,
            commands::seq_rec,
            commands::seq_update,
            commands::seq_add_lane,
            commands::seq_audition_step,
            commands::keys_status,
            commands::keys_note,
            commands::keys_pitch,
            commands::keys_mod,
            commands::set_keys,
            commands::trigger_pad_in,
            commands::release_pad_in
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Launchpad Drummer");
}
