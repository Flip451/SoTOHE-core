//! `conventions` command family — primary adapter driver.
//!
//! `ConventionsDriver` holds an injected [`usecase::conventions::ConventionsPort`]
//! and exposes `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::conventions::ConventionsPort;

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
/// Holds an injected [`ConventionsPort`]; exposes `handle(input) -> CommandOutcome`.
pub struct ConventionsDriver {
    port: Arc<dyn ConventionsPort>,
}

impl ConventionsDriver {
    /// Create a new `ConventionsDriver` with the given port.
    pub fn new(port: Arc<dyn ConventionsPort>) -> Self {
        Self { port }
    }

    /// Handle a conventions command.
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

    fn conventions_add(
        &self,
        project_root: PathBuf,
        name: String,
        slug: Option<String>,
        title: Option<String>,
        summary: Option<String>,
    ) -> CommandOutcome {
        match self.port.add_convention(
            project_root.as_path(),
            &name,
            slug.as_deref(),
            title.as_deref(),
            summary.as_deref(),
        ) {
            Ok(msg) => CommandOutcome::success(Some(msg)),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn conventions_update_index(&self, project_root: PathBuf) -> CommandOutcome {
        match self.port.update_index(project_root.as_path()) {
            Ok(msg) => CommandOutcome::success(Some(msg)),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn conventions_verify_index(&self, project_root: PathBuf) -> CommandOutcome {
        match self.port.verify_index(project_root.as_path()) {
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
            Ok(result) => {
                if result.ok {
                    CommandOutcome::success(Some(
                        "[OK] Convention README index is in sync.".to_owned(),
                    ))
                } else {
                    let stderr = result.findings.join("\n");
                    CommandOutcome { stdout: None, stderr: Some(stderr), exit_code: 1 }
                }
            }
        }
    }
}
