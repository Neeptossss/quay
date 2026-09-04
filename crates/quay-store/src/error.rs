#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite refused the statement: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error(
        "the database is at schema version {found}, newer than the {supported} this build knows"
    )]
    DatabaseFromTheFuture { found: i64, supported: i64 },

    #[error("migration {version} ({name}) failed: {message}")]
    MigrationFailed {
        version: i64,
        name: &'static str,
        message: String,
    },

    #[error("the database could not be backed up before migration {version}: {message}")]
    BackupFailed { version: i64, message: String },

    #[error("`{qualifier}` is not answerable yet: {because}")]
    QualifierNotSupportedYet {
        qualifier: &'static str,
        because: &'static str,
    },

    #[error("`{qualifier}` was given {value:?}, expected {expected}")]
    QualifierValueNotUnderstood {
        qualifier: &'static str,
        value: String,
        expected: &'static str,
    },

    #[error("no row in {table} carries the node id {node_id}")]
    UnknownTarget {
        table: &'static str,
        node_id: String,
    },

    #[error("the stored value {value:?} in column {column} is not one this build understands")]
    UnreadableValue { column: &'static str, value: String },
}
