//! v2 review system composition root.
//!
//! Thin delegation layer: all wiring logic lives in
//! `infrastructure::review_v2::cli_composition` so this file never imports
//! `domain::` types directly (CN-01 / AC-03).

// Re-export composition types and builders used by sibling review command modules.
pub(crate) use infrastructure::review_v2::{
    build_scope_query_interactor_no_diff_str, build_scope_query_interactor_str,
    validate_scope_for_track_str,
};
