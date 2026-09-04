use std::sync::mpsc;
use std::time::Duration;

use crate::error::ForgeError;
use crate::token::Token;

pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(5);

pub struct Keychain {
    service: String,
}

pub fn within_deadline<T, F>(deadline: Duration, work: F) -> Option<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work());
    });
    receiver.recv_timeout(deadline).ok()
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
        self.load_within(account, DEFAULT_DEADLINE)
    }

    pub fn load_within(
        &self,
        account: &str,
        deadline: Duration,
    ) -> Result<Option<Token>, ForgeError> {
        let service = self.service.clone();
        let account = account.to_owned();
        let read = move || -> Result<Option<Token>, ForgeError> {
            match keyring::Entry::new(&service, &account)
                .map_err(describe)?
                .get_password()
            {
                Ok(secret) => Ok(Some(Token::new(secret))),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(error) => Err(describe(error)),
            }
        };
        within_deadline(deadline, read).unwrap_or(Err(ForgeError::CredentialStoreDidNotAnswer {
            seconds: deadline.as_secs(),
        }))
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
    use std::time::Duration;

    use super::{Keychain, within_deadline};
    use crate::token::Token;

    fn service() -> String {
        format!("quay.test.{}", std::process::id())
    }

    #[test]
    fn work_that_answers_in_time_is_returned() {
        assert_eq!(within_deadline(Duration::from_secs(5), || 7), Some(7));
    }

    #[test]
    fn work_that_never_answers_gives_up_instead_of_blocking_the_process() {
        let slow = || {
            std::thread::sleep(Duration::from_secs(30));
            7
        };
        assert_eq!(within_deadline(Duration::from_millis(50), slow), None);
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
