use tauri::State;

use crate::app_state::AppState;
use crate::domain::{validate_id, validate_text_len};
use crate::error::AppError;
use crate::services::discovery_service::{
    self, RemoteDetail, SaveResult, SearchResult, SourceView,
};
use crate::sources::dto::RemoteChapter;

fn snapshot(
    state: &State<'_, AppState>,
) -> Result<crate::sources::registry::SourceRegistry, AppError> {
    let guard = state.registry.lock().map_err(|_| AppError::Internal)?;
    Ok((*guard).clone())
}

#[tauri::command]
pub async fn list_supported_sources(
    state: State<'_, AppState>,
) -> Result<Vec<SourceView>, AppError> {
    let registry = snapshot(&state)?;
    discovery_service::list_sources_view(&registry, &state.db).await
}

#[tauri::command]
pub async fn add_manga_source(
    display_name: String,
    base_url: String,
    image_hosts: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<SourceView, AppError> {
    let registry = snapshot(&state)?;
    let view = discovery_service::add_manga_source(
        &registry,
        &state.db,
        &display_name,
        &base_url,
        image_hosts,
    )
    .await?;
    let fresh = discovery_service::registry_from_pool(&state.db).await?;
    *state.registry.lock().map_err(|_| AppError::Internal)? = fresh;
    Ok(view)
}

#[tauri::command]
pub async fn remove_manga_source(
    source_id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    discovery_service::remove_manga_source(&state.db, &source_id).await?;
    let fresh = discovery_service::registry_from_pool(&state.db).await?;
    *state.registry.lock().map_err(|_| AppError::Internal)? = fresh;
    Ok(())
}

#[tauri::command]
pub async fn set_source_enabled(
    source_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<bool, AppError> {
    let registry = snapshot(&state)?;
    discovery_service::set_source_enabled(&registry, &state.db, &source_id, enabled).await
}

#[tauri::command]
pub async fn search_manga(
    query: String,
    source_ids: Option<Vec<String>>,
    page: Option<u32>,
    state: State<'_, AppState>,
) -> Result<SearchResult, AppError> {
    if let Some(ids) = &source_ids {
        if ids.len() > 10 {
            return Err(AppError::validation(
                "Tối đa 10 nguồn cho một lượt tìm kiếm.",
            ));
        }
        for id in ids {
            validate_id(id, "sourceId")?;
        }
    }
    let registry = snapshot(&state)?;
    discovery_service::search(&registry, &state.db, &query, source_ids, page.unwrap_or(1)).await
}

#[tauri::command]
pub async fn resolve_manga_url(
    url: String,
    state: State<'_, AppState>,
) -> Result<RemoteDetail, AppError> {
    validate_text_len(&url, 2048, "url")?;
    let registry = snapshot(&state)?;
    discovery_service::resolve_url(&registry, &state.db, &url).await
}

#[tauri::command]
pub async fn get_remote_manga_details(
    source_id: String,
    source_manga_id: String,
    state: State<'_, AppState>,
) -> Result<RemoteDetail, AppError> {
    let registry = snapshot(&state)?;
    discovery_service::remote_detail(&registry, &state.db, &source_id, &source_manga_id).await
}

#[tauri::command]
pub async fn get_remote_chapters(
    source_id: String,
    source_manga_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<RemoteChapter>, AppError> {
    let detail = get_remote_manga_details(source_id, source_manga_id, state).await?;
    Ok(detail.detail.chapters)
}

#[tauri::command]
pub async fn save_discovered_manga(
    source_id: String,
    source_manga_id: String,
    state: State<'_, AppState>,
) -> Result<SaveResult, AppError> {
    let registry = snapshot(&state)?;
    discovery_service::save(&registry, &state.db, &source_id, &source_manga_id).await
}

/// Cover bytes for a (source, manga, key) triple previously published by an
/// adapter. Binary like reader pages — neither a URL nor a path ever
/// crosses the boundary in either direction.
#[tauri::command]
pub async fn get_manga_cover(
    source_id: String,
    source_manga_id: String,
    cover_key: String,
    state: State<'_, AppState>,
) -> Result<tauri::ipc::Response, AppError> {
    validate_id(&source_id, "sourceId")?;
    validate_text_len(&source_manga_id, 200, "sourceMangaId")?;
    let registry = snapshot(&state)?;
    let bytes = crate::services::cover_service::cover_bytes(
        &registry,
        &state.app_data_dir,
        &source_id,
        &source_manga_id,
        &cover_key,
    )
    .await?;
    Ok(tauri::ipc::Response::new(bytes))
}
