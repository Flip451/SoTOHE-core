//! Format helpers for `rustdoc_types` values.
//!
//! Provides short-name string representations of `Type`, `GenericArgs`,
//! `GenericBound`, `WherePredicate`, and `Abi` values used in Phase 2
//! structural equality checks.  All formatting uses L1 resolution (only
//! the last path segment is kept for named types).
//!
//! ## Sub-module layout
//!
//! - `abi`        — `format_abi`
//! - `canon`      — `apply_canon_to_str`, occurrence-key helpers
//! - `ty_base`    — `format_type`, `format_generic_args`, `format_hrtb_type_params`,
//!   `format_generic_bounds`
//! - `ty_canon`   — `format_type_with_canon`, `format_generic_args_with_canon`,
//!   `format_generic_bounds_with_canon`, `build_generic_canon_map`,
//!   binder-lifetime count helpers
//! - `ty_strip`   — `format_type_strip_type_params`, `format_generic_args_strip_type_params`,
//!   `format_generic_bounds_strip_type_params`
//! - `ty_occ`     — `format_type_with_canon_occ` and occurrence-aware helpers
//! - `where_pred` — `format_where_predicate_with_canon`

mod abi;
mod canon;
mod ty_base;
mod ty_canon;
mod ty_occ;
mod ty_strip;
mod where_pred;

pub(crate) use abi::format_abi;
pub(crate) use canon::apply_canon_to_str;
pub(crate) use ty_base::{
    ShortTypeFormatPolicy, format_dyn_trait_with, format_function_pointer_with,
    format_generic_args, format_impl_trait_with, format_qualified_path_with,
    format_short_type_with_policy, format_type,
};
pub(crate) use ty_canon::{
    build_generic_canon_map, build_generic_canon_map_from_groups, format_generic_bounds_with_canon,
    format_type_with_canon,
};
pub(crate) use ty_occ::format_type_with_canon_occ;
pub(crate) use ty_strip::format_type_strip_type_params;
pub(crate) use where_pred::format_where_predicate_with_canon;
