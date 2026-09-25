//! ECH (Encrypted ClientHello) fallback transport.
//!
//! Some ISPs reset TLS handshakes whose plaintext SNI names a blocked
//! domain (Vietnam DPI pattern, observed on truyentuoitho.com 2026-09-25).
//! Real browsers ride through because Chrome encrypts the SNI via ECH with
//! Cloudflare. wreq emulates fingerprints but not ECH, so this module does
//! what Chrome's stack does for the fetches we own:
//!
//! 1. Resolve the target through DNS-over-HTTPS (`cloudflare-dns.com`,
//!    whose own SNI is not blocked) querying HTTPS records — they carry the
//!    `ech=` ECHConfigList and `ipv4hint=` addresses.
//! 2. Open TCP to the DoH-returned (validated public) IP, then a btls
//!    handshake with hostname + ECH config list: the inner ClientHello
//!    carries the real SNI encrypted; the outer one uses the config's
//!    public name. Certificate verification is against the REAL hostname.
//! 3. Speak plain HTTP/1.1 over it (Connection: close, 2 MB cap, chunked
//!    decoding) — enough for GET text/JSON and admin-ajax form POSTs.
//!
//! GuardedHttp keeps trying its normal client first; this path only runs
//! after a connection-level failure, and every host/IP still passes the
//! exact-host allowlist and public-IP checks.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::OnceLock;
use std::time::Duration;

use base64::Engine as _;
use btls::ssl::{SslContext, SslMethod, SslVerifyMode};
use btls::x509::store::X509StoreBuilder;
use btls::x509::X509;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio_btls::SslStream;
use url::Url;

use super::adapter::SourceError;
use super::http::{is_public_ip, MAX_BODY_BYTES, REQUEST_TIMEOUT};

const DOH_BASE: &str = "https://cloudflare-dns.com/dns-query";
const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/139.0.0.0 Safari/537.36";
const READ_CAP: u64 = MAX_BODY_BYTES + 65_536;

pub struct EchResponse {
    pub status: u16,
    headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl EchResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Clone)]
struct Bundle {
    ech: Vec<u8>,
    ips: Vec<IpAddr>,
}

static CACHE: OnceLock<Mutex<HashMap<String, Bundle>>> = OnceLock::new();
static TLS_CTX: OnceLock<SslContext> = OnceLock::new();
static DOH_CLIENT: OnceLock<wreq::Client> = OnceLock::new();

fn tls_context() -> &'static SslContext {
    TLS_CTX.get_or_init(|| {
        let mut builder =
            btls::ssl::SslContextBuilder::new(SslMethod::tls()).expect("btls TLS client context");
        // Windows system root store (DER) — same trust anchors Chrome uses.
        let mut store = X509StoreBuilder::new().expect("x509 store");
        for cert_der in rustls_native_certs::load_native_certs().certs {
            if let Ok(cert) = X509::from_der(&cert_der) {
                let _ = store.add_cert(&cert);
            }
        }
        builder.set_cert_store(store.build());
        builder.set_verify(SslVerifyMode::PEER);
        builder.build()
    })
}

fn doh_client() -> &'static wreq::Client {
    DOH_CLIENT.get_or_init(|| {
        wreq::Client::builder()
            .emulation(wreq_util::Emulation::Chrome139)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("doh client")
    })
}

fn extract_field(data: &str, key: &str) -> Option<String> {
    let mut rest = data;
    while let Some(pos) = rest.find(key) {
        let after = &rest[pos + key.len()..];
        // Only accept when `key` starts a token (preceded by space/start).
        let preceded_ok = &rest[..pos]
            .chars()
            .last()
            .map(|c| c.is_whitespace())
            .unwrap_or(true);
        if *preceded_ok {
            let value = after.trim_start().strip_prefix('=')?;
            let value = value.trim_start_matches('"');
            let end = value
                .find(|c: char| c == '"' || c.is_whitespace())
                .unwrap_or(value.len());
            let token = &value[..end];
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
        rest = after;
    }
    None
}

fn decode_ech(token: &str) -> Option<Vec<u8>> {
    let engines = [
        base64::engine::general_purpose::URL_SAFE_NO_PAD,
        base64::engine::general_purpose::URL_SAFE,
        base64::engine::general_purpose::STANDARD,
    ];
    engines
        .iter()
        .filter_map(|engine| engine.decode(token).ok())
        .find(|bytes| !bytes.is_empty())
}

fn parse_ips(csv: &str) -> Vec<IpAddr> {
    csv.split(',')
        .filter_map(|entry| entry.trim().parse::<IpAddr>().ok())
        .filter(is_public_ip)
        .collect()
}

async fn doh_query(host: &str, qtype: &str) -> Result<serde_json::Value, SourceError> {
    let url = format!("{DOH_BASE}?name={host}&type={qtype}");
    let response = doh_client()
        .get(&url)
        .header("accept", "application/dns-json")
        .send()
        .await
        .map_err(|_| SourceError::Network)?;
    let body = response.text().await.map_err(|_| SourceError::Parse)?;
    serde_json::from_str(&body).map_err(|_| SourceError::Parse)
}

async fn resolve_bundle(host: &str) -> Result<Bundle, SourceError> {
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(hit) = cache.lock().await.get(host) {
        return Ok(hit.clone());
    }
    let https = doh_query(host, "HTTPS").await?;
    let mut ech: Option<Vec<u8>> = None;
    let mut ips: Vec<IpAddr> = Vec::new();
    if let Some(answers) = https["Answer"].as_array() {
        for answer in answers {
            let Some(data) = answer["data"].as_str() else {
                continue;
            };
            if ech.is_none() {
                if let Some(token) = extract_field(data, "ech") {
                    ech = decode_ech(&token);
                }
            }
            if ips.is_empty() {
                if let Some(hint) = extract_field(data, "ipv4hint") {
                    ips = parse_ips(&hint);
                }
            }
        }
    }
    if ips.is_empty() {
        let a = doh_query(host, "A").await?;
        if let Some(answers) = a["Answer"].as_array() {
            for answer in answers {
                if let Some(data) = answer["data"].as_str() {
                    ips.extend(parse_ips(data));
                }
            }
        }
    }
    let bundle = Bundle {
        ech: ech.ok_or(SourceError::Network)?,
        ips,
    };
    if bundle.ips.is_empty() {
        return Err(SourceError::Network);
    }
    cache.lock().await.insert(host.to_string(), bundle.clone());
    Ok(bundle)
}

fn request_path(url: &Url) -> String {
    let path = if url.path().is_empty() {
        "/"
    } else {
        url.path()
    };
    match url.query() {
        Some(query) => format!("{path}?{query}"),
        None => path.to_string(),
    }
}

fn encode_form(form: &[(&str, &str)]) -> String {
    let encode = |value: &str| -> String {
        value
            .chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                ' ' => "+".to_string(),
                _ => c
                    .to_string()
                    .as_bytes()
                    .iter()
                    .map(|b| format!("%{b:02X}"))
                    .collect::<String>(),
            })
            .collect()
    };
    form.iter()
        .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn dechunk(mut body: &[u8]) -> Result<Vec<u8>, SourceError> {
    let mut out = Vec::new();
    loop {
        let Some(end) = find(body, b"\r\n") else {
            return Err(SourceError::Parse);
        };
        let size_line = std::str::from_utf8(&body[..end]).map_err(|_| SourceError::Parse)?;
        let size = usize::from_str_radix(size_line.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| SourceError::Parse)?;
        body = &body[end + 2..];
        if size == 0 {
            return Ok(out);
        }
        if body.len() < size + 2 {
            return Err(SourceError::Parse);
        }
        if out.len() as u64 + size as u64 > MAX_BODY_BYTES {
            return Err(SourceError::Parse);
        }
        out.extend_from_slice(&body[..size]);
        body = &body[size + 2..];
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len())
        .position(|window| window == needle)
}

fn parse_response(raw: Vec<u8>) -> Result<EchResponse, SourceError> {
    let split = find(&raw, b"\r\n\r\n").ok_or(SourceError::Parse)?;
    let head = std::str::from_utf8(&raw[..split]).map_err(|_| SourceError::Parse)?;
    let mut lines = head.lines();
    let status_line = lines.next().ok_or(SourceError::Parse)?;
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .ok_or(SourceError::Parse)?;
    let mut headers = Vec::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    let mut body = raw[split + 4..].to_vec();
    let chunked = headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("transfer-encoding") && value.contains("chunked")
    });
    if chunked {
        body = dechunk(&body)?;
    }
    if body.len() as u64 > MAX_BODY_BYTES {
        return Err(SourceError::Parse);
    }
    Ok(EchResponse {
        status,
        headers,
        body,
    })
}

async fn attempt(
    url: &Url,
    bundle: &Bundle,
    ip: IpAddr,
    form: Option<&[(&str, &str)]>,
) -> Result<EchResponse, SourceError> {
    let host = url.host_str().ok_or(SourceError::MalformedUrl)?;
    let tcp = timeout(
        Duration::from_secs(10),
        TcpStream::connect(SocketAddr::new(ip, 443)),
    )
    .await
    .map_err(|_| SourceError::Network)?
    .map_err(|_| SourceError::Network)?;
    let _ = tcp.set_nodelay(true);

    let mut ssl = btls::ssl::Ssl::new(tls_context()).map_err(|_| SourceError::Network)?;
    ssl.set_hostname(host).map_err(|_| SourceError::Network)?;
    ssl.set_alpn_protos(b"\x08http/1.1")
        .map_err(|_| SourceError::Network)?;
    ssl.set_ech_config_list(&bundle.ech)
        .map_err(|_| SourceError::Network)?;

    let mut stream = SslStream::new(ssl, tcp).map_err(|_| SourceError::Network)?;
    timeout(REQUEST_TIMEOUT, Pin::new(&mut stream).connect())
        .await
        .map_err(|_| SourceError::Network)?
        .map_err(|_| SourceError::Network)?;

    let (method, payload) = match form {
        None => ("GET", Vec::new()),
        Some(fields) => ("POST", encode_form(fields).into_bytes()),
    };
    let target = request_path(url);
    let mut request = format!(
        "{method} {target} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: {CHROME_UA}\r\n\
         Accept: text/html,application/xhtml+xml,application/json;q=0.9,*/*;q=0.8\r\n\
         Accept-Language: vi-VN,vi;q=0.9,en-US;q=0.8,en;q=0.7\r\n\
         Accept-Encoding: identity\r\nConnection: close\r\n"
    );
    if !payload.is_empty() {
        request.push_str("Content-Type: application/x-www-form-urlencoded\r\n");
        request.push_str(&format!("Content-Length: {}\r\n", payload.len()));
    }
    request.push_str("\r\n");
    let raw = timeout(REQUEST_TIMEOUT, async {
        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|_| SourceError::Network)?;
        if !payload.is_empty() {
            stream
                .write_all(&payload)
                .await
                .map_err(|_| SourceError::Network)?;
        }
        stream.flush().await.map_err(|_| SourceError::Network)?;
        let mut collected: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 16 * 1024];
        loop {
            let read = stream
                .read(&mut chunk)
                .await
                .map_err(|_| SourceError::Network)?;
            if read == 0 {
                break;
            }
            collected.extend_from_slice(&chunk[..read]);
            if collected.len() as u64 > READ_CAP {
                return Err(SourceError::Parse);
            }
        }
        Ok::<Vec<u8>, SourceError>(collected)
    })
    .await
    .map_err(|_| SourceError::Network)??;
    parse_response(raw)
}

/// Public entry: one HTTP exchange over ECH. Caller already validated the
/// URL against its host policy.
pub async fn fetch(url: &Url, form: Option<&[(&str, &str)]>) -> Result<EchResponse, SourceError> {
    let host = url.host_str().ok_or(SourceError::MalformedUrl)?;
    if url.scheme() != "https" {
        return Err(SourceError::MalformedUrl);
    }
    let bundle = resolve_bundle(host).await?;
    let mut last = SourceError::Network;
    for ip in bundle.ips.iter().take(3) {
        match attempt(url, &bundle, *ip, form).await {
            Ok(response) => return Ok(response),
            Err(error) => last = error,
        }
    }
    Err(last)
}
