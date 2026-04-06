//! Error types for the v1 review domain module.

use thiserror::Error;

/// Errors returned by review domain operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReviewError {
    #[error("invalid concern: {0}")]
    InvalidConcern(String),
    #[error("stale code hash: stored={stored}, current={current}")]
    StaleCodeHash { stored: String, current: String },
}
