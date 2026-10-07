use std::collections::HashSet;
use std::sync::Arc;

use crate::sources::adapter::{SourceAdapter, SourceError};
use crate::sources::dto::{
    Capability, HomeFeedItem, PageDescriptor, PageManifest, RemoteChapter, RemoteHomeFeed,
    RemoteMangaDetail, RemoteMangaSummary, RemoteSearchPage, SourceInfo, SourceStatus,
};
use crate::sources::http::{
    GuardedHttp, HttpPolicy, Transport, COVER_MAX_BYTES, IMAGE_MAX_BYTES, MAX_PAGE_COUNT,
};
use async_trait::async_trait;
use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

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
    /// ASP.NET "Comic" engine (NetTruyen): /tim-truyen search, FULL chapter
    /// list as JSON from ComicService.asmx/ChapterList?slug=, page images
    /// referenced from sharded *.kcgsbok.com CDNs.
    Nettruyen,
    /// Dilib custom engine: story at /<slug>-<id>.html with the FULL chapter
    /// list inline (/truyen-tranh/<story>-chap-<n>.html), reader images under
    /// /img/comic/ on the base host. No server-rendered search.
    Dilib,
    /// DocTruyen3Q's own engine (not WordPress): story at
    /// /truyen-tranh/<slug>/<numericId> and chapters at
    /// /truyen-tranh/<slug>/chapter-<n>/<numericChapterId> — the numeric ids
    /// are REQUIRED (without them the site answers a soft-404 or redirects to
    /// /error/404-manga), so both ids are carried in the stored slugs, joined
    /// with `.` (valid_slug allows dots and the ids are pure digits). Search is
    /// GET /tim-truyen?keyword= and matches the DIACRITIC-FREE slug, so queries
    /// are folded. Reader images live on rotating shards of vetted image
    /// domains and 403 without a Referer.
    Doctruyen3q,
}

/// DocTruyen3Q serves chapter pages from numbered shards of these domains
/// (recon 2026-10-05: s15.anhvip.xyz, s8.topcdnv1.art) — the shard differs
/// per story, so the family is vetted, not one host.
const DOCTRUYEN3Q_IMAGE_DOMAINS: &[&str] = &["anhvip.xyz", "topcdnv1.art"];
/// Cover shards, same shape as TruyenQQ's: `/comics/<slug>-<id>.jpg`.
const DOCTRUYEN3Q_COVER_DOMAINS: &[(&str, &str)] = &[("anhvip", "anhvip.xyz")];

/// Fold a search query to the vocabulary the site actually indexes: its
/// `keyword` route matches the ASCII, diacritic-free story slug, so "võ luyện"
/// has to arrive as "vo luyen" or every real Vietnamese query returns zero.
fn fold_search_query(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'a' | 'à' | 'ả' | 'ã' | 'á' | 'ạ' | 'ă' | 'ắ' | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ấ'
            | 'ầ' | 'ẩ' | 'ẫ' | 'ậ' => 'a',
            'e' | 'è' | 'é' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' => {
                'e'
            }
            'i' | 'ì' | 'í' | 'ỉ' | 'ĩ' | 'ị' => 'i',
            'o' | 'ò' | 'ó' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ớ'
            | 'ờ' | 'ở' | 'ỡ' | 'ợ' => 'o',
            'u' | 'ù' | 'ú' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' => {
                'u'
            }
            'y' | 'ỳ' | 'ý' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
            'd' | 'đ' => 'd',
            other => other,
        })
        .collect()
}

/// Split a `<stem>.<numeric-id>` slug back into its parts. The id is all
/// digits and a stem never ends with a dot, so the LAST dot is the boundary.
fn split_id_slug(slug: &str) -> Option<(&str, &str)> {
    let (stem, id) = slug.rsplit_once('.')?;
    (!stem.is_empty() && !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        .then_some((stem, id))
}

/// Rebuild `<stem>.<numeric-id>` from the two halves a URL path supplied.
fn join_id_slug(stem: &str, id: &str) -> String {
    format!("{stem}.{id}")
}

/// NetTruyen chapter/thumb CDN domain (rotating imageN. subdomains).
const NETTRUYEN_IMAGE_DOMAIN: &str = "kcgsbok.com";
/// NetTruyen covers never ship from the site host — the theme loads every
/// thumbnail from this sharded CDN, whose path is fully determined by the
/// story slug (`/nettruyen/thumb/<slug>.jpg`). One fixed shard is used for
/// fetching; it is a compile-time built-in constant, never site data.
const NETTRUYEN_COVER_HOST: &str = "image1.kcgsbok.com";
/// Dilib navigation/footer slugs that are never manga stories.
const DILIB_RESERVED: &[&str] = &[
    "ban-quyen",
    "chinh-sach-quyen-rieng-tu-privacy-policy",
    "gioi-thieu-sach-moi",
    "lien-he",
    "review-sach",
    "thu-vien-sach-dien-tu",
    "tinh-hoa-giao-duc-di-san-cho-muon-doi-sau",
    "ung-ho-donate",
];

/// TruyenQQ serves chapter images from a small family of fixed CDN domains
/// that rotate per series (observed: `*.truyenvua.com` shards, and since
/// 2026-10-02 `hinhtruyen.com` with a `tintruyen.com` alternate carried in
/// `data-cdn`). A domain (not exact-host) allowlist is only ever set by
/// built-in code — user-registered sources keep the exact-host fail-closed
/// policy.
const TRUYENQQ_IMAGE_DOMAINS: &[&str] = &["truyenvua.com", "hinhtruyen.com", "tintruyen.com"];
/// TruyenQQ covers are not hosted by the site itself either; they ship from
/// this second CDN. File names carry an upload timestamp, so unlike NetTruyen
/// the key must keep the source's own path — it is validated as a bare
/// `/ebook/…` path and always re-fetched against this fixed host.
/// TruyenQQ covers are sharded across two CDN families, and a file only
/// exists on the shard that served it, so the shard has to survive in the
/// key. The key stays a PATH (never a host): it records a short shard label
/// plus a two-value token, and fetch_cover rebuilds the host from the
/// compile-time domain for that token — an arbitrary host cannot be expressed.
const TRUYENQQ_COVER_DOMAINS: &[(&str, &str)] = &[("hh", "hinhhinh.com"), ("tv", "truyenvua.com")];
/// Site sections whose root-level paths are never manga slugs.
const TRUYENQQ_RESERVED: &[&str] = &[
    "doc-truyen",
    "truyen-tranh",
    "tim-kiem",
    "tim-kiem-nang-cao",
    "frontend",
    "top-ngay",
    "top-tuan",
    "top-thang",
    "truyen-moi-cap-nhat",
    "truyen-tranh-moi",
    "truyen-hoan-thanh",
    "truyen-yeu-thich",
    "truyen-dang-theo-doi",
    "truyen-ngau-nhien",
    "truyen-dich-qq",
    "lich-su",
    "the-loai",
    "tac-gia",
    "dang-nhap",
    "dang-ky",
    "doi-mat-kha",
    "gioi-thieu",
    "lien-he",
    "bao-tai",
    "rss",
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
            if normalized == host || extras.contains(&normalized) {
                // The base host is already allowed, and a repeated entry adds
                // nothing. A SUBDOMAIN of the base is NOT redundant: Madara
                // pages are matched by exact host, so dropping it here silently
                // un-registered a real CDN (live bug 2026-10-05, TTT moved its
                // "(Full)" chapters to `img.truyentuoitho.com`).
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
        // Hotlink-protected CDNs (the sharded image hosts these sites serve
        // chapters from, and TTT's Cloudflare worker for its script-rendered
        // chapters) 403 any request without the site's own origin as Referer.
        // Always our own base origin, never caller-supplied. Dilib serves from
        // its base host and stays without one.
        let referer = matches!(
            profile,
            SiteProfile::TruyenQQ
                | SiteProfile::Nettruyen
                | SiteProfile::Madara
                | SiteProfile::Doctruyen3q
        )
        .then(|| base.as_str().to_string());
        let http = Arc::new(GuardedHttp::new(match profile {
            SiteProfile::Madara => HttpPolicy {
                allowed_hosts,
                allowed_domains: Vec::new(),
                referer,
            },
            SiteProfile::TruyenQQ => HttpPolicy {
                allowed_hosts,
                allowed_domains: TRUYENQQ_COVER_DOMAINS
                    .iter()
                    .map(|(_, domain)| domain.to_string())
                    .chain(
                        TRUYENQQ_IMAGE_DOMAINS
                            .iter()
                            .map(|domain| domain.to_string()),
                    )
                    .collect(),
                referer,
            },
            SiteProfile::Nettruyen => HttpPolicy {
                allowed_hosts,
                allowed_domains: vec![NETTRUYEN_IMAGE_DOMAIN.to_string()],
                referer,
            },
            SiteProfile::Dilib => HttpPolicy {
                allowed_hosts,
                allowed_domains: Vec::new(),
                referer,
            },
            SiteProfile::Doctruyen3q => HttpPolicy {
                allowed_hosts,
                allowed_domains: DOCTRUYEN3Q_IMAGE_DOMAINS
                    .iter()
                    .chain(DOCTRUYEN3Q_COVER_DOMAINS.iter().map(|(_, domain)| domain))
                    .map(|domain| domain.to_string())
                    .collect(),
                referer,
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

    /// Mint a cover key (path-only — theme thumbnails ship with the site;
    /// built-in CDNs may also ship their own, see below). A key never carries
    /// scheme or host, so nothing fetchable leaks to the renderer.
    fn cover_key_from_src(&self, raw: &str) -> Option<String> {
        let parsed = self.base.join(raw.trim()).ok()?;
        if parsed.scheme() != "https" {
            return None;
        }
        let host = parsed.host_str()?;
        // TruyenQQ's and DocTruyen3Q's covers live on a second CDN whose file
        // names carry an upload timestamp or a numeric id, so the path cannot
        // always be re-derived from the slug the way NetTruyen's can. Only the
        // reader path of the vetted cover domain is accepted (`/ebook/…` and
        // `/comics/…`), and it is always re-fetched against that one domain
        // family.
        let cover_table = match self.profile {
            SiteProfile::TruyenQQ => Some((TRUYENQQ_COVER_DOMAINS, "/ebook/")),
            SiteProfile::Doctruyen3q => Some((DOCTRUYEN3Q_COVER_DOMAINS, "/comics/")),
            _ => None,
        };
        if let Some((domains, path_prefix)) = cover_table {
            if let Some(token) = domains
                .iter()
                .find(|(_, domain)| host == *domain || host.ends_with(&format!(".{domain}")))
                .map(|(token, _)| *token)
            {
                // shard.<domain> -> label "shard" (bare domain uses "i").
                let label = host.split_once('.').map_or("i", |(l, _)| l);
                let path = parsed.path();
                let key = format!("/cdn/{label}.{token}{path}");
                let safe = label.len() <= 16
                    && label
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                    && path.starts_with(path_prefix)
                    && key.len() <= 480;
                return safe.then_some(key);
            }
        }
        if host != self.host.as_str() {
            return None;
        }
        let path = parsed.path();
        (!path.starts_with("//") && !path.is_empty() && path.len() <= 480).then(|| path.to_string())
    }

    /// NetTruyen ships every cover on its CDN at a slug-determined path, so
    /// the key is minted from the manga id instead of scraped markup: the
    /// renderer still only ever sees a path, and the fetch below re-derives
    /// the exact same path from the manga id before requesting anything.
    fn cover_key(&self, slug: &str) -> Option<String> {
        if self.profile == SiteProfile::Nettruyen {
            return valid_slug(slug).then(|| format!("/nettruyen/thumb/{slug}.jpg"));
        }
        None
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
            SiteProfile::Nettruyen => "nettruyen".to_string(),
            SiteProfile::Dilib => "dilib".to_string(),
            SiteProfile::Doctruyen3q => "doctruyen3q".to_string(),
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
            SiteProfile::TruyenQQ => format!("{}truyen-tranh/{slug}", self.base),
            SiteProfile::Nettruyen => format!("{}truyen-tranh/{slug}", self.base),
            SiteProfile::Dilib => format!("{}{slug}.html", self.base),
            // The numeric story id is part of the route: /truyen-tranh/<slug>/<id>
            SiteProfile::Doctruyen3q => match split_id_slug(slug) {
                Some((stem, id)) => format!("{}truyen-tranh/{stem}/{id}", self.base),
                None => format!("{}truyen-tranh/{slug}", self.base),
            },
        }
    }

    fn chapter_url(&self, manga_slug: &str, chapter_slug: &str) -> String {
        match self.profile {
            SiteProfile::Madara => format!("{}manga/{}/{}/", self.base, manga_slug, chapter_slug),
            // DocTruyen3Q chapters carry BOTH ids:
            // /truyen-tranh/<story-slug>/chapter-<n>/<chapterId>
            SiteProfile::Doctruyen3q => {
                let story = split_id_slug(manga_slug).map_or(manga_slug, |(stem, _)| stem);
                match split_id_slug(chapter_slug) {
                    Some((stem, id)) => {
                        format!("{}truyen-tranh/{story}/{stem}/{id}", self.base)
                    }
                    None => format!("{}truyen-tranh/{story}/{chapter_slug}", self.base),
                }
            }
            // TruyenQQ chapters are siblings of the story, not children:
            // /truyen-tranh/<story>-chap-<n>. The stored chapter id is the
            // whole stem, so `manga_slug` is already contained in it.
            SiteProfile::TruyenQQ => format!("{}truyen-tranh/{chapter_slug}", self.base),
            SiteProfile::Nettruyen => {
                format!("{}truyen-tranh/{}/{}", self.base, manga_slug, chapter_slug)
            }
            // Dilib chapters are /truyen-tranh/<story>-chap-<n>.html and the
            // stored chapter id is the full file stem (chapter_url rebuilds it).
            SiteProfile::Dilib => format!("{}truyen-tranh/{chapter_slug}.html", self.base),
        }
    }

    /// Detail-page fetch URL. TruyenQQ renders its whole chapter list inline
    /// on the story page (950 links verified live), so no extra page param.
    fn detail_chapter_list_url(&self, slug: &str) -> String {
        match self.profile {
            SiteProfile::Madara
            | SiteProfile::Nettruyen
            | SiteProfile::Dilib
            | SiteProfile::Doctruyen3q => self.manga_url(slug),
            SiteProfile::TruyenQQ => self.manga_url(slug),
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
        if self.profile == SiteProfile::Doctruyen3q {
            // /truyen-tranh/<slug>/<numericId> — the id is part of the route,
            // so the stored identity carries both halves as `<slug>.<id>`.
            let mut parts = parsed
                .path_segments()?
                .filter(|segment| !segment.is_empty());
            if parts.next()? != "truyen-tranh" {
                return None;
            }
            let stem = parts.next()?;
            let id = parts.next()?;
            if parts.next().is_some()
                || id.is_empty()
                || !id.bytes().all(|byte| byte.is_ascii_digit())
            {
                return None;
            }
            let slug = join_id_slug(stem, id);
            return valid_slug(&slug).then_some(slug);
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
                // Stories and chapters are SIBLINGS here: /truyen-tranh/<slug>
                // and /truyen-tranh/<slug>-chap-<n>. The "-chap-" marker is the
                // only thing separating an identity from a chapter link.
                if segments.next()? != "truyen-tranh" {
                    return None;
                }
                let stem = segments.next()?;
                if TRUYENQQ_RESERVED.contains(&stem) || stem.contains("-chap-") {
                    return None;
                }
                stem
            }
            SiteProfile::Nettruyen => {
                if segments.next()? != "truyen-tranh" {
                    return None;
                }
                segments.next()?
            }
            SiteProfile::Dilib => {
                let file = segments.next()?;
                let slug = file.strip_suffix(".html")?;
                if DILIB_RESERVED.contains(&slug) || !dilib_story_slug(slug) {
                    return None;
                }
                slug
            }
            // Handled above: the identity is two path segments joined with `.`.
            SiteProfile::Doctruyen3q => return None,
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
        if self.profile == SiteProfile::Doctruyen3q {
            // /truyen-tranh/<story-slug>/chapter-<n>/<chapterId>; the numeric
            // chapter id is required, so it rides along in the stored id.
            let story = split_id_slug(manga_slug).map_or(manga_slug, |(stem, _)| stem);
            let mut parts = parsed
                .path_segments()?
                .filter(|segment| !segment.is_empty());
            if parts.next()? != "truyen-tranh" || parts.next()? != story {
                return None;
            }
            let stem = parts.next()?;
            let id = parts.next()?;
            if parts.next().is_some()
                || !stem.starts_with("chapter-")
                || id.is_empty()
                || !id.bytes().all(|byte| byte.is_ascii_digit())
            {
                return None;
            }
            let slug = join_id_slug(stem, id);
            return valid_slug(&slug).then_some(slug);
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
                // /truyen-tranh/<manga>-chap-<n> — like Dilib, the stored
                // chapter id is the whole stem; chapter_url rebuilds from it.
                if segments.next()? != "truyen-tranh" {
                    return None;
                }
                let stem = segments.next()?;
                if segments.next().is_some()
                    || !stem
                        .strip_prefix(&format!("{manga_slug}-"))
                        .is_some_and(|rest| rest.starts_with("chap-"))
                    || !valid_slug(stem)
                {
                    return None;
                }
                return Some(stem.to_string());
            }
            SiteProfile::Nettruyen => {
                if segments.next()? != "truyen-tranh" || segments.next()? != manga_slug {
                    return None;
                }
            }
            SiteProfile::Dilib => {
                // /truyen-tranh/<manga>-chap-<n>.html — the stored identity is
                // the whole file stem, chapter_url rebuilds from it.
                if segments.next()? != "truyen-tranh" {
                    return None;
                }
                let stem = segments.next()?.strip_suffix(".html")?;
                if segments.next().is_some()
                    || !stem
                        .strip_prefix(&format!("{manga_slug}-"))
                        .is_some_and(|rest| rest.starts_with("chap-"))
                    || !valid_slug(stem)
                {
                    return None;
                }
                return Some(stem.to_string());
            }
            // Handled above: the identity is the chapter stem plus its id.
            SiteProfile::Doctruyen3q => return None,
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
            let cover_key = self.cover_key(&slug).or_else(|| {
                node.select(thumb).find_map(|image| {
                    ["src", "data-src", "data-lazy-src", "data-original"]
                        .iter()
                        .find_map(|attr| image.value().attr(attr))
                        .and_then(|raw| self.cover_key_from_src(raw))
                })
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

    /// Collect one `ul.list_grid` catalog grid into a rail. Each card's story
    /// identity and title come from the heading link, the cover from the
    /// avatar, and the chapter/age labels are the source's own wording;
    /// repeats and foreign links are dropped.
    /// The source's own ranking pages are separate routes per period; an
    /// unknown period is a client error, never a silent fallback to "day".
    async fn ranking_rail(&self, period: &str) -> Result<Vec<HomeFeedItem>, SourceError> {
        let route = match period {
            "day" => "top-ngay",
            "week" => "top-tuan",
            "month" => "top-thang",
            _ => return Err(SourceError::MalformedUrl),
        };
        let html = self
            .http
            .get_text(&format!("{}{}", self.base, route))
            .await?;
        let document = Html::parse_document(&html);
        Ok(self.home_rail(&document, "ul.list_grid li", 10))
    }

    /// Collect one catalog grid into a rail, dropping repeats and foreign links.
    fn home_rail(&self, document: &Html, grid: &str, cap: usize) -> Vec<HomeFeedItem> {
        let rows = Selector::parse(grid).unwrap();
        let title = Selector::parse("div.book_info h3 a[href]").unwrap();
        let avatar = Selector::parse("div.book_avatar img[src]").unwrap();
        let chapter = Selector::parse("div.last_chapter a[href]").unwrap();
        let time = Selector::parse("span.time-ago").unwrap();
        let mut seen: HashSet<String> = HashSet::new();
        let mut items: Vec<HomeFeedItem> = Vec::new();
        for node in document.select(&rows) {
            let Some(anchor) = node.select(&title).next() else {
                continue;
            };
            let Some(href) = anchor.value().attr("href") else {
                continue;
            };
            let Some(slug) = self.slug_from_manga_url(href) else {
                continue;
            };
            if !seen.insert(slug.clone()) {
                continue;
            }
            let text_of = |sel: &Selector| {
                node.select(sel)
                    .next()
                    .map(|n| n.text().collect::<String>().trim().to_string())
                    .filter(|value| !value.is_empty())
            };
            items.push(HomeFeedItem {
                summary: RemoteMangaSummary {
                    source_id: self.id.clone(),
                    source_manga_id: slug.clone(),
                    source_display_name: self.display_name.clone(),
                    title: text_of(&title).unwrap_or_else(|| slug.clone()),
                    author: None,
                    cover_key: node
                        .select(&avatar)
                        .next()
                        .and_then(|img| img.value().attr("src"))
                        .and_then(|raw| self.cover_key_from_src(raw)),
                    status: "unknown".to_string(),
                },
                latest_chapter: text_of(&chapter),
                latest_time: text_of(&time),
            });
            if items.len() >= cap {
                break;
            }
        }
        items
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
                ".works-chapter-list a[href], .list_chapter a[href], ul.list-chapter a[href]"
            }
            SiteProfile::Nettruyen => {
                ".list-chapter a.chapter[href], .nav-chapters a[href], #chapter_list a[href], ul.list-chapter a[href]"
            }
            SiteProfile::Dilib => {
                // The story page ships the COMPLETE chapter list as plain
                // `<a href="/truyen-tranh/<story>-chap-<n>.html">` anchors
                // (no distinguishing class); `chapter_slug_from_url` rejects
                // any that are not this story's chapter links.
                "a[href$=\".html\"]"
            }
            SiteProfile::Doctruyen3q => {
                // Full list inline (no AJAX). The first row is a hidden ad
                // pointing at a DIFFERENT domain, which `chapter_slug_from_url`
                // rejects because it only accepts this host.
                "#list-chapter-dt a.chapter[href]"
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
            // DocTruyen3Q prints the real number in `data-chapter` ("97",
            // "60.5"); the stored id also carries the numeric chapter key, so
            // parsing the stem would read `60.5.2502652` as the number.
            let number = match self.profile {
                SiteProfile::Doctruyen3q => node
                    .value()
                    .attr("data-chapter")
                    .and_then(|value| value.trim().parse::<f64>().ok())
                    .or_else(|| chapter_number_from_slug(&slug)),
                _ => chapter_number_from_slug(&slug),
            };
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

/// The engine's own catalog grid holds 42 cards per page.
const QQ_BROWSE_CAP: usize = 42;

/// Catalog route for one filtered page. Live recon (2026-09-30) found
/// `the-loai/<slug>`, `truyen-dang-theo-doi`, `truyen-hoan-thanh` and
/// `truyen-moi-cap-nhat`, all paginating as `/trang-N`.
/// The catalog page's own sort `?select` (recon 2026-09-30) offers exactly six
/// orderings; anything else is rejected rather than silently defaulted.
fn qq_sort_value(sort: &str) -> Result<u8, SourceError> {
    match sort {
        "newest-desc" => Ok(0),
        "newest-asc" => Ok(1),
        "updated-desc" => Ok(2),
        "updated-asc" => Ok(3),
        "views-desc" => Ok(4),
        "views-asc" => Ok(5),
        _ => Err(SourceError::MalformedUrl),
    }
}

/// Genre and status DO combine on a genre page (`?status=0|2`) — the standalone
/// `truyen-*` routes only exist for a status on its own. Sorting is a genre
/// page feature only (`?sort=0..5`).
fn qq_browse_path(
    genre: Option<&str>,
    status: Option<&str>,
    sort: Option<&str>,
    page: u32,
) -> Result<String, SourceError> {
    let base = match genre {
        Some(slug) => format!("the-loai/{slug}"),
        None => match status {
            Some("ongoing") => "truyen-dang-theo-doi",
            Some("completed") => "truyen-hoan-thanh",
            Some("new") | None => "truyen-moi-cap-nhat",
            Some(_) => return Err(SourceError::MalformedUrl),
        }
        .to_string(),
    };
    let path = if page > 1 {
        format!("{base}/trang-{page}")
    } else {
        base
    };
    let mut query: Vec<String> = Vec::new();
    if genre.is_some() {
        let status_value = match status {
            Some("ongoing") => Some(0),
            Some("completed") => Some(2),
            // "new" is the genre page's own default order, so no param.
            _ => None,
        };
        if let Some(value) = status_value {
            query.push(format!("status={value}"));
        }
    }
    if let Some(sort) = sort {
        // Live probe 2026-09-30: `?sort=` reorders a genre page but changes
        // nothing on the three `truyen-*` status routes (identical order). The
        // source only publishes the sort select on genre pages, so asking for
        // a sort anywhere else is refused rather than quietly ignored.
        if genre.is_none() {
            return Err(SourceError::UnsupportedOperation);
        }
        query.push(format!("sort={}", qq_sort_value(sort)?));
    }
    Ok(if query.is_empty() {
        path
    } else {
        format!("{path}?{}", query.join("&"))
    })
}

/// Dilib stories are `/<slug>-<numeric-id>.html`; the numeric tail separates
/// real story slugs from section pages that happen to end in .html.
fn dilib_story_slug(slug: &str) -> bool {
    matches!(
        slug.rsplit_once('-'),
        Some((head, tail)) if !head.is_empty() && !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit())
    )
}

fn chapter_number_from_slug(slug: &str) -> Option<f64> {
    // TruyenQQ-family slugs write fractional chapters with a dash:
    // "-chap-2-1" means chapter 2.1 (live bug 2026-10-02: reading only the
    // trailing digit run collapsed every "X.1" to number 1 and exported
    // chapter 2.1 onto chapter-1.cbz). Take the WHOLE trailing numeric block
    // (groups joined by '.', ',' or '-'): exactly one separator is a decimal;
    // no separator is an integer; two or more ("1-2-3", "1.2.3") are ambiguous
    // — a missing number only sorts the chapter as unnumbered, a wrong one
    // corrupts the export filename, so refuse to guess.
    let re = Regex::new(r"(\d+(?:[.,-]\d+)*)$").ok()?;
    let block = re.captures(slug)?.get(1)?.as_str().to_string();
    let separators = block.matches(['.', ',', '-']).count();
    match separators {
        0 => block.parse::<f64>().ok(),
        1 => Some(block.replace(['-', ','], ".").parse::<f64>().ok()?),
        _ => None,
    }
}

#[async_trait]
impl SourceAdapter for MadaraGeneric {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            capabilities: if self.profile == SiteProfile::Dilib {
                // Dilib has no server-rendered search results page; stories
                // enter through URL import, then behave like any other source.
                vec![
                    Capability::ResolveUrl,
                    Capability::Details,
                    Capability::Pages,
                ]
            } else {
                vec![
                    Capability::Search,
                    Capability::ResolveUrl,
                    Capability::Details,
                    Capability::Pages,
                ]
            },
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
                // /truyen-tranh/<story> and /truyen-tranh/<story>-chap-<n>
                // are the only two shapes this source owns.
                let mut it = url
                    .path_segments()
                    .into_iter()
                    .flatten()
                    .filter(|s| !s.is_empty());
                it.next() == Some("truyen-tranh")
                    && it
                        .next()
                        .is_some_and(|stem| !TRUYENQQ_RESERVED.contains(&stem) && valid_slug(stem))
                    && it.next().is_none()
            }
            SiteProfile::Nettruyen => {
                let mut it = url
                    .path_segments()
                    .into_iter()
                    .flatten()
                    .filter(|s| !s.is_empty());
                it.next() == Some("truyen-tranh")
                    && it.next().is_some()
                    && it
                        .next()
                        .is_none_or(|c| it.next().is_none() && valid_slug(c))
            }
            SiteProfile::Dilib => {
                let mut it = url
                    .path_segments()
                    .into_iter()
                    .flatten()
                    .filter(|s| !s.is_empty());
                match (it.next(), it.next()) {
                    (Some(first), None) => match first.strip_suffix(".html") {
                        Some(slug) => !DILIB_RESERVED.contains(&slug) && dilib_story_slug(slug),
                        None => false,
                    },
                    _ => false,
                }
            }
            // /truyen-tranh/<slug>/<id> (story) and
            // /truyen-tranh/<slug>/chapter-<n>/<id> (chapter).
            SiteProfile::Doctruyen3q => {
                let parts: Vec<&str> = url
                    .path_segments()
                    .into_iter()
                    .flatten()
                    .filter(|segment| !segment.is_empty())
                    .collect();
                let numeric = |value: &str| {
                    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
                };
                match parts.as_slice() {
                    ["truyen-tranh", stem, id] => numeric(id) && valid_slug(stem),
                    ["truyen-tranh", stem, chapter, id] => {
                        numeric(id) && valid_slug(stem) && chapter.starts_with("chapter-")
                    }
                    _ => false,
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
            SiteProfile::TruyenQQ => {
                // /truyen-tranh/<story>-chap-<n>: the stem carries both halves.
                if segments.next()? != "truyen-tranh" {
                    return None;
                }
                let stem = segments.next()?;
                let (manga, rest) = stem.split_once("-chap-")?;
                let _ = rest;
                return Some((manga.to_string(), stem.to_string()));
            }
            SiteProfile::Nettruyen => {
                if segments.next()? != "truyen-tranh" {
                    return None;
                }
                segments.next()?
            }
            SiteProfile::Dilib => {
                if segments.next()? != "truyen-tranh" {
                    return None;
                }
                let stem = segments.next()?.strip_suffix(".html")?;
                let (manga, rest) = stem.split_once("-chap-")?;
                let _ = rest;
                return Some((manga.to_string(), stem.to_string()));
            }
            // A chapter URL carries the story SLUG but not its numeric id, and
            // this site 404s any story route without the id — so a pasted
            // chapter link cannot be turned into a manga identity here. It
            // fails as "URL not recognized" rather than guessing an id.
            SiteProfile::Doctruyen3q => return None,
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
                // The site's own suggest endpoint (its search box posts here;
                // the GET /tim-kiem?q= route renders an empty shell). The
                // fragment carries no pagination, so page > 1 repeats page 1.
                format!("{}frontend/search/search", self.base)
            }
            SiteProfile::Nettruyen => {
                // The site's own search route; `page` is their pagination param.
                if page == 1 {
                    format!("{}tim-truyen?keyword={}", self.base, encoded)
                } else {
                    format!("{}tim-truyen?keyword={}&page={}", self.base, encoded, page)
                }
            }
            SiteProfile::Dilib => return Err(SourceError::UnsupportedOperation),
            SiteProfile::Doctruyen3q => {
                // The route indexes the site's ASCII slugs, so the query has to
                // arrive folded: "võ luyện" must leave as "vo luyen" or every
                // real Vietnamese search returns nothing at all.
                let folded =
                    url::form_urlencoded::byte_serialize(fold_search_query(query).as_bytes())
                        .collect::<String>();
                if page == 1 {
                    format!("{}tim-truyen?keyword={}", self.base, folded)
                } else {
                    format!("{}tim-truyen?keyword={}&page={}", self.base, folded, page)
                }
            }
        };
        let html = if self.profile == SiteProfile::TruyenQQ {
            self.http
                .post_form_text(&search_url, &[("search", query), ("type", "0")])
                .await?
        } else {
            self.http.get_text(&search_url).await?
        };
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
                // The suggest fragment wraps the whole card in one <a> and
                // keeps the title in `p.name` (link text would concatenate the
                // alternate title and chapter too), so it needs its own walk.
                let rows = Selector::parse("li").unwrap();
                let row_link = Selector::parse("a[href]").unwrap();
                let row_name = Selector::parse("p.name").unwrap();
                let row_img = Selector::parse("div.search_avatar img[src]").unwrap();
                for node in document.select(&rows) {
                    let Some(href) = node
                        .select(&row_link)
                        .next()
                        .and_then(|a| a.value().attr("href"))
                    else {
                        continue;
                    };
                    let Some(slug) = self.slug_from_manga_url(href) else {
                        continue;
                    };
                    if !seen.insert(slug.clone()) {
                        continue;
                    }
                    let title = node
                        .select(&row_name)
                        .next()
                        .map(|p| p.text().collect::<String>().trim().to_string())
                        .filter(|value| !value.is_empty())
                        .unwrap_or_else(|| slug.clone());
                    let cover_key = node
                        .select(&row_img)
                        .next()
                        .and_then(|img| img.value().attr("src"))
                        .and_then(|raw| self.cover_key_from_src(raw));
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
                        break;
                    }
                }
            }
            SiteProfile::Nettruyen => {
                // Search cards are `.item` blocks whose title sits in
                // `figcaption h3`; the slider carousel has no figcaption and
                // is therefore immune to this selector.
                self.collect_cards(
                    &document,
                    &Selector::parse(".item").unwrap(),
                    &Selector::parse("figcaption h3 a[href]").unwrap(),
                    &Selector::parse("figure .image img").unwrap(),
                    &mut seen,
                    &mut items,
                );
            }
            SiteProfile::Dilib => {}
            SiteProfile::Doctruyen3q => {
                // One card per story; the title link carries `data-id` and the
                // cover is a plain `src` on the vetted image domain.
                self.collect_cards(
                    &document,
                    &Selector::parse("div.item-manga").unwrap(),
                    &Selector::parse("a.title-manga[href]").unwrap(),
                    &Selector::parse("div.image-item img").unwrap(),
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
            SiteProfile::TruyenQQ => false,
            SiteProfile::Nettruyen => document
                .select(&Selector::parse(".pagination a[href*='page']").unwrap())
                .next()
                .is_some(),
            SiteProfile::Dilib => false,
            SiteProfile::Doctruyen3q => document
                .select(&Selector::parse("div.paginate a.page-link[href*='page']").unwrap())
                .next()
                .is_some(),
        };
        Ok(RemoteSearchPage { items, has_more })
    }

    /// TruyenQQ's catalog (`/doc-truyen`) publishes the spotlight and the
    /// newest grid in one document; the most-read rail is its own ranking
    /// page. Every card keeps its own de-duplication set because one story
    /// legitimately appears in several rails.
    async fn home_feed(&self) -> Result<RemoteHomeFeed, SourceError> {
        if self.profile != SiteProfile::TruyenQQ {
            return Err(SourceError::UnsupportedOperation);
        }
        // Both documents are fetched before ANY Html value exists: scraper's
        // Html is !Send and must never live across an await.
        let catalog_html = self
            .http
            .get_text(&format!("{}doc-truyen", self.base))
            .await?;
        // Scoped so the !Send document is dead before the ranking await.
        let (featured, newest) = {
            let catalog_doc = Html::parse_document(&catalog_html);
            (
                self.home_rail(&catalog_doc, "#list_suggest li", 8),
                self.home_rail(&catalog_doc, "#list_new li", 12),
            )
        };
        let popular = self.ranking_rail("day").await.unwrap_or_default();
        let feed = RemoteHomeFeed {
            featured,
            newest,
            popular,
        };
        if feed.featured.is_empty() && feed.newest.is_empty() && feed.popular.is_empty() {
            return Err(SourceError::Parse);
        }
        Ok(feed)
    }

    async fn ranking(&self, period: &str) -> Result<Vec<HomeFeedItem>, SourceError> {
        if self.profile != SiteProfile::TruyenQQ {
            return Err(SourceError::UnsupportedOperation);
        }
        self.ranking_rail(period).await
    }

    async fn genres(&self) -> Result<Vec<crate::sources::dto::GenreTag>, SourceError> {
        if self.profile != SiteProfile::TruyenQQ {
            return Err(SourceError::UnsupportedOperation);
        }
        let html = self
            .http
            .get_text(&format!("{}doc-truyen", self.base))
            .await?;
        let tags = {
            let document = Html::parse_document(&html);
            let selector =
                Selector::parse("a[href*='/the-loai/']").map_err(|_| SourceError::Parse)?;
            let mut seen: HashSet<String> = HashSet::new();
            let mut out: Vec<crate::sources::dto::GenreTag> = Vec::new();
            for node in document.select(&selector) {
                let Some(href) = node.value().attr("href") else {
                    continue;
                };
                let Some(rest) = href.rsplit("/the-loai/").next() else {
                    continue;
                };
                let slug = rest.trim_matches('/').to_string();
                // Only the engine's own `<slug>-<id>` form is a genre route.
                if !valid_slug(&slug) || !slug.contains('-') {
                    continue;
                }
                let label = node.text().collect::<String>().trim().to_string();
                if label.is_empty() || !seen.insert(slug.clone()) {
                    continue;
                }
                out.push(crate::sources::dto::GenreTag { slug, label });
            }
            out
        };
        if tags.is_empty() {
            return Err(SourceError::Parse);
        }
        Ok(tags)
    }

    async fn browse(
        &self,
        genre: Option<&str>,
        status: Option<&str>,
        sort: Option<&str>,
        page: u32,
    ) -> Result<RemoteSearchPage, SourceError> {
        if self.profile != SiteProfile::TruyenQQ {
            return Err(SourceError::UnsupportedOperation);
        }
        let page = page.max(1);
        let path = qq_browse_path(genre, status, sort, page)?;
        let html = self
            .http
            .get_text(&format!("{}{}", self.base, path))
            .await?;
        let (items, has_more) = {
            let document = Html::parse_document(&html);
            // A page without the catalog grid is not the layout we read: that
            // is a parse failure, distinct from a grid that is genuinely empty.
            if document
                .select(&Selector::parse("ul.list_grid").unwrap())
                .next()
                .is_none()
            {
                return Err(SourceError::Parse);
            }
            let rail = self.home_rail(&document, "ul.list_grid li", QQ_BROWSE_CAP);
            let next = format!("/trang-{}", page + 1);
            let has_more = document
                .select(&Selector::parse("a[href]").unwrap())
                .any(|anchor| {
                    anchor
                        .value()
                        .attr("href")
                        .is_some_and(|href| href.contains(&next))
                });
            (
                rail.into_iter().map(|item| item.summary).collect(),
                has_more,
            )
        };
        Ok(RemoteSearchPage { items, has_more })
    }

    async fn resolve_url(&self, url: &Url) -> Result<String, SourceError> {
        let slug = match self.profile {
            SiteProfile::Madara => url.path_segments().and_then(|mut segments| {
                segments.next();
                segments.next().map(str::to_string)
            }),
            SiteProfile::TruyenQQ | SiteProfile::Nettruyen => {
                url.path_segments().and_then(|mut segments| {
                    segments.next();
                    segments.next().map(str::to_string)
                })
            }
            SiteProfile::Dilib => url
                .path_segments()
                .and_then(|mut segments| segments.next())
                .and_then(|file| file.strip_suffix(".html").map(str::to_string)),
            // Story links only (see chapter_from_url): /truyen-tranh/<slug>/<id>
            SiteProfile::Doctruyen3q => url.path_segments().and_then(|segments| {
                let parts: Vec<&str> = segments.filter(|segment| !segment.is_empty()).collect();
                match parts.as_slice() {
                    ["truyen-tranh", stem, id] => Some(join_id_slug(stem, id)),
                    _ => None,
                }
            }),
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
        // Real pages live on a sharded CDN domain for both lazy-loading sites.
        // TruyenQQ prints them in plain `src`, NetTruyen in `data-src`, so both
        // attributes are tried and the CDN host test is what rejects ads,
        // avatars and placeholders. Dilib (and Madara) serve pages from base.
        let lazy_cdn: Option<&'static [&'static str]> = match self.profile {
            SiteProfile::TruyenQQ => Some(TRUYENQQ_IMAGE_DOMAINS),
            SiteProfile::Nettruyen => Some(std::slice::from_ref(&NETTRUYEN_IMAGE_DOMAIN)),
            SiteProfile::Doctruyen3q => Some(DOCTRUYEN3Q_IMAGE_DOMAINS),
            SiteProfile::Madara | SiteProfile::Dilib => None,
        };
        if let Some(cdn) = lazy_cdn {
            // DocTruyen3Q reuses `.page-chapter` for its ad banners too, and
            // those point at the SAME image domain, so only the numbered page
            // wrappers are real pages here.
            let lazy = if self.profile == SiteProfile::Doctruyen3q {
                Selector::parse(
                    "div.list-image-detail div.page-chapter[id^='page_'] img[data-src], div.list-image-detail div.page-chapter[id^='page_'] img[src]",
                )
                .unwrap()
            } else {
                Selector::parse(".page-chapter img[data-src], .page-chapter img[src]").unwrap()
            };
            let mut seen: HashSet<Url> = HashSet::new();
            for node in document.select(&lazy) {
                let Some(raw) = ["data-src", "src"]
                    .iter()
                    .filter_map(|attr| node.value().attr(attr))
                    .map(str::trim)
                    .find(|v| !v.is_empty())
                else {
                    continue;
                };
                let Ok(joined) = self.base.join(raw) else {
                    continue;
                };
                if joined.scheme() != "https"
                    || !joined.host_str().is_some_and(|host| {
                        cdn.iter()
                            .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
                    })
                {
                    continue;
                }
                if seen.insert(joined.clone()) {
                    urls.push(joined.to_string());
                }
                // Stop one PAST the cap so the caller's `> MAX_PAGE_COUNT`
                // guard fires: breaking exactly AT it would silently ship a
                // cropped chapter that later reports "completed".
                if urls.len() > MAX_PAGE_COUNT {
                    break;
                }
            }
        } else if self.profile == SiteProfile::Dilib {
            // Dilib reader images are base-host `img[src*="/img/comic/"]`;
            // anything else on the page (logo, sidebar covers) is not a page.
            let allowed = self.allowed_image_hosts();
            let comic = Selector::parse("img[src]").unwrap();
            for node in document.select(&comic) {
                let Some(raw) = node
                    .value()
                    .attr("src")
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                else {
                    continue;
                };
                if !raw.contains("/img/comic/") {
                    continue;
                }
                let Ok(joined) = self.base.join(raw) else {
                    continue;
                };
                if joined.scheme() != "https"
                    || !joined
                        .host_str()
                        .is_some_and(|host| allowed.iter().any(|allowed| allowed == host))
                {
                    return Err(SourceError::Parse);
                }
                urls.push(joined.to_string());
                if urls.len() > MAX_PAGE_COUNT {
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
            collect_page_urls(&document, &strict, &self.base, &allowed, true, &mut urls)?;
            if urls.is_empty() {
                let boxed =
                    Selector::parse(".reading-content .page-box img, .page-box img.page-image")
                        .unwrap();
                collect_page_urls(&document, &boxed, &self.base, &allowed, true, &mut urls)?;
            }
            if urls.is_empty() {
                // No theme marker on this page, so every image in the reader
                // container is a candidate — including the ads that live there.
                let loose = Selector::parse(".reading-content img").unwrap();
                collect_page_urls(&document, &loose, &self.base, &allowed, false, &mut urls)?;
            }
            if urls.is_empty() {
                // The container exists but is empty because the images are
                // injected by an encrypted script. Still filtered through the
                // source's own allowlist: decrypting never widens what we fetch.
                for raw in encrypted_reader_images(&document).unwrap_or_default() {
                    let Ok(joined) = self.base.join(&raw) else {
                        continue;
                    };
                    if joined.scheme() != "https"
                        || !joined
                            .host_str()
                            .is_some_and(|host| allowed.iter().any(|allowed| allowed == host))
                    {
                        continue;
                    }
                    if urls.len() > MAX_PAGE_COUNT {
                        break;
                    }
                    urls.push(joined.to_string());
                }
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
        source_manga_id: &str,
        cover_key: &str,
    ) -> Result<Vec<u8>, SourceError> {
        // NetTruyen covers live on the vetted built-in CDN shard, and the key
        // must still be EXACTLY the path this manga's slug determines — a
        // stored key is never allowed to select an arbitrary CDN path.
        if self.profile == SiteProfile::Nettruyen {
            let expected = self
                .cover_key(source_manga_id)
                .ok_or(SourceError::MalformedUrl)?;
            if cover_key != expected {
                return Err(SourceError::MalformedUrl);
            }
            let url = format!("https://{NETTRUYEN_COVER_HOST}{cover_key}");
            return self.http.get_bytes(&url, COVER_MAX_BYTES).await;
        }
        // TruyenQQ and DocTruyen3Q covers keep the source's own path (the file
        // name embeds an upload timestamp or the numeric story id), but the host
        // is always this fixed, compile-time vetted one — never anything the
        // page supplied. Keys that are NOT `/cdn/…` are base-host paths (the
        // site also proxies covers under `/img/…`) and fall through below.
        let cover_prefix = match self.profile {
            SiteProfile::TruyenQQ => Some((TRUYENQQ_COVER_DOMAINS, "ebook/")),
            SiteProfile::Doctruyen3q => Some((DOCTRUYEN3Q_COVER_DOMAINS, "comics/")),
            _ => None,
        };
        if let Some((domains, path_prefix)) = cover_prefix {
            if let Some(rest) = cover_key.strip_prefix("/cdn/") {
                let (shard, path) = rest.split_once('/').ok_or(SourceError::MalformedUrl)?;
                let (label, token) = shard.split_once('.').ok_or(SourceError::MalformedUrl)?;
                let domain = domains
                    .iter()
                    .find(|(t, _)| *t == token)
                    .map(|(_, d)| *d)
                    .ok_or(SourceError::MalformedUrl)?;
                let safe = !label.is_empty()
                    && label.len() <= 16
                    && label
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                    && path.starts_with(path_prefix)
                    && !path.contains("//")
                    && !path.split('/').any(|seg| seg == ".." || seg == ".");
                if !safe {
                    return Err(SourceError::MalformedUrl);
                }
                let url = format!("https://{label}.{domain}/{path}");
                return self.http.get_bytes(&url, COVER_MAX_BYTES).await;
            }
        }
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
        // NetTruyen story pages ship only ~20 chapter anchors; the site's own
        // reader GETs the complete list as JSON from ComicService/ChapterList.
        // Fetched here BEFORE the Html document exists (same !Send rule), and
        // used in place of the sparse HTML anchors when it is richer.
        let nettruyen_json: Option<serde_json::Value> = if self.profile == SiteProfile::Nettruyen {
            let url = format!(
                "{}Comic/Services/ComicService.asmx/ChapterList?slug={}",
                self.base, source_manga_id
            );
            self.http.get_json(&url).await.ok()
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
        let cover_key = self.cover_key(source_manga_id).or_else(|| {
            document
                .select(&og_image)
                .next()
                .and_then(|node| node.value().attr("content"))
                .and_then(|content| self.cover_key_from_src(content))
        });

        // Madara tags its genre links; DocTruyen3Q puts the same list in its
        // category row. Both are plain text links, and the dedupe below keeps a
        // page that somehow matched both from listing a genre twice.
        let tag = Selector::parse("a[rel=\"tag\"], li.category p.detail-info > a").unwrap();
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
        if let Some(json) = &nettruyen_json {
            let via_json = chapters_from_nettruyen_json(json);
            // The story page's ~20 anchors are always a subset of the JSON
            // list; only the complete JSON manifest represents every chapter.
            if via_json.len() >= chapters.len() {
                chapters = via_json;
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
        if chapters.is_empty() {
            // A story page that yields NO chapter at all is a maintenance or
            // interstitial shell (HTTP 200, live Dilib 2026-10-02 served
            // "Hệ Thống Đang Bảo Trì" for every URL) — reporting it as a
            // genuine empty catalog makes a dead source look alive.
            return Err(SourceError::Upstream);
        }
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

/// TTT hides the image list of some chapters in an inline script instead of
/// `<img>` tags. Both live variants decrypt the same way — reverse the payload,
/// base64-decode it, XOR against a key, `JSON.parse` — and differ only in where
/// the key comes from and what the JSON looks like:
///
/// * `comic-app` chapters: `const d=atob('..')+atob('..')+…,s='<payload>'`,
///   giving `{"images":["url", …]}`.
/// * `c-w` chapters: `var k1='..',…,k4='..',d='<payload>'` with
///   `gk(){atob(k1)+…}`, giving `{"i":[{"u":"url", …}, …]}`.
///
/// So the key is every `atob('<literal>')` in the script (in both variants
/// those are exactly the four key chunks, and nothing else uses a literal), and
/// the payload is the longest quoted string. Both JSON shapes are read. If a
/// future variant stops matching, this returns `None` and the chapter fails
/// loudly as `Parse` rather than downloading the wrong things.
fn encrypted_reader_images(document: &Html) -> Option<Vec<String>> {
    use base64::Engine as _;
    let engine = base64::engine::general_purpose::STANDARD;
    let script_selector = Selector::parse("script").ok()?;
    let atob_literal = Regex::new(r#"atob\(['"]([^'"]+)['"]\)"#).ok()?;
    let named_key = Regex::new(r#"\bk\d+=['"]([A-Za-z0-9+/=]{4,})['"]"#).ok()?;
    let quoted = Regex::new(r#"['"]([A-Za-z0-9+/=]{200,})['"]"#).ok()?;

    for node in document.select(&script_selector) {
        let script = node.text().collect::<String>();
        if !script.contains("String.fromCharCode(") || !script.contains("atob(") {
            continue;
        }
        let mut key: Vec<u8> = Vec::new();
        let decode_into = |key: &mut Vec<u8>, text: &str| {
            if let Ok(bytes) = engine.decode(text.trim()) {
                // Key chunks are short; anything large is payload material.
                if !bytes.is_empty() && bytes.len() <= 32 {
                    key.extend(bytes);
                }
            }
        };
        // Variant 1 inlines the chunks: `atob('..')+atob('..')`.
        for capture in atob_literal.captures_iter(&script) {
            decode_into(&mut key, &capture[1]);
        }
        // Variant 2 names them and calls `atob(k1)`, so the literals sit in
        // `k1='..'` assignments instead.
        if key.is_empty() {
            for capture in named_key.captures_iter(&script) {
                decode_into(&mut key, &capture[1]);
            }
        }
        let payload = quoted
            .captures_iter(&script)
            .map(|capture| capture[1].to_string())
            .max_by_key(|entry| entry.len())?;
        if key.is_empty() {
            continue;
        }
        let reversed: String = payload.chars().rev().collect();
        let bytes = engine.decode(&reversed).ok()?;
        let plain: Vec<u8> = bytes
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ key[index % key.len()])
            .collect();
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&plain) else {
            continue;
        };
        let entries = value
            .get("images")
            .or_else(|| value.get("i"))
            .and_then(|list| list.as_array())?;
        let urls: Vec<String> = entries
            .iter()
            .filter_map(|entry| match entry {
                serde_json::Value::String(url) => Some(url.clone()),
                object => object
                    .get("u")
                    .and_then(|url| url.as_str())
                    .map(str::to_string),
            })
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty())
            .collect();
        if !urls.is_empty() {
            return Some(urls);
        }
    }
    None
}

/// Collect reader images into `out`, resolving them against `base`.
///
/// `reject_foreign` decides what an image on a host outside the source's
/// allowlist MEANS. Where the theme marks its pages (class/box selectors) an
/// unexpected host is the whole signal that the CDN moved, so the manifest
/// fails rather than half-downloading. The unmarked fallback selects every
/// image in the reader container, which on a live site also catches rotating
/// ad creatives — there, one ad must not sink a 230-page chapter, so foreign
/// hosts are skipped instead. Skipping never widens what we request: a foreign
/// image is simply not downloaded, and if nothing allowed remains the caller
/// still fails with `Parse`.
fn collect_page_urls(
    document: &Html,
    selector: &Selector,
    base: &Url,
    allowed_hosts: &[String],
    reject_foreign: bool,
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
            if reject_foreign {
                return Err(SourceError::Parse);
            }
            continue;
        }
        if seen.insert(joined.clone()) {
            out.push(joined.to_string());
        }
    }
    Ok(())
}

/// Map NetTruyen's `ChapterList` JSON (`{"data":[{chapter_slug, chapter_name,
/// chapter_num, ...}]}`) into chapters. Malformed or non-string entries are
/// skipped; the caller only keeps the result when richer than the HTML anchors.
fn chapters_from_nettruyen_json(json: &serde_json::Value) -> Vec<RemoteChapter> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut chapters: Vec<RemoteChapter> = Vec::new();
    let Some(entries) = json.get("data").and_then(|data| data.as_array()) else {
        return chapters;
    };
    for entry in entries {
        let Some(slug) = entry.get("chapter_slug").and_then(|v| v.as_str()) else {
            continue;
        };
        if !valid_slug(slug) || !seen.insert(slug.to_string()) {
            continue;
        }
        let number = match entry.get("chapter_num") {
            Some(serde_json::Value::Number(n)) => n.as_f64(),
            Some(serde_json::Value::String(s)) => s.parse::<f64>().ok(),
            _ => None,
        };
        let title = entry
            .get("chapter_name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .unwrap_or_else(|| {
                format!(
                    "Chương {}",
                    number
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| slug.to_string())
                )
            });
        chapters.push(RemoteChapter {
            source_chapter_id: slug.to_string(),
            title,
            number,
            order: 0,
            published_at: None,
        });
        if chapters.len() >= MAX_PAGE_COUNT {
            break;
        }
    }
    chapters
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

    /// The built-in's shipped image hosts, split the way the registry splits
    /// them: `live()` validates every entry as ONE exact hostname.
    fn ttt_image_hosts() -> Vec<String> {
        crate::sources::registry::TTT_IMAGE_HOSTS
            .split(',')
            .map(|entry| entry.trim().to_string())
            .collect()
    }

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

    /// A fractional chapter written with hyphen-separated digits in the slug
    /// ("chap-2-1" = chapter 2.1) must NOT collapse to the trailing part —
    /// that made every "X.1" chapter number 1 and saved chapter 2.1 as
    /// chapter-1.cbz over chapter 1.1's file (live bug, 2026-10-02).
    #[test]
    fn chapter_number_parses_hyphen_decimals_not_last_segment() {
        assert_eq!(
            chapter_number_from_slug("ninja-loan-thi-ban-dep-8777-chap-2-1"),
            Some(2.1)
        );
        assert_eq!(
            chapter_number_from_slug("ninja-loan-thi-ban-dep-8777-chap-1-1"),
            Some(1.1)
        );
        assert_eq!(chapter_number_from_slug("tap-56"), Some(56.0));
        assert_eq!(chapter_number_from_slug("chapter-12-5"), Some(12.5));
        assert_eq!(chapter_number_from_slug("one-piece-chap-590"), Some(590.0));
        assert_eq!(chapter_number_from_slug("tap-3-75"), Some(3.75));
        assert_eq!(chapter_number_from_slug("chap-10-2"), Some(10.2));
        // Three or more numeric groups are ambiguous ("1.2.3"? "12" part 3?) —
        // a missing number only sorts the chapter as unnumbered, a WRONG one
        // corrupts the export filename, so refuse to guess.
        assert_eq!(chapter_number_from_slug("chap-1-2-3"), None);
        assert_eq!(chapter_number_from_slug("chapter-1.2.3"), None);
        assert_eq!(chapter_number_from_slug("chap-1-2.3"), None);
    }

    /// A 200 page with story markup but zero chapter anchors is a maintenance
    /// shell (live Dilib, 2026-10-02) — it must fail as an upstream problem,
    /// never surface as a real "0 chapters" catalog.
    #[tokio::test]
    async fn detail_of_a_chapterless_shell_page_is_upstream_error() {
        let (provider, _) = adapter(vec![Ok(
            "<html><body><h1>Hệ Thống Đang Bảo Trì</h1></body></html>",
        )]);
        let error = provider
            .manga_detail("dao-hai-tac-one-piece-14728")
            .await
            .expect_err("a shell page is not a catalog");
        assert!(
            matches!(error, SourceError::Upstream),
            "shell pages must read as source trouble: {error:?}"
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
            &ttt_image_hosts(),
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

    /// TTT chapter downloads started failing with SOURCE_PARSE_ERROR on
    /// 2026-09-30 although the SAME source delivered 188–234-page chapters on
    /// 2026-09-25. Print what the live reader page actually is now, so the fix
    /// follows the source instead of a guess.
    #[tokio::test]
    #[ignore = "live network probe: why TTT chapter pages no longer parse"]
    async fn live_builtin_ttt_chapter_diagnostic() {
        let adapter = MadaraGeneric::live(
            "Truyen Tuoi Tho",
            "https://truyentuoitho.com/",
            &ttt_image_hosts(),
        )
        .unwrap();
        for chapter in ["tap-9", "tap-10"] {
            match adapter.page_manifest("ninja-loan-thi", chapter).await {
                Ok(manifest) => println!("DIAG {chapter} ok pages={}", manifest.pages.len()),
                Err(error) => println!("DIAG {chapter} err={}", error.code()),
            }
        }
        let url = adapter.chapter_url("ninja-loan-thi", "tap-9");
        let html = adapter
            .http
            .get_text(&url)
            .await
            .expect("chapter page fetch");
        {
            let document = Html::parse_document(&html);
            let title = Selector::parse("title").unwrap();
            println!(
                "DIAG url={url} bytes={} title={:?}",
                html.len(),
                document.select(&title).next().map(|n| n
                    .text()
                    .collect::<String>()
                    .trim()
                    .to_string())
            );
            for sel in [
                ".reading-content",
                "img.wp-manga-chapter-img",
                ".page-break",
                ".container-chapter-reader",
                "img",
            ] {
                let count = document.select(&Selector::parse(sel).unwrap()).count();
                println!("DIAG sel {sel} = {count}");
            }
            let mut hosts: std::collections::BTreeMap<String, usize> = Default::default();
            let imgs = Selector::parse("img").unwrap();
            for node in document.select(&imgs) {
                let Some(raw) = ["src", "data-src", "data-lazy-src", "data-original"]
                    .iter()
                    .filter_map(|attr| node.value().attr(attr))
                    .map(str::trim)
                    .find(|value| !value.is_empty())
                else {
                    continue;
                };
                if let Ok(joined) = adapter.base.join(raw) {
                    *hosts
                        .entry(joined.host_str().unwrap_or("-").to_string())
                        .or_default() += 1;
                }
            }
            println!("DIAG img hosts {hosts:?}");
            println!("DIAG allowed={:?}", adapter.allowed_image_hosts());
            if let Some(node) = document
                .select(&Selector::parse(".reading-content").unwrap())
                .next()
            {
                let snippet: String = node.html().chars().take(1200).collect();
                println!("DIAG reading-content = {snippet}");
            }
        }
    }

    /// The app downloads chapters with TWO concurrent workers; this probe runs
    /// the same shape against five real chapters, several rounds, to see
    /// whether a manifest that parses fine on its own fails under load — the
    /// failure mode the user hit on 2026-09-30 (five SOURCE_PARSE_ERROR tasks
    /// in ten seconds, while a sequential probe returned 234 pages).
    #[tokio::test]
    #[ignore = "live network probe: does TTT parsing fail under concurrent chapter fetches"]
    async fn live_builtin_ttt_concurrency_diagnostic() {
        let adapter = Arc::new(
            MadaraGeneric::live(
                "Truyen Tuoi Tho",
                "https://truyentuoitho.com/",
                &ttt_image_hosts(),
            )
            .unwrap(),
        );
        for round in 1..=3 {
            let mut handles = Vec::new();
            for chapter in ["tap-6", "tap-7", "tap-8", "tap-9", "tap-10"] {
                let adapter = adapter.clone();
                handles.push(tokio::spawn(async move {
                    let started = std::time::Instant::now();
                    let outcome = adapter.page_manifest("ninja-loan-thi", chapter).await;
                    (chapter, started.elapsed().as_millis(), outcome)
                }));
            }
            let mut ok = 0;
            let mut failed: Vec<String> = Vec::new();
            for handle in handles {
                let (chapter, millis, outcome) = handle.await.unwrap();
                match outcome {
                    Ok(manifest) => {
                        ok += 1;
                        println!(
                            "CONC r{round} {chapter} ok pages={} in {millis}ms",
                            manifest.pages.len()
                        );
                    }
                    Err(error) => {
                        println!("CONC r{round} {chapter} ERR {} in {millis}ms", error.code());
                        failed.push(chapter.to_string());
                    }
                }
            }
            println!("CONC r{round} summary ok={ok} failed={failed:?}");
        }
    }

    /// End-to-end proof of the 2026-10-01 fix: build the adapter the way the app
    /// does — from the user's persisted row, whose `image_hosts` is NULL — and
    /// read a real chapter. Before the fix this failed with SOURCE_PARSE_ERROR
    /// because the shipped CDN host was dropped on the way.
    #[tokio::test]
    #[ignore = "live probe: a registry-built TTT adapter parses real chapters"]
    async fn live_registry_builtin_parses_ttt_chapter() {
        let row = crate::db::repositories::source_repo::SourceRow {
            id: "madara-truyentuoitho-com".to_string(),
            display_name: "Truyen Tuoi Tho".to_string(),
            adapter: "madara".to_string(),
            base_url: None,
            image_hosts: None,
            enabled: true,
        };
        let registry = crate::sources::registry::SourceRegistry::from_rows(&[row]);
        let adapter = registry
            .get("madara-truyentuoitho-com")
            .expect("TTT is a built-in");
        let raw = MadaraGeneric::live(
            "Truyen Tuoi Tho",
            "https://truyentuoitho.com/",
            &ttt_image_hosts(),
        )
        .unwrap();
        // 61 and 65 are the chapters the user's retry still fails on (build
        // with the NULL-host and ad-skip fixes), 32 is a known good one. The
        // rest sample the SOURCE_NETWORK burst: those may simply retry fine.
        for chapter in [
            "tap-16", "tap-17", "tap-30", "tap-32", "tap-44", "tap-47", "tap-51", "tap-56",
            "tap-57", "tap-61", "tap-64", "tap-65",
        ] {
            match adapter.page_manifest("ninja-loan-thi", chapter).await {
                Ok(manifest) => {
                    println!("LIVE REGISTRY {chapter} pages={}", manifest.pages.len())
                }
                Err(error) => println!("LIVE REGISTRY {chapter} ERR {}", error.code()),
            }
            // Count the raw candidates too: a chapter over MAX_PAGE_COUNT is
            // refused as Parse, which looks identical to a markup failure.
            let url = format!("https://truyentuoitho.com/manga/ninja-loan-thi/{chapter}/");
            let html = match raw.http.get_text(&url).await {
                Ok(html) => html,
                Err(error) => {
                    println!("LIVE REGISTRY {chapter} raw fetch err {error:?}");
                    continue;
                }
            };
            let document = Html::parse_document(&html);
            let container = Selector::parse(".reading-content").unwrap();
            let imgs = Selector::parse(".reading-content img").unwrap();
            println!(
                "LIVE REGISTRY {chapter} bytes={} container={} imgs_in_container={}",
                html.len(),
                document.select(&container).count(),
                document.select(&imgs).count()
            );
            if document.select(&imgs).count() == 0 {
                // The container exists but holds no image: dump what it DOES
                // hold, and where the page's images actually are.
                if let Some(node) = document.select(&container).next() {
                    let snippet: String = node.html().chars().take(1500).collect();
                    println!("LIVE REGISTRY {chapter} container html = {snippet}");
                }
                let mut hosts: std::collections::BTreeMap<String, usize> = Default::default();
                let all = Selector::parse("img").unwrap();
                for node in document.select(&all) {
                    let Some(raw) = ["src", "data-src", "data-lazy-src", "data-original"]
                        .iter()
                        .filter_map(|attr| node.value().attr(attr))
                        .map(str::trim)
                        .find(|value| !value.is_empty())
                    else {
                        continue;
                    };
                    if let Ok(joined) = Url::parse(raw) {
                        *hosts
                            .entry(joined.host_str().unwrap_or("-").to_string())
                            .or_default() += 1;
                    }
                }
                let classes: Vec<String> = document
                    .select(&Selector::parse("div[class], span[class], iframe").unwrap())
                    .take(400)
                    .filter_map(|node| node.value().attr("class").map(str::to_string))
                    .collect::<std::collections::BTreeSet<String>>()
                    .into_iter()
                    .take(30)
                    .collect();
                println!("LIVE REGISTRY {chapter} page img hosts={hosts:?}");
                println!(
                    "LIVE REGISTRY {chapter} iframes={}",
                    document.select(&Selector::parse("iframe").unwrap()).count()
                );
                println!("LIVE REGISTRY {chapter} class sample={classes:?}");
            }
        }
    }

    /// Live journey for the DocTruyen3Q built-in, built the way the app builds
    /// it (registry + persisted row): folded Vietnamese search → story detail →
    /// inline chapter list → page manifest → REAL image bytes. The last step is
    /// the one that matters: the shards answer 403 without the source's own
    /// Referer, so a manifest alone proves nothing.
    #[tokio::test]
    #[ignore = "live probe: DocTruyen3Q search → detail → pages → image bytes"]
    async fn live_builtin_d3q_journey_probe() {
        let row = crate::db::repositories::source_repo::SourceRow {
            id: "doctruyen3q".to_string(),
            display_name: "DocTruyen3Q".to_string(),
            adapter: "doctruyen3q".to_string(),
            base_url: None,
            image_hosts: None,
            enabled: true,
        };
        let registry = crate::sources::registry::SourceRegistry::from_rows(&[row]);
        let adapter = registry.get("doctruyen3q").expect("built-in is registered");
        // Several queries on purpose: an empty result here can mean the card
        // selector is wrong OR that this site's index simply does not match the
        // phrase, and the two need different fixes.
        let mut page = adapter.search("naruto", 1).await.unwrap();
        println!(
            "D3Q search \"naruto\" items={} has_more={}",
            page.items.len(),
            page.has_more
        );
        if page.items.is_empty() {
            page = adapter.search("võ luyện", 1).await.unwrap();
            println!("D3Q search folded \"võ luyện\" items={}", page.items.len());
        }
        let first = page.items.first().expect("the search finds a story");
        println!(
            "D3Q search items={} id={} title={} cover={:?}",
            page.items.len(),
            first.source_manga_id,
            first.title,
            first.cover_key
        );
        let detail = adapter.manga_detail(&first.source_manga_id).await.unwrap();
        println!(
            "D3Q detail title={} chapters={} genres={} desc={}",
            detail.summary.title,
            detail.chapters.len(),
            detail.genres.len(),
            detail.description.is_some()
        );
        let chapter = detail
            .chapters
            .iter()
            .rev()
            .find(|chapter| chapter.source_chapter_id.starts_with("chapter-"))
            .expect("at least one chapter");
        let manifest = adapter
            .page_manifest(&first.source_manga_id, &chapter.source_chapter_id)
            .await
            .unwrap();
        let first_url = &manifest.pages[0].url;
        let bytes = adapter.fetch_image(first_url).await.unwrap();
        println!(
            "D3Q chapter={} pages={} first={} bytes={}",
            chapter.source_chapter_id,
            manifest.pages.len(),
            first_url,
            bytes.len()
        );
        assert!(bytes.len() > 1024, "the shard must serve a real image");
        if let Some(key) = &first.cover_key {
            let cover = adapter
                .fetch_cover(&first.source_manga_id, key)
                .await
                .map(|bytes| bytes.len());
            println!("D3Q cover key={key} bytes={cover:?}");
            assert!(cover.is_ok_and(|size| size > 512), "cover must fetch");
        }
    }

    /// What does a Mankichi chapter page actually contain? The user's whole
    /// queue of this title fails with `total_pages = 0`, so distinguish the
    /// three live possibilities: the JS reader changed shape, the images moved
    /// to a host the built-in never registered, or the page is not a reader at
    /// all. Built through the registry like the app does.
    #[tokio::test]
    #[ignore = "live probe: why a TTT chapter yields zero pages"]
    async fn live_builtin_ttt_zero_page_chapter_probe() {
        let row = crate::db::repositories::source_repo::SourceRow {
            id: "madara-truyentuoitho-com".to_string(),
            display_name: "Truyen Tuoi Tho".to_string(),
            adapter: "madara".to_string(),
            base_url: None,
            image_hosts: None,
            enabled: true,
        };
        let registry = crate::sources::registry::SourceRegistry::from_rows(&[row]);
        let adapter = registry
            .get("madara-truyentuoitho-com")
            .expect("TTT is a built-in");
        let raw = MadaraGeneric::live(
            "Truyen Tuoi Tho",
            "https://truyentuoitho.com/",
            &ttt_image_hosts(),
        )
        .unwrap();

        for chapter in [
            "tap-1-dai-tuong-ra-doi",
            "tap-2-ba-trum-mito",
            "tap-3-mankichi-bi-giao-huan",
            "tap-4-mat-troi-khong-bao-gio-lan",
            "tap-5-binh-minh-dai-bi-kich",
            "tap-6-thuc-luc-cua-than-chet",
            "tap-7-chuong-phu-si-thieng-lieng",
        ] {
            match adapter
                .page_manifest("mankichi-dai-tuong-nhoc-con-full", chapter)
                .await
            {
                Ok(manifest) => {
                    // A manifest is only half the proof: the shard must
                    // actually serve bytes to this client (Referer/hotlink
                    // rules).
                    let first = &manifest.pages[0].url;
                    let fetched = raw.fetch_image(first).await.map(|b| b.len());
                    println!(
                        "ZP {chapter} pages={} first={first} fetched={fetched:?}",
                        manifest.pages.len()
                    );
                }
                Err(error) => println!("ZP {chapter} ERR {}", error.code()),
            }
            let url = format!(
                "https://truyentuoitho.com/manga/mankichi-dai-tuong-nhoc-con-full/{chapter}/"
            );
            let html = match raw.http.get_text(&url).await {
                Ok(html) => html,
                Err(error) => {
                    println!("ZP {chapter} raw fetch err {error:?}");
                    continue;
                }
            };
            let document = Html::parse_document(&html);
            let title = Selector::parse("title").unwrap();
            println!(
                "ZP {chapter} bytes={} title={:?}",
                html.len(),
                document.select(&title).next().map(|n| n
                    .text()
                    .collect::<String>()
                    .trim()
                    .to_string())
            );
            for sel in [
                ".reading-content",
                ".reading-content img",
                "img.wp-manga-chapter-img",
                ".page-break",
                ".container-chapter-reader",
                "script",
                "iframe",
            ] {
                println!(
                    "ZP {chapter} sel {sel} = {}",
                    document.select(&Selector::parse(sel).unwrap()).count()
                );
            }
            // Where the page's own <img> tags point, and what the decoder finds.
            let mut hosts: std::collections::BTreeMap<String, usize> = Default::default();
            let imgs = Selector::parse("img").unwrap();
            for node in document.select(&imgs) {
                let Some(value) = ["src", "data-src", "data-lazy-src", "data-original"]
                    .iter()
                    .filter_map(|attr| node.value().attr(attr))
                    .map(str::trim)
                    .find(|value| !value.is_empty())
                else {
                    continue;
                };
                if let Ok(joined) = raw.base.join(value) {
                    *hosts
                        .entry(joined.host_str().unwrap_or("-").to_string())
                        .or_default() += 1;
                }
            }
            println!("ZP {chapter} img hosts={hosts:?}");
            match encrypted_reader_images(&document) {
                Some(urls) => {
                    let decoded: std::collections::BTreeMap<String, usize> = urls
                        .iter()
                        .filter_map(|u| Url::parse(u).ok())
                        .fold(std::collections::BTreeMap::new(), |mut acc, u| {
                            *acc.entry(u.host_str().unwrap_or("-").to_string())
                                .or_default() += 1;
                            acc
                        });
                    println!(
                        "ZP {chapter} DECODED {} urls, hosts={decoded:?}, first={:?}",
                        urls.len(),
                        urls.first()
                    );
                }
                None => println!("ZP {chapter} DECODED none"),
            }
            println!("ZP {chapter} allowed={:?}", raw.allowed_image_hosts());
            let container_imgs = document
                .select(&Selector::parse(".reading-content img").unwrap())
                .count();
            println!("ZP {chapter} container imgs={container_imgs}");
            if container_imgs == 0 {
                if let Some(node) = document
                    .select(&Selector::parse(".reading-content").unwrap())
                    .next()
                {
                    let snippet: String = node.html().chars().take(2000).collect();
                    println!("ZP {chapter} container html = {snippet}");
                }
            }
        }
    }

    /// Dump the obfuscated reader script of a real chapter to disk so the
    /// decoding scheme can be studied offline instead of guessed from a
    /// truncated log line.
    #[tokio::test]
    #[ignore = "live probe: save TTT's JS reader script for analysis"]
    async fn live_builtin_ttt_dump_reader_script() {
        let adapter = MadaraGeneric::live(
            "Truyen Tuoi Tho",
            "https://truyentuoitho.com/",
            &ttt_image_hosts(),
        )
        .unwrap();
        for chapter in ["tap-56", "tap-61", "tap-65"] {
            let url = format!("https://truyentuoitho.com/manga/ninja-loan-thi/{chapter}/");
            let html = adapter
                .http
                .get_text(&url)
                .await
                .unwrap_or_else(|error| panic!("{chapter} fetch: {}", error.code()));
            let document = Html::parse_document(&html);
            let container = Selector::parse(".reading-content").unwrap();
            let node = document
                .select(&container)
                .next()
                .expect("reading-content must exist");
            std::fs::write(
                format!("D:/Project/Mangav2/.scratch/{chapter}-container.html"),
                node.html(),
            )
            .expect("write container");
            let scripts = Selector::parse("script").unwrap();
            let mut longest = String::new();
            for script in document.select(&scripts) {
                let text = script.text().collect::<String>();
                if text.len() > longest.len() {
                    longest = text;
                }
            }
            std::fs::write(
                format!("D:/Project/Mangav2/.scratch/{chapter}-script.js"),
                &longest,
            )
            .expect("write script");
            println!(
                "DUMP {chapter} container={} script={}",
                node.html().len(),
                longest.len()
            );
        }
    }

    /// Encode a payload the way the site does: XOR with the key, base64, then
    /// reverse the string. The key is handed back split into four `atob`
    /// literals, which is how both live variants ship it.
    fn reader_script(json: &str, container_id: &str) -> String {
        use base64::Engine as _;
        let engine = base64::engine::general_purpose::STANDARD;
        let key = "urqGM1maNScG7tAI";
        let xored: Vec<u8> = json
            .bytes()
            .enumerate()
            .map(|(index, byte)| byte ^ key.as_bytes()[index % key.len()])
            .collect();
        let payload: String = engine.encode(&xored).chars().rev().collect();
        let parts: Vec<String> = key
            .as_bytes()
            .chunks(4)
            .map(|chunk| format!("atob('{}')", engine.encode(chunk)))
            .collect();
        format!(
            "<html><body><div class=\"reading-content\"><script>(function(){{var c=document.getElementById('{container_id}');var d={};s='{payload}';var b=atob(s);var o='';for(var i=0;i<b.length;i++){{o+=String.fromCharCode(b.charCodeAt(i)^i)}}JSON.parse(o)}})();</script></div></body></html>",
            parts.join("+")
        )
    }

    /// Round-trips variant 1 (the `comic-app` chapters, verified against
    /// tap-61 live 2026-10-01): `{{"images":["url", …]}}`.
    #[test]
    fn encrypted_reader_images_decodes_the_images_list_shape() {
        let json = format!(
            "{{\"images\":[{}]}}",
            (0..6)
                .map(|index| format!("\"https://img.resourcehub.shop/p{index}.jpg\""))
                .collect::<Vec<_>>()
                .join(",")
        );
        let document = Html::parse_document(&reader_script(&json, "cc"));
        let images = encrypted_reader_images(&document).expect("the payload must decode");
        assert_eq!(images.len(), 6);
        assert_eq!(images[0], "https://img.resourcehub.shop/p0.jpg");
        assert_eq!(images[5], "https://img.resourcehub.shop/p5.jpg");
    }

    /// Variant 2 (the `c-w` chapters, verified against tap-56 live
    /// 2026-10-01): `{{"i":[{{"u":"url","w":…,"h":…}}, …]}}`.
    #[test]
    fn encrypted_reader_images_decodes_the_object_list_shape() {
        let items = (0..6)
            .map(|index| {
                format!(
                    "{{\"u\":\"https://truyentuoitho.anooymi.workers.dev/img?d=AAA{index}\",\"w\":800,\"h\":1200}}"
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let json = format!("{{\"i\":[{items}]}}");
        let document = Html::parse_document(&reader_script(&json, "c-w"));
        let images = encrypted_reader_images(&document).expect("the payload must decode");
        assert_eq!(images.len(), 6);
        assert!(images[0].starts_with("https://truyentuoitho.anooymi.workers.dev/"));
    }

    #[test]
    fn encrypted_reader_images_ignores_pages_without_the_js_reader() {
        let document = Html::parse_document(
            r#"<html><body><div class="reading-content"><img src="/a.jpg"/></div></body></html>"#,
        );
        assert!(encrypted_reader_images(&document).is_none());
    }

    /// Live proof of the decrypt path on the chapters that were failing.
    #[tokio::test]
    #[ignore = "live probe: TTT's script-rendered chapters decode to real pages"]
    async fn live_builtin_ttt_encrypted_chapter_parses() {
        let adapter = MadaraGeneric::live(
            "Truyen Tuoi Tho",
            "https://truyentuoitho.com/",
            &ttt_image_hosts(),
        )
        .unwrap();
        for chapter in ["tap-56", "tap-61", "tap-65"] {
            let manifest = adapter
                .page_manifest("ninja-loan-thi", chapter)
                .await
                .unwrap_or_else(|error| panic!("{chapter} still fails: {}", error.code()));
            println!("ENCRYPTED {chapter} pages={}", manifest.pages.len());
            assert!(manifest.pages.len() > 100, "a chapter is ~200+ pages");
            assert!(manifest
                .pages
                .iter()
                .all(|page| page.url.starts_with("https://")));
            // A decrypted URL list is only useful if the worker really serves
            // the image through the app's own host policy.
            let first = &manifest.pages[0].url;
            let bytes = adapter
                .fetch_image(first)
                .await
                .unwrap_or_else(|error| panic!("{chapter} image fetch failed: {}", error.code()));
            println!(
                "ENCRYPTED {chapter} page1 bytes={} host={}",
                bytes.len(),
                first.split('/').nth(2).unwrap_or("-")
            );
            assert!(bytes.len() > 5_000, "a manga page is far bigger than that");
        }
    }

    /// TTT serves some newer chapters with a JS reader: `.reading-content`
    /// holds only `<div id="comic-app"><div id="cc">` plus a script that
    /// assembles the image URLs from `atob(...)` chunks, so the plain HTML has
    /// ZERO `<img>`. Before writing a decoder, find out whether the theme also
    /// answers one of its own plain-page / ajax routes for the same chapter —
    /// that is a far more stable seam than reverse-engineering the script.
    #[tokio::test]
    #[ignore = "live probe: does TTT expose a non-JS route for its obfuscated chapters"]
    async fn live_builtin_ttt_js_reader_endpoints() {
        let adapter = MadaraGeneric::live(
            "Truyen Tuoi Tho",
            "https://truyentuoitho.com/",
            &ttt_image_hosts(),
        )
        .unwrap();
        let base = "https://truyentuoitho.com/manga/ninja-loan-thi/tap-61/";
        for url in [
            base.to_string(),
            format!("{base}?style=list"),
            format!("{base}?mode=list"),
            format!("{base}?page-chap=1"),
            format!("{base}?chapter_id=6762"),
            format!("{base}01/"),
            format!("{}?p=6762", adapter.base),
            format!("{}wp-json/wp/v2/posts/6762", adapter.base),
        ] {
            match adapter.http.get_text(&url).await {
                Ok(html) => {
                    let document = Html::parse_document(&html);
                    let imgs = Selector::parse(".reading-content img").unwrap();
                    let any = Selector::parse("img").unwrap();
                    let script_only = html.contains("id=\"comic-app\"");
                    println!(
                        "READER {} -> bytes={} reading_imgs={} all_imgs={} js_reader={script_only}",
                        url.replace("https://truyentuoitho.com", ""),
                        html.len(),
                        document.select(&imgs).count(),
                        document.select(&any).count(),
                    );
                }
                Err(error) => println!(
                    "READER {} -> err {}",
                    url.replace("https://truyentuoitho.com", ""),
                    error.code()
                ),
            }
        }
        for action in [
            "madara_get_content",
            "manga_get_chapters",
            "get_reader_images",
        ] {
            let result = adapter
                .http
                .post_form_text(
                    &format!("{}wp-admin/admin-ajax.php", adapter.base),
                    &[
                        ("action", action),
                        ("post", "6762"),
                        ("chapter", "tap-61"),
                        ("chapter_id", "6762"),
                    ],
                )
                .await;
            match result {
                Ok(body) => println!(
                    "READER ajax {action} -> bytes={} head={}",
                    body.len(),
                    body.chars().take(160).collect::<String>().trim()
                ),
                Err(error) => println!("READER ajax {action} -> err {}", error.code()),
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

    /// Live 2026-09-30: TTT stopped printing the `wp-manga-chapter-img` class,
    /// so its pages are read by the unmarked fallback that takes every image in
    /// the reader container — the same container rotating ad creatives live in.
    /// One ad on an unregistered host used to fail a 234-page chapter.
    #[tokio::test]
    async fn loose_reader_pages_skip_an_ad_from_another_host_instead_of_failing() {
        let (provider, _) = adapter(vec![Ok(r#"<div class="reading-content">
                 <img src="/uploads/p1.jpg"/>
                 <img src="https://ads.test/banner.gif"/>
                 <img src="https://truyentuoitho.com/uploads/p2.jpg"/>
               </div>"#)]);
        let manifest = provider.page_manifest("m", "tap-1").await.unwrap();
        assert_eq!(manifest.pages.len(), 2, "the ad is skipped, not fatal");
        assert!(
            manifest
                .pages
                .iter()
                .all(|page| !page.url.contains("ads.test")),
            "a host outside the allowlist is never queued for download"
        );
    }

    /// Skipping foreign hosts must not turn a moved CDN into a silent
    /// short chapter: with nothing allowed left, the manifest still fails.
    #[tokio::test]
    async fn loose_reader_pages_with_only_foreign_hosts_still_fail() {
        let (provider, _) = adapter(vec![Ok(
            r#"<div class="reading-content"><img src="https://cdn-moved.test/p1.jpg"/></div>"#,
        )]);
        assert!(matches!(
            provider.page_manifest("m", "tap-1").await,
            Err(SourceError::Parse)
        ));
    }

    /// 2026-10-05: TTT serves its "(Full)" titles from a third shard,
    /// `img.truyentuoitho.com`, wrapped in `.page-box` with no Madara image
    /// class. A subdomain of the base host is not an exact-host match, so
    /// before the shard was registered every one of the 346 pages looked
    /// foreign and the chapter died as SOURCE_PARSE_ERROR.
    #[tokio::test]
    async fn page_box_images_on_the_truyentuoitho_shard_are_accepted() {
        let provider = built_in_ttt(vec![Ok(r#"<div class="reading-content">
             <div class="page-box"><img src="https://img.truyentuoitho.com/mankichi/01.%20Tap/01-001.webp"/></div>
             <div class="page-box"><img src="https://img.truyentuoitho.com/mankichi/01.%20Tap/01-002.webp"/></div>
           </div>"#)]);
        let manifest = provider.page_manifest("m", "tap-1").await.unwrap();
        assert_eq!(manifest.pages.len(), 2);
        assert!(manifest
            .pages
            .iter()
            .all(|page| page.url.starts_with("https://img.truyentuoitho.com/")));
    }

    /// The rule that hid the outage: a registered subdomain used to be dropped
    /// as "already covered by the base host", but Madara pages are matched by
    /// EXACT host — so the drop silently un-registered TTT's real image shard.
    #[test]
    fn a_registered_subdomain_of_the_base_host_stays_allowed() {
        let provider = MadaraGeneric::live(
            "Truyen Tuoi Tho",
            "https://truyentuoitho.com/",
            &[
                "img.truyentuoitho.com".to_string(),
                "truyentuoitho.com".to_string(),
            ],
        )
        .unwrap();
        let allowed = provider.allowed_image_hosts();
        assert!(
            allowed.iter().any(|h| h == "img.truyentuoitho.com"),
            "an explicitly registered shard must survive, got {allowed:?}"
        );
        assert_eq!(
            allowed
                .iter()
                .filter(|host| host.as_str() == "truyentuoitho.com")
                .count(),
            1,
            "the base host is allowed once, never duplicated by an entry"
        );
    }

    /// The page cap is a guard, not a crop. Collecting exactly up to the limit
    /// and returning it would export a truncated CBZ that later reports
    /// "completed" — the user must instead see the chapter fail.
    #[tokio::test]
    async fn a_chapter_over_the_page_cap_fails_instead_of_being_cropped() {
        let pages: String = (0..=MAX_PAGE_COUNT)
            .map(|index| format!(r#"<img src="https://img.truyentuoitho.com/p{index}.webp"/>"#))
            .collect();
        let html = format!(r#"<div class="reading-content">{pages}</div>"#);
        let provider = built_in_ttt_owned(html);
        assert!(matches!(
            provider.page_manifest("m", "tap-1").await,
            Err(SourceError::Parse)
        ));
    }

    /// A Madara adapter carrying the built-in's SHIPPED image-host list, which
    /// is what the app actually runs — `adapter()` deliberately has none.
    fn built_in_ttt(replies: Vec<Result<&'static str, &'static str>>) -> MadaraGeneric {
        let transport = Arc::new(MockTransport::with(replies));
        MadaraGeneric::with_transport_and_image_hosts(
            "Truyen Tuoi Tho",
            "https://truyentuoitho.com/",
            transport,
            &ttt_image_hosts(),
        )
        .unwrap()
    }

    fn built_in_ttt_owned(html: String) -> MadaraGeneric {
        let transport = Arc::new(MockTransport::with(vec![Ok(html.as_str())]));
        MadaraGeneric::with_transport_and_image_hosts(
            "Truyen Tuoi Tho",
            "https://truyentuoitho.com/",
            transport,
            &ttt_image_hosts(),
        )
        .unwrap()
    }

    /// The exact state the live failure had: TTT serves every page from
    /// `img.resourcehub.shop`, and a built-in row whose image-host column is
    /// NULL produced precisely this adapter — base host only. Every page then
    /// looked like a foreign ad and the chapter died. `registry::tests` is what
    /// stops that row shape from being built; this pins what it would do.
    #[tokio::test]
    async fn pages_on_a_host_the_source_never_registered_fail_the_chapter() {
        let (provider, _) = adapter(vec![Ok(r#"<div class="reading-content">
                 <img src="https://img.resourcehub.shop/truyen/1.jpg"/>
                 <img src="https://img.resourcehub.shop/truyen/2.jpg"/>
               </div>"#)]);
        assert!(matches!(
            provider.page_manifest("ninja-loan-thi", "tap-8").await,
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
            "https://truyenqqko.com/",
            transport.clone(),
            SiteProfile::TruyenQQ,
        )
        .unwrap();
        (provider, transport)
    }

    fn nt(replies: Vec<Result<&'static str, &'static str>>) -> (MadaraGeneric, Arc<MockTransport>) {
        let transport = Arc::new(MockTransport::with(replies));
        let provider = MadaraGeneric::with_transport_profile(
            "NetTruyen",
            "https://nettruyen.gg/",
            transport.clone(),
            SiteProfile::Nettruyen,
        )
        .unwrap();
        (provider, transport)
    }

    fn d3q(
        replies: Vec<Result<&'static str, &'static str>>,
    ) -> (MadaraGeneric, Arc<MockTransport>) {
        let transport = Arc::new(MockTransport::with(replies));
        let provider = MadaraGeneric::with_transport_profile(
            "DocTruyen3Q",
            "https://doctruyen3qhub.fit/",
            transport.clone(),
            SiteProfile::Doctruyen3q,
        )
        .unwrap();
        (provider, transport)
    }

    /// One card, exactly as the live `/tim-truyen?keyword=naruto` page prints
    /// it: cover in `div.image-item img` on the CDN domain, title in
    /// `a.title-manga`, both halves of the identity in the path.
    const D3Q_SEARCH_FIXTURE: &str = r#"<div class="item-manga"><div class="item"><div class="clearfix">
       <div class="image-item"><a href="https://doctruyen3qhub.fit/truyen-tranh/naruto-full-color-edition/1263"
         title="Naruto Full Color Edition"><img src="https://s2.anhvip.xyz/comics/naruto-full-color-edition-1263.jpg"
         alt="Naruto Full Color Edition" class="image-item lazy"></a>
         <div class="info-manga"><span> 254.5K </span></div></div>
       <div class="caption"><h3><a href="https://doctruyen3qhub.fit/truyen-tranh/naruto-full-color-edition/1263"
         class="title-manga" data-id="1263">Naruto Full Color Edition</a></h3>
         <ul><li class="chapter-detail"><a href="https://doctruyen3qhub.fit/truyen-tranh/naruto-full-color-edition/chapter-700.9/2470001">Chap 700.9</a></li></ul>
       </div></div></div></div>
       <div class="paginate"><ul class="pagination"><li class="page-item active"><a class="page-link"
         href="https://doctruyen3qhub.fit/tim-truyen?keyword=naruto&amp;page=1">1</a></li>
         <li class="page-item"><a class="page-link" href="https://doctruyen3qhub.fit/tim-truyen?keyword=naruto&amp;page=2">2</a></li></ul></div>"#;

    #[tokio::test]
    async fn d3q_search_maps_cards_and_cdn_cover_key() {
        let (provider, _) = d3q(vec![Ok(D3Q_SEARCH_FIXTURE)]);
        let page = provider.search("naruto", 1).await.unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(
            page.items[0].source_manga_id, "naruto-full-color-edition.1263",
            "the numeric story id is part of the identity"
        );
        assert_eq!(page.items[0].title, "Naruto Full Color Edition");
        assert_eq!(
            page.items[0].cover_key.as_deref(),
            Some("/cdn/s2.anhvip/comics/naruto-full-color-edition-1263.jpg"),
            "shard + vetted-domain token, same encoding TruyenQQ covers use"
        );
        assert!(page.has_more, "the page's own pagination bar means more");
    }

    /// The site indexes ASCII slugs: without folding, every Vietnamese query
    /// the user actually types returns an empty result list.
    #[tokio::test]
    async fn d3q_search_folds_diacritics_and_paginates() {
        let (provider, transport) = d3q(vec![Ok(D3Q_SEARCH_FIXTURE), Ok(D3Q_SEARCH_FIXTURE)]);
        provider.search("Võ Luyện Đỉnh Phong", 2).await.unwrap();
        let requested = transport.requested.lock().unwrap();
        let (_, url) = &requested[0];
        assert!(
            url.contains("keyword=vo+luyen+dinh+phong"),
            "query must arrive folded, got {url}"
        );
        assert!(
            url.contains("&page=2"),
            "page 2 must be requested, got {url}"
        );
    }

    #[test]
    fn d3q_urls_rebuild_both_ids() {
        let (provider, _) = d3q(vec![]);
        assert_eq!(
            provider.manga_url("vo-luyen-dinh-phong.442"),
            "https://doctruyen3qhub.fit/truyen-tranh/vo-luyen-dinh-phong/442"
        );
        assert_eq!(
            provider.chapter_url("vo-luyen-dinh-phong.442", "chapter-60.5.2502652"),
            "https://doctruyen3qhub.fit/truyen-tranh/vo-luyen-dinh-phong/chapter-60.5/2502652"
        );
    }

    /// Story page: the full list is inline, and the FIRST row is a hidden ad
    /// pointing at a different domain — it must never become a chapter.
    #[tokio::test]
    async fn d3q_detail_reads_inline_chapters_and_drops_the_foreign_ad_row() {
        let (provider, _) = d3q(vec![Ok(r#"<article id="comic-detail">
           <h1 class="title-manga">Ai Đó Đang Điều Khiển Cơ Thể Của Tôi</h1>
           <ul><li class="category"><p class="detail-info">Thể loại:
             <a href="https://doctruyen3qhub.fit/tim-truyen/manhwa">Manhwa</a>
             <a href="https://doctruyen3qhub.fit/tim-truyen/action">Action</a></p></li></ul>
           <div class="list-chapter" id="list-chapter-dt">
             <li class="row" style="display: none"><div class="chapters">
               <a href="//doctruyen3q.xyz/truyen-tranh/khac/chapter-1/9" class="chapter" data-chapter="9">Quảng cáo</a>
             </div></li>
             <li class="row"><div class="chapters">
               <a href="https://doctruyen3qhub.fit/truyen-tranh/ai-do-dang-dieu-khien-co-the-cua-toi/chapter-97/3606761"
                  class="chapter" data-chapter="97">Chap 97</a></div></li>
             <li class="row"><div class="chapters">
               <a href="https://doctruyen3qhub.fit/truyen-tranh/ai-do-dang-dieu-khien-co-the-cua-toi/chapter-60.5/2502652"
                  class="chapter" data-chapter="60.5">Chap 60.5: Tên chương</a></div></li>
           </div></article>"#)]);
        let detail = provider
            .manga_detail("ai-do-dang-dieu-khien-co-the-cua-toi.14345")
            .await
            .unwrap();
        assert_eq!(detail.genres, vec!["Manhwa", "Action"]);
        let chapters = detail.chapters;
        assert_eq!(
            chapters.len(),
            2,
            "the foreign-domain ad row is not a chapter"
        );
        // The site prints newest first; the adapter stores them ascending, so
        // the queue walks the story in reading order.
        assert_eq!(chapters[0].source_chapter_id, "chapter-60.5.2502652");
        assert_eq!(
            chapters[0].number,
            Some(60.5),
            "data-chapter wins over parsing the id"
        );
        assert_eq!(chapters[0].title, "Chap 60.5: Tên chương");
        assert_eq!(chapters[1].source_chapter_id, "chapter-97.3606761");
        assert_eq!(chapters[1].number, Some(97.0));
    }

    /// `.page-chapter` is reused by the site's own ad banners ON THE SAME image
    /// domain, so only the numbered page wrappers may become pages.
    #[tokio::test]
    async fn d3q_pages_take_numbered_wrappers_and_ignore_ads() {
        let (provider, _) = d3q(vec![Ok(r#"<div class="list-image-detail">
           <div class="page-chapter"><img src="//s2.anhvip.xyz/3qhub3.jpg"></div>
           <div id="page_1" class="page-chapter"><img src="//s15.anhvip.xyz/image_comics/14345/3606761/img_001.jpg?data=vipx"></div>
           <div id="page_2" class="page-chapter"><img src="//s8.topcdnv1.art/image_top/14345/3606761/img_002.webp?data=3q"></div>
           <div class="page-chapter"><img src="//s2.anhvip.xyz/luotnv.jpg"></div>
           </div>"#)]);
        let manifest = provider
            .page_manifest(
                "ai-do-dang-dieu-khien-co-the-cua-toi.14345",
                "chapter-97.3606761",
            )
            .await
            .unwrap();
        assert_eq!(manifest.pages.len(), 2);
        assert_eq!(
            manifest.pages[0].url,
            "https://s15.anhvip.xyz/image_comics/14345/3606761/img_001.jpg?data=vipx"
        );
        assert_eq!(manifest.pages[1].index, 1);
    }

    #[test]
    fn d3q_owns_only_its_three_and_four_segment_routes() {
        let (provider, _) = d3q(vec![]);
        let owns = |path: &str| {
            provider.owns_url(&Url::parse(&format!("https://doctruyen3qhub.fit{path}")).unwrap())
        };
        assert!(owns(
            "/truyen-tranh/ai-do-dang-dieu-khien-co-the-cua-toi/14345"
        ));
        assert!(owns(
            "/truyen-tranh/ai-do-dang-dieu-khien-co-the-cua-toi/chapter-97/3606761"
        ));
        assert!(!owns("/truyen-tranh/ai-do-dang-dieu-khien-co-the-cua-toi"));
        assert!(!owns("/truyen-tranh/ai-do/chapter-97/not-a-number"));
        assert!(!owns("/tim-truyen/manhwa"));
        assert!(!owns("/"));
    }

    /// A chapter link cannot be turned into a manga identity (the story id is
    /// missing), so it must be refused rather than guessed.
    #[tokio::test]
    async fn d3q_resolve_url_accepts_stories_and_refuses_chapter_links() {
        let (provider, _) = d3q(vec![]);
        let story =
            Url::parse("https://doctruyen3qhub.fit/truyen-tranh/naruto-full-color-edition/1263")
                .unwrap();
        assert_eq!(
            provider.resolve_url(&story).await.unwrap(),
            "naruto-full-color-edition.1263"
        );
        let chapter = Url::parse(
            "https://doctruyen3qhub.fit/truyen-tranh/naruto-full-color-edition/chapter-700.9/2470001",
        )
        .unwrap();
        assert!(provider.chapter_from_url(&chapter).is_none());
        assert!(provider.resolve_url(&chapter).await.is_err());
    }

    #[test]
    fn fold_search_query_strips_vietnamese_diacritics() {
        assert_eq!(
            fold_search_query("Võ Luyện Đỉnh Phong"),
            "vo luyen dinh phong"
        );
        assert_eq!(fold_search_query("Đàm Phán"), "dam phan");
        assert_eq!(fold_search_query("Naruto 123"), "naruto 123");
    }

    const NT_SEARCH: &str = r#"<html><body>
        <div class="item">
          <figure class="clearfix">
            <div class="image">
              <a title="Truyện tranh Batman - Ninja Rùa" href="https://nettruyen.gg/truyen-tranh/batman-ninja-rua">
                <img src="/assets/images/thumb-default.jpg" data-original="https://image2.kcgsbok.com/nettruyen/thumb/batman-ninja-rua.jpg" class="lazy image-thumb">
              </a>
            </div>
            <figcaption><h3><a href="https://nettruyen.gg/truyen-tranh/batman-ninja-rua">Batman - Ninja Rùa</a></h3></figcaption>
          </figure>
        </div>
    </body></html>"#;

    #[tokio::test]
    async fn nt_cards_use_the_slug_determined_cdn_cover_key() {
        let (provider, _) = nt(vec![Ok(NT_SEARCH)]);
        let page = provider.search("ninja", 1).await.unwrap();
        assert_eq!(page.items.len(), 1);
        // The scraped `src` is a placeholder and the real URL is CDN-hosted;
        // neither may become the key — it is minted from the slug instead.
        assert_eq!(
            page.items[0].cover_key.as_deref(),
            Some("/nettruyen/thumb/batman-ninja-rua.jpg")
        );
    }

    #[tokio::test]
    async fn nt_fetch_cover_requests_the_vetted_shard_and_rejects_other_keys() {
        let (provider, transport) = nt(vec![Ok("cover-bytes")]);
        let key = "/nettruyen/thumb/batman-ninja-rua.jpg";
        let bytes = provider.fetch_cover("batman-ninja-rua", key).await.unwrap();
        assert_eq!(bytes, b"cover-bytes".to_vec());
        assert_eq!(
            transport.requested.lock().unwrap()[0],
            (
                "GET-BYTES".to_string(),
                format!("https://{NETTRUYEN_COVER_HOST}{key}")
            )
        );
        // A stored key may never retarget the CDN at another path.
        for (manga, bad) in [
            ("batman-ninja-rua", "/nettruyen/thumb/other.jpg"),
            ("batman-ninja-rua", "//evil.com/x.jpg"),
            ("../../etc", key),
        ] {
            assert!(
                provider.fetch_cover(manga, bad).await.is_err(),
                "{manga} + {bad} must not reach the network"
            );
        }
    }

    // Fixtures mirror truyenqqko.com's real markup: the search endpoint is a
    // POST returning a bare <li> fragment, stories and chapters are siblings
    // under /truyen-tranh/, covers live on i.hinhhinh.com and pages on a
    // truyenvua shard.
    const QQ_SEARCH: &str = r#"<html><body>
      <li>
        <a href="https://truyenqqko.com/truyen-tranh/the-yagyu-ninja-scroll-896">
          <div class="search_avatar"><img src="https://i.hinhhinh.com/ebook/80x105/the-yagyu-ninja-scroll_1443539433.jpg?gt=hdfgdfg&amp;mobile=2"></div>
          <div class="search_info"><p class="name">The Yagyu Ninja Scroll</p><p class="name_other"></p><p>Chưng 49</p></div>
        </a>
      </li>
      <li>
        <a href="https://truyenqqko.com/truyen-tranh/under-ninja-5861">
          <div class="search_avatar"><img src="https://i.hinhhinh.com/ebook/80x105/under-ninja_1541136998.jpg?gt=hdfgdfg&amp;mobile=2"></div>
          <div class="search_info"><p class="name">Under Ninja</p><p class="name_other">An Ninja</p><p>Chưng 12</p></div>
        </a>
      </li>
      <li><a href="https://truyenqqko.com/doc-truyen"><div class="search_info"><p class="name">Bang tin</p></div></a></li>
    </body></html>"#;

    #[tokio::test]
    async fn qq_search_posts_to_the_suggest_endpoint_and_maps_cards() {
        let (provider, transport) = qq(vec![Ok(QQ_SEARCH)]);
        let page = provider.search("ninja", 1).await.unwrap();
        assert_eq!(page.items.len(), 2, "non-story links are dropped");
        assert_eq!(page.items[0].title, "The Yagyu Ninja Scroll");
        assert_eq!(page.items[0].source_manga_id, "the-yagyu-ninja-scroll-896");
        // The key is the CDN path only — no host, no query string.
        assert_eq!(
            page.items[0].cover_key.as_deref(),
            Some("/cdn/i.hh/ebook/80x105/the-yagyu-ninja-scroll_1443539433.jpg")
        );
        assert!(!page.has_more, "the suggest fragment has no pagination");
        let requested = transport.requested.lock().unwrap();
        assert_eq!(
            requested[0],
            (
                "POST-TEXT".to_string(),
                "https://truyenqqko.com/frontend/search/search".to_string()
            )
        );
    }

    const QQ_DETAIL: &str = r#"<html><head>
        <meta property="og:image" content="https://i.hinhhinh.com/ebook/190x247/dai-quan-gia-la-ma-hoang_1555489297.jpg?gt=hdfgdfg&amp;mobile=2">
      </head><body>
      <h1>Dai Quan Gia La Ma Hoang</h1>
      <ul class="works-chapter-list">
        <li class="works-chapter-item"><a href="/truyen-tranh/dai-quan-gia-la-ma-hoang-7015-chap-2">Chơng 2</a></li>
        <li class="works-chapter-item"><a href="/truyen-tranh/dai-quan-gia-la-ma-hoang-7015-chap-1">Chơng 1</a></li>
        <li class="works-chapter-item"><a href="/truyen-tranh/dai-quan-gia-la-ma-hoang-7015-chap-0">Chơng 0</a></li>
      </ul>
    </body></html>"#;

    #[tokio::test]
    async fn qq_detail_reads_the_inline_chapter_list_and_cdn_cover() {
        let (provider, transport) = qq(vec![Ok(QQ_DETAIL)]);
        let detail = provider
            .manga_detail("dai-quan-gia-la-ma-hoang-7015")
            .await
            .unwrap();
        assert_eq!(detail.summary.title, "Dai Quan Gia La Ma Hoang");
        assert_eq!(
            detail.summary.cover_key.as_deref(),
            Some("/cdn/i.hh/ebook/190x247/dai-quan-gia-la-ma-hoang_1555489297.jpg")
        );
        let ids: Vec<&str> = detail
            .chapters
            .iter()
            .map(|c| c.source_chapter_id.as_str())
            .collect();
        // The whole stem is the chapter identity, ordered oldest-first.
        assert_eq!(
            ids,
            [
                "dai-quan-gia-la-ma-hoang-7015-chap-0",
                "dai-quan-gia-la-ma-hoang-7015-chap-1",
                "dai-quan-gia-la-ma-hoang-7015-chap-2",
            ]
        );
        let requested = transport.requested.lock().unwrap();
        assert_eq!(
            requested[0].1,
            "https://truyenqqko.com/truyen-tranh/dai-quan-gia-la-ma-hoang-7015"
        );
    }

    const QQ_PAGES: &str = r#"<html><body>
      <div class="page-chapter"><img class="lazy-image" src="https://i178.truyenvua.com/7015/916/chap_7015-1.jpg?r=r8645456"></div>
      <div class="page-chapter"><img class="lazy-image" src="https://i178.truyenvua.com/7015/916/chap_7015-2.jpg?r=r8645456"></div>
      <img src="https://ads.example.invalid/banner.gif">
      <img src="https://av.truyenvua.com/160x160/avatar_1733759465.jpg">
    </body></html>"#;

    #[tokio::test]
    async fn qq_pages_accept_plain_src_on_the_shard_and_drop_ads_and_avatars() {
        let (provider, _) = qq(vec![Ok(QQ_PAGES)]);
        let manifest = provider
            .page_manifest(
                "dai-quan-gia-la-ma-hoang-7015",
                "dai-quan-gia-la-ma-hoang-7015-chap-916",
            )
            .await
            .unwrap();
        assert_eq!(manifest.pages.len(), 2, "only the numbered shard pages");
        assert!(manifest.pages[0]
            .url
            .starts_with("https://i178.truyenvua.com/"));
    }

    // Live-observed 2026-10-02: "bản đẹp" series rotate their page CDN to
    // hinhtruyen.com (data-cdn names the tintruyen.com alternate) while
    // other series stay on truyenvua shards. Both shapes must parse.
    const QQ_PAGES_ROTATED: &str = r#"<html><body>
      <div id="page_0" class="page-chapter"><img class="lazy" src="https://hinhtruyen.com/8777/1.1/0.jpg?pt=sdffsd" data-original="https://hinhtruyen.com/8777/1.1/0.jpg?pt=sdffsd" data-cdn="https://tintruyen.com/8777/1.1/0.jpg"></div>
      <div id="page_1" class="page-chapter"><img class="lazy" src="https://hinhtruyen.com/8777/1.1/1.jpg?pt=sdffsd" data-original="https://hinhtruyen.com/8777/1.1/1.jpg?pt=sdffsd" data-cdn="https://tintruyen.com/8777/1.1/1.jpg"></div>
      <img src="https://av.truyenvua.com/160x160/avatar_1.jpg">
      <img src="https://ads.example.invalid/banner.gif">
    </body></html>"#;

    #[tokio::test]
    async fn qq_pages_accept_the_rotated_hinhtruyen_cdn() {
        let (provider, _) = qq(vec![Ok(QQ_PAGES_ROTATED)]);
        let manifest = provider
            .page_manifest(
                "ninja-loan-thi-ban-dep-8777",
                "ninja-loan-thi-ban-dep-8777-chap-1-1",
            )
            .await
            .unwrap();
        assert_eq!(manifest.pages.len(), 2, "avatar and ad hosts stay excluded");
        assert!(manifest.pages[0].url.starts_with("https://hinhtruyen.com/"));
    }

    #[tokio::test]
    async fn qq_covers_fetch_from_the_vetted_cover_host() {
        let (provider, transport) = qq(vec![Ok("cover-bytes"), Ok("base-bytes")]);
        let bytes = provider
            .fetch_cover(
                "dai-quan-gia-la-ma-hoang-7015",
                "/cdn/i.hh/ebook/190x247/dai-quan-gia-la-ma-hoang_1555489297.jpg",
            )
            .await
            .unwrap();
        assert_eq!(bytes, b"cover-bytes".to_vec());
        assert_eq!(
            transport.requested.lock().unwrap()[0].1,
            "https://i.hinhhinh.com/ebook/190x247/dai-quan-gia-la-ma-hoang_1555489297.jpg"
        );
        for key in ["//evil.com/a.jpg", "/ebook/../secret.jpg"] {
            assert!(
                provider
                    .fetch_cover("dai-quan-gia-la-ma-hoang-7015", key)
                    .await
                    .is_err(),
                "{key} must never reach any host"
            );
        }
    }

    /// Live shape (2026-10-03): truyenqqko.com serves some story covers from
    /// its OWN host through an image proxy (`/img/<sig>/rs:fit:…/<b64>`). The
    /// key minter accepts base-host paths, so fetch must not reject them —
    /// they go through the same base-host guard every other profile uses.
    #[tokio::test]
    async fn qq_covers_on_the_base_host_fetch_from_the_base_host() {
        let (provider, transport) = qq(vec![Ok("base-bytes")]);
        let key = "/img/zYPf48xn-LOim1OwjM6xUKVKTHwB3Hy8diWDSUkAAZI/rs:fit:600:800/aHR0cDovL25naW54L2Nkbi9lYm9vay9vcmlnbmFsL25pbmphLWxvYW4tdGhpLWJhbi1kZXBfMTU3ODAyNjk3NC5qcGc_Z3Q9aGRmZ2RmZyZtb2JpbGU9Mg";
        let bytes = provider
            .fetch_cover("ninja-loan-thi-ban-dep-8777", key)
            .await
            .unwrap();
        assert_eq!(bytes, b"base-bytes".to_vec());
        assert_eq!(
            transport.requested.lock().unwrap()[0].1,
            format!("https://truyenqqko.com{key}")
        );
    }

    #[test]
    fn qq_owns_truyen_tranh_stems_but_not_sections_or_chapters() {
        let (provider, _) = qq(vec![]);
        let manga: Url = "https://truyenqqko.com/truyen-tranh/dai-quan-gia-la-ma-hoang-7015"
            .parse()
            .unwrap();
        assert!(provider.owns_url(&manga));
        let chapter: Url =
            "https://truyenqqko.com/truyen-tranh/dai-quan-gia-la-ma-hoang-7015-chap-916"
                .parse()
                .unwrap();
        assert!(provider.owns_url(&chapter));
        assert_eq!(
            provider.chapter_from_url(&chapter),
            Some((
                "dai-quan-gia-la-ma-hoang-7015".to_string(),
                "dai-quan-gia-la-ma-hoang-7015-chap-916".to_string()
            ))
        );
        for reserved in ["doc-truyen", "top-ngay", "tim-kiem-nang-cao", "frontend"] {
            let url: Url = format!("https://truyenqqko.com/{reserved}")
                .parse()
                .unwrap();
            assert!(!provider.owns_url(&url), "{reserved} is a site section");
        }
        let foreign: Url = "https://other.example/truyen-tranh/x-1".parse().unwrap();
        assert!(!provider.owns_url(&foreign));
    }

    fn qq_card(stem: &str, title: &str, chapter: &str, age: &str) -> String {
        format!(
            r#"<li>
              <div class="book_avatar">
                <a href="https://truyenqqko.com/truyen-tranh/{stem}"><img src="https://i.hinhhinh.com/ebook/190x247/{stem}.jpg?gt=x&amp;mobile=2"></a>
                <div class="top-notice"><span class="time-ago">{age}</span><span class="type-label hot">Hot</span></div>
              </div>
              <div class="book_info"><div class="book_name"><h3><a href="https://truyenqqko.com/truyen-tranh/{stem}">{title}</a></h3></div>
                <div class="last_chapter"><a href="https://truyenqqko.com/truyen-tranh/{stem}-chap-9">{chapter}</a></div>
              </div>
            </li>"#
        )
    }

    #[tokio::test]
    async fn qq_home_feed_splits_the_three_front_page_rails() {
        let catalog = format!(
            r#"<html><body>
              <ul class="list_grid grid" id="list_suggest">{}</ul>
              <ul class="list_grid grid" id="list_new">{}</ul>
            </body></html>"#,
            qq_card(
                "dai-quan-gia-la-ma-hoang-7015",
                "Dai Quan Gia La Ma Hoang",
                "Chuong 916",
                "10 Gio Truoc",
            ),
            qq_card(
                "vo-luyen-dinh-phong-14728",
                "Vo Luyen Dinh Phong",
                "Chuong 3824",
                "21 phut truoc",
            ),
        );
        let ranking = format!(
            r#"<html><body><ul class="list_grid grid">{}</ul></body></html>"#,
            qq_card(
                "under-ninja-5861",
                "Under Ninja",
                "Chuong 12",
                "2 gio truoc"
            ),
        );
        let catalog_ref: &'static str = Box::leak(catalog.into_boxed_str());
        let ranking_ref: &'static str = Box::leak(ranking.into_boxed_str());
        let (provider, transport) = qq(vec![Ok(catalog_ref), Ok(ranking_ref)]);
        let feed = provider.home_feed().await.unwrap();
        assert_eq!(feed.featured.len(), 1);
        assert_eq!(feed.featured[0].summary.title, "Dai Quan Gia La Ma Hoang");
        assert_eq!(
            feed.featured[0].summary.cover_key.as_deref(),
            Some("/cdn/i.hh/ebook/190x247/dai-quan-gia-la-ma-hoang-7015.jpg")
        );
        assert_eq!(
            feed.featured[0].latest_chapter.as_deref(),
            Some("Chuong 916")
        );
        assert_eq!(
            feed.featured[0].latest_time.as_deref(),
            Some("10 Gio Truoc")
        );
        assert_eq!(feed.newest[0].summary.title, "Vo Luyen Dinh Phong");
        assert_eq!(feed.popular[0].summary.title, "Under Ninja");
        let requested = transport.requested.lock().unwrap();
        assert_eq!(requested[0].1, "https://truyenqqko.com/doc-truyen");
        assert_eq!(requested[1].1, "https://truyenqqko.com/top-ngay");
    }

    #[tokio::test]
    async fn home_feed_is_refused_for_non_truyenqq_profiles() {
        let (provider, _) = adapter(vec![]);
        assert!(matches!(
            provider.home_feed().await,
            Err(SourceError::UnsupportedOperation)
        ));
    }

    #[tokio::test]
    #[ignore = "live network probe: built-in TruyenQQ home feed (featured/newest/popular)"]
    async fn live_builtin_qq_home_probe() {
        let adapter = MadaraGeneric::built_in(
            "truyenqq",
            "TruyenQQ",
            "https://truyenqqko.com/",
            &[],
            SiteProfile::TruyenQQ,
        )
        .unwrap();
        let feed = adapter.home_feed().await.expect("live home feed");
        println!(
            "LIVE HOME featured={} newest={} popular={}",
            feed.featured.len(),
            feed.newest.len(),
            feed.popular.len()
        );
        for (label, rail) in [
            ("featured", &feed.featured),
            ("newest", &feed.newest),
            ("popular", &feed.popular),
        ] {
            assert!(!rail.is_empty(), "{label} rail is empty");
            let first = &rail[0];
            println!(
                "  {label}[0]: {} / {} / chap={:?} / time={:?} / cover={:?}",
                first.summary.source_manga_id,
                first.summary.title,
                first.latest_chapter,
                first.latest_time,
                first.summary.cover_key
            );
            assert!(
                !first.summary.title.is_empty(),
                "{label} title must not be a slug"
            );
        }
        // Covers must be mintable from the keys the rails published.
        assert!(
            feed.newest
                .iter()
                .any(|item| item.latest_time.as_deref().is_some_and(|t| !t.is_empty())),
            "newest rail must carry the source's own relative timestamps"
        );
        // Some cards point at files the CDN itself no longer serves (404), so
        // the rails are tried in order and the UI's placeholder fallback covers
        // the rest; at least one real cover must be fetchable.
        let mut fetched = None;
        for item in feed.newest.iter().chain(feed.featured.iter()) {
            let Some(key) = item.summary.cover_key.as_deref() else {
                continue;
            };
            if let Ok(bytes) = adapter
                .fetch_cover(&item.summary.source_manga_id, key)
                .await
            {
                fetched = Some((key.to_string(), bytes.len()));
                break;
            }
        }
        let (key, len) = fetched.expect("at least one rail cover must fetch");
        println!("LIVE HOME cover bytes: {len} ({key})");
        assert!(len > 1000);
    }

    /// Two open questions, both answerable only from the live site:
    /// (1) does a catalog page expose a sort control we can drive, and
    /// (2) do the three ranking periods actually return different lists?
    #[tokio::test]
    #[ignore = "live network probe: TruyenQQ sort vocabulary and ranking period differences"]
    async fn live_builtin_qq_sort_and_periods_recon() {
        let adapter = MadaraGeneric::built_in(
            "truyenqq",
            "TruyenQQ",
            "https://truyenqqko.com/",
            &[],
            SiteProfile::TruyenQQ,
        )
        .unwrap();

        // 1. What sort controls does a genre page itself publish?
        let html = adapter
            .http
            .get_text(&format!("{}the-loai/action-26", adapter.base))
            .await
            .expect("genre page");
        {
            let document = Html::parse_document(&html);
            for select in document.select(&Selector::parse("select").unwrap()) {
                let name = select.value().attr("name").unwrap_or("-");
                let options: Vec<String> = select
                    .select(&Selector::parse("option").unwrap())
                    .map(|o| {
                        format!(
                            "{}={}",
                            o.value().attr("value").unwrap_or("-"),
                            o.text().collect::<String>().trim()
                        )
                    })
                    .take(10)
                    .collect();
                println!("RECON SORT select name={name} options={options:?}");
            }
            let mut hrefs: HashSet<String> = HashSet::new();
            for anchor in document.select(&Selector::parse("a[href]").unwrap()) {
                let Some(href) = anchor.value().attr("href") else {
                    continue;
                };
                let text = anchor.text().collect::<String>().trim().to_string();
                let interesting = href.contains('?')
                    || href.contains("danh-sach")
                    || href.contains("sap-xep")
                    || text.chars().count() < 24
                        && (text.contains("Hot")
                            || text.contains("hot")
                            || text.contains("ới")
                            || text.contains("hi hoàn thành")
                            || text.contains("ượt xem"));
                if interesting {
                    hrefs.insert(format!("{href} | {text}"));
                }
            }
            let mut hrefs: Vec<String> = hrefs.into_iter().collect();
            hrefs.sort();
            println!("RECON SORT links ({})", hrefs.len());
            for line in hrefs.iter().take(25) {
                println!("  {line}");
            }
        }

        // 2. Do the ranking periods actually differ?
        let mut lists: Vec<(String, Vec<String>)> = Vec::new();
        for period in ["day", "week", "month"] {
            let rail = adapter.ranking(period).await.unwrap_or_else(|error| {
                println!("RECON RANKING {period} error {error:?}");
                Vec::new()
            });
            println!("RECON RANKING {period} = {} rows", rail.len());
            lists.push((
                period.to_string(),
                rail.iter()
                    .map(|item| item.summary.source_manga_id.clone())
                    .collect(),
            ));
        }
        for a in 0..lists.len() {
            for b in (a + 1)..lists.len() {
                let shared = lists[a]
                    .1
                    .iter()
                    .filter(|slug| lists[b].1.contains(slug))
                    .count();
                println!(
                    "RECON RANKING overlap {}∩{} = {shared}/{}",
                    lists[a].0,
                    lists[b].0,
                    lists[a].1.len().min(lists[b].1.len())
                );
            }
        }
        for (period, slugs) in &lists {
            println!("  {period}: {:?}", slugs.iter().take(3).collect::<Vec<_>>());
        }
    }

    /// Browse must reuse the catalog grid the ranking rails already parse —
    /// but only if the genre/status pages really use it. Count the SAME
    /// selectors there before writing any implementation.
    #[tokio::test]
    #[ignore = "live network probe: do TruyenQQ genre/status pages share the catalog grid?"]
    async fn live_builtin_qq_browse_grid_recon() {
        let adapter = MadaraGeneric::built_in(
            "truyenqq",
            "TruyenQQ",
            "https://truyenqqko.com/",
            &[],
            SiteProfile::TruyenQQ,
        )
        .unwrap();
        for path in [
            "the-loai/action-26",
            "the-loai/action-26/trang-2",
            "truyen-hoan-thanh",
            "truyen-dang-theo-doi",
            "truyen-moi-cap-nhat",
        ] {
            let html = match adapter
                .http
                .get_text(&format!("{}{}", adapter.base, path))
                .await
            {
                Ok(html) => html,
                Err(error) => {
                    println!("RECON {path} -> error {error:?}");
                    continue;
                }
            };
            let (grid, info, avatar, chapter, time, next) = {
                let document = Html::parse_document(&html);
                (
                    document
                        .select(&Selector::parse("ul.list_grid li").unwrap())
                        .count(),
                    document
                        .select(
                            &Selector::parse("ul.list_grid li div.book_info h3 a[href]").unwrap(),
                        )
                        .count(),
                    document
                        .select(
                            &Selector::parse("ul.list_grid li div.book_avatar img[src]").unwrap(),
                        )
                        .count(),
                    document
                        .select(
                            &Selector::parse("ul.list_grid li div.last_chapter a[href]").unwrap(),
                        )
                        .count(),
                    document
                        .select(&Selector::parse("ul.list_grid li span.time-ago").unwrap())
                        .count(),
                    document
                        .select(&Selector::parse("a[href*='/trang-']").unwrap())
                        .count(),
                )
            };
            println!("RECON {path} -> grid={grid} info={info} avatar={avatar} chapter={chapter} time={time} pagelinks={next}");
        }
        // The genre vocabulary the catalog page itself links.
        let catalog = adapter
            .http
            .get_text(&format!("{}doc-truyen", adapter.base))
            .await
            .expect("catalog");
        let tags = {
            let document = Html::parse_document(&catalog);
            let selector =
                Selector::parse("a[href^='/the-loai/'], a[href*='truyenqqko.com/the-loai/']")
                    .unwrap();
            let mut seen: HashSet<String> = HashSet::new();
            let mut out: Vec<(String, String)> = Vec::new();
            for node in document.select(&selector) {
                let Some(href) = node.value().attr("href") else {
                    continue;
                };
                let Some(slug) = href.rsplit("/the-loai/").next() else {
                    continue;
                };
                let slug = slug.trim_matches('/').to_string();
                let label = node.text().collect::<String>().trim().to_string();
                if !slug.is_empty() && !label.is_empty() && seen.insert(slug.clone()) {
                    out.push((slug, label));
                }
            }
            out
        };
        println!("RECON genres found: {}", tags.len());
        for (slug, label) in tags.iter().take(8) {
            println!("  {slug} = {label}");
        }
    }

    #[tokio::test]
    #[ignore = "live network probe: TruyenQQ genres() and browse() over the app transport"]
    async fn live_builtin_qq_browse_probe() {
        let adapter = MadaraGeneric::built_in(
            "truyenqq",
            "TruyenQQ",
            "https://truyenqqko.com/",
            &[],
            SiteProfile::TruyenQQ,
        )
        .unwrap();
        let tags = adapter.genres().await.expect("live genres");
        println!("LIVE GENRES {}", tags.len());
        assert!(
            tags.len() > 20,
            "the catalog links far more genres than this"
        );
        assert!(
            tags.iter().any(|tag| tag.slug == "action-26"),
            "Action must be in the published list"
        );
        for tag in tags.iter().take(5) {
            println!("  {} = {}", tag.slug, tag.label);
        }
        assert!(
            tags.iter().all(|tag| tag.slug.len() <= 80
                && tag
                    .slug
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')),
            "every published slug must survive the service's own validation"
        );

        let first = adapter
            .browse(Some("action-26"), None, None, 1)
            .await
            .expect("browse p1");
        println!(
            "LIVE BROWSE genre p1={} has_more={} first={}",
            first.items.len(),
            first.has_more,
            first
                .items
                .first()
                .map(|item| item.title.as_str())
                .unwrap_or("-")
        );
        assert!(first.items.len() >= 30, "the grid holds 42 cards");
        assert!(first.has_more, "page 2 exists for this genre");
        assert!(
            first
                .items
                .iter()
                .all(|item| item.title != item.source_manga_id),
            "a card whose title is its slug means the parser grabbed the wrong node"
        );
        assert!(
            first.items.iter().any(|item| item.cover_key.is_some()),
            "catalog cards carry covers"
        );

        let second = adapter
            .browse(Some("action-26"), None, None, 2)
            .await
            .expect("browse p2");
        let overlap = second
            .items
            .iter()
            .filter(|item| {
                first
                    .items
                    .iter()
                    .any(|prior| prior.source_manga_id == item.source_manga_id)
            })
            .count();
        println!(
            "LIVE BROWSE genre p2={} overlap_with_p1={}",
            second.items.len(),
            overlap
        );
        assert!(
            overlap * 2 < first.items.len(),
            "page 2 must be a different slice, not page 1 again"
        );

        let done = adapter
            .browse(None, Some("completed"), None, 1)
            .await
            .expect("completed");
        println!("LIVE BROWSE completed={}", done.items.len());
        assert!(!done.items.is_empty());
    }

    /// The "Sắp xếp" pill is only honest if `?sort=` actually reorders the
    /// grid and survives `/trang-N` — a source that ignored the parameter
    /// would make the control a lie. The same probe settles whether a genre
    /// and a status combine or exclude each other.
    #[tokio::test]
    #[ignore = "live network probe: TruyenQQ ?sort= reorders, pages, and combines with status"]
    async fn live_builtin_qq_sort_probe() {
        let adapter = MadaraGeneric::built_in(
            "truyenqq",
            "TruyenQQ",
            "https://truyenqqko.com/",
            &[],
            SiteProfile::TruyenQQ,
        )
        .unwrap();
        let ids = |page: &RemoteSearchPage| -> Vec<String> {
            page.items
                .iter()
                .map(|item| item.source_manga_id.clone())
                .collect()
        };

        let mut orders: Vec<(String, Vec<String>)> = Vec::new();
        for sort in ["newest-desc", "updated-desc", "views-desc", "views-asc"] {
            match adapter.browse(Some("action-26"), None, Some(sort), 1).await {
                Ok(page) => {
                    let list = ids(&page);
                    println!(
                        "PROBE SORT {sort} n={} head={:?}",
                        list.len(),
                        list.iter().take(3).collect::<Vec<_>>()
                    );
                    orders.push((sort.to_string(), list));
                }
                Err(error) => println!("PROBE SORT {sort} error {error:?}"),
            }
        }
        assert_eq!(orders.len(), 4, "every advertised sort must resolve a grid");
        let mut distinct = 0;
        for a in 0..orders.len() {
            for b in (a + 1)..orders.len() {
                let same_order = orders[a].1 == orders[b].1;
                let shared = orders[b]
                    .1
                    .iter()
                    .filter(|id| orders[a].1.contains(id))
                    .count();
                println!(
                    "PROBE SORT {} vs {} same_order={same_order} shared={shared}/{}",
                    orders[a].0,
                    orders[b].0,
                    orders[a].1.len().min(orders[b].1.len())
                );
                if !same_order {
                    distinct += 1;
                }
            }
        }
        assert!(
            distinct >= 3,
            "the sorts must actually change the ordering, not be ignored"
        );

        let hot1 = adapter
            .browse(Some("action-26"), None, Some("views-desc"), 1)
            .await
            .expect("sorted p1");
        let hot2 = adapter
            .browse(Some("action-26"), None, Some("views-desc"), 2)
            .await
            .expect("sorted p2");
        let overlap = ids(&hot2)
            .iter()
            .filter(|id| ids(&hot1).contains(id))
            .count();
        println!(
            "PROBE SORT paged p1={} p2={} overlap={}",
            hot1.items.len(),
            hot2.items.len(),
            overlap
        );
        assert!(
            hot2.items.len() >= 20 && overlap * 2 < hot1.items.len(),
            "?sort= must survive /trang-N"
        );

        let plain = adapter
            .browse(Some("action-26"), None, None, 1)
            .await
            .expect("genre only");
        let combo = adapter
            .browse(Some("action-26"), Some("completed"), None, 1)
            .await
            .expect("genre + status");
        println!(
            "PROBE COMBO genre={} genre+status={} head_orphan={}",
            plain.items.len(),
            combo.items.len(),
            ids(&combo)
                .iter()
                .filter(|id| !ids(&plain).contains(id))
                .count()
        );
        assert!(!combo.items.is_empty());
        assert_ne!(
            ids(&plain),
            ids(&combo),
            "status must narrow a genre page, not be silently dropped"
        );

        // Does a sort mean anything without a genre? Each status route answers
        // for itself — a route that ignores the parameter must not be offered
        // a control that pretends to drive it.
        for status in ["new", "ongoing", "completed"] {
            let plain = adapter.browse(None, Some(status), None, 1).await;
            let sorted = adapter
                .browse(None, Some(status), Some("views-desc"), 1)
                .await;
            match (plain, sorted) {
                (Ok(plain), Ok(sorted)) => println!(
                    "PROBE SORTONLY status={status} same_order={}",
                    ids(&plain) == ids(&sorted)
                ),
                (plain, sorted) => {
                    println!("PROBE SORTONLY status={status} plain={plain:?} sorted={sorted:?}")
                }
            }
        }

        let all_three = adapter
            .browse(Some("action-26"), Some("ongoing"), Some("views-desc"), 2)
            .await
            .expect("genre + status + sort, page 2");
        println!(
            "PROBE ALL THREE n={} head={:?}",
            all_three.items.len(),
            all_three
                .items
                .iter()
                .take(2)
                .map(|item| item.source_manga_id.as_str())
                .collect::<Vec<_>>()
        );
        assert!(!all_three.items.is_empty());
    }

    #[test]
    fn qq_browse_path_uses_the_routes_recon_found() {
        assert_eq!(
            qq_browse_path(Some("action-26"), None, None, 1).unwrap(),
            "the-loai/action-26"
        );
        assert_eq!(
            qq_browse_path(Some("action-26"), None, None, 3).unwrap(),
            "the-loai/action-26/trang-3"
        );
        assert_eq!(
            qq_browse_path(None, Some("ongoing"), None, 1).unwrap(),
            "truyen-dang-theo-doi"
        );
        assert_eq!(
            qq_browse_path(None, Some("completed"), None, 2).unwrap(),
            "truyen-hoan-thanh/trang-2"
        );
        assert_eq!(
            qq_browse_path(None, Some("new"), None, 1).unwrap(),
            "truyen-moi-cap-nhat"
        );
        assert_eq!(
            qq_browse_path(None, None, None, 1).unwrap(),
            "truyen-moi-cap-nhat"
        );
        // An unknown status is rejected rather than silently defaulted.
        assert!(qq_browse_path(None, Some("yesterday"), None, 1).is_err());
        assert!(qq_browse_path(None, None, Some("by-vibes"), 1).is_err());
    }

    #[test]
    fn qq_browse_path_combines_genre_status_and_sort() {
        // Recon: a genre page narrows with `?status=0|2` and orders with `?sort=0..5`.
        assert_eq!(
            qq_browse_path(Some("action-26"), Some("ongoing"), None, 1).unwrap(),
            "the-loai/action-26?status=0"
        );
        assert_eq!(
            qq_browse_path(Some("action-26"), Some("completed"), Some("views-desc"), 2).unwrap(),
            "the-loai/action-26/trang-2?status=2&sort=4"
        );
        assert_eq!(
            qq_browse_path(Some("action-26"), Some("new"), Some("updated-asc"), 1).unwrap(),
            "the-loai/action-26?sort=3"
        );
        // A status route has no sort control, so it is refused, not ignored.
        assert!(matches!(
            qq_browse_path(None, Some("new"), Some("updated-asc"), 1),
            Err(SourceError::UnsupportedOperation)
        ));
        // "new" is a genre page's own default order, so no status param is sent.
        assert_eq!(
            qq_browse_path(Some("action-26"), Some("new"), None, 1).unwrap(),
            "the-loai/action-26"
        );
        for (name, value) in [
            ("newest-desc", 0),
            ("newest-asc", 1),
            ("updated-desc", 2),
            ("updated-asc", 3),
            ("views-desc", 4),
            ("views-asc", 5),
        ] {
            assert_eq!(qq_sort_value(name).unwrap(), value, "{name}");
        }
    }

    /// The advanced-search page builds its form in JS, so its endpoint has to
    /// come from the bundle. Print every `/frontend/...` path the scripts use.
    #[tokio::test]
    #[ignore = "live network probe: which /frontend endpoints the TruyenQQ bundle calls"]
    async fn live_builtin_qq_frontend_endpoints_recon() {
        let adapter = MadaraGeneric::built_in(
            "truyenqq",
            "TruyenQQ",
            "https://truyenqqko.com/",
            &[],
            SiteProfile::TruyenQQ,
        )
        .unwrap();
        let html = adapter
            .http
            .get_text(&format!("{}tim-kiem-nang-cao", adapter.base))
            .await
            .expect("advanced search page");
        let scripts = {
            let document = Html::parse_document(&html);
            let selector = Selector::parse("script[src]").unwrap();
            document
                .select(&selector)
                .filter_map(|node| node.value().attr("src").map(str::to_string))
                .collect::<Vec<String>>()
        };
        println!("RECON scripts {:?}", scripts.len());
        for src in scripts {
            let url = if src.starts_with("http") {
                src.clone()
            } else {
                format!("{}{}", adapter.base, src.trim_start_matches('/'))
            };
            let Ok(body) = adapter.http.get_text(&url).await else {
                println!("RECON script unreadable: {url}");
                continue;
            };
            let mut found: HashSet<String> = HashSet::new();
            let mut cursor = body.as_str();
            while let Some(index) = cursor.find("/frontend/") {
                let tail = &cursor[index..];
                let end = tail
                    .find(|c: char| {
                        !(c.is_alphanumeric() || c == '/' || c == '-' || c == '_' || c == '.')
                    })
                    .unwrap_or(tail.len());
                found.insert(tail[..end].to_string());
                cursor = &tail[1..];
            }
            let mut found: Vec<String> = found.into_iter().collect();
            found.sort();
            if !found.is_empty() {
                println!("RECON {url} -> {found:?}");
            }
            // The filter vocabulary the form posts.
            for needle in ["more-story", "genre", "status", "sort", "keyword"] {
                if let Some(index) = body.find(needle) {
                    let start = index.saturating_sub(60);
                    let slice = &body[start..(index + 90).min(body.len())];
                    let flat: String = slice.chars().filter(|c| *c != '\n').collect();
                    println!("RECON ctx {needle}: {flat}");
                }
            }
        }
    }

    /// The advanced-search page is the only candidate that could combine a
    /// keyword with genre/status/sort, so its form contract must be read from
    /// the markup rather than guessed.
    #[tokio::test]
    #[ignore = "live network probe: TruyenQQ advanced-search form fields and genre listing markup"]
    async fn live_builtin_qq_advanced_search_recon() {
        let adapter = MadaraGeneric::built_in(
            "truyenqq",
            "TruyenQQ",
            "https://truyenqqko.com/",
            &[],
            SiteProfile::TruyenQQ,
        )
        .unwrap();
        let html = adapter
            .http
            .get_text(&format!("{}tim-kiem-nang-cao", adapter.base))
            .await
            .expect("advanced search page");
        {
            let document = Html::parse_document(&html);
            for form in document.select(&Selector::parse("form").unwrap()) {
                let action = form.value().attr("action").unwrap_or("-");
                let method = form.value().attr("method").unwrap_or("-");
                println!("RECON FORM action={action} method={method}");
                for field in form.select(&Selector::parse("input, select").unwrap()) {
                    let name = field.value().attr("name").unwrap_or("-");
                    match field.value().name() {
                        "input" => println!(
                            "  input name={name} type={}",
                            field.value().attr("type").unwrap_or("-")
                        ),
                        "select" => {
                            let options: Vec<String> = field
                                .select(&Selector::parse("option").unwrap())
                                .map(|o| {
                                    format!(
                                        "{}={}",
                                        o.value().attr("value").unwrap_or("-"),
                                        o.text().collect::<String>().trim()
                                    )
                                })
                                .take(12)
                                .collect();
                            println!("  select name={name} options={:?}", options);
                        }
                        _ => {}
                    }
                }
            }
        }

        // Genre listing markup: which grid does a /the-loai page actually use?
        let genre_html = adapter
            .http
            .get_text(&format!("{}the-loai/action-26", adapter.base))
            .await
            .expect("genre page");
        {
            let document = Html::parse_document(&genre_html);
            for probe in [
                "li",
                ".items .item",
                ".list_truyen .item",
                ".row",
                ".page-item-detail",
                ".card",
            ] {
                let Ok(selector) = Selector::parse(probe) else {
                    continue;
                };
                let count = document.select(&selector).count();
                println!("RECON GENRE selector {probe:?} -> {count} nodes");
            }
            let anchors = Selector::parse("a[href*='/truyen-tranh/']").unwrap();
            let names = Selector::parse("h3, .name, a[title]").unwrap();
            let mut shown = 0;
            for node in document.select(&anchors) {
                let title = node
                    .value()
                    .attr("title")
                    .map(str::to_string)
                    .or_else(|| {
                        node.select(&names)
                            .next()
                            .map(|n| n.text().collect::<String>().trim().to_string())
                    })
                    .unwrap_or_default();
                println!(
                    "  card href={} title={}",
                    node.value().attr("href").unwrap_or("-"),
                    title
                );
                shown += 1;
                if shown >= 3 {
                    break;
                }
            }
            let pagination = Selector::parse("a[href*='trang']").unwrap();
            let pages: Vec<String> = document
                .select(&pagination)
                .filter_map(|a| a.value().attr("href").map(str::to_string))
                .take(6)
                .collect();
            println!("RECON GENRE pagination {pages:?}");
        }
    }

    /// Discovery for the Phase 2 filter pills: instead of guessing the engine's
    /// routes, read the route names the catalog page itself links to.
    #[tokio::test]
    #[ignore = "live network probe: which genre/status/sort routes TruyenQQ publishes"]
    async fn live_builtin_qq_filter_recon() {
        let adapter = MadaraGeneric::built_in(
            "truyenqq",
            "TruyenQQ",
            "https://truyenqqko.com/",
            &[],
            SiteProfile::TruyenQQ,
        )
        .unwrap();
        let catalog_html = adapter
            .http
            .get_text(&format!("{}doc-truyen", adapter.base))
            .await
            .expect("catalog page");
        // Block scope: scraper::Html is !Send and must die before the next await.
        let paths = {
            let document = Html::parse_document(&catalog_html);
            let links = Selector::parse("a[href]").unwrap();
            let mut found: HashSet<String> = HashSet::new();
            for anchor in document.select(&links) {
                let Some(href) = anchor.value().attr("href") else {
                    continue;
                };
                let Some(rest) = href
                    .strip_prefix("https://truyenqqko.com")
                    .or_else(|| href.strip_prefix("http://truyenqqko.com"))
                    .or_else(|| href.strip_prefix('/'))
                else {
                    continue;
                };
                let path = format!("/{}", rest.trim_matches('/'));
                let keep = path.len() > 1
                    && !path.starts_with("/truyen-tranh")
                    && !path.starts_with("/chapter")
                    && !path.starts_with("/tac-gia")
                    && !path.contains('.');
                if keep {
                    found.insert(path);
                }
            }
            found
        };
        let mut paths: Vec<String> = paths.into_iter().collect();
        paths.sort();
        println!("RECON qq paths ({}):", paths.len());
        for path in &paths {
            println!("  {path}");
        }
        assert!(!paths.is_empty(), "catalog linked no filter routes at all");

        // Probe one of each family so the card selector is known before any
        // implementation is written.
        for prefix in ["/the-loai", "/danh-sach", "/nhom"] {
            let Some(sample) = paths.iter().find(|p| p.starts_with(prefix)).cloned() else {
                println!("RECON no {prefix} route");
                continue;
            };
            let html = match adapter
                .http
                .get_text(&format!("{}{}", adapter.base, &sample[1..]))
                .await
            {
                Ok(html) => html,
                Err(error) => {
                    println!("RECON {sample} -> fetch error {error:?}");
                    continue;
                }
            };
            let (cards, titles) = {
                let document = Html::parse_document(&html);
                let rows = Selector::parse("li").unwrap();
                let anchors = Selector::parse("a[href*='/truyen-tranh/']").unwrap();
                let names = Selector::parse("p.name, h3, .caption a").unwrap();
                let mut slugs: HashSet<String> = HashSet::new();
                let mut sample_titles: Vec<String> = Vec::new();
                for node in document.select(&rows) {
                    let Some(href) = node
                        .select(&anchors)
                        .next()
                        .and_then(|a| a.value().attr("href"))
                    else {
                        continue;
                    };
                    let Some(slug) = adapter.slug_from_manga_url(href) else {
                        continue;
                    };
                    if slugs.insert(slug.clone()) && sample_titles.len() < 5 {
                        let title = node
                            .select(&names)
                            .next()
                            .map(|n| n.text().collect::<String>().trim().to_string())
                            .unwrap_or_default();
                        sample_titles.push(format!("{slug} | {title}"));
                    }
                }
                (slugs.len(), sample_titles)
            };
            println!("RECON {sample} -> {cards} story cards");
            for line in titles {
                println!("    {line}");
            }
        }
    }

    #[tokio::test]
    #[ignore = "live network probe: built-in TruyenQQ search -> detail -> pages over the app transport (wreq + ECH)"]
    async fn live_builtin_qq_probe() {
        let adapter = MadaraGeneric::built_in(
            "truyenqq",
            "TruyenQQ",
            "https://truyenqqko.com/",
            &[],
            SiteProfile::TruyenQQ,
        )
        .unwrap();
        let page = adapter.search("ninja", 1).await.expect("live search");
        assert!(!page.items.is_empty(), "search returned zero cards");
        let first = &page.items[0];
        let detail = adapter
            .manga_detail(&first.source_manga_id)
            .await
            .expect("live detail");
        println!("LIVE detail: {} chapters", detail.chapters.len());
        assert!(!detail.chapters.is_empty());
        let last = detail.chapters.last().unwrap();
        let manifest = adapter
            .page_manifest(&first.source_manga_id, &last.source_chapter_id)
            .await
            .expect("live pages");
        println!("LIVE pages: {}", manifest.pages.len());
        assert!(manifest.pages.len() >= 5);
    }

    #[tokio::test]
    #[ignore = "live network probe: built-in NetTruyen search -> JSON chapters -> pages over the app transport (wreq + ECH)"]
    async fn live_builtin_nt_probe() {
        let adapter = MadaraGeneric::built_in(
            "nettruyen",
            "NetTruyen",
            "https://nettruyen.gg/",
            &[],
            SiteProfile::Nettruyen,
        )
        .unwrap();
        let page = adapter.search("ninja", 1).await.expect("live search");
        assert!(!page.items.is_empty(), "search returned zero cards");
        let first = &page.items[0];
        let detail = adapter
            .manga_detail(&first.source_manga_id)
            .await
            .expect("live detail");
        println!(
            "LIVE NT detail: {} chapters ({}.{})",
            detail.chapters.len(),
            first.source_manga_id,
            detail
                .chapters
                .last()
                .map(|c| c.source_chapter_id.as_str())
                .unwrap_or("-")
        );
        assert!(
            detail.chapters.len() > 20,
            "JSON list should beat the ~20 HTML anchors"
        );
        let last = detail.chapters.last().unwrap();
        let manifest = adapter
            .page_manifest(&first.source_manga_id, &last.source_chapter_id)
            .await
            .expect("live pages");
        println!("LIVE NT pages: {}", manifest.pages.len());
        assert!(manifest.pages.len() >= 5);
        // The real proof for both fixes: cover bytes and page bytes come from
        // the hotlink-protected CDN, which 403s without the site Referer.
        let cover_key = first
            .cover_key
            .clone()
            .expect("search card must carry a cover key");
        let cover = adapter
            .fetch_cover(&first.source_manga_id, &cover_key)
            .await
            .expect("live cover bytes");
        println!("LIVE NT cover: {} bytes (key {cover_key})", cover.len());
        assert!(cover.len() > 1000, "cover should be a real image");
        let page_bytes = adapter
            .fetch_image(&manifest.pages[0].url)
            .await
            .expect("live page bytes");
        println!("LIVE NT page bytes: {}", page_bytes.len());
        assert!(page_bytes.len() > 1000);
    }

    #[tokio::test]
    #[ignore = "live network probe: built-in Dilib URL import -> detail -> pages over the app transport (wreq)"]
    async fn live_builtin_dilib_probe() {
        // SITE STATUS 2026-10-02: dilib.vn serves a "Hệ Thống Đang Bảo Trì"
        // maintenance shell (HTTP 200, no content) for EVERY story/chapter
        // URL — this probe then fails at "live detail" with SOURCE_UPSTREAM
        // (the deliberate empty-catalog guard below). Re-run it to learn when
        // the source serves content again; nothing in the adapter changed.
        let adapter = MadaraGeneric::built_in(
            "dilib",
            "Dilib",
            "https://dilib.vn/",
            &[],
            SiteProfile::Dilib,
        )
        .unwrap();
        let story: Url = "https://dilib.vn/dao-hai-tac-one-piece-14728.html"
            .parse()
            .unwrap();
        assert!(adapter.owns_url(&story));
        let slug = adapter.resolve_url(&story).await.expect("resolve story");
        assert_eq!(slug, "dao-hai-tac-one-piece-14728");
        let detail = adapter.manga_detail(&slug).await.expect("live detail");
        println!("LIVE DL detail: {} chapters", detail.chapters.len());
        assert!(
            detail.chapters.len() > 50,
            "a long-running story must list many chapters"
        );
        let last = detail.chapters.last().unwrap();
        let manifest = adapter
            .page_manifest(&slug, &last.source_chapter_id)
            .await
            .expect("live pages");
        println!("LIVE DL pages: {}", manifest.pages.len());
        assert!(manifest.pages.len() >= 3);
    }
}
