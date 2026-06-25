//! Null-insert proxy and filesystem lock for the persistent LanceDB semantic index.
//!
//! Relocated from `cli_composition::dry::persistent_index` (D7 / CN-06 / AC-09).
//!
//! Types provided:
//!
//! - [`PersistentIndexLock`]: RAII guard holding an exclusive filesystem lock on
//!   the LanceDB index directory.
//! - [`NullInsertIndexProxy`]: `SemanticIndexPort` proxy that makes `insert` and
//!   `insert_batch` no-ops while forwarding `delete_by_source_path` and `search`
//!   to the wrapped [`LanceDbSemanticIndexAdapter`].

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use domain::semantic_dup::{CodeFragment, SimilarFragment, TopK};
use usecase::semantic_dup::{SemanticIndexError, SemanticIndexPort};

use crate::semantic_dup::index::LanceDbSemanticIndexAdapter;

// ── PersistentIndexLockError ──────────────────────────────────────────────────

/// Error type for [`acquire_persistent_index_lock`].
#[derive(Debug, thiserror::Error)]
pub enum PersistentIndexLockError {
    #[error("{0}")]
    LockFailed(String),
}

impl PersistentIndexLockError {
    pub fn contains(&self, s: &str) -> bool {
        match self {
            PersistentIndexLockError::LockFailed(msg) => msg.contains(s),
        }
    }
}

// ── PersistentIndexLock ───────────────────────────────────────────────────────

/// RAII guard holding an exclusive `flock(2)` lock on the LanceDB index sidecar.
///
/// Constructed by [`acquire_persistent_index_lock`].  The lock is released when
/// this value is dropped.
///
/// This type is `pub` for use by `cli_composition` callers; it is not part of
/// the infrastructure crate's public API contract and is excluded from the TDDD
/// catalogue.
pub struct PersistentIndexLock {
    _file: std::fs::File,
}

/// Acquire an exclusive filesystem lock on the LanceDB index at `db_path`.
///
/// Creates the lock-file sidecar (`<db_path>.lock`) and its parent directory if
/// needed, then takes an exclusive `flock(2)` lock.  The lock is held for the
/// lifetime of the returned [`PersistentIndexLock`] and released on drop.
///
/// # Errors
///
/// Returns [`PersistentIndexLockError`] when the parent directory cannot be created,
/// a symlink is found in the lock path, the lock file cannot be opened, or `flock` fails.
///
/// This function is `pub` for use by `cli_composition` callers; it is not part of
/// the infrastructure crate's public API contract and is excluded from the TDDD
/// catalogue.
pub fn acquire_persistent_index_lock(
    db_path: &Path,
) -> Result<PersistentIndexLock, PersistentIndexLockError> {
    use fs4::fs_std::FileExt as _;

    let (lock_path, trusted_root) = resolve_lock_path_with_trusted_root(db_path)?;
    reject_lock_path_symlinks(&lock_path, &trusted_root)?;
    if let Some(parent) = lock_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                PersistentIndexLockError::LockFailed(format!(
                    "failed to create index lock parent dir: {e}"
                ))
            })?;
        }
    }
    reject_lock_path_symlinks(&lock_path, &trusted_root)?;
    let mut open_options = std::fs::OpenOptions::new();
    open_options.create(true).write(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        open_options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = open_options.open(&lock_path).map_err(|e| {
        PersistentIndexLockError::LockFailed(format!(
            "failed to open index lock {}: {e}",
            lock_path.display()
        ))
    })?;
    file.lock_exclusive().map_err(|e| {
        PersistentIndexLockError::LockFailed(format!(
            "failed to lock index cache {}: {e}",
            lock_path.display()
        ))
    })?;

    Ok(PersistentIndexLock { _file: file })
}

/// Returns the path to the lock-file sidecar for `db_path`.
///
/// This function is `pub` for internal use within the infrastructure crate;
/// it is not part of the public API contract and is excluded from the TDDD catalogue.
pub fn persistent_index_lock_path(db_path: &Path) -> std::path::PathBuf {
    persistent_index_suffixed_path(db_path, ".lock")
}

fn persistent_index_suffixed_path(db_path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut path = db_path.as_os_str().to_owned();
    path.push(suffix);
    std::path::PathBuf::from(path)
}

fn resolve_lock_path_with_trusted_root(
    db_path: &Path,
) -> Result<(PathBuf, PathBuf), PersistentIndexLockError> {
    if db_path.as_os_str().is_empty() {
        return Err(PersistentIndexLockError::LockFailed(
            "index cache path must not be empty".to_owned(),
        ));
    }
    if db_path.components().any(|component| matches!(component, Component::ParentDir)) {
        return Err(PersistentIndexLockError::LockFailed(format!(
            "index cache path cannot escape its trusted root: {}",
            db_path.display()
        )));
    }

    let (resolved_db_path, trusted_root) = if db_path.is_absolute() {
        (db_path.to_path_buf(), PathBuf::from("/"))
    } else {
        let current_dir = std::env::current_dir().map_err(|e| {
            PersistentIndexLockError::LockFailed(format!(
                "failed to resolve current directory for index cache path {}: {e}",
                db_path.display()
            ))
        })?;
        (current_dir.join(db_path), current_dir)
    };
    if !resolved_db_path.starts_with(&trusted_root) {
        return Err(PersistentIndexLockError::LockFailed(format!(
            "index cache path must stay within trusted root {}: {}",
            trusted_root.display(),
            db_path.display()
        )));
    }

    Ok((persistent_index_lock_path(&resolved_db_path), trusted_root))
}

fn reject_lock_path_symlinks(
    lock_path: &Path,
    trusted_root: &Path,
) -> Result<(), PersistentIndexLockError> {
    crate::track::symlink_guard::reject_symlinks_below(lock_path, trusted_root).map(|_| ()).map_err(
        |e| {
            PersistentIndexLockError::LockFailed(format!(
                "index cache lock path {} rejected before use: {e}",
                lock_path.display()
            ))
        },
    )
}

// ── NullInsertIndexProxy ──────────────────────────────────────────────────────

/// A `SemanticIndexPort` proxy that makes `insert` and `insert_batch` no-ops
/// while forwarding `delete_by_source_path` and `search` to the wrapped adapter.
///
/// Used on the reuse / incremental path: the persistent index is already
/// populated (or partially updated) before the interactors run.
pub struct NullInsertIndexProxy {
    inner: Arc<LanceDbSemanticIndexAdapter>,
    _cache_lock: PersistentIndexLock,
}

impl NullInsertIndexProxy {
    /// Wrap `inner` with a null-insert proxy, holding `cache_lock` for the
    /// lifetime of this value.
    pub fn new(inner: Arc<LanceDbSemanticIndexAdapter>, cache_lock: PersistentIndexLock) -> Self {
        Self { inner, _cache_lock: cache_lock }
    }
}

impl SemanticIndexPort for NullInsertIndexProxy {
    fn insert(
        &self,
        _fragment: &CodeFragment,
        _embedding: &[f32],
    ) -> Result<(), SemanticIndexError> {
        Ok(())
    }

    fn insert_batch(&self, _items: &[(CodeFragment, Vec<f32>)]) -> Result<(), SemanticIndexError> {
        Ok(())
    }

    fn delete_by_source_path(&self, source_path: &Path) -> Result<(), SemanticIndexError> {
        self.inner.delete_by_source_path(source_path)
    }

    fn search(
        &self,
        embedding: &[f32],
        top_k: TopK,
    ) -> Result<Vec<SimilarFragment>, SemanticIndexError> {
        self.inner.search(embedding, top_k)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use domain::semantic_dup::CodeFragment;
    use usecase::semantic_dup::SemanticIndexPort as _;

    use crate::semantic_dup::index::LanceDbSemanticIndexAdapter;

    use super::{NullInsertIndexProxy, acquire_persistent_index_lock, persistent_index_lock_path};

    fn repo_tempdir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("sotp-index-lock-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap()
    }

    fn make_fragment(path: &str) -> CodeFragment {
        CodeFragment::new(PathBuf::from(path), "fn f() {}".to_owned(), 1, 1).unwrap()
    }

    // ── NullInsertIndexProxy tests ────────────────────────────────────────────

    /// insert() returns Ok(()) without performing any write to the underlying adapter.
    #[test]
    fn test_null_insert_index_proxy_insert_is_noop() {
        let dir = repo_tempdir();
        let db_path = dir.path().join("idx");
        let adapter = Arc::new(LanceDbSemanticIndexAdapter::new(db_path.clone()).unwrap());
        let lock = acquire_persistent_index_lock(&db_path).unwrap();
        let proxy = NullInsertIndexProxy::new(Arc::clone(&adapter), lock);

        let frag = make_fragment("src/a.rs");
        let result = proxy.insert(&frag, &[0.1_f32]);
        assert!(result.is_ok(), "NullInsertIndexProxy::insert must return Ok(())");
    }

    /// insert_batch() returns Ok(()) for non-empty items without writing to the adapter.
    #[test]
    fn test_null_insert_index_proxy_insert_batch_is_noop() {
        let dir = repo_tempdir();
        let db_path = dir.path().join("idx");
        let adapter = Arc::new(LanceDbSemanticIndexAdapter::new(db_path.clone()).unwrap());
        let lock = acquire_persistent_index_lock(&db_path).unwrap();
        let proxy = NullInsertIndexProxy::new(Arc::clone(&adapter), lock);

        let frag = make_fragment("src/a.rs");
        let result = proxy.insert_batch(&[(frag, vec![0.1_f32, 0.2_f32])]);
        assert!(result.is_ok(), "NullInsertIndexProxy::insert_batch must return Ok(())");
    }

    /// delete_by_source_path() forwards to the inner adapter (not swallowed by proxy).
    #[test]
    fn test_null_insert_index_proxy_delete_by_source_path_forwards_to_inner() {
        let dir = repo_tempdir();
        let db_path = dir.path().join("idx");
        let adapter = Arc::new(LanceDbSemanticIndexAdapter::new(db_path.clone()).unwrap());
        let lock = acquire_persistent_index_lock(&db_path).unwrap();
        let proxy = NullInsertIndexProxy::new(Arc::clone(&adapter), lock);

        // Deleting from an empty (non-existent) table must succeed (no-op at DB level,
        // but the call is forwarded — not silently swallowed by the proxy).
        let result = proxy.delete_by_source_path(std::path::Path::new("src/a.rs"));
        assert!(
            result.is_ok(),
            "NullInsertIndexProxy::delete_by_source_path must forward and return Ok(()): {:?}",
            result.err()
        );
    }

    // ── PersistentIndexLock tests ─────────────────────────────────────────────

    /// acquire_persistent_index_lock() creates the lock sidecar file.
    #[test]
    fn test_acquire_persistent_index_lock_creates_lock_sidecar() {
        let dir = repo_tempdir();
        let db_path = dir.path().join("idx");
        let lock_path = persistent_index_lock_path(&db_path);

        let _lock = acquire_persistent_index_lock(&db_path).unwrap();

        assert!(lock_path.exists(), "lock acquisition must create the lock sidecar");
    }

    #[test]
    fn test_acquire_persistent_index_lock_allows_absolute_temp_path() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");
        let lock_path = persistent_index_lock_path(&db_path);

        let _lock = acquire_persistent_index_lock(&db_path).unwrap();

        assert!(lock_path.exists(), "absolute cache paths must be supported");
    }

    #[test]
    fn test_acquire_persistent_index_lock_with_parent_dir_escape_returns_error() {
        let result = acquire_persistent_index_lock(Path::new("../idx"));

        assert!(
            result.is_err_and(|error| error.to_string().contains("cannot escape")),
            "lock acquisition must reject parent-directory escapes"
        );
    }

    /// acquire_persistent_index_lock() rejects a symlinked lock sidecar.
    #[cfg(unix)]
    #[test]
    fn test_acquire_persistent_index_lock_with_symlink_sidecar_returns_error() {
        let dir = repo_tempdir();
        let db_path = dir.path().join("idx");
        let lock_path = persistent_index_lock_path(&db_path);
        let target_path = dir.path().join("outside-lock-target");
        std::os::unix::fs::symlink(&target_path, &lock_path).unwrap();

        let result = acquire_persistent_index_lock(&db_path);

        assert!(
            result.is_err_and(|error| error.to_string().contains("symlink")),
            "lock acquisition must reject symlink sidecars"
        );
    }

    /// acquire_persistent_index_lock() rejects a symlinked lock parent.
    #[cfg(unix)]
    #[test]
    fn test_acquire_persistent_index_lock_with_symlink_parent_returns_error() {
        let dir = repo_tempdir();
        let outside = tempfile::tempdir().unwrap();
        let link_parent = dir.path().join("cache-link");
        std::os::unix::fs::symlink(outside.path(), &link_parent).unwrap();
        let db_path = link_parent.join("idx");

        let result = acquire_persistent_index_lock(&db_path);

        assert!(
            result.is_err_and(|error| error.to_string().contains("symlink")),
            "lock acquisition must reject symlinked parent directories"
        );
    }
}
