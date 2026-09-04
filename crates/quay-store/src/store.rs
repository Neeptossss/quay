use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::StoreError;
use crate::inbox::{InboxFilter, InboxQuery, InboxRow};
use crate::migrations::{self, MIGRATIONS};
use crate::resource_cache::{self, CacheEntry};
use crate::schema;
use crate::write;
use quay_core::PullRequestSnapshot;

pub struct Store {
    connection: Connection,
    path: PathBuf,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let mut connection = schema::open(path)?;
        migrations::apply(&mut connection, path, &MIGRATIONS)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        Ok(Self {
            connection,
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn schema_version(&self) -> Result<i64, StoreError> {
        migrations::current_version(&self.connection)
    }

    pub fn inbox(
        &self,
        query: InboxQuery,
        filter: &InboxFilter,
    ) -> Result<Vec<InboxRow>, StoreError> {
        crate::inbox::run(&self.connection, query, filter)
    }

    pub fn freshness(&self, key: &str) -> Result<Option<CacheEntry>, StoreError> {
        resource_cache::get(&self.connection, key)
    }

    pub fn remember_freshness(&self, entry: &CacheEntry) -> Result<(), StoreError> {
        resource_cache::put(&self.connection, entry)
    }

    pub fn due_for_refresh(&self, now: i64, limit: i64) -> Result<Vec<CacheEntry>, StoreError> {
        resource_cache::due(&self.connection, now, limit)
    }

    pub fn forget_freshness(&self, key: &str) -> Result<(), StoreError> {
        resource_cache::forget(&self.connection, key)
    }

    pub fn remember_account(
        &self,
        host: &str,
        login: &str,
        auth_kind: &str,
        keychain_ref: &str,
    ) -> Result<i64, StoreError> {
        write::upsert_account(&self.connection, host, login, auth_kind, keychain_ref)
    }

    pub fn save_pull_request(
        &mut self,
        account_id: i64,
        snapshot: &PullRequestSnapshot,
    ) -> Result<i64, StoreError> {
        write::save_snapshot(&mut self.connection, account_id, snapshot)
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}
