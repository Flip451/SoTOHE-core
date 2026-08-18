//! Shared value objects for the Track TDDD lint command contexts.

use std::path::{Path, PathBuf};

use crate::git_workflow::DiagnosticText;

use super::validate_non_traversing_path;

/// A validated path to a catalogue-lint rules file.
#[derive(PartialEq, Eq)]
pub struct TrackLintRulesFile(PathBuf);

impl TrackLintRulesFile {
    /// Validates and wraps a lint-rules-file path.
    pub fn try_new(value: PathBuf) -> Result<Self, DiagnosticText> {
        validate_non_traversing_path(&value, "track lint rules file")?;
        Ok(Self(value))
    }

    /// Returns the wrapped path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}
