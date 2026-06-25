//! `verify` command family — per-context composition root and CliApp shim.

use std::path::PathBuf;

use crate::{CommandOutcome, error::CompositionError};

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
    ///
    /// Wire chain: `FsVerifyAdapter` → `VerifyInteractor` → `VerifyDriver`.
    pub fn verify_driver(&self) -> cli_driver::verify::VerifyDriver {
        use infrastructure::FsVerifyAdapter;
        use std::sync::Arc;
        use usecase::verify::{VerifyInteractor, VerifyPort};

        let adapter = Arc::new(FsVerifyAdapter::new());
        let interactor = Arc::new(VerifyInteractor::new(adapter as Arc<dyn VerifyPort>));
        cli_driver::verify::VerifyDriver::new(interactor)
    }
}
