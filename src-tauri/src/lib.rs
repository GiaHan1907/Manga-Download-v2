mod app_state;
mod commands;
mod db;
mod domain;
mod error;
mod services;
mod sources;

use app_state::AppState;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let pool = tauri::async_runtime::block_on(db::init_pool(&app_data_dir))?;
            // Storage-root preparation is recoverable: log and continue rather
            // than failing startup for directories no Phase 04 feature needs yet.
            if let Err(error) = services::storage_service::prepare(&app_data_dir) {
                eprintln!("storage preparation skipped: {error}");
            }
            let rows =
                tauri::async_runtime::block_on(db::repositories::source_repo::list_sources(&pool))?;
            let registry = sources::registry::SourceRegistry::from_rows(&rows);
            app.manage(AppState {
                db: pool,
                registry: std::sync::Arc::new(std::sync::Mutex::new(registry)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::discovery::list_supported_sources,
            commands::discovery::add_manga_source,
            commands::discovery::remove_manga_source,
            commands::discovery::set_source_enabled,
            commands::discovery::search_manga,
            commands::discovery::resolve_manga_url,
            commands::discovery::get_remote_manga_details,
            commands::discovery::get_remote_chapters,
            commands::discovery::save_discovered_manga,
            commands::system::app_get_info,
            commands::system::app_health_check,
            commands::settings::app_get_settings,
            commands::settings::app_update_settings,
            commands::settings::app_reset_settings,
            commands::local_catalog::app_list_local_mangas,
            commands::local_catalog::app_get_local_manga,
            commands::favorites::app_set_manga_favorite,
            commands::favorites::app_list_favorite_mangas,
            commands::reading_progress::app_get_reading_progress,
            commands::reading_progress::app_save_reading_progress,
            commands::tasks::app_list_local_download_tasks
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
