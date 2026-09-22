use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::downloader::scheduler::Coordinator;
use crate::sources::registry::SourceRegistry;

pub struct AppState {
    pub db: sqlx::SqlitePool,
    /// Provider registry (Phase 05). Rebuilt when the user adds/removes a source.
    pub registry: Arc<Mutex<SourceRegistry>>,
    /// Phase 06 download queue coordinator (dispatcher loop + task actions).
    pub downloads: Arc<Coordinator>,
    /// Phase 07: roots for library/reader artifact resolution (never a
    /// renderer-supplied path).
    pub app_data_dir: PathBuf,
}
