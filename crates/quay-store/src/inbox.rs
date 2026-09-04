use rusqlite::{Connection, Row};

const OPEN_ACROSS_TRACKED_REPOS: &str = "\
SELECT r.owner, r.name, pr.number, pr.title, pr.is_draft, pr.author,
       pr.review_state, pr.checks_state, pr.updated_at, 0
FROM pull_request pr
JOIN repo r ON r.id = pr.repo_id
WHERE pr.state = 'open' AND r.is_tracked = 1
ORDER BY pr.updated_at DESC
LIMIT ?1";

const OPEN_WITH_UNRESOLVED_THREAD_COUNT: &str = "\
SELECT r.owner, r.name, pr.number, pr.title, pr.is_draft, pr.author,
       pr.review_state, pr.checks_state, pr.updated_at,
       (SELECT COUNT(*) FROM review_thread t
         WHERE t.pr_id = pr.id AND t.is_resolved = 0)
FROM pull_request pr
JOIN repo r ON r.id = pr.repo_id
WHERE pr.state = 'open' AND r.is_tracked = 1
ORDER BY pr.updated_at DESC
LIMIT ?1";

const REVIEW_REQUESTED_APPROXIMATION: &str = "\
SELECT r.owner, r.name, pr.number, pr.title, pr.is_draft, pr.author,
       pr.review_state, pr.checks_state, pr.updated_at, 0
FROM pull_request pr
JOIN repo r ON r.id = pr.repo_id
WHERE pr.state = 'open' AND r.is_tracked = 1
  AND pr.author <> ?2
  AND (pr.review_state IS NULL OR pr.review_state = 'commented')
ORDER BY pr.updated_at DESC
LIMIT ?1";

const OPEN_FILTERED_BY_TITLE: &str = "\
SELECT r.owner, r.name, pr.number, pr.title, pr.is_draft, pr.author,
       pr.review_state, pr.checks_state, pr.updated_at, 0
FROM pull_request pr
JOIN repo r ON r.id = pr.repo_id
WHERE pr.state = 'open' AND r.is_tracked = 1
  AND pr.title LIKE ?2
ORDER BY pr.updated_at DESC
LIMIT ?1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxQuery {
    OpenAcrossTrackedRepos,
    OpenWithUnresolvedThreadCount,
    ReviewRequestedApproximation,
    OpenFilteredByCommonTitle,
    OpenFilteredByRareTitle,
}

impl InboxQuery {
    pub const ALL: [InboxQuery; 5] = [
        InboxQuery::OpenAcrossTrackedRepos,
        InboxQuery::OpenWithUnresolvedThreadCount,
        InboxQuery::ReviewRequestedApproximation,
        InboxQuery::OpenFilteredByCommonTitle,
        InboxQuery::OpenFilteredByRareTitle,
    ];

    pub fn id(self) -> &'static str {
        match self {
            InboxQuery::OpenAcrossTrackedRepos => "open_across_tracked_repos",
            InboxQuery::OpenWithUnresolvedThreadCount => "open_with_unresolved_thread_count",
            InboxQuery::ReviewRequestedApproximation => "review_requested_approximation",
            InboxQuery::OpenFilteredByCommonTitle => "open_filtered_by_common_title",
            InboxQuery::OpenFilteredByRareTitle => "open_filtered_by_rare_title",
        }
    }

    pub fn sql(self) -> &'static str {
        match self {
            InboxQuery::OpenAcrossTrackedRepos => OPEN_ACROSS_TRACKED_REPOS,
            InboxQuery::OpenWithUnresolvedThreadCount => OPEN_WITH_UNRESOLVED_THREAD_COUNT,
            InboxQuery::ReviewRequestedApproximation => REVIEW_REQUESTED_APPROXIMATION,
            InboxQuery::OpenFilteredByCommonTitle | InboxQuery::OpenFilteredByRareTitle => {
                OPEN_FILTERED_BY_TITLE
            }
        }
    }

    fn second_parameter(self, filter: &InboxFilter) -> Option<String> {
        match self {
            InboxQuery::OpenAcrossTrackedRepos | InboxQuery::OpenWithUnresolvedThreadCount => None,
            InboxQuery::ReviewRequestedApproximation => Some(filter.viewer.clone()),
            InboxQuery::OpenFilteredByCommonTitle => Some(filter.common_title_pattern.clone()),
            InboxQuery::OpenFilteredByRareTitle => Some(filter.rare_title_pattern.clone()),
        }
    }
}

pub struct InboxFilter {
    pub viewer: String,
    pub common_title_pattern: String,
    pub rare_title_pattern: String,
    pub limit: i64,
}

impl Default for InboxFilter {
    fn default() -> Self {
        Self {
            viewer: "viewer".to_owned(),
            common_title_pattern: "%path%".to_owned(),
            rare_title_pattern: "%a title that no pull request carries%".to_owned(),
            limit: 50,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxRow {
    pub owner: String,
    pub name: String,
    pub number: i64,
    pub title: String,
    pub is_draft: bool,
    pub author: String,
    pub review_state: Option<String>,
    pub checks_state: Option<String>,
    pub updated_at: String,
    pub unresolved_threads: i64,
}

fn read_row(row: &Row<'_>) -> rusqlite::Result<InboxRow> {
    Ok(InboxRow {
        owner: row.get(0)?,
        name: row.get(1)?,
        number: row.get(2)?,
        title: row.get(3)?,
        is_draft: row.get::<_, i64>(4)? != 0,
        author: row.get(5)?,
        review_state: row.get(6)?,
        checks_state: row.get(7)?,
        updated_at: row.get(8)?,
        unresolved_threads: row.get(9)?,
    })
}

pub fn run(
    connection: &Connection,
    query: InboxQuery,
    filter: &InboxFilter,
) -> rusqlite::Result<Vec<InboxRow>> {
    let mut statement = connection.prepare_cached(query.sql())?;
    match query.second_parameter(filter) {
        None => statement
            .query_map((filter.limit,), read_row)?
            .collect::<rusqlite::Result<Vec<InboxRow>>>(),
        Some(second) => statement
            .query_map((filter.limit, second), read_row)?
            .collect::<rusqlite::Result<Vec<InboxRow>>>(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{InboxFilter, InboxQuery, run};
    use crate::dataset::{DatasetShape, seed};
    use crate::schema;

    fn reference_database() -> (tempfile::TempDir, rusqlite::Connection) {
        let directory = tempfile::tempdir().unwrap();
        let mut connection = schema::create(&directory.path().join("quay.db")).unwrap();
        seed(&mut connection, &DatasetShape::reference()).unwrap();
        (directory, connection)
    }

    #[test]
    fn every_declared_query_runs_against_the_reference_dataset() {
        let (_directory, connection) = reference_database();
        let filter = InboxFilter::default();
        for query in InboxQuery::ALL {
            let rows = run(&connection, query, &filter).unwrap();
            assert!(rows.len() <= filter.limit as usize, "{}", query.id());
        }
    }

    #[test]
    fn the_inbox_is_ordered_from_the_most_recently_updated_pull_request() {
        let (_directory, connection) = reference_database();
        let rows = run(
            &connection,
            InboxQuery::OpenAcrossTrackedRepos,
            &InboxFilter::default(),
        )
        .unwrap();
        let mut sorted = rows.clone();
        sorted.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        assert_eq!(rows, sorted);
    }

    #[test]
    fn untracked_repositories_never_reach_the_inbox() {
        let (_directory, connection) = reference_database();
        let filter = InboxFilter {
            limit: 10_000,
            ..InboxFilter::default()
        };
        let rows = run(&connection, InboxQuery::OpenAcrossTrackedRepos, &filter).unwrap();
        let tracked: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pull_request pr JOIN repo r ON r.id = pr.repo_id
                 WHERE pr.state = 'open' AND r.is_tracked = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows.len() as i64, tracked);
        assert!(tracked < 300);
    }

    #[test]
    fn the_unresolved_thread_count_matches_the_review_thread_table() {
        let (_directory, connection) = reference_database();
        let rows = run(
            &connection,
            InboxQuery::OpenWithUnresolvedThreadCount,
            &InboxFilter::default(),
        )
        .unwrap();
        assert!(rows.iter().any(|row| row.unresolved_threads > 0));
    }

    #[test]
    fn the_review_requested_approximation_excludes_the_viewer_own_pull_requests() {
        let (_directory, connection) = reference_database();
        let filter = InboxFilter {
            limit: 10_000,
            ..InboxFilter::default()
        };
        let rows = run(
            &connection,
            InboxQuery::ReviewRequestedApproximation,
            &filter,
        )
        .unwrap();
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|row| row.author != filter.viewer));
    }

    #[test]
    fn no_table_records_which_reviewer_a_pull_request_is_requested_from() {
        let (_directory, connection) = reference_database();
        let reviewer_tables: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND (name LIKE '%review_request%' OR name LIKE '%reviewer%')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(reviewer_tables, 0);

        let has_reviewer_column: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('pull_request')
                 WHERE name IN ('requested_reviewers', 'reviewer', 'review_requested')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(has_reviewer_column, 0);
    }

    #[test]
    fn an_empty_database_yields_an_empty_inbox_rather_than_an_error() {
        let directory = tempfile::tempdir().unwrap();
        let connection = schema::create(&directory.path().join("quay.db")).unwrap();
        for query in InboxQuery::ALL {
            assert!(
                run(&connection, query, &InboxFilter::default())
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn the_common_filter_matches_every_title_and_the_rare_filter_matches_none() {
        let (_directory, connection) = reference_database();
        let filter = InboxFilter {
            limit: 10_000,
            ..InboxFilter::default()
        };
        let common = run(&connection, InboxQuery::OpenFilteredByCommonTitle, &filter).unwrap();
        let rare = run(&connection, InboxQuery::OpenFilteredByRareTitle, &filter).unwrap();
        assert!(!common.is_empty());
        assert!(rare.is_empty());
    }

    #[test]
    fn a_negative_limit_returns_the_whole_inbox_rather_than_failing() {
        let (_directory, connection) = reference_database();
        let filter = InboxFilter {
            limit: -1,
            ..InboxFilter::default()
        };
        let rows = run(&connection, InboxQuery::OpenAcrossTrackedRepos, &filter).unwrap();
        assert!(rows.len() > 50);
    }
}
