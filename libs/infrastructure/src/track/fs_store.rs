//! File-system backed TrackReader + TrackWriter using atomic writes for crash-safe persistence.

use std::path::{Component, Path, PathBuf};

use domain::{
    DomainError, ImplPlanDocument, ImplPlanReader, ImplPlanWriter, RepositoryError, TrackId,
    TrackMetadata, TrackReadError, TrackReader, TrackWriteError, TrackWriter,
};

// NOTE: FsTrackStore no longer validates task descriptions or task removal on
// save — those invariants are enforced on ImplPlanDocument (impl-plan.json).
// The identity-only TrackMetadata has no tasks/plan.

use super::atomic_write::atomic_write_file;
use super::codec::{self, DocumentMeta};
use super::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};

/// File-system backed TrackReader + TrackWriter.
/// Uses `atomic_write_file` for crash-safe persistence.
pub struct FsTrackStore {
    root: PathBuf,
}

impl FsTrackStore {
    /// Creates a new `FsTrackStore`.
    ///
    /// # Arguments
    /// * `root` - Root directory containing track item directories (e.g., `track/items/`).
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Reads and decodes `metadata.json` for a given track ID.
    fn read_track(
        &self,
        id: &TrackId,
    ) -> Result<Option<(TrackMetadata, DocumentMeta)>, RepositoryError> {
        let Some(path) = guarded_track_file_path(&self.root, id, "metadata.json")? else {
            return Ok(None);
        };
        let json = std::fs::read_to_string(&path).map_err(|e| {
            RepositoryError::Message(format!("failed to read {}: {e}", path.display()))
        })?;

        let (track, meta) = codec::decode(&json).map_err(|e| {
            RepositoryError::Message(format!("failed to decode {}: {e}", path.display()))
        })?;

        Ok(Some((track, meta)))
    }

    /// Encodes and atomically writes `metadata.json` for a given track.
    pub(crate) fn write_track(
        &self,
        track: &TrackMetadata,
        meta: &DocumentMeta,
    ) -> Result<(), RepositoryError> {
        let path = guarded_track_file_path_for_write(&self.root, track.id(), "metadata.json")?;

        // Ensure the track directory exists.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                RepositoryError::Message(format!(
                    "failed to create directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        let json = codec::encode(track, meta)
            .map_err(|e| RepositoryError::Message(format!("failed to encode: {e}")))?;

        // Append trailing newline for POSIX compatibility.
        let content = format!("{json}\n");

        atomic_write_file(&path, content.as_bytes()).map_err(|e| {
            RepositoryError::Message(format!("failed to write {}: {e}", path.display()))
        })?;

        Ok(())
    }

    /// Returns the current timestamp as an ISO 8601 string.
    ///
    /// # Errors
    /// Returns `RepositoryError` if `timestamp_now()` fails (should never happen in practice).
    fn now_iso8601() -> Result<String, RepositoryError> {
        crate::timestamp_now()
            .map(|ts| ts.as_str().to_owned())
            .map_err(|e| RepositoryError::Message(format!("timestamp_now: {e}")))
    }
}

impl FsTrackStore {
    /// Read-only metadata load returning both domain model and document metadata.
    ///
    /// Unlike `TrackReader::find`, this also returns `DocumentMeta` (schema version,
    /// timestamps, original status) needed by callers that inspect document-level fields.
    ///
    /// # Errors
    /// Returns `TrackReadError` on I/O or decode failure.
    pub fn find_with_meta(
        &self,
        id: &TrackId,
    ) -> Result<Option<(TrackMetadata, DocumentMeta)>, TrackReadError> {
        self.read_track(id).map_err(TrackReadError::from)
    }
}

impl TrackReader for FsTrackStore {
    fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError> {
        self.read_track(id).map(|opt| opt.map(|(track, _meta)| track)).map_err(TrackReadError::from)
    }
}

impl TrackWriter for FsTrackStore {
    fn save(&self, track: &TrackMetadata) -> Result<(), TrackWriteError> {
        // Read existing meta to preserve created_at, or create new meta.
        let meta = match self.read_track(track.id()).map_err(TrackWriteError::from)? {
            Some((_existing, mut meta)) => {
                // NOTE: task description / removal validation removed — those
                // invariants now belong to ImplPlanDocument (impl-plan.json).
                meta.updated_at = Self::now_iso8601().map_err(TrackWriteError::from)?;
                meta
            }
            None => DocumentMeta {
                schema_version: 6,
                created_at: Self::now_iso8601().map_err(TrackWriteError::from)?,
                updated_at: Self::now_iso8601().map_err(TrackWriteError::from)?,
            },
        };

        self.write_track(track, &meta).map_err(TrackWriteError::from)?;

        Ok(())
    }

    fn update<F>(&self, id: &TrackId, mutate: F) -> Result<TrackMetadata, TrackWriteError>
    where
        F: FnOnce(&mut TrackMetadata) -> Result<(), DomainError>,
    {
        // Read current state.
        let (mut track, mut meta) =
            self.read_track(id).map_err(TrackWriteError::from)?.ok_or_else(|| {
                TrackWriteError::Repository(RepositoryError::TrackNotFound(id.to_string()))
            })?;

        // Apply mutation (domain logic only, no I/O).
        mutate(&mut track).map_err(TrackWriteError::from)?;

        // Update timestamp.
        meta.updated_at = Self::now_iso8601().map_err(TrackWriteError::from)?;
        self.write_track(&track, &meta).map_err(TrackWriteError::from)?;

        Ok(track)
    }
}

impl FsTrackStore {
    /// Execute a closure with full control over both the domain model and
    /// infrastructure metadata. Unlike `update`, this gives the caller full
    /// control over `DocumentMeta` (including `updated_at`) and does NOT
    /// auto-set any timestamps — the closure is responsible for setting them.
    ///
    /// The closure receives `(&mut TrackMetadata, &mut DocumentMeta)` and may
    /// perform multiple mutations in a single read-modify-write cycle. After
    /// the closure returns `Ok`, the state is written to disk atomically.
    /// On `Err`, nothing is written.
    ///
    /// Note: this method relies on single-process sequential execution for
    /// correctness. Concurrent callers are not supported — parallel access
    /// will be handled by worktree isolation (Phase 4 SPEC-04).
    ///
    /// # Errors
    /// Returns `TrackWriteError` if the track is not found, the closure returns
    /// an error, or the write fails.
    pub fn with_locked_document<F>(
        &self,
        id: &TrackId,
        f: F,
    ) -> Result<TrackMetadata, TrackWriteError>
    where
        F: FnOnce(&mut TrackMetadata, &mut DocumentMeta) -> Result<(), DomainError>,
    {
        use fs4::fs_std::FileExt;

        guarded_track_file_path(&self.root, id, "metadata.json")
            .map_err(TrackWriteError::from)?
            .ok_or_else(|| {
                TrackWriteError::Repository(RepositoryError::TrackNotFound(id.to_string()))
            })?;

        // Acquire an exclusive advisory lock on a sibling `.lock` file.
        // This serializes concurrent `with_locked_document` calls (e.g., parallel auto-record).
        let lock_path = guarded_track_file_path_for_write(&self.root, id, "metadata.json.lock")
            .map_err(TrackWriteError::from)?;
        let lock_file = std::fs::File::create(&lock_path).map_err(|e| {
            TrackWriteError::Repository(RepositoryError::Message(format!(
                "failed to create lock file {}: {e}",
                lock_path.display()
            )))
        })?;
        lock_file.lock_exclusive().map_err(|e| {
            TrackWriteError::Repository(RepositoryError::Message(format!(
                "failed to acquire exclusive lock on {}: {e}",
                lock_path.display()
            )))
        })?;

        // Read current state (under lock — guaranteed fresh).
        let (mut track, mut meta) =
            self.read_track(id).map_err(TrackWriteError::from)?.ok_or_else(|| {
                TrackWriteError::Repository(RepositoryError::TrackNotFound(id.to_string()))
            })?;

        // Invoke the closure — the caller controls all mutations including timestamps.
        let result = f(&mut track, &mut meta).map_err(TrackWriteError::from);

        if result.is_ok() {
            // Write the final state atomically.
            self.write_track(&track, &meta).map_err(TrackWriteError::from)?;
        }

        // Lock is released when `lock_file` is dropped (end of scope).
        result.map(|()| track)
    }
}

impl ImplPlanReader for FsTrackStore {
    fn load_impl_plan(&self, id: &TrackId) -> Result<Option<ImplPlanDocument>, RepositoryError> {
        let Some(path) = guarded_track_file_path(&self.root, id, "impl-plan.json")? else {
            return Ok(None);
        };
        let json = std::fs::read_to_string(&path).map_err(|e| {
            RepositoryError::Message(format!("failed to read {}: {e}", path.display()))
        })?;
        let doc = crate::impl_plan_codec::decode(&json).map_err(|e| {
            RepositoryError::Message(format!("failed to decode {}: {e}", path.display()))
        })?;
        Ok(Some(doc))
    }
}

impl ImplPlanWriter for FsTrackStore {
    fn save_impl_plan(&self, id: &TrackId, doc: &ImplPlanDocument) -> Result<(), RepositoryError> {
        let path = guarded_track_file_path_for_write(&self.root, id, "impl-plan.json")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                RepositoryError::Message(format!(
                    "failed to create directory {}: {e}",
                    parent.display()
                ))
            })?;
        }
        let json = crate::impl_plan_codec::encode(doc)
            .map_err(|e| RepositoryError::Message(format!("failed to encode impl-plan: {e}")))?;
        let content = format!("{json}\n");
        super::atomic_write::atomic_write_file(&path, content.as_bytes()).map_err(|e| {
            RepositoryError::Message(format!("failed to write {}: {e}", path.display()))
        })?;
        Ok(())
    }
}

/// Resolves the metadata.json path from root and track ID.
/// Exposed for CLI composition (e.g., listing available tracks).
#[must_use]
pub fn metadata_json_path(root: &Path, id: &TrackId) -> PathBuf {
    root.join(id.as_ref()).join("metadata.json")
}

fn guarded_items_dir(items_dir: &Path) -> Result<PathBuf, RepositoryError> {
    reject_parent_dir_items_dir(items_dir)?;
    let lexical_items_dir = absolutize_lexical(items_dir);
    reject_symlinks_up_to_root(&lexical_items_dir).map_err(|e| {
        let message = if e.kind() == std::io::ErrorKind::InvalidInput {
            format!("symlink guard: refusing to use symlinked items_dir component: {e}")
        } else {
            format!(
                "symlink guard: cannot stat items_dir component {}: {e}",
                lexical_items_dir.display()
            )
        };
        RepositoryError::Message(message)
    })?;
    canonicalize_deepest_existing_ancestor(&lexical_items_dir, items_dir)
}

fn reject_parent_dir_items_dir(items_dir: &Path) -> Result<(), RepositoryError> {
    if items_dir.components().any(|component| component == Component::ParentDir) {
        return Err(RepositoryError::Message(format!(
            "symlink guard: refusing items_dir with parent-dir component: {}",
            items_dir.display()
        )));
    }
    Ok(())
}

/// Canonicalize the deepest existing ancestor and retain the missing suffix.
///
/// `track init` is allowed to bootstrap a checkout where `track/items` has not
/// been materialized. The symlink chain has already been checked before this
/// function runs, so the retained suffix consists only of missing components
/// that `create_dir_all` may safely create.
fn canonicalize_deepest_existing_ancestor(
    lexical_items_dir: &Path,
    requested_items_dir: &Path,
) -> Result<PathBuf, RepositoryError> {
    for ancestor in lexical_items_dir.ancestors() {
        match ancestor.symlink_metadata() {
            Ok(_) => {
                let missing_suffix = lexical_items_dir.strip_prefix(ancestor).map_err(|e| {
                    RepositoryError::Message(format!(
                        "items_dir '{}' cannot be resolved relative to existing ancestor {}: {e}",
                        requested_items_dir.display(),
                        ancestor.display()
                    ))
                })?;
                let canonical_ancestor = ancestor.canonicalize().map_err(|e| {
                    RepositoryError::Message(format!(
                        "items_dir '{}' cannot be resolved: {e}",
                        requested_items_dir.display()
                    ))
                })?;
                return Ok(canonical_ancestor.join(missing_suffix));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(RepositoryError::Message(format!(
                    "items_dir '{}' cannot be resolved: {e}",
                    requested_items_dir.display()
                )));
            }
        }
    }

    Err(RepositoryError::Message(format!(
        "items_dir '{}' has no existing ancestor",
        requested_items_dir.display()
    )))
}

fn absolutize_lexical(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map(|cwd| cwd.join(path)).unwrap_or_else(|_| path.to_path_buf())
    };
    lexical_normalize(&absolute)
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut components: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => match components.last() {
                Some(Component::Normal(_)) => {
                    components.pop();
                }
                _ => components.push(component),
            },
            Component::CurDir => {}
            _ => components.push(component),
        }
    }
    components.iter().collect()
}

fn guarded_track_file_path(
    items_dir: &Path,
    id: &TrackId,
    file_name: &str,
) -> Result<Option<PathBuf>, RepositoryError> {
    let items_dir = guarded_items_dir(items_dir)?;
    let path = items_dir.join(id.as_ref()).join(file_name);
    match reject_symlinks_below(&path, &items_dir) {
        Ok(true) => Ok(Some(path)),
        Ok(false) => Ok(None),
        Err(e) => Err(RepositoryError::Message(format!(
            "symlink guard: refusing to read {} {}: {e}",
            file_name,
            path.display()
        ))),
    }
}

fn guarded_track_file_path_for_write(
    items_dir: &Path,
    id: &TrackId,
    file_name: &str,
) -> Result<PathBuf, RepositoryError> {
    let items_dir = guarded_items_dir(items_dir)?;
    let path = items_dir.join(id.as_ref()).join(file_name);
    match reject_symlinks_below(&path, &items_dir) {
        Ok(_) => Ok(path),
        Err(e) => Err(RepositoryError::Message(format!(
            "symlink guard: refusing to write {} {}: {e}",
            file_name,
            path.display()
        ))),
    }
}

fn repository_error_message(error: RepositoryError) -> String {
    match error {
        RepositoryError::Message(message) => message,
        other => other.to_string(),
    }
}

/// Read-only metadata load directly from disk.
///
/// Reads and decodes `metadata.json` for a given track ID.
/// Use this for read-only paths (e.g., `track resolve`) that only need
/// to inspect metadata without constructing a full `FsTrackStore`.
///
/// # Errors
/// Returns `RepositoryError` on I/O or decode failure.
pub fn read_track_metadata(
    items_dir: &Path,
    id: &TrackId,
) -> Result<(TrackMetadata, DocumentMeta), RepositoryError> {
    let requested_path = items_dir.join(id.as_ref()).join("metadata.json");
    let Some(path) = guarded_track_file_path(items_dir, id, "metadata.json")? else {
        return Err(RepositoryError::Message(format!(
            "cannot read {}: file not found",
            requested_path.display()
        )));
    };
    let json = std::fs::read_to_string(&path).map_err(|err| {
        RepositoryError::Message(format!("cannot read {}: {err}", path.display()))
    })?;
    codec::decode(&json)
        .map_err(|err| RepositoryError::Message(format!("cannot parse {}: {err}", path.display())))
}

/// Loads the effective track status as a string for the given track ID string.
///
/// Returns `Ok(status_str)` where `status_str` is one of `"planned"`, `"in_progress"`,
/// `"done"`, `"blocked"`, or `"cancelled"`.
///
/// Constructs `domain::TrackId` internally so that callers in the CLI layer do not
/// need to import domain types (CN-01 / AC-03).
///
/// # Errors
///
/// Returns an error string on metadata read failure, codec failure, or impl-plan
/// load failure.
pub fn read_track_status_str(items_dir: &Path, track_id_str: &str) -> Result<String, String> {
    let items_dir = guarded_items_dir(items_dir).map_err(repository_error_message)?;
    let items_dir = items_dir.as_path();

    let valid_id =
        domain::TrackId::try_new(track_id_str).map_err(|e| format!("invalid track id: {e}"))?;

    // Symlink guard on the metadata read path: reject symlinks at the track directory or
    // any ancestor below `items_dir` before reading (fail-closed per ADR §D7).
    let metadata_path = items_dir.join(valid_id.as_ref()).join("metadata.json");
    reject_symlinks_below(&metadata_path, items_dir)
        .map_err(|e| format!("refusing to read metadata: {e}"))?;

    let (metadata, _doc_meta) =
        read_track_metadata(items_dir, &valid_id).map_err(|e| format!("{e}"))?;

    // Symlink guard on the impl-plan read path (fail-closed per ADR §D7).
    let impl_plan_path = items_dir.join(valid_id.as_ref()).join("impl-plan.json");
    reject_symlinks_below(&impl_plan_path, items_dir)
        .map_err(|e| format!("refusing to read impl-plan: {e}"))?;

    let store = FsTrackStore::new(items_dir);
    let impl_plan = store.load_impl_plan(&valid_id).map_err(|e| format!("{e}"))?;
    let status = domain::derive_track_status(impl_plan.as_ref(), metadata.status_override());
    Ok(status.to_string())
}

/// Load `impl-plan.json` for a track, returning `None` when the file is absent.
///
/// **WARNING — render-only helper**: this function silently absorbs I/O and
/// decode errors. A present-but-corrupt `impl-plan.json` is indistinguishable
/// from a missing one and the caller receives `None` in both cases.
///
/// This is acceptable **only** in display/rendering contexts where a corrupt
/// plan falls back gracefully to "Planned" status without security implications.
///
/// For any security-sensitive guard (active-track check, activation preflight,
/// type-signals guard, etc.) use `FsTrackStore::load_impl_plan` instead, which
/// propagates errors so that corruption blocks the operation rather than
/// silently bypassing it.
///
/// # Errors
///
/// Always returns `Ok`; individual failures are swallowed and treated as
/// "absent".
pub fn load_impl_plan_for_track(items_dir: &Path, id: &TrackId) -> Option<ImplPlanDocument> {
    let path = guarded_track_file_path(items_dir, id, "impl-plan.json").ok()??;
    let json = std::fs::read_to_string(&path).ok()?;
    crate::impl_plan_codec::decode(&json).ok()
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    use domain::{ImplPlanDocument, PlanView, StatusOverride, TrackId, TrackMetadata};

    fn test_snapshot() -> domain::branch_strategy::BranchStrategySnapshot {
        domain::branch_strategy::BranchStrategySnapshot::new(
            domain::NonEmptyString::try_new("main").unwrap(),
            domain::NonEmptyString::try_new("main").unwrap(),
            domain::branch_strategy::MergeMethod::Squash,
        )
    }

    fn sample_track(id: &str) -> TrackMetadata {
        // Identity-only TrackMetadata; status is derived on demand via derive_track_status.
        TrackMetadata::new(TrackId::try_new(id).unwrap(), "Test Track", None, test_snapshot())
            .unwrap()
    }

    fn write_sample_metadata(path: &Path, id: &str) {
        let track = sample_track(id);
        let meta = DocumentMeta {
            schema_version: 6,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        };
        let json = codec::encode(&track, &meta).unwrap();
        std::fs::write(path, json).unwrap();
    }

    fn write_empty_impl_plan(path: &Path) {
        let doc = ImplPlanDocument::new(vec![], PlanView::new(vec![], vec![])).unwrap();
        let json = crate::impl_plan_codec::encode(&doc).unwrap();
        std::fs::write(path, json).unwrap();
    }

    fn empty_impl_plan() -> ImplPlanDocument {
        ImplPlanDocument::new(vec![], PlanView::new(vec![], vec![])).unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn test_read_track_metadata_symlinked_items_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let real_items = dir.path().join("real-items");
        let link_items = dir.path().join("items-link");
        let real_track = real_items.join("test-track");
        std::fs::create_dir_all(&real_track).unwrap();
        write_sample_metadata(&real_track.join("metadata.json"), "test-track");
        std::os::unix::fs::symlink(&real_items, &link_items).unwrap();
        let id = TrackId::try_new("test-track").unwrap();

        let Err(RepositoryError::Message(message)) = read_track_metadata(&link_items, &id) else {
            panic!("symlinked items_dir must be rejected");
        };
        assert!(
            message.contains("symlink guard: refusing to use symlinked items_dir component"),
            "expected symlink guard error, got: {message}"
        );
    }

    #[test]
    fn test_read_track_metadata_parent_dir_items_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        write_sample_metadata(&track_dir.join("metadata.json"), "test-track");
        let parent_dir_items = dir.path().join("other").join("..").join("items");
        let id = TrackId::try_new("test-track").unwrap();

        let Err(RepositoryError::Message(message)) = read_track_metadata(&parent_dir_items, &id)
        else {
            panic!("items_dir with parent-dir component must be rejected");
        };
        assert!(
            message.contains("parent-dir component"),
            "expected parent-dir guard error, got: {message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_read_track_metadata_symlinked_track_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("items");
        let outside_track = dir.path().join("outside-track");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::create_dir_all(&outside_track).unwrap();
        write_sample_metadata(&outside_track.join("metadata.json"), "test-track");
        let id = TrackId::try_new("test-track").unwrap();
        std::os::unix::fs::symlink(&outside_track, items_dir.join(id.as_ref())).unwrap();

        let Err(RepositoryError::Message(message)) = read_track_metadata(&items_dir, &id) else {
            panic!("symlinked track directory must be rejected");
        };
        assert!(
            message.contains("symlink guard: refusing to read metadata.json"),
            "expected metadata symlink guard error, got: {message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_load_impl_plan_for_track_symlinked_items_dir_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let real_items = dir.path().join("real-items");
        let link_items = dir.path().join("items-link");
        let real_track = real_items.join("test-track");
        std::fs::create_dir_all(&real_track).unwrap();
        write_empty_impl_plan(&real_track.join("impl-plan.json"));
        std::os::unix::fs::symlink(&real_items, &link_items).unwrap();
        let id = TrackId::try_new("test-track").unwrap();

        let result = load_impl_plan_for_track(&link_items, &id);

        assert!(result.is_none(), "symlinked items_dir must not be read");
    }

    #[test]
    fn test_load_impl_plan_for_track_parent_dir_items_dir_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        write_empty_impl_plan(&track_dir.join("impl-plan.json"));
        let parent_dir_items = dir.path().join("other").join("..").join("items");
        let id = TrackId::try_new("test-track").unwrap();

        let result = load_impl_plan_for_track(&parent_dir_items, &id);

        assert!(result.is_none(), "items_dir with parent-dir component must not be read");
    }

    #[cfg(unix)]
    #[test]
    fn test_load_impl_plan_for_track_symlinked_track_dir_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("items");
        let outside_track = dir.path().join("outside-track");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::create_dir_all(&outside_track).unwrap();
        write_empty_impl_plan(&outside_track.join("impl-plan.json"));
        let id = TrackId::try_new("test-track").unwrap();
        std::os::unix::fs::symlink(&outside_track, items_dir.join(id.as_ref())).unwrap();

        let result = load_impl_plan_for_track(&items_dir, &id);

        assert!(result.is_none(), "symlinked track directory must not be read");
    }

    #[cfg(unix)]
    #[test]
    fn test_store_find_symlinked_track_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("items");
        let outside_track = dir.path().join("outside-track");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::create_dir_all(&outside_track).unwrap();
        write_sample_metadata(&outside_track.join("metadata.json"), "test-track");
        let id = TrackId::try_new("test-track").unwrap();
        std::os::unix::fs::symlink(&outside_track, items_dir.join(id.as_ref())).unwrap();

        let store = FsTrackStore::new(&items_dir);
        let Err(TrackReadError::Repository(RepositoryError::Message(message))) = store.find(&id)
        else {
            panic!("symlinked track directory must be rejected");
        };
        assert!(
            message.contains("symlink guard: refusing to read metadata.json"),
            "expected metadata symlink guard error, got: {message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_store_find_symlinked_metadata_file_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("items");
        let track_dir = items_dir.join("test-track");
        let outside_file = dir.path().join("metadata.json");
        std::fs::create_dir_all(&track_dir).unwrap();
        write_sample_metadata(&outside_file, "test-track");
        let id = TrackId::try_new("test-track").unwrap();
        std::os::unix::fs::symlink(&outside_file, track_dir.join("metadata.json")).unwrap();

        let store = FsTrackStore::new(&items_dir);
        let Err(TrackReadError::Repository(RepositoryError::Message(message))) = store.find(&id)
        else {
            panic!("symlinked metadata file must be rejected");
        };
        assert!(
            message.contains("symlink guard: refusing to read metadata.json"),
            "expected metadata symlink guard error, got: {message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_store_load_impl_plan_symlinked_track_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("items");
        let outside_track = dir.path().join("outside-track");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::create_dir_all(&outside_track).unwrap();
        write_empty_impl_plan(&outside_track.join("impl-plan.json"));
        let id = TrackId::try_new("test-track").unwrap();
        std::os::unix::fs::symlink(&outside_track, items_dir.join(id.as_ref())).unwrap();

        let store = FsTrackStore::new(&items_dir);
        let Err(RepositoryError::Message(message)) = store.load_impl_plan(&id) else {
            panic!("symlinked track directory must be rejected");
        };
        assert!(
            message.contains("symlink guard: refusing to read impl-plan.json"),
            "expected impl-plan symlink guard error, got: {message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_store_load_impl_plan_symlinked_file_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("items");
        let track_dir = items_dir.join("test-track");
        let outside_file = dir.path().join("impl-plan.json");
        std::fs::create_dir_all(&track_dir).unwrap();
        write_empty_impl_plan(&outside_file);
        let id = TrackId::try_new("test-track").unwrap();
        std::os::unix::fs::symlink(&outside_file, track_dir.join("impl-plan.json")).unwrap();

        let store = FsTrackStore::new(&items_dir);
        let Err(RepositoryError::Message(message)) = store.load_impl_plan(&id) else {
            panic!("symlinked impl-plan file must be rejected");
        };
        assert!(
            message.contains("symlink guard: refusing to read impl-plan.json"),
            "expected impl-plan symlink guard error, got: {message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_store_save_impl_plan_symlinked_items_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let real_items = dir.path().join("real-items");
        let link_items = dir.path().join("items-link");
        std::fs::create_dir_all(&real_items).unwrap();
        std::os::unix::fs::symlink(&real_items, &link_items).unwrap();
        let id = TrackId::try_new("test-track").unwrap();

        let store = FsTrackStore::new(&link_items);
        let Err(RepositoryError::Message(message)) = store.save_impl_plan(&id, &empty_impl_plan())
        else {
            panic!("symlinked items_dir must be rejected");
        };
        assert!(
            message.contains("symlink guard: refusing to use symlinked items_dir component"),
            "expected items_dir symlink guard error, got: {message}"
        );
        assert!(
            !real_items.join(id.as_ref()).join("impl-plan.json").exists(),
            "save_impl_plan must not write through a symlinked items_dir"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_store_save_impl_plan_symlinked_track_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("items");
        let outside_track = dir.path().join("outside-track");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::create_dir_all(&outside_track).unwrap();
        let id = TrackId::try_new("test-track").unwrap();
        std::os::unix::fs::symlink(&outside_track, items_dir.join(id.as_ref())).unwrap();

        let store = FsTrackStore::new(&items_dir);
        let Err(RepositoryError::Message(message)) = store.save_impl_plan(&id, &empty_impl_plan())
        else {
            panic!("symlinked track directory must be rejected");
        };
        assert!(
            message.contains("symlink guard: refusing to write impl-plan.json"),
            "expected impl-plan symlink guard error, got: {message}"
        );
        assert!(
            !outside_track.join("impl-plan.json").exists(),
            "save_impl_plan must not write through a symlinked track directory"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_store_save_symlinked_track_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("items");
        let outside_track = dir.path().join("outside-track");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::create_dir_all(&outside_track).unwrap();
        let track = sample_track("test-track");
        std::os::unix::fs::symlink(&outside_track, items_dir.join(track.id().as_ref())).unwrap();

        let store = FsTrackStore::new(&items_dir);
        let Err(TrackWriteError::Repository(RepositoryError::Message(message))) =
            store.save(&track)
        else {
            panic!("symlinked track directory must be rejected");
        };
        assert!(
            message.contains("symlink guard: refusing to read metadata.json")
                || message.contains("symlink guard: refusing to write metadata.json"),
            "expected metadata symlink guard error, got: {message}"
        );
        assert!(
            !outside_track.join("metadata.json").exists(),
            "save must not write through a symlinked track directory"
        );
    }

    #[test]
    fn test_save_and_find_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");

        store.save(&track).unwrap();
        let loaded = store.find(track.id()).unwrap().unwrap();
        assert_eq!(loaded, track);
    }

    #[test]
    fn test_find_returns_none_for_missing_track() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let id = TrackId::try_new("nonexistent").unwrap();

        let result = store.find(&id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_mutates_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");

        store.save(&track).unwrap();

        // Status is not stored; test that set_status_override persists.
        let updated = store
            .update(track.id(), |t| {
                t.set_status_override(Some(StatusOverride::blocked("testing").unwrap()));
                Ok(())
            })
            .unwrap();

        assert!(updated.status_override().is_some(), "override must be set after update");

        // Verify persistence.
        let reloaded = store.find(track.id()).unwrap().unwrap();
        assert!(reloaded.status_override().is_some(), "override must survive reload");
    }

    #[test]
    fn test_update_returns_error_for_missing_track() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let id = TrackId::try_new("nonexistent").unwrap();

        let result = store.update(&id, |_| Ok(()));
        assert!(matches!(
            result,
            Err(TrackWriteError::Repository(RepositoryError::TrackNotFound(_)))
        ));
    }

    #[test]
    fn test_save_new_track_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("new-track");

        let result = store.save(&track);
        assert!(result.is_ok());
    }

    #[test]
    fn test_store_save_missing_items_dir_creates_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track").join("items");
        let track = sample_track("new-track");

        assert!(!items_dir.exists(), "fixture must start without track/items");

        let store = FsTrackStore::new(&items_dir);
        store.save(&track).unwrap();

        assert!(
            items_dir.join(track.id().as_ref()).join("metadata.json").is_file(),
            "save must bootstrap track/items and write metadata"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_store_save_symlinked_items_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let real_items = dir.path().join("real-items");
        let link_items = dir.path().join("items-link");
        let track = sample_track("test-track");
        std::fs::create_dir_all(&real_items).unwrap();
        std::os::unix::fs::symlink(&real_items, &link_items).unwrap();

        let store = FsTrackStore::new(&link_items);
        let Err(TrackWriteError::Repository(RepositoryError::Message(message))) =
            store.save(&track)
        else {
            panic!("symlinked items_dir must be rejected");
        };

        assert!(
            message.contains("symlink guard: refusing to use symlinked items_dir component"),
            "expected items_dir symlink guard error, got: {message}"
        );
        assert!(
            !real_items.join(track.id().as_ref()).join("metadata.json").exists(),
            "save must not write through a symlinked items_dir"
        );
    }

    #[test]
    fn test_save_twice_with_same_data_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");

        store.save(&track).unwrap();
        let result = store.save(&track);
        assert!(result.is_ok());
    }

    #[test]
    fn test_save_preserves_created_at() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");

        store.save(&track).unwrap();

        // Read the raw JSON to check created_at.
        let path = dir.path().join("test-track").join("metadata.json");
        let json = std::fs::read_to_string(&path).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        let created_at = doc["created_at"].as_str().unwrap().to_owned();

        // Save again — created_at should be preserved.
        store.save(&track).unwrap();

        let json2 = std::fs::read_to_string(&path).unwrap();
        let doc2: serde_json::Value = serde_json::from_str(&json2).unwrap();
        assert_eq!(doc2["created_at"].as_str().unwrap(), created_at);
    }

    // --- with_locked_document tests ---

    #[test]
    fn test_with_locked_document_returns_error_for_missing_track() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let id = TrackId::try_new("nonexistent").unwrap();

        let result = store.with_locked_document(&id, |_, _| Ok(()));
        assert!(matches!(
            result,
            Err(TrackWriteError::Repository(RepositoryError::TrackNotFound(_)))
        ));
    }

    #[test]
    fn test_with_locked_document_mutates_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");
        store.save(&track).unwrap();

        // Status is not stored; test that set_status_override persists via with_locked_document.
        let updated = store
            .with_locked_document(track.id(), |t, _meta| {
                t.set_status_override(Some(StatusOverride::blocked("locked test").unwrap()));
                Ok(())
            })
            .unwrap();

        assert!(
            updated.status_override().is_some(),
            "override must be set after with_locked_document"
        );

        // Verify persistence.
        let reloaded = store.find(track.id()).unwrap().unwrap();
        assert!(
            reloaded.status_override().is_some(),
            "override must survive reload after with_locked_document"
        );
    }

    #[test]
    fn test_with_locked_document_does_not_auto_set_updated_at() {
        // The closure sets updated_at explicitly; with_locked_document must not
        // override it.
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");
        store.save(&track).unwrap();

        let sentinel = "1999-01-01T00:00:00Z".to_owned();
        store
            .with_locked_document(track.id(), |_t, meta| {
                meta.updated_at = sentinel.clone();
                Ok(())
            })
            .unwrap();

        let path = dir.path().join("test-track").join("metadata.json");
        let json = std::fs::read_to_string(&path).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(doc["updated_at"].as_str().unwrap(), sentinel);
    }

    #[test]
    fn test_with_locked_document_does_not_write_on_closure_error() {
        // If the closure returns Err, nothing should be written to disk.
        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        let track = sample_track("test-track");
        store.save(&track).unwrap();

        // Record the content before the failed call.
        let path = dir.path().join("test-track").join("metadata.json");
        let json_before = std::fs::read_to_string(&path).unwrap();

        let result = store.with_locked_document(track.id(), |_, _| {
            Err(DomainError::Validation(domain::ValidationError::InvalidTaskId(
                domain::tddd::test_obligation::ids::DiagnosticMessage::try_new(
                    "intentional error".to_owned(),
                )
                .expect("non-empty diagnostic"),
            )))
        });
        assert!(result.is_err());

        // File must be unchanged.
        let json_after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(json_before, json_after);
    }

    #[cfg(unix)]
    #[test]
    fn test_with_locked_document_symlinked_lock_file_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("items");
        let outside_file = dir.path().join("outside-lock-target");
        std::fs::create_dir_all(&items_dir).unwrap();
        let store = FsTrackStore::new(&items_dir);
        let track = sample_track("test-track");
        store.save(&track).unwrap();
        std::fs::write(&outside_file, "do not truncate").unwrap();

        let lock_path = items_dir.join(track.id().as_ref()).join("metadata.json.lock");
        std::os::unix::fs::symlink(&outside_file, &lock_path).unwrap();

        let Err(TrackWriteError::Repository(RepositoryError::Message(message))) =
            store.with_locked_document(track.id(), |_, _| Ok(()))
        else {
            panic!("symlinked lock file must be rejected");
        };
        assert!(
            message.contains("symlink guard: refusing to write metadata.json.lock"),
            "expected lock-file symlink guard error, got: {message}"
        );
        assert_eq!(std::fs::read_to_string(outside_file).unwrap(), "do not truncate");
    }

    #[test]
    fn test_init_with_branch_strategy_port_writes_snapshot() {
        use crate::branch_strategy::SnapshotBranchStrategyAdapter;
        use domain::NonEmptyString;
        use domain::branch_strategy::{BranchStrategySnapshot, MergeMethod};
        use usecase::branch_strategy::BranchStrategyPort;

        // Stub port via SnapshotBranchStrategyAdapter
        let snapshot = BranchStrategySnapshot::new(
            NonEmptyString::try_new("develop").unwrap(),
            NonEmptyString::try_new("develop").unwrap(),
            MergeMethod::Squash,
        );
        let port = SnapshotBranchStrategyAdapter::new(snapshot.clone());

        // Simulate what init would do: create TrackMetadata using port values
        let branch_strategy_snapshot = BranchStrategySnapshot::new(
            NonEmptyString::try_new(port.base_branch()).unwrap(),
            NonEmptyString::try_new(port.merge_target()).unwrap(),
            port.merge_method(),
        );
        let track = TrackMetadata::new(
            TrackId::try_new("init-test-track").unwrap(),
            "Init Test",
            None,
            branch_strategy_snapshot,
        )
        .unwrap();

        let dir = tempfile::tempdir().unwrap();
        let store = FsTrackStore::new(dir.path());
        store.save(&track).unwrap();

        let loaded = store.find(track.id()).unwrap().unwrap();
        let loaded_snap = loaded.branch_strategy_snapshot();
        assert_eq!(loaded_snap.base_branch(), "develop");
        assert_eq!(loaded_snap.merge_target(), "develop");
        assert_eq!(loaded_snap.merge_method(), MergeMethod::Squash);
    }
}
