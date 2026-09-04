use quay_core::{MutationKind, MutationState};
use quay_store::{Store, StoreError};

const NOW: i64 = 1_788_516_000;

fn store() -> (tempfile::TempDir, Store) {
    let directory = match tempfile::tempdir() {
        Ok(directory) => directory,
        Err(error) => panic!("temporary directory: {error}"),
    };
    let store = match Store::open(&directory.path().join("quay.db")) {
        Ok(store) => store,
        Err(error) => panic!("the store must open: {error}"),
    };
    seed(&store);
    (directory, store)
}

fn seed(store: &Store) {
    let statements = "
        INSERT INTO account (id, host, login, auth_kind, keychain_ref)
            VALUES (1, 'api.github.com', 'octocat', 'pat_classic', 'quay/octocat');
        INSERT INTO repo (id, account_id, node_id, owner, name, is_tracked)
            VALUES (1, 1, 'R_1', 'acme', 'api', 1);
        INSERT INTO pull_request (
            id, repo_id, number, node_id, title, state, is_draft, author,
            base_ref, head_sha, review_state, updated_at)
            VALUES (1, 1, 7, 'PR_1', 'Fix', 'open', 0, 'avery',
                    'main', 'deadbeef', 'commented', '2026-09-04T10:00:00Z');
        INSERT INTO pull_request (
            id, repo_id, number, node_id, title, state, is_draft, author,
            base_ref, head_sha, review_state, updated_at)
            VALUES (2, 1, 8, 'PR_2', 'Other', 'open', 0, 'blake',
                    'main', 'cafebabe', NULL, '2026-09-04T10:00:00Z');
        INSERT INTO review_thread (id, pr_id, node_id, path, is_resolved, is_outdated)
            VALUES (1, 1, 'RT_1', 'src/main.rs', 0, 0);
    ";
    if let Err(error) = store.connection().execute_batch(statements) {
        panic!("the fixture must load: {error}");
    }
}

fn review_state(store: &Store, node_id: &str) -> Option<String> {
    match store.connection().query_row(
        "SELECT review_state FROM pull_request WHERE node_id = ?1",
        [node_id],
        |row| row.get::<_, Option<String>>(0),
    ) {
        Ok(value) => value,
        Err(error) => panic!("the pull request must be readable: {error}"),
    }
}

fn is_resolved(store: &Store, node_id: &str) -> i64 {
    match store.connection().query_row(
        "SELECT is_resolved FROM review_thread WHERE node_id = ?1",
        [node_id],
        |row| row.get(0),
    ) {
        Ok(value) => value,
        Err(error) => panic!("the thread must be readable: {error}"),
    }
}

fn queued(store: &Store) -> usize {
    match store.mutations_in_state(MutationState::Pending) {
        Ok(rows) => rows.len(),
        Err(error) => panic!("the queue must be readable: {error}"),
    }
}

#[test]
fn a_mutation_on_an_unknown_target_writes_neither_the_state_nor_the_queue_row() {
    let (_directory, mut store) = store();
    match store.approve_pull_request("PR_missing", "idem-1", NOW) {
        Err(StoreError::UnknownTarget { table, node_id }) => {
            assert_eq!(table, "pull_request");
            assert_eq!(node_id, "PR_missing");
        }
        other => panic!("an unknown target must be refused, got {other:?}"),
    }
    assert_eq!(queued(&store), 0, "a refused mutation leaves no queue row");
}

#[test]
fn the_optimistic_state_and_the_queue_row_are_written_together() {
    let (_directory, mut store) = store();
    match store.approve_pull_request("PR_1", "idem-1", NOW) {
        Ok(_) => {}
        Err(error) => panic!("the mutation must enqueue: {error}"),
    }
    assert_eq!(review_state(&store, "PR_1").as_deref(), Some("approved"));
    assert_eq!(queued(&store), 1);
}

#[test]
fn the_same_idempotency_key_is_refused_so_a_replay_never_enqueues_twice() {
    let (_directory, mut store) = store();
    match store.approve_pull_request("PR_1", "idem-1", NOW) {
        Ok(_) => {}
        Err(error) => panic!("the first mutation must enqueue: {error}"),
    }
    assert!(store.approve_pull_request("PR_1", "idem-1", NOW).is_err());
    assert_eq!(queued(&store), 1);
}

#[test]
fn two_mutations_on_the_same_target_are_serialised() {
    let (_directory, mut store) = store();
    for key in ["idem-1", "idem-2"] {
        if let Err(error) = store.approve_pull_request("PR_1", key, NOW) {
            panic!("the mutation must enqueue: {error}");
        }
    }

    let first = match store.claim_next_mutation() {
        Ok(Some(mutation)) => mutation,
        other => panic!("the first mutation must be claimable, got {other:?}"),
    };
    assert_eq!(first.state, MutationState::InFlight);
    assert_eq!(first.attempts, 1);

    match store.claim_next_mutation() {
        Ok(None) => {}
        other => panic!("the second mutation on the same target must wait, got {other:?}"),
    }

    if let Err(error) = store.settle_mutation(first.id) {
        panic!("the first mutation must settle: {error}");
    }
    match store.claim_next_mutation() {
        Ok(Some(second)) => assert_ne!(second.id, first.id),
        other => panic!("the second mutation must follow, got {other:?}"),
    }
}

#[test]
fn two_mutations_on_different_targets_do_not_block_each_other() {
    let (_directory, mut store) = store();
    if let Err(error) = store.approve_pull_request("PR_1", "idem-1", NOW) {
        panic!("the mutation must enqueue: {error}");
    }
    if let Err(error) = store.approve_pull_request("PR_2", "idem-2", NOW) {
        panic!("the mutation must enqueue: {error}");
    }
    let first = store.claim_next_mutation();
    let second = store.claim_next_mutation();
    assert!(matches!(first, Ok(Some(_))));
    assert!(matches!(second, Ok(Some(_))));
}

#[test]
fn an_interrupted_flight_is_requeued_when_resending_only_sets_a_state() {
    let (_directory, mut store) = store();
    if let Err(error) = store.resolve_thread("RT_1", "idem-1", NOW) {
        panic!("the mutation must enqueue: {error}");
    }
    if let Err(error) = store.claim_next_mutation() {
        panic!("the mutation must be claimable: {error}");
    }

    match store.replay_interrupted_mutations() {
        Ok(report) => {
            assert_eq!(report.requeued, 1);
            assert_eq!(report.held_back, 0);
        }
        Err(error) => panic!("the replay must succeed: {error}"),
    }
    assert_eq!(queued(&store), 1);
}

#[test]
fn an_interrupted_flight_is_never_resent_when_it_would_create_a_duplicate() {
    let (_directory, mut store) = store();
    if let Err(error) = store.approve_pull_request("PR_1", "idem-1", NOW) {
        panic!("the mutation must enqueue: {error}");
    }
    let claimed = match store.claim_next_mutation() {
        Ok(Some(mutation)) => mutation,
        other => panic!("the mutation must be claimable, got {other:?}"),
    };

    match store.replay_interrupted_mutations() {
        Ok(report) => {
            assert_eq!(report.requeued, 0);
            assert_eq!(report.held_back, 1);
        }
        Err(error) => panic!("the replay must succeed: {error}"),
    }
    assert_eq!(queued(&store), 0);
    match store.mutation(claimed.id) {
        Ok(Some(mutation)) => {
            assert_eq!(mutation.state, MutationState::Failed);
            match mutation.last_error {
                Some(reason) => assert!(reason.contains("duplicate")),
                None => panic!("the reason must be recorded for the user"),
            }
        }
        other => panic!("the mutation must still exist, got {other:?}"),
    }
}

#[test]
fn a_definitive_failure_rolls_the_optimistic_state_back_to_what_it_was() {
    let (_directory, mut store) = store();
    assert_eq!(review_state(&store, "PR_1").as_deref(), Some("commented"));
    let identifier = match store.approve_pull_request("PR_1", "idem-1", NOW) {
        Ok(identifier) => identifier,
        Err(error) => panic!("the mutation must enqueue: {error}"),
    };
    assert_eq!(review_state(&store, "PR_1").as_deref(), Some("approved"));

    if let Err(error) = store.roll_back_mutation(identifier, "422 unprocessable") {
        panic!("the rollback must succeed: {error}");
    }
    assert_eq!(review_state(&store, "PR_1").as_deref(), Some("commented"));
    match store.mutation(identifier) {
        Ok(Some(mutation)) => {
            assert_eq!(mutation.state, MutationState::Failed);
            assert_eq!(mutation.last_error.as_deref(), Some("422 unprocessable"));
        }
        other => panic!("the mutation must still exist, got {other:?}"),
    }
}

#[test]
fn a_rollback_restores_an_absent_previous_value_as_absent() {
    let (_directory, mut store) = store();
    assert!(review_state(&store, "PR_2").is_none());
    let identifier = match store.approve_pull_request("PR_2", "idem-1", NOW) {
        Ok(identifier) => identifier,
        Err(error) => panic!("the mutation must enqueue: {error}"),
    };
    if let Err(error) = store.roll_back_mutation(identifier, "403 forbidden") {
        panic!("the rollback must succeed: {error}");
    }
    assert!(
        review_state(&store, "PR_2").is_none(),
        "an absent previous value must come back absent, not as an empty string"
    );
}

#[test]
fn a_resolved_thread_rolls_back_to_unresolved() {
    let (_directory, mut store) = store();
    assert_eq!(is_resolved(&store, "RT_1"), 0);
    let identifier = match store.resolve_thread("RT_1", "idem-1", NOW) {
        Ok(identifier) => identifier,
        Err(error) => panic!("the mutation must enqueue: {error}"),
    };
    assert_eq!(is_resolved(&store, "RT_1"), 1);
    if let Err(error) = store.roll_back_mutation(identifier, "404 not found") {
        panic!("the rollback must succeed: {error}");
    }
    assert_eq!(is_resolved(&store, "RT_1"), 0);
}

#[test]
fn a_deferred_mutation_returns_to_the_queue_with_its_attempt_counted() {
    let (_directory, mut store) = store();
    if let Err(error) = store.resolve_thread("RT_1", "idem-1", NOW) {
        panic!("the mutation must enqueue: {error}");
    }
    let claimed = match store.claim_next_mutation() {
        Ok(Some(mutation)) => mutation,
        other => panic!("the mutation must be claimable, got {other:?}"),
    };
    if let Err(error) = store.defer_mutation(claimed.id, "429 throttled") {
        panic!("the deferral must succeed: {error}");
    }
    match store.claim_next_mutation() {
        Ok(Some(again)) => {
            assert_eq!(again.id, claimed.id);
            assert_eq!(again.attempts, 2);
        }
        other => panic!("the mutation must come back, got {other:?}"),
    }
}

#[test]
fn the_queue_survives_reopening_the_database() {
    let directory = match tempfile::tempdir() {
        Ok(directory) => directory,
        Err(error) => panic!("temporary directory: {error}"),
    };
    let path = directory.path().join("quay.db");
    {
        let mut store = match Store::open(&path) {
            Ok(store) => store,
            Err(error) => panic!("the store must open: {error}"),
        };
        seed(&store);
        if let Err(error) = store.approve_pull_request("PR_1", "idem-1", NOW) {
            panic!("the mutation must enqueue: {error}");
        }
    }
    let store = match Store::open(&path) {
        Ok(store) => store,
        Err(error) => panic!("the store must reopen: {error}"),
    };
    assert_eq!(queued(&store), 1);
    assert_eq!(review_state(&store, "PR_1").as_deref(), Some("approved"));
    match store.mutations_in_state(MutationState::Pending) {
        Ok(rows) => assert_eq!(rows[0].kind, MutationKind::ApprovePullRequest),
        Err(error) => panic!("the queue must be readable: {error}"),
    }
}

#[test]
fn requesting_changes_sets_the_review_state_and_rolls_back_to_what_it_was() {
    let (_directory, mut store) = store();
    let identifier = match store.request_changes("PR_1", "idem-1", NOW) {
        Ok(identifier) => identifier,
        Err(error) => panic!("the mutation must enqueue: {error}"),
    };
    assert_eq!(
        review_state(&store, "PR_1").as_deref(),
        Some("changes_requested")
    );
    if let Err(error) = store.roll_back_mutation(identifier, "403 forbidden") {
        panic!("the rollback must succeed: {error}");
    }
    assert_eq!(review_state(&store, "PR_1").as_deref(), Some("commented"));
}

#[test]
fn merging_closes_the_pull_request_locally_and_rolls_back_to_open() {
    let (_directory, mut store) = store();
    let state = |store: &Store| -> String {
        match store.connection().query_row(
            "SELECT state FROM pull_request WHERE node_id = 'PR_1'",
            [],
            |row| row.get(0),
        ) {
            Ok(value) => value,
            Err(error) => panic!("the pull request must be readable: {error}"),
        }
    };
    assert_eq!(state(&store), "open");
    let identifier = match store.merge_pull_request("PR_1", "idem-1", NOW) {
        Ok(identifier) => identifier,
        Err(error) => panic!("the mutation must enqueue: {error}"),
    };
    assert_eq!(state(&store), "merged");
    if let Err(error) = store.roll_back_mutation(identifier, "409 conflict") {
        panic!("the rollback must succeed: {error}");
    }
    assert_eq!(
        state(&store),
        "open",
        "a refused merge must put the pull request back where the user found it"
    );
}

#[test]
fn a_merge_and_an_approval_on_the_same_target_are_serialised() {
    let (_directory, mut store) = store();
    if let Err(error) = store.approve_pull_request("PR_1", "idem-1", NOW) {
        panic!("the approval must enqueue: {error}");
    }
    if let Err(error) = store.merge_pull_request("PR_1", "idem-2", NOW) {
        panic!("the merge must enqueue: {error}");
    }
    assert!(matches!(store.claim_next_mutation(), Ok(Some(_))));
    assert!(matches!(store.claim_next_mutation(), Ok(None)));
}
