//! External crate helpers for Phase 1 S/D scope construction.
//!
//! Builds per-scope `external_crates` maps and patches `ItemSummary.crate_id`
//! values so S and D each have their own independent crate-id numbering.

use std::collections::{BTreeSet, HashMap, HashSet};

use rustdoc_types::{
    Crate, ExternalCrate, GenericArg, GenericArgs, GenericBound, Id, Item, ItemSummary, Type,
};

use super::collect_refs::collect_type_refs_from_item;
use super::is_local_unresolved_path;
use crate::tddd::type_ref_parser::UNRESOLVED_CRATE_ID;

/// Builds per-scope `external_crates` and a crate-name → new-crate-id mapping.
///
/// Collects crate names from B's `external_crates` that are referenced by items
/// in the scope, then assigns fresh `crate_id`s starting from 1 (0 is self).
///
/// `extra_a` is an optional reference to the A-side catalogue crate.  Pass `Some(a_krate)`
/// when building S (which may contain A-sourced items that reference external types via
/// A-side synthetic Ids not present in `b.paths`); pass `None` when building D (B-only items).
///
/// `a_side_ids` is the set of path `Id`s known to be A-sourced (force-inserted from
/// `a_krate.paths`).  Pass `Some(&a_side_path_ids)` when building S so that A-sourced
/// paths entries are resolved via `extra_a.external_crates` (A's crate_id namespace)
/// rather than `b.external_crates` (B's independent namespace).  Pass `None` when
/// building D (no A-sourced entries).
///
/// Returns `(external_crates_map, name_to_new_crate_id)`.  The caller must use
/// `name_to_new_crate_id` to patch up `ItemSummary.crate_id` values in the
/// scope's `paths` map so that they reference the new IDs, not B's IDs.
pub(super) fn build_external_crates_for_scope(
    index: &HashMap<Id, Item>,
    paths: &HashMap<Id, ItemSummary>,
    b: &Crate,
    extra_a: Option<&Crate>,
    a_side_ids: Option<&HashSet<Id>>,
) -> (HashMap<u32, ExternalCrate>, HashMap<String, u32>) {
    // Collect external crate names referenced in this scope's type fields.
    // Use a HashSet internally for O(1) insert; then sort for deterministic id assignment.
    let mut referenced_crate_names_set: HashSet<String> = HashSet::new();

    // Check paths entries for external items (crate_id != 0).
    // A-side paths use `extra_a.external_crates` for crate_id resolution;
    // B-side paths use `b.external_crates`.  The two tables use independent
    // numeric namespaces and must not be mixed.
    for (id, summary) in paths {
        if summary.crate_id != 0 {
            let is_a_side = a_side_ids.is_some_and(|set| set.contains(id));
            if is_a_side {
                if let Some(a) = extra_a {
                    if let Some(ext) = a.external_crates.get(&summary.crate_id) {
                        referenced_crate_names_set.insert(ext.name.clone());
                    }
                }
            } else if let Some(ext) = b.external_crates.get(&summary.crate_id) {
                referenced_crate_names_set.insert(ext.name.clone());
            }
        }
    }

    // Also scan items for ResolvedPath ids that reference non-local items.
    for item in index.values() {
        for ty in collect_type_refs_from_item(item) {
            collect_external_crate_names_from_type(
                &ty,
                b,
                extra_a,
                &mut referenced_crate_names_set,
            );
        }
    }

    // Sort names so that crate-id assignment is deterministic across runs.
    let referenced_crate_names: BTreeSet<String> = referenced_crate_names_set.into_iter().collect();

    // Build fresh crate_id map and a name → new-id lookup.
    let mut result: HashMap<u32, ExternalCrate> = HashMap::new();
    let mut name_to_new_id: HashMap<String, u32> = HashMap::new();
    let mut next_id: u32 = 1;
    for name in &referenced_crate_names {
        result.insert(
            next_id,
            ExternalCrate {
                name: name.clone(),
                html_root_url: None,
                path: std::path::PathBuf::new(),
            },
        );
        name_to_new_id.insert(name.clone(), next_id);
        next_id += 1;
    }
    (result, name_to_new_id)
}

/// Patches the `crate_id` values in a `paths` map so that they reference the
/// new per-scope IDs rather than B's original IDs.
///
/// B's old-crate-id → crate-name lookup is performed via `b.external_crates`;
/// the name is then mapped to the new ID via `name_to_new_id`.
///
/// `exclude_ids`: if `Some`, only entries whose `Id` key is **not** in the set are
/// patched.  Pass the set of A-side path Ids to avoid mis-mapping A-sourced entries
/// through B's external-crate numbering (A and B use independent `crate_id` spaces
/// and share no numbering contract).
pub(super) fn patch_paths_crate_ids(
    paths: &mut HashMap<Id, ItemSummary>,
    b: &Crate,
    name_to_new_id: &HashMap<String, u32>,
    exclude_ids: Option<&HashSet<Id>>,
) {
    for (id, summary) in paths.iter_mut() {
        if let Some(excl) = exclude_ids {
            if excl.contains(id) {
                continue;
            }
        }
        if summary.crate_id != 0 {
            if let Some(ext) = b.external_crates.get(&summary.crate_id) {
                if let Some(&new_id) = name_to_new_id.get(&ext.name) {
                    summary.crate_id = new_id;
                }
            }
        }
    }
}

/// Patches the `crate_id` values in a `paths` map for entries that came from
/// an extra source crate (e.g. `a_krate`).
///
/// A-side path summaries use `a_krate.external_crates` for crate-id lookup.
/// Pass `only_ids = Some(&a_side_path_ids)` to restrict patching to entries that
/// are known to be A-sourced, so that B-sourced entries (which may coincidentally
/// share the same numeric `crate_id` from B's independent id space) are left for
/// `patch_paths_crate_ids` to handle via B's table.
pub(super) fn patch_paths_crate_ids_extra(
    paths: &mut HashMap<Id, ItemSummary>,
    extra: &Crate,
    name_to_new_id: &HashMap<String, u32>,
    only_ids: Option<&HashSet<Id>>,
) {
    for (id, summary) in paths.iter_mut() {
        if let Some(only) = only_ids {
            if !only.contains(id) {
                continue;
            }
        }
        if summary.crate_id != 0 {
            if let Some(ext) = extra.external_crates.get(&summary.crate_id) {
                if let Some(&new_id) = name_to_new_id.get(&ext.name) {
                    summary.crate_id = new_id;
                }
            }
        }
    }
}

/// Collects external crate names from a resolved or unresolved trait path.
///
/// Handles A-sourced (`UNRESOLVED_CRATE_ID`), B-sourced (id in `b.paths`), and A-sourced
/// non-sentinel (id in `extra_a.paths` with `crate_id != 0`) paths.
fn collect_ext_crate_name_from_path(
    id: &Id,
    path: &str,
    args: Option<&GenericArgs>,
    b: &Crate,
    extra_a: Option<&Crate>,
    out: &mut HashSet<String>,
) {
    if *id == Id(UNRESOLVED_CRATE_ID) {
        if !is_local_unresolved_path(path) {
            if let Some(first_seg) = path.split("::").next() {
                if !first_seg.is_empty() {
                    out.insert(first_seg.to_string());
                }
            }
        }
    } else {
        // Non-sentinel id: check b.paths and a.paths independently, adding external crate
        // names from both sources.  The id may appear in b.paths as a B-local item (crate_id==0)
        // and simultaneously in a.paths as an A-side external ref (crate_id!=0) — these are
        // independent id spaces, so both must be consulted.
        if let Some(summary) = b.paths.get(id) {
            if summary.crate_id != 0 {
                if let Some(ext) = b.external_crates.get(&summary.crate_id) {
                    out.insert(ext.name.clone());
                }
            }
        }
        if let Some(a) = extra_a {
            if let Some(a_summary) = a.paths.get(id) {
                if a_summary.crate_id != 0 {
                    if let Some(ext) = a.external_crates.get(&a_summary.crate_id) {
                        out.insert(ext.name.clone());
                    }
                }
            }
        }
    }
    if let Some(a) = args {
        collect_ext_crate_names_from_generic_args(a, b, extra_a, out);
    }
}

/// Recursively scans HRTB `generic_params` (from `TraitBound`, `PolyTrait`, or
/// `WherePredicate`) for external crate names in type bounds and their nested binders.
fn collect_ext_crate_names_from_hrtb_params(
    generic_params: &[rustdoc_types::GenericParamDef],
    b: &Crate,
    extra_a: Option<&Crate>,
    out: &mut HashSet<String>,
) {
    use rustdoc_types::GenericParamDefKind;
    for param in generic_params {
        if let GenericParamDefKind::Type { bounds, default, .. } = &param.kind {
            for bound in bounds {
                if let GenericBound::TraitBound { trait_, generic_params: nested, .. } = bound {
                    collect_ext_crate_name_from_path(
                        &trait_.id,
                        &trait_.path,
                        trait_.args.as_deref(),
                        b,
                        extra_a,
                        out,
                    );
                    // Recurse into the nested HRTB binder of this TraitBound.
                    collect_ext_crate_names_from_hrtb_params(nested, b, extra_a, out);
                }
            }
            if let Some(default_ty) = default {
                collect_external_crate_names_from_type(default_ty, b, extra_a, out);
            }
        }
    }
}

fn collect_external_crate_names_from_type(
    ty: &Type,
    b: &Crate,
    extra_a: Option<&Crate>,
    out: &mut HashSet<String>,
) {
    match ty {
        Type::ResolvedPath(p) => {
            collect_ext_crate_name_from_path(&p.id, &p.path, p.args.as_deref(), b, extra_a, out);
        }
        Type::BorrowedRef { type_: inner, .. } => {
            collect_external_crate_names_from_type(inner, b, extra_a, out);
        }
        Type::Slice(inner) => collect_external_crate_names_from_type(inner, b, extra_a, out),
        Type::Array { type_: inner, .. } => {
            collect_external_crate_names_from_type(inner, b, extra_a, out);
        }
        Type::Tuple(tys) => {
            for t in tys {
                collect_external_crate_names_from_type(t, b, extra_a, out);
            }
        }
        Type::FunctionPointer(fp) => {
            for (_, t) in &fp.sig.inputs {
                collect_external_crate_names_from_type(t, b, extra_a, out);
            }
            if let Some(ret) = &fp.sig.output {
                collect_external_crate_names_from_type(ret, b, extra_a, out);
            }
            // Scan HRTB `generic_params` (recursively) for external type references.
            collect_ext_crate_names_from_hrtb_params(&fp.generic_params, b, extra_a, out);
        }
        Type::QualifiedPath { self_type, trait_, args, .. } => {
            collect_external_crate_names_from_type(self_type, b, extra_a, out);
            if let Some(args) = args {
                collect_ext_crate_names_from_generic_args(args, b, extra_a, out);
            }
            if let Some(p) = trait_ {
                collect_ext_crate_name_from_path(
                    &p.id,
                    &p.path,
                    p.args.as_deref(),
                    b,
                    extra_a,
                    out,
                );
            }
        }
        Type::ImplTrait(bounds) => {
            for bound in bounds {
                if let GenericBound::TraitBound { trait_, generic_params: nested, .. } = bound {
                    collect_ext_crate_name_from_path(
                        &trait_.id,
                        &trait_.path,
                        trait_.args.as_deref(),
                        b,
                        extra_a,
                        out,
                    );
                    // Recurse into the nested HRTB binder of this TraitBound.
                    collect_ext_crate_names_from_hrtb_params(nested, b, extra_a, out);
                }
            }
        }
        Type::DynTrait(dyn_trait) => {
            for pt in &dyn_trait.traits {
                let p = &pt.trait_;
                collect_ext_crate_name_from_path(
                    &p.id,
                    &p.path,
                    p.args.as_deref(),
                    b,
                    extra_a,
                    out,
                );
                // Recurse into the nested HRTB binder (PolyTrait.generic_params).
                collect_ext_crate_names_from_hrtb_params(&pt.generic_params, b, extra_a, out);
            }
        }
        Type::RawPointer { type_: inner, .. } => {
            collect_external_crate_names_from_type(inner, b, extra_a, out);
        }
        Type::Pat { type_: inner, .. } => {
            collect_external_crate_names_from_type(inner, b, extra_a, out);
        }
        _ => {}
    }
}

fn collect_ext_crate_names_from_generic_args(
    args: &GenericArgs,
    b: &Crate,
    extra_a: Option<&Crate>,
    out: &mut HashSet<String>,
) {
    match args {
        GenericArgs::AngleBracketed { args: ga, constraints } => {
            for arg in ga {
                if let GenericArg::Type(t) = arg {
                    collect_external_crate_names_from_type(t, b, extra_a, out);
                }
            }
            use rustdoc_types::{AssocItemConstraintKind, Term};
            for c in constraints {
                match &c.binding {
                    AssocItemConstraintKind::Equality(Term::Type(ty)) => {
                        collect_external_crate_names_from_type(ty, b, extra_a, out);
                    }
                    AssocItemConstraintKind::Constraint(bounds) => {
                        for bound in bounds {
                            if let GenericBound::TraitBound {
                                trait_, generic_params: nested, ..
                            } = bound
                            {
                                collect_ext_crate_name_from_path(
                                    &trait_.id,
                                    &trait_.path,
                                    trait_.args.as_deref(),
                                    b,
                                    extra_a,
                                    out,
                                );
                                // Recurse into nested HRTB binders on the constraint bound.
                                collect_ext_crate_names_from_hrtb_params(nested, b, extra_a, out);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        GenericArgs::Parenthesized { inputs, output } => {
            for t in inputs {
                collect_external_crate_names_from_type(t, b, extra_a, out);
            }
            if let Some(ret) = output {
                collect_external_crate_names_from_type(ret, b, extra_a, out);
            }
        }
        _ => {}
    }
}
