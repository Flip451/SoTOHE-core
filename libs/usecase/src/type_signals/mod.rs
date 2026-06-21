//! `TypeSignalsService` and `TypeSignalsInteractor`.
//!
//! Application service (driving port) and interactor for the
//! `sotp signal calc-impl-catalog` use case.
//!
//! Orchestrates track-status guard, layer-bindings resolution, and per-layer
//! signal evaluation. Absent catalogue files are always skipped unconditionally;
//! present catalogues are always evaluated strictly. All I/O is performed via
//! injected secondary ports — no direct infrastructure calls.

mod interactor;
mod service;

pub use interactor::TypeSignalsInteractor;
pub use service::{TypeSignalsError, TypeSignalsRequest, TypeSignalsService};
