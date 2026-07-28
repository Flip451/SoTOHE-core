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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use usecase::dry_check::CodeFragmentExtractorPort as _;

    use super::CodeFragmentExtractorAdapter;

    #[test]
    fn test_code_fragment_extractor_adapter_extract_delegates_to_workspace_scanner() {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("sample.rs"), "pub fn sample() {}\n").unwrap();

        let fragments = CodeFragmentExtractorAdapter::new().extract(workspace.path()).unwrap();

        assert_eq!(fragments.len(), 1, "the adapter must return the scanner's fragment");
        assert_eq!(
            fragments.first().map(|fragment| fragment.content()),
            Some("pub fn sample() {}")
        );
    }

    #[test]
    fn test_code_fragment_extractor_adapter_extract_missing_workspace_returns_port_error() {
        let missing_workspace = tempfile::tempdir().unwrap().path().join("missing-workspace");

        let result = CodeFragmentExtractorAdapter::new().extract(&missing_workspace);

        assert!(
            result.is_err(),
            "the adapter must convert scanner failures into the usecase port error"
        );
    }
}
