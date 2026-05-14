//! `SpecFileLoaderPort` — secondary port for loading raw spec.json content.
//!
//! Abstracts filesystem I/O behind a domain port so that callers can load
//! spec.json without calling `std::fs` directly (hexagonal-purity rule).
//! The infrastructure adapter (`FsSpecFileLoader`) lives in `libs/infrastructure`.

use std::path::Path;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error returned by [`SpecFileLoaderPort::load`].
#[derive(Debug, thiserror::Error)]
#[error("spec file load failed: {0}")]
pub struct SpecFileLoadError(pub String);

// ---------------------------------------------------------------------------
// Port trait
// ---------------------------------------------------------------------------

/// Secondary port for loading the raw text content of a `spec.json` file.
///
/// Implementors live in the infrastructure layer (e.g. `FsSpecFileLoader`).
/// The port is intentionally minimal: it returns the raw `String` so that
/// callers can perform their own JSON parsing and validation without coupling
/// the port to a specific schema version or codec.
pub trait SpecFileLoaderPort: Send + Sync {
    /// Load the raw text content of the spec file at `spec_path`.
    ///
    /// # Errors
    ///
    /// Returns [`SpecFileLoadError`] when the file cannot be read (not found,
    /// permission denied, symlink rejected, etc.).
    fn load(&self, spec_path: &Path) -> Result<String, SpecFileLoadError>;
}
