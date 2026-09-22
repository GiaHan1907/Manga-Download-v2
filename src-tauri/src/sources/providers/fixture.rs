use async_trait::async_trait;
use url::Url;

use crate::sources::adapter::{SourceAdapter, SourceError};
use crate::sources::dto::{
    Capability, RemoteChapter, RemoteMangaDetail, RemoteMangaSummary, RemoteSearchPage, SourceInfo,
    SourceStatus,
};

/// Deterministic, offline, TEST-ONLY adapter (never in the production
/// registry). Scenario triggers embedded in queries keep failure paths
/// exercised without network: `__net`, `__parse`, `__rate`, `__empty`.
#[derive(Default)]
pub struct FixtureProvider;

fn summary(id: &str, title: &str) -> RemoteMangaSummary {
    RemoteMangaSummary {
        source_id: "fixture".to_string(),
        source_manga_id: id.to_string(),
        source_display_name: "Fixture Provider".to_string(),
        title: title.to_string(),
        author: Some("Fixture Author".to_string()),
        cover_key: Some("fixture/cover.png".to_string()),
        status: "ongoing".to_string(),
    }
}

fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("fx-01", "Đêm Sáng Thử Nghiệm"),
        ("fx-02", "Vườn Kiểm Thử Idempotent"),
        ("fx-03", "Signal Cũ Không Ghi Đè"),
    ]
}

impl FixtureProvider {
    fn detail(id: &str) -> Option<RemoteMangaDetail> {
        let (_, title) = catalog()
            .into_iter()
            .find(|(manga_id, _)| *manga_id == id)?;
        Some(RemoteMangaDetail {
            summary: summary(id, title),
            description: Some("Fixture description — plain text, deterministic.".to_string()),
            genres: vec!["test".to_string()],
            canonical_url: Some(format!("https://fixture.manga1989.invalid/manga/{id}")),
            chapters: vec![
                RemoteChapter {
                    source_chapter_id: format!("{id}-c1"),
                    title: "Chương 1".to_string(),
                    number: Some(1.0),
                    order: 0,
                    published_at: Some("2026-01-01T00:00:00.000Z".to_string()),
                },
                RemoteChapter {
                    source_chapter_id: format!("{id}-c2"),
                    title: "Chapter 2".to_string(),
                    number: Some(2.0),
                    order: 1,
                    published_at: None,
                },
            ],
            saved_manga_id: None,
            available_offline: false,
        })
    }
}

#[async_trait]
impl SourceAdapter for FixtureProvider {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            id: "fixture".to_string(),
            display_name: "Fixture Provider".to_string(),
            capabilities: vec![
                Capability::Search,
                Capability::ResolveUrl,
                Capability::Details,
            ],
            status: SourceStatus::Ready,
            attribution: Some("Local deterministic test fixture".to_string()),
        }
    }

    fn owns_url(&self, url: &Url) -> bool {
        url.host_str() == Some("fixture.manga1989.invalid") && url.path().starts_with("/manga/")
    }

    async fn search(&self, query: &str, page: u32) -> Result<RemoteSearchPage, SourceError> {
        match query {
            q if q.contains("__net") => return Err(SourceError::Network),
            q if q.contains("__parse") => return Err(SourceError::Parse),
            q if q.contains("__rate") => return Err(SourceError::RateLimited("60".to_string())),
            q if q.contains("__empty") => {
                return Ok(RemoteSearchPage {
                    items: vec![],
                    has_more: false,
                })
            }
            _ => {}
        }
        let needle = query.trim().to_lowercase();
        let all: Vec<RemoteMangaSummary> = catalog()
            .into_iter()
            .filter(|(_, title)| needle.is_empty() || title.to_lowercase().contains(&needle))
            .map(|(id, title)| summary(id, title))
            .collect();
        if page <= 1 {
            let has_more = all.len() > 2;
            Ok(RemoteSearchPage {
                items: all.into_iter().take(2).collect(),
                has_more,
            })
        } else {
            Ok(RemoteSearchPage {
                items: all.into_iter().skip(2).take(2).collect(),
                has_more: false,
            })
        }
    }

    async fn resolve_url(&self, url: &Url) -> Result<String, SourceError> {
        let id = url
            .path_segments()
            .and_then(|mut s| {
                s.next();
                s.next()
            })
            .filter(|id| catalog().iter().any(|(known, _)| known == id))
            .ok_or(SourceError::NotFound)?
            .to_string();
        Ok(id)
    }

    async fn manga_detail(&self, source_manga_id: &str) -> Result<RemoteMangaDetail, SourceError> {
        if source_manga_id == "fx-invalid-status" {
            // Simulates a provider violating our schema CHECK constraints.
            return Ok(RemoteMangaDetail {
                summary: RemoteMangaSummary {
                    source_id: "fixture".to_string(),
                    source_manga_id: source_manga_id.to_string(),
                    source_display_name: "Fixture Provider".to_string(),
                    title: "Invalid".to_string(),
                    author: None,
                    cover_key: None,
                    status: "hiatus-forever".to_string(),
                },
                description: None,
                genres: vec![],
                canonical_url: None,
                chapters: vec![],
                saved_manga_id: None,
                available_offline: false,
            });
        }
        Self::detail(source_manga_id).ok_or(SourceError::NotFound)
    }
}
