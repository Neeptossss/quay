use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::StoreError;
use crate::schema;

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
    pub rewrites_existing_rows: bool,
}

pub const MIGRATIONS: [Migration; 4] = [
    Migration {
        version: 1,
        name: "initial",
        sql: schema::CORRECTED,
        rewrites_existing_rows: false,
    },
    Migration {
        version: 2,
        name: "unique_saved_views",
        sql: schema::UNIQUE_SAVED_VIEWS,
        rewrites_existing_rows: false,
    },
    Migration {
        version: 3,
        name: "organizations",
        sql: schema::ORGANIZATIONS,
        rewrites_existing_rows: false,
    },
    Migration {
        version: 4,
        name: "timeline",
        sql: schema::TIMELINE,
        rewrites_existing_rows: false,
    },
];

impl Migration {
    pub fn borrowed(&self) -> Migration {
        Migration {
            version: self.version,
            name: self.name,
            sql: self.sql,
            rewrites_existing_rows: self.rewrites_existing_rows,
        }
    }
}

pub fn latest_version(migrations: &[Migration]) -> i64 {
    migrations
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or(0)
}

pub fn current_version(connection: &Connection) -> Result<i64, StoreError> {
    Ok(connection.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

pub fn apply(
    connection: &mut Connection,
    database: &Path,
    migrations: &[Migration],
) -> Result<i64, StoreError> {
    let supported = latest_version(migrations);
    let found = current_version(connection)?;
    if found > supported {
        return Err(StoreError::DatabaseFromTheFuture { found, supported });
    }

    let mut pending: Vec<&Migration> = migrations
        .iter()
        .filter(|migration| migration.version > found)
        .collect();
    pending.sort_by_key(|migration| migration.version);

    for migration in pending {
        if migration.rewrites_existing_rows && found > 0 {
            back_up(connection, database, migration.version)?;
        }
        run(connection, migration)?;
    }
    current_version(connection)
}

fn run(connection: &mut Connection, migration: &Migration) -> Result<(), StoreError> {
    let transaction = connection.transaction()?;
    transaction
        .execute_batch(migration.sql)
        .map_err(|error| StoreError::MigrationFailed {
            version: migration.version,
            name: migration.name,
            message: error.to_string(),
        })?;
    transaction.pragma_update(None, "user_version", migration.version)?;
    transaction.commit()?;
    Ok(())
}

fn back_up(connection: &Connection, database: &Path, version: i64) -> Result<PathBuf, StoreError> {
    let destination = database.with_extension(format!("before-v{version}.backup"));
    if destination.exists() {
        std::fs::remove_file(&destination).map_err(|error| StoreError::BackupFailed {
            version,
            message: error.to_string(),
        })?;
    }
    let target = destination
        .to_str()
        .ok_or_else(|| StoreError::BackupFailed {
            version,
            message: "the backup path is not valid unicode".to_owned(),
        })?;
    connection
        .execute("VACUUM INTO ?1", (target,))
        .map_err(|error| StoreError::BackupFailed {
            version,
            message: error.to_string(),
        })?;
    Ok(destination)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{MIGRATIONS, Migration, apply, current_version, latest_version};
    use crate::error::StoreError;
    use crate::schema;

    fn fresh() -> (tempfile::TempDir, std::path::PathBuf, rusqlite::Connection) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("quay.db");
        let connection = schema::open(&path).unwrap();
        (directory, path, connection)
    }

    #[test]
    fn a_new_database_is_brought_to_the_latest_declared_version() {
        let (_directory, path, mut connection) = fresh();
        let version = apply(&mut connection, &path, &MIGRATIONS).unwrap();
        assert_eq!(version, latest_version(&MIGRATIONS));
        assert_eq!(
            current_version(&connection).unwrap(),
            latest_version(&MIGRATIONS)
        );
    }

    #[test]
    fn a_database_stopped_at_the_first_version_is_carried_to_the_latest() {
        let (_directory, path, mut connection) = fresh();
        apply(&mut connection, &path, &MIGRATIONS[..1]).unwrap();
        assert_eq!(current_version(&connection).unwrap(), 1);
        apply(&mut connection, &path, &MIGRATIONS).unwrap();
        assert_eq!(
            current_version(&connection).unwrap(),
            latest_version(&MIGRATIONS)
        );
    }

    #[test]
    fn applying_the_migrations_twice_changes_nothing_the_second_time() {
        let (_directory, path, mut connection) = fresh();
        apply(&mut connection, &path, &MIGRATIONS).unwrap();
        let version = apply(&mut connection, &path, &MIGRATIONS).unwrap();
        assert_eq!(version, latest_version(&MIGRATIONS));
    }

    #[test]
    fn a_database_newer_than_this_build_is_refused_rather_than_downgraded() {
        let (_directory, path, mut connection) = fresh();
        apply(&mut connection, &path, &MIGRATIONS).unwrap();
        connection
            .pragma_update(None, "user_version", 99i64)
            .unwrap();
        match apply(&mut connection, &path, &MIGRATIONS) {
            Err(StoreError::DatabaseFromTheFuture { found, supported }) => {
                assert_eq!(found, 99);
                assert_eq!(supported, latest_version(&MIGRATIONS));
            }
            other => panic!("a future database must be refused, got {other:?}"),
        }
    }

    #[test]
    fn a_failing_migration_leaves_the_version_untouched() {
        let (_directory, path, mut connection) = fresh();
        let broken = [Migration {
            version: 1,
            name: "broken",
            sql: "CREATE TABLE not valid sql (",
            rewrites_existing_rows: false,
        }];
        match apply(&mut connection, &path, &broken) {
            Err(StoreError::MigrationFailed { version, name, .. }) => {
                assert_eq!(version, 1);
                assert_eq!(name, "broken");
            }
            other => panic!("a broken migration must fail loudly, got {other:?}"),
        }
        assert_eq!(current_version(&connection).unwrap(), 0);
    }

    #[test]
    fn a_failing_migration_rolls_back_the_statements_that_had_succeeded() {
        let (_directory, path, mut connection) = fresh();
        let broken = [Migration {
            version: 1,
            name: "half_broken",
            sql: "CREATE TABLE early (id INTEGER PRIMARY KEY); CREATE TABLE not valid sql (",
            rewrites_existing_rows: false,
        }];
        assert!(apply(&mut connection, &path, &broken).is_err());
        let tables: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'early'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables, 0);
    }

    #[test]
    fn a_destructive_migration_copies_the_database_before_touching_it() {
        let (_directory, path, mut connection) = fresh();
        apply(&mut connection, &path, &MIGRATIONS).unwrap();

        let with_destructive = [
            MIGRATIONS[0].borrowed(),
            MIGRATIONS[1].borrowed(),
            MIGRATIONS[2].borrowed(),
            MIGRATIONS[3].borrowed(),
            Migration {
                version: 5,
                name: "drop_saved_views",
                sql: "DROP TABLE saved_view",
                rewrites_existing_rows: true,
            },
        ];
        apply(&mut connection, &path, &with_destructive).unwrap();

        let backup = path.with_extension("before-v5.backup");
        assert!(backup.exists(), "the backup must exist at {backup:?}");
        let saved = rusqlite::Connection::open(&backup).unwrap();
        let survives: i64 = saved
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'saved_view'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(survives, 1, "the dropped table must survive in the backup");
    }

    #[test]
    fn the_backup_of_a_write_ahead_logged_database_contains_the_committed_rows() {
        let (_directory, path, mut connection) = fresh();
        apply(&mut connection, &path, &MIGRATIONS).unwrap();
        connection
            .execute(
                "INSERT INTO saved_view (id, name, query, columns, sort, position)
                 VALUES (1, 'inbox', 'is:pr is:open', '[]', 'updated-desc', 0)",
                [],
            )
            .unwrap();

        let with_destructive = [
            MIGRATIONS[0].borrowed(),
            MIGRATIONS[1].borrowed(),
            MIGRATIONS[2].borrowed(),
            MIGRATIONS[3].borrowed(),
            Migration {
                version: 5,
                name: "drop_saved_views",
                sql: "DROP TABLE saved_view",
                rewrites_existing_rows: true,
            },
        ];
        apply(&mut connection, &path, &with_destructive).unwrap();

        let backup = path.with_extension("before-v5.backup");
        let saved = rusqlite::Connection::open(&backup).unwrap();
        let rows: i64 = saved
            .query_row("SELECT COUNT(*) FROM saved_view", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            rows, 1,
            "the row committed before the migration must survive"
        );
    }

    #[test]
    fn a_non_destructive_migration_leaves_no_backup_behind() {
        let (_directory, path, mut connection) = fresh();
        apply(&mut connection, &path, &MIGRATIONS).unwrap();
        assert!(!path.with_extension("before-v1.backup").exists());
    }
}
