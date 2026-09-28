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
use crate::sources::http::{
    GuardedHttp, HttpPolicy, Transport, COVER_MAX_BYTES, IMAGE_MAX_BYTES, MAX_PAGE_COUNT,
};

/// Generic WordPress/Madara adapter. Users register any Madara-based site by
/// name + https base URL; only the fixed public endpoints the site's own
/// frontend uses are called (the `?s=` search page, /manga/<slug>/ pages).
/// Chapter images may legitimately live on a separate CDN host, but ONLY one
/// the user explicitly registered per source (`image_hosts`) — the transport
/// allowlist stays exact-host and fail-closed (no wildcards, no IPs, cap 4).
/// Layout/selector profile for the shared HTML pipeline. `Madara` is the
/// user-registration profile (WordPress/Madara theme conventions).
/// `TruyenQQ` is an app-vetted built-in layout: manga live at root
/// `/<slug>`, chapters at `/<slug>/chapter-<n>`, search at
/// `/tim-kiem?s=`, images lazy-loaded from a sharded CDN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteProfile {
    Madara,
    TruyenQQ,
}

/// TruyenQQ serves chapter images from rotating shards of this CDN domain.
/// A domain (not exact-host) allowlist is only ever set by built-in code —
/// user-registered sources keep the exact-host fail-closed policy.
const TRUYENQQ_IMAGE_DOMAIN: &str = "cc3t.net";
/// Site sections whose root-level paths are never manga slugs.
const TRUYENQQ_RESERVED: &[&str] = &[
    "tim-kiem",
    "truyen-hot",
    "the-loai",
    "tac-gia",
    "dang-nhap",
    "dang-ky",
    "doi-mat-kha",
    "gioi-thieu",
    "lien-he",
    "bao-tai",
    "lich-su",
    "trang-thai",
    "contact",
];

pub struct MadaraGeneric {
    id: String,
    display_name: String,
    host: String,
    base: Url,
    image_hosts: Vec<String>,
    profile: SiteProfile,
    http: Arc<dyn Transport>,
}

/// Normalize one exact image-host allowance: bare lowercase hostname,
/// dotted, no IP literal, no scheme/port/path, bounded length.
pub fn normalize_image_host(raw: &str) -> Option<String> {
    let candidate = raw.trim().to_lowercase();
    if candidate.is_empty()
        || candidate.len() > 253
        || candidate.parse::<std::net::IpAddr>().is_ok()
        || !candidate.contains('.')
        || candidate
            .chars()
            .any(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.'))
    {
        return None;
    }
    let valid_segments = candidate.split('.').all(|segment| {
        !segment.is_empty()
            && !segment.starts_with('-')
            && !segment.ends_with('-')
            && segment.len() <= 63
    });
    valid_segments.then_some(candidate)
}

impl MadaraGeneric {
    /// User-registration entry point: identity is derived from the host.
    pub fn live(
        display_name: &str,
        base_url: &str,
        image_hosts: &[String],
    ) -> Result<Self, SourceError> {
        let derived = Url::parse(base_url.trim()).ok().and_then(|parsed| {
            parsed
                .host_str()
                .map(|host| format!("madara-{}", host.replace('.', "-")))
        });
        let id = derived.ok_or(SourceError::MalformedUrl)?;
        Self::with_profile(
            &id,
            display_name,
            base_url,
            image_hosts,
            SiteProfile::Madara,
        )
    }

    /// Built-in entry point: the id is app-fixed and STAYS STABLE when the
    /// domain is edited later, so saved manga keep resolving to this source.
    pub fn built_in(
        id: &str,
        display_name: &str,
        base_url: &str,
        image_hosts: &[String],
        profile: SiteProfile,
    ) -> Result<Self, SourceError> {
        Self::with_profile(id, display_name, base_url, image_hosts, profile)
    }

    fn with_profile(
        id: &str,
        display_name: &str,
        base_url: &str,
        image_hosts: &[String],
        profile: SiteProfile,
    ) -> Result<Self, SourceError> {
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
        if image_hosts.len() > 4 {
            return Err(SourceError::MalformedUrl);
        }
        let mut extras: Vec<String> = Vec::new();
        for extra in image_hosts {
            let normalized = normalize_image_host(extra).ok_or(SourceError::MalformedUrl)?;
            if normalized == host
                || normalized.ends_with(&format!(".{host}"))
                || extras.contains(&normalized)
            {
                // Subdomains of the base are already pointless-but-safe to
                // drop; duplicates and the base itself never need an entry.
                continue;
            }
            extras.push(normalized);
        }
        let mut base = parsed.clone();
        base.set_path("/");
        base.set_query(None);
        base.set_fragment(None);
        let mut allowed_hosts = vec![host.clone()];
        allowed_hosts.extend(extras.iter().cloned());
        let http = Arc::new(GuardedHttp::new(match profile {
            SiteProfile::Madara => HttpPolicy {
                allowed_hosts,
                allowed_domains: Vec::new(),
            },
            SiteProfile::TruyenQQ => HttpPolicy {
                allowed_hosts,
                allowed_domains: vec![TRUYENQQ_IMAGE_DOMAIN.to_string()],
            },
        }));
        Ok(MadaraGeneric {
            id: id.to_string(),
            display_name: display_name.to_string(),
            http,
            host,
            base,
            image_hosts: extras,
            profile,
        })
    }

    /// Every hostname chapter images may come from: base + registered extras.
    fn allowed_image_hosts(&self) -> Vec<String> {
        let mut allowed = vec![self.host.clone()];
        allowed.extend(self.image_hosts.iter().cloned());
        allowed
    }

    /// Mint a cover key (path-only, base host only — theme thumbnails ship
    /// with the site; CDN allowlist hosts serve chapter pages). A key never
    /// carries scheme or host, so nothing fetchable leaks to the renderer.
    fn cover_key_from_src(&self, raw: &str) -> Option<String> {
        let parsed = self.base.join(raw.trim()).ok()?;
        if parsed.scheme() != "https" || parsed.host_str() != Some(self.host.as_str()) {
            return None;
        }
        let path = parsed.path();
        (!path.starts_with("//") && !path.is_empty() && path.len() <= 480).then(|| path.to_string())
    }

    /// Re-derive the cover URL from a stored key, re-checking that joining
    /// cannot escape the base host (dot segments, query, fragment, protocol
    /// overrides all fail here).
    fn cover_url_from_key(&self, cover_key: &str) -> Result<String, SourceError> {
        let joined = self
            .base
            .join(cover_key)
            .map_err(|_| SourceError::MalformedUrl)?;
        if joined.scheme() != "https"
            || joined.host_str() != Some(self.host.as_str())
            || joined.path() != cover_key
        {
            return Err(SourceError::MalformedUrl);
        }
        Ok(joined.to_string())
    }

    #[cfg(test)]
    pub fn with_transport(
        display_name: &str,
        base_url: &str,
        http: Arc<dyn Transport>,
    ) -> Result<Self, SourceError> {
        Self::with_transport_and_image_hosts(display_name, base_url, http, &[])
    }

    #[cfg(test)]
    pub fn with_transport_and_image_hosts(
        display_name: &str,
        base_url: &str,
        http: Arc<dyn Transport>,
        image_hosts: &[String],
    ) -> Result<Self, SourceError> {
        Self::with_transport_profile_and_hosts(
            display_name,
            base_url,
            http,
            image_hosts,
            SiteProfile::Madara,
        )
    }

    #[cfg(test)]
    pub fn with_transport_profile(
        display_name: &str,
        base_url: &str,
        http: Arc<dyn Transport>,
        profile: SiteProfile,
    ) -> Result<Self, SourceError> {
        Self::with_transport_profile_and_hosts(display_name, base_url, http, &[], profile)
    }

    #[cfg(test)]
    fn with_transport_profile_and_hosts(
        display_name: &str,
        base_url: &str,
        http: Arc<dyn Transport>,
        image_hosts: &[String],
        profile: SiteProfile,
    ) -> Result<Self, SourceError> {
        let parsed = Url::parse(base_url).map_err(|_| SourceError::MalformedUrl)?;
        let host = parsed
            .host_str()
            .ok_or(SourceError::MalformedUrl)?
            .to_lowercase();
        let mut base = parsed.clone();
        base.set_path("/");
        let id = match profile {
            SiteProfile::Madara => format!("madara-{}", host.replace('.', "-")),
            SiteProfile::TruyenQQ => "truyenqq".to_string(),
        };
        Ok(MadaraGeneric {
            id,
            display_name: display_name.to_string(),
            host,
            base,
            image_hosts: image_hosts.to_vec(),
            profile,
            http,
        })
    }

    fn manga_url(&self, slug: &str) -> String {
        match self.profile {
            SiteProfile::Madara => format!("{}manga/{slug}/", self.base),
            SiteProfile::TruyenQQ => format!("{}{slug}", self.base),
        }
    }

    fn chapter_url(&self, manga_slug: &str, chapter_slug: &str) -> String {
        match self.profile {
            SiteProfile::Madara => format!("{}manga/{}/{}/", self.base, manga_slug, chapter_slug),
            SiteProfile::TruyenQQ => format!("{}{}/{}", self.base, manga_slug, chapter_slug),
        }
    }

    /// Detail-page fetch URL. TruyenQQ ships ~70 latest chapters inline;
    /// ANY `?page-chap=` request renders the FULL list (live-verified
    /// 2026-09-28), so the built-in always asks for page 1.
    fn detail_chapter_list_url(&self, slug: &str) -> String {
        match self.profile {
            SiteProfile::Madara => self.manga_url(slug),
            SiteProfile::TruyenQQ => format!("{}{slug}?page-chap=1", self.base),
        }
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
        let slug = match self.profile {
            SiteProfile::Madara => {
                if segments.next()? != "manga" {
                    return None;
                }
                segments.next()?
            }
            SiteProfile::TruyenQQ => {
                let slug = segments.next()?;
                if TRUYENQQ_RESERVED.contains(&slug) {
                    return None;
                }
                slug
            }
        };
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
        match self.profile {
            SiteProfile::Madara => {
                if segments.next()? != "manga" || segments.next()? != manga_slug {
                    return None;
                }
            }
            SiteProfile::TruyenQQ => {
                if segments.next()? != manga_slug {
                    return None;
                }
            }
        }
        let chapter = segments.next()?;
        if segments.next().is_some() || !valid_slug(chapter) {
            return None;
        }
        Some(chapter.to_string())
    }

    /// One search-results card (any Madara container variant) → summary.
    /// Non-manga links, duplicate slugs and foreign-host thumbnails are
    /// dropped; the cover stays a path key on the base host only.
    fn collect_cards(
        &self,
        document: &Html,
        containers: &Selector,
        title_link: &Selector,
        thumb: &Selector,
        seen: &mut HashSet<String>,
        items: &mut Vec<RemoteMangaSummary>,
    ) {
        for node in document.select(containers) {
            let Some(link) = node.select(title_link).next() else {
                continue;
            };
            let Some(href) = link.value().attr("href") else {
                continue;
            };
            let Some(slug) = self.slug_from_manga_url(href) else {
                continue;
            };
            if !seen.insert(slug.clone()) {
                continue;
            }
            let raw_title = link.text().collect::<String>().trim().to_string();
            let title = if raw_title.is_empty() {
                slug.clone()
            } else {
                raw_title
            };
            let cover_key = node.select(thumb).find_map(|image| {
                ["src", "data-src", "data-lazy-src", "data-original"]
                    .iter()
                    .find_map(|attr| image.value().attr(attr))
                    .and_then(|raw| self.cover_key_from_src(raw))
            });
            items.push(RemoteMangaSummary {
                source_id: self.id.clone(),
                source_manga_id: slug,
                source_display_name: self.display_name.clone(),
                title,
                author: None,
                cover_key,
                status: "unknown".to_string(),
            });
            if items.len() >= 50 {
                return;
            }
        }
    }

    /// Extract chapter links from any fragment (page or AJAX markup). Only
    /// same-manga, same-host slugs survive — sidebar/related links for other
    /// series are filtered out by `chapter_slug_from_url`.
    fn parse_chapter_nodes(&self, document: &Html, source_manga_id: &str) -> Vec<RemoteChapter> {
        let chapters_selector = Selector::parse(match self.profile {
            SiteProfile::Madara => {
                "ul.main.version-chap a[href], li.wp-manga-chapter a[href], div.chapter-item a[href]"
            }
            SiteProfile::TruyenQQ => {
                "#chapter-list a.chapter-name[href], .reading-list .item-name a[href]"
            }
        })
        .unwrap();
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
        chapters
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
                "Site do bên thứ ba vận hành — {}: {}",
                if crate::sources::registry::is_builtin(&self.id) {
                    "nguồn tích hợp sẵn"
                } else {
                    "nguồn do người dùng đăng ký"
                },
                self.host
            )),
        }
    }

    fn owns_url(&self, url: &Url) -> bool {
        if url.host_str() != Some(self.host.as_str()) {
            return false;
        }
        match self.profile {
            SiteProfile::Madara => url.path().starts_with("/manga/"),
            SiteProfile::TruyenQQ => {
                // Root-level single or double segment paths are manga/chapter
                // links; reserved site sections never are.
                let mut segments = url
                    .path_segments()
                    .into_iter()
                    .flatten()
                    .filter(|s| !s.is_empty());
                let Some(first) = segments.next() else {
                    return false;
                };
                if TRUYENQQ_RESERVED.contains(&first) || !valid_slug(first) {
                    return false;
                }
                match segments.next() {
                    None => true,
                    Some(chapter) => segments.next().is_none() && valid_slug(chapter),
                }
            }
        }
    }

    fn chapter_from_url(&self, url: &Url) -> Option<(String, String)> {
        if !self.owns_url(url) {
            return None;
        }
        let mut segments = url.path_segments()?.filter(|segment| !segment.is_empty());
        let manga = match self.profile {
            SiteProfile::Madara => {
                if segments.next()? != "manga" {
                    return None;
                }
                segments.next()?
            }
            SiteProfile::TruyenQQ => segments.next()?,
        };
        let chapter = segments.next()?;
        // Exactly <manga>/<chapter> — deeper paths and bad slugs are not
        // chapter links.
        if segments.next().is_some() || !valid_slug(manga) || !valid_slug(chapter) {
            return None;
        }
        Some((manga.to_string(), chapter.to_string()))
    }

    async fn search(&self, query: &str, page: u32) -> Result<RemoteSearchPage, SourceError> {
        let page = page.max(1);
        let encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
        let search_url = match self.profile {
            SiteProfile::Madara => {
                // The theme's own search results page — the admin-ajax suggest
                // endpoint it replaced ships titles only, while every card here
                // carries the cover thumbnail. `paged` is standard WP pagination.
                if page == 1 {
                    format!("{}?s={}&post_type=wp-manga", self.base, encoded)
                } else {
                    format!(
                        "{}?s={}&post_type=wp-manga&paged={}",
                        self.base, encoded, page
                    )
                }
            }
            SiteProfile::TruyenQQ => {
                // The site's own search form: GET /tim-kiem with `s=`.
                if page == 1 {
                    format!("{}tim-kiem?s={}", self.base, encoded)
                } else {
                    format!("{}tim-kiem?s={}&page={}", self.base, encoded, page)
                }
            }
        };
        let html = self.http.get_text(&search_url).await?;
        let document = Html::parse_document(&html);
        let mut items: Vec<RemoteMangaSummary> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        match self.profile {
            SiteProfile::Madara => {
                // Primary: the tabbed search layout most Madara sites ship.
                self.collect_cards(
                    &document,
                    &Selector::parse("div.row.c-tabs-item__content").unwrap(),
                    &Selector::parse("div.post-title h3 a[href], div.post-title a[href]").unwrap(),
                    &Selector::parse("div.tab-thumb img, .c-image img").unwrap(),
                    &mut seen,
                    &mut items,
                );
                if items.is_empty() {
                    // Fallback: grid forks (.page-item-detail). Scoped to the two
                    // listing wrappers so sidebar "popular" widgets can never
                    // masquerade as search results.
                    self.collect_cards(
                        &document,
                        &Selector::parse(
                            ".page-content-listing .page-item-detail, .tab-content-wrap .page-item-detail",
                        )
                        .unwrap(),
                        &Selector::parse("div.post-title a[href], h3 a[href]").unwrap(),
                        &Selector::parse(".poster img, .c-image img, img").unwrap(),
                        &mut seen,
                        &mut items,
                    );
                }
            }
            SiteProfile::TruyenQQ => {
                // Results are `.item` cards inside `.listing` only — the
                // homepage-style popular/genres blocks outside it never count.
                self.collect_cards(
                    &document,
                    &Selector::parse(".listing .item").unwrap(),
                    &Selector::parse("div.info h3 a[href]").unwrap(),
                    &Selector::parse("div.cover img").unwrap(),
                    &mut seen,
                    &mut items,
                );
            }
        }
        let has_more = match self.profile {
            SiteProfile::Madara => document
                .select(
                    &Selector::parse("a.next.page-numbers, .c-pagination a[href*='paged']")
                        .unwrap(),
                )
                .next()
                .is_some(),
            SiteProfile::TruyenQQ => document
                .select(&Selector::parse(".pagination a[href*='page']").unwrap())
                .next()
                .is_some(),
        };
        Ok(RemoteSearchPage { items, has_more })
    }

    async fn resolve_url(&self, url: &Url) -> Result<String, SourceError> {
        let slug = match self.profile {
            SiteProfile::Madara => url.path_segments().and_then(|mut segments| {
                segments.next();
                segments.next().map(str::to_string)
            }),
            SiteProfile::TruyenQQ => url
                .path_segments()
                .and_then(|mut segments| segments.next().map(str::to_string)),
        }
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
        let page_url = self.chapter_url(source_manga_id, source_chapter_id);
        let html = self.http.get_text(&page_url).await?;
        let document = Html::parse_document(&html);
        let mut urls: Vec<String> = Vec::new();
        if self.profile == SiteProfile::TruyenQQ {
            // Lazy reader: real pages are img[data-src] on the reviewed CDN
            // domain; plain-src images are ads and never become pages.
            let lazy = Selector::parse("img[data-src]").unwrap();
            let mut seen: HashSet<Url> = HashSet::new();
            for node in document.select(&lazy) {
                let Some(raw) = node
                    .value()
                    .attr("data-src")
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                else {
                    continue;
                };
                let Ok(joined) = self.base.join(raw) else {
                    continue;
                };
                if joined.scheme() != "https"
                    || !joined.host_str().is_some_and(|host| {
                        host == TRUYENQQ_IMAGE_DOMAIN
                            || host.ends_with(&format!(".{TRUYENQQ_IMAGE_DOMAIN}"))
                    })
                {
                    continue;
                }
                if seen.insert(joined.clone()) {
                    urls.push(joined.to_string());
                }
                if urls.len() >= MAX_PAGE_COUNT {
                    break;
                }
            }
        } else {
            // Primary dom-hint documented by the site's own theme; the page-box
            // variant covers the common "chapter image on a registered CDN host"
            // layout; the loose fallback keeps plain Madara forks working. An
            // image host outside the source's explicit allowlist fails the WHOLE
            // manifest — it is never widened silently.
            let allowed = self.allowed_image_hosts();
            let strict = Selector::parse(".reading-content img.wp-manga-chapter-img").unwrap();
            collect_page_urls(&document, &strict, &self.base, &allowed, &mut urls)?;
            if urls.is_empty() {
                let boxed =
                    Selector::parse(".reading-content .page-box img, .page-box img.page-image")
                        .unwrap();
                collect_page_urls(&document, &boxed, &self.base, &allowed, &mut urls)?;
            }
            if urls.is_empty() {
                let loose = Selector::parse(".reading-content img").unwrap();
                collect_page_urls(&document, &loose, &self.base, &allowed, &mut urls)?;
            }
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

    async fn fetch_cover(
        &self,
        _source_manga_id: &str,
        cover_key: &str,
    ) -> Result<Vec<u8>, SourceError> {
        let url = self.cover_url_from_key(cover_key)?;
        self.http.get_bytes(&url, COVER_MAX_BYTES).await
    }

    async fn manga_detail(&self, source_manga_id: &str) -> Result<RemoteMangaDetail, SourceError> {
        if !valid_slug(source_manga_id) {
            return Err(SourceError::MalformedUrl);
        }
        let page_url = self.detail_chapter_list_url(source_manga_id);
        let html = self.http.get_text(&page_url).await?;
        // Madara "load chapters via AJAX" pattern: the page ships only an
        // empty #manga-chapters-holder. The site's own frontend POSTs to
        // {manga_url}ajax/chapters/; the fragment uses the same markup.
        // The request must happen BEFORE any Html value exists (scraper's
        // document is !Send and must not live across an await).
        let ajax_fragment: Option<String> = if html.contains("manga-chapters-holder") {
            let ajax_url = format!("{}ajax/chapters/", page_url);
            self.http.post_form_text(&ajax_url, &[]).await.ok()
        } else {
            None
        };
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
            .and_then(|content| self.cover_key_from_src(content));

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

        let mut chapters = self.parse_chapter_nodes(&document, source_manga_id);
        if let Some(fragment) = &ajax_fragment {
            let doc = Html::parse_document(fragment);
            let via_ajax = self.parse_chapter_nodes(&doc, source_manga_id);
            // Prefer whichever list is richer — sidebar links that happen to
            // match this manga must not beat the real chapter list.
            if via_ajax.len() >= chapters.len() {
                chapters = via_ajax;
            }
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
    allowed_hosts: &[String],
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
        if joined.scheme() != "https"
            || !joined
                .host_str()
                .is_some_and(|host| allowed_hosts.iter().any(|allowed| allowed == host))
        {
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

    const SEARCH_FIXTURE: &str = r#"<html><body>
      <div class="row c-tabs-item__content">
        <div class="tab-thumb c-image-hover"><a href="https://truyentuoitho.com/manga/ninja-loan-thi/" title="Ninja Loạn Thị">
          <img width="193" height="278" src="https://truyentuoitho.com/wp-content/uploads/2024/05/ninja-193x278.jpg" class="img-responsive" alt="Ninja"/>
        </a></div>
        <div class="tab-summary"><div class="post-title"><h3 class="h4">
          <a href="https://truyentuoitho.com/manga/ninja-loan-thi/">Ninja Loạn Thị</a>
        </h3></div></div>
      </div>
      <div class="row c-tabs-item__content">
        <div class="tab-thumb c-image-hover"><a href="https://truyentuoitho.com/manga/trau-boro/">
          <img data-src="https://cdn-other.example/trau.jpg" class="img-responsive" alt="Trấu"/>
        </a></div>
        <div class="tab-summary"><div class="post-title"><h3 class="h4">
          <a href="https://truyentuoitho.com/manga/trau-boro/">Trấu BorO</a>
        </h3></div></div>
      </div>
      <div class="row c-tabs-item__content">
        <div class="tab-summary"><div class="post-title"><h3 class="h4">
          <a href="https://truyentuoitho.com/about">Không phải manga</a>
        </h3></div></div>
      </div>
      <div class="c-pagination"><ul class="page-numbers"><li>
        <a class="next page-numbers" href="https://truyentuoitho.com/?s=ninja&amp;post_type=wp-manga&amp;paged=2">→</a>
      </li></ul></div>
      </body></html>"#;

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
    async fn search_maps_html_cards_with_cover_keys() {
        let (provider, transport) = adapter(vec![Ok(SEARCH_FIXTURE)]);
        let page = provider.search("ninja", 1).await.unwrap();
        assert_eq!(page.items.len(), 2, "non-manga rows must be filtered");
        assert_eq!(page.items[0].source_manga_id, "ninja-loan-thi");
        assert_eq!(page.items[0].source_id, "madara-truyentuoitho-com");
        assert_eq!(page.items[0].title, "Ninja Loạn Thị");
        assert_eq!(
            page.items[0].cover_key.as_deref(),
            Some("/wp-content/uploads/2024/05/ninja-193x278.jpg")
        );
        assert_eq!(
            page.items[1].cover_key, None,
            "thumbnails hosted outside the base host are dropped"
        );
        assert!(page.has_more, "next-page marker present in fixture");
        let requested = transport.requested.lock().unwrap();
        assert_eq!(requested[0].0, "GET-TEXT");
        assert_eq!(
            requested[0].1,
            "https://truyentuoitho.com/?s=ninja&post_type=wp-manga"
        );
    }

    #[tokio::test]
    async fn search_encodes_query_and_pagination() {
        let (provider, transport) = adapter(vec![Ok(SEARCH_FIXTURE)]);
        provider.search("đô thị", 2).await.unwrap();
        let requested = transport.requested.lock().unwrap();
        assert!(
            requested[0]
                .1
                .ends_with("?s=%C4%91%C3%B4+th%E1%BB%8B&post_type=wp-manga&paged=2"),
            "url was {}",
            requested[0].1
        );
    }

    const SEARCH_GRID_FIXTURE: &str = r#"<html><body>
      <div class="page-content-listing single-page">
        <div class="row page-item-detail">
          <div class="poster"><a href="https://truyentuoitho.com/manga/dao-hai-tac/">
            <img src="https://truyentuoitho.com/wp-content/dht-193x278.jpg"/>
          </a></div>
          <h3 class="h4"><a href="https://truyentuoitho.com/manga/dao-hai-tac/">Đảo Hải Tặc</a></h3>
        </div>
        <div class="row page-item-detail">
          <h3 class="h4"><a href="https://truyentuoitho.com/manga/ninja-loan-thi/">Ninja Loạn Thị</a></h3>
        </div>
      </div>
      <div class="widget c-popular"><div class="row page-item-detail">
        <h3 class="h4"><a href="https://truyentuoitho.com/manga/sidebar-spam/">Không phải kết quả</a></h3>
      </div></div>
      </body></html>"#;

    #[tokio::test]
    async fn search_falls_back_to_grid_variant_without_sidebar_pollution() {
        let (provider, _) = adapter(vec![Ok(SEARCH_GRID_FIXTURE)]);
        let page = provider.search("dao", 1).await.unwrap();
        let slugs: Vec<&str> = page
            .items
            .iter()
            .map(|item| item.source_manga_id.as_str())
            .collect();
        assert_eq!(
            slugs,
            vec!["dao-hai-tac", "ninja-loan-thi"],
            "grid containers parse; popular-widget copy is excluded"
        );
        assert_eq!(
            page.items[0].cover_key.as_deref(),
            Some("/wp-content/dht-193x278.jpg")
        );
        assert_eq!(page.items[1].cover_key, None);
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn fetch_cover_requests_base_host_path_within_cap() {
        let (provider, transport) = adapter(vec![Ok("cover-bytes")]);
        let bytes = provider
            .fetch_cover("ninja-loan-thi", "/wp-content/uploads/2024/ninja.jpg")
            .await
            .unwrap();
        assert_eq!(bytes, b"cover-bytes".to_vec());
        let requested = transport.requested.lock().unwrap();
        assert_eq!(
            requested[0],
            (
                "GET-BYTES".to_string(),
                "https://truyentuoitho.com/wp-content/uploads/2024/ninja.jpg".to_string()
            )
        );
    }

    #[tokio::test]
    async fn fetch_cover_rejects_keys_that_escape_the_base_host() {
        let (provider, transport) = adapter(vec![]);
        for key in [
            "//evil.com/x.jpg",
            "https://evil.com/x.jpg",
            "/../secrets",
            "/ok/../../etc/passwd",
            "?next=x",
            "#frag",
            "",
        ] {
            assert!(
                provider.fetch_cover("m", key).await.is_err(),
                "key {key:?} must be rejected"
            );
        }
        assert!(
            transport.requested.lock().unwrap().is_empty(),
            "rejected keys must never reach the transport"
        );
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
    fn chapter_from_url_maps_wellformed_chapter_links_only() {
        let (provider, _) = adapter(vec![]);
        let ok = |u: &str| provider.chapter_from_url(&Url::parse(u).unwrap());
        assert_eq!(
            ok("https://truyentuoitho.com/manga/ninja-loan-thi/tap-1/"),
            Some(("ninja-loan-thi".to_string(), "tap-1".to_string()))
        );
        assert_eq!(
            ok("https://truyentuoitho.com/manga/ninja-loan-thi/tap-1"),
            Some(("ninja-loan-thi".to_string(), "tap-1".to_string())),
            "trailing slash optional"
        );
        assert!(
            ok("https://truyentuoitho.com/manga/ninja-loan-thi/tap-1/?x=1").is_some(),
            "query strings do not affect the path grammar"
        );
        assert!(ok("https://truyentuoitho.com/manga/ninja-loan-thi/").is_none());
        assert!(ok("https://truyentuoitho.com/manga/ninja-loan-thi/tap-1/extra/").is_none());
        assert!(ok("https://evil.test/manga/ninja-loan-thi/tap-1/").is_none());
        assert!(
            ok("https://truyentuoitho.com/manga/ninja-loan-thi/tap%201/").is_none(),
            "space in slug is not a valid slug"
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

    const CHAPTER_ITEM_FIXTURE: &str = r#"<html><body>
      <h1>Ninja Loạn Thị</h1>
      <div class="list-chapter">
        <div class="chapter-item"><span class="chapter font-meta">
          <a href="https://truyentuoitho.com/manga/ninja-loan-thi/tap-65/">Tập 65</a>
        </span></div>
      </div>
      <div class="list-chapter">
        <div class="chapter-item"><span class="chapter font-meta">
          <a href="/manga/ninja-loan-thi/tap-1/">Tập 1</a>
        </span></div>
      </div></body></html>"#;

    #[tokio::test]
    async fn detail_parses_chapter_item_theme_and_orders_numbered_first() {
        let (provider, _) = adapter(vec![Ok(CHAPTER_ITEM_FIXTURE)]);
        let detail = provider.manga_detail("ninja-loan-thi").await.unwrap();
        let ids: Vec<&str> = detail
            .chapters
            .iter()
            .map(|c| c.source_chapter_id.as_str())
            .collect();
        assert_eq!(ids, vec!["tap-1", "tap-65"]);
    }

    const HOLDER_PAGE_FIXTURE: &str = r#"<html><body><h1>Ninja Loạn Thị</h1>
      <div id="manga-chapters-holder" data-id="1901"><i class="fa-spin"></i></div>
      <div class="chapter-item"><a href="https://truyentuoitho.com/manga/other-series/tap-9/">sidebar</a></div>
    </body></html>"#;

    const CHAPTERS_FRAGMENT_FIXTURE: &str = r#"<div class="listing-chapters_wrap"><ul class="main version-chap">
      <li class="wp-manga-chapter"><a href="https://truyentuoitho.com/manga/ninja-loan-thi/tap-2/">Tập 2</a></li>
      <li class="wp-manga-chapter"><a href="https://truyentuoitho.com/manga/ninja-loan-thi/tap-1/">Tập 1</a></li>
    </ul></div>"#;

    #[tokio::test]
    async fn holder_page_loads_full_list_via_ajax_chapters_post() {
        let (provider, transport) =
            adapter(vec![Ok(HOLDER_PAGE_FIXTURE), Ok(CHAPTERS_FRAGMENT_FIXTURE)]);
        let detail = provider.manga_detail("ninja-loan-thi").await.unwrap();
        let ids: Vec<&str> = detail
            .chapters
            .iter()
            .map(|c| c.source_chapter_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec!["tap-1", "tap-2"],
            "sidebar other-series link filtered"
        );
        let requested = transport.requested.lock().unwrap();
        assert_eq!(requested[1].0, "POST-TEXT");
        assert!(requested[1]
            .1
            .ends_with("/manga/ninja-loan-thi/ajax/chapters/"));
    }

    const PAGE_BOX_FIXTURE: &str = r#"<html><body><div class="reading-content">
      <div class="page-box"><img src="https://img.resourcehub.shop/ninja/0001.webp"/></div>
      <div class="page-box"><img src="https://img.resourcehub.shop/ninja/0002.webp"/></div>
    </div></body></html>"#;

    fn adapter_with_hosts(
        replies: Vec<Result<&'static str, &'static str>>,
        hosts: &[&str],
    ) -> MadaraGeneric {
        let transport = Arc::new(MockTransport::with(replies));
        MadaraGeneric::with_transport_and_image_hosts(
            "Truyen Tuoi Tho",
            "https://truyentuoitho.com/",
            transport,
            &hosts.iter().map(|h| h.to_string()).collect::<Vec<String>>(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn manifest_serves_pages_from_registered_extra_image_host() {
        let provider = adapter_with_hosts(vec![Ok(PAGE_BOX_FIXTURE)], &["img.resourcehub.shop"]);
        let manifest = provider
            .page_manifest("ninja-loan-thi", "tap-1")
            .await
            .unwrap();
        assert_eq!(manifest.pages.len(), 2);
        assert!(manifest.pages[0]
            .url
            .starts_with("https://img.resourcehub.shop/"));
    }

    #[tokio::test]
    async fn manifest_without_the_image_host_registered_stays_fail_closed() {
        let provider = adapter_with_hosts(vec![Ok(PAGE_BOX_FIXTURE)], &[]);
        let error = provider
            .page_manifest("ninja-loan-thi", "tap-1")
            .await
            .unwrap_err();
        assert!(matches!(error, SourceError::Parse));
    }

    #[test]
    fn normalize_image_host_accepts_bare_hosts_and_rejects_everything_else() {
        assert_eq!(
            normalize_image_host(" Img.ResourceHub.shop ").as_deref(),
            Some("img.resourcehub.shop")
        );
        assert_eq!(normalize_image_host("1.2.3.4"), None);
        assert_eq!(normalize_image_host("https://cdn.example"), None);
        assert_eq!(normalize_image_host("cdn.example:443"), None);
        assert_eq!(normalize_image_host("cdn_example.com"), None);
        assert_eq!(normalize_image_host("-bad.example"), None);
        assert_eq!(normalize_image_host("bad..example"), None);
        assert_eq!(normalize_image_host("nodot"), None);
    }

    #[test]
    fn live_rejects_invalid_extra_hosts() {
        assert!(
            MadaraGeneric::live("x", "https://example.com", &["10.0.0.5".to_string()]).is_err()
        );
        assert!(
            MadaraGeneric::live("x", "https://example.com", &["cdn.example".to_string()]).is_ok()
        );
    }

    #[test]
    fn live_constructor_rejects_unsafe_base_urls() {
        assert!(MadaraGeneric::live("x", "http://a.example.com", &[]).is_err());
        assert!(MadaraGeneric::live("x", "https://127.0.0.1", &[]).is_err());
        assert!(MadaraGeneric::live("x", "https://user@pass.example.org", &[]).is_err());
        assert!(MadaraGeneric::live("x", "https://localhost:8443", &[]).is_err());
        assert!(MadaraGeneric::live("x", "https://example.com", &[]).is_ok());
    }

    #[tokio::test]
    #[ignore = "live WAF-vs-app triage: built-in TTT search over the real network"]
    async fn live_builtin_ttt_search_probe() {
        let adapter = MadaraGeneric::live(
            "Truyen Tuoi Tho",
            "https://truyentuoitho.com/",
            &[crate::sources::registry::TTT_IMAGE_HOSTS.to_string()],
        )
        .unwrap();
        match adapter.search("ninja", 1).await {
            Ok(page) => {
                for item in page.items.iter().take(5) {
                    println!(
                        "LIVE ok: [{}] {} ({}) cover={:?}",
                        item.source_id, item.title, item.source_manga_id, item.cover_key
                    );
                }
                println!("LIVE total items: {}", page.items.len());
                assert!(!page.items.is_empty(), "search returned zero cards");
            }
            Err(error) => {
                println!("LIVE FAIL: {} ({})", error.code(), error.safe_message());
                panic!("live search failed: {}", error.code());
            }
        }
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

    // ----- TruyenQQ profile (built-in layout) --------------------------------

    fn qq(replies: Vec<Result<&'static str, &'static str>>) -> (MadaraGeneric, Arc<MockTransport>) {
        let transport = Arc::new(MockTransport::with(replies));
        let provider = MadaraGeneric::with_transport_profile(
            "TruyenQQ",
            "https://truyenqq.com.vn/",
            transport.clone(),
            SiteProfile::TruyenQQ,
        )
        .unwrap();
        (provider, transport)
    }

    const QQ_SEARCH: &str = r#"<html><body>
        <h1>TÌM KIẾM TRUYỆN: <span class="keyword">"ninja"</span></h1>
        <div class="listing"><div class="inner">
            <div class="item">
                <div class="cover"><a href="/ninja-to-koroshiya" class="cover"><img src="/media/book/ninja.jpg" alt="Ninja"></a></div>
                <div class="info"><h3><a href="/ninja-to-koroshiya">Ninja To Koroshiya</a></h3></div>
            </div>
            <div class="item">
                <div class="cover"><a href="/vo-luyen-dinh-phong"><img src="/media/book/vo.jpg"></a></div>
                <div class="info"><h3><a href="/vo-luyen-dinh-phong">V&#245; Luyện Đỉnh Phong</a></h3></div>
            </div>
        </div></div>
        <div class="pagination"><div class="page-total">16 kết quả</div></div>
        </body></html>"#;

    #[tokio::test]
    async fn qq_search_hits_tim_kiem_and_maps_listing_cards() {
        let (provider, transport) = qq(vec![Ok(QQ_SEARCH)]);
        let page = provider.search("ninja", 1).await.unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].source_id, "truyenqq");
        assert_eq!(page.items[0].source_manga_id, "ninja-to-koroshiya");
        assert_eq!(page.items[0].title, "Ninja To Koroshiya");
        assert_eq!(
            page.items[1].title, "Võ Luyện Đỉnh Phong",
            "entities decode"
        );
        assert_eq!(
            page.items[0].cover_key.as_deref(),
            Some("/media/book/ninja.jpg")
        );
        assert!(!page.has_more, "count text is not a next-page link");
        let requested = transport.requested.lock().unwrap();
        assert_eq!(requested[0].1, "https://truyenqq.com.vn/tim-kiem?s=ninja");
    }

    #[tokio::test]
    async fn qq_search_ignores_popular_blocks_outside_listing() {
        let (provider, _) = qq(vec![Ok(r#"<html><body>
            <div class="slide"><div class="item"><a href="/hot-one"><img src="/media/book/h.jpg"></a></div></div>
            <div class="listing"></div>
        </body></html>"#)]);
        let page = provider.search("x", 1).await.unwrap();
        assert!(
            page.items.is_empty(),
            "carousel items outside .listing never count"
        );
    }

    #[tokio::test]
    async fn qq_detail_uses_full_list_param_and_parses_chapter_rows() {
        let (provider, transport) = qq(vec![Ok(r#"<html><body>
            <h1 itemprop="name">V&#245; Luyện Đỉnh Phong</h1>
            <div id="chapter-list" class="chapter-list"><div class="inner"><div class="reading-list">
                <div class="item"><div class="item-name"><a href="/vo-luyen/chapter-2" class="chapter-name d-block">Chapter 2</a></div></div>
                <div class="item"><div class="item-name"><a href="/vo-luyen/chapter-1" class="chapter-name d-block">Chapter 1</a></div></div>
            </div></div></div>
        </body></html>"#)]);
        let detail = provider.manga_detail("vo-luyen").await.unwrap();
        assert_eq!(detail.summary.title, "Võ Luyện Đỉnh Phong");
        assert_eq!(detail.chapters.len(), 2);
        assert_eq!(
            detail.chapters[0].source_chapter_id, "chapter-1",
            "chronological"
        );
        assert_eq!(detail.chapters[0].number, Some(1.0));
        assert_eq!(detail.chapters[1].number, Some(2.0));
        assert!(!detail.available_offline);
        let requested = transport.requested.lock().unwrap();
        assert_eq!(
            requested[0].1, "https://truyenqq.com.vn/vo-luyen?page-chap=1",
            "full chapter list requested"
        );
    }

    #[tokio::test]
    async fn qq_pages_prefer_data_src_and_follow_cdn_shards() {
        let (provider, _) = qq(vec![Ok(r#"<html><body>
            <div class="image-box"><img class="lazy" data-src="https://s34.cc3t.net/chapters/b469/chapter-1/p0.PNG" src="/images/placeholder.jpg"></div>
            <div class="image-box"><img class="lazy" data-src="https://s12.cc3t.net/chapters/b469/chapter-1/p1.jpg" src="/images/loading.svg"></div>
            <div class="image-box"><img src="https://cdn.adnetwork.example/banner.gif"></div>
        </body></html>"#)]);
        let manifest = provider
            .page_manifest("vo-luyen", "chapter-1")
            .await
            .unwrap();
        assert_eq!(manifest.pages.len(), 2, "ads without data-src are skipped");
        assert!(manifest.pages[0].url.starts_with("https://s34.cc3t.net/"));
        assert!(manifest.pages[1].url.starts_with("https://s12.cc3t.net/"));
    }

    #[tokio::test]
    #[ignore = "live network probe: built-in TruyenQQ search -> detail -> pages over the app transport (wreq + ECH)"]
    async fn live_builtin_qq_probe() {
        let adapter = MadaraGeneric::built_in(
            "truyenqq",
            "TruyenQQ",
            "https://truyenqq.com.vn/",
            &[],
            SiteProfile::TruyenQQ,
        )
        .unwrap();
        let page = adapter.search("ninja", 1).await.expect("live search");
        assert!(!page.items.is_empty(), "search returned zero cards");
        let first = &page.items[0];
        println!(
            "LIVE search: {} items; first {} ({})",
            page.items.len(),
            first.title,
            first.source_manga_id
        );
        let detail = adapter
            .manga_detail(&first.source_manga_id)
            .await
            .expect("live detail");
        println!(
            "LIVE detail: {} — {} chapters",
            detail.summary.title,
            detail.chapters.len()
        );
        assert!(!detail.chapters.is_empty(), "chapter list empty");
        let last = &detail.chapters[detail.chapters.len() - 1];
        let manifest = adapter
            .page_manifest(&first.source_manga_id, &last.source_chapter_id)
            .await
            .expect("live pages");
        println!(
            "LIVE pages: chapter {} -> {} images",
            last.source_chapter_id,
            manifest.pages.len()
        );
        assert!(manifest.pages.len() >= 5, "implausibly small page set");
    }

    #[test]
    fn qq_owns_root_slugs_but_not_reserved_sections() {
        let (provider, _) = qq(vec![]);
        let manga: Url = "https://truyenqq.com.vn/vo-luyen-dinh-phong"
            .parse()
            .unwrap();
        assert!(provider.owns_url(&manga));
        let chapter: Url = "https://truyenqq.com.vn/vo-luyen-dinh-phong/chapter-12"
            .parse()
            .unwrap();
        assert!(provider.owns_url(&chapter));
        assert_eq!(
            provider.chapter_from_url(&chapter),
            Some(("vo-luyen-dinh-phong".to_string(), "chapter-12".to_string()))
        );
        for reserved in ["tim-kiem", "the-loai", "truyen-hot", "contact"] {
            let url: Url = format!("https://truyenqq.com.vn/{reserved}")
                .parse()
                .unwrap();
            assert!(!provider.owns_url(&url), "{reserved} is a site section");
        }
        let foreign: Url = "https://other.example/vo-luyen".parse().unwrap();
        assert!(!provider.owns_url(&foreign));
    }
}
