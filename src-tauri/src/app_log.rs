//! Tiny append-only log for support. stderr stays the live view (dev console,
//! installer runs); the file is what a bug report can carry after the fact.
//! Rotates on the day boundary and at 5 MB, keeping one backup.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAX_BYTES: u64 = 5 * 1024 * 1024;

struct Sink {
    dir: PathBuf,
    day: String,
    file: File,
    written: u64,
}

static SINK: OnceLock<Mutex<Option<Sink>>> = OnceLock::new();

/// Point the log at `<app_data_dir>/logs`. Never fails startup: a log that
/// cannot be written is a stderr-only log, not an error the user must see.
pub fn init(app_data_dir: &Path) {
    let dir = app_data_dir.join("logs");
    let sink = std::fs::create_dir_all(&dir)
        .ok()
        .and_then(|_| open_append(&dir).ok());
    let _ = SINK.set(Mutex::new(sink));
}

fn day_stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn open_append(dir: &Path) -> std::io::Result<Sink> {
    let day = day_stamp();
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("app-{day}.log")))?;
    let written = file.metadata()?.len();
    Ok(Sink {
        dir: dir.to_path_buf(),
        day,
        file,
        written,
    })
}

/// Move today's log aside so a fresh one can start (used only at the size cap).
fn rotate(dir: &Path, day: &str) -> std::io::Result<()> {
    let current = dir.join(format!("app-{day}.log"));
    let backup = dir.join(format!("app-{day}.log.1"));
    let _ = std::fs::remove_file(&backup);
    if current.exists() {
        std::fs::rename(&current, &backup)?;
    }
    Ok(())
}

pub fn write(line: &str) {
    eprintln!("{line}");
    let Some(lock) = SINK.get() else {
        return;
    };
    let Ok(mut guard) = lock.lock() else {
        return;
    };
    let Some(sink) = guard.as_mut() else {
        return;
    };
    let today = day_stamp();
    if sink.day != today {
        // New day: start its file, leave yesterday's alone.
        if let Ok(next) = open_append(&sink.dir) {
            *sink = next;
        }
    } else if sink.written >= MAX_BYTES && rotate(&sink.dir, &today).is_ok() {
        if let Ok(next) = open_append(&sink.dir) {
            *sink = next;
        }
    }
    let entry = format!(
        "{} {line}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    if sink.file.write_all(entry.as_bytes()).is_ok() {
        sink.written += entry.len() as u64;
    }
}

#[macro_export]
macro_rules! app_log {
    ($($arg:tt)*) => {{
        $crate::app_log::write(&format!($($arg)*));
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn writes_are_appended_to_the_days_file() {
        let dir = tempfile::tempdir().unwrap();
        let sink = open_append(dir.path()).unwrap();
        let path = sink.dir.join(format!("app-{}.log", sink.day));
        drop(sink);

        // The static sink is process-wide; exercise the file path directly.
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"2026-10-27 10:00:00 hello\n").unwrap();
        drop(file);

        let mut text = String::new();
        File::open(&path)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert!(text.contains("hello"));
    }

    #[test]
    fn rotation_keeps_one_backup() {
        let dir = tempfile::tempdir().unwrap();
        let day = "2026-10-27";
        let current = dir.path().join(format!("app-{day}.log"));
        std::fs::write(&current, b"first run").unwrap();
        std::fs::write(dir.path().join(format!("app-{day}.log.1")), b"older").unwrap();

        rotate(dir.path(), day).unwrap();

        assert!(!current.exists());
        let backup = std::fs::read_to_string(dir.path().join(format!("app-{day}.log.1"))).unwrap();
        assert_eq!(
            backup, "first run",
            "the newest log replaces the older backup"
        );
    }

    #[test]
    fn writing_before_init_is_silent() {
        // No panic, no file: logging must never become a startup failure.
        write("unit test line");
    }
}
