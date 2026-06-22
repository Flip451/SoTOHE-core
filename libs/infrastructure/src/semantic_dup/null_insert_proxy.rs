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

use std::path::Path;
use std::sync::Arc;

use domain::semantic_dup::{CodeFragment, SimilarFragment, TopK};
use usecase::semantic_dup::{SemanticIndexError, SemanticIndexPort};

use crate::semantic_dup::index::LanceDbSemanticIndexAdapter;

// ── PersistentIndexLock ───────────────────────────────────────────────────────

/// RAII guard holding an exclusive `flock(2)` lock on the LanceDB index sidecar.
///
/// Constructed by [`acquire_persistent_index_lock`].  The lock is released when
/// this value is dropped.
///
/// This type is `pub` for use by `cli_composition` callers; it is not part of
/// the infrastructure crate's public API contract and is excluded from the TDDD
/// catalogue.
#[doc(hidden)]
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
/// Returns a human-readable `String` when the parent directory cannot be created,
/// the lock file cannot be opened, or `flock` fails.
///
/// This function is `pub` for use by `cli_composition` callers; it is not part of
/// the infrastructure crate's public API contract and is excluded from the TDDD
/// catalogue.
#[doc(hidden)]
pub fn acquire_persistent_index_lock(db_path: &Path) -> Result<PersistentIndexLock, String> {
    use fs4::fs_std::FileExt as _;

    let lock_path = persistent_index_lock_path(db_path);
    if let Some(parent) = lock_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create index lock parent dir: {e}"))?;
        }
    }
    match std::fs::symlink_metadata(&lock_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!("index cache lock {} is a symlink", lock_path.display()));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(format!("failed to inspect index lock {}: {e}", lock_path.display()));
        }
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| format!("failed to open index lock {}: {e}", lock_path.display()))?;
    file.lock_exclusive()
        .map_err(|e| format!("failed to lock index cache {}: {e}", lock_path.display()))?;

    Ok(PersistentIndexLock { _file: file })
}

/// Returns the path to the lock-file sidecar for `db_path`.
///
/// This function is `pub` for internal use within the infrastructure crate;
/// it is not part of the public API contract and is excluded from the TDDD catalogue.
#[doc(hidden)]
pub fn persistent_index_lock_path(db_path: &Path) -> std::path::PathBuf {
    persistent_index_suffixed_path(db_path, ".lock")
}

fn persistent_index_suffixed_path(db_path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut path = db_path.as_os_str().to_owned();
    path.push(suffix);
    std::path::PathBuf::from(path)
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
    use std::path::PathBuf;
    use std::sync::Arc;

    use domain::semantic_dup::CodeFragment;
    use usecase::semantic_dup::SemanticIndexPort as _;

    use crate::semantic_dup::index::LanceDbSemanticIndexAdapter;

    use super::{NullInsertIndexProxy, acquire_persistent_index_lock, persistent_index_lock_path};

    fn make_fragment(path: &str) -> CodeFragment {
        CodeFragment::new(PathBuf::from(path), "fn f() {}".to_owned(), 1, 1).unwrap()
    }

    // ── NullInsertIndexProxy tests ────────────────────────────────────────────

    /// insert() returns Ok(()) without performing any write to the underlying adapter.
    #[test]
    fn test_null_insert_index_proxy_insert_is_noop() {
        let dir = tempfile::tempdir().unwrap();
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
        let dir = tempfile::tempdir().unwrap();
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
        let dir = tempfile::tempdir().unwrap();
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
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");
        let lock_path = persistent_index_lock_path(&db_path);

        let _lock = acquire_persistent_index_lock(&db_path).unwrap();

        assert!(lock_path.exists(), "lock acquisition must create the lock sidecar");
    }

    /// acquire_persistent_index_lock() rejects a symlinked lock sidecar.
    #[cfg(unix)]
    #[test]
    fn test_acquire_persistent_index_lock_with_symlink_sidecar_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("idx");
        let lock_path = persistent_index_lock_path(&db_path);
        let target_path = dir.path().join("outside-lock-target");
        std::os::unix::fs::symlink(&target_path, &lock_path).unwrap();

        let result = acquire_persistent_index_lock(&db_path);

        assert!(
            result.is_err_and(|error| error.contains("is a symlink")),
            "lock acquisition must reject symlink sidecars"
        );
    }
}
