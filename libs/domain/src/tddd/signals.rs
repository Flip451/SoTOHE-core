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

use std::collections::{HashMap, HashSet};

use crate::ConfidenceSignal;
use crate::schema::{FunctionNode, TypeGraph, TypeKind};
use crate::tddd::catalogue::{
    MemberDeclaration, ParamDeclaration, TraitImplDecl, TypeAction, TypeCatalogueEntry,
    TypeDefinitionKind, TypeSignal, TypestateTransitions,
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
/// `workspace_crates` is the set of crate names that belong to the workspace
/// (e.g., `{"domain", "usecase", "infrastructure", "cli"}`). It is used by
/// `Interactor` and `SecondaryAdapter` evaluators to filter `TypeNode::trait_impls`
/// to workspace-owned traits for the reverse extra check (IN-10, ADR
/// `2026-04-26-0855` §D3). Pass an empty `HashSet` to disable workspace-origin
/// filtering (backward-compatible: no extra workspace-trait reverse signals).
///
/// Signal rules: Blue = spec and code fully match. Red = everything else.
#[must_use]
pub fn evaluate_type_signals(
    entries: &[TypeCatalogueEntry],
    profile: &TypeGraph,
    workspace_crates: &HashSet<String>,
) -> Vec<TypeSignal> {
    evaluate_type_signals_with_baseline(entries, profile, workspace_crates, &HashSet::new())
}

/// Like [`evaluate_type_signals`] but also receives the set of fully-qualified
/// baseline function names (e.g. `"module::fn"` or `"fn"` for top-level).
///
/// The FreeFunction reverse-extra post-pass skips functions whose FQ name
/// appears in `baseline_fn_fq_names`: those are handled by the group-3 baseline
/// skip logic in `check_consistency` and must not be counted as extra here.
///
/// Pass an empty `HashSet` to get the same behaviour as `evaluate_type_signals`.
#[must_use]
pub(crate) fn evaluate_type_signals_with_baseline(
    entries: &[TypeCatalogueEntry],
    profile: &TypeGraph,
    workspace_crates: &HashSet<String>,
    baseline_fn_fq_names: &HashSet<String>,
) -> Vec<TypeSignal> {
    // Collect names of typestate-declared types — only these count as valid transition targets.
    let typestate_names: HashSet<&str> = entries
        .iter()
        .filter(|e| matches!(e.kind(), TypeDefinitionKind::Typestate { .. }))
        .map(|e| e.name())
        .collect();

    // Pre-compute per-module_path declared FreeFunction name sets for CN-07 reverse check.
    // Key: module_path (None → top-level), Value: set of declared function names.
    let mut free_fn_declared_by_module: HashMap<Option<String>, HashSet<String>> = HashMap::new();
    for entry in entries {
        if let TypeDefinitionKind::FreeFunction { module_path, .. } = entry.kind() {
            free_fn_declared_by_module
                .entry(module_path.clone())
                .or_default()
                .insert(entry.name().to_string());
        }
    }

    let mut signals: Vec<TypeSignal> = entries
        .iter()
        .map(|entry| evaluate_single(entry, profile, &typestate_names, workspace_crates))
        .collect();

    // Post-pass: attach FreeFunction reverse extras (CN-07 module_path-scoped).
    // For each FreeFunction entry, compute extra = functions in the same module_path
    // that are not in the per-module declared set AND not in the baseline (baseline
    // functions are handled by group-3 in check_consistency and must be excluded here).
    for (i, entry) in entries.iter().enumerate() {
        if let TypeDefinitionKind::FreeFunction { module_path, .. } = entry.kind() {
            let action = entry.action();
            let declared_in_module =
                free_fn_declared_by_module.get(module_path).cloned().unwrap_or_default();

            // Functions in the same module_path not declared in any FreeFunction entry
            // and not present in the baseline snapshot (group-3 will handle those).
            let mut extra: Vec<String> = profile
                .functions()
                .iter()
                .filter(|((fn_name, fn_module), _)| {
                    if fn_module.as_deref() != module_path.as_deref() {
                        return false;
                    }
                    if declared_in_module.contains(fn_name.as_str()) {
                        return false;
                    }
                    // Exclude functions tracked in the baseline snapshot.
                    let fq = match fn_module {
                        Some(m) => format!("{m}::{fn_name}"),
                        None => fn_name.clone(),
                    };
                    !baseline_fn_fq_names.contains(&fq)
                })
                .map(|((fn_name, _), _)| fn_name.clone())
                .collect();
            extra.sort();

            if !extra.is_empty() {
                let extra_signal = signal_for_reverse_extra(action);
                // Merge with the existing signal (take the most severe).
                // SAFETY: `i` is a valid index derived from `entries.iter().enumerate()`;
                // `signals` is built from the same entries slice so length matches.
                if let Some(existing) = signals.get(i) {
                    let new_signal = most_severe(existing.signal(), extra_signal);
                    // Reconstruct the signal with extra_items appended.
                    let mut new_extra = existing.extra_items().to_vec();
                    new_extra.extend(extra);
                    new_extra.sort();
                    let replacement = TypeSignal::new(
                        existing.type_name(),
                        existing.kind_tag(),
                        new_signal,
                        existing.found_type(),
                        existing.found_items().to_vec(),
                        existing.missing_items().to_vec(),
                        new_extra,
                    );
                    if let Some(slot) = signals.get_mut(i) {
                        *slot = replacement;
                    }
                }
            }
        }
    }

    signals
}

fn evaluate_single(
    entry: &TypeCatalogueEntry,
    profile: &TypeGraph,
    typestate_names: &HashSet<&str>,
    workspace_crates: &HashSet<String>,
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
        TypeDefinitionKind::Typestate { transitions, expected_members, .. } => evaluate_typestate(
            name,
            &kind_tag,
            transitions,
            expected_members,
            profile,
            typestate_names,
            action,
        ),
        TypeDefinitionKind::Enum { expected_variants } => {
            evaluate_enum(name, &kind_tag, expected_variants, profile, action)
        }
        TypeDefinitionKind::ValueObject { expected_members, .. } => {
            evaluate_struct_with_members(name, &kind_tag, expected_members, profile, action)
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
        TypeDefinitionKind::UseCase { expected_members, .. }
        | TypeDefinitionKind::Dto { expected_members, .. }
        | TypeDefinitionKind::Command { expected_members, .. }
        | TypeDefinitionKind::Query { expected_members, .. }
        | TypeDefinitionKind::Factory { expected_members, .. }
        | TypeDefinitionKind::DomainService { expected_members, .. } => {
            evaluate_struct_with_members(name, &kind_tag, expected_members, profile, action)
        }
        TypeDefinitionKind::Interactor {
            expected_members, declares_application_service, ..
        } => evaluate_interactor(
            name,
            &kind_tag,
            expected_members,
            declares_application_service,
            profile,
            workspace_crates,
            action,
        ),
        TypeDefinitionKind::SecondaryAdapter { implements, expected_members, .. } => {
            evaluate_secondary_adapter(
                name,
                &kind_tag,
                implements,
                expected_members,
                profile,
                workspace_crates,
                action,
            )
        }
        TypeDefinitionKind::FreeFunction {
            module_path,
            expected_params,
            expected_returns,
            expected_is_async,
        } => evaluate_free_function(
            name,
            &kind_tag,
            module_path.as_deref(),
            expected_params,
            expected_returns,
            *expected_is_async,
            profile,
            action,
        ),
    }
}

/// Evaluates a `Delete`-action entry: absent from code → Blue, still present → Yellow.
///
/// - `SecondaryPort` and `ApplicationService` entries check `graph.get_trait()`.
/// - `FreeFunction` entries check `graph.get_function(name, module_path)`.
/// - All other kinds check `graph.get_type()`.
///
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
    } else if let TypeDefinitionKind::FreeFunction { module_path, .. } = kind {
        profile.get_function(name, module_path.as_deref()).is_some()
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
    expected_members: &[MemberDeclaration],
    profile: &TypeGraph,
    _typestate_names: &HashSet<&str>,
    action: TypeAction,
) -> TypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        let miss_signal = signal_for_forward_miss(action);
        return TypeSignal::new(name, kind_tag, miss_signal, false, vec![], vec![], vec![]);
    };

    // Typestate must be a Struct (not Enum or TypeAlias) — structural contract violation otherwise.
    if *code_type.kind() != TypeKind::Struct {
        return red(name, kind_tag, true);
    }

    // Use pre-filtered outgoing transitions from TypeGraph (set by build_type_graph).
    // Self-transitions are excluded during construction.
    let code_transitions: HashSet<&str> =
        code_type.outgoing().iter().filter(|t| t.as_str() != name).map(|s| s.as_str()).collect();

    // Transition check.
    let (mut found, mut missing, mut extra) = match transitions {
        TypestateTransitions::Terminal => {
            let extra: Vec<String> = code_transitions.iter().map(|s| s.to_string()).collect();
            (vec![], vec![], extra)
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
            let extra: Vec<String> = code_transitions
                .iter()
                .filter(|ct| !declared.contains(**ct))
                .map(|s| s.to_string())
                .collect();
            (found, missing, extra)
        }
    };

    // Member check (AC-05): expected_members forward + reverse check.
    let (member_found, member_missing, member_extra) =
        evaluate_members_check(expected_members, code_type.members());
    found.extend(member_found);
    missing.extend(member_missing);
    extra.extend(member_extra);

    extra.sort();
    missing.sort();

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

/// Evaluates struct-based kinds with `expected_members` (AC-05).
///
/// Covers `ValueObject`, `UseCase`, `Dto`, `Command`, `Query`, `Factory` — all 9
/// struct-based kinds except `Interactor` and `SecondaryAdapter` which have their
/// own evaluators.
///
/// Forward check: each declared member (`name` + optionally `ty` for fields)
/// must appear in `TypeNode::members`. Reverse check: each code member not
/// declared in `expected_members` is extra. Signal per §S action mapping.
fn evaluate_struct_with_members(
    name: &str,
    kind_tag: &str,
    expected_members: &[MemberDeclaration],
    profile: &TypeGraph,
    action: TypeAction,
) -> TypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        let miss_signal = signal_for_forward_miss(action);
        return TypeSignal::new(name, kind_tag, miss_signal, false, vec![], vec![], vec![]);
    };
    // Struct-based kinds must be a Struct (not Enum or TypeAlias).
    if *code_type.kind() != TypeKind::Struct {
        return red(name, kind_tag, true);
    }

    let (found, mut missing, mut extra) =
        evaluate_members_check(expected_members, code_type.members());
    missing.sort();
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

/// Evaluates a `SecondaryAdapter` entry: struct existence + trait impl presence + method matching
/// + member check + workspace-origin reverse check (T008, AC-05, IN-10).
///
/// - Step 1: `profile.get_type(name)` — forward-miss signal if struct absent, Red if not a Struct.
/// - Step 2: For each `TraitImplDecl` in `implements`, check `profile.get_impl(name, trait_name)`.
///   - If the impl is absent, add to `missing_items` (forward miss).
///   - If present and `expected_methods` is non-empty, check method signatures via
///     `method_structurally_matches`.
/// - Step 3: `expected_members` forward + reverse check against `TypeNode::members` (AC-05).
/// - Step 4: Workspace-origin reverse check (IN-10): any workspace-crate trait in
///   `TypeNode::trait_impls` not declared in `implements` → reverse extra per §S mapping.
///   Only applies when `workspace_crates` is non-empty. Empty `implements` (existence-only)
///   still gets the workspace reverse check applied.
///
/// All three check axes (implements forward, members, workspace reverse) are combined using
/// the §S dominant-signal rule: forward miss and reverse extra signals are accumulated and the
/// most severe wins.
#[allow(clippy::too_many_arguments)]
fn evaluate_secondary_adapter(
    name: &str,
    kind_tag: &str,
    implements: &[TraitImplDecl],
    expected_members: &[MemberDeclaration],
    profile: &TypeGraph,
    workspace_crates: &HashSet<String>,
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

    let mut found_items = Vec::new();
    let mut missing_items = Vec::new();
    let mut extra_items = Vec::new();

    // implements forward check.
    for decl in implements {
        let trait_name = decl.trait_name();
        match profile.get_impl(name, trait_name) {
            Some(impl_entry) => {
                if decl.expected_methods().is_empty() {
                    // Existence-only for this trait — impl found is sufficient.
                    found_items.push(trait_name.to_string());
                } else {
                    // L1 method matching for this trait impl.
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

    // expected_members forward + reverse check (AC-05).
    let (member_found, member_missing, member_extra) =
        evaluate_members_check(expected_members, code_type.members());
    found_items.extend(member_found);
    missing_items.extend(member_missing);
    extra_items.extend(member_extra);

    // Workspace-origin reverse check (IN-10): workspace-crate traits not declared
    // in implements → reverse extra (ADR 2026-04-26-0855 §D3).
    if !workspace_crates.is_empty() {
        let declared_trait_names: HashSet<&str> =
            implements.iter().map(|d| d.trait_name()).collect();
        let workspace_extras = workspace_origin_extra_traits(
            code_type.trait_impls(),
            &declared_trait_names,
            workspace_crates,
        );
        extra_items.extend(workspace_extras);
    }

    missing_items.sort();
    extra_items.sort();

    let signal = if missing_items.is_empty() && extra_items.is_empty() {
        ConfidenceSignal::Blue
    } else {
        dominant_signal(
            signal_for_forward_miss(action),
            !missing_items.is_empty(),
            signal_for_reverse_extra(action),
            !extra_items.is_empty(),
        )
    };
    TypeSignal::new(name, kind_tag, signal, true, found_items, missing_items, extra_items)
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
// Shared helpers (T008)
// ---------------------------------------------------------------------------

/// Forward + reverse member check for `expected_members` vs `code_members`.
///
/// Forward: each declared `MemberDeclaration` must appear in `code_members`.
///   - `Variant(name)`: code must have a member with the same `name()`.
///   - `Field { name, ty }`: code must have a member with the same `name()` **and** `ty()`.
///
/// Reverse: each code member not declared in `expected_members` (keyed by name) → extra.
///
/// Returns `(found, missing, extra)` where each entry is a display string of the member.
fn evaluate_members_check(
    expected: &[MemberDeclaration],
    code_members: &[MemberDeclaration],
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut found = Vec::new();
    let mut missing = Vec::new();

    for decl in expected {
        let decl_name = decl.name();
        let matched = code_members.iter().find(|c| c.name() == decl_name);
        match (decl, matched) {
            // Variant: only name must match.
            (MemberDeclaration::Variant(_), Some(_)) => {
                found.push(decl_name.to_string());
            }
            // Field: name + ty must match.
            (MemberDeclaration::Field { ty: decl_ty, .. }, Some(code_m)) => {
                if code_m.ty() == Some(decl_ty.as_str()) {
                    found.push(format!("{}: {}", decl_name, decl_ty));
                } else {
                    // Name exists but type mismatches → forward miss.
                    missing.push(format!("{}: {}", decl_name, decl_ty));
                }
            }
            // Not found in code.
            _ => {
                let label = match decl {
                    MemberDeclaration::Variant(_) => decl_name.to_string(),
                    MemberDeclaration::Field { ty, .. } => format!("{}: {}", decl_name, ty),
                };
                missing.push(label);
            }
        }
    }

    // Reverse: code members not declared.
    let declared_names: HashSet<&str> = expected.iter().map(|d| d.name()).collect();
    let extra: Vec<String> = code_members
        .iter()
        .filter(|c| !declared_names.contains(c.name()))
        .map(|c| match c {
            MemberDeclaration::Variant(n) => n.clone(),
            MemberDeclaration::Field { name, ty } => format!("{}: {}", name, ty),
        })
        .collect();

    (found, missing, extra)
}

/// Returns the workspace-owned traits from `trait_impls` that are NOT in `declared_names`.
///
/// Used by both `evaluate_interactor` (declares_application_service reverse check) and
/// `evaluate_secondary_adapter` (implements reverse check) to avoid duplicate logic (DRY).
///
/// A trait is workspace-owned when `TraitImplEntry::origin_crate` is in `workspace_crates`
/// (and non-empty — an empty `origin_crate` means "unknown origin", skipped to avoid
/// false positives).
fn workspace_origin_extra_traits(
    trait_impls: &[crate::schema::TraitImplEntry],
    declared_names: &HashSet<&str>,
    workspace_crates: &HashSet<String>,
) -> Vec<String> {
    trait_impls
        .iter()
        .filter(|t| {
            let origin = t.origin_crate();
            !origin.is_empty()
                && workspace_crates.contains(origin)
                && !declared_names.contains(t.trait_name())
        })
        .map(|t| format!("impl {} (workspace trait not declared)", t.trait_name()))
        .collect()
}

/// Evaluates an `Interactor` entry: struct existence + expected_members + declares_application_service
/// forward + workspace-origin reverse check (T008, AC-04, AC-05, IN-10).
///
/// - Step 1: struct existence (forward-miss signal if absent, Red if not Struct).
/// - Step 2: `expected_members` forward + reverse check against `TypeNode::members`.
/// - Step 3: `declares_application_service` forward check — each declared trait must appear
///   in `TypeNode::trait_impls`.
/// - Step 4: Workspace-origin reverse check — workspace-crate traits in `TypeNode::trait_impls`
///   not listed in `declares_application_service` → reverse extra per §S mapping.
#[allow(clippy::too_many_arguments)]
fn evaluate_interactor(
    name: &str,
    kind_tag: &str,
    expected_members: &[MemberDeclaration],
    declares_application_service: &[String],
    profile: &TypeGraph,
    workspace_crates: &HashSet<String>,
    action: TypeAction,
) -> TypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        let miss_signal = signal_for_forward_miss(action);
        return TypeSignal::new(name, kind_tag, miss_signal, false, vec![], vec![], vec![]);
    };

    if *code_type.kind() != TypeKind::Struct {
        return red(name, kind_tag, true);
    }

    let mut found_items = Vec::new();
    let mut missing_items = Vec::new();
    let mut extra_items = Vec::new();

    // expected_members forward + reverse check (AC-05).
    let (member_found, member_missing, member_extra) =
        evaluate_members_check(expected_members, code_type.members());
    found_items.extend(member_found);
    missing_items.extend(member_missing);
    extra_items.extend(member_extra);

    // declares_application_service forward check (AC-04): declared trait must be in trait_impls.
    let code_trait_names: HashSet<&str> =
        code_type.trait_impls().iter().map(|t| t.trait_name()).collect();
    for trait_name in declares_application_service {
        if code_trait_names.contains(trait_name.as_str()) {
            found_items.push(format!("impl {trait_name}"));
        } else {
            missing_items.push(format!("impl {trait_name}"));
        }
    }

    // Workspace-origin reverse check (IN-10, AC-04): workspace-crate traits not declared
    // in declares_application_service → reverse extra.
    if !workspace_crates.is_empty() {
        let declared_svc_names: HashSet<&str> =
            declares_application_service.iter().map(|s| s.as_str()).collect();
        let workspace_extras = workspace_origin_extra_traits(
            code_type.trait_impls(),
            &declared_svc_names,
            workspace_crates,
        );
        extra_items.extend(workspace_extras);
    }

    missing_items.sort();
    extra_items.sort();

    let signal = if missing_items.is_empty() && extra_items.is_empty() {
        ConfidenceSignal::Blue
    } else {
        dominant_signal(
            signal_for_forward_miss(action),
            !missing_items.is_empty(),
            signal_for_reverse_extra(action),
            !extra_items.is_empty(),
        )
    };
    TypeSignal::new(name, kind_tag, signal, true, found_items, missing_items, extra_items)
}

/// Evaluates a `FreeFunction` entry against `TypeGraph::functions` (T008, AC-06).
///
/// Forward miss: the function `(name, module_path)` key does not exist in `profile.functions()`,
/// OR it exists but `params`/`returns`/`is_async` do not all match the declaration.
///
/// Reverse extra (CN-07 scoped): only functions within the same `module_path` as the catalogue
/// declaration are reverse-checked. Functions in other module paths are out of scope.
/// Reverse extra fires when a function in the same `module_path` has a name not declared
/// in any `FreeFunction` entry with that same `module_path`.
///
/// Signal per §S action mapping.
#[allow(clippy::too_many_arguments)]
fn evaluate_free_function(
    name: &str,
    kind_tag: &str,
    module_path: Option<&str>,
    expected_params: &[ParamDeclaration],
    expected_returns: &[String],
    expected_is_async: bool,
    profile: &TypeGraph,
    action: TypeAction,
) -> TypeSignal {
    // Forward check: look up (name, module_path) in TypeGraph::functions.
    let found_node = profile.get_function(name, module_path);
    let forward_match = found_node.is_some_and(|node| {
        function_signature_matches(node, expected_params, expected_returns, expected_is_async)
    });

    let mut missing_items = Vec::new();
    if !forward_match {
        // Forward miss: either missing entirely or signature mismatch.
        let sig = render_fn_signature(name, expected_params, expected_returns, expected_is_async);
        missing_items.push(sig);
    }

    // Reverse extra (CN-07): collect all functions in the same module_path that are
    // not named `name`. The caller builds the full reverse check across all FreeFunction
    // entries sharing the same module_path in `evaluate_type_signals_free_function_reverse`.
    // Here we only record the forward check result; full reverse is done in the entry-point.
    // (See design note below.)
    //
    // Design note: unlike members or trait_impls, the reverse check for FreeFunction is
    // scoped to the module_path and must consider ALL FreeFunction declarations sharing that
    // module_path, not just this single entry's name. The entry-point therefore collects
    // all FreeFunction entries, groups them by module_path, and passes the per-module
    // declared-name-set here via `same_module_declared_names`.
    //
    // For the per-entry TypeSignal (the return value of this function), we cannot do the
    // full module-scoped reverse check in isolation. Instead, `evaluate_type_signals`
    // performs a post-pass to attach extra_items to FreeFunction signals; this function
    // returns an empty extra_items list as a placeholder.
    //
    // That design is correct but adds a two-pass structure. To keep this function
    // self-contained (and avoid coupling the entry-point to free-function internals),
    // we require callers to pass `same_module_declared_names`: the set of ALL names
    // declared across all FreeFunction entries with this same module_path. This lets
    // the reverse check run here.
    //
    // The entry-point collects that set before calling this function.
    // (end design note)

    let signal = if missing_items.is_empty() {
        ConfidenceSignal::Blue
    } else {
        signal_for_forward_miss(action)
    };

    TypeSignal::new(name, kind_tag, signal, found_node.is_some(), vec![], missing_items, vec![])
}

/// Returns `true` if `node` matches all declared FreeFunction axes:
/// `params` (count + type order), `returns` (as a `Vec<String>`), and `is_async`.
fn function_signature_matches(
    node: &FunctionNode,
    expected_params: &[ParamDeclaration],
    expected_returns: &[String],
    expected_is_async: bool,
) -> bool {
    if node.is_async() != expected_is_async {
        return false;
    }
    if node.params().len() != expected_params.len() {
        return false;
    }
    for (np, ep) in node.params().iter().zip(expected_params) {
        if np.ty() != ep.ty() {
            return false;
        }
    }
    // returns: node stores Vec<String> (last-segment short names).
    if node.returns() != expected_returns {
        return false;
    }
    true
}

/// Renders a FreeFunction signature as a human-readable string for `missing_items`.
fn render_fn_signature(
    name: &str,
    params: &[ParamDeclaration],
    returns: &[String],
    is_async: bool,
) -> String {
    let prefix = if is_async { "async " } else { "" };
    let params_str =
        params.iter().map(|p| format!("{}: {}", p.name(), p.ty())).collect::<Vec<_>>().join(", ");
    let returns_str = returns.join(", ");
    format!("{prefix}fn {name}({params_str}) -> {returns_str}")
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
/// For undeclared free functions, use [`undeclared_functions_to_signals`].
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

/// Converts undeclared free function fully-qualified names into Red `TypeSignal`s.
///
/// Undeclared functions get `kind_tag = "undeclared_function"`. All signals are
/// `ConfidenceSignal::Red` with `found_type = true` (they exist in code but not in
/// the catalogue).
///
/// The `fq_names` are fully-qualified function names as returned by
/// `ConsistencyReport::undeclared_functions()` (e.g., `"module_path::fn_name"` or `"fn_name"`
/// for top-level functions).
///
/// # Errors
///
/// This function is infallible.
#[must_use]
pub fn undeclared_functions_to_signals(fq_names: &[String]) -> Vec<TypeSignal> {
    fq_names
        .iter()
        .map(|name| {
            TypeSignal::new(
                name.clone(),
                "undeclared_function",
                ConfidenceSignal::Red,
                true,
                vec![],
                vec![],
                vec![],
            )
        })
        .collect()
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
                expected_methods: Vec::new(),
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
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_transition("Draft", "Published");
        let results = evaluate_type_signals(&[draft, published], &profile, &HashSet::new());
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
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Type exists but no method returning Published.
        let profile = make_profile(&["Draft"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
                expected_methods: Vec::new(),
            },
            TypeAction::Reference,
            true,
        )
        .unwrap();
        // Type exists but no method returning Published.
        let profile = make_profile(&["Draft"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Red);
        assert_eq!(results.first().unwrap().missing_items(), &["Published"]);
    }

    #[test]
    fn test_evaluate_value_object_blue_when_exists() {
        let entry = TypeCatalogueEntry::new(
            "TrackId",
            "desc",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["TrackId"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_value_object_yellow_when_not_implemented() {
        let entry = TypeCatalogueEntry::new(
            "TrackId",
            "desc",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
        assert!(!results.first().unwrap().found_type());
    }

    // --- Delete action forward check ---

    #[test]
    fn test_delete_value_object_blue_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "OldType",
            "desc",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Delete,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]); // type absent
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
        assert!(!results.first().unwrap().found_type());
    }

    #[test]
    fn test_delete_value_object_yellow_when_still_present() {
        let entry = TypeCatalogueEntry::new(
            "OldType",
            "desc",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Delete,
            true,
        )
        .unwrap();
        let profile = make_profile(&["OldType"]); // type still present
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
            TypeDefinitionKind::UseCase {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["SaveTrackUseCase"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_use_case_yellow_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "SaveTrackUseCase",
            "desc",
            TypeDefinitionKind::UseCase {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["SaveTrackInteractor"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_evaluate_dto_blue_when_type_exists() {
        let entry = TypeCatalogueEntry::new(
            "TrackDto",
            "desc",
            TypeDefinitionKind::Dto { expected_members: Vec::new(), expected_methods: Vec::new() },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["TrackDto"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_dto_yellow_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "TrackDto",
            "desc",
            TypeDefinitionKind::Dto { expected_members: Vec::new(), expected_methods: Vec::new() },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_evaluate_command_blue_when_type_exists() {
        let entry = TypeCatalogueEntry::new(
            "CreateTrackCommand",
            "desc",
            TypeDefinitionKind::Command {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["CreateTrackCommand"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_command_yellow_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "CreateTrackCommand",
            "desc",
            TypeDefinitionKind::Command {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_evaluate_query_blue_when_type_exists() {
        let entry = TypeCatalogueEntry::new(
            "FindTrackQuery",
            "desc",
            TypeDefinitionKind::Query {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["FindTrackQuery"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_query_yellow_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "FindTrackQuery",
            "desc",
            TypeDefinitionKind::Query {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_evaluate_factory_blue_when_type_exists() {
        let entry = TypeCatalogueEntry::new(
            "TrackFactory",
            "desc",
            TypeDefinitionKind::Factory {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["TrackFactory"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_factory_yellow_when_absent() {
        let entry = TypeCatalogueEntry::new(
            "TrackFactory",
            "desc",
            TypeDefinitionKind::Factory {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["Final"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
                expected_methods: Vec::new(),
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
                expected_methods: Vec::new(),
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

        let results =
            evaluate_type_signals(&[draft_entry, published_entry], &profile, &HashSet::new());
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
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_adapter(
            "FsReviewStore",
            vec![TraitImplEntry::new("ReviewReader", vec![])],
        );
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&[]); // no types at all
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
                expected_methods: Vec::new(),
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
                expected_methods: Vec::new(),
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile(&["FsReviewStore"]); // struct exists
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
                expected_methods: Vec::new(),
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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

    #[test]
    fn test_undeclared_functions_to_signals_converts_to_red() {
        let fns = vec!["usecase::track::save_track".to_string(), "top_fn".to_string()];
        let signals = undeclared_functions_to_signals(&fns);

        assert_eq!(signals.len(), 2);
        assert_eq!(signals[0].type_name(), "usecase::track::save_track");
        assert_eq!(signals[0].kind_tag(), "undeclared_function");
        assert_eq!(signals[0].signal(), ConfidenceSignal::Red);
        assert!(signals[0].found_type());

        assert_eq!(signals[1].type_name(), "top_fn");
        assert_eq!(signals[1].kind_tag(), "undeclared_function");
        assert_eq!(signals[1].signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_undeclared_functions_to_signals_empty_returns_empty() {
        assert!(undeclared_functions_to_signals(&[]).is_empty());
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_action_add_perfect_match_returns_blue_for_secondary_port() {
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Add);
        let profile = make_trait_profile("Repo", &["save"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
    }

    // --- add × forward miss → Yellow ------------------------------------------

    #[test]
    fn test_action_add_forward_miss_returns_yellow_for_value_object() {
        // Struct absent from code while action=add → forward miss → Yellow.
        let entry = value_object_entry("Bar", TypeAction::Add);
        let profile = make_profile(&[]); // absent
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(!results[0].found_type());
    }

    #[test]
    fn test_action_add_forward_miss_returns_yellow_for_secondary_port() {
        // Trait absent from code while action=add → forward miss → Yellow.
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Add);
        let profile = make_profile(&[]); // no traits either
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(!results[0].found_type());
    }

    #[test]
    fn test_action_add_method_forward_miss_returns_yellow() {
        // Trait exists but declared method absent → forward miss → Yellow.
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Add);
        let profile = make_trait_profile("Repo", &[]); // trait exists, no methods
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert_eq!(results[0].missing_items().len(), 1);
    }

    // --- add × reverse extra → Red --------------------------------------------

    #[test]
    fn test_action_add_reverse_extra_returns_red_for_secondary_port() {
        // Declared "save" matches; code also has undeclared "delete" → reverse extra → Red.
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Add);
        let profile = make_trait_profile("Repo", &["save", "delete"]); // extra "delete"
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
        assert!(results[0].extra_items().iter().any(|e| e.contains("delete")));
    }

    // --- modify × perfect match → Blue ----------------------------------------

    #[test]
    fn test_action_modify_perfect_match_returns_blue_for_value_object() {
        let entry = value_object_entry("Baz", TypeAction::Modify);
        let profile = make_profile(&["Baz"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_action_modify_perfect_match_returns_blue_for_secondary_port() {
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Modify);
        let profile = make_trait_profile("Repo", &["save"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
    }

    // --- modify × forward miss → Yellow ----------------------------------------

    #[test]
    fn test_action_modify_forward_miss_returns_yellow_for_value_object() {
        // Struct absent while action=modify → forward miss → Yellow (WIP).
        let entry = value_object_entry("Baz", TypeAction::Modify);
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(!results[0].found_type());
    }

    #[test]
    fn test_action_modify_method_forward_miss_returns_yellow() {
        // Declared method absent from code while action=modify → Yellow (WIP).
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Modify);
        let profile = make_trait_profile("Repo", &[]); // trait exists, no methods
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
    }

    // --- modify × reverse extra → Yellow (absorbed as WIP) --------------------

    #[test]
    fn test_action_modify_reverse_extra_returns_yellow_not_red() {
        // Code has an extra undeclared method while action=modify → reverse extra → Yellow.
        // (Contrast with add: reverse extra → Red.)
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Modify);
        let profile = make_trait_profile("Repo", &["save", "delete"]); // extra "delete"
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
        assert!(!results[0].found_type());
    }

    // --- delete × present (not yet deleted) → Yellow --------------------------

    #[test]
    fn test_action_delete_present_returns_yellow() {
        // action=delete, type still present → not yet deleted → Yellow.
        let entry = value_object_entry("Old", TypeAction::Delete);
        let profile = make_profile(&["Old"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(results[0].found_type());
    }

    // --- reference × perfect match → Blue -------------------------------------

    #[test]
    fn test_action_reference_perfect_match_returns_blue_for_value_object() {
        let entry = value_object_entry("Qux", TypeAction::Reference);
        let profile = make_profile(&["Qux"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_action_reference_perfect_match_returns_blue_for_secondary_port() {
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Reference);
        let profile = make_trait_profile("Repo", &["save"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
    }

    // --- reference × forward miss → Red ---------------------------------------

    #[test]
    fn test_action_reference_forward_miss_returns_red_for_value_object() {
        // Struct absent while action=reference → contract violation → Red.
        let entry = value_object_entry("Qux", TypeAction::Reference);
        let profile = make_profile(&[]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
        assert!(!results[0].found_type());
    }

    #[test]
    fn test_action_reference_method_forward_miss_returns_red() {
        // Declared method absent while action=reference → contract violation → Red.
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Reference);
        let profile = make_trait_profile("Repo", &[]); // trait exists, method absent
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
    }

    // --- reference × reverse extra → Red --------------------------------------

    #[test]
    fn test_action_reference_reverse_extra_returns_red() {
        // Code has undeclared extra method while action=reference → contract violation → Red.
        let entry = secondary_port_with_method("Repo", "save", TypeAction::Reference);
        let profile = make_trait_profile("Repo", &["save", "delete"]); // extra "delete"
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
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
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
    }

    // ---------------------------------------------------------------------------
    // T008 AC-05: expected_members — struct-based kinds forward + reverse check
    // ---------------------------------------------------------------------------

    /// Build a TypeGraph with a struct that has explicit members.
    fn make_profile_with_struct_members(
        type_name: &str,
        members: Vec<MemberDeclaration>,
    ) -> TypeGraph {
        let mut types = HashMap::new();
        types.insert(
            type_name.to_string(),
            TypeNode::new(TypeKind::Struct, members, vec![], HashSet::new()),
        );
        TypeGraph::new(types, HashMap::new())
    }

    #[test]
    fn test_expected_members_blue_when_all_fields_match() {
        // AC-05: All declared fields found in code → Blue.
        let entry = TypeCatalogueEntry::new(
            "TrackId",
            "desc",
            TypeDefinitionKind::ValueObject {
                expected_members: vec![MemberDeclaration::field("0", "String")],
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_struct_members(
            "TrackId",
            vec![MemberDeclaration::field("0", "String")],
        );
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
        assert!(results[0].found_type());
        assert!(results[0].missing_items().is_empty());
        assert!(results[0].extra_items().is_empty());
    }

    #[test]
    fn test_expected_members_forward_miss_with_add_action_returns_yellow() {
        // AC-05: Declared field absent from code while action=add → forward miss → Yellow.
        let entry = TypeCatalogueEntry::new(
            "TrackId",
            "desc",
            TypeDefinitionKind::ValueObject {
                expected_members: vec![MemberDeclaration::field("value", "String")],
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Struct exists but has no members.
        let profile = make_profile_with_struct_members("TrackId", vec![]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(!results[0].missing_items().is_empty(), "should have missing items");
        assert!(
            results[0].missing_items().iter().any(|m| m.contains("value")),
            "missing_items should mention 'value'"
        );
    }

    #[test]
    fn test_expected_members_forward_miss_with_reference_action_returns_red() {
        // AC-05: Declared field absent from code while action=reference → Red.
        let entry = TypeCatalogueEntry::new(
            "TrackId",
            "desc",
            TypeDefinitionKind::ValueObject {
                expected_members: vec![MemberDeclaration::field("value", "String")],
                expected_methods: Vec::new(),
            },
            TypeAction::Reference,
            true,
        )
        .unwrap();
        let profile = make_profile_with_struct_members("TrackId", vec![]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_expected_members_reverse_extra_with_add_action_returns_red() {
        // AC-05: Code has extra field not declared while action=add → reverse extra → Red.
        let entry = TypeCatalogueEntry::new(
            "TrackId",
            "desc",
            TypeDefinitionKind::ValueObject {
                expected_members: vec![MemberDeclaration::field("value", "String")],
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Code has declared field plus an extra undeclared field.
        let profile = make_profile_with_struct_members(
            "TrackId",
            vec![
                MemberDeclaration::field("value", "String"),
                MemberDeclaration::field("extra_field", "u64"),
            ],
        );
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
        assert!(
            results[0].extra_items().iter().any(|e| e.contains("extra_field")),
            "extra_items should mention 'extra_field'"
        );
    }

    #[test]
    fn test_expected_members_reverse_extra_with_modify_action_returns_yellow() {
        // AC-05: Code has extra field while action=modify → reverse extra → Yellow (WIP).
        let entry = TypeCatalogueEntry::new(
            "TrackId",
            "desc",
            TypeDefinitionKind::ValueObject {
                expected_members: vec![MemberDeclaration::field("value", "String")],
                expected_methods: Vec::new(),
            },
            TypeAction::Modify,
            true,
        )
        .unwrap();
        let profile = make_profile_with_struct_members(
            "TrackId",
            vec![
                MemberDeclaration::field("value", "String"),
                MemberDeclaration::field("extra_field", "u64"),
            ],
        );
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(
            results[0].signal(),
            ConfidenceSignal::Yellow,
            "modify reverse extra must be Yellow, not Red"
        );
    }

    #[test]
    fn test_expected_members_type_mismatch_counts_as_forward_miss() {
        // AC-05: Field name exists in code but type string differs → forward miss.
        let entry = TypeCatalogueEntry::new(
            "Money",
            "desc",
            TypeDefinitionKind::ValueObject {
                // Declares field "amount" as u64.
                expected_members: vec![MemberDeclaration::field("amount", "u64")],
                expected_methods: Vec::new(),
            },
            TypeAction::Reference,
            true,
        )
        .unwrap();
        // Code has "amount" with type "i64" — name matches but type doesn't.
        let profile = make_profile_with_struct_members(
            "Money",
            vec![MemberDeclaration::field("amount", "i64")],
        );
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        // reference × forward miss → Red.
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
        assert!(results[0].missing_items().iter().any(|m| m.contains("amount")));
    }

    #[test]
    fn test_expected_members_variant_match_for_typestate() {
        // AC-05: Typestate with expected_members (variants) — all found → Blue.
        let entry = TypeCatalogueEntry::new(
            "Status",
            "desc",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::Terminal,
                expected_members: vec![
                    MemberDeclaration::field("id", "StatusId"),
                    MemberDeclaration::field("label", "String"),
                ],
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_struct_members(
            "Status",
            vec![
                MemberDeclaration::field("id", "StatusId"),
                MemberDeclaration::field("label", "String"),
            ],
        );
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_typestate_with_non_struct_kind_returns_red() {
        // Kind guard (P1 fix): a Typestate declaration matched against an Enum TypeNode
        // must return Red — structural contract violation, same guard as other struct-based kinds.
        let entry = TypeCatalogueEntry::new(
            "Status",
            "desc",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::Terminal,
                expected_members: vec![],
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Code has "Status" as an Enum, not a Struct.
        let profile = make_profile_with_enum("Status", &["Active", "Done"]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(
            results[0].signal(),
            ConfidenceSignal::Red,
            "Typestate matched against non-Struct TypeNode must be Red"
        );
        assert!(results[0].found_type(), "found_type must be true (the name exists in code)");
    }

    // ---------------------------------------------------------------------------
    // T008 AC-04: Interactor declares_application_service forward + reverse check
    // ---------------------------------------------------------------------------

    /// Build a TypeGraph with a struct that has trait impls carrying origin_crate.
    fn make_profile_with_interactor(
        type_name: &str,
        trait_impls: Vec<TraitImplEntry>,
    ) -> TypeGraph {
        let mut types = HashMap::new();
        let mut node = TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new());
        node.set_trait_impls(trait_impls);
        types.insert(type_name.to_string(), node);
        TypeGraph::new(types, HashMap::new())
    }

    #[test]
    fn test_interactor_declares_application_service_blue_when_trait_found() {
        // AC-04: Interactor declares trait + trait exists in code → Blue.
        let entry = TypeCatalogueEntry::new(
            "SaveTrackInteractor",
            "desc",
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: vec!["SaveTrackUseCase".to_string()],
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_interactor(
            "SaveTrackInteractor",
            vec![TraitImplEntry::new("SaveTrackUseCase", vec![])],
        );
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
        assert!(results[0].found_items().iter().any(|f| f.contains("SaveTrackUseCase")));
    }

    #[test]
    fn test_interactor_declares_application_service_forward_miss_returns_yellow() {
        // AC-04: Declared trait absent in code while action=add → forward miss → Yellow.
        let entry = TypeCatalogueEntry::new(
            "SaveTrackInteractor",
            "desc",
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: vec!["SaveTrackUseCase".to_string()],
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        // Struct exists but does not implement the declared trait.
        let profile = make_profile_with_interactor("SaveTrackInteractor", vec![]);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(results[0].missing_items().iter().any(|m| m.contains("SaveTrackUseCase")));
    }

    #[test]
    fn test_interactor_workspace_reverse_check_fires_for_workspace_trait() {
        // AC-04 / IN-10: Code implements workspace-owned trait not declared in
        // declares_application_service → reverse extra → Red (action=add).
        let entry = TypeCatalogueEntry::new(
            "SaveTrackInteractor",
            "desc",
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: vec!["SaveTrackUseCase".to_string()],
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_interactor(
            "SaveTrackInteractor",
            vec![
                TraitImplEntry::with_origin_crate("SaveTrackUseCase", vec![], "usecase"),
                // Extra workspace-owned trait not declared.
                TraitImplEntry::with_origin_crate("DeleteTrackUseCase", vec![], "usecase"),
            ],
        );
        let workspace_crates: HashSet<String> =
            ["usecase".to_string(), "domain".to_string()].into();
        let results = evaluate_type_signals(&[entry], &profile, &workspace_crates);
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
        assert!(
            results[0].extra_items().iter().any(|e| e.contains("DeleteTrackUseCase")),
            "extra_items should mention undeclared workspace trait"
        );
    }

    #[test]
    fn test_interactor_workspace_reverse_check_skips_external_trait() {
        // IN-10: Trait with non-workspace origin_crate is NOT reverse-extra checked.
        let entry = TypeCatalogueEntry::new(
            "SaveTrackInteractor",
            "desc",
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: vec!["SaveTrackUseCase".to_string()],
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_interactor(
            "SaveTrackInteractor",
            vec![
                TraitImplEntry::with_origin_crate("SaveTrackUseCase", vec![], "usecase"),
                // External (non-workspace) trait — should be skipped by reverse check.
                TraitImplEntry::with_origin_crate("Debug", vec![], "std"),
            ],
        );
        let workspace_crates: HashSet<String> =
            ["usecase".to_string(), "domain".to_string()].into();
        let results = evaluate_type_signals(&[entry], &profile, &workspace_crates);
        // Debug is external → no reverse extra → Blue.
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
        assert!(results[0].extra_items().is_empty(), "external trait must not appear in extras");
    }

    #[test]
    fn test_interactor_workspace_reverse_check_skips_unknown_origin() {
        // IN-10: Trait with empty origin_crate (unknown) is skipped to avoid false positives.
        let entry = TypeCatalogueEntry::new(
            "SaveTrackInteractor",
            "desc",
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: vec!["SaveTrackUseCase".to_string()],
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_interactor(
            "SaveTrackInteractor",
            vec![
                TraitImplEntry::with_origin_crate("SaveTrackUseCase", vec![], "usecase"),
                // Unknown origin (empty string) — TraitImplEntry::new sets origin to "".
                TraitImplEntry::new("UnknownTrait", vec![]),
            ],
        );
        let workspace_crates: HashSet<String> =
            ["usecase".to_string(), "domain".to_string()].into();
        let results = evaluate_type_signals(&[entry], &profile, &workspace_crates);
        // Unknown-origin trait must be skipped → Blue.
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
        assert!(
            results[0].extra_items().is_empty(),
            "unknown-origin trait must not appear in extras"
        );
    }

    // ---------------------------------------------------------------------------
    // T008: SecondaryAdapter workspace-origin reverse check (IN-10)
    // ---------------------------------------------------------------------------

    #[test]
    fn test_secondary_adapter_workspace_reverse_fires_for_undeclared_workspace_trait() {
        // IN-10: SecondaryAdapter code has workspace-owned trait not declared in `implements`
        // → reverse extra → Red (action=add).
        let entry = TypeCatalogueEntry::new(
            "FsReviewStore",
            "desc",
            TypeDefinitionKind::SecondaryAdapter {
                implements: vec![TraitImplDecl::new("ReviewReader", vec![])],
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_adapter(
            "FsReviewStore",
            vec![
                TraitImplEntry::with_origin_crate("ReviewReader", vec![], "domain"),
                // Undeclared workspace-owned trait.
                TraitImplEntry::with_origin_crate("ReviewWriter", vec![], "domain"),
            ],
        );
        let workspace_crates: HashSet<String> =
            ["domain".to_string(), "usecase".to_string()].into();
        let results = evaluate_type_signals(&[entry], &profile, &workspace_crates);
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
        assert!(
            results[0].extra_items().iter().any(|e| e.contains("ReviewWriter")),
            "extra_items should mention undeclared workspace trait ReviewWriter"
        );
    }

    #[test]
    fn test_secondary_adapter_workspace_reverse_skips_external_traits() {
        // IN-10: External traits (e.g. Debug) are not reverse-checked.
        let entry = TypeCatalogueEntry::new(
            "FsReviewStore",
            "desc",
            TypeDefinitionKind::SecondaryAdapter {
                implements: vec![TraitImplDecl::new("ReviewReader", vec![])],
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let profile = make_profile_with_adapter(
            "FsReviewStore",
            vec![
                TraitImplEntry::with_origin_crate("ReviewReader", vec![], "domain"),
                TraitImplEntry::with_origin_crate("Debug", vec![], "std"),
                TraitImplEntry::with_origin_crate("Display", vec![], "std"),
            ],
        );
        let workspace_crates: HashSet<String> =
            ["domain".to_string(), "usecase".to_string()].into();
        let results = evaluate_type_signals(&[entry], &profile, &workspace_crates);
        // All code workspace traits are declared → Blue (external traits skipped).
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
        assert!(results[0].extra_items().is_empty());
    }

    // ---------------------------------------------------------------------------
    // T008 AC-06: FreeFunction evaluator — forward + reverse check
    // ---------------------------------------------------------------------------

    /// Build a TypeGraph with a single free function.
    fn make_profile_with_fn(
        fn_name: &str,
        module_path: Option<&str>,
        params: Vec<crate::tddd::catalogue::ParamDeclaration>,
        returns: Vec<String>,
        is_async: bool,
    ) -> TypeGraph {
        let mut functions = HashMap::new();
        functions.insert(
            (fn_name.to_string(), module_path.map(str::to_string)),
            FunctionNode::new(params, returns, is_async, module_path.map(str::to_string)),
        );
        TypeGraph::with_functions(HashMap::new(), HashMap::new(), functions)
    }

    /// Build a FreeFunction TypeCatalogueEntry.
    fn free_fn_entry(
        name: &str,
        module_path: Option<&str>,
        params: Vec<crate::tddd::catalogue::ParamDeclaration>,
        returns: Vec<String>,
        is_async: bool,
        action: TypeAction,
    ) -> TypeCatalogueEntry {
        TypeCatalogueEntry::new(
            name,
            "desc",
            TypeDefinitionKind::FreeFunction {
                module_path: module_path.map(str::to_string),
                expected_params: params,
                expected_returns: returns,
                expected_is_async: is_async,
            },
            action,
            true,
        )
        .unwrap()
    }

    #[test]
    fn test_free_function_blue_when_signature_matches() {
        // AC-06: FreeFunction exists with matching signature → Blue.
        let entry = free_fn_entry(
            "save_track",
            Some("usecase::track"),
            vec![crate::tddd::catalogue::ParamDeclaration::new("cmd", "SaveCommand")],
            vec!["Result".to_string()],
            false,
            TypeAction::Add,
        );
        let profile = make_profile_with_fn(
            "save_track",
            Some("usecase::track"),
            vec![crate::tddd::catalogue::ParamDeclaration::new("cmd", "SaveCommand")],
            vec!["Result".to_string()],
            false,
        );
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
        assert!(results[0].missing_items().is_empty());
        assert!(results[0].extra_items().is_empty());
    }

    #[test]
    fn test_free_function_forward_miss_absent_returns_yellow_for_add() {
        // AC-06: Function absent from TypeGraph while action=add → forward miss → Yellow.
        let entry = free_fn_entry(
            "save_track",
            Some("usecase::track"),
            vec![],
            vec!["()".to_string()],
            false,
            TypeAction::Add,
        );
        // Empty TypeGraph — no functions.
        let profile = TypeGraph::new(HashMap::new(), HashMap::new());
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(!results[0].missing_items().is_empty(), "should report forward miss");
    }

    #[test]
    fn test_free_function_forward_miss_absent_returns_red_for_reference() {
        // AC-06: Function absent while action=reference → contract violation → Red.
        let entry = free_fn_entry(
            "save_track",
            Some("usecase::track"),
            vec![],
            vec!["()".to_string()],
            false,
            TypeAction::Reference,
        );
        let profile = TypeGraph::new(HashMap::new(), HashMap::new());
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_free_function_signature_mismatch_is_forward_miss() {
        // AC-06: Function exists but params differ → signature mismatch → forward miss → Yellow (add).
        let entry = free_fn_entry(
            "save_track",
            Some("usecase::track"),
            vec![crate::tddd::catalogue::ParamDeclaration::new("cmd", "SaveCommand")],
            vec!["Result".to_string()],
            false,
            TypeAction::Add,
        );
        // Code has different params (wrong type string).
        let profile = make_profile_with_fn(
            "save_track",
            Some("usecase::track"),
            vec![crate::tddd::catalogue::ParamDeclaration::new("cmd", "WrongCommand")],
            vec!["Result".to_string()],
            false,
        );
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(
            !results[0].missing_items().is_empty(),
            "signature mismatch should be a forward miss"
        );
    }

    #[test]
    fn test_free_function_reverse_extra_in_same_module_returns_red_for_add() {
        // AC-06 / CN-07: Code has extra function in same module_path not declared → reverse extra → Red (add).
        // Declare only "save_track"; code also has "delete_track" in the same module.
        let entry = free_fn_entry(
            "save_track",
            Some("usecase::track"),
            vec![],
            vec!["()".to_string()],
            false,
            TypeAction::Add,
        );
        // Build TypeGraph with both "save_track" (declared) and "delete_track" (extra).
        let mut functions = HashMap::new();
        functions.insert(
            ("save_track".to_string(), Some("usecase::track".to_string())),
            FunctionNode::new(
                vec![],
                vec!["()".to_string()],
                false,
                Some("usecase::track".to_string()),
            ),
        );
        functions.insert(
            ("delete_track".to_string(), Some("usecase::track".to_string())),
            FunctionNode::new(
                vec![],
                vec!["()".to_string()],
                false,
                Some("usecase::track".to_string()),
            ),
        );
        let profile = TypeGraph::with_functions(HashMap::new(), HashMap::new(), functions);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
        assert!(
            results[0].extra_items().iter().any(|e| e.contains("delete_track")),
            "extra_items should mention undeclared 'delete_track' in same module"
        );
    }

    #[test]
    fn test_free_function_reverse_extra_in_different_module_is_ignored() {
        // CN-07: Reverse check is scoped to the declared module_path.
        // Function in a different module must NOT appear in extra_items.
        let entry = free_fn_entry(
            "save_track",
            Some("usecase::track"),
            vec![],
            vec!["()".to_string()],
            false,
            TypeAction::Add,
        );
        // "delete_track" is in a different module — out of scope for reverse check.
        let mut functions = HashMap::new();
        functions.insert(
            ("save_track".to_string(), Some("usecase::track".to_string())),
            FunctionNode::new(
                vec![],
                vec!["()".to_string()],
                false,
                Some("usecase::track".to_string()),
            ),
        );
        functions.insert(
            ("delete_track".to_string(), Some("usecase::other".to_string())),
            FunctionNode::new(
                vec![],
                vec!["()".to_string()],
                false,
                Some("usecase::other".to_string()),
            ),
        );
        let profile = TypeGraph::with_functions(HashMap::new(), HashMap::new(), functions);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        // No extra in same module → Blue.
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
        assert!(
            results[0].extra_items().is_empty(),
            "function in a different module must not be flagged as extra"
        );
    }

    #[test]
    fn test_free_function_reverse_extra_with_modify_returns_yellow() {
        // AC-06: Extra function in same module_path while action=modify → Yellow (WIP absorbed).
        let entry = free_fn_entry(
            "save_track",
            Some("usecase::track"),
            vec![],
            vec!["()".to_string()],
            false,
            TypeAction::Modify,
        );
        let mut functions = HashMap::new();
        functions.insert(
            ("save_track".to_string(), Some("usecase::track".to_string())),
            FunctionNode::new(
                vec![],
                vec!["()".to_string()],
                false,
                Some("usecase::track".to_string()),
            ),
        );
        functions.insert(
            ("extra_fn".to_string(), Some("usecase::track".to_string())),
            FunctionNode::new(
                vec![],
                vec!["()".to_string()],
                false,
                Some("usecase::track".to_string()),
            ),
        );
        let profile = TypeGraph::with_functions(HashMap::new(), HashMap::new(), functions);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(
            results[0].signal(),
            ConfidenceSignal::Yellow,
            "modify reverse extra must be Yellow, not Red"
        );
        assert!(results[0].extra_items().iter().any(|e| e.contains("extra_fn")));
    }

    #[test]
    fn test_free_function_two_declarations_share_module_reverse_check() {
        // CN-07: Two FreeFunction entries in the same module_path; code has both plus one extra.
        // Both declared names are in the per-module set, so only the undeclared one is extra.
        let entry_save = free_fn_entry(
            "save_track",
            Some("usecase::track"),
            vec![],
            vec!["()".to_string()],
            false,
            TypeAction::Add,
        );
        let entry_load = free_fn_entry(
            "load_track",
            Some("usecase::track"),
            vec![],
            vec!["()".to_string()],
            false,
            TypeAction::Add,
        );
        let mut functions = HashMap::new();
        functions.insert(
            ("save_track".to_string(), Some("usecase::track".to_string())),
            FunctionNode::new(
                vec![],
                vec!["()".to_string()],
                false,
                Some("usecase::track".to_string()),
            ),
        );
        functions.insert(
            ("load_track".to_string(), Some("usecase::track".to_string())),
            FunctionNode::new(
                vec![],
                vec!["()".to_string()],
                false,
                Some("usecase::track".to_string()),
            ),
        );
        functions.insert(
            ("purge_track".to_string(), Some("usecase::track".to_string())),
            FunctionNode::new(
                vec![],
                vec!["()".to_string()],
                false,
                Some("usecase::track".to_string()),
            ),
        );
        let profile = TypeGraph::with_functions(HashMap::new(), HashMap::new(), functions);
        let results = evaluate_type_signals(&[entry_save, entry_load], &profile, &HashSet::new());
        // Both save_track and load_track are declared → purge_track is the only extra.
        // Both entries share the same extra from the module-scope reverse check.
        for sig in &results {
            assert_eq!(
                sig.signal(),
                ConfidenceSignal::Red,
                "add × reverse extra must be Red for entry '{}'",
                sig.type_name()
            );
            assert!(
                sig.extra_items().iter().any(|e| e.contains("purge_track")),
                "extra_items for '{}' should mention 'purge_track'",
                sig.type_name()
            );
        }
    }

    // ---------------------------------------------------------------------------
    // T008: FreeFunction Delete action — presence check via get_function
    // ---------------------------------------------------------------------------

    #[test]
    fn test_free_function_delete_blue_when_function_absent() {
        // delete × absent (function removed from code) → Blue (deletion complete).
        let entry = free_fn_entry(
            "old_fn",
            Some("usecase::track"),
            vec![],
            vec!["()".to_string()],
            false,
            TypeAction::Delete,
        );
        // Empty TypeGraph — function is absent.
        let profile = TypeGraph::new(HashMap::new(), HashMap::new());
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
        assert!(!results[0].found_type());
    }

    #[test]
    fn test_free_function_delete_yellow_when_function_still_present() {
        // delete × present (function still in code) → Yellow (not yet deleted).
        let entry = free_fn_entry(
            "old_fn",
            Some("usecase::track"),
            vec![],
            vec!["()".to_string()],
            false,
            TypeAction::Delete,
        );
        // TypeGraph has the function — it is still present.
        let profile = make_profile_with_fn(
            "old_fn",
            Some("usecase::track"),
            vec![],
            vec!["()".to_string()],
            false,
        );
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(results[0].found_type());
    }

    #[test]
    fn test_free_function_delete_with_undeclared_extra_in_same_module_returns_yellow() {
        // §S: delete × reverse extra → Yellow.
        // A Delete entry has its own function in `declared_in_module` (no false extra),
        // but an undeclared function in the same module must still surface as Yellow.
        let entry = free_fn_entry(
            "old_fn",
            Some("usecase::track"),
            vec![],
            vec!["()".to_string()],
            false,
            TypeAction::Delete,
        );
        // TypeGraph has "old_fn" (declared, being deleted) and "extra_fn" (undeclared).
        let mut functions = HashMap::new();
        functions.insert(
            ("old_fn".to_string(), Some("usecase::track".to_string())),
            FunctionNode::new(
                vec![],
                vec!["()".to_string()],
                false,
                Some("usecase::track".to_string()),
            ),
        );
        functions.insert(
            ("extra_fn".to_string(), Some("usecase::track".to_string())),
            FunctionNode::new(
                vec![],
                vec!["()".to_string()],
                false,
                Some("usecase::track".to_string()),
            ),
        );
        let profile = TypeGraph::with_functions(HashMap::new(), HashMap::new(), functions);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        // "old_fn" still present → Yellow (not yet deleted) + "extra_fn" reverse extra → Yellow.
        // Most-severe of Yellow + Yellow = Yellow.
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(
            results[0].extra_items().iter().any(|e| e.contains("extra_fn")),
            "extra_items should mention undeclared 'extra_fn' even for delete entries"
        );
    }

    // ---------------------------------------------------------------------------
    // T008 AC-06: FreeFunction with module_path=None (top-level scope)
    // ---------------------------------------------------------------------------

    #[test]
    fn test_free_function_none_module_path_blue_when_signature_matches() {
        // AC-06: FreeFunction at top level (module_path=None) matches → Blue.
        let entry =
            free_fn_entry("top_fn", None, vec![], vec!["()".to_string()], false, TypeAction::Add);
        let profile = make_profile_with_fn("top_fn", None, vec![], vec!["()".to_string()], false);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
        assert!(results[0].missing_items().is_empty());
        assert!(results[0].extra_items().is_empty());
    }

    #[test]
    fn test_free_function_none_module_path_yellow_when_absent() {
        // AC-06: Top-level function absent while action=add → forward miss → Yellow.
        let entry =
            free_fn_entry("top_fn", None, vec![], vec!["()".to_string()], false, TypeAction::Add);
        let profile = TypeGraph::new(HashMap::new(), HashMap::new());
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(!results[0].missing_items().is_empty());
    }

    #[test]
    fn test_free_function_none_module_path_reverse_extra_returns_red_for_add() {
        // CN-07: Top-level module (None) scope — extra function not declared → Red (add).
        let entry =
            free_fn_entry("top_fn", None, vec![], vec!["()".to_string()], false, TypeAction::Add);
        let mut functions = HashMap::new();
        functions.insert(
            ("top_fn".to_string(), None),
            FunctionNode::new(vec![], vec!["()".to_string()], false, None),
        );
        functions.insert(
            ("extra_top_fn".to_string(), None),
            FunctionNode::new(vec![], vec!["()".to_string()], false, None),
        );
        let profile = TypeGraph::with_functions(HashMap::new(), HashMap::new(), functions);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
        assert!(
            results[0].extra_items().iter().any(|e| e.contains("extra_top_fn")),
            "top-level extra must be flagged"
        );
    }

    #[test]
    fn test_free_function_none_module_path_delete_blue_when_absent() {
        // delete × absent at top level → Blue (deletion complete).
        let entry = free_fn_entry(
            "top_fn",
            None,
            vec![],
            vec!["()".to_string()],
            false,
            TypeAction::Delete,
        );
        let profile = TypeGraph::new(HashMap::new(), HashMap::new());
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Blue);
        assert!(!results[0].found_type());
    }

    #[test]
    fn test_free_function_none_module_path_delete_yellow_when_still_present() {
        // delete × present at top level → Yellow (not yet deleted).
        let entry = free_fn_entry(
            "top_fn",
            None,
            vec![],
            vec!["()".to_string()],
            false,
            TypeAction::Delete,
        );
        let profile = make_profile_with_fn("top_fn", None, vec![], vec!["()".to_string()], false);
        let results = evaluate_type_signals(&[entry], &profile, &HashSet::new());
        assert_eq!(results[0].signal(), ConfidenceSignal::Yellow);
        assert!(results[0].found_type());
    }
}
