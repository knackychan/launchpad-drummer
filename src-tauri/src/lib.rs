pub mod audio;
mod launchpad;
pub mod lights;
pub mod router;
pub mod store;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            launchpad::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![launchpad::send_led_frame])
        .run(tauri::generate_context!())
        .expect("failed to start Launchpad Drummer");
}
