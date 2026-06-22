//! `conventions` command family — CliApp impl methods.

use std::path::Path;

use crate::{CliApp, CommandOutcome, error::CompositionError};

impl CliApp {
    /// Create a new convention document and update the README index.
    ///
    /// # Errors
    /// Returns `Err` when the slug is invalid, README is missing or lacks markers,
    /// the document already exists, or any I/O operation fails.
    pub fn conventions_add(
        &self,
        project_root: &Path,
        name: &str,
        slug: Option<&str>,
        title: Option<&str>,
        summary: Option<&str>,
    ) -> Result<CommandOutcome, CompositionError> {
        infrastructure::conventions::add_convention_doc(project_root, name, slug, title, summary)
            .map(|()| CommandOutcome::success(Some("[OK] Convention document added.".to_owned())))
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))
    }

    /// Regenerate the README.md index from current convention documents.
    ///
    /// # Errors
    /// Returns `Err` when README is missing, markers are absent, or any I/O operation fails.
    pub fn conventions_update_index(
        &self,
        project_root: &Path,
    ) -> Result<CommandOutcome, CompositionError> {
        infrastructure::conventions::update_convention_index(project_root)
            .map(|()| {
                CommandOutcome::success(Some("[OK] Convention README index updated.".to_owned()))
            })
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))
    }

    /// Verify that the README.md indexes all convention documents.
    ///
    /// Returns exit 0 when the index is in sync, exit 1 with findings otherwise.
    ///
    /// # Errors
    /// Returns `Err` only on unexpected infrastructure failures.
    pub fn conventions_verify_index(
        &self,
        project_root: &Path,
    ) -> Result<CommandOutcome, CompositionError> {
        let outcome = infrastructure::conventions::verify_convention_index(project_root);
        if outcome.is_ok() {
            Ok(CommandOutcome::success(Some("[OK] Convention README index is in sync.".to_owned())))
        } else {
            let messages: Vec<String> = outcome.findings().iter().map(|f| f.to_string()).collect();
            let stderr = messages.join("\n");
            Ok(CommandOutcome { stdout: None, stderr: Some(stderr), exit_code: 1 })
        }
    }
}
