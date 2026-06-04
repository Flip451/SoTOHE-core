//! Finding type: `DryCheckFinding`.

use thiserror::Error;

use super::fragment::FragmentRef;
use super::value_objects::RefactorProposal;

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
