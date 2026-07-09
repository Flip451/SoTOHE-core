//! Uncited-element value object for the obligation gate.
//!
//! [`UncitedSpecElementFinding`] names a spec element that no obligation cites,
//! feeding the uncited-element report (IN-16 / AC-13 / CN-07).

use crate::SpecElementId;
use crate::tddd::semantic_verify::SpecSectionKind;

/// A spec element that no derived obligation cites.
///
/// Recorded for the uncited-element report so a spec element left without test
/// coverage is surfaced rather than silently accepted (IN-16 / AC-13 / CN-07).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UncitedSpecElementFinding {
    // Read by the results interactor (T019) when it renders the uncited-element
    // report; no read accessor is declared in this batch's type contract.
    #[allow(dead_code)]
    element_id: SpecElementId,
    #[allow(dead_code)]
    section: SpecSectionKind,
}

impl UncitedSpecElementFinding {
    /// Builds an [`UncitedSpecElementFinding`] for `element_id` in `section`.
    #[must_use]
    pub fn new(element_id: SpecElementId, section: SpecSectionKind) -> Self {
        Self { element_id, section }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_uncited_finding_equality_by_element_and_section() {
        let a = UncitedSpecElementFinding::new(
            SpecElementId::try_new("IN-16").unwrap(),
            SpecSectionKind::InScope,
        );
        let b = UncitedSpecElementFinding::new(
            SpecElementId::try_new("IN-16").unwrap(),
            SpecSectionKind::InScope,
        );
        let c = UncitedSpecElementFinding::new(
            SpecElementId::try_new("AC-13").unwrap(),
            SpecSectionKind::AcceptanceCriteria,
        );
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
