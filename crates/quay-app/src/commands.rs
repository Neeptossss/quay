use quay_forge::{Capabilities, Capability};

use crate::keys::KeyChord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Global,
    List,
    PullRequest,
    ReviewPanel,
    Diff,
}

impl Scope {
    pub const ALL: [Scope; 5] = [
        Scope::Global,
        Scope::List,
        Scope::PullRequest,
        Scope::ReviewPanel,
        Scope::Diff,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::List => "list",
            Scope::PullRequest => "pull_request",
            Scope::ReviewPanel => "review_panel",
            Scope::Diff => "diff",
        }
    }

    pub fn ancestors(self) -> &'static [Scope] {
        match self {
            Scope::Global => &[Scope::Global],
            Scope::List => &[Scope::List, Scope::Global],
            Scope::PullRequest => &[Scope::PullRequest, Scope::Global],
            Scope::ReviewPanel => &[Scope::ReviewPanel, Scope::PullRequest, Scope::Global],
            Scope::Diff => &[Scope::Diff, Scope::PullRequest, Scope::Global],
        }
    }
}

pub struct Command {
    pub id: &'static str,
    pub icon: &'static str,
    pub bindings: &'static [&'static str],
    pub scope: Scope,
    pub requires: Option<Capability>,
}

pub const REGISTRY: [Command; 32] = [
    Command {
        id: "goto.inbox",
        icon: "inbox",
        bindings: &["g i"],
        scope: Scope::Global,
        requires: None,
    },
    Command {
        id: "goto.my_pull_requests",
        icon: "git-pull-request",
        bindings: &["g p"],
        scope: Scope::Global,
        requires: None,
    },
    Command {
        id: "view.open_nth",
        icon: "layout-list",
        bindings: &[
            "g 1", "g 2", "g 3", "g 4", "g 5", "g 6", "g 7", "g 8", "g 9",
        ],
        scope: Scope::Global,
        requires: None,
    },
    Command {
        id: "palette.open",
        icon: "command",
        bindings: &["⌘k"],
        scope: Scope::Global,
        requires: None,
    },
    Command {
        id: "search.open",
        icon: "search",
        bindings: &["⌘p"],
        scope: Scope::Global,
        requires: None,
    },
    Command {
        id: "help.shortcuts",
        icon: "keyboard",
        bindings: &["?"],
        scope: Scope::Global,
        requires: None,
    },
    Command {
        id: "list.next",
        icon: "arrow-down",
        bindings: &["j"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.previous",
        icon: "arrow-up",
        bindings: &["k"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.next_and_open",
        icon: "corner-down-right",
        bindings: &["J"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.previous_and_open",
        icon: "corner-up-right",
        bindings: &["K"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.first",
        icon: "chevrons-up",
        bindings: &["g g"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.last",
        icon: "chevrons-down",
        bindings: &["G"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.open",
        icon: "square-arrow-out-up-right",
        bindings: &["o", "Enter"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.preview",
        icon: "eye",
        bindings: &["Space"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.select",
        icon: "square-check",
        bindings: &["x"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.archive",
        icon: "archive",
        bindings: &["e"],
        scope: Scope::List,
        requires: Some(Capability::Notifications),
    },
    Command {
        id: "list.mark_unread",
        icon: "mail",
        bindings: &["u"],
        scope: Scope::List,
        requires: Some(Capability::Notifications),
    },
    Command {
        id: "list.filter",
        icon: "funnel",
        bindings: &["/"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.clear",
        icon: "x",
        bindings: &["Esc"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "pr.next_file",
        icon: "file-plus",
        bindings: &["]"],
        scope: Scope::PullRequest,
        requires: None,
    },
    Command {
        id: "pr.previous_file",
        icon: "file-minus",
        bindings: &["["],
        scope: Scope::PullRequest,
        requires: None,
    },
    Command {
        id: "pr.next_hunk",
        icon: "chevron-down",
        bindings: &["}"],
        scope: Scope::Diff,
        requires: None,
    },
    Command {
        id: "pr.previous_hunk",
        icon: "chevron-up",
        bindings: &["{"],
        scope: Scope::Diff,
        requires: None,
    },
    Command {
        id: "pr.next_unresolved",
        icon: "message-circle",
        bindings: &["n"],
        scope: Scope::PullRequest,
        requires: None,
    },
    Command {
        id: "pr.previous_unresolved",
        icon: "message-circle-off",
        bindings: &["p"],
        scope: Scope::PullRequest,
        requires: None,
    },
    Command {
        id: "pr.comment",
        icon: "message-square-plus",
        bindings: &["c"],
        scope: Scope::PullRequest,
        requires: Some(Capability::ReviewSubmit),
    },
    Command {
        id: "pr.reply",
        icon: "reply",
        bindings: &["r"],
        scope: Scope::PullRequest,
        requires: Some(Capability::ReviewSubmit),
    },
    Command {
        id: "pr.resolve",
        icon: "circle-check",
        bindings: &["R"],
        scope: Scope::PullRequest,
        requires: Some(Capability::ThreadResolve),
    },
    Command {
        id: "review.open",
        icon: "clipboard-check",
        bindings: &["v"],
        scope: Scope::PullRequest,
        requires: Some(Capability::ReviewSubmit),
    },
    Command {
        id: "pr.open_in_editor",
        icon: "external-link",
        bindings: &["V"],
        scope: Scope::PullRequest,
        requires: None,
    },
    Command {
        id: "pr.mark_viewed",
        icon: "eye-off",
        bindings: &["w"],
        scope: Scope::PullRequest,
        requires: None,
    },
    Command {
        id: "pr.merge",
        icon: "git-merge",
        bindings: &["m"],
        scope: Scope::PullRequest,
        requires: Some(Capability::Merge),
    },
];

pub const REVIEW_PANEL: [Command; 2] = [
    Command {
        id: "review.approve",
        icon: "check",
        bindings: &["a"],
        scope: Scope::ReviewPanel,
        requires: Some(Capability::ReviewSubmit),
    },
    Command {
        id: "review.request_changes",
        icon: "pencil-line",
        bindings: &["c"],
        scope: Scope::ReviewPanel,
        requires: Some(Capability::ReviewSubmit),
    },
];

pub fn registry() -> Vec<&'static Command> {
    REGISTRY.iter().chain(REVIEW_PANEL.iter()).collect()
}

impl Command {
    pub fn title_key(&self) -> String {
        format!("command.{}", self.id)
    }

    pub fn chords(&self) -> Vec<KeyChord> {
        self.bindings
            .iter()
            .filter_map(|binding| KeyChord::parse(binding).ok())
            .collect()
    }

    pub fn is_discoverable(&self, capabilities: &Capabilities) -> bool {
        match self.requires {
            None => true,
            Some(capability) => capabilities.get(capability).is_discoverable(),
        }
    }

    pub fn is_enabled(&self, capabilities: &Capabilities) -> bool {
        match self.requires {
            None => true,
            Some(capability) => capabilities.get(capability).is_enabled(),
        }
    }
}

pub fn declared_in(scope: Scope) -> Vec<&'static Command> {
    registry()
        .into_iter()
        .filter(|command| command.scope == scope)
        .collect()
}

pub fn available_in(scope: Scope, capabilities: &Capabilities) -> Vec<&'static Command> {
    let mut available: Vec<&'static Command> = Vec::new();
    for level in scope.ancestors() {
        for command in declared_in(*level) {
            if !command.is_discoverable(capabilities) {
                continue;
            }
            let shadowed = command
                .chords()
                .iter()
                .any(|chord| available.iter().any(|kept| kept.chords().contains(chord)));
            if !shadowed {
                available.push(command);
            }
        }
    }
    available
}

pub fn resolve(
    scope: Scope,
    chord: &KeyChord,
    capabilities: &Capabilities,
) -> Option<&'static Command> {
    for level in scope.ancestors() {
        let found: Vec<&'static Command> = declared_in(*level)
            .into_iter()
            .filter(|command| command.chords().contains(chord))
            .collect();
        if let Some(command) = found.first()
            && command.is_enabled(capabilities)
        {
            return Some(command);
        }
        if !found.is_empty() {
            return None;
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub id: &'static str,
    pub score: i32,
}

pub fn rank(
    needle: &str,
    scope: Scope,
    capabilities: &Capabilities,
    catalogue: &crate::i18n::Catalogue,
) -> Vec<Match> {
    let mut matched: Vec<Match> = available_in(scope, capabilities)
        .into_iter()
        .filter_map(|command| {
            best_score(needle, command, catalogue).map(|score| Match {
                id: command.id,
                score,
            })
        })
        .collect();
    matched.sort_by(|left, right| right.score.cmp(&left.score).then(left.id.cmp(right.id)));
    matched
}

fn best_score(needle: &str, command: &Command, catalogue: &crate::i18n::Catalogue) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let title_key = command.title_key();
    let mut best: Option<i32> = fuzzy_score(needle, catalogue.get(&title_key));
    let keyword_key = format!("{title_key}.keywords");
    for keyword in catalogue.get(&keyword_key).split_whitespace() {
        if let Some(score) = fuzzy_score(needle, keyword) {
            best = Some(best.map_or(score - 5, |current| current.max(score - 5)));
        }
    }
    if let Some(score) = fuzzy_score(needle, command.id) {
        best = Some(best.map_or(score - 2, |current| current.max(score - 2)));
    }
    best
}

pub fn fuzzy_score(needle: &str, haystack: &str) -> Option<i32> {
    let needle: Vec<char> = needle.to_lowercase().chars().collect();
    let characters: Vec<char> = haystack.chars().collect();
    let lowered: Vec<char> = haystack.to_lowercase().chars().collect();

    let mut score = 0;
    let mut cursor = 0usize;
    let mut previous_matched = false;

    for wanted in &needle {
        if wanted.is_whitespace() {
            continue;
        }
        let found = lowered
            .iter()
            .enumerate()
            .skip(cursor)
            .find(|(_, character)| *character == wanted)
            .map(|(position, _)| position);
        let position = found?;
        score += 1;
        if position == 0 {
            score += 8;
        } else {
            let before = characters[position - 1];
            if before.is_whitespace() || before == '.' || before == '_' || before == '-' {
                score += 6;
            } else if before.is_lowercase() && characters[position].is_uppercase() {
                score += 4;
            }
        }
        if previous_matched && found == Some(cursor) {
            score += 3;
        }
        previous_matched = found == Some(cursor);
        cursor = position + 1;
    }
    Some(score)
}
