use std::path::Path;

use rusqlite::Connection;

pub const INITIAL: &str = include_str!("../schema/0001_initial.sql");

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let connection = Connection::open(path)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(connection)
}

pub fn create(path: &Path) -> rusqlite::Result<Connection> {
    let connection = open(path)?;
    connection.execute_batch(INITIAL)?;
    Ok(connection)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{INITIAL, create};

    fn table_names(connection: &rusqlite::Connection) -> Vec<String> {
        let mut statement = connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<String>>>()
            .unwrap()
    }

    #[test]
    fn the_schema_declares_every_table_of_the_specification() {
        let directory = tempfile::tempdir().unwrap();
        let connection = create(&directory.path().join("quay.db")).unwrap();

        assert_eq!(
            table_names(&connection),
            vec![
                "account",
                "mutation",
                "navigation_event",
                "preload_outcome",
                "pull_request",
                "repo",
                "resource_cache",
                "review_comment",
                "review_thread",
                "saved_view",
            ]
        );
    }

    #[test]
    fn the_schema_file_carries_no_comment() {
        assert!(!INITIAL.contains("--"));
        assert!(!INITIAL.contains("/*"));
    }

    #[test]
    fn applying_the_schema_twice_fails_rather_than_silently_diverging() {
        let directory = tempfile::tempdir().unwrap();
        let connection = create(&directory.path().join("quay.db")).unwrap();
        assert!(connection.execute_batch(INITIAL).is_err());
    }

    #[test]
    fn opening_a_database_enables_write_ahead_logging() {
        let directory = tempfile::tempdir().unwrap();
        let connection = create(&directory.path().join("quay.db")).unwrap();
        let mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }
}
