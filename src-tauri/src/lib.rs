mod app_state;
mod commands;
mod db;
mod domain;
mod downloader;
mod error;
#[cfg(test)]
mod perf;
#[cfg(test)]
mod probe;
mod reader;
mod services;
mod sources;

use std::sync::Arc;

use app_state::{AppState, RuntimeFlags};
use tauri::Emitter;
use tauri::Manager;

pub fn run() {
    // Settings mirror for synchronous paths (window close, notify sink);
    // populated from the DB during setup and refreshed by settings commands.
    let flags = Arc::new(RuntimeFlags::default());
    let window_flags = Arc::clone(&flags);

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(move |window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && window_flags.close_to_tray() {
                    // Close-to-tray: window hides, process (and downloads)
                    // keep running — the tray is the only way back or out.
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(move |app| {
            let app_data_dir = app.path().app_data_dir()?;
            let pool = tauri::async_runtime::block_on(db::init_pool(&app_data_dir))?;
            if let Ok(settings) = tauri::async_runtime::block_on(services::settings_service::load(&pool)) {
                flags.apply(&settings);
            }
            // Storage-root preparation is recoverable: log and continue rather
            // than failing startup for directories no feature needs yet.
            if let Err(error) = services::storage_service::prepare(&app_data_dir) {
                eprintln!("storage preparation skipped: {:?}", error.kind());
            }
            let rows =
                tauri::async_runtime::block_on(db::repositories::source_repo::list_sources(&pool))?;
            let registry = Arc::new(std::sync::Mutex::new(
                sources::registry::SourceRegistry::from_rows(&rows),
            ));

            // Phase 06: normalize any interrupted lifecycle, then start the
            // bounded dispatcher over the shared registry/pool.
            let events = Arc::new(downloader::progress::ProgressEmitter::new(Arc::new(
                commands::downloads::TauriEventSink::new(app.handle().clone(), Arc::clone(&flags)),
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
            // Phase 08: drop staging dirs left by terminal/unknown tasks.
            match tauri::async_runtime::block_on(services::storage_service::clear_regenerable_cache(
                &pool,
                &app_data_dir,
            )) {
                Ok(cleanup) => {
                    if cleanup.removed_dirs > 0 {
                        eprintln!(
                            "startup cache cleanup: {} dirs, {} bytes freed",
                            cleanup.removed_dirs, cleanup.freed_bytes
                        );
                    }
                }
                Err(error) => eprintln!("startup cache cleanup skipped: {}", error.wire_pair().0),
            }
            let downloads = downloader::scheduler::Coordinator::new(
                pool.clone(),
                Arc::clone(&registry),
                app_data_dir.clone(),
                Arc::clone(&events),
            );
            downloads.start();

            // The tray is an enhancement — a platform refusal must never
            // brick startup (same policy as recovery/cleanup logging).
            if let Err(error) = build_tray(app.handle()) {
                eprintln!("system tray unavailable: {error}");
            }

            app.manage(AppState {
                db: pool,
                registry,
                downloads,
                app_data_dir: app_data_dir.clone(),
                flags: Arc::clone(&flags),
            });
            // Feature plan F1: ~6 h new-chapter watcher (metadata only).
            commands::watch::spawn_watcher_loop(app.handle());
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
            commands::reading_progress::app_list_read_history,
            commands::reading_progress::app_clear_read_history,
            commands::tasks::app_list_local_download_tasks,
            commands::downloads::queue_chapter_downloads,
            commands::downloads::queue_chapter_download_by_url,
            commands::downloads::get_download_task,
            commands::downloads::pause_download,
            commands::downloads::resume_download,
            commands::downloads::cancel_download,
            commands::downloads::retry_download,
            commands::downloads::set_download_priority,
            commands::downloads::clear_download_history,
            commands::downloads::retry_all_failed_downloads,
            commands::downloads::cancel_downloads_for_manga,
            commands::watch::watch_run_now,
            commands::watch::library_set_watch,
            commands::watch::library_list_alerts,
            commands::watch::library_ack_alerts,
            commands::watch::library_get_watch,
            commands::library::library_stats,
            commands::library::library_list,
            commands::library::library_get_downloaded_chapters,
            commands::library::library_reconcile,
            commands::library::library_remove_chapter,
            commands::library::library_remove_manga_files,
            commands::import::library_import_cbz,
            commands::reader::reader_open_chapter,
            commands::reader::reader_get_page_bytes,
            commands::storage::storage_status,
            commands::storage::clear_regenerable_cache
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Cooperative app shutdown: signal every worker before exit so
            // in-flight page writes quiesce. Recovery on next start re-queues
            // anything that never reached a terminal state.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app_handle.state::<AppState>();
                tauri::async_runtime::block_on(state.downloads.shutdown());
            }
        });
}

/// Phase 08 tray: Show/Hide, Open Downloads, Quit. Quit goes through
/// `app.exit` so the ExitRequested shutdown lifecycle always runs —
/// there is no "keep downloading after process exit" claim anywhere.
fn build_tray(handle: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::TrayIconBuilder;

    let show = MenuItem::with_id(handle, "tray-show", "Hiển thị cửa sổ", true, None::<&str>)?;
    let downloads_item =
        MenuItem::with_id(handle, "tray-downloads", "Mở Downloads", true, None::<&str>)?;
    let quit = MenuItem::with_id(handle, "tray-quit", "Thoát", true, None::<&str>)?;
    let menu = Menu::with_items(handle, &[&show, &downloads_item, &quit])?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Manga Downloader 1989")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray-show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "tray-downloads" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                let _ = app.emit("tray-open-downloads", ());
            }
            "tray-quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = handle.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(handle)?;
    Ok(())
}
