//! ADR decision YAML front-matter codec and adapter modules.
//!
//! Mirrors the domain `adr_decision` module on the infrastructure side: this
//! is where `serde` derives live (CN-05 hexagonal rule), where raw YAML is
//! decoded into the domain [`domain::AdrFrontMatter`] aggregate, and where
//! filesystem-backed adapters bind the domain
//! [`domain::AdrFilePort`] secondary port to concrete I/O.
//!
//! - `dto` (private) — `serde`-derived DTOs (`AdrFrontMatterDto` /
//!   `AdrDecisionDto`) mirroring the YAML schema. Infrastructure-internal
//!   per CN-05; domain consumers receive the validated
//!   [`domain::AdrFrontMatter`] aggregate, not the DTOs.
//! - [`error`] — [`AdrFrontMatterCodecError`] variants raised by the codec.
//! - [`parse`] — [`parse_adr_frontmatter`] free function that turns raw
//!   markdown content into the domain [`domain::AdrFrontMatter`] value object.

mod dto;
pub mod error;
pub mod parse;

pub use error::AdrFrontMatterCodecError;
pub use parse::parse_adr_frontmatter;
