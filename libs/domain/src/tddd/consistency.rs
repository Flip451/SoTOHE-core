//! Bidirectional consistency checking and the Stage 2 signal gate.
//!
//! This module bridges the forward-only `evaluate_type_signals` (from
//! `super::signals`) with baseline-aware 4-group evaluation, and exposes the
//! merge-time signal gate `check_type_signals` used by both the CI path
//! (`verify_from_spec_json`) and the merge gate (`usecase::merge_gate`).
//!
//! Historical note (T001): the consistency report and check functions used to
//! live in `catalogue.rs`. They were extracted here during the TDDD-01 rename +
//! DM-06 split.

use std::collections::HashSet;

use crate::ConfidenceSignal;
use crate::TypeBaseline;
use crate::schema::TypeGraph;
use crate::tddd::catalogue::{
    TypeAction, TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind, TypeSignal,
};
use crate::tddd::signals::{evaluate_type_signals, red};
use crate::verify::{VerifyFinding, VerifyOutcome};

// ---------------------------------------------------------------------------
// ActionContradiction — action vs baseline mismatch warnings
// ---------------------------------------------------------------------------

/// Describes a contradiction between an entry's declared `action` and the baseline state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionContradiction {
    name: String,
    action: TypeAction,
    kind: ActionContradictionKind,
}

impl ActionContradiction {
    /// Creates a new `ActionContradiction`.
    #[must_use]
    pub fn new(name: impl Into<String>, action: TypeAction, kind: ActionContradictionKind) -> Self {
        Self { name: name.into(), action, kind }
    }

    /// Returns the type name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the declared action.
    #[must_use]
    pub fn action(&self) -> TypeAction {
        self.action
    }

    /// Returns the kind of contradiction.
    #[must_use]
    pub fn kind(&self) -> &ActionContradictionKind {
        &self.kind
    }
}

/// Classifies the nature of an action-baseline contradiction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionContradictionKind {
    /// `action: "add"` but type already exists in baseline.
    AddButAlreadyInBaseline,
    /// `action: "modify"` but type not found in baseline.
    ModifyButNotInBaseline,
    /// `action: "reference"` but type not found in baseline.
    ReferenceButNotInBaseline,
    /// `action: "reference"` but forward check signal is not Blue (implementation differs).
    ReferenceButNotBlue,
}

// ---------------------------------------------------------------------------
// ConsistencyReport — bidirectional spec ↔ code check
// ---------------------------------------------------------------------------

/// Result of a bidirectional consistency check between the type catalogue
/// (spec) and the crate's public API (code), with baseline-aware filtering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsistencyReport {
    /// Forward signals: spec → code evaluation results (groups 1 + 2).
    forward_signals: Vec<TypeSignal>,
    /// Types found in code but not in declarations or baseline (group 4).
    undeclared_types: Vec<String>,
    /// Traits found in code but not in declarations or baseline (group 4).
    undeclared_traits: Vec<String>,
    /// Count of baseline types/traits skipped because structure is unchanged (group 3).
    skipped_count: usize,
    /// Red signals from baseline comparison: structural changes or deletions (group 3).
    baseline_red_types: Vec<String>,
    /// Red signals from baseline comparison for traits (group 3).
    baseline_red_traits: Vec<String>,
    /// Advisory warnings for action-baseline contradictions.
    contradictions: Vec<ActionContradiction>,
    /// Hard errors: `delete` action declared for types not in baseline.
    delete_errors: Vec<String>,
}

impl ConsistencyReport {
    /// Returns the forward (spec → code) signals.
    #[must_use]
    pub fn forward_signals(&self) -> &[TypeSignal] {
        &self.forward_signals
    }

    /// Returns type names found in code but not in declarations or baseline (group 4).
    #[must_use]
    pub fn undeclared_types(&self) -> &[String] {
        &self.undeclared_types
    }

    /// Returns trait names found in code but not in declarations or baseline (group 4).
    #[must_use]
    pub fn undeclared_traits(&self) -> &[String] {
        &self.undeclared_traits
    }

    /// Returns the count of baseline entries skipped (structure unchanged, group 3).
    #[must_use]
    pub fn skipped_count(&self) -> usize {
        self.skipped_count
    }

    /// Returns type names from baseline with structural changes or deletions (group 3 Red).
    #[must_use]
    pub fn baseline_red_types(&self) -> &[String] {
        &self.baseline_red_types
    }

    /// Returns trait names from baseline with structural changes or deletions (group 3 Red).
    #[must_use]
    pub fn baseline_red_traits(&self) -> &[String] {
        &self.baseline_red_traits
    }

    /// Returns advisory warnings for action-baseline contradictions.
    #[must_use]
    pub fn contradictions(&self) -> &[ActionContradiction] {
        &self.contradictions
    }

    /// Returns hard errors for `delete` action on types not in baseline.
    #[must_use]
    pub fn delete_errors(&self) -> &[String] {
        &self.delete_errors
    }
}

/// Performs a baseline-aware bidirectional consistency check.
///
/// Uses the 4-group evaluation from ADR TDDD-02 §3:
/// - **Group 1 (A\B)**: declared, not in baseline → forward check
/// - **Group 2 (A∩B)**: declared and in baseline → forward check
/// - **Group 3 (B\A)**: baseline, not declared → skip if unchanged, Red if changed/deleted
/// - **Group 4 (∁(A∪B)∩C)**: not declared, not in baseline, in code → Red
///
/// Groups 1+2 are handled by `evaluate_type_signals` (forward check).
/// Groups 3+4 replace the old undeclared-types reverse check.
#[must_use]
pub fn check_consistency(
    entries: &[TypeCatalogueEntry],
    graph: &TypeGraph,
    baseline: &TypeBaseline,
) -> ConsistencyReport {
    // Forward check (groups 1 + 2): evaluate declared entries against code.
    let mut forward_signals = evaluate_type_signals(entries, graph);

    // Kind-specific declared sets: types and traits are partitioned separately
    // so that cross-kind undeclared code is detected by reverse check.
    // Kind migration (e.g., struct -> trait) is handled via delete+add pairs:
    // declare the old kind with action:"delete" and the new kind with action:"add".
    let declared_type_names: HashSet<&str> = entries
        .iter()
        .filter(|e| {
            !matches!(
                e.kind(),
                TypeDefinitionKind::SecondaryPort { .. }
                    | TypeDefinitionKind::ApplicationService { .. }
            )
        })
        .map(|e| e.name())
        .collect();

    let declared_trait_names: HashSet<&str> = entries
        .iter()
        .filter(|e| {
            matches!(
                e.kind(),
                TypeDefinitionKind::SecondaryPort { .. }
                    | TypeDefinitionKind::ApplicationService { .. }
            )
        })
        .map(|e| e.name())
        .collect();

    let mut skipped_count: usize = 0;
    let mut baseline_red_types: Vec<String> = Vec::new();
    let mut baseline_red_traits: Vec<String> = Vec::new();

    // Group 3 — types: B\A (in baseline types, not declared as a type)
    for (name, baseline_entry) in baseline.types() {
        if declared_type_names.contains(name.as_str()) {
            continue; // Group 2: declared → handled by forward check
        }
        match graph.get_type(name) {
            Some(code_node) => {
                // Compare using the full structured shape
                // (`Vec<MemberDeclaration>` and `Vec<MethodDeclaration>`).
                let current = crate::TypeBaselineEntry::new(
                    code_node.kind().clone(),
                    code_node.members().to_vec(),
                    code_node.methods().to_vec(),
                );
                if baseline_entry.structurally_equal(&current) {
                    skipped_count += 1; // Unchanged → skip
                } else {
                    baseline_red_types.push(name.clone()); // Structural change → Red
                }
            }
            None => {
                baseline_red_types.push(name.clone()); // Deleted → Red
            }
        }
    }

    // Group 3 — traits: B\A (in baseline traits, not declared as a trait)
    for (name, baseline_entry) in baseline.traits() {
        if declared_trait_names.contains(name.as_str()) {
            continue; // Group 2: declared → handled by forward check
        }
        match graph.get_trait(name) {
            Some(code_node) => {
                let current = crate::TraitBaselineEntry::new(code_node.methods().to_vec());
                if baseline_entry.structurally_equal(&current) {
                    skipped_count += 1;
                } else {
                    baseline_red_traits.push(name.clone());
                }
            }
            None => {
                baseline_red_traits.push(name.clone());
            }
        }
    }

    baseline_red_types.sort();
    baseline_red_traits.sort();

    // Group 4 — ∁(A∪B)∩C: in code, not declared (same kind), not in baseline → Red
    let mut undeclared_types: Vec<String> = graph
        .type_names()
        .filter(|name| !declared_type_names.contains(name.as_str()) && !baseline.has_type(name))
        .cloned()
        .collect();
    undeclared_types.sort();

    let mut undeclared_traits: Vec<String> = graph
        .trait_names()
        .filter(|name| !declared_trait_names.contains(name.as_str()) && !baseline.has_trait(name))
        .cloned()
        .collect();
    undeclared_traits.sort();

    // Action-baseline contradiction detection + delete validation.
    let mut contradictions = Vec::new();
    let mut delete_errors = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        let name = entry.name();
        let is_trait = matches!(
            entry.kind(),
            TypeDefinitionKind::SecondaryPort { .. }
                | TypeDefinitionKind::ApplicationService { .. }
        );
        let in_baseline = if is_trait { baseline.has_trait(name) } else { baseline.has_type(name) };

        match entry.action() {
            TypeAction::Add => {
                if in_baseline {
                    contradictions.push(ActionContradiction::new(
                        name,
                        TypeAction::Add,
                        ActionContradictionKind::AddButAlreadyInBaseline,
                    ));
                }
            }
            TypeAction::Modify => {
                if !in_baseline {
                    contradictions.push(ActionContradiction::new(
                        name,
                        TypeAction::Modify,
                        ActionContradictionKind::ModifyButNotInBaseline,
                    ));
                }
            }
            TypeAction::Reference => {
                if !in_baseline {
                    contradictions.push(ActionContradiction::new(
                        name,
                        TypeAction::Reference,
                        ActionContradictionKind::ReferenceButNotInBaseline,
                    ));
                } else if let Some(signal) = forward_signals.get(i) {
                    if signal.signal() != ConfidenceSignal::Blue {
                        contradictions.push(ActionContradiction::new(
                            name,
                            TypeAction::Reference,
                            ActionContradictionKind::ReferenceButNotBlue,
                        ));
                    }
                }
            }
            TypeAction::Delete => {
                if !in_baseline {
                    delete_errors.push(name.to_string());
                    // Patch the forward signal to Red so that existing consumers
                    // that only inspect `forward_signals` see this as an error.
                    // Without baseline evidence the delete declaration cannot be
                    // validated, so the entry must not silently resolve to Blue.
                    if let Some(sig) = forward_signals.get_mut(i) {
                        *sig = red(name, entry.kind().kind_tag(), false);
                    }
                }
            }
        }
    }

    ConsistencyReport {
        forward_signals,
        undeclared_types,
        undeclared_traits,
        skipped_count,
        baseline_red_types,
        baseline_red_traits,
        contradictions,
        delete_errors,
    }
}

// ---------------------------------------------------------------------------
// Stage 2 signal gate (check_type_signals)
// ---------------------------------------------------------------------------

/// Evaluates Stage 2 signal gate rules against a `TypeCatalogueDocument`.
///
/// Shared pure function used by both the CI path (`verify_from_spec_json`
/// Stage 2) and the merge gate (via `usecase::merge_gate`). The caller is
/// responsible for handling the `NotFound` case (no catalogue = TDDD not
/// active for the track/layer, per ADR §D2.1 opt-in model).
///
/// Layer-neutral naming (T001, formerly `check_domain_types_signals`).
///
/// # Rules
///
/// - `entries` is empty → `VerifyFinding::error` (malformed catalogue)
/// - `signals` is `None` → `VerifyFinding::error` (unevaluated; run `sotp track type-signals`)
/// - Signal coverage incomplete (entry has no matching signal) → `VerifyFinding::error`
/// - Any Red signal (forward or reverse) → `VerifyFinding::error` (always an error, regardless of mode)
/// - Declared-entry Yellow signal, `strict = true` → `VerifyFinding::error`
/// - Declared-entry Yellow signal, `strict = false` → `VerifyFinding::warning` (D8.6 visualization)
/// - Undeclared reverse signals (outside entry set) that are Yellow are not blocked
///   (only their Red counterparts are caught by the Red check above)
/// - All Blue / no declared Yellow → `VerifyOutcome::pass()`
///
/// The `strict` parameter is:
/// - `true` for the merge gate (all declared Yellow must be upgraded to Blue)
/// - `false` for CI interim mode (declared Yellow is allowed but visualized)
///
/// Reference: ADR `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` §D2, §D8.6.
#[must_use]
pub fn check_type_signals(
    doc: &TypeCatalogueDocument,
    strict: bool,
    catalogue_file: &str,
) -> VerifyOutcome {
    // ADR 2026-04-19-1242 §D6.4: empty catalogues (zero type declarations) are a
    // valid state for tracks that only reuse pre-existing types. However, if
    // `<layer>-type-signals.json` has already hydrated reverse-direction Red
    // findings (undeclared types detected by `check_consistency` /
    // `undeclared_to_signals`) into `doc.signals()`, those must still surface
    // so the merge gate does not suppress real drift violations.
    if doc.entries().is_empty() {
        let Some(signals) = doc.signals() else {
            return VerifyOutcome::pass();
        };
        let reds: Vec<&str> = signals
            .iter()
            .filter(|s| s.signal() == ConfidenceSignal::Red)
            .map(|s| s.type_name())
            .collect();
        if reds.is_empty() {
            return VerifyOutcome::pass();
        }
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: {} type(s) have Red signal (reverse-direction drift on empty catalogue — add the types to the catalogue or remove them from code): {}",
            reds.len(),
            reds.join(", ")
        ))]);
    }

    let Some(signals) = doc.signals() else {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: type signals not yet evaluated — run `sotp track type-signals` first",
        ))]);
    };

    // Signal coverage: every entry must have a matching (name, kind_tag) signal.
    let signal_keys: HashSet<(&str, &str)> =
        signals.iter().map(|s| (s.type_name(), s.kind_tag())).collect();
    let uncovered: Vec<&str> = doc
        .entries()
        .iter()
        .filter(|e| !signal_keys.contains(&(e.name(), e.kind().kind_tag())))
        .map(|e| e.name())
        .collect();
    if !uncovered.is_empty() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: {} type(s) have no signal evaluation: {} — re-run `sotp track type-signals`",
            uncovered.len(),
            uncovered.join(", ")
        ))]);
    }

    // Red check: applies to all signals (forward + reverse).
    let all_red: Vec<&str> = signals
        .iter()
        .filter(|s| s.signal() == ConfidenceSignal::Red)
        .map(|s| s.type_name())
        .collect();
    if !all_red.is_empty() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: {} type(s) have Red signal (TDDD violation — run /track:design): {}",
            all_red.len(),
            all_red.join(", ")
        ))]);
    }

    // Yellow check: declared entries only. Mode-dependent: error in strict, warning in interim.
    let entry_keys: HashSet<(&str, &str)> =
        doc.entries().iter().map(|e| (e.name(), e.kind().kind_tag())).collect();
    let yellow_entries: Vec<&str> = signals
        .iter()
        .filter(|s| entry_keys.contains(&(s.type_name(), s.kind_tag())))
        .filter(|s| s.signal() == ConfidenceSignal::Yellow)
        .map(|s| s.type_name())
        .collect();

    if !yellow_entries.is_empty() {
        let message = format!(
            "{catalogue_file}: {} declared type(s) have Yellow signal: {} — merge gate will block these until upgraded to Blue. Resolve each type (implement or remove per its declared action) and re-run `sotp track type-signals`.",
            yellow_entries.len(),
            yellow_entries.join(", ")
        );
        if strict {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(message)]);
        }
        return VerifyOutcome::from_findings(vec![VerifyFinding::warning(message)]);
    }

    VerifyOutcome::pass()
}

// ---------------------------------------------------------------------------
// Tests — consistency + Stage 2 signal gate
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;
    use crate::Timestamp;
    use crate::schema::{TraitNode, TypeGraph, TypeKind, TypeNode};
    use crate::tddd::baseline::{TraitBaselineEntry, TypeBaseline, TypeBaselineEntry};
    use crate::tddd::catalogue::{MemberDeclaration, MethodDeclaration};

    /// Helper: build a `MethodDeclaration` that takes no args and returns unit.
    fn unit_method(name: &str) -> MethodDeclaration {
        MethodDeclaration::new(name, Some("&self".into()), vec![], "()", false)
    }

    /// Helper: turn a slice of field/variant names into `Vec<MemberDeclaration>`
    /// by treating each as an enum variant (field/variant name is the only
    /// thing the tests below inspect).
    fn variants(names: &[&str]) -> Vec<MemberDeclaration> {
        names.iter().copied().map(MemberDeclaration::variant).collect()
    }

    fn empty_baseline() -> TypeBaseline {
        TypeBaseline::new(
            1,
            Timestamp::new("2026-04-11T00:00:00Z").unwrap(),
            HashMap::new(),
            HashMap::new(),
        )
    }

    fn baseline_with_types(entries: Vec<(&str, TypeBaselineEntry)>) -> TypeBaseline {
        let types = entries.into_iter().map(|(n, e)| (n.to_string(), e)).collect();
        TypeBaseline::new(1, Timestamp::new("2026-04-11T00:00:00Z").unwrap(), types, HashMap::new())
    }

    fn baseline_with_traits(entries: Vec<(&str, TraitBaselineEntry)>) -> TypeBaseline {
        let traits = entries.into_iter().map(|(n, e)| (n.to_string(), e)).collect();
        TypeBaseline::new(
            1,
            Timestamp::new("2026-04-11T00:00:00Z").unwrap(),
            HashMap::new(),
            traits,
        )
    }

    #[test]
    fn test_group4_undeclared_new_type_is_red() {
        // Type in code, not declared, not in baseline → group 4 Red
        let mut types = HashMap::new();
        types.insert(
            "NewType".to_string(),
            TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new()),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let report = check_consistency(&[], &graph, &empty_baseline());
        assert_eq!(report.undeclared_types(), &["NewType"]);
        assert_eq!(report.skipped_count(), 0);
    }

    #[test]
    fn test_group3_baseline_unchanged_type_is_skipped() {
        // Type in baseline and code, not declared, structure unchanged → skip
        let bl = baseline_with_types(vec![(
            "ExistingType",
            TypeBaselineEntry::new(
                TypeKind::Struct,
                vec![MemberDeclaration::field("field", "String")],
                vec![],
            ),
        )]);

        let mut types = HashMap::new();
        types.insert(
            "ExistingType".to_string(),
            TypeNode::new(
                TypeKind::Struct,
                vec![MemberDeclaration::field("field", "String")],
                vec![],
                HashSet::new(),
            ),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let report = check_consistency(&[], &graph, &bl);
        assert_eq!(report.skipped_count(), 1);
        assert!(report.undeclared_types().is_empty());
        assert!(report.baseline_red_types().is_empty());
    }

    #[test]
    fn test_group3_baseline_changed_type_is_red() {
        // Type in baseline and code, not declared, structure changed → Red
        let bl = baseline_with_types(vec![(
            "ChangedType",
            TypeBaselineEntry::new(TypeKind::Enum, variants(&["A"]), vec![]),
        )]);

        let mut types = HashMap::new();
        types.insert(
            "ChangedType".to_string(),
            TypeNode::new(
                TypeKind::Enum,
                variants(&["A", "B"]), // new variant added
                vec![],
                HashSet::new(),
            ),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let report = check_consistency(&[], &graph, &bl);
        assert_eq!(report.baseline_red_types(), &["ChangedType"]);
        assert_eq!(report.skipped_count(), 0);
    }

    #[test]
    fn test_group3_baseline_deleted_type_is_red() {
        // Type in baseline but not in code, not declared → Red (deletion)
        let bl = baseline_with_types(vec![(
            "DeletedType",
            TypeBaselineEntry::new(TypeKind::Struct, vec![], vec![]),
        )]);

        let graph = TypeGraph::new(HashMap::new(), HashMap::new());

        let report = check_consistency(&[], &graph, &bl);
        assert_eq!(report.baseline_red_types(), &["DeletedType"]);
        assert_eq!(report.skipped_count(), 0);
    }

    #[test]
    fn test_group2_declared_baseline_type_uses_forward_check() {
        // Type in both baseline and declarations → forward check (group 2)
        let bl = baseline_with_types(vec![(
            "TrackId",
            TypeBaselineEntry::new(
                TypeKind::Struct,
                vec![MemberDeclaration::field("0", "u64")],
                vec![],
            ),
        )]);

        let entry = TypeCatalogueEntry::new(
            "TrackId",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap();

        let mut types = HashMap::new();
        types.insert(
            "TrackId".to_string(),
            TypeNode::new(
                TypeKind::Struct,
                vec![MemberDeclaration::field("0", "u64")],
                vec![],
                HashSet::new(),
            ),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let report = check_consistency(&[entry], &graph, &bl);
        assert_eq!(report.forward_signals().len(), 1);
        assert_eq!(report.forward_signals()[0].signal(), ConfidenceSignal::Blue);
        // Not counted as skipped (it's declared → forward check handles it)
        assert_eq!(report.skipped_count(), 0);
        assert!(report.baseline_red_types().is_empty());
    }

    #[test]
    fn test_group1_new_declared_type_uses_forward_check() {
        // Declared but not in baseline → group 1, forward check
        let entry = TypeCatalogueEntry::new(
            "NewType",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap();

        let mut types = HashMap::new();
        types.insert(
            "NewType".to_string(),
            TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new()),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let report = check_consistency(&[entry], &graph, &empty_baseline());
        assert_eq!(report.forward_signals().len(), 1);
        assert_eq!(report.forward_signals()[0].signal(), ConfidenceSignal::Blue);
        assert!(report.undeclared_types().is_empty());
    }

    // --- Trait vs type classification for new variants ---

    #[test]
    fn test_application_service_entry_is_classified_as_trait() {
        // ApplicationService uses `get_trait()` — declared as a trait-shaped variant.
        // Declaring an ApplicationService entry means it goes to `declared_trait_names`.
        // Supplying the trait in the graph → forward check Blue.
        let method = unit_method("execute");
        let entry = TypeCatalogueEntry::new(
            "CreateUseCase",
            "Primary port",
            TypeDefinitionKind::ApplicationService { expected_methods: vec![method.clone()] },
            TypeAction::Add,
            true,
        )
        .unwrap();

        let mut traits = HashMap::new();
        traits.insert("CreateUseCase".to_string(), TraitNode::new(vec![method]));
        let graph = TypeGraph::new(HashMap::new(), traits);

        let report = check_consistency(&[entry], &graph, &empty_baseline());
        // Forward check passes (Blue) — trait was found via get_trait.
        assert_eq!(report.forward_signals().len(), 1);
        assert_eq!(report.forward_signals()[0].signal(), ConfidenceSignal::Blue);
        // Not classified as an undeclared type.
        assert!(report.undeclared_types().is_empty());
    }

    #[test]
    fn test_use_case_entry_is_classified_as_type() {
        // UseCase uses `get_type()` — classified as a type (not a trait).
        let entry = TypeCatalogueEntry::new(
            "SaveTrackUseCase",
            "Struct use case",
            TypeDefinitionKind::UseCase,
            TypeAction::Add,
            true,
        )
        .unwrap();

        let mut types = HashMap::new();
        types.insert(
            "SaveTrackUseCase".to_string(),
            TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new()),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let report = check_consistency(&[entry], &graph, &empty_baseline());
        // Forward check passes (Blue) — type was found via get_type.
        assert_eq!(report.forward_signals().len(), 1);
        assert_eq!(report.forward_signals()[0].signal(), ConfidenceSignal::Blue);
        // Not classified as an undeclared trait.
        assert!(report.undeclared_traits().is_empty());
    }

    #[test]
    fn test_group3_baseline_unchanged_trait_is_skipped() {
        let bl = baseline_with_traits(vec![(
            "MyTrait",
            TraitBaselineEntry::new(vec![unit_method("method_a")]),
        )]);

        let mut traits = HashMap::new();
        traits.insert("MyTrait".to_string(), TraitNode::new(vec![unit_method("method_a")]));
        let graph = TypeGraph::new(HashMap::new(), traits);

        let report = check_consistency(&[], &graph, &bl);
        assert_eq!(report.skipped_count(), 1);
        assert!(report.baseline_red_traits().is_empty());
    }

    #[test]
    fn test_group3_baseline_changed_trait_is_red() {
        let bl = baseline_with_traits(vec![(
            "MyTrait",
            TraitBaselineEntry::new(vec![unit_method("method_a")]),
        )]);

        let mut traits = HashMap::new();
        traits.insert(
            "MyTrait".to_string(),
            TraitNode::new(vec![unit_method("method_a"), unit_method("method_b")]),
        );
        let graph = TypeGraph::new(HashMap::new(), traits);

        let report = check_consistency(&[], &graph, &bl);
        assert_eq!(report.baseline_red_traits(), &["MyTrait"]);
        assert_eq!(report.skipped_count(), 0);
    }

    #[test]
    fn test_mixed_groups_comprehensive() {
        // Set up a scenario with all 4 groups:
        // - "DeclaredNew" (group 1): declared, not in baseline
        // - "DeclaredExisting" (group 2): declared, in baseline
        // - "UnchangedExisting" (group 3 skip): in baseline, unchanged
        // - "ChangedExisting" (group 3 red): in baseline, changed
        // - "BrandNew" (group 4): not declared, not in baseline
        let bl = baseline_with_types(vec![
            ("DeclaredExisting", TypeBaselineEntry::new(TypeKind::Struct, vec![], vec![])),
            (
                "UnchangedExisting",
                TypeBaselineEntry::new(
                    TypeKind::Struct,
                    vec![MemberDeclaration::field("x", "String")],
                    vec![],
                ),
            ),
            ("ChangedExisting", TypeBaselineEntry::new(TypeKind::Enum, variants(&["A"]), vec![])),
        ]);

        let entries = vec![
            TypeCatalogueEntry::new(
                "DeclaredNew",
                "d",
                TypeDefinitionKind::ValueObject,
                TypeAction::Add,
                true,
            )
            .unwrap(),
            TypeCatalogueEntry::new(
                "DeclaredExisting",
                "d",
                TypeDefinitionKind::ValueObject,
                TypeAction::Add,
                true,
            )
            .unwrap(),
        ];

        let mut types = HashMap::new();
        for name in
            &["DeclaredNew", "DeclaredExisting", "UnchangedExisting", "ChangedExisting", "BrandNew"]
        {
            let (kind, members): (TypeKind, Vec<MemberDeclaration>) = if *name == "ChangedExisting"
            {
                (TypeKind::Enum, variants(&["A", "B"])) // changed
            } else if *name == "UnchangedExisting" {
                (TypeKind::Struct, vec![MemberDeclaration::field("x", "String")])
            } else {
                (TypeKind::Struct, vec![])
            };
            types.insert(name.to_string(), TypeNode::new(kind, members, vec![], HashSet::new()));
        }
        let graph = TypeGraph::new(types, HashMap::new());

        let report = check_consistency(&entries, &graph, &bl);

        // Groups 1+2: 2 forward signals
        assert_eq!(report.forward_signals().len(), 2);
        // Group 3 skip: UnchangedExisting
        assert_eq!(report.skipped_count(), 1);
        // Group 3 red: ChangedExisting
        assert_eq!(report.baseline_red_types(), &["ChangedExisting"]);
        // Group 4: BrandNew
        assert_eq!(report.undeclared_types(), &["BrandNew"]);
    }

    // --- Contradiction detection ---

    #[test]
    fn test_contradiction_add_already_in_baseline() {
        let entry = TypeCatalogueEntry::new(
            "Foo",
            "d",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap();
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let baseline = baseline_with_types(vec![(
            "Foo",
            TypeBaselineEntry::new(TypeKind::Struct, vec![], vec![]),
        )]);
        let report = check_consistency(&[entry], &graph, &baseline);
        assert_eq!(report.contradictions().len(), 1);
        assert_eq!(
            report.contradictions()[0].kind(),
            &ActionContradictionKind::AddButAlreadyInBaseline
        );
    }

    #[test]
    fn test_no_contradiction_add_not_in_baseline() {
        let entry = TypeCatalogueEntry::new(
            "Foo",
            "d",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap();
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let baseline = empty_baseline();
        let report = check_consistency(&[entry], &graph, &baseline);
        assert!(report.contradictions().is_empty());
    }

    #[test]
    fn test_contradiction_modify_not_in_baseline() {
        let entry = TypeCatalogueEntry::new(
            "Foo",
            "d",
            TypeDefinitionKind::ValueObject,
            TypeAction::Modify,
            true,
        )
        .unwrap();
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let baseline = empty_baseline();
        let report = check_consistency(&[entry], &graph, &baseline);
        assert_eq!(report.contradictions().len(), 1);
        assert_eq!(
            report.contradictions()[0].kind(),
            &ActionContradictionKind::ModifyButNotInBaseline
        );
    }

    #[test]
    fn test_no_contradiction_modify_in_baseline() {
        let entry = TypeCatalogueEntry::new(
            "Foo",
            "d",
            TypeDefinitionKind::ValueObject,
            TypeAction::Modify,
            true,
        )
        .unwrap();
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let baseline = baseline_with_types(vec![(
            "Foo",
            TypeBaselineEntry::new(TypeKind::Struct, vec![], vec![]),
        )]);
        let report = check_consistency(&[entry], &graph, &baseline);
        assert!(report.contradictions().is_empty());
    }

    #[test]
    fn test_contradiction_reference_not_in_baseline() {
        let entry = TypeCatalogueEntry::new(
            "Foo",
            "d",
            TypeDefinitionKind::ValueObject,
            TypeAction::Reference,
            true,
        )
        .unwrap();
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let baseline = empty_baseline();
        let report = check_consistency(&[entry], &graph, &baseline);
        assert_eq!(report.contradictions().len(), 1);
        assert_eq!(
            report.contradictions()[0].kind(),
            &ActionContradictionKind::ReferenceButNotInBaseline
        );
    }

    #[test]
    fn test_contradiction_reference_not_blue() {
        // Reference entry with type in baseline but not in code → forward Yellow → contradiction
        let entry = TypeCatalogueEntry::new(
            "Foo",
            "d",
            TypeDefinitionKind::ValueObject,
            TypeAction::Reference,
            true,
        )
        .unwrap();
        let graph = TypeGraph::new(HashMap::new(), HashMap::new()); // type absent → Yellow
        let baseline = baseline_with_types(vec![(
            "Foo",
            TypeBaselineEntry::new(TypeKind::Struct, vec![], vec![]),
        )]);
        let report = check_consistency(&[entry], &graph, &baseline);
        assert_eq!(report.contradictions().len(), 1);
        assert_eq!(
            report.contradictions()[0].kind(),
            &ActionContradictionKind::ReferenceButNotBlue
        );
    }

    #[test]
    fn test_delete_error_not_in_baseline() {
        let entry = TypeCatalogueEntry::new(
            "Ghost",
            "d",
            TypeDefinitionKind::ValueObject,
            TypeAction::Delete,
            true,
        )
        .unwrap();
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let baseline = empty_baseline();
        let report = check_consistency(&[entry], &graph, &baseline);
        assert_eq!(report.delete_errors(), &["Ghost"]);
    }

    #[test]
    fn test_delete_error_not_in_baseline_signal_is_red() {
        // An invalid delete (no baseline) must produce a Red forward signal so that
        // consumers who only inspect `forward_signals` see the error without having
        // to also consult `delete_errors`.
        let entry = TypeCatalogueEntry::new(
            "Ghost",
            "d",
            TypeDefinitionKind::ValueObject,
            TypeAction::Delete,
            true,
        )
        .unwrap();
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let baseline = empty_baseline();
        let report = check_consistency(&[entry], &graph, &baseline);
        assert_eq!(report.delete_errors(), &["Ghost"]);
        assert_eq!(report.forward_signals()[0].signal(), ConfidenceSignal::Red);
        assert!(!report.forward_signals()[0].found_type());
    }

    #[test]
    fn test_delete_in_baseline_no_error() {
        let entry = TypeCatalogueEntry::new(
            "Foo",
            "d",
            TypeDefinitionKind::ValueObject,
            TypeAction::Delete,
            true,
        )
        .unwrap();
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let baseline = baseline_with_types(vec![(
            "Foo",
            TypeBaselineEntry::new(TypeKind::Struct, vec![], vec![]),
        )]);
        let report = check_consistency(&[entry], &graph, &baseline);
        assert!(report.delete_errors().is_empty());
    }

    // --- check_type_signals (Stage 2 signal gate) ---
    //
    // Cases mirror the D7–D13 rows in the ADR Test Matrix. The function is
    // shared by both the CI path and the merge gate; Yellow flips between
    // warning and error based on the `strict` parameter.

    fn make_entry(name: &str) -> TypeCatalogueEntry {
        TypeCatalogueEntry::new(
            name,
            "test entry",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap()
    }

    fn make_signal(name: &str, signal: ConfidenceSignal) -> TypeSignal {
        TypeSignal::new(name, "value_object", signal, true, Vec::new(), Vec::new(), Vec::new())
    }

    #[test]
    fn test_check_type_signals_empty_entries_passes_per_adr_d64() {
        // ADR 2026-04-19-1242 §D6.4: empty catalogues (zero type declarations) are
        // a valid state for tracks that only reuse pre-existing types. Drift
        // (types added in code without catalogue declarations) is still surfaced
        // downstream via the reverse SoT Chain ③ evaluation.
        let doc = TypeCatalogueDocument::new(1, Vec::new());
        let outcome = check_type_signals(&doc, false, "domain-types.json");
        assert!(outcome.findings().is_empty(), "empty entries must pass per D6.4");
    }

    #[test]
    fn test_check_type_signals_empty_entries_with_red_signals_blocks() {
        // When the catalogue is empty but `<layer>-type-signals.json` has already
        // hydrated reverse-direction Red findings (undeclared types in code)
        // into `doc.signals()`, the gate must surface them instead of short-circuiting
        // to pass. Otherwise drift could silently merge.
        let mut doc = TypeCatalogueDocument::new(1, Vec::new());
        doc.set_signals(vec![TypeSignal::new(
            "UndeclaredType",
            "undeclared_type",
            ConfidenceSignal::Red,
            true,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )]);
        let outcome = check_type_signals(&doc, false, "domain-types.json");
        assert!(outcome.has_errors(), "empty entries + red reverse signal must be an error");
        let msg = outcome.findings()[0].message();
        assert!(msg.contains("domain-types.json"), "must mention catalogue file: {msg}");
        assert!(msg.contains("UndeclaredType"), "must mention the offending type: {msg}");
        assert!(msg.contains("reverse-direction drift"), "must name the condition: {msg}");
    }

    #[test]
    fn test_check_type_signals_empty_entries_with_yellow_only_passes() {
        // Undeclared yellow signals alone must not block an empty catalogue
        // (only reverse-direction Red is a drift violation).
        let mut doc = TypeCatalogueDocument::new(1, Vec::new());
        doc.set_signals(vec![TypeSignal::new(
            "UndeclaredType",
            "undeclared_type",
            ConfidenceSignal::Yellow,
            false,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )]);
        let outcome = check_type_signals(&doc, false, "domain-types.json");
        assert!(outcome.findings().is_empty(), "empty entries + yellow-only reverse must pass");
    }

    #[test]
    fn test_check_type_signals_none_signals_returns_error() {
        // D8: signals=None → BLOCKED (unevaluated)
        let doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        let outcome = check_type_signals(&doc, false, "domain-types.json");
        assert!(outcome.has_errors(), "None signals must be an error");
        assert!(outcome.findings()[0].message().contains("not yet evaluated"));
    }

    #[test]
    fn test_check_type_signals_coverage_gap_returns_error() {
        // D9: entry has no matching signal → BLOCKED
        let mut doc =
            TypeCatalogueDocument::new(1, vec![make_entry("TrackId"), make_entry("ReviewState")]);
        // Only TrackId has a signal; ReviewState is uncovered
        doc.set_signals(vec![make_signal("TrackId", ConfidenceSignal::Blue)]);
        let outcome = check_type_signals(&doc, false, "domain-types.json");
        assert!(outcome.has_errors());
        let msg = outcome.findings()[0].message();
        assert!(msg.contains("no signal evaluation"), "message: {msg}");
        assert!(msg.contains("ReviewState"));
    }

    #[test]
    fn test_check_type_signals_red_is_error_regardless_of_mode() {
        // D10: Red signal → BLOCKED in both modes
        let mut doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        doc.set_signals(vec![make_signal("TrackId", ConfidenceSignal::Red)]);
        let outcome_interim = check_type_signals(&doc, false, "domain-types.json");
        assert!(outcome_interim.has_errors(), "red in interim must be an error");
        let outcome_strict = check_type_signals(&doc, true, "domain-types.json");
        assert!(outcome_strict.has_errors(), "red in strict must be an error");
    }

    #[test]
    fn test_check_type_signals_yellow_is_warning_in_interim_mode() {
        // D11: declared Yellow, strict=false → PASS with warning
        let mut doc =
            TypeCatalogueDocument::new(1, vec![make_entry("TrackId"), make_entry("ReviewState")]);
        doc.set_signals(vec![
            make_signal("TrackId", ConfidenceSignal::Blue),
            make_signal("ReviewState", ConfidenceSignal::Yellow),
        ]);
        let outcome = check_type_signals(&doc, false, "domain-types.json");
        assert!(!outcome.has_errors(), "yellow in interim must not be an error");
        let findings = outcome.findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity(), crate::verify::Severity::Warning);
        let msg = findings[0].message();
        assert!(msg.contains("1 declared type"), "must mention count: {msg}");
        assert!(msg.contains("ReviewState"), "must list the type name: {msg}");
        assert!(msg.contains("merge gate will block"), "must warn: {msg}");
    }

    #[test]
    fn test_check_type_signals_yellow_is_error_in_strict_mode() {
        // D12: declared Yellow, strict=true → BLOCKED
        let mut doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        doc.set_signals(vec![make_signal("TrackId", ConfidenceSignal::Yellow)]);
        let outcome = check_type_signals(&doc, true, "domain-types.json");
        assert!(outcome.has_errors());
        let findings = outcome.findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity(), crate::verify::Severity::Error);
        assert!(findings[0].message().contains("TrackId"));
    }

    #[test]
    fn test_check_type_signals_all_blue_passes_in_both_modes() {
        // D13: all Blue + coverage complete → PASS
        let mut doc =
            TypeCatalogueDocument::new(1, vec![make_entry("TrackId"), make_entry("ReviewState")]);
        doc.set_signals(vec![
            make_signal("TrackId", ConfidenceSignal::Blue),
            make_signal("ReviewState", ConfidenceSignal::Blue),
        ]);

        let outcome_interim = check_type_signals(&doc, false, "domain-types.json");
        assert!(!outcome_interim.has_errors());
        assert!(outcome_interim.findings().is_empty());

        let outcome_strict = check_type_signals(&doc, true, "domain-types.json");
        assert!(!outcome_strict.has_errors());
        assert!(outcome_strict.findings().is_empty());
    }

    #[test]
    fn test_check_type_signals_undeclared_yellow_is_not_blocked() {
        // Undeclared reverse signals that are Yellow are allowed even in strict
        // mode (per existing verify_from_spec_json logic — only declared Yellow
        // is gated). Undeclared Red is caught by the Red check.
        let mut doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        doc.set_signals(vec![
            make_signal("TrackId", ConfidenceSignal::Blue),
            // Yellow signal for a type not in the entries list (reverse/undeclared)
            TypeSignal::new(
                "UndeclaredType",
                "undeclared_type",
                ConfidenceSignal::Yellow,
                false,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
        ]);

        let outcome_strict = check_type_signals(&doc, true, "domain-types.json");
        assert!(
            !outcome_strict.has_errors(),
            "undeclared Yellow must not block even in strict mode: {outcome_strict:?}"
        );
        assert!(outcome_strict.findings().is_empty());
    }

    // Note: the former `test_check_type_signals_empty_entries_error_mentions_catalogue_file`
    // regression guard (TDDD-BUG-02) is retired — empty-entries no longer produces an
    // error after ADR 2026-04-19-1242 §D6.4. The sibling
    // `test_check_type_signals_yellow_error_mentions_catalogue_file` continues to
    // guard catalogue_file parametrization via the Yellow-strict error path.

    #[test]
    fn test_check_type_signals_yellow_error_mentions_catalogue_file() {
        // TDDD-BUG-02 regression guard: the Yellow-mode error (strict) must use
        // the catalogue_file argument, not a hardcoded layer name.
        let mut doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        doc.set_signals(vec![make_signal("TrackId", ConfidenceSignal::Yellow)]);
        let outcome = check_type_signals(&doc, true, "infrastructure-types.json");
        assert!(outcome.has_errors());
        let msg = outcome.findings()[0].message();
        assert!(msg.contains("infrastructure-types.json"), "must mention caller file: {msg}");
        assert!(!msg.contains("domain-types.json"), "must NOT hardcode domain-types.json: {msg}");
    }

    #[test]
    fn test_consistency_partitions_secondary_adapter_as_type() {
        // The key observable: SecondaryAdapter must land in `declared_type_names`,
        // not `declared_trait_names`. An empty TypeGraph makes the partition
        // invisible (both undeclared lists stay empty regardless). Supply the
        // adapter as a Struct-kinded TypeNode so that:
        //   - If correctly classified as type: `declared_type_names` contains it
        //     → forward check (not group 4) → `undeclared_types` stays empty.
        //   - If wrongly classified as trait: `declared_type_names` does NOT contain
        //     it → group 4 fires → `undeclared_types` would contain "FsReviewStore".
        let entry = TypeCatalogueEntry::new(
            "FsReviewStore",
            "Adapter implementing ReviewReader",
            TypeDefinitionKind::SecondaryAdapter { implements: vec![] },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let entries = vec![entry];

        let mut types = std::collections::HashMap::new();
        types.insert(
            "FsReviewStore".to_string(),
            crate::schema::TypeNode::new(
                crate::schema::TypeKind::Struct,
                vec![],
                vec![],
                std::collections::HashSet::new(),
            ),
        );
        let graph = TypeGraph::new(types, std::collections::HashMap::new());

        let baseline = TypeBaseline::new(
            2,
            crate::timestamp::Timestamp::new("2026-04-16T00:00:00Z").unwrap(),
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
        );
        let report = check_consistency(&entries, &graph, &baseline);
        assert!(
            report.undeclared_types().is_empty(),
            "SecondaryAdapter declared in entries must not appear in undeclared_types \
             (it should be absorbed by declared_type_names, not declared_trait_names)"
        );
        assert!(
            report.undeclared_traits().is_empty(),
            "SecondaryAdapter must not be classified as a trait"
        );
    }
}
