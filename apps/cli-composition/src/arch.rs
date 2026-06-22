//! `arch` command family — `ArchCompositionRoot` impl methods.

use std::path::Path;

use infrastructure::arch::ArchRulesError;

use crate::{CommandOutcome, error::CompositionError};

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `arch` command family.
///
/// This family has no injectable adapter dependencies; adapters are
/// constructed inline inside each method (infrastructure::arch::* functions).
pub struct ArchCompositionRoot;

impl ArchCompositionRoot {
    /// Create a new `ArchCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ArchCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

fn render(
    f: impl FnOnce(&Path) -> Result<String, ArchRulesError>,
    root: &Path,
) -> Result<CommandOutcome, CompositionError> {
    f(root)
        .map(|output| CommandOutcome::success(Some(output)))
        .map_err(|e| CompositionError::Infrastructure(e.to_string()))
}

impl ArchCompositionRoot {
    /// Render the workspace tree (crate paths only).
    ///
    /// # Errors
    /// Returns `Err` when the architecture rules file cannot be read, parsed, or is structurally
    /// invalid.
    pub fn arch_tree(&self, project_root: &Path) -> Result<CommandOutcome, CompositionError> {
        render(infrastructure::arch::render_workspace_tree, project_root)
    }

    /// Render the workspace tree including extra_dirs.
    ///
    /// # Errors
    /// Returns `Err` when the architecture rules file cannot be read, parsed, or is structurally
    /// invalid.
    pub fn arch_tree_full(&self, project_root: &Path) -> Result<CommandOutcome, CompositionError> {
        render(infrastructure::arch::render_workspace_tree_full, project_root)
    }

    /// List workspace member paths (one per line).
    ///
    /// # Errors
    /// Returns `Err` when the architecture rules file cannot be read, parsed, or is structurally
    /// invalid.
    pub fn arch_members(&self, project_root: &Path) -> Result<CommandOutcome, CompositionError> {
        render(infrastructure::arch::render_workspace_members, project_root)
    }

    /// Print the direct dependency check matrix.
    ///
    /// # Errors
    /// Returns `Err` when the architecture rules file cannot be read, parsed, or is structurally
    /// invalid.
    pub fn arch_direct_checks(
        &self,
        project_root: &Path,
    ) -> Result<CommandOutcome, CompositionError> {
        render(infrastructure::arch::render_direct_checks, project_root)
    }
}
