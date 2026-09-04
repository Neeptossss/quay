use quay_core::{Mutation, MutationKind, MutationState};
use rusqlite::{Connection, Row, Transaction, params};

use crate::error::StoreError;

const COLUMNS: &str =
    "id, kind, target, payload, idempotency, state, attempts, last_error, created_at";

pub struct OptimisticChange {
    pub kind: MutationKind,
    pub target: String,
    pub payload: String,
    pub idempotency: String,
    pub created_at: i64,
}

struct StoredMutation {
    id: i64,
    kind: String,
    target: String,
    payload: String,
    idempotency: String,
    state: String,
    attempts: i64,
    last_error: Option<String>,
    created_at: i64,
}

fn read(row: &Row<'_>) -> Result<StoredMutation, rusqlite::Error> {
    Ok(StoredMutation {
        id: row.get(0)?,
        kind: row.get(1)?,
        target: row.get(2)?,
        payload: row.get(3)?,
        idempotency: row.get(4)?,
        state: row.get(5)?,
        attempts: row.get(6)?,
        last_error: row.get(7)?,
        created_at: row.get(8)?,
    })
}

fn interpret(stored: StoredMutation) -> Result<Mutation, StoreError> {
    let kind = MutationKind::parse(&stored.kind).ok_or(StoreError::UnreadableValue {
        column: "mutation.kind",
        value: stored.kind,
    })?;
    let state = MutationState::parse(&stored.state).ok_or(StoreError::UnreadableValue {
        column: "mutation.state",
        value: stored.state,
    })?;
    Ok(Mutation {
        id: stored.id,
        kind,
        target: stored.target,
        payload: stored.payload,
        idempotency: stored.idempotency,
        state,
        attempts: stored.attempts.max(0) as u32,
        last_error: stored.last_error,
        created_at: stored.created_at,
    })
}

pub fn enqueue_with_local_effect<F>(
    connection: &mut Connection,
    change: &OptimisticChange,
    apply_locally: F,
) -> Result<i64, StoreError>
where
    F: FnOnce(&Transaction<'_>) -> Result<(), StoreError>,
{
    let transaction = connection.transaction()?;
    apply_locally(&transaction)?;
    transaction.execute(
        "INSERT INTO mutation (kind, target, payload, idempotency, state, attempts, created_at)
         VALUES (?1, ?2, ?3, ?4, 'pending', 0, ?5)",
        params![
            change.kind.id(),
            change.target,
            change.payload,
            change.idempotency,
            change.created_at,
        ],
    )?;
    let identifier = transaction.last_insert_rowid();
    transaction.commit()?;
    Ok(identifier)
}

pub fn claim_next(connection: &mut Connection) -> Result<Option<Mutation>, StoreError> {
    let transaction = connection.transaction()?;
    let claimed = {
        let mut statement = transaction.prepare(&format!(
            "SELECT {COLUMNS} FROM mutation candidate
             WHERE candidate.state = 'pending'
               AND NOT EXISTS (
                     SELECT 1 FROM mutation busy
                      WHERE busy.target = candidate.target AND busy.state = 'inflight')
             ORDER BY candidate.id LIMIT 1"
        ))?;
        let mut rows = statement.query_map([], read)?;
        match rows.next() {
            None => None,
            Some(stored) => Some(interpret(stored?)?),
        }
    };
    let Some(mut mutation) = claimed else {
        transaction.commit()?;
        return Ok(None);
    };
    transaction.execute(
        "UPDATE mutation SET state = 'inflight', attempts = attempts + 1 WHERE id = ?1",
        params![mutation.id],
    )?;
    transaction.commit()?;
    mutation.state = MutationState::InFlight;
    mutation.attempts += 1;
    Ok(Some(mutation))
}

pub fn mark_done(connection: &Connection, id: i64) -> Result<(), StoreError> {
    connection.execute(
        "UPDATE mutation SET state = 'done', last_error = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn mark_failed(connection: &Connection, id: i64, reason: &str) -> Result<(), StoreError> {
    connection.execute(
        "UPDATE mutation SET state = 'failed', last_error = ?2 WHERE id = ?1",
        params![id, reason],
    )?;
    Ok(())
}

pub fn park(connection: &Connection, id: i64) -> Result<(), StoreError> {
    connection.execute(
        "UPDATE mutation SET state = 'pending' WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn return_to_queue(connection: &Connection, id: i64, reason: &str) -> Result<(), StoreError> {
    connection.execute(
        "UPDATE mutation SET state = 'pending', last_error = ?2 WHERE id = ?1",
        params![id, reason],
    )?;
    Ok(())
}

pub fn replay_interrupted(connection: &mut Connection) -> Result<ReplayReport, StoreError> {
    let interrupted = by_state(connection, MutationState::InFlight)?;
    let mut report = ReplayReport::default();
    let transaction = connection.transaction()?;
    for mutation in interrupted {
        if mutation.kind.is_safe_to_resend() {
            transaction.execute(
                "UPDATE mutation SET state = 'pending' WHERE id = ?1",
                params![mutation.id],
            )?;
            report.requeued += 1;
        } else {
            transaction.execute(
                "UPDATE mutation SET state = 'failed', last_error = ?2 WHERE id = ?1",
                params![
                    mutation.id,
                    "sent before the process stopped, outcome unknown; \
                     resending would create a duplicate"
                ],
            )?;
            report.held_back += 1;
        }
    }
    transaction.commit()?;
    Ok(report)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReplayReport {
    pub requeued: usize,
    pub held_back: usize,
}

pub fn by_state(
    connection: &Connection,
    state: MutationState,
) -> Result<Vec<Mutation>, StoreError> {
    let mut statement = connection.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM mutation WHERE state = ?1 ORDER BY id"
    ))?;
    statement
        .query_map((state.id(),), read)?
        .collect::<Result<Vec<StoredMutation>, rusqlite::Error>>()?
        .into_iter()
        .map(interpret)
        .collect()
}

pub fn get(connection: &Connection, id: i64) -> Result<Option<Mutation>, StoreError> {
    let mut statement =
        connection.prepare_cached(&format!("SELECT {COLUMNS} FROM mutation WHERE id = ?1"))?;
    let mut rows = statement.query_map((id,), read)?;
    match rows.next() {
        None => Ok(None),
        Some(stored) => interpret(stored?).map(Some),
    }
}
