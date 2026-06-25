//! Conventions use case port.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

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

/// Application-level contract for convention document management.
///
/// `PrimaryAdapter` (`ConventionsDriver`) depends on this interface rather than directly on
/// `ConventionsPort` (DIP). `ConventionsInteractor` implements this service by delegating to the
/// injected `ConventionsPort`.
pub trait ConventionsService: Send + Sync {
    /// Create a new convention document and update the README index.
    fn add_convention(
        &self,
        root: PathBuf,
        name: String,
        slug: Option<String>,
        title: Option<String>,
        summary: Option<String>,
    ) -> Result<String, ConventionsPortError>;

    /// Regenerate the README.md index from current convention documents.
    fn update_index(&self, root: PathBuf) -> Result<String, ConventionsPortError>;

    /// Verify that the README.md indexes all convention documents.
    fn verify_index(&self, root: PathBuf) -> Result<VerifyIndexResult, ConventionsPortError>;
}

/// Interactor that implements `ConventionsService` by delegating to the injected `ConventionsPort`.
pub struct ConventionsInteractor {
    port: Arc<dyn ConventionsPort>,
}

impl ConventionsInteractor {
    /// Create a new `ConventionsInteractor` wrapping the given `ConventionsPort`.
    pub fn new(port: Arc<dyn ConventionsPort>) -> Self {
        Self { port }
    }
}

impl ConventionsService for ConventionsInteractor {
    fn add_convention(
        &self,
        root: PathBuf,
        name: String,
        slug: Option<String>,
        title: Option<String>,
        summary: Option<String>,
    ) -> Result<String, ConventionsPortError> {
        self.port.add_convention(
            root.as_path(),
            &name,
            slug.as_deref(),
            title.as_deref(),
            summary.as_deref(),
        )
    }

    fn update_index(&self, root: PathBuf) -> Result<String, ConventionsPortError> {
        self.port.update_index(root.as_path())
    }

    fn verify_index(&self, root: PathBuf) -> Result<VerifyIndexResult, ConventionsPortError> {
        self.port.verify_index(root.as_path())
    }
}
