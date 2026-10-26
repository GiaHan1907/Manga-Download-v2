use async_trait::async_trait;
use url::Url;

use crate::sources::adapter::{SourceAdapter, SourceError};
use crate::sources::dto::{
    Capability, PageDescriptor, PageManifest, RemoteChapter, RemoteMangaDetail, RemoteMangaSummary,
    RemoteSearchPage, SourceInfo, SourceStatus,
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
                Capability::Pages,
            ],
            status: SourceStatus::Ready,
            attribution: Some("Local deterministic test fixture".to_string()),
        }
    }

    /// Ordered manifest with a stable page count derived from the catalog.
    /// Two pages per chapter keeps fixture assertions deterministic.
    async fn page_manifest(
        &self,
        source_manga_id: &str,
        source_chapter_id: &str,
    ) -> Result<PageManifest, SourceError> {
        if source_chapter_id == "fx-no-pages" {
            return Err(SourceError::NotFound);
        }
        // A chapter whose id carries `-pdf` is ONE scanned volume served as a
        // single file, the shape TTT produces with its Drive embeds.
        if source_chapter_id.ends_with("-pdf") {
            return Ok(PageManifest {
                source_chapter_id: source_chapter_id.to_string(),
                pages: vec![PageDescriptor {
                    index: 0,
                    url: format!(
                        "https://fixture.manga1989.invalid/files/{source_chapter_id}__pdf"
                    ),
                }],
                single_file_format: Some("pdf".to_string()),
            });
        }
        let count = if source_chapter_id.ends_with("-c2") {
            2
        } else {
            3
        };
        let pages = (0..count)
            .map(|index| PageDescriptor {
                index,
                url: format!(
                    "https://fixture.manga1989.invalid/pages/{}/{index}.png",
                    source_chapter_id
                ),
            })
            .collect();
        let _ = source_manga_id;
        Ok(PageManifest {
            source_chapter_id: source_chapter_id.to_string(),
            pages,
            single_file_format: None,
        })
    }

    async fn fetch_chapter_file(&self, url: &str) -> Result<Vec<u8>, SourceError> {
        if url.contains("__net") {
            return Err(SourceError::Network);
        }
        // Importing shape: one PDF volume. Content is deterministic so tests can
        // assert the stored bytes are untouched.
        let mut bytes = b"%PDF-1.7\n".to_vec();
        bytes.extend(std::iter::repeat_n(b'p', 4096));
        Ok(bytes)
    }

    /// Deterministic in-memory image bytes. Pages whose chapter id carries
    /// `__corrupt` return non-image bytes so the validator failure path is
    /// exercised without network.
    async fn fetch_image(&self, url: &str) -> Result<Vec<u8>, SourceError> {
        if url.contains("__net") {
            return Err(SourceError::Network);
        }
        if url.contains("__corrupt") {
            // 200 with HTML body — must be rejected by image validation.
            return Ok(b"<html>not an image</html>".to_vec());
        }
        let index = url
            .rsplit('/')
            .next()
            .and_then(|tail| tail.split('.').next())
            .and_then(|digits| digits.parse::<u8>().ok())
            .unwrap_or(0);
        // PNG magic + a deterministic payload sized by the page index.
        let mut bytes: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0d, 0x1a, 0x0a, 0x00];
        bytes.extend(std::iter::repeat_n(index.wrapping_add(1), 64));
        Ok(bytes)
    }

    fn owns_url(&self, url: &Url) -> bool {
        url.host_str() == Some("fixture.manga1989.invalid") && url.path().starts_with("/manga/")
    }

    /// Chapter grammar /manga/<known-id>/<chapter> — mirrors the Madara shape
    /// so the paste-a-link service path is testable offline.
    fn chapter_from_url(&self, url: &Url) -> Option<(String, String)> {
        if !self.owns_url(url) {
            return None;
        }
        let mut segments = url.path_segments()?.filter(|segment| !segment.is_empty());
        if segments.next()? != "manga" {
            return None;
        }
        let manga = segments.next()?;
        let chapter = segments.next()?;
        if segments.next().is_some() || !catalog().iter().any(|(known, _)| *known == manga) {
            return None;
        }
        Some((manga.to_string(), chapter.to_string()))
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
