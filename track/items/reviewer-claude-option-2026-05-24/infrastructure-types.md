<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewV2CompositionWithClaude | dto | — | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ClaudeReviewer | secondary_adapter | — | impl Reviewer | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::review_v2::cli_composition::build_review_v2_with_claude_reviewer | free_function | — | fn(track_id: &domain::TrackId, items_dir: &std::path::Path, reviewer: ClaudeReviewer) -> Result<ReviewV2CompositionWithClaude, String> | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::build_review_v2_with_claude_reviewer_str | free_function | — | fn(track_id_str: &str, items_dir: &std::path::Path, reviewer: ClaudeReviewer) -> Result<ReviewV2CompositionWithClaude, String> | 🔵 | 🔵 |
| infrastructure::review_v2::cli_composition::run_claude_review_str | free_function | — | fn(track_id_str: &str, items_dir: &std::path::Path, group_str: &str, round_type_str: &str, reviewer: ClaudeReviewer) -> Result<CodexReviewOutcome, String> | 🔵 | 🔵 |

