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
    pub title: &'static str,
    pub keywords: &'static [&'static str],
    pub bindings: &'static [&'static str],
    pub scope: Scope,
    pub requires: Option<Capability>,
}

pub const REGISTRY: [Command; 32] = [
    Command {
        id: "goto.inbox",
        title: "Aller à l'inbox",
        keywords: &["inbox", "revue", "file"],
        bindings: &["g i"],
        scope: Scope::Global,
        requires: None,
    },
    Command {
        id: "goto.my_pull_requests",
        title: "Aller à mes pull requests",
        keywords: &["mine", "auteur"],
        bindings: &["g p"],
        scope: Scope::Global,
        requires: None,
    },
    Command {
        id: "view.open_nth",
        title: "Aller à une vue sauvegardée",
        keywords: &["vue", "saved"],
        bindings: &[
            "g 1", "g 2", "g 3", "g 4", "g 5", "g 6", "g 7", "g 8", "g 9",
        ],
        scope: Scope::Global,
        requires: None,
    },
    Command {
        id: "palette.open",
        title: "Ouvrir la palette de commandes",
        keywords: &["palette", "commande"],
        bindings: &["⌘k"],
        scope: Scope::Global,
        requires: None,
    },
    Command {
        id: "search.open",
        title: "Recherche rapide",
        keywords: &["chercher", "aller à"],
        bindings: &["⌘p"],
        scope: Scope::Global,
        requires: None,
    },
    Command {
        id: "help.shortcuts",
        title: "Aide des raccourcis",
        keywords: &["aide", "touches"],
        bindings: &["?"],
        scope: Scope::Global,
        requires: None,
    },
    Command {
        id: "list.next",
        title: "Ligne suivante",
        keywords: &["bas"],
        bindings: &["j"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.previous",
        title: "Ligne précédente",
        keywords: &["haut"],
        bindings: &["k"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.next_and_open",
        title: "Ligne suivante et ouvrir",
        keywords: &["liée"],
        bindings: &["J"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.previous_and_open",
        title: "Ligne précédente et ouvrir",
        keywords: &["liée"],
        bindings: &["K"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.first",
        title: "Première ligne",
        keywords: &["début"],
        bindings: &["g g"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.last",
        title: "Dernière ligne",
        keywords: &["fin"],
        bindings: &["G"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.open",
        title: "Ouvrir",
        keywords: &["entrer"],
        bindings: &["o", "Enter"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.preview",
        title: "Aperçu sans quitter la liste",
        keywords: &["aperçu"],
        bindings: &["Space"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.select",
        title: "Sélectionner",
        keywords: &["multi"],
        bindings: &["x"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.archive",
        title: "Archiver la notification",
        keywords: &["archiver"],
        bindings: &["e"],
        scope: Scope::List,
        requires: Some(Capability::Notifications),
    },
    Command {
        id: "list.mark_unread",
        title: "Marquer comme non lu",
        keywords: &["non lu"],
        bindings: &["u"],
        scope: Scope::List,
        requires: Some(Capability::Notifications),
    },
    Command {
        id: "list.filter",
        title: "Filtrer dans la liste",
        keywords: &["filtre", "chercher"],
        bindings: &["/"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "list.clear",
        title: "Effacer le filtre puis désélectionner",
        keywords: &["annuler"],
        bindings: &["Esc"],
        scope: Scope::List,
        requires: None,
    },
    Command {
        id: "pr.next_file",
        title: "Fichier suivant",
        keywords: &["fichier"],
        bindings: &["]"],
        scope: Scope::PullRequest,
        requires: None,
    },
    Command {
        id: "pr.previous_file",
        title: "Fichier précédent",
        keywords: &["fichier"],
        bindings: &["["],
        scope: Scope::PullRequest,
        requires: None,
    },
    Command {
        id: "pr.next_hunk",
        title: "Hunk suivant",
        keywords: &["hunk"],
        bindings: &["}"],
        scope: Scope::Diff,
        requires: None,
    },
    Command {
        id: "pr.previous_hunk",
        title: "Hunk précédent",
        keywords: &["hunk"],
        bindings: &["{"],
        scope: Scope::Diff,
        requires: None,
    },
    Command {
        id: "pr.next_unresolved",
        title: "Thread non résolu suivant",
        keywords: &["thread"],
        bindings: &["n"],
        scope: Scope::PullRequest,
        requires: None,
    },
    Command {
        id: "pr.previous_unresolved",
        title: "Thread non résolu précédent",
        keywords: &["thread"],
        bindings: &["p"],
        scope: Scope::PullRequest,
        requires: None,
    },
    Command {
        id: "pr.comment",
        title: "Commenter à la position courante",
        keywords: &["commentaire"],
        bindings: &["c"],
        scope: Scope::PullRequest,
        requires: Some(Capability::ReviewSubmit),
    },
    Command {
        id: "pr.reply",
        title: "Répondre au thread",
        keywords: &["réponse"],
        bindings: &["r"],
        scope: Scope::PullRequest,
        requires: Some(Capability::ReviewSubmit),
    },
    Command {
        id: "pr.resolve",
        title: "Résoudre le thread",
        keywords: &["résoudre"],
        bindings: &["R"],
        scope: Scope::PullRequest,
        requires: Some(Capability::ThreadResolve),
    },
    Command {
        id: "review.open",
        title: "Ouvrir le panneau de review",
        keywords: &["review", "soumettre"],
        bindings: &["v"],
        scope: Scope::PullRequest,
        requires: Some(Capability::ReviewSubmit),
    },
    Command {
        id: "pr.open_in_editor",
        title: "Ouvrir dans l'éditeur externe",
        keywords: &["éditeur"],
        bindings: &["V"],
        scope: Scope::PullRequest,
        requires: None,
    },
    Command {
        id: "pr.mark_viewed",
        title: "Marquer le fichier comme vu",
        keywords: &["vu"],
        bindings: &["w"],
        scope: Scope::PullRequest,
        requires: None,
    },
    Command {
        id: "pr.merge",
        title: "Merger",
        keywords: &["merge", "fusionner"],
        bindings: &["m"],
        scope: Scope::PullRequest,
        requires: Some(Capability::Merge),
    },
];

pub const REVIEW_PANEL: [Command; 2] = [
    Command {
        id: "review.approve",
        title: "Approuver et soumettre",
        keywords: &["approuver"],
        bindings: &["a"],
        scope: Scope::ReviewPanel,
        requires: Some(Capability::ReviewSubmit),
    },
    Command {
        id: "review.request_changes",
        title: "Demander des changements",
        keywords: &["changements"],
        bindings: &["c"],
        scope: Scope::ReviewPanel,
        requires: Some(Capability::ReviewSubmit),
    },
];

pub fn registry() -> Vec<&'static Command> {
    REGISTRY.iter().chain(REVIEW_PANEL.iter()).collect()
}

impl Command {
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

pub fn rank(needle: &str, scope: Scope, capabilities: &Capabilities) -> Vec<Match> {
    let mut matched: Vec<Match> = available_in(scope, capabilities)
        .into_iter()
        .filter_map(|command| {
            best_score(needle, command).map(|score| Match {
                id: command.id,
                score,
            })
        })
        .collect();
    matched.sort_by(|left, right| right.score.cmp(&left.score).then(left.id.cmp(right.id)));
    matched
}

fn best_score(needle: &str, command: &Command) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let mut best: Option<i32> = fuzzy_score(needle, command.title);
    for keyword in command.keywords {
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
