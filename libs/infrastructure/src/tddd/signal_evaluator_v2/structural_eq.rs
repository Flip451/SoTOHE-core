//! Structural equality helpers for Phase 2 item comparison.
//!
//! Compares `rustdoc_types` items for structural equality, ignoring docs and
//! parameter binding names.  Used in Phase 2 to decide whether S-side and C-side
//! items are identical (determining the `Match` / `Mismatch` sub-region).
//!
//! Generics, function, and trait helpers live in the sibling `generics_eq` module.

use std::collections::{BTreeMap, HashMap};

use rustdoc_types::{Id, Item, ItemEnum};

use super::format::format_type;
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
        // For trait impls: compare for_ + trait path (identity), header flags,
        // generics, AND the method map (structural content) so that any change
        // inside an impl block produces a structural mismatch.
        (ItemEnum::Impl(ia), ItemEnum::Impl(ib)) => {
            use super::format::format_generic_args;
            if ia.is_unsafe != ib.is_unsafe || ia.is_negative != ib.is_negative {
                return false;
            }
            let for_equal = format_type(&ia.for_) == format_type(&ib.for_);
            // Normalize the trait path to a short-name form for structural equality.
            //
            // Identity-key matching (in `build_impl_identity_map`) already confirmed that
            // S and C refer to the same trait — using `krate.paths` for disambiguation.
            // Here we only need to confirm they are the same trait (not identical path
            // strings), so we reduce to the short name + generic args.
            //
            // S-side (A-codec): generics embedded in path string with `args: None`
            //   e.g. Path { path: "core::convert::From<CatalogueLoaderError>", args: None }
            // C-side (rustdoc): base path in `path`, generics in `args`
            //   e.g. Path { path: "From", args: Some(AngleBracketed(["CatalogueLoaderError"])) }
            //
            // Reducing both to `"From<CatalogueLoaderError>"` produces equality.
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
            if !generics_structurally_equal(&ia.generics, &ib.generics) {
                return false;
            }
            // Compare method/associated-item maps only when the S-side impl has methods.
            // Catalogue-derived trait impls are identity-only (empty `items` list): the
            // catalogue declares "Foo implements Display" without encoding the actual method
            // bodies.  If the S-side impl is identity-only (no items), skip the content
            // comparison — identity equality (for_ + trait_ + generics) is sufficient.
            // When a_items is non-empty, the S-side carries a modified impl with explicit
            // content, so a full method-map comparison is required.
            if !ia.items.is_empty() {
                let a_methods = build_trait_method_map(&ia.items, a_index, Some(&ia.generics));
                let b_methods = build_trait_method_map(&ib.items, b_index, Some(&ib.generics));
                if a_methods != b_methods {
                    return false;
                }
            }
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
) -> BTreeMap<String, String> {
    let mut merged = BTreeMap::new();
    for impl_id in impl_ids {
        if let Some(impl_item) = index.get(impl_id) {
            if let ItemEnum::Impl(impl_) = &impl_item.inner {
                // Only inherent impls (no trait).
                if impl_.trait_.is_some() {
                    continue;
                }
                let methods = build_trait_method_map(&impl_.items, index, Some(&impl_.generics));
                for (name, sig) in methods {
                    merged.insert(name, sig);
                }
            }
        }
    }
    merged
}

fn structs_structurally_equal(
    a: &rustdoc_types::Struct,
    b: &rustdoc_types::Struct,
    a_index: &HashMap<Id, Item>,
    b_index: &HashMap<Id, Item>,
) -> bool {
    if !generics_structurally_equal(&a.generics, &b.generics) {
        return false;
    }
    // Compare inherent method signatures so that adding, removing, or changing an
    // inherent method (TypeEntry.methods in the catalogue) registers as a mismatch.
    let a_methods = build_inherent_method_map(&a.impls, a_index);
    let b_methods = build_inherent_method_map(&b.impls, b_index);
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
    let a_methods = build_inherent_method_map(&a.impls, a_index);
    let b_methods = build_inherent_method_map(&b.impls, b_index);
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

    use rustdoc_types::{Generics, Id, Item, ItemEnum, Struct, StructKind, Type, Visibility};

    use super::structs_structurally_equal;

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
}
