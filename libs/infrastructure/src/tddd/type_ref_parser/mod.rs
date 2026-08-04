//! TypeRef → `rustdoc_types::Type` conversion using the `syn` crate.
//!
//! Converts a `domain::tddd::catalogue_v2::TypeRef` string (e.g.
//! `"Result<Option<User>, DomainError>"`) into the equivalent
//! `rustdoc_types::Type` representation.
//!
//! ## Responsibilities
//!
//! * Parse the string via `syn::parse_str::<syn::Type>()`.
//! * Walk the `syn::Type` AST recursively and produce `rustdoc_types::Type`.
//! * Resolve each identifier against:
//!   1. Rust primitive names → `Type::Primitive`.
//!   2. The `Self` keyword → `Type::ResolvedPath` with sentinel `Id(0)`.
//!   3. std prelude allowlist → `Type::ResolvedPath`.
//!   4. Known identifiers with a crate prefix (e.g. `"domain_core::UserId"`) → external crate.
//!   5. Identifiers declared in the current catalogue (looked up via a closure).
//!   6. Anything else → an "unresolved marker" using sentinel crate_id `u32::MAX`.
//!
//! ## Unresolved marker
//!
//! Per ADR 2 D10, the A codec is open-world: identifiers that are not known at
//! codec time are recorded as unresolved markers rather than rejected.
//! Closed-world validation occurs in Phase 1 (Signal evaluator).
//!
//! (CN-08 / spec.json IN-09 / ADR 2 D9 / D10 / D11)

mod bound_spelling;
mod constants;
mod generic_tokens;
mod helpers;
mod parse_ctx;
mod parse_fns;
mod precise_capture;

// ---------------------------------------------------------------------------
// Re-exports — public surface of this module
// ---------------------------------------------------------------------------

pub(crate) use constants::{STD_PRELUDE_TYPES, UNRESOLVED_CRATE_ID};
pub(crate) use generic_tokens::is_plain_generic_param_name;
pub(crate) use helpers::{core_canonical_path, std_canonical_path};
pub(crate) use parse_fns::{
    parse_generic_bound_with_generics, parse_generic_bound_with_generics_preserving_spelling,
    parse_syn_type, parse_syn_type_param_bound, parse_type_ref, parse_type_ref_with_generics,
    parse_type_ref_with_generics_preserving_spelling, validate_generic_identifier_ambiguities,
    validate_legacy_type_ref, validate_lexical_generic_bound, validate_lexical_type_ref,
    validate_maybe_const_bound,
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
#[path = "../type_ref_parser_tests.rs"]
mod tests;
