//! Free helper functions used across the codec sub-modules.
use std::collections::HashMap;

use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::SelfReceiver;
use domain::tddd::catalogue_v2::entries::{AssocConstDecl, AssocTypeDecl};
use rustdoc_types::{GenericBound, Generics, Id, Impl, Item, ItemEnum, Path, Type, Visibility};

use super::encoder::EncoderState;
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

impl EncoderState {
    pub(super) fn encode_assoc_type_item(
        &mut self,
        id: Id,
        decl: &AssocTypeDecl,
        trait_generic_names: &[&str],
    ) -> Result<rustdoc_types::Item, NewTypeGraphCodecError> {
        let mut bounds: Vec<GenericBound> = Vec::with_capacity(decl.bounds.len());
        for b in &decl.bounds {
            bounds.push(self.encode_trait_scoped_bound(b.as_str(), trait_generic_names)?);
        }
        let type_ = decl
            .default
            .as_ref()
            .map(|d| self.encode_trait_scoped_type_ref(d.as_str(), trait_generic_names))
            .transpose()?;

        let generics = Generics { params: vec![], where_predicates: vec![] };

        Ok(make_item(
            id,
            Some(decl.name.to_string()),
            None,
            ItemEnum::AssocType { generics, bounds, type_ },
        ))
    }

    fn encode_trait_scoped_bound(
        &mut self,
        bound_str: &str,
        trait_generic_names: &[&str],
    ) -> Result<GenericBound, NewTypeGraphCodecError> {
        self.encode_bound_str_with_suppressed_external_prefixes_and_generics(
            bound_str,
            trait_generic_names,
            trait_generic_names,
        )
    }

    pub(super) fn encode_assoc_const_item(
        &mut self,
        id: Id,
        decl: &AssocConstDecl,
        trait_generic_names: &[&str],
    ) -> Result<rustdoc_types::Item, NewTypeGraphCodecError> {
        let type_ = self.encode_trait_scoped_type_ref(decl.ty.as_str(), trait_generic_names)?;

        let value = decl.default_value.as_ref().map(|e| e.as_str().to_owned());

        Ok(make_item(id, Some(decl.name.to_string()), None, ItemEnum::AssocConst { type_, value }))
    }

    fn encode_trait_scoped_type_ref(
        &mut self,
        type_ref_str: &str,
        trait_generic_names: &[&str],
    ) -> Result<Type, NewTypeGraphCodecError> {
        self.parse_type_ref_str_with_suppressed_external_prefixes(
            type_ref_str,
            trait_generic_names,
            trait_generic_names,
        )
    }
}
