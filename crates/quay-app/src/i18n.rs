use std::collections::BTreeMap;

pub const FRENCH: &str = include_str!("../i18n/fr.json");
pub const ENGLISH: &str = include_str!("../i18n/en.json");
pub const LOCALES: [&str; 2] = ["fr", "en"];
pub const FALLBACK: &str = "fr";

#[derive(Debug, Clone, Default)]
pub struct Catalogue {
    locale: String,
    entries: BTreeMap<String, String>,
}

impl Catalogue {
    pub fn for_locale(locale: &str) -> Self {
        let source = match locale {
            "en" => ENGLISH,
            _ => FRENCH,
        };
        Self {
            locale: if LOCALES.contains(&locale) {
                locale.to_owned()
            } else {
                FALLBACK.to_owned()
            },
            entries: serde_json::from_str(source).unwrap_or_default(),
        }
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.entries.get(key).map(String::as_str).unwrap_or(key)
    }

    pub fn render(&self, key: &str, values: &[(&str, &str)]) -> String {
        let mut rendered = self.get(key).to_owned();
        for (name, value) in values {
            rendered = rendered.replace(&format!("{{{name}}}"), value);
        }
        rendered
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    pub fn entries(&self) -> &BTreeMap<String, String> {
        &self.entries
    }
}

pub fn preferred_locale() -> String {
    std::env::var("QUAY_LOCALE")
        .ok()
        .filter(|locale| LOCALES.contains(&locale.as_str()))
        .or_else(|| {
            std::env::var("LANG")
                .ok()
                .and_then(|lang| lang.split(['_', '.']).next().map(str::to_owned))
                .filter(|locale| LOCALES.contains(&locale.as_str()))
        })
        .unwrap_or_else(|| FALLBACK.to_owned())
}

pub const REVIEW_VERDICTS: [&str; 4] = ["approved", "changes_requested", "commented", "dismissed"];

pub fn feed_key(kind: quay_core::TimelineKind, reference: Option<&str>) -> String {
    if kind == quay_core::TimelineKind::ReviewRequested && reference.is_none() {
        return "feed.review_requested.anyone".to_owned();
    }
    if kind != quay_core::TimelineKind::Review {
        return format!("feed.{}", kind.id());
    }
    let verdict = reference
        .filter(|state| REVIEW_VERDICTS.contains(state))
        .unwrap_or("commented");
    format!("feed.review.{verdict}")
}

#[cfg(test)]
mod tests {
    use super::{Catalogue, ENGLISH, FRENCH, LOCALES};

    #[test]
    fn every_shipped_catalogue_parses() {
        for source in [FRENCH, ENGLISH] {
            let parsed: serde_json::Result<std::collections::BTreeMap<String, String>> =
                serde_json::from_str(source);
            assert!(parsed.is_ok());
        }
    }

    #[test]
    fn every_locale_carries_exactly_the_same_keys() {
        let french = Catalogue::for_locale("fr");
        let english = Catalogue::for_locale("en");
        let french_keys: Vec<&String> = french.entries().keys().collect();
        let english_keys: Vec<&String> = english.entries().keys().collect();
        assert_eq!(
            french_keys, english_keys,
            "a locale is missing keys the other carries"
        );
    }

    #[test]
    fn no_translation_is_left_empty() {
        for locale in LOCALES {
            let catalogue = Catalogue::for_locale(locale);
            for (key, value) in catalogue.entries() {
                assert!(!value.trim().is_empty(), "{locale} leaves {key} empty");
            }
        }
    }

    #[test]
    fn an_unknown_key_renders_as_itself_so_a_user_named_view_survives() {
        let catalogue = Catalogue::for_locale("fr");
        assert_eq!(catalogue.get("Mon truc à moi"), "Mon truc à moi");
    }

    #[test]
    fn an_unknown_locale_falls_back_rather_than_emptying_the_interface() {
        let catalogue = Catalogue::for_locale("kl");
        assert_eq!(catalogue.locale(), "fr");
        assert!(!catalogue.entries().is_empty());
    }

    #[test]
    fn every_command_of_the_registry_has_a_title_and_keywords_in_every_locale() {
        for locale in LOCALES {
            let catalogue = Catalogue::for_locale(locale);
            for command in crate::commands::registry() {
                let title = command.title_key();
                assert_ne!(catalogue.get(&title), title, "{locale} misses {title}");
                let keywords = format!("{title}.keywords");
                assert_ne!(
                    catalogue.get(&keywords),
                    keywords,
                    "{locale} misses {keywords}"
                );
            }
        }
    }

    #[test]
    fn every_timeline_kind_names_itself_in_every_locale() {
        for locale in LOCALES {
            let catalogue = Catalogue::for_locale(locale);
            for kind in quay_core::TimelineKind::ALL {
                for key in feed_keys_of(kind) {
                    assert_ne!(catalogue.get(&key), key, "{locale} misses {key}");
                }
            }
        }
    }

    #[test]
    fn a_review_state_the_forge_invents_later_still_names_itself() {
        assert_eq!(
            super::feed_key(quay_core::TimelineKind::Review, Some("ESCALATED")),
            "feed.review.commented"
        );
        assert_eq!(
            super::feed_key(quay_core::TimelineKind::Review, None),
            "feed.review.commented"
        );
    }

    fn feed_keys_of(kind: quay_core::TimelineKind) -> Vec<String> {
        if kind == quay_core::TimelineKind::Review {
            return super::REVIEW_VERDICTS
                .iter()
                .map(|verdict| super::feed_key(kind, Some(verdict)))
                .collect();
        }
        let mut keys = vec![
            super::feed_key(kind, None),
            super::feed_key(kind, Some("avery")),
        ];
        keys.dedup();
        keys
    }

    #[test]
    fn a_review_asked_of_someone_this_build_cannot_name_still_reads_as_a_sentence() {
        let catalogue = Catalogue::for_locale("fr");
        let asked = super::feed_key(quay_core::TimelineKind::ReviewRequested, None);
        assert!(!catalogue.get(&asked).contains('{'));
    }

    #[test]
    fn a_placeholder_without_a_value_is_left_alone_rather_than_emptied() {
        let catalogue = Catalogue::for_locale("fr");
        let rendered = catalogue.render("feed.review_requested", &[]);
        assert!(rendered.contains("{target}"));
        let filled = catalogue.render("feed.review_requested", &[("target", "avery")]);
        assert!(filled.contains("avery"));
    }

    #[test]
    fn every_shipped_view_has_a_name_in_every_locale() {
        let views = match quay_store::views::shipped() {
            Ok(views) => views,
            Err(error) => panic!("the shipped views must parse: {error}"),
        };
        for locale in LOCALES {
            let catalogue = Catalogue::for_locale(locale);
            for view in &views {
                assert_ne!(
                    catalogue.get(&view.name),
                    view.name,
                    "{locale} misses {}",
                    view.name
                );
            }
        }
    }
}
