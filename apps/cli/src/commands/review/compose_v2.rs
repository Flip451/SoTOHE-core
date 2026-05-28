//! v2 review system composition root — re-export shim.
//!
//! Thin re-export layer: composition logic lives in `cli_composition::review_v2`
//! so this file never imports `domain::` or `infrastructure::` types directly
//! (CN-01 / AC-03).
//!
//! Note: this shim is intentionally placed in `apps/cli` (NOT in
//! `libs/infrastructure`) to avoid a circular crate dependency:
//! infrastructure → cli_composition → infrastructure would create a cycle.

// Re-export composition types and builders used by sibling review command modules.
pub(crate) use cli_composition::review_v2::{
    build_scope_query_interactor_no_diff_str, build_scope_query_interactor_str,
    validate_scope_for_track_str,
};
