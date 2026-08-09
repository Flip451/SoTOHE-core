//! Verification logic modules for `sotp verify` subcommands.
//!
//! Each submodule implements a specific verification check, returning
//! [`domain::verify::VerifyOutcome`] to the CLI layer.
//!
//! Re-exports `VerifyOutcome`, `VerifyFinding`, and `Severity` from the domain
//! layer so that `apps/cli/src/` can import these types through `infrastructure`
//! rather than directly from `domain` (CN-01 / AC-03 compliance path).

// Re-export core verify types so the CLI layer imports from here instead of `domain`.
pub use domain::verify::{Severity, VerifyFinding, VerifyOutcome};

pub mod adr_signals;
pub mod architecture_rules;
pub(crate) mod branch_implementation_inputs;
pub mod canonical_modules;
pub mod catalogue_spec_refs;
pub mod catalogue_spec_signals;
pub mod doc_links;
pub mod doc_patterns;
pub mod domain_purity;
pub mod domain_strings;
pub mod frontmatter;
pub(crate) mod git_inventory;
pub mod hooks_path;
pub(crate) mod implementation_input_hash;
pub mod latest_track;
pub mod layers;
pub mod machine_paths;
pub mod merge_gate_adapter;
pub mod module_size;
pub(crate) mod path_safety;
pub mod plan_artifact_refs;
pub mod retention_gate;
pub mod signal_gates_config;
pub mod sotp_version_tag;
pub mod spec_attribution;
pub mod spec_frontmatter;
pub mod spec_signals;
pub mod spec_states;
pub(crate) mod spec_states_legacy_markdown;
pub(crate) mod syn_helpers;
pub mod tddd_layers;
pub mod template_refs;
pub mod trusted_root;
pub mod usecase_purity;
pub mod view_freshness;

#[cfg(any(test, feature = "test-helpers"))]
pub mod test_support;
