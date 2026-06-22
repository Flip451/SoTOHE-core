//! `DryCompositionRoot` definition and `impl CliApp` delegation shims for the
//! `dry` command family.
//!
//! Each `CliApp` method forwards to `DryCompositionRoot::new().method(...)`,
//! preserving `apps/cli` call sites unchanged during the per-context dissolution
//! migration (T013).

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
