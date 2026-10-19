use crate::audio::{self, AsioInfo, AudioController, AudioDevice, AudioStatus};
use crate::config::{AppConfig, AudioPrefs};
use crate::devices::MidiDevice;
use crate::router::LearnTarget;
use crate::saver::SaveStatus;
use crate::startup::Startup;
use crate::state::{
    AppState, DropReport, HistoryState, InputMode, KeysStatus, MissingPad, OpenResult, SamplePeaks,
};
use lpd_core::instrument::KeysSettings;
use lpd_core::kit::{Group, LightModes, Pad, Set};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

/// Run slow work (disk, decoding) off the command thread and off the UI.
async fn worker<T: Send + 'static>(
    state: &State<'_, AppState>,
    job: impl FnOnce(AppState) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || job(state))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn get_audio_status(state: State<'_, AppState>) -> Result<AudioStatus, String> {
    Ok(state.get_audio_status())
}

/// Output devices of a backend ("wasapi" | "asio") with their buffer sizes. Enumeration can be
/// slow, so it runs off the command thread.
#[tauri::command]
pub(crate) async fn audio_devices(backend: String) -> Result<Vec<AudioDevice>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        audio::list_devices(&backend).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn asio_available() -> Result<AsioInfo, String> {
    tauri::async_runtime::spawn_blocking(audio::asio_info)
        .await
        .map_err(|e| e.to_string())
}

/// Save the audio prefs and restart the stream with them.
#[tauri::command]
pub(crate) async fn set_audio(
    prefs: AudioPrefs,
    state: State<'_, AppState>,
    audio: State<'_, AudioController>,
) -> Result<AudioStatus, String> {
    let audio = audio.inner().clone();
    worker(&state, move |s| audio.apply(&s, prefs)).await
}

/// Restart the stream with the saved prefs when it is lost, stopped or failed to start.
#[tauri::command]
pub(crate) async fn audio_reconnect(
    state: State<'_, AppState>,
    audio: State<'_, AudioController>,
) -> Result<AudioStatus, String> {
    let audio = audio.inner().clone();
    worker(&state, move |s| audio.reconnect(&s)).await
}

/// The UI wants (or no longer wants) the `telemetry` feed.
#[tauri::command]
pub(crate) fn set_telemetry(on: bool, state: State<'_, AppState>) {
    state.set_telemetry(on);
}

#[tauri::command]
pub(crate) fn reset_dropouts(state: State<'_, AppState>) {
    state.reset_dropouts();
}

#[tauri::command]
pub(crate) fn trigger_pad(x: u8, y: u8, state: State<'_, AppState>) {
    state.trigger_pad(x, y);
}

#[tauri::command]
pub(crate) fn release_pad(x: u8, y: u8, state: State<'_, AppState>) {
    state.release_pad(x, y);
}

#[tauri::command]
pub(crate) fn select_bank(bank: usize, state: State<'_, AppState>) -> bool {
    state.select_bank(bank)
}

#[tauri::command]
pub(crate) fn get_current_bank(state: State<'_, AppState>) -> usize {
    state.current_bank()
}

#[tauri::command]
pub(crate) fn get_current_set(state: State<'_, AppState>) -> Set {
    state.get_current_set()
}

#[tauri::command]
pub(crate) fn get_startup(state: State<'_, AppState>) -> Startup {
    state.get_startup()
}

#[tauri::command]
pub(crate) async fn new_set(dir: String, state: State<'_, AppState>) -> Result<OpenResult, String> {
    worker(&state, move |s| s.new_set(&dir)).await
}

#[tauri::command]
pub(crate) async fn open_set(
    dir: String,
    state: State<'_, AppState>,
) -> Result<OpenResult, String> {
    worker(&state, move |s| s.open_set(&dir)).await
}

/// Save immediately through the autosave path.
#[tauri::command]
pub(crate) async fn save_now(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let result = worker(&state, |s| s.save_set()).await;
    if let Some(status) = state.take_save_change() {
        let _ = app.emit("save-status", status);
    }
    result
}

#[tauri::command]
pub(crate) fn missing_samples(state: State<'_, AppState>) -> Vec<MissingPad> {
    state.missing_samples()
}

/// Async: the config lock can be held across a config write.
#[tauri::command]
pub(crate) async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    worker(&state, |s| Ok(s.get_config())).await
}

#[tauri::command]
pub(crate) async fn set_library_dir(
    dir: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    worker(&state, move |s| s.set_library_dir(dir)).await
}

#[tauri::command]
pub(crate) fn midi_devices(state: State<'_, AppState>) -> Vec<MidiDevice> {
    state.midi_devices()
}

/// `role`: "keys" | "other". Saved in the app config; the input poller reopens the port.
#[tauri::command]
pub(crate) async fn set_midi_role(
    port: String,
    role: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    if let Some(list) = worker(&state, move |s| s.set_midi_role(&port, &role)).await? {
        let _ = app.emit("midi-devices", list);
    }
    Ok(())
}

/// Load a General MIDI `.sf2` as the keys sound (or `null`: back to the built-in synth).
#[tauri::command]
pub(crate) async fn set_soundfont(
    path: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<KeysStatus, String> {
    let status = worker(&state, move |s| s.set_soundfont(path)).await?;
    let _ = app.emit("keys-status", status.clone());
    Ok(status)
}

#[tauri::command]
pub(crate) fn keys_status(state: State<'_, AppState>) -> KeysStatus {
    state.keys_status()
}

/// A key played on the screen. Also emits `keys-note` so every mirror of the keybed shows it.
#[tauri::command]
pub(crate) fn keys_note(
    note: u8,
    velocity: u8,
    on: bool,
    state: State<'_, AppState>,
    app: AppHandle,
) {
    let event = state.keys_note(note.min(127), velocity.min(127), on);
    let _ = app.emit(event.name(), event.payload());
}

/// The screen's pitch wheel, −1..1 (±2 semitones).
#[tauri::command]
pub(crate) fn keys_pitch(value: f32, state: State<'_, AppState>) {
    state.keys_pitch(value);
}

/// The screen's mod wheel, 0..1.
#[tauri::command]
pub(crate) fn keys_mod(value: f32, state: State<'_, AppState>) {
    state.keys_mod(value);
}

#[tauri::command]
pub(crate) fn set_keys(settings: KeysSettings, state: State<'_, AppState>) -> Set {
    state.set_keys(settings)
}

#[tauri::command]
pub(crate) async fn pick_folder(
    title: String,
    default_dir: Option<String>,
    app: AppHandle,
) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = app.dialog().file().set_title(title);
        if let Some(dir) = default_dir {
            dialog = dialog.set_directory(dir);
        }
        picked(dialog.blocking_pick_folder())
    })
    .await
    .ok()
    .flatten()
}

#[tauri::command]
pub(crate) async fn pick_file(
    title: String,
    extensions: Vec<String>,
    app: AppHandle,
) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let name = if extensions == ["sf2"] {
            "SoundFont"
        } else {
            "Audio"
        };
        let extensions: Vec<&str> = extensions.iter().map(String::as_str).collect();
        let dialog = app
            .dialog()
            .file()
            .set_title(title)
            .add_filter(name, &extensions);
        picked(dialog.blocking_pick_file())
    })
    .await
    .ok()
    .flatten()
}

fn picked(path: Option<tauri_plugin_dialog::FilePath>) -> Option<String> {
    path?
        .into_path()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

/// Show a file or folder in Explorer. Relative paths are inside the open set.
#[tauri::command]
pub(crate) fn reveal_path(
    path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut target = PathBuf::from(&path);
    if target.is_relative() {
        let dir = state
            .kit_manager
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .dir
            .clone();
        if let Some(dir) = dir {
            target = dir.join(target);
        }
    }
    app.opener()
        .reveal_item_in_dir(&target)
        .map_err(|e| format!("Couldn't show {}: {e}", target.display()))
}

#[tauri::command]
pub(crate) fn open_log_folder(app: AppHandle) -> Result<(), String> {
    let dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    let _ = std::fs::create_dir_all(&dir);
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// `<Documents>\Launchpad Drummer`, created if missing.
#[tauri::command]
pub(crate) async fn default_sets_dir(app: AppHandle) -> Result<String, String> {
    let dir = app
        .path()
        .document_dir()
        .map_err(|e| e.to_string())?
        .join("Launchpad Drummer");
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(dir.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) fn get_save_status(state: State<'_, AppState>) -> SaveStatus {
    state.save_status()
}

/// `key` names a continuous control (`pad:<bank>:<x>:<y>:<field>`) so a drag is one undo step.
#[tauri::command]
pub(crate) fn update_pad(bank: usize, pad: Pad, key: Option<String>, state: State<'_, AppState>) {
    state.update_pad(bank, pad, key.as_deref());
}

#[tauri::command]
pub(crate) fn clear_pad(bank: usize, x: u8, y: u8, state: State<'_, AppState>) -> Set {
    state.clear_pad(bank, x, y)
}

#[tauri::command]
pub(crate) fn move_pad(
    from_bank: usize,
    from_x: u8,
    from_y: u8,
    to_bank: usize,
    to_x: u8,
    to_y: u8,
    copy: bool,
    state: State<'_, AppState>,
) -> Option<Set> {
    state.move_pad((from_bank, from_x, from_y), (to_bank, to_x, to_y), copy)
}

#[tauri::command]
pub(crate) async fn drop_files(
    bank: usize,
    x: u8,
    y: u8,
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> Result<DropReport, String> {
    worker(&state, move |s| s.drop_files(bank, x, y, paths)).await
}

#[tauri::command]
pub(crate) async fn list_dir(path: String) -> Result<Vec<crate::store::DirEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::store::list_dir(std::path::Path::new(&path)).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn audition_file(path: String, state: State<'_, AppState>) -> Result<(), String> {
    worker(&state, move |s| {
        let sample = lpd_core::kit::decode_file(std::path::Path::new(&path))
            .map_err(|e| format!("{e:#}"))?;
        s.router
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .audition(std::sync::Arc::new(sample));
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) fn stop_audition(state: State<'_, AppState>) {
    state
        .router
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .stop_audition();
}

#[tauri::command]
pub(crate) async fn load_into_pad(
    bank: usize,
    x: u8,
    y: u8,
    path: String,
    state: State<'_, AppState>,
) -> Result<DropReport, String> {
    worker(&state, move |s| s.drop_files(bank, x, y, vec![path])).await
}

#[tauri::command]
pub(crate) async fn locate_sample(
    bank: usize,
    x: u8,
    y: u8,
    path: String,
    state: State<'_, AppState>,
) -> Result<Set, String> {
    worker(&state, move |s| s.locate_sample(bank, x, y, &path)).await
}

#[tauri::command]
pub(crate) fn update_group(group: usize, group_data: Group, state: State<'_, AppState>) {
    state.update_group(group, group_data);
}

#[tauri::command]
pub(crate) fn update_master_db(master_db: f32, state: State<'_, AppState>) {
    state.update_master_db(master_db);
}

#[tauri::command]
pub(crate) fn update_bank_lights(bank: usize, lights: LightModes, state: State<'_, AppState>) {
    state.update_bank_lights(bank, lights);
}

#[tauri::command]
pub(crate) fn rename_bank(bank: usize, name: String, state: State<'_, AppState>) {
    state.rename_bank(bank, name);
}

#[tauri::command]
pub(crate) fn stop_all(state: State<'_, AppState>) {
    state.stop_all();
}

#[tauri::command]
pub(crate) async fn import_sample(
    src: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    worker(&state, move |s| s.import_sample(&src)).await
}

/// Decode a sample (set-relative path, or any file, which is copied into the set first) and
/// assign it to a pad. Returns the updated set.
#[tauri::command]
pub(crate) async fn assign_sample(
    bank: usize,
    x: u8,
    y: u8,
    path: String,
    default_color: Option<[u8; 3]>,
    state: State<'_, AppState>,
) -> Result<Set, String> {
    worker(&state, move |s| {
        s.assign_sample(bank, x, y, &path, default_color)
    })
    .await
}

#[tauri::command]
pub(crate) fn update_bank_humanize(bank: usize, humanize_db: f32, state: State<'_, AppState>) {
    state.update_bank_humanize(bank, humanize_db);
}

#[tauri::command]
pub(crate) fn sample_peaks(
    path: String,
    buckets: usize,
    state: State<'_, AppState>,
) -> Option<SamplePeaks> {
    state.sample_peaks(&path, buckets)
}

#[tauri::command]
pub(crate) fn undo(state: State<'_, AppState>) -> Option<Set> {
    state.undo()
}

#[tauri::command]
pub(crate) fn redo(state: State<'_, AppState>) -> Option<Set> {
    state.redo()
}

/// Learn the next MIDI input for `target` (`{ pad: { bank, x, y } }` or `{ param: id }`).
#[tauri::command]
pub(crate) fn arm_learn(target: LearnTarget, state: State<'_, AppState>) {
    state.arm_learn(target);
}

#[tauri::command]
pub(crate) fn cancel_learn(state: State<'_, AppState>) {
    state.cancel_learn();
}

#[tauri::command]
pub(crate) fn unbind(index: usize, state: State<'_, AppState>) -> Set {
    state.unbind(index)
}

#[tauri::command]
pub(crate) fn seq_play(play: bool, state: State<'_, AppState>) {
    state.seq_play(play);
}

#[tauri::command]
pub(crate) fn seq_rec(armed: bool, state: State<'_, AppState>) {
    state.seq_rec(armed);
}

#[tauri::command]
pub(crate) fn seq_update(
    pattern: lpd_core::seq::Pattern,
    key: Option<String>,
    state: State<'_, AppState>,
) -> Result<Set, String> {
    state.seq_update(pattern, key.as_deref())
}

#[tauri::command]
pub(crate) fn seq_add_lane(
    bank: usize,
    x: u8,
    y: u8,
    state: State<'_, AppState>,
) -> Result<Set, String> {
    state.seq_add_lane(bank, x, y)
}

#[tauri::command]
pub(crate) fn seq_audition_step(bank: usize, x: u8, y: u8, state: State<'_, AppState>) {
    state.seq_audition_step(bank, x, y);
}

#[tauri::command]
pub(crate) fn history_state(state: State<'_, AppState>) -> HistoryState {
    state.history_state()
}

/// `"sampler"` or `"game"`: who owns the Launchpad pads and LEDs. Returns the sampler's bank.
#[tauri::command]
pub(crate) fn set_input_mode(mode: String, state: State<'_, AppState>) -> Result<usize, String> {
    let mode = match mode.as_str() {
        "sampler" => InputMode::Sampler,
        "game" => InputMode::Game,
        other => return Err(format!("unknown input mode {other:?}")),
    };
    Ok(state.set_input_mode(mode))
}

/// Repaint every Launchpad LED on the next frame.
#[tauri::command]
pub(crate) fn refresh_leds(state: State<'_, AppState>) {
    state
        .painter
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .invalidate();
}
