//! Error types for the semantic duplicate detection use case.

use std::fmt;

use thiserror::Error;

// ── Error types ───────────────────────────────────────────────────────────────

/// Error type for the [`super::EmbeddingPort`].
///
/// `source` is an opaque string from fastembed-rs — no domain concept.
#[derive(Debug)]
pub enum EmbeddingError {
    /// The embedding model failed to load or initialise.
    ModelLoadFailed {
        /// Opaque error string from the underlying fastembed-rs error.
        source: String,
    },
    /// Inference over a fragment failed.
    InferenceFailed {
        /// Opaque error string from the underlying fastembed-rs error.
        source: String,
    },
}

impl fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelLoadFailed { source } => {
                write!(f, "embedding model load failed: {source}")
            }
            Self::InferenceFailed { source } => {
                write!(f, "embedding inference failed: {source}")
            }
        }
    }
}

impl std::error::Error for EmbeddingError {}

/// Error type for the [`super::SemanticIndexPort`].
///
/// `source` is an opaque string from LanceDB — no domain concept.
#[derive(Debug)]
pub enum SemanticIndexError {
    /// Opening (or creating) the vector index failed.
    OpenFailed {
        /// Opaque error string from the underlying LanceDB error.
        source: String,
    },
    /// Inserting a fragment+embedding into the index failed.
    InsertFailed {
        /// Opaque error string from the underlying LanceDB error.
        source: String,
    },
    /// Deleting fragments by source path from the index failed.
    DeleteFailed {
        /// Opaque error string from the underlying LanceDB error.
        source: String,
    },
    /// Searching the index failed.
    SearchFailed {
        /// Opaque error string from the underlying LanceDB error.
        source: String,
    },
}

impl fmt::Display for SemanticIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenFailed { source } => {
                write!(f, "semantic index open failed: {source}")
            }
            Self::InsertFailed { source } => {
                write!(f, "semantic index insert failed: {source}")
            }
            Self::DeleteFailed { source } => {
                write!(f, "semantic index delete failed: {source}")
            }
            Self::SearchFailed { source } => {
                write!(f, "semantic index search failed: {source}")
            }
        }
    }
}

impl std::error::Error for SemanticIndexError {}

/// Composite error for the find-similar use case.
#[derive(Debug, Error)]
pub enum FindSimilarError {
    /// An embedding operation failed.
    #[error(transparent)]
    Embedding(#[from] EmbeddingError),
    /// An index operation failed.
    #[error(transparent)]
    Index(#[from] SemanticIndexError),
}

/// Composite error for the dup-check use case.
#[derive(Debug, Error)]
pub enum DupCheckError {
    /// An embedding operation failed.
    #[error(transparent)]
    Embedding(#[from] EmbeddingError),
    /// An index operation failed.
    #[error(transparent)]
    Index(#[from] SemanticIndexError),
}

/// Composite error for the build-index use case.
#[derive(Debug)]
pub enum BuildIndexError {
    /// An embedding operation failed.
    Embedding(EmbeddingError),
    /// An index operation failed.
    Index(SemanticIndexError),
    /// A filesystem I/O operation failed.
    Io {
        /// The path that was being accessed when the error occurred.
        path: std::path::PathBuf,
        /// Opaque error string from the underlying I/O error.
        source: String,
    },
}

impl fmt::Display for BuildIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Embedding(e) => fmt::Display::fmt(e, f),
            Self::Index(e) => fmt::Display::fmt(e, f),
            Self::Io { path, source } => {
                write!(f, "I/O error at {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for BuildIndexError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Embedding(e) => Some(e),
            Self::Index(e) => Some(e),
            Self::Io { .. } => None,
        }
    }
}

impl From<EmbeddingError> for BuildIndexError {
    fn from(e: EmbeddingError) -> Self {
        Self::Embedding(e)
    }
}

impl From<SemanticIndexError> for BuildIndexError {
    fn from(e: SemanticIndexError) -> Self {
        Self::Index(e)
    }
}

/// Composite error for the measure-quality use case.
#[derive(Debug)]
pub enum MeasureQualityError {
    /// An embedding operation failed.
    Embedding(EmbeddingError),
    /// An index operation failed.
    Index(SemanticIndexError),
    /// A filesystem I/O operation failed.
    Io {
        /// The path that was being accessed when the error occurred.
        path: std::path::PathBuf,
        /// Opaque error string from the underlying I/O error.
        source: String,
    },
}

impl fmt::Display for MeasureQualityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Embedding(e) => fmt::Display::fmt(e, f),
            Self::Index(e) => fmt::Display::fmt(e, f),
            Self::Io { path, source } => {
                write!(f, "I/O error at {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for MeasureQualityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Embedding(e) => Some(e),
            Self::Index(e) => Some(e),
            Self::Io { .. } => None,
        }
    }
}

impl From<EmbeddingError> for MeasureQualityError {
    fn from(e: EmbeddingError) -> Self {
        Self::Embedding(e)
    }
}

impl From<SemanticIndexError> for MeasureQualityError {
    fn from(e: SemanticIndexError) -> Self {
        Self::Index(e)
    }
}
