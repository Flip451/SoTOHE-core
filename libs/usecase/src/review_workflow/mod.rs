//! Pure workflow rules for local reviewer verdict normalization.

mod verdict;

pub use verdict::{
    REVIEW_OUTPUT_SCHEMA_JSON, ReviewFinalMessageState, ReviewFinalPayload, ReviewFinding,
    ReviewPayloadVerdict, ReviewVerdict, ReviewWorkflowError, classify_review_verdict,
    extract_verdict_from_content, normalize_final_message, parse_review_final_message,
    render_review_payload,
};
