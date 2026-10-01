use std::sync::Arc;

use super::adapter::SourceAdapter;
use super::providers::madara_generic::{MadaraGeneric, SiteProfile};
use super::providers::mangadex::MangaDex;
use crate::db::repositories::source_repo::SourceRow;

/// App-shipped sources: never removable, toggleable like any other.
pub const BUILTIN_IDS: &[&str] = &[
    "mangadex",
    "madara-truyentuoitho-com",
    "truyenqq",
    "nettruyen",
    "dilib",
];
pub const TTT_BASE_URL: &str = "https://truyentuoitho.com/";
pub const TTT_DISPLAY_NAME: &str = "Truyen Tuoi Tho";
pub const TTT_IMAGE_HOSTS: &str = "img.resourcehub.shop";
pub const QQ_BASE_URL: &str = "https://truyenqqko.com/";
pub const QQ_DISPLAY_NAME: &str = "TruyenQQ";
pub const NT_BASE_URL: &str = "https://nettruyen.gg/";
pub const NT_DISPLAY_NAME: &str = "NetTruyen";
pub const DL_BASE_URL: &str = "https://dilib.vn/";
pub const DL_DISPLAY_NAME: &str = "Dilib";

/// Reviewed HTML-site built-ins sharing the generic scraper pipeline. Their
/// ids are FIXED (never host-derived) so a domain edit keeps saved manga
/// pointing at the same source.
pub struct BuiltInHtmlSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    /// Value stored in `sources.adapter` for this row.
    pub adapter: &'static str,
    pub base_url: &'static str,
    /// Exact image hosts (madara profile only), CSV.
    pub image_hosts: Option<&'static str>,
    pub profile: SiteProfile,
}

pub const BUILTIN_HTML_SOURCES: &[BuiltInHtmlSpec] = &[
    BuiltInHtmlSpec {
        id: "madara-truyentuoitho-com",
        display_name: TTT_DISPLAY_NAME,
        adapter: "madara",
        base_url: TTT_BASE_URL,
        image_hosts: Some(TTT_IMAGE_HOSTS),
        profile: SiteProfile::Madara,
    },
    BuiltInHtmlSpec {
        id: "truyenqq",
        display_name: QQ_DISPLAY_NAME,
        adapter: "truyenqq",
        base_url: QQ_BASE_URL,
        image_hosts: None,
        profile: SiteProfile::TruyenQQ,
    },
    BuiltInHtmlSpec {
        id: "nettruyen",
        display_name: NT_DISPLAY_NAME,
        adapter: "nettruyen",
        base_url: NT_BASE_URL,
        image_hosts: None,
        profile: SiteProfile::Nettruyen,
    },
    BuiltInHtmlSpec {
        id: "dilib",
        display_name: DL_DISPLAY_NAME,
        adapter: "dilib",
        base_url: DL_BASE_URL,
        image_hosts: None,
        profile: SiteProfile::Dilib,
    },
];

pub fn builtin_html_spec(id: &str) -> Option<&'static BuiltInHtmlSpec> {
    BUILTIN_HTML_SOURCES.iter().find(|spec| spec.id == id)
}

pub fn is_builtin(id: &str) -> bool {
    BUILTIN_IDS.contains(&id)
}

/// Built-in adapters, honoring a persisted override row (domain/name/hosts
/// edited by the user in Settings → Nguồn). An invalid override falls back to
/// the shipped defaults with a diagnostic — startup never fails on it.
/// Image hosts a built-in adapter may read chapter pages from. A persisted row
/// wins, but its NULL column is the state of a row created by a plain
/// enable/disable toggle — that means "never edited", so the shipped list
/// still applies. Only an explicit value replaces it.
fn built_in_image_hosts<'a>(
    row: Option<&'a SourceRow>,
    spec: &'a BuiltInHtmlSpec,
) -> Option<&'a str> {
    row.and_then(|r| r.image_hosts.as_deref())
        .or(spec.image_hosts)
}

fn builtin_adapters(rows: &[SourceRow]) -> Vec<Arc<dyn SourceAdapter>> {
    let mut adapters: Vec<Arc<dyn SourceAdapter>> = vec![Arc::new(MangaDex::live())];
    for spec in BUILTIN_HTML_SOURCES {
        let row = rows.iter().find(|r| r.id == spec.id);
        let display_name = row
            .map(|r| r.display_name.as_str())
            .unwrap_or(spec.display_name);
        let base_url = row
            .and_then(|r| r.base_url.as_deref())
            .unwrap_or(spec.base_url);
        // A built-in's row is created by a mere enable/disable toggle, which
        // stores NULL — so NULL means "never edited", NOT "no image hosts".
        // Treating it as an empty list silently strips the shipped CDN host and
        // every chapter then fails as SOURCE_PARSE_ERROR (live bug, 2026-10-01:
        // TTT serves all its pages from img.resourcehub.shop).
        let hosts_csv = built_in_image_hosts(row, spec);
        let hosts: Vec<String> = hosts_csv
            .map(|csv| {
                csv.split(',')
                    .filter_map(|entry| {
                        crate::sources::providers::madara_generic::normalize_image_host(entry)
                    })
                    .collect()
            })
            .unwrap_or_default();
        match MadaraGeneric::built_in(spec.id, display_name, base_url, &hosts, spec.profile) {
            Ok(adapter) => adapters.push(Arc::new(adapter)),
            Err(error) => {
                eprintln!(
                    "built-in {} override invalid ({}); using shipped defaults",
                    spec.id,
                    error.code()
                );
                let fallback_hosts: Vec<String> = spec
                    .image_hosts
                    .map(|csv| {
                        csv.split(',')
                            .filter_map(|entry| {
                                crate::sources::providers::madara_generic::normalize_image_host(
                                    entry,
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                adapters.push(Arc::new(
                    MadaraGeneric::built_in(
                        spec.id,
                        spec.display_name,
                        spec.base_url,
                        &fallback_hosts,
                        spec.profile,
                    )
                    .expect("built-in config must be valid"),
                ));
            }
        }
    }
    adapters
}

/// Registry of app-shipped, vetted adapters. The renderer cannot register new
/// hosts or adapters; tests inject bespoke registries (fixture adapters).
#[derive(Clone, Default)]
pub struct SourceRegistry {
    adapters: Vec<Arc<dyn SourceAdapter>>,
}

impl SourceRegistry {
    #[cfg(test)]
    pub fn with_adapters(adapters: Vec<Arc<dyn SourceAdapter>>) -> Self {
        SourceRegistry { adapters }
    }

    /// Built-in providers plus user-registered Madara sites from the DB.
    /// Invalid or duplicate rows are skipped with a diagnostic, never fatal.
    pub fn from_rows(rows: &[SourceRow]) -> Self {
        let mut adapters: Vec<Arc<dyn SourceAdapter>> = builtin_adapters(rows);
        let mut seen: std::collections::HashSet<String> =
            BUILTIN_IDS.iter().map(|id| id.to_string()).collect();
        for row in rows {
            if row.adapter != "madara" {
                continue;
            }
            let Some(base_url) = row.base_url.as_deref() else {
                eprintln!("source row {} has no base_url; skipped", row.id);
                continue;
            };
            let hosts: Vec<String> = row
                .image_hosts
                .as_deref()
                .map(|csv| {
                    csv.split(',')
                        .filter_map(|entry| {
                            crate::sources::providers::madara_generic::normalize_image_host(entry)
                        })
                        .collect()
                })
                .unwrap_or_default();
            // The stored row id IS the source identity — it deliberately
            // survives domain moves (the whole point of the edit feature:
            // saved manga keep pointing at the same source id).
            match MadaraGeneric::built_in(
                &row.id,
                &row.display_name,
                base_url,
                &hosts,
                SiteProfile::Madara,
            ) {
                Ok(adapter) => {
                    if seen.insert(row.id.clone()) {
                        adapters.push(Arc::new(adapter));
                    }
                }
                Err(error) => eprintln!("invalid source row {}: {}; skipped", row.id, error.code()),
            }
        }
        SourceRegistry { adapters }
    }

    pub fn adapters(&self) -> &[Arc<dyn SourceAdapter>] {
        &self.adapters
    }

    pub fn get(&self, source_id: &str) -> Option<Arc<dyn SourceAdapter>> {
        self.adapters
            .iter()
            .find(|adapter| adapter.info().id == source_id)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(image_hosts: Option<&str>) -> SourceRow {
        SourceRow {
            id: "madara-truyentuoitho-com".to_string(),
            display_name: TTT_DISPLAY_NAME.to_string(),
            adapter: "madara".to_string(),
            base_url: Some(TTT_BASE_URL.to_string()),
            image_hosts: image_hosts.map(str::to_string),
            enabled: true,
        }
    }

    /// A row created by a plain enable toggle stores NULL. That must not be
    /// read as "this source has no image CDN" — it silently stripped TTT's
    /// shipped host and made every chapter fail SOURCE_PARSE_ERROR.
    #[test]
    fn a_never_edited_builtin_row_keeps_the_shipped_image_hosts() {
        let spec = builtin_html_spec("madara-truyentuoitho-com").expect("TTT is built in");
        assert_eq!(
            built_in_image_hosts(Some(&row(None)), spec),
            Some(TTT_IMAGE_HOSTS)
        );
        assert_eq!(built_in_image_hosts(None, spec), Some(TTT_IMAGE_HOSTS));
    }

    /// An explicit value is the user's own list and replaces the shipped one.
    #[test]
    fn an_edited_builtin_row_overrides_the_shipped_image_hosts() {
        let spec = builtin_html_spec("madara-truyentuoitho-com").expect("TTT is built in");
        assert_eq!(
            built_in_image_hosts(Some(&row(Some("cdn.example.net"))), spec),
            Some("cdn.example.net")
        );
        // A built-in that ships no extra host stays base-host-only.
        let qq = builtin_html_spec("truyenqq").expect("TruyenQQ is built in");
        assert_eq!(built_in_image_hosts(Some(&row(None)), qq), None);
    }
}
