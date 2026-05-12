//! Secondary ports for `CatalogueImplSignalsInteractor`.
//!
//! Declared alongside `CatalogueToExtendedCratePort` and `SignalEvaluatorPort`
//! because `CatalogueDocument` and `rustdoc_types::Crate` are domain types.
//!
//! ## Design (ADR 2026-05-11-2330 §D2)
//!
//! `CatalogueDocumentLoaderPort` wraps filesystem loading of a `CatalogueDocument`.
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

use crate::tddd::catalogue_v2::CatalogueDocument;

// ---------------------------------------------------------------------------
// CatalogueDocumentLoaderError
// ---------------------------------------------------------------------------

/// Error type for [`CatalogueDocumentLoaderPort::load`].
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
// CatalogueDocumentLoaderPort
// ---------------------------------------------------------------------------

/// Secondary port for loading a `CatalogueDocument` from a filesystem path.
///
/// A-side input for `CatalogueImplSignalsInteractor`. Placed in the domain
/// alongside `CatalogueToExtendedCratePort` and `SignalEvaluatorPort` because
/// `CatalogueDocument` is a domain type.
///
/// The infrastructure adapter (`FsCatalogueDocumentLoader`) wraps
/// `CatalogueDocumentCodec::load`.
///
/// [source: ADR 2026-05-11-2330 D2 — hexagonal consequence of moving
/// orchestration to usecase]
pub trait CatalogueDocumentLoaderPort: Send + Sync {
    /// Loads a `CatalogueDocument` from the given filesystem path.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogueDocumentLoaderError::NotFound`] if the file is absent.
    ///
    /// Returns [`CatalogueDocumentLoaderError::Io`] if a non-symlink I/O error
    /// occurs while reading the file.
    ///
    /// Returns [`CatalogueDocumentLoaderError::Decode`] if JSON deserialization
    /// or schema-version validation fails.
    fn load(&self, path: &Path) -> Result<CatalogueDocument, CatalogueDocumentLoaderError>;
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
    /// (C-side live capture).
    ///
    /// # Errors
    ///
    /// Returns [`RustdocCratePortError::CaptureFailed`] if `cargo rustdoc` fails.
    ///
    /// Returns [`RustdocCratePortError::ParseFailed`] if the generated JSON cannot
    /// be deserialized.
    fn capture_current(
        &self,
        crate_name: &str,
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
