use quay_core::PreloadReason;
use rusqlite::{Connection, params};

use crate::error::StoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Utilisation {
    pub used: usize,
    pub observed: usize,
}

impl Utilisation {
    pub fn ratio(&self) -> Option<f64> {
        if self.observed == 0 {
            None
        } else {
            Some(self.used as f64 / self.observed as f64)
        }
    }

    pub fn percent(&self) -> Option<f64> {
        self.ratio().map(|ratio| ratio * 100.0)
    }
}

pub fn record_speculation(
    connection: &Connection,
    key: &str,
    reason: PreloadReason,
    at: i64,
) -> Result<i64, StoreError> {
    connection.execute(
        "INSERT INTO preload_outcome (key, reason, used, at) VALUES (?1, ?2, 0, ?3)",
        params![key, reason.id(), at],
    )?;
    Ok(connection.last_insert_rowid())
}

pub fn record_navigation(
    connection: &Connection,
    key: &str,
    served_locally: bool,
    at: i64,
) -> Result<i64, StoreError> {
    connection.execute(
        "INSERT INTO preload_outcome (key, reason, used, at) VALUES (?1, ?2, ?3, ?4)",
        params![
            key,
            PreloadReason::Navigation.id(),
            i64::from(served_locally),
            at
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

pub fn mark_speculation_used(connection: &Connection, key: &str) -> Result<bool, StoreError> {
    let touched = connection.execute(
        "UPDATE preload_outcome SET used = 1
         WHERE id = (SELECT id FROM preload_outcome
                      WHERE key = ?1 AND used = 0 AND reason <> 'navigation'
                      ORDER BY at DESC, id DESC LIMIT 1)",
        params![key],
    )?;
    Ok(touched == 1)
}

pub fn speculation_utilisation(
    connection: &Connection,
    window: usize,
) -> Result<Utilisation, StoreError> {
    tally(
        connection,
        "SELECT used FROM preload_outcome WHERE reason <> 'navigation' ORDER BY id DESC LIMIT ?1",
        window,
    )
}

pub fn navigations_served_locally(
    connection: &Connection,
    window: usize,
) -> Result<Utilisation, StoreError> {
    tally(
        connection,
        "SELECT used FROM preload_outcome WHERE reason = 'navigation' ORDER BY id DESC LIMIT ?1",
        window,
    )
}

fn tally(connection: &Connection, sql: &str, window: usize) -> Result<Utilisation, StoreError> {
    let mut statement = connection.prepare_cached(sql)?;
    let flags = statement
        .query_map((window as i64,), |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<i64>, rusqlite::Error>>()?;
    Ok(Utilisation {
        used: flags.iter().filter(|used| **used != 0).count(),
        observed: flags.len(),
    })
}

pub fn record_navigation_event(
    connection: &Connection,
    from_state: &str,
    to_state: &str,
    action: &str,
    entity_kind: Option<&str>,
    dwell_ms: Option<i64>,
    at: i64,
) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO navigation_event (from_state, to_state, action, entity_kind, dwell_ms, at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![from_state, to_state, action, entity_kind, dwell_ms, at],
    )?;
    Ok(())
}
