use crate::app_state::AppState;
use crate::domain::{
    LibraryGenreDto, LibraryMangaDto, LibraryStatsDto, PagedResult, ReconcileReportDto,
    RemoveMangaFilesDto,
};
use crate::error::AppError;
use crate::services::library_service;

#[tauri::command]
pub async fn library_stats(state: tauri::State<'_, AppState>) -> Result<LibraryStatsDto, AppError> {
    library_service::stats(&state.db).await
}

#[tauri::command]
pub async fn library_list(
    query: Option<String>,
    sort: Option<String>,
    filter: Option<String>,
    genre: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<PagedResult<LibraryMangaDto>, AppError> {
    let limit = limit.unwrap_or(crate::domain::DEFAULT_PAGE_LIMIT);
    let offset = offset.unwrap_or(0);
    let (items, total) = library_service::list_mangas(
        &state.db,
        query.as_deref(),
        sort.as_deref(),
        filter.as_deref(),
        genre.as_deref(),
        limit,
        offset,
    )
    .await?;
    Ok(PagedResult {
        items,
        total,
        limit,
        offset,
    })
}

/// Genres present in the local library, with our own per-genre story counts.
#[tauri::command]
pub async fn library_genres(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<LibraryGenreDto>, AppError> {
    library_service::genres(&state.db).await
}

#[tauri::command]
pub async fn library_get_downloaded_chapters(
    manga_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, AppError> {
    let chapters =
        library_service::downloaded_chapters(&state.db, &state.app_data_dir, &manga_id).await?;
    Ok(serde_json::json!({ "chapters": chapters }))
}

#[tauri::command]
pub async fn library_reconcile(
    state: tauri::State<'_, AppState>,
) -> Result<ReconcileReportDto, AppError> {
    library_service::reconcile(&state.db, &state.app_data_dir).await
}

#[tauri::command]
pub async fn library_remove_chapter(
    chapter_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    library_service::remove_chapter_files(&state.db, &state.app_data_dir, &chapter_id).await
}

#[tauri::command]
pub async fn library_remove_manga_files(
    manga_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<RemoveMangaFilesDto, AppError> {
    library_service::remove_manga_files(&state.db, &state.app_data_dir, &manga_id).await
}
