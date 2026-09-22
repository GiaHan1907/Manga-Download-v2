use std::sync::{Arc, Mutex};

use crate::sources::registry::SourceRegistry;

pub struct AppState {
    pub db: sqlx::SqlitePool,
    /// Provider registry (Phase 05). Rebuilt when the user adds/removes a source.
    pub registry: Arc<Mutex<SourceRegistry>>,
}
