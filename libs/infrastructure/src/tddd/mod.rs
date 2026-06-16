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
pub mod baseline_graph_loader_adapter;
pub mod baseline_graph_renderer_adapter;
pub mod baseline_graph_writer_adapter;
pub mod baseline_rustdoc_codec;
pub mod catalogue_bulk_loader;
pub mod catalogue_document_codec;
pub mod catalogue_spec_signals_codec;
pub mod catalogue_spec_signals_refresher;
pub mod catalogue_to_extended_crate_codec;
pub mod catalogue_to_extended_crate_codec_error;
pub mod contract_map_adapter;
pub mod contract_map_renderer_adapter;
pub mod fs_catalogue_spec_signals_store;
pub mod fs_lint_config_loader;
pub(crate) mod mermaid_style;
pub mod rustdoc_baseline_capture_adapter;
pub mod rustdoc_crate_adapter;
pub mod semantic_verify_codec;
pub mod signal_evaluator_v2;
pub mod spec_ground_codec;
pub mod tddd_catalogue_document_loader;
pub mod tddd_layer_bindings_adapter;
pub mod type_graph_cluster;
pub mod type_graph_export;
pub mod type_graph_render;
pub mod type_ref_parser;
pub mod type_signals_codec;
pub mod type_signals_evaluator;
pub mod type_signals_executor_adapter;

// ---------------------------------------------------------------------------
// Shared test support
// ---------------------------------------------------------------------------

/// Test utilities shared across sibling TDDD codec test modules.
///
/// This module is compiled only in test builds (`#[cfg(test)]`). Child modules
/// reference it as `use super::super::test_support::...`.
#[cfg(test)]
pub(crate) mod test_support {
    /// Build a 64-character lowercase hex string by repeating `byte` 32 times.
    ///
    /// Useful for constructing deterministic fake [`ContentHash`] values in
    /// tests without depending on any particular hash function.
    pub(crate) fn hex_pattern(byte: u8) -> String {
        let mut s = String::with_capacity(64);
        for _ in 0..32 {
            s.push_str(&format!("{byte:02x}"));
        }
        s
    }
}
