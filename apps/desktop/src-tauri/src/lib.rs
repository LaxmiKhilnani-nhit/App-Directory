mod config;

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreInfo {
    app_name: String,
    version: String,
    api_base_url: String,
    running_in_tauri: bool,
}

#[tauri::command]
fn core_info() -> CoreInfo {
    CoreInfo {
        app_name: config::APP_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        api_base_url: config::api_base_url(),
        running_in_tauri: true,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![core_info])
        .run(tauri::generate_context!())
        .expect("error while running App Directory");
}
