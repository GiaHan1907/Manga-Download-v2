use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::domain::settings::AppSettings;
use crate::downloader::scheduler::Coordinator;
use crate::sources::registry::SourceRegistry;

/// Settings snapshot the synchronous event paths (window close handler,
/// notification sink) read without awaiting the database. Refreshed by the
/// settings commands right after a validated persist.
#[derive(Debug)]
pub struct RuntimeFlags {
    pub notify_enabled: AtomicBool,
    pub close_to_tray: AtomicBool,
}

impl Default for RuntimeFlags {
    /// Mirrors `AppSettings::default()` so documented behavior holds even
    /// before (or if) the startup settings load lands.
    fn default() -> Self {
        RuntimeFlags {
            notify_enabled: AtomicBool::new(true),
            close_to_tray: AtomicBool::new(false),
        }
    }
}

impl RuntimeFlags {
    pub fn apply(&self, settings: &AppSettings) {
        self.notify_enabled
            .store(settings.notifications_enabled, Ordering::Relaxed);
        self.close_to_tray
            .store(settings.close_to_tray, Ordering::Relaxed);
    }

    pub fn notify_enabled(&self) -> bool {
        self.notify_enabled.load(Ordering::Relaxed)
    }

    pub fn close_to_tray(&self) -> bool {
        self.close_to_tray.load(Ordering::Relaxed)
    }
}

pub struct AppState {
    pub db: sqlx::SqlitePool,
    /// Provider registry (Phase 05). Rebuilt when the user adds/removes a source.
    pub registry: Arc<Mutex<SourceRegistry>>,
    /// Phase 06 download queue coordinator (dispatcher loop + task actions).
    pub downloads: Arc<Coordinator>,
    /// Phase 07: roots for library/reader artifact resolution (never a
    /// renderer-supplied path).
    pub app_data_dir: PathBuf,
    /// Phase 08: live mirror of notification/tray settings for sync paths.
    pub flags: Arc<RuntimeFlags>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t08_runtime_flags_mirror_settings_defaults() {
        let flags = RuntimeFlags::default();
        let settings = AppSettings::default();
        assert_eq!(flags.notify_enabled(), settings.notifications_enabled);
        assert_eq!(flags.close_to_tray(), settings.close_to_tray);

        let flipped = AppSettings {
            notifications_enabled: false,
            close_to_tray: true,
            speed_limit_kbps: 0,
            ..AppSettings::default()
        };
        flags.apply(&flipped);
        assert!(!flags.notify_enabled());
        assert!(flags.close_to_tray());
    }
}
