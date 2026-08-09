//! Inert-resolver conversions for the closed grammar's canonical round-trip.
//!
//! Rule A compares a catalogue string against the canonical rendering of its
//! own converted representation. Resolution affects only graph IDs and
//! external-crate registration, never the preserved spelling, so running the
//! real converter with an inert resolver context yields exactly the rendering
//! the real encoding pass would produce.

use std::collections::HashMap;

use rustdoc_types::Id;

use super::parse_ctx::ParseCtx;

pub(super) fn convert_bound_with_dummy_context(
    syn_bound: &syn::TypeParamBound,
    generic_params: &[&str],
) -> rustdoc_types::GenericBound {
    with_dummy_context(generic_params, |ctx| match syn_bound {
        syn::TypeParamBound::Lifetime(lt) => {
            rustdoc_types::GenericBound::Outlives(format!("'{}", lt.ident))
        }
        syn::TypeParamBound::Trait(tb) => {
            let modifier = match tb.modifier {
                syn::TraitBoundModifier::None => rustdoc_types::TraitBoundModifier::None,
                syn::TraitBoundModifier::Maybe(_) => rustdoc_types::TraitBoundModifier::Maybe,
            };
            let generic_params_rendered =
                super::parse_ctx::bound_lifetimes_to_generic_params(tb.lifetimes.as_ref());
            let trait_path = ctx.resolve_trait_bound_path(&tb.path);
            rustdoc_types::GenericBound::TraitBound {
                trait_: trait_path,
                generic_params: generic_params_rendered,
                modifier,
            }
        }
        syn::TypeParamBound::PreciseCapture(capture) => {
            super::precise_capture::convert_precise_capture(capture)
        }
        _ => rustdoc_types::GenericBound::TraitBound {
            trait_: rustdoc_types::Path {
                path: "<unsupported_bound>".to_owned(),
                id: Id(u32::MAX),
                args: None,
            },
            generic_params: vec![],
            modifier: rustdoc_types::TraitBoundModifier::None,
        },
    })
}

pub(super) fn convert_type_with_dummy_context(
    syn_type: &syn::Type,
    generic_params: &[&str],
) -> rustdoc_types::Type {
    with_dummy_context(generic_params, |ctx| ctx.convert_type(syn_type))
}

fn with_dummy_context<R>(
    generic_params: &[&str],
    convert: impl FnOnce(&mut ParseCtx<'_, fn(&str) -> Option<Id>, Box<dyn FnMut(String) -> u32>>) -> R,
) -> R {
    fn no_local(_name: &str) -> Option<Id> {
        None
    }
    let external_crate_ids: HashMap<String, u32> = HashMap::new();
    let mut emit: Box<dyn FnMut(String) -> u32> = Box::new(|_name: String| 0);
    let mut ctx = ParseCtx {
        resolve_local: &(no_local as fn(&str) -> Option<Id>),
        external_crate_ids: &external_crate_ids,
        emit_external_crate: &mut emit,
        std_crate_id: 0,
        generic_params,
        preserve_prelude_spelling: true,
    };
    convert(&mut ctx)
}
