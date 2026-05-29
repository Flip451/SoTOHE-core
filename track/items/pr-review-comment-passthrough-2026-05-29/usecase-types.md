<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrReviewError | error_type | reference | UnsupportedProvider | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrReviewFinding | dto | modify | — | 🟡 | 🔵 |
| PrReviewResult | dto | modify | — | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::pr_review::classify_severity | free_function | delete | fn(text: &str) -> &'static str | 🟡 | 🔵 |
| usecase::pr_review::parse_body_findings | free_function | delete | fn(body: &str) -> Vec<PrReviewFinding> | 🟡 | 🔵 |
| usecase::pr_review::parse_paginated_json | free_function | reference | fn(text: &str) -> Result<Vec<serde_json::Value>, serde_json::Error> | 🔵 | 🔵 |
| usecase::pr_review::sanitize_text | free_function | reference | fn(text: &str) -> String | 🔵 | 🔵 |
| usecase::pr_review::validate_reviewer_provider | free_function | reference | fn(provider: &str) -> Result<(), PrReviewError> | 🔵 | 🔵 |

