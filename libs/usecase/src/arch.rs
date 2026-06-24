//! Arch use case port.

use std::path::Path;

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
