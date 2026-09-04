#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("the store refused the write: {0}")]
    Store(#[from] quay_store::StoreError),

    #[error("the forge refused the request: {0}")]
    Forge(#[from] quay_forge::ForgeError),
}
