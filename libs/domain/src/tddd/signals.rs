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
//!
//! ## Action-aware signal mapping (ADR 2026-04-26-0855 §S)
//!
//! Signal decisions are determined by two structural facts — *forward miss*
//! (declared element absent in code) and *reverse extra* (code element absent
//! in catalogue) — combined with the entry's `TypeAction`:
//!
//! | action    | perfect match | forward miss | reverse extra |
//! |-----------|---------------|--------------|---------------|
//! | add       | Blue          | Yellow       | Red           |
//! | modify    | Blue          | Yellow       | Yellow        |
//! | delete    | Blue (none)   | — (no axis)  | Yellow        |
//! | reference | Blue          | Red          | Red           |

use std::collections::HashSet;

use crate::ConfidenceSignal;
use crate::schema::{TypeGraph, TypeKind};
use crate::tddd::catalogue::{
    TraitImplDecl, TypeAction, TypeCatalogueEntry, TypeDefinitionKind, TypeSignal,
    TypestateTransitions,
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
    let action = entry.action();

    // Delete action inverts the forward check: absent → Blue, present → Yellow.
    // This is orthogonal to kind, so we branch before the kind dispatch.
    if action == TypeAction::Delete {
        return evaluate_delete(name, &kind_tag, entry.kind(), profile);
    }

    match entry.kind() {
        TypeDefinitionKind::Typestate { transitions, .. } => {
            evaluate_typestate(name, &kind_tag, transitions, profile, typestate_names, action)
        }
        TypeDefinitionKind::Enum { expected_variants } => {
            evaluate_enum(name, &kind_tag, expected_variants, profile, action)
        }
        TypeDefinitionKind::ValueObject { .. } => {
            evaluate_value_object(name, &kind_tag, profile, action)
        }
        TypeDefinitionKind::ErrorType { expected_variants } => {
            evaluate_error_type(name, &kind_tag, expected_variants, profile, action)
        }
        TypeDefinitionKind::SecondaryPort { expected_methods } => {
            evaluate_secondary_port(name, &kind_tag, expected_methods, profile, action)
        }
        TypeDefinitionKind::ApplicationService { expected_methods } => {
            evaluate_application_service(name, &kind_tag, expected_methods, profile, action)
        }
        TypeDefinitionKind::UseCase { .. }
        | TypeDefinitionKind::Interactor { .. }
        | TypeDefinitionKind::Dto { .. }
        | TypeDefinitionKind::Command { .. }
        | TypeDefinitionKind::Query { .. }
        | TypeDefinitionKind::Factory { .. } => {
            evaluate_struct_only(name, &kind_tag, profile, action)
        }
        TypeDefinitionKind::SecondaryAdapter { implements, .. } => {
            evaluate_secondary_adapter(name, &kind_tag, implements, profile, action)
        }
        TypeDefinitionKind::FreeFunction { .. } => {
            evaluate_struct_only(name, &kind_tag, profile, action)
        }
    }
}

/// Evaluates a `Delete`-action entry: absent from code → Blue, still present → Yellow.
///
/// `SecondaryPort` and `ApplicationService` entries check `graph.get_trait()`; all other
/// kinds check `graph.get_type()`.
fn evaluate_delete(
    name: &str,
    kind_tag: &str,
    kind: &TypeDefinitionKind,
    profile: &TypeGraph,
) -> TypeSignal {
    let present = if matches!(
        kind,
        TypeDefinitionKind::SecondaryPort { .. } | TypeDefinitionKind::ApplicationService { .. }
    ) {
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

fn blue(name: &str, kind_tag: &str) -> TypeSignal {
    TypeSignal::new(name, kind_tag, ConfidenceSignal::Blue, true, vec![], vec![], vec![])
}

// ---------------------------------------------------------------------------
// Action-aware signal decision helpers (ADR 2026-04-26-0855 §S)
// ---------------------------------------------------------------------------

/// Returns the `ConfidenceSignal` for a **forward miss** (declared element absent in code)
/// given the entry's `TypeAction`.
///
/// | action    | forward miss |
/// |-----------|--------------|
/// | add       | Yellow       |
/// | modify    | Yellow       |
/// | reference | Red          |
/// | delete    | — (no forward axis; callers must not call this for Delete) |
#[must_use]
fn signal_for_forward_miss(action: TypeAction) -> ConfidenceSignal {
    match action {
        TypeAction::Add | TypeAction::Modify => ConfidenceSignal::Yellow,
        TypeAction::Reference => ConfidenceSignal::Red,
        TypeAction::Delete => ConfidenceSignal::Yellow, // delete has no forward axis; callers guard
    }
}

/// Returns the `ConfidenceSignal` for a **reverse extra** (code element absent in catalogue)
/// given the entry's `TypeAction`.
///
/// | action    | reverse extra |
/// |-----------|---------------|
/// | add       | Red           |
/// | modify    | Yellow        |
/// | reference | Red           |
/// | delete    | Yellow (still present — not yet deleted) |
#[must_use]
fn signal_for_reverse_extra(action: TypeAction) -> ConfidenceSignal {
    match action {
        TypeAction::Add | TypeAction::Reference => ConfidenceSignal::Red,
        TypeAction::Modify | TypeAction::Delete => ConfidenceSignal::Yellow,
    }
}

/// Returns the most severe (lowest-confidence) signal between two candidates.
///
/// Severity order (most to least severe): Red > Yellow > Blue.
/// Note: `ConfidenceSignal` implements `Ord` with Blue > Yellow > Red,
/// so the most severe is the *minimum* under that ordering.
#[must_use]
fn most_severe(a: ConfidenceSignal, b: ConfidenceSignal) -> ConfidenceSignal {
    // Ord: Blue > Yellow > Red, so min() gives the most severe.
    a.min(b)
}

/// Combines forward-miss signal and reverse-extra signal into the dominant one.
///
/// Priority: Red > Yellow > Blue.  Both empty ⇒ Blue is returned by the caller,
/// not here; this function is called only when at least one list is non-empty.
#[must_use]
fn dominant_signal(
    forward_signal: ConfidenceSignal,
    has_missing: bool,
    reverse_signal: ConfidenceSignal,
    has_extra: bool,
) -> ConfidenceSignal {
    let mut result = ConfidenceSignal::Blue;
    if has_missing {
        result = most_severe(result, forward_signal);
    }
    if has_extra {
        result = most_severe(result, reverse_signal);
    }
    result
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
    action: TypeAction,
) -> TypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        let miss_signal = signal_for_forward_miss(action);
        return TypeSignal::new(name, kind_tag, miss_signal, false, vec![], vec![], vec![]);
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
                let extra_signal = signal_for_reverse_extra(action);
                TypeSignal::new(name, kind_tag, extra_signal, true, vec![], vec![], extra)
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
                dominant_signal(
                    signal_for_forward_miss(action),
                    !missing.is_empty(),
                    signal_for_reverse_extra(action),
                    !extra.is_empty(),
                )
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
    action: TypeAction,
) -> TypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        let miss_signal = signal_for_forward_miss(action);
        return TypeSignal::new(name, kind_tag, miss_signal, false, vec![], vec![], vec![]);
    };
    if *code_type.kind() != TypeKind::Enum {
        // Type exists but wrong kind — always Red (structural contract violation).
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
        dominant_signal(
            signal_for_forward_miss(action),
            !missing.is_empty(),
            signal_for_reverse_extra(action),
            !extra.is_empty(),
        )
    };

    TypeSignal::new(name, kind_tag, signal, true, found, missing, extra)
}

fn evaluate_value_object(
    name: &str,
    kind_tag: &str,
    profile: &TypeGraph,
    action: TypeAction,
) -> TypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        let miss_signal = signal_for_forward_miss(action);
        return TypeSignal::new(name, kind_tag, miss_signal, false, vec![], vec![], vec![]);
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
    action: TypeAction,
) -> TypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        let miss_signal = signal_for_forward_miss(action);
        return TypeSignal::new(name, kind_tag, miss_signal, false, vec![], vec![], vec![]);
    };
    if *code_type.kind() != TypeKind::Enum {
        // Type exists but wrong kind — always Red (structural contract violation).
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

    let mut found = Vec::new();
    let mut missing = Vec::new();
    for v in expected_variants {
        if code_variants.contains(v.as_str()) {
            found.push(v.clone());
        } else {
            missing.push(v.clone());
        }
    }

    // Reverse check — any code variant not declared in spec is extra.
    // Note: when expected_variants is empty, all code variants are extra.
    let mut extra: Vec<String> =
        code_variants.difference(&spec_variants).map(|s| s.to_string()).collect();
    extra.sort();

    let signal = if missing.is_empty() && extra.is_empty() {
        ConfidenceSignal::Blue
    } else {
        dominant_signal(
            signal_for_forward_miss(action),
            !missing.is_empty(),
            signal_for_reverse_extra(action),
            !extra.is_empty(),
        )
    };
    TypeSignal::new(name, kind_tag, signal, true, found, missing, extra)
}

/// L1 forward+reverse check for a `SecondaryPort` entry (secondary/driven port boundary).
///
/// Forward check (ADR 0002 §D2): each declared method must match a code
/// method on all six L1 axes — step 1 name, step 2 receiver, step 3 params
/// count, step 4 params type order, step 5 returns, step 6 `is_async`. A
/// mismatch on any axis adds the method (as rendered `signature_string()`)
/// to `missing`.
///
/// Reverse check (step 7): any code method on the trait that is not
/// declared in `expected_methods` (keyed by name) is added to `extra`.
///
/// The signal level for forward miss and reverse extra is determined by the
/// `action` via the §S mapping (ADR 2026-04-26-0855). When the trait does not
/// exist in code, a forward-miss signal is returned.
///
/// Delegates to `evaluate_trait_methods` which is shared with `ApplicationService`.
fn evaluate_secondary_port(
    name: &str,
    kind_tag: &str,
    expected_methods: &[crate::tddd::catalogue::MethodDeclaration],
    profile: &TypeGraph,
    action: TypeAction,
) -> TypeSignal {
    evaluate_trait_methods(name, kind_tag, expected_methods, profile, action)
}

/// L1 forward+reverse check for an `ApplicationService` entry (primary/driving port boundary).
///
/// Identical evaluation logic to `SecondaryPort` — the difference is semantic (primary vs
/// secondary port role), not structural. Both use the six L1 axes for forward check and
/// the name-keyed reverse check. Signal levels follow the §S action mapping.
fn evaluate_application_service(
    name: &str,
    kind_tag: &str,
    expected_methods: &[crate::tddd::catalogue::MethodDeclaration],
    profile: &TypeGraph,
    action: TypeAction,
) -> TypeSignal {
    evaluate_trait_methods(name, kind_tag, expected_methods, profile, action)
}

/// Existence-only check for struct-only variants (`UseCase`, `Interactor`, `Dto`, `Command`,
/// `Query`, `Factory`). Checks `graph.get_type()` — forward-miss signal if absent, Blue if
/// present as a `Struct`, Red if present as a different kind. Signal level follows §S mapping.
fn evaluate_struct_only(
    name: &str,
    kind_tag: &str,
    profile: &TypeGraph,
    action: TypeAction,
) -> TypeSignal {
    evaluate_value_object(name, kind_tag, profile, action)
}

/// Evaluates a `SecondaryAdapter` entry: struct existence + trait impl presence + method matching.
///
/// - Step 1: `profile.get_type(name)` — forward-miss signal if struct absent, Red if not a Struct kind.
/// - Step 2: For each `TraitImplDecl` in `implements`, check `profile.get_impl(name, trait_name)`.
///   - If the impl is absent, add trait_name to `missing_items`.
///   - If present and `expected_methods` is non-empty, check method signatures via
///     `method_structurally_matches`.
/// - Step 3: Aggregate — all found → Blue, any missing → forward-miss signal per §S action mapping.
///   Empty `implements` with struct present → Blue (existence-only).
///
/// Note: secondary_adapter intentionally has no reverse check in T001. The ADR
/// 2026-04-15-1636 §Consequences §Bad explicitly scoped out reverse-check for
/// impl-only deletions; ADR 2026-04-26-0855 §D3 adds workspace-origin reverse
/// check for SecondaryAdapter once `TraitImplEntry::origin_crate` is introduced
/// (T004). Until then, only the declared `implements` traits are forward-checked,
/// and missing_items yield the forward-miss signal per §S action mapping
/// (Yellow for add/modify, Red for reference). This replaces the previous
/// unconditional Yellow for forward misses.
fn evaluate_secondary_adapter(
    name: &str,
    kind_tag: &str,
    implements: &[TraitImplDecl],
    profile: &TypeGraph,
    action: TypeAction,
) -> TypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        let miss_signal = signal_for_forward_miss(action);
        return TypeSignal::new(name, kind_tag, miss_signal, false, vec![], vec![], vec![]);
    };

    // SecondaryAdapter must be a Struct (not Enum or TypeAlias).
    if *code_type.kind() != TypeKind::Struct {
        return red(name, kind_tag, true);
    }

    // Empty implements = existence-only check (struct present → Blue)
    if implements.is_empty() {
        return blue(name, kind_tag);
    }

    let mut found_items = Vec::new();
    let mut missing_items = Vec::new();

    for decl in implements {
        let trait_name = decl.trait_name();
        match profile.get_impl(name, trait_name) {
            Some(impl_entry) => {
                if decl.expected_methods().is_empty() {
                    // Existence-only for this trait — impl found is sufficient.
                    found_items.push(trait_name.to_string());
                } else {
                    // L1 method matching for this trait impl
                    let (trait_found, trait_missing) =
                        evaluate_impl_methods(decl.expected_methods(), impl_entry.methods());
                    found_items.extend(trait_found);
                    missing_items.extend(trait_missing);
                }
            }
            None => {
                missing_items.push(format!("impl {trait_name}"));
            }
        }
    }

    // Per ADR 2026-04-11-0003 and 2026-04-26-0855 §S:
    // No reverse check in T001 — the full workspace-origin reverse check for
    // SecondaryAdapter is deferred to T004 (requires TraitImplEntry::origin_crate,
    // ADR 2026-04-26-0855 §D3). Missing declared impls → forward-miss signal per
    // §S action mapping: add/modify → Yellow, reference → Red.
    let signal = if missing_items.is_empty() {
        ConfidenceSignal::Blue
    } else {
        signal_for_forward_miss(action)
    };
    TypeSignal::new(name, kind_tag, signal, true, found_items, missing_items, vec![])
}

/// Compares declared expected methods against actual impl methods.
///
/// Returns `(found, missing)` where each entry is a signature string.
fn evaluate_impl_methods(
    expected: &[crate::tddd::catalogue::MethodDeclaration],
    code_methods: &[crate::tddd::catalogue::MethodDeclaration],
) -> (Vec<String>, Vec<String>) {
    let mut found = Vec::new();
    let mut missing = Vec::new();
    for declared in expected {
        let rendered = declared.signature_string();
        match code_methods.iter().find(|c| c.name() == declared.name()) {
            Some(code) if method_structurally_matches(declared, code) => {
                found.push(rendered);
            }
            _ => {
                missing.push(rendered);
            }
        }
    }
    (found, missing)
}

/// Shared L1 forward+reverse method check for both `SecondaryPort` and `ApplicationService`.
///
/// Signal levels for forward miss and reverse extra are determined by the `action` via the
/// §S mapping (ADR 2026-04-26-0855). When the trait does not exist in code, a forward-miss
/// signal is returned with `found_type = false`.
fn evaluate_trait_methods(
    name: &str,
    kind_tag: &str,
    expected_methods: &[crate::tddd::catalogue::MethodDeclaration],
    profile: &TypeGraph,
    action: TypeAction,
) -> TypeSignal {
    let Some(code_trait) = profile.get_trait(name) else {
        let miss_signal = signal_for_forward_miss(action);
        return TypeSignal::new(name, kind_tag, miss_signal, false, vec![], vec![], vec![]);
    };

    // Forward check — every expected method must appear and match.
    let mut found = Vec::new();
    let mut missing = Vec::new();
    for declared in expected_methods {
        let rendered = declared.signature_string();
        match code_trait.methods().iter().find(|c| c.name() == declared.name()) {
            Some(code) if method_structurally_matches(declared, code) => {
                found.push(rendered);
            }
            _ => {
                missing.push(rendered);
            }
        }
    }

    // Reverse check — any code method not declared is extra.
    let declared_names: HashSet<&str> = expected_methods.iter().map(|m| m.name()).collect();
    let mut extra: Vec<String> = code_trait
        .methods()
        .iter()
        .filter(|c| !declared_names.contains(c.name()))
        .map(|c| c.signature_string())
        .collect();
    extra.sort();

    // Signal levels per ADR 2026-04-26-0855 §S action mapping:
    // - forward miss → signal_for_forward_miss(action)
    // - reverse extra → signal_for_reverse_extra(action)
    // - both empty → Blue
    let signal = if missing.is_empty() && extra.is_empty() {
        ConfidenceSignal::Blue
    } else {
        dominant_signal(
            signal_for_forward_miss(action),
            !missing.is_empty(),
            signal_for_reverse_extra(action),
            !extra.is_empty(),
        )
    };
    TypeSignal::new(name, kind_tag, signal, true, found, missing, extra)
}

/// Returns `true` if two `MethodDeclaration`s match on all six L1 axes:
/// name → receiver → params count → params types (ordered) → returns → async.
/// Parameter names are intentionally ignored — only the type strings matter.
fn method_structurally_matches(
    a: &crate::tddd::catalogue::MethodDeclaration,
    b: &crate::tddd::catalogue::MethodDeclaration,
) -> bool {
    if a.name() != b.name() {
        return false;
    }
    if a.receiver() != b.receiver() {
        return false;
    }
    if a.params().len() != b.params().len() {
        return false;
    }
    for (pa, pb) in a.params().iter().zip(b.params()) {
        if pa.ty() != pb.ty() {
            return false;
        }
    }
    if a.returns() != b.returns() {
        return false;
    }
    if a.is_async() != b.is_async() {
        return false;
    }
    true
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
    use crate::schema::{TraitImplEntry, TraitNode, TypeNode};
    use crate::tddd::catalogue::{MemberDeclaration, MethodDeclaration, TraitImplDecl};

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
                expected_members: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let published = TypeCatalogueEntry::new(
            "Published",
            "desc",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::Terminal,
                expected_members: Vec::new(),
            },
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
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::Terminal,
                expected_members: Vec::new(),
            },
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
    fn test_evaluate_typestate_yellow_when_transition_missing_with_add_action() {
        // action=add + forward miss (transition to Published not found) → Yellow (WIP).
        // Per ADR 2026-04-26-0855 §S: add × forward miss → Yellow.
        let entry = TypeCatalogueEntry::new(
            "Draft",
            "desc",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::To(vec!["Published".into()]),
                expected_members: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Type exists but no method returning Published.
        let profile = make_profile(&["Draft"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
        assert_eq!(results.first().unwrap().missing_items(), &["Published"]);
    }

    #[test]
    fn test_evaluate_typestate_red_when_transition_missing_with_reference_action() {
        // action=reference + forward miss (transition to Published not found) → Red.
        // Per ADR 2026-04-26-0855 §S: reference × forward miss → Red.
        let entry = TypeCatalogueEntry::new(
            "Draft",
            "desc",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::To(vec!["Published".into()]),
                expected_members: Vec::new(),
            },
            TypeAction::Reference,
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
            TypeDefinitionKind::ValueObject { expected_members: Vec::new() },
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
            TypeDefinitionKind::ValueObject { expected_members: Vec::new() },
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
            TypeDefinitionKind::ValueObject { expected_members: Vec::new() },
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
            TypeDefinitionKind::ValueObject { expected_members: Vec::new() },
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
    fn test_delete_secondary_port_blue_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "OldRepo",
            "desc",
            TypeDefinitionKind::SecondaryPort { expected_methods: vec![unit_method("find")] },
            TypeAction::Delete,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]); // trait absent
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_delete_secondary_port_yellow_when_still_present() {
        let entry = TypeCatalogueEntry::new(
            "OldRepo",
            "desc",
            TypeDefinitionKind::SecondaryPort { expected_methods: vec![unit_method("find")] },
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
    fn test_evaluate_error_type_red_when_extra_variant_with_add_action() {
        // Code has extra variant not declared in ErrorType with action=add → reverse extra → Red.
        let entry = TypeCatalogueEntry::new(
            "DomainError",
            "desc",
            TypeDefinitionKind::ErrorType { expected_variants: vec!["NotFound".into()] },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Code enum has "NotFound" (declared) plus "Unexpected" (extra, undeclared).
        let profile = make_profile_with_enum("DomainError", &["NotFound", "Unexpected"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Red);
        assert!(results.first().unwrap().extra_items().iter().any(|e| e == "Unexpected"));
    }

    #[test]
    fn test_evaluate_error_type_yellow_when_extra_variant_with_modify_action() {
        // Code has extra variant not declared in ErrorType with action=modify → reverse extra → Yellow (WIP).
        let entry = TypeCatalogueEntry::new(
            "DomainError",
            "desc",
            TypeDefinitionKind::ErrorType { expected_variants: vec!["NotFound".into()] },
            TypeAction::Modify,
            true,
        )
        .unwrap();
        // Code enum has "NotFound" (declared) plus "Unexpected" (extra, undeclared).
        let profile = make_profile_with_enum("DomainError", &["NotFound", "Unexpected"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
        assert!(results.first().unwrap().extra_items().iter().any(|e| e == "Unexpected"));
    }

    #[test]
    fn test_evaluate_error_type_red_when_extra_variant_with_reference_action() {
        // Code has extra variant not declared in ErrorType with action=reference → contract violation → Red.
        let entry = TypeCatalogueEntry::new(
            "DomainError",
            "desc",
            TypeDefinitionKind::ErrorType { expected_variants: vec!["NotFound".into()] },
            TypeAction::Reference,
            true,
        )
        .unwrap();
        // Code enum has "NotFound" (declared) plus "Unexpected" (extra, undeclared).
        let profile = make_profile_with_enum("DomainError", &["NotFound", "Unexpected"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Red);
        assert!(results.first().unwrap().extra_items().iter().any(|e| e == "Unexpected"));
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
    fn test_evaluate_secondary_port_yellow_when_not_implemented() {
        let entry = TypeCatalogueEntry::new(
            "Repo",
            "desc",
            TypeDefinitionKind::SecondaryPort { expected_methods: vec![unit_method("save")] },
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
    fn test_evaluate_secondary_port_blue_when_methods_match() {
        let entry = TypeCatalogueEntry::new(
            "Repo",
            "desc",
            TypeDefinitionKind::SecondaryPort {
                expected_methods: vec![unit_method("save"), unit_method("find")],
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_trait("Repo", &["save", "find"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_secondary_port_yellow_when_returns_mismatch() {
        // Declared `fn save(&self, user: User) -> Result<(), DomainError>` but
        // the code trait has `fn save(&self, user: User) -> ()` — forward-check miss.
        // Per ADR 2026-04-11-0003 WIP-Yellow rule: forward-check missing yields Yellow
        // (the code has a method with the same name, so no reverse-check extra).
        let entry = TypeCatalogueEntry::new(
            "Repo",
            "desc",
            TypeDefinitionKind::SecondaryPort {
                expected_methods: vec![MethodDeclaration::new(
                    "save",
                    Some("&self".into()),
                    vec![crate::tddd::catalogue::ParamDeclaration::new("user", "User")],
                    "Result<(), DomainError>",
                    false,
                )],
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Code trait has the same name and params but different return.
        let mut traits = HashMap::new();
        let code_method = MethodDeclaration::new(
            "save",
            Some("&self".into()),
            vec![crate::tddd::catalogue::ParamDeclaration::new("user", "User")],
            "()",
            false,
        );
        traits.insert("Repo".to_string(), TraitNode::new(vec![code_method]));
        let profile = TypeGraph::new(HashMap::new(), traits);
        let results = evaluate_type_signals(&[entry], &profile);
        let sig = results.first().unwrap();
        assert_eq!(sig.signal(), ConfidenceSignal::Yellow);
        assert_eq!(sig.missing_items().len(), 1);
        assert!(sig.extra_items().is_empty(), "reverse check should be clean");
    }

    #[test]
    fn test_evaluate_secondary_port_red_when_extra_method_in_code() {
        // Code has an extra `delete` method that the catalogue does not declare.
        let entry = TypeCatalogueEntry::new(
            "Repo",
            "desc",
            TypeDefinitionKind::SecondaryPort { expected_methods: vec![unit_method("save")] },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_trait("Repo", &["save", "delete"]);
        let results = evaluate_type_signals(&[entry], &profile);
        let sig = results.first().unwrap();
        assert_eq!(sig.signal(), ConfidenceSignal::Red);
        assert_eq!(sig.extra_items().len(), 1);
        assert!(sig.extra_items()[0].contains("delete"));
    }

    // --- ApplicationService tests ---

    #[test]
    fn test_evaluate_application_service_yellow_when_not_implemented() {
        let entry = TypeCatalogueEntry::new(
            "CreateUseCase",
            "desc",
            TypeDefinitionKind::ApplicationService {
                expected_methods: vec![unit_method("execute")],
            },
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
    fn test_evaluate_application_service_blue_when_methods_match() {
        let entry = TypeCatalogueEntry::new(
            "CreateUseCase",
            "desc",
            TypeDefinitionKind::ApplicationService {
                expected_methods: vec![unit_method("execute"), unit_method("validate")],
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_trait("CreateUseCase", &["execute", "validate"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_application_service_red_when_extra_method_in_code() {
        // Code has an extra method not declared in ApplicationService.
        let entry = TypeCatalogueEntry::new(
            "CreateUseCase",
            "desc",
            TypeDefinitionKind::ApplicationService {
                expected_methods: vec![unit_method("execute")],
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_trait("CreateUseCase", &["execute", "rollback"]);
        let results = evaluate_type_signals(&[entry], &profile);
        let sig = results.first().unwrap();
        assert_eq!(sig.signal(), ConfidenceSignal::Red);
        assert_eq!(sig.extra_items().len(), 1);
    }

    // --- Struct-only new variant tests ---

    #[test]
    fn test_evaluate_use_case_blue_when_type_exists() {
        let entry = TypeCatalogueEntry::new(
            "SaveTrackUseCase",
            "desc",
            TypeDefinitionKind::UseCase { expected_members: Vec::new() },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["SaveTrackUseCase"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_use_case_yellow_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "SaveTrackUseCase",
            "desc",
            TypeDefinitionKind::UseCase { expected_members: Vec::new() },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_evaluate_interactor_blue_when_type_exists() {
        let entry = TypeCatalogueEntry::new(
            "SaveTrackInteractor",
            "desc",
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["SaveTrackInteractor"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_interactor_yellow_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "SaveTrackInteractor",
            "desc",
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_evaluate_dto_blue_when_type_exists() {
        let entry = TypeCatalogueEntry::new(
            "TrackDto",
            "desc",
            TypeDefinitionKind::Dto { expected_members: Vec::new() },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["TrackDto"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_dto_yellow_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "TrackDto",
            "desc",
            TypeDefinitionKind::Dto { expected_members: Vec::new() },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_evaluate_command_blue_when_type_exists() {
        let entry = TypeCatalogueEntry::new(
            "CreateTrackCommand",
            "desc",
            TypeDefinitionKind::Command { expected_members: Vec::new() },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["CreateTrackCommand"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_command_yellow_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "CreateTrackCommand",
            "desc",
            TypeDefinitionKind::Command { expected_members: Vec::new() },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_evaluate_query_blue_when_type_exists() {
        let entry = TypeCatalogueEntry::new(
            "FindTrackQuery",
            "desc",
            TypeDefinitionKind::Query { expected_members: Vec::new() },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["FindTrackQuery"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_query_yellow_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "FindTrackQuery",
            "desc",
            TypeDefinitionKind::Query { expected_members: Vec::new() },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_evaluate_factory_blue_when_type_exists() {
        let entry = TypeCatalogueEntry::new(
            "TrackFactory",
            "desc",
            TypeDefinitionKind::Factory { expected_members: Vec::new() },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["TrackFactory"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_factory_yellow_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "TrackFactory",
            "desc",
            TypeDefinitionKind::Factory { expected_members: Vec::new() },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_evaluate_typestate_blue_empty_transitions() {
        // Typestate with Terminal transitions = terminal state.
        let entry = TypeCatalogueEntry::new(
            "Final",
            "desc",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::Terminal,
                expected_members: Vec::new(),
            },
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
                expected_members: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let published_entry = TypeCatalogueEntry::new(
            "Published",
            "desc",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::Terminal,
                expected_members: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();

        // `outgoing` is the sole source of truth for typestate transitions;
        // there is no separate `method_return_types` field that could smuggle
        // non-typestate extras through to the Draft signal. This test
        // constructs `outgoing` directly with only the typestate target and
        // asserts that the evaluation is Blue and
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

    // --- TDDD-05: SecondaryAdapter evaluator tests ---

    /// Build a `TypeGraph` with a struct that has trait impls.
    fn make_profile_with_adapter(type_name: &str, trait_impls: Vec<TraitImplEntry>) -> TypeGraph {
        let mut types = HashMap::new();
        let mut node = TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new());
        node.set_trait_impls(trait_impls);
        types.insert(type_name.to_string(), node);
        TypeGraph::new(types, HashMap::new())
    }

    #[test]
    fn test_evaluate_secondary_adapter_blue_all_impls_found() {
        let entry = TypeCatalogueEntry::new(
            "FsReviewStore",
            "desc",
            TypeDefinitionKind::SecondaryAdapter {
                implements: vec![TraitImplDecl::new("ReviewReader", vec![])],
                expected_members: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_adapter(
            "FsReviewStore",
            vec![TraitImplEntry::new("ReviewReader", vec![])],
        );
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
        assert!(results[0].found_type());
    }

    #[test]
    fn test_evaluate_secondary_adapter_yellow_struct_missing() {
        let entry = TypeCatalogueEntry::new(
            "FsReviewStore",
            "desc",
            TypeDefinitionKind::SecondaryAdapter {
                implements: vec![TraitImplDecl::new("ReviewReader", vec![])],
                expected_members: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]); // no types at all
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(!results[0].found_type());
    }

    #[test]
    fn test_evaluate_secondary_adapter_yellow_one_impl_missing() {
        // Per ADR 2026-04-11-0003 WIP-Yellow rule: adapter missing a declared trait impl
        // is forward-check WIP → Yellow (not Red). Secondary adapters have no reverse
        // check (per ADR 2026-04-15-1636), so missing_items always yields Yellow.
        let entry = TypeCatalogueEntry::new(
            "FsReviewStore",
            "desc",
            TypeDefinitionKind::SecondaryAdapter {
                implements: vec![
                    TraitImplDecl::new("ReviewReader", vec![]),
                    TraitImplDecl::new("ReviewWriter", vec![]),
                ],
                expected_members: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Only ReviewReader is implemented, ReviewWriter is missing
        let profile = make_profile_with_adapter(
            "FsReviewStore",
            vec![TraitImplEntry::new("ReviewReader", vec![])],
        );
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(results[0].missing_items().iter().any(|m| m.contains("ReviewWriter")));
    }

    #[test]
    fn test_evaluate_secondary_adapter_yellow_method_signature_mismatch() {
        // Per ADR 2026-04-11-0003 WIP-Yellow rule: method signature mismatch is a
        // forward-check miss → Yellow (not Red).
        let declared_method =
            MethodDeclaration::new("find", Some("&self".into()), vec![], "Option<Review>", false);
        let entry = TypeCatalogueEntry::new(
            "FsReviewStore",
            "desc",
            TypeDefinitionKind::SecondaryAdapter {
                implements: vec![TraitImplDecl::new("ReviewReader", vec![declared_method])],
                expected_members: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Code has the method but with different return type
        let code_method = MethodDeclaration::new(
            "find",
            Some("&self".into()),
            vec![],
            "Result<Review, Error>", // different returns
            false,
        );
        let profile = make_profile_with_adapter(
            "FsReviewStore",
            vec![TraitImplEntry::new("ReviewReader", vec![code_method])],
        );
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(!results[0].missing_items().is_empty());
    }

    #[test]
    fn test_evaluate_secondary_adapter_blue_with_empty_implements() {
        let entry = TypeCatalogueEntry::new(
            "FsReviewStore",
            "desc",
            TypeDefinitionKind::SecondaryAdapter {
                implements: vec![],
                expected_members: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["FsReviewStore"]); // struct exists
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_secondary_adapter_with_two_traits_one_missing_is_yellow() {
        let entry = TypeCatalogueEntry::new(
            "SystemGitRepo",
            "desc",
            TypeDefinitionKind::SecondaryAdapter {
                implements: vec![
                    TraitImplDecl::new("WorktreeReader", vec![]),
                    TraitImplDecl::new("TrackWriter", vec![]),
                ],
                expected_members: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Only WorktreeReader is implemented
        let profile = make_profile_with_adapter(
            "SystemGitRepo",
            vec![TraitImplEntry::new("WorktreeReader", vec![])],
        );
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(results[0].found_items().iter().any(|f| f == "WorktreeReader"));
        assert!(results[0].missing_items().iter().any(|m| m.contains("TrackWriter")));
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

    // ---------------------------------------------------------------------------
    // AC-11: §S action × structural-fact mapping — full coverage
    // (ADR 2026-04-26-0855 §S, impl-plan.json T001)
    //
    // We use SecondaryPort (trait-based) and ValueObject (struct-based) as
    // representative kinds.  The helpers below build minimal graphs so each
    // combination of {action, structural-fact} produces exactly one signal.
    //
    // Coverage matrix (11 cells):
    //  add    × perfect match  → Blue
    //  add    × forward miss   → Yellow
    //  add    × reverse extra  → Red
    //  modify × perfect match  → Blue
    //  modify × forward miss   → Yellow
    //  modify × reverse extra  → Yellow
    //  delete × absent (= "perfect" for delete) → Blue
    //  delete × present (reverse extra) → Yellow
    //  reference × perfect match → Blue
    //  reference × forward miss  → Red
    //  reference × reverse extra → Red
    // ---------------------------------------------------------------------------

    // --- Helpers ----------------------------------------------------------------

    /// Build a ValueObject entry for `name` with the given action.
    fn value_object_entry(name: &str, action: TypeAction) -> TypeCatalogueEntry {
        TypeCatalogueEntry::new(
            name,
            "desc",
            TypeDefinitionKind::ValueObject { expected_members: Vec::new() },
            action,
            true,
        )
        .unwrap()
    }

    /// Build a SecondaryPort entry that declares a single method `"save"` for `name`.
    fn secondary_port_with_method(
        name: &str,
        method: &str,
        action: TypeAction,
    ) -> TypeCatalogueEntry {
        TypeCatalogueEntry::new(
            name,
            "desc",
            TypeDefinitionKind::SecondaryPort { expected_methods: vec![unit_method(method)] },
            action,
            true,
        )
        .unwrap()
    }

    /// Build a TypeGraph that has a trait `trait_name` with exactly the methods in `methods`.
    fn make_trait_profile(trait_name: &str, methods: &[&str]) -> TypeGraph {
        make_profile_with_trait(trait_name, methods)
    }

    // --- add × perfect match → Blue -------------------------------------------

    #[test]
    fn test_action_add_perfect_match_returns_blue_for_value_object() {
        let entry = value_object_entry("Foo", TypeAction::Add);
        let profile = make_profile(&["Foo"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_action_add_perfect_match_returns_blue_for_secondary_port() {
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Add);
        let profile = make_trait_profile("Repo", &["save"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
    }

    // --- add × forward miss → Yellow ------------------------------------------

    #[test]
    fn test_action_add_forward_miss_returns_yellow_for_value_object() {
        // Struct absent from code while action=add → forward miss → Yellow.
        let entry = value_object_entry("Bar", TypeAction::Add);
        let profile = make_profile(&[]); // absent
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(!results[0].found_type());
    }

    #[test]
    fn test_action_add_forward_miss_returns_yellow_for_secondary_port() {
        // Trait absent from code while action=add → forward miss → Yellow.
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Add);
        let profile = make_profile(&[]); // no traits either
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(!results[0].found_type());
    }

    #[test]
    fn test_action_add_method_forward_miss_returns_yellow() {
        // Trait exists but declared method absent → forward miss → Yellow.
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Add);
        let profile = make_trait_profile("Repo", &[]); // trait exists, no methods
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert_eq!(results[0].missing_items().len(), 1);
    }

    // --- add × reverse extra → Red --------------------------------------------

    #[test]
    fn test_action_add_reverse_extra_returns_red_for_secondary_port() {
        // Declared "save" matches; code also has undeclared "delete" → reverse extra → Red.
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Add);
        let profile = make_trait_profile("Repo", &["save", "delete"]); // extra "delete"
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
        assert!(results[0].extra_items().iter().any(|e| e.contains("delete")));
    }

    // --- modify × perfect match → Blue ----------------------------------------

    #[test]
    fn test_action_modify_perfect_match_returns_blue_for_value_object() {
        let entry = value_object_entry("Baz", TypeAction::Modify);
        let profile = make_profile(&["Baz"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_action_modify_perfect_match_returns_blue_for_secondary_port() {
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Modify);
        let profile = make_trait_profile("Repo", &["save"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
    }

    // --- modify × forward miss → Yellow ----------------------------------------

    #[test]
    fn test_action_modify_forward_miss_returns_yellow_for_value_object() {
        // Struct absent while action=modify → forward miss → Yellow (WIP).
        let entry = value_object_entry("Baz", TypeAction::Modify);
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(!results[0].found_type());
    }

    #[test]
    fn test_action_modify_method_forward_miss_returns_yellow() {
        // Declared method absent from code while action=modify → Yellow (WIP).
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Modify);
        let profile = make_trait_profile("Repo", &[]); // trait exists, no methods
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
    }

    // --- modify × reverse extra → Yellow (absorbed as WIP) --------------------

    #[test]
    fn test_action_modify_reverse_extra_returns_yellow_not_red() {
        // Code has an extra undeclared method while action=modify → reverse extra → Yellow.
        // (Contrast with add: reverse extra → Red.)
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Modify);
        let profile = make_trait_profile("Repo", &["save", "delete"]); // extra "delete"
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(
            results[0].signal(),
            ConfidenceSignal::Yellow,
            "modify reverse extra must be Yellow (WIP absorbed), not Red"
        );
        assert!(results[0].extra_items().iter().any(|e| e.contains("delete")));
    }

    // --- delete × absent (deleted) → Blue -------------------------------------

    #[test]
    fn test_action_delete_absent_returns_blue() {
        // action=delete, type absent → deletion complete → Blue.
        let entry = value_object_entry("Old", TypeAction::Delete);
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
        assert!(!results[0].found_type());
    }

    // --- delete × present (not yet deleted) → Yellow --------------------------

    #[test]
    fn test_action_delete_present_returns_yellow() {
        // action=delete, type still present → not yet deleted → Yellow.
        let entry = value_object_entry("Old", TypeAction::Delete);
        let profile = make_profile(&["Old"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(results[0].found_type());
    }

    // --- reference × perfect match → Blue -------------------------------------

    #[test]
    fn test_action_reference_perfect_match_returns_blue_for_value_object() {
        let entry = value_object_entry("Qux", TypeAction::Reference);
        let profile = make_profile(&["Qux"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_action_reference_perfect_match_returns_blue_for_secondary_port() {
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Reference);
        let profile = make_trait_profile("Repo", &["save"]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
    }

    // --- reference × forward miss → Red ---------------------------------------

    #[test]
    fn test_action_reference_forward_miss_returns_red_for_value_object() {
        // Struct absent while action=reference → contract violation → Red.
        let entry = value_object_entry("Qux", TypeAction::Reference);
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
        assert!(!results[0].found_type());
    }

    #[test]
    fn test_action_reference_method_forward_miss_returns_red() {
        // Declared method absent while action=reference → contract violation → Red.
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Reference);
        let profile = make_trait_profile("Repo", &[]); // trait exists, method absent
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
    }

    // --- reference × reverse extra → Red --------------------------------------

    #[test]
    fn test_action_reference_reverse_extra_returns_red() {
        // Code has undeclared extra method while action=reference → contract violation → Red.
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Reference);
        let profile = make_trait_profile("Repo", &["save", "delete"]); // extra "delete"
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
        assert!(results[0].extra_items().iter().any(|e| e.contains("delete")));
    }

    // --- Cross-kind: Enum action mapping ---

    #[test]
    fn test_action_add_enum_forward_miss_returns_yellow() {
        // Enum absent while action=add → forward miss → Yellow.
        let entry = TypeCatalogueEntry::new(
            "Status",
            "desc",
            TypeDefinitionKind::Enum { expected_variants: vec!["Active".into()] },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_action_add_enum_reverse_extra_returns_red() {
        // Enum has extra variant not declared while action=add → reverse extra → Red.
        let entry = TypeCatalogueEntry::new(
            "Status",
            "desc",
            TypeDefinitionKind::Enum { expected_variants: vec!["Active".into()] },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_enum("Status", &["Active", "Deleted"]); // extra "Deleted"
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_action_modify_enum_reverse_extra_returns_yellow() {
        // Enum has extra variant not declared while action=modify → reverse extra → Yellow (WIP).
        let entry = TypeCatalogueEntry::new(
            "Status",
            "desc",
            TypeDefinitionKind::Enum { expected_variants: vec!["Active".into()] },
            TypeAction::Modify,
            true,
        )
        .unwrap();
        let profile = make_profile_with_enum("Status", &["Active", "Deleted"]); // extra "Deleted"
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(
            results[0].signal(),
            ConfidenceSignal::Yellow,
            "modify reverse extra must be Yellow, not Red"
        );
    }

    #[test]
    fn test_action_reference_enum_forward_miss_returns_red() {
        // Enum variant missing while action=reference → contract violation → Red.
        let entry = TypeCatalogueEntry::new(
            "Status",
            "desc",
            TypeDefinitionKind::Enum { expected_variants: vec!["Active".into(), "Done".into()] },
            TypeAction::Reference,
            true,
        )
        .unwrap();
        let profile = make_profile_with_enum("Status", &["Active"]); // "Done" missing
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_action_reference_enum_reverse_extra_returns_red() {
        // Enum has extra variant while action=reference → contract violation → Red.
        let entry = TypeCatalogueEntry::new(
            "Status",
            "desc",
            TypeDefinitionKind::Enum { expected_variants: vec!["Active".into()] },
            TypeAction::Reference,
            true,
        )
        .unwrap();
        let profile = make_profile_with_enum("Status", &["Active", "Deleted"]); // extra
        let results = evaluate_type_signals(&[entry], &profile);
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
    }
}
