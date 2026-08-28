//! Domain values and secondary ports for `CatalogueImplSignalsInteractor`.
//!
//! The catalogue attestation and loader error remain domain values because
//! they cross the application boundary. The filesystem loader port is owned
//! by `libs/usecase`, where the loading orchestration is declared.
//!
//! ## Design (ADR 2026-05-11-2330 §D2)
//!
//! `RustdocCratePort` wraps both baseline load (B-side) and live capture (C-side)
//! of a `rustdoc_types::Crate`.  Injecting these via ports instead of calling
//! infrastructure codecs directly keeps `libs/usecase` free of `infrastructure`
//! dependencies (hexagonal architecture).
//!
//! ## No serde derives
//!
//! Per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`, the domain
//! layer is serialization-free.  Codec errors are mapped to the domain error
//! variants below so `libs/usecase` never imports `infrastructure` error types.

use std::fmt;
use std::path::{Path, PathBuf};

use sha2::Digest as _;

use crate::tddd::CargoFeatureName;
use crate::tddd::catalogue_v2::{CatalogueDocument, CrateName};
use crate::tddd::type_signals_doc::{CatalogueDeclarationHash, Sha256Digest};
use crate::{ContentHash, TrackId};

// ---------------------------------------------------------------------------
// CatalogueDocumentLoaderError
// ---------------------------------------------------------------------------

/// Error type returned when a catalogue document cannot be loaded.
///
/// Three variants: `NotFound` (file absent), `Io` (non-symlink I/O failure),
/// `Decode` (JSON or schema-version failure from `CatalogueDocumentCodec`).
///
/// [source: ADR 2026-05-11-2330 D2]
#[derive(Debug)]
pub enum CatalogueDocumentLoaderError {
    /// The catalogue file was not found at the given path.
    NotFound {
        /// Absolute or workspace-relative path that was absent.
        path: PathBuf,
    },
    /// Non-symlink I/O failure while reading the catalogue file.
    Io {
        /// Path that was being read.
        path: PathBuf,
        /// Human-readable reason from the underlying I/O error.
        reason: String,
    },
    /// JSON or schema-version decode failure.
    Decode {
        /// Path of the file that failed to decode.
        path: PathBuf,
        /// Human-readable reason from the codec error.
        reason: String,
    },
}

impl fmt::Display for CatalogueDocumentLoaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { path } => {
                write!(f, "catalogue file not found: {}", path.display())
            }
            Self::Io { path, reason } => {
                write!(f, "I/O error reading '{}': {reason}", path.display())
            }
            Self::Decode { path, reason } => {
                write!(f, "failed to decode '{}': {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for CatalogueDocumentLoaderError {}

// ---------------------------------------------------------------------------
// AttestedCatalogueDocument
// ---------------------------------------------------------------------------

/// A catalogue document paired with the declaration hash of the exact bytes
/// from which it was decoded.
///
/// Keeping the attestation beside, rather than inside, [`CatalogueDocument`]
/// preserves the persisted catalogue model while making freshness validation a
/// mandatory part of every successful loader result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestedCatalogueDocument {
    document: CatalogueDocument,
    declaration_hash: CatalogueDeclarationHash,
}

impl AttestedCatalogueDocument {
    /// Decodes and attests one source byte sequence.
    ///
    /// The decoder receives the same bytes that are hashed, so callers cannot
    /// construct a successful result with a separately supplied declaration
    /// hash.
    ///
    /// # Errors
    ///
    /// Returns the error produced by `decode(source)` when decoding fails.
    pub fn attest<E, F>(source: &[u8], decode: F) -> Result<Self, E>
    where
        F: FnOnce(&[u8]) -> Result<CatalogueDocument, E>,
    {
        let document = decode(source)?;
        let bytes: [u8; 32] = sha2::Sha256::digest(source).into();
        let declaration_hash = CatalogueDeclarationHash::new(Sha256Digest::from_content_hash(
            ContentHash::from_bytes(bytes),
        ));
        Ok(Self { document, declaration_hash })
    }

    /// Returns the decoded catalogue document.
    #[must_use]
    pub fn document(&self) -> &CatalogueDocument {
        &self.document
    }

    /// Returns the declaration hash of the exact source bytes read by the loader.
    #[must_use]
    pub fn declaration_hash(&self) -> &CatalogueDeclarationHash {
        &self.declaration_hash
    }

    /// Consumes the attested result and returns its decoded document.
    #[must_use]
    pub fn into_document(self) -> CatalogueDocument {
        self.document
    }
}

// ---------------------------------------------------------------------------
// RustdocCratePortError
// ---------------------------------------------------------------------------

/// Error type for [`RustdocCratePort`] methods.
///
/// `NotFound` / `Io` cover baseline file load failures;
/// `ParseFailed` covers JSON parse failures (from `BaselineRustdocCodec::from_json`);
/// `CaptureFailed` covers `cargo rustdoc` invocation failures (from
/// `RustdocSchemaExporter::export_rustdoc_json_path`).
///
/// [source: ADR 2026-05-11-2330 D2]
#[derive(Debug)]
pub enum RustdocCratePortError {
    /// The baseline file was not found at the given path.
    NotFound {
        /// Path that was absent.
        path: PathBuf,
    },
    /// Non-symlink I/O failure while reading the baseline file.
    Io {
        /// Path that was being read.
        path: PathBuf,
        /// Human-readable reason from the underlying I/O error.
        reason: String,
    },
    /// JSON parse failure for the rustdoc output.
    ParseFailed {
        /// Crate name for which parsing failed.
        crate_name: String,
        /// Human-readable reason from the codec error.
        reason: String,
    },
    /// `cargo rustdoc` invocation failed during live capture.
    CaptureFailed {
        /// Crate name for which capture was attempted.
        crate_name: String,
        /// Human-readable reason from the exporter error.
        reason: String,
    },
}

impl fmt::Display for RustdocCratePortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { path } => {
                write!(f, "rustdoc JSON not found: {}", path.display())
            }
            Self::Io { path, reason } => {
                write!(f, "I/O error reading '{}': {reason}", path.display())
            }
            Self::ParseFailed { crate_name, reason } => {
                write!(f, "failed to parse rustdoc JSON for '{crate_name}': {reason}")
            }
            Self::CaptureFailed { crate_name, reason } => {
                write!(f, "rustdoc capture failed for '{crate_name}': {reason}")
            }
        }
    }
}

impl std::error::Error for RustdocCratePortError {}

// ---------------------------------------------------------------------------
// RustdocCratePort
// ---------------------------------------------------------------------------

/// Secondary port for loading or capturing `rustdoc_types::Crate` instances.
///
/// - `load_from_path`: loads a previously captured rustdoc JSON file (B-side baseline).
/// - `capture_current`: captures the current crate's rustdoc JSON via the nightly
///   toolchain (C-side).
///
/// Placed in the domain alongside `SignalEvaluatorPort` because `rustdoc_types::Crate`
/// is already part of the domain's vocabulary (the domain depends on `rustdoc_types`).
///
/// [source: ADR 2026-05-11-2330 D2]
pub trait RustdocCratePort: Send + Sync {
    /// Loads a `rustdoc_types::Crate` from the given JSON file path (B-side).
    ///
    /// # Errors
    ///
    /// Returns [`RustdocCratePortError::NotFound`] if the file is absent.
    ///
    /// Returns [`RustdocCratePortError::Io`] if a non-symlink I/O error occurs.
    ///
    /// Returns [`RustdocCratePortError::ParseFailed`] if JSON deserialization or
    /// format-version validation fails.
    fn load_from_path(&self, path: &Path) -> Result<rustdoc_types::Crate, RustdocCratePortError>;

    /// Captures the current `rustdoc_types::Crate` via `cargo +nightly rustdoc`
    /// (C-side live capture) with the validated layer feature selection.
    ///
    /// # Errors
    ///
    /// Returns [`RustdocCratePortError::CaptureFailed`] if `cargo rustdoc` fails.
    ///
    /// Returns [`RustdocCratePortError::ParseFailed`] if the generated JSON cannot
    /// be deserialized.
    fn capture_current(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
    ) -> Result<rustdoc_types::Crate, RustdocCratePortError>;
}

// ---------------------------------------------------------------------------
// TdddLayerBindingsPort
// ---------------------------------------------------------------------------

/// Binding for a single TDDD-enabled layer resolved from `architecture-rules.json`.
///
/// Used by `CatalogueImplSignalsInteractor` to locate the catalogue file, baseline
/// file, and target crate names without performing file I/O directly inside the
/// usecase layer.
#[derive(Debug, Clone)]
pub struct TdddLayerBinding {
    /// Layer id (e.g., `"domain"`, `"usecase"`, `"infrastructure"`).
    pub layer_id: String,
    /// Filename of the catalogue document (e.g., `"domain-types.json"`).
    pub catalogue_file: String,
    /// Filename of the baseline rustdoc JSON (e.g., `"domain-types-baseline.json"`).
    pub baseline_file: String,
    /// `schema_export.targets` — crate names to pass to `cargo +nightly rustdoc`.
    pub targets: Vec<String>,
}

impl TdddLayerBinding {
    /// Derives the type-signals output filename from `catalogue_file`.
    ///
    /// Strips the trailing `s` from the catalogue stem before appending
    /// `-signals.json`, matching the convention in `infrastructure::verify::tddd_layers`:
    /// `"domain-types.json"` → `"domain-type-signals.json"`.
    ///
    /// # Examples
    ///
    /// ```
    /// use domain::tddd::catalogue_v2::TdddLayerBinding;
    /// let b = TdddLayerBinding {
    ///     layer_id: "domain".to_owned(),
    ///     catalogue_file: "domain-types.json".to_owned(),
    ///     baseline_file: "domain-types-baseline.json".to_owned(),
    ///     targets: vec!["domain".to_owned()],
    /// };
    /// assert_eq!(b.signal_file(), "domain-type-signals.json");
    /// ```
    #[must_use]
    pub fn signal_file(&self) -> String {
        let stem = self.catalogue_file.strip_suffix(".json").unwrap_or(&self.catalogue_file);
        let signal_stem = if let Some(trimmed) = stem.strip_suffix('s') {
            format!("{trimmed}-signals")
        } else {
            format!("{stem}-signals")
        };
        format!("{signal_stem}.json")
    }
}

/// Error type for [`TdddLayerBindingsPort::load`].
#[derive(Debug)]
pub enum TdddLayerBindingsError {
    /// The `architecture-rules.json` file could not be read or parsed.
    LoadFailed {
        /// Human-readable reason.
        reason: String,
    },
    /// A specific layer was requested but not found (or not `tddd.enabled`).
    LayerNotFound {
        /// The requested layer id.
        layer_id: String,
    },
    /// No `tddd.enabled` layers found in the rules file.
    NoLayers,
}

impl fmt::Display for TdddLayerBindingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoadFailed { reason } => {
                write!(f, "failed to load architecture-rules.json: {reason}")
            }
            Self::LayerNotFound { layer_id } => {
                write!(
                    f,
                    "layer '{layer_id}' not found or not tddd.enabled in architecture-rules.json"
                )
            }
            Self::NoLayers => {
                write!(f, "no TDDD-enabled layers found in architecture-rules.json")
            }
        }
    }
}

impl std::error::Error for TdddLayerBindingsError {}

/// Secondary port for loading TDDD layer bindings from `architecture-rules.json`.
///
/// Injected into `CatalogueImplSignalsInteractor` so the usecase layer never
/// calls `std::fs` directly (hexagonal architecture / usecase-purity rule).
///
/// The infrastructure adapter (`FsTdddLayerBindingsAdapter`) reads
/// `architecture-rules.json` and returns the matching `TdddLayerBinding` entries.
pub trait TdddLayerBindingsPort: Send + Sync {
    /// Loads the TDDD-enabled layer bindings from `workspace_root/architecture-rules.json`.
    ///
    /// If `layer_filter` is `Some`, returns only the binding for that layer id
    /// (fails if not found or not `tddd.enabled`). If `None`, returns all enabled
    /// layers.
    ///
    /// # Errors
    ///
    /// Returns [`TdddLayerBindingsError`] on load failure, missing layer, or no layers.
    fn load(
        &self,
        workspace_root: &Path,
        layer_filter: Option<&str>,
    ) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError>;
}

// ---------------------------------------------------------------------------
// RustdocBaselineCapturePort
// ---------------------------------------------------------------------------

/// Error type returned by [`RustdocBaselineCapturePort::capture`].
///
/// Wraps any failure from the rustdoc capture pipeline (symlink guard,
/// missing track directory, rustdoc export failure, file write failure).
/// The message is human-readable and suitable for direct display via `to_string()`.
#[derive(Debug, Clone)]
pub struct BaselineCaptureIoError(pub String);

impl fmt::Display for BaselineCaptureIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for BaselineCaptureIoError {}

/// Secondary port for capturing the rustdoc-format baseline snapshot.
///
/// Implementations run `cargo +nightly rustdoc` against `rustdoc_workspace`,
/// read the output JSON, validate `format_version`, and write the result to
/// `<items_dir>/<track_id>/<layer>-types-baseline.json`. The operation is
/// always idempotent: if the file already exists and passes format validation,
/// `capture` returns `Ok(())` immediately without overwriting it. To
/// re-capture, delete the baseline file first and then call `capture` again.
///
/// Injected into `BaselineCaptureInteractor` so the usecase layer never
/// calls infrastructure code directly.
pub trait RustdocBaselineCapturePort: Send + Sync {
    /// Captures the rustdoc-format baseline for a single layer binding.
    ///
    /// # Arguments
    ///
    /// * `items_dir` — trusted root directory (`workspace_root/track/items`).
    /// * `track_id` — validated track ID slug.
    /// * `rustdoc_workspace` — Cargo workspace from which rustdoc is invoked.
    ///   May differ from the workspace that contains `items_dir` (git-worktree
    ///   capture flow).
    /// * `binding` — the TDDD layer binding resolved from `architecture-rules.json`.
    /// * `features` — validated Cargo features declared for `binding.layer_id`.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineCaptureIoError`] on security guard rejection, missing
    /// track directory, rustdoc export failure, format validation failure, or
    /// file write failure.
    fn capture(
        &self,
        items_dir: &std::path::Path,
        track_id: &TrackId,
        rustdoc_workspace: &std::path::Path,
        binding: &TdddLayerBinding,
        features: &[CargoFeatureName],
    ) -> Result<(), BaselineCaptureIoError>;
}

// ---------------------------------------------------------------------------
// TrackStatusReaderPort
// ---------------------------------------------------------------------------

/// Error type returned by [`TrackStatusReaderPort::read_status`].
#[derive(Debug, Clone)]
pub struct TrackStatusReadError(pub String);

impl fmt::Display for TrackStatusReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for TrackStatusReadError {}

/// Secondary port for reading the derived [`crate::TrackStatus`] of a track.
///
/// Returns the `TrackStatus` derived from `metadata.json` + `impl-plan.json`
/// without exposing filesystem or codec details to the usecase layer.
///
/// The infrastructure adapter (`FsTrackStatusReaderAdapter`) reads the two
/// files, applies the symlink guard, validates the track id, and calls
/// `domain::derive_track_status`. Injected into `TypeSignalsInteractor`.
pub trait TrackStatusReaderPort: Send + Sync {
    /// Returns the derived [`crate::TrackStatus`] for the given track.
    ///
    /// # Errors
    ///
    /// Returns [`TrackStatusReadError`] when the track id is invalid, any file
    /// is unreadable, a symlink guard fires, or the metadata cannot be decoded.
    fn read_status(
        &self,
        items_dir: &Path,
        track_id: &str,
    ) -> Result<crate::TrackStatus, TrackStatusReadError>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::tddd::LayerId;

    #[test]
    fn attested_catalogue_document_preserves_document_and_hash() {
        let document = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        let source = b"catalogue source bytes";
        let attested = AttestedCatalogueDocument::attest(source, |decoded| {
            assert_eq!(decoded, source);
            Ok::<_, std::convert::Infallible>(document.clone())
        })
        .unwrap();

        assert_eq!(attested.document(), &document);
        assert_eq!(attested.into_document(), document);
    }
}
