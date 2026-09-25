use std::sync::Arc;

use super::adapter::SourceAdapter;
use super::providers::madara_generic::MadaraGeneric;
use super::providers::mangadex::MangaDex;
use crate::db::repositories::source_repo::SourceRow;

/// App-shipped sources: never removable, toggleable like any other.
pub const BUILTIN_IDS: &[&str] = &["mangadex", "madara-truyentuoitho-com"];
pub const TTT_BASE_URL: &str = "https://truyentuoitho.com/";
pub const TTT_DISPLAY_NAME: &str = "Truyen Tuoi Tho";
pub const TTT_IMAGE_HOSTS: &str = "img.resourcehub.shop";

pub fn is_builtin(id: &str) -> bool {
    BUILTIN_IDS.contains(&id)
}

fn builtin_adapters() -> Vec<Arc<dyn SourceAdapter>> {
    let ttt_hosts: Vec<String> = vec![TTT_IMAGE_HOSTS.to_string()];
    vec![
        Arc::new(MangaDex::live()),
        // Static, reviewed config: a panic here would be a shipping bug,
        // surfaced immediately by the registry unit tests instead.
        Arc::new(
            MadaraGeneric::live(TTT_DISPLAY_NAME, TTT_BASE_URL, &ttt_hosts)
                .expect("built-in Truyen Tuoi Tho config must be valid"),
        ),
    ]
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
        let mut adapters: Vec<Arc<dyn SourceAdapter>> = builtin_adapters();
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
            match MadaraGeneric::live(&row.display_name, base_url, &hosts) {
                Ok(adapter) => {
                    if adapter.info().id != row.id {
                        eprintln!(
                            "source row id {} mismatches host-derived id; skipped",
                            row.id
                        );
                        continue;
                    }
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
