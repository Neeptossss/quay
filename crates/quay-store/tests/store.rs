use quay_core::Tier;
use quay_store::inbox::{InboxFilter, InboxQuery};
use quay_store::{CacheEntry, Store, StoreError};

fn store() -> (tempfile::TempDir, Store) {
    let directory = match tempfile::tempdir() {
        Ok(directory) => directory,
        Err(error) => panic!("temporary directory: {error}"),
    };
    let store = match Store::open(&directory.path().join("quay.db")) {
        Ok(store) => store,
        Err(error) => panic!("the store must open: {error}"),
    };
    (directory, store)
}

fn entry(key: &str, stale_after: i64, tier: Tier) -> CacheEntry {
    CacheEntry {
        key: key.to_owned(),
        etag: Some(format!("W/\"{key}\"")),
        last_modified: None,
        fetched_at: 1_788_000_000,
        stale_after,
        tier,
    }
}

fn remember(store: &Store, entry: &CacheEntry) {
    if let Err(error) = store.remember_freshness(entry) {
        panic!("the entry must be stored: {error}");
    }
}

#[test]
fn opening_a_new_database_brings_it_to_the_latest_schema_version() {
    let (_directory, store) = store();
    match store.schema_version() {
        Ok(version) => assert_eq!(version, 3),
        Err(error) => panic!("the version must be readable: {error}"),
    }
}

#[test]
fn reopening_a_database_does_not_replay_the_migrations() {
    let directory = match tempfile::tempdir() {
        Ok(directory) => directory,
        Err(error) => panic!("temporary directory: {error}"),
    };
    let path = directory.path().join("quay.db");
    if let Err(error) = Store::open(&path) {
        panic!("the first open must succeed: {error}");
    }
    match Store::open(&path) {
        Ok(store) => assert!(matches!(store.schema_version(), Ok(3))),
        Err(error) => panic!("the second open must succeed: {error}"),
    }
}

#[test]
fn a_database_stamped_by_a_newer_build_is_refused_rather_than_opened() {
    let directory = match tempfile::tempdir() {
        Ok(directory) => directory,
        Err(error) => panic!("temporary directory: {error}"),
    };
    let path = directory.path().join("quay.db");
    {
        let store = match Store::open(&path) {
            Ok(store) => store,
            Err(error) => panic!("the first open must succeed: {error}"),
        };
        if let Err(error) = store
            .connection()
            .pragma_update(None, "user_version", 42i64)
        {
            panic!("stamping a future version must succeed: {error}");
        }
    }
    match Store::open(&path) {
        Err(StoreError::DatabaseFromTheFuture { found, supported }) => {
            assert_eq!(found, 42);
            assert_eq!(supported, 3);
        }
        Err(error) => panic!("a future database must name the version gap, got {error}"),
        Ok(_) => panic!("a future database must be refused, not opened"),
    }
}

#[test]
fn a_review_thread_pointing_at_no_pull_request_is_refused() {
    let (_directory, store) = store();
    let inserted = store.connection().execute(
        "INSERT INTO review_thread (id, pr_id, node_id, path, is_resolved, is_outdated)
         VALUES (1, 999, 'RT_1', 'src/main.rs', 0, 0)",
        [],
    );
    assert!(
        inserted.is_err(),
        "foreign keys must be enforced on an opened store"
    );
}

#[test]
fn an_empty_store_answers_the_inbox_with_no_row_rather_than_an_error() {
    let (_directory, store) = store();
    match store.inbox(InboxQuery::ReviewRequestedExact, &InboxFilter::default()) {
        Ok(rows) => assert!(rows.is_empty()),
        Err(error) => panic!("an empty inbox is not an error: {error}"),
    }
}

#[test]
fn an_unknown_resource_has_no_freshness_rather_than_a_default_one() {
    let (_directory, store) = store();
    match store.freshness("pr:PR_unknown") {
        Ok(None) => {}
        other => panic!("an unknown resource must read as absent, got {other:?}"),
    }
}

#[test]
fn a_remembered_resource_comes_back_with_its_validators_and_its_tier() {
    let (_directory, store) = store();
    let remembered = entry("pr:PR_1", 1_788_000_060, Tier::Hot);
    remember(&store, &remembered);
    match store.freshness("pr:PR_1") {
        Ok(Some(loaded)) => {
            assert_eq!(loaded, remembered);
            assert!(loaded.revalidatable());
        }
        other => panic!("the entry must come back, got {other:?}"),
    }
}

#[test]
fn remembering_the_same_resource_twice_replaces_it_instead_of_duplicating_it() {
    let (_directory, store) = store();
    let first = entry("pr:PR_1", 1_788_000_060, Tier::Warm);
    let second = CacheEntry {
        etag: Some("W/\"newer\"".to_owned()),
        stale_after: 1_788_000_600,
        tier: Tier::Cold,
        ..first.clone()
    };
    remember(&store, &first);
    remember(&store, &second);
    match store.freshness("pr:PR_1") {
        Ok(Some(loaded)) => {
            assert_eq!(loaded.etag.as_deref(), Some("W/\"newer\""));
            assert_eq!(loaded.tier, Tier::Cold);
        }
        other => panic!("the newer entry must win, got {other:?}"),
    }
}

#[test]
fn an_entry_with_no_validator_is_not_revalidatable() {
    let (_directory, store) = store();
    let bare = CacheEntry {
        etag: None,
        last_modified: None,
        ..entry("pr:PR_1", 1_788_000_060, Tier::Warm)
    };
    remember(&store, &bare);
    match store.freshness("pr:PR_1") {
        Ok(Some(loaded)) => assert!(!loaded.revalidatable()),
        other => panic!("the entry must come back, got {other:?}"),
    }
}

#[test]
fn a_tier_this_build_cannot_read_is_refused_rather_than_guessed() {
    let (_directory, store) = store();
    let inserted = store.connection().execute(
        "INSERT INTO resource_cache (key, etag, last_modified, fetched_at, stale_after, tier)
         VALUES ('pr:PR_1', NULL, NULL, 0, 0, 'lukewarm')",
        [],
    );
    if let Err(error) = inserted {
        panic!("the row must insert: {error}");
    }
    match store.freshness("pr:PR_1") {
        Err(StoreError::UnreadableValue { column, value }) => {
            assert_eq!(column, "resource_cache.tier");
            assert_eq!(value, "lukewarm");
        }
        other => panic!("an unreadable tier must be refused, got {other:?}"),
    }
}

#[test]
fn only_resources_past_their_staleness_deadline_are_due() {
    let (_directory, store) = store();
    remember(&store, &entry("fresh", 1_788_000_600, Tier::Hot));
    remember(&store, &entry("stale", 1_788_000_000, Tier::Warm));
    match store.due_for_refresh(1_788_000_100, 10) {
        Ok(due) => {
            assert_eq!(due.len(), 1);
            assert_eq!(due[0].key, "stale");
        }
        other => panic!("the due list must be readable, got {other:?}"),
    }
}

#[test]
fn the_hottest_resource_is_refreshed_first() {
    let (_directory, store) = store();
    remember(&store, &entry("cold", 1_788_000_000, Tier::Cold));
    remember(&store, &entry("warm", 1_788_000_000, Tier::Warm));
    remember(&store, &entry("hot", 1_788_000_000, Tier::Hot));
    match store.due_for_refresh(1_788_000_100, 10) {
        Ok(due) => {
            let order: Vec<&str> = due.iter().map(|entry| entry.key.as_str()).collect();
            assert_eq!(order, vec!["hot", "warm", "cold"]);
        }
        other => panic!("the due list must be readable, got {other:?}"),
    }
}

#[test]
fn the_due_list_never_exceeds_the_limit_it_is_given() {
    let (_directory, store) = store();
    for index in 0..5 {
        remember(
            &store,
            &entry(&format!("pr:{index}"), 1_788_000_000, Tier::Warm),
        );
    }
    match store.due_for_refresh(1_788_000_100, 2) {
        Ok(due) => assert_eq!(due.len(), 2),
        other => panic!("the due list must be readable, got {other:?}"),
    }
}

#[test]
fn forgetting_a_resource_removes_it_and_forgetting_it_again_is_harmless() {
    let (_directory, store) = store();
    remember(&store, &entry("pr:PR_1", 1_788_000_000, Tier::Warm));
    for _ in 0..2 {
        if let Err(error) = store.forget_freshness("pr:PR_1") {
            panic!("forgetting must be harmless: {error}");
        }
    }
    assert!(matches!(store.freshness("pr:PR_1"), Ok(None)));
}

#[test]
fn the_default_store_is_safe_against_a_process_crash_not_against_a_power_loss() {
    let (_directory, store) = store();
    assert_eq!(store.durability(), quay_store::Durability::ProcessCrashSafe);
    let synchronous: i64 = match store
        .connection()
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
    {
        Ok(value) => value,
        Err(error) => panic!("the pragma must be readable: {error}"),
    };
    assert_eq!(synchronous, 1, "synchronous NORMAL is the level 1");
}

#[test]
fn asking_for_power_loss_safety_turns_on_both_pragmas_it_needs() {
    let directory = match tempfile::tempdir() {
        Ok(directory) => directory,
        Err(error) => panic!("temporary directory: {error}"),
    };
    let store = match Store::open_with(
        &directory.path().join("quay.db"),
        quay_store::Durability::PowerLossSafe,
    ) {
        Ok(store) => store,
        Err(error) => panic!("the store must open: {error}"),
    };
    let read = |pragma: &str| -> i64 {
        match store
            .connection()
            .query_row(&format!("PRAGMA {pragma}"), [], |row| row.get(0))
        {
            Ok(value) => value,
            Err(error) => panic!("the pragma must be readable: {error}"),
        }
    };
    assert_eq!(read("synchronous"), 2, "synchronous FULL is the level 2");
    assert_eq!(
        read("fullfsync"),
        1,
        "without fullfsync a macOS fsync returns before the drive has written"
    );
}
