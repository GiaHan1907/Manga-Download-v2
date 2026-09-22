mod app_state;
mod commands;
mod db;
mod domain;
mod downloader;
mod error;
mod services;
mod sources;

use std::sync::Arc;

use app_state::AppState;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let pool = tauri::async_runtime::block_on(db::init_pool(&app_data_dir))?;
            // Storage-root preparation is recoverable: log and continue rather
            // than failing startup for directories no feature needs yet.
            if let Err(error) = services::storage_service::prepare(&app_data_dir) {
                eprintln!("storage preparation skipped: {error}");
            }
            let rows =
                tauri::async_runtime::block_on(db::repositories::source_repo::list_sources(&pool))?;
            let registry = Arc::new(std::sync::Mutex::new(
                sources::registry::SourceRegistry::from_rows(&rows),
            ));

            // Phase 06: normalize any interrupted lifecycle, then start the
            // bounded dispatcher over the shared registry/pool.
            let events = Arc::new(downloader::progress::ProgressEmitter::new(Arc::new(
                commands::downloads::TauriEventSink::new(app.handle().clone()),
            )));
            match tauri::async_runtime::block_on(downloader::recovery::reconcile_startup(
                &pool,
                &app_data_dir,
            )) {
                Ok(report) => {
                    if report.normalized_tasks > 0 || report.cleared_missing_outputs > 0 {
                        eprintln!(
                            "download recovery: {:?} tasks requeued, {} stale offline flags cleared",
                            report.normalized_tasks, report.cleared_missing_outputs
                        );
                    }
                }
                Err(error) => eprintln!("download recovery skipped: {}", error.wire_pair().0),
            }
            let downloads = downloader::scheduler::Coordinator::new(
                pool.clone(),
                Arc::clone(&registry),
                app_data_dir.clone(),
                Arc::clone(&events),
            );
            downloads.start();

            app.manage(AppState {
                db: pool,
                registry,
                downloads,
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
            commands::tasks::app_list_local_download_tasks,
            commands::downloads::queue_chapter_downloads,
            commands::downloads::get_download_task,
            commands::downloads::pause_download,
            commands::downloads::resume_download,
            commands::downloads::cancel_download,
            commands::downloads::retry_download,
            commands::downloads::set_download_priority
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Cooperative app shutdown: signal every worker before exit so
            // in-flight page writes quiesce. Startup recovery remains the
            // backstop for hard crashes.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app_handle.state::<AppState>();
                tauri::async_runtime::block_on(state.downloads.shutdown());
            }
        });
}
