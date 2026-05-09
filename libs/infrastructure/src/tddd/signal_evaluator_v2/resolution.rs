//! Phase 1.5 — closed-world unresolved-marker resolution.
//!
//! Resolves `Id(UNRESOLVED_CRATE_ID)` placeholders in S items to real Ids
//! by looking up bare type names in the S-universe name map.

use std::collections::{BTreeMap, HashSet};

use domain::tddd::Phase1Error;
use rustdoc_types::{Id, Item, ItemEnum};

use super::is_local_unresolved_path;
use super::resolve_type::{
    resolve_generic_args, resolve_generic_bound, resolve_generics, resolve_type,
};
use crate::tddd::type_ref_parser::UNRESOLVED_CRATE_ID;

pub(super) fn resolve_unresolved_in_item(
    item: Item,
    s_known_names: &HashSet<String>,
    s_name_to_id: &BTreeMap<String, Id>,
) -> Result<Item, Phase1Error> {
    let mut resolved_item = item;
    resolved_item.inner = match resolved_item.inner {
        ItemEnum::StructField(ty) => {
            ItemEnum::StructField(resolve_type(ty, s_known_names, s_name_to_id)?)
        }
        ItemEnum::TypeAlias(mut ta) => {
            ta.type_ = resolve_type(ta.type_, s_known_names, s_name_to_id)?;
            ta.generics = resolve_generics(ta.generics, s_known_names, s_name_to_id)?;
            ItemEnum::TypeAlias(ta)
        }
        ItemEnum::Struct(mut s) => {
            s.generics = resolve_generics(s.generics, s_known_names, s_name_to_id)?;
            ItemEnum::Struct(s)
        }
        ItemEnum::Enum(mut e) => {
            e.generics = resolve_generics(e.generics, s_known_names, s_name_to_id)?;
            ItemEnum::Enum(e)
        }
        ItemEnum::Trait(mut t) => {
            t.generics = resolve_generics(t.generics, s_known_names, s_name_to_id)?;
            // Also resolve supertrait bounds.
            let new_bounds: Result<Vec<_>, _> = t
                .bounds
                .into_iter()
                .map(|b| resolve_generic_bound(b, s_known_names, s_name_to_id))
                .collect();
            t.bounds = new_bounds?;
            ItemEnum::Trait(t)
        }
        ItemEnum::Function(mut f) => {
            let mut new_inputs = Vec::with_capacity(f.sig.inputs.len());
            for (name, ty) in f.sig.inputs {
                new_inputs.push((name, resolve_type(ty, s_known_names, s_name_to_id)?));
            }
            let new_output = match f.sig.output {
                Some(ty) => Some(resolve_type(ty, s_known_names, s_name_to_id)?),
                None => None,
            };
            f.sig = rustdoc_types::FunctionSignature {
                inputs: new_inputs,
                output: new_output,
                ..f.sig
            };
            f.generics = resolve_generics(f.generics, s_known_names, s_name_to_id)?;
            ItemEnum::Function(f)
        }
        ItemEnum::Impl(mut impl_) => {
            // Resolve unresolved markers in the `for` type.
            impl_.for_ = resolve_type(impl_.for_, s_known_names, s_name_to_id)?;
            // Resolve the trait path if it's an unresolved marker.
            if let Some(mut trait_path) = impl_.trait_ {
                if trait_path.id == Id(UNRESOLVED_CRATE_ID)
                    && is_local_unresolved_path(&trait_path.path)
                {
                    // Use last segment so `crate::nested::MyTrait` resolves to `MyTrait`.
                    let bare_name = trait_path.path.rsplit("::").next().unwrap_or(&trait_path.path);
                    if s_known_names.contains(bare_name) {
                        if let Some(&resolved_id) = s_name_to_id.get(bare_name) {
                            trait_path.id = resolved_id;
                        }
                    } else {
                        return Err(Phase1Error::UnresolvedTypeRef(trait_path.path.clone()));
                    }
                }
                let new_args = match trait_path.args {
                    Some(boxed) => {
                        Some(Box::new(resolve_generic_args(*boxed, s_known_names, s_name_to_id)?))
                    }
                    None => None,
                };
                impl_.trait_ = Some(rustdoc_types::Path {
                    path: trait_path.path,
                    id: trait_path.id,
                    args: new_args,
                });
            }
            impl_.generics = resolve_generics(impl_.generics, s_known_names, s_name_to_id)?;
            ItemEnum::Impl(impl_)
        }
        ItemEnum::AssocType { generics, bounds, type_ } => {
            // Resolve bounds and optional default.
            let new_bounds: Result<Vec<_>, _> = bounds
                .into_iter()
                .map(|b| resolve_generic_bound(b, s_known_names, s_name_to_id))
                .collect();
            let new_type = match type_ {
                Some(ty) => Some(resolve_type(ty, s_known_names, s_name_to_id)?),
                None => None,
            };
            let new_generics = resolve_generics(generics, s_known_names, s_name_to_id)?;
            ItemEnum::AssocType { generics: new_generics, bounds: new_bounds?, type_: new_type }
        }
        ItemEnum::AssocConst { type_, value } => {
            ItemEnum::AssocConst { type_: resolve_type(type_, s_known_names, s_name_to_id)?, value }
        }
        other => other,
    };
    Ok(resolved_item)
}
