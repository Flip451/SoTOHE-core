//! Record types: `DryCheckEntry` and `DryCheckRecord`.

use thiserror::Error;

use crate::ids::CommitHash;
use crate::review_v2::types::FilePath;
use crate::semantic_dup::{SimilarityScore, SimilarityThreshold};
use crate::timestamp::Timestamp;

use super::coverage::DryCheckConfigFingerprint;
use super::fragment::DryCheckPairKey;
use super::value_objects::Rationale;
use super::verdict::DryCheckVerdict;

// ── DryCheckEntry ─────────────────────────────────────────────────────────────

/// Write-input type for the dry-check persistence path (write-read separation).
///
/// Carries the 8 fields the interactor knows at verdict time. Does NOT carry
/// `recorded_at` — the infra adapter (`FsDryCheckStore`) stamps `Timestamp`
/// internally.
///
/// `config_fingerprint` identifies which `.harness/config/dry-check.json`
/// configuration was active when this entry was judged. On read, the interactor
/// uses it to decide whether to seed the `verified_set` with this record: records
/// whose fingerprint differs from the current config are NOT added to
/// `verified_set` and will be re-judged under the new config.
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
    config_fingerprint: DryCheckConfigFingerprint,
}

impl DryCheckEntry {
    /// Construct a [`DryCheckEntry`].
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckEntryError::ChangedPathOutsidePair`] when `changed_path`
    /// is neither `pair_key.low().path()` nor `pair_key.high().path()`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pair_key: DryCheckPairKey,
        changed_path: FilePath,
        verdict: DryCheckVerdict,
        similarity_score: SimilarityScore,
        threshold: SimilarityThreshold,
        base_commit: CommitHash,
        rationale: Rationale,
        config_fingerprint: DryCheckConfigFingerprint,
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
            config_fingerprint,
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

    /// Return the config fingerprint that was active when this entry was judged.
    ///
    /// Used by the interactor to filter the `verified_set` seed: only records
    /// whose fingerprint matches the current config are added to `verified_set`.
    /// Records under a different fingerprint (stale config) will be re-judged.
    pub fn config_fingerprint(&self) -> &DryCheckConfigFingerprint {
        &self.config_fingerprint
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
/// `Timestamp`. The interactor constructs [`DryCheckEntry`] (8 fields, no
/// `recorded_at`) and passes it to `DryCheckWriter::append_record`; the infra
/// adapter builds this record.
///
/// Illegal states are unrepresentable by construction (D9):
/// - `pair_key.low > pair_key.high` is impossible (sorted by `DryCheckPairKey::new`).
/// - Self-match is impossible (`DryCheckPairKey::new` rejects equal refs).
/// - `changed_path` outside the pair is impossible (`DryCheckEntry::new` validates it).
/// - `recorded_at`, `rationale`, and `refactor_proposal` (in `Violation`) are always valid.
///
/// `config_fingerprint` is the fingerprint of the `.harness/config/dry-check.json`
/// settings that were active when this record was written. The interactor uses it
/// during `verified_set` seeding to skip records written under a different config.
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
    config_fingerprint: DryCheckConfigFingerprint,
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
            config_fingerprint: entry.config_fingerprint,
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

    /// Return the config fingerprint embedded when this record was written.
    ///
    /// The interactor seeds `verified_set` only from records whose fingerprint
    /// matches the current config. Records with a different fingerprint are
    /// excluded, forcing re-judgment under the updated config.
    pub fn config_fingerprint(&self) -> &DryCheckConfigFingerprint {
        &self.config_fingerprint
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
