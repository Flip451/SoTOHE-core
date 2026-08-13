//! Public merge-gate blob-reader handle.

use std::path::PathBuf;

/// Adapter that reads track documents from the local git repository via
/// `git show origin/<branch>:<path>`.
///
/// Construct with `GitShowTrackBlobReader::new(repo_root)`. The adapter
/// is stateless apart from the repo root path, so a single instance can
/// be shared across multiple usecase calls (e.g. merge_gate +
/// task_completion from the same `pr.rs::wait_and_merge` invocation).
pub struct GitShowTrackBlobReader {
    pub(super) repo_root: PathBuf,
}
