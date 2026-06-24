//! Conventions use case port.

use std::path::Path;

/// Error returned by [`ConventionsPort`] methods.
#[derive(Debug, thiserror::Error)]
pub enum ConventionsPortError {
    /// The infrastructure layer could not fulfill the request.
    #[error("{0}")]
    Unavailable(String),
}

/// Result of verifying the convention README index.
pub struct VerifyIndexResult {
    /// Whether the index is in sync (no findings).
    pub ok: bool,
    /// Human-readable finding messages (empty when `ok` is true).
    pub findings: Vec<String>,
}

/// Secondary port for convention document management.
pub trait ConventionsPort: Send + Sync {
    /// Create a new convention document and update the README index.
    fn add_convention(
        &self,
        root: &Path,
        name: &str,
        slug: Option<&str>,
        title: Option<&str>,
        summary: Option<&str>,
    ) -> Result<String, ConventionsPortError>;

    /// Regenerate the README.md index from current convention documents.
    fn update_index(&self, root: &Path) -> Result<String, ConventionsPortError>;

    /// Verify that the README.md indexes all convention documents.
    fn verify_index(&self, root: &Path) -> Result<VerifyIndexResult, ConventionsPortError>;
}
