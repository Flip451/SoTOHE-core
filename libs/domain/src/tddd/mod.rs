//! TDDD (Type-Definition-Driven Development) module.
//!
//! Groups type catalogue definitions, signal evaluation, and consistency
//! checking for the per-track / per-layer type catalogue (e.g.
//! `domain-types.json`).
//!
//! Historical note (T001): the catalogue + signal + consistency logic used to
//! live in a single `catalogue.rs` (2088 lines). The TDDD-01 track split it
//! into three modules to meet DM-06's module-size guideline and enable the
//! layer-neutral rename from `DomainType*` to `TypeDefinition*` /
//! `TypeCatalogue*` / `TypeSignal` (ADR 0002 §D3).

// T008: baseline kept mod-scope (no pub re-export); signals removed.
// baseline.rs types are private to this crate — public re-exports removed from lib.rs.
mod baseline;
pub mod catalogue;
pub mod catalogue_linter;
pub mod catalogue_ports;
pub mod catalogue_spec_signal;
pub mod catalogue_to_extended_crate_port;
pub mod catalogue_v2;
pub mod consistency;
pub mod contract_map_content;
pub mod contract_map_options;
pub mod extended_crate;
pub mod layer_id;
pub mod new_typegraph_codec_error;
pub mod signal_evaluator;
pub mod type_signals_doc;

pub use catalogue_ports::{
    CatalogueLoader, CatalogueLoaderError, ContractMapWriter, ContractMapWriterError,
};
pub use catalogue_to_extended_crate_port::CatalogueToExtendedCratePort;
pub use contract_map_content::ContractMapContent;
pub use contract_map_options::ContractMapRenderOptions;
pub use extended_crate::ExtendedCrate;
pub use layer_id::LayerId;
pub use new_typegraph_codec_error::NewTypeGraphCodecError;
pub use signal_evaluator::{
    Phase1Error, SignalEvaluatorPort, SignalRegion, ThreeWayEvaluationReport, ThreeWaySignal,
    ThreeWaySignalKind,
};
// Note: `signal_for_region` is pub(crate) — use ThreeWaySignal::new() for public API.
