// Prevent additional windows from opening after the first one
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Set CRABLET_RESOURCE_DIR environment variable
            let resource_dir = app.path_resolver()
                .app_dir()
                .expect("failed to resolve app directory")
                .join("Contents")
                .join("Resources");

            std::env::set_var("CRABLET_RESOURCE_DIR", resource_dir);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
