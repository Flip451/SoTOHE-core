//! Shared TDDD value objects used by Track command contexts.

use std::path::{Path, PathBuf};

use domain::SpecElementId;

use crate::git_workflow::DiagnosticText;

/// A validated source workspace path used by TDDD capture commands.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackSourceWorkspace(PathBuf);

impl TrackSourceWorkspace {
    /// Validates and wraps a source workspace path.
    pub fn try_new(value: PathBuf) -> Result<Self, DiagnosticText> {
        validate_non_traversing_path(&value, "track source workspace")?;
        Ok(Self(value))
    }

    /// Returns the wrapped path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// A validated catalogue file path used by TDDD commands.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackCataloguePath(PathBuf);

impl TrackCataloguePath {
    /// Validates and wraps a catalogue file path.
    pub fn try_new(value: PathBuf) -> Result<Self, DiagnosticText> {
        validate_non_traversing_path(&value, "track catalogue path")?;
        Ok(Self(value))
    }

    /// Returns the wrapped path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// A selected layer for a TDDD operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackLayerSelection {
    /// Apply to every enabled layer.
    All,
    /// Apply to one layer.
    One(domain::tddd::LayerId),
}

/// A filter selecting zero or more TDDD layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackLayerFilter {
    /// Include every enabled layer.
    All,
    /// Include only the listed layers.
    Selected(Vec<domain::tddd::LayerId>),
}

/// A selected spec anchor for a TDDD operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackSpecAnchorSelection {
    /// Process every anchor.
    All,
    /// Process one validated anchor.
    One(SpecElementId),
}

/// A layer-level signal result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackLayerSignalResult {
    /// The layer was evaluated and produced counts.
    Evaluated { layer: domain::tddd::LayerId, counts: domain::SignalCounts },
    /// The layer was skipped.
    Skipped { layer: domain::tddd::LayerId },
}

/// Signals captured from one catalogue implementation layer.
pub struct TrackCatalogueImplLayerResult {
    /// The layer represented by the signals.
    pub layer: domain::tddd::LayerId,
    /// Signals captured from the implementation catalogue.
    pub signals: Vec<domain::tddd::ThreeWaySignal>,
}

/// Number of rendered TDDD layers.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TrackRenderedLayerCount(usize);

impl TrackRenderedLayerCount {
    /// Wraps a rendered-layer count.
    #[must_use]
    pub fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the numeric count.
    #[must_use]
    pub fn value(&self) -> usize {
        self.0
    }
}

/// Number of files written by a TDDD operation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TrackWrittenFileCount(usize);

impl TrackWrittenFileCount {
    /// Wraps a written-file count.
    #[must_use]
    pub fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the numeric count.
    #[must_use]
    pub fn value(&self) -> usize {
        self.0
    }
}

/// Number of entries found in a catalogue.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TrackCatalogueEntryCount(usize);

impl TrackCatalogueEntryCount {
    /// Wraps a catalogue-entry count.
    #[must_use]
    pub fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the numeric count.
    #[must_use]
    pub fn value(&self) -> usize {
        self.0
    }
}

fn validate_non_traversing_path(value: &Path, label: &str) -> Result<(), DiagnosticText> {
    if value.as_os_str().is_empty() {
        return Err(DiagnosticText::new(format!("{label} must not be empty")));
    }
    if value.components().any(|component| component == std::path::Component::ParentDir) {
        return Err(DiagnosticText::new(format!(
            "{label} must not contain parent traversal: {}",
            value.display()
        )));
    }
    Ok(())
}
