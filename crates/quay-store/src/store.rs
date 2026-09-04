use std::path::{Path, PathBuf};

use rusqlite::Connection;

use quay_core::{Mutation, MutationState};

use crate::error::StoreError;
use crate::inbox::{InboxFilter, InboxQuery, InboxRow};
use crate::migrations::{self, MIGRATIONS};
use crate::mutations::{self, ReplayReport};
use crate::optimistic;
use crate::resource_cache::{self, CacheEntry};
use crate::schema;
use crate::write;
use quay_core::PullRequestSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Durability {
    ProcessCrashSafe,
    PowerLossSafe,
}

impl Durability {
    pub const ALL: [Durability; 2] = [Durability::ProcessCrashSafe, Durability::PowerLossSafe];

    pub fn id(self) -> &'static str {
        match self {
            Durability::ProcessCrashSafe => "process_crash_safe",
            Durability::PowerLossSafe => "power_loss_safe",
        }
    }

    pub fn synchronous(self) -> &'static str {
        match self {
            Durability::ProcessCrashSafe => "NORMAL",
            Durability::PowerLossSafe => "FULL",
        }
    }

    pub fn flushes_the_drive_cache(self) -> bool {
        self == Durability::PowerLossSafe
    }
}

pub struct Store {
    connection: Connection,
    path: PathBuf,
    durability: Durability,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::open_with(path, Durability::ProcessCrashSafe)
    }

    pub fn open_with(path: &Path, durability: Durability) -> Result<Self, StoreError> {
        let mut connection = schema::open(path)?;
        migrations::apply(&mut connection, path, &MIGRATIONS)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "synchronous", durability.synchronous())?;
        connection.pragma_update(None, "fullfsync", durability.flushes_the_drive_cache())?;
        Ok(Self {
            connection,
            path: path.to_path_buf(),
            durability,
        })
    }

    pub fn durability(&self) -> Durability {
        self.durability
    }

    pub fn approve_pull_request(
        &mut self,
        pull_request_node_id: &str,
        idempotency: &str,
        now: i64,
    ) -> Result<i64, StoreError> {
        optimistic::approve_pull_request(
            &mut self.connection,
            pull_request_node_id,
            idempotency,
            now,
        )
    }

    pub fn resolve_thread(
        &mut self,
        thread_node_id: &str,
        idempotency: &str,
        now: i64,
    ) -> Result<i64, StoreError> {
        optimistic::resolve_thread(&mut self.connection, thread_node_id, idempotency, now)
    }

    pub fn roll_back_mutation(&mut self, mutation_id: i64, reason: &str) -> Result<(), StoreError> {
        optimistic::roll_back(&mut self.connection, mutation_id, reason)
    }

    pub fn claim_next_mutation(&mut self) -> Result<Option<Mutation>, StoreError> {
        mutations::claim_next(&mut self.connection)
    }

    pub fn find_pull_request(
        &self,
        owner: &str,
        name: &str,
        number: i64,
    ) -> Result<Option<String>, StoreError> {
        let mut statement = self.connection.prepare_cached(
            "SELECT pr.node_id FROM pull_request pr JOIN repo r ON r.id = pr.repo_id
             WHERE r.owner = ?1 AND r.name = ?2 AND pr.number = ?3",
        )?;
        let mut rows = statement.query_map((owner, name, number), |row| row.get::<_, String>(0))?;
        match rows.next() {
            None => Ok(None),
            Some(node_id) => Ok(Some(node_id?)),
        }
    }

    pub fn locate_pull_request(
        &self,
        node_id: &str,
    ) -> Result<Option<(String, String, i64)>, StoreError> {
        let mut statement = self.connection.prepare_cached(
            "SELECT r.owner, r.name, pr.number FROM pull_request pr
             JOIN repo r ON r.id = pr.repo_id WHERE pr.node_id = ?1",
        )?;
        let mut rows = statement.query_map((node_id,), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        match rows.next() {
            None => Ok(None),
            Some(located) => Ok(Some(located?)),
        }
    }

    pub fn locate_thread_pull_request(
        &self,
        thread_node_id: &str,
    ) -> Result<Option<(String, String, i64)>, StoreError> {
        let mut statement = self.connection.prepare_cached(
            "SELECT r.owner, r.name, pr.number FROM review_thread t
             JOIN pull_request pr ON pr.id = t.pr_id
             JOIN repo r ON r.id = pr.repo_id WHERE t.node_id = ?1",
        )?;
        let mut rows = statement.query_map((thread_node_id,), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        match rows.next() {
            None => Ok(None),
            Some(located) => Ok(Some(located?)),
        }
    }

    pub fn mutation(&self, id: i64) -> Result<Option<Mutation>, StoreError> {
        mutations::get(&self.connection, id)
    }

    pub fn mutations_in_state(&self, state: MutationState) -> Result<Vec<Mutation>, StoreError> {
        mutations::by_state(&self.connection, state)
    }

    pub fn settle_mutation(&self, id: i64) -> Result<(), StoreError> {
        mutations::mark_done(&self.connection, id)
    }

    pub fn defer_mutation(&self, id: i64, reason: &str) -> Result<(), StoreError> {
        mutations::return_to_queue(&self.connection, id, reason)
    }

    pub fn park_mutation(&self, id: i64) -> Result<(), StoreError> {
        mutations::park(&self.connection, id)
    }

    pub fn replay_interrupted_mutations(&mut self) -> Result<ReplayReport, StoreError> {
        mutations::replay_interrupted(&mut self.connection)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn schema_version(&self) -> Result<i64, StoreError> {
        migrations::current_version(&self.connection)
    }

    pub fn inbox(
        &self,
        query: InboxQuery,
        filter: &InboxFilter,
    ) -> Result<Vec<InboxRow>, StoreError> {
        crate::inbox::run(&self.connection, query, filter)
    }

    pub fn freshness(&self, key: &str) -> Result<Option<CacheEntry>, StoreError> {
        resource_cache::get(&self.connection, key)
    }

    pub fn remember_freshness(&self, entry: &CacheEntry) -> Result<(), StoreError> {
        resource_cache::put(&self.connection, entry)
    }

    pub fn due_for_refresh(&self, now: i64, limit: i64) -> Result<Vec<CacheEntry>, StoreError> {
        resource_cache::due(&self.connection, now, limit)
    }

    pub fn forget_freshness(&self, key: &str) -> Result<(), StoreError> {
        resource_cache::forget(&self.connection, key)
    }

    pub fn remember_account(
        &self,
        host: &str,
        login: &str,
        auth_kind: &str,
        keychain_ref: &str,
    ) -> Result<i64, StoreError> {
        write::upsert_account(&self.connection, host, login, auth_kind, keychain_ref)
    }

    pub fn save_pull_request(
        &mut self,
        account_id: i64,
        snapshot: &PullRequestSnapshot,
    ) -> Result<i64, StoreError> {
        write::save_snapshot(&mut self.connection, account_id, snapshot)
    }

    pub fn search(
        &self,
        query: &quay_core::Query,
        viewer: &str,
        limit: i64,
    ) -> Result<Vec<crate::inbox::InboxRow>, StoreError> {
        let compiled = crate::compile::compile(query, viewer, limit)?;
        let mut statement = self.connection.prepare_cached(&compiled.sql)?;
        Ok(statement
            .query_map(
                rusqlite::params_from_iter(compiled.parameters.iter()),
                crate::inbox::read_row,
            )?
            .collect::<Result<Vec<crate::inbox::InboxRow>, rusqlite::Error>>()?)
    }

    pub fn views(&self) -> Result<Vec<crate::views::SavedView>, StoreError> {
        crate::views::all(&self.connection)
    }

    pub fn view(&self, name: &str) -> Result<Option<crate::views::SavedView>, StoreError> {
        crate::views::by_name(&self.connection, name)
    }

    pub fn install_shipped_views(&self) -> Result<usize, StoreError> {
        crate::views::install_shipped_if_empty(&self.connection)
    }

    pub fn pull_request_view(
        &self,
        owner: &str,
        name: &str,
        number: i64,
    ) -> Result<Option<crate::detail::PullRequestView>, StoreError> {
        crate::detail::pull_request(&self.connection, owner, name, number)
    }

    pub fn note_speculation(
        &self,
        key: &str,
        reason: quay_core::PreloadReason,
        at: i64,
    ) -> Result<i64, StoreError> {
        crate::preload::record_speculation(&self.connection, key, reason, at)
    }

    pub fn note_navigation(
        &self,
        key: &str,
        served_locally: bool,
        at: i64,
    ) -> Result<i64, StoreError> {
        crate::preload::record_navigation(&self.connection, key, served_locally, at)
    }

    pub fn note_speculation_used(&self, key: &str) -> Result<bool, StoreError> {
        crate::preload::mark_speculation_used(&self.connection, key)
    }

    pub fn speculation_utilisation(
        &self,
        window: usize,
    ) -> Result<crate::preload::Utilisation, StoreError> {
        crate::preload::speculation_utilisation(&self.connection, window)
    }

    pub fn navigations_served_locally(
        &self,
        window: usize,
    ) -> Result<crate::preload::Utilisation, StoreError> {
        crate::preload::navigations_served_locally(&self.connection, window)
    }

    pub fn note_navigation_event(
        &self,
        from_state: &str,
        to_state: &str,
        action: &str,
        entity_kind: Option<&str>,
        dwell_ms: Option<i64>,
        at: i64,
    ) -> Result<(), StoreError> {
        crate::preload::record_navigation_event(
            &self.connection,
            from_state,
            to_state,
            action,
            entity_kind,
            dwell_ms,
            at,
        )
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}
