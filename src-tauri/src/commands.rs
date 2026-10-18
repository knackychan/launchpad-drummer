use crate::audio::AudioStatus;
use crate::config::AppConfig;
use crate::saver::SaveStatus;
use crate::startup::Startup;
use crate::state::{
    AppState, HistoryState, InputMode, KeysStatus, MissingPad, OpenResult, SamplePeaks,
};
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
pub(crate) fn get_audio_status(state: State<'_, AppState>) -> AudioStatus {
    state.get_audio_status()
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

#[tauri::command]
pub(crate) fn get_config(state: State<'_, AppState>) -> AppConfig {
    state.get_config()
}

#[tauri::command]
pub(crate) async fn set_library_dir(
    dir: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    worker(&state, move |s| s.set_library_dir(dir)).await
}

#[tauri::command]
pub(crate) async fn set_soundfont(
    path: Option<String>,
    state: State<'_, AppState>,
) -> Result<KeysStatus, String> {
    worker(&state, move |s| s.set_soundfont(path)).await
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
pub(crate) fn default_sets_dir(app: AppHandle) -> Result<String, String> {
    let dir = app
        .path()
        .document_dir()
        .map_err(|e| e.to_string())?
        .join("Launchpad Drummer");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().into_owned())
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
pub(crate) fn remove_pad(bank: usize, x: u8, y: u8, state: State<'_, AppState>) {
    state.remove_pad(bank, x, y);
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
