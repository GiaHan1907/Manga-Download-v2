use std::sync::Arc;

use super::adapter::SourceAdapter;
use super::providers::madara_generic::MadaraGeneric;
use super::providers::mangadex::MangaDex;
use crate::db::repositories::source_repo::SourceRow;

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
        let mut adapters: Vec<Arc<dyn SourceAdapter>> = vec![Arc::new(MangaDex::live())];
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        seen.insert("mangadex".to_string());
        for row in rows {
            if row.adapter != "madara" {
                continue;
            }
            let Some(base_url) = row.base_url.as_deref() else {
                eprintln!("source row {} has no base_url; skipped", row.id);
                continue;
            };
            match MadaraGeneric::live(&row.display_name, base_url) {
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
