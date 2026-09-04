use rusqlite::{Connection, params};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const RAW_PAYLOAD_BYTES: usize = 3_072;
const COMMENTS_PER_THREAD: usize = 4;
const OWNERS: [&str; 4] = ["acme", "acme-infra", "acme-labs", "contoso"];
const AUTHORS: [&str; 8] = [
    "avery", "blake", "casey", "devon", "emery", "finley", "harper", "jordan",
];
const REVIEW_STATES: [Option<&str>; 4] = [
    None,
    Some("approved"),
    Some("changes_requested"),
    Some("commented"),
];
const CHECKS_STATES: [&str; 4] = ["success", "failure", "pending", "none"];

pub struct DatasetShape {
    pub repos: usize,
    pub open_pull_requests: usize,
    pub closed_pull_requests: usize,
    pub review_comments: usize,
    pub viewer: String,
    pub seed: u64,
}

impl DatasetShape {
    pub fn reference() -> Self {
        Self {
            repos: 20,
            open_pull_requests: 300,
            closed_pull_requests: 0,
            review_comments: 5_000,
            viewer: "viewer".to_owned(),
            seed: 0x5157_4159,
        }
    }

    pub fn scaled(factor: usize) -> Self {
        let reference = Self::reference();
        Self {
            repos: reference.repos,
            open_pull_requests: reference.open_pull_requests * factor,
            closed_pull_requests: reference.closed_pull_requests * factor,
            review_comments: reference.review_comments * factor,
            ..reference
        }
    }

    pub fn label(&self) -> String {
        format!(
            "{}repos-{}open-{}closed-{}comments",
            self.repos, self.open_pull_requests, self.closed_pull_requests, self.review_comments
        )
    }

    fn pull_requests(&self) -> usize {
        self.open_pull_requests + self.closed_pull_requests
    }

    fn review_threads(&self) -> usize {
        self.review_comments.div_ceil(COMMENTS_PER_THREAD).max(1)
    }
}

struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u64() % bound as u64) as usize
        }
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    fn bytes(&mut self, length: usize) -> Vec<u8> {
        let mut filled = Vec::with_capacity(length);
        while filled.len() < length {
            filled.extend_from_slice(&self.next_u64().to_le_bytes());
        }
        filled.truncate(length);
        filled
    }

    fn hex(&mut self, length: usize) -> String {
        let mut rendered = String::with_capacity(length);
        while rendered.len() < length {
            rendered.push_str(&format!("{:016x}", self.next_u64()));
        }
        rendered.truncate(length);
        rendered
    }
}

fn spread_timestamp(base: OffsetDateTime, rank: usize, total: usize) -> String {
    let window_seconds = 30 * 24 * 3_600i64;
    let offset = if total <= 1 {
        0
    } else {
        window_seconds * rank as i64 / total as i64
    };
    let moment = base - time::Duration::seconds(offset);
    moment.format(&Rfc3339).unwrap_or_else(|_| String::new())
}

pub fn seed(connection: &mut Connection, shape: &DatasetShape) -> rusqlite::Result<()> {
    let mut rng = DeterministicRng::new(shape.seed);
    let base =
        OffsetDateTime::from_unix_timestamp(1_788_000_000).unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let transaction = connection.transaction()?;

    transaction.execute(
        "INSERT INTO account (id, host, login, auth_kind, keychain_ref)
         VALUES (1, 'api.github.com', ?1, 'pat_classic', 'quay/account/1')",
        params![shape.viewer],
    )?;

    {
        let mut insert_repo = transaction.prepare(
            "INSERT INTO repo (id, account_id, node_id, owner, name, default_branch, local_path, is_tracked)
             VALUES (?1, 1, ?2, ?3, ?4, 'main', NULL, ?5)")?;
        for repo_index in 0..shape.repos {
            insert_repo.execute(params![
                repo_index as i64 + 1,
                format!("R_repo{repo_index}"),
                OWNERS[repo_index % OWNERS.len()],
                format!("service-{repo_index}"),
                i64::from(repo_index % 10 != 9),
            ])?;
        }
    }

    let total_pull_requests = shape.pull_requests();
    {
        let mut insert_pull_request = transaction.prepare(
            "INSERT INTO pull_request (
                 id, repo_id, number, node_id, title, state, is_draft, author,
                 base_ref, head_sha, review_state, checks_state, mergeable, updated_at, raw)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'main', ?9, ?10, ?11, 'MERGEABLE', ?12, ?13)",
        )?;
        for pull_request_index in 0..total_pull_requests {
            let repo_id = (pull_request_index % shape.repos) as i64 + 1;
            let state = if pull_request_index < shape.open_pull_requests {
                "open"
            } else {
                "closed"
            };
            let author = if pull_request_index % 7 == 0 {
                shape.viewer.as_str()
            } else {
                rng.pick(&AUTHORS)
            };
            insert_pull_request.execute(params![
                pull_request_index as i64 + 1,
                repo_id,
                pull_request_index as i64 / shape.repos as i64 + 1,
                format!("PR_node{pull_request_index}"),
                format!("Refactor the {} path", rng.pick(&AUTHORS)),
                state,
                i64::from(pull_request_index % 11 == 0),
                author,
                rng.hex(40),
                rng.pick(&REVIEW_STATES),
                rng.pick(&CHECKS_STATES),
                spread_timestamp(base, pull_request_index, total_pull_requests),
                rng.bytes(RAW_PAYLOAD_BYTES),
            ])?;
        }
    }

    let thread_count = shape.review_threads();
    {
        let mut insert_thread = transaction.prepare(
            "INSERT INTO review_thread (
                 id, pr_id, node_id, path, line, side, original_line, diff_hunk, is_resolved, is_outdated)
             VALUES (?1, ?2, ?3, ?4, ?5, 'RIGHT', ?6, ?7, ?8, ?9)")?;
        for thread_index in 0..thread_count {
            let pull_request_id = (thread_index % shape.open_pull_requests.max(1)) as i64 + 1;
            insert_thread.execute(params![
                thread_index as i64 + 1,
                pull_request_id,
                format!("RT_node{thread_index}"),
                format!("src/module_{}/handler.rs", thread_index % 40),
                (rng.below(400) + 1) as i64,
                (rng.below(400) + 1) as i64,
                format!("@@ -{},7 +{},9 @@", thread_index % 400, thread_index % 400),
                i64::from(thread_index % 3 == 0),
                i64::from(thread_index % 17 == 0),
            ])?;
        }
    }

    {
        let mut insert_comment = transaction.prepare(
            "INSERT INTO review_comment (id, thread_id, node_id, author, body, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for comment_index in 0..shape.review_comments {
            let thread_id = (comment_index % thread_count) as i64 + 1;
            insert_comment.execute(params![
                comment_index as i64 + 1,
                thread_id,
                format!("RC_node{comment_index}"),
                rng.pick(&AUTHORS),
                format!(
                    "This branch is not covered by a test. Consider {} instead.",
                    rng.pick(&AUTHORS)
                ),
                spread_timestamp(base, comment_index, shape.review_comments),
            ])?;
        }
    }

    {
        let mut insert_cache = transaction.prepare(
            "INSERT INTO resource_cache (key, etag, last_modified, fetched_at, stale_after, tier)
             VALUES (?1, ?2, NULL, ?3, ?4, ?5)",
        )?;
        for pull_request_index in 0..total_pull_requests {
            insert_cache.execute(params![
                format!("pr:PR_node{pull_request_index}"),
                format!("W/\"{}\"", rng.hex(32)),
                1_788_000_000i64,
                1_788_000_060i64,
                if pull_request_index == 0 {
                    "hot"
                } else {
                    "warm"
                },
            ])?;
        }
    }

    transaction.commit()?;
    connection.execute_batch("ANALYZE")?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{DatasetShape, DeterministicRng, seed};
    use crate::schema;

    fn seeded(shape: &DatasetShape) -> (tempfile::TempDir, rusqlite::Connection) {
        let directory = tempfile::tempdir().unwrap();
        let mut connection = schema::create(&directory.path().join("quay.db")).unwrap();
        seed(&mut connection, shape).unwrap();
        (directory, connection)
    }

    fn count(connection: &rusqlite::Connection, table: &str) -> i64 {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    #[test]
    fn the_reference_dataset_matches_the_shape_of_the_performance_budgets() {
        let shape = DatasetShape::reference();
        let (_directory, connection) = seeded(&shape);
        assert_eq!(count(&connection, "repo"), 20);
        assert_eq!(count(&connection, "pull_request"), 300);
        assert_eq!(count(&connection, "pull_request WHERE state = 'open'"), 300);
        assert_eq!(count(&connection, "review_comment"), 5_000);
    }

    #[test]
    fn seeding_twice_with_the_same_seed_produces_identical_rows() {
        let shape = DatasetShape::reference();
        let (_first_directory, first) = seeded(&shape);
        let (_second_directory, second) = seeded(&shape);
        let digest = |connection: &rusqlite::Connection| -> String {
            connection
                .query_row(
                    "SELECT group_concat(node_id || head_sha || updated_at, '|')
                     FROM pull_request ORDER BY id",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap()
        };
        assert_eq!(digest(&first), digest(&second));
    }

    #[test]
    fn timestamps_sort_chronologically_as_text() {
        let shape = DatasetShape::reference();
        let (_directory, connection) = seeded(&shape);
        let newest: String = connection
            .query_row(
                "SELECT updated_at FROM pull_request ORDER BY updated_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let oldest: String = connection
            .query_row(
                "SELECT updated_at FROM pull_request ORDER BY updated_at ASC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(newest > oldest);
        assert!(newest.ends_with('Z'));
    }

    #[test]
    fn the_generator_produces_a_stable_sequence_for_a_given_seed() {
        let mut first = DeterministicRng::new(42);
        let mut second = DeterministicRng::new(42);
        let drawn: Vec<u64> = (0..8).map(|_| first.next_u64()).collect();
        let redrawn: Vec<u64> = (0..8).map(|_| second.next_u64()).collect();
        assert_eq!(drawn, redrawn);
        assert!(drawn.windows(2).any(|pair| pair[0] != pair[1]));
    }
}
