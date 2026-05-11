//! TDDD infrastructure — codec and builder modules for the type catalogue.

// Re-export domain TDDD ports and signal types used by the CLI composition root.
// These re-exports allow the CLI (which depends on infrastructure but not domain)
// to access the traits needed to drive `CatalogueToExtendedCrateCodec` and
// `SignalEvaluatorV2`.
pub use domain::tddd::CatalogueToExtendedCratePort;
pub use domain::tddd::SignalEvaluatorPort;
pub use domain::tddd::signal_evaluator::region::{ThreeWaySignal, ThreeWaySignalKind};

// T008: baseline_builder stubbed (TypeGraph removed).
pub mod baseline_builder;
pub mod baseline_capture;
pub mod baseline_codec;
pub mod baseline_rustdoc_codec;
pub mod catalogue_bulk_loader;
pub mod catalogue_codec;
pub mod catalogue_document_codec;
pub mod catalogue_spec_signals_codec;
pub mod catalogue_spec_signals_refresher;
pub mod catalogue_to_extended_crate_codec;
pub mod catalogue_to_extended_crate_codec_error;
pub mod contract_map_adapter;
pub mod fs_catalogue_spec_signals_store;
pub mod in_memory_catalogue_linter;
pub mod signal_evaluator_v2;
pub mod spec_ground_codec;
pub mod type_graph_cluster;
pub mod type_graph_export;
pub mod type_graph_render;
pub mod type_ref_parser;
pub mod type_signals_codec;
pub mod type_signals_evaluator;
pub(crate) mod v3_stub;
