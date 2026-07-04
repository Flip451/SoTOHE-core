//! `EncoderState` methods for encoding `TypeEntry` variants (unit struct, tuple struct,
//! plain struct, enum, type alias) and enum variant payloads.

use domain::tddd::catalogue_v2::entries::TypeEntry;
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl, VariantPayload};
use domain::tddd::catalogue_v2::{MethodDeclaration, TypeName};
use rustdoc_types::{Id, ItemEnum, ItemKind, Struct, StructKind, TypeAlias, Variant, VariantKind};

use crate::tddd::catalogue_to_extended_crate_codec_error::CatalogueToExtendedCrateCodecError;

use super::encoder::EncoderState;
use super::helpers::{make_impl, make_item, resolved_path_type};

impl EncoderState {
    /// Shared finalization for struct-like type entries.
    ///
    /// Encodes inherent methods, builds the inherent impl block, inserts the
    /// `Struct` item with the provided `kind`, and registers the path.  This
    /// eliminates the repeated finalization sequence in `encode_unit_struct` and
    /// `encode_plain_struct`.
    fn encode_struct_item_with_inherent_impl(
        &mut self,
        type_id: Id,
        type_name: &TypeName,
        entry: &TypeEntry,
        struct_kind: StructKind,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        let module_path = entry.module_path().clone();
        let docs = entry.docs().map(|d| d.as_str().to_owned());

        // Type-declaration-level generics / where predicates are encoded into the
        // struct's `Generics` (ADR `2026-07-02-1345` D6): declared values must be
        // observable by the signal comparator, never silently dropped. Empty decls
        // yield empty `Generics`, preserving backward compatibility with catalogues
        // that predate these fields. Symmetric with trait / function encoding.
        let generic_names: Vec<&str> = entry.generics().iter().map(|g| g.name.as_str()).collect();
        let generics = self.build_where_form_generics(
            entry.generics(),
            entry.where_predicates(),
            &generic_names,
        )?;

        let methods: Vec<MethodDeclaration> = entry.methods().to_vec();
        let method_ids = self.encode_method_items(
            &methods,
            true,
            type_name.as_str(),
            &module_path,
            &generic_names,
        )?;

        let impl_id = self.alloc_id();
        let for_type = resolved_path_type(type_id, type_name.as_str());
        self.index.insert(
            impl_id,
            make_item(impl_id, None, None, ItemEnum::Impl(make_impl(for_type, None, method_ids))),
        );

        let struct_item = make_item(
            type_id,
            Some(type_name.as_str().to_string()),
            docs,
            ItemEnum::Struct(Struct { kind: struct_kind, generics, impls: vec![impl_id] }),
        );
        self.index.insert(type_id, struct_item);
        self.register_path(type_id, ItemKind::Struct, type_name.as_str(), &module_path);
        Ok(())
    }

    /// Encodes a `UnitStruct` kind `TypeEntry`.
    pub(super) fn encode_unit_struct(
        &mut self,
        type_id: Id,
        type_name: &TypeName,
        entry: &TypeEntry,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        self.encode_struct_item_with_inherent_impl(type_id, type_name, entry, StructKind::Unit)
    }

    /// Encodes a `TupleStruct` kind `TypeEntry`.
    pub(super) fn encode_tuple_struct(
        &mut self,
        type_id: Id,
        type_name: &TypeName,
        entry: &TypeEntry,
        fields: Vec<domain::tddd::catalogue_v2::identifiers::TypeRef>,
        has_stripped_fields: bool,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        let module_path = entry.module_path().clone();
        let docs = entry.docs().map(|d| d.as_str().to_owned());

        // Type-declaration-level generics / where predicates (ADR `2026-07-02-1345` D6).
        let generic_names: Vec<&str> = entry.generics().iter().map(|g| g.name.as_str()).collect();
        let generics = self.build_where_form_generics(
            entry.generics(),
            entry.where_predicates(),
            &generic_names,
        )?;

        // Encode positional fields as StructField items with None names (tuple style).
        // Positional field names (.0, .1, ...) are synthesized by the rustdoc format;
        // the catalogue stores only the types.
        let mut field_ids: Vec<Option<Id>> = vec![];
        for field_ty_ref in &fields {
            let field_id = self.alloc_id();
            // Field types must resolve type-declaration-level generics so that a
            // field like `.0: T` in `struct Foo<T>(T)` encodes as `Type::Generic("T")`,
            // matching rustdoc's C-side, rather than an unresolved local-path marker.
            let field_ty =
                self.parse_type_ref_str_with_generics(field_ty_ref.as_str(), &generic_names)?;
            self.index
                .insert(field_id, make_item(field_id, None, None, ItemEnum::StructField(field_ty)));
            field_ids.push(Some(field_id));
        }
        // Represent the presence of private fields as a single trailing None.
        //
        // Note on position fidelity: the catalogue (TupleStruct.has_stripped_fields) records
        // only *whether* private fields exist, not their exact positions.  Rustdoc's
        // StructKind::Tuple preserves exact None-slot positions.  The structural equality
        // check therefore may not distinguish "private field moved before a public field" from
        // an unchanged layout — both produce length-matching vectors if public-field types
        // match.  This is an accepted limitation of the current catalogue schema: we prevent
        // false Blue when private fields are *added or removed* (length change) but cannot
        // surface field-position changes within tuple structs as a Mismatch.
        if has_stripped_fields {
            field_ids.push(None);
        }

        let struct_kind = StructKind::Tuple(field_ids);

        // Encode inherent method items.
        let methods: Vec<MethodDeclaration> = entry.methods().to_vec();
        let method_ids = self.encode_method_items(
            &methods,
            true,
            type_name.as_str(),
            &module_path,
            &generic_names,
        )?;

        let impl_id = self.alloc_id();
        let for_type = resolved_path_type(type_id, type_name.as_str());
        self.index.insert(
            impl_id,
            make_item(impl_id, None, None, ItemEnum::Impl(make_impl(for_type, None, method_ids))),
        );

        let struct_item = make_item(
            type_id,
            Some(type_name.as_str().to_string()),
            docs,
            ItemEnum::Struct(Struct { kind: struct_kind, generics, impls: vec![impl_id] }),
        );
        self.index.insert(type_id, struct_item);
        self.register_path(type_id, ItemKind::Struct, type_name.as_str(), &module_path);
        Ok(())
    }

    /// Encodes a `PlainStruct` kind `TypeEntry`.
    ///
    /// The `typestate` marker (if present) does not affect the rustdoc structure — the
    /// plain struct is encoded identically with or without it. The marker is carried
    /// only at the catalogue domain level for signal evaluation and rendering.
    pub(super) fn encode_plain_struct(
        &mut self,
        type_id: Id,
        type_name: &TypeName,
        entry: &TypeEntry,
        fields: Vec<FieldDecl>,
        has_stripped_fields: bool,
        _typestate: Option<domain::tddd::catalogue_v2::composite::TypestateMarker>,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        // Encode named fields → StructField items. Field types must resolve
        // type-declaration-level generics (ADR `2026-07-02-1345` D6) so that a
        // field like `value: T` in `struct Foo<T> { value: T }` encodes as
        // `Type::Generic("T")`, matching rustdoc's C-side, rather than an
        // unresolved local-path marker (which would surface a false non-Blue signal).
        let generic_names: Vec<&str> = entry.generics().iter().map(|g| g.name.as_str()).collect();
        let mut field_ids: Vec<Id> = vec![];
        for field in &fields {
            let field_id = self.alloc_id();
            let field_ty =
                self.parse_type_ref_str_with_generics(field.ty.as_str(), &generic_names)?;
            self.index.insert(
                field_id,
                make_item(
                    field_id,
                    Some(field.name.as_str().to_string()),
                    None,
                    ItemEnum::StructField(field_ty),
                ),
            );
            field_ids.push(field_id);
        }
        let struct_kind = StructKind::Plain { fields: field_ids, has_stripped_fields };
        self.encode_struct_item_with_inherent_impl(type_id, type_name, entry, struct_kind)
    }

    /// Encodes an enum-kind `TypeEntry`.
    pub(super) fn encode_enum(
        &mut self,
        type_id: Id,
        type_name: &TypeName,
        entry: &TypeEntry,
        variants: Vec<VariantDecl>,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        let module_path = entry.module_path().clone();
        let docs = entry.docs().map(|d| d.as_str().to_owned());

        // Type-declaration-level generics / where predicates (ADR `2026-07-02-1345` D6).
        let generic_names: Vec<&str> = entry.generics().iter().map(|g| g.name.as_str()).collect();
        let generics = self.build_where_form_generics(
            entry.generics(),
            entry.where_predicates(),
            &generic_names,
        )?;

        // Encode variant items.
        let mut variant_ids: Vec<Id> = vec![];
        for variant in &variants {
            let variant_id = self.alloc_id();
            let variant_name = variant.name.as_str().to_string();
            let payload = variant.payload.clone();
            let kind = self.encode_variant_kind(payload, &generic_names)?;
            self.index.insert(
                variant_id,
                make_item(
                    variant_id,
                    Some(variant_name),
                    None,
                    ItemEnum::Variant(Variant { kind, discriminant: None }),
                ),
            );
            variant_ids.push(variant_id);
        }

        // Inherent methods (concrete implementations → has_body: true).
        let methods: Vec<MethodDeclaration> = entry.methods().to_vec();
        let method_ids = self.encode_method_items(
            &methods,
            true,
            type_name.as_str(),
            &module_path,
            &generic_names,
        )?;

        // Inherent Impl block.
        let impl_id = self.alloc_id();
        let for_type = resolved_path_type(type_id, type_name.as_str());
        self.index.insert(
            impl_id,
            make_item(impl_id, None, None, ItemEnum::Impl(make_impl(for_type, None, method_ids))),
        );

        // Enum item.
        let enum_item = make_item(
            type_id,
            Some(type_name.as_str().to_string()),
            docs,
            ItemEnum::Enum(rustdoc_types::Enum {
                generics,
                variants: variant_ids,
                impls: vec![impl_id],
                has_stripped_variants: false,
            }),
        );
        self.index.insert(type_id, enum_item);
        self.register_path(type_id, ItemKind::Enum, type_name.as_str(), &module_path);
        Ok(())
    }

    /// Encodes a type-alias-kind `TypeEntry`.
    pub(super) fn encode_type_alias(
        &mut self,
        type_id: Id,
        type_name: &TypeName,
        entry: &TypeEntry,
        target: domain::tddd::catalogue_v2::TypeRef,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        let module_path = entry.module_path().clone();
        let docs = entry.docs().map(|d| d.as_str().to_owned());

        // Type-declaration-level generics / where predicates (ADR `2026-07-02-1345` D6).
        let generic_names: Vec<&str> = entry.generics().iter().map(|g| g.name.as_str()).collect();
        let generics = self.build_where_form_generics(
            entry.generics(),
            entry.where_predicates(),
            &generic_names,
        )?;

        // The alias target must resolve type-declaration-level generics so that
        // `type Foo<T> = Vec<T>` encodes the inner `T` as `Type::Generic("T")`,
        // matching rustdoc's C-side rather than an unresolved local-path marker.
        let target_ty = self.parse_type_ref_str_with_generics(target.as_str(), &generic_names)?;

        // Encode inherent methods (rare for type aliases; concrete implementations → has_body: true).
        let methods: Vec<MethodDeclaration> = entry.methods().to_vec();
        let method_ids = self.encode_method_items(
            &methods,
            true,
            type_name.as_str(),
            &module_path,
            &generic_names,
        )?;
        let impl_id = self.alloc_id();
        let for_type = resolved_path_type(type_id, type_name.as_str());
        self.index.insert(
            impl_id,
            make_item(impl_id, None, None, ItemEnum::Impl(make_impl(for_type, None, method_ids))),
        );

        let alias_item = make_item(
            type_id,
            Some(type_name.as_str().to_string()),
            docs,
            ItemEnum::TypeAlias(TypeAlias { type_: target_ty, generics }),
        );
        self.index.insert(type_id, alias_item);
        self.register_path(type_id, ItemKind::TypeAlias, type_name.as_str(), &module_path);
        Ok(())
    }

    /// Encodes a `VariantPayload` into `rustdoc_types::VariantKind`.
    ///
    /// `generic_names` lists the enum's type-declaration-level generic parameter
    /// names (ADR `2026-07-02-1345` D6) so that a variant payload referencing a
    /// generic — e.g. `Some(T)` in `enum Opt<T> { Some(T) }` — encodes as
    /// `Type::Generic("T")`, matching rustdoc's C-side, rather than an unresolved
    /// local-path marker.
    pub(super) fn encode_variant_kind(
        &mut self,
        payload: VariantPayload,
        generic_names: &[&str],
    ) -> Result<VariantKind, CatalogueToExtendedCrateCodecError> {
        match payload {
            VariantPayload::Unit => Ok(VariantKind::Plain),
            VariantPayload::Tuple(type_refs) => {
                let mut field_ids = vec![];
                for ty_ref in type_refs {
                    let field_id = self.alloc_id();
                    let field_ty =
                        self.parse_type_ref_str_with_generics(ty_ref.as_str(), generic_names)?;
                    self.index.insert(
                        field_id,
                        make_item(field_id, None, None, ItemEnum::StructField(field_ty)),
                    );
                    field_ids.push(Some(field_id));
                }
                Ok(VariantKind::Tuple(field_ids))
            }
            VariantPayload::Struct(fields) => {
                let mut field_ids = vec![];
                for f in fields {
                    let field_id = self.alloc_id();
                    let field_ty =
                        self.parse_type_ref_str_with_generics(f.ty.as_str(), generic_names)?;
                    self.index.insert(
                        field_id,
                        make_item(
                            field_id,
                            Some(f.name.as_str().to_string()),
                            None,
                            ItemEnum::StructField(field_ty),
                        ),
                    );
                    field_ids.push(field_id);
                }
                Ok(VariantKind::Struct { fields: field_ids, has_stripped_fields: false })
            }
        }
    }
}
