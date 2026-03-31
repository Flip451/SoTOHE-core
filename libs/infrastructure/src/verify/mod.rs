//! Verification logic modules for `sotp verify` subcommands.
//!
//! Each submodule implements a specific verification check, returning
//! [`domain::verify::VerifyOutcome`] to the CLI layer.

pub mod architecture_rules;
pub mod canonical_modules;
pub mod convention_docs;
pub mod doc_links;
pub mod doc_patterns;
pub mod domain_purity;
pub mod domain_strings;
pub mod frontmatter;
pub mod latest_track;
pub mod layers;
pub mod module_size;
pub mod orchestra;
pub mod spec_attribution;
pub mod spec_coverage;
pub mod spec_frontmatter;
pub mod spec_signals;
pub mod spec_states;
pub mod tech_stack;
pub mod usecase_purity;
pub mod view_freshness;
