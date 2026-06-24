//! `verify` command family — per-context composition root and CliApp shim.

use std::path::PathBuf;

use crate::{CommandOutcome, cmd_outcome::render_outcome, error::CompositionError};

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `verify` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct VerifyCompositionRoot;

impl VerifyCompositionRoot {
    /// Create a new `VerifyCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for VerifyCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute catalogue-spec-refs verification, returning a `CommandOutcome`.
///
/// Validates the track id, delegates I/O to the infrastructure layer,
/// and maps the result to a `CommandOutcome`.
pub fn execute_catalogue_spec_refs(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    skip_stale: bool,
) -> Result<CommandOutcome, CompositionError> {
    // Validate track id (path traversal guard) — delegates to the canonical domain rule.
    crate::track::validate_track_id_str(&track_id)
        .map_err(|e| CompositionError::WiringFailed(format!("invalid track ID: {e}")))?;

    let mut all_formatted_findings: Vec<String> = Vec::new();
    let no_findings =
        infrastructure::verify::catalogue_spec_refs::execute_verify_catalogue_spec_refs(
            items_dir,
            track_id,
            workspace_root,
            skip_stale,
            &mut all_formatted_findings,
        )
        .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;

    if no_findings {
        Ok(CommandOutcome::success(Some("[OK] catalogue-spec-refs: no findings".to_owned())))
    } else {
        let stderr = all_formatted_findings
            .iter()
            .chain(std::iter::once(&format!(
                "[FAIL] catalogue-spec-refs: {} finding(s)",
                all_formatted_findings.len()
            )))
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        Ok(CommandOutcome { stdout: None, stderr: Some(stderr), exit_code: 1 })
    }
}

impl VerifyCompositionRoot {
    /// Build a wired [`cli_driver::verify::VerifyDriver`] for the verify family.
    pub fn verify_driver(&self) -> cli_driver::verify::VerifyDriver {
        use infrastructure::FsVerifyAdapter;
        use std::sync::Arc;

        let port = Arc::new(FsVerifyAdapter::new());
        cli_driver::verify::VerifyDriver::new(port)
    }

    /// Check local Git config uses `.githooks` as `core.hooksPath`.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_hooks_path(
        &self,
        project_root: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        let outcome = infrastructure::verify::hooks_path::verify(&project_root);
        Ok(render_outcome("verify hooks path", &outcome))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use infrastructure::verify::test_support::run_git;

    use super::*;

    #[test]
    fn test_verify_hooks_path_with_githooks_configured_returns_success() {
        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["init"]);
        run_git(dir.path(), &["config", "--local", "core.hooksPath", ".githooks"]);

        let outcome =
            VerifyCompositionRoot::new().verify_hooks_path(dir.path().to_path_buf()).unwrap();

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.unwrap();
        assert!(stdout.contains("--- verify hooks path ---"));
        assert!(stdout.contains("[OK] All checks passed."));
        assert!(stdout.contains("--- verify hooks path PASSED ---"));
        assert!(outcome.stderr.is_none());
    }

    #[test]
    fn test_verify_hooks_path_with_unset_config_returns_failure() {
        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["init"]);

        let outcome =
            VerifyCompositionRoot::new().verify_hooks_path(dir.path().to_path_buf()).unwrap();

        assert_eq!(outcome.exit_code, 1);
        let stdout = outcome.stdout.unwrap();
        assert!(stdout.contains("--- verify hooks path ---"));
        assert!(stdout.contains("core.hooksPath is not set to .githooks"));
        assert!(stdout.contains("--- verify hooks path FAILED ---"));
        assert!(outcome.stderr.is_none());
    }
}
