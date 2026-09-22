use std::path::Path;

/// Prepares app-managed sub-directories used by later phases (cover cache in
/// Phase 05+, stored pages in Phase 06). Creating directories is not a
/// permission to read/write arbitrary paths: every future file operation must
/// still resolve through this root and be validated by the owning service.
pub fn prepare(app_data_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(app_data_dir.join("covers"))?;
    std::fs::create_dir_all(app_data_dir.join("cache"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn creates_subdirectories_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        super::prepare(dir.path()).unwrap();
        super::prepare(dir.path()).unwrap();
        assert!(dir.path().join("covers").is_dir());
        assert!(dir.path().join("cache").is_dir());
    }
}
