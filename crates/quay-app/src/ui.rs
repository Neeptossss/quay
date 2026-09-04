use std::sync::Mutex;

use quay_forge::Capabilities;
use quay_store::Store;
use tauri::{Manager, State};

use crate::ipc::{self, CommandEntry, InboxEntry, KeyBinding, PullRequestEntry, ViewEntry};
use crate::paths;

pub struct Desktop {
    store: Mutex<Store>,
    capabilities: Mutex<Capabilities>,
    viewer: Mutex<String>,
}

impl Desktop {
    fn with_store<T>(&self, read: impl FnOnce(&Store) -> T) -> T {
        match self.store.lock() {
            Ok(store) => read(&store),
            Err(poisoned) => read(&poisoned.into_inner()),
        }
    }

    fn with_capabilities<T>(&self, read: impl FnOnce(&Capabilities) -> T) -> T {
        match self.capabilities.lock() {
            Ok(capabilities) => read(&capabilities),
            Err(poisoned) => read(&poisoned.into_inner()),
        }
    }

    fn viewer(&self) -> String {
        match self.viewer.lock() {
            Ok(viewer) => viewer.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

#[tauri::command]
fn key_map(scope: String, desktop: State<'_, Desktop>) -> Vec<KeyBinding> {
    desktop.with_capabilities(|capabilities| ipc::key_map(ipc::scope_of(&scope), capabilities))
}

#[tauri::command]
fn palette(needle: String, scope: String, desktop: State<'_, Desktop>) -> Vec<CommandEntry> {
    desktop.with_capabilities(|capabilities| {
        ipc::palette(&needle, ipc::scope_of(&scope), capabilities)
    })
}

#[tauri::command]
fn saved_views(desktop: State<'_, Desktop>) -> Result<Vec<ViewEntry>, String> {
    desktop.with_store(ipc::saved_views)
}

#[tauri::command]
fn run_view(name: String, desktop: State<'_, Desktop>) -> Result<Vec<InboxEntry>, String> {
    let viewer = desktop.viewer();
    desktop.with_store(|store| ipc::run_view(store, &name, &viewer))
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
fn completions(partial: String) -> Vec<String> {
    quay_core::completions(&partial)
        .into_iter()
        .map(|completion| completion.insertion)
        .collect()
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let path = paths::database();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let store = Store::open(&path)?;
    let viewer = crate::session::stored_login(&store)?.unwrap_or_default();
    let capabilities = Capabilities::from_scopes(
        quay_forge::TokenKind::Unrecognised,
        &quay_forge::GrantedScopes::default(),
    );

    tauri::Builder::default()
        .setup(move |application| {
            application.manage(Desktop {
                store: Mutex::new(store),
                capabilities: Mutex::new(capabilities),
                viewer: Mutex::new(viewer),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            key_map,
            palette,
            saved_views,
            run_view,
            run_query,
            pull_request,
            completions
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}
