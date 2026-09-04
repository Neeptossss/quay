use crate::error::ForgeError;
use crate::token::Token;

pub struct Keychain {
    service: String,
}

impl Keychain {
    pub fn for_service(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    pub fn store(&self, account: &str, token: &Token) -> Result<(), ForgeError> {
        self.entry(account)?
            .set_password(token.expose_for_credential_store())
            .map_err(describe)
    }

    pub fn load(&self, account: &str) -> Result<Option<Token>, ForgeError> {
        match self.entry(account)?.get_password() {
            Ok(secret) => Ok(Some(Token::new(secret))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(describe(error)),
        }
    }

    pub fn delete(&self, account: &str) -> Result<(), ForgeError> {
        match self.entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(describe(error)),
        }
    }

    fn entry(&self, account: &str) -> Result<keyring::Entry, ForgeError> {
        keyring::Entry::new(&self.service, account).map_err(describe)
    }
}

fn describe(error: keyring::Error) -> ForgeError {
    ForgeError::CredentialStore(error.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::Keychain;
    use crate::token::Token;

    fn service() -> String {
        format!("quay.test.{}", std::process::id())
    }

    #[test]
    #[ignore = "touches the operating system credential store, absent from headless CI"]
    fn a_stored_token_comes_back_unchanged() {
        let keychain = Keychain::for_service(service());
        let token = Token::new("ghp_storedvalue");
        keychain.store("octocat", &token).unwrap();
        match keychain.load("octocat") {
            Ok(Some(loaded)) => assert_eq!(loaded.header_value(), token.header_value()),
            other => panic!("the token must come back: {other:?}"),
        }
        keychain.delete("octocat").unwrap();
    }

    #[test]
    #[ignore = "touches the operating system credential store, absent from headless CI"]
    fn an_unknown_account_reads_as_absent_rather_than_failing() {
        let keychain = Keychain::for_service(service());
        match keychain.load("nobody") {
            Ok(None) => {}
            other => panic!("an unknown account must read as absent: {other:?}"),
        }
    }

    #[test]
    #[ignore = "touches the operating system credential store, absent from headless CI"]
    fn deleting_an_absent_credential_is_not_an_error() {
        let keychain = Keychain::for_service(service());
        assert!(keychain.delete("nobody").is_ok());
    }
}
