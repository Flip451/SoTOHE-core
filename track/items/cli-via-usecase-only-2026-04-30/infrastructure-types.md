<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodexReviewOutcome | enum | — | Skipped, FinalCompleted, FastCompleted | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewV2Composition | dto | — | — | 🔵 | 🔵 |
| ReviewV2CompositionWithCodex | dto | — | — | 🔵 | 🔵 |
| CaptureBaselineError | dto | — | — | 🔵 | 🔵 |
| EvaluateSignalsError | dto | — | — | 🔵 | 🔵 |
| TypeGraphExportError | dto | — | — | 🔵 | 🔵 |
| SignalSummary | dto | — | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsTrackStore | secondary_adapter | reference | impl TrackReader, impl TrackWriter, impl ImplPlanReader, impl ImplPlanWriter | 🔵 | 🔵 |
| RustdocSchemaExporter | secondary_adapter | modify | impl SchemaExporter, impl SchemaExporterPort | 🔵 | 🔵 |
| ConchShellParser | secondary_adapter | modify | impl ShellParser, impl ShellParserPort, impl HookShellParserPort | 🔵 | 🔵 |
| NullDiffGetter | secondary_adapter | — | impl DiffGetter | 🔵 | 🔵 |
| NullReviewer | secondary_adapter | — | impl Reviewer | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| run_example_demo | free_function | — | — | 🟡 | 🔵 |
| persist_commit_hash_for_track | free_function | — | — | 🟡 | 🔵 |
| append_scope_briefing_reference_str | free_function | — | — | 🟡 | 🔵 |
| build_check_approved_service | free_function | — | — | 🟡 | 🔵 |
| build_review_v2 | free_function | — | — | 🟡 | 🔵 |
| build_review_v2_str | free_function | — | — | 🟡 | 🔵 |
| build_review_v2_with_reviewer | free_function | — | — | 🟡 | 🔵 |
| build_review_v2_with_reviewer_str | free_function | — | — | 🟡 | 🔵 |
| build_run_review_service | free_function | — | — | 🟡 | 🔵 |
| build_scope_query_interactor_no_diff_str | free_function | — | — | 🟡 | 🔵 |
| build_scope_query_interactor_str | free_function | — | — | 🟡 | 🔵 |
| check_approved_str | free_function | — | — | 🟡 | 🔵 |
| get_briefing_for_scope_str | free_function | — | — | 🟡 | 🔵 |
| load_scope_config_only | free_function | — | — | 🟡 | 🔵 |
| load_scope_config_only_str | free_function | — | — | 🟡 | 🔵 |
| render_review_results_str | free_function | — | — | 🟡 | 🔵 |
| resolve_diff_base_and_getter | free_function | — | — | 🟡 | 🔵 |
| run_codex_review_str | free_function | — | — | 🟡 | 🔵 |
| validate_review_group_name_str | free_function | — | — | 🟡 | 🔵 |
| validate_scope_for_track_str | free_function | — | — | 🟡 | 🔵 |
| validate_track_id_str | free_function | — | — | 🟡 | 🔵 |
| capture_baseline_for_layer | free_function | — | — | 🟡 | 🔵 |
| refresh_one_layer | free_function | — | — | 🟡 | 🔵 |
| execute_type_graph_for_layer | free_function | — | — | 🟡 | 🔵 |
| evaluate_and_write_signals | free_function | — | — | 🟡 | 🔵 |
| execute_type_signals_for_layer | free_function | — | — | 🟡 | 🔵 |
| validate_action_diagnostics | free_function | — | — | 🟡 | 🔵 |
| validate_and_write_catalogue | free_function | — | — | 🟡 | 🔵 |
| read_track_status_str | free_function | — | — | 🟡 | 🔵 |
| execute_verify_adr_signals | free_function | — | — | 🟡 | 🔵 |
| any_enabled_catalogue_present | free_function | — | — | 🟡 | 🔵 |
| format_finding | free_function | — | — | 🟡 | 🔵 |
| read_spec_element_hashes | free_function | — | — | 🟡 | 🔵 |
| verify_one_layer_formatted | free_function | — | — | 🟡 | 🔵 |
| execute_catalogue_spec_signals | free_function | — | — | 🔵 | 🔵 |
| execute_catalogue_spec_signals_check | free_function | — | — | 🔵 | 🔵 |
| consistency_report_to_findings | free_function | — | — | 🟡 | 🔵 |
| evaluate_consistency_from_components | free_function | — | — | 🟡 | 🔵 |
| execute_spec_code_consistency_str | free_function | — | — | 🟡 | 🔵 |

