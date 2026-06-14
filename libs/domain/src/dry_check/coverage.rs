//! Coverage record for the read-only `dry check-approved` staleness gate (D5).

use std::collections::BTreeSet;

use super::fragment::FragmentRef;

// ── DryCheckCoverageRecord ─────────────────────────────────────────────────────

/// The set of diff-fragment [`FragmentRef`]s that a `dry write` run has processed.
///
/// `dry check-approved` (D5) reads this record and checks, for each current diff
/// fragment's `FragmentRef = (path, content_hash)`, whether it is present here.
///
/// Coverage is matched at **`FragmentRef` granularity**, NOT by `content_hash`
/// alone: an identical `content_hash` at a *different* path is a distinct
/// `FragmentRef` and is therefore NOT treated as covered (IN-06 / CN-08).
/// `FragmentRef`'s `Eq` / `Ord` over `(path, content_hash)` makes the
/// `BTreeSet` enforce this identity automatically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryCheckCoverageRecord {
    fragment_refs: BTreeSet<FragmentRef>,
}

impl DryCheckCoverageRecord {
    /// Construct a [`DryCheckCoverageRecord`] from the set of processed fragment refs.
    ///
    /// An empty set is permitted (a `dry write` run that processed no diff
    /// fragments produces an empty coverage record).
    pub fn new(fragment_refs: BTreeSet<FragmentRef>) -> DryCheckCoverageRecord {
        DryCheckCoverageRecord { fragment_refs }
    }

    /// Return the set of covered [`FragmentRef`]s.
    pub fn fragment_refs(&self) -> &BTreeSet<FragmentRef> {
        &self.fragment_refs
    }

    /// Return `true` when `fragment_ref` (exact `(path, content_hash)` identity)
    /// is present in the coverage set.
    ///
    /// An identical `content_hash` at a different path is NOT covered — the
    /// staleness gate must force a fresh `dry write` for such a fragment.
    pub fn covers(&self, fragment_ref: &FragmentRef) -> bool {
        self.fragment_refs.contains(fragment_ref)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::dry_check::value_objects::FragmentContentHash;
    use crate::review_v2::types::FilePath;

    fn make_fragment_ref(path: &str, hash_char: char) -> FragmentRef {
        let hash = hash_char.to_string().repeat(64);
        FragmentRef::new(FilePath::new(path).unwrap(), FragmentContentHash::new(hash).unwrap())
    }

    #[test]
    fn test_dry_check_coverage_record_new_with_empty_set_succeeds() {
        let record = DryCheckCoverageRecord::new(BTreeSet::new());
        assert!(record.fragment_refs().is_empty());
    }

    #[test]
    fn test_dry_check_coverage_record_new_with_non_empty_set_retains_all_refs() {
        let a = make_fragment_ref("src/a.rs", 'a');
        let b = make_fragment_ref("src/b.rs", 'b');
        let mut refs = BTreeSet::new();
        refs.insert(a.clone());
        refs.insert(b.clone());

        let record = DryCheckCoverageRecord::new(refs);

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
        let record = DryCheckCoverageRecord::new(refs);

        assert!(record.covers(&a), "the recorded (path, hash) is covered");
        assert!(!record.covers(&b), "same content_hash at a different path must NOT be covered");
    }

    #[test]
    fn test_dry_check_coverage_record_is_cloneable_and_eq() {
        let a = make_fragment_ref("src/a.rs", 'a');
        let mut refs = BTreeSet::new();
        refs.insert(a);
        let record = DryCheckCoverageRecord::new(refs);
        let clone = record.clone();
        assert_eq!(record, clone);
    }
}
