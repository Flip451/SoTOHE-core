//! Signal counts for spec requirement confidence evaluation.
//!
//! Each requirement in `spec.md` can carry a traffic-light signal:
//! - 🔵 Blue: confirmed with explicit evidence
//! - 🟡 Yellow: inferred or partially verified
//! - 🔴 Red: unverified or contradicted
//!
//! `SignalCounts` aggregates these per-spec for summary display in frontmatter.

/// Aggregate signal counts for a spec document.
///
/// All counts are non-negative by construction (`u32`).
///
/// # Examples
///
/// ```
/// use domain::SignalCounts;
///
/// let signals = SignalCounts::new(12, 1, 0);
/// assert_eq!(signals.blue(), 12);
/// assert_eq!(signals.total(), 13);
/// assert!(!signals.has_red());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalCounts {
    blue: u32,
    yellow: u32,
    red: u32,
}

impl SignalCounts {
    /// Creates a new `SignalCounts`.
    #[must_use]
    pub const fn new(blue: u32, yellow: u32, red: u32) -> Self {
        Self { blue, yellow, red }
    }

    /// Returns the blue (confirmed) count.
    #[must_use]
    pub const fn blue(&self) -> u32 {
        self.blue
    }

    /// Returns the yellow (inferred) count.
    #[must_use]
    pub const fn yellow(&self) -> u32 {
        self.yellow
    }

    /// Returns the red (unverified) count.
    #[must_use]
    pub const fn red(&self) -> u32 {
        self.red
    }

    /// Returns the total number of signals.
    #[must_use]
    pub const fn total(&self) -> u32 {
        self.blue + self.yellow + self.red
    }

    /// Returns `true` if any red signals exist.
    #[must_use]
    pub const fn has_red(&self) -> bool {
        self.red > 0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_counts_new_and_accessors() {
        let s = SignalCounts::new(12, 1, 0);
        assert_eq!(s.blue(), 12);
        assert_eq!(s.yellow(), 1);
        assert_eq!(s.red(), 0);
        assert_eq!(s.total(), 13);
        assert!(!s.has_red());
    }

    #[test]
    fn test_signal_counts_has_red() {
        let s = SignalCounts::new(0, 0, 1);
        assert!(s.has_red());
    }

    #[test]
    fn test_signal_counts_zero() {
        let s = SignalCounts::new(0, 0, 0);
        assert_eq!(s.total(), 0);
        assert!(!s.has_red());
    }
}
