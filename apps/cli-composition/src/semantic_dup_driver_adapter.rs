//! Adapter implementing [`usecase::semantic_dup_driver::SemanticDupDriverPort`].
//!
//! Delegates to [`SemanticDupCompositionRoot`] methods, converting
//! `CompositionError` to `SemanticDupDriverOutcome::failure`.

use usecase::semantic_dup_driver::{
    DupCheckDriverInput, FindSimilarDriverInput, IndexBuildDriverInput,
    IndexMeasureQualityDriverInput, SemanticDupDriverOutcome, SemanticDupDriverPort,
};

use crate::semantic_dup::{
    DupCheckInput, DupIndexBuildInput, DupIndexMeasureQualityInput, FindSimilarInput,
    SemanticDupCompositionRoot,
};

// ---------------------------------------------------------------------------
// Adapter struct
// ---------------------------------------------------------------------------

/// Adapter implementing `SemanticDupDriverPort` by delegating to
/// `SemanticDupCompositionRoot` methods.
pub struct SemanticDupDriverAdapter {
    root: SemanticDupCompositionRoot,
}

impl SemanticDupDriverAdapter {
    /// Create a new adapter.
    pub fn new() -> Self {
        Self { root: SemanticDupCompositionRoot::new() }
    }
}

impl Default for SemanticDupDriverAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Port implementation
// ---------------------------------------------------------------------------

impl SemanticDupDriverPort for SemanticDupDriverAdapter {
    fn find_similar(&self, input: FindSimilarDriverInput) -> SemanticDupDriverOutcome {
        let composition_input = FindSimilarInput {
            fragment_text: input.fragment_text,
            file_path: input.file_path,
            top_k: input.top_k,
            db_path: input.db_path,
        };
        match self.root.semantic_dup_find_similar(composition_input) {
            Ok(outcome) => SemanticDupDriverOutcome {
                stdout: outcome.stdout,
                stderr: outcome.stderr,
                exit_code: outcome.exit_code,
            },
            Err(e) => SemanticDupDriverOutcome::failure(Some(e.to_string())),
        }
    }

    fn index_build(&self, input: IndexBuildDriverInput) -> SemanticDupDriverOutcome {
        let composition_input =
            DupIndexBuildInput { workspace_root: input.workspace_root, db_path: input.db_path };
        match self.root.semantic_dup_index_build(composition_input) {
            Ok(outcome) => SemanticDupDriverOutcome {
                stdout: outcome.stdout,
                stderr: outcome.stderr,
                exit_code: outcome.exit_code,
            },
            Err(e) => SemanticDupDriverOutcome::failure(Some(e.to_string())),
        }
    }

    fn index_measure_quality(
        &self,
        input: IndexMeasureQualityDriverInput,
    ) -> SemanticDupDriverOutcome {
        let composition_input =
            DupIndexMeasureQualityInput { workspace_root: input.workspace_root };
        match self.root.semantic_dup_index_measure_quality(composition_input) {
            Ok(outcome) => SemanticDupDriverOutcome {
                stdout: outcome.stdout,
                stderr: outcome.stderr,
                exit_code: outcome.exit_code,
            },
            Err(e) => SemanticDupDriverOutcome::failure(Some(e.to_string())),
        }
    }

    fn dup_check(&self, input: DupCheckDriverInput) -> SemanticDupDriverOutcome {
        let composition_input = DupCheckInput {
            files_from: input.files_from,
            threshold: input.threshold,
            db_path: input.db_path,
            ack_file: input.ack_file,
            ack: input.ack,
        };
        match self.root.semantic_dup_check(composition_input) {
            Ok(outcome) => SemanticDupDriverOutcome {
                stdout: outcome.stdout,
                stderr: outcome.stderr,
                exit_code: outcome.exit_code,
            },
            Err(e) => SemanticDupDriverOutcome::failure(Some(e.to_string())),
        }
    }
}
