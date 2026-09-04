use std::path::{Path, PathBuf};

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf()
}

pub fn measurements() -> PathBuf {
    workspace_root().join("measurements")
}

pub fn raw() -> PathBuf {
    measurements().join("raw")
}

pub fn summaries() -> PathBuf {
    measurements().join("summary")
}

pub fn report() -> PathBuf {
    measurements().join("REPORT.md")
}
