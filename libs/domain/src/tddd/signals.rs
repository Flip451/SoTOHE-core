//! Signal evaluation for the type catalogue.
//!
//! Per-kind evaluators compare `TypeCatalogueEntry` declarations against a
//! pre-indexed `TypeGraph` (which `build_type_graph` produces from rustdoc
//! JSON) and return `TypeSignal` values. The entry point
//! `evaluate_type_signals` iterates all entries and dispatches to the right
//! helper per kind.
//!
//! Historical note (T001): this logic used to live in `catalogue.rs`. It was
//! extracted here during the TDDD-01 rename + DM-06 split.

use std::collections::HashSet;

use crate::ConfidenceSignal;
use crate::schema::{TypeGraph, TypeKind};
use crate::tddd::catalogue::{
    TypeAction, TypeCatalogueEntry, TypeDefinitionKind, TypeSignal, TypestateTransitions,
};

// ---------------------------------------------------------------------------
// Signal evaluation — public entry point
// ---------------------------------------------------------------------------

/// Evaluates type-catalogue signals by comparing entries against a pre-indexed
/// `TypeGraph`.
///
/// Only types declared as `Typestate` in entries are considered valid
/// transition targets.
///
/// Signal rules: Blue = spec and code fully match. Red = everything else.
#[must_use]
pub fn evaluate_type_signals(
    entries: &[TypeCatalogueEntry],
    profile: &TypeGraph,
) -> Vec<TypeSignal> {
    // Collect names of typestate-declared types — only these count as valid transition targets.
    let typestate_names: HashSet<&str> = entries
        .iter()
        .filter(|e| matches!(e.kind(), TypeDefinitionKind::Typestate { .. }))
        .map(|e| e.name())
        .collect();
    entries.iter().map(|entry| evaluate_single(entry, profile, &typestate_names)).collect()
}

fn evaluate_single(
    entry: &TypeCatalogueEntry,
    profile: &TypeGraph,
    typestate_names: &HashSet<&str>,
) -> TypeSignal {
    let name = entry.name();
    let kind_tag = entry.kind().kind_tag().to_string();

    // Delete action inverts the forward check: absent → Blue, present → Yellow.
    // This is orthogonal to kind, so we branch before the kind dispatch.
    if entry.action() == TypeAction::Delete {
        return evaluate_delete(name, &kind_tag, entry.kind(), profile);
    }

    match entry.kind() {
        TypeDefinitionKind::Typestate { transitions } => {
            evaluate_typestate(name, &kind_tag, transitions, profile, typestate_names)
        }
        TypeDefinitionKind::Enum { expected_variants } => {
            evaluate_enum(name, &kind_tag, expected_variants, profile)
        }
        TypeDefinitionKind::ValueObject => evaluate_value_object(name, &kind_tag, profile),
        TypeDefinitionKind::ErrorType { expected_variants } => {
            evaluate_error_type(name, &kind_tag, expected_variants, profile)
        }
        TypeDefinitionKind::TraitPort { expected_methods } => {
            evaluate_trait_port(name, &kind_tag, expected_methods, profile)
        }
    }
}

/// Evaluates a `Delete`-action entry: absent from code → Blue, still present → Yellow.
///
/// TraitPort entries check `graph.get_trait()`; all other kinds check `graph.get_type()`.
fn evaluate_delete(
    name: &str,
    kind_tag: &str,
    kind: &TypeDefinitionKind,
    profile: &TypeGraph,
) -> TypeSignal {
    let present = if matches!(kind, TypeDefinitionKind::TraitPort { .. }) {
        profile.get_trait(name).is_some()
    } else {
        profile.get_type(name).is_some()
    };

    if present {
        // Type still exists — not yet deleted.
        TypeSignal::new(name, kind_tag, ConfidenceSignal::Yellow, true, vec![], vec![], vec![])
    } else {
        // Type is gone — deletion complete.
        TypeSignal::new(name, kind_tag, ConfidenceSignal::Blue, false, vec![], vec![], vec![])
    }
}

/// Builds a Red `TypeSignal`. Exposed `pub(crate)` so `consistency::check_consistency`
/// can patch forward signals for invalid delete actions.
pub(crate) fn red(name: &str, kind_tag: &str, found_type: bool) -> TypeSignal {
    TypeSignal::new(name, kind_tag, ConfidenceSignal::Red, found_type, vec![], vec![], vec![])
}

fn yellow(name: &str, kind_tag: &str) -> TypeSignal {
    TypeSignal::new(name, kind_tag, ConfidenceSignal::Yellow, false, vec![], vec![], vec![])
}

fn blue(name: &str, kind_tag: &str) -> TypeSignal {
    TypeSignal::new(name, kind_tag, ConfidenceSignal::Blue, true, vec![], vec![], vec![])
}

// ---------------------------------------------------------------------------
// Per-kind evaluators
// ---------------------------------------------------------------------------

fn evaluate_typestate(
    name: &str,
    kind_tag: &str,
    transitions: &TypestateTransitions,
    profile: &TypeGraph,
    _typestate_names: &HashSet<&str>,
) -> TypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        return yellow(name, kind_tag);
    };

    // Use pre-filtered outgoing transitions from TypeGraph (set by build_type_graph).
    // Self-transitions are excluded during construction.
    let code_transitions: HashSet<&str> =
        code_type.outgoing().iter().filter(|t| t.as_str() != name).map(|s| s.as_str()).collect();

    match transitions {
        TypestateTransitions::Terminal => {
            if code_transitions.is_empty() {
                blue(name, kind_tag)
            } else {
                let mut extra: Vec<String> =
                    code_transitions.into_iter().map(|s| s.to_string()).collect();
                extra.sort();
                TypeSignal::new(name, kind_tag, ConfidenceSignal::Red, true, vec![], vec![], extra)
            }
        }
        TypestateTransitions::To(targets) => {
            let declared: HashSet<&str> = targets.iter().map(|s| s.as_str()).collect();

            let mut found = Vec::new();
            let mut missing = Vec::new();
            for target in targets {
                if code_transitions.contains(target.as_str()) {
                    found.push(target.clone());
                } else {
                    missing.push(target.clone());
                }
            }

            // Detect undeclared transitions (code has them, spec doesn't).
            let mut extra: Vec<String> = code_transitions
                .iter()
                .filter(|ct| !declared.contains(**ct))
                .map(|s| s.to_string())
                .collect();
            extra.sort();

            let signal = if missing.is_empty() && extra.is_empty() {
                ConfidenceSignal::Blue
            } else {
                ConfidenceSignal::Red
            };
            TypeSignal::new(name, kind_tag, signal, true, found, missing, extra)
        }
    }
}

fn evaluate_enum(
    name: &str,
    kind_tag: &str,
    expected_variants: &[String],
    profile: &TypeGraph,
) -> TypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        return yellow(name, kind_tag);
    };
    if *code_type.kind() != TypeKind::Enum {
        return TypeSignal::new(
            name,
            kind_tag,
            ConfidenceSignal::Red,
            true,
            vec![],
            expected_variants.to_vec(),
            vec![],
        );
    }

    let code_variants: HashSet<&str> = code_type.members().iter().map(|m| m.name()).collect();
    let spec_variants: HashSet<&str> = expected_variants.iter().map(|s| s.as_str()).collect();

    let mut missing: Vec<String> =
        spec_variants.difference(&code_variants).map(|s| s.to_string()).collect();
    let mut extra: Vec<String> =
        code_variants.difference(&spec_variants).map(|s| s.to_string()).collect();
    let mut found: Vec<String> =
        spec_variants.intersection(&code_variants).map(|s| s.to_string()).collect();
    missing.sort();
    extra.sort();
    found.sort();

    let signal = if missing.is_empty() && extra.is_empty() {
        ConfidenceSignal::Blue
    } else {
        ConfidenceSignal::Red
    };

    TypeSignal::new(name, kind_tag, signal, true, found, missing, extra)
}

fn evaluate_value_object(name: &str, kind_tag: &str, profile: &TypeGraph) -> TypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        return yellow(name, kind_tag);
    };
    // ValueObject must be a Struct (not Enum or TypeAlias).
    if *code_type.kind() == TypeKind::Struct {
        blue(name, kind_tag)
    } else {
        red(name, kind_tag, true)
    }
}

fn evaluate_error_type(
    name: &str,
    kind_tag: &str,
    expected_variants: &[String],
    profile: &TypeGraph,
) -> TypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        return yellow(name, kind_tag);
    };
    if *code_type.kind() != TypeKind::Enum {
        return TypeSignal::new(
            name,
            kind_tag,
            ConfidenceSignal::Red,
            true,
            vec![],
            expected_variants.to_vec(),
            vec![],
        );
    }

    // Empty expected_variants with enum confirmation = Blue (existence-only).
    if expected_variants.is_empty() {
        return blue(name, kind_tag);
    }

    let code_variants: HashSet<&str> = code_type.members().iter().map(|m| m.name()).collect();

    let mut found = Vec::new();
    let mut missing = Vec::new();
    for v in expected_variants {
        if code_variants.contains(v.as_str()) {
            found.push(v.clone());
        } else {
            missing.push(v.clone());
        }
    }

    let signal = if missing.is_empty() { ConfidenceSignal::Blue } else { ConfidenceSignal::Red };
    TypeSignal::new(name, kind_tag, signal, true, found, missing, vec![])
}

fn evaluate_trait_port(
    name: &str,
    kind_tag: &str,
    expected_methods: &[String],
    profile: &TypeGraph,
) -> TypeSignal {
    let Some(code_trait) = profile.get_trait(name) else {
        return yellow(name, kind_tag);
    };

    let code_methods: HashSet<&str> = code_trait.methods().iter().map(|m| m.name()).collect();

    let mut found = Vec::new();
    let mut missing = Vec::new();
    for m in expected_methods {
        if code_methods.contains(m.as_str()) {
            found.push(m.clone());
        } else {
            missing.push(m.clone());
        }
    }

    let signal = if missing.is_empty() { ConfidenceSignal::Blue } else { ConfidenceSignal::Red };
    TypeSignal::new(name, kind_tag, signal, true, found, missing, vec![])
}

// ---------------------------------------------------------------------------
// undeclared_to_signals — reverse check Red signal conversion
// ---------------------------------------------------------------------------

/// Converts undeclared type and trait names into Red `TypeSignal`s.
///
/// - Undeclared types get `kind_tag = "undeclared_type"`
/// - Undeclared traits get `kind_tag = "undeclared_trait"`
/// - All signals are `ConfidenceSignal::Red` with `found_type = true`
///   (they exist in code but not in the catalogue).
///
/// # Errors
///
/// This function is infallible.
#[must_use]
pub fn undeclared_to_signals(
    undeclared_types: &[String],
    undeclared_traits: &[String],
) -> Vec<TypeSignal> {
    let mut signals = Vec::with_capacity(undeclared_types.len() + undeclared_traits.len());

    for name in undeclared_types {
        signals.push(TypeSignal::new(
            name.clone(),
            "undeclared_type",
            ConfidenceSignal::Red,
            true,
            vec![],
            vec![],
            vec![],
        ));
    }

    for name in undeclared_traits {
        signals.push(TypeSignal::new(
            name.clone(),
            "undeclared_trait",
            ConfidenceSignal::Red,
            true,
            vec![],
            vec![],
            vec![],
        ));
    }

    signals
}

// ---------------------------------------------------------------------------
// Tests — signal evaluation
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::schema::{TraitNode, TypeNode};
    use crate::tddd::catalogue::{MemberDeclaration, MethodDeclaration};

    /// Build a `MethodDeclaration` that takes no args and returns unit.
    fn unit_method(name: &str) -> MethodDeclaration {
        MethodDeclaration::new(name, Some("&self".into()), vec![], "()", false)
    }

    /// Build a `TypeGraph` with struct-kinded types only (no members, no methods).
    fn make_profile(type_names: &[&str]) -> TypeGraph {
        let mut types = HashMap::new();
        for name in type_names {
            types.insert(
                name.to_string(),
                TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new()),
            );
        }
        TypeGraph::new(types, HashMap::new())
    }

    /// Build a `TypeGraph` with a single enum type and given variants.
    fn make_profile_with_enum(name: &str, variants: &[&str]) -> TypeGraph {
        let mut types = HashMap::new();
        types.insert(
            name.to_string(),
            TypeNode::new(
                TypeKind::Enum,
                variants.iter().copied().map(MemberDeclaration::variant).collect(),
                vec![],
                HashSet::new(),
            ),
        );
        TypeGraph::new(types, HashMap::new())
    }

    /// Build a `TypeGraph` where `from_type` has a method returning `to_type`.
    fn make_profile_with_transition(from_type: &str, to_type: &str) -> TypeGraph {
        let mut types = HashMap::new();
        let outgoing: HashSet<String> = [to_type.to_string()].into();
        let from_node = TypeNode::new(TypeKind::Struct, vec![], vec![], outgoing);
        types.insert(from_type.to_string(), from_node);
        types.insert(
            to_type.to_string(),
            TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new()),
        );
        TypeGraph::new(types, HashMap::new())
    }

    /// Build a `TypeGraph` with a trait and given method names.
    fn make_profile_with_trait(trait_name: &str, methods: &[&str]) -> TypeGraph {
        let mut traits = HashMap::new();
        traits.insert(
            trait_name.to_string(),
            TraitNode::new(methods.iter().copied().map(unit_method).collect()),
        );
        TypeGraph::new(HashMap::new(), traits)
    }

    #[test]
    fn test_evaluate_typestate_blue_when_all_transitions_found() {
        let draft = TypeCatalogueEntry::new(
            "Draft",
            "desc",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::To(vec!["Published".into()]),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let published = TypeCatalogueEntry::new(
            "Published",
            "desc",
            TypeDefinitionKind::Typestate { transitions: TypestateTransitions::Terminal },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_transition("Draft", "Published");
        let results = evaluate_type_signals(&[draft, published], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_typestate_yellow_when_type_not_implemented() {
        let entry = TypeCatalogueEntry::new(
            "Ghost",
            "desc",
            TypeDefinitionKind::Typestate { transitions: TypestateTransitions::Terminal },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
        assert!(!results.first().unwrap().found_type());
    }

    #[test]
    fn test_evaluate_typestate_red_when_transition_missing() {
        let entry = TypeCatalogueEntry::new(
            "Draft",
            "desc",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::To(vec!["Published".into()]),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Type exists but no method returning Published.
        let profile = make_profile(&["Draft"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Red);
        assert_eq!(results.first().unwrap().missing_items(), &["Published"]);
    }

    #[test]
    fn test_evaluate_value_object_blue_when_exists() {
        let entry = TypeCatalogueEntry::new(
            "TrackId",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["TrackId"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_value_object_yellow_when_not_implemented() {
        let entry = TypeCatalogueEntry::new(
            "TrackId",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
        assert!(!results.first().unwrap().found_type());
    }

    // --- Delete action forward check ---

    #[test]
    fn test_delete_value_object_blue_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "OldType",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Delete,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]); // type absent
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
        assert!(!results.first().unwrap().found_type());
    }

    #[test]
    fn test_delete_value_object_yellow_when_still_present() {
        let entry = TypeCatalogueEntry::new(
            "OldType",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Delete,
            true,
        )
        .unwrap();
        let profile = make_profile(&["OldType"]); // type still present
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
        assert!(results.first().unwrap().found_type());
    }

    #[test]
    fn test_delete_trait_port_blue_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "OldRepo",
            "desc",
            TypeDefinitionKind::TraitPort { expected_methods: vec!["find".into()] },
            TypeAction::Delete,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]); // trait absent
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_delete_trait_port_yellow_when_still_present() {
        let entry = TypeCatalogueEntry::new(
            "OldRepo",
            "desc",
            TypeDefinitionKind::TraitPort { expected_methods: vec!["find".into()] },
            TypeAction::Delete,
            true,
        )
        .unwrap();
        // Build a profile with the trait present
        let types = std::collections::HashMap::new();
        let traits = std::collections::HashMap::from([(
            "OldRepo".to_string(),
            TraitNode::new(vec![unit_method("find")]),
        )]);
        let profile = TypeGraph::new(types, traits);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_evaluate_enum_yellow_when_not_implemented() {
        let entry = TypeCatalogueEntry::new(
            "Status",
            "desc",
            TypeDefinitionKind::Enum { expected_variants: vec!["Active".into()] },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Profile has no "Status" type.
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
        assert!(!results.first().unwrap().found_type());
    }

    #[test]
    fn test_evaluate_error_type_yellow_when_not_implemented() {
        let entry = TypeCatalogueEntry::new(
            "DomainError",
            "desc",
            TypeDefinitionKind::ErrorType { expected_variants: vec!["NotFound".into()] },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Profile has no "DomainError" type — declared in spec, not yet implemented.
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
        assert!(!results.first().unwrap().found_type());
    }

    #[test]
    fn test_evaluate_enum_blue_when_variants_match() {
        let entry = TypeCatalogueEntry::new(
            "Status",
            "desc",
            TypeDefinitionKind::Enum { expected_variants: vec!["Active".into(), "Done".into()] },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_enum("Status", &["Active", "Done"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_trait_port_yellow_when_not_implemented() {
        let entry = TypeCatalogueEntry::new(
            "Repo",
            "desc",
            TypeDefinitionKind::TraitPort { expected_methods: vec!["save".into()] },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
        assert!(!results.first().unwrap().found_type());
    }

    #[test]
    fn test_evaluate_trait_port_blue_when_methods_match() {
        let entry = TypeCatalogueEntry::new(
            "Repo",
            "desc",
            TypeDefinitionKind::TraitPort { expected_methods: vec!["save".into(), "find".into()] },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_trait("Repo", &["save", "find"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_typestate_blue_empty_transitions() {
        // Typestate with Terminal transitions = terminal state.
        let entry = TypeCatalogueEntry::new(
            "Final",
            "desc",
            TypeDefinitionKind::Typestate { transitions: TypestateTransitions::Terminal },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["Final"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_typestate_uses_outgoing_not_method_return_types() {
        // "Draft" has method_return_types = {"Published", "NonTypestate"},
        // but outgoing = {"Published"} only (NonTypestate was filtered out by build_type_graph).
        // Evaluation must use outgoing — not method_return_types — so "NonTypestate" must not
        // appear in extra_items even though it is in method_return_types.
        let draft_entry = TypeCatalogueEntry::new(
            "Draft",
            "desc",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::To(vec!["Published".into()]),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let published_entry = TypeCatalogueEntry::new(
            "Published",
            "desc",
            TypeDefinitionKind::Typestate { transitions: TypestateTransitions::Terminal },
            TypeAction::Add,
            true,
        )
        .unwrap();

        // T005: `outgoing` is the sole source of truth for typestate
        // transitions; there is no separate `method_return_types` field
        // that could smuggle non-typestate extras through to the Draft
        // signal. This test now constructs `outgoing` directly with only
        // the typestate target and asserts that the evaluation is Blue and
        // reports no extras.
        let mut types = HashMap::new();
        let outgoing: HashSet<String> = ["Published".to_string()].into();
        let from_node = TypeNode::new(TypeKind::Struct, vec![], vec![], outgoing);
        types.insert("Draft".to_string(), from_node);
        types.insert(
            "Published".to_string(),
            TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new()),
        );
        let profile = TypeGraph::new(types, HashMap::new());

        let results = evaluate_type_signals(&[draft_entry, published_entry], &profile);
        let draft_signal = results.first().unwrap();
        assert_eq!(draft_signal.signal(), ConfidenceSignal::Blue);
        assert!(
            draft_signal.extra_items().is_empty(),
            "expected no extra_items, got {:?}",
            draft_signal.extra_items()
        );
    }

    // --- undeclared_to_signals tests ---

    #[test]
    fn test_undeclared_to_signals_converts_types_to_red() {
        let undeclared = vec!["Foo".to_string(), "Bar".to_string()];
        let signals = undeclared_to_signals(&undeclared, &[]);

        assert_eq!(signals.len(), 2);
        assert_eq!(signals[0].type_name(), "Foo");
        assert_eq!(signals[0].kind_tag(), "undeclared_type");
        assert_eq!(signals[0].signal(), ConfidenceSignal::Red);
        assert!(signals[0].found_type());
        assert!(signals[0].missing_items().is_empty());
        assert!(signals[0].extra_items().is_empty());

        assert_eq!(signals[1].type_name(), "Bar");
        assert_eq!(signals[1].kind_tag(), "undeclared_type");
        assert_eq!(signals[1].signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_undeclared_to_signals_converts_traits_to_red() {
        let undeclared_traits = vec!["MyTrait".to_string()];
        let signals = undeclared_to_signals(&[], &undeclared_traits);

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].type_name(), "MyTrait");
        assert_eq!(signals[0].kind_tag(), "undeclared_trait");
        assert_eq!(signals[0].signal(), ConfidenceSignal::Red);
        assert!(signals[0].found_type());
    }

    #[test]
    fn test_undeclared_to_signals_empty_inputs_returns_empty() {
        let signals = undeclared_to_signals(&[], &[]);
        assert!(signals.is_empty());
    }

    #[test]
    fn test_undeclared_to_signals_mixed_types_and_traits() {
        let types = vec!["Foo".to_string()];
        let traits = vec!["Bar".to_string()];
        let signals = undeclared_to_signals(&types, &traits);

        assert_eq!(signals.len(), 2);
        assert_eq!(signals[0].kind_tag(), "undeclared_type");
        assert_eq!(signals[1].kind_tag(), "undeclared_trait");
    }
}
