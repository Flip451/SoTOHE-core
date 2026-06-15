//! Coverage record for the read-only `dry check-approved` staleness gate (D5).

use std::collections::BTreeSet;

use super::fragment::{DryCheckPairKey, FragmentRef};

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryCheckCoverageRecord {
    fragment_refs: BTreeSet<FragmentRef>,
    processed_pair_keys: BTreeSet<DryCheckPairKey>,
}

impl DryCheckCoverageRecord {
    /// Construct a [`DryCheckCoverageRecord`] from the set of processed fragment refs
    /// and the set of pair keys judged in the latest `dry write` run.
    ///
    /// Empty sets are permitted (a `dry write` run that processed no diff
    /// fragments produces empty coverage records).
    pub fn new(
        fragment_refs: BTreeSet<FragmentRef>,
        processed_pair_keys: BTreeSet<DryCheckPairKey>,
    ) -> DryCheckCoverageRecord {
        DryCheckCoverageRecord { fragment_refs, processed_pair_keys }
    }

    /// Return the set of covered [`FragmentRef`]s.
    pub fn fragment_refs(&self) -> &BTreeSet<FragmentRef> {
        &self.fragment_refs
    }

    /// Return the set of [`DryCheckPairKey`]s that were judged in the latest run.
    pub fn processed_pair_keys(&self) -> &BTreeSet<DryCheckPairKey> {
        &self.processed_pair_keys
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

    #[test]
    fn test_dry_check_coverage_record_new_with_empty_set_succeeds() {
        let record = DryCheckCoverageRecord::new(BTreeSet::new(), BTreeSet::new());
        assert!(record.fragment_refs().is_empty());
        assert!(record.processed_pair_keys().is_empty());
    }

    #[test]
    fn test_dry_check_coverage_record_new_with_non_empty_set_retains_all_refs() {
        let a = make_fragment_ref("src/a.rs", 'a');
        let b = make_fragment_ref("src/b.rs", 'b');
        let mut refs = BTreeSet::new();
        refs.insert(a.clone());
        refs.insert(b.clone());

        let record = DryCheckCoverageRecord::new(refs, BTreeSet::new());

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
        let record = DryCheckCoverageRecord::new(refs, BTreeSet::new());

        assert!(record.covers(&a), "the recorded (path, hash) is covered");
        assert!(!record.covers(&b), "same content_hash at a different path must NOT be covered");
    }

    #[test]
    fn test_dry_check_coverage_record_is_cloneable_and_eq() {
        let a = make_fragment_ref("src/a.rs", 'a');
        let mut refs = BTreeSet::new();
        refs.insert(a);
        let record = DryCheckCoverageRecord::new(refs, BTreeSet::new());
        let clone = record.clone();
        assert_eq!(record, clone);
    }

    #[test]
    fn test_dry_check_coverage_record_contains_pair_returns_true_when_present() {
        let pair = make_pair_key("src/a.rs", 'a', "src/b.rs", 'b');
        let mut pairs = BTreeSet::new();
        pairs.insert(pair.clone());
        let record = DryCheckCoverageRecord::new(BTreeSet::new(), pairs);

        assert!(record.contains_pair(&pair));
    }

    #[test]
    fn test_dry_check_coverage_record_contains_pair_returns_false_when_absent() {
        let pair_in = make_pair_key("src/a.rs", 'a', "src/b.rs", 'b');
        let pair_out = make_pair_key("src/c.rs", 'c', "src/d.rs", 'd');
        let mut pairs = BTreeSet::new();
        pairs.insert(pair_in.clone());
        let record = DryCheckCoverageRecord::new(BTreeSet::new(), pairs);

        assert!(record.contains_pair(&pair_in));
        assert!(!record.contains_pair(&pair_out));
    }

    #[test]
    fn test_dry_check_coverage_record_contains_pair_empty_set_always_returns_false() {
        let pair = make_pair_key("src/a.rs", 'a', "src/b.rs", 'b');
        let record = DryCheckCoverageRecord::new(BTreeSet::new(), BTreeSet::new());
        assert!(!record.contains_pair(&pair));
    }
}
