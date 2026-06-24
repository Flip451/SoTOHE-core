//! Infrastructure adapter implementing [`usecase::dry_check::CodeFragmentExtractorPort`].
//!
//! Wraps [`crate::semantic_dup::extractor::extract_code_fragments`] behind the
//! usecase secondary port. The adapter owns no state; it is a unit struct with
//! no constructor arguments.
//!
//! Relocated responsibility: previously the CLI composition root called
//! `extract_code_fragments` directly. After T007 the call goes through the
//! usecase port boundary.

use domain::semantic_dup::CodeFragment;
use usecase::dry_check::CodeFragmentExtractorPort;
use usecase::dry_check::fragment_pipeline::CodeFragmentExtractorError;

use super::extractor::extract_code_fragments;

/// Infrastructure adapter implementing [`CodeFragmentExtractorPort`].
///
/// Delegates to [`extract_code_fragments`] and converts `ExtractError` to
/// `String` at the boundary.
#[derive(Debug, Default)]
pub struct CodeFragmentExtractorAdapter;

impl CodeFragmentExtractorAdapter {
    /// Construct a new adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl CodeFragmentExtractorPort for CodeFragmentExtractorAdapter {
    fn extract(
        &self,
        workspace_root: &std::path::Path,
    ) -> Result<Vec<CodeFragment>, CodeFragmentExtractorError> {
        extract_code_fragments(workspace_root)
            .map_err(|e| CodeFragmentExtractorError::ExtractionFailed(e.to_string()))
    }
}
