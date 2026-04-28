//! Domain secondary port for ADR file enumeration and front-matter parsing.

use std::path::PathBuf;

use thiserror::Error;

use super::front_matter::AdrFrontMatter;

/// Error type for [`AdrFilePort`] failures.
///
/// Adapter implementations (infrastructure layer) absorb both I/O errors and
/// YAML parse errors into these two variants so the usecase layer never sees
/// raw file system or serde types — preserves the CN-05 hexagonal boundary.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdrFilePortError {
    /// Listing ADR file paths failed (e.g. directory missing, permission denied,
    /// glob pattern error).
    #[error("failed to list ADR paths: {0}")]
    ListPaths(String),
    /// Reading or parsing a single ADR file failed (e.g. I/O error, malformed
    /// YAML front-matter, schema violation in the front-matter).
    #[error("failed to read ADR file: {0}")]
    ReadFile(String),
}

/// Domain secondary port abstracting the filesystem-backed operations the
/// usecase layer needs to verify ADR signals.
///
/// Adapter implementations (e.g. `FsAdrFileAdapter` in T004) live in the
/// infrastructure layer. The port surface deliberately accepts owned
/// [`PathBuf`] arguments to remain object-safe with `Arc<dyn AdrFilePort>`
/// without introducing lifetime gymnastics.
pub trait AdrFilePort: Send + Sync {
    /// Return every ADR markdown file path under the configured directory.
    ///
    /// # Errors
    ///
    /// Returns [`AdrFilePortError::ListPaths`] when the directory cannot be
    /// listed (missing, permission denied, etc.).
    fn list_adr_paths(&self) -> Result<Vec<PathBuf>, AdrFilePortError>;

    /// Read and parse the YAML front-matter of a single ADR file at `path`,
    /// returning a domain [`AdrFrontMatter`] value.
    ///
    /// Adapter implementations encapsulate the YAML parse step internally
    /// (delegating to `parse_adr_frontmatter` in T003) so the usecase layer
    /// receives a domain-shaped value directly.
    ///
    /// # Errors
    ///
    /// Returns [`AdrFilePortError::ReadFile`] for I/O errors, missing files,
    /// malformed YAML, or schema-violating front-matter.
    fn read_adr_frontmatter(&self, path: PathBuf) -> Result<AdrFrontMatter, AdrFilePortError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adr_file_port_error_list_paths_displays_message() {
        let e = AdrFilePortError::ListPaths("dir not found: /tmp/missing".to_string());
        assert_eq!(format!("{e}"), "failed to list ADR paths: dir not found: /tmp/missing");
    }

    #[test]
    fn test_adr_file_port_error_read_file_displays_message() {
        let e = AdrFilePortError::ReadFile("yaml parse failed at line 4".to_string());
        assert_eq!(format!("{e}"), "failed to read ADR file: yaml parse failed at line 4");
    }
}
