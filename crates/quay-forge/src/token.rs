use std::fmt;

#[derive(Clone)]
pub struct Token(String);

impl Token {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn from_env(variable: &str) -> Option<Self> {
        std::env::var(variable)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .map(Self)
    }

    pub fn header_value(&self) -> String {
        format!("Bearer {}", self.0)
    }

    pub fn redact(&self, text: &str) -> String {
        if self.0.is_empty() {
            text.to_owned()
        } else {
            text.replace(&self.0, "[redacted]")
        }
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Token([redacted])")
    }
}

impl fmt::Display for Token {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}

#[cfg(test)]
mod tests {
    use super::Token;

    #[test]
    fn debug_never_reveals_the_token() {
        let token = Token::new("ghp_secretvalue");
        assert_eq!(format!("{token:?}"), "Token([redacted])");
        assert!(!format!("{token:?}").contains("ghp_"));
    }

    #[test]
    fn display_never_reveals_the_token() {
        let token = Token::new("ghp_secretvalue");
        assert_eq!(format!("{token}"), "[redacted]");
    }

    #[test]
    fn redaction_removes_the_token_from_arbitrary_text() {
        let token = Token::new("ghp_secretvalue");
        let logged = token.redact("Authorization: Bearer ghp_secretvalue failed");
        assert!(!logged.contains("ghp_secretvalue"));
        assert!(logged.contains("[redacted]"));
    }

    #[test]
    fn an_absent_environment_variable_yields_no_token() {
        assert!(Token::from_env("QUAY_TOKEN_THAT_DOES_NOT_EXIST").is_none());
    }
}
