// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod zjuam;

#[tauri::command]
async fn login_zju_command(username: String, password: String) -> Result<String, String> {
    match zjuam::login_zju(&username, &password).await {
        Ok(_) => Ok("Login successful".to_string()),
        Err(e) => Err(e),
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![login_zju_command])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
