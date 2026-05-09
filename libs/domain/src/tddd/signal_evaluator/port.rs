//! `SignalEvaluatorPort` — secondary port for the 3-way signal evaluator.
//!
//! ## Contract (ADR 3 D5)
//!
//! The port takes three TypeGraph inputs:
//! - `a`: `ExtendedCrate` — TypeGraph A (Catalogue-derived, action-annotated).
//! - `b`: `rustdoc_types::Crate` — TypeGraph B (Baseline, pure rustdoc output).
//! - `c`: `rustdoc_types::Crate` — TypeGraph C (Current, pure rustdoc output).
//!
//! And produces either a `ThreeWayEvaluationReport` (success) or a `Phase1Error`
//! (catalogue declare inconsistency detected during S / D construction).
//!
//! ## Why Phase 1 error is the only error kind
//!
//! Phase 2 (3-way evaluation of S / D / C) is a total function — every item in
//! S, D, and C is classified into exactly one of the 12 `SignalRegion` variants.
//! No errors can arise in Phase 2 itself; all error conditions (action
//! contradictions, unresolved type references, dangling ids) are fully
//! exhausted in Phase 1.  Therefore the port's `Result` error variant is
//! `Phase1Error` only.
//!
//! ## Implementation note
//!
//! The algorithm (Phase 1 S / D construction + Phase 2 evaluation) lives in the
//! infrastructure layer (`SignalEvaluatorV2`, T007).  The domain layer only
//! declares this port trait, keeping the domain free of `rustdoc` parsing and
//! I/O dependencies.
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free.

use rustdoc_types::Crate;

use crate::tddd::extended_crate::ExtendedCrate;
use crate::tddd::signal_evaluator::phase1_error::Phase1Error;
use crate::tddd::signal_evaluator::region::ThreeWayEvaluationReport;

/// Secondary port for the 3-way signal evaluator (Phase 1 + Phase 2).
///
/// Implementors live in the infrastructure layer (see `SignalEvaluatorV2`).
/// The domain layer declares only this trait; it does not know about S / D
/// construction details, `rustdoc_types` parsing, or external crate resolution.
///
/// ## Input contract
///
/// - `a` — TypeGraph A: `ExtendedCrate` built by `CatalogueToExtendedCratePort`.
///   Contains all declared types/traits/functions with their `ItemAction` annotations.
/// - `b` — TypeGraph B: baseline `rustdoc_types::Crate` (pure rustdoc output,
///   no action annotations).  All B items are treated as implicitly `Reference`
///   during Phase 1 S construction (ADR 3 D4).
/// - `c` — TypeGraph C: current `rustdoc_types::Crate` (pure rustdoc output).
///   Used in Phase 2 3-way evaluation only — not consumed during Phase 1.
///
/// ## Output contract
///
/// Returns a `ThreeWayEvaluationReport` containing all non-skip signals for
/// evaluated items (Blue / Yellow / Red).  Skip signals (`SIntersectC_Match_Reference`)
/// are omitted from the report to reduce noise (ADR 3 D3).
///
/// Returns `Err(Phase1Error)` if Phase 1 S / D construction detects an
/// inconsistency in the catalogue declarations.
///
/// # Errors
///
/// Returns `Err(Phase1Error::ActionContradiction)` when a catalogue action is
/// inconsistent with the baseline (e.g., `Add` for a type already in B).
///
/// Returns `Err(Phase1Error::UnresolvedTypeRef)` when a TypeRef unresolved
/// marker from the A codec cannot be resolved against the closed-world universe
/// (Delete-processed S).
///
/// Returns `Err(Phase1Error::DanglingId)` when an `Id` inside S refers to a
/// deleted item after unresolved-marker resolution.
pub trait SignalEvaluatorPort: Send + Sync {
    /// Runs Phase 1 (S / D construction) and Phase 2 (3-way evaluation).
    ///
    /// # Errors
    ///
    /// See trait-level documentation for error conditions.
    fn evaluate(
        &self,
        a: ExtendedCrate,
        b: Crate,
        c: Crate,
    ) -> Result<ThreeWayEvaluationReport, Phase1Error>;
}

// ---------------------------------------------------------------------------
// Tests — port trait shape (AC-07: method signature + implementability)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::tddd::signal_evaluator::region::{SignalRegion, ThreeWaySignal};
    use rustdoc_types::{Crate, FORMAT_VERSION};
    use std::collections::{BTreeMap, HashMap};

    // Minimal `rustdoc_types::Crate` for testing (no items).
    fn empty_crate() -> Crate {
        Crate {
            root: rustdoc_types::Id(0),
            crate_version: None,
            includes_private: false,
            index: HashMap::new(),
            paths: HashMap::new(),
            external_crates: HashMap::new(),
            format_version: FORMAT_VERSION,
            target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
        }
    }

    // A no-op implementation that always returns an empty report.
    struct AlwaysEmptyEvaluator;

    impl SignalEvaluatorPort for AlwaysEmptyEvaluator {
        fn evaluate(
            &self,
            _a: ExtendedCrate,
            _b: Crate,
            _c: Crate,
        ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
            Ok(ThreeWayEvaluationReport::new(vec![]))
        }
    }

    // An implementation that always returns ActionContradiction.
    struct AlwaysContradictionEvaluator;

    impl SignalEvaluatorPort for AlwaysContradictionEvaluator {
        fn evaluate(
            &self,
            _a: ExtendedCrate,
            _b: Crate,
            _c: Crate,
        ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
            Err(Phase1Error::ActionContradiction("stub contradiction".into()))
        }
    }

    // An implementation that returns a report with one signal.
    struct SingleSignalEvaluator {
        item: String,
        region: SignalRegion,
    }

    impl SignalEvaluatorPort for SingleSignalEvaluator {
        fn evaluate(
            &self,
            _a: ExtendedCrate,
            _b: Crate,
            _c: Crate,
        ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
            let signal = ThreeWaySignal::new(self.item.clone(), self.region);
            Ok(ThreeWayEvaluationReport::new(vec![signal]))
        }
    }

    #[test]
    fn test_signal_evaluator_port_always_empty_returns_ok() {
        let evaluator = AlwaysEmptyEvaluator;
        let a = ExtendedCrate::new(empty_crate(), BTreeMap::new());
        let b = empty_crate();
        let c = empty_crate();
        let result = evaluator.evaluate(a, b, c);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_signal_evaluator_port_contradiction_returns_phase1_error() {
        let evaluator = AlwaysContradictionEvaluator;
        let a = ExtendedCrate::new(empty_crate(), BTreeMap::new());
        let b = empty_crate();
        let c = empty_crate();
        let result = evaluator.evaluate(a, b, c);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Phase1Error::ActionContradiction(_)));
    }

    #[test]
    fn test_signal_evaluator_port_single_signal_blue() {
        let evaluator = SingleSignalEvaluator {
            item: "User".to_string(),
            region: SignalRegion::SIntersectC_Match_Add,
        };
        let a = ExtendedCrate::new(empty_crate(), BTreeMap::new());
        let b = empty_crate();
        let c = empty_crate();
        let report = evaluator.evaluate(a, b, c).unwrap();
        assert_eq!(report.len(), 1);
        let signals: Vec<_> = report.iter().collect();
        assert_eq!(signals[0].item_name(), "User");
        assert!(signals[0].signal().is_blue());
    }

    #[test]
    fn test_signal_evaluator_port_single_signal_red() {
        let evaluator = SingleSignalEvaluator {
            item: "Ghost".to_string(),
            region: SignalRegion::CMinusSUnionD,
        };
        let a = ExtendedCrate::new(empty_crate(), BTreeMap::new());
        let b = empty_crate();
        let c = empty_crate();
        let report = evaluator.evaluate(a, b, c).unwrap();
        assert!(report.has_violations());
    }

    #[test]
    fn test_signal_evaluator_port_is_send_sync() {
        // Verify that the port can be used as a trait object behind Arc.
        fn assert_send_sync<T: Send + Sync + ?Sized>() {}
        assert_send_sync::<dyn SignalEvaluatorPort>();
    }

    #[test]
    fn test_signal_evaluator_port_unresolved_type_ref_error_kind() {
        struct UnresolvedEvaluator;
        impl SignalEvaluatorPort for UnresolvedEvaluator {
            fn evaluate(
                &self,
                _a: ExtendedCrate,
                _b: Crate,
                _c: Crate,
            ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
                Err(Phase1Error::UnresolvedTypeRef("MissingType".into()))
            }
        }
        let a = ExtendedCrate::new(empty_crate(), BTreeMap::new());
        let err = UnresolvedEvaluator.evaluate(a, empty_crate(), empty_crate()).unwrap_err();
        assert!(matches!(err, Phase1Error::UnresolvedTypeRef(_)));
    }

    #[test]
    fn test_signal_evaluator_port_dangling_id_error_kind() {
        struct DanglingEvaluator;
        impl SignalEvaluatorPort for DanglingEvaluator {
            fn evaluate(
                &self,
                _a: ExtendedCrate,
                _b: Crate,
                _c: Crate,
            ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
                Err(Phase1Error::DanglingId("Order.user_id -> deleted UserId".into()))
            }
        }
        let a = ExtendedCrate::new(empty_crate(), BTreeMap::new());
        let err = DanglingEvaluator.evaluate(a, empty_crate(), empty_crate()).unwrap_err();
        assert!(matches!(err, Phase1Error::DanglingId(_)));
    }
}
