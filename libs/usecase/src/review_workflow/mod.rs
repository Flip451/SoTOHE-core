//! Pure workflow rules for local reviewer verdict normalization.

pub mod scope;
pub mod usecases;
mod verdict;

pub use scope::{
    DiffScope, DiffScopeProvider, DiffScopeProviderError, FindingScopeClass, RepoRelativePath,
    ScopeFilterResult, ScopeFilteredPayload, apply_scope_filter, classify_finding_scope,
    partition_findings_by_scope,
};
pub use verdict::{
    ModelProfile, REVIEW_OUTPUT_SCHEMA_JSON, ReviewFinalMessageState, ReviewFinalPayload,
    ReviewFinding, ReviewPayloadVerdict, ReviewVerdict, ReviewWorkflowError,
    classify_review_verdict, extract_verdict_from_content, normalize_final_message,
    parse_review_final_message, render_review_payload, resolve_full_auto,
};
