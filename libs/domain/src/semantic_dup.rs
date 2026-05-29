//! Domain types for semantic duplicate detection.
//!
//! These value objects represent the core abstractions for the discoverability
//! soft-gate feature (ADR 2026-05-29-1118-semantic-dup-detection-discoverability-gate).

use std::fmt;
use std::path::PathBuf;

// ── SemanticDupError ──────────────────────────────────────────────────────────

/// Domain-level errors for semantic duplicate detection value object invariant violations.
#[derive(Debug)]
pub enum SemanticDupError {
    /// A [`SimilarityScore`] was constructed with a value outside `[0.0, 1.0]`.
    InvalidScore {
        /// The rejected score value.
        value: f32,
    },
    /// A [`TopK`] was constructed with `k == 0`.
    InvalidTopK {
        /// The rejected top-k value.
        value: usize,
    },
    /// A [`SimilarityThreshold`] was constructed with a value outside `[0.0, 1.0]`.
    InvalidThreshold {
        /// The rejected threshold value.
        value: f32,
    },
    /// A [`CodeFragment`] was constructed with an empty content string.
    EmptyContent,
}

impl fmt::Display for SemanticDupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScore { value } => {
                write!(
                    f,
                    "similarity score {value} is out of range; expected a value in [0.0, 1.0]"
                )
            }
            Self::InvalidTopK { value } => {
                write!(f, "top-k value {value} is invalid; must be >= 1")
            }
            Self::InvalidThreshold { value } => {
                write!(
                    f,
                    "similarity threshold {value} is out of range; expected a value in [0.0, 1.0]"
                )
            }
            Self::EmptyContent => {
                write!(f, "code fragment content must be non-empty")
            }
        }
    }
}

impl std::error::Error for SemanticDupError {}

// ── SimilarityScore ───────────────────────────────────────────────────────────

/// Cosine similarity score in the range `[0.0, 1.0]`.
///
/// Values outside this range are rejected at construction. Raw cosine similarity
/// (range `[-1.0, 1.0]`) is normalized to this range by the `SemanticIndexPort`
/// adapter before constructing a `SimilarityScore` (e.g., via clamping negative
/// values to `0.0`, since the Jina v2 base code model produces near-zero negative
/// similarities in practice).
#[derive(Debug, Clone, Copy)]
pub struct SimilarityScore(f32);

impl SimilarityScore {
    /// Construct a [`SimilarityScore`] from a raw `f32`.
    ///
    /// # Errors
    ///
    /// Returns [`SemanticDupError::InvalidScore`] when `value` is not in
    /// the closed interval `[0.0, 1.0]`.
    pub fn new(value: f32) -> Result<Self, SemanticDupError> {
        if (0.0_f32..=1.0_f32).contains(&value) {
            Ok(Self(value))
        } else {
            Err(SemanticDupError::InvalidScore { value })
        }
    }

    /// Return the underlying `f32` score.
    pub fn value(&self) -> f32 {
        self.0
    }
}

// ── TopK ─────────────────────────────────────────────────────────────────────

/// Top-k count for similarity search results. Must be `>= 1`.
#[derive(Debug, Clone, Copy)]
pub struct TopK(usize);

impl TopK {
    /// Construct a [`TopK`] from a `usize`.
    ///
    /// # Errors
    ///
    /// Returns [`SemanticDupError::InvalidTopK`] when `k == 0`.
    pub fn new(k: usize) -> Result<Self, SemanticDupError> {
        if k >= 1 { Ok(Self(k)) } else { Err(SemanticDupError::InvalidTopK { value: k }) }
    }

    /// Return the underlying `usize` count.
    pub fn value(&self) -> usize {
        self.0
    }
}

// ── SimilarityThreshold ───────────────────────────────────────────────────────

/// Cosine similarity threshold for the soft-gate duplicate check.
///
/// Must be in `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy)]
pub struct SimilarityThreshold(f32);

impl SimilarityThreshold {
    /// Construct a [`SimilarityThreshold`] from a raw `f32`.
    ///
    /// # Errors
    ///
    /// Returns [`SemanticDupError::InvalidThreshold`] when `value` is not in
    /// the closed interval `[0.0, 1.0]`.
    pub fn new(value: f32) -> Result<Self, SemanticDupError> {
        if (0.0_f32..=1.0_f32).contains(&value) {
            Ok(Self(value))
        } else {
            Err(SemanticDupError::InvalidThreshold { value })
        }
    }

    /// Return the underlying `f32` threshold.
    pub fn value(&self) -> f32 {
        self.0
    }
}

// ── CodeFragment ──────────────────────────────────────────────────────────────

/// A code fragment with its text content and an associated path.
///
/// For fragments extracted from source files, `source_path` is the originating
/// file path. For query fragments provided via CLI (e.g., `sotp find-similar`),
/// `source_path` is set to a sentinel value (`PathBuf::from("<query>")`) since
/// there is no originating file.
///
/// Content must be non-empty.
#[derive(Debug, Clone)]
pub struct CodeFragment {
    /// The path of the source file this fragment was extracted from, or
    /// `PathBuf::from("<query>")` for ad-hoc query fragments.
    pub source_path: PathBuf,
    /// The text content of the fragment. Always non-empty.
    content: String,
}

impl CodeFragment {
    /// Construct a [`CodeFragment`].
    ///
    /// # Errors
    ///
    /// Returns [`SemanticDupError::EmptyContent`] when `content` is empty.
    pub fn new(source_path: PathBuf, content: String) -> Result<Self, SemanticDupError> {
        if content.is_empty() {
            return Err(SemanticDupError::EmptyContent);
        }
        Ok(Self { source_path, content })
    }

    /// Return the text content of this fragment.
    ///
    /// The returned string is always non-empty (enforced at construction).
    pub fn content(&self) -> &str {
        &self.content
    }
}

// ── SimilarFragment ───────────────────────────────────────────────────────────

/// A code fragment that was retrieved as semantically similar to a query,
/// paired with its cosine similarity score.
#[derive(Debug, Clone)]
pub struct SimilarFragment {
    /// The retrieved code fragment.
    pub fragment: CodeFragment,
    /// The cosine similarity score between this fragment and the query.
    pub score: SimilarityScore,
}
