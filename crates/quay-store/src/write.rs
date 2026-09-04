use quay_core::{PullRequestSnapshot, Repository, ReviewRequest, ReviewThread, TimelineEvent};
use rusqlite::{Connection, Transaction, params};

use crate::error::StoreError;

pub fn upsert_account(
    connection: &Connection,
    host: &str,
    login: &str,
    auth_kind: &str,
    keychain_ref: &str,
) -> Result<i64, StoreError> {
    connection.execute(
        "INSERT INTO account (host, login, auth_kind, keychain_ref)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(host, login) DO UPDATE SET
             auth_kind = excluded.auth_kind,
             keychain_ref = excluded.keychain_ref",
        params![host, login, auth_kind, keychain_ref],
    )?;
    Ok(connection.query_row(
        "SELECT id FROM account WHERE host = ?1 AND login = ?2",
        params![host, login],
        |row| row.get(0),
    )?)
}

pub fn save_snapshot(
    connection: &mut Connection,
    account_id: i64,
    snapshot: &PullRequestSnapshot,
) -> Result<i64, StoreError> {
    let transaction = connection.transaction()?;
    let repo_id = upsert_repository(&transaction, account_id, &snapshot.repository)?;
    let pull_request_id = upsert_pull_request(&transaction, repo_id, snapshot)?;
    replace_threads(&transaction, pull_request_id, &snapshot.threads)?;
    replace_review_requests(&transaction, pull_request_id, &snapshot.review_requests)?;
    replace_timeline(&transaction, pull_request_id, &snapshot.events)?;
    transaction.commit()?;
    Ok(pull_request_id)
}

fn upsert_repository(
    transaction: &Transaction<'_>,
    account_id: i64,
    repository: &Repository,
) -> Result<i64, StoreError> {
    transaction.execute(
        "INSERT INTO repo (account_id, node_id, owner, name, default_branch, is_tracked)
         VALUES (?1, ?2, ?3, ?4, ?5, 1)
         ON CONFLICT(node_id) DO UPDATE SET
             owner = excluded.owner,
             name = excluded.name,
             default_branch = COALESCE(excluded.default_branch, repo.default_branch),
             is_tracked = 1",
        params![
            account_id,
            repository.node_id,
            repository.owner,
            repository.name,
            repository.default_branch,
        ],
    )?;
    Ok(transaction.query_row(
        "SELECT id FROM repo WHERE node_id = ?1",
        params![repository.node_id],
        |row| row.get(0),
    )?)
}

fn upsert_pull_request(
    transaction: &Transaction<'_>,
    repo_id: i64,
    snapshot: &PullRequestSnapshot,
) -> Result<i64, StoreError> {
    let pull_request = &snapshot.pull_request;
    transaction.execute(
        "INSERT INTO pull_request (
             repo_id, number, node_id, title, state, is_draft, author,
             base_ref, head_sha, review_state, checks_state, mergeable, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(node_id) DO UPDATE SET
             repo_id = excluded.repo_id,
             number = excluded.number,
             title = excluded.title,
             state = excluded.state,
             is_draft = excluded.is_draft,
             author = excluded.author,
             base_ref = excluded.base_ref,
             head_sha = excluded.head_sha,
             review_state = excluded.review_state,
             checks_state = excluded.checks_state,
             mergeable = excluded.mergeable,
             updated_at = excluded.updated_at",
        params![
            repo_id,
            pull_request.number,
            pull_request.node_id,
            pull_request.title,
            pull_request.state.id(),
            i64::from(pull_request.is_draft),
            pull_request.author,
            pull_request.base_ref,
            pull_request.head_sha,
            pull_request.review_state,
            pull_request.checks_state,
            pull_request.mergeable,
            pull_request.updated_at,
        ],
    )?;
    let pull_request_id: i64 = transaction.query_row(
        "SELECT id FROM pull_request WHERE node_id = ?1",
        params![pull_request.node_id],
        |row| row.get(0),
    )?;

    transaction.execute(
        "INSERT INTO pull_request_payload (pr_id, raw) VALUES (?1, ?2)
         ON CONFLICT(pr_id) DO UPDATE SET raw = excluded.raw",
        params![pull_request_id, snapshot.raw],
    )?;
    Ok(pull_request_id)
}

fn replace_threads(
    transaction: &Transaction<'_>,
    pull_request_id: i64,
    threads: &[ReviewThread],
) -> Result<(), StoreError> {
    transaction.execute(
        "DELETE FROM review_thread WHERE pr_id = ?1",
        params![pull_request_id],
    )?;
    let mut insert_thread = transaction.prepare(
        "INSERT INTO review_thread (
             pr_id, node_id, path, line, side, original_line, diff_hunk, is_resolved, is_outdated)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    let mut insert_comment = transaction.prepare(
        "INSERT INTO review_comment (thread_id, node_id, author, body, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;

    for thread in threads {
        insert_thread.execute(params![
            pull_request_id,
            thread.node_id,
            thread.path,
            thread.line,
            thread.side,
            thread.original_line,
            thread.diff_hunk,
            i64::from(thread.is_resolved),
            i64::from(thread.is_outdated),
        ])?;
        let thread_id = transaction.last_insert_rowid();
        for comment in &thread.comments {
            insert_comment.execute(params![
                thread_id,
                comment.node_id,
                comment.author,
                comment.body,
                comment.created_at,
            ])?;
        }
    }
    Ok(())
}

fn replace_review_requests(
    transaction: &Transaction<'_>,
    pull_request_id: i64,
    requests: &[ReviewRequest],
) -> Result<(), StoreError> {
    transaction.execute(
        "DELETE FROM review_request WHERE pr_id = ?1",
        params![pull_request_id],
    )?;
    let mut insert = transaction.prepare(
        "INSERT INTO review_request (pr_id, reviewer, is_team, requested_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(pr_id, reviewer, is_team) DO UPDATE SET
             requested_at = excluded.requested_at",
    )?;
    for request in requests {
        insert.execute(params![
            pull_request_id,
            request.reviewer,
            i64::from(request.is_team),
            request.requested_at,
        ])?;
    }
    Ok(())
}

fn replace_timeline(
    transaction: &Transaction<'_>,
    pull_request_id: i64,
    events: &[TimelineEvent],
) -> Result<(), StoreError> {
    transaction.execute(
        "DELETE FROM timeline_event WHERE pr_id = ?1",
        params![pull_request_id],
    )?;
    let mut insert = transaction.prepare(
        "INSERT INTO timeline_event (pr_id, node_id, kind, actor, body, reference, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for event in events {
        insert.execute(params![
            pull_request_id,
            event.node_id,
            event.kind.id(),
            event.actor,
            event.body,
            event.reference,
            event.created_at,
        ])?;
    }
    Ok(())
}
