//! T008: `baseline_codec` is deleted.
//!
//! The codec serialised `TypeBaseline` / `TypeBaselineEntry` / `FunctionBaselineEntry`
//! / `TraitBaselineEntry` / `TraitImplBaselineEntry` — all of which are removed in T008
//! along with `TypeGraph`. The rustdoc-format baseline (captured via `cargo +nightly rustdoc`)
//! now lives in `<layer>-types-baseline.json` and is decoded by `baseline_rustdoc_codec`.
