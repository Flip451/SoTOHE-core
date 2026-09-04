//! Deletion/tombstone encoding for the catalogue-to-ExtendedCrate codec.
//!
//! These helpers keep delete-marked type, trait, and function entries aligned
//! with their pre-pass identities and `ItemAction::Delete` signals without
//! mixing tombstone construction into the main encoding pipeline.

use std::collections::BTreeMap;

use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::identifiers::CatalogueItemNamespace;
use domain::tddd::catalogue_v2::roles::ItemAction;
use domain::tddd::catalogue_v2::{
    CatalogueEntryKey, DeletionRecord, FunctionPath, ModulePath, TypeRef,
};
use rustdoc_types::{
    FunctionHeader, FunctionSignature, Id, ItemEnum, ItemKind, Struct, StructKind,
    Trait as RustdocTrait,
};

use super::encoder::EncoderState;
use super::helpers::{empty_generics, make_item, make_item_with_crate_id};
use super::invalid_type_ref;
use crate::tddd::canonical_type_identity::canonicalize_catalogue_type_ref;

pub(super) fn encode_deletion_record(
    state: &mut EncoderState,
    item_actions: &mut BTreeMap<Id, ItemAction>,
    record: &DeletionRecord,
) -> Result<(), NewTypeGraphCodecError> {
    match record {
        DeletionRecord::Type { name, .. } => {
            encode_type_deletion(state, item_actions, name, CatalogueItemNamespace::Type)
        }
        DeletionRecord::Trait { name, .. } => {
            encode_trait_deletion(state, item_actions, name, CatalogueItemNamespace::Trait)
        }
        DeletionRecord::Function { path, .. } => {
            encode_function_deletion(state, item_actions, path)
        }
    }
}

fn encode_type_deletion(
    state: &mut EncoderState,
    item_actions: &mut BTreeMap<Id, ItemAction>,
    name: &CatalogueEntryKey,
    namespace: CatalogueItemNamespace,
) -> Result<(), NewTypeGraphCodecError> {
    let (type_id, item_name, module_path) = deletion_local_id(state, name, namespace)?;
    let item = make_item(
        type_id,
        Some(item_name.to_owned()),
        None,
        ItemEnum::Struct(Struct {
            kind: StructKind::Unit,
            generics: empty_generics(),
            impls: vec![],
        }),
    );
    state.index.insert(type_id, item);
    state.register_path(type_id, ItemKind::Struct, &item_name, &module_path);
    item_actions.insert(type_id, ItemAction::Delete);
    Ok(())
}

fn encode_trait_deletion(
    state: &mut EncoderState,
    item_actions: &mut BTreeMap<Id, ItemAction>,
    name: &CatalogueEntryKey,
    namespace: CatalogueItemNamespace,
) -> Result<(), NewTypeGraphCodecError> {
    let (trait_id, item_name, module_path) = deletion_local_id(state, name, namespace)?;
    let item = make_item(
        trait_id,
        Some(item_name.to_owned()),
        None,
        ItemEnum::Trait(RustdocTrait {
            is_auto: false,
            is_unsafe: false,
            is_dyn_compatible: true,
            items: vec![],
            generics: empty_generics(),
            bounds: vec![],
            implementations: vec![],
        }),
    );
    state.index.insert(trait_id, item);
    state.register_path(trait_id, ItemKind::Trait, &item_name, &module_path);
    item_actions.insert(trait_id, ItemAction::Delete);
    Ok(())
}

fn encode_function_deletion(
    state: &mut EncoderState,
    item_actions: &mut BTreeMap<Id, ItemAction>,
    path: &FunctionPath,
) -> Result<(), NewTypeGraphCodecError> {
    let fn_path = path.to_string();
    let fn_id = state.fn_path_to_id.get(&fn_path).copied().ok_or_else(|| {
        invalid_type_ref(&fn_path, "delete tombstone id not found (internal error)")
    })?;
    let crate_id = if path.crate_name.as_str() == state.crate_name.as_str() {
        0
    } else {
        state.ensure_external_crate(path.crate_name.as_str().to_owned())
    };
    let item = make_item_with_crate_id(
        crate_id,
        fn_id,
        Some(path.name.as_str().to_owned()),
        None,
        ItemEnum::Function(rustdoc_types::Function {
            sig: FunctionSignature { inputs: vec![], output: None, is_c_variadic: false },
            generics: empty_generics(),
            has_body: true,
            header: FunctionHeader {
                is_async: false,
                is_const: false,
                is_unsafe: false,
                abi: rustdoc_types::Abi::Rust,
            },
        }),
    );
    state.index.insert(fn_id, item);
    state.register_path_for_crate(
        fn_id,
        ItemKind::Function,
        path.name.as_str(),
        &path.module_path,
        &path.crate_name,
    );
    item_actions.insert(fn_id, ItemAction::Delete);
    Ok(())
}

fn deletion_local_id(
    state: &EncoderState,
    key: &CatalogueEntryKey,
    namespace: CatalogueItemNamespace,
) -> Result<(Id, String, ModulePath), NewTypeGraphCodecError> {
    let identity = state.resolve_catalogue_key_identity(key, namespace)?;
    let item_name = identity.name().as_str().to_owned();
    let module_path = identity.module_path().cloned().ok_or_else(|| {
        invalid_type_ref(
            key.as_str(),
            "delete catalogue identity has no authoritative module placement",
        )
    })?;
    let identity_ref = TypeRef::new(identity.to_string())
        .map_err(|_| invalid_type_ref(key.as_str(), "delete catalogue identity is invalid"))?;
    let namespace_paths = state.resolution_paths_for_namespace(namespace);
    let canonical =
        canonicalize_catalogue_type_ref(&identity_ref, &state.crate_name, &namespace_paths, &[])?;
    let id = state.local_id_for_identity_in_namespace(&canonical, namespace)?.ok_or_else(|| {
        invalid_type_ref(key.as_str(), "delete catalogue identity is not registered")
    })?;
    Ok((id, item_name, module_path))
}
