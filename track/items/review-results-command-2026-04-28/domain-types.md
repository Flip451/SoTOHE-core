<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewApprovalVerdict | enum | — | Approved, ApprovedWithBypass, Blocked | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ScopeRound | value_object | — | — | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewReader | secondary_port | reference | fn read_latest_finals(&self) -> Result<HashMap<ScopeName, (Verdict, ReviewHash)>, ReviewReaderError>, fn read_all_rounds(&self, scope: &ScopeName) -> Result<Vec<ScopeRound>, ReviewReaderError> | 🔵 | 🔵 |
| ReviewWriter | secondary_port | reference | fn write_verdict(&self, scope: &ScopeName, verdict: &Verdict, hash: &ReviewHash) -> Result<(), ReviewWriterError>, fn write_fast_verdict(&self, scope: &ScopeName, verdict: &FastVerdict, hash: &ReviewHash) -> Result<(), ReviewWriterError>, fn init(&self) -> Result<(), ReviewWriterError>, fn reset(&self) -> Result<(), ReviewWriterError> | 🔵 | 🔵 |
| ReviewExistsPort | secondary_port | — | fn review_json_exists(&self) -> Result<bool, ReviewReaderError> | 🔵 | 🔵 |

