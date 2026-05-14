<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ClusterPlan | value_object | delete | — | 🔵 | 🔵 |
| CrossEdge | value_object | delete | — | 🔵 | 🔵 |
| EdgeSet | value_object | delete | — | 🔵 | 🔵 |
| SignalSummary | value_object | delete | — | 🔵 | 🔵 |
| TypeGraphRenderOptions | value_object | delete | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineCodecError | error_type | delete | Json, UnsupportedSchemaVersion, MissingField | 🔵 | 🔵 |
| BaselineRustdocCodecError | error_type | — | Json, IoError, UnsupportedFormatVersion | 🔵 | 🔵 |
| CaptureBaselineError | error_type | reference | — | 🔵 | 🔵 |
| CatalogueDocumentCodecError | error_type | — | Json, Io, UnsupportedSchemaVersion, InvalidEntry, CrateNameMismatch, CrossCrateFunctionPath | 🔵 | 🔵 |
| CatalogueToExtendedCrateCodecError | error_type | — | InvalidTypeRef, AmbiguousIdentifier | 🔵 | 🔵 |
| EvaluateSignalsError | error_type | reference | — | 🔵 | 🔵 |
| LoadAllCataloguesError | error_type | delete | LayerBindings, ArchRulesParse, Io, CatalogueNotFound, Decode, TopologicalSortFailed, InvalidLayerId | 🔵 | 🔵 |
| LoadAllCataloguesNativeError | error_type | — | LayerBindings, ArchRulesParse, Io, CatalogueNotFound, Decode, TopologicalSortFailed, InvalidLayerId | 🔵 | 🔵 |
| SchemaExportCodecError | error_type | reference | Json | 🔵 | 🔵 |
| TypeCatalogueCodecError | error_type | delete | Json, Validation, UnsupportedSchemaVersion, InvalidEntry | 🔵 | 🔵 |
| TypeGraphExportError | error_type | delete | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineRustdocCodec | secondary_adapter | — | impl Debug, impl Clone | 🔵 | 🔵 |
| CatalogueDocumentCodec | secondary_adapter | — | impl Debug, impl Clone, impl Default | 🔵 | 🔵 |
| CatalogueToExtendedCrateCodec | secondary_adapter | — | impl Debug, impl Clone, impl CatalogueToExtendedCratePort, impl Default | 🔵 | 🔵 |
| FsCatalogueDocumentLoader | secondary_adapter | — | impl Debug, impl Clone, impl CatalogueDocumentLoaderPort, impl Default | 🔵 | 🔵 |
| FsCatalogueLoader | secondary_adapter | modify | impl CatalogueLoader | 🔵 | 🔵 |
| FsCatalogueSpecSignalsStore | secondary_adapter | reference | impl CatalogueSpecSignalsWriter | 🔵 | 🔵 |
| FsContractMapWriter | secondary_adapter | reference | impl ContractMapWriter | 🔵 | 🔵 |
| FsSymlinkGuard | secondary_adapter | — | impl SymlinkGuardPort, impl Default | 🔵 | 🔵 |
| FsTdddLayerBindingsAdapter | secondary_adapter | — | impl Debug, impl Clone, impl TdddLayerBindingsPort, impl Default | 🔵 | 🔵 |
| GitShowTrackBlobReader | secondary_adapter | modify | impl TrackBlobReader, impl SpecElementHashReader | 🔵 | 🔵 |
| InMemoryCatalogueLinter | secondary_adapter | modify | impl CatalogueLinter, impl Default | 🔵 | 🔵 |
| RustdocCrateAdapter | secondary_adapter | — | impl RustdocCratePort | 🔵 | 🔵 |
| RustdocSchemaExporter | secondary_adapter | modify | impl SchemaExporter, impl SchemaExporterPort | 🔵 | 🔵 |
| SignalEvaluatorV2 | secondary_adapter | — | impl Debug, impl Clone, impl SignalEvaluatorPort, impl Default | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::code_profile_builder::build_type_graph | free_function | delete | fn() -> TypeGraph | 🔵 | 🔵 |
| infrastructure::tddd::baseline_builder::build_baseline | free_function | delete | fn() -> TypeBaseline | 🔵 | 🔵 |
| infrastructure::tddd::baseline_capture::capture_baseline_for_layer | free_function | delete | fn() -> Result<(), BaselineCodecError> | 🔵 | 🔵 |
| infrastructure::tddd::baseline_capture::capture_rustdoc_baseline_for_layer | free_function | — | fn(items_dir: &std::path::Path, track_id: &str, workspace_root: &std::path::Path, binding: &infrastructure::verify::tddd_layers::TdddLayerBinding) -> Result<(), CaptureBaselineError> | 🔵 | 🔵 |
| infrastructure::tddd::baseline_capture::force_capture_rustdoc_baseline_for_layer | free_function | — | fn(items_dir: &std::path::Path, track_id: &str, workspace_root: &std::path::Path, binding: &infrastructure::verify::tddd_layers::TdddLayerBinding) -> Result<(), CaptureBaselineError> | 🔵 | 🔵 |
| infrastructure::tddd::baseline_codec::decode | free_function | delete | fn() -> Result<TypeBaseline, BaselineCodecError> | 🔵 | 🔵 |
| infrastructure::tddd::baseline_codec::encode | free_function | delete | fn() -> Result<String, BaselineCodecError> | 🔵 | 🔵 |
| infrastructure::tddd::catalogue_bulk_loader::load_all_catalogues | free_function | delete | fn(track_dir: &std::path::Path, rules_path: &std::path::Path, trusted_root: &std::path::Path) -> Result<(Vec<domain::tddd::LayerId>, std::collections::BTreeMap<domain::tddd::LayerId, domain::tddd::TypeCatalogueDocument>), LoadAllCataloguesError> | 🔵 | 🔵 |
| infrastructure::tddd::catalogue_bulk_loader::load_all_catalogues_native | free_function | — | fn(track_dir: &std::path::Path, rules_path: &std::path::Path, trusted_root: &std::path::Path) -> Result<(Vec<domain::tddd::LayerId>, std::collections::BTreeMap<domain::tddd::LayerId, domain::tddd::catalogue_v2::CatalogueDocument>), LoadAllCataloguesNativeError> | 🔵 | 🔵 |
| infrastructure::tddd::catalogue_codec::decode | free_function | delete | fn(json: &str) -> Result<TypeCatalogueDocument, TypeCatalogueCodecError> | 🔵 | 🔵 |
| infrastructure::tddd::catalogue_codec::encode | free_function | delete | fn(doc: &TypeCatalogueDocument) -> Result<String, TypeCatalogueCodecError> | 🔵 | 🔵 |
| infrastructure::tddd::catalogue_spec_signals_refresher::refresh_one_layer | free_function | modify | fn(items_dir: &std::path::Path, track_dir: &std::path::Path, track_id: &str, binding: &TdddLayerBinding, writer: &FsCatalogueSpecSignalsStore) -> Result<(), String> | 🔵 | 🔵 |
| infrastructure::tddd::type_graph_cluster::classify_types | free_function | delete | fn() -> ClusterPlan | 🔵 | 🔵 |
| infrastructure::tddd::type_graph_export::execute_type_graph_for_layer | free_function | delete | fn() -> Result<(), TypeGraphExportError> | 🔵 | 🔵 |
| infrastructure::tddd::type_graph_render::render_type_graph_clustered | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::tddd::type_graph_render::render_type_graph_flat | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::tddd::type_graph_render::render_type_graph_overview | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::tddd::type_graph_render::write_type_graph_dir | free_function | delete | fn() -> Result<(), TypeGraphExportError> | 🔵 | 🔵 |
| infrastructure::tddd::type_graph_render::write_type_graph_file | free_function | delete | fn() -> Result<(), TypeGraphExportError> | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_evaluator::evaluate_and_write_signals | free_function | delete | fn() -> Result<(), EvaluateSignalsError> | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_evaluator::execute_type_signals_for_layer | free_function | modify | fn(items_dir: &std::path::Path, track_id: &str, workspace_root: &std::path::Path, binding: &TdddLayerBinding) -> Result<std::process::ExitCode, EvaluateSignalsError> | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_evaluator::validate_action_diagnostics | free_function | delete | fn() -> Result<(), EvaluateSignalsError> | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_evaluator::validate_and_write_catalogue | free_function | delete | fn() -> Result<(), EvaluateSignalsError> | 🔵 | 🔵 |
| infrastructure::type_catalogue_render::render_type_catalogue | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::type_catalogue_render::render_type_catalogue_v3 | free_function | — | fn(doc: &domain::tddd::catalogue_v2::CatalogueDocument, source_file_name: &str, type_signals: Option<&[domain::TypeSignal]>, catalogue_spec_signals: Option<&domain::CatalogueSpecSignalsDocument>) -> String | 🔵 | 🔵 |
| infrastructure::verify::spec_code_consistency::consistency_report_to_findings | free_function | delete | fn() -> Vec<VerifyFinding> | 🔵 | 🔵 |
| infrastructure::verify::spec_code_consistency::evaluate_consistency_from_components | free_function | delete | fn() -> ConsistencyReport | 🔵 | 🔵 |
| infrastructure::verify::spec_code_consistency::execute_spec_code_consistency_str | free_function | delete | fn(_track_id_str: &str, _crate_name: &str, _project_root: &std::path::Path) -> VerifyOutcome | 🔵 | 🔵 |
| infrastructure::verify::spec_states::verify_from_spec_json | free_function | modify | fn(spec_json_path: &std::path::Path, strict: bool, trusted_root: &std::path::Path) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |

