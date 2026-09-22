use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

use crate::sources::adapter::{SourceAdapter, SourceError};
use crate::sources::dto::{
    Capability, PageDescriptor, PageManifest, RemoteChapter, RemoteMangaDetail, RemoteMangaSummary,
    RemoteSearchPage, SourceInfo, SourceStatus,
};
use crate::sources::http::{GuardedHttp, HttpPolicy, Transport, IMAGE_MAX_BYTES, MAX_PAGE_COUNT};

/// Generic WordPress/Madara adapter. Users register any Madara-based site by
/// name + https base URL; only the fixed public endpoints the site's own
/// frontend uses are called (admin-ajax search, /manga/<slug>/ pages).
pub struct MadaraGeneric {
    id: String,
    display_name: String,
    host: String,
    base: Url,
    http: Arc<dyn Transport>,
}

impl MadaraGeneric {
    pub fn live(display_name: &str, base_url: &str) -> Result<Self, SourceError> {
        let parsed = Url::parse(base_url.trim()).map_err(|_| SourceError::MalformedUrl)?;
        if parsed.scheme() != "https"
            || parsed.port().is_some()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(SourceError::MalformedUrl);
        }
        let host = parsed
            .host_str()
            .ok_or(SourceError::MalformedUrl)?
            .to_lowercase();
        if host.parse::<std::net::IpAddr>().is_ok() || !host.contains('.') {
            return Err(SourceError::MalformedUrl);
        }
        let mut base = parsed.clone();
        base.set_path("/");
        base.set_query(None);
        base.set_fragment(None);
        Ok(MadaraGeneric {
            id: format!("madara-{}", host.replace('.', "-")),
            display_name: display_name.to_string(),
            http: Arc::new(GuardedHttp::new(HttpPolicy {
                allowed_hosts: vec![host.clone()],
            })),
            host,
            base,
        })
    }

    #[cfg(test)]
    pub fn with_transport(
        display_name: &str,
        base_url: &str,
        http: Arc<dyn Transport>,
    ) -> Result<Self, SourceError> {
        let parsed = Url::parse(base_url).map_err(|_| SourceError::MalformedUrl)?;
        let host = parsed
            .host_str()
            .ok_or(SourceError::MalformedUrl)?
            .to_lowercase();
        let mut base = parsed.clone();
        base.set_path("/");
        Ok(MadaraGeneric {
            id: format!("madara-{}", host.replace('.', "-")),
            display_name: display_name.to_string(),
            host,
            base,
            http,
        })
    }

    fn manga_url(&self, slug: &str) -> String {
        format!("{}manga/{slug}/", self.base)
    }

    fn ajax_url(&self) -> String {
        format!("{}wp-admin/admin-ajax.php", self.base)
    }

    fn owns_host(&self, candidate: &str) -> bool {
        candidate == self.host
    }

    fn slug_from_manga_url(&self, raw: &str) -> Option<String> {
        let parsed = raw
            .parse::<Url>()
            .ok()
            .or_else(|| self.base.join(raw).ok())?;
        if !self.owns_host(parsed.host_str()?) {
            return None;
        }
        // Trailing slashes produce an empty final segment; ignore them so
        // "/manga/slug/" is still exactly two meaningful segments.
        let mut segments = parsed
            .path_segments()?
            .filter(|segment| !segment.is_empty());
        if segments.next()? != "manga" {
            return None;
        }
        let slug = segments.next()?;
        // Deeper paths are chapter links, not manga identities.
        if segments.next().is_some() || !valid_slug(slug) {
            return None;
        }
        Some(slug.to_string())
    }

    fn chapter_slug_from_url(&self, href: &str, manga_slug: &str) -> Option<String> {
        let parsed = href
            .parse::<Url>()
            .ok()
            .or_else(|| self.base.join(href).ok())?;
        if !self.owns_host(parsed.host_str()?) {
            return None;
        }
        let mut segments = parsed
            .path_segments()?
            .filter(|segment| !segment.is_empty());
        if segments.next()? != "manga" || segments.next()? != manga_slug {
            return None;
        }
        let chapter = segments.next()?;
        if segments.next().is_some() || !valid_slug(chapter) {
            return None;
        }
        Some(chapter.to_string())
    }
}

fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 140
        && slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn chapter_number_from_slug(slug: &str) -> Option<f64> {
    let re = Regex::new(r"(\d+(?:[.,]\d+)?)$").ok()?;
    let cap = re.captures(slug)?.get(1)?.as_str().to_string();
    cap.replace(',', ".").parse::<f64>().ok()
}

#[async_trait]
impl SourceAdapter for MadaraGeneric {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            capabilities: vec![
                Capability::Search,
                Capability::ResolveUrl,
                Capability::Details,
                Capability::Pages,
            ],
            status: SourceStatus::Ready,
            attribution: Some(format!(
                "Site do bên thứ ba vận hành — nguồn do người dùng đăng ký: {}",
                self.host
            )),
        }
    }

    fn owns_url(&self, url: &Url) -> bool {
        url.host_str() == Some(self.host.as_str()) && url.path().starts_with("/manga/")
    }

    async fn search(&self, query: &str, page: u32) -> Result<RemoteSearchPage, SourceError> {
        if page > 1 {
            // The suggestion endpoint returns a single page of results.
            return Ok(RemoteSearchPage {
                items: vec![],
                has_more: false,
            });
        }
        let body = self
            .http
            .post_form_json(
                &self.ajax_url(),
                &[("action", "wp-manga-search-manga"), ("title", query)],
            )
            .await?;
        let entries = body["data"].as_array().ok_or(SourceError::Parse)?;
        let mut items = Vec::new();
        let mut seen = HashSet::new();
        for entry in entries {
            let Some(raw_url) = entry["url"].as_str() else {
                continue;
            };
            let Some(slug) = self.slug_from_manga_url(raw_url) else {
                continue;
            };
            if !seen.insert(slug.clone()) {
                continue;
            }
            let title = entry["title"]
                .as_str()
                .unwrap_or(slug.as_str())
                .trim()
                .to_string();
            if title.is_empty() {
                continue;
            }
            items.push(RemoteMangaSummary {
                source_id: self.id.clone(),
                source_manga_id: slug,
                source_display_name: self.display_name.clone(),
                title,
                author: None,
                cover_key: None,
                status: "unknown".to_string(),
            });
        }
        Ok(RemoteSearchPage {
            items,
            has_more: false,
        })
    }

    async fn resolve_url(&self, url: &Url) -> Result<String, SourceError> {
        let slug = url
            .path_segments()
            .and_then(|mut segments| {
                segments.next();
                segments.next().map(str::to_string)
            })
            .filter(|candidate| valid_slug(candidate))
            .ok_or(SourceError::MalformedUrl)?;
        Ok(slug)
    }

    async fn page_manifest(
        &self,
        source_manga_id: &str,
        source_chapter_id: &str,
    ) -> Result<PageManifest, SourceError> {
        if !valid_slug(source_manga_id) || !valid_slug(source_chapter_id) {
            return Err(SourceError::MalformedUrl);
        }
        let page_url = format!(
            "{}manga/{}/{}/",
            self.base, source_manga_id, source_chapter_id
        );
        let html = self.http.get_text(&page_url).await?;
        let document = Html::parse_document(&html);
        // Primary dom-hint documented by the site's own theme; the loose
        // fallback keeps plain Madara forks working. A foreign image host
        // fails the whole manifest — the allowlist is never widened silently.
        let strict = Selector::parse(".reading-content img.wp-manga-chapter-img").unwrap();
        let mut urls: Vec<String> = Vec::new();
        collect_page_urls(&document, &strict, &self.base, &self.host, &mut urls)?;
        if urls.is_empty() {
            let loose = Selector::parse(".reading-content img").unwrap();
            collect_page_urls(&document, &loose, &self.base, &self.host, &mut urls)?;
        }
        if urls.is_empty() || urls.len() > MAX_PAGE_COUNT {
            return Err(SourceError::Parse);
        }
        let pages = urls
            .into_iter()
            .enumerate()
            .map(|(index, url)| PageDescriptor { index, url })
            .collect();
        Ok(PageManifest {
            source_chapter_id: source_chapter_id.to_string(),
            pages,
        })
    }

    async fn fetch_image(&self, url: &str) -> Result<Vec<u8>, SourceError> {
        self.http.get_bytes(url, IMAGE_MAX_BYTES).await
    }

    async fn manga_detail(&self, source_manga_id: &str) -> Result<RemoteMangaDetail, SourceError> {
        if !valid_slug(source_manga_id) {
            return Err(SourceError::MalformedUrl);
        }
        let page_url = self.manga_url(source_manga_id);
        let html = self.http.get_text(&page_url).await?;
        let document = Html::parse_document(&html);

        let h1 = Selector::parse("h1").unwrap();
        let raw_title = document
            .select(&h1)
            .next()
            .map(|node| node.text().collect::<String>().trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or(SourceError::Parse)?;
        let title = clean_title(&raw_title);

        let og_image = Selector::parse("meta[property=\"og:image\"]").unwrap();
        let cover_key = document
            .select(&og_image)
            .next()
            .and_then(|node| node.value().attr("content"))
            .and_then(|content| Url::parse(content).ok())
            .map(|parsed| parsed.path().to_string());

        let tag = Selector::parse("a[rel=\"tag\"]").unwrap();
        let mut genres: Vec<String> = Vec::new();
        for node in document.select(&tag) {
            let name = node.text().collect::<String>().trim().to_string();
            if !name.is_empty() && !genres.iter().any(|existing| existing == &name) {
                genres.push(name);
            }
            if genres.len() >= 12 {
                break;
            }
        }

        let chapters_selector =
            Selector::parse("ul.main.version-chap a[href], li.wp-manga-chapter a[href]").unwrap();
        let mut seen: HashSet<String> = HashSet::new();
        let mut chapters: Vec<RemoteChapter> = Vec::new();
        for node in document.select(&chapters_selector) {
            let Some(href) = node.value().attr("href") else {
                continue;
            };
            let Some(slug) = self.chapter_slug_from_url(href, source_manga_id) else {
                continue;
            };
            if !seen.insert(slug.clone()) {
                continue;
            }
            let label = node.text().collect::<String>().trim().to_string();
            let number = chapter_number_from_slug(&slug);
            let chapter_title = if label.is_empty() {
                format!(
                    "Chương {}",
                    number
                        .map(|value| value.to_string())
                        .unwrap_or(slug.clone())
                )
            } else {
                label
            };
            chapters.push(RemoteChapter {
                source_chapter_id: slug,
                title: chapter_title,
                number,
                order: 0,
                published_at: None,
            });
        }
        // Listing pages commonly order newest-first; expose chronological
        // order: numbered chapters ascending by number, unnumbered chapters
        // keep relative order at the end.
        chapters.reverse();
        let mut numbered: Vec<RemoteChapter> = chapters
            .iter()
            .filter(|chapter| chapter.number.is_some())
            .cloned()
            .collect();
        numbered.sort_by(|a, b| {
            a.number
                .partial_cmp(&b.number)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        numbered.extend(
            chapters
                .into_iter()
                .filter(|chapter| chapter.number.is_none()),
        );
        chapters = numbered;
        for (index, chapter) in chapters.iter_mut().enumerate() {
            chapter.order = index as i64;
        }

        Ok(RemoteMangaDetail {
            summary: RemoteMangaSummary {
                source_id: self.id.clone(),
                source_manga_id: source_manga_id.to_string(),
                source_display_name: self.display_name.clone(),
                title,
                author: None,
                cover_key,
                status: detect_status(&html).to_string(),
            },
            description: None,
            genres,
            canonical_url: Some(page_url),
            chapters,
            saved_manga_id: None,
            available_offline: false,
        })
    }
}

fn collect_page_urls(
    document: &Html,
    selector: &Selector,
    base: &Url,
    host: &str,
    out: &mut Vec<String>,
) -> Result<(), SourceError> {
    let mut seen: HashSet<Url> = HashSet::new();
    for node in document.select(selector) {
        let raw = ["src", "data-src", "data-lazy-src", "data-original"]
            .iter()
            .find_map(|attr| node.value().attr(attr))
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let Some(raw) = raw else { continue };
        let joined = base.join(raw).map_err(|_| SourceError::Parse)?;
        if joined.scheme() != "https" || joined.host_str() != Some(host) {
            return Err(SourceError::Parse);
        }
        if seen.insert(joined.clone()) {
            out.push(joined.to_string());
        }
    }
    Ok(())
}

fn clean_title(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix("Đọc truyện ") {
        if let Some(end) = rest.find(" Full") {
            return rest[..end].trim().to_string();
        }
    }
    raw.split(" - ").next().unwrap_or(raw).trim().to_string()
}

fn detect_status(html: &str) -> &'static str {
    let lower = html.to_lowercase();
    if lower.contains("đang tiến hành") || lower.contains("đang ra") || lower.contains("ongoing")
    {
        "ongoing"
    } else if lower.contains("hoàn thành") || lower.contains("completed") {
        "completed"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::http::mock::MockTransport;

    const SEARCH_FIXTURE: &str = r#"{"success":true,"data":[
      {"title":"Ninja Lo\u1ea1n Th\u1ecb","url":"https://truyentuoitho.com/manga/ninja-loan-thi/","type":"manga"},
      {"title":"Tr\u1eabu\u1ed3","url":"https://truyentuoitho.com/manga/trau-boro/","type":"manga"},
      {"title":"Kh\u00f4ng ph\u1ea3i manga","url":"https://truyentuoitho.com/about","type":"page"}
    ]}"#;

    const PAGE_FIXTURE: &str = r#"<html><head>
      <meta property="og:image" content="https://truyentuoitho.com/wp-content/uploads/2024/ninja.jpg"/>
      </head><body class="postid-1901">
      <h1>Đọc truyện Ninja Loạn Thị Full - Truyen tuoi tho</h1>
      <span class="m-genre"><a rel="tag" href="/genres/shounen">Shounen</a> <a rel="tag" href="/genres/comedy">Comedy</a></span>
      <div class="manga-status"><h5>Trạng thái</h5><div class="summary-content">Đang tiến hành</div></div>
      <div class="listing-chapters_wrap"><ul class="main version-chap">
        <li><a href="https://truyentuoitho.com/manga/ninja-loan-thi/tap-65/">Tap 65</a></li>
        <li><a href="https://truyentuoitho.com/manga/ninja-loan-thi/tap-2/">Tap 2</a></li>
        <li><a href="https://truyentuoitho.com/manga/ninja-loan-thi/tap-1/">Chương 1</a></li>
        <li><a href="https://truyentuoitho.com/manga/ninja-loan-thi/extern-special/">Special</a></li>
      </ul></div>
      </body></html>"#;

    fn adapter(
        replies: Vec<Result<&'static str, &'static str>>,
    ) -> (MadaraGeneric, Arc<MockTransport>) {
        let transport = Arc::new(MockTransport::with(replies));
        let provider = MadaraGeneric::with_transport(
            "Truyen Tuoi Tho",
            "https://truyentuoitho.com/",
            transport.clone(),
        )
        .unwrap();
        (provider, transport)
    }

    #[tokio::test]
    async fn search_maps_ajax_results_to_slugs() {
        let (provider, transport) = adapter(vec![Ok(SEARCH_FIXTURE)]);
        let page = provider.search("ninja", 1).await.unwrap();
        assert_eq!(page.items.len(), 2, "non-manga URLs must be filtered");
        assert_eq!(page.items[0].source_manga_id, "ninja-loan-thi");
        assert_eq!(page.items[0].source_id, "madara-truyentuoitho-com");
        assert_eq!(page.items[0].title, "Ninja Loạn Thị");
        let requested = transport.requested.lock().unwrap();
        assert_eq!(requested[0].0, "POST");
        assert!(requested[0].1.ends_with("/wp-admin/admin-ajax.php"));
    }

    #[tokio::test]
    async fn detail_parses_title_cover_genres_status_and_chronological_chapters() {
        let (provider, _) = adapter(vec![Ok(PAGE_FIXTURE)]);
        let detail = provider.manga_detail("ninja-loan-thi").await.unwrap();
        assert_eq!(detail.summary.title, "Ninja Loạn Thị");
        assert_eq!(
            detail.summary.cover_key.as_deref(),
            Some("/wp-content/uploads/2024/ninja.jpg")
        );
        assert_eq!(detail.summary.status, "ongoing");
        assert_eq!(detail.genres, vec!["Shounen", "Comedy"]);
        let ids: Vec<&str> = detail
            .chapters
            .iter()
            .map(|c| c.source_chapter_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec!["tap-1", "tap-2", "tap-65", "extern-special"],
            "numbered chapters ascending by number; unnumbered at the end"
        );
        assert_eq!(detail.chapters[0].order, 0);
        assert_eq!(detail.chapters[0].number, Some(1.0));
        assert!(!detail.available_offline);
        assert_eq!(
            detail.canonical_url.as_deref(),
            Some("https://truyentuoitho.com/manga/ninja-loan-thi/")
        );
    }

    #[test]
    fn ownership_requires_exact_host_and_manga_path() {
        let (provider, _) = adapter(vec![]);
        assert!(provider.owns_url(&Url::parse("https://truyentuoitho.com/manga/x/").unwrap()));
        assert!(!provider
            .owns_url(&Url::parse("https://truyentuoitho.com.evil.test/manga/x/").unwrap()));
        assert!(!provider.owns_url(&Url::parse("https://www.truyentuoitho.com/manga/x/").unwrap()));
        assert!(!provider.owns_url(&Url::parse("https://truyentuoitho.com/other").unwrap()));
    }

    #[test]
    fn live_constructor_rejects_unsafe_base_urls() {
        assert!(MadaraGeneric::live("x", "http://a.example.com").is_err());
        assert!(MadaraGeneric::live("x", "https://127.0.0.1").is_err());
        assert!(MadaraGeneric::live("x", "https://user@pass.example.org").is_err());
        assert!(MadaraGeneric::live("x", "https://localhost:8443").is_err());
        assert!(MadaraGeneric::live("x", "https://example.com").is_ok());
    }

    const CHAPTER_FIXTURE: &str = r#"<html><body>
      <div class="reading-content">
        <img class="wp-manga-chapter-img" src="  /uploads/p1.jpg  "/>
        <img class="wp-manga-chapter-img" data-src="https://truyentuoitho.com/uploads/p2.jpg"/>
        <img class="wp-manga-chapter-img" src="https://truyentuoitho.com/uploads/p2.jpg"/>
      </div></body></html>"#;

    #[tokio::test]
    async fn page_manifest_is_ordered_absolute_https_and_deduped() {
        let (provider, transport) = adapter(vec![Ok(CHAPTER_FIXTURE)]);
        let manifest = provider
            .page_manifest("ninja-loan-thi", "tap-1")
            .await
            .unwrap();
        assert_eq!(manifest.source_chapter_id, "tap-1");
        assert_eq!(manifest.pages.len(), 2, "duplicate p2 must collapse");
        assert_eq!(manifest.pages[0].index, 0);
        assert_eq!(
            manifest.pages[0].url, "https://truyentuoitho.com/uploads/p1.jpg",
            "relative src resolved against base and trimmed"
        );
        assert_eq!(
            manifest.pages[1].url,
            "https://truyentuoitho.com/uploads/p2.jpg"
        );
        let requested = transport.requested.lock().unwrap();
        assert!(requested[0].1.ends_with("/manga/ninja-loan-thi/tap-1/"));
    }

    #[tokio::test]
    async fn page_manifest_rejects_foreign_image_host() {
        let (provider, _) = adapter(vec![Ok(
            r#"<div class="reading-content"><img class="wp-manga-chapter-img" src="https://evil.test/x.jpg"/></div>"#,
        )]);
        assert!(matches!(
            provider.page_manifest("m", "tap-1").await,
            Err(SourceError::Parse)
        ));
    }

    #[tokio::test]
    async fn page_manifest_rejects_bad_slugs_and_empty_listing() {
        let (provider, _) = adapter(vec![Ok("<html><body>none</body></html>")]);
        assert!(matches!(
            provider.page_manifest("m", "../etc").await,
            Err(SourceError::MalformedUrl)
        ));
        assert!(matches!(
            provider.page_manifest("m", "tap-1").await,
            Err(SourceError::Parse)
        ));
    }
}
