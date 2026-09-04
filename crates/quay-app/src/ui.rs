use std::sync::Mutex;
use std::time::{Duration, Instant};

use quay_forge::{Capabilities, PollingCheckpoint, PollingSource, SearchSource};
use quay_store::Store;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::ipc::{self, CommandEntry, InboxEntry, KeyBinding, PullRequestEntry, ViewEntry};
use crate::paths;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    pub phase: String,
    pub detail: String,
    pub healthy: bool,
}

pub struct Desktop {
    account_id: Mutex<i64>,
    catalogue: crate::i18n::Catalogue,
    started: Instant,
    first_paint: Mutex<Option<f64>>,
    store: Mutex<Store>,
    capabilities: Mutex<Capabilities>,
    viewer: Mutex<String>,
    sync: Mutex<SyncState>,
}

impl SyncState {
    fn starting() -> Self {
        Self {
            phase: "starting".to_owned(),
            detail: "démarrage".to_owned(),
            healthy: true,
        }
    }
}

impl Desktop {
    fn with_store<T>(&self, read: impl FnOnce(&Store) -> T) -> T {
        match self.store.lock() {
            Ok(store) => read(&store),
            Err(poisoned) => read(&poisoned.into_inner()),
        }
    }

    fn with_store_mut<T>(&self, write: impl FnOnce(&mut Store) -> T) -> T {
        match self.store.lock() {
            Ok(mut store) => write(&mut store),
            Err(poisoned) => write(&mut poisoned.into_inner()),
        }
    }

    fn with_capabilities<T>(&self, read: impl FnOnce(&Capabilities) -> T) -> T {
        match self.capabilities.lock() {
            Ok(capabilities) => read(&capabilities),
            Err(poisoned) => read(&poisoned.into_inner()),
        }
    }

    fn account(&self) -> i64 {
        match self.account_id.lock() {
            Ok(identifier) => *identifier,
            Err(poisoned) => *poisoned.into_inner(),
        }
    }

    fn adopt_account(&self, identifier: i64) {
        match self.account_id.lock() {
            Ok(mut held) => *held = identifier,
            Err(poisoned) => *poisoned.into_inner() = identifier,
        }
    }

    fn viewer(&self) -> String {
        match self.viewer.lock() {
            Ok(viewer) => viewer.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn adopt(&self, capabilities: Capabilities, viewer: String) {
        match self.capabilities.lock() {
            Ok(mut held) => *held = capabilities,
            Err(poisoned) => *poisoned.into_inner() = capabilities,
        }
        if viewer.is_empty() {
            return;
        }
        match self.viewer.lock() {
            Ok(mut held) => *held = viewer,
            Err(poisoned) => *poisoned.into_inner() = viewer,
        }
    }
}

fn announce(application: &AppHandle, phase: &str, detail: String, healthy: bool) {
    let state = SyncState {
        phase: phase.to_owned(),
        detail,
        healthy,
    };
    let desktop = application.state::<Desktop>();
    match desktop.sync.lock() {
        Ok(mut held) => *held = state.clone(),
        Err(poisoned) => *poisoned.into_inner() = state.clone(),
    }
    let _ = application.emit("sync_state", state);
}

#[tauri::command]
fn key_map(scope: String, desktop: State<'_, Desktop>) -> Vec<KeyBinding> {
    desktop.with_capabilities(|capabilities| ipc::key_map(ipc::scope_of(&scope), capabilities))
}

#[tauri::command]
fn palette(needle: String, scope: String, desktop: State<'_, Desktop>) -> Vec<CommandEntry> {
    desktop.with_capabilities(|capabilities| {
        ipc::palette(
            &needle,
            ipc::scope_of(&scope),
            capabilities,
            &desktop.catalogue,
        )
    })
}

#[tauri::command]
fn catalogue(desktop: State<'_, Desktop>) -> std::collections::BTreeMap<String, String> {
    desktop.catalogue.entries().clone()
}

#[tauri::command]
fn locale(desktop: State<'_, Desktop>) -> String {
    desktop.catalogue.locale().to_owned()
}

#[tauri::command]
fn saved_views(desktop: State<'_, Desktop>) -> Result<Vec<ViewEntry>, String> {
    desktop.with_store(ipc::saved_views)
}

#[tauri::command]
fn run_view(name: String, desktop: State<'_, Desktop>) -> Result<Vec<InboxEntry>, String> {
    let viewer = desktop.viewer();
    let account = desktop.account();
    desktop.with_store(|store| {
        let scope = store.selected_organization(account).ok().flatten();
        ipc::run_view(store, &name, &viewer, scope.as_deref())
    })
}

#[tauri::command]
fn run_query(dsl: String, desktop: State<'_, Desktop>) -> Result<Vec<InboxEntry>, String> {
    let viewer = desktop.viewer();
    desktop.with_store(|store| ipc::run_query(store, &dsl, &viewer))
}

#[tauri::command]
fn pull_request(
    key: String,
    desktop: State<'_, Desktop>,
) -> Result<Option<PullRequestEntry>, String> {
    desktop.with_store(|store| ipc::pull_request(store, &key))
}

#[tauri::command]
fn bundle_loaded(desktop: State<'_, Desktop>) {
    tracing::info!(
        elapsed_ms = desktop.started.elapsed().as_secs_f64() * 1_000.0,
        "frontend bundle running"
    );
}

#[tauri::command]
fn mark(phase: String, desktop: State<'_, Desktop>) {
    tracing::info!(
        elapsed_ms = desktop.started.elapsed().as_secs_f64() * 1_000.0,
        "{phase}"
    );
}

#[tauri::command]
fn first_paint(desktop: State<'_, Desktop>) -> f64 {
    let elapsed = desktop.started.elapsed().as_secs_f64() * 1_000.0;
    tracing::info!(elapsed_ms = elapsed, "first paint");
    match desktop.first_paint.lock() {
        Ok(mut held) => *held.get_or_insert(elapsed),
        Err(poisoned) => *poisoned.into_inner().get_or_insert(elapsed),
    }
}

#[tauri::command]
fn sync_state(desktop: State<'_, Desktop>) -> SyncState {
    match desktop.sync.lock() {
        Ok(state) => state.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

#[tauri::command]
fn sync_now(application: AppHandle) {
    tauri::async_runtime::spawn(async move {
        run_one_cycle(&application).await;
    });
}

#[tauri::command]
fn organizations(desktop: State<'_, Desktop>) -> Result<Vec<ipc::ScopeEntry>, String> {
    let account = desktop.account();
    desktop.with_store(|store| ipc::organizations(store, account))
}

#[tauri::command]
fn selected_organization(desktop: State<'_, Desktop>) -> Option<String> {
    let account = desktop.account();
    desktop.with_store(|store| store.selected_organization(account).ok().flatten())
}

#[tauri::command]
fn select_organization(login: Option<String>, desktop: State<'_, Desktop>) -> Result<(), String> {
    let account = desktop.account();
    desktop.with_store(|store| {
        store
            .select_organization(account, login.as_deref())
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
fn view_query(name: String, desktop: State<'_, Desktop>) -> Result<String, String> {
    let account = desktop.account();
    desktop.with_store(|store| {
        let scope = store.selected_organization(account).ok().flatten();
        ipc::view_query(store, &name, scope.as_deref())
    })
}

#[tauri::command]
fn queued(desktop: State<'_, Desktop>) -> Result<Vec<ipc::QueuedMutation>, String> {
    desktop.with_store(ipc::queued)
}

#[tauri::command]
fn approve(key: String, desktop: State<'_, Desktop>) -> Result<i64, String> {
    let now = now_seconds();
    desktop.with_store_mut(|store| ipc::approve(store, &key, now))
}

#[tauri::command]
fn request_changes(key: String, desktop: State<'_, Desktop>) -> Result<i64, String> {
    let now = now_seconds();
    desktop.with_store_mut(|store| ipc::request_changes(store, &key, now))
}

#[tauri::command]
fn merge(key: String, desktop: State<'_, Desktop>) -> Result<i64, String> {
    let now = now_seconds();
    desktop.with_store_mut(|store| ipc::merge(store, &key, now))
}

#[tauri::command]
fn resolve_thread(node_id: String, desktop: State<'_, Desktop>) -> Result<i64, String> {
    let now = now_seconds();
    desktop.with_store_mut(|store| ipc::resolve_thread(store, &node_id, now))
}

#[tauri::command]
fn cancel(id: i64, desktop: State<'_, Desktop>) -> Result<(), String> {
    desktop.with_store_mut(|store| ipc::cancel(store, id))
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
fn completions(partial: String) -> Vec<String> {
    quay_core::completions(&partial)
        .into_iter()
        .map(|completion| completion.insertion)
        .collect()
}

fn account_of(store: &Store, viewer: &str) -> i64 {
    if viewer.is_empty() {
        return 0;
    }
    store
        .remember_account(
            "api.github.com",
            viewer,
            "pat_classic",
            &format!("quay/{viewer}"),
        )
        .unwrap_or(0)
}

pub fn serves_embedded_frontend() -> bool {
    !cfg!(dev)
}

async fn run_one_cycle(application: &AppHandle) {
    let desktop = application.state::<Desktop>();
    let path = desktop.with_store(|store| store.path().to_path_buf());
    let viewer = desktop.viewer();

    let token = match crate::session::token_for_login(if viewer.is_empty() {
        None
    } else {
        Some(viewer.as_str())
    }) {
        Ok(token) => token,
        Err(error) => {
            announce(application, "idle", error.to_string(), false);
            return;
        }
    };
    let kind = token.kind();
    let governor = match crate::session::governor_for(token) {
        Ok(governor) => governor,
        Err(error) => {
            announce(application, "idle", error.to_string(), false);
            return;
        }
    };

    announce(
        application,
        "identifying",
        "identification".to_owned(),
        true,
    );
    let identity = match crate::session::identify(&governor, kind).await {
        Ok(identity) => identity,
        Err(error) => {
            announce(application, "offline", error.to_string(), false);
            return;
        }
    };
    let login = identity.login.clone();
    desktop.adopt(Capabilities::from_identity(&identity), login.clone());
    let memberships: Vec<(String, Option<String>)> = identity
        .organizations
        .iter()
        .map(|organization| (organization.clone(), None))
        .collect();
    let _ = application.emit("capabilities_changed", ());

    let mut engine = {
        let store = match Store::open(&path) {
            Ok(store) => store,
            Err(error) => {
                announce(application, "offline", error.to_string(), false);
                return;
            }
        };
        let account_id = match store.remember_account(
            "api.github.com",
            &login,
            "pat_classic",
            &format!("quay/{login}"),
        ) {
            Ok(identifier) => identifier,
            Err(error) => {
                announce(application, "offline", error.to_string(), false);
                return;
            }
        };
        desktop.adopt_account(account_id);
        let mut store = store;
        let _ = store.remember_organizations(account_id, &memberships);
        let checkpoint =
            PollingCheckpoint::holding(quay_sync::notification_validators(&store).ok().flatten());
        quay_sync::SyncEngine::new(store, governor.clone(), crate::session::API, account_id)
            .listening_to(Box::new(PollingSource::sharing(
                governor.clone(),
                crate::session::API,
                checkpoint,
            )))
            .listening_to(Box::new(SearchSource::review_queue(
                governor.clone(),
                crate::session::API,
            )))
    };

    announce(application, "syncing", "rafraîchissement".to_owned(), true);
    let stored = match engine.tick().await {
        Ok(report) => report.stored,
        Err(error) => {
            announce(application, "offline", error.to_string(), false);
            return;
        }
    };
    let preloaded = engine
        .preload(&login)
        .await
        .map(|report| report.fetched)
        .unwrap_or(0);

    if stored + preloaded > 0 {
        let _ = application.emit("inbox_changed", ());
    }
    announce(
        application,
        "idle",
        format!(
            "{} req · {} pts · {} gratuite(s)",
            governor.issued_requests(),
            governor.spent_points(),
            governor.free_revalidations()
        ),
        true,
    );
}

fn spawn_sync_loop(application: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            run_one_cycle(&application).await;
        }
    });
}

pub fn run(started: Instant) -> Result<(), Box<dyn std::error::Error>> {
    let path = paths::database();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let store = Store::open(&path)?;
    tracing::info!(
        elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0,
        "store opened"
    );
    let viewer = crate::session::stored_login(&store)?.unwrap_or_default();
    let account_id = account_of(&store, &viewer);
    let capabilities = Capabilities::from_scopes(
        quay_forge::TokenKind::Unrecognised,
        &quay_forge::GrantedScopes::default(),
    );

    tracing::info!(
        elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0,
        "builder about to run"
    );
    tauri::Builder::default()
        .setup(move |application| {
            application.manage(Desktop {
                store: Mutex::new(store),
                capabilities: Mutex::new(capabilities),
                viewer: Mutex::new(viewer),
                sync: Mutex::new(SyncState::starting()),
                account_id: Mutex::new(account_id),
                catalogue: crate::i18n::Catalogue::for_locale(&crate::i18n::preferred_locale()),
                started,
                first_paint: Mutex::new(None),
            });
            tracing::info!(
                elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0,
                "runtime ready, creating the window"
            );
            tauri::WebviewWindowBuilder::new(
                application,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("Quay")
            .inner_size(1280.0, 820.0)
            .min_inner_size(720.0, 480.0)
            .build()?;
            tracing::info!(
                elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0,
                "first window built"
            );
            spawn_sync_loop(application.handle().clone());
            tracing::info!(
                elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0,
                "setup finished"
            );
            Ok(())
        })
        .on_page_load(move |window, _| {
            tracing::info!(
                elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0,
                "page loaded in {}",
                window.label()
            );
        })
        .invoke_handler(tauri::generate_handler![
            sync_now,
            sync_state,
            first_paint,
            bundle_loaded,
            mark,
            key_map,
            palette,
            catalogue,
            locale,
            saved_views,
            organizations,
            selected_organization,
            select_organization,
            view_query,
            run_view,
            run_query,
            pull_request,
            queued,
            approve,
            request_changes,
            merge,
            resolve_thread,
            cancel,
            completions
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}
