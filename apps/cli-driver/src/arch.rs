//! `arch` command family — primary adapter driver.
//!
//! `ArchDriver` holds an injected [`usecase::arch::ArchService`] and exposes
//! `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::arch::ArchService;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `arch` command family.
pub enum ArchInput {
    /// Render the workspace tree (crate paths only).
    Tree {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Render the workspace tree including extra_dirs.
    TreeFull {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// List workspace member paths (one per line).
    Members {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Print the direct dependency check matrix.
    DirectChecks {
        /// Project root directory.
        project_root: PathBuf,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `arch` command family.
///
/// Holds an injected [`ArchService`]; exposes `handle(input) -> CommandOutcome`.
pub struct ArchDriver {
    service: Arc<dyn ArchService>,
}

impl ArchDriver {
    /// Create a new `ArchDriver` with the given `ArchService`.
    pub fn new(service: Arc<dyn ArchService>) -> Self {
        Self { service }
    }

    /// Handle an arch command.
    pub fn handle(&self, input: ArchInput) -> CommandOutcome {
        match input {
            ArchInput::Tree { project_root } => self.arch_tree(project_root),
            ArchInput::TreeFull { project_root } => self.arch_tree_full(project_root),
            ArchInput::Members { project_root } => self.arch_members(project_root),
            ArchInput::DirectChecks { project_root } => self.arch_direct_checks(project_root),
        }
    }

    fn arch_tree(&self, project_root: PathBuf) -> CommandOutcome {
        match self.service.render_tree(project_root) {
            Ok(output) => CommandOutcome::success(Some(output)),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn arch_tree_full(&self, project_root: PathBuf) -> CommandOutcome {
        match self.service.render_tree_full(project_root) {
            Ok(output) => CommandOutcome::success(Some(output)),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn arch_members(&self, project_root: PathBuf) -> CommandOutcome {
        match self.service.render_members(project_root) {
            Ok(output) => CommandOutcome::success(Some(output)),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn arch_direct_checks(&self, project_root: PathBuf) -> CommandOutcome {
        match self.service.render_direct_checks(project_root) {
            Ok(output) => CommandOutcome::success(Some(output)),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }
}
