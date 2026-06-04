//! Error types for the dry-check use case layer.

use thiserror::Error;

use domain::dry_check::{DryCheckEntryError, DryCheckReaderError, DryCheckWriterError};

use crate::semantic_dup::{EmbeddingError, SemanticIndexError};

// ── DryCheckAgentError ────────────────────────────────────────────────────────

/// Errors from the [`super::DryCheckAgentPort`] usecase port.
///
/// Mirrors `ReviewerError`. Covers agent abort, timeout, illegal schema output,
/// and unexpected failures.
#[derive(Debug, Error)]
pub enum DryCheckAgentError {
    /// The user requested abort during the agent run.
    #[error("dry-check agent aborted by user")]
    UserAbort,
    /// The agent subprocess exited with a non-zero status.
    #[error("dry-check agent aborted (non-zero exit)")]
    AgentAbort,
    /// The agent subprocess timed out.
    #[error("dry-check agent timed out")]
    Timeout,
    /// The agent produced output that does not conform to the required schema.
    #[error("dry-check agent produced illegal output")]
    IllegalOutput,
    /// An unexpected error occurred.
    #[error("dry-check agent unexpected error: {0}")]
    Unexpected(String),
}

// ── DryCheckDiffError ─────────────────────────────────────────────────────────

/// Error type for [`super::DryCheckDiffSource`] — dry-check's own diff-source
/// port error.
///
/// CN-01: mirrors `DiffGetError` semantics but is dry-check's independent type,
/// not `review_v2`'s `DiffGetError`. Kept structurally simple: a single `Failed`
/// variant carrying the underlying git / I/O message.
#[derive(Debug, Error)]
pub enum DryCheckDiffError {
    /// The diff operation failed.
    #[error("dry-check diff failed: {0}")]
    Failed(String),
}

// ── DryCheckCycleError ────────────────────────────────────────────────────────

/// Composite error for the dry-check use case cycle.
///
/// Covers embedding, index, agent, persistence, diff, and entry construction
/// errors. No `PairKey` variant: self-match (`DryCheckPairKey::new` returning
/// `Err(SelfMatch)` when both path AND content_hash are equal) is a
/// control-flow skip signal — the interactor skips that candidate pair and
/// proceeds to the next; it is not a cycle-fatal error and is never wrapped
/// here.
///
/// `Entry` wraps [`DryCheckEntryError`] (changed_path-outside-pair rejection
/// from `DryCheckEntry::new`), which is an internal invariant violation
/// (interactor bug) and is abort-worthy.
///
/// Uses dry-check's own [`DryCheckDiffError`] (not `review_v2`'s `DiffGetError`)
/// per CN-01 loose coupling.
#[derive(Debug, Error)]
pub enum DryCheckCycleError {
    /// An embedding operation failed.
    #[error("dry-check cycle embedding error: {0}")]
    Embedding(EmbeddingError),
    /// An index operation failed.
    #[error("dry-check cycle index error: {0}")]
    Index(SemanticIndexError),
    /// The dry-check agent returned an error.
    #[error("dry-check cycle agent error: {0}")]
    Agent(DryCheckAgentError),
    /// The dry-check reader returned an error.
    #[error("dry-check cycle reader error: {0}")]
    Reader(DryCheckReaderError),
    /// The dry-check writer returned an error.
    #[error("dry-check cycle writer error: {0}")]
    Writer(DryCheckWriterError),
    /// The diff-source port returned an error.
    #[error("dry-check cycle diff error: {0}")]
    Diff(DryCheckDiffError),
    /// `DryCheckEntry::new` returned `ChangedPathOutsidePair` — an internal
    /// invariant violation.
    #[error("dry-check cycle entry error: {0}")]
    Entry(DryCheckEntryError),
}
