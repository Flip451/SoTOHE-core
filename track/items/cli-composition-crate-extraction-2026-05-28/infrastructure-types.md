<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodexReviewOutcome | enum | delete | Skipped, FinalCompleted, FastCompleted | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ScopeConfigLoadError | error_type | reference | Io, Parse, InvalidField, Config | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ClaudeReviewer | secondary_adapter | reference | impl Reviewer | 🔵 | 🔵 |
| CodexReviewer | secondary_adapter | reference | impl Reviewer | 🔵 | 🔵 |
| FsCommitHashStore | secondary_adapter | reference | impl CommitHashReader, impl CommitHashWriter | 🔵 | 🔵 |
| FsReviewStore | secondary_adapter | reference | impl ReviewReader, impl ReviewWriter, impl ReviewExistsPort | 🔵 | 🔵 |
| GitDiffGetter | secondary_adapter | reference | impl DiffGetter | 🔵 | 🔵 |
| NullDiffGetter | secondary_adapter | delete | impl DiffGetter | 🔵 | 🔵 |
| NullReviewer | secondary_adapter | delete | impl Reviewer | 🔵 | 🔵 |
| ReviewV2Composition | secondary_adapter | delete | — | 🔵 | 🔵 |
| ReviewV2CompositionWithClaude | secondary_adapter | delete | — | 🔵 | 🔵 |
| ReviewV2CompositionWithCodex | secondary_adapter | delete | — | 🔵 | 🔵 |
| SystemReviewHasher | secondary_adapter | reference | impl ReviewHasher | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::review_v2::cli_composition::append_scope_briefing_reference_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::build_check_approved_service | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::build_review_v2 | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::build_review_v2_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::build_review_v2_with_claude_reviewer | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::build_review_v2_with_claude_reviewer_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::build_review_v2_with_reviewer | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::build_review_v2_with_reviewer_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::build_run_review_service | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::build_scope_query_interactor_no_diff_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::build_scope_query_interactor_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::check_approved_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::get_briefing_for_scope_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::load_scope_config_only | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::load_scope_config_only_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::render_review_results_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::resolve_diff_base_and_getter | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::run_claude_review_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::run_codex_review_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::validate_review_group_name_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::validate_scope_for_track_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::validate_track_id_str | free_function | delete | fn() -> String | 🔵 | 🔵 |
| infrastructure::review_v2::persist_commit_hash_for_track | free_function | delete | fn(track_id: &str) -> Result<String, String> | 🟡 | 🔵 |
| infrastructure::review_v2::scope_config_loader::load_v2_scope_config | free_function | reference | fn(track_id: domain::TrackId, items_dir: &std::path::Path) -> Result<domain::review_v2::ReviewScopeConfig, ScopeConfigLoadError> | 🔵 | 🔵 |

