//! `arch` command family — CliApp impl methods.

use std::path::Path;

use infrastructure::arch::ArchRulesError;

use crate::{CliApp, CommandOutcome};

fn render(
    f: impl FnOnce(&Path) -> Result<String, ArchRulesError>,
    root: &Path,
) -> Result<CommandOutcome, String> {
    f(root).map(|output| CommandOutcome::success(Some(output))).map_err(|e| e.to_string())
}

impl CliApp {
    /// Render the workspace tree (crate paths only).
    ///
    /// # Errors
    /// Returns `Err` when the architecture rules file cannot be read, parsed, or is structurally
    /// invalid.
    pub fn arch_tree(&self, project_root: &Path) -> Result<CommandOutcome, String> {
        render(infrastructure::arch::render_workspace_tree, project_root)
    }

    /// Render the workspace tree including extra_dirs.
    ///
    /// # Errors
    /// Returns `Err` when the architecture rules file cannot be read, parsed, or is structurally
    /// invalid.
    pub fn arch_tree_full(&self, project_root: &Path) -> Result<CommandOutcome, String> {
        render(infrastructure::arch::render_workspace_tree_full, project_root)
    }

    /// List workspace member paths (one per line).
    ///
    /// # Errors
    /// Returns `Err` when the architecture rules file cannot be read, parsed, or is structurally
    /// invalid.
    pub fn arch_members(&self, project_root: &Path) -> Result<CommandOutcome, String> {
        render(infrastructure::arch::render_workspace_members, project_root)
    }

    /// Print the direct dependency check matrix.
    ///
    /// # Errors
    /// Returns `Err` when the architecture rules file cannot be read, parsed, or is structurally
    /// invalid.
    pub fn arch_direct_checks(&self, project_root: &Path) -> Result<CommandOutcome, String> {
        render(infrastructure::arch::render_direct_checks, project_root)
    }
}
