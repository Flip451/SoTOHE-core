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
//! The first three variants are early-rejection errors — they reject a catalogue
//! declare before Phase 2 (3-way evaluation) is reached. `RustdocRootResolution`
//! instead reports that the evaluator could not translate a package root to the
//! rustdoc root required for function-path comparison. Callers should surface
//! these with sufficient context to diagnose either the catalogue mistake or
//! the metadata-resolution failure.
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free.

use thiserror::Error;

use crate::tddd::test_obligation::ids::{DiagnosticMessage, unavailable_diagnostic_message};

/// Error returned by [`crate::tddd::signal_evaluator::SignalEvaluatorPort::evaluate`]
/// when Phase 1 (S / D construction) detects a declare inconsistency or when
/// pre-evaluation rustdoc-root resolution fails.
///
/// The first three variants represent early-rejection conditions that prevent
/// Phase 2 (3-way evaluation) from proceeding and require a catalogue
/// correction. `RustdocRootResolution` instead represents a metadata/root
/// resolution failure that prevents the evaluator from preparing its
/// function-path comparison.
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
    #[error("action contradiction: {}", .0.as_str())]
    ActionContradiction(DiagnosticMessage),

    /// An unresolved `TypeRef` marker (from the A codec open-world pass, ADR 2 D9)
    /// cannot be resolved against the closed-world universe (Delete-processed S).
    ///
    /// Caused by typos, name mismatches, or references to types that have been
    /// `Delete`-processed out of S. The [`DiagnosticMessage`] payload carries
    /// the unresolvable type name.
    ///
    /// Phase 1.5 (ADR 3 D2): after all Delete operations are applied, S serves as
    /// the closed-world universe; a marker not found in `S.index` is rejected here.
    #[error("unresolved type reference: {}", .0.as_str())]
    UnresolvedTypeRef(DiagnosticMessage),

    /// After unresolved-marker resolution (Phase 1.5), an `Id` inside S still
    /// refers to a deleted item (dangling reference, ADR 3 D2 Phase 1.6).
    ///
    /// This indicates that a field or variant of a surviving type references a
    /// type that was `Delete`-processed.  The catalogue must declare that
    /// dependency removed (e.g., via a `Modify` action on the referencing type).
    ///
    /// The [`DiagnosticMessage`] payload carries a human-readable description
    /// of the dangling reference (item name + dangling id info).
    #[error("dangling id reference: {}", .0.as_str())]
    DanglingId(DiagnosticMessage),

    /// Cargo metadata could not resolve a package-to-rustdoc-root translation
    /// needed to compare bin-target function paths with catalogue paths.
    #[error("rustdoc root resolution failed: {}", .0.as_str())]
    RustdocRootResolution(DiagnosticMessage),
}

impl Phase1Error {
    /// Builds an action-contradiction error from human-readable detail.
    #[must_use]
    pub fn action_contradiction(detail: impl Into<String>) -> Self {
        Self::ActionContradiction(diagnostic_message(detail))
    }

    /// Builds an unresolved-type-reference error from human-readable detail.
    #[must_use]
    pub fn unresolved_type_ref(detail: impl Into<String>) -> Self {
        Self::UnresolvedTypeRef(diagnostic_message(detail))
    }

    /// Builds a dangling-id error from human-readable detail.
    #[must_use]
    pub fn dangling_id(detail: impl Into<String>) -> Self {
        Self::DanglingId(diagnostic_message(detail))
    }

    /// Builds a rustdoc-root-resolution error from human-readable detail.
    #[must_use]
    pub fn rustdoc_root_resolution(detail: impl Into<String>) -> Self {
        Self::RustdocRootResolution(diagnostic_message(detail))
    }
}

fn diagnostic_message(detail: impl Into<String>) -> DiagnosticMessage {
    match DiagnosticMessage::try_new(detail.into()) {
        Ok(message) => message,
        Err(_) => unavailable_diagnostic_message(),
    }
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
        let err = Phase1Error::action_contradiction("User: add declared but already in baseline");
        let s = err.to_string();
        assert!(s.contains("action contradiction"));
        assert!(s.contains("User"));
    }

    #[test]
    fn test_phase1_error_unresolved_type_ref_display() {
        let err = Phase1Error::unresolved_type_ref("NonExistentType");
        let s = err.to_string();
        assert!(s.contains("unresolved type reference"));
        assert!(s.contains("NonExistentType"));
    }

    #[test]
    fn test_phase1_error_dangling_id_display() {
        let err = Phase1Error::dangling_id("Order: field order_id refers to deleted type UserId");
        let s = err.to_string();
        assert!(s.contains("dangling id reference"));
        assert!(s.contains("Order"));
    }

    #[test]
    fn test_phase1_error_variants_are_clone_and_eq() {
        let a = Phase1Error::action_contradiction("x");
        let b = a.clone();
        assert_eq!(a, b);

        let c = Phase1Error::unresolved_type_ref("y");
        assert_ne!(a, c);
    }

    #[test]
    fn test_phase1_error_action_contradiction_empty_detail_uses_fallback() {
        let err = Phase1Error::action_contradiction(String::new());
        assert!(err.to_string().contains("diagnostic detail unavailable"));
    }

    #[test]
    fn test_phase1_error_unresolved_type_ref_empty_detail_uses_fallback() {
        let err = Phase1Error::unresolved_type_ref(String::new());
        assert!(err.to_string().contains("diagnostic detail unavailable"));
    }

    #[test]
    fn test_phase1_error_dangling_id_empty_detail_uses_fallback() {
        let err = Phase1Error::dangling_id(String::new());
        assert!(err.to_string().contains("diagnostic detail unavailable"));
    }

    #[test]
    fn test_phase1_error_rustdoc_root_resolution_display() {
        let err = Phase1Error::rustdoc_root_resolution("cargo metadata failed");
        assert!(err.to_string().contains("rustdoc root resolution failed"));
    }
}
