use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Key {
    pub command: bool,
    pub name: String,
}

impl Key {
    fn render(&self) -> String {
        if self.command {
            format!("⌘{}", self.name)
        } else {
            self.name.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub keys: Vec<Key>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    Empty,
    UnknownKey { name: String },
}

impl fmt::Display for KeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyError::Empty => write!(formatter, "a chord binds no key"),
            KeyError::UnknownKey { name } => write!(formatter, "unknown key `{name}`"),
        }
    }
}

const NAMED_KEYS: [&str; 5] = ["Enter", "Space", "Esc", "Tab", "Backspace"];

impl KeyChord {
    pub fn parse(input: &str) -> Result<Self, KeyError> {
        let keys = input
            .split_whitespace()
            .map(parse_key)
            .collect::<Result<Vec<Key>, KeyError>>()?;
        if keys.is_empty() {
            return Err(KeyError::Empty);
        }
        Ok(Self { keys })
    }

    pub fn render(&self) -> String {
        self.keys
            .iter()
            .map(Key::render)
            .collect::<Vec<String>>()
            .join(" ")
    }

    pub fn is_prefix_of(&self, other: &KeyChord) -> bool {
        self.keys.len() < other.keys.len() && other.keys.starts_with(&self.keys)
    }
}

fn parse_key(token: &str) -> Result<Key, KeyError> {
    let (command, rest) = match token.strip_prefix('⌘') {
        Some(rest) => (true, rest),
        None => (false, token),
    };
    if rest.is_empty() {
        return Err(KeyError::UnknownKey {
            name: token.to_owned(),
        });
    }
    if NAMED_KEYS.contains(&rest) || rest.chars().count() == 1 {
        Ok(Key {
            command,
            name: rest.to_owned(),
        })
    } else {
        Err(KeyError::UnknownKey {
            name: rest.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyChord, KeyError};

    #[test]
    fn a_single_letter_is_a_chord_of_one_key() {
        let chord = match KeyChord::parse("j") {
            Ok(chord) => chord,
            Err(error) => panic!("a letter must parse: {error}"),
        };
        assert_eq!(chord.keys.len(), 1);
        assert_eq!(chord.render(), "j");
    }

    #[test]
    fn case_distinguishes_two_bindings_because_shift_is_part_of_the_key() {
        assert_ne!(KeyChord::parse("j"), KeyChord::parse("J"));
    }

    #[test]
    fn a_vim_style_sequence_keeps_its_keys_in_order() {
        let chord = match KeyChord::parse("g i") {
            Ok(chord) => chord,
            Err(error) => panic!("a sequence must parse: {error}"),
        };
        assert_eq!(chord.keys.len(), 2);
        assert_eq!(chord.render(), "g i");
    }

    #[test]
    fn the_command_modifier_is_carried_by_the_key_it_decorates() {
        let chord = match KeyChord::parse("⌘K") {
            Ok(chord) => chord,
            Err(error) => panic!("a modified key must parse: {error}"),
        };
        assert!(chord.keys[0].command);
        assert_eq!(chord.render(), "⌘K");
    }

    #[test]
    fn a_named_key_is_accepted_where_a_single_character_would_not_do() {
        for name in ["Enter", "Space", "Esc"] {
            assert!(KeyChord::parse(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn an_empty_binding_is_refused_rather_than_bound_to_nothing() {
        assert_eq!(KeyChord::parse("   "), Err(KeyError::Empty));
    }

    #[test]
    fn an_unknown_key_name_is_refused_rather_than_silently_dropped() {
        assert!(matches!(
            KeyChord::parse("Meta"),
            Err(KeyError::UnknownKey { .. })
        ));
    }

    #[test]
    fn a_shorter_chord_is_a_prefix_of_a_longer_one_that_starts_with_it() {
        let short = KeyChord::parse("g").unwrap_or(KeyChord { keys: Vec::new() });
        let long = KeyChord::parse("g i").unwrap_or(KeyChord { keys: Vec::new() });
        assert!(short.is_prefix_of(&long));
        assert!(!long.is_prefix_of(&short));
    }

    #[test]
    fn a_chord_is_never_a_prefix_of_itself() {
        let chord = KeyChord::parse("g i").unwrap_or(KeyChord { keys: Vec::new() });
        assert!(!chord.is_prefix_of(&chord));
    }

    #[test]
    fn two_chords_that_only_share_a_first_key_are_not_prefixes() {
        let first = KeyChord::parse("g i").unwrap_or(KeyChord { keys: Vec::new() });
        let second = KeyChord::parse("g p").unwrap_or(KeyChord { keys: Vec::new() });
        assert!(!first.is_prefix_of(&second));
    }
}
