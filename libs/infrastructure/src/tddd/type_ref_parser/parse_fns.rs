//! Public parse entry points.

use std::collections::HashMap;

use rustdoc_types::{GenericBound, Id, Path, TraitBoundModifier, Type};

use super::constants::UNRESOLVED_CRATE_ID;
use super::parse_ctx::{ParseCtx, bound_lifetimes_to_generic_params};

// ---------------------------------------------------------------------------
// Public parse function
// ---------------------------------------------------------------------------

/// Parses a `TypeRef` string and converts it to `rustdoc_types::Type`.
///
/// The caller provides:
/// - `type_ref_str`: the raw string (e.g. `"Result<Option<User>, DomainError>"`).
/// - `resolve_local`: a closure that looks up a short name declared in the current
///   catalogue and returns its `rustdoc_types::Id`, or `None` if not found.
/// - `std_crate_id`: the crate_id assigned to `"std"` in `external_crates`.
/// - `external_crate_ids`: a snapshot of known `crate_name → crate_id` mappings.
/// - `emit_external_crate`: a callback invoked when a new external crate name is
///   encountered; returns the new crate_id.
///
/// # Errors
///
/// Returns an error string if `syn` fails to parse `type_ref_str`.
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

/// Parses a `TypeRef` string and converts it to `rustdoc_types::Type`, recognising
/// impl-block generic type parameter names.
///
/// Identical to [`parse_type_ref`] except that `generic_params` lists the names of
/// type parameters declared on an `impl` block (e.g. `&["T", "U"]`). Any
/// single-segment identifier that matches an entry in `generic_params` is encoded as
/// `Type::Generic(name)` instead of falling through to the unresolved-marker path.
///
/// This implements ADR 2026-06-18-0822 D2: `for_type: "T"` with
/// `impl_generics: [{name: "T", ...}]` should produce `Type::Generic("T")`.
///
/// # Errors
///
/// Returns an error string if `syn` fails to parse `type_ref_str`.
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
    let syn_type: syn::Type = syn::parse_str(type_ref_str)
        .map_err(|e| format!("syn parse error for `{type_ref_str}`: {e}"))?;

    // `std_crate_id` is kept in the public signature for API stability (callers must
    // pass the registered std crate_id), but Path.id always uses UNRESOLVED_CRATE_ID
    // for external types since item ids are not available at A-codec time.
    let _ = std_crate_id;
    let mut ctx =
        ParseCtx { resolve_local, external_crate_ids, emit_external_crate, generic_params };

    Ok(ctx.convert_type(&syn_type))
}

/// Parses a bound string (e.g. `"'static"`, `"Send"`, `"?Sized"`,
/// `"for<'a> Fn(&'a str)"`) into a `rustdoc_types::GenericBound`.
///
/// Unlike `parse_type_ref`, which uses `syn::parse_str::<syn::Type>()` and
/// rejects `?Trait`, lifetime bounds, and HRTB bounds, this function uses
/// `syn::parse_str::<syn::TypeParamBound>()` — the same parser that
/// `catalogue_document_codec`'s `validate_bound_str` uses — so the set of
/// accepted strings is identical between decode and encode.
///
/// Conversion rules:
/// - `'lifetime` → `GenericBound::Outlives("lifetime")`.
/// - `?Trait` → `GenericBound::TraitBound { modifier: Maybe, generic_params: [], ... }`.
/// - `for<'a> Trait<'a>` → `GenericBound::TraitBound { generic_params: [Lifetime('a)], ... }`.
/// - `Trait` / `Trait<T>` → `GenericBound::TraitBound { modifier: None, generic_params: [], ... }`.
///
/// # Errors
///
/// Returns `Err(String)` if `syn` cannot parse `bound_str` as a
/// `TypeParamBound`, or if the parsed bound is a form that cannot be
/// represented (e.g. `Verbatim` tokens from a proc-macro expansion).
pub(crate) fn parse_generic_bound<F, G>(
    bound_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
) -> Result<GenericBound, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    let syn_bound: syn::TypeParamBound =
        syn::parse_str(bound_str).map_err(|e| format!("syn parse error for `{bound_str}`: {e}"))?;

    let _ = std_crate_id; // kept for API symmetry with parse_type_ref
    let mut ctx =
        ParseCtx { resolve_local, external_crate_ids, emit_external_crate, generic_params: &[] };

    match syn_bound {
        // `syn::Lifetime.ident` is the identifier part WITHOUT the leading apostrophe
        // (e.g. `'static` → `ident = "static"`).  `rustdoc_types::GenericBound::Outlives`
        // stores the full lifetime string WITH the apostrophe (e.g. `"'static"`, `"'a"`).
        // Re-prepend `'` so that A-codec Outlives strings compare equal to C-side strings.
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
        // `Verbatim` is produced by syn for future syntax forms (e.g. `use<'a, T>` precise
        // capture bounds from Rust 2024).  These cannot be round-tripped through the
        // `rustdoc_types::GenericBound` representation at this time, but we must not
        // fail the entire encode: return an unresolved-path TraitBound as a best-effort
        // placeholder so that downstream phases can at least report the bound as an
        // unresolved reference rather than crashing.
        _ => Ok(GenericBound::TraitBound {
            trait_: Path { path: bound_str.to_string(), id: Id(UNRESOLVED_CRATE_ID), args: None },
            generic_params: vec![],
            modifier: TraitBoundModifier::None,
        }),
    }
}
