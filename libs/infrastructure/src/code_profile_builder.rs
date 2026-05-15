//! T008: `code_profile_builder` is deleted.
//!
//! `build_type_graph(schema: &SchemaExport, ...) -> TypeGraph` is removed
//! because `TypeGraph` no longer exists in the domain layer.  Signal evaluation
//! now uses `rustdoc_types::Crate` / `ExtendedCrate` directly via `SignalEvaluatorV2`.
//!
//! This stub is kept so that `lib.rs` does not need to be modified simultaneously.
//! Callers (`spec_code_consistency`, `type_signals_evaluator`) are deleted in T008.
