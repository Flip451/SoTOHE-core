//! Shared error type for the D4 orchestration extraction services.
//!
//! `D4OrchestrationError` is the shared error envelope for application services
//! added by the `cli-composition-split-presentation-layer-2026-06-21` track:
//! `DryFragmentPipelineService`, `FixpointDryGateService`, and
//! `PrReviewPollingService`. String payloads carry opaque diagnostic text
//! mapped at the usecase boundary and rendered by `cli_driver`.

use std::fmt;

// ── D4OrchestrationError ──────────────────────────────────────────────────────

/// Shared error type for D4 orchestration extraction services.
///
/// String payloads are opaque diagnostic text mapped at the usecase boundary
/// and rendered by `cli_driver`; they are not domain value objects.
///
/// Variants:
/// - `DiffFragment` — diff hunk listing or fragment extraction pipeline failure.
/// - `DryGate` — dry-check approval gate failure.
/// - `PrPolling` — PR review polling failure.
#[derive(Debug, Clone)]
pub enum D4OrchestrationError {
    /// Diff hunk listing or fragment extraction pipeline failure.
    DiffFragment(String),
    /// Dry-check approval gate failure.
    DryGate(String),
    /// PR review polling failure.
    PrPolling(String),
}

impl fmt::Display for D4OrchestrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DiffFragment(msg) => write!(f, "diff-fragment pipeline error: {msg}"),
            Self::DryGate(msg) => write!(f, "dry gate error: {msg}"),
            Self::PrPolling(msg) => write!(f, "PR polling error: {msg}"),
        }
    }
}

impl std::error::Error for D4OrchestrationError {}
