//! Structural equality helpers for Phase 2 item comparison.
//!
//! Compares `rustdoc_types` items for structural equality, ignoring docs and
//! parameter binding names.  Used in Phase 2 to decide whether S-side and C-side
//! items are identical (determining the `Match` / `Mismatch` sub-region).
//!
//! Generics, function, and trait helpers live in the sibling `generics_eq` module.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rustdoc_types::{Id, Item, ItemEnum};

use super::format::{format_type, format_type_strip_type_params};
use super::generics_eq::{
    fn_sigs_structurally_equal, generics_structurally_equal, traits_structurally_equal,
};

// Re-export so callers (tests, phase2) can access via this module path.
pub(super) use super::generics_eq::build_trait_method_map;

/// Returns `true` if two items are structurally equal (same type/trait/function shape).
///
/// Comparison ignores docs and parameter binding names; only structural fields
/// (field types, enum variant shapes, function signatures) are compared.
///
/// Requires `a_index` / `b_index` to resolve child items (fields, variants,
/// trait methods) by their Ids.  This avoids false mismatches from comparing
/// graph-local Ids across different Id spaces (S vs C).
///
/// Type comparison uses `format_type` (L1 short-name string representation) so
/// A-derived and rustdoc-derived items compare symmetrically.
pub(super) fn items_structurally_equal(
    a: &Item,
    b: &Item,
    a_index: &HashMap<Id, Item>,
    b_index: &HashMap<Id, Item>,
    _crate_name: &str,
) -> bool {
    match (&a.inner, &b.inner) {
        (ItemEnum::Struct(sa), ItemEnum::Struct(sb)) => {
            structs_structurally_equal(sa, sb, a_index, b_index)
        }
        (ItemEnum::Enum(ea), ItemEnum::Enum(eb)) => {
            enums_structurally_equal(ea, eb, a_index, b_index)
        }
        (ItemEnum::TypeAlias(ta), ItemEnum::TypeAlias(tb)) => {
            format_type(&ta.type_) == format_type(&tb.type_)
                && generics_structurally_equal(&ta.generics, &tb.generics)
        }
        (ItemEnum::Trait(ta), ItemEnum::Trait(tb)) => {
            traits_structurally_equal(ta, tb, a_index, b_index)
        }
        (ItemEnum::Function(fa), ItemEnum::Function(fb)) => fn_sigs_structurally_equal(
            &fa.sig,
            &fb.sig,
            &fa.header,
            &fb.header,
            &fa.generics,
            &fb.generics,
        ),
        // For trait impls: compare for_ + trait path (identity), header flags, and
        // generics only.  Method-map comparison is intentionally omitted per ADR D9:
        // the catalogue declares "which traits are implemented" without recording
        // method bodies or the provenance of the implementation
        // (`#[derive(...)]`-generated vs hand-written).  A `#[derive(Default)]` and
        // a hand-written `impl Default { fn default() -> Self { Self::new() } }` are
        // structurally equal from the catalogue's perspective — both implement the
        // same trait contract.
        //
        // S-side (A-codec): generics embedded in path string with `args: None`
        //   e.g. Path { path: "core::convert::From<CatalogueLoaderError>", args: None }
        // C-side (rustdoc): base path in `path`, generics in `args`
        //   e.g. Path { path: "From", args: Some(AngleBracketed(["CatalogueLoaderError"])) }
        //
        // Reducing both to `"From<CatalogueLoaderError>"` produces equality.
        (ItemEnum::Impl(ia), ItemEnum::Impl(ib)) => {
            use super::format::format_generic_args;
            if ia.is_unsafe != ib.is_unsafe || ia.is_negative != ib.is_negative {
                return false;
            }
            // Strip impl-block generic params from `for_` before comparing so that
            // `impl<S> TaskOperationInteractor<S>: TaskOperationService` (C-side, where
            // `for_` carries the generic arg `<S>`) matches the A-codec S-side entry
            // where `for_` is the bare name `TaskOperationInteractor` (no impl-block
            // params are encoded by the catalogue codec for trait impls).
            //
            // We take the union of both sides' generic param names (type, lifetime, and
            // const params) and strip them from both `for_` types.  On the A-codec side
            // the impl generics list is empty, so the union equals the C-side params —
            // stripping with an empty set is a no-op (falls through to `format_type`).
            // This is safe because the identity-key lookup already confirmed that S and C
            // refer to the same implementation.
            let type_params: BTreeSet<String> = ia
                .generics
                .params
                .iter()
                .chain(ib.generics.params.iter())
                .map(|p| p.name.clone())
                .collect();
            let for_a = if type_params.is_empty() {
                format_type(&ia.for_)
            } else {
                format_type_strip_type_params(&ia.for_, &type_params)
            };
            let for_b = if type_params.is_empty() {
                format_type(&ib.for_)
            } else {
                format_type_strip_type_params(&ib.for_, &type_params)
            };
            let for_equal = for_a == for_b;
            // Normalize the trait path to a short-name form for structural equality.
            //
            // Identity-key matching (in `build_impl_identity_map`) already confirmed that
            // S and C refer to the same trait — using `krate.paths` for disambiguation.
            // Here we only need to confirm they are the same trait (not identical path
            // strings), so we reduce to the short name + generic args.
            let normalize_trait_path = |p: &rustdoc_types::Path| {
                // Strip module prefix to get the short base name (last segment).
                // Also strip any generic suffix embedded in the path string so we can
                // compare it with the `args`-derived suffix.
                let last_seg = p.path.rsplit("::").next().unwrap_or(p.path.as_str());
                // A-side codec form: generics embedded in the path string with args = None.
                // Keep the entire last segment (including `<...>` suffix) as-is.
                if p.args.is_none() {
                    last_seg.to_string()
                } else {
                    // C-side rustdoc form: base in path, generics in args.
                    let base = last_seg.split('<').next().unwrap_or(last_seg);
                    let rendered = p.args.as_deref().map_or(String::new(), format_generic_args);
                    if rendered.is_empty() {
                        base.to_string()
                    } else {
                        format!("{base}<{rendered}>")
                    }
                }
            };
            let trait_equal = ia.trait_.as_ref().map(normalize_trait_path)
                == ib.trait_.as_ref().map(normalize_trait_path);
            if !for_equal || !trait_equal {
                return false;
            }
            // Impl-block generics (e.g. `impl<S>`) are intentionally not compared here.
            // The catalogue codec (`TraitImplDeclV2`) does not encode impl-block type
            // parameters — they are identity-neutral for the purpose of declaring "which
            // trait is implemented on which type".  The identity-key lookup (in
            // `build_impl_identity_map`) already stripped them from `for_` so that
            // `impl<S> TaskOperationInteractor<S>: TaskOperationService` and
            // `impl TaskOperationInteractor: TaskOperationService` produce the same key.
            // Comparing impl-block generics here would always fail for generic impls
            // (A-side: empty, C-side: `[S]`) and produce spurious Yellow signals.
            true
        }
        _ => false,
    }
}

/// Builds a merged `method_name → sig_str` map for all inherent impl blocks
/// (impl blocks without a trait) of a type.
///
/// Inherent methods are not independently evaluated in Phase 2's impl identity
/// map (see `build_impl_identity_map` doc comment), so type structural equality
/// must cover them to detect changes in `TypeEntry.methods` (catalogue-declared
/// inherent methods encoded as inherent impl items by the codec).
fn build_inherent_method_map(
    impl_ids: &[Id],
    index: &HashMap<Id, Item>,
) -> (BTreeMap<String, String>, bool) {
    let mut merged = BTreeMap::new();
    let mut any_unsupported = false;
    for impl_id in impl_ids {
        if let Some(impl_item) = index.get(impl_id) {
            if let ItemEnum::Impl(impl_) = &impl_item.inner {
                // Only inherent impls (no trait).
                if impl_.trait_.is_some() {
                    continue;
                }
                let (methods, has_unsupported) =
                    build_trait_method_map(&impl_.items, index, Some(&impl_.generics));
                if has_unsupported {
                    any_unsupported = true;
                }
                for (name, sig) in methods {
                    merged.insert(name, sig);
                }
            }
        }
    }
    (merged, any_unsupported)
}

fn structs_structurally_equal(
    a: &rustdoc_types::Struct,
    b: &rustdoc_types::Struct,
    a_index: &HashMap<Id, Item>,
    b_index: &HashMap<Id, Item>,
) -> bool {
    // The A-codec (`encode_plain_struct`) always uses `empty_generics()` for struct
    // items — it never encodes struct-level generic parameters.  Comparing
    // `a.generics` (always empty) against `b.generics` (may carry `<S: ...>`) would
    // unconditionally fail for generic structs and produce spurious Yellow signals.
    // Skip the struct-level generics check when A-side params are empty.
    if !a.generics.params.is_empty() && !generics_structurally_equal(&a.generics, &b.generics) {
        return false;
    }
    // Compare inherent method signatures so that adding, removing, or changing an
    // inherent method (TypeEntry.methods in the catalogue) registers as a mismatch.
    //
    // Symmetric comparison: build C-side method map unconditionally and compare both
    // sides, regardless of whether A-side declares any methods.  This matches
    // `enums_structurally_equal` behaviour and closes the TDDD-BUG-03 skip opt-out hole
    // (ADR 2026-05-20-0413 D1).
    //
    // Both A-side (catalogue) and C-side (rustdoc) retain Outlives bounds in their
    // method generics.  Since ADR `2026-05-18-1223` D1, the A-codec no longer rejects
    // Outlives bounds (`validate_supported_bound` removed in T002) and
    // `strip_outlives_from_index` has been removed (T003).  Catalogue authors can now
    // declare `<F: Fn(...) + Send + Sync + 'static>` directly; both sides carry the
    // same lifetime bounds and produce symmetric fingerprints via
    // `generics_structurally_equal` / `build_generics_fingerprint_with_combined_canon`.
    let (a_methods, a_methods_unsupported) = build_inherent_method_map(&a.impls, a_index);
    let (b_methods, b_methods_unsupported) = build_inherent_method_map(&b.impls, b_index);
    // D3 fail-closed: any method with unsupported generics on either side.
    if a_methods_unsupported || b_methods_unsupported {
        return false;
    }
    if a_methods != b_methods {
        return false;
    }
    use rustdoc_types::StructKind;
    match (&a.kind, &b.kind) {
        (StructKind::Unit, StructKind::Unit) => true,
        (StructKind::Tuple(af), StructKind::Tuple(bf)) => {
            // Compare field types by position, including `None` slots.
            //
            // Tuple field positions are part of the public API: a `pub` field at index `.1`
            // vs `.0` is a semantic difference. The catalogue (S-side) does NOT add `None`
            // placeholder slots for private fields (since it cannot know their positions).
            // Structs with private fields will therefore always have a different vector
            // length from the C-side (rustdoc) representation: this is the intended
            // fail-closed behaviour — they produce a Mismatch rather than a false Blue.
            af.len() == bf.len()
                && af.iter().zip(bf.iter()).all(|(a_opt, b_opt)| match (a_opt, b_opt) {
                    (Some(aid), Some(bid)) => field_types_equal(aid, bid, a_index, b_index),
                    (None, None) => true,
                    _ => false,
                })
        }
        (
            StructKind::Plain { fields: af, has_stripped_fields: asf },
            StructKind::Plain { fields: bf, has_stripped_fields: bsf },
        ) => {
            // A struct with stripped (hidden) fields cannot compare equal to one without.
            // Including this flag prevents a rustdoc-truncated shape from matching a
            // fully-visible empty/unit shape.
            if asf != bsf {
                return false;
            }
            // Compare named fields: same count, same names, same types (order-insensitive).
            if af.len() != bf.len() {
                return false;
            }
            // Build name → type-string maps for both sides then compare.
            let a_field_map = build_field_name_type_map(af, a_index);
            let b_field_map = build_field_name_type_map(bf, b_index);
            a_field_map == b_field_map
        }
        _ => false,
    }
}

/// Returns `true` if the tuple-field items at `a_id` and `b_id` have the same type.
fn field_types_equal(
    a_id: &Id,
    b_id: &Id,
    a_index: &HashMap<Id, Item>,
    b_index: &HashMap<Id, Item>,
) -> bool {
    let a_ty = match a_index.get(a_id) {
        Some(item) => match &item.inner {
            ItemEnum::StructField(ty) => format_type(ty),
            _ => return false,
        },
        None => return false,
    };
    let b_ty = match b_index.get(b_id) {
        Some(item) => match &item.inner {
            ItemEnum::StructField(ty) => format_type(ty),
            _ => return false,
        },
        None => return false,
    };
    a_ty == b_ty
}

/// Builds a `name → format_type(field_type)` map from a list of field Ids.
fn build_field_name_type_map(
    field_ids: &[Id],
    index: &HashMap<Id, Item>,
) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for id in field_ids {
        if let Some(item) = index.get(id) {
            if let Some(name) = &item.name {
                let ty_str = match &item.inner {
                    ItemEnum::StructField(ty) => format_type(ty),
                    _ => continue,
                };
                map.insert(name.clone(), ty_str);
            }
        }
    }
    map
}

fn enums_structurally_equal(
    a: &rustdoc_types::Enum,
    b: &rustdoc_types::Enum,
    a_index: &HashMap<Id, Item>,
    b_index: &HashMap<Id, Item>,
) -> bool {
    if !generics_structurally_equal(&a.generics, &b.generics) {
        return false;
    }
    // `has_stripped_variants` means some variants were excluded from rustdoc output.
    // If one side has stripped variants and the other does not, the full variant set
    // may differ — treat as structurally unequal to avoid false Blue signals.
    if a.has_stripped_variants != b.has_stripped_variants {
        return false;
    }
    if a.variants.len() != b.variants.len() {
        return false;
    }
    // Compare variant names (and their kind) in sorted order.
    let a_variants = build_variant_shape_map(&a.variants, a_index);
    let b_variants = build_variant_shape_map(&b.variants, b_index);
    if a_variants != b_variants {
        return false;
    }
    // Compare inherent method signatures so that adding, removing, or changing an
    // inherent method (TypeEntry.methods in the catalogue) registers as a mismatch.
    let (a_methods, a_methods_unsupported) = build_inherent_method_map(&a.impls, a_index);
    let (b_methods, b_methods_unsupported) = build_inherent_method_map(&b.impls, b_index);
    // D3 fail-closed: any method with unsupported generics on either side.
    if a_methods_unsupported || b_methods_unsupported {
        return false;
    }
    a_methods == b_methods
}

/// Builds a `variant_name → shape_string` map for enum variants.
///
/// The shape string captures the variant kind (unit / tuple field-type-list /
/// struct field-name:type-pairs) using `format_type` for type strings.
fn build_variant_shape_map(
    variant_ids: &[Id],
    index: &HashMap<Id, Item>,
) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for id in variant_ids {
        if let Some(item) = index.get(id) {
            if let Some(name) = &item.name {
                let shape = match &item.inner {
                    ItemEnum::Variant(v) => {
                        let kind_str = format_variant_kind(&v.kind, index);
                        // Include the explicit discriminant so that `A = 1` vs `A = 2`
                        // produce different shape strings and register as a mismatch.
                        match &v.discriminant {
                            Some(d) => format!("{kind_str}={}", d.expr.replace("::", ".")),
                            None => kind_str,
                        }
                    }
                    _ => continue,
                };
                map.insert(name.clone(), shape);
            }
        }
    }
    map
}

/// Formats an enum variant kind as a deterministic string for structural comparison.
fn format_variant_kind(kind: &rustdoc_types::VariantKind, index: &HashMap<Id, Item>) -> String {
    use rustdoc_types::VariantKind;
    match kind {
        VariantKind::Plain => "plain".to_string(),
        VariantKind::Tuple(opt_ids) => {
            // Preserve `None` slots as `_` so that a variant with stripped/hidden
            // fields does not compare equal to a shorter variant.
            let types: Vec<String> = opt_ids
                .iter()
                .map(|opt| match opt {
                    None => "_".to_string(),
                    Some(id) => index
                        .get(id)
                        .and_then(|item| match &item.inner {
                            ItemEnum::StructField(ty) => Some(format_type(ty)),
                            _ => None,
                        })
                        .unwrap_or_else(|| "_".to_string()),
                })
                .collect();
            format!("tuple({})", types.join(","))
        }
        VariantKind::Struct { fields, has_stripped_fields } => {
            let mut field_map: BTreeMap<String, String> = BTreeMap::new();
            for id in fields {
                if let Some(item) = index.get(id) {
                    if let Some(name) = &item.name {
                        if let ItemEnum::StructField(ty) = &item.inner {
                            field_map.insert(name.clone(), format_type(ty));
                        }
                    }
                }
            }
            let entries: Vec<String> = field_map.iter().map(|(n, t)| format!("{n}:{t}")).collect();
            // Include the stripped-fields marker so a variant with hidden fields does
            // not compare equal to a fully-visible variant with the same field set.
            let stripped = if *has_stripped_fields { ",..stripped" } else { "" };
            format!("struct{{{}{}}}", entries.join(","), stripped)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::collections::HashMap;

    use rustdoc_types::{
        FunctionHeader, FunctionSignature, GenericArg, GenericArgs, GenericParamDef,
        GenericParamDefKind, Generics, Id, Impl, Item, ItemEnum, Path, Struct, StructKind, Type,
        Visibility,
    };

    use super::{items_structurally_equal, structs_structurally_equal};
    use crate::tddd::signal_evaluator_v2::generics_eq::make_simple_trait_bound as make_trait_bound;

    fn make_struct_field_item(id: Id, ty_str: &str) -> Item {
        Item {
            id,
            crate_id: 0,
            name: None,
            span: None,
            visibility: Visibility::Public,
            docs: None,
            links: std::collections::HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner: ItemEnum::StructField(Type::Primitive(ty_str.to_owned())),
        }
    }

    fn empty_generics() -> Generics {
        Generics { params: vec![], where_predicates: vec![] }
    }

    fn make_tuple_struct(field_ids: Vec<Option<Id>>) -> Struct {
        Struct { kind: StructKind::Tuple(field_ids), generics: empty_generics(), impls: vec![] }
    }

    // Build an index with field items for all Some entries in `field_ids`,
    // paired with `ty_strs` in order (skipping None slots when pairing).
    fn build_index(field_ids: &[Option<Id>], ty_strs: &[&str]) -> HashMap<Id, Item> {
        let mut index = HashMap::new();
        let some_ids: Vec<Id> = field_ids.iter().filter_map(|opt| *opt).collect();
        for (id, ty) in some_ids.iter().zip(ty_strs.iter()) {
            index.insert(*id, make_struct_field_item(*id, ty));
        }
        index
    }

    /// All-public tuple struct: S-side and C-side both have only Some entries.
    /// Types match — must compare equal.
    #[test]
    fn test_tuple_struct_all_public_fields_match() {
        // S-side: [Some(1), Some(2)] — no private fields
        let s_fields: Vec<Option<Id>> = vec![Some(Id(1)), Some(Id(2))];
        let s_index = build_index(&s_fields, &["u32", "String"]);
        let s_struct = make_tuple_struct(s_fields);

        // C-side: [Some(10), Some(11)] — same types in same positions
        let c_fields: Vec<Option<Id>> = vec![Some(Id(10)), Some(Id(11))];
        let c_index = build_index(&c_fields, &["u32", "String"]);
        let c_struct = make_tuple_struct(c_fields);

        assert!(
            structs_structurally_equal(&s_struct, &c_struct, &s_index, &c_index),
            "all-public tuple structs with matching types must compare equal"
        );
    }

    /// S-side adds a trailing None when has_stripped_fields=true.
    /// C-side (code changed to no private fields) has no Nones.
    /// Lengths differ → Mismatch — detects private-field removal.
    #[test]
    fn test_tuple_struct_private_field_removed_does_not_match() {
        // S-side: [Some(1), None] — one public + stripped flag encoded as trailing None
        let s_fields: Vec<Option<Id>> = vec![Some(Id(1)), None];
        let s_index = build_index(&s_fields, &["String"]);
        let s_struct = make_tuple_struct(s_fields);

        // C-side: [Some(10)] — code now has no private fields
        let c_fields: Vec<Option<Id>> = vec![Some(Id(10))];
        let c_index = build_index(&c_fields, &["String"]);
        let c_struct = make_tuple_struct(c_fields);

        // Lengths differ (2 vs 1) → Mismatch, preventing false Blue on private-field removal.
        assert!(
            !structs_structurally_equal(&s_struct, &c_struct, &s_index, &c_index),
            "S-side trailing-None vs C-side with no None must not match (detects removal)"
        );
    }

    /// Different public field types must not match.
    #[test]
    fn test_tuple_struct_different_field_types_does_not_match() {
        // S-side: [Some(1)] type=u32
        let s_fields: Vec<Option<Id>> = vec![Some(Id(1))];
        let s_index = build_index(&s_fields, &["u32"]);
        let s_struct = make_tuple_struct(s_fields);

        // C-side: [Some(10)] type=String
        let c_fields: Vec<Option<Id>> = vec![Some(Id(10))];
        let c_index = build_index(&c_fields, &["String"]);
        let c_struct = make_tuple_struct(c_fields);

        assert!(
            !structs_structurally_equal(&s_struct, &c_struct, &s_index, &c_index),
            "different field types must not match"
        );
    }

    // -----------------------------------------------------------------------
    // ADR D13 / IN-27: cross-crate ref structural equality (shape-based,
    // L1 short-name reduction independent of full path length / id values)
    // -----------------------------------------------------------------------

    /// Helper: build a StructField item whose type is a `Type::ResolvedPath` with
    /// the given full path and per-graph id. Mirrors what `parse_type_ref` +
    /// `resolve_external_type_ids` produces for crate-prefixed TypeRefs.
    fn make_struct_field_resolved_path(id: Id, full_path: &str, item_id: Id) -> Item {
        Item {
            id,
            crate_id: 0,
            name: None,
            span: None,
            visibility: Visibility::Public,
            docs: None,
            links: HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner: ItemEnum::StructField(Type::ResolvedPath(Path {
                path: full_path.to_string(),
                id: item_id,
                args: None,
            })),
        }
    }

    /// Per D13: A-side may render a cross-crate ref with a short path (e.g.
    /// `"domain::TypeSignalsDocument"`) while C-side (rustdoc) renders the same
    /// item with the canonical module path (`"domain::tddd::type_signals_doc::TypeSignalsDocument"`).
    /// Per-graph `id` values differ between A and C (A's id is allocated by the
    /// codec, C's by rustdoc). Structural equality must succeed regardless,
    /// because `format_type` reduces to the L1 short name and ignores ids.
    #[test]
    fn test_cross_crate_ref_with_different_path_lengths_and_ids_matches() {
        // A-side (catalogue-derived): "domain::TypeSignalsDocument" with synthetic id 99
        let a_fields: Vec<Option<Id>> = vec![Some(Id(1))];
        let mut a_index: HashMap<Id, Item> = HashMap::new();
        a_index.insert(
            Id(1),
            make_struct_field_resolved_path(Id(1), "domain::TypeSignalsDocument", Id(99)),
        );
        let a_struct = make_tuple_struct(a_fields);

        // C-side (rustdoc): full module path with a different per-graph id
        let c_fields: Vec<Option<Id>> = vec![Some(Id(10))];
        let c_index = {
            let mut idx: HashMap<Id, Item> = HashMap::new();
            idx.insert(
                Id(10),
                make_struct_field_resolved_path(
                    Id(10),
                    "domain::tddd::type_signals_doc::TypeSignalsDocument",
                    Id(7531),
                ),
            );
            idx
        };
        let c_struct = make_tuple_struct(c_fields);

        assert!(
            structs_structurally_equal(&a_struct, &c_struct, &a_index, &c_index),
            "cross-crate refs with different path lengths and differing per-graph ids \
             must still compare equal at L1 short-name (D13 shape-based matching)"
        );
    }

    // -----------------------------------------------------------------------
    // ADR D9: provenance-agnostic trait-impl comparison
    // -----------------------------------------------------------------------

    fn make_item(id: Id, inner: ItemEnum) -> Item {
        Item {
            id,
            crate_id: 0,
            name: None,
            span: None,
            visibility: Visibility::Public,
            docs: None,
            links: std::collections::HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner,
        }
    }

    fn default_fn_item(id: Id) -> Item {
        make_item(
            id,
            ItemEnum::Function(rustdoc_types::Function {
                sig: FunctionSignature { inputs: vec![], output: None, is_c_variadic: false },
                generics: empty_generics(),
                header: FunctionHeader {
                    is_unsafe: false,
                    is_const: false,
                    is_async: false,
                    abi: rustdoc_types::Abi::Rust,
                },
                has_body: true,
            }),
        )
    }

    fn make_impl_item(id: Id, for_name: &str, trait_name: &str, items: Vec<Id>) -> Item {
        make_item(
            id,
            ItemEnum::Impl(Impl {
                is_unsafe: false,
                generics: empty_generics(),
                provided_trait_methods: vec![],
                trait_: Some(Path { path: trait_name.to_string(), id: Id(999), args: None }),
                for_: Type::ResolvedPath(Path {
                    path: for_name.to_string(),
                    id: Id(1),
                    args: None,
                }),
                items,
                is_negative: false,
                is_synthetic: false,
                blanket_impl: None,
            }),
        )
    }

    /// ADR D9: hand-written `impl Default` (S-side, with `default` method in items)
    /// vs `#[derive(Default)]` (C-side, empty items) → structurally equal.
    ///
    /// This is the exact scenario from Issue 2: `InMemoryCatalogueLinter` baseline
    /// had a hand-written `impl Default` with a `default` method; this track replaced
    /// it with `#[derive(Default)]` which produces an impl with empty items list.
    /// Per D9, these are structurally equal because the catalogue only cares that
    /// `Default` is implemented, not how it is implemented.
    #[test]
    fn test_impl_hand_written_default_vs_derive_default_are_structurally_equal() {
        // S-side: hand-written `impl Default for Foo { fn default() -> Self { ... } }`
        // Items list is non-empty (contains the `default` function id).
        let s_default_fn_id = Id(100);
        let s_impl = make_impl_item(Id(10), "Foo", "Default", vec![s_default_fn_id]);
        let mut s_index = HashMap::new();
        s_index.insert(s_default_fn_id, default_fn_item(s_default_fn_id));

        // C-side: `#[derive(Default)]` produces an impl with empty items list.
        let c_impl = make_impl_item(Id(20), "Foo", "Default", vec![]);
        let c_index = HashMap::new();

        assert!(
            items_structurally_equal(&s_impl, &c_impl, &s_index, &c_index, "my_crate"),
            "hand-written impl Default vs #[derive(Default)] must be structurally equal (ADR D9)"
        );
    }

    /// ADR D9 converse: `#[derive(Default)]` (S-side, empty items)
    /// vs hand-written `impl Default` (C-side, with items) → structurally equal.
    #[test]
    fn test_impl_derive_default_vs_hand_written_default_are_structurally_equal() {
        // S-side: `#[derive(Default)]` — empty items list.
        let s_impl = make_impl_item(Id(10), "Foo", "Default", vec![]);
        let s_index = HashMap::new();

        // C-side: hand-written `impl Default` with a `default` method.
        let c_default_fn_id = Id(200);
        let c_impl = make_impl_item(Id(20), "Foo", "Default", vec![c_default_fn_id]);
        let mut c_index = HashMap::new();
        c_index.insert(c_default_fn_id, default_fn_item(c_default_fn_id));

        assert!(
            items_structurally_equal(&s_impl, &c_impl, &s_index, &c_index, "my_crate"),
            "#[derive(Default)] vs hand-written impl Default must be structurally equal (ADR D9)"
        );
    }

    /// Different trait names must NOT be structurally equal.
    #[test]
    fn test_impl_different_trait_names_are_not_equal() {
        let s_impl = make_impl_item(Id(10), "Foo", "Default", vec![]);
        let s_index = HashMap::new();

        let c_impl = make_impl_item(Id(20), "Foo", "Display", vec![]);
        let c_index = HashMap::new();

        assert!(
            !items_structurally_equal(&s_impl, &c_impl, &s_index, &c_index, "my_crate"),
            "impls with different trait names must not be structurally equal"
        );
    }

    /// Different `for_` types must NOT be structurally equal.
    #[test]
    fn test_impl_different_for_types_are_not_equal() {
        let s_impl = make_impl_item(Id(10), "Foo", "Default", vec![]);
        let s_index = HashMap::new();

        let c_impl = make_impl_item(Id(20), "Bar", "Default", vec![]);
        let c_index = HashMap::new();

        assert!(
            !items_structurally_equal(&s_impl, &c_impl, &s_index, &c_index, "my_crate"),
            "impls with different for_ types must not be structurally equal"
        );
    }

    // -----------------------------------------------------------------------
    // T044: Mismatch_Modify generic strip — `impl<S> Foo<S>: Bar` vs `impl Foo: Bar`
    // -----------------------------------------------------------------------

    /// Builds an `Impl` item whose `for_` type carries a single generic type argument
    /// that is an impl-block type parameter (e.g. `impl<S> TaskOperationInteractor<S>`).
    /// This mirrors the C-side rustdoc output for a generic struct impl.
    fn make_impl_item_with_generic_for(
        id: Id,
        for_base: &str,
        generic_param: &str,
        trait_name: &str,
    ) -> Item {
        make_item(
            id,
            ItemEnum::Impl(Impl {
                is_unsafe: false,
                generics: Generics {
                    params: vec![GenericParamDef {
                        name: generic_param.to_string(),
                        kind: GenericParamDefKind::Type {
                            bounds: vec![],
                            default: None,
                            is_synthetic: false,
                        },
                    }],
                    where_predicates: vec![],
                },
                provided_trait_methods: vec![],
                trait_: Some(Path { path: trait_name.to_string(), id: Id(999), args: None }),
                for_: Type::ResolvedPath(Path {
                    path: for_base.to_string(),
                    id: Id(1),
                    args: Some(Box::new(GenericArgs::AngleBracketed {
                        args: vec![GenericArg::Type(Type::Generic(generic_param.to_string()))],
                        constraints: vec![],
                    })),
                }),
                items: vec![],
                is_negative: false,
                is_synthetic: false,
                blanket_impl: None,
            }),
        )
    }

    /// T044 regression: A-codec S-side `impl TaskOperationInteractor: TaskOperationService`
    /// (bare `for_`, no impl-block generics) must compare structurally equal to
    /// C-side `impl<S> TaskOperationInteractor<S>: TaskOperationService`
    /// (generic `for_` with `S` from the impl block).
    ///
    /// Before the fix, `format_type` produced `"TaskOperationInteractor"` for the
    /// S-side and `"TaskOperationInteractor<S>"` for the C-side, causing `for_equal`
    /// to be `false` and emitting a spurious Yellow `SIntersectC_Mismatch_Modify`.
    #[test]
    fn test_impl_generic_for_type_matches_bare_for_type() {
        // S-side (A-codec): bare `for_` with no impl-block generics.
        let s_impl =
            make_impl_item(Id(10), "TaskOperationInteractor", "TaskOperationService", vec![]);
        let s_index = HashMap::new();

        // C-side (rustdoc): `impl<S> TaskOperationInteractor<S>: TaskOperationService`.
        let c_impl = make_impl_item_with_generic_for(
            Id(20),
            "TaskOperationInteractor",
            "S",
            "TaskOperationService",
        );
        let c_index = HashMap::new();

        assert!(
            items_structurally_equal(&s_impl, &c_impl, &s_index, &c_index, "my_crate"),
            "bare `impl Foo: Bar` must be structurally equal to `impl<S> Foo<S>: Bar` \
             (impl-block generics are identity-neutral; T044 regression)"
        );
    }

    /// Converse: C-side bare vs S-side generic — must also compare equal.
    #[test]
    fn test_impl_generic_for_type_matches_bare_for_type_reversed() {
        // S-side: generic `for_` with impl-block param.
        let s_impl = make_impl_item_with_generic_for(
            Id(10),
            "TaskOperationInteractor",
            "S",
            "TaskOperationService",
        );
        let s_index = HashMap::new();

        // C-side: bare (no impl-block generics).
        let c_impl =
            make_impl_item(Id(20), "TaskOperationInteractor", "TaskOperationService", vec![]);
        let c_index = HashMap::new();

        assert!(
            items_structurally_equal(&s_impl, &c_impl, &s_index, &c_index, "my_crate"),
            "generic `impl<S> Foo<S>: Bar` must be structurally equal to bare `impl Foo: Bar` \
             (T044 regression, reversed order)"
        );
    }

    /// `impl<S> Foo<S>: Bar` must NOT compare equal to `impl<S> Qux<S>: Bar`
    /// (different base names, even though both have the same generic param name).
    #[test]
    fn test_impl_generic_for_different_base_types_are_not_equal() {
        let a_impl = make_impl_item_with_generic_for(Id(10), "Foo", "S", "Bar");
        let a_index = HashMap::new();

        let b_impl = make_impl_item_with_generic_for(Id(20), "Qux", "S", "Bar");
        let b_index = HashMap::new();

        assert!(
            !items_structurally_equal(&a_impl, &b_impl, &a_index, &b_index, "my_crate"),
            "`impl<S> Foo<S>: Bar` must NOT equal `impl<S> Qux<S>: Bar`"
        );
    }

    // -----------------------------------------------------------------------
    // T044: structs_structurally_equal — generic-param and method-map relaxations
    // -----------------------------------------------------------------------

    /// Builds a plain struct with `has_stripped_fields = true` and no fields.
    /// Used to model A-codec output for interactor structs with private fields.
    fn make_plain_struct_stripped(generics: Generics, impls: Vec<Id>) -> Struct {
        Struct {
            kind: StructKind::Plain { fields: vec![], has_stripped_fields: true },
            generics,
            impls,
        }
    }

    /// Builds a type-param `GenericParamDef` with the given bounds.
    fn make_type_param(name: &str, bounds: Vec<rustdoc_types::GenericBound>) -> GenericParamDef {
        GenericParamDef {
            name: name.to_string(),
            kind: GenericParamDefKind::Type { bounds, default: None, is_synthetic: false },
        }
    }

    /// Builds a `GenericBound::Outlives` for a lifetime (e.g. `'static`).
    fn make_outlives_bound(lifetime: &str) -> rustdoc_types::GenericBound {
        rustdoc_types::GenericBound::Outlives(lifetime.to_string())
    }

    /// Builds an `Item` wrapping a no-arg, no-return, non-async, non-unsafe function
    /// with the given generics (used to model method items in inherent impl blocks).
    fn make_fn_item_with_generics(id: Id, name: &str, generics: Generics) -> Item {
        Item {
            id,
            crate_id: 0,
            name: Some(name.to_string()),
            span: None,
            visibility: Visibility::Public,
            docs: None,
            links: HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner: ItemEnum::Function(rustdoc_types::Function {
                sig: FunctionSignature { inputs: vec![], output: None, is_c_variadic: false },
                generics,
                header: FunctionHeader {
                    is_unsafe: false,
                    is_const: false,
                    is_async: false,
                    abi: rustdoc_types::Abi::Rust,
                },
                has_body: true,
            }),
        }
    }

    /// Builds an inherent impl item (no trait) containing a list of method item Ids.
    fn make_inherent_impl_item(impl_id: Id, for_name: &str, method_ids: Vec<Id>) -> Item {
        make_item(
            impl_id,
            ItemEnum::Impl(Impl {
                is_unsafe: false,
                generics: empty_generics(),
                provided_trait_methods: vec![],
                trait_: None, // inherent impl
                for_: Type::ResolvedPath(Path {
                    path: for_name.to_string(),
                    id: Id(1),
                    args: None,
                }),
                items: method_ids,
                is_negative: false,
                is_synthetic: false,
                blanket_impl: None,
            }),
        )
    }

    /// T044 regression: A-side struct with empty `generics.params` (as produced by
    /// the A-codec which always uses `empty_generics()` for plain structs) must
    /// compare equal to a C-side struct that carries type parameters.
    ///
    /// Before the fix, `generics_structurally_equal(empty, <S: ...>)` returned
    /// `false` and produced a spurious Yellow signal for generic structs.
    #[test]
    fn test_plain_struct_a_side_empty_generics_matches_c_side_generic_params() {
        // A-side (A-codec): empty generics (codec never encodes struct-level params).
        let a_struct = make_plain_struct_stripped(empty_generics(), vec![]);
        let a_index = HashMap::new();

        // C-side (rustdoc): struct `Foo<S: Send + Sync>` with a type param.
        let s_bound = make_trait_bound("Send");
        let c_generics = Generics {
            params: vec![make_type_param("S", vec![s_bound])],
            where_predicates: vec![],
        };
        let c_struct = make_plain_struct_stripped(c_generics, vec![]);
        let c_index = HashMap::new();

        assert!(
            structs_structurally_equal(&a_struct, &c_struct, &a_index, &c_index),
            "A-side empty generics must match C-side generic struct params \
             (A-codec never encodes struct-level generics; T044 regression)"
        );
    }

    /// ADR 2026-05-20-0413 D1: A-side struct with `methods: []` (no inherent method
    /// decls in the catalogue) must NOT compare equal to C-side struct that has
    /// inherent methods.  This replaces the former T044 regression test
    /// (`test_plain_struct_empty_a_side_methods_matches_c_side_with_methods`) which
    /// protected the now-removed skip opt-out.
    ///
    /// With symmetric comparison, `a_methods == {}` vs `b_methods = {new: sig}`
    /// correctly detects a method-drift mismatch (Yellow signal).
    #[test]
    fn test_plain_struct_empty_a_side_methods_does_not_match_c_side_with_methods() {
        // A-side (catalogue): no inherent methods declared (methods: []).
        let a_struct = make_plain_struct_stripped(empty_generics(), vec![]);
        let a_index = HashMap::new();

        // C-side: struct with one inherent impl containing a `new` method.
        let fn_id = Id(100);
        let impl_id = Id(50);
        let fn_item = make_fn_item_with_generics(fn_id, "new", empty_generics());
        let impl_item = make_inherent_impl_item(impl_id, "Foo", vec![fn_id]);
        let mut c_index = HashMap::new();
        c_index.insert(fn_id, fn_item);
        c_index.insert(impl_id, impl_item);
        let c_struct = make_plain_struct_stripped(empty_generics(), vec![impl_id]);

        assert!(
            !structs_structurally_equal(&a_struct, &c_struct, &a_index, &c_index),
            "A-side with no inherent methods must NOT match C-side with methods \
             (symmetric comparison; method drift must produce Yellow — ADR 2026-05-20-0413 D1)"
        );
    }

    /// ADR 2026-05-20-0413 D1 (AC-16a): A `methods:[]` + C-side has no methods → equal (Blue).
    /// Pure data struct with no inherent methods on either side — must still compare equal.
    #[test]
    fn test_plain_struct_empty_methods_both_sides_compares_equal() {
        let a_struct = make_plain_struct_stripped(empty_generics(), vec![]);
        let a_index = HashMap::new();
        let c_struct = make_plain_struct_stripped(empty_generics(), vec![]);
        let c_index = HashMap::new();

        assert!(
            structs_structurally_equal(&a_struct, &c_struct, &a_index, &c_index),
            "Both sides with no inherent methods must compare equal (AC-16c)"
        );
    }

    /// ADR 2026-05-20-0413 D1 (AC-16b): A `[f]` + C `[f]` → equal (Blue).
    /// Both sides declare the same method.
    #[test]
    fn test_plain_struct_same_methods_both_sides_compares_equal() {
        let fn_id_a = Id(10);
        let impl_id_a = Id(5);
        let fn_item_a = make_fn_item_with_generics(fn_id_a, "new", empty_generics());
        let impl_item_a = make_inherent_impl_item(impl_id_a, "Foo", vec![fn_id_a]);
        let mut a_index = HashMap::new();
        a_index.insert(fn_id_a, fn_item_a);
        a_index.insert(impl_id_a, impl_item_a);
        let a_struct = make_plain_struct_stripped(empty_generics(), vec![impl_id_a]);

        let fn_id_c = Id(20);
        let impl_id_c = Id(15);
        let fn_item_c = make_fn_item_with_generics(fn_id_c, "new", empty_generics());
        let impl_item_c = make_inherent_impl_item(impl_id_c, "Foo", vec![fn_id_c]);
        let mut c_index = HashMap::new();
        c_index.insert(fn_id_c, fn_item_c);
        c_index.insert(impl_id_c, impl_item_c);
        let c_struct = make_plain_struct_stripped(empty_generics(), vec![impl_id_c]);

        assert!(
            structs_structurally_equal(&a_struct, &c_struct, &a_index, &c_index),
            "Both sides with same inherent method must compare equal (AC-16b)"
        );
    }

    /// T003 (ADR D1): Both A-side and C-side now retain Outlives bounds (strip_outlives_from_index
    /// removed). When both sides declare the same Outlives bound they must compare equal.
    /// This is the symmetric-both-sides scenario: A-side `new<F: Send + Sync + 'static>`
    /// matches C-side `new<F: Send + Sync + 'static>`.
    #[test]
    fn test_plain_struct_method_outlives_both_sides_retained_compares_equal() {
        // A-side: `new<F: Send + Sync + 'static>` — catalogue now declares the Outlives bound
        // (A-codec validate_supported_bound removed in T002; catalogue can express 'static).
        let f_bounds = vec![
            make_trait_bound("Send"),
            make_trait_bound("Sync"),
            make_outlives_bound("'static"),
        ];
        let make_fn_struct = |fn_id: Id, impl_id: Id| {
            let fn_generics = Generics {
                params: vec![make_type_param("F", f_bounds.clone())],
                where_predicates: vec![],
            };
            let fn_item = make_fn_item_with_generics(fn_id, "new", fn_generics);
            let impl_item = make_inherent_impl_item(impl_id, "Bar", vec![fn_id]);
            let mut idx = HashMap::new();
            idx.insert(fn_id, fn_item);
            idx.insert(impl_id, impl_item);
            let s = make_plain_struct_stripped(empty_generics(), vec![impl_id]);
            (s, idx)
        };
        let (a_struct, a_index) = make_fn_struct(Id(10), Id(5));
        let (c_struct, c_index) = make_fn_struct(Id(20), Id(15));

        assert!(
            structs_structurally_equal(&a_struct, &c_struct, &a_index, &c_index),
            "A-side and C-side both with `new<F: Send+Sync+'static>` must compare equal \
             (T003: Outlives retained on both sides symmetrically)"
        );
    }

    /// T003 (ADR D1): With strip_outlives_from_index removed, A-side without an Outlives bound
    /// must NOT compare equal to C-side with an Outlives bound. Catalogue authors must now
    /// declare the 'static bound explicitly to achieve Blue.
    #[test]
    fn test_plain_struct_method_outlives_asymmetric_is_mismatch() {
        // A-side: `new<F: Send + Sync>` (no Outlives).
        let f_bounds_a = vec![make_trait_bound("Send"), make_trait_bound("Sync")];
        let a_generics =
            Generics { params: vec![make_type_param("F", f_bounds_a)], where_predicates: vec![] };
        let a_fn_id = Id(10);
        let a_fn_item = make_fn_item_with_generics(a_fn_id, "new", a_generics);
        let a_impl_id = Id(5);
        let a_impl_item = make_inherent_impl_item(a_impl_id, "Bar", vec![a_fn_id]);
        let mut a_index = HashMap::new();
        a_index.insert(a_fn_id, a_fn_item);
        a_index.insert(a_impl_id, a_impl_item);
        let a_struct = make_plain_struct_stripped(empty_generics(), vec![a_impl_id]);

        // C-side: `new<F: Send + Sync + 'static>` (carries 'static Outlives bound).
        let f_bounds_c = vec![
            make_trait_bound("Send"),
            make_trait_bound("Sync"),
            make_outlives_bound("'static"),
        ];
        let c_generics =
            Generics { params: vec![make_type_param("F", f_bounds_c)], where_predicates: vec![] };
        let c_fn_id = Id(20);
        let c_fn_item = make_fn_item_with_generics(c_fn_id, "new", c_generics);
        let c_impl_id = Id(15);
        let c_impl_item = make_inherent_impl_item(c_impl_id, "Bar", vec![c_fn_id]);
        let mut c_index = HashMap::new();
        c_index.insert(c_fn_id, c_fn_item);
        c_index.insert(c_impl_id, c_impl_item);
        let c_struct = make_plain_struct_stripped(empty_generics(), vec![c_impl_id]);

        assert!(
            !structs_structurally_equal(&a_struct, &c_struct, &a_index, &c_index),
            "A-side `new<F: Send+Sync>` must NOT match C-side `new<F: Send+Sync+'static>` \
             (T003: Outlives no longer stripped; catalogue must declare 'static explicitly)"
        );
    }

    /// T003 (ADR D1): Asymmetric Outlives in where_predicates is also a mismatch after
    /// strip_outlives_from_index removal. A-side without Outlives vs C-side with `where F: 'static`.
    #[test]
    fn test_plain_struct_method_outlives_in_where_predicates_asymmetric_is_mismatch() {
        // A-side: `new<F: Send + Sync>` (no Outlives).
        let f_bounds_a = vec![make_trait_bound("Send"), make_trait_bound("Sync")];
        let a_generics =
            Generics { params: vec![make_type_param("F", f_bounds_a)], where_predicates: vec![] };
        let a_fn_id = Id(10);
        let a_fn_item = make_fn_item_with_generics(a_fn_id, "new", a_generics);
        let a_impl_id = Id(5);
        let a_impl_item = make_inherent_impl_item(a_impl_id, "Bar", vec![a_fn_id]);
        let mut a_index = HashMap::new();
        a_index.insert(a_fn_id, a_fn_item);
        a_index.insert(a_impl_id, a_impl_item);
        let a_struct = make_plain_struct_stripped(empty_generics(), vec![a_impl_id]);

        // C-side: `new<F: Send + Sync>` with `where F: 'static` in where_predicates.
        let f_bounds_c_inline = vec![make_trait_bound("Send"), make_trait_bound("Sync")];
        let static_predicate = rustdoc_types::WherePredicate::BoundPredicate {
            type_: rustdoc_types::Type::Generic("F".to_string()),
            bounds: vec![make_outlives_bound("'static")],
            generic_params: vec![],
        };
        let c_generics = Generics {
            params: vec![make_type_param("F", f_bounds_c_inline)],
            where_predicates: vec![static_predicate],
        };
        let c_fn_id = Id(20);
        let c_fn_item = make_fn_item_with_generics(c_fn_id, "new", c_generics);
        let c_impl_id = Id(15);
        let c_impl_item = make_inherent_impl_item(c_impl_id, "Bar", vec![c_fn_id]);
        let mut c_index = HashMap::new();
        c_index.insert(c_fn_id, c_fn_item);
        c_index.insert(c_impl_id, c_impl_item);
        let c_struct = make_plain_struct_stripped(empty_generics(), vec![c_impl_id]);

        assert!(
            !structs_structurally_equal(&a_struct, &c_struct, &a_index, &c_index),
            "A-side without 'static must NOT match C-side with `where F: 'static` \
             (T003: Outlives in where_predicates no longer stripped)"
        );
    }

    /// A genuine method signature difference (different trait bound, not just Outlives)
    /// must still detect the mismatch.
    #[test]
    fn test_plain_struct_different_non_outlives_method_bound_is_mismatch() {
        // A-side: `new<F: Send + Sync>`.
        let f_bounds_a = vec![make_trait_bound("Send"), make_trait_bound("Sync")];
        let a_generics =
            Generics { params: vec![make_type_param("F", f_bounds_a)], where_predicates: vec![] };
        let a_fn_id = Id(10);
        let a_fn_item = make_fn_item_with_generics(a_fn_id, "new", a_generics);
        let a_impl_id = Id(5);
        let a_impl_item = make_inherent_impl_item(a_impl_id, "Bar", vec![a_fn_id]);
        let mut a_index = HashMap::new();
        a_index.insert(a_fn_id, a_fn_item);
        a_index.insert(a_impl_id, a_impl_item);
        let a_struct = make_plain_struct_stripped(empty_generics(), vec![a_impl_id]);

        // C-side: `new<F: Send + Clone>` — different non-Outlives bounds.
        let f_bounds_c = vec![make_trait_bound("Send"), make_trait_bound("Clone")];
        let c_generics =
            Generics { params: vec![make_type_param("F", f_bounds_c)], where_predicates: vec![] };
        let c_fn_id = Id(20);
        let c_fn_item = make_fn_item_with_generics(c_fn_id, "new", c_generics);
        let c_impl_id = Id(15);
        let c_impl_item = make_inherent_impl_item(c_impl_id, "Bar", vec![c_fn_id]);
        let mut c_index = HashMap::new();
        c_index.insert(c_fn_id, c_fn_item);
        c_index.insert(c_impl_id, c_impl_item);
        let c_struct = make_plain_struct_stripped(empty_generics(), vec![c_impl_id]);

        assert!(
            !structs_structurally_equal(&a_struct, &c_struct, &a_index, &c_index),
            "A-side `new<F: Send+Sync>` must NOT match C-side `new<F: Send+Clone>` \
             (different non-Outlives bounds must still detect mismatch)"
        );
    }
}
