//! `TypeSignalsService` and `TypeSignalsInteractor`.
//!
//! Application service (driving port) and interactor for the
//! `sotp track type-signals` use case and its lenient pre-commit variant.
//!
//! Orchestrates track-status guard, layer-bindings resolution, and per-layer
//! signal evaluation. All I/O is performed via injected secondary ports —
//! no direct infrastructure calls.

mod interactor;
mod service;

pub use interactor::TypeSignalsInteractor;
pub use service::{TypeSignalsError, TypeSignalsRequest, TypeSignalsService};
