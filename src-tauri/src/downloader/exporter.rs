//! Verified, transactional export of a completed chapter into a CBZ archive
//! (ZIP, Stored method — images are already compressed; no lossy re-encode)
//! or an image folder.
//!
//! Rules enforced here (design §5):
//! - Entries are basename-only zero-padded names in manifest order (no ZIP
//!   slip, no remote names).
//! - The archive is built as `*.cbz.part` inside the final directory (same
//!   volume), then re-opened and fully verified (count, order, per-entry
//!   size + CRC32) before an atomic rename publishes it.
//! - Publishing never overwrites: an existing valid artifact for the same
//!   task is adopted (crash recovery), any other collision fails safely.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::downloader::image_validate;
use crate::error::AppError;

/// One export input: a validated staged page.
pub struct ExportPage<'a> {
    pub index: usize,
    /// Final staged file name (`000007.png`), basename only.
    pub staged_name: &'a str,
    pub size_bytes: i64,
    pub checksum_hex: &'a str,
}

fn le16(value: u16) -> [u8; 2] {
    value.to_le_bytes()
}

fn le32(value: u32) -> [u8; 4] {
    value.to_le_bytes()
}

/// Fixed DOS timestamp 1980-01-01 00:00:00 — deterministic archives.
const DOS_TIME: u16 = 0;
const DOS_DATE: u16 = 0x21;

/// Build a Stored-method ZIP from ordered staged pages into `part_path`.
/// Entries are sorted by manifest index so export order is deterministic
/// regardless of the caller's iteration order (e.g. post-recovery page list).
pub fn write_cbz(
    staging_dir: &Path,
    pages: &[ExportPage<'_>],
    part_path: &Path,
) -> Result<(), AppError> {
    let mut ordered: Vec<&ExportPage> = pages.iter().collect();
    ordered.sort_by_key(|page| page.index);
    let mut file = std::fs::File::create(part_path)?;
    let mut central: Vec<u8> = Vec::new();
    let mut offset: u32 = 0;

    for page in ordered {
        let bytes = std::fs::read(staging_dir.join(page.staged_name))?;
        let crc = image_validate::crc32(&bytes);
        let name = page.staged_name.as_bytes();

        let mut local = Vec::with_capacity(30 + name.len());
        local.extend_from_slice(&le32(0x0403_4b50));
        local.extend_from_slice(&le16(20)); // version needed
        local.extend_from_slice(&le16(0x0800)); // UTF-8 names
        local.extend_from_slice(&le16(0)); // method: stored
        local.extend_from_slice(&le16(DOS_TIME));
        local.extend_from_slice(&le16(DOS_DATE));
        local.extend_from_slice(&le32(crc));
        local.extend_from_slice(&le32(bytes.len() as u32));
        local.extend_from_slice(&le32(bytes.len() as u32));
        local.extend_from_slice(&le16(name.len() as u16));
        local.extend_from_slice(&le16(0)); // no extra field
        local.extend_from_slice(name);
        file.write_all(&local)?;
        file.write_all(&bytes)?;

        let mut entry = Vec::with_capacity(46 + name.len());
        entry.extend_from_slice(&le32(0x0201_4b50));
        entry.extend_from_slice(&le16(20)); // version made by
        entry.extend_from_slice(&le16(20)); // version needed
        entry.extend_from_slice(&le16(0x0800));
        entry.extend_from_slice(&le16(0));
        entry.extend_from_slice(&le16(DOS_TIME));
        entry.extend_from_slice(&le16(DOS_DATE));
        entry.extend_from_slice(&le32(crc));
        entry.extend_from_slice(&le32(bytes.len() as u32));
        entry.extend_from_slice(&le32(bytes.len() as u32));
        entry.extend_from_slice(&le16(name.len() as u16));
        entry.extend_from_slice(&le16(0)); // extra
        entry.extend_from_slice(&le16(0)); // comment
        entry.extend_from_slice(&le16(0)); // disk
        entry.extend_from_slice(&le16(0)); // internal attrs
        entry.extend_from_slice(&le32(0)); // external attrs
        entry.extend_from_slice(&le32(offset));
        entry.extend_from_slice(name);
        central.extend_from_slice(&entry);

        offset += (local.len() + bytes.len()) as u32;
    }

    let central_offset = offset;
    let central_len = central.len() as u32;
    file.write_all(&central)?;
    let mut eocd = Vec::with_capacity(22);
    eocd.extend_from_slice(&le32(0x0605_4b50));
    eocd.extend_from_slice(&le16(0));
    eocd.extend_from_slice(&le16(0));
    eocd.extend_from_slice(&le16(pages.len() as u16));
    eocd.extend_from_slice(&le16(pages.len() as u16));
    eocd.extend_from_slice(&le32(central_len));
    eocd.extend_from_slice(&le32(central_offset));
    eocd.extend_from_slice(&le16(0));
    file.write_all(&eocd)?;
    file.flush()?;
    Ok(())
}

/// Re-open an archive and verify it contains exactly `pages` in order with
/// matching sizes and CRC32 values. This is the only evidence allowed to
/// mark a chapter completed.
pub fn verify_cbz(archive_path: &Path, pages: &[ExportPage<'_>]) -> Result<(), AppError> {
    let data = std::fs::read(archive_path)
        .map_err(|error| AppError::io_stage("Không đọc được file chương để kiểm tra", error))?;
    let bad = || AppError::validation("Archive không vượt qua kiểm tra toàn vẹn.");
    if data.len() < 22 {
        return Err(bad());
    }
    let eocd_pos = find_eocd(&data).ok_or_else(bad)?;
    let entry_count = u16::from_le_bytes([data[eocd_pos + 10], data[eocd_pos + 11]]) as usize;
    let central_len = u32::from_le_bytes([
        data[eocd_pos + 12],
        data[eocd_pos + 13],
        data[eocd_pos + 14],
        data[eocd_pos + 15],
    ]) as usize;
    let central_offset = u32::from_le_bytes([
        data[eocd_pos + 16],
        data[eocd_pos + 17],
        data[eocd_pos + 18],
        data[eocd_pos + 19],
    ]) as usize;
    if entry_count != pages.len() || central_offset + central_len != eocd_pos {
        return Err(bad());
    }

    let mut cursor = central_offset;
    let mut ordered: Vec<&ExportPage> = pages.iter().collect();
    ordered.sort_by_key(|page| page.index);
    for page in ordered {
        if cursor + 46 > data.len() {
            return Err(bad());
        }
        if &data[cursor..cursor + 4] != b"PK\x01\x02" {
            return Err(bad());
        }
        let crc_expected = u32::from_le_bytes([
            data[cursor + 16],
            data[cursor + 17],
            data[cursor + 18],
            data[cursor + 19],
        ]);
        let size = u32::from_le_bytes([
            data[cursor + 20],
            data[cursor + 21],
            data[cursor + 22],
            data[cursor + 23],
        ]) as usize;
        let name_len = u16::from_le_bytes([data[cursor + 28], data[cursor + 29]]) as usize;
        let extra_len = u16::from_le_bytes([data[cursor + 30], data[cursor + 31]]) as usize;
        let comment_len = u16::from_le_bytes([data[cursor + 32], data[cursor + 33]]) as usize;
        let local_offset = u32::from_le_bytes([
            data[cursor + 42],
            data[cursor + 43],
            data[cursor + 44],
            data[cursor + 45],
        ]) as usize;
        let name_end = cursor + 46 + name_len;
        if name_end > data.len() {
            return Err(bad());
        }
        let name = std::str::from_utf8(&data[cursor + 46..name_end]).map_err(|_| bad())?;
        cursor = name_end + extra_len + comment_len;

        if name != page.staged_name || size as i64 != page.size_bytes {
            return Err(bad());
        }
        // Parse the local header and extract payload bytes. Every offset read
        // from the central directory is attacker-controlled when we VERIFY A
        // PRE-EXISTING file (adoption path), so bound-check before slicing.
        if local_offset + 30 > data.len() {
            return Err(bad());
        }
        if &data[local_offset..local_offset + 4] != b"PK\x03\x04" {
            return Err(bad());
        }
        let local_name_len =
            u16::from_le_bytes([data[local_offset + 26], data[local_offset + 27]]) as usize;
        let local_extra_len =
            u16::from_le_bytes([data[local_offset + 28], data[local_offset + 29]]) as usize;
        let payload_start = local_offset + 30 + local_name_len + local_extra_len;
        if payload_start + size > data.len() {
            return Err(bad());
        }
        let payload = &data[payload_start..payload_start + size];
        if image_validate::crc32(payload) != crc_expected
            || !crc_hex_matches(crc_expected, page.checksum_hex)
        {
            return Err(bad());
        }
    }
    if cursor != eocd_pos {
        return Err(bad());
    }
    Ok(())
}

fn crc_hex_matches(crc: u32, hex: &str) -> bool {
    format!("{crc:08x}") == hex
}

fn find_eocd(data: &[u8]) -> Option<usize> {
    data.windows(4)
        .rposition(|window| window == b"PK\x05\x06")
        .filter(|&candidate| candidate + 22 == data.len())
}

/// Publish a verified `.cbz.part` in `final_dir` to `final_name` atomically.
/// Same task re-run (verified artifact already published) is idempotent;
/// any other existing file is never overwritten.
pub fn publish_artifact(
    part_path: &Path,
    final_path: &Path,
    pages: &[ExportPage<'_>],
) -> Result<(), AppError> {
    if final_path.exists() {
        // Adopt only after full verification — an unverified pre-existing
        // file (possibly user content) is never silently replaced.
        verify_cbz(final_path, pages).map_err(|_| name_conflict_error(final_path))?;
        let _ = std::fs::remove_file(part_path);
        return Ok(());
    }
    std::fs::rename(part_path, final_path)
        .map_err(|error| AppError::io_stage("Không công bố được file chương", error))?;
    Ok(())
}

/// The destination holds a DIFFERENT verified-shaped archive. In practice the
/// only way a task lands here is a stale chapter number in the catalog (e.g.
/// saved by an old parser) resolving onto another chapter's file — say so,
/// and point at the action that heals the row.
fn name_conflict_error(final_path: &Path) -> AppError {
    let name = final_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "file tải".to_string());
    AppError::Validation(format!(
        "Xung đột tên file: \"{name}\" đã thuộc chương khác (số chương trong danh mục có thể cũ). \
         Mở truyện bấm \"Kiểm tra chương mới\" hoặc dán lại link chương rồi tải."
    ))
}

/// Image-folder export: copy ordered, validated pages into `<name>.part-dir/`
/// beside the final location, verify every file byte-for-byte, then rename
/// the directory into place. A partial folder is never observable.
/// Export pages as a directory tree beside `final_dir`, then rename
/// atomically. Returns `true` when THIS run created `final_dir` (a cancel may
/// then safely undo the rename), `false` when a pre-existing verified
/// directory was adopted (the content predates this run and stays).
pub fn export_folder(
    staging_dir: &Path,
    pages: &[ExportPage<'_>],
    part_dir: &Path,
    final_dir: &Path,
) -> Result<bool, AppError> {
    if final_dir.exists() {
        // Idempotent adoption only when content matches exactly.
        verify_folder(final_dir, pages, staging_dir).map_err(|_| name_conflict_error(final_dir))?;
        let _ = std::fs::remove_dir_all(part_dir);
        return Ok(false);
    }
    let _ = std::fs::remove_dir_all(part_dir);
    std::fs::create_dir_all(part_dir)
        .map_err(|error| AppError::io_stage("Không tạo được thư mục chương", error))?;
    let mut ordered: Vec<&ExportPage> = pages.iter().collect();
    ordered.sort_by_key(|page| page.index);
    for page in ordered {
        let source = staging_dir.join(page.staged_name);
        let bytes = std::fs::read(&source)
            .map_err(|error| AppError::io_stage("Không đọc được trang đã tải", error))?;
        if bytes.len() as i64 != page.size_bytes {
            return Err(AppError::validation(
                "Thư mục ảnh không vượt qua kiểm tra toàn vẹn.",
            ));
        }
        std::fs::write(part_dir.join(page.staged_name), &bytes).map_err(|error| {
            AppError::io_stage("Không ghi được trang vào thư mục chương", error)
        })?;
    }
    verify_folder(part_dir, pages, part_dir)?;
    std::fs::rename(part_dir, final_dir)
        .map_err(|error| AppError::io_stage("Không công bố được thư mục chương", error))?;
    Ok(true)
}

fn verify_folder(
    dir: &Path,
    pages: &[ExportPage<'_>],
    expected_source: &Path,
) -> Result<(), AppError> {
    let bad = || AppError::validation("Thư mục ảnh không vượt qua kiểm tra toàn vẹn.");
    let entries = std::fs::read_dir(dir)?.count();
    if entries != pages.len() {
        return Err(bad());
    }
    for page in pages {
        let left = std::fs::read(dir.join(page.staged_name)).map_err(|_| bad())?;
        let right = std::fs::read(expected_source.join(page.staged_name)).map_err(|_| bad())?;
        if left != right || left.len() as i64 != page.size_bytes {
            return Err(bad());
        }
    }
    Ok(())
}

/// List helper to convert export inputs for callers holding page rows.
pub fn part_paths(final_path: &Path) -> (PathBuf, PathBuf) {
    let part = {
        let mut name = final_path.file_name().unwrap_or_default().to_os_string();
        name.push(".part");
        final_path.with_file_name(name)
    };
    (part, final_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloader::image_validate::crc32_hex;

    fn png(seed: u8, size: usize) -> Vec<u8> {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x1a, 0x0a, 0x00];
        bytes.extend(std::iter::repeat_n(seed, size));
        bytes
    }

    #[test]
    fn t06_17_cbz_roundtrip_ordered_verified() {
        let staging = tempfile::tempdir().unwrap();
        let out_dir = tempfile::tempdir().unwrap();
        let pages_bytes = [png(1, 40), png(2, 60), png(3, 50)];
        let names = ["000000.png", "000001.jpg", "000002.gif"];
        for (name, bytes) in names.iter().zip(pages_bytes.iter()) {
            std::fs::write(staging.path().join(name), bytes).unwrap();
        }
        let checksums: Vec<String> = pages_bytes.iter().map(|b| crc32_hex(b)).collect();
        let pages: Vec<ExportPage> = pages_bytes
            .iter()
            .enumerate()
            .map(|(index, bytes)| ExportPage {
                index,
                staged_name: names[index],
                size_bytes: bytes.len() as i64,
                checksum_hex: checksums[index].as_str(),
            })
            .collect();

        let part = out_dir.path().join("chapter.cbz.part");
        let final_path = out_dir.path().join("chapter.cbz");
        write_cbz(staging.path(), &pages, &part).unwrap();
        verify_cbz(&part, &pages).unwrap();
        publish_artifact(&part, &final_path, &pages).unwrap();
        assert!(final_path.exists() && !part.exists());
        verify_cbz(&final_path, &pages).unwrap();

        // Publish again is idempotent (same verified content), never wipes.
        publish_artifact(&part, &final_path, &pages).unwrap();
        assert!(final_path.exists());

        // A foreign file at the destination is never adopted or replaced.
        let other_dir = tempfile::tempdir().unwrap();
        std::fs::write(other_dir.path().join("chapter.cbz"), b"someone else's file").unwrap();
        let error =
            publish_artifact(&part, &other_dir.path().join("chapter.cbz"), &pages).unwrap_err();
        assert!(
            matches!(&error, AppError::Validation(m) if m.contains("chapter.cbz")
                && m.contains("Kiểm tra chương mới")),
            "the message must name the colliding file and the healing action: {error:?}"
        );
        assert_eq!(
            std::fs::read(other_dir.path().join("chapter.cbz")).unwrap(),
            b"someone else's file"
        );
    }

    #[test]
    fn tampered_archive_fails_verification() {
        let staging = tempfile::tempdir().unwrap();
        let out_dir = tempfile::tempdir().unwrap();
        let bytes = png(9, 45);
        std::fs::write(staging.path().join("000000.png"), &bytes).unwrap();
        let checksum = crc32_hex(&bytes);
        let pages = [ExportPage {
            index: 0,
            staged_name: "000000.png",
            size_bytes: bytes.len() as i64,
            checksum_hex: checksum.as_str(),
        }];
        let part = out_dir.path().join("a.cbz.part");
        write_cbz(staging.path(), &pages, &part).unwrap();
        verify_cbz(&part, &pages).unwrap();

        let mut corrupted = std::fs::read(&part).unwrap();
        // Flip a byte inside the single entry's payload: last 22 (EOCD) +
        // 56 (central dir record incl. name) bytes belong to metadata.
        let payload_pos = corrupted.len() - 22 - 56 - 1;
        corrupted[payload_pos] ^= 0xff;
        std::fs::write(&part, &corrupted).unwrap();
        assert!(verify_cbz(&part, &pages).is_err());
    }

    #[test]
    fn t06_18_folder_only_publishes_complete() {
        let staging = tempfile::tempdir().unwrap();
        let out_dir = tempfile::tempdir().unwrap();
        let a = png(1, 30);
        let b = png(2, 30);
        std::fs::write(staging.path().join("000000.png"), &a).unwrap();
        std::fs::write(staging.path().join("000001.png"), &b).unwrap();
        let checksums = [crc32_hex(&a), crc32_hex(&b)];
        let pages = [
            ExportPage {
                index: 0,
                staged_name: "000000.png",
                size_bytes: a.len() as i64,
                checksum_hex: checksums[0].as_str(),
            },
            ExportPage {
                index: 1,
                staged_name: "000001.png",
                size_bytes: b.len() as i64,
                checksum_hex: checksums[1].as_str(),
            },
        ];
        let part_dir = out_dir.path().join("ch.part-dir");
        let final_dir = out_dir.path().join("ch");
        assert!(
            export_folder(staging.path(), &pages, &part_dir, &final_dir).unwrap(),
            "first publish is created by this run"
        );
        assert!(final_dir.join("000000.png").exists());
        assert!(final_dir.join("000001.png").exists());
        assert!(!part_dir.exists());
        // Idempotent re-run adopts the verified folder — and reports it as
        // NOT newly created (a cancel must never delete an adopted artifact).
        assert!(
            !export_folder(staging.path(), &pages, &part_dir, &final_dir).unwrap(),
            "adoption returns false"
        );
    }

    #[test]
    fn t10_verify_cbz_adversarial_offsets_error_instead_of_panicking() {
        // The adoption path runs verify_cbz on a PRE-EXISTING (possibly
        // hostile) file. Untrusted central-directory offsets must produce a
        // validation error, never a slice-out-of-bounds panic in the worker.
        let staging = tempfile::tempdir().unwrap();
        let out_dir = tempfile::tempdir().unwrap();
        let bytes = png(7, 40);
        std::fs::write(staging.path().join("000000.png"), &bytes).unwrap();
        let checksum = crc32_hex(&bytes);
        let pages = [ExportPage {
            index: 0,
            staged_name: "000000.png",
            size_bytes: bytes.len() as i64,
            checksum_hex: checksum.as_str(),
        }];
        let part = out_dir.path().join("a.cbz.part");
        write_cbz(staging.path(), &pages, &part).unwrap();
        let valid = std::fs::read(&part).unwrap();
        let central = valid
            .windows(4)
            .rposition(|w| w == b"PK\x01\x02")
            .expect("central header");

        // 1) name_len pointing past end of file.
        let mut tampered = valid.clone();
        tampered[central + 28..central + 30].copy_from_slice(&10_000u16.to_le_bytes());
        let hostile = out_dir.path().join("hostile-name.cbz");
        std::fs::write(&hostile, &tampered).unwrap();
        assert!(verify_cbz(&hostile, &pages).is_err());

        // 2) local header offset pointing past end of file.
        let mut tampered = valid.clone();
        let beyond = (valid.len() as u32) + 100;
        tampered[central + 42..central + 46].copy_from_slice(&beyond.to_le_bytes());
        let hostile = out_dir.path().join("hostile-offset.cbz");
        std::fs::write(&hostile, &tampered).unwrap();
        assert!(verify_cbz(&hostile, &pages).is_err());

        // 3) extra_len pushing the next record past end of file.
        let mut tampered = valid.clone();
        tampered[central + 30..central + 32].copy_from_slice(&60_000u16.to_le_bytes());
        let hostile = out_dir.path().join("hostile-extra.cbz");
        std::fs::write(&hostile, &tampered).unwrap();
        assert!(verify_cbz(&hostile, &pages).is_err());
    }
}
