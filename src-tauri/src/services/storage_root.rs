//! User-selectable download root (Settings → Lưu trữ).
//!
//! The engine keeps addressing everything through `downloader::storage`,
//! which derives `<base>/library`. A custom root is implemented as: real
//! files live at `<chosen>/library`, and the app-data `library` slot becomes
//! an NTFS junction pointing there. Every existing consumer (reader,
//! reconcile, reveal, import, purge) resolves through the junction without
//! knowing it moved, and DB rows stay relative paths. Staging/cache never
//! move — only the verified library does.
//!
//! The move is copy-first with byte-level verification; nothing at the old
//! location is removed until the new tree is proven identical.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sqlx::SqlitePool;

use crate::db::repositories::settings_repo;
use crate::domain::now_iso;
use crate::downloader::storage;
use crate::error::AppError;

pub const SETTINGS_KEY: &str = "storage.downloadRoot.v1";
const MAX_TREE_ENTRIES: usize = 100_000;
const COPY_CHUNK: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootInfoDto {
    pub is_custom: bool,
    /// Human label safe for the settings screen: the folder the user picked,
    /// or a fixed string for the built-in default.
    pub label: String,
    /// When custom: whether the junction into the chosen folder currently
    /// resolves. Broken links surface here, never as silent empty library.
    pub link_ok: bool,
}

fn default_library(app_data_dir: &Path) -> PathBuf {
    storage::library_root(app_data_dir)
}

/// True when the path is a reparse point (junction or directory symlink).
pub fn is_reparse_dir(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => meta.file_type().is_symlink(),
        Err(_) => false,
    }
}

async fn stored_custom_root(pool: &SqlitePool) -> Option<PathBuf> {
    let raw = settings_repo::get_json(pool, SETTINGS_KEY).await.ok()??;
    serde_json::from_str::<String>(&raw).ok().map(PathBuf::from)
}

/// Where the verified files physically live right now: the app-data `library`
/// directory itself, or the folder the junction points at. `None` means the
/// junction dangles — the folder was moved or renamed outside the app, which
/// is exactly the state Settings asks the user to repair by re-choosing it.
fn reachable_source(lib_slot: &Path) -> Option<PathBuf> {
    if is_reparse_dir(lib_slot) {
        lib_slot.canonicalize().ok()
    } else {
        Some(lib_slot.to_path_buf())
    }
}

fn has_contents(path: &Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

async fn downloaded_chapters(pool: &SqlitePool) -> Result<i64, AppError> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapters WHERE local_path IS NOT NULL")
            .fetch_one(pool)
            .await?;
    Ok(count)
}

pub async fn current(pool: &SqlitePool, app_data_dir: &Path) -> Result<RootInfoDto, AppError> {
    let custom = stored_custom_root(pool).await;
    let lib = default_library(app_data_dir);
    match custom {
        Some(base) => Ok(RootInfoDto {
            is_custom: true,
            label: base.display().to_string(),
            link_ok: is_reparse_dir(&lib) && lib.exists(),
        }),
        None => Ok(RootInfoDto {
            is_custom: false,
            label: "Mặc định (AppData của app)".to_string(),
            link_ok: true,
        }),
    }
}

/// Reject bases where the junction scheme would loop, nest inside the app's
/// own data, or land the library on an OS-managed directory.
pub fn validate_candidate(app_data_dir: &Path, candidate: &Path) -> Result<(), AppError> {
    let reject = |msg: &str| AppError::validation(msg.to_string());
    if !candidate.is_absolute() {
        return Err(reject("Đường dẫn phải là tuyệt đối."));
    }
    if candidate.parent().is_none() {
        return Err(reject(
            "Không thể chọn gốc ổ đĩa — hãy chọn một thư mục con.",
        ));
    }
    // Compare through canonical forms where they exist; the candidate may
    // not exist yet, so canonicalize its nearest existing ancestor and
    // re-append the remainder (keeps `\\?\` prefixes consistent on both
    // sides, otherwise starts_with() silently lies).
    fn anchor(p: &Path) -> PathBuf {
        let mut suffix: Vec<std::ffi::OsString> = Vec::new();
        let mut cur = p;
        loop {
            if let Ok(canonical) = cur.canonicalize() {
                let mut out = canonical;
                for part in suffix.iter().rev() {
                    out.push(part);
                }
                return out;
            }
            match (cur.parent(), cur.file_name()) {
                (Some(parent), Some(name)) => {
                    suffix.push(name.to_os_string());
                    cur = parent;
                }
                _ => return p.to_path_buf(),
            }
        }
    }
    let app_data = anchor(app_data_dir);
    let cand = anchor(candidate);
    if cand == app_data || cand.starts_with(&app_data) {
        return Err(reject(
            "Thư mục này nằm trong dữ liệu của app — chọn nơi khác (VD: D:\\Manga).",
        ));
    }
    // A candidate ABOVE the app-data dir would make library contain appdata.
    if app_data.starts_with(&cand) {
        return Err(reject(
            "Thư mục chọn chứa cả dữ liệu của app — chọn thư mục cụ thể hơn.",
        ));
    }
    for var in [
        "SystemRoot",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
    ] {
        if let Ok(value) = std::env::var(var) {
            let blocked = anchor(Path::new(&value));
            if cand == blocked || blocked.starts_with(&cand) {
                return Err(reject(
                    "Không thể dùng thư mục hệ thống Windows làm nơi lưu.",
                ));
            }
        }
    }
    if is_reparse_dir(candidate) {
        return Err(reject("Thư mục chọn là liên kết — chọn thư mục thật."));
    }
    Ok(())
}

fn stream_equal(a: &Path, b: &Path) -> Result<bool, AppError> {
    let size_a = std::fs::metadata(a)?.len();
    let size_b = std::fs::metadata(b)?.len();
    if size_a != size_b {
        return Ok(false);
    }
    let (mut fa, mut fb) = (File::open(a)?, File::open(b)?);
    let (mut buf_a, mut buf_b) = (vec![0u8; COPY_CHUNK], vec![0u8; COPY_CHUNK]);
    loop {
        let n_a = read_partial(&mut fa, &mut buf_a)?;
        let n_b = read_partial(&mut fb, &mut buf_b)?;
        if n_a != n_b || buf_a[..n_a] != buf_b[..n_b] {
            return Ok(false);
        }
        if n_a == 0 {
            return Ok(true);
        }
    }
}

fn read_partial(reader: &mut File, buf: &mut [u8]) -> Result<usize, AppError> {
    let mut filled = 0usize;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(AppError::Io(e)),
        }
    }
    Ok(filled)
}

fn walk_files(root: &Path) -> Result<Vec<PathBuf>, AppError> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue; // engine never creates links inside library
            }
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() {
                out.push(path);
                if out.len() > MAX_TREE_ENTRIES {
                    return Err(AppError::validation("Thư viện quá lớn để tự chuyển."));
                }
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Default)]
pub struct CopyReport {
    pub copied: usize,
    pub skipped_identical: usize,
}

/// Merge-copy `src` into `dst`, verifying every byte after copy. A differing
/// same-name file is a hard conflict: nothing is rolled back because nothing
/// was overwritten yet.
pub fn copy_tree_verified(src: &Path, dst: &Path) -> Result<CopyReport, AppError> {
    let mut report = CopyReport::default();
    if !src.exists() {
        return Ok(report);
    }
    if is_reparse_dir(src) {
        return Err(AppError::validation(
            "Nguồn chuyển là liên kết — bất thường, hủy.",
        ));
    }
    for file in walk_files(src)? {
        let rel = file
            .strip_prefix(src)
            .map_err(|_| AppError::Internal)?
            .to_path_buf();
        let target = dst.join(&rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if target.exists() {
            if stream_equal(&file, &target)? {
                report.skipped_identical += 1;
                continue;
            }
            return Err(AppError::validation(format!(
                "Xung đột tệp '{}' — thư mục đích đã có tệp khác. Dọn nó rồi thử lại.",
                rel.display()
            )));
        }
        let tmp = {
            let mut name = target
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".to_string());
            name.push_str(".m1989-part");
            target.with_file_name(name)
        };
        std::fs::copy(&file, &tmp)?;
        if !stream_equal(&file, &tmp)? {
            let _ = std::fs::remove_file(&tmp);
            return Err(AppError::validation(
                "Bản sao không khớp byte gốc — hủy chuyển (đĩa lỗi?).".to_string(),
            ));
        }
        std::fs::rename(&tmp, &target)?;
        report.copied += 1;
    }
    Ok(report)
}

/// Windows: create `<appdata>/library` junction to `target`.
#[cfg(windows)]
fn create_junction(target: &Path, junction: &Path) -> Result<(), AppError> {
    junction::create(target, junction)
        .map_err(|e| AppError::validation(format!("Không tạo được liên kết thư mục: {}", e.kind())))
}

#[cfg(not(windows))]
fn create_junction(_target: &Path, _junction: &Path) -> Result<(), AppError> {
    Err(AppError::Unsupported)
}

/// Windows: remove a junction without touching its target contents.
///
/// `junction::delete` silently no-ops on a DANGLING link (it cannot stat the
/// target), which leaves the reparse point in place so the next create fails
/// with "entity already exists" — precisely the repair needed after the user
/// moved the folder in Explorer. RemoveDirectory does drop a dangling
/// junction, so fall back to it.
#[cfg(windows)]
fn remove_junction(junction: &Path) -> Result<(), AppError> {
    if std::fs::symlink_metadata(junction).is_err() {
        return Ok(()); // nothing to detach
    }
    if junction::delete(junction).is_ok() && std::fs::symlink_metadata(junction).is_err() {
        return Ok(());
    }
    match std::fs::remove_dir(junction) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::Io(error)),
    }
}

#[cfg(not(windows))]
fn remove_junction(_junction: &Path) -> Result<(), AppError> {
    Err(AppError::Unsupported)
}

async fn remember_root(pool: &SqlitePool, base: &Path) -> Result<(), AppError> {
    settings_repo::upsert_json(
        pool,
        SETTINGS_KEY,
        &serde_json::to_string(&base.display().to_string()).map_err(|_| AppError::Internal)?,
        &now_iso(),
    )
    .await
}

async fn forget_root(pool: &SqlitePool) -> Result<(), AppError> {
    settings_repo::delete_json(pool, SETTINGS_KEY).await
}

/// Move the verified library under `new_base` and link it into app-data.
pub async fn relocate(
    pool: &SqlitePool,
    app_data_dir: &Path,
    new_base: &Path,
) -> Result<RootInfoDto, AppError> {
    validate_candidate(app_data_dir, new_base)?;
    let lib_slot = default_library(app_data_dir);
    let dst = new_base.join("library");

    // Source of truth today: the real app-data dir, or the junction's target
    // (root switch A → B). A dangling junction has nothing readable to copy —
    // the tree already lives under the folder the user is now pointing at.
    let src = reachable_source(&lib_slot);
    match &src {
        Some(current) => {
            if *current == dst.canonicalize().unwrap_or_else(|_| dst.clone()) {
                return Err(AppError::validation("Thư viện đã ở đúng nơi này."));
            }
        }
        None => {
            let claimed = downloaded_chapters(pool).await?;
            if claimed > 0 && !has_contents(&dst) {
                return Err(AppError::validation(format!(
                    "Liên kết cũ đang hỏng và `{}` không có truyện nào, trong khi app vẫn ghi nhận \
                     {claimed} chương đã tải. Hãy chọn đúng thư mục ĐANG chứa thư mục `library` \
                     (VD: file ở `D:\\Manga\\library` thì chọn `D:\\Manga`).",
                    dst.display()
                )));
            }
        }
    }

    std::fs::create_dir_all(&dst)?;
    if let Some(current) = &src {
        copy_tree_verified(current, &dst)?;
    }

    // Detach the old slot, attach the junction; roll back on link failure.
    let rollback_dir: Option<PathBuf> = if is_reparse_dir(&lib_slot) {
        remove_junction(&lib_slot)?;
        None
    } else if lib_slot.exists() {
        let stamp = now_iso().replace([':', '.', '-'], "");
        let parked = lib_slot.with_file_name(format!("library.old-{stamp}"));
        std::fs::rename(&lib_slot, &parked)?;
        Some(parked)
    } else {
        None
    };
    if let Err(error) = create_junction(&dst, &lib_slot) {
        if let Some(parked) = &rollback_dir {
            let _ = std::fs::rename(parked, &lib_slot);
        }
        return Err(error);
    }
    if let Err(error) = remember_root(pool, new_base).await {
        let _ = remove_junction(&lib_slot);
        if let Some(parked) = &rollback_dir {
            let _ = std::fs::rename(parked, &lib_slot);
        }
        return Err(error);
    }
    if let Some(parked) = &rollback_dir {
        // Best effort: unlink failures leave a clearly-named backup.
        let _ = std::fs::remove_dir_all(parked);
    }
    Ok(RootInfoDto {
        is_custom: true,
        label: new_base.display().to_string(),
        link_ok: true,
    })
}

/// Copy the custom library back to app-data and drop the junction.
pub async fn reset_to_default(
    pool: &SqlitePool,
    app_data_dir: &Path,
) -> Result<RootInfoDto, AppError> {
    let lib_slot = default_library(app_data_dir);
    if !is_reparse_dir(&lib_slot) {
        forget_root(pool).await.ok();
        return Ok(RootInfoDto {
            is_custom: false,
            label: "Mặc định (AppData của app)".to_string(),
            link_ok: true,
        });
    }
    let target = match lib_slot.canonicalize() {
        Ok(target) => target,
        // Nothing is readable, so there is nothing to copy back. Say so
        // instead of an opaque io failure, and leave the link alone.
        Err(_) => {
            return Err(AppError::validation(
                "Không đọc được thư viện ở nơi lưu hiện tại (ngắt ổ đĩa hay đổi tên thư mục?) — \
                 không có gì để copy về. Hãy bấm \"Chọn thư mục lưu…\" và trỏ tới thư mục đang chứa \
                 `library` trước.",
            ))
        }
    };
    let stamp = now_iso().replace([':', '.', '-'], "");
    let staged_back = lib_slot.with_file_name(format!("library.in-{stamp}"));
    std::fs::create_dir_all(&staged_back)?;
    if let Err(error) = copy_tree_verified(&target, &staged_back) {
        let _ = std::fs::remove_dir_all(&staged_back);
        return Err(error);
    }
    remove_junction(&lib_slot)?;
    if let Err(error) = std::fs::rename(&staged_back, &lib_slot) {
        // Re-link the old junction so nothing is lost mid-flight.
        let _ = create_junction(&target, &lib_slot);
        let _ = std::fs::remove_dir_all(&staged_back);
        return Err(AppError::Io(error));
    }
    forget_root(pool).await?;
    // Custom folder is intentionally left in place (user's disk choice);
    // removing foreign trees automatically is not our business.
    Ok(RootInfoDto {
        is_custom: false,
        label: "Mặc định (AppData của app)".to_string(),
        link_ok: true,
    })
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn write_file(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn validate_rejects_relative_drive_root_appdata_nesting_and_ancestors() {
        let base = tempfile::tempdir().unwrap();
        let app_data = base.path().join("appdata");
        std::fs::create_dir_all(&app_data).unwrap();
        assert!(validate_candidate(&app_data, Path::new("relative/path")).is_err());
        assert!(validate_candidate(&app_data, Path::new("C:\\")).is_err());
        assert!(validate_candidate(&app_data, &app_data).is_err());
        assert!(validate_candidate(&app_data, &app_data.join("library")).is_err());
        // Ancestor of app_data would swallow the app data itself.
        assert!(validate_candidate(&app_data, base.path()).is_err());
        let ok = base.path().join("manga-store");
        assert!(validate_candidate(&app_data, &ok).is_ok());
    }

    #[test]
    fn copy_tree_skips_identical_copies_new_and_aborts_on_conflict() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        write_file(&src.path().join("manga/tap-1.cbz"), "abc");
        write_file(&src.path().join("manga/tap-2.cbz"), "def");
        write_file(&dst.path().join("manga/tap-1.cbz"), "abc");
        let report = copy_tree_verified(src.path(), dst.path()).unwrap();
        assert_eq!(report.copied, 1);
        assert_eq!(report.skipped_identical, 1);
        assert_eq!(
            std::fs::read(dst.path().join("manga/tap-2.cbz")).unwrap(),
            b"def"
        );

        write_file(&src.path().join("manga/tap-3.cbz"), "new");
        write_file(&dst.path().join("manga/tap-3.cbz"), "different");
        let error = copy_tree_verified(src.path(), dst.path()).unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
        assert_eq!(
            std::fs::read(dst.path().join("manga/tap-3.cbz")).unwrap(),
            b"different",
            "conflicting destination files are never overwritten"
        );
    }

    #[tokio::test]
    async fn relocate_moves_library_behind_junction_then_resets() {
        let (_dbdir, pool) = crate::db::test_pool().await;
        let base = tempfile::tempdir().unwrap();
        let app_data = base.path().join("appdata");
        let lib = storage::library_root(&app_data);
        write_file(&lib.join("ninja-loan-thi/tap-1.cbz"), "cbz-bytes");
        let chosen = base.path().join("D-manga");

        let info = relocate(&pool, &app_data, &chosen).await.unwrap();
        assert!(info.is_custom);
        // Real bytes now live under the chosen folder...
        assert_eq!(
            std::fs::read(chosen.join("library/ninja-loan-thi/tap-1.cbz")).unwrap(),
            b"cbz-bytes"
        );
        // ...while every existing consumer still sees appdata/library.
        assert!(is_reparse_dir(&lib));
        assert_eq!(
            std::fs::read(storage::library_root(&app_data).join("ninja-loan-thi/tap-1.cbz"))
                .unwrap(),
            b"cbz-bytes"
        );
        // The parking copy is gone after success.
        let leftovers: Vec<_> = std::fs::read_dir(&app_data)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("library.old-"))
            .collect();
        assert!(leftovers.is_empty());
        let stored = settings_repo::get_json(&pool, SETTINGS_KEY).await.unwrap();
        assert!(stored.is_some());

        let reset = reset_to_default(&pool, &app_data).await.unwrap();
        assert!(!reset.is_custom);
        assert!(!is_reparse_dir(&lib));
        assert_eq!(
            std::fs::read(lib.join("ninja-loan-thi/tap-1.cbz")).unwrap(),
            b"cbz-bytes"
        );
        assert!(settings_repo::get_json(&pool, SETTINGS_KEY)
            .await
            .unwrap()
            .is_none());
        // User-chosen folder is left untouched (never auto-deleted).
        assert!(chosen.join("library").exists());
    }

    #[tokio::test]
    async fn relocate_into_same_place_is_rejected() {
        let (_dbdir, pool) = crate::db::test_pool().await;
        let base = tempfile::tempdir().unwrap();
        let app_data = base.path().join("appdata2");
        std::fs::create_dir_all(storage::library_root(&app_data)).unwrap();
        let chosen = base.path().join("store");
        std::fs::create_dir_all(chosen.join("library")).unwrap();
        std::fs::write(chosen.join("library/x.cbz"), "x").unwrap();
        relocate(&pool, &app_data, &chosen).await.unwrap();
        let again = relocate(&pool, &app_data, &chosen).await.unwrap_err();
        assert!(matches!(again, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn status_flags_broken_custom_link_instead_of_empty_library() {
        let (_dbdir, pool) = crate::db::test_pool().await;
        let base = tempfile::tempdir().unwrap();
        let app_data = base.path().join("appdata3");
        std::fs::create_dir_all(storage::library_root(&app_data)).unwrap();
        let chosen = base.path().join("store3");
        relocate(&pool, &app_data, &chosen).await.unwrap();
        junction::delete(storage::library_root(&app_data)).unwrap();

        let info = current(&pool, &app_data).await.unwrap();
        assert!(info.is_custom);
        assert!(!info.link_ok);
        let status = crate::services::storage_service::status(&pool, &app_data)
            .await
            .unwrap();
        assert!(status.custom_root);
        assert!(!status.root_link_ok);
        assert!(
            !status.root_available,
            "broken link is never available-empty"
        );
    }

    /// The user dragged `library` out of the chosen folder in Explorer, so the
    /// junction dangles. Settings tells them to re-choose the folder — that
    /// repair must work without trying to read the tree that is no longer
    /// reachable, and without copying anything.
    #[tokio::test]
    async fn broken_link_relinks_the_folder_that_still_holds_the_library() {
        let (_dbdir, pool) = crate::db::test_pool().await;
        let base = tempfile::tempdir().unwrap();
        let app_data = base.path().join("appdata4");
        std::fs::create_dir_all(storage::library_root(&app_data)).unwrap();
        let old = base.path().join("store4");
        relocate(&pool, &app_data, &old).await.unwrap();
        std::fs::write(old.join("library/keep.cbz"), b"cbz-bytes").unwrap();

        let moved = base.path().join("moved4");
        std::fs::create_dir_all(&moved).unwrap();
        std::fs::rename(old.join("library"), moved.join("library")).unwrap();
        assert!(!storage::library_root(&app_data).exists());

        let info = relocate(&pool, &app_data, &moved).await.unwrap();
        assert!(info.is_custom && info.link_ok);
        assert_eq!(
            std::fs::read(storage::library_root(&app_data).join("keep.cbz")).unwrap(),
            b"cbz-bytes",
            "the existing tree is re-linked in place, not duplicated"
        );
        assert!(moved.join("library/keep.cbz").exists());
    }

    /// The same repair must not silently point the app at an empty folder
    /// while the database still claims chapters are on disk — that would read
    /// as "library lost" instead of "you picked the wrong folder".
    #[tokio::test]
    async fn broken_link_refuses_a_folder_holding_no_library() {
        let (_dbdir, pool) = crate::db::test_pool().await;
        let base = tempfile::tempdir().unwrap();
        let app_data = base.path().join("appdata5");
        std::fs::create_dir_all(storage::library_root(&app_data)).unwrap();
        let old = base.path().join("store5");
        relocate(&pool, &app_data, &old).await.unwrap();
        let (_manga, chapters) = crate::reader::testkit::seed_manga(&pool, "st5", 1).await;
        crate::reader::testkit::mark_downloaded(&pool, &chapters[0], "st5/chapter-1.cbz", "cbz")
            .await;
        std::fs::write(old.join("library/keep.cbz"), b"cbz-bytes").unwrap();
        let moved = base.path().join("moved5");
        std::fs::create_dir_all(&moved).unwrap();
        std::fs::rename(old.join("library"), moved.join("library")).unwrap();

        let wrong = base.path().join("somewhere-else");
        let error = relocate(&pool, &app_data, &wrong).await.unwrap_err();
        assert!(matches!(error, AppError::Validation(_)), "got {error:?}");
        assert!(is_reparse_dir(&storage::library_root(&app_data)));
        assert!(
            !wrong.join("library").exists(),
            "a refused repair must not leave an empty library behind"
        );

        // Picking the folder that really holds the tree still works.
        let recovered = relocate(&pool, &app_data, &moved).await.unwrap();
        assert!(recovered.link_ok);
    }

    /// "Về mặc định" cannot copy from a tree it cannot read; it must say so
    /// rather than surface an io failure.
    #[tokio::test]
    async fn reset_with_a_broken_link_reports_the_real_problem() {
        let (_dbdir, pool) = crate::db::test_pool().await;
        let base = tempfile::tempdir().unwrap();
        let app_data = base.path().join("appdata6");
        std::fs::create_dir_all(storage::library_root(&app_data)).unwrap();
        let chosen = base.path().join("store6");
        relocate(&pool, &app_data, &chosen).await.unwrap();
        std::fs::remove_dir_all(chosen.join("library")).unwrap();

        let error = reset_to_default(&pool, &app_data).await.unwrap_err();
        assert!(matches!(error, AppError::Validation(_)), "got {error:?}");
        assert!(is_reparse_dir(&storage::library_root(&app_data)));
        assert!(settings_repo::get_json(&pool, SETTINGS_KEY)
            .await
            .unwrap()
            .is_some());
    }
}
