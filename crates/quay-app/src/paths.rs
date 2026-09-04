use std::path::PathBuf;

pub const DATABASE_VARIABLE: &str = "QUAY_DB";

pub fn database() -> PathBuf {
    match std::env::var(DATABASE_VARIABLE) {
        Ok(explicit) if !explicit.trim().is_empty() => PathBuf::from(explicit),
        _ => data_directory().join("quay.db"),
    }
}

fn data_directory() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
    let home = PathBuf::from(home);
    if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("quay")
    } else {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("share"))
            .join("quay")
    }
}

#[cfg(test)]
mod tests {
    use super::data_directory;

    #[test]
    fn the_data_directory_ends_with_the_product_name() {
        assert!(data_directory().ends_with("quay"));
    }

    #[test]
    fn the_data_directory_is_absolute_when_a_home_is_known() {
        if std::env::var("HOME").is_ok() {
            assert!(data_directory().is_absolute());
        }
    }
}
