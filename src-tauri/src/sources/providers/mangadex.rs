use std::sync::Arc;

use async_trait::async_trait;
use percent_encoding::utf8_percent_encode;
use serde_json::Value;
use url::Url;

use crate::sources::adapter::{SourceAdapter, SourceError};
use crate::sources::dto::{
    Capability, RemoteChapter, RemoteMangaDetail, RemoteMangaSummary, RemoteSearchPage, SourceInfo,
    SourceStatus,
};
use crate::sources::http::{mangadex_policy, GuardedHttp, Transport};

const PAGE_SIZE: u32 = 20;
const UUID_LEN: usize = 36;

pub struct MangaDex {
    http: Arc<dyn Transport>,
}

impl MangaDex {
    pub fn live() -> Self {
        MangaDex {
            http: Arc::new(GuardedHttp::new(mangadex_policy())),
        }
    }

    #[cfg(test)]
    pub fn with_transport(http: Arc<dyn Transport>) -> Self {
        MangaDex { http }
    }

    fn is_uuid_like(value: &str) -> bool {
        value.len() == UUID_LEN
            && value.chars().enumerate().all(|(i, c)| match i {
                8 | 13 | 18 | 23 => c == '-',
                _ => c.is_ascii_hexdigit(),
            })
    }

    fn url(path: &str) -> String {
        format!("https://api.mangadex.org{path}")
    }
}

fn localized(map: &Value) -> Option<String> {
    let obj = map.as_object()?;
    for key in ["en", "ja-ro", "romaji", "ja"] {
        if let Some(text) = obj.get(key).and_then(Value::as_str) {
            if !text.trim().is_empty() {
                return Some(text.trim().to_string());
            }
        }
    }
    let mut keys: Vec<&String> = obj.keys().collect();
    keys.sort();
    for key in keys {
        if let Some(text) = obj.get(key).and_then(Value::as_str) {
            if !text.trim().is_empty() {
                return Some(text.trim().to_string());
            }
        }
    }
    None
}

fn map_status(raw: &str) -> &'static str {
    match raw {
        "ongoing" => "ongoing",
        "completed" => "completed",
        _ => "unknown",
    }
}

fn summary_from_entry(entry: &Value) -> Option<RemoteMangaSummary> {
    let id = entry["id"].as_str()?.to_string();
    let attrs = &entry["attributes"];
    let title = localized(&attrs["title"])?;
    let status = attrs["status"]
        .as_str()
        .map(map_status)
        .unwrap_or("unknown");
    let mut author = None;
    let mut cover_key = None;
    if let Some(rels) = attrs["relationship"].as_array() {
        for rel in rels {
            match rel["type"].as_str() {
                Some("author") | Some("artist") if author.is_none() => {
                    author = rel["attributes"]["name"].as_str().map(str::to_string);
                }
                Some("cover_art") => {
                    cover_key = rel["attributes"]["fileName"].as_str().map(str::to_string);
                }
                _ => {}
            }
        }
    }
    Some(RemoteMangaSummary {
        source_id: "mangadex".to_string(),
        source_manga_id: id,
        source_display_name: "MangaDex".to_string(),
        title,
        author,
        cover_key,
        status: status.to_string(),
    })
}

#[async_trait]
impl SourceAdapter for MangaDex {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            id: "mangadex".to_string(),
            display_name: "MangaDex".to_string(),
            capabilities: vec![
                Capability::Search,
                Capability::ResolveUrl,
                Capability::Details,
            ],
            status: SourceStatus::Ready,
            attribution: Some(
                "Metadata & cover references courtesy of MangaDex (api.mangadex.org)".to_string(),
            ),
        }
    }

    fn owns_url(&self, url: &Url) -> bool {
        let Some(host) = url.host_str() else {
            return false;
        };
        matches!(
            host,
            "mangadex.org" | "www.mangadex.org" | "api.mangadex.org"
        ) && (url.path().starts_with("/title/") || url.path().starts_with("/manga/"))
    }

    async fn search(&self, query: &str, page: u32) -> Result<RemoteSearchPage, SourceError> {
        let offset = (page.saturating_sub(1) * PAGE_SIZE) as i64;
        let encoded = utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC);
        let body = self
            .http
            .get_json(&Self::url(&format!(
                "/manga?limit={PAGE_SIZE}&offset={offset}&title={encoded}&includes[]=manga&includes[]=cover_art&includes[]=author&includes[]=artist"
            )))
            .await?;

        let entries = body["data"].as_array().ok_or(SourceError::Parse)?;
        let mut items = Vec::new();
        for entry in entries {
            if entry["type"].as_str() != Some("manga") {
                continue;
            }
            if let Some(summary) = summary_from_entry(entry) {
                items.push(summary);
            }
        }
        let total = body["total"].as_i64().unwrap_or(0);
        let has_more = total > (offset + entries.len() as i64);
        Ok(RemoteSearchPage { items, has_more })
    }

    async fn resolve_url(&self, url: &Url) -> Result<String, SourceError> {
        let id = url
            .path_segments()
            .and_then(|segments| {
                let collected: Vec<&str> = segments.collect();
                collected
                    .iter()
                    .position(|s| matches!(*s, "title" | "manga"))
                    .and_then(|i| collected.get(i + 1))
                    .copied()
            })
            .filter(|candidate| Self::is_uuid_like(candidate))
            .ok_or(SourceError::MalformedUrl)?
            .to_string();
        Ok(id)
    }

    async fn manga_detail(&self, manga_id: &str) -> Result<RemoteMangaDetail, SourceError> {
        if !Self::is_uuid_like(manga_id) {
            return Err(SourceError::MalformedUrl);
        }
        let entry = self
            .http
            .get_json(&Self::url(&format!(
                "/manga/{manga_id}?includes[]=cover_art&includes[]=author&includes[]=artist"
            )))
            .await?;
        let summary = summary_from_entry(&entry["data"]).ok_or(SourceError::Parse)?;
        let attrs = &entry["data"]["attributes"];
        let description = localized(&attrs["description"]);
        let mut genres = Vec::new();
        if let Some(tags) = attrs["tags"].as_array() {
            for tag in tags {
                if let Some(name) = localized(&tag["attributes"]["name"]) {
                    genres.push(name);
                }
            }
        }
        genres.truncate(12);

        let feed = self
            .http
            .get_json(&Self::url(&format!(
                "/manga/{manga_id}/feed?limit=500&offset=0&includes[]=scanlation_group&order[volume]=asc&order[chapter]=asc"
            )))
            .await?;
        let mut chapters = Vec::new();
        if let Some(entries) = feed["data"].as_array() {
            for (index, chapter) in entries.iter().enumerate() {
                let Some(id) = chapter["id"].as_str() else {
                    continue;
                };
                let cattrs = &chapter["attributes"];
                let number = cattrs["chapter"]
                    .as_str()
                    .and_then(|s| s.trim().parse::<f64>().ok());
                let title = cattrs["title"]
                    .as_str()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .or_else(|| number.map(|n| format!("Chapter {n}")))
                    .unwrap_or_else(|| "Untitled chapter".to_string());
                let published_at = cattrs["publishAt"]
                    .as_str()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);
                chapters.push(RemoteChapter {
                    source_chapter_id: id.to_string(),
                    title,
                    number,
                    order: index as i64,
                    published_at,
                });
            }
        }

        Ok(RemoteMangaDetail {
            summary,
            description,
            genres,
            canonical_url: Some(format!("https://mangadex.org/title/{manga_id}")),
            chapters,
            saved_manga_id: None,
            available_offline: false,
        })
    }

    async fn fetch_cover(
        &self,
        source_manga_id: &str,
        cover_key: &str,
    ) -> Result<Vec<u8>, SourceError> {
        // Covers live on MangaDex's fixed uploads CDN (documented public
        // URL shape). The relation fileName must stay a bare token — any
        // path separator or traversal is rejected before a request exists.
        if !Self::is_uuid_like(source_manga_id) {
            return Err(SourceError::MalformedUrl);
        }
        let token = cover_key.trim_start_matches('/');
        let safe = !token.is_empty()
            && token.len() <= 120
            && !token.contains("..")
            && token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
        if !safe {
            return Err(SourceError::MalformedUrl);
        }
        self.http
            .get_bytes(
                &format!("https://uploads.mangadex.org/covers/{source_manga_id}/{token}.250.jpg"),
                crate::sources::http::COVER_MAX_BYTES,
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::http::mock::MockTransport;

    const SEARCH_FIXTURE: &str = r#"{
      "result": "ok",
      "response": "collection",
      "data": [
        {"id":"2a692f23-dcbb-4bc1-9620-957d76d7abdd","type":"manga",
         "attributes":{"title":{"en":"Yotsuba&!!"},"status":"ongoing",
         "description":{"en":"A girl and her garden."},
         "tags":[{"attributes":{"name":{"en":"Comedy"},"description":[]}}],
         "year":2003,
         "relationship":[
            {"type":"author","id":"a","attributes":{"name":"Kiyohiko Azuma"}},
            {"type":"cover_art","id":"b","attributes":{"fileName":"cover.jpg"}}]}}
      ],
      "limit": 20, "offset": 0, "total": 41
    }"#;

    const DETAIL_FIXTURE: &str = r#"{"data":{"id":"2a692f23-dcbb-4bc1-9620-957d76d7abdd","type":"manga","attributes":{
      "title":{"ja":"ヨツバと!","en":"Yotsuba&!!"},"status":"hiatus","description":{"en":"Desc."},
      "tags":[],"relationship":[{"type":"artist","id":"a","attributes":{"name":"Azuma"}}]}}}"#;

    const FEED_FIXTURE: &str = r#"{"data":[
      {"id":"c1aa0000-0000-0000-0000-000000000001","type":"chapter","attributes":{"title":"  ","chapter":"1","volume":"1","publishAt":"2003-09-04T00:00:00+00:00"}},
      {"id":"c1aa0000-0000-0000-0000-000000000002","type":"chapter","attributes":{"title":"Special","chapter":"0.5","publishAt":""}},
      {"id":"c1aa0000-0000-0000-0000-000000000003","type":"chapter","attributes":{"title":null,"chapter":null,"publishAt":null}}
    ],"total":3}"#;

    #[tokio::test]
    async fn search_normalizes_entries_with_identity_and_cover_key() {
        let transport = Arc::new(MockTransport::with(vec![Ok(SEARCH_FIXTURE)]));
        let provider = MangaDex::with_transport(transport.clone());
        let page = provider.search("yotsuba", 1).await.unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].source_id, "mangadex");
        assert_eq!(
            page.items[0].source_manga_id,
            "2a692f23-dcbb-4bc1-9620-957d76d7abdd"
        );
        assert_eq!(page.items[0].title, "Yotsuba&!!");
        assert_eq!(page.items[0].author.as_deref(), Some("Kiyohiko Azuma"));
        assert_eq!(page.items[0].cover_key.as_deref(), Some("cover.jpg"));
        assert!(page.has_more);
        let requested = transport.requested.lock().unwrap();
        assert!(requested[0]
            .1
            .starts_with("https://api.mangadex.org/manga?limit=20&offset=0&title=yotsuba"));
    }

    #[tokio::test]
    async fn detail_maps_status_and_chapter_edge_cases() {
        let transport = Arc::new(MockTransport::with(vec![
            Ok(DETAIL_FIXTURE),
            Ok(FEED_FIXTURE),
        ]));
        let provider = MangaDex::with_transport(transport);
        let detail = provider
            .manga_detail("2a692f23-dcbb-4bc1-9620-957d76d7abdd")
            .await
            .unwrap();
        assert_eq!(detail.summary.title, "Yotsuba&!!"); // en preferred
        assert_eq!(detail.summary.status, "unknown"); // hiatus not in our CHECK set
        assert_eq!(detail.summary.author.as_deref(), Some("Azuma"));
        assert_eq!(detail.chapters.len(), 3);
        // whitespace title falls back to number label
        assert_eq!(detail.chapters[0].title, "Chapter 1");
        assert_eq!(detail.chapters[0].number, Some(1.0));
        assert_eq!(detail.chapters[1].title, "Special");
        assert_eq!(
            detail.chapters[1].published_at, None,
            "empty publishAt -> None"
        );
        // unnumbered chapter keeps stable id + order, number stays None
        assert_eq!(detail.chapters[2].number, None);
        assert_eq!(detail.chapters[2].order, 2);
        assert_eq!(
            detail.canonical_url.as_deref(),
            Some("https://mangadex.org/title/2a692f23-dcbb-4bc1-9620-957d76d7abdd")
        );
        assert!(!detail.available_offline);
    }

    #[test]
    fn url_ownership_is_exact_host_and_path() {
        let provider = MangaDex::live();
        assert!(provider.owns_url(
            &Url::parse("https://mangadex.org/title/2a692f23-dcbb-4bc1-9620-957d76d7abdd").unwrap()
        ));
        assert!(provider.owns_url(
            &Url::parse("https://www.mangadex.org/manga/2a692f23-dcbb-4bc1-9620-957d76d7abdd")
                .unwrap()
        ));
        // spoofed suffix host must NOT be owned
        assert!(!provider.owns_url(&Url::parse("https://mangadex.org.evil.test/title/x").unwrap()));
        assert!(!provider.owns_url(&Url::parse("https://evil.test/mangadex.org").unwrap()));
        assert!(!provider.owns_url(&Url::parse("https://mangadex.org/forum/thread/1").unwrap()));
    }

    #[tokio::test]
    async fn resolve_rejects_non_uuid_path_segment() {
        let provider = MangaDex::live();
        let bad = Url::parse("https://mangadex.org/title/not-a-uuid").unwrap();
        assert!(matches!(
            provider.resolve_url(&bad).await,
            Err(SourceError::MalformedUrl)
        ));
        let ok =
            Url::parse("https://mangadex.org/title/2a692f23-dcbb-4bc1-9620-957d76d7abdd/some-slug")
                .unwrap();
        assert_eq!(
            provider.resolve_url(&ok).await.unwrap(),
            "2a692f23-dcbb-4bc1-9620-957d76d7abdd"
        );
    }

    #[tokio::test]
    async fn fetch_cover_builds_documented_cdn_url_and_rejects_tokens() {
        let transport = Arc::new(MockTransport::with(vec![Ok("img")]));
        let provider = MangaDex::with_transport(transport.clone());
        let uuid = "2a692f23-dcbb-4bc1-9620-957d76d7abdd";
        let bytes = provider.fetch_cover(uuid, "cover.jpg").await.unwrap();
        assert_eq!(bytes, b"img".to_vec());
        assert_eq!(
            transport.requested.lock().unwrap()[0].1,
            format!("https://uploads.mangadex.org/covers/{uuid}/cover.jpg.250.jpg")
        );
        for (manga, key) in [
            ("not-a-uuid", "cover.jpg"),
            (uuid, "../etc/passwd"),
            (uuid, "sub/dir.jpg"),
            (uuid, ""),
        ] {
            assert!(
                provider.fetch_cover(manga, key).await.is_err(),
                "must reject {manga:?}/{key:?}"
            );
        }
        assert_eq!(
            transport.requested.lock().unwrap().len(),
            1,
            "only the valid call fired"
        );
    }
}
