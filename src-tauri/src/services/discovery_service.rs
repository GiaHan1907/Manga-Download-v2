use serde::Serialize;
use sqlx::{Connection, SqlitePool};
use url::Url;

use crate::db::repositories::source_repo;
use crate::domain::{now_iso, validate_id, validate_text_len};
use crate::error::AppError;
use crate::sources::adapter::{SourceAdapter, SourceError};
use crate::sources::dto::{RemoteMangaDetail, SourceInfo};
use crate::sources::registry::SourceRegistry;
use crate::sources::resolver::resolve_owned_url;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceView {
    #[serde(flatten)]
    pub info: SourceInfo,
    pub enabled: bool,
    pub builtin: bool,
    pub base_url: Option<String>,
    /// Exact comma-separated image hosts the user registered (null = base
    /// host only). Display-only for the Sources screen.
    pub image_hosts: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSourceError {
    pub source_id: String,
    pub code: String,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub items: Vec<crate::sources::dto::RemoteMangaSummary>,
    pub source_errors: Vec<SearchSourceError>,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDetail {
    #[serde(flatten)]
    pub detail: RemoteMangaDetail,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub manga_id: String,
    pub saved_chapter_count: i64,
    pub updated: bool,
}

fn enabled_map(rows: &[source_repo::SourceRow]) -> std::collections::HashMap<String, bool> {
    rows.iter()
        .map(|row| (row.id.clone(), row.enabled))
        .collect()
}

pub async fn list_sources_view(
    registry: &SourceRegistry,
    pool: &SqlitePool,
) -> Result<Vec<SourceView>, AppError> {
    let rows = source_repo::list_sources(pool).await?;
    let enabled = enabled_map(&rows);
    let row_by_id: std::collections::HashMap<_, _> =
        rows.iter().map(|r| (r.id.clone(), r)).collect();
    Ok(registry
        .adapters()
        .iter()
        .map(|adapter| {
            let info = adapter.info();
            let builtin = crate::sources::registry::is_builtin(&info.id);
            let row = row_by_id.get(&info.id);
            // Built-in rows may carry a user-edited domain/name (sites rotate
            // domains); fall back to the shipped spec when no row exists.
            let spec = crate::sources::registry::builtin_html_spec(&info.id);
            let base_url = row
                .and_then(|r| r.base_url.clone())
                .or_else(|| spec.map(|s| s.base_url.to_string()));
            let image_hosts = row
                .and_then(|r| r.image_hosts.clone())
                .or_else(|| spec.and_then(|s| s.image_hosts.map(str::to_string)));
            SourceView {
                enabled: *enabled.get(&info.id).unwrap_or(&true),
                builtin,
                base_url,
                image_hosts,
                info,
            }
        })
        .collect())
}

pub async fn add_manga_source(
    registry: &SourceRegistry,
    pool: &SqlitePool,
    display_name: &str,
    base_url: &str,
    image_hosts: Option<Vec<String>>,
) -> Result<SourceView, AppError> {
    let display_name = display_name.trim();
    validate_text_len(display_name, 80, "Tên nguồn")?;
    if display_name.is_empty() {
        return Err(AppError::validation("Tên nguồn không được để trống."));
    }
    let image_hosts = image_hosts.unwrap_or_default();
    if image_hosts.len() > 4 {
        return Err(AppError::validation(
            "Tối đa 4 host ảnh phụ trợ cho một nguồn.",
        ));
    }
    let mut normalized_hosts: Vec<String> = Vec::new();
    for host in &image_hosts {
        let Some(normalized) =
            crate::sources::providers::madara_generic::normalize_image_host(host)
        else {
            return Err(AppError::validation(format!(
                "Host ảnh không hợp lệ: {host}"
            )));
        };
        if !normalized_hosts.contains(&normalized) {
            normalized_hosts.push(normalized);
        }
    }
    let adapter = crate::sources::providers::madara_generic::MadaraGeneric::live(
        display_name,
        base_url,
        &normalized_hosts,
    )
    .map_err(AppError::Source)?;
    let candidate = adapter.info();
    if registry.get(&candidate.id).is_some() {
        return Err(AppError::Conflict);
    }
    let image_hosts_csv = if normalized_hosts.is_empty() {
        None
    } else {
        Some(normalized_hosts.join(","))
    };
    source_repo::upsert_source(
        pool,
        &candidate.id,
        display_name,
        "madara",
        Some(base_url.trim()),
        image_hosts_csv.as_deref(),
    )
    .await?;
    let rows = source_repo::list_sources(pool).await?;
    let row = rows
        .iter()
        .find(|r| r.id == candidate.id)
        .ok_or(AppError::Internal)?;
    Ok(SourceView {
        info: candidate,
        enabled: row.enabled,
        builtin: false,
        base_url: row.base_url.clone(),
        image_hosts: row.image_hosts.clone(),
    })
}

/// Fresh registry from current DB rows — used after add/remove.
pub async fn registry_from_pool(pool: &SqlitePool) -> Result<SourceRegistry, AppError> {
    Ok(SourceRegistry::from_rows(
        &source_repo::list_sources(pool).await?,
    ))
}

pub async fn remove_manga_source(pool: &SqlitePool, source_id: &str) -> Result<(), AppError> {
    validate_id(source_id, "sourceId")?;
    if crate::sources::registry::is_builtin(source_id) {
        return Err(AppError::Unsupported);
    }
    source_repo::delete_source(pool, source_id).await
}

/// Edit name + base URL (+ exact image hosts) of any HTML source — user rows
/// AND built-ins (domains rotate; the source id never changes, so saved
/// manga keep working). MangaDex (fixed REST host) is not editable. Validation
/// runs through the same constructor the registry uses; the stored row id IS
/// the identity.
pub async fn update_manga_source(
    registry: &SourceRegistry,
    pool: &SqlitePool,
    source_id: &str,
    display_name: &str,
    base_url: &str,
    image_hosts: Option<Vec<String>>,
) -> Result<SourceView, AppError> {
    validate_id(source_id, "sourceId")?;
    let display_name = display_name.trim();
    validate_text_len(display_name, 80, "Tên nguồn")?;
    if display_name.is_empty() {
        return Err(AppError::validation("Tên nguồn không được để trống."));
    }
    let rows = source_repo::list_sources(pool).await?;
    let row = rows.iter().find(|r| r.id == source_id);
    let spec = crate::sources::registry::builtin_html_spec(source_id);
    let (adapter_kind, profile) = match (spec, row) {
        (Some(spec), _) => (spec.adapter.to_string(), spec.profile),
        (None, Some(row)) if row.adapter == "madara" => (
            row.adapter.clone(),
            crate::sources::providers::madara_generic::SiteProfile::Madara,
        ),
        (None, Some(_)) => return Err(AppError::Unsupported),
        (None, None) => {
            if registry.get(source_id).is_some() {
                return Err(AppError::Unsupported);
            }
            return Err(AppError::not_found("source"));
        }
    };
    if registry.get(source_id).is_none() && row.is_none() {
        return Err(AppError::not_found("source"));
    }
    let image_hosts = image_hosts.unwrap_or_default();
    if image_hosts.len() > 4 {
        return Err(AppError::validation(
            "Tối đa 4 host ảnh phụ trợ cho một nguồn.",
        ));
    }
    let mut normalized_hosts: Vec<String> = Vec::new();
    for host in &image_hosts {
        let Some(normalized) =
            crate::sources::providers::madara_generic::normalize_image_host(host)
        else {
            return Err(AppError::validation(format!(
                "Host ảnh không hợp lệ: {host}"
            )));
        };
        if !normalized_hosts.contains(&normalized) {
            normalized_hosts.push(normalized);
        }
    }
    // Constructor doubles as the validator (https, no port/credentials, no IP
    // host, slugs, host sanity).
    crate::sources::providers::madara_generic::MadaraGeneric::built_in(
        source_id,
        display_name,
        base_url,
        &normalized_hosts,
        profile,
    )
    .map_err(AppError::Source)?;
    // A new domain must not collide with a DIFFERENT source's host.
    let new_host = Url::parse(base_url.trim())
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_lowercase));
    if let Some(host) = new_host {
        for other in registry.adapters() {
            let info = other.info();
            if info.id == source_id {
                continue;
            }
            let other_base = match (
                crate::sources::registry::builtin_html_spec(&info.id),
                rows.iter().find(|r| r.id == info.id),
            ) {
                (Some(spec), r) => r
                    .and_then(|r| r.base_url.clone())
                    .or(Some(spec.base_url.to_string())),
                (None, r) => r.and_then(|r| r.base_url.clone()),
            };
            let same = other_base
                .as_deref()
                .and_then(|u| Url::parse(u.trim()).ok())
                .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
                .is_some_and(|other_host| other_host == host);
            if same {
                return Err(AppError::Conflict);
            }
        }
    }
    let csv = (!normalized_hosts.is_empty()).then(|| normalized_hosts.join(","));
    source_repo::upsert_source(
        pool,
        source_id,
        display_name,
        &adapter_kind,
        Some(base_url.trim()),
        csv.as_deref(),
    )
    .await?;
    let fresh = source_repo::list_sources(pool).await?;
    let stored = fresh
        .iter()
        .find(|r| r.id == source_id)
        .ok_or(AppError::Internal)?;
    let adapter = crate::sources::providers::madara_generic::MadaraGeneric::built_in(
        source_id,
        &stored.display_name,
        stored
            .base_url
            .as_deref()
            .ok_or_else(|| AppError::validation("Nguồn thiếu base_url."))?,
        &csv.clone()
            .unwrap_or_default()
            .split(',')
            .filter(|entry| !entry.is_empty())
            .map(str::to_string)
            .collect::<Vec<String>>(),
        profile,
    )
    .map_err(AppError::Source)?;
    Ok(SourceView {
        info: adapter.info(),
        enabled: stored.enabled,
        builtin: crate::sources::registry::is_builtin(source_id),
        base_url: stored.base_url.clone(),
        image_hosts: stored.image_hosts.clone(),
    })
}

pub async fn set_source_enabled(
    registry: &SourceRegistry,
    pool: &SqlitePool,
    source_id: &str,
    enabled: bool,
) -> Result<bool, AppError> {
    validate_id(source_id, "sourceId")?;
    if registry.get(source_id).is_none() {
        return Err(AppError::not_found("source"));
    }
    if !crate::sources::registry::is_builtin(source_id) {
        source_repo::set_enabled(pool, source_id, enabled).await?;
        return Ok(enabled);
    }
    // Built-ins get a row on first toggle. An EXISTING row keeps the user's
    // edited domain/name (toggling must never reset a migration override).
    let rows = source_repo::list_sources(pool).await?;
    if !rows.iter().any(|r| r.id == source_id) {
        let (name, adapter, base, hosts) =
            if let Some(spec) = crate::sources::registry::builtin_html_spec(source_id) {
                (
                    spec.display_name,
                    spec.adapter,
                    Some(spec.base_url),
                    spec.image_hosts,
                )
            } else {
                ("MangaDex", "mangadex", None, None)
            };
        source_repo::upsert_source(pool, source_id, name, adapter, base, hosts).await?;
    }
    source_repo::set_enabled(pool, source_id, enabled).await?;
    Ok(enabled)
}

pub(crate) fn require_enabled(
    registry: &SourceRegistry,
    rows: &[source_repo::SourceRow],
    source_id: &str,
) -> Result<std::sync::Arc<dyn SourceAdapter>, AppError> {
    if rows.iter().any(|row| row.id == source_id && !row.enabled) {
        return Err(AppError::Source(SourceError::SourceDisabled));
    }
    registry
        .get(source_id)
        .ok_or_else(|| AppError::validation("sourceId không thuộc danh sách nguồn đã đăng ký."))
}

pub async fn search(
    registry: &SourceRegistry,
    pool: &SqlitePool,
    query: &str,
    source_ids: Option<Vec<String>>,
    page: u32,
) -> Result<SearchResult, AppError> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > 200 {
        return Err(AppError::validation(
            "Câu tìm kiếm phải từ 1 đến 200 ký tự.",
        ));
    }
    if page == 0 || page > 50 {
        return Err(AppError::validation(
            "Trang tìm kiếm phải trong khoảng 1..=50.",
        ));
    }
    let rows = source_repo::list_sources(pool).await?;
    let mut items = Vec::new();
    let mut source_errors = Vec::new();
    let mut has_more = false;

    match source_ids.filter(|ids| !ids.is_empty()) {
        None => {
            for adapter in registry.adapters() {
                let id = adapter.info().id;
                if rows.iter().any(|row| row.id == id && !row.enabled) {
                    continue;
                }
                collect(
                    adapter.as_ref(),
                    query,
                    page,
                    &mut items,
                    &mut source_errors,
                    &mut has_more,
                )
                .await;
            }
        }
        Some(ids) => {
            let requested: Vec<String> = ids.into_iter().take(10).collect();
            for id in requested {
                match require_enabled(registry, &rows, &id) {
                    Ok(adapter) => {
                        collect(
                            adapter.as_ref(),
                            query,
                            page,
                            &mut items,
                            &mut source_errors,
                            &mut has_more,
                        )
                        .await
                    }
                    Err(error) => {
                        let (code, message) = match &error {
                            AppError::Source(source) => (source.code(), source.safe_message()),
                            AppError::Validation(message) => ("VALIDATION_ERROR", message.as_str()),
                            _ => ("INTERNAL_ERROR", "Lỗi nội bộ không xác định."),
                        };
                        source_errors.push(SearchSourceError {
                            source_id: id,
                            code: code.to_string(),
                            message: message.to_string(),
                        });
                    }
                }
            }
        }
    }

    if !items.is_empty() {
        let mut seen = std::collections::HashSet::new();
        items.retain(|item| seen.insert((item.source_id.clone(), item.source_manga_id.clone())));
    }

    Ok(SearchResult {
        items,
        source_errors,
        has_more,
    })
}

async fn collect(
    adapter: &dyn SourceAdapter,
    query: &str,
    page: u32,
    items: &mut Vec<crate::sources::dto::RemoteMangaSummary>,
    source_errors: &mut Vec<SearchSourceError>,
    has_more: &mut bool,
) {
    let id = adapter.info().id;
    match adapter.search(query, page).await {
        Ok(page_result) => {
            *has_more |= page_result.has_more;
            items.extend(page_result.items);
        }
        Err(error) => source_errors.push(SearchSourceError {
            source_id: id,
            code: error.code().to_string(),
            message: error.safe_message().to_string(),
        }),
    }
}

pub async fn resolve_url(
    registry: &SourceRegistry,
    pool: &SqlitePool,
    raw_url: &str,
) -> Result<RemoteDetail, AppError> {
    let rows = source_repo::list_sources(pool).await?;
    let (adapter, parsed): (std::sync::Arc<dyn SourceAdapter>, Url) =
        resolve_owned_url(registry, raw_url).map_err(AppError::Source)?;
    if rows
        .iter()
        .any(|row| row.id == adapter.info().id && !row.enabled)
    {
        return Err(AppError::Source(SourceError::SourceDisabled));
    }
    let source_id = adapter.info().id;
    let slug = adapter
        .resolve_url(&parsed)
        .await
        .map_err(AppError::Source)?;
    fetch_detail(&source_id, adapter.as_ref(), pool, &slug).await
}

/// The Home screen's three rails, read from one vetted source's own front
/// page. The source must be registered and enabled like any other call.
pub async fn home_feed(
    registry: &SourceRegistry,
    pool: &SqlitePool,
    source_id: &str,
) -> Result<crate::sources::dto::RemoteHomeFeed, AppError> {
    validate_id(source_id, "sourceId")?;
    let rows = source_repo::list_sources(pool).await?;
    let adapter = require_enabled(registry, &rows, source_id)?;
    adapter.home_feed().await.map_err(AppError::Source)
}

/// One ranking rail by period, for the Home screen's tab switch.
pub async fn home_ranking(
    registry: &SourceRegistry,
    pool: &SqlitePool,
    source_id: &str,
    period: &str,
) -> Result<Vec<crate::sources::dto::HomeFeedItem>, AppError> {
    validate_id(source_id, "sourceId")?;
    if !matches!(period, "day" | "week" | "month") {
        return Err(AppError::validation(
            "Kỳ xếp hạng phải là day, week hoặc month.",
        ));
    }
    let rows = source_repo::list_sources(pool).await?;
    let adapter = require_enabled(registry, &rows, source_id)?;
    adapter.ranking(period).await.map_err(AppError::Source)
}

/// The source's own genre list, for Discover's genre filter. Adapters without
/// a catalog page report `UNSUPPORTED_OPERATION`, which the UI reads as "hide
/// the filter" rather than as an error to show the user.
pub async fn genres(
    registry: &SourceRegistry,
    pool: &SqlitePool,
    source_id: &str,
) -> Result<Vec<crate::sources::dto::GenreTag>, AppError> {
    validate_id(source_id, "sourceId")?;
    let rows = source_repo::list_sources(pool).await?;
    let adapter = require_enabled(registry, &rows, source_id)?;
    adapter.genres().await.map_err(AppError::Source)
}

/// One filtered catalog page. Status and sort are closed sets (rejected, never
/// silently defaulted) and the genre slug is validated before it reaches a URL.
pub async fn browse(
    registry: &SourceRegistry,
    pool: &SqlitePool,
    source_id: &str,
    genre: Option<&str>,
    status: Option<&str>,
    sort: Option<&str>,
    page: u32,
) -> Result<SearchResult, AppError> {
    validate_id(source_id, "sourceId")?;
    if let Some(value) = status {
        if !matches!(value, "new" | "ongoing" | "completed") {
            return Err(AppError::validation(
                "Trạng thái phải là new, ongoing hoặc completed.",
            ));
        }
    }
    if let Some(value) = sort {
        if !matches!(
            value,
            "newest-desc"
                | "newest-asc"
                | "updated-desc"
                | "updated-asc"
                | "views-desc"
                | "views-asc"
        ) {
            return Err(AppError::validation(
                "Kiểu sắp xếp không được nguồn hỗ trợ.",
            ));
        }
    }
    if let Some(slug) = genre {
        if !valid_genre_slug(slug) {
            return Err(AppError::validation("Thể loại không hợp lệ."));
        }
    }
    if sort.is_some() && genre.is_none() {
        // The source only publishes its sort control on genre pages; its
        // status routes answer any `?sort=` with the unchanged list.
        return Err(AppError::validation(
            "Chỉ sắp xếp được khi đã chọn thể loại.",
        ));
    }
    if genre.is_none() && status.is_none() {
        return Err(AppError::validation("Cần chọn thể loại hoặc trạng thái."));
    }
    let rows = source_repo::list_sources(pool).await?;
    let adapter = require_enabled(registry, &rows, source_id)?;
    let result = adapter
        .browse(genre, status, sort, page.max(1))
        .await
        .map_err(AppError::Source)?;
    // One source serves a browse, so a failure is the whole failure — there is
    // no per-source error list to aggregate the way multi-source search has.
    Ok(SearchResult {
        items: result.items,
        source_errors: Vec::new(),
        has_more: result.has_more,
    })
}

/// Genre slugs go into a URL path, so they are restricted to exactly what the
/// source itself publishes: lowercase letters, digits and inner hyphens.
fn valid_genre_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 80
        && !slug.starts_with('-')
        && !slug.ends_with('-')
        && !slug.contains("--")
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub async fn remote_detail(
    registry: &SourceRegistry,
    pool: &SqlitePool,
    source_id: &str,
    source_manga_id: &str,
) -> Result<RemoteDetail, AppError> {
    validate_id(source_id, "sourceId")?;
    validate_text_len(source_manga_id, 140, "sourceMangaId")?;
    let rows = source_repo::list_sources(pool).await?;
    let adapter = require_enabled(registry, &rows, source_id)?;
    fetch_detail(source_id, adapter.as_ref(), pool, source_manga_id).await
}

async fn fetch_detail(
    source_id: &str,
    adapter: &dyn SourceAdapter,
    pool: &SqlitePool,
    source_manga_id: &str,
) -> Result<RemoteDetail, AppError> {
    let mut detail = adapter
        .manga_detail(source_manga_id)
        .await
        .map_err(AppError::Source)?;
    detail.summary.source_id = source_id.to_string();
    let saved: Option<(String,)> =
        sqlx::query_as("SELECT id FROM mangas WHERE source_id = ?1 AND source_manga_id = ?2")
            .bind(source_id)
            .bind(source_manga_id)
            .fetch_optional(pool)
            .await?;
    detail.saved_manga_id = saved.map(|row| row.0);
    Ok(RemoteDetail { detail })
}

/// Explicit save: re-fetch from the trusted provider (never trusts renderer
/// metadata), then one transaction upserting source/manga/chapters.
/// A chapter link resolved by one registered adapter: which source and which
/// stable ids inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterLink {
    pub source_id: String,
    pub manga_slug: String,
    pub chapter_slug: String,
}

/// Classify a user-pasted URL through the registered adapters WITHOUT any
/// network: exact https host of a registered source plus that adapter's own
/// path grammar (`chapter_from_url`). Manga-level URLs intentionally do NOT
/// match here — they belong to the Save-in-Discover flow.
pub fn find_chapter_link(registry: &SourceRegistry, url: &str) -> Result<ChapterLink, AppError> {
    let parsed = Url::parse(url.trim()).map_err(|_| AppError::validation("URL không hợp lệ."))?;
    if parsed.scheme() != "https" {
        return Err(AppError::validation("Chỉ hỗ trợ liên kết HTTPS."));
    }
    for adapter in registry.adapters() {
        if let Some((manga_slug, chapter_slug)) = adapter.chapter_from_url(&parsed) {
            return Ok(ChapterLink {
                source_id: adapter.info().id,
                manga_slug,
                chapter_slug,
            });
        }
    }
    // Distinguish the two failure shapes so the user knows WHAT to fix:
    // missing source registration vs wrong link level.
    let host_owned = registry.adapters().iter().any(|a| a.owns_url(&parsed));
    if host_owned {
        Err(AppError::validation(
            "Nguồn đã đăng ký nhưng đây không phải link chương — hãy mở một chương rồi copy URL (dạng /manga/<truyện>/<chương>/).",
        ))
    } else {
        Err(AppError::validation(
            "Link không thuộc nguồn nào đã đăng ký — vào Settings → Nguồn để thêm site trước (kèm host ảnh nếu ảnh nằm ở CDN khác).",
        ))
    }
}

/// Ensure the manga exists locally and return the DB row id of the chapter
/// the link points at. EVERY paste refresh-saves the catalog first (one live
/// `manga_detail`): a paste is an explicit manual act, and the refresh heals
/// rows a previous buggy parser stored with wrong numbers/titles — otherwise
/// the download would export under the stale name (live bug 2026-10-02:
/// chapter 2.1 stored as number 1 exported onto chapter-1.cbz).
pub async fn ensure_chapter_row(
    registry: &SourceRegistry,
    pool: &SqlitePool,
    link: &ChapterLink,
) -> Result<String, AppError> {
    async fn lookup(pool: &SqlitePool, manga_id: &str, link: &ChapterLink) -> Option<String> {
        sqlx::query_as::<_, (String,)>(
            "SELECT id FROM chapters WHERE manga_id = ?1 AND source_chapter_id = ?2",
        )
        .bind(manga_id)
        .bind(&link.chapter_slug)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .map(|(id,)| id)
    }

    let saved = save(registry, pool, &link.source_id, &link.manga_slug).await?;
    lookup(pool, &saved.manga_id, link)
        .await
        .ok_or_else(|| AppError::not_found("chương tương ứng với link — nguồn có thể đã đổi slug"))
}

pub async fn save(
    registry: &SourceRegistry,
    pool: &SqlitePool,
    source_id: &str,
    source_manga_id: &str,
) -> Result<SaveResult, AppError> {
    let RemoteDetail { detail } = remote_detail(registry, pool, source_id, source_manga_id).await?;
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin_with("BEGIN IMMEDIATE").await?;

    let ts = now_iso();
    // Key the row by the identity the detail ACTUALLY represents. An adapter may
    // heal an incomplete one — DocTruyen3Q's routes need a numeric story id that
    // a pasted chapter URL does not carry — and storing the caller's raw input
    // would leave a row whose own id cannot be fetched again.
    let manga_key = detail.summary.source_manga_id.as_str();
    // mangas.source_id is an FK; built-in sources may have no row until first save.
    sqlx::query(
        "INSERT INTO sources (id, display_name, enabled, created_at, updated_at)
         VALUES (?1, ?2, 1, ?3, ?3) ON CONFLICT(id) DO NOTHING",
    )
    .bind(source_id)
    .bind(&detail.summary.source_display_name)
    .bind(&ts)
    .execute(&mut *tx)
    .await?;
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM mangas WHERE source_id = ?1 AND source_manga_id = ?2")
            .bind(source_id)
            .bind(manga_key)
            .fetch_optional(&mut *tx)
            .await?;
    let updated = existing.is_some();
    let manga_id = match existing {
        Some((id,)) => {
            sqlx::query(
                "UPDATE mangas SET title=?2, description=?3, status=?4, cover_key=?5, updated_at=?6 WHERE id=?1",
            )
            .bind(&id)
            .bind(&detail.summary.title)
            .bind(&detail.description)
            .bind(&detail.summary.status)
            .bind(&detail.summary.cover_key)
            .bind(&ts)
            .execute(&mut *tx)
            .await?;
            id
        }
        None => {
            let id = crate::domain::new_id();
            sqlx::query(
                "INSERT INTO mangas (id, source_id, source_manga_id, title, author, description, status, cover_key, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)",
            )
            .bind(&id)
            .bind(source_id)
            .bind(manga_key)
            .bind(&detail.summary.title)
            .bind(&None::<String>)
            .bind(&detail.description)
            .bind(&detail.summary.status)
            .bind(&detail.summary.cover_key)
            .bind(&ts)
            .execute(&mut *tx)
            .await?;
            id
        }
    };

    // Genres come from the same trusted detail fetch, so the set is rewritten
    // wholesale: a refresh must not leave stale tags behind.
    sqlx::query("DELETE FROM manga_genres WHERE manga_id = ?1")
        .bind(&manga_id)
        .execute(&mut *tx)
        .await?;
    let mut genre_order: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for genre in &detail.genres {
        if genre.trim().is_empty() || !genre_order.insert(genre.as_str()) {
            continue;
        }
        sqlx::query("INSERT INTO manga_genres (manga_id, genre) VALUES (?1, ?2)")
            .bind(&manga_id)
            .bind(genre.trim())
            .execute(&mut *tx)
            .await?;
    }

    let mut saved_chapters = 0_i64;
    for chapter in &detail.chapters {
        let existing_chapter: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM chapters WHERE manga_id = ?1 AND source_chapter_id = ?2",
        )
        .bind(&manga_id)
        .bind(&chapter.source_chapter_id)
        .fetch_optional(&mut *tx)
        .await?;
        match existing_chapter {
            Some((id,)) => {
                sqlx::query("UPDATE chapters SET title=?2, number=?3, updated_at=?4 WHERE id=?1")
                    .bind(&id)
                    .bind(&chapter.title)
                    .bind(chapter.number)
                    .bind(&ts)
                    .execute(&mut *tx)
                    .await?;
            }
            None => {
                let id = crate::domain::new_id();
                sqlx::query(
                    "INSERT INTO chapters (id, manga_id, source_chapter_id, title, number, page_count, created_at, updated_at)
                     VALUES (?1,?2,?3,?4,?5,NULL,?6,?6)",
                )
                .bind(&id)
                .bind(&manga_id)
                .bind(&chapter.source_chapter_id)
                .bind(&chapter.title)
                .bind(chapter.number)
                .bind(&ts)
                .execute(&mut *tx)
                .await?;
            }
        }
        saved_chapters += 1;
    }

    tx.commit().await?;
    Ok(SaveResult {
        manga_id,
        saved_chapter_count: saved_chapters,
        updated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::source_repo;
    use crate::sources::providers::fixture::FixtureProvider;
    use std::sync::Arc;

    fn fixture_registry() -> SourceRegistry {
        SourceRegistry::with_adapters(vec![Arc::new(FixtureProvider)])
    }

    async fn count(pool: &SqlitePool, table: &str) -> i64 {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let (value,): (i64,) = sqlx::query_as(&sql).fetch_one(pool).await.unwrap();
        value
    }

    #[tokio::test]
    async fn t05_02_source_view_defaults_to_enabled_and_flags_builtin() {
        let (_dir, pool) = crate::db::test_pool().await;
        let views = list_sources_view(&fixture_registry(), &pool).await.unwrap();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].info.id, "fixture");
        assert!(views[0].enabled, "no DB row must default to enabled");
        assert!(!views[0].builtin);

        let builtins = list_sources_view(&SourceRegistry::from_rows(&[]), &pool)
            .await
            .unwrap();
        assert_eq!(builtins.len(), 6);
        assert!(builtins.iter().all(|v| v.builtin));
        let ids: Vec<&str> = builtins.iter().map(|v| v.info.id.as_str()).collect();
        assert!(ids.contains(&"mangadex"));
        assert!(ids.contains(&"madara-truyentuoitho-com"));
        assert!(ids.contains(&"truyenqq"));
        assert!(ids.contains(&"nettruyen"));
        assert!(ids.contains(&"dilib"));
        assert!(ids.contains(&"doctruyen3q"));
        let ttt = builtins
            .iter()
            .find(|v| v.info.id == "madara-truyentuoitho-com")
            .unwrap();
        assert_eq!(ttt.base_url.as_deref(), Some("https://truyentuoitho.com/"));
        assert!(ttt.enabled);
        // Built-in TTT is toggleable (creates its row) but never removable.
        set_source_enabled(
            &SourceRegistry::from_rows(&[]),
            &pool,
            "madara-truyentuoitho-com",
            false,
        )
        .await
        .unwrap();
        let after = list_sources_view(&SourceRegistry::from_rows(&[]), &pool)
            .await
            .unwrap();
        assert!(
            !after
                .iter()
                .find(|v| v.info.id == "madara-truyentuoitho-com")
                .unwrap()
                .enabled
        );
        assert!(matches!(
            remove_manga_source(&pool, "madara-truyentuoitho-com").await,
            Err(AppError::Unsupported)
        ));
    }

    #[tokio::test]
    async fn t05_04_search_isolates_provider_errors_and_rejects_unknown_ids() {
        let (_dir, pool) = crate::db::test_pool().await;
        let registry = fixture_registry();

        let ok = search(&registry, &pool, "Đêm", None, 1).await.unwrap();
        assert_eq!(ok.items.len(), 1);
        assert!(ok.source_errors.is_empty());

        let failed = search(&registry, &pool, "__net", None, 1).await.unwrap();
        assert!(failed.items.is_empty());
        assert_eq!(failed.source_errors.len(), 1);
        assert_eq!(failed.source_errors[0].code, "SOURCE_NETWORK");

        let unknown = search(&registry, &pool, "Đêm", Some(vec!["ghost".to_string()]), 1)
            .await
            .unwrap();
        assert_eq!(unknown.source_errors.len(), 1);
        assert_eq!(unknown.source_errors[0].code, "VALIDATION_ERROR");

        source_repo::upsert_source(&pool, "fixture", "Fixture Provider", "fixture", None, None)
            .await
            .unwrap();
        source_repo::set_enabled(&pool, "fixture", false)
            .await
            .unwrap();
        let disabled = search(
            &registry,
            &pool,
            "Đêm",
            Some(vec!["fixture".to_string()]),
            1,
        )
        .await
        .unwrap();
        assert!(disabled.items.is_empty());
        assert_eq!(disabled.source_errors[0].code, "SOURCE_DISABLED");
        // Disabled sources are silently skipped in "All sources" mode.
        let all = search(&registry, &pool, "Đêm", None, 1).await.unwrap();
        assert!(all.items.is_empty() && all.source_errors.is_empty());
    }

    #[tokio::test]
    async fn t05_13_14_save_is_idempotent_and_refreshes_without_duplicates() {
        let (_dir, pool) = crate::db::test_pool().await;
        let registry = fixture_registry();

        let first = save(&registry, &pool, "fixture", "fx-01").await.unwrap();
        assert!(!first.updated);
        assert_eq!(first.saved_chapter_count, 2);
        assert_eq!(count(&pool, "mangas").await, 1);
        assert_eq!(count(&pool, "chapters").await, 2);
        assert_eq!(count(&pool, "sources").await, 1, "FK row registered");

        let second = save(&registry, &pool, "fixture", "fx-01").await.unwrap();
        assert_eq!(second.manga_id, first.manga_id, "stable internal id");
        assert!(second.updated);
        assert_eq!(count(&pool, "mangas").await, 1);
        assert_eq!(count(&pool, "chapters").await, 2, "no duplicate chapters");
    }

    #[tokio::test]
    async fn t05_15_save_preserves_favorites_and_reading_progress() {
        let (_dir, pool) = crate::db::test_pool().await;
        let registry = fixture_registry();
        let saved = save(&registry, &pool, "fixture", "fx-02").await.unwrap();

        let (chapter_id,): (String,) = sqlx::query_as(
            "SELECT id FROM chapters WHERE manga_id = ?1 ORDER BY number ASC LIMIT 1",
        )
        .bind(&saved.manga_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO favorites (manga_id, created_at) VALUES (?1, ?2)")
            .bind(&saved.manga_id)
            .bind(now_iso())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO reading_progress (manga_id, chapter_id, page_index, updated_at) VALUES (?1, ?2, 7, ?3)",
        )
        .bind(&saved.manga_id)
        .bind(&chapter_id)
        .bind(now_iso())
        .execute(&pool)
        .await
        .unwrap();

        save(&registry, &pool, "fixture", "fx-02").await.unwrap();

        let (favorite,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM favorites WHERE manga_id = ?1")
                .bind(&saved.manga_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(favorite, 1);
        let (page_index,): (i64,) =
            sqlx::query_as("SELECT page_index FROM reading_progress WHERE manga_id = ?1")
                .bind(&saved.manga_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(page_index, 7, "metadata refresh must not reset progress");
    }

    #[tokio::test]
    async fn t05_16_constraint_violation_rolls_back_entirely() {
        let (_dir, pool) = crate::db::test_pool().await;
        let registry = fixture_registry();
        // fx-invalid-status returns a status outside the schema CHECK list.
        let result = save(&registry, &pool, "fixture", "fx-invalid-status").await;
        assert!(result.is_err());
        assert_eq!(count(&pool, "mangas").await, 0);
        assert_eq!(count(&pool, "chapters").await, 0);
        assert_eq!(
            count(&pool, "sources").await,
            0,
            "source row from the same transaction must be rolled back too"
        );
    }

    #[tokio::test]
    async fn t05_17_saved_metadata_survives_reopen() {
        let (dir, pool) = crate::db::test_pool().await;
        let registry = fixture_registry();
        let saved = save(&registry, &pool, "fixture", "fx-03").await.unwrap();
        drop(pool);

        let reopened = crate::db::init_pool(dir.path()).await.unwrap();
        let (title,): (String,) = sqlx::query_as("SELECT title FROM mangas WHERE id = ?1")
            .bind(&saved.manga_id)
            .fetch_one(&reopened)
            .await
            .unwrap();
        assert_eq!(title, "Signal Cũ Không Ghi Đè");
        assert_eq!(count(&reopened, "chapters").await, 2);
    }

    #[tokio::test]
    async fn resolve_url_previews_and_links_saved_manga() {
        let (_dir, pool) = crate::db::test_pool().await;
        let registry = fixture_registry();

        let preview = resolve_url(
            &registry,
            &pool,
            "https://fixture.manga1989.invalid/manga/fx-01",
        )
        .await
        .unwrap();
        assert_eq!(preview.detail.summary.source_manga_id, "fx-01");
        assert_eq!(preview.detail.saved_manga_id, None);

        let saved = save(&registry, &pool, "fixture", "fx-01").await.unwrap();
        let relinked = resolve_url(
            &registry,
            &pool,
            "https://fixture.manga1989.invalid/manga/fx-01/",
        )
        .await
        .unwrap();
        assert_eq!(
            relinked.detail.saved_manga_id.as_deref(),
            Some(saved.manga_id.as_str())
        );

        let unsupported = resolve_url(&registry, &pool, "https://not-registered.test/manga/x")
            .await
            .unwrap_err();
        assert_eq!(unsupported.wire_code(), "UNSUPPORTED_SOURCE");
        let malformed = resolve_url(&registry, &pool, "khong phai url")
            .await
            .unwrap_err();
        assert_eq!(malformed.wire_code(), "INVALID_URL");
    }

    #[tokio::test]
    async fn source_registry_rebuilds_after_add_toggle_and_remove() {
        let (_dir, pool) = crate::db::test_pool().await;
        let registry = fixture_registry();

        let view = add_manga_source(
            &registry,
            &pool,
            "Example Site",
            "https://example.com/",
            Some(vec![" CDN.Example.net ".to_string()]),
        )
        .await
        .unwrap();
        assert_eq!(view.info.id, "madara-example-com");
        assert!(!view.builtin);
        assert_eq!(view.image_hosts.as_deref(), Some("cdn.example.net"));

        // Duplicate host registration is rejected once the registry knows it.
        let refreshed = registry_from_pool(&pool).await.unwrap();
        let again = add_manga_source(
            &refreshed,
            &pool,
            "Example Site",
            "https://example.com/",
            None,
        )
        .await;
        assert!(matches!(again, Err(AppError::Conflict)));

        // A bogus image host is refused before anything is persisted.
        let bogus = add_manga_source(
            &refreshed,
            &pool,
            "Other",
            "https://other.example/",
            Some(vec!["10.1.2.3".to_string()]),
        )
        .await;
        assert!(matches!(bogus, Err(AppError::Validation(_))));

        let rebuilt = registry_from_pool(&pool).await.unwrap();
        let ids: Vec<String> = rebuilt
            .adapters()
            .iter()
            .map(|adapter| adapter.info().id)
            .collect();
        assert!(ids.contains(&"mangadex".to_string()));
        assert!(ids.contains(&"madara-example-com".to_string()));

        set_source_enabled(&rebuilt, &pool, "madara-example-com", false)
            .await
            .unwrap();
        let views = list_sources_view(&rebuilt, &pool).await.unwrap();
        let disabled = views
            .iter()
            .find(|v| v.info.id == "madara-example-com")
            .unwrap();
        assert!(!disabled.enabled);
        assert_eq!(disabled.base_url.as_deref(), Some("https://example.com/"));

        remove_manga_source(&pool, "madara-example-com")
            .await
            .unwrap();
        let after_remove = registry_from_pool(&pool).await.unwrap();
        assert_eq!(
            after_remove.adapters().len(),
            6,
            "only the built-ins remain"
        );
        assert!(matches!(
            remove_manga_source(&pool, "mangadex").await,
            Err(AppError::Unsupported)
        ));
    }

    #[tokio::test]
    async fn chapter_link_saves_unsaved_manga_then_resolves_chapter_row() {
        let (_dir, pool) = crate::db::test_pool().await;
        let registry = fixture_registry();

        let link = find_chapter_link(
            &registry,
            "https://fixture.manga1989.invalid/manga/fx-01/fx-01-c1/",
        )
        .unwrap();
        assert_eq!(link.source_id, "fixture");
        assert_eq!(link.manga_slug, "fx-01");
        assert_eq!(link.chapter_slug, "fx-01-c1");

        // Manga not saved yet → ensure_chapter_row runs the normal save first.
        assert_eq!(count(&pool, "mangas").await, 0);
        let chapter_id = ensure_chapter_row(&registry, &pool, &link).await.unwrap();
        assert_eq!(count(&pool, "mangas").await, 1);
        let (manga_id,): (String,) = sqlx::query_as("SELECT manga_id FROM chapters WHERE id = ?1")
            .bind(&chapter_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        // Second call refresh-saves and resolves the SAME row — no duplicate manga.
        let again = ensure_chapter_row(&registry, &pool, &link).await.unwrap();
        assert_eq!(again, chapter_id);
        assert_eq!(count(&pool, "mangas").await, 1);
        let _ = manga_id;

        // Source published after our last save: a miss on an EXISTING manga
        // triggers exactly one refresh-save, then resolves the chapter.
        sqlx::query("DELETE FROM chapters WHERE id = ?1")
            .bind(&chapter_id)
            .execute(&pool)
            .await
            .unwrap();
        let revived = ensure_chapter_row(&registry, &pool, &link)
            .await
            .expect("refresh-save must recover the chapter row");
        assert_ne!(revived, chapter_id, "refresh re-inserted the catalog row");

        // Unknown chapter slug for a known manga → honest not-found.
        let bad = ChapterLink {
            source_id: "fixture".to_string(),
            manga_slug: "fx-01".to_string(),
            chapter_slug: "no-such-chapter".to_string(),
        };
        assert!(ensure_chapter_row(&registry, &pool, &bad).await.is_err());

        // Rejections carry distinct, actionable hints: unregistered host vs
        // wrong link level.
        let foreign =
            find_chapter_link(&registry, "https://evil.example/manga/fx-01/fx-01-c1/").unwrap_err();
        assert!(
            matches!(&foreign, AppError::Validation(m) if m.contains("Settings")),
            "unregistered host must point at Settings → Nguồn: {foreign:?}"
        );
        let wrong_level =
            find_chapter_link(&registry, "https://fixture.manga1989.invalid/manga/fx-01/")
                .unwrap_err();
        assert!(
            matches!(&wrong_level, AppError::Validation(m) if m.contains("không phải link chương")),
            "registered host + manga-level link must ask for a chapter URL: {wrong_level:?}"
        );
        assert!(find_chapter_link(
            &registry,
            "http://fixture.manga1989.invalid/manga/fx-01/fx-01-c1/"
        )
        .is_err());
    }

    /// A pasted link is an explicit, manual act on ONE chapter — refresh the
    /// saved catalog even when the row already exists. Without this, rows
    /// stored by an older (buggy) parser keep their wrong number forever and
    /// the download exports under the stale name (live bug 2026-10-02:
    /// chapter 2.1 stored as number 1 → exported onto chapter-1.cbz).
    #[tokio::test]
    async fn chapter_link_refreshes_stale_numbers_of_existing_rows() {
        let (_dir, pool) = crate::db::test_pool().await;
        let registry = fixture_registry();
        let link = find_chapter_link(
            &registry,
            "https://fixture.manga1989.invalid/manga/fx-01/fx-01-c1/",
        )
        .unwrap();
        let chapter_id = ensure_chapter_row(&registry, &pool, &link).await.unwrap();
        sqlx::query("UPDATE chapters SET number = 99.0 WHERE id = ?1")
            .bind(&chapter_id)
            .execute(&pool)
            .await
            .unwrap();

        let again = ensure_chapter_row(&registry, &pool, &link)
            .await
            .expect("existing row must still resolve");
        assert_eq!(again, chapter_id, "same row, not a re-insert");
        let number: Option<f64> = sqlx::query_scalar("SELECT number FROM chapters WHERE id = ?1")
            .bind(&chapter_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            number,
            Some(1.0),
            "the refresh-save must heal the stale number"
        );
    }
}
