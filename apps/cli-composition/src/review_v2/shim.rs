//! `ReviewCompositionRoot` definition and `impl CliApp` delegation shims for
//! the `review_v2` command family.
//!
//! Each `CliApp` method forwards to `ReviewCompositionRoot::new().method(...)`,
//! preserving `apps/cli` call sites unchanged during the per-context dissolution
//! migration (T013).

// ── Per-context composition root ──────────────────────────────────────────────

/// Composition root for the `review_v2` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct ReviewCompositionRoot;

impl ReviewCompositionRoot {
    /// Create a new `ReviewCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReviewCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}
