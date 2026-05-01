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

// Re-export domain types needed by CLI test code (AC-03: no domain imports in apps/cli/src/).
pub use spec_code_consistency::{
    ConfidenceSignal, ConsistencyReport, Timestamp, TypeAction, TypeBaseline, TypeBaselineEntry,
    TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind, TypeGraph, TypeKind,
    check_consistency, consistency_report_to_findings, evaluate_consistency_from_components,
};

pub mod adr_signals;
pub mod architecture_rules;
pub mod canonical_modules;
pub mod catalogue_spec_refs;
pub mod catalogue_spec_signals;
pub mod convention_docs;
pub mod doc_links;
pub mod doc_patterns;
pub mod domain_purity;
pub mod domain_strings;
pub mod frontmatter;
pub mod latest_track;
pub mod layers;
pub mod merge_gate_adapter;
pub mod module_size;
pub mod orchestra;
pub mod plan_artifact_refs;
pub mod spec_attribution;
pub mod spec_code_consistency;
pub mod spec_frontmatter;
pub mod spec_signals;
pub mod spec_states;
pub mod tddd_layers;
pub mod tech_stack;
pub mod trusted_root;
pub mod usecase_purity;
pub mod view_freshness;
