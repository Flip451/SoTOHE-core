// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `arch` command family — primary adapter driver.
//!
//! `ArchDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The render helpers here mirror
//! `apps/cli-composition/src/arch.rs`; T021 removes the `cli_composition`
//! duplicate when the live path is flipped.

// TODO(T021): add infrastructure imports once Cargo.toml is materialized.
// use std::path::Path;
// use infrastructure::arch::ArchRulesError;

use std::path::PathBuf;

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
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct ArchDriver {
    // TODO(T021): inject use-case interactors here (currently this family has
    // no injectable adapter dependencies — infrastructure functions are called
    // inline, same as cli_composition::ArchCompositionRoot).
}

impl ArchDriver {
    /// Create a new `ArchDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle an arch command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
    pub fn handle(&self, input: ArchInput) -> CommandOutcome {
        match input {
            ArchInput::Tree { project_root } => self.arch_tree(project_root),
            ArchInput::TreeFull { project_root } => self.arch_tree_full(project_root),
            ArchInput::Members { project_root } => self.arch_members(project_root),
            ArchInput::DirectChecks { project_root } => self.arch_direct_checks(project_root),
        }
    }

    // -----------------------------------------------------------------------
    // Render helpers (logic duplicated from cli_composition/src/arch.rs;
    // T021 removes the cli_composition copy).
    // -----------------------------------------------------------------------

    fn arch_tree(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::arch::render_workspace_tree here.
        // Mirrors cli_composition/src/arch.rs ArchCompositionRoot::arch_tree.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn arch_tree_full(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::arch::render_workspace_tree_full here.
        // Mirrors cli_composition/src/arch.rs ArchCompositionRoot::arch_tree_full.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn arch_members(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::arch::render_workspace_members here.
        // Mirrors cli_composition/src/arch.rs ArchCompositionRoot::arch_members.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn arch_direct_checks(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::arch::render_direct_checks here.
        // Mirrors cli_composition/src/arch.rs ArchCompositionRoot::arch_direct_checks.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }
}

impl Default for ArchDriver {
    fn default() -> Self {
        Self::new()
    }
}
