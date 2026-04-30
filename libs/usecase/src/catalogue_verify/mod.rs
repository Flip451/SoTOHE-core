//! Catalogue verification application services (usecase layer, T007).
//!
//! This module exposes usecase-layer application service traits and interactors
//! for all catalogue verification use cases so the CLI never imports domain
//! catalogue types directly (CN-01 / D1):
//!
//! - [`VerifyCatalogueConsistencyService`] / [`VerifyCatalogueConsistencyInteractor`]
//! - [`VerifyCatalogueSpecSignalsService`] / [`VerifyCatalogueSpecSignalsInteractor`]
//! - [`TypeSignalsService`] / [`TypeSignalsInteractor`]
//! - [`VerifyCatalogueSpecRefsService`] / [`VerifyCatalogueSpecRefsInteractor`]

pub mod consistency;
pub mod spec_refs;
pub mod spec_signals;
pub mod type_signals;

pub use consistency::{
    VerifyCatalogueConsistencyError, VerifyCatalogueConsistencyInteractor,
    VerifyCatalogueConsistencyOutput, VerifyCatalogueConsistencyService,
};
pub use spec_refs::{
    VerifyCatalogueSpecRefsError, VerifyCatalogueSpecRefsInteractor, VerifyCatalogueSpecRefsOutput,
    VerifyCatalogueSpecRefsService,
};
pub use spec_signals::{
    VerifyCatalogueSpecSignalsInteractor, VerifyCatalogueSpecSignalsService,
    VerifySpecSignalsError, VerifySpecSignalsOutput,
};
pub use type_signals::{
    LayerSignalSummary, TypeSignalsError, TypeSignalsInteractor, TypeSignalsService,
};
