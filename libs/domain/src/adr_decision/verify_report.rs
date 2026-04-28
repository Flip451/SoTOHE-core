//! Aggregate signal counts produced by `verify-adr-signals`.

/// Aggregate result of evaluating every decision across every ADR file in a
/// single `verify-adr-signals` run.
///
/// Four-band counts: 🔵 user-approved, 🟡 review-process derived, 🔴 no
/// traced grounds, and a separate `grandfathered_count` for decisions
/// excluded from signal evaluation per ADR convention §grandfathered (D4).
/// Grandfathered decisions are deliberately **not** lumped into 🔵 so that
/// legacy back-fill debt remains observable.
///
/// `red_count >= 1` drives a non-zero CLI exit per AC-01 — the caller
/// decides the exit code by inspecting [`AdrVerifyReport::red_count`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdrVerifyReport {
    blue_count: usize,
    yellow_count: usize,
    red_count: usize,
    grandfathered_count: usize,
}

impl AdrVerifyReport {
    /// Construct a report with the given aggregate counts.
    #[must_use]
    pub fn new(
        blue_count: usize,
        yellow_count: usize,
        red_count: usize,
        grandfathered_count: usize,
    ) -> Self {
        Self { blue_count, yellow_count, red_count, grandfathered_count }
    }

    /// The empty report (zero counts in every band).
    #[must_use]
    pub fn empty() -> Self {
        Self { blue_count: 0, yellow_count: 0, red_count: 0, grandfathered_count: 0 }
    }

    /// Decisions evaluated as 🔵 (user-approved).
    #[must_use]
    pub fn blue_count(&self) -> usize {
        self.blue_count
    }

    /// Decisions evaluated as 🟡 (review-process derived).
    #[must_use]
    pub fn yellow_count(&self) -> usize {
        self.yellow_count
    }

    /// Decisions evaluated as 🔴 (no traced grounds).
    ///
    /// `>= 1` triggers a non-zero CLI exit per AC-01.
    #[must_use]
    pub fn red_count(&self) -> usize {
        self.red_count
    }

    /// Decisions exempted from signal evaluation via `grandfathered: true`
    /// (ADR convention §grandfathered, D4). Counted separately from 🔵 so
    /// legacy back-fill debt remains visible to operators.
    #[must_use]
    pub fn grandfathered_count(&self) -> usize {
        self.grandfathered_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adr_verify_report_new_records_counts() {
        let r = AdrVerifyReport::new(3, 2, 1, 4);
        assert_eq!(r.blue_count(), 3);
        assert_eq!(r.yellow_count(), 2);
        assert_eq!(r.red_count(), 1);
        assert_eq!(r.grandfathered_count(), 4);
    }

    #[test]
    fn test_adr_verify_report_empty_has_zero_counts() {
        let r = AdrVerifyReport::empty();
        assert_eq!(r.blue_count(), 0);
        assert_eq!(r.yellow_count(), 0);
        assert_eq!(r.red_count(), 0);
        assert_eq!(r.grandfathered_count(), 0);
    }

    #[test]
    fn test_adr_verify_report_red_count_drives_block_decision() {
        let blocked = AdrVerifyReport::new(0, 0, 1, 0);
        assert!(blocked.red_count() >= 1);

        let clean = AdrVerifyReport::new(5, 3, 0, 7);
        assert!(clean.red_count() == 0);
    }
}
