use quay_core::Tier;
use rusqlite::{Connection, Row, params};

use crate::error::StoreError;

const COLUMNS: &str = "key, etag, last_modified, fetched_at, stale_after, tier";
const DUE_ORDER: &str = "\
ORDER BY CASE tier WHEN 'hot' THEN 0 WHEN 'warm' THEN 1 ELSE 2 END, stale_after ASC";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry {
    pub key: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub fetched_at: i64,
    pub stale_after: i64,
    pub tier: Tier,
}

impl CacheEntry {
    pub fn revalidatable(&self) -> bool {
        self.etag.is_some() || self.last_modified.is_some()
    }

    pub fn is_stale_at(&self, now: i64) -> bool {
        self.stale_after <= now
    }
}

struct StoredEntry {
    key: String,
    etag: Option<String>,
    last_modified: Option<String>,
    fetched_at: i64,
    stale_after: i64,
    tier: String,
}

fn read(row: &Row<'_>) -> Result<StoredEntry, rusqlite::Error> {
    Ok(StoredEntry {
        key: row.get(0)?,
        etag: row.get(1)?,
        last_modified: row.get(2)?,
        fetched_at: row.get(3)?,
        stale_after: row.get(4)?,
        tier: row.get(5)?,
    })
}

fn interpret(stored: StoredEntry) -> Result<CacheEntry, StoreError> {
    let tier = Tier::parse(&stored.tier).ok_or(StoreError::UnreadableValue {
        column: "resource_cache.tier",
        value: stored.tier,
    })?;
    Ok(CacheEntry {
        key: stored.key,
        etag: stored.etag,
        last_modified: stored.last_modified,
        fetched_at: stored.fetched_at,
        stale_after: stored.stale_after,
        tier,
    })
}

pub fn get(connection: &Connection, key: &str) -> Result<Option<CacheEntry>, StoreError> {
    let mut statement = connection.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM resource_cache WHERE key = ?1"
    ))?;
    let mut rows = statement.query_map((key,), read)?;
    match rows.next() {
        None => Ok(None),
        Some(stored) => interpret(stored?).map(Some),
    }
}

pub fn put(connection: &Connection, entry: &CacheEntry) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO resource_cache (key, etag, last_modified, fetched_at, stale_after, tier)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(key) DO UPDATE SET
             etag = excluded.etag,
             last_modified = excluded.last_modified,
             fetched_at = excluded.fetched_at,
             stale_after = excluded.stale_after,
             tier = excluded.tier",
        params![
            entry.key,
            entry.etag,
            entry.last_modified,
            entry.fetched_at,
            entry.stale_after,
            entry.tier.id(),
        ],
    )?;
    Ok(())
}

pub fn due(connection: &Connection, now: i64, limit: i64) -> Result<Vec<CacheEntry>, StoreError> {
    let mut statement = connection.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM resource_cache WHERE stale_after <= ?1 {DUE_ORDER} LIMIT ?2"
    ))?;
    statement
        .query_map((now, limit), read)?
        .collect::<Result<Vec<StoredEntry>, rusqlite::Error>>()?
        .into_iter()
        .map(interpret)
        .collect()
}

pub fn forget(connection: &Connection, key: &str) -> Result<(), StoreError> {
    connection.execute("DELETE FROM resource_cache WHERE key = ?1", (key,))?;
    Ok(())
}
