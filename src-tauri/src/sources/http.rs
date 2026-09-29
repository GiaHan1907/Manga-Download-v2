use std::net::{IpAddr, ToSocketAddrs};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::Value;
use url::Url;

use super::adapter::SourceError;

pub const MAX_BODY_BYTES: u64 = 2 * 1024 * 1024;
/// Covers are decorative thumbnails — 2 MB is a deliberate hard cap.
pub const COVER_MAX_BYTES: u64 = 2 * 1024 * 1024;
/// Cap for a single page image (legitimately larger than HTML/JSON bodies).
pub const IMAGE_MAX_BYTES: u64 = 20 * 1024 * 1024;
/// Upper bound on pages per chapter manifest (malformed listings guard).
pub const MAX_PAGE_COUNT: usize = 300;
pub const MAX_REDIRECTS: u32 = 3;
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// Politeness floor between requests to the same host. Owners of user-added
/// sources may raise limits server-side; the client never hammers.
pub const MIN_REQUEST_SPACING: Duration = Duration::from_secs(3);
/// Kept for docs/tests only: real requests carry the Chrome profile UA
/// supplied by the emulation.
#[allow(dead_code)]
const APP_IDENTITY_NOTE: &str = "MangaDownloader1989/0.5 (local desktop manga client)";

/// JSON transport hardened by an exact-host allowlist policy. Redirects are
/// followed manually so every hop is re-validated before any request leaves.
#[async_trait]
pub trait Transport: Send + Sync {
    async fn get_json(&self, url: &str) -> Result<Value, SourceError>;
    /// Plain-text GET (HTML pages). Size-capped and policy-guarded like JSON.
    async fn get_text(&self, url: &str) -> Result<String, SourceError>;
    /// Form POST returning an HTML fragment (Madara `ajax/chapters/` list).
    /// Same host allowlist, pacing and size caps as every other request.
    async fn post_form_text(&self, url: &str, form: &[(&str, &str)])
        -> Result<String, SourceError>;
    /// Binary GET with a caller-supplied byte cap (page images). Same
    /// host/redirect/pacing policy as text fetches.
    async fn get_bytes(&self, url: &str, max_bytes: u64) -> Result<Vec<u8>, SourceError>;
}

#[derive(Clone)]
pub struct HttpPolicy {
    /// Exact hostnames this transport may contact (no suffix tricks).
    pub allowed_hosts: Vec<String>,
    /// Registrable-domain allowlist for sharded CDNs (host == domain or a
    /// strict subdomain passes). ONLY vetted app-shipped built-ins may set
    /// this — user-registered sources always pass an empty list. Resolved
    /// addresses still must be public and everything stays https.
    pub allowed_domains: Vec<String>,
    /// `Referer` sent on every request. Hotlink-protected CDNs reject image
    /// GETs (403) without the site's own origin; ONLY app-shipped built-ins
    /// set it, always to their own base origin (never a caller-supplied URL).
    pub referer: Option<String>,
}

impl HttpPolicy {
    pub fn new(allowed_hosts: &[&str]) -> Self {
        HttpPolicy {
            allowed_hosts: allowed_hosts.iter().map(|host| host.to_string()).collect(),
            allowed_domains: Vec::new(),
            referer: None,
        }
    }

    fn host_allowed(&self, host: &str) -> bool {
        if self.allowed_hosts.iter().any(|allowed| allowed == host) {
            return true;
        }
        self.allowed_domains
            .iter()
            .any(|domain| host == domain.as_str() || host.ends_with(&format!(".{domain}")))
    }

    pub fn validate_request_url(&self, parsed: &Url) -> Result<(), SourceError> {
        if parsed.scheme() != "https" {
            return Err(SourceError::MalformedUrl);
        }
        let host = parsed.host_str().ok_or(SourceError::MalformedUrl)?;
        if !self.host_allowed(host) {
            return Err(SourceError::MalformedUrl);
        }
        if parsed.port().is_some() && parsed.port() != Some(443) {
            return Err(SourceError::MalformedUrl);
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(SourceError::MalformedUrl);
        }
        if let Ok(ip) = host.parse::<IpAddr>() {
            if !is_public_ip(&ip) {
                return Err(SourceError::Network);
            }
        }
        Ok(())
    }

    /// Resolve the host and require every candidate address to be public:
    /// loopback/private/link-local/CGNAT/multicast/reserved fail before fetch.
    pub fn assert_resolved_addresses_public(&self, parsed: &Url) -> Result<(), SourceError> {
        let host = parsed.host_str().ok_or(SourceError::MalformedUrl)?;
        if let Ok(ip) = host.parse::<IpAddr>() {
            return if is_public_ip(&ip) {
                Ok(())
            } else {
                Err(SourceError::Network)
            };
        }
        let port = parsed.port_or_known_default().unwrap_or(443);
        let addrs: Vec<IpAddr> = match (host, port).to_socket_addrs() {
            Ok(iter) => iter.map(|socket| socket.ip()).collect(),
            Err(_) => return Err(SourceError::Network),
        };
        if addrs.is_empty() {
            return Err(SourceError::Network);
        }
        if addrs.iter().any(|ip| !is_public_ip(ip)) {
            return Err(SourceError::Network);
        }
        Ok(())
    }
}

pub(crate) fn is_public_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_unspecified()
                || (o[0] == 100 && (o[1] & 0b0100_0000) != 0) // CGNAT 100.64/10
                || o[0] == 0
                || o[0] >= 240) // reserved/broadcast
        }
        IpAddr::V6(v6) => {
            if v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() {
                return false;
            }
            let first = v6.segments()[0];
            // fc00::/7 ULA, fe80::/10 link-local
            !(first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80)
        }
    }
}

pub struct GuardedHttp {
    policy: HttpPolicy,
    client: wreq::Client,
    last_request_at: Mutex<Option<Instant>>,
}

impl GuardedHttp {
    pub fn new(policy: HttpPolicy) -> Self {
        // Cloudflare on user-registered manga sites resets non-browser TLS
        // fingerprints (2026-09-25 incident); wreq emulates a real Chrome's
        // TLS + HTTP/2 + header profile. The UA comes from the emulation —
        // overriding it here would re-break the fingerprint consistency.
        let client = wreq::Client::builder()
            .emulation(wreq_util::Emulation::Chrome139)
            .redirect(wreq::redirect::Policy::none())
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("wreq client");
        GuardedHttp {
            policy,
            client,
            last_request_at: Mutex::new(None),
        }
    }

    fn validate(&self, raw: &str) -> Result<Url, SourceError> {
        if raw.len() > 2048 || !raw.is_ascii() {
            return Err(SourceError::MalformedUrl);
        }
        let parsed = Url::parse(raw).map_err(|_| SourceError::MalformedUrl)?;
        self.policy.validate_request_url(&parsed)?;
        self.policy.assert_resolved_addresses_public(&parsed)?;
        Ok(parsed)
    }

    /// Enforce the politeness spacing without holding the lock across awaits.
    async fn pace(&self) {
        let wait = {
            let guard = self.last_request_at.lock().unwrap();
            match *guard {
                Some(previous) => {
                    let due = previous + MIN_REQUEST_SPACING;
                    let now = Instant::now();
                    if due > now {
                        Some(due - now)
                    } else {
                        None
                    }
                }
                None => None,
            }
        };
        if let Some(duration) = wait {
            tokio::time::sleep(duration).await;
        }
        *self.last_request_at.lock().unwrap() = Some(Instant::now());
    }

    fn classify_status(status: wreq::StatusCode, retry_after: Option<String>) -> SourceError {
        match status.as_u16() {
            401 | 403 => SourceError::PermissionDenied,
            404 => SourceError::NotFound,
            429 => {
                SourceError::RateLimited(retry_after.unwrap_or_else(|| "retry-after".to_string()))
            }
            _ if status.is_server_error() => SourceError::Upstream,
            _ => SourceError::Upstream,
        }
    }

    fn classify_u16(code: u16, retry_after: Option<String>) -> SourceError {
        match code {
            401 | 403 => SourceError::PermissionDenied,
            404 => SourceError::NotFound,
            429 => {
                SourceError::RateLimited(retry_after.unwrap_or_else(|| "retry-after".to_string()))
            }
            _ => SourceError::Upstream,
        }
    }

    fn ech_body_or_error(
        response: crate::sources::ech::EchResponse,
        max_bytes: u64,
    ) -> Result<Vec<u8>, SourceError> {
        if !(200..300).contains(&response.status) {
            return Err(Self::classify_u16(
                response.status,
                response.header("retry-after").map(str::to_string),
            ));
        }
        if response.body.len() as u64 > max_bytes {
            return Err(SourceError::Parse);
        }
        Ok(response.body)
    }

    /// Connection-level failure path: same hop, but SNI encrypted via ECH.
    /// Redirects are re-validated through the normal policy each time.
    async fn fetch_via_ech(
        &self,
        start: &url::Url,
        max_bytes: u64,
    ) -> Result<Vec<u8>, SourceError> {
        let mut current = start.clone();
        for _ in 0..=MAX_REDIRECTS {
            let response = crate::sources::ech::fetch(&current, None).await?;
            if (300..400).contains(&response.status) {
                let location = response.header("location").ok_or(SourceError::Network)?;
                let next = current
                    .join(location)
                    .map_err(|_| SourceError::MalformedUrl)?;
                current = self.validate(next.as_str())?;
                continue;
            }
            return Self::ech_body_or_error(response, max_bytes);
        }
        Err(SourceError::Network)
    }

    /// Attach the policy's own-origin `Referer` when a built-in needs it.
    fn with_referer(&self, request: wreq::RequestBuilder) -> wreq::RequestBuilder {
        match &self.policy.referer {
            Some(referer) => request.header(wreq::header::REFERER, referer.as_str()),
            None => request,
        }
    }

    async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, SourceError> {
        self.fetch_bytes_bounded(url, MAX_BODY_BYTES).await
    }

    async fn fetch_bytes_bounded(&self, url: &str, max_bytes: u64) -> Result<Vec<u8>, SourceError> {
        let mut current = self.validate(url)?;
        for _ in 0..=MAX_REDIRECTS {
            self.pace().await;
            let request = self.with_referer(self.client.get(current.as_str()));
            let response = match request.send().await {
                Ok(response) => response,
                Err(error) if error.is_connect() => {
                    return self.fetch_via_ech(&current, max_bytes).await;
                }
                Err(_) => return Err(SourceError::Network),
            };
            let status = response.status();
            if status.is_redirection() {
                let location = response
                    .headers()
                    .get(wreq::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or(SourceError::Network)?;
                let next = current
                    .join(location)
                    .map_err(|_| SourceError::MalformedUrl)?;
                current = self.validate(next.as_str())?;
                continue;
            }
            if !status.is_success() {
                let retry_after = response
                    .headers()
                    .get(wreq::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string);
                return Err(Self::classify_status(status, retry_after));
            }
            if let Some(len) = response.content_length() {
                if len > max_bytes {
                    return Err(SourceError::Parse);
                }
            }
            let bytes = response.bytes().await.map_err(|_| SourceError::Network)?;
            if bytes.len() as u64 > max_bytes {
                return Err(SourceError::Parse);
            }
            return Ok(bytes.to_vec());
        }
        Err(SourceError::Network)
    }
}

#[async_trait]
impl Transport for GuardedHttp {
    async fn get_json(&self, url: &str) -> Result<Value, SourceError> {
        let bytes = self.fetch_bytes(url).await?;
        serde_json::from_slice(&bytes).map_err(|_| SourceError::Parse)
    }

    async fn get_text(&self, url: &str) -> Result<String, SourceError> {
        let bytes = self.fetch_bytes(url).await?;
        String::from_utf8(bytes).map_err(|_| SourceError::Parse)
    }

    async fn post_form_text(
        &self,
        url: &str,
        form: &[(&str, &str)],
    ) -> Result<String, SourceError> {
        let bytes = self.post_form_bytes(url, form).await?;
        String::from_utf8(bytes).map_err(|_| SourceError::Parse)
    }

    async fn get_bytes(&self, url: &str, max_bytes: u64) -> Result<Vec<u8>, SourceError> {
        self.fetch_bytes_bounded(url, max_bytes).await
    }
}

impl GuardedHttp {
    async fn post_form_bytes(
        &self,
        url: &str,
        form: &[(&str, &str)],
    ) -> Result<Vec<u8>, SourceError> {
        let parsed = self.validate(url)?;
        self.pace().await;
        let result = self
            .with_referer(
                self.client
                    .post(parsed.as_str())
                    .header(
                        wreq::header::HeaderName::from_static("x-requested-with"),
                        "XMLHttpRequest",
                    )
                    .form(form),
            )
            .send()
            .await;
        let response = match result {
            Ok(response) => response,
            Err(error) if error.is_connect() => {
                let fallback = crate::sources::ech::fetch(&parsed, Some(form)).await?;
                return Self::ech_body_or_error(fallback, MAX_BODY_BYTES);
            }
            Err(_) => return Err(SourceError::Network),
        };
        let status = response.status();
        if !status.is_success() {
            let retry_after = response
                .headers()
                .get(wreq::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            return Err(Self::classify_status(status, retry_after));
        }
        if let Some(len) = response.content_length() {
            if len > MAX_BODY_BYTES {
                return Err(SourceError::Parse);
            }
        }
        let bytes = response.bytes().await.map_err(|_| SourceError::Network)?;
        if bytes.len() as u64 > MAX_BODY_BYTES {
            return Err(SourceError::Parse);
        }
        Ok(bytes.to_vec())
    }
}

pub fn mangadex_policy() -> HttpPolicy {
    // uploads.mangadex.org is MangaDex's own fixed cover CDN (no at-home
    // hostname indirection for covers — only for chapter pages, which this
    // adapter deliberately does not declare).
    HttpPolicy::new(&["api.mangadex.org", "uploads.mangadex.org"])
}

#[cfg(test)]
pub mod mock {
    use super::*;

    /// Scripted transport for tests: each call pops the next canned result.
    pub struct MockTransport {
        pub replies: Mutex<Vec<Result<String, &'static str>>>,
        pub requested: Mutex<Vec<(String, String)>>,
    }

    impl MockTransport {
        pub fn with(replies: Vec<Result<&str, &'static str>>) -> Self {
            MockTransport {
                replies: Mutex::new(replies.into_iter().map(|r| r.map(str::to_string)).collect()),
                requested: Mutex::new(Vec::new()),
            }
        }

        fn pop_raw(&self, method: &str, url: &str) -> Result<String, SourceError> {
            self.requested
                .lock()
                .unwrap()
                .push((method.to_string(), url.to_string()));
            let next = self.replies.lock().unwrap().remove(0);
            match next {
                Ok(body) => Ok(body),
                Err("network") => Err(SourceError::Network),
                Err("404") => Err(SourceError::NotFound),
                Err("429") => Err(SourceError::RateLimited("120".to_string())),
                Err("500") => Err(SourceError::Upstream),
                Err("parse") => Err(SourceError::Parse),
                Err(_) => Err(SourceError::Upstream),
            }
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn get_json(&self, url: &str) -> Result<Value, SourceError> {
            serde_json::from_str(&self.pop_raw("GET", url)?).map_err(|_| SourceError::Parse)
        }
        async fn get_text(&self, url: &str) -> Result<String, SourceError> {
            self.pop_raw("GET-TEXT", url)
        }
        async fn post_form_text(
            &self,
            url: &str,
            _: &[(&str, &str)],
        ) -> Result<String, SourceError> {
            self.pop_raw("POST-TEXT", url)
        }
        async fn get_bytes(&self, url: &str, max_bytes: u64) -> Result<Vec<u8>, SourceError> {
            let body = self.pop_raw("GET-BYTES", url)?;
            if body.len() as u64 > max_bytes {
                return Err(SourceError::Parse);
            }
            Ok(body.into_bytes())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn guarded() -> GuardedHttp {
        GuardedHttp::new(mangadex_policy())
    }

    #[test]
    fn rejects_non_https_and_foreign_hosts() {
        let g = guarded();
        assert!(g.validate("http://api.mangadex.org/manga").is_err());
        assert!(g.validate("https://evil.example/manga").is_err());
        // substring-spoof host must fail exact match
        assert!(g
            .validate("https://api.mangadex.org.evil.test/manga")
            .is_err());
        assert!(g.validate("https://api.mangadex.org:8443/manga").is_err());
    }

    #[test]
    fn rejects_credentials_and_ip_literals_and_private_targets() {
        let g = guarded();
        assert!(g
            .validate("https://user:pass@api.mangadex.org/manga")
            .is_err());
        assert!(g.validate("https://127.0.0.1/manga").is_err());
        assert!(g.validate("https://10.0.0.5/manga").is_err());
        assert!(g
            .validate("https://169.254.169.254/latest/meta-data")
            .is_err());
        assert!(g.validate("https://100.64.0.1/manga").is_err());
        assert!(g.validate("https://api.mangadex.org/manga?title=x").is_ok());
    }

    #[test]
    fn cgnat_and_ula_ranges_are_blocked() {
        assert!(!is_public_ip(&"100.100.5.5".parse().unwrap()));
        assert!(!is_public_ip(&"fc00::1".parse().unwrap()));
        assert!(!is_public_ip(&"fe80::2".parse().unwrap()));
        assert!(!is_public_ip(&"127.5.5.5".parse().unwrap()));
        assert!(is_public_ip(&"1.1.1.1".parse().unwrap()));
    }

    #[test]
    fn redirect_targets_are_revalidated_before_the_next_hop() {
        let g = guarded();
        let current = Url::parse("https://api.mangadex.org/manga/x/").unwrap();
        // Every redirect Location is fed back through validate() before the
        // next request leaves — these must all be rejected.
        for location in [
            "https://evil.test/y",
            "http://api.mangadex.org/y",
            "https://127.0.0.1:8080/y",
            "https://api.mangadex.org.evil.test/y",
        ] {
            let next = current.join(location).unwrap();
            assert!(
                g.validate(next.as_str()).is_err(),
                "redirect hop to {location} must be rejected"
            );
        }
        let same_host = current.join("/manga/y/").unwrap();
        assert!(g.validate(same_host.as_str()).is_ok());
    }

    #[tokio::test]
    async fn script_404_429_500_parse_map_to_distinct_errors() {
        let transport = Arc::new(mock::MockTransport::with(vec![
            Err("404"),
            Err("429"),
            Err("500"),
            Err("parse"),
            Err("network"),
        ]));
        assert!(matches!(
            transport.get_json("x").await,
            Err(SourceError::NotFound)
        ));
        assert!(matches!(
            transport.post_form_text("x", &[]).await,
            Err(SourceError::RateLimited(_))
        ));
        assert!(matches!(
            transport.get_json("x").await,
            Err(SourceError::Upstream)
        ));
        assert!(matches!(
            transport.get_json("x").await,
            Err(SourceError::Parse)
        ));
        assert!(matches!(
            transport.get_json("x").await,
            Err(SourceError::Network)
        ));
    }

    #[test]
    fn t09_classify_status_maps_http_codes_and_retry_after() {
        use wreq::StatusCode;
        let classify = GuardedHttp::classify_status;
        assert!(matches!(
            classify(StatusCode::UNAUTHORIZED, None),
            SourceError::PermissionDenied
        ));
        assert!(matches!(
            classify(StatusCode::FORBIDDEN, None),
            SourceError::PermissionDenied
        ));
        assert!(matches!(
            classify(StatusCode::NOT_FOUND, None),
            SourceError::NotFound
        ));
        assert!(matches!(
            classify(StatusCode::TOO_MANY_REQUESTS, Some("120".into())),
            SourceError::RateLimited(ref value) if value == "120"
        ));
        // Missing Retry-After still yields a retryable RateLimited marker.
        assert!(matches!(
            classify(StatusCode::TOO_MANY_REQUESTS, None),
            SourceError::RateLimited(_)
        ));
        assert!(matches!(
            classify(StatusCode::INTERNAL_SERVER_ERROR, None),
            SourceError::Upstream
        ));
        assert!(matches!(
            classify(StatusCode::IM_A_TEAPOT, None),
            SourceError::Upstream
        ));
    }

    #[test]
    fn t09_dns_guard_rejects_private_literals_and_unresolvable_hosts() {
        let policy = mangadex_policy();
        // Private/loopback/metadata hosts fail even before any connection.
        for url in [
            "https://127.0.0.1/manga",
            "https://10.1.2.3/manga",
            "https://169.254.169.254/latest/meta-data/",
            "https://[::1]/manga",
        ] {
            let parsed = Url::parse(url).unwrap();
            assert!(
                matches!(
                    policy.assert_resolved_addresses_public(&parsed),
                    Err(SourceError::Network)
                ),
                "{url} must not resolve-fail-open"
            );
        }
        // A guaranteed-NXDOMAIN host must error, never fall through silently.
        let parsed = Url::parse("https://nonexistent-host-for-p09.invalid/").unwrap();
        assert!(policy.assert_resolved_addresses_public(&parsed).is_err());
    }

    #[tokio::test]
    async fn t09_per_host_pacing_enforces_minimum_spacing() {
        let g = guarded();
        g.pace().await; // first call: no history
        let start = Instant::now();
        g.pace().await; // second call must wait ~MIN_REQUEST_SPACING
        let elapsed = start.elapsed();
        assert!(
            elapsed >= MIN_REQUEST_SPACING - Duration::from_millis(400),
            "pacing leaked: {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn t09_mock_byte_transport_enforces_size_cap() {
        let transport = Arc::new(mock::MockTransport::with(vec![
            Ok("0123456789"),
            Ok("0123456789"),
        ]));
        let oversize = transport.get_bytes("https://api.mangadex.org/x", 4).await;
        assert!(matches!(oversize, Err(SourceError::Parse)));
        let ok = transport
            .get_bytes("https://api.mangadex.org/x", 1024)
            .await
            .unwrap();
        assert_eq!(ok.len(), 10);
    }
}
