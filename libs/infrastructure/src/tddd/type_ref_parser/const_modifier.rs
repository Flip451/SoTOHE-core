//! Token-stream scan for unrepresentable `[const]` / `~const` bound-modifier
//! spellings.
//!
//! Unlike the `syn`-visitor guards in the sibling `bound_spelling` module,
//! this guard runs on the raw token stream: the modifier syntax it rejects
//! predates stable `syn` support, so a bare token scan is the only reliable
//! detection point.

use proc_macro2::{Delimiter, TokenStream, TokenTree};

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
