//! Client-specific ports for loading TDDD feature declarations.

use std::fmt;
use std::path::{Path, PathBuf};

use domain::tddd::catalogue_v2::TdddLayerBinding;
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::tddd::{CargoFeatureName, LayerId, TdddFeatureDeclaration};

/// Shared failures while reading and validating a feature declaration.
#[derive(Debug)]
pub enum TdddFeatureDeclarationReadError {
    /// The required declaration artifact was absent.
    MissingDeclaration { path: PathBuf },
    /// Filesystem access to the declaration or manifest failed.
    ReadDeclaration { path: PathBuf, reason: DiagnosticMessage },
    /// The declaration could not be decoded or validated.
    DecodeDeclaration { path: PathBuf, reason: DiagnosticMessage },
    /// A declared feature is absent from the target crate manifest.
    UnknownCargoFeature { layer: LayerId, feature: CargoFeatureName },
}

impl fmt::Display for TdddFeatureDeclarationReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDeclaration { path } => {
                write!(formatter, "feature declaration not found: {}", path.display())
            }
            Self::ReadDeclaration { path, reason } => {
                write!(formatter, "failed to read '{}': {}", path.display(), reason.as_str())
            }
            Self::DecodeDeclaration { path, reason } => {
                write!(formatter, "failed to decode '{}': {}", path.display(), reason.as_str())
            }
            Self::UnknownCargoFeature { layer, feature } => write!(
                formatter,
                "feature '{}' is not declared by layer '{layer}'",
                feature.as_str()
            ),
        }
    }
}

impl std::error::Error for TdddFeatureDeclarationReadError {}

/// Baseline-capture-specific declaration loading errors.
#[derive(Debug)]
pub enum TdddBaselineFeatureDeclarationPortError {
    /// Shared declaration read or validation failure.
    Read(TdddFeatureDeclarationReadError),
    /// The initial declaration snapshot could not be persisted.
    SnapshotWrite { path: PathBuf, reason: DiagnosticMessage },
    /// Existing baseline bytes differ from the current declaration bytes.
    BaselineSnapshotMismatch,
}

impl fmt::Display for TdddBaselineFeatureDeclarationPortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(formatter),
            Self::SnapshotWrite { path, reason } => {
                write!(formatter, "failed to write '{}': {}", path.display(), reason.as_str())
            }
            Self::BaselineSnapshotMismatch => {
                formatter.write_str("feature declaration differs from its baseline snapshot")
            }
        }
    }
}

impl std::error::Error for TdddBaselineFeatureDeclarationPortError {}

/// Actual-capture-specific declaration loading errors.
#[derive(Debug)]
pub enum TdddActualFeatureDeclarationPortError {
    /// Shared declaration read or validation failure.
    Read(TdddFeatureDeclarationReadError),
    /// The baseline declaration snapshot was absent.
    MissingBaselineSnapshot { path: PathBuf },
    /// Existing baseline bytes differ from the current declaration bytes.
    BaselineSnapshotMismatch,
    /// A catalogue item is gated by a feature omitted from the declaration.
    CatalogueUsesUndeclaredFeature {
        layer: LayerId,
        feature: CargoFeatureName,
        item: DiagnosticMessage,
    },
}

impl fmt::Display for TdddActualFeatureDeclarationPortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(formatter),
            Self::MissingBaselineSnapshot { path } => write!(
                formatter,
                "feature declaration baseline snapshot not found: {}",
                path.display()
            ),
            Self::BaselineSnapshotMismatch => {
                formatter.write_str("feature declaration differs from its baseline snapshot")
            }
            Self::CatalogueUsesUndeclaredFeature { layer, feature, item } => write!(
                formatter,
                "catalogue item '{}' uses undeclared feature '{}' for layer '{layer}'",
                item.as_str(),
                feature.as_str()
            ),
        }
    }
}

impl std::error::Error for TdddActualFeatureDeclarationPortError {}

/// Loads and freezes a declaration for baseline capture.
pub trait TdddBaselineFeatureDeclarationPort: Send + Sync {
    /// Reads the declaration, validates it, and creates or verifies its baseline snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`TdddBaselineFeatureDeclarationPortError`] when the declaration is
    /// unavailable, invalid, has unknown features, cannot be snapshotted, or differs
    /// from its existing snapshot.
    fn load_for_baseline(
        &self,
        track_dir: &Path,
        workspace_root: &Path,
        layers: &[TdddLayerBinding],
    ) -> Result<TdddFeatureDeclaration, TdddBaselineFeatureDeclarationPortError>;
}

/// Loads and verifies a declaration for actual capture.
pub trait TdddActualFeatureDeclarationPort: Send + Sync {
    /// Reads the declaration, validates it, and verifies its frozen baseline bytes.
    ///
    /// # Errors
    ///
    /// Returns [`TdddActualFeatureDeclarationPortError`] when the declaration is
    /// unavailable, invalid, has unknown features, its snapshot is missing, or its
    /// bytes differ from the snapshot.
    fn load_for_actual(
        &self,
        track_dir: &Path,
        workspace_root: &Path,
        layers: &[TdddLayerBinding],
    ) -> Result<TdddFeatureDeclaration, TdddActualFeatureDeclarationPortError>;
}
