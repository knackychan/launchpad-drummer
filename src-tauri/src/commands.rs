use crate::audio::AudioStatus;
use crate::state::{AppState, SamplePeaks};
use lpd_core::kit::{Group, LightModes, Pad, Set};
use tauri::State;

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
pub(crate) fn load_set_dir(dir: String, state: State<'_, AppState>) -> Result<Set, String> {
    state.load_set_dir(&dir)
}

#[tauri::command]
pub(crate) fn new_set_dir(dir: String, state: State<'_, AppState>) -> Result<Set, String> {
    state.new_set_dir(&dir)
}

#[tauri::command]
pub(crate) fn save_set_dir(state: State<'_, AppState>) -> Result<(), String> {
    state.save_set()
}

#[tauri::command]
pub(crate) fn update_pad(bank: usize, pad: Pad, state: State<'_, AppState>) {
    state.update_pad(bank, pad);
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
pub(crate) fn import_sample(src: String, state: State<'_, AppState>) -> Result<String, String> {
    state.import_sample(&src)
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
