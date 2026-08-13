//! Conversion for Rust 2024 precise-capturing bounds (`use<'a, T>`).

use rustdoc_types::{GenericBound, PreciseCapturingArg};

use super::parse_ctx::normalized_ident_name;

pub(super) fn convert_precise_capture(capture: &syn::PreciseCapture) -> GenericBound {
    let params = capture
        .params
        .iter()
        .map(|param| match param {
            syn::CapturedParam::Lifetime(lifetime) => {
                PreciseCapturingArg::Lifetime(format!("'{}", lifetime.ident))
            }
            syn::CapturedParam::Ident(ident) => {
                PreciseCapturingArg::Param(normalized_ident_name(ident))
            }
            // `CapturedParam` is non-exhaustive for forward compatibility.
            // Preserve an explicit marker instead of silently dropping the entry.
            _ => PreciseCapturingArg::Param("<unsupported-capture-param>".to_owned()),
        })
        .collect();
    GenericBound::Use(params)
}
