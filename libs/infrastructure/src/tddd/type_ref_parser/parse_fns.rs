//! Public parse entry points.

use std::collections::HashMap;

use rustdoc_types::{GenericBound, Id, Path, TraitBoundModifier, Type};

use super::constants::UNRESOLVED_CRATE_ID;
use super::generic_tokens;
use super::parse_ctx::{ParseCtx, bound_lifetimes_to_generic_params};
use super::precise_capture::convert_precise_capture;

/// Validates the declared generic-parameter context using plain lexical rules.
/// Type/bound strings themselves are not interpreted here; `syn` and chain ③
/// remain the source of syntax validity and mismatch visibility.
pub(crate) fn validate_generic_identifier_ambiguities(
    input: &str,
    generic_params: &[&str],
) -> Result<(), String> {
    generic_tokens::validate(input, generic_params)
}

pub(crate) fn parse_type_ref<F, G>(
    type_ref_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
) -> Result<Type, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    parse_type_ref_with_generics(
        type_ref_str,
        resolve_local,
        std_crate_id,
        external_crate_ids,
        emit_external_crate,
        &[],
    )
}

pub(crate) fn parse_type_ref_with_generics<F, G>(
    type_ref_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
    generic_params: &[&str],
) -> Result<Type, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    parse_type_ref_with_generics_inner(
        type_ref_str,
        resolve_local,
        std_crate_id,
        external_crate_ids,
        emit_external_crate,
        generic_params,
        false,
    )
}

/// Parses a type reference while preserving the source spelling of prelude
/// paths for lexical comparison with rustdoc output.
pub(crate) fn parse_type_ref_with_generics_preserving_spelling<F, G>(
    type_ref_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
    generic_params: &[&str],
) -> Result<Type, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    parse_type_ref_with_generics_inner(
        type_ref_str,
        resolve_local,
        std_crate_id,
        external_crate_ids,
        emit_external_crate,
        generic_params,
        true,
    )
}

fn parse_type_ref_with_generics_inner<F, G>(
    type_ref_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
    generic_params: &[&str],
    preserve_prelude_spelling: bool,
) -> Result<Type, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    validate_generic_identifier_ambiguities(type_ref_str, generic_params)?;
    let syn_type: syn::Type = syn::parse_str(type_ref_str)
        .map_err(|e| format!("syn parse error for `{type_ref_str}`: {e}"))?;
    let mut ctx = ParseCtx {
        resolve_local,
        external_crate_ids,
        emit_external_crate,
        std_crate_id,
        generic_params,
        preserve_prelude_spelling,
    };
    Ok(ctx.convert_type(&syn_type))
}

pub(crate) fn parse_syn_type(type_ref_str: &str) -> syn::Result<syn::Type> {
    syn::parse_str(type_ref_str)
}

pub(crate) fn parse_syn_type_param_bound(type_ref_str: &str) -> syn::Result<syn::TypeParamBound> {
    syn::parse_str(type_ref_str)
}

pub(crate) fn parse_generic_bound_with_generics<F, G>(
    bound_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
    generic_params: &[&str],
) -> Result<GenericBound, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    parse_generic_bound_with_generics_inner(
        bound_str,
        resolve_local,
        std_crate_id,
        external_crate_ids,
        emit_external_crate,
        generic_params,
        false,
    )
}

/// Parses a generic bound while preserving the source spelling of prelude
/// paths.  Alias declarations are compared lexically against rustdoc output,
/// so `Clone` must remain `Clone` rather than being expanded to
/// `std::clone::Clone` by the general type resolver.
pub(crate) fn parse_generic_bound_with_generics_preserving_spelling<F, G>(
    bound_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
    generic_params: &[&str],
) -> Result<GenericBound, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    parse_generic_bound_with_generics_inner(
        bound_str,
        resolve_local,
        std_crate_id,
        external_crate_ids,
        emit_external_crate,
        generic_params,
        true,
    )
}

fn parse_generic_bound_with_generics_inner<F, G>(
    bound_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
    generic_params: &[&str],
    preserve_prelude_spelling: bool,
) -> Result<GenericBound, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    validate_generic_identifier_ambiguities(bound_str, generic_params)?;
    let syn_bound: syn::TypeParamBound =
        syn::parse_str(bound_str).map_err(|e| format!("syn parse error for `{bound_str}`: {e}"))?;

    let mut ctx = ParseCtx {
        resolve_local,
        external_crate_ids,
        emit_external_crate,
        std_crate_id,
        generic_params,
        preserve_prelude_spelling,
    };
    match syn_bound {
        syn::TypeParamBound::Lifetime(lt) => Ok(GenericBound::Outlives(format!("'{}", lt.ident))),
        syn::TypeParamBound::Trait(tb) => {
            let modifier = match tb.modifier {
                syn::TraitBoundModifier::None => TraitBoundModifier::None,
                syn::TraitBoundModifier::Maybe(_) => TraitBoundModifier::Maybe,
            };
            let generic_params = bound_lifetimes_to_generic_params(tb.lifetimes.as_ref());
            let trait_path = ctx.resolve_trait_bound_path(&tb.path);
            Ok(GenericBound::TraitBound { trait_: trait_path, generic_params, modifier })
        }
        syn::TypeParamBound::PreciseCapture(capture) => Ok(convert_precise_capture(&capture)),
        _ => Ok(GenericBound::TraitBound {
            trait_: Path { path: bound_str.to_string(), id: Id(UNRESOLVED_CRATE_ID), args: None },
            generic_params: vec![],
            modifier: TraitBoundModifier::None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rustdoc_types::Type;

    #[test]
    fn test_plain_generic_name_parses() {
        let parsed = super::parse_type_ref_with_generics(
            "Option<T>",
            &|_| None,
            0,
            &HashMap::new(),
            &mut |_| 1,
            &["T"],
        );
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_keyword_generic_name_is_rejected_lexically() {
        let result = super::validate_generic_identifier_ambiguities("Vec<type>", &["type"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_raw_generic_name_is_rejected_lexically() {
        let result = super::validate_generic_identifier_ambiguities("Vec<T>", &["r#T"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_weak_keyword_generic_names_are_rejected_lexically() {
        for name in ["macro_rules", "raw", "safe"] {
            assert!(
                super::validate_generic_identifier_ambiguities("Vec<T>", &[name]).is_err(),
                "weak keyword `{name}` must be rejected"
            );
        }
    }

    #[test]
    fn test_primitive_spelled_generic_remains_a_plain_name() {
        let parsed = super::parse_type_ref_with_generics(
            "bool",
            &|_| None,
            0,
            &HashMap::new(),
            &mut |_| 1,
            &["Bool"],
        );
        assert_eq!(parsed, Ok(Type::Primitive("bool".to_owned())));
    }
}
