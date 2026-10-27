//! Test-only artifact builders shared by reader/library service tests:
//! real CBZ bytes produced through the actual Phase 06 exporter, and image
//! folders with deterministic PNG-magic files.

use std::path::Path;

use crate::db::repositories::download_repo;
use crate::db::repositories::manga_repo::{ensure_source, upsert_chapter, upsert_manga};
use crate::downloader::exporter::{self, ExportPage};
use crate::downloader::image_validate::crc32_hex;
use sqlx::SqlitePool;

/// Valid-enough PNG: magic + IHDR carrying the given dimensions + filler.
pub fn png_bytes(seed: u8, width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    bytes.extend([0u8, 0, 0, 13, b'I', b'H', b'D', b'R']);
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes.extend(std::iter::repeat_n(seed, 48));
    bytes
}

fn page_sets(count: usize) -> (Vec<String>, Vec<String>, Vec<Vec<u8>>) {
    let names: Vec<String> = (0..count).map(|index| format!("{index:06}.png")).collect();
    let blobs: Vec<Vec<u8>> = names
        .iter()
        .enumerate()
        // (index % 250) + 1: a 300-page fixture must not overflow the u8 seed.
        .map(|(index, _)| png_bytes((index % 250) as u8 + 1, 40, 60))
        .collect();
    let checksums: Vec<String> = blobs.iter().map(|b| crc32_hex(b)).collect();
    (names, checksums, blobs)
}

fn write_staging(staging: &Path, names: &[String], blobs: &[Vec<u8>]) {
    std::fs::create_dir_all(staging).unwrap();
    for (name, blob) in names.iter().zip(blobs.iter()) {
        std::fs::write(staging.join(name), blob).unwrap();
    }
}

/// Build a real library CBZ under `library_root/relpath` using the exporter.
pub fn build_cbz(staging: &Path, library_root: &Path, relpath: &str, count: usize) {
    let (names, checksums, blobs) = page_sets(count);
    write_staging(staging, &names, &blobs);
    let exports: Vec<ExportPage> = names
        .iter()
        .enumerate()
        .map(|(index, name)| ExportPage {
            index,
            staged_name: name.as_str(),
            size_bytes: blobs[index].len() as i64,
            checksum_hex: checksums[index].as_str(),
        })
        .collect();
    let final_path = library_root.join(relpath);
    std::fs::create_dir_all(final_path.parent().unwrap()).unwrap();
    let (part_path, _) = exporter::part_paths(&final_path);
    exporter::write_cbz(staging, &exports, &part_path).unwrap();
    std::fs::rename(&part_path, &final_path).unwrap();
}

/// Build a library image folder under `library_root/relpath`.
pub fn build_folder(staging: &Path, library_root: &Path, relpath: &str, count: usize) {
    let (names, _, blobs) = page_sets(count);
    write_staging(staging, &names, &blobs);
    let target = library_root.join(relpath);
    std::fs::create_dir_all(&target).unwrap();
    for name in &names {
        std::fs::copy(staging.join(name), target.join(name)).unwrap();
    }
}

/// Seed a manga (source `src-a`) with `chapters` rows; returns manga_id and
/// chapter ids in insertion order.
pub async fn seed_manga(pool: &SqlitePool, slug: &str, chapters: usize) -> (String, Vec<String>) {
    ensure_source(pool, "src-a", "Source A").await.unwrap();
    let (manga_id, _) = upsert_manga(pool, "src-a", slug, "Title", None, None, "ongoing")
        .await
        .unwrap();
    let mut ids = Vec::new();
    for index in 1..=chapters {
        let (chapter_id, _) = upsert_chapter(
            pool,
            &manga_id,
            &format!("{slug}-c{index}"),
            &format!("Chapter {index}"),
            Some(index as f64),
            None,
        )
        .await
        .unwrap();
        ids.push(chapter_id);
    }
    (manga_id, ids)
}

/// Mark a chapter as having a verified local artifact (publish-equivalent row).
pub async fn mark_downloaded(pool: &SqlitePool, chapter_id: &str, relpath: &str, format: &str) {
    download_repo::attach_artifact_for_test(pool, chapter_id, relpath, format).await;
}
