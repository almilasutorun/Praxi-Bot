//! Errors at the adapter boundary; concrete service errors stay in the adapter.

use thiserror::Error;

/// Convenience result for the shared channel contract.
pub type Result<T> = std::result::Result<T, ChannelError>;

/// Failure from a messaging adapter.
#[derive(Debug, Error)]
pub enum ChannelError {
    /// Identifier or configuration is invalid for this adapter.
    #[error("{0}")]
    InvalidInput(String),
    /// The adapter does not implement this operation.
    #[error("unsupported channel operation: {0}")]
    Unsupported(&'static str),
    /// Service-specific failure, retaining its original display and source.
    #[error(transparent)]
    Adapter(Box<dyn std::error::Error + Send + Sync>),
}
