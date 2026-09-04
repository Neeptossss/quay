use rusqlite::Connection;

use crate::error::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentView {
    pub author: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadView {
    pub node_id: String,
    pub path: String,
    pub line: Option<i64>,
    pub is_resolved: bool,
    pub is_outdated: bool,
    pub comments: Vec<CommentView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestView {
    pub owner: String,
    pub name: String,
    pub number: i64,
    pub node_id: String,
    pub title: String,
    pub state: String,
    pub is_draft: bool,
    pub author: String,
    pub review_state: Option<String>,
    pub checks_state: Option<String>,
    pub updated_at: String,
    pub threads: Vec<ThreadView>,
}

impl PullRequestView {
    pub fn unresolved_threads(&self) -> usize {
        self.threads
            .iter()
            .filter(|thread| !thread.is_resolved)
            .count()
    }
}

pub fn pull_request(
    connection: &Connection,
    owner: &str,
    name: &str,
    number: i64,
) -> Result<Option<PullRequestView>, StoreError> {
    let mut statement = connection.prepare_cached(
        "SELECT pr.id, pr.node_id, pr.title, pr.state, pr.is_draft, pr.author,
                pr.review_state, pr.checks_state, pr.updated_at
         FROM pull_request pr JOIN repo r ON r.id = pr.repo_id
         WHERE r.owner = ?1 AND r.name = ?2 AND pr.number = ?3",
    )?;
    let mut rows = statement.query_map((owner, name, number), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, String>(8)?,
        ))
    })?;
    let Some(found) = rows.next() else {
        return Ok(None);
    };
    let (id, node_id, title, state, is_draft, author, review_state, checks_state, updated_at) =
        found?;
    drop(rows);

    Ok(Some(PullRequestView {
        owner: owner.to_owned(),
        name: name.to_owned(),
        number,
        node_id,
        title,
        state,
        is_draft: is_draft != 0,
        author,
        review_state,
        checks_state,
        updated_at,
        threads: threads_of(connection, id)?,
    }))
}

fn threads_of(
    connection: &Connection,
    pull_request_id: i64,
) -> Result<Vec<ThreadView>, StoreError> {
    let mut statement = connection.prepare_cached(
        "SELECT id, node_id, path, line, is_resolved, is_outdated
         FROM review_thread WHERE pr_id = ?1 ORDER BY is_resolved, path, id",
    )?;
    let rows = statement
        .query_map((pull_request_id,), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?
        .collect::<Result<Vec<(i64, String, String, Option<i64>, i64, i64)>, rusqlite::Error>>()?;

    let mut threads = Vec::with_capacity(rows.len());
    for (id, node_id, path, line, is_resolved, is_outdated) in rows {
        threads.push(ThreadView {
            node_id,
            path,
            line,
            is_resolved: is_resolved != 0,
            is_outdated: is_outdated != 0,
            comments: comments_of(connection, id)?,
        });
    }
    Ok(threads)
}

fn comments_of(connection: &Connection, thread_id: i64) -> Result<Vec<CommentView>, StoreError> {
    let mut statement = connection.prepare_cached(
        "SELECT author, body, created_at FROM review_comment WHERE thread_id = ?1
         ORDER BY created_at, id",
    )?;
    Ok(statement
        .query_map((thread_id,), |row| {
            Ok(CommentView {
                author: row.get(0)?,
                body: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<CommentView>, rusqlite::Error>>()?)
}
