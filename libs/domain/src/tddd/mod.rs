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
pub mod baseline_document;
pub mod baseline_graph_ports;
pub mod catalog_gen;
pub mod catalogue;
pub mod catalogue_linter;
pub mod catalogue_ports;
pub mod catalogue_spec_signal;
pub mod catalogue_to_extended_crate_port;
pub mod catalogue_v2;
pub mod consistency;
pub mod contract_map_content;
pub mod contract_map_options;
pub mod contract_map_renderer;
pub mod extended_crate;
pub mod feature_declaration;
pub mod layer_id;
pub mod new_typegraph_codec_error;
pub mod primitive_occurrence_scanner;
pub mod semantic_verify;
pub mod signal_evaluator;
pub mod test_obligation;
pub mod type_signals_doc;

pub use type_signals_doc::{
    BaselineHash, CapturedRustdocJson, CargoProfileName, CatalogueDeclarationHash,
    ExpectedRustdocJsonPath, ImplementationFingerprint, ResolutionFingerprint,
    ResolvedCargoTargetDirectory, RustdocExecutionIdentity, RustdocExecutionIdentityError,
    RustdocJsonHash, RustdocSnapshot, Sha256Digest, Sha256DigestError, TYPE_SIGNALS_SCHEMA_VERSION,
    TypeSignalsCacheKey, TypeSignalsDocument, TypeSignalsLoadResult, TypeSignalsReuseDecision,
    TypeSignalsSchemaVersion, TypeSignalsSchemaVersionError, construct_captured_rustdoc_json,
    construct_rustdoc_snapshot,
};

pub use baseline_document::BaselineDocument;
pub use baseline_graph_ports::{
    BaselineGraphLoader, BaselineGraphLoaderError, BaselineGraphRenderer,
    BaselineGraphRendererError, BaselineGraphWriter, BaselineGraphWriterError, ClusterRender,
};
pub use catalogue_ports::{
    CatalogueLoader, CatalogueLoaderError, ContractMapWriter, ContractMapWriterError,
};
pub use catalogue_to_extended_crate_port::{
    AuthoritativeRustdocContext, CatalogueToExtendedCratePort,
};
pub use contract_map_content::ContractMapContent;
pub use contract_map_options::ContractMapRenderOptions;
pub use contract_map_renderer::{
    ContractMapRenderResult, ContractMapRenderWarning, ContractMapRenderer,
    ContractMapRendererError,
};
pub use extended_crate::ExtendedCrate;
pub use feature_declaration::{
    CargoFeatureName, CargoFeatureNameError, TdddFeatureDeclaration, TdddFeatureDeclarationError,
    TdddFeatureLookupError,
};
pub use layer_id::LayerId;
pub use new_typegraph_codec_error::NewTypeGraphCodecError;
pub use signal_evaluator::{
    Phase1Error, SignalEvaluatorPort, SignalRegion, ThreeWayEvaluationReport, ThreeWaySignal,
    ThreeWaySignalIdentity, ThreeWaySignalKind,
};
// Note: `signal_for_region` is pub(crate) — use the explicit ThreeWaySignal
// constructors for the public API.
