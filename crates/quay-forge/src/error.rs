#[derive(Debug, thiserror::Error)]
pub enum ForgeError {
    #[error("read-only mode rejected a {kind} request to {url}")]
    ReadOnlyModeRejected { kind: &'static str, url: String },

    #[error("the write allowlist rejected a request targeting {repo}")]
    WriteAllowlistRejected { repo: String },

    #[error("a write request named no repository, which the write allowlist requires")]
    WriteTargetMissing,

    #[error("a {kind} request may not use the {method} method")]
    MethodMismatch { kind: &'static str, method: String },

    #[error("a request declared as a GraphQL query carries a mutation operation")]
    MutationDeclaredAsQuery,

    #[error("the hourly budget of {budget} requests is exhausted for another {retry_in_seconds}s")]
    BudgetExhausted { budget: u32, retry_in_seconds: u64 },

    #[error(
        "the speculation budget of {budget} requests is exhausted for another {retry_in_seconds}s"
    )]
    SpeculationBudgetExhausted { budget: u32, retry_in_seconds: u64 },

    #[error("the forge throttled the request {attempts} times, last status {last_status}")]
    Throttled { attempts: u32, last_status: u16 },

    #[error("the forge rejected the token: {message}")]
    TokenRejected { message: String },

    #[error("the forge refused access: {message}")]
    AccessForbidden { message: String },

    #[error("the forge answered {status}: {message}")]
    UnexpectedStatus { status: u16, message: String },

    #[error("the forge answered with a payload this client cannot read: {message}")]
    MalformedPayload { message: String },

    #[error("the operating system credential store refused the request: {0}")]
    CredentialStore(String),

    #[error("the client could not be built: {0}")]
    ClientBuild(String),

    #[error("transport failure: {0}")]
    Transport(String),
}
