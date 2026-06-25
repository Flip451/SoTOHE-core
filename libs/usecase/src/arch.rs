//! Arch use case port.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

/// Error returned by [`ArchPort`] methods.
#[derive(Debug, thiserror::Error)]
pub enum ArchPortError {
    /// The infrastructure layer could not fulfill the request.
    #[error("{0}")]
    Unavailable(String),
}

/// Secondary port for workspace architecture rendering.
pub trait ArchPort: Send + Sync {
    /// Render the workspace tree (crate paths only).
    fn render_tree(&self, project_root: &Path) -> Result<String, ArchPortError>;
    /// Render the workspace tree including extra_dirs.
    fn render_tree_full(&self, project_root: &Path) -> Result<String, ArchPortError>;
    /// List workspace member paths (one per line).
    fn render_members(&self, project_root: &Path) -> Result<String, ArchPortError>;
    /// Print the direct dependency check matrix.
    fn render_direct_checks(&self, project_root: &Path) -> Result<String, ArchPortError>;
}

/// Application-level contract for workspace architecture rendering.
///
/// `PrimaryAdapter` (`ArchDriver`) depends on this interface rather than directly on
/// `ArchPort` (DIP). `ArchInteractor` implements this service by delegating to the
/// injected `ArchPort`.
pub trait ArchService: Send + Sync {
    /// Render the workspace tree (crate paths only).
    fn render_tree(&self, project_root: PathBuf) -> Result<String, ArchPortError>;
    /// Render the workspace tree including extra_dirs.
    fn render_tree_full(&self, project_root: PathBuf) -> Result<String, ArchPortError>;
    /// List workspace member paths (one per line).
    fn render_members(&self, project_root: PathBuf) -> Result<String, ArchPortError>;
    /// Print the direct dependency check matrix.
    fn render_direct_checks(&self, project_root: PathBuf) -> Result<String, ArchPortError>;
}

/// Interactor that implements `ArchService` by delegating to the injected `ArchPort`.
pub struct ArchInteractor {
    port: Arc<dyn ArchPort>,
}

impl ArchInteractor {
    /// Create a new `ArchInteractor` wrapping the given `ArchPort`.
    pub fn new(port: Arc<dyn ArchPort>) -> Self {
        Self { port }
    }
}

impl ArchService for ArchInteractor {
    fn render_tree(&self, project_root: PathBuf) -> Result<String, ArchPortError> {
        self.port.render_tree(project_root.as_path())
    }

    fn render_tree_full(&self, project_root: PathBuf) -> Result<String, ArchPortError> {
        self.port.render_tree_full(project_root.as_path())
    }

    fn render_members(&self, project_root: PathBuf) -> Result<String, ArchPortError> {
        self.port.render_members(project_root.as_path())
    }

    fn render_direct_checks(&self, project_root: PathBuf) -> Result<String, ArchPortError> {
        self.port.render_direct_checks(project_root.as_path())
    }
}
