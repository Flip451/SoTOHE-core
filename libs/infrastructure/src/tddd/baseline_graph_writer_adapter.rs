//! Filesystem-backed adapter for the `BaselineGraphWriter` domain port.
//!
//! [`BaselineGraphWriterAdapter`] implements [`BaselineGraphWriter`] and
//! persists Reality View (baseline graph) Mermaid markdown files to the
//! per-track, per-layer subdirectories under `track_root`.
//!
//! ## Output paths
//!
//! - `write_overview` → `track_root/<track_id>/<layer>-graph-d1/index.md`
//! - `write_cluster`  → `track_root/<track_id>/<layer>-graph-d2/<cluster_key>.md`
//!
//! Both output directories are created on first write if they do not already
//! exist.
//!
//! ## Atomic write
//!
//! All writes use [`atomic_write_file`] (tmp-in-same-dir + fsync + rename +
//! parent fsync) so partial files are never observed.
//!
//! ## Symlink policy
//!
//! Every path on the write chain — the track directory, the output
//! subdirectory, and the final file path — is checked by
//! [`reject_symlinks_below`] anchored at `trusted_root`.  A symlink
//! anywhere along the chain is rejected fail-closed as
//! [`BaselineGraphWriterError::SymlinkRejected`].
//!
//! ## Symmetric design
//!
//! Symmetric to [`super::contract_map_adapter::FsContractMapWriter`]:
//! - Same two constructor fields (`track_root`, `trusted_root`).
//! - Same atomic-write + symlink-guard pattern.
//! - Same `TrackNotFound` / `SymlinkRejected` / `IoError` error variants.
//!
//! (IN-02 / IN-19 / AC-02 / CN-03)

use std::path::PathBuf;

use domain::TrackId;
use domain::tddd::{BaselineGraphWriter, BaselineGraphWriterError, LayerId};

use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;

// ---------------------------------------------------------------------------
// BaselineGraphWriterAdapter
// ---------------------------------------------------------------------------

/// Filesystem-backed [`BaselineGraphWriter`] implementation.
///
/// Writes Reality View Mermaid markdown files atomically:
/// - Overview: `track_root/<track_id>/<layer>-graph-d1/index.md`
/// - Cluster:  `track_root/<track_id>/<layer>-graph-d2/<cluster_key>.md`
///
/// Rejects symlinks below `trusted_root` fail-closed. The target
/// subdirectory is created if absent. Symmetric to `FsContractMapWriter`.
/// (IN-02 / IN-19 / AC-02 / AC-15 / CN-03)
pub struct BaselineGraphWriterAdapter {
    /// Directory containing per-track subdirectories (typically `<workspace>/track/items`).
    pub track_root: PathBuf,
    /// Directory below which symlink traversal is refused fail-closed.
    pub trusted_root: PathBuf,
}

impl BaselineGraphWriterAdapter {
    /// Creates a new `BaselineGraphWriterAdapter`.
    ///
    /// * `track_root` — directory containing per-track subdirectories.
    /// * `trusted_root` — directory below which symlink traversal is refused
    ///   fail-closed (usually the workspace root).
    #[must_use]
    pub fn new(track_root: PathBuf, trusted_root: PathBuf) -> Self {
        Self { track_root, trusted_root }
    }
}

impl BaselineGraphWriter for BaselineGraphWriterAdapter {
    /// Persist overview Mermaid content to `track_root/<track_id>/<layer>-graph-d1/index.md`.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineGraphWriterError::TrackNotFound`] if the track
    /// directory does not exist,
    /// [`BaselineGraphWriterError::SymlinkRejected`] if any path component
    /// is a symlink, or [`BaselineGraphWriterError::IoError`] for other
    /// I/O failures.
    fn write_overview(
        &self,
        track_id: &TrackId,
        layer: &LayerId,
        content: &str,
    ) -> Result<(), BaselineGraphWriterError> {
        let track_dir = self.track_root.join(track_id.as_ref());

        // Guard the track directory itself before the existence check so that
        // a broken symlink at `track_dir` is classified as `SymlinkRejected`
        // rather than silently collapsed into `TrackNotFound` by `.is_dir()`
        // (which follows symlinks and returns false for broken ones).
        map_symlink_guard(reject_symlinks_below(&track_dir, &self.trusted_root), &track_dir)?;

        if !track_dir.is_dir() {
            return Err(BaselineGraphWriterError::TrackNotFound {
                track_id: track_id.clone(),
                expected_dir: track_dir,
            });
        }

        let out_dir = track_dir.join(format!("{}-graph-d1", layer.as_ref()));
        let out_path = out_dir.join("index.md");

        write_file(&out_dir, &out_path, content.as_bytes(), &self.trusted_root)
    }

    /// Persist cluster-detail Mermaid content to
    /// `track_root/<track_id>/<layer>-graph-d2/<cluster_key>.md`.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineGraphWriterError::TrackNotFound`] if the track
    /// directory does not exist,
    /// [`BaselineGraphWriterError::SymlinkRejected`] if any path component
    /// is a symlink, or [`BaselineGraphWriterError::IoError`] for other
    /// I/O failures.
    fn write_cluster(
        &self,
        track_id: &TrackId,
        layer: &LayerId,
        cluster_key: &str,
        content: &str,
    ) -> Result<(), BaselineGraphWriterError> {
        let track_dir = self.track_root.join(track_id.as_ref());

        // Same symlink pre-check as write_overview.
        map_symlink_guard(reject_symlinks_below(&track_dir, &self.trusted_root), &track_dir)?;

        if !track_dir.is_dir() {
            return Err(BaselineGraphWriterError::TrackNotFound {
                track_id: track_id.clone(),
                expected_dir: track_dir,
            });
        }

        validate_cluster_key(cluster_key)?;

        let out_dir = track_dir.join(format!("{}-graph-d2", layer.as_ref()));
        let out_path = out_dir.join(format!("{cluster_key}.md"));

        write_file(&out_dir, &out_path, content.as_bytes(), &self.trusted_root)
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Returns `Ok(())` if `key` is a safe single path segment, otherwise `Err`.
///
/// A safe cluster key must not contain `/`, `\`, or `..` as a component,
/// which would allow path traversal out of the intended output subdirectory.
/// Only the raw string is checked; the caller still calls `reject_symlinks_below`
/// on the resolved path to guard against symlink attacks.
fn validate_cluster_key(key: &str) -> Result<(), BaselineGraphWriterError> {
    // An empty key or one that is exactly ".." is always invalid.
    // A key containing a path separator (Unix '/' or Windows '\') is invalid.
    // A key that, after splitting on '/', produces a ".." component is invalid.
    let has_separator = key.contains('/') || key.contains('\\');
    let has_dotdot = std::path::Path::new(key)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir));
    if key.is_empty() || has_separator || has_dotdot {
        return Err(BaselineGraphWriterError::IoError {
            path: std::path::PathBuf::from(key),
            reason: format!(
                "cluster_key {:?} is not a safe single path segment (must not contain '/', \
                 '\\', '..' or be empty)",
                key
            ),
        });
    }
    Ok(())
}

/// Translate the result of `reject_symlinks_below` into a `BaselineGraphWriterError`.
///
/// * `Ok(_)` — no symlink found; continue.
/// * `Err(e)` with `InvalidInput` kind — symlink detected; return `SymlinkRejected`.
/// * `Err(e)` other — non-symlink I/O error; return `IoError`.
fn map_symlink_guard(
    result: std::io::Result<bool>,
    path: &std::path::Path,
) -> Result<(), BaselineGraphWriterError> {
    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::InvalidInput {
                Err(BaselineGraphWriterError::SymlinkRejected { path: path.to_path_buf() })
            } else {
                Err(BaselineGraphWriterError::IoError {
                    path: path.to_path_buf(),
                    reason: e.to_string(),
                })
            }
        }
    }
}

/// Create `out_dir` if absent (fail-closed on symlink), guard `out_path`,
/// then atomically write `content`.
fn write_file(
    out_dir: &std::path::Path,
    out_path: &std::path::Path,
    content: &[u8],
    trusted_root: &std::path::Path,
) -> Result<(), BaselineGraphWriterError> {
    // Guard the output subdirectory path before creating it.
    map_symlink_guard(reject_symlinks_below(out_dir, trusted_root), out_dir)?;

    // Create the subdirectory if it does not yet exist.
    std::fs::create_dir_all(out_dir).map_err(|e| BaselineGraphWriterError::IoError {
        path: out_dir.to_path_buf(),
        reason: e.to_string(),
    })?;

    // Guard the final output file path.
    map_symlink_guard(reject_symlinks_below(out_path, trusted_root), out_path)?;

    // Atomic write.
    atomic_write_file(out_path, content).map_err(|e| BaselineGraphWriterError::IoError {
        path: out_path.to_path_buf(),
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::str_to_string
)]
mod tests {
    use super::*;

    fn track_id(slug: &str) -> TrackId {
        TrackId::try_new(slug.to_owned()).unwrap()
    }

    fn layer_id(s: &str) -> LayerId {
        LayerId::try_new(s.to_owned()).unwrap()
    }

    // -----------------------------------------------------------------------
    // write_overview — happy path
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_overview_creates_subdirectory_and_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("domain");
        adapter.write_overview(&tid, &layer, "```mermaid\nflowchart LR\n```\n").unwrap();

        let out = track_dir.join("domain-graph-d1").join("index.md");
        assert!(out.is_file(), "index.md must exist at {}", out.display());
        let content = std::fs::read_to_string(&out).unwrap();
        assert!(content.contains("flowchart LR"));
    }

    // -----------------------------------------------------------------------
    // write_overview — output path convention
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_overview_path_follows_layer_graph_d1_index_md_convention() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let tid = track_id("reality-view-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("infrastructure");
        adapter.write_overview(&tid, &layer, "content").unwrap();

        // Verify the exact path: <track_dir>/<layer>-graph-d1/index.md
        let expected = track_dir.join("infrastructure-graph-d1").join("index.md");
        assert!(expected.is_file(), "expected path does not exist: {}", expected.display());
    }

    // -----------------------------------------------------------------------
    // write_overview — overwrites existing file
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_overview_overwrites_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        let out_dir = track_dir.join("domain-graph-d1");
        std::fs::create_dir_all(&out_dir).unwrap();
        std::fs::write(out_dir.join("index.md"), b"stale").unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("domain");
        adapter.write_overview(&tid, &layer, "fresh content").unwrap();

        let content = std::fs::read_to_string(out_dir.join("index.md")).unwrap();
        assert!(content.contains("fresh content"));
        assert!(!content.contains("stale"));
    }

    // -----------------------------------------------------------------------
    // write_overview — missing track directory
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_overview_missing_track_dir_returns_track_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        std::fs::create_dir_all(&track_root).unwrap();
        let tid = track_id("nonexistent-track");

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("domain");
        let err = adapter.write_overview(&tid, &layer, "content").unwrap_err();

        match err {
            BaselineGraphWriterError::TrackNotFound { track_id: t, expected_dir } => {
                assert_eq!(t.as_ref(), "nonexistent-track");
                assert!(expected_dir.ends_with("nonexistent-track"));
            }
            other => panic!("expected TrackNotFound, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // write_cluster — happy path
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_cluster_creates_subdirectory_and_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("domain");
        adapter
            .write_cluster(&tid, &layer, "domain_root", "```mermaid\nclassDiagram\n```\n")
            .unwrap();

        let out = track_dir.join("domain-graph-d2").join("domain_root.md");
        assert!(out.is_file(), "cluster file must exist at {}", out.display());
        let content = std::fs::read_to_string(&out).unwrap();
        assert!(content.contains("classDiagram"));
    }

    // -----------------------------------------------------------------------
    // write_cluster — output path convention
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_cluster_path_follows_layer_graph_d2_cluster_key_md_convention() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let tid = track_id("reality-view-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("usecase");
        adapter.write_cluster(&tid, &layer, "usecase_command", "content").unwrap();

        // Exact path: <track_dir>/<layer>-graph-d2/<cluster_key>.md
        let expected = track_dir.join("usecase-graph-d2").join("usecase_command.md");
        assert!(expected.is_file(), "expected path does not exist: {}", expected.display());
    }

    // -----------------------------------------------------------------------
    // write_cluster — missing track directory
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_cluster_missing_track_dir_returns_track_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        std::fs::create_dir_all(&track_root).unwrap();
        let tid = track_id("nonexistent-track");

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("domain");
        let err = adapter.write_cluster(&tid, &layer, "domain_root", "content").unwrap_err();

        match err {
            BaselineGraphWriterError::TrackNotFound { track_id: t, .. } => {
                assert_eq!(t.as_ref(), "nonexistent-track");
            }
            other => panic!("expected TrackNotFound, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // write_cluster — multiple cluster keys produce distinct files
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_cluster_multiple_keys_produce_distinct_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("domain");
        adapter.write_cluster(&tid, &layer, "domain_root", "root content").unwrap();
        adapter.write_cluster(&tid, &layer, "domain_user", "user content").unwrap();

        let d2 = track_dir.join("domain-graph-d2");
        assert!(d2.join("domain_root.md").is_file());
        assert!(d2.join("domain_user.md").is_file());

        let root_content = std::fs::read_to_string(d2.join("domain_root.md")).unwrap();
        let user_content = std::fs::read_to_string(d2.join("domain_user.md")).unwrap();
        assert!(root_content.contains("root content"));
        assert!(user_content.contains("user content"));
    }

    // -----------------------------------------------------------------------
    // Symlink rejection: symlinked track directory
    // -----------------------------------------------------------------------

    #[cfg(unix)]
    #[test]
    fn test_write_overview_rejects_symlinked_track_dir() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        std::fs::create_dir_all(&track_root).unwrap();

        let real_dir = root.join("real-track");
        std::fs::create_dir_all(&real_dir).unwrap();
        let tid = track_id("symlinked-track");
        symlink(&real_dir, track_root.join(tid.as_ref())).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("domain");
        let err = adapter.write_overview(&tid, &layer, "content").unwrap_err();

        assert!(
            matches!(err, BaselineGraphWriterError::SymlinkRejected { .. }),
            "expected SymlinkRejected, got {err:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_write_cluster_rejects_symlinked_track_dir() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        std::fs::create_dir_all(&track_root).unwrap();

        let real_dir = root.join("real-track");
        std::fs::create_dir_all(&real_dir).unwrap();
        let tid = track_id("symlinked-track");
        symlink(&real_dir, track_root.join(tid.as_ref())).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("domain");
        let err = adapter.write_cluster(&tid, &layer, "domain_root", "content").unwrap_err();

        assert!(
            matches!(err, BaselineGraphWriterError::SymlinkRejected { .. }),
            "expected SymlinkRejected, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Symlink rejection: symlinked output file
    // -----------------------------------------------------------------------

    #[cfg(unix)]
    #[test]
    fn test_write_overview_rejects_symlinked_output_file() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        let out_dir = track_dir.join("domain-graph-d1");
        std::fs::create_dir_all(&out_dir).unwrap();

        // Create a symlink at the write target pointing outside the trusted root.
        let real_outside = root.join("outside.md");
        std::fs::write(&real_outside, b"external").unwrap();
        symlink(&real_outside, out_dir.join("index.md")).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("domain");
        let err = adapter.write_overview(&tid, &layer, "content").unwrap_err();

        assert!(
            matches!(err, BaselineGraphWriterError::SymlinkRejected { .. }),
            "expected SymlinkRejected, got {err:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_write_cluster_rejects_symlinked_output_file() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        let out_dir = track_dir.join("domain-graph-d2");
        std::fs::create_dir_all(&out_dir).unwrap();

        // Create a symlink at the write target pointing outside the trusted root.
        let real_outside = root.join("outside.md");
        std::fs::write(&real_outside, b"external").unwrap();
        symlink(&real_outside, out_dir.join("domain_root.md")).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("domain");
        let err = adapter.write_cluster(&tid, &layer, "domain_root", "content").unwrap_err();

        assert!(
            matches!(err, BaselineGraphWriterError::SymlinkRejected { .. }),
            "expected SymlinkRejected, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // write_cluster — path traversal rejection via cluster_key validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_cluster_rejects_dotdot_cluster_key() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("domain");
        let err = adapter.write_cluster(&tid, &layer, "../escape", "content").unwrap_err();

        assert!(
            matches!(err, BaselineGraphWriterError::IoError { .. }),
            "expected IoError for path-traversal cluster_key, got {err:?}"
        );
    }

    #[test]
    fn test_write_cluster_rejects_slash_in_cluster_key() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("domain");
        let err = adapter.write_cluster(&tid, &layer, "sub/key", "content").unwrap_err();

        assert!(
            matches!(err, BaselineGraphWriterError::IoError { .. }),
            "expected IoError for slash-containing cluster_key, got {err:?}"
        );
    }

    #[test]
    fn test_write_cluster_rejects_empty_cluster_key() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("domain");
        let err = adapter.write_cluster(&tid, &layer, "", "content").unwrap_err();

        assert!(
            matches!(err, BaselineGraphWriterError::IoError { .. }),
            "expected IoError for empty cluster_key, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // IoError on unwritable directory (unix only)
    // -----------------------------------------------------------------------

    #[cfg(unix)]
    #[test]
    fn test_write_overview_io_error_on_unwritable_track_dir() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        // Make the track directory read-only so subdirectory creation fails.
        std::fs::set_permissions(&track_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("domain");
        let err = adapter.write_overview(&tid, &layer, "content").unwrap_err();

        // Restore permissions before assertion so tempdir cleanup succeeds.
        let _ = std::fs::set_permissions(&track_dir, std::fs::Permissions::from_mode(0o755));

        assert!(
            matches!(err, BaselineGraphWriterError::IoError { .. }),
            "expected IoError, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Layer-agnostic: non-standard layer name
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_overview_custom_layer_name_used_in_path() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        // Non-standard layer name (not "domain"/"usecase"/"infrastructure").
        let layer = layer_id("application");
        adapter.write_overview(&tid, &layer, "content").unwrap();

        let expected = track_dir.join("application-graph-d1").join("index.md");
        assert!(expected.is_file(), "custom layer path must be used: {}", expected.display());
    }

    #[test]
    fn test_write_cluster_custom_layer_name_used_in_path() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let tid = track_id("my-track-2026-05-22");
        let track_dir = track_root.join(tid.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        let adapter = BaselineGraphWriterAdapter::new(track_root.clone(), root.to_path_buf());
        let layer = layer_id("application");
        adapter.write_cluster(&tid, &layer, "application_core", "content").unwrap();

        let expected = track_dir.join("application-graph-d2").join("application_core.md");
        assert!(expected.is_file(), "custom layer path must be used: {}", expected.display());
    }
}
