//! Record types: `DryCheckEntry` and `DryCheckRecord`.

use thiserror::Error;

use crate::ids::CommitHash;
use crate::review_v2::types::FilePath;
use crate::semantic_dup::{SimilarityScore, SimilarityThreshold};
use crate::timestamp::Timestamp;

use super::fragment::DryCheckPairKey;
use super::value_objects::Rationale;
use super::verdict::DryCheckVerdict;

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
