use quay_core::{Qualifier, Query, Term, VIEWER};
use rusqlite::types::Value;

use crate::error::StoreError;

const SELECTION: &str = "\
SELECT r.owner, r.name, pr.number, pr.title, pr.is_draft, pr.author,
       pr.review_state, pr.checks_state, pr.updated_at,
       (SELECT COUNT(*) FROM review_thread t
         WHERE t.pr_id = pr.id AND t.is_resolved = 0)
FROM pull_request pr
JOIN repo r ON r.id = pr.repo_id
WHERE r.is_tracked = 1";

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledQuery {
    pub sql: String,
    pub parameters: Vec<Value>,
}

pub fn compile(query: &Query, viewer: &str, limit: i64) -> Result<CompiledQuery, StoreError> {
    let mut conditions: Vec<String> = Vec::new();
    let mut parameters: Vec<Value> = Vec::new();

    for term in &query.terms {
        if let Some(condition) = condition_for(term, viewer, &mut parameters)? {
            conditions.push(condition);
        }
    }
    for word in &query.words {
        conditions.push("pr.title LIKE ?".to_owned());
        parameters.push(Value::Text(format!("%{word}%")));
    }

    let mut sql = String::from(SELECTION);
    for condition in &conditions {
        sql.push_str("\n  AND ");
        sql.push_str(condition);
    }
    sql.push_str("\nORDER BY ");
    sql.push_str(order_for(query));
    sql.push_str("\nLIMIT ?");
    parameters.push(Value::Integer(limit));

    Ok(CompiledQuery { sql, parameters })
}

fn condition_for(
    term: &Term,
    viewer: &str,
    parameters: &mut Vec<Value>,
) -> Result<Option<String>, StoreError> {
    let value = resolve(&term.value, viewer);
    match term.qualifier {
        Qualifier::Is => is_condition(term, parameters),
        Qualifier::Author => {
            parameters.push(Value::Text(value));
            Ok(Some(
                if term.negated {
                    "pr.author <> ?"
                } else {
                    "pr.author = ?"
                }
                .to_owned(),
            ))
        }
        Qualifier::ReviewRequested => {
            parameters.push(Value::Text(value));
            let exists =
                "SELECT 1 FROM review_request rr WHERE rr.pr_id = pr.id AND rr.reviewer = ?";
            Ok(Some(if term.negated {
                format!("NOT EXISTS ({exists})")
            } else {
                format!("EXISTS ({exists})")
            }))
        }
        Qualifier::Repo => {
            let (owner, name) =
                value
                    .split_once('/')
                    .ok_or_else(|| StoreError::QualifierValueNotUnderstood {
                        qualifier: "repo",
                        value: term.value.clone(),
                        expected: "owner/name",
                    })?;
            parameters.push(Value::Text(owner.to_owned()));
            parameters.push(Value::Text(name.to_owned()));
            Ok(Some(
                if term.negated {
                    "NOT (r.owner = ? AND r.name = ?)"
                } else {
                    "(r.owner = ? AND r.name = ?)"
                }
                .to_owned(),
            ))
        }
        Qualifier::Checks => {
            let stored = match term.value.as_str() {
                "failing" => "failure",
                other => other,
            };
            if stored == "none" {
                return Ok(Some(
                    if term.negated {
                        "pr.checks_state IS NOT NULL AND pr.checks_state <> 'none'"
                    } else {
                        "(pr.checks_state IS NULL OR pr.checks_state = 'none')"
                    }
                    .to_owned(),
                ));
            }
            parameters.push(Value::Text(stored.to_owned()));
            Ok(Some(
                if term.negated {
                    "(pr.checks_state IS NULL OR pr.checks_state <> ?)"
                } else {
                    "pr.checks_state = ?"
                }
                .to_owned(),
            ))
        }
        Qualifier::Sort => Ok(None),
        Qualifier::Assignee => Err(StoreError::QualifierNotSupportedYet {
            qualifier: "assignee",
            because: "no table records assignees; issues arrive at M3",
        }),
        Qualifier::Label => Err(StoreError::QualifierNotSupportedYet {
            qualifier: "label",
            because: "no table records labels; issues arrive at M3",
        }),
    }
}

fn is_condition(term: &Term, parameters: &mut Vec<Value>) -> Result<Option<String>, StoreError> {
    match term.value.as_str() {
        "pr" => Ok(None),
        "issue" => Err(StoreError::QualifierNotSupportedYet {
            qualifier: "is:issue",
            because: "this build stores pull requests only; issues arrive at M3",
        }),
        "draft" => Ok(Some(
            if term.negated {
                "pr.is_draft = 0"
            } else {
                "pr.is_draft = 1"
            }
            .to_owned(),
        )),
        state => {
            parameters.push(Value::Text(state.to_owned()));
            Ok(Some(
                if term.negated {
                    "pr.state <> ?"
                } else {
                    "pr.state = ?"
                }
                .to_owned(),
            ))
        }
    }
}

fn order_for(query: &Query) -> &'static str {
    match query.value_of(Qualifier::Sort) {
        Some("updated-asc") => "pr.updated_at ASC",
        Some("number-desc") => "pr.number DESC",
        Some("number-asc") => "pr.number ASC",
        _ => "pr.updated_at DESC",
    }
}

fn resolve(value: &str, viewer: &str) -> String {
    if value == VIEWER {
        viewer.to_owned()
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use quay_core::parse_query;

    use super::compile;
    use crate::error::StoreError;

    fn sql_of(input: &str) -> String {
        compile(&parse_query(input).unwrap(), "octocat", 50)
            .unwrap()
            .sql
    }

    #[test]
    fn an_empty_query_still_restricts_itself_to_tracked_repositories() {
        let sql = sql_of("");
        assert!(sql.contains("r.is_tracked = 1"));
        assert!(sql.contains("ORDER BY pr.updated_at DESC"));
    }

    #[test]
    fn the_viewer_shorthand_is_resolved_to_the_login_and_bound_not_interpolated() {
        let compiled = compile(&parse_query("author:@me").unwrap(), "octocat", 50).unwrap();
        assert!(compiled.sql.contains("pr.author = ?"));
        assert!(!compiled.sql.contains("octocat"));
        assert!(
            compiled
                .parameters
                .contains(&rusqlite::types::Value::Text("octocat".to_owned()))
        );
    }

    #[test]
    fn a_negated_author_becomes_an_inequality() {
        assert!(sql_of("-author:@me").contains("pr.author <> ?"));
    }

    #[test]
    fn a_review_request_becomes_an_exists_and_its_negation_a_not_exists() {
        assert!(sql_of("review-requested:@me").contains("EXISTS (SELECT 1 FROM review_request"));
        assert!(
            sql_of("-review-requested:@me").contains("NOT EXISTS (SELECT 1 FROM review_request")
        );
    }

    #[test]
    fn failing_checks_are_stored_under_the_name_the_forge_uses() {
        let compiled = compile(&parse_query("checks:failing").unwrap(), "octocat", 50).unwrap();
        assert!(
            compiled
                .parameters
                .contains(&rusqlite::types::Value::Text("failure".to_owned()))
        );
    }

    #[test]
    fn absent_checks_are_matched_without_a_parameter_because_null_is_not_a_value() {
        let sql = sql_of("checks:none");
        assert!(sql.contains("pr.checks_state IS NULL"));
    }

    #[test]
    fn a_repository_is_split_into_its_owner_and_its_name() {
        let compiled = compile(&parse_query("repo:acme/api").unwrap(), "octocat", 50).unwrap();
        assert!(compiled.sql.contains("(r.owner = ? AND r.name = ?)"));
        assert_eq!(compiled.parameters.len(), 3);
    }

    #[test]
    fn a_repository_without_a_slash_is_refused_with_the_shape_it_expected() {
        match compile(&parse_query("repo:acme").unwrap(), "octocat", 50) {
            Err(StoreError::QualifierValueNotUnderstood { expected, .. }) => {
                assert_eq!(expected, "owner/name");
            }
            other => panic!("a malformed repository must be refused, got {other:?}"),
        }
    }

    #[test]
    fn free_text_becomes_a_title_match_bound_as_a_parameter() {
        let compiled = compile(&parse_query("transport").unwrap(), "octocat", 50).unwrap();
        assert!(compiled.sql.contains("pr.title LIKE ?"));
        assert!(
            compiled
                .parameters
                .contains(&rusqlite::types::Value::Text("%transport%".to_owned()))
        );
    }

    #[test]
    fn a_sort_changes_the_order_without_adding_a_condition() {
        let sorted = sql_of("sort:number-asc");
        assert!(sorted.contains("ORDER BY pr.number ASC"));
        assert_eq!(
            sorted.matches("\n  AND ").count(),
            0,
            "a sort must not narrow the result"
        );
    }

    #[test]
    fn each_term_adds_exactly_one_condition() {
        let compiled = sql_of("is:open author:@me checks:failing");
        assert_eq!(compiled.matches("\n  AND ").count(), 3);
    }

    #[test]
    fn a_qualifier_this_milestone_cannot_answer_is_refused_by_name_and_says_why() {
        for (input, expected) in [
            ("is:issue", "is:issue"),
            ("label:bug", "label"),
            ("assignee:@me", "assignee"),
        ] {
            match compile(&parse_query(input).unwrap(), "octocat", 50) {
                Err(StoreError::QualifierNotSupportedYet { qualifier, because }) => {
                    assert_eq!(qualifier, expected);
                    assert!(because.contains("M3"));
                }
                other => panic!("{input} must be refused by name, got {other:?}"),
            }
        }
    }

    #[test]
    fn the_canonical_query_of_the_specification_compiles_whole() {
        let compiled = compile(
            &parse_query("is:pr is:open review-requested:@me -author:@me sort:updated-desc")
                .unwrap(),
            "octocat",
            50,
        )
        .unwrap();
        assert!(compiled.sql.contains("pr.state = ?"));
        assert!(
            compiled
                .sql
                .contains("EXISTS (SELECT 1 FROM review_request")
        );
        assert!(compiled.sql.contains("pr.author <> ?"));
        assert!(compiled.sql.contains("ORDER BY pr.updated_at DESC"));
        assert_eq!(compiled.parameters.len(), 4);
    }
}
