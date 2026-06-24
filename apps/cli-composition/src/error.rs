//! Typed error boundary for all `CliApp` public methods.
//!
//! `CompositionError` replaces the stringly-typed `CommandOutcome` boundary
//! (ADR 2026-06-21-1328 D2 / spec AC-05). Every variant wraps a
//! human-readable `String` message so that callers can always convert to a plain
//! string with `.to_string()` when needed (e.g. `CliError::Message` in apps/cli).

use thiserror::Error;

/// Typed error for construction and wiring failures at the `cli_composition` boundary.
///
/// Callers in `apps/cli` should convert via `.map_err(|e| e.to_string())` unless
/// specific exit-code routing per variant is desired.
#[derive(Debug, Error)]
pub enum CompositionError {
    /// Config file load errors (e.g. signal-gates.json, agent-profiles.json).
    #[error("{0}")]
    ConfigLoad(String),

    /// Adapter constructor failures (e.g. git discovery, infra initialization).
    #[error("{0}")]
    AdapterInit(String),

    /// Generic wiring layer failure (composition / DI wiring errors).
    #[error("{0}")]
    WiringFailed(String),

    /// Usecase invocation error mapped to string.
    #[error("{0}")]
    Usecase(String),

    /// Infrastructure error mapped to string.
    #[error("{0}")]
    Infrastructure(String),
}
