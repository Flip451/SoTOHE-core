//! Deletion/tombstone encoding for the catalogue-to-ExtendedCrate codec.
//!
//! These helpers keep delete-marked type, trait, and function entries aligned
//! with their pre-pass identities and `ItemAction::Delete` signals without
//! mixing tombstone construction into the main encoding pipeline.

use std::collections::BTreeMap;

use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::roles::ItemAction;
use domain::tddd::catalogue_v2::{CatalogueEntryKey, DeletionRecord, FunctionPath, ModulePath};
use rustdoc_types::{
    FunctionHeader, FunctionSignature, Id, ItemEnum, ItemKind, Struct, StructKind,
    Trait as RustdocTrait,
};

use super::encoder::EncoderState;
use super::helpers::{empty_generics, make_item, make_item_with_crate_id};
use super::invalid_type_ref;

pub(super) fn encode_deletion_record(
    state: &mut EncoderState,
    item_actions: &mut BTreeMap<Id, ItemAction>,
    record: &DeletionRecord,
) -> Result<(), NewTypeGraphCodecError> {
    match record {
        DeletionRecord::Type { name, .. } => encode_type_deletion(state, item_actions, name),
        DeletionRecord::Trait { name, .. } => encode_trait_deletion(state, item_actions, name),
        DeletionRecord::Function { path, .. } => {
            encode_function_deletion(state, item_actions, path)
        }
    }
}

fn encode_type_deletion(
    state: &mut EncoderState,
    item_actions: &mut BTreeMap<Id, ItemAction>,
    name: &CatalogueEntryKey,
) -> Result<(), NewTypeGraphCodecError> {
    let (_, item_name, module_path) = state.resolved_catalogue_key_path(name)?;
    let type_id = deletion_local_id(state, &module_path, &item_name)?;
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
) -> Result<(), NewTypeGraphCodecError> {
    let (_, item_name, module_path) = state.resolved_catalogue_key_path(name)?;
    let trait_id = deletion_local_id(state, &module_path, &item_name)?;
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
    module_path: &ModulePath,
    item_name: &str,
) -> Result<Id, NewTypeGraphCodecError> {
    state.local_id_for_catalogue_entry(module_path, item_name)
}
