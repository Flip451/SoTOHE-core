//! `SpecDocumentLoaderPort` — domain loader port for the persisted `spec.json`.
//!
//! Placed in the domain per `type-designer-kind-selection.md` R1's persistence-/aggregate-port
//! bucket, symmetric with `CatalogueDocumentLoaderPort` and the `TrackReader` /
//! `WorktreeReader` family — not the application-service port bucket. Its error
//! type [`SpecDocumentLoadError`] lives here too.

use std::path::{Path, PathBuf};

use crate::SpecDocument;
use crate::tddd::test_obligation::ids::DiagnosticMessage;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error returned by [`SpecDocumentLoaderPort::load`].
///
/// Covers file I/O, JSON parse, and domain-invariant failures so the usecase
/// interactor never has to `serde_json`-parse directly (hexagonal purity).
#[derive(Debug, thiserror::Error)]
pub enum SpecDocumentLoadError {
    /// The spec document was absent at the requested path.
    #[error("spec document not found: {}", .path.display())]
    NotFound {
        /// Path that was requested.
        path: PathBuf,
    },
    /// Non-symlink I/O failure while reading the spec document.
    #[error("I/O error reading spec document '{}': {}", .path.display(), .reason.as_str())]
    Io {
        /// Path that was being read.
        path: PathBuf,
        /// Human-readable reason from the underlying I/O error.
        reason: DiagnosticMessage,
    },
    /// JSON parse failure before domain validation.
    #[error("failed to parse spec document '{}': {}", .path.display(), .reason.as_str())]
    JsonParse {
        /// Path that failed to parse.
        path: PathBuf,
        /// Human-readable parse failure reason.
        reason: DiagnosticMessage,
    },
    /// Parsed JSON failed [`SpecDocument`] invariants.
    #[error("invalid spec document '{}': {}", .path.display(), .reason.as_str())]
    Invariant {
        /// Path that decoded to an invalid document.
        path: PathBuf,
        /// Human-readable invariant failure reason.
        reason: DiagnosticMessage,
    },
}

// ---------------------------------------------------------------------------
// Port trait
// ---------------------------------------------------------------------------

/// Loader port for the persisted domain document `spec.json`.
///
/// Placed in the domain per `type-designer-kind-selection.md` R1's
/// persistence-/aggregate-port bucket (symmetric with `CatalogueDocumentLoaderPort` at
/// `tddd::catalogue_v2::catalogue_impl_signals_ports` and with the `TrackReader` /
/// `WorktreeReader` family), not the application-service port bucket. Unlike the
/// minimal [`SpecFileLoaderPort`](crate::SpecFileLoaderPort) which returns raw
/// text, this port returns a fully deserialized [`SpecDocument`] so callers can
/// enumerate spec elements without coupling to a codec (IN-07 / IN-08).
pub trait SpecDocumentLoaderPort {
    /// Loads and parses the spec document at `spec_path`.
    ///
    /// # Errors
    ///
    /// Returns [`SpecDocumentLoadError`] when the file cannot be read or the
    /// content cannot be parsed into a [`SpecDocument`].
    fn load(&self, spec_path: &Path) -> Result<SpecDocument, SpecDocumentLoadError>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_load_error_display_distinguishes_not_found() {
        let err = SpecDocumentLoadError::NotFound { path: PathBuf::from("spec.json") };
        assert_eq!(err.to_string(), "spec document not found: spec.json");
    }

    #[test]
    fn test_load_error_display_distinguishes_json_parse() {
        let err = SpecDocumentLoadError::JsonParse {
            path: PathBuf::from("spec.json"),
            reason: DiagnosticMessage::try_new("expected value".to_owned()).unwrap(),
        };
        assert_eq!(err.to_string(), "failed to parse spec document 'spec.json': expected value");
    }

    #[test]
    fn test_load_error_is_debuggable_and_is_error() {
        let err = SpecDocumentLoadError::Invariant {
            path: PathBuf::from("spec.json"),
            reason: DiagnosticMessage::try_new("duplicate element id AC-01".to_owned()).unwrap(),
        };
        assert!(format!("{err:?}").contains("duplicate element id AC-01"));
        // Exercise the `core::error::Error` impl (via thiserror).
        let as_error: &dyn std::error::Error = &err;
        assert!(as_error.source().is_none());
    }
}
