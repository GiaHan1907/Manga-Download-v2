mod app_state;
mod commands;
mod db;
mod error;

use app_state::AppState;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let pool = tauri::async_runtime::block_on(db::init_pool(&app_data_dir))?;
            app.manage(AppState { db: pool });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::app_get_info,
            commands::system::app_health_check
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
