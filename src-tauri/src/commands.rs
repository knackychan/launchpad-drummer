use crate::audio::AudioStatus;
use crate::saver::SaveStatus;
use crate::state::{AppState, HistoryState, InputMode, SamplePeaks};
use lpd_core::kit::{Group, LightModes, Pad, Set};
use tauri::State;

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
pub(crate) async fn load_set_dir(dir: String, state: State<'_, AppState>) -> Result<Set, String> {
    worker(&state, move |s| s.load_set_dir(&dir)).await
}

#[tauri::command]
pub(crate) async fn new_set_dir(dir: String, state: State<'_, AppState>) -> Result<Set, String> {
    worker(&state, move |s| s.new_set_dir(&dir)).await
}

#[tauri::command]
pub(crate) async fn save_set_dir(state: State<'_, AppState>) -> Result<(), String> {
    worker(&state, |s| s.save_set()).await
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
