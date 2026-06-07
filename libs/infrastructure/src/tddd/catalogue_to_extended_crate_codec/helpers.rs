//! Free helper functions used across the codec sub-modules.

use std::collections::HashMap;

use domain::tddd::catalogue_v2::SelfReceiver;
use rustdoc_types::{
    AssocItemConstraint, AssocItemConstraintKind, DynTrait, GenericArg, GenericArgs, GenericBound,
    Generics, Id, Impl, Item, ItemEnum, Path, PolyTrait, Term, Type, Visibility,
};

// ---------------------------------------------------------------------------
// Item construction helpers
// ---------------------------------------------------------------------------

/// Creates a `rustdoc_types::Item` with common fixed-value fields.
///
/// Sets `crate_id: 0` (local crate). Use `make_item_with_crate_id` when the item
/// belongs to an external crate.
pub(super) fn make_item(
    id: Id,
    name: Option<String>,
    docs: Option<String>,
    inner: ItemEnum,
) -> Item {
    make_item_with_crate_id(0, id, name, docs, inner)
}

/// Creates a `rustdoc_types::Item` with an explicit `crate_id`.
///
/// Use `0` for items belonging to the document crate; pass the external crate's
/// numeric id for items belonging to a foreign crate.
pub(super) fn make_item_with_crate_id(
    crate_id: u32,
    id: Id,
    name: Option<String>,
    docs: Option<String>,
    inner: ItemEnum,
) -> Item {
    Item {
        id,
        crate_id,
        name,
        span: None,
        visibility: Visibility::Public,
        docs,
        links: HashMap::new(),
        attrs: vec![],
        deprecation: None,
        inner,
    }
}

/// Normalizes the `path` field of a `Type::ResolvedPath` inside an `impl.for_` type
/// to its last path segment (short name).
///
/// Rustdoc emits the short name (e.g. `"Vec"`) in `impl.for_.path` for external types,
/// not the fully-qualified form (e.g. `"std::vec::Vec"`).  The catalogue codec must emit
/// the same short-name form so that the `for_path_raw` secondary sort key in
/// `build_impl_identity_map` is consistent between A-origin (catalogue) and C-side
/// (rustdoc) impls, preventing spurious Phase 2 structural-equality mismatches.
///
/// Applies only to `Type::ResolvedPath`; container types (Tuple, Slice, etc.) are
/// recursed into so that generic args like `Vec<LocalError>` are also normalized.
/// All other type variants are returned unchanged.
///
/// This normalization applies ONLY to the `for_` field of impl blocks.  The trait
/// path (from `resolve_trait_ref_for_top_level`) must keep its fully-qualified form
/// so `build_impl_identity_map` can disambiguate external traits by qualified name.
pub(super) fn normalize_impl_for_type_path(ty: Type) -> Type {
    match ty {
        Type::ResolvedPath(p) => {
            let short_path = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
            Type::ResolvedPath(Path { path: short_path, id: p.id, args: p.args })
        }
        Type::Tuple(elems) => {
            Type::Tuple(elems.into_iter().map(normalize_impl_for_type_path).collect())
        }
        Type::Slice(inner) => Type::Slice(Box::new(normalize_impl_for_type_path(*inner))),
        Type::Array { type_, len } => {
            Type::Array { type_: Box::new(normalize_impl_for_type_path(*type_)), len }
        }
        Type::BorrowedRef { lifetime, is_mutable, type_ } => Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_: Box::new(normalize_impl_for_type_path(*type_)),
        },
        Type::RawPointer { is_mutable, type_ } => {
            Type::RawPointer { is_mutable, type_: Box::new(normalize_impl_for_type_path(*type_)) }
        }
        other => other,
    }
}

/// Returns a `Type::ResolvedPath` for a self-referential / placeholder path.
///
/// `path` is the short type name (without module prefix) used in `Impl.for_` so
/// downstream consumers can identify the owning type by name.
pub(super) fn resolved_path_type(id: Id, path: &str) -> Type {
    Type::ResolvedPath(Path { path: path.to_string(), id, args: None })
}

/// Builds an `Impl` with the given `for_` type and optional trait.
pub(super) fn make_impl(for_: Type, trait_: Option<Path>, items: Vec<Id>) -> Impl {
    Impl {
        is_unsafe: false,
        generics: empty_generics(),
        provided_trait_methods: vec![],
        trait_,
        for_,
        items,
        is_synthetic: false,
        is_negative: false,
        blanket_impl: None,
    }
}

/// Returns an empty `rustdoc_types::Generics`.
pub(super) fn empty_generics() -> Generics {
    Generics { params: vec![], where_predicates: vec![] }
}

// ---------------------------------------------------------------------------
// Generic-type rewriting helpers
// ---------------------------------------------------------------------------

/// Recursively rewrites a `Type` tree, replacing any `ResolvedPath` node whose
/// `path` exactly matches a method-level generic parameter name (and whose `id`
/// is `Id(UNRESOLVED_CRATE_ID)` with no generic args) with `Type::Generic(name)`.
///
/// Rustdoc emits `Type::Generic("T")` for generic parameters in function
/// signatures (e.g. `fn foo<T>(x: T)`, `fn foo<T>(x: Option<T>)`).  The
/// catalogue codec must emit the same representation so that Phase 1 / Phase 2
/// structural comparison succeeds.
///
/// Only plain single-segment unresolved paths are replaced — composite args such
/// as `Option<T>` keep their outer `ResolvedPath(Option)` form but have the inner
/// `T` arg rewritten to `GenericArg::Type(Type::Generic("T"))`.
pub(super) fn rewrite_generic_types(ty: Type, generic_names: &[&str]) -> Type {
    match ty {
        // Single-segment path (no `::` in path, no generic args) whose name is a method-level
        // generic parameter → `Type::Generic`.
        //
        // Method-scope generics take precedence over catalogue-local type resolution.
        // `parse_type_ref_str` may resolve a method generic name (e.g. `"T"`) to a local
        // `ResolvedPath` if the catalogue also declares a type named `"T"`.  Rustdoc always
        // emits `Type::Generic("T")` for method generics, so we must rewrite ANY
        // single-segment no-args path whose name is in `generic_names`, regardless of its Id.
        //
        // Only bare single-segment paths (no `::` in `p.path`) without generic args are
        // eligible: composite outer paths like `Option` in `Option<T>` must NOT be replaced
        // even if a generic happens to share that name.
        Type::ResolvedPath(ref p)
            if p.args.is_none()
                && !p.path.contains("::")
                && generic_names.contains(&p.path.as_str()) =>
        {
            Type::Generic(p.path.clone())
        }
        // Composite ResolvedPath: keep the path but recurse into generic args.
        Type::ResolvedPath(p) => {
            let new_args = p.args.map(|args| Box::new(rewrite_generic_args(*args, generic_names)));
            Type::ResolvedPath(Path { args: new_args, ..p })
        }
        Type::BorrowedRef { lifetime, is_mutable, type_ } => Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_: Box::new(rewrite_generic_types(*type_, generic_names)),
        },
        Type::RawPointer { is_mutable, type_ } => Type::RawPointer {
            is_mutable,
            type_: Box::new(rewrite_generic_types(*type_, generic_names)),
        },
        Type::Tuple(elems) => Type::Tuple(
            elems.into_iter().map(|t| rewrite_generic_types(t, generic_names)).collect(),
        ),
        Type::Slice(inner) => Type::Slice(Box::new(rewrite_generic_types(*inner, generic_names))),
        Type::Array { type_, len } => {
            Type::Array { type_: Box::new(rewrite_generic_types(*type_, generic_names)), len }
        }
        // ImplTrait: recurse into each bound (e.g. `impl Iterator<Item = T>`).
        Type::ImplTrait(bounds) => Type::ImplTrait(
            bounds.into_iter().map(|b| rewrite_generic_types_in_bound(b, generic_names)).collect(),
        ),
        // DynTrait: recurse into each PolyTrait's path args (e.g. `dyn Iterator<Item = T>`).
        Type::DynTrait(dyn_trait) => {
            let new_traits = dyn_trait
                .traits
                .into_iter()
                .map(|pt| {
                    let new_args = pt
                        .trait_
                        .args
                        .map(|args| Box::new(rewrite_generic_args(*args, generic_names)));
                    PolyTrait {
                        trait_: Path { args: new_args, ..pt.trait_ },
                        generic_params: pt.generic_params,
                    }
                })
                .collect();
            Type::DynTrait(DynTrait { traits: new_traits, lifetime: dyn_trait.lifetime })
        }
        // FunctionPointer: recurse into input and output types.
        // A method with a generic `fn(T) -> T` parameter type must have `T` rewritten
        // to `Type::Generic("T")` inside the function pointer signature.
        Type::FunctionPointer(fp) => {
            let new_inputs = fp
                .sig
                .inputs
                .into_iter()
                .map(|(name, ty)| (name, rewrite_generic_types(ty, generic_names)))
                .collect();
            let new_output = fp.sig.output.map(|t| rewrite_generic_types(t, generic_names));
            Type::FunctionPointer(Box::new(rustdoc_types::FunctionPointer {
                sig: rustdoc_types::FunctionSignature {
                    inputs: new_inputs,
                    output: new_output,
                    is_c_variadic: fp.sig.is_c_variadic,
                },
                generic_params: fp.generic_params,
                header: fp.header,
            }))
        }
        // Primitive, Generic, Infer, QualifiedPath: leave unchanged.
        other => other,
    }
}

/// Rewrites method-generic names that appear as type arguments inside a `GenericBound`.
///
/// `encode_bound_str` produces `GenericBound::TraitBound { trait_: Path, ... }`.
/// If the bound has generic args (e.g. `Into<U>`) and `U` is a method-level generic,
/// the `U` arg will be `ResolvedPath(UNRESOLVED_CRATE_ID)` after parsing.  This
/// function rewrites those occurrences to `Type::Generic("U")` so that Phase 1
/// does not misreport them as unresolved catalogue types.
pub(super) fn rewrite_generic_types_in_bound(
    bound: GenericBound,
    generic_names: &[&str],
) -> GenericBound {
    match bound {
        GenericBound::TraitBound { trait_: path, generic_params, modifier } => {
            let new_args =
                path.args.map(|args| Box::new(rewrite_generic_args(*args, generic_names)));
            GenericBound::TraitBound {
                trait_: Path { args: new_args, ..path },
                generic_params,
                modifier,
            }
        }
        // Outlives bounds have no nested types.
        GenericBound::Outlives(_) => bound,
        // Use bound (e.g. `T: use<'a>`) has no type args to rewrite.
        GenericBound::Use(_) => bound,
    }
}

/// Recursively rewrites generic args inside a `GenericArgs` node.
///
/// For `AngleBracketed` args, rewrites both type arguments and associated-type
/// constraint values (e.g. `Iterator<Item = T>` where `T` is a method generic).
pub(super) fn rewrite_generic_args(args: GenericArgs, generic_names: &[&str]) -> GenericArgs {
    match args {
        GenericArgs::AngleBracketed { args: arg_list, constraints } => {
            let new_args = arg_list
                .into_iter()
                .map(|a| match a {
                    GenericArg::Type(t) => {
                        GenericArg::Type(rewrite_generic_types(t, generic_names))
                    }
                    other => other,
                })
                .collect();
            // Also rewrite types inside associated-type constraints
            // (e.g. `Iterator<Item = T>` where `T` is a method generic).
            let new_constraints = constraints
                .into_iter()
                .map(|c| rewrite_assoc_constraint(c, generic_names))
                .collect();
            GenericArgs::AngleBracketed { args: new_args, constraints: new_constraints }
        }
        GenericArgs::Parenthesized { inputs, output } => {
            let new_inputs =
                inputs.into_iter().map(|t| rewrite_generic_types(t, generic_names)).collect();
            let new_output = output.map(|t| rewrite_generic_types(t, generic_names));
            GenericArgs::Parenthesized { inputs: new_inputs, output: new_output }
        }
        // ReturnTypeNotation (`(..)`) has no nested types to rewrite.
        GenericArgs::ReturnTypeNotation => GenericArgs::ReturnTypeNotation,
    }
}

/// Rewrites method-generic names inside an `AssocItemConstraint`.
///
/// Handles all three constraint variants:
/// - `Equality(Term::Type(T))` (e.g. `Item = T`) — rewrites `T` if it matches a generic name.
/// - `Constraint(Vec<GenericBound>)` (e.g. `Item: Into<T>`) — rewrites each bound via
///   `rewrite_generic_types_in_bound` so trait-path type args (e.g. `T` in `Into<T>`) are
///   also rewritten to `Type::Generic("T")` when `T` is a method generic name.
/// - `Equality(Term::Const(_))` — left unchanged (no type parameter to rewrite).
pub(super) fn rewrite_assoc_constraint(
    constraint: AssocItemConstraint,
    generic_names: &[&str],
) -> AssocItemConstraint {
    let new_args = constraint.args.map(|args| Box::new(rewrite_generic_args(*args, generic_names)));
    let new_binding = match constraint.binding {
        AssocItemConstraintKind::Equality(Term::Type(ty)) => {
            AssocItemConstraintKind::Equality(Term::Type(rewrite_generic_types(ty, generic_names)))
        }
        AssocItemConstraintKind::Constraint(bounds) => {
            // `Item: T` bound constraints — T may be a method generic name.
            AssocItemConstraintKind::Constraint(
                bounds
                    .into_iter()
                    .map(|b| rewrite_generic_types_in_bound(b, generic_names))
                    .collect(),
            )
        }
        // Const equality: no type to rewrite.
        other => other,
    };
    AssocItemConstraint { name: constraint.name, args: new_args, binding: new_binding }
}

// ---------------------------------------------------------------------------
// Receiver and generic-name helpers
// ---------------------------------------------------------------------------

/// Converts a `SelfReceiver` into the corresponding `rustdoc_types::Type`.
///
/// Used as the receiver parameter type in `FunctionSignature::inputs`.
pub(super) fn receiver_type(receiver: SelfReceiver) -> Type {
    match receiver {
        SelfReceiver::Owned => {
            Type::ResolvedPath(Path { path: "Self".to_string(), id: Id(0), args: None })
        }
        SelfReceiver::SharedRef => {
            let inner =
                Type::ResolvedPath(Path { path: "Self".to_string(), id: Id(0), args: None });
            Type::BorrowedRef { lifetime: None, is_mutable: false, type_: Box::new(inner) }
        }
        SelfReceiver::ExclusiveRef => {
            let inner =
                Type::ResolvedPath(Path { path: "Self".to_string(), id: Id(0), args: None });
            Type::BorrowedRef { lifetime: None, is_mutable: true, type_: Box::new(inner) }
        }
    }
}

/// Returns `true` when `type_str` is a bare single-word identifier that matches
/// one of the method-level generic parameter names in `generic_names`.
///
/// "Bare single-word" means:
/// - No `::` (not a qualified path like `std::fmt::Display`)
/// - No `<` or `>` (not a generic application like `Option<T>`)
/// - No `'` prefix (not a lifetime)
/// - No `&`, `*`, `[`, `(` (not a reference, pointer, slice, or tuple)
///
/// This pre-check prevents `parse_type_ref_str` from expanding well-known names
/// via `STD_PRELUDE_TYPES` (e.g. `"From"` → `"std::convert::From"`) before
/// `rewrite_generic_types` gets a chance to recognise and replace them.
pub(super) fn is_bare_generic_name(type_str: &str, generic_names: &[&str]) -> bool {
    // Quick character-level checks before the slice lookup.
    let t = type_str.trim();
    if t.is_empty()
        || t.contains("::")
        || t.contains('<')
        || t.contains('>')
        || t.contains('\'')
        || t.contains('&')
        || t.contains('*')
        || t.contains('[')
        || t.contains('(')
    {
        return false;
    }
    generic_names.contains(&t)
}

/// If `type_str` is a single-level associated-type projection path whose LHS is a
/// known generic parameter (`T::Item`), build the corresponding
/// `Type::QualifiedPath` that matches what rustdoc emits for such predicates.
///
/// This covers the form `GENERIC_PARAM::ASSOC_IDENT` (no extra `::` nesting, no
/// angle-bracket args on the associated type).  More complex forms (`T::Item<U>`,
/// `<T as Trait>::Assoc`, multi-level `T::A::B`) return `None` so the caller can
/// fall back to `parse_type_ref_str`.
///
/// Background: rustdoc represents `where T::Item: Send` as
/// `WherePredicate::BoundPredicate { type_: Type::QualifiedPath { name: "Item",
/// self_type: Generic("T"), trait_: None, args: None }, ... }`.  `parse_type_ref_str`
/// cannot produce this shape (it treats the first segment as a crate name), so we
/// must handle the pattern here before falling through to the parser.
pub(super) fn try_build_generic_projection(type_str: &str, generic_names: &[&str]) -> Option<Type> {
    let t = type_str.trim();
    // Must contain exactly one `::` separator (two-segment form only).
    let sep_pos = t.find("::")?;
    let prefix = &t[..sep_pos];
    let rest = &t[sep_pos + 2..];
    // No further `::` in the rest (single-level projection only).
    if rest.contains("::") {
        return None;
    }
    // No angle brackets (associated type with no generic args).
    if rest.contains('<') || rest.contains('>') {
        return None;
    }
    // Prefix must be a known generic parameter name.
    if !generic_names.contains(&prefix) {
        return None;
    }
    // `rest` must be a valid identifier (non-empty, starts with letter or `_`).
    let first_char = rest.chars().next()?;
    if !first_char.is_ascii_alphabetic() && first_char != '_' {
        return None;
    }
    Some(Type::QualifiedPath {
        name: rest.to_string(),
        args: None,
        self_type: Box::new(Type::Generic(prefix.to_string())),
        trait_: None,
    })
}
