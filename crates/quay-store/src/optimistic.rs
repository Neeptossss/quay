use quay_core::{Mutation, MutationKind};
use rusqlite::{Connection, Transaction, params};
use serde_json::json;

use crate::error::StoreError;
use crate::mutations::{self, OptimisticChange};

pub fn approve_pull_request(
    connection: &mut Connection,
    pull_request_node_id: &str,
    idempotency: &str,
    now: i64,
) -> Result<i64, StoreError> {
    let previous: Option<String> = read_optional_text(
        connection,
        "SELECT review_state FROM pull_request WHERE node_id = ?1",
        pull_request_node_id,
    )?;
    let change = OptimisticChange {
        kind: MutationKind::ApprovePullRequest,
        target: pull_request_node_id.to_owned(),
        payload: json!({ "previous_review_state": previous }).to_string(),
        idempotency: idempotency.to_owned(),
        created_at: now,
    };
    mutations::enqueue_with_local_effect(connection, &change, |transaction| {
        expect_one_row(
            transaction,
            "UPDATE pull_request SET review_state = 'approved' WHERE node_id = ?1",
            pull_request_node_id,
            "pull_request",
        )
    })
}

pub fn request_changes(
    connection: &mut Connection,
    pull_request_node_id: &str,
    idempotency: &str,
    now: i64,
) -> Result<i64, StoreError> {
    let previous: Option<String> = read_optional_text(
        connection,
        "SELECT review_state FROM pull_request WHERE node_id = ?1",
        pull_request_node_id,
    )?;
    let change = OptimisticChange {
        kind: MutationKind::RequestChanges,
        target: pull_request_node_id.to_owned(),
        payload: json!({ "previous_review_state": previous }).to_string(),
        idempotency: idempotency.to_owned(),
        created_at: now,
    };
    mutations::enqueue_with_local_effect(connection, &change, |transaction| {
        expect_one_row(
            transaction,
            "UPDATE pull_request SET review_state = 'changes_requested' WHERE node_id = ?1",
            pull_request_node_id,
            "pull_request",
        )
    })
}

pub fn merge_pull_request(
    connection: &mut Connection,
    pull_request_node_id: &str,
    idempotency: &str,
    now: i64,
) -> Result<i64, StoreError> {
    let previous: Option<String> = read_optional_text(
        connection,
        "SELECT state FROM pull_request WHERE node_id = ?1",
        pull_request_node_id,
    )?;
    let change = OptimisticChange {
        kind: MutationKind::MergePullRequest,
        target: pull_request_node_id.to_owned(),
        payload: json!({ "previous_state": previous }).to_string(),
        idempotency: idempotency.to_owned(),
        created_at: now,
    };
    mutations::enqueue_with_local_effect(connection, &change, |transaction| {
        expect_one_row(
            transaction,
            "UPDATE pull_request SET state = 'merged' WHERE node_id = ?1",
            pull_request_node_id,
            "pull_request",
        )
    })
}

pub fn resolve_thread(
    connection: &mut Connection,
    thread_node_id: &str,
    idempotency: &str,
    now: i64,
) -> Result<i64, StoreError> {
    let previous: Option<i64> = read_optional_integer(
        connection,
        "SELECT is_resolved FROM review_thread WHERE node_id = ?1",
        thread_node_id,
    )?;
    let change = OptimisticChange {
        kind: MutationKind::ResolveThread,
        target: thread_node_id.to_owned(),
        payload: json!({ "previous_is_resolved": previous }).to_string(),
        idempotency: idempotency.to_owned(),
        created_at: now,
    };
    mutations::enqueue_with_local_effect(connection, &change, |transaction| {
        expect_one_row(
            transaction,
            "UPDATE review_thread SET is_resolved = 1 WHERE node_id = ?1",
            thread_node_id,
            "review_thread",
        )
    })
}

pub fn roll_back(
    connection: &mut Connection,
    mutation_id: i64,
    reason: &str,
) -> Result<(), StoreError> {
    let mutation = mutations::get(connection, mutation_id)?.ok_or(StoreError::UnreadableValue {
        column: "mutation.id",
        value: mutation_id.to_string(),
    })?;
    let transaction = connection.transaction()?;
    restore(&transaction, &mutation)?;
    transaction.execute(
        "UPDATE mutation SET state = 'failed', last_error = ?2 WHERE id = ?1",
        params![mutation_id, reason],
    )?;
    transaction.commit()?;
    Ok(())
}

fn restore(transaction: &Transaction<'_>, mutation: &Mutation) -> Result<(), StoreError> {
    let payload: serde_json::Value =
        serde_json::from_str(&mutation.payload).map_err(|error| StoreError::UnreadableValue {
            column: "mutation.payload",
            value: error.to_string(),
        })?;
    match mutation.kind {
        MutationKind::ApprovePullRequest | MutationKind::RequestChanges => {
            let previous = payload
                .get("previous_review_state")
                .and_then(|value| value.as_str().map(str::to_owned));
            transaction.execute(
                "UPDATE pull_request SET review_state = ?2 WHERE node_id = ?1",
                params![mutation.target, previous],
            )?;
        }
        MutationKind::ResolveThread | MutationKind::UnresolveThread => {
            let previous = payload
                .get("previous_is_resolved")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            transaction.execute(
                "UPDATE review_thread SET is_resolved = ?2 WHERE node_id = ?1",
                params![mutation.target, previous],
            )?;
        }
        MutationKind::MergePullRequest => {
            let previous = payload
                .get("previous_state")
                .and_then(|value| value.as_str())
                .unwrap_or("open");
            transaction.execute(
                "UPDATE pull_request SET state = ?2 WHERE node_id = ?1",
                params![mutation.target, previous],
            )?;
        }
        MutationKind::PostComment => {}
    }
    Ok(())
}

fn expect_one_row(
    transaction: &Transaction<'_>,
    sql: &str,
    node_id: &str,
    table: &'static str,
) -> Result<(), StoreError> {
    let touched = transaction.execute(sql, params![node_id])?;
    if touched == 1 {
        Ok(())
    } else {
        Err(StoreError::UnknownTarget {
            table,
            node_id: node_id.to_owned(),
        })
    }
}

fn read_optional_text(
    connection: &Connection,
    sql: &str,
    node_id: &str,
) -> Result<Option<String>, StoreError> {
    let mut statement = connection.prepare_cached(sql)?;
    let mut rows = statement.query_map((node_id,), |row| row.get::<_, Option<String>>(0))?;
    match rows.next() {
        None => Ok(None),
        Some(value) => Ok(value?),
    }
}

fn read_optional_integer(
    connection: &Connection,
    sql: &str,
    node_id: &str,
) -> Result<Option<i64>, StoreError> {
    let mut statement = connection.prepare_cached(sql)?;
    let mut rows = statement.query_map((node_id,), |row| row.get::<_, Option<i64>>(0))?;
    match rows.next() {
        None => Ok(None),
        Some(value) => Ok(value?),
    }
}
