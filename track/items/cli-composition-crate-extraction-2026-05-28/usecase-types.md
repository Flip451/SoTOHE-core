<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DiffGetter | secondary_port | reference | fn list_diff_files(&self, base: &domain::CommitHash) -> Result<Vec<domain::review_v2::FilePath>, DiffGetError> | 🔵 | 🔵 |
| ReviewHasher | secondary_port | reference | fn calc(&self, target: &domain::review_v2::ReviewTarget) -> Result<domain::review_v2::ReviewHash, ReviewHasherError> | 🔵 | 🔵 |
| Reviewer | secondary_port | reference | fn review(&self, target: &domain::review_v2::ReviewTarget) -> Result<(domain::review_v2::Verdict, domain::review_v2::LogInfo), ReviewerError>, fn fast_review(&self, target: &domain::review_v2::ReviewTarget) -> Result<(domain::review_v2::FastVerdict, domain::review_v2::LogInfo), ReviewerError> | 🔵 | 🔵 |

