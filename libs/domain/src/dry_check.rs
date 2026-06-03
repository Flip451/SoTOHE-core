//! Domain types and ports for the DRY violation auto-detection capability.
//!
//! This module implements the core abstractions for detecting duplicate code
//! (DRY violations) using semantic similarity search and agent-based judgment.
//! See ADR 2026-06-02-0716-dry-checker for the design decisions.

use std::fmt;
use std::path::Path;

use thiserror::Error;

use crate::ids::CommitHash;
use crate::review_v2::types::FilePath;
use crate::semantic_dup::{CodeFragment, SimilarityScore, SimilarityThreshold};
use crate::timestamp::Timestamp;

// ── FragmentContentHash ───────────────────────────────────────────────────────

/// Validated SHA-256 content hash of a code fragment.
///
/// Format: 64 lowercase hex chars. Part of [`FragmentRef`] — the pair
/// (`FilePath`, `FragmentContentHash`) is the fragment identifier (D8/D9/CN-07).
/// When content changes, `content_hash` changes, so the `FragmentRef` changes,
/// so the `DryCheckPairKey` changes — invalidation is implicit in the identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FragmentContentHash(String);

impl FragmentContentHash {
    /// Construct a [`FragmentContentHash`] from a string.
    ///
    /// # Errors
    ///
    /// Returns [`FragmentContentHashError::InvalidFormat`] when `s` is not a
    /// 64-character lowercase hexadecimal string.
    pub fn new(s: impl Into<String>) -> Result<Self, FragmentContentHashError> {
        let s = s.into();
        if s.len() != 64 || !s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
            return Err(FragmentContentHashError::InvalidFormat(s));
        }
        Ok(Self(s))
    }

    /// Return the underlying hash string (always 64 lowercase hex chars).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FragmentContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Error from [`FragmentContentHash::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FragmentContentHashError {
    /// Input is not a valid 64-char lowercase hex SHA-256 string.
    #[error("fragment content hash must be 64 lowercase hex chars: {0}")]
    InvalidFormat(String),
}

// ── RefactorProposal ──────────────────────────────────────────────────────────

/// Validated non-empty refactor proposal text produced by the dry-checker agent.
///
/// The empty-proposal state is structurally unrepresentable: `DryCheckVerdict::Violation`
/// and `DryCheckFinding::refactor_proposal` both use this type, so an empty
/// proposal cannot exist in a valid violation record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefactorProposal(String);

impl RefactorProposal {
    /// Construct a [`RefactorProposal`].
    ///
    /// # Errors
    ///
    /// Returns [`RefactorProposalError::Empty`] when `s` is empty.
    pub fn new(s: impl Into<String>) -> Result<Self, RefactorProposalError> {
        let s = s.into();
        if s.is_empty() {
            return Err(RefactorProposalError::Empty);
        }
        Ok(Self(s))
    }

    /// Return the underlying proposal text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RefactorProposal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Error from [`RefactorProposal::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RefactorProposalError {
    /// Input string is empty.
    #[error("refactor proposal must not be empty")]
    Empty,
}

// ── Rationale ─────────────────────────────────────────────────────────────────

/// Validated non-empty rationale text (agent judgment reason).
///
/// Required on all records: the D9 schema mandates a non-null judgment reason
/// for every verdict (violation, not-a-violation, accepted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rationale(String);

impl Rationale {
    /// Construct a [`Rationale`].
    ///
    /// # Errors
    ///
    /// Returns [`RationaleError::Empty`] when `s` is empty.
    pub fn new(s: impl Into<String>) -> Result<Self, RationaleError> {
        let s = s.into();
        if s.is_empty() {
            return Err(RationaleError::Empty);
        }
        Ok(Self(s))
    }

    /// Return the underlying rationale text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Rationale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Error from [`Rationale::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RationaleError {
    /// Input string is empty.
    #[error("rationale must not be empty")]
    Empty,
}

// ── FragmentRef ───────────────────────────────────────────────────────────────

/// Fragment identifier: the pair (repo-relative path, content_hash) uniquely
/// identifies a code fragment by both location and content (D8/IN-06/CN-07).
///
/// Two `FragmentRef`s are equal iff both `path` AND `content_hash` match — this
/// is the basis for self-match detection in `DryCheckPairKey::new()`.
/// `Ord` is lexicographic `(path, content_hash)` so `DryCheckPairKey::new()`
/// can sort two `FragmentRef`s into `(low, high)` deterministically.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FragmentRef {
    path: FilePath,
    content_hash: FragmentContentHash,
}

impl FragmentRef {
    /// Construct a [`FragmentRef`] (infallible — both components are already
    /// validated value objects).
    pub fn new(path: FilePath, content_hash: FragmentContentHash) -> FragmentRef {
        FragmentRef { path, content_hash }
    }

    /// Return the repo-relative file path.
    pub fn path(&self) -> &FilePath {
        &self.path
    }

    /// Return the SHA-256 content hash.
    pub fn content_hash(&self) -> &FragmentContentHash {
        &self.content_hash
    }
}

// ── DryCheckPairKey ───────────────────────────────────────────────────────────

/// Normalized (sorted) pair of [`FragmentRef`]s used as the dry-check dedup/cache key.
///
/// `low <= high` by `(path, content_hash)` lexicographic order, ensuring `(X,Y)`
/// and `(Y,X)` produce the same key (CN-08). Self-match (both refs equal) is
/// rejected at construction. Paths-different-hash-same (complete copies in
/// different files) is NOT a self-match and produces a valid pair.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DryCheckPairKey {
    low: FragmentRef,
    high: FragmentRef,
}

impl DryCheckPairKey {
    /// Construct a [`DryCheckPairKey`] from two [`FragmentRef`]s.
    ///
    /// Sorts `a` and `b` into `(low, high)` so `(X,Y)` and `(Y,X)` produce the
    /// same key. Rejects self-match when `a == b` on BOTH path AND content_hash.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckPairKeyError::SelfMatch`] when both refs are equal.
    pub fn new(a: FragmentRef, b: FragmentRef) -> Result<DryCheckPairKey, DryCheckPairKeyError> {
        if a == b {
            return Err(DryCheckPairKeyError::SelfMatch);
        }
        let (low, high) = if a <= b { (a, b) } else { (b, a) };
        Ok(DryCheckPairKey { low, high })
    }

    /// Return the lower [`FragmentRef`] in `(path, content_hash)` order.
    pub fn low(&self) -> &FragmentRef {
        &self.low
    }

    /// Return the higher [`FragmentRef`] in `(path, content_hash)` order.
    pub fn high(&self) -> &FragmentRef {
        &self.high
    }
}

/// Error from [`DryCheckPairKey::new`].
#[derive(Debug, Error)]
pub enum DryCheckPairKeyError {
    /// Both [`FragmentRef`] arguments are equal (path AND content_hash both match).
    #[error("self-match: both fragment refs are identical (same path and content_hash)")]
    SelfMatch,
}

// ── DryCheckVerdict ───────────────────────────────────────────────────────────

/// Per-pair DRY check verdict (enum-first design, D9).
///
/// - `NotAViolation`: false positive rejected by agent.
/// - `Accepted`: agent-determined acceptable duplication.
/// - `Violation`: genuine DRY violation with a mandatory non-empty refactor proposal.
///
/// "Violation without a proposal" and "non-violation with a proposal" are
/// structurally unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DryCheckVerdict {
    /// False positive: the agent determined this pair is not a DRY violation.
    NotAViolation,
    /// Agent-approved duplication: acceptable similarity (cross-layer mirror,
    /// tests, boilerplate, etc.).
    Accepted,
    /// Genuine DRY violation carrying the mandatory non-empty refactor proposal.
    Violation {
        /// Non-empty refactor proposal from the agent.
        refactor_proposal: RefactorProposal,
    },
}

impl fmt::Display for DryCheckVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAViolation => f.write_str("not-a-violation"),
            Self::Accepted => f.write_str("accepted"),
            Self::Violation { refactor_proposal } => {
                write!(f, "violation({})", refactor_proposal.as_str())
            }
        }
    }
}

// ── VerdictFilter ─────────────────────────────────────────────────────────────

/// Classification filter for the dry-check read path.
///
/// Passed to `DryCheckResultsService::get_results()` to limit results to a
/// specific verdict class or return all records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerdictFilter {
    /// Return all records.
    All,
    /// Return only `NotAViolation` records.
    NotAViolation,
    /// Return only `Accepted` records.
    Accepted,
    /// Return only `Violation` records.
    Violation,
}

// ── DryCheckEntry ─────────────────────────────────────────────────────────────

/// Write-input type for the dry-check persistence path (write-read separation).
///
/// Carries the 7 fields the interactor knows at verdict time. Does NOT carry
/// `recorded_at` — the infra adapter (`FsDryCheckStore`) stamps `Timestamp`
/// internally.
///
/// `Eq` is not derived because `SimilarityScore` and `SimilarityThreshold` wrap
/// `f32`, which does not implement `Eq`.
#[derive(Debug, Clone, PartialEq)]
pub struct DryCheckEntry {
    pair_key: DryCheckPairKey,
    changed_path: FilePath,
    verdict: DryCheckVerdict,
    similarity_score: SimilarityScore,
    threshold: SimilarityThreshold,
    base_commit: CommitHash,
    rationale: Rationale,
}

impl DryCheckEntry {
    /// Construct a [`DryCheckEntry`].
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckEntryError::ChangedPathOutsidePair`] when `changed_path`
    /// is neither `pair_key.low().path()` nor `pair_key.high().path()`.
    pub fn new(
        pair_key: DryCheckPairKey,
        changed_path: FilePath,
        verdict: DryCheckVerdict,
        similarity_score: SimilarityScore,
        threshold: SimilarityThreshold,
        base_commit: CommitHash,
        rationale: Rationale,
    ) -> Result<DryCheckEntry, DryCheckEntryError> {
        if &changed_path != pair_key.low().path() && &changed_path != pair_key.high().path() {
            return Err(DryCheckEntryError::ChangedPathOutsidePair);
        }
        Ok(DryCheckEntry {
            pair_key,
            changed_path,
            verdict,
            similarity_score,
            threshold,
            base_commit,
            rationale,
        })
    }

    /// Return the pair key.
    pub fn pair_key(&self) -> &DryCheckPairKey {
        &self.pair_key
    }

    /// Return the display-only changed path.
    pub fn changed_path(&self) -> &FilePath {
        &self.changed_path
    }

    /// Return the verdict.
    pub fn verdict(&self) -> &DryCheckVerdict {
        &self.verdict
    }

    /// Return the similarity score.
    pub fn similarity_score(&self) -> &SimilarityScore {
        &self.similarity_score
    }

    /// Return the similarity threshold.
    pub fn threshold(&self) -> &SimilarityThreshold {
        &self.threshold
    }

    /// Return the base commit hash.
    pub fn base_commit(&self) -> &CommitHash {
        &self.base_commit
    }

    /// Return the agent's rationale (always non-empty).
    pub fn rationale(&self) -> &Rationale {
        &self.rationale
    }
}

/// Error from [`DryCheckEntry::new`].
#[derive(Debug, Error)]
pub enum DryCheckEntryError {
    /// `changed_path` is neither `pair_key.low().path()` nor `pair_key.high().path()`.
    #[error("changed_path is not part of the pair (must equal low().path() or high().path())")]
    ChangedPathOutsidePair,
}

// ── DryCheckRecord ────────────────────────────────────────────────────────────

/// A single entry in the dry-check history (read/persistent form).
///
/// Constructed exclusively by `FsDryCheckStore::append_record` which stamps
/// `Timestamp`. The interactor constructs [`DryCheckEntry`] (7 fields, no
/// `recorded_at`) and passes it to `DryCheckWriter::append_record`; the infra
/// adapter builds this record.
///
/// Illegal states are unrepresentable by construction (D9):
/// - `pair_key.low > pair_key.high` is impossible (sorted by `DryCheckPairKey::new`).
/// - Self-match is impossible (`DryCheckPairKey::new` rejects equal refs).
/// - `changed_path` outside the pair is impossible (`DryCheckEntry::new` validates it).
/// - `recorded_at`, `rationale`, and `refactor_proposal` (in `Violation`) are always valid.
///
/// `Eq` is not derived because `SimilarityScore` and `SimilarityThreshold` wrap
/// `f32`, which does not implement `Eq`.
#[derive(Debug, Clone, PartialEq)]
pub struct DryCheckRecord {
    pair_key: DryCheckPairKey,
    changed_path: FilePath,
    verdict: DryCheckVerdict,
    similarity_score: SimilarityScore,
    threshold: SimilarityThreshold,
    base_commit: CommitHash,
    rationale: Rationale,
    recorded_at: Timestamp,
}

impl DryCheckRecord {
    /// Infra-internal constructor: build a `DryCheckRecord` from a `DryCheckEntry`
    /// and a stamped `Timestamp`.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckRecordError::ChangedPathOutsidePair`] when the entry's
    /// `changed_path` is not part of the pair (should be unreachable in correct
    /// usage since `DryCheckEntry::new` already validates this).
    pub fn from_entry_and_timestamp(
        entry: DryCheckEntry,
        recorded_at: Timestamp,
    ) -> Result<DryCheckRecord, DryCheckRecordError> {
        // Validate again for defence-in-depth (should be unreachable).
        if entry.changed_path() != entry.pair_key().low().path()
            && entry.changed_path() != entry.pair_key().high().path()
        {
            return Err(DryCheckRecordError::ChangedPathOutsidePair);
        }
        Ok(DryCheckRecord {
            pair_key: entry.pair_key,
            changed_path: entry.changed_path,
            verdict: entry.verdict,
            similarity_score: entry.similarity_score,
            threshold: entry.threshold,
            base_commit: entry.base_commit,
            rationale: entry.rationale,
            recorded_at,
        })
    }

    /// Return the pair key.
    pub fn pair_key(&self) -> &DryCheckPairKey {
        &self.pair_key
    }

    /// Return the display-only changed path.
    pub fn changed_path(&self) -> &FilePath {
        &self.changed_path
    }

    /// Return the verdict.
    pub fn verdict(&self) -> &DryCheckVerdict {
        &self.verdict
    }

    /// Return the similarity score.
    pub fn similarity_score(&self) -> &SimilarityScore {
        &self.similarity_score
    }

    /// Return the similarity threshold.
    pub fn threshold(&self) -> &SimilarityThreshold {
        &self.threshold
    }

    /// Return the base commit hash.
    pub fn base_commit(&self) -> &CommitHash {
        &self.base_commit
    }

    /// Return the agent's rationale (always non-empty).
    pub fn rationale(&self) -> &Rationale {
        &self.rationale
    }

    /// Return the record timestamp (stamped by the infra adapter at write time).
    pub fn recorded_at(&self) -> &Timestamp {
        &self.recorded_at
    }
}

/// Error from `DryCheckRecord::from_entry_and_timestamp`.
///
/// Retained for infra-internal constructor completeness. In practice, this
/// variant is unreachable in correct usage because `DryCheckEntry::new` already
/// validates `changed_path`.
#[derive(Debug, Error)]
pub enum DryCheckRecordError {
    /// `changed_path` is not part of the pair.
    #[error("changed_path is not part of the pair (must equal low().path() or high().path())")]
    ChangedPathOutsidePair,
}

// ── DryCheckFinding ───────────────────────────────────────────────────────────

/// A genuine DRY violation finding produced by the dry-checker agent.
///
/// The live write-path finding returned to dfl. A finding only exists when the
/// agent verdict is `Violation` — `NotAViolation` and `Accepted` carry no finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryCheckFinding {
    changed_fragment_ref: FragmentRef,
    candidate_fragment_ref: FragmentRef,
    refactor_proposal: RefactorProposal,
}

impl DryCheckFinding {
    /// Construct a [`DryCheckFinding`].
    ///
    /// Calls `RefactorProposal::new` internally.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckFindingError::EmptyProposal`] when `refactor_proposal`
    /// is empty.
    pub fn new(
        changed_fragment_ref: FragmentRef,
        candidate_fragment_ref: FragmentRef,
        refactor_proposal: impl Into<String>,
    ) -> Result<DryCheckFinding, DryCheckFindingError> {
        let refactor_proposal = RefactorProposal::new(refactor_proposal)
            .map_err(|_| DryCheckFindingError::EmptyProposal)?;
        Ok(DryCheckFinding { changed_fragment_ref, candidate_fragment_ref, refactor_proposal })
    }

    /// Return the changed (diff-side) fragment ref.
    pub fn changed_fragment_ref(&self) -> &FragmentRef {
        &self.changed_fragment_ref
    }

    /// Return the candidate (retrieval-side) fragment ref.
    pub fn candidate_fragment_ref(&self) -> &FragmentRef {
        &self.candidate_fragment_ref
    }

    /// Return the non-empty refactor proposal.
    pub fn refactor_proposal(&self) -> &RefactorProposal {
        &self.refactor_proposal
    }
}

/// Error from [`DryCheckFinding::new`].
#[derive(Debug, Error)]
pub enum DryCheckFindingError {
    /// The refactor proposal string is empty.
    #[error("refactor proposal must not be empty")]
    EmptyProposal,
}

// ── DryCheckApprovalVerdict ───────────────────────────────────────────────────

/// Domain verdict for the dry-check gate (D7/D10).
///
/// - `Approved`: all above-threshold pairs verified as not-a-violation or accepted.
/// - `Blocked`: unresolved violations or unverified pairs remain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DryCheckApprovalVerdict {
    /// All above-threshold pairs are verified (not-a-violation or accepted).
    Approved,
    /// Unresolved violations or unverified pairs remain.
    Blocked {
        /// Number of unresolved pairs.
        unresolved_pair_count: usize,
    },
}

// ── DiffHunkRange ─────────────────────────────────────────────────────────────

/// A 1-indexed inclusive line range `[start_line, end_line]` for a single
/// added/changed hunk from `git diff`.
///
/// The invariant `start <= end` (both >= 1) makes the empty/inverted-range
/// and zero-line states unrepresentable at construction time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunkRange {
    start_line: u32,
    end_line: u32,
}

impl DiffHunkRange {
    /// Construct a [`DiffHunkRange`].
    ///
    /// # Errors
    ///
    /// Returns [`DiffHunkRangeError::ZeroLine`] when `start_line` or `end_line` is 0.
    /// Returns [`DiffHunkRangeError::StartExceedsEnd`] when `start_line > end_line`.
    pub fn new(start_line: u32, end_line: u32) -> Result<DiffHunkRange, DiffHunkRangeError> {
        if start_line == 0 || end_line == 0 {
            return Err(DiffHunkRangeError::ZeroLine);
        }
        if start_line > end_line {
            return Err(DiffHunkRangeError::StartExceedsEnd { start: start_line, end: end_line });
        }
        Ok(DiffHunkRange { start_line, end_line })
    }

    /// Return the 1-indexed start line (inclusive).
    pub fn start_line(&self) -> u32 {
        self.start_line
    }

    /// Return the 1-indexed end line (inclusive).
    pub fn end_line(&self) -> u32 {
        self.end_line
    }
}

/// Error from [`DiffHunkRange::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DiffHunkRangeError {
    /// `start_line > end_line` (empty or inverted range).
    #[error("start_line ({start}) exceeds end_line ({end}) — range must be non-empty")]
    StartExceedsEnd {
        /// The rejected start line.
        start: u32,
        /// The rejected end line.
        end: u32,
    },
    /// A line number is 0 (line numbers are 1-indexed).
    #[error("line numbers must be >= 1 (got 0)")]
    ZeroLine,
}

// ── DiffFileHunks ─────────────────────────────────────────────────────────────

/// A changed file path with its non-empty list of added/changed hunk line ranges.
///
/// Returned by `DryCheckDiffSource::list_changed_hunks()`. The "hunks non-empty"
/// invariant is enforced in the constructor: a file with no added/changed hunks
/// is structurally absent from the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffFileHunks {
    path: FilePath,
    hunks: Vec<DiffHunkRange>,
}

impl DiffFileHunks {
    /// Construct a [`DiffFileHunks`].
    ///
    /// # Errors
    ///
    /// Returns [`DiffFileHunksError::EmptyHunks`] when `hunks` is empty.
    pub fn new(
        path: FilePath,
        hunks: Vec<DiffHunkRange>,
    ) -> Result<DiffFileHunks, DiffFileHunksError> {
        if hunks.is_empty() {
            return Err(DiffFileHunksError::EmptyHunks);
        }
        Ok(DiffFileHunks { path, hunks })
    }

    /// Return the repo-relative file path.
    pub fn path(&self) -> &FilePath {
        &self.path
    }

    /// Return the non-empty list of hunk ranges (always >= 1 element).
    pub fn hunks(&self) -> &[DiffHunkRange] {
        &self.hunks
    }
}

/// Error from [`DiffFileHunks::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DiffFileHunksError {
    /// The hunk list is empty. A file with no added/changed hunks should be
    /// omitted from the result entirely.
    #[error("hunks list must not be empty")]
    EmptyHunks,
}

// ── Port traits ───────────────────────────────────────────────────────────────

/// Errors from [`DryCheckReader`] port operations.
#[derive(Debug, Error)]
pub enum DryCheckReaderError {
    /// File system I/O failure.
    #[error("dry-check reader I/O error: {path}: {detail}")]
    Io {
        /// The file path involved.
        path: String,
        /// Human-readable description of the failure.
        detail: String,
    },
    /// The target path is a symlink (rejected for security).
    #[error("dry-check reader: symlink detected at {path}")]
    SymlinkDetected {
        /// The symlink path.
        path: String,
    },
    /// JSON codec failure.
    #[error("dry-check reader codec error: {path}: {detail}")]
    Codec {
        /// The file path involved.
        path: String,
        /// Human-readable description of the failure.
        detail: String,
    },
    /// A record could not be deserialized to valid domain types.
    #[error("dry-check reader invalid data: {0}")]
    InvalidData(String),
    /// The on-disk schema version is newer than what this implementation supports.
    #[error("dry-check reader incompatible schema version: {version}")]
    IncompatibleSchema {
        /// The unsupported schema version found on disk.
        version: u64,
    },
}

/// Errors from [`DryCheckWriter`] port operations.
#[derive(Debug, Error)]
pub enum DryCheckWriterError {
    /// File system I/O failure.
    #[error("dry-check writer I/O error: {path}: {detail}")]
    Io {
        /// The file path involved.
        path: String,
        /// Human-readable description of the failure.
        detail: String,
    },
    /// The target path is a symlink (rejected for security).
    #[error("dry-check writer: symlink detected at {path}")]
    SymlinkDetected {
        /// The symlink path.
        path: String,
    },
    /// JSON codec failure.
    #[error("dry-check writer codec error: {detail}")]
    Codec {
        /// Human-readable description of the failure.
        detail: String,
    },
    /// The on-disk schema version is newer than what this implementation supports.
    #[error("dry-check writer incompatible schema version: {version}")]
    IncompatibleSchema {
        /// The unsupported schema version found on disk.
        version: u64,
    },
}

/// Read-only port for dry-check history retrieval.
///
/// Returns the full history array of [`DryCheckRecord`] entries. The caller is
/// responsible for latest-per-pair derivation (last occurrence per pair key
/// wins). Persistence port — defined in domain layer (mirrors `ReviewReader`).
pub trait DryCheckReader: Send + Sync {
    /// Read all recorded dry-check history entries.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckReaderError`] on I/O, codec, invalid data, or schema
    /// incompatibility failures.
    fn read_records(&self) -> Result<Vec<DryCheckRecord>, DryCheckReaderError>;
}

/// Write port for dry-check history persistence.
///
/// Receives a [`DryCheckEntry`] (7 fields, no `recorded_at`). The adapter
/// (`FsDryCheckStore`) stamps a `Timestamp` internally to produce a
/// [`DryCheckRecord`] before writing. init-on-first-write: if the file is
/// absent the implementation creates a fresh envelope before appending.
pub trait DryCheckWriter: Send + Sync {
    /// Append a verdict record for the given entry.
    ///
    /// The infra adapter calls `infrastructure::timestamp_now()?` to obtain a
    /// `Timestamp` directly — no `Timestamp::new` re-wrap is needed because
    /// `timestamp_now()` already returns `Result<Timestamp, ValidationError>`.
    /// The interactor never produces a `Timestamp`.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckWriterError`] on I/O, codec, or schema incompatibility
    /// failures.
    fn append_record(&self, entry: &DryCheckEntry) -> Result<(), DryCheckWriterError>;
}

// ── fragments_overlapping_hunks ───────────────────────────────────────────────

/// Filter a slice of [`CodeFragment`]s to those whose source span overlaps any
/// added/changed hunk in `changed_hunks`.
///
/// A fragment overlaps a hunk when:
/// - (a) `fragment.source_path` matches the `DiffFileHunks.path` exactly (byte-equal
///   path comparison), AND
/// - (b) the fragment's `[start_line..=end_line]` range shares at least one
///   line with a `DiffHunkRange [start_line..=end_line]`.
///
/// Fragments from files not appearing in `changed_hunks` are excluded.
/// Fragments from changed files that don't overlap any hunk are also excluded.
///
/// # Contract
///
/// Both `CodeFragment.source_path` values in `fragments` and the `DiffFileHunks.path`
/// values in `changed_hunks` **must be in repo-relative form** (the same format as
/// `git diff` hunk paths, e.g. `src/a.rs`). Absolute paths will not match
/// repo-relative hunk paths. Normalizing absolute paths to repo-relative form is the
/// responsibility of the caller (cli-composition layer, T007/T009), which bridges the
/// fragment extractor output and the diff source output before invoking this function.
///
/// This is the core mechanism making CN-04 (unchanged fragments structurally
/// absent from the diff query) deterministic without LLM involvement (D4).
/// Pure function — no I/O, no side effects.
pub fn fragments_overlapping_hunks(
    fragments: &[CodeFragment],
    changed_hunks: &[DiffFileHunks],
) -> Vec<CodeFragment> {
    fragments
        .iter()
        .filter(|fragment| {
            changed_hunks.iter().any(|file_hunks| {
                if !fragment_path_matches_hunk_path(
                    fragment.source_path.as_path(),
                    file_hunks.path(),
                ) {
                    return false;
                }
                // Check overlap with any hunk.
                file_hunks.hunks().iter().any(|hunk| {
                    // Ranges overlap when: frag.start <= hunk.end AND frag.end >= hunk.start
                    fragment.start_line() <= hunk.end_line()
                        && fragment.end_line() >= hunk.start_line()
                })
            })
        })
        .cloned()
        .collect()
}

/// Returns `true` when `fragment_path` exactly equals the repo-relative `hunk_path`.
///
/// Both paths must already be in repo-relative form (e.g. `src/a.rs`). Suffix
/// matching is intentionally absent: `Path::ends_with` does component-level suffix
/// matching, which causes `tests/src/a.rs` to spuriously match a hunk path of
/// `src/a.rs`, introducing unrelated fragments into the hunk-scope query and
/// corrupting the CN-04 scope guarantee.
///
/// Normalization of absolute paths to repo-relative form is the responsibility of
/// the cli-composition layer (T007/T009) before fragments are passed to
/// `fragments_overlapping_hunks`.
fn fragment_path_matches_hunk_path(fragment_path: &Path, hunk_path: &FilePath) -> bool {
    let repo_relative_hunk_path = Path::new(hunk_path.as_str());
    fragment_path == repo_relative_hunk_path
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::ids::CommitHash;
    use crate::review_v2::types::FilePath;
    use crate::semantic_dup::{CodeFragment, SimilarityScore, SimilarityThreshold};
    use crate::timestamp::Timestamp;

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_hash(hex: &str) -> FragmentContentHash {
        FragmentContentHash::new(hex).unwrap()
    }

    fn make_file_path(s: &str) -> FilePath {
        FilePath::new(s).unwrap()
    }

    fn make_fragment_ref(path: &str, hash: &str) -> FragmentRef {
        FragmentRef::new(make_file_path(path), make_hash(hash))
    }

    fn make_score() -> SimilarityScore {
        SimilarityScore::new(0.9).unwrap()
    }

    fn make_threshold() -> SimilarityThreshold {
        SimilarityThreshold::new(0.8).unwrap()
    }

    fn make_commit() -> CommitHash {
        CommitHash::try_new("abcdef1234567").unwrap()
    }

    fn make_timestamp() -> Timestamp {
        Timestamp::new("2026-06-02T07:16:00Z").unwrap()
    }

    // ── RefactorProposal ──────────────────────────────────────────────────────

    #[test]
    fn test_refactor_proposal_new_with_non_empty_string_succeeds() {
        let result = RefactorProposal::new("Extract shared logic into a helper function.");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "Extract shared logic into a helper function.");
    }

    #[test]
    fn test_refactor_proposal_new_with_empty_string_returns_empty_error() {
        let result = RefactorProposal::new("");
        assert!(matches!(result, Err(RefactorProposalError::Empty)));
    }

    // ── Rationale ─────────────────────────────────────────────────────────────

    #[test]
    fn test_rationale_new_with_non_empty_string_succeeds() {
        let result = Rationale::new("This is a genuine DRY violation.");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "This is a genuine DRY violation.");
    }

    #[test]
    fn test_rationale_new_with_empty_string_returns_empty_error() {
        let result = Rationale::new("");
        assert!(matches!(result, Err(RationaleError::Empty)));
    }

    // ── FragmentContentHash ───────────────────────────────────────────────────

    #[test]
    fn test_fragment_content_hash_new_with_valid_64_hex_succeeds() {
        let hex = "a".repeat(64);
        let result = FragmentContentHash::new(&hex);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), &hex);
    }

    #[test]
    fn test_fragment_content_hash_new_with_63_chars_returns_invalid_format_error() {
        let hex = "a".repeat(63);
        let result = FragmentContentHash::new(&hex);
        assert!(matches!(result, Err(FragmentContentHashError::InvalidFormat(_))));
    }

    #[test]
    fn test_fragment_content_hash_new_with_65_chars_returns_invalid_format_error() {
        let hex = "a".repeat(65);
        let result = FragmentContentHash::new(&hex);
        assert!(matches!(result, Err(FragmentContentHashError::InvalidFormat(_))));
    }

    #[test]
    fn test_fragment_content_hash_new_with_uppercase_hex_returns_invalid_format_error() {
        let hex = "A".repeat(64);
        let result = FragmentContentHash::new(&hex);
        assert!(matches!(result, Err(FragmentContentHashError::InvalidFormat(_))));
    }

    #[test]
    fn test_fragment_content_hash_new_with_non_hex_chars_returns_invalid_format_error() {
        let hex = "g".repeat(64);
        let result = FragmentContentHash::new(&hex);
        assert!(matches!(result, Err(FragmentContentHashError::InvalidFormat(_))));
    }

    // ── FragmentRef Ord ───────────────────────────────────────────────────────

    #[test]
    fn test_fragment_ref_ord_sorts_by_path_then_content_hash() {
        let a = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let b = make_fragment_ref("src/b.rs", &"a".repeat(64));
        assert!(a < b, "path 'src/a.rs' should sort before 'src/b.rs'");

        let c = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let d = make_fragment_ref("src/a.rs", &"b".repeat(64));
        assert!(c < d, "same path: hash 'aaa...' should sort before 'bbb...'");
    }

    // ── DryCheckPairKey ───────────────────────────────────────────────────────

    #[test]
    fn test_dry_check_pair_key_new_normalizes_order_xy_equals_yx() {
        let x = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let y = make_fragment_ref("src/b.rs", &"b".repeat(64));

        let key_xy = DryCheckPairKey::new(x.clone(), y.clone()).unwrap();
        let key_yx = DryCheckPairKey::new(y.clone(), x.clone()).unwrap();

        assert_eq!(key_xy, key_yx, "(X,Y) and (Y,X) must produce the same key");
        assert_eq!(key_xy.low(), key_yx.low());
        assert_eq!(key_xy.high(), key_yx.high());
    }

    #[test]
    fn test_dry_check_pair_key_new_rejects_self_match_when_both_path_and_hash_match() {
        let same = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let result = DryCheckPairKey::new(same.clone(), same);
        assert!(matches!(result, Err(DryCheckPairKeyError::SelfMatch)));
    }

    #[test]
    fn test_dry_check_pair_key_new_allows_same_path_different_hash() {
        // paths identical but hashes differ → valid pair (distinct content states)
        let a = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let b = make_fragment_ref("src/a.rs", &"b".repeat(64));
        let result = DryCheckPairKey::new(a, b);
        assert!(result.is_ok(), "same path with different hash is NOT a self-match");
    }

    #[test]
    fn test_dry_check_pair_key_new_allows_different_path_same_hash() {
        // complete copies in different files → valid pair
        let a = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let b = make_fragment_ref("src/b.rs", &"a".repeat(64));
        let result = DryCheckPairKey::new(a, b);
        assert!(result.is_ok(), "different path with same hash is NOT a self-match");
    }

    // ── DryCheckEntry ─────────────────────────────────────────────────────────

    #[test]
    fn test_dry_check_entry_new_round_trips_all_7_fields() {
        let low = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let high = make_fragment_ref("src/b.rs", &"b".repeat(64));
        let pair_key = DryCheckPairKey::new(low, high).unwrap();
        let changed_path = make_file_path("src/a.rs");
        let verdict = DryCheckVerdict::NotAViolation;
        let score = make_score();
        let threshold = make_threshold();
        let commit = make_commit();
        let rationale = Rationale::new("Rejected — self-similar.").unwrap();

        let entry = DryCheckEntry::new(
            pair_key.clone(),
            changed_path.clone(),
            verdict.clone(),
            score,
            threshold,
            commit.clone(),
            rationale.clone(),
        )
        .unwrap();

        assert_eq!(entry.pair_key(), &pair_key);
        assert_eq!(entry.changed_path(), &changed_path);
        assert_eq!(entry.verdict(), &verdict);
        assert_eq!(entry.similarity_score().value(), score.value());
        assert_eq!(entry.threshold().value(), threshold.value());
        assert_eq!(entry.base_commit().as_ref(), commit.as_ref());
        assert_eq!(entry.rationale(), &rationale);
    }

    #[test]
    fn test_dry_check_entry_new_rejects_changed_path_outside_pair() {
        let low = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let high = make_fragment_ref("src/b.rs", &"b".repeat(64));
        let pair_key = DryCheckPairKey::new(low, high).unwrap();
        let changed_path = make_file_path("src/c.rs"); // not in pair
        let verdict = DryCheckVerdict::NotAViolation;
        let rationale = Rationale::new("reason").unwrap();

        let result = DryCheckEntry::new(
            pair_key,
            changed_path,
            verdict,
            make_score(),
            make_threshold(),
            make_commit(),
            rationale,
        );

        assert!(matches!(result, Err(DryCheckEntryError::ChangedPathOutsidePair)));
    }

    // ── DryCheckRecord ────────────────────────────────────────────────────────

    #[test]
    fn test_dry_check_record_from_entry_and_timestamp_round_trips_with_recorded_at() {
        let low = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let high = make_fragment_ref("src/b.rs", &"b".repeat(64));
        let pair_key = DryCheckPairKey::new(low, high).unwrap();
        let changed_path = make_file_path("src/a.rs");
        let rationale = Rationale::new("acceptable").unwrap();

        let entry = DryCheckEntry::new(
            pair_key,
            changed_path,
            DryCheckVerdict::Accepted,
            make_score(),
            make_threshold(),
            make_commit(),
            rationale.clone(),
        )
        .unwrap();

        let ts = make_timestamp();
        let record = DryCheckRecord::from_entry_and_timestamp(entry, ts.clone()).unwrap();

        assert_eq!(record.recorded_at(), &ts);
        assert_eq!(record.rationale(), &rationale);
        assert_eq!(record.verdict(), &DryCheckVerdict::Accepted);
    }

    // ── DryCheckVerdict::Violation ─────────────────────────────────────────────

    #[test]
    fn test_dry_check_verdict_violation_carries_non_empty_proposal() {
        let proposal = RefactorProposal::new("Extract helper.").unwrap();
        let verdict = DryCheckVerdict::Violation { refactor_proposal: proposal.clone() };
        match verdict {
            DryCheckVerdict::Violation { refactor_proposal } => {
                assert_eq!(refactor_proposal, proposal);
            }
            _ => panic!("expected Violation variant"),
        }
    }

    // ── DryCheckFinding ───────────────────────────────────────────────────────

    #[test]
    fn test_dry_check_finding_new_with_non_empty_proposal_succeeds() {
        let changed = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let candidate = make_fragment_ref("src/b.rs", &"b".repeat(64));
        let result = DryCheckFinding::new(changed.clone(), candidate.clone(), "Extract helper.");
        assert!(result.is_ok());
        let finding = result.unwrap();
        assert_eq!(finding.changed_fragment_ref(), &changed);
        assert_eq!(finding.candidate_fragment_ref(), &candidate);
        assert_eq!(finding.refactor_proposal().as_str(), "Extract helper.");
    }

    #[test]
    fn test_dry_check_finding_new_with_empty_proposal_returns_empty_proposal_error() {
        let changed = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let candidate = make_fragment_ref("src/b.rs", &"b".repeat(64));
        let result = DryCheckFinding::new(changed, candidate, "");
        assert!(matches!(result, Err(DryCheckFindingError::EmptyProposal)));
    }

    // ── DiffHunkRange ─────────────────────────────────────────────────────────

    #[test]
    fn test_diff_hunk_range_new_with_valid_range_succeeds() {
        let result = DiffHunkRange::new(1, 10);
        assert!(result.is_ok());
        let range = result.unwrap();
        assert_eq!(range.start_line(), 1);
        assert_eq!(range.end_line(), 10);
    }

    #[test]
    fn test_diff_hunk_range_new_with_start_zero_returns_zero_line_error() {
        let result = DiffHunkRange::new(0, 10);
        assert!(matches!(result, Err(DiffHunkRangeError::ZeroLine)));
    }

    #[test]
    fn test_diff_hunk_range_new_with_end_zero_returns_zero_line_error() {
        let result = DiffHunkRange::new(1, 0);
        assert!(matches!(result, Err(DiffHunkRangeError::ZeroLine)));
    }

    #[test]
    fn test_diff_hunk_range_new_with_start_greater_than_end_returns_start_exceeds_end_error() {
        let result = DiffHunkRange::new(10, 5);
        assert!(matches!(result, Err(DiffHunkRangeError::StartExceedsEnd { start: 10, end: 5 })));
    }

    #[test]
    fn test_diff_hunk_range_new_with_single_line_range_succeeds() {
        let result = DiffHunkRange::new(5, 5);
        assert!(result.is_ok());
    }

    // ── DiffFileHunks ─────────────────────────────────────────────────────────

    #[test]
    fn test_diff_file_hunks_new_with_non_empty_hunks_succeeds() {
        let path = make_file_path("src/a.rs");
        let hunk = DiffHunkRange::new(1, 10).unwrap();
        let result = DiffFileHunks::new(path.clone(), vec![hunk.clone()]);
        assert!(result.is_ok());
        let dfh = result.unwrap();
        assert_eq!(dfh.path(), &path);
        assert_eq!(dfh.hunks(), &[hunk]);
    }

    #[test]
    fn test_diff_file_hunks_new_with_empty_hunks_returns_empty_hunks_error() {
        let path = make_file_path("src/a.rs");
        let result = DiffFileHunks::new(path, vec![]);
        assert!(matches!(result, Err(DiffFileHunksError::EmptyHunks)));
    }

    // ── fragments_overlapping_hunks ───────────────────────────────────────────

    fn make_code_fragment(
        path: &str,
        content: &str,
        start_line: u32,
        end_line: u32,
    ) -> CodeFragment {
        CodeFragment::new(PathBuf::from(path), content.to_owned(), start_line, end_line).unwrap()
    }

    #[test]
    fn test_fragments_overlapping_hunks_returns_overlapping_fragments() {
        // Fragment at lines 5-10 in src/a.rs; hunk covers lines 8-12.
        let frag = make_code_fragment("src/a.rs", "fn foo() {}", 5, 10);
        let hunk = DiffHunkRange::new(8, 12).unwrap();
        let file_hunks = DiffFileHunks::new(make_file_path("src/a.rs"), vec![hunk]).unwrap();

        let result = fragments_overlapping_hunks(std::slice::from_ref(&frag), &[file_hunks]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content(), frag.content());
    }

    #[test]
    fn test_fragments_overlapping_hunks_repo_relative_path_matches_exact() {
        // Both fragment path and hunk path are repo-relative — exact match succeeds.
        let frag = make_code_fragment("src/a.rs", "fn foo() {}", 5, 10);
        let hunk = DiffHunkRange::new(8, 12).unwrap();
        let file_hunks = DiffFileHunks::new(make_file_path("src/a.rs"), vec![hunk]).unwrap();

        let result = fragments_overlapping_hunks(std::slice::from_ref(&frag), &[file_hunks]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content(), frag.content());
    }

    #[test]
    fn test_fragments_overlapping_hunks_suffix_path_does_not_match_hunk_path() {
        // Regression: `tests/src/a.rs` must NOT match hunk path `src/a.rs`.
        // Path::ends_with would spuriously match because `src/a.rs` is a component
        // suffix of `tests/src/a.rs`. The domain contract requires exact (repo-relative)
        // path equality; suffix matching is prohibited (CN-04 correctness).
        let frag = make_code_fragment("tests/src/a.rs", "fn test_foo() {}", 8, 12);
        let hunk = DiffHunkRange::new(8, 12).unwrap();
        let file_hunks = DiffFileHunks::new(make_file_path("src/a.rs"), vec![hunk]).unwrap();

        let result = fragments_overlapping_hunks(std::slice::from_ref(&frag), &[file_hunks]);
        assert!(
            result.is_empty(),
            "`tests/src/a.rs` must not match hunk path `src/a.rs` (suffix match prohibited)"
        );
    }

    #[test]
    fn test_fragments_overlapping_hunks_excludes_non_overlapping_fragments() {
        // Fragment at lines 1-4 in src/a.rs; hunk covers lines 8-12 (no overlap).
        let frag = make_code_fragment("src/a.rs", "fn bar() {}", 1, 4);
        let hunk = DiffHunkRange::new(8, 12).unwrap();
        let file_hunks = DiffFileHunks::new(make_file_path("src/a.rs"), vec![hunk]).unwrap();

        let result = fragments_overlapping_hunks(&[frag], &[file_hunks]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_fragments_overlapping_hunks_excludes_fragments_from_other_files() {
        // Fragment in src/b.rs; hunk is in src/a.rs.
        let frag = make_code_fragment("src/b.rs", "fn baz() {}", 1, 20);
        let hunk = DiffHunkRange::new(1, 20).unwrap();
        let file_hunks = DiffFileHunks::new(make_file_path("src/a.rs"), vec![hunk]).unwrap();

        let result = fragments_overlapping_hunks(&[frag], &[file_hunks]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_fragments_overlapping_hunks_sentinel_query_fragment_always_overlaps() {
        // Ad-hoc query fragment with start_line=1, end_line=u32::MAX always overlaps.
        let frag = make_code_fragment("<query>", "fn query() {}", 1, u32::MAX);
        let hunk = DiffHunkRange::new(1, 100).unwrap();
        // For the query path to match, we'd need to use "<query>" as the file name.
        // Test with a normal file path overlap instead (query path won't match real file).
        let frag2 = make_code_fragment("src/a.rs", "fn real() {}", 1, u32::MAX);
        let file_hunks = DiffFileHunks::new(make_file_path("src/a.rs"), vec![hunk]).unwrap();

        let result = fragments_overlapping_hunks(&[frag, frag2.clone()], &[file_hunks]);
        // Only frag2 matches the file path "src/a.rs"
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content(), frag2.content());
    }

    #[test]
    fn test_fragments_overlapping_hunks_with_empty_inputs_returns_empty() {
        let result = fragments_overlapping_hunks(&[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_fragments_overlapping_hunks_with_empty_hunks_list_excludes_all() {
        let frag = make_code_fragment("src/a.rs", "fn foo() {}", 1, 10);
        let result = fragments_overlapping_hunks(&[frag], &[]);
        assert!(result.is_empty());
    }
}
