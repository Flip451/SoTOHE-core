//! Adapter implementing [`usecase::dry_driver::DryDriverPort`].
//!
//! Delegates to [`DryCompositionRoot`] and [`DryFixRunnerCompositionRoot`]
//! methods, converting `CompositionError` to `DryDriverOutcome::failure`.

use usecase::dry_driver::{
    DryCheckApprovedDriverInput, DryDriverOutcome, DryDriverPort, DryFixLocalDriverInput,
    DryResultsDriverInput, DryWriteDriverInput,
};

use crate::dry::RunDryFixLocalInput;
use crate::dry::{DryCheckApprovedInput, DryCompositionRoot, DryResultsInput, DryWriteInput};
use crate::dry_fix_runner::DryFixRunnerCompositionRoot;

// ---------------------------------------------------------------------------
// Adapter struct
// ---------------------------------------------------------------------------

/// Adapter implementing `DryDriverPort` by delegating to `DryCompositionRoot`
/// and `DryFixRunnerCompositionRoot`.
pub struct DryDriverAdapter {
    dry_root: DryCompositionRoot,
    fix_root: DryFixRunnerCompositionRoot,
}

impl DryDriverAdapter {
    /// Create a new adapter.
    pub fn new() -> Self {
        Self { dry_root: DryCompositionRoot::new(), fix_root: DryFixRunnerCompositionRoot::new() }
    }
}

impl Default for DryDriverAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Port implementation
// ---------------------------------------------------------------------------

impl DryDriverPort for DryDriverAdapter {
    fn dry_write(&self, input: DryWriteDriverInput) -> DryDriverOutcome {
        let composition_input = DryWriteInput {
            track_id: input.track_id,
            base_commit: input.base_commit,
            db_path: input.db_path,
            threshold: input.threshold,
            workspace_root: input.workspace_root,
            items_dir: input.items_dir,
            model: input.model,
            capability_name: input.capability_name,
        };
        match self.dry_root.dry_write(composition_input) {
            Ok(outcome) => DryDriverOutcome {
                stdout: outcome.stdout,
                stderr: outcome.stderr,
                exit_code: outcome.exit_code,
            },
            Err(e) => DryDriverOutcome::failure(Some(e.to_string())),
        }
    }

    fn dry_results(&self, input: DryResultsDriverInput) -> DryDriverOutcome {
        let composition_input = DryResultsInput {
            track_id: input.track_id,
            filter: input.filter,
            items_dir: input.items_dir,
        };
        match self.dry_root.dry_results(composition_input) {
            Ok(outcome) => DryDriverOutcome {
                stdout: outcome.stdout,
                stderr: outcome.stderr,
                exit_code: outcome.exit_code,
            },
            Err(e) => DryDriverOutcome::failure(Some(e.to_string())),
        }
    }

    fn dry_check_approved(&self, input: DryCheckApprovedDriverInput) -> DryDriverOutcome {
        let composition_input = DryCheckApprovedInput {
            track_id: input.track_id,
            base_commit: input.base_commit,
            items_dir: input.items_dir,
        };
        match self.dry_root.dry_check_approved(composition_input) {
            Ok(outcome) => DryDriverOutcome {
                stdout: outcome.stdout,
                stderr: outcome.stderr,
                exit_code: outcome.exit_code,
            },
            Err(e) => DryDriverOutcome::failure(Some(e.to_string())),
        }
    }

    fn dry_fix_local(&self, input: DryFixLocalDriverInput) -> DryDriverOutcome {
        let composition_input = RunDryFixLocalInput {
            track_id: input.track_id,
            briefing_file: input.briefing_file,
            model: input.model,
        };
        match self.fix_root.dry_run_fix_local(composition_input) {
            Ok(outcome) => DryDriverOutcome {
                stdout: outcome.stdout,
                stderr: outcome.stderr,
                exit_code: outcome.exit_code,
            },
            Err(e) => DryDriverOutcome::failure(Some(e.to_string())),
        }
    }
}
