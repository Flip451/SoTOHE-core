//! `DryCompositionRoot` definition and `impl CliApp` delegation shims for the
//! `dry` command family.
//!
//! Each `CliApp` method forwards to `DryCompositionRoot::new().method(...)`,
//! preserving `apps/cli` call sites unchanged during the per-context dissolution
//! migration (T013).

use super::{DryCheckApprovedInput, DryResultsInput, DryWriteInput};
use crate::{CliApp, CommandOutcome, error::CompositionError};

// ── Per-context composition root ──────────────────────────────────────────────

/// Composition root for the `dry` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct DryCompositionRoot;

impl DryCompositionRoot {
    /// Create a new `DryCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DryCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

// ── CliApp delegation shims ───────────────────────────────────────────────────

impl CliApp {
    /// Delegates to [`DryCompositionRoot::dry_write`].
    pub fn dry_write(&self, input: DryWriteInput) -> Result<CommandOutcome, CompositionError> {
        DryCompositionRoot::new().dry_write(input)
    }

    /// Delegates to [`DryCompositionRoot::dry_results`].
    pub fn dry_results(&self, input: DryResultsInput) -> Result<CommandOutcome, CompositionError> {
        DryCompositionRoot::new().dry_results(input)
    }

    /// Delegates to [`DryCompositionRoot::dry_check_approved`].
    pub fn dry_check_approved(
        &self,
        input: DryCheckApprovedInput,
    ) -> Result<CommandOutcome, CompositionError> {
        DryCompositionRoot::new().dry_check_approved(input)
    }
}
