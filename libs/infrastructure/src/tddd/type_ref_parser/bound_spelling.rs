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
