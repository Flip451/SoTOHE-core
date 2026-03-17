//! Verification logic modules for `sotp verify` subcommands.
//!
//! Each submodule implements a specific verification check, returning
//! [`domain::verify::VerifyOutcome`] to the CLI layer.

pub mod architecture_rules;
pub mod canonical_modules;
pub mod convention_docs;
pub mod doc_patterns;
pub mod latest_track;
pub mod layers;
pub mod orchestra;
pub mod spec_attribution;
pub mod spec_frontmatter;
pub mod tech_stack;
