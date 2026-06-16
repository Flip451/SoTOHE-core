//! Coverage record for the read-only `dry check-approved` staleness gate (D5).

use std::collections::BTreeSet;

use super::fragment::{DryCheckPairKey, FragmentRef};

fn is_valid_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

// ── DryCheckCorpusFingerprint ─────────────────────────────────────────────────

/// A stable fingerprint (SHA-256 hex string) over the corpus scanned by the
/// `dry write` indexer.
///
/// The corpus fingerprint captures the full set of `(repo_relative_path,
/// content_hash)` pairs that the indexer saw during a `dry write` run.  A
/// change to any corpus file (add, delete, modify) produces a different
/// fingerprint, allowing `dry check-approved` to detect when the index is stale
/// even though the diff-side fragments are unchanged.
///
/// The serialized value is exactly 64 lowercase hex characters (SHA-256).  The
/// empty 64-zero string is used as the persisted fail-closed sentinel. Coverage
/// records preserve whether that sentinel came from a failed write, so a later
/// failed read cannot compare equal and accidentally approve stale coverage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryCheckCorpusFingerprint {
    value: String,
    fail_closed_state: DryCheckCorpusFingerprintFailClosedState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DryCheckCorpusFingerprintFailClosedState {
    Observed,
    CurrentFailure,
    RecordedFailure,
}

/// Validation errors for [`DryCheckCorpusFingerprint`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DryCheckCorpusFingerprintError {
    /// The provided string is not exactly 64 lowercase hex characters.
    #[error("corpus fingerprint must be exactly 64 lowercase hex characters, got: {0:?}")]
    InvalidFormat(String),
}

impl DryCheckCorpusFingerprint {
    /// Construct a [`DryCheckCorpusFingerprint`] from a 64-char lowercase hex string.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckCorpusFingerprintError::InvalidFormat`] if `s` is not
    /// exactly 64 lowercase hexadecimal characters.
    pub fn new(
        s: impl Into<String>,
    ) -> Result<DryCheckCorpusFingerprint, DryCheckCorpusFingerprintError> {
        let s = s.into();
        if !is_valid_sha256_hex(&s) {
            return Err(DryCheckCorpusFingerprintError::InvalidFormat(s));
        }
        Ok(DryCheckCorpusFingerprint {
            value: s,
            fail_closed_state: DryCheckCorpusFingerprintFailClosedState::Observed,
        })
    }

    /// The all-zeros sentinel fingerprint used on fail-closed writes/reads.
    ///
    /// When stored in a [`DryCheckCoverageRecord`], this value is marked as a
    /// recorded failure while still serializing to the same 64-zero string. That
    /// prevents failed write/read paths from comparing equal.
    pub fn fail_closed() -> DryCheckCorpusFingerprint {
        DryCheckCorpusFingerprint {
            value: "0".repeat(64),
            fail_closed_state: DryCheckCorpusFingerprintFailClosedState::CurrentFailure,
        }
    }

    fn into_recorded(self) -> DryCheckCorpusFingerprint {
        match self.fail_closed_state {
            DryCheckCorpusFingerprintFailClosedState::CurrentFailure => DryCheckCorpusFingerprint {
                value: self.value,
                fail_closed_state: DryCheckCorpusFingerprintFailClosedState::RecordedFailure,
            },
            DryCheckCorpusFingerprintFailClosedState::Observed
            | DryCheckCorpusFingerprintFailClosedState::RecordedFailure => self,
        }
    }

    /// Return the inner 64-char hex string.
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

// ── DryCheckConfigFingerprint ─────────────────────────────────────────────────

/// A stable fingerprint (SHA-256 hex string) over all `dry-check.json` fields
/// that affect `dry write` semantics.
///
/// Written into the coverage manifest by `dry write` and compared by
/// `dry check-approved` against the current config.  If the fingerprints differ
/// (the config was changed since the last `dry write` run), `check_approved`
/// returns `Blocked` so that the new threshold / calibration settings are applied.
///
/// The inner value is exactly 64 lowercase hex characters (SHA-256).  The empty
/// 64-zero string is used as a sentinel for the fail-closed "no valid run"
/// case (calibration failure, embed/index/agent error) — this guarantees that
/// `check_approved` returns `Blocked` regardless of the current config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryCheckConfigFingerprint(String);

/// Validation errors for [`DryCheckConfigFingerprint`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DryCheckConfigFingerprintError {
    /// The provided string is not exactly 64 lowercase hex characters.
    #[error("config fingerprint must be exactly 64 lowercase hex characters, got: {0:?}")]
    InvalidFormat(String),
}

impl DryCheckConfigFingerprint {
    /// Construct a [`DryCheckConfigFingerprint`] from a 64-char lowercase hex string.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckConfigFingerprintError::InvalidFormat`] if `s` is not
    /// exactly 64 lowercase hexadecimal characters.
    pub fn new(
        s: impl Into<String>,
    ) -> Result<DryCheckConfigFingerprint, DryCheckConfigFingerprintError> {
        let s = s.into();
        if !is_valid_sha256_hex(&s) {
            return Err(DryCheckConfigFingerprintError::InvalidFormat(s));
        }
        Ok(DryCheckConfigFingerprint(s))
    }

    /// The all-zeros sentinel fingerprint used on fail-closed writes.
    ///
    /// Any non-zero fingerprint (even one that matches the current config) will
    /// differ from this sentinel, so `check_approved` returns `Blocked`.
    pub fn fail_closed() -> DryCheckConfigFingerprint {
        DryCheckConfigFingerprint("0".repeat(64))
    }

    /// Return the inner 64-char hex string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ── DryCheckCoverageRecord ─────────────────────────────────────────────────────

/// The set of diff-fragment [`FragmentRef`]s and judged [`DryCheckPairKey`]s
/// that a `dry write` run has processed.
///
/// `dry check-approved` (D5) reads this record and checks, for each current diff
/// fragment's `FragmentRef = (path, content_hash)`, whether it is present here.
///
/// Coverage is matched at **`FragmentRef` granularity**, NOT by `content_hash`
/// alone: an identical `content_hash` at a *different* path is a distinct
/// `FragmentRef` and is therefore NOT treated as covered (IN-06 / CN-08).
/// `FragmentRef`'s `Eq` / `Ord` over `(path, content_hash)` makes the
/// `BTreeSet` enforce this identity automatically.
///
/// `processed_pair_keys` tracks every [`DryCheckPairKey`] that was actually
/// considered (sent to the agent OR was a verified-set hit) during Phase 2 of
/// the last `dry write` run. The gate uses this set to skip historical
/// dry-check records whose pair was NOT re-judged in the latest run — such
/// records are stale because the candidate side may have been fixed or removed.
///
/// `config_fingerprint` is a SHA-256 fingerprint of all `dry-check.json` fields
/// that affect `dry write` semantics (threshold, max_parallelism, reasoning
/// efforts, known-bad percentages).  `check_approved` compares this against the
/// current config: a mismatch means the coverage was built under a different
/// config (e.g., the threshold was lowered) and must be regenerated.
///
/// `corpus_fingerprint` is a SHA-256 fingerprint over the full set of
/// `(repo_relative_path, content_hash)` pairs that the corpus indexer scanned
/// during the `dry write` run.  `check_approved` compares this against the
/// current corpus fingerprint: a mismatch means the index is stale (a corpus
/// file was added, removed, or changed) and the coverage must be regenerated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryCheckCoverageRecord {
    fragment_refs: BTreeSet<FragmentRef>,
    processed_pair_keys: BTreeSet<DryCheckPairKey>,
    config_fingerprint: DryCheckConfigFingerprint,
    corpus_fingerprint: DryCheckCorpusFingerprint,
}

impl DryCheckCoverageRecord {
    /// Construct a [`DryCheckCoverageRecord`] from the set of processed fragment refs,
    /// the set of pair keys judged in the latest `dry write` run, the config
    /// fingerprint of the `dry-check.json` settings used by that run, and the
    /// corpus fingerprint of the workspace files scanned by the indexer.
    ///
    /// Empty sets are permitted (a `dry write` run that processed no diff
    /// fragments produces empty coverage records).  Use
    /// [`DryCheckConfigFingerprint::fail_closed`] / [`DryCheckCorpusFingerprint::fail_closed`]
    /// as the fingerprints when the run failed (calibration error, embed/index/agent/writer
    /// error) to guarantee that `check_approved` returns `Blocked` on the subsequent read.
    pub fn new(
        fragment_refs: BTreeSet<FragmentRef>,
        processed_pair_keys: BTreeSet<DryCheckPairKey>,
        config_fingerprint: DryCheckConfigFingerprint,
        corpus_fingerprint: DryCheckCorpusFingerprint,
    ) -> DryCheckCoverageRecord {
        DryCheckCoverageRecord {
            fragment_refs,
            processed_pair_keys,
            config_fingerprint,
            corpus_fingerprint: corpus_fingerprint.into_recorded(),
        }
    }

    /// Return the set of covered [`FragmentRef`]s.
    pub fn fragment_refs(&self) -> &BTreeSet<FragmentRef> {
        &self.fragment_refs
    }

    /// Return the set of [`DryCheckPairKey`]s that were judged in the latest run.
    pub fn processed_pair_keys(&self) -> &BTreeSet<DryCheckPairKey> {
        &self.processed_pair_keys
    }

    /// Return the config fingerprint written by the latest `dry write` run.
    ///
    /// `check_approved` compares this against the current `dry-check.json`
    /// fingerprint.  A mismatch means the config changed and coverage is stale.
    pub fn config_fingerprint(&self) -> &DryCheckConfigFingerprint {
        &self.config_fingerprint
    }

    /// Return the corpus fingerprint written by the latest `dry write` run.
    ///
    /// `check_approved` compares this against the current corpus fingerprint.
    /// A mismatch means one or more corpus files changed (added, removed, or
    /// modified) since the last `dry write` run, so the index is stale.
    pub fn corpus_fingerprint(&self) -> &DryCheckCorpusFingerprint {
        &self.corpus_fingerprint
    }

    /// Return `true` when `fragment_ref` (exact `(path, content_hash)` identity)
    /// is present in the coverage set.
    ///
    /// An identical `content_hash` at a different path is NOT covered — the
    /// staleness gate must force a fresh `dry write` for such a fragment.
    pub fn covers(&self, fragment_ref: &FragmentRef) -> bool {
        self.fragment_refs.contains(fragment_ref)
    }

    /// Return `true` when `pair_key` was judged in the latest `dry write` run.
    ///
    /// The gate uses this to skip stale historical records whose pair was
    /// NOT re-examined in the most recent run (stale candidate-side pairs).
    pub fn contains_pair(&self, pair_key: &DryCheckPairKey) -> bool {
        self.processed_pair_keys.contains(pair_key)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::dry_check::fragment::DryCheckPairKey;
    use crate::dry_check::value_objects::FragmentContentHash;
    use crate::review_v2::types::FilePath;

    fn make_fragment_ref(path: &str, hash_char: char) -> FragmentRef {
        let hash = hash_char.to_string().repeat(64);
        FragmentRef::new(FilePath::new(path).unwrap(), FragmentContentHash::new(hash).unwrap())
    }

    fn make_pair_key(path_a: &str, hash_a: char, path_b: &str, hash_b: char) -> DryCheckPairKey {
        let a = make_fragment_ref(path_a, hash_a);
        let b = make_fragment_ref(path_b, hash_b);
        DryCheckPairKey::new(a, b).unwrap()
    }

    fn test_config_fingerprint() -> DryCheckConfigFingerprint {
        DryCheckConfigFingerprint::new("a".repeat(64)).unwrap()
    }

    fn test_corpus_fingerprint() -> DryCheckCorpusFingerprint {
        DryCheckCorpusFingerprint::new("b".repeat(64)).unwrap()
    }

    // legacy alias for tests that only care about config fingerprint
    fn test_fingerprint() -> DryCheckConfigFingerprint {
        test_config_fingerprint()
    }

    // ── DryCheckConfigFingerprint tests ──────────────────────────────────────

    #[test]
    fn test_dry_check_config_fingerprint_new_with_valid_64_hex_succeeds() {
        let fp = DryCheckConfigFingerprint::new("a".repeat(64));
        assert!(fp.is_ok());
        assert_eq!(fp.unwrap().as_str(), &"a".repeat(64));
    }

    #[test]
    fn test_dry_check_config_fingerprint_new_with_63_chars_returns_invalid_format() {
        let fp = DryCheckConfigFingerprint::new("a".repeat(63));
        assert!(matches!(fp, Err(DryCheckConfigFingerprintError::InvalidFormat(_))));
    }

    #[test]
    fn test_dry_check_config_fingerprint_new_with_65_chars_returns_invalid_format() {
        let fp = DryCheckConfigFingerprint::new("a".repeat(65));
        assert!(matches!(fp, Err(DryCheckConfigFingerprintError::InvalidFormat(_))));
    }

    #[test]
    fn test_dry_check_config_fingerprint_new_with_uppercase_returns_invalid_format() {
        let fp = DryCheckConfigFingerprint::new("A".repeat(64));
        assert!(matches!(fp, Err(DryCheckConfigFingerprintError::InvalidFormat(_))));
    }

    #[test]
    fn test_dry_check_config_fingerprint_new_with_non_hex_returns_invalid_format() {
        let fp = DryCheckConfigFingerprint::new("g".repeat(64));
        assert!(matches!(fp, Err(DryCheckConfigFingerprintError::InvalidFormat(_))));
    }

    #[test]
    fn test_dry_check_config_fingerprint_fail_closed_is_all_zeros() {
        let fp = DryCheckConfigFingerprint::fail_closed();
        assert_eq!(fp.as_str(), &"0".repeat(64));
    }

    #[test]
    fn test_dry_check_config_fingerprint_fail_closed_differs_from_valid_fingerprint() {
        let fail_closed = DryCheckConfigFingerprint::fail_closed();
        let valid = DryCheckConfigFingerprint::new("a".repeat(64)).unwrap();
        assert_ne!(fail_closed, valid);
    }

    // ── DryCheckCoverageRecord tests ──────────────────────────────────────────

    #[test]
    fn test_dry_check_coverage_record_new_with_empty_set_succeeds() {
        let record = DryCheckCoverageRecord::new(
            BTreeSet::new(),
            BTreeSet::new(),
            test_fingerprint(),
            test_corpus_fingerprint(),
        );
        assert!(record.fragment_refs().is_empty());
        assert!(record.processed_pair_keys().is_empty());
        assert_eq!(record.config_fingerprint(), &test_fingerprint());
        assert_eq!(record.corpus_fingerprint(), &test_corpus_fingerprint());
    }

    #[test]
    fn test_dry_check_coverage_record_new_with_non_empty_set_retains_all_refs() {
        let a = make_fragment_ref("src/a.rs", 'a');
        let b = make_fragment_ref("src/b.rs", 'b');
        let mut refs = BTreeSet::new();
        refs.insert(a.clone());
        refs.insert(b.clone());

        let record = DryCheckCoverageRecord::new(
            refs,
            BTreeSet::new(),
            test_fingerprint(),
            test_corpus_fingerprint(),
        );

        assert_eq!(record.fragment_refs().len(), 2);
        assert!(record.covers(&a));
        assert!(record.covers(&b));
    }

    #[test]
    fn test_dry_check_coverage_record_distinguishes_same_hash_different_path() {
        // Identical content_hash at two different paths must be two distinct
        // FragmentRefs (IN-06 / CN-08): hash alone is never treated as covered.
        let same_hash = 'a';
        let a = make_fragment_ref("src/a.rs", same_hash);
        let b = make_fragment_ref("src/b.rs", same_hash);
        assert_eq!(a.content_hash(), b.content_hash(), "precondition: same content_hash");

        let mut refs = BTreeSet::new();
        refs.insert(a.clone());
        let record = DryCheckCoverageRecord::new(
            refs,
            BTreeSet::new(),
            test_fingerprint(),
            test_corpus_fingerprint(),
        );

        assert!(record.covers(&a), "the recorded (path, hash) is covered");
        assert!(!record.covers(&b), "same content_hash at a different path must NOT be covered");
    }

    #[test]
    fn test_dry_check_coverage_record_is_cloneable_and_eq() {
        let a = make_fragment_ref("src/a.rs", 'a');
        let mut refs = BTreeSet::new();
        refs.insert(a);
        let record = DryCheckCoverageRecord::new(
            refs,
            BTreeSet::new(),
            test_fingerprint(),
            test_corpus_fingerprint(),
        );
        let clone = record.clone();
        assert_eq!(record, clone);
    }

    #[test]
    fn test_dry_check_coverage_record_contains_pair_returns_true_when_present() {
        let pair = make_pair_key("src/a.rs", 'a', "src/b.rs", 'b');
        let mut pairs = BTreeSet::new();
        pairs.insert(pair.clone());
        let record = DryCheckCoverageRecord::new(
            BTreeSet::new(),
            pairs,
            test_fingerprint(),
            test_corpus_fingerprint(),
        );

        assert!(record.contains_pair(&pair));
    }

    #[test]
    fn test_dry_check_coverage_record_contains_pair_returns_false_when_absent() {
        let pair_in = make_pair_key("src/a.rs", 'a', "src/b.rs", 'b');
        let pair_out = make_pair_key("src/c.rs", 'c', "src/d.rs", 'd');
        let mut pairs = BTreeSet::new();
        pairs.insert(pair_in.clone());
        let record = DryCheckCoverageRecord::new(
            BTreeSet::new(),
            pairs,
            test_fingerprint(),
            test_corpus_fingerprint(),
        );

        assert!(record.contains_pair(&pair_in));
        assert!(!record.contains_pair(&pair_out));
    }

    #[test]
    fn test_dry_check_coverage_record_contains_pair_empty_set_always_returns_false() {
        let pair = make_pair_key("src/a.rs", 'a', "src/b.rs", 'b');
        let record = DryCheckCoverageRecord::new(
            BTreeSet::new(),
            BTreeSet::new(),
            test_fingerprint(),
            test_corpus_fingerprint(),
        );
        assert!(!record.contains_pair(&pair));
    }

    #[test]
    fn test_dry_check_coverage_record_config_fingerprint_accessor_returns_stored_value() {
        let fp = DryCheckConfigFingerprint::new("c".repeat(64)).unwrap();
        let record = DryCheckCoverageRecord::new(
            BTreeSet::new(),
            BTreeSet::new(),
            fp.clone(),
            test_corpus_fingerprint(),
        );
        assert_eq!(record.config_fingerprint(), &fp);
    }

    // ── DryCheckCorpusFingerprint tests ──────────────────────────────────────

    #[test]
    fn test_dry_check_corpus_fingerprint_new_with_valid_64_hex_succeeds() {
        let fp = DryCheckCorpusFingerprint::new("b".repeat(64));
        assert!(fp.is_ok());
        assert_eq!(fp.unwrap().as_str(), &"b".repeat(64));
    }

    #[test]
    fn test_dry_check_corpus_fingerprint_new_with_63_chars_returns_invalid_format() {
        let fp = DryCheckCorpusFingerprint::new("b".repeat(63));
        assert!(matches!(fp, Err(DryCheckCorpusFingerprintError::InvalidFormat(_))));
    }

    #[test]
    fn test_dry_check_corpus_fingerprint_new_with_65_chars_returns_invalid_format() {
        let fp = DryCheckCorpusFingerprint::new("b".repeat(65));
        assert!(matches!(fp, Err(DryCheckCorpusFingerprintError::InvalidFormat(_))));
    }

    #[test]
    fn test_dry_check_corpus_fingerprint_new_with_uppercase_returns_invalid_format() {
        let fp = DryCheckCorpusFingerprint::new("B".repeat(64));
        assert!(matches!(fp, Err(DryCheckCorpusFingerprintError::InvalidFormat(_))));
    }

    #[test]
    fn test_dry_check_corpus_fingerprint_new_with_non_hex_returns_invalid_format() {
        let fp = DryCheckCorpusFingerprint::new("z".repeat(64));
        assert!(matches!(fp, Err(DryCheckCorpusFingerprintError::InvalidFormat(_))));
    }

    #[test]
    fn test_dry_check_corpus_fingerprint_fail_closed_is_all_zeros() {
        let fp = DryCheckCorpusFingerprint::fail_closed();
        assert_eq!(fp.as_str(), &"0".repeat(64));
    }

    #[test]
    fn test_dry_check_corpus_fingerprint_fail_closed_instances_compare_equal() {
        let left = DryCheckCorpusFingerprint::fail_closed();
        let right = DryCheckCorpusFingerprint::fail_closed();
        assert_eq!(left, right);
    }

    #[test]
    fn test_dry_check_corpus_fingerprint_fail_closed_differs_from_deserialized_zero_sentinel() {
        let fail_closed = DryCheckCorpusFingerprint::fail_closed();
        let deserialized = DryCheckCorpusFingerprint::new("0".repeat(64)).unwrap();
        assert_ne!(fail_closed, deserialized);
    }

    #[test]
    fn test_dry_check_corpus_fingerprint_fail_closed_differs_from_valid_fingerprint() {
        let fail_closed = DryCheckCorpusFingerprint::fail_closed();
        let valid = DryCheckCorpusFingerprint::new("b".repeat(64)).unwrap();
        assert_ne!(fail_closed, valid);
    }

    #[test]
    fn test_dry_check_coverage_record_fail_closed_corpus_differs_from_current_fail_closed() {
        let record = DryCheckCoverageRecord::new(
            BTreeSet::new(),
            BTreeSet::new(),
            test_fingerprint(),
            DryCheckCorpusFingerprint::fail_closed(),
        );
        let current_failed_read = DryCheckCorpusFingerprint::fail_closed();
        assert_ne!(record.corpus_fingerprint(), &current_failed_read);
    }

    #[test]
    fn test_dry_check_coverage_record_corpus_fingerprint_accessor_returns_stored_value() {
        let cfp = DryCheckCorpusFingerprint::new("d".repeat(64)).unwrap();
        let record = DryCheckCoverageRecord::new(
            BTreeSet::new(),
            BTreeSet::new(),
            test_fingerprint(),
            cfp.clone(),
        );
        assert_eq!(record.corpus_fingerprint(), &cfp);
    }
}
