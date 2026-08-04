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
