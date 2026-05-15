//! `Phase1Error` — errors produced during Signal evaluator Phase 1 (S/D construction).
//!
//! ## Variants (ADR 3 D2)
//!
//! * `ActionContradiction` — a catalogue action is inconsistent with the baseline:
//!   - `Add` declared for a type that already exists in B.
//!   - `Modify`, `Reference`, or `Delete` declared for a type absent from B.
//!
//! * `UnresolvedTypeRef` — a catalogue `TypeRef` (unresolved marker, ADR 2 D9)
//!   cannot be resolved against the closed-world universe (Delete-processed S).
//!   This catches typos, name mismatches, and references to deleted types.
//!
//! * `DanglingId` — after unresolved marker resolution, an `Id` inside S still
//!   refers to a deleted item (e.g., a field references a type whose catalogue
//!   entry was `Delete`-processed and is no longer in S).
//!
//! ## Error reporting intent
//!
//! All three variants are early-rejection errors — they reject a catalogue
//! declare before Phase 2 (3-way evaluation) is reached.  Callers should
//! surface these with sufficient context (item name, offending type ref, etc.)
//! for the developer to diagnose the catalogue mistake quickly.
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free.

use thiserror::Error;

/// Error returned by [`crate::tddd::signal_evaluator::SignalEvaluatorPort::evaluate`]
/// when Phase 1 (S / D construction) detects a declare inconsistency.
///
/// All three variants represent early-rejection conditions that prevent Phase 2
/// (3-way evaluation) from proceeding.  The catalogue must be corrected before
/// the evaluator can produce a `ThreeWayEvaluationReport`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Phase1Error {
    /// A catalogue `action` declaration contradicts the baseline state.
    ///
    /// Examples:
    /// - `action: add` but the type already exists in B (would be a duplicate).
    /// - `action: modify / reference / delete` but the type is absent from B.
    ///
    /// Contains a human-readable description identifying the item and the
    /// contradicting action.
    #[error("action contradiction: {0}")]
    ActionContradiction(String),

    /// An unresolved `TypeRef` marker (from the A codec open-world pass, ADR 2 D9)
    /// cannot be resolved against the closed-world universe (Delete-processed S).
    ///
    /// Caused by typos, name mismatches, or references to types that have been
    /// `Delete`-processed out of S.  The `String` payload carries the unresolvable
    /// type name.
    ///
    /// Phase 1.5 (ADR 3 D2): after all Delete operations are applied, S serves as
    /// the closed-world universe; a marker not found in `S.index` is rejected here.
    #[error("unresolved type reference: {0}")]
    UnresolvedTypeRef(String),

    /// After unresolved-marker resolution (Phase 1.5), an `Id` inside S still
    /// refers to a deleted item (dangling reference, ADR 3 D2 Phase 1.6).
    ///
    /// This indicates that a field or variant of a surviving type references a
    /// type that was `Delete`-processed.  The catalogue must declare that
    /// dependency removed (e.g., via a `Modify` action on the referencing type).
    ///
    /// The `String` payload carries a human-readable description of the dangling
    /// reference (item name + dangling id info).
    #[error("dangling id reference: {0}")]
    DanglingId(String),
}

// ---------------------------------------------------------------------------
// Tests — structural + display coverage (AC-07)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_phase1_error_action_contradiction_display() {
        let err =
            Phase1Error::ActionContradiction("User: add declared but already in baseline".into());
        let s = err.to_string();
        assert!(s.contains("action contradiction"));
        assert!(s.contains("User"));
    }

    #[test]
    fn test_phase1_error_unresolved_type_ref_display() {
        let err = Phase1Error::UnresolvedTypeRef("NonExistentType".into());
        let s = err.to_string();
        assert!(s.contains("unresolved type reference"));
        assert!(s.contains("NonExistentType"));
    }

    #[test]
    fn test_phase1_error_dangling_id_display() {
        let err =
            Phase1Error::DanglingId("Order: field order_id refers to deleted type UserId".into());
        let s = err.to_string();
        assert!(s.contains("dangling id reference"));
        assert!(s.contains("Order"));
    }

    #[test]
    fn test_phase1_error_variants_are_clone_and_eq() {
        let a = Phase1Error::ActionContradiction("x".into());
        let b = a.clone();
        assert_eq!(a, b);

        let c = Phase1Error::UnresolvedTypeRef("y".into());
        assert_ne!(a, c);
    }

    #[test]
    fn test_phase1_error_action_contradiction_with_empty_payload() {
        // Payload is allowed to be empty (minimal case; real callers will populate it).
        let err = Phase1Error::ActionContradiction(String::new());
        assert!(err.to_string().starts_with("action contradiction"));
    }

    #[test]
    fn test_phase1_error_unresolved_type_ref_with_empty_payload() {
        let err = Phase1Error::UnresolvedTypeRef(String::new());
        assert!(err.to_string().starts_with("unresolved type reference"));
    }

    #[test]
    fn test_phase1_error_dangling_id_with_empty_payload() {
        let err = Phase1Error::DanglingId(String::new());
        assert!(err.to_string().starts_with("dangling id reference"));
    }
}
