// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `conventions` command family — primary adapter driver.
//!
//! `ConventionsDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The render helpers here mirror
//! `apps/cli-composition/src/conventions.rs`; T021 removes the
//! `cli_composition` duplicate when the live path is flipped.

// TODO(T021): add infrastructure imports once Cargo.toml is materialized.
// use std::path::Path;
// use infrastructure::conventions::{
//     add_convention_doc, update_convention_index, verify_convention_index,
// };

use std::path::PathBuf;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `conventions` command family.
pub enum ConventionsInput {
    /// Create a new convention document and update the README index.
    Add {
        /// Project root directory.
        project_root: PathBuf,
        /// Convention name (used as the document file stem when slug is absent).
        name: String,
        /// Optional slug override.
        slug: Option<String>,
        /// Optional document title.
        title: Option<String>,
        /// Optional one-line summary for the README index entry.
        summary: Option<String>,
    },
    /// Regenerate the README.md index from current convention documents.
    UpdateIndex {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Verify that the README.md indexes all convention documents.
    VerifyIndex {
        /// Project root directory.
        project_root: PathBuf,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `conventions` command family.
///
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct ConventionsDriver {
    // TODO(T021): inject use-case interactors here (currently this family has
    // no injectable adapter dependencies — infrastructure functions are called
    // inline, same as cli_composition::ConventionsCompositionRoot).
}

impl ConventionsDriver {
    /// Create a new `ConventionsDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a conventions command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
    pub fn handle(&self, input: ConventionsInput) -> CommandOutcome {
        match input {
            ConventionsInput::Add { project_root, name, slug, title, summary } => {
                self.conventions_add(project_root, name, slug, title, summary)
            }
            ConventionsInput::UpdateIndex { project_root } => {
                self.conventions_update_index(project_root)
            }
            ConventionsInput::VerifyIndex { project_root } => {
                self.conventions_verify_index(project_root)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Render helpers (logic duplicated from cli_composition/src/conventions.rs;
    // T021 removes the cli_composition copy).
    // -----------------------------------------------------------------------

    fn conventions_add(
        &self,
        _project_root: PathBuf,
        _name: String,
        _slug: Option<String>,
        _title: Option<String>,
        _summary: Option<String>,
    ) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::conventions::add_convention_doc here.
        // Mirrors cli_composition/src/conventions.rs
        // ConventionsCompositionRoot::conventions_add.
        // Success stdout: "[OK] Convention document added."
        CommandOutcome::success(Some("[OK] Convention document added.".to_owned()))
    }

    fn conventions_update_index(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::conventions::update_convention_index here.
        // Mirrors cli_composition/src/conventions.rs
        // ConventionsCompositionRoot::conventions_update_index.
        // Success stdout: "[OK] Convention README index updated."
        CommandOutcome::success(Some("[OK] Convention README index updated.".to_owned()))
    }

    fn conventions_verify_index(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::conventions::verify_convention_index here.
        // Mirrors cli_composition/src/conventions.rs
        // ConventionsCompositionRoot::conventions_verify_index.
        // Success stdout: "[OK] Convention README index is in sync."
        // Failure: stderr = findings joined by "\n", exit_code = 1.
        CommandOutcome::success(Some("[OK] Convention README index is in sync.".to_owned()))
    }
}

impl Default for ConventionsDriver {
    fn default() -> Self {
        Self::new()
    }
}
