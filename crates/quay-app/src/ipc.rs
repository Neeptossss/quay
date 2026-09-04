use quay_forge::Capabilities;
use quay_store::Store;
use quay_store::inbox::InboxRow;
use serde::Serialize;

use crate::commands::{self, Scope};

pub const RESULT_LIMIT: i64 = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyBinding {
    pub chord: String,
    pub command: String,
    pub title_key: String,
    pub icon: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEntry {
    pub id: String,
    pub title_key: String,
    pub icon: String,
    pub scope: String,
    pub bindings: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxEntry {
    pub key: String,
    pub owner: String,
    pub name: String,
    pub number: i64,
    pub title: String,
    pub author: String,
    pub is_draft: bool,
    pub review_state: Option<String>,
    pub checks_state: Option<String>,
    pub unresolved_threads: i64,
    pub updated_at: String,
}

impl From<&InboxRow> for InboxEntry {
    fn from(row: &InboxRow) -> Self {
        Self {
            key: format!("{}/{}#{}", row.owner, row.name, row.number),
            owner: row.owner.clone(),
            name: row.name.clone(),
            number: row.number,
            title: row.title.clone(),
            author: row.author.clone(),
            is_draft: row.is_draft,
            review_state: row.review_state.clone(),
            checks_state: row.checks_state.clone(),
            unresolved_threads: row.unresolved_threads,
            updated_at: row.updated_at.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewEntry {
    pub name: String,
    pub query: String,
    pub shortcut: Option<String>,
    pub position: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentEntry {
    pub author: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadEntry {
    pub path: String,
    pub line: Option<i64>,
    pub is_resolved: bool,
    pub is_outdated: bool,
    pub comments: Vec<CommentEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestEntry {
    pub key: String,
    pub owner: String,
    pub name: String,
    pub number: i64,
    pub title: String,
    pub author: String,
    pub is_draft: bool,
    pub state: String,
    pub review_state: Option<String>,
    pub checks_state: Option<String>,
    pub unresolved_threads: i64,
    pub updated_at: String,
    pub threads: Vec<ThreadEntry>,
}

pub fn pull_request(store: &Store, key: &str) -> Result<Option<PullRequestEntry>, String> {
    let (repository, number) = key
        .rsplit_once('#')
        .ok_or_else(|| format!("clé illisible : {key}"))?;
    let (owner, name) = repository
        .split_once('/')
        .ok_or_else(|| format!("clé illisible : {key}"))?;
    let number: i64 = number
        .parse()
        .map_err(|_| format!("clé illisible : {key}"))?;

    let found = store
        .pull_request_view(owner, name, number)
        .map_err(|error| error.to_string())?;
    Ok(found.map(|view| PullRequestEntry {
        key: key.to_owned(),
        unresolved_threads: view.unresolved_threads() as i64,
        owner: view.owner,
        name: view.name,
        number: view.number,
        title: view.title,
        author: view.author,
        is_draft: view.is_draft,
        state: view.state,
        review_state: view.review_state,
        checks_state: view.checks_state,
        updated_at: view.updated_at,
        threads: view
            .threads
            .into_iter()
            .map(|thread| ThreadEntry {
                path: thread.path,
                line: thread.line,
                is_resolved: thread.is_resolved,
                is_outdated: thread.is_outdated,
                comments: thread
                    .comments
                    .into_iter()
                    .map(|comment| CommentEntry {
                        author: comment.author,
                        body: comment.body,
                        created_at: comment.created_at,
                    })
                    .collect(),
            })
            .collect(),
    }))
}

pub fn scope_of(id: &str) -> Scope {
    Scope::ALL
        .into_iter()
        .find(|scope| scope.id() == id)
        .unwrap_or(Scope::Global)
}

pub fn key_map(scope: Scope, capabilities: &Capabilities) -> Vec<KeyBinding> {
    let mut bindings = Vec::new();
    for command in commands::available_in(scope, capabilities) {
        for chord in command.chords() {
            let rendered = chord.render();
            if bindings
                .iter()
                .any(|kept: &KeyBinding| kept.chord == rendered)
            {
                continue;
            }
            bindings.push(KeyBinding {
                chord: rendered,
                command: command.id.to_owned(),
                title_key: command.title_key(),
                icon: command.icon.to_owned(),
                enabled: command.is_enabled(capabilities),
            });
        }
    }
    bindings
}

pub fn palette(
    needle: &str,
    scope: Scope,
    capabilities: &Capabilities,
    catalogue: &crate::i18n::Catalogue,
) -> Vec<CommandEntry> {
    commands::rank(needle, scope, capabilities, catalogue)
        .into_iter()
        .filter_map(|found| {
            commands::registry()
                .into_iter()
                .find(|command| command.id == found.id)
                .map(|command| CommandEntry {
                    id: command.id.to_owned(),
                    title_key: command.title_key(),
                    icon: command.icon.to_owned(),
                    scope: command.scope.id().to_owned(),
                    bindings: command
                        .chords()
                        .iter()
                        .map(crate::keys::KeyChord::render)
                        .collect(),
                    enabled: command.is_enabled(capabilities),
                })
        })
        .collect()
}

pub fn saved_views(store: &Store) -> Result<Vec<ViewEntry>, String> {
    store
        .install_shipped_views()
        .map_err(|error| error.to_string())?;
    Ok(store
        .views()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|view| ViewEntry {
            name: view.name,
            query: view.query,
            shortcut: view.shortcut,
            position: view.position,
        })
        .collect())
}

pub fn run_query(store: &Store, dsl: &str, viewer: &str) -> Result<Vec<InboxEntry>, String> {
    let parsed = quay_core::parse_query(dsl).map_err(|error| crate::render::query_error(&error))?;
    let rows = store
        .search(&parsed, viewer, RESULT_LIMIT)
        .map_err(|error| crate::render::store_error(&error).unwrap_or_else(|| error.to_string()))?;
    Ok(rows.iter().map(InboxEntry::from).collect())
}

pub fn run_view(store: &Store, name: &str, viewer: &str) -> Result<Vec<InboxEntry>, String> {
    let view = store
        .view(name)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("aucune vue nommée « {name} »"))?;
    run_query(store, &view.query, viewer)
}

#[cfg(test)]
mod tests {
    use quay_forge::{Capabilities, Capability, GrantedScopes, TokenKind};
    use quay_store::Store;

    use super::{key_map, palette, run_query, run_view, saved_views};
    use crate::commands::Scope;
    use crate::i18n::Catalogue;

    fn capabilities(scopes: &str) -> Capabilities {
        Capabilities::from_scopes(TokenKind::PatClassic, &GrantedScopes::parse(scopes))
    }

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

    #[test]
    fn the_key_map_hands_the_frontend_a_resolved_chord_to_command_table() {
        let map = key_map(
            Scope::ReviewPanel,
            &capabilities("repo, notifications, read:org, workflow"),
        );
        let commenting = map.iter().find(|binding| binding.chord == "c");
        match commenting {
            Some(binding) => assert_eq!(
                binding.command, "review.request_changes",
                "the innermost scope must already have won before the frontend sees it"
            ),
            None => panic!("`c` must be in the review panel map"),
        }
    }

    #[test]
    fn the_key_map_never_hands_the_same_chord_twice() {
        for scope in Scope::ALL {
            let map = key_map(
                scope,
                &capabilities("repo, notifications, read:org, workflow"),
            );
            let mut chords: Vec<&str> = map.iter().map(|binding| binding.chord.as_str()).collect();
            let handed = chords.len();
            chords.sort_unstable();
            chords.dedup();
            assert_eq!(chords.len(), handed, "scope {}", scope.id());
        }
    }

    #[test]
    fn a_command_blocked_by_an_organization_policy_never_reaches_the_frontend() {
        let mut capabilities = capabilities("repo, notifications, read:org, workflow");
        capabilities.observe_failure(Capability::Merge, 403, "Organization policy blocks this");
        let map = key_map(Scope::PullRequest, &capabilities);
        assert!(!map.iter().any(|binding| binding.command == "pr.merge"));
        assert!(
            !palette(
                "",
                Scope::PullRequest,
                &capabilities,
                &Catalogue::for_locale("fr")
            )
            .iter()
            .any(|entry| entry.id == "pr.merge")
        );
    }

    #[test]
    fn a_command_the_user_can_recover_reaches_the_frontend_marked_disabled() {
        let capabilities = capabilities("repo, notifications, read:org");
        let entry = palette(
            "",
            Scope::PullRequest,
            &capabilities,
            &Catalogue::for_locale("fr"),
        )
        .into_iter()
        .find(|entry| entry.id == "pr.merge");
        match entry {
            Some(entry) => assert!(entry.enabled),
            None => panic!("merge stays discoverable with the repo scope granted"),
        }
    }

    #[test]
    fn the_palette_carries_the_shortcut_of_each_command_so_it_can_teach_it() {
        let entries = palette(
            "merg",
            Scope::PullRequest,
            &capabilities("repo"),
            &Catalogue::for_locale("fr"),
        );
        match entries.first() {
            Some(entry) => {
                assert_eq!(entry.id, "pr.merge");
                assert_eq!(entry.bindings, vec!["m".to_owned()]);
            }
            None => panic!("merging must be found"),
        }
    }

    #[test]
    fn the_shipped_views_reach_the_frontend_with_their_shortcuts() {
        let (_directory, store) = store();
        let views = match saved_views(&store) {
            Ok(views) => views,
            Err(error) => panic!("the views must load: {error}"),
        };
        assert!(
            views
                .iter()
                .any(|view| view.shortcut.as_deref() == Some("g r"))
        );
    }

    #[test]
    fn an_empty_store_answers_a_view_with_no_entry_rather_than_an_error() {
        let (_directory, store) = store();
        if let Err(error) = saved_views(&store) {
            panic!("the views must install: {error}");
        }
        match run_view(&store, "view.to_review", "octocat") {
            Ok(entries) => assert!(entries.is_empty()),
            Err(error) => panic!("an empty view is not an error: {error}"),
        }
    }

    #[test]
    fn a_query_the_user_mistyped_comes_back_as_a_sentence_they_can_act_on() {
        let (_directory, store) = store();
        match run_query(&store, "is:pr auth:@me", "octocat") {
            Err(message) => {
                assert!(message.contains("author"));
                assert!(message.contains("Vouliez-vous"));
            }
            Ok(_) => panic!("a mistyped qualifier must be refused"),
        }
    }

    #[test]
    fn a_qualifier_this_milestone_cannot_answer_says_so_in_the_user_language() {
        let (_directory, store) = store();
        match run_query(&store, "label:bug", "octocat") {
            Err(message) => assert!(message.contains("ne sait pas y répondre")),
            Ok(_) => panic!("an unanswerable qualifier must be refused"),
        }
    }

    #[test]
    fn an_unknown_view_is_named_in_the_refusal() {
        let (_directory, store) = store();
        match run_view(&store, "Néant", "octocat") {
            Err(message) => assert!(message.contains("Néant")),
            Ok(_) => panic!("an unknown view must be refused"),
        }
    }
}
