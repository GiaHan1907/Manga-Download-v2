//! Feature plan F3b — CBZ import command. The native file dialog is opened
//! HERE, in Rust: the chosen path never crosses to the renderer, which sees
//! only the resulting metadata DTO. Cancelled dialog → `Ok(None)`.

use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::app_state::AppState;
use crate::domain::ImportedChapterDto;
use crate::error::AppError;
use crate::services::import_service;

/// Prompt for one `.cbz` and import it under the given manga. Runs the
/// blocking dialog on a pooled thread so the async command never stalls the
/// runtime; the picker itself has no path-in/path-out exposure to JS.
#[tauri::command]
pub async fn library_import_cbz(
    manga_id: String,
    chapter_number: Option<f64>,
    chapter_title: Option<String>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Option<ImportedChapterDto>, AppError> {
    let picked = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Chọn tệp CBZ để nhập")
            .add_filter("CBZ", &["cbz"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| AppError::Internal)?;

    let source = match picked {
        None => return Ok(None),
        Some(FilePath::Path(path)) => path,
        Some(FilePath::Url(url)) => url
            .to_file_path()
            .map_err(|_| AppError::validation("Đường dẫn tệp không hợp lệ."))?,
    };

    let dto = import_service::import_cbz(
        &state.db,
        &state.app_data_dir,
        &source,
        &manga_id,
        chapter_number,
        chapter_title.as_deref(),
    )
    .await?;
    Ok(Some(dto))
}

/// Drag-and-drop variant: the path comes from the webview's native drop
/// event relayed by the renderer, so it is NOT dialog-produced. Guards:
/// hard `.cbz` extension requirement, then the same untrusted-archive
/// validation (traversal, zip-bomb, image-only, size cap) and canonical
/// UUID-named copy as the dialog flow. Nothing but ids + metadata returns.
#[tauri::command]
pub async fn library_import_cbz_path(
    manga_id: String,
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<ImportedChapterDto, AppError> {
    let source = std::path::PathBuf::from(&path);
    let is_cbz = source
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cbz"));
    if !is_cbz {
        return Err(AppError::validation("Chỉ nhận tệp .cbz."));
    }
    import_service::import_cbz(
        &state.db,
        &state.app_data_dir,
        &source,
        &manga_id,
        None,
        None,
    )
    .await
}
