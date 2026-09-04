use std::path::Path;

use rusqlite::Connection;

pub const CORRECTED: &str = include_str!("../schema/0001_initial.sql");
pub const UNIQUE_SAVED_VIEWS: &str = include_str!("../schema/0002_unique_saved_views.sql");
pub const ORGANIZATIONS: &str = include_str!("../schema/0003_organizations.sql");
pub const TIMELINE: &str = include_str!("../schema/0004_timeline.sql");
pub const SPECIFICATION_SECTION_SIX: &str = include_str!("../schema/baseline-section-6.sql");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaVariant {
    SpecificationSectionSix,
    Corrected,
}

impl SchemaVariant {
    pub const ALL: [SchemaVariant; 2] = [
        SchemaVariant::SpecificationSectionSix,
        SchemaVariant::Corrected,
    ];

    pub fn id(self) -> &'static str {
        match self {
            SchemaVariant::SpecificationSectionSix => "section6",
            SchemaVariant::Corrected => "corrected",
        }
    }

    pub fn ddl(self) -> &'static str {
        match self {
            SchemaVariant::SpecificationSectionSix => SPECIFICATION_SECTION_SIX,
            SchemaVariant::Corrected => CORRECTED,
        }
    }

    pub fn records_review_requests(self) -> bool {
        self == SchemaVariant::Corrected
    }

    pub fn stores_payload_outside_the_pull_request_row(self) -> bool {
        self == SchemaVariant::Corrected
    }
}

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let connection = Connection::open(path)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(connection)
}

pub fn create(path: &Path, variant: SchemaVariant) -> Result<Connection, crate::StoreError> {
    let mut connection = open(path)?;
    match variant {
        SchemaVariant::Corrected => {
            crate::migrations::apply(&mut connection, path, &crate::migrations::MIGRATIONS)?;
        }
        SchemaVariant::SpecificationSectionSix => connection.execute_batch(variant.ddl())?,
    }
    Ok(connection)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{SchemaVariant, create};

    fn names(connection: &rusqlite::Connection, kind: &str) -> Vec<String> {
        let mut statement = connection
            .prepare("SELECT name FROM sqlite_master WHERE type = ?1 ORDER BY name")
            .unwrap();
        statement
            .query_map([kind], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<String>>>()
            .unwrap()
    }

    fn database(variant: SchemaVariant) -> (tempfile::TempDir, rusqlite::Connection) {
        let directory = tempfile::tempdir().unwrap();
        let connection = create(&directory.path().join("quay.db"), variant).unwrap();
        (directory, connection)
    }

    #[test]
    fn the_baseline_schema_declares_exactly_the_tables_of_section_six() {
        let (_directory, connection) = database(SchemaVariant::SpecificationSectionSix);
        assert_eq!(
            names(&connection, "table"),
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
    fn the_corrected_schema_adds_the_review_request_and_payload_tables() {
        let (_directory, connection) = database(SchemaVariant::Corrected);
        let tables = names(&connection, "table");
        assert!(tables.contains(&"review_request".to_owned()));
        assert!(tables.contains(&"pull_request_payload".to_owned()));
    }

    #[test]
    fn the_corrected_schema_indexes_every_foreign_key_the_inbox_walks() {
        let (_directory, connection) = database(SchemaVariant::Corrected);
        let indexes = names(&connection, "index");
        for expected in [
            "idx_pr_inbox",
            "idx_thread_by_pull_request",
            "idx_comment_by_thread",
            "idx_review_request_by_reviewer",
        ] {
            assert!(indexes.contains(&expected.to_owned()), "{expected}");
        }
    }

    #[test]
    fn the_baseline_schema_indexes_neither_review_thread_nor_review_comment() {
        let (_directory, connection) = database(SchemaVariant::SpecificationSectionSix);
        let indexes = names(&connection, "index");
        assert!(!indexes.contains(&"idx_thread_by_pull_request".to_owned()));
        assert!(!indexes.contains(&"idx_comment_by_thread".to_owned()));
    }

    #[test]
    fn the_pull_request_row_carries_the_raw_payload_only_in_the_baseline_schema() {
        let has_raw_column = |variant| {
            let (_directory, connection) = database(variant);
            connection
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('pull_request') WHERE name = 'raw'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap()
        };
        assert_eq!(has_raw_column(SchemaVariant::SpecificationSectionSix), 1);
        assert_eq!(has_raw_column(SchemaVariant::Corrected), 0);
    }

    #[test]
    fn neither_schema_file_carries_a_comment() {
        for variant in SchemaVariant::ALL {
            assert!(!variant.ddl().contains("--"), "{}", variant.id());
            assert!(!variant.ddl().contains("/*"), "{}", variant.id());
        }
    }

    #[test]
    fn applying_a_schema_twice_fails_rather_than_silently_diverging() {
        for variant in SchemaVariant::ALL {
            let (_directory, connection) = database(variant);
            assert!(connection.execute_batch(variant.ddl()).is_err());
        }
    }

    #[test]
    fn the_corrected_schema_is_created_through_the_migration_runner_and_stamps_its_version() {
        let (_directory, connection) = database(SchemaVariant::Corrected);
        assert_eq!(
            crate::migrations::current_version(&connection).unwrap(),
            crate::migrations::latest_version(&crate::migrations::MIGRATIONS)
        );
    }

    #[test]
    fn the_witness_schema_stays_unversioned_because_no_product_code_opens_it() {
        let (_directory, connection) = database(SchemaVariant::SpecificationSectionSix);
        assert_eq!(crate::migrations::current_version(&connection).unwrap(), 0);
    }

    #[test]
    fn opening_a_database_enables_write_ahead_logging() {
        let (_directory, connection) = database(SchemaVariant::Corrected);
        let mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }
}
