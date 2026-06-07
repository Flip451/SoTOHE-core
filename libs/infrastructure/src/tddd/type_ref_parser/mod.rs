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

mod constants;
mod helpers;
mod parse_ctx;
mod parse_fns;

// ---------------------------------------------------------------------------
// Re-exports — public surface of this module
// ---------------------------------------------------------------------------

pub(crate) use constants::UNRESOLVED_CRATE_ID;
pub(crate) use helpers::{core_canonical_path, std_canonical_path};
pub(crate) use parse_fns::{parse_generic_bound, parse_type_ref};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
#[path = "../type_ref_parser_tests.rs"]
mod tests;
