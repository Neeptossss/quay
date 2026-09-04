use std::fmt;

pub const VIEWER: &str = "@me";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qualifier {
    Is,
    Author,
    ReviewRequested,
    Assignee,
    Label,
    Org,
    Repo,
    Checks,
    Sort,
}

impl Qualifier {
    pub const ALL: [Qualifier; 9] = [
        Qualifier::Is,
        Qualifier::Author,
        Qualifier::ReviewRequested,
        Qualifier::Assignee,
        Qualifier::Label,
        Qualifier::Org,
        Qualifier::Repo,
        Qualifier::Checks,
        Qualifier::Sort,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Qualifier::Is => "is",
            Qualifier::Author => "author",
            Qualifier::ReviewRequested => "review-requested",
            Qualifier::Assignee => "assignee",
            Qualifier::Label => "label",
            Qualifier::Org => "org",
            Qualifier::Repo => "repo",
            Qualifier::Checks => "checks",
            Qualifier::Sort => "sort",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Qualifier::ALL
            .into_iter()
            .find(|candidate| candidate.id() == id)
    }

    pub fn accepted_values(self) -> Option<&'static [&'static str]> {
        match self {
            Qualifier::Is => Some(&["pr", "issue", "open", "closed", "merged", "draft"]),
            Qualifier::Checks => Some(&["success", "failing", "pending", "none"]),
            Qualifier::Sort => Some(&["updated-desc", "updated-asc", "number-desc", "number-asc"]),
            Qualifier::Author | Qualifier::ReviewRequested | Qualifier::Assignee => None,
            Qualifier::Label | Qualifier::Org | Qualifier::Repo => None,
        }
    }

    pub fn accepts_negation(self) -> bool {
        self != Qualifier::Sort
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    pub qualifier: Qualifier,
    pub value: String,
    pub negated: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    pub terms: Vec<Term>,
    pub words: Vec<String>,
}

impl Query {
    pub fn value_of(&self, qualifier: Qualifier) -> Option<&str> {
        self.terms
            .iter()
            .find(|term| term.qualifier == qualifier && !term.negated)
            .map(|term| term.value.as_str())
    }

    pub fn mentions(&self, qualifier: Qualifier, value: &str) -> bool {
        self.terms
            .iter()
            .any(|term| term.qualifier == qualifier && term.value == value && !term.negated)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    UnknownQualifier {
        qualifier: String,
        closest: Option<&'static str>,
    },
    UnknownValue {
        qualifier: &'static str,
        value: String,
        accepted: &'static [&'static str],
    },
    EmptyValue {
        qualifier: String,
    },
    NegationNotAccepted {
        qualifier: &'static str,
    },
    UnclosedQuote,
}

impl fmt::Display for QueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryError::UnknownQualifier { qualifier, closest } => match closest {
                Some(closest) => {
                    write!(
                        formatter,
                        "unknown qualifier `{qualifier}`, did you mean `{closest}`"
                    )
                }
                None => write!(formatter, "unknown qualifier `{qualifier}`"),
            },
            QueryError::UnknownValue {
                qualifier,
                value,
                accepted,
            } => write!(
                formatter,
                "`{qualifier}` does not accept `{value}`, only {}",
                accepted.join(", ")
            ),
            QueryError::EmptyValue { qualifier } => {
                write!(formatter, "`{qualifier}` was given no value")
            }
            QueryError::NegationNotAccepted { qualifier } => {
                write!(formatter, "`{qualifier}` cannot be negated")
            }
            QueryError::UnclosedQuote => write!(formatter, "a quoted value is never closed"),
        }
    }
}

impl std::error::Error for QueryError {}

pub fn parse(input: &str) -> Result<Query, QueryError> {
    let mut query = Query::default();
    for token in tokenize(input)? {
        let (negated, token) = match token.strip_prefix('-') {
            Some(rest) => (true, rest.to_owned()),
            None => (false, token),
        };
        let Some((name, value)) = token.split_once(':') else {
            if !token.is_empty() {
                query.words.push(token);
            }
            continue;
        };
        let qualifier = Qualifier::parse(name).ok_or_else(|| QueryError::UnknownQualifier {
            qualifier: name.to_owned(),
            closest: closest_qualifier(name),
        })?;
        if value.is_empty() {
            return Err(QueryError::EmptyValue {
                qualifier: name.to_owned(),
            });
        }
        if negated && !qualifier.accepts_negation() {
            return Err(QueryError::NegationNotAccepted {
                qualifier: qualifier.id(),
            });
        }
        if let Some(accepted) = qualifier.accepted_values()
            && !accepted.contains(&value)
        {
            return Err(QueryError::UnknownValue {
                qualifier: qualifier.id(),
                value: value.to_owned(),
                accepted,
            });
        }
        query.terms.push(Term {
            qualifier,
            value: value.to_owned(),
            negated,
        });
    }
    Ok(query)
}

fn tokenize(input: &str) -> Result<Vec<String>, QueryError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for character in input.chars() {
        match character {
            '"' => quoted = !quoted,
            character if character.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            character => current.push(character),
        }
    }
    if quoted {
        return Err(QueryError::UnclosedQuote);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

fn closest_qualifier(name: &str) -> Option<&'static str> {
    Qualifier::ALL
        .into_iter()
        .map(|qualifier| qualifier.id())
        .find(|candidate| candidate.starts_with(name) || name.starts_with(candidate))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub insertion: String,
    pub describes: &'static str,
}

pub fn completions(input: &str) -> Vec<Completion> {
    let partial = input.rsplit(char::is_whitespace).next().unwrap_or_default();
    let partial = partial.strip_prefix('-').unwrap_or(partial);

    match partial.split_once(':') {
        None => Qualifier::ALL
            .into_iter()
            .filter(|qualifier| qualifier.id().starts_with(partial))
            .map(|qualifier| Completion {
                insertion: format!("{}:", qualifier.id()),
                describes: qualifier.id(),
            })
            .collect(),
        Some((name, started)) => {
            let Some(qualifier) = Qualifier::parse(name) else {
                return Vec::new();
            };
            let values: Vec<&'static str> = qualifier
                .accepted_values()
                .map(<[&str]>::to_vec)
                .unwrap_or_else(|| match qualifier {
                    Qualifier::Author | Qualifier::ReviewRequested | Qualifier::Assignee => {
                        vec![VIEWER]
                    }
                    _ => Vec::new(),
                });
            values
                .into_iter()
                .filter(|value| value.starts_with(started))
                .map(|value| Completion {
                    insertion: format!("{}:{value}", qualifier.id()),
                    describes: qualifier.id(),
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Completion, Qualifier, QueryError, completions, parse};

    #[test]
    fn an_empty_query_selects_everything_and_carries_no_term() {
        let parsed = match parse("") {
            Ok(parsed) => parsed,
            Err(error) => panic!("an empty query is valid: {error}"),
        };
        assert!(parsed.terms.is_empty());
        assert!(parsed.words.is_empty());
    }

    #[test]
    fn the_canonical_review_query_of_the_specification_parses() {
        let parsed = match parse("is:pr is:open review-requested:@me -author:@me sort:updated-desc")
        {
            Ok(parsed) => parsed,
            Err(error) => panic!("the canonical query must parse: {error}"),
        };
        assert!(parsed.mentions(Qualifier::Is, "pr"));
        assert!(parsed.mentions(Qualifier::Is, "open"));
        assert!(parsed.mentions(Qualifier::ReviewRequested, "@me"));
        assert_eq!(parsed.value_of(Qualifier::Sort), Some("updated-desc"));
        let negated: Vec<&str> = parsed
            .terms
            .iter()
            .filter(|term| term.negated)
            .map(|term| term.value.as_str())
            .collect();
        assert_eq!(negated, vec!["@me"]);
    }

    #[test]
    fn the_authored_query_of_the_specification_parses() {
        let parsed = match parse("is:pr is:open author:@me checks:failing") {
            Ok(parsed) => parsed,
            Err(error) => panic!("the authored query must parse: {error}"),
        };
        assert_eq!(parsed.value_of(Qualifier::Author), Some("@me"));
        assert_eq!(parsed.value_of(Qualifier::Checks), Some("failing"));
    }

    #[test]
    fn the_issue_query_of_the_specification_parses_even_though_issues_come_later() {
        let parsed = match parse("is:issue label:bug assignee:@me repo:org/api") {
            Ok(parsed) => parsed,
            Err(error) => panic!("the issue query must parse: {error}"),
        };
        assert!(parsed.mentions(Qualifier::Is, "issue"));
        assert_eq!(parsed.value_of(Qualifier::Repo), Some("org/api"));
    }

    #[test]
    fn an_organisation_qualifier_parses_and_is_negatable() {
        let parsed = match parse("is:pr is:open org:acme") {
            Ok(parsed) => parsed,
            Err(error) => panic!("an organisation filter must parse: {error}"),
        };
        assert_eq!(parsed.value_of(Qualifier::Org), Some("acme"));
        assert!(parse("-org:acme").is_ok());
    }

    #[test]
    fn an_unknown_qualifier_is_refused_and_suggests_the_closest_one() {
        match parse("is:pr auth:me") {
            Err(QueryError::UnknownQualifier { qualifier, closest }) => {
                assert_eq!(qualifier, "auth");
                assert_eq!(closest, Some("author"));
            }
            other => panic!("an unknown qualifier must be refused, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_qualifier_with_no_neighbour_is_still_refused() {
        match parse("banana:yellow") {
            Err(QueryError::UnknownQualifier { closest, .. }) => assert_eq!(closest, None),
            other => panic!("an unknown qualifier must be refused, got {other:?}"),
        }
    }

    #[test]
    fn a_closed_value_set_refuses_a_value_it_does_not_contain_and_lists_what_it_takes() {
        match parse("checks:red") {
            Err(QueryError::UnknownValue {
                qualifier,
                value,
                accepted,
            }) => {
                assert_eq!(qualifier, "checks");
                assert_eq!(value, "red");
                assert!(accepted.contains(&"failing"));
            }
            other => panic!("an unknown value must be refused, got {other:?}"),
        }
    }

    #[test]
    fn a_qualifier_with_no_value_is_refused_rather_than_ignored() {
        assert!(matches!(
            parse("author:"),
            Err(QueryError::EmptyValue { .. })
        ));
    }

    #[test]
    fn a_sort_cannot_be_negated_because_the_result_would_be_meaningless() {
        assert!(matches!(
            parse("-sort:updated-desc"),
            Err(QueryError::NegationNotAccepted { .. })
        ));
    }

    #[test]
    fn an_unclosed_quote_is_refused_rather_than_silently_truncating_the_query() {
        assert!(matches!(
            parse("label:\"needs work"),
            Err(QueryError::UnclosedQuote)
        ));
    }

    #[test]
    fn a_quoted_value_keeps_its_spaces() {
        let parsed = match parse("label:\"needs work\"") {
            Ok(parsed) => parsed,
            Err(error) => panic!("a quoted value must parse: {error}"),
        };
        assert_eq!(parsed.value_of(Qualifier::Label), Some("needs work"));
    }

    #[test]
    fn bare_words_are_kept_as_free_text() {
        let parsed = match parse("is:open transport layer") {
            Ok(parsed) => parsed,
            Err(error) => panic!("free text must parse: {error}"),
        };
        assert_eq!(parsed.words, vec!["transport", "layer"]);
    }

    #[test]
    fn completing_an_empty_query_offers_every_qualifier() {
        assert_eq!(completions("").len(), Qualifier::ALL.len());
    }

    #[test]
    fn completing_a_partial_qualifier_narrows_the_offer() {
        let offered = completions("is:pr rev");
        assert_eq!(
            offered,
            vec![Completion {
                insertion: "review-requested:".to_owned(),
                describes: "review-requested",
            }]
        );
    }

    #[test]
    fn completing_a_closed_value_set_offers_its_values() {
        let offered: Vec<String> = completions("checks:")
            .into_iter()
            .map(|completion| completion.insertion)
            .collect();
        assert!(offered.contains(&"checks:failing".to_owned()));
        assert_eq!(offered.len(), 4);
    }

    #[test]
    fn completing_a_person_qualifier_offers_the_viewer_shorthand() {
        let offered: Vec<String> = completions("-author:")
            .into_iter()
            .map(|completion| completion.insertion)
            .collect();
        assert_eq!(offered, vec!["author:@me".to_owned()]);
    }

    #[test]
    fn completing_an_unknown_qualifier_offers_nothing_rather_than_guessing() {
        assert!(completions("banana:").is_empty());
    }
}
