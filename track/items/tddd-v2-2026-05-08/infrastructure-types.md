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
| BaselineCodecError | error_type | delete | — | 🔵 | 🔵 |
| BaselineRustdocCodecError | error_type | — | — | 🔵 | 🔵 |
| CatalogueDocumentCodecError | error_type | — | — | 🔵 | 🔵 |
| CatalogueToExtendedCrateCodecError | error_type | — | — | 🔵 | 🔵 |
| EvaluateSignalsError | error_type | reference | — | 🔵 | 🔵 |
| LoadAllCataloguesError | error_type | modify | — | 🔵 | 🔵 |
| SchemaExportCodecError | error_type | reference | — | 🔵 | 🔵 |
| TypeCatalogueCodecError | error_type | reference | — | 🔵 | 🔵 |
| TypeGraphExportError | error_type | delete | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineRustdocCodec | secondary_adapter | — | — | 🟡 | 🔵 |
| CatalogueDocumentCodec | secondary_adapter | — | — | 🟡 | 🔵 |
| CatalogueToExtendedCrateCodec | secondary_adapter | — | — | 🟡 | 🔵 |
| FsCatalogueLoader | secondary_adapter | modify | — | 🟡 | 🔵 |
| FsCatalogueSpecSignalsStore | secondary_adapter | reference | — | 🔵 | 🔵 |
| FsContractMapWriter | secondary_adapter | reference | — | 🔵 | 🔵 |
| InMemoryCatalogueLinter | secondary_adapter | modify | — | 🟡 | 🔵 |
| RustdocSchemaExporter | secondary_adapter | modify | — | 🟡 | 🔵 |
| SignalEvaluatorV2 | secondary_adapter | — | — | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::code_profile_builder::build_type_graph | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::tddd::baseline_builder::build_baseline | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::tddd::baseline_capture::capture_baseline_for_layer | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::tddd::baseline_capture::capture_rustdoc_baseline_for_layer | free_function | — | — | 🟡 | 🔵 |
| infrastructure::tddd::baseline_capture::force_capture_rustdoc_baseline_for_layer | free_function | — | — | 🟡 | 🔵 |
| infrastructure::tddd::baseline_codec::decode | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::tddd::baseline_codec::encode | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::tddd::type_graph_cluster::classify_types | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::tddd::type_graph_export::execute_type_graph_for_layer | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::tddd::type_graph_render::render_type_graph_clustered | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::tddd::type_graph_render::render_type_graph_flat | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::tddd::type_graph_render::render_type_graph_overview | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::tddd::type_graph_render::write_type_graph_dir | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::tddd::type_graph_render::write_type_graph_file | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_evaluator::evaluate_and_write_signals | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_evaluator::validate_action_diagnostics | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_evaluator::validate_and_write_catalogue | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::verify::spec_code_consistency::consistency_report_to_findings | free_function | delete | — | 🔵 | 🔵 |
| infrastructure::verify::spec_code_consistency::evaluate_consistency_from_components | free_function | delete | — | 🔵 | 🔵 |

