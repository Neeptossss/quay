use quay_app::commands::{Scope, available_in, declared_in, fuzzy_score, rank, registry, resolve};
use quay_app::keys::KeyChord;
use quay_forge::{Capabilities, Capability, GrantedScopes, TokenKind};

fn capabilities(scopes: &str) -> Capabilities {
    Capabilities::from_scopes(TokenKind::PatClassic, &GrantedScopes::parse(scopes))
}

fn full() -> Capabilities {
    capabilities("repo, notifications, read:org, workflow")
}

fn chord(input: &str) -> KeyChord {
    match KeyChord::parse(input) {
        Ok(chord) => chord,
        Err(error) => panic!("the binding {input} must parse: {error}"),
    }
}

#[test]
fn every_command_carries_at_least_one_binding_that_parses() {
    for command in registry() {
        assert!(!command.bindings.is_empty(), "{}", command.id);
        assert_eq!(
            command.chords().len(),
            command.bindings.len(),
            "a binding of {} does not parse",
            command.id
        );
    }
}

#[test]
fn every_command_identifier_is_unique() {
    let mut identifiers: Vec<&str> = registry().into_iter().map(|command| command.id).collect();
    let declared = identifiers.len();
    identifiers.sort_unstable();
    identifiers.dedup();
    assert_eq!(identifiers.len(), declared);
}

#[test]
fn every_command_carries_a_title_a_user_can_read() {
    for command in registry() {
        assert!(!command.title.is_empty(), "{}", command.id);
        assert!(
            command.title.chars().next().is_some_and(char::is_uppercase),
            "{} reads as {}",
            command.id,
            command.title
        );
    }
}

#[test]
fn no_two_commands_declared_in_the_same_scope_claim_the_same_chord() {
    for scope in Scope::ALL {
        let mut claimed: Vec<(String, &str)> = Vec::new();
        for command in declared_in(scope) {
            for chord in command.chords() {
                let rendered = chord.render();
                if let Some((_, holder)) = claimed.iter().find(|(kept, _)| kept == &rendered) {
                    panic!(
                        "{} and {} both claim `{rendered}` in scope {}",
                        holder,
                        command.id,
                        scope.id()
                    );
                }
                claimed.push((rendered, command.id));
            }
        }
    }
}

#[test]
fn no_chord_is_a_strict_prefix_of_another_in_the_same_scope() {
    for scope in Scope::ALL {
        let chords: Vec<(KeyChord, &str)> = declared_in(scope)
            .into_iter()
            .flat_map(|command| {
                command
                    .chords()
                    .into_iter()
                    .map(move |chord| (chord, command.id))
            })
            .collect();
        for (chord, owner) in &chords {
            for (other, holder) in &chords {
                assert!(
                    !chord.is_prefix_of(other),
                    "`{}` of {owner} is a prefix of `{}` of {holder} in scope {}",
                    chord.render(),
                    other.render(),
                    scope.id()
                );
            }
        }
    }
}

#[test]
fn every_command_is_reachable_from_the_scope_it_declares() {
    let capabilities = full();
    for command in registry() {
        let available = available_in(command.scope, &capabilities);
        assert!(
            available.iter().any(|found| found.id == command.id),
            "{} is unreachable from its own scope",
            command.id
        );
    }
}

#[test]
fn a_chord_resolves_to_exactly_one_command_in_every_scope() {
    let capabilities = full();
    for scope in Scope::ALL {
        for command in available_in(scope, &capabilities) {
            for chord in command.chords() {
                match resolve(scope, &chord, &capabilities) {
                    Some(resolved) => assert_eq!(
                        resolved.id,
                        command.id,
                        "`{}` resolves elsewhere in scope {}",
                        chord.render(),
                        scope.id()
                    ),
                    None => panic!(
                        "`{}` resolves to nothing in scope {}",
                        chord.render(),
                        scope.id()
                    ),
                }
            }
        }
    }
}

#[test]
fn an_inner_scope_shadows_the_chord_of_the_scope_it_sits_inside() {
    let capabilities = full();
    let commenting = match resolve(Scope::PullRequest, &chord("c"), &capabilities) {
        Some(command) => command.id,
        None => panic!("`c` must resolve inside a pull request"),
    };
    let panelled = match resolve(Scope::ReviewPanel, &chord("c"), &capabilities) {
        Some(command) => command.id,
        None => panic!("`c` must resolve inside the review panel"),
    };
    assert_eq!(commenting, "pr.comment");
    assert_eq!(panelled, "review.request_changes");
}

#[test]
fn a_diff_still_answers_the_file_navigation_of_the_pull_request_around_it() {
    let capabilities = full();
    match resolve(Scope::Diff, &chord("]"), &capabilities) {
        Some(command) => assert_eq!(command.id, "pr.next_file"),
        None => panic!("file navigation must survive inside a diff"),
    }
}

#[test]
fn the_palette_is_reachable_from_every_scope() {
    let capabilities = full();
    for scope in Scope::ALL {
        match resolve(scope, &chord("⌘k"), &capabilities) {
            Some(command) => assert_eq!(command.id, "palette.open"),
            None => panic!("the palette must open from scope {}", scope.id()),
        }
    }
}

#[test]
fn a_structurally_unavailable_command_leaves_the_registry_entirely() {
    let mut capabilities = full();
    capabilities.observe_failure(
        Capability::Merge,
        403,
        "Organization has enabled a policy that blocks this",
    );
    assert!(
        !available_in(Scope::PullRequest, &capabilities)
            .iter()
            .any(|command| command.id == "pr.merge"),
        "a command blocked by policy must not be discoverable"
    );
    assert!(resolve(Scope::PullRequest, &chord("m"), &capabilities).is_none());
}

#[test]
fn a_command_the_user_can_recover_stays_visible_but_does_not_fire() {
    let capabilities = capabilities("repo, notifications, read:org");
    let rerun = available_in(Scope::Global, &capabilities);
    assert!(
        !rerun.iter().any(|command| command.id == "pr.merge"),
        "merge is not a global command"
    );
    let pull_request = available_in(Scope::PullRequest, &capabilities);
    assert!(pull_request.iter().any(|command| command.id == "pr.merge"));
    assert!(resolve(Scope::PullRequest, &chord("m"), &capabilities).is_some());
}

#[test]
fn ranking_with_no_needle_offers_every_available_command() {
    let capabilities = full();
    assert_eq!(
        rank("", Scope::PullRequest, &capabilities).len(),
        available_in(Scope::PullRequest, &capabilities).len()
    );
}

#[test]
fn a_match_at_a_word_start_outranks_the_same_letters_inside_a_word() {
    let start = match fuzzy_score("mer", "Merger") {
        Some(score) => score,
        None => panic!("a prefix must match"),
    };
    let inside = match fuzzy_score("mer", "Aimer merger") {
        Some(score) => score,
        None => panic!("an inner match must match"),
    };
    assert!(start > inside, "{start} should beat {inside}");
}

#[test]
fn a_needle_that_is_not_a_subsequence_matches_nothing() {
    assert!(fuzzy_score("zzz", "Merger").is_none());
}

#[test]
fn ranking_puts_the_command_the_letters_name_first() {
    let capabilities = full();
    let ranked = rank("merg", Scope::PullRequest, &capabilities);
    match ranked.first() {
        Some(best) => assert_eq!(best.id, "pr.merge"),
        None => panic!("merging must be found"),
    }
}

#[test]
fn ranking_never_offers_a_command_from_an_unrelated_scope() {
    let capabilities = full();
    let ranked = rank("", Scope::List, &capabilities);
    assert!(!ranked.iter().any(|found| found.id == "pr.merge"));
}
