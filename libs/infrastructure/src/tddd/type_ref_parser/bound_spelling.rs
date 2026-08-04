//! Lexical bound-spelling guards for the alias comparison path.
//!
//! Rustdoc's structural representation cannot retain every source spelling a
//! `syn`-parseable bound can carry. Each guard here rejects a spelling that
//! would either be lost in translation (turbofish, parenthesized bounds,
//! `[const]` modifiers) or that Rust itself refuses to compile (primitive or
//! generic-parameter-rooted trait paths), keeping the lexical comparison
//! fail-closed instead of silently comparing a lossy rendering.

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::visit::Visit;

use super::constants::PRIMITIVE_TYPES;

/// Rejects trait-shaped paths Rust itself rejects: bare primitives (`T: u8`) or paths rooted at
/// declared generic parameters (`T: U`). Multi-segment paths such as `str::pattern::Pattern` can
/// resolve to real traits and stay accepted.
pub(super) fn reject_deterministically_non_trait_bound(
    syntax: &syn::TypeParamBound,
    generic_params: &[&str],
) -> Result<(), String> {
    let mut visitor = DeterministicallyNonTraitBoundVisitor { generic_params, found: false };
    visitor.visit_type_param_bound(syntax);
    if visitor.found {
        Err("bounds must name a trait, not a primitive type or declared generic parameter"
            .to_owned())
    } else {
        Ok(())
    }
}

struct DeterministicallyNonTraitBoundVisitor<'params, 'names> {
    generic_params: &'params [&'names str],
    found: bool,
}

impl<'ast, 'params, 'names> Visit<'ast> for DeterministicallyNonTraitBoundVisitor<'params, 'names> {
    fn visit_type_param_bound(&mut self, node: &'ast syn::TypeParamBound) {
        if let syn::TypeParamBound::Trait(trait_bound) = node {
            let path = &trait_bound.path;
            if let Some(first) = path.segments.first() {
                let first_name = first.ident.to_string();
                self.found |= path.segments.len() == 1
                    && PRIMITIVE_TYPES.contains(&first_name.as_str())
                    || (path.leading_colon.is_none() || path.segments.len() == 1)
                        && self.generic_params.iter().any(|generic| *generic == first_name);
            }
        }
        syn::visit::visit_type_param_bound(self, node);
    }
}

/// Rustdoc's structural representation omits `syn`'s turbofish marker, so
/// `Tr<u8>` and `Tr::<u8>` would serialize identically and a notation
/// difference the alias lexical contract must surface would silently vanish.
/// The lexical path rejects the turbofish spelling instead of comparing it
/// lossily.
#[derive(Default)]
struct TurbofishArgumentVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for TurbofishArgumentVisitor {
    fn visit_angle_bracketed_generic_arguments(
        &mut self,
        node: &'ast syn::AngleBracketedGenericArguments,
    ) {
        self.found |= node.colon2_token.is_some();
        syn::visit::visit_angle_bracketed_generic_arguments(self, node);
    }
}

pub(super) fn reject_turbofish_generic_arguments_in_bound(
    syntax: &syn::TypeParamBound,
) -> Result<(), String> {
    let mut visitor = TurbofishArgumentVisitor::default();
    visitor.visit_type_param_bound(syntax);
    if visitor.found {
        Err("turbofish generic arguments are not supported by lexical type comparison".to_owned())
    } else {
        Ok(())
    }
}

/// Rustdoc and the converted representation keep only a bound's trait path,
/// so the parenthesized spelling `T: (Clone)` would serialize identically to
/// `T: Clone` and the notation difference the alias lexical contract must
/// surface would silently vanish. The lexical path rejects the parenthesized
/// spelling instead of comparing it lossily, mirroring the turbofish rule.
/// Grammatically required parentheses (`&(dyn A + B)`) are a `TypeParen`
/// node, not a `TraitBound` token, and stay accepted.
#[derive(Default)]
struct ParenthesizedBoundVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for ParenthesizedBoundVisitor {
    fn visit_trait_bound(&mut self, node: &'ast syn::TraitBound) {
        self.found |= node.paren_token.is_some();
        syn::visit::visit_trait_bound(self, node);
    }
}

pub(super) fn reject_parenthesized_bounds_in_bound(
    syntax: &syn::TypeParamBound,
) -> Result<(), String> {
    let mut visitor = ParenthesizedBoundVisitor::default();
    visitor.visit_type_param_bound(syntax);
    if visitor.found {
        Err("parenthesized bounds are not supported by lexical type comparison".to_owned())
    } else {
        Ok(())
    }
}

/// Rustdoc and `ParseCtx::convert_type` both erase redundant parentheses
/// around a nested type, so `Outer<(u8)>` and `Outer<u8>` would compare
/// equal despite differing canonical notation. Parentheses Rust's grammar
/// requires — around a multi-bound trait object directly behind `&` / `*`
/// (`&(dyn A + B)`) — carry no spelling variance (both sides must write
/// them) and stay accepted.
#[derive(Default)]
struct RedundantTypeParenVisitor {
    found: bool,
}

impl RedundantTypeParenVisitor {
    /// Recurses past a grammatically required paren (multi-bound trait object
    /// directly behind a reference or raw pointer) and reports whether it did.
    fn skip_required_paren(&mut self, elem: &syn::Type) -> bool {
        if let syn::Type::Paren(paren) = elem {
            if matches!(
                &*paren.elem,
                syn::Type::TraitObject(trait_object) if trait_object.bounds.len() > 1
            ) {
                self.visit_type(&paren.elem);
                return true;
            }
        }
        false
    }
}

impl<'ast> Visit<'ast> for RedundantTypeParenVisitor {
    fn visit_type_reference(&mut self, node: &'ast syn::TypeReference) {
        if self.skip_required_paren(&node.elem) {
            return;
        }
        syn::visit::visit_type_reference(self, node);
    }

    fn visit_type_ptr(&mut self, node: &'ast syn::TypePtr) {
        if self.skip_required_paren(&node.elem) {
            return;
        }
        syn::visit::visit_type_ptr(self, node);
    }

    fn visit_type_paren(&mut self, node: &'ast syn::TypeParen) {
        self.found = true;
        syn::visit::visit_type_paren(self, node);
    }
}

pub(super) fn reject_redundant_parenthesized_types_in_bound(
    syntax: &syn::TypeParamBound,
) -> Result<(), String> {
    let mut visitor = RedundantTypeParenVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_redundant_paren_found(visitor.found)
}

pub(super) fn reject_redundant_parenthesized_types_in_type(
    syntax: &syn::Type,
) -> Result<(), String> {
    let mut visitor = RedundantTypeParenVisitor::default();
    visitor.visit_type(syntax);
    reject_if_redundant_paren_found(visitor.found)
}

fn reject_if_redundant_paren_found(found: bool) -> Result<(), String> {
    if found {
        Err("redundant parenthesized type spellings are not supported by lexical type comparison"
            .to_owned())
    } else {
        Ok(())
    }
}

/// A trait object collapses into a trait list plus a separate lifetime field
/// in both the converted and rustdoc representations, so `dyn 'static + Tr`
/// and `dyn Tr + 'static` would compare equal despite differing bound order.
/// Only the representable spelling — every lifetime bound written after the
/// trait bounds — is accepted; a lifetime bound preceding a trait bound is
/// rejected. `impl Trait` keeps its ordered bound list and is unaffected.
#[derive(Default)]
struct NonFinalDynLifetimeVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for NonFinalDynLifetimeVisitor {
    fn visit_type_trait_object(&mut self, node: &'ast syn::TypeTraitObject) {
        let mut seen_lifetime = false;
        for bound in &node.bounds {
            if matches!(bound, syn::TypeParamBound::Lifetime(_)) {
                seen_lifetime = true;
            } else if seen_lifetime {
                self.found = true;
            }
        }
        syn::visit::visit_type_trait_object(self, node);
    }
}

pub(super) fn reject_non_final_dyn_lifetimes_in_bound(
    syntax: &syn::TypeParamBound,
) -> Result<(), String> {
    let mut visitor = NonFinalDynLifetimeVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_non_final_dyn_lifetime_found(visitor.found)
}

pub(super) fn reject_non_final_dyn_lifetimes_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = NonFinalDynLifetimeVisitor::default();
    visitor.visit_type(syntax);
    reject_if_non_final_dyn_lifetime_found(visitor.found)
}

fn reject_if_non_final_dyn_lifetime_found(found: bool) -> Result<(), String> {
    if found {
        Err(
            "trait-object lifetime bounds written before a trait bound are not supported by lexical type comparison"
                .to_owned(),
        )
    } else {
        Ok(())
    }
}

/// The converter and rustdoc both drop trailing punctuation, so `Tr<u8,>`
/// and `Tr<u8>` would compare equal despite differing canonical notation.
/// Trailing commas are rejected wherever they are redundant; the semantic
/// positions — a one-element tuple (`(u8,)`) and the comma before a variadic
/// (`fn(u8, ...)`) — stay accepted.
#[derive(Default)]
struct TrailingCommaVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for TrailingCommaVisitor {
    fn visit_bound_lifetimes(&mut self, node: &'ast syn::BoundLifetimes) {
        self.found |= node.lifetimes.trailing_punct();
        syn::visit::visit_bound_lifetimes(self, node);
    }

    fn visit_angle_bracketed_generic_arguments(
        &mut self,
        node: &'ast syn::AngleBracketedGenericArguments,
    ) {
        self.found |= node.args.trailing_punct();
        syn::visit::visit_angle_bracketed_generic_arguments(self, node);
    }

    fn visit_parenthesized_generic_arguments(
        &mut self,
        node: &'ast syn::ParenthesizedGenericArguments,
    ) {
        self.found |= node.inputs.trailing_punct();
        syn::visit::visit_parenthesized_generic_arguments(self, node);
    }

    fn visit_precise_capture(&mut self, node: &'ast syn::PreciseCapture) {
        self.found |= node.params.trailing_punct();
        syn::visit::visit_precise_capture(self, node);
    }

    fn visit_type_bare_fn(&mut self, node: &'ast syn::TypeBareFn) {
        self.found |= (node.variadic.is_none() && node.inputs.trailing_punct())
            || node.variadic.as_ref().is_some_and(|variadic| variadic.comma.is_some());
        syn::visit::visit_type_bare_fn(self, node);
    }

    fn visit_type_tuple(&mut self, node: &'ast syn::TypeTuple) {
        self.found |= node.elems.len() > 1 && node.elems.trailing_punct();
        syn::visit::visit_type_tuple(self, node);
    }
}

pub(super) fn reject_trailing_commas_in_bound(syntax: &syn::TypeParamBound) -> Result<(), String> {
    let mut visitor = TrailingCommaVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_trailing_comma_found(visitor.found)
}

pub(super) fn reject_trailing_commas_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = TrailingCommaVisitor::default();
    visitor.visit_type(syntax);
    reject_if_trailing_comma_found(visitor.found)
}

fn reject_if_trailing_comma_found(found: bool) -> Result<(), String> {
    if found {
        Err("redundant trailing commas are not supported by lexical type comparison".to_owned())
    } else {
        Ok(())
    }
}

/// The converter and rustdoc discard trailing `+` punctuation from trait
/// objects, `impl Trait`, associated-type constraints, and higher-ranked
/// lifetime bounds. They also discard an empty colon on an HRTB lifetime
/// parameter. Each notation difference would otherwise silently compare equal,
/// so it is rejected at the boundary.
#[derive(Default)]
struct TrailingPlusVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for TrailingPlusVisitor {
    fn visit_type_trait_object(&mut self, node: &'ast syn::TypeTraitObject) {
        self.found |= node.bounds.trailing_punct();
        syn::visit::visit_type_trait_object(self, node);
    }

    fn visit_type_impl_trait(&mut self, node: &'ast syn::TypeImplTrait) {
        self.found |= node.bounds.trailing_punct();
        syn::visit::visit_type_impl_trait(self, node);
    }

    fn visit_constraint(&mut self, node: &'ast syn::Constraint) {
        self.found |= node.bounds.trailing_punct();
        syn::visit::visit_constraint(self, node);
    }

    fn visit_lifetime_param(&mut self, node: &'ast syn::LifetimeParam) {
        self.found |=
            node.bounds.trailing_punct() || (node.colon_token.is_some() && node.bounds.is_empty());
        syn::visit::visit_lifetime_param(self, node);
    }
}

pub(super) fn reject_trailing_pluses_in_bound(syntax: &syn::TypeParamBound) -> Result<(), String> {
    let mut visitor = TrailingPlusVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_trailing_plus_found(visitor.found)
}

pub(super) fn reject_trailing_pluses_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = TrailingPlusVisitor::default();
    visitor.visit_type(syntax);
    reject_if_trailing_plus_found(visitor.found)
}

fn reject_if_trailing_plus_found(found: bool) -> Result<(), String> {
    if found {
        Err("trailing `+` bound punctuation is not supported by lexical type comparison".to_owned())
    } else {
        Ok(())
    }
}

/// Bare-function spellings the representation cannot retain: an explicitly
/// written `_` parameter name (`fn(_: u8)`) collapses to the same form as an
/// omitted name (`fn(u8)`), a named variadic (`args: ...`) is discarded, and
/// several ABI spellings normalize to the same representation. In particular,
/// `extern fn()` becomes explicit C, `extern "Rust" fn()` becomes plain Rust,
/// and raw or escaped ABI literals lose their source spelling. Those variants
/// are rejected; canonical omitted-name, unnamed-variadic, and explicit ABI
/// forms stay accepted.
#[derive(Default)]
struct BareFnVariantSpellingVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for BareFnVariantSpellingVisitor {
    fn visit_bare_fn_arg(&mut self, node: &'ast syn::BareFnArg) {
        self.found |= node.name.as_ref().is_some_and(|(ident, _)| ident == "_");
        syn::visit::visit_bare_fn_arg(self, node);
    }

    fn visit_bare_variadic(&mut self, node: &'ast syn::BareVariadic) {
        self.found |= node.name.is_some();
        syn::visit::visit_bare_variadic(self, node);
    }

    fn visit_type_bare_fn(&mut self, node: &'ast syn::TypeBareFn) {
        self.found |= node.abi.as_ref().is_some_and(|abi| {
            let Some(name) = &abi.name else {
                return true;
            };
            let value = name.value();
            value == "Rust" || name.token().to_string() != format!("{value:?}")
        });
        syn::visit::visit_type_bare_fn(self, node);
    }
}

pub(super) fn reject_bare_fn_variant_spellings_in_bound(
    syntax: &syn::TypeParamBound,
) -> Result<(), String> {
    let mut visitor = BareFnVariantSpellingVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_bare_fn_variant_spelling_found(visitor.found)
}

pub(super) fn reject_bare_fn_variant_spellings_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = BareFnVariantSpellingVisitor::default();
    visitor.visit_type(syntax);
    reject_if_bare_fn_variant_spelling_found(visitor.found)
}

fn reject_if_bare_fn_variant_spelling_found(found: bool) -> Result<(), String> {
    if found {
        Err(
            "non-canonical bare-function parameter, variadic, and ABI spellings are not supported by lexical type comparison"
                .to_owned(),
        )
    } else {
        Ok(())
    }
}

/// Rustc rejects `impl Trait` in generic arguments (E0562) and `Self` in
/// top-level type-alias declarations (E0411), so either node would record an
/// alias contract no implementation can compile. Both are rejected at the
/// boundary.
#[derive(Default)]
struct AliasInvalidNodeVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for AliasInvalidNodeVisitor {
    fn visit_type_impl_trait(&mut self, node: &'ast syn::TypeImplTrait) {
        self.found = true;
        syn::visit::visit_type_impl_trait(self, node);
    }

    fn visit_path(&mut self, node: &'ast syn::Path) {
        self.found |= node.segments.iter().any(|segment| segment.ident == "Self");
        syn::visit::visit_path(self, node);
    }
}

pub(super) fn reject_alias_invalid_nodes_in_bound(
    syntax: &syn::TypeParamBound,
) -> Result<(), String> {
    let mut visitor = AliasInvalidNodeVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_alias_invalid_node_found(visitor.found)
}

pub(super) fn reject_alias_invalid_nodes_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = AliasInvalidNodeVisitor::default();
    visitor.visit_type(syntax);
    reject_if_alias_invalid_node_found(visitor.found)
}

fn reject_if_alias_invalid_node_found(found: bool) -> Result<(), String> {
    if found {
        Err("`impl Trait` and `Self` are not valid in type-alias declarations".to_owned())
    } else {
        Ok(())
    }
}

/// The alias model declares only type parameters, so a lifetime bound can
/// reference no declared lifetime: rustc rejects `type A<T: 'a> = T` for the
/// undeclared `'a`. Only `'static` is always in scope and stays accepted.
pub(super) fn reject_undeclared_lifetime_bound(syntax: &syn::TypeParamBound) -> Result<(), String> {
    if let syn::TypeParamBound::Lifetime(lifetime) = syntax {
        if lifetime.ident != "static" {
            return Err(format!(
                "lifetime bound `'{}` references no declarable lifetime parameter; only `'static` is supported",
                lifetime.ident
            ));
        }
    }
    Ok(())
}

/// Rust rejects the infer placeholder `_` in type-alias item signatures
/// (E0121), so a bound such as `T: Outer<_>` would record an alias contract
/// no implementation can compile. Nested `Type::Infer` nodes are rejected at
/// the boundary.
#[derive(Default)]
struct InferPlaceholderVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for InferPlaceholderVisitor {
    fn visit_type_infer(&mut self, node: &'ast syn::TypeInfer) {
        self.found = true;
        syn::visit::visit_type_infer(self, node);
    }
}

pub(super) fn reject_infer_placeholders_in_bound(
    syntax: &syn::TypeParamBound,
) -> Result<(), String> {
    let mut visitor = InferPlaceholderVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_infer_placeholder_found(visitor.found)
}

pub(super) fn reject_infer_placeholders_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = InferPlaceholderVisitor::default();
    visitor.visit_type(syntax);
    reject_if_infer_placeholder_found(visitor.found)
}

fn reject_if_infer_placeholder_found(found: bool) -> Result<(), String> {
    if found {
        Err("infer placeholders (`_`) are not valid in type-alias signatures".to_owned())
    } else {
        Ok(())
    }
}

/// Both the binder conversion and rustdoc omit attributes on higher-ranked
/// lifetime parameters (`for<#[allow(unused)] 'a> Tr<&'a u8>`), so the
/// attributed spelling would compare equal to the plain `for<'a>` form.
/// Attribute-bearing binder parameters are rejected at the boundary.
#[derive(Default)]
struct AttributedBinderParamVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for AttributedBinderParamVisitor {
    fn visit_bound_lifetimes(&mut self, node: &'ast syn::BoundLifetimes) {
        self.found |= node.lifetimes.iter().any(|param| match param {
            syn::GenericParam::Lifetime(lifetime_param) => !lifetime_param.attrs.is_empty(),
            syn::GenericParam::Type(type_param) => !type_param.attrs.is_empty(),
            syn::GenericParam::Const(const_param) => !const_param.attrs.is_empty(),
        });
        syn::visit::visit_bound_lifetimes(self, node);
    }
}

pub(super) fn reject_attributed_binder_params_in_bound(
    syntax: &syn::TypeParamBound,
) -> Result<(), String> {
    let mut visitor = AttributedBinderParamVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_attributed_binder_param_found(visitor.found)
}

pub(super) fn reject_attributed_binder_params_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = AttributedBinderParamVisitor::default();
    visitor.visit_type(syntax);
    reject_if_attributed_binder_param_found(visitor.found)
}

fn reject_if_attributed_binder_param_found(found: bool) -> Result<(), String> {
    if found {
        Err(
            "attributes on higher-ranked binder parameters are not supported by lexical type comparison"
                .to_owned(),
        )
    } else {
        Ok(())
    }
}

/// Rustdoc evaluates attributes on bare-function parameters (a
/// `#[cfg(any())]` argument disappears from the emitted signature), while the
/// lexical converter has no attribute evaluator — reproducing one would
/// reimplement the compiler. Attribute-bearing bare-function arguments and
/// variadics are rejected at the boundary instead of comparing a signature
/// rustdoc may have rewritten.
#[derive(Default)]
struct AttributedBareFnArgVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for AttributedBareFnArgVisitor {
    fn visit_bare_fn_arg(&mut self, node: &'ast syn::BareFnArg) {
        self.found |= !node.attrs.is_empty();
        syn::visit::visit_bare_fn_arg(self, node);
    }

    fn visit_bare_variadic(&mut self, node: &'ast syn::BareVariadic) {
        self.found |= !node.attrs.is_empty();
        syn::visit::visit_bare_variadic(self, node);
    }
}

pub(super) fn reject_attributed_bare_fn_args_in_bound(
    syntax: &syn::TypeParamBound,
) -> Result<(), String> {
    let mut visitor = AttributedBareFnArgVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_attributed_bare_fn_arg_found(visitor.found)
}

pub(super) fn reject_attributed_bare_fn_args_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = AttributedBareFnArgVisitor::default();
    visitor.visit_type(syntax);
    reject_if_attributed_bare_fn_arg_found(visitor.found)
}

fn reject_if_attributed_bare_fn_arg_found(found: bool) -> Result<(), String> {
    if found {
        Err("attributes on bare-function parameters are not supported by lexical type comparison"
            .to_owned())
    } else {
        Ok(())
    }
}

/// Precise-capture syntax (`use<'a, T>`) is only valid on `impl Trait`
/// opaque types; Rust rejects it as a generic-parameter or where-predicate
/// bound, so admitting it would record an alias contract no implementation
/// can compile. Reject the top-level form at the alias validation boundary.
pub(super) fn reject_precise_capture_bound(syntax: &syn::TypeParamBound) -> Result<(), String> {
    if matches!(syntax, syn::TypeParamBound::PreciseCapture(_)) {
        Err("`use<..>` precise-capture lists are not valid generic-parameter bounds".to_owned())
    } else {
        Ok(())
    }
}

pub(super) fn reject_unsupported_const_bound_modifier(bound_str: &str) -> Result<(), String> {
    let scan_str = bound_str.strip_prefix("~const").map_or(bound_str, str::trim_start);
    let tokens: TokenStream =
        scan_str.parse().map_err(|e| format!("invalid bound token stream '{bound_str}': {e}"))?;
    if contains_const_bound_modifier(tokens) {
        Err("`[const]` bound modifiers are not supported by lexical type comparison".to_owned())
    } else {
        Ok(())
    }
}

fn contains_const_bound_modifier(tokens: TokenStream) -> bool {
    // A `const` identifier immediately after `*` is raw-pointer type syntax
    // (`*const u8`), not a bound modifier; every other bare `const` stays
    // fail-closed as modifier syntax alongside the `[const]` token group.
    let mut after_star = false;
    for token in tokens {
        match &token {
            TokenTree::Ident(ident) if *ident == "const" && !after_star => return true,
            TokenTree::Group(group) => {
                if (group.delimiter() == Delimiter::Bracket
                    && is_const_modifier_tokens(group.stream()))
                    || contains_const_bound_modifier(group.stream())
                {
                    return true;
                }
            }
            _ => {}
        }
        after_star = matches!(&token, TokenTree::Punct(punct) if punct.as_char() == '*');
    }
    false
}

fn is_const_modifier_tokens(tokens: TokenStream) -> bool {
    let mut iter = tokens.into_iter();
    matches!(
        (iter.next(), iter.next()),
        (Some(TokenTree::Ident(ident)), None) if ident == "const"
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_const_bound_modifier_detection_is_token_wise() {
        for input in ["[const] Clone", "[ const ] Clone", "for<'a> [const] Clone"] {
            assert!(super::reject_unsupported_const_bound_modifier(input).is_err());
        }
        assert!(super::reject_unsupported_const_bound_modifier("[constant] Clone").is_ok());
    }

    #[test]
    fn test_raw_pointer_const_is_not_a_bound_modifier() {
        for input in ["Outer<*const u8>", "Outer<fn(*const u8)>", "Outer<*const u8, *mut u8>"] {
            assert!(
                super::reject_unsupported_const_bound_modifier(input).is_ok(),
                "raw-pointer `const` must not be treated as a bound modifier: {input}"
            );
        }
        for input in ["const Clone", "Outer<~const Clone>"] {
            assert!(
                super::reject_unsupported_const_bound_modifier(input).is_err(),
                "modifier-position `const` must fail closed: {input}"
            );
        }
    }
}
