<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommitHashReader | secondary_port | reference | fn read(&self) -> Result<Option<CommitHash>, CommitHashError> | 🔵 | 🔵 |
| CommitHashWriter | secondary_port | reference | fn write(&self, hash: &CommitHash) -> Result<(), CommitHashError>, fn clear(&self) -> Result<(), CommitHashError> | 🔵 | 🔵 |
| ReviewExistsPort | secondary_port | reference | fn review_json_exists(&self) -> Result<bool, ReviewReaderError> | 🔵 | 🔵 |
| ReviewReader | secondary_port | reference | fn read_latest_finals(&self) -> Result<std::collections::HashMap<ScopeName, (Verdict, ReviewHash)>, ReviewReaderError>, fn read_all_rounds(&self, scope: &ScopeName) -> Result<Vec<ScopeRound>, ReviewReaderError> | 🔵 | 🔵 |
| ReviewWriter | secondary_port | reference | fn write_verdict(&self, scope: &ScopeName, verdict: &Verdict, hash: &ReviewHash) -> Result<(), ReviewWriterError>, fn write_fast_verdict(&self, scope: &ScopeName, verdict: &FastVerdict, hash: &ReviewHash) -> Result<(), ReviewWriterError>, fn init(&self) -> Result<(), ReviewWriterError>, fn reset(&self) -> Result<(), ReviewWriterError> | 🔵 | 🔵 |

