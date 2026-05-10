//! T008: `baseline_builder` is deleted.
//!
//! `build_baseline(graph: &TypeGraph, ...)` is no longer needed because
//! baselines are now stored as raw `rustdoc_types::Crate` JSON via
//! `capture_rustdoc_baseline_for_layer`.  This file is kept as a stub module
//! to avoid removing it from the `infrastructure::tddd` module declaration
//! (the declaration is cleaned up in the next step).
