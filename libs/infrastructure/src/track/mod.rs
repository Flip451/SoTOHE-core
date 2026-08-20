pub mod atomic_write;
pub mod codec;
pub mod fixpoint_resolve_driver;
pub mod fs_spec_file_loader;
pub mod fs_store;
pub mod fs_symlink_guard;
pub mod gate_state;
pub(crate) mod registry_lock;
pub mod render;
pub mod spec_element_hash;
pub mod symlink_guard;
pub mod track_status_reader_adapter;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::{BranchStrategySnapshot, CommitHash, NonEmptyString, TrackId, TrackMetadata};
use usecase::git_workflow::{
    DiagnosticText, GitPrimitivePort, GitWorkflowError, ReviewGitInteractor,
};
use usecase::track_lifecycle::{
    RenderedViewPath, TrackBranchStrategyPort, TrackCommitHashPort, TrackItemsDirectory,
    TrackMetadataPort, TrackSelection, TrackSelectionPort, TrackViewsPort, TrackViewsScope,
    TrackWorkspaceRoot,
};

use crate::git_cli::SystemGitRepo;
use crate::review_v2::FsCommitHashStore;

/// Filesystem adapter for track metadata persistence.
pub struct FsTrackMetadataAdapter;

impl FsTrackMetadataAdapter {
    /// Creates a metadata adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsTrackMetadataAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TrackMetadataPort for FsTrackMetadataAdapter {
    fn save(
        &self,
        items_dir: &TrackItemsDirectory,
        metadata: TrackMetadata,
    ) -> Result<(), usecase::git_workflow::DiagnosticText> {
        use domain::TrackWriter as _;

        fs_store::FsTrackStore::new(items_dir.as_path())
            .save(&metadata)
            .map_err(|error| track_write_diagnostic(error, items_dir.as_path()))
    }

    fn find(
        &self,
        items_dir: &TrackItemsDirectory,
        track_id: &TrackId,
    ) -> Result<Option<TrackMetadata>, usecase::git_workflow::DiagnosticText> {
        use domain::TrackReader as _;

        fs_store::FsTrackStore::new(items_dir.as_path())
            .find(track_id)
            .map_err(|error| track_read_diagnostic(error, items_dir.as_path()))
    }
}

/// Filesystem adapter for rendered Track views.
pub struct FsTrackViewsAdapter;

impl FsTrackViewsAdapter {
    /// Creates a rendered-view adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsTrackViewsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TrackViewsPort for FsTrackViewsAdapter {
    fn validate(
        &self,
        workspace_root: &TrackWorkspaceRoot,
    ) -> Result<(), usecase::git_workflow::DiagnosticText> {
        render::validate_track_snapshots(workspace_root.as_path())
            .map_err(|error| render_diagnostic(error, workspace_root.as_path()))
    }

    fn sync(
        &self,
        workspace_root: &TrackWorkspaceRoot,
        scope: &TrackViewsScope,
    ) -> Result<Vec<RenderedViewPath>, usecase::git_workflow::DiagnosticText> {
        let track_id = match scope {
            TrackViewsScope::RegistryOnly => None,
            TrackViewsScope::Track(track_id) => Some(track_id.as_ref()),
        };
        render::sync_rendered_views(workspace_root.as_path(), track_id)
            .map(|paths| paths.into_iter().map(RenderedViewPath::new).collect())
            .map_err(|error| render_diagnostic(error, workspace_root.as_path()))
    }
}

/// Git/filesystem adapter for persisting a track's current commit hash.
pub struct GitTrackCommitHashAdapter;

impl GitTrackCommitHashAdapter {
    /// Creates a commit-hash adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitTrackCommitHashAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TrackCommitHashPort for GitTrackCommitHashAdapter {
    fn persist_current_for_track(
        &self,
        track_id: &TrackId,
    ) -> Result<CommitHash, usecase::git_workflow::DiagnosticText> {
        let git = SystemGitRepo::discover()
            .map_err(|error| git_diagnostic("git repository discovery", error, &[]))?;
        let branch = git
            .current_branch()
            .map_err(|error| git_diagnostic("git current branch lookup", error, &[git.root()]))?
            .ok_or_else(|| {
                usecase::git_workflow::DiagnosticText::new(
                    "git rev-parse --abbrev-ref HEAD failed (cannot verify branch)",
                )
            })?;
        let expected = format!("track/{track_id}");
        if branch != expected {
            return Err(usecase::git_workflow::DiagnosticText::new(format!(
                "current branch '{branch}' does not match track branch '{expected}'"
            )));
        }

        let port: Arc<dyn GitPrimitivePort> = Arc::new(crate::FsGitWorkflowAdapter::new());
        let commit_hash = ReviewGitInteractor::new(port)
            .resolve_head_for_track_branch(track_id)
            .map_err(|error| git_head_resolution_diagnostic(error, track_id, git.root()))?;
        let canonical_root = git.root().canonicalize().map_err(|error| {
            diagnostic(format!("failed to canonicalize repository root: {error}"), &[git.root()])
        })?;
        let track_dir = canonical_root.join("track/items").join(track_id.as_ref());
        ensure_track_directory(&track_dir, track_id)?;
        let commit_hash_path = track_dir.join(".commit_hash");
        let store = FsCommitHashStore::new(commit_hash_path.clone(), canonical_root);
        <FsCommitHashStore as domain::review_v2::CommitHashWriter>::write(&store, &commit_hash)
            .map_err(|error| commit_hash_diagnostic(error, &commit_hash_path))?;
        Ok(commit_hash)
    }
}

fn ensure_track_directory(
    track_dir: &Path,
    track_id: &TrackId,
) -> Result<(), usecase::git_workflow::DiagnosticText> {
    let link_metadata = std::fs::symlink_metadata(track_dir).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            DiagnosticText::new(format!("track directory for track '{track_id}' does not exist"))
        } else {
            diagnostic(
                format!("failed to inspect track directory for track '{track_id}': {error}"),
                &[track_dir],
            )
        }
    })?;

    if link_metadata.file_type().is_symlink() {
        return Err(diagnostic(
            format!("track directory for track '{track_id}' is a symlink"),
            &[track_dir],
        ));
    }

    let metadata = std::fs::metadata(track_dir).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            DiagnosticText::new(format!("track directory for track '{track_id}' does not exist"))
        } else {
            diagnostic(
                format!("failed to inspect track directory for track '{track_id}': {error}"),
                &[track_dir],
            )
        }
    })?;
    if !metadata.is_dir() {
        return Err(diagnostic(
            format!("track path for track '{track_id}' is not a directory"),
            &[track_dir],
        ));
    }
    Ok(())
}

/// Filesystem adapter for branch-strategy configuration and track snapshots.
pub struct FsTrackBranchStrategyAdapter;

impl TrackBranchStrategyPort for FsTrackBranchStrategyAdapter {
    fn global_for_items(
        &self,
        items_dir: &TrackItemsDirectory,
    ) -> Result<BranchStrategySnapshot, usecase::git_workflow::DiagnosticText> {
        let workspace_root = workspace_root_for_items(items_dir.as_path())?;
        let config_path = workspace_root.join(".harness/config/branch-strategy.json");
        let config = crate::JsonConfigBranchStrategyAdapter::new(config_path.clone())
            .map_err(|error| branch_strategy_config_diagnostic(error, &config_path))?;
        snapshot_from_strategy(&config)
    }

    fn snapshot_for_track(
        &self,
        workspace_root: &TrackWorkspaceRoot,
        track_id: &TrackId,
    ) -> Result<BranchStrategySnapshot, usecase::git_workflow::DiagnosticText> {
        use domain::TrackReader as _;

        let items_dir = workspace_root.as_path().join("track/items");
        let track = fs_store::FsTrackStore::new(items_dir)
            .find(track_id)
            .map_err(|error| track_read_diagnostic(error, workspace_root.as_path()))?
            .ok_or_else(|| {
                usecase::git_workflow::DiagnosticText::new(format!(
                    "track '{track_id}' metadata was not found"
                ))
            })?;
        Ok(track.branch_strategy_snapshot().clone())
    }
}

/// Git-backed adapter for active and explicit track selection.
pub struct GitTrackSelectionAdapter;

impl GitTrackSelectionAdapter {
    fn resolve_for_write(
        &self,
        workspace_root: &TrackWorkspaceRoot,
        track_id: &TrackId,
    ) -> Result<TrackId, usecase::git_workflow::DiagnosticText> {
        let repo = SystemGitRepo::discover_from(workspace_root.as_path()).map_err(|error| {
            git_diagnostic("git repository discovery", error, &[workspace_root.as_path()])
        })?;
        let interactor =
            usecase::track_resolution::ActiveTrackResolveInteractor::new(Arc::new(repo));
        let resolved = usecase::track_resolution::ActiveTrackResolveService::resolve_for_write(
            &interactor,
            Some(track_id.as_ref().to_owned()),
        )
        .map_err(|error| active_track_resolution_diagnostic(error, workspace_root.as_path()))?;
        TrackId::try_new(resolved).map_err(|error| {
            usecase::git_workflow::DiagnosticText::new(format!("invalid active track id: {error}"))
        })
    }
}

impl TrackSelectionPort for GitTrackSelectionAdapter {
    fn resolve_required(
        &self,
        items_dir: &TrackItemsDirectory,
        selection: &TrackSelection,
    ) -> Result<TrackId, usecase::git_workflow::DiagnosticText> {
        match selection {
            TrackSelection::Explicit(track_id) => {
                let root = workspace_root_for_items(items_dir.as_path())?;
                self.resolve_for_write(&TrackWorkspaceRoot::try_new(root)?, track_id)
            }
            TrackSelection::Active => {
                let root = workspace_root_for_items(items_dir.as_path())?;
                self.resolve_active(&TrackWorkspaceRoot::try_new(root)?)
            }
        }
    }

    fn resolve_active(
        &self,
        workspace_root: &TrackWorkspaceRoot,
    ) -> Result<TrackId, usecase::git_workflow::DiagnosticText> {
        let repo = SystemGitRepo::discover_from(workspace_root.as_path()).map_err(|error| {
            git_diagnostic("git repository discovery", error, &[workspace_root.as_path()])
        })?;
        let interactor =
            usecase::track_resolution::ActiveTrackResolveInteractor::new(Arc::new(repo));
        usecase::track_resolution::ActiveTrackResolveService::resolve_active_track(&interactor)
            .map_err(|error| active_track_resolution_diagnostic(error, workspace_root.as_path()))
            .and_then(|track_id| {
                TrackId::try_new(track_id).map_err(|error| {
                    usecase::git_workflow::DiagnosticText::new(format!(
                        "invalid active track id: {error}"
                    ))
                })
            })
    }

    fn resolve_views_scope(
        &self,
        workspace_root: &TrackWorkspaceRoot,
        selection: &TrackSelection,
    ) -> Result<TrackViewsScope, usecase::git_workflow::DiagnosticText> {
        match selection {
            TrackSelection::Explicit(track_id) => {
                Ok(TrackViewsScope::Track(self.resolve_for_write(workspace_root, track_id)?))
            }
            TrackSelection::Active => match self.resolve_active(workspace_root) {
                Ok(track_id) => Ok(TrackViewsScope::Track(track_id)),
                Err(_) => Ok(TrackViewsScope::RegistryOnly),
            },
        }
    }
}

fn workspace_root_for_items(
    items_dir: &Path,
) -> Result<PathBuf, usecase::git_workflow::DiagnosticText> {
    let track_dir = items_dir.parent().ok_or_else(|| {
        usecase::git_workflow::DiagnosticText::new("track items directory has no track parent")
    })?;
    let root = track_dir.parent().ok_or_else(|| {
        usecase::git_workflow::DiagnosticText::new("track items directory has no workspace root")
    })?;
    if root.as_os_str().is_empty() { Ok(PathBuf::from(".")) } else { Ok(root.to_path_buf()) }
}

fn snapshot_from_strategy(
    strategy: &dyn usecase::branch_strategy::BranchStrategyPort,
) -> Result<BranchStrategySnapshot, usecase::git_workflow::DiagnosticText> {
    let base_branch = NonEmptyString::try_new(strategy.base_branch()).map_err(|error| {
        usecase::git_workflow::DiagnosticText::new(format!("invalid base branch: {error}"))
    })?;
    let merge_target = NonEmptyString::try_new(strategy.merge_target()).map_err(|error| {
        usecase::git_workflow::DiagnosticText::new(format!("invalid merge target: {error}"))
    })?;
    Ok(BranchStrategySnapshot::new(base_branch, merge_target, strategy.merge_method()))
}

fn diagnostic(message: impl Into<String>, known_paths: &[&Path]) -> DiagnosticText {
    DiagnosticText::new(sanitize_environment_paths(&message.into(), known_paths))
}

fn sanitize_environment_paths(message: &str, known_paths: &[&Path]) -> String {
    let mut sanitized = message.to_owned();
    for path in known_paths {
        if path.is_absolute() && path.components().count() > 1 {
            let display = path.to_string_lossy();
            sanitized = sanitized.replace(display.as_ref(), "<path>");
        }
    }
    sanitize_absolute_path_tokens(&sanitized)
}

fn sanitize_absolute_path_tokens(message: &str) -> String {
    let chars: Vec<char> = message.chars().collect();
    let mut sanitized = String::with_capacity(message.len());
    let mut index = 0;
    while index < chars.len() {
        if starts_unix_path(&chars, index) || starts_windows_path(&chars, index) {
            sanitized.push_str("<path>");
            index = consume_path_token(&chars, index);
        } else {
            if let Some(character) = chars.get(index) {
                sanitized.push(*character);
            }
            index += 1;
        }
    }
    sanitized
}

fn starts_unix_path(chars: &[char], index: usize) -> bool {
    chars.get(index) == Some(&'/')
        && (index == 0
            || chars
                .get(index.saturating_sub(1))
                .is_some_and(|character| is_path_boundary(*character)))
}

fn starts_windows_path(chars: &[char], index: usize) -> bool {
    let (Some(&drive), Some(&colon), Some(&separator)) =
        (chars.get(index), chars.get(index + 1), chars.get(index + 2))
    else {
        return false;
    };
    drive.is_ascii_alphabetic()
        && colon == ':'
        && matches!(separator, '/' | '\\')
        && (index == 0
            || chars
                .get(index.saturating_sub(1))
                .is_some_and(|character| is_path_boundary(*character)))
}

fn is_path_boundary(character: char) -> bool {
    character.is_whitespace() || matches!(character, ':' | '=' | '(' | '[' | '{' | '\'' | '"')
}

fn consume_path_token(chars: &[char], mut index: usize) -> usize {
    let start = index;
    let is_windows_drive = chars
        .get(start)
        .zip(chars.get(start + 1))
        .is_some_and(|(drive, colon)| drive.is_ascii_alphabetic() && *colon == ':');
    while let Some(&character) = chars.get(index) {
        let is_drive_colon = is_windows_drive && index == start + 1;
        if character.is_whitespace()
            || (matches!(character, ':' | ',' | ';' | ')' | ']' | '}' | '>' | '\'' | '"')
                && !is_drive_colon)
        {
            break;
        }
        index += 1;
    }
    index
}

fn track_write_diagnostic(error: domain::TrackWriteError, items_dir: &Path) -> DiagnosticText {
    let message = match error {
        domain::TrackWriteError::Domain(error) => {
            format!("track metadata validation failed: {error}")
        }
        domain::TrackWriteError::Repository(domain::RepositoryError::TrackNotFound(track_id)) => {
            format!("track metadata for '{track_id}' was not found")
        }
        domain::TrackWriteError::Repository(domain::RepositoryError::Message(message)) => {
            format!("track metadata persistence failed: {message}")
        }
    };
    diagnostic(message, &[items_dir])
}

fn track_read_diagnostic(error: domain::TrackReadError, items_dir: &Path) -> DiagnosticText {
    let message = match error {
        domain::TrackReadError::Repository(domain::RepositoryError::TrackNotFound(track_id)) => {
            format!("track metadata for '{track_id}' was not found")
        }
        domain::TrackReadError::Repository(domain::RepositoryError::Message(message)) => {
            format!("track metadata read failed: {message}")
        }
    };
    diagnostic(message, &[items_dir])
}

fn render_diagnostic(error: render::RenderError, workspace_root: &Path) -> DiagnosticText {
    match error {
        render::RenderError::Io(error) => {
            diagnostic(format!("rendered view I/O failed: {error}"), &[workspace_root])
        }
        render::RenderError::InvalidMetadata { path, source } => diagnostic(
            format!("invalid rendered view metadata at {}: {source}", path.display()),
            &[workspace_root, path.as_path()],
        ),
        render::RenderError::OutOfSync { path, reason } => diagnostic(
            format!("rendered view out of sync at {}: {reason}", path.display()),
            &[workspace_root, path.as_path()],
        ),
        render::RenderError::UnsupportedSchemaVersion { path, schema_version } => diagnostic(
            format!(
                "unsupported rendered view schema version {schema_version} at {}",
                path.display()
            ),
            &[workspace_root, path.as_path()],
        ),
        render::RenderError::InvalidTrackMetadata { path, reason } => diagnostic(
            format!("invalid track metadata at {}: {reason}", path.display()),
            &[workspace_root, path.as_path()],
        ),
    }
}

fn git_diagnostic(
    operation: &str,
    error: crate::git_cli::GitError,
    known_paths: &[&Path],
) -> DiagnosticText {
    let detail = match error {
        crate::git_cli::GitError::CurrentDir(error) => {
            format!("current directory unavailable: {error}")
        }
        crate::git_cli::GitError::Spawn { command, source } => {
            format!("git {command} could not be started: {source}")
        }
        crate::git_cli::GitError::CommandFailed { command, code, stderr } => {
            format!("git {command} failed with status {code}: {stderr}")
        }
        crate::git_cli::GitError::EmptyRepoRoot => "repository root was empty".to_owned(),
    };
    diagnostic(format!("{operation}: {detail}"), known_paths)
}

fn commit_hash_diagnostic(
    error: domain::review_v2::CommitHashError,
    commit_hash_path: &Path,
) -> DiagnosticText {
    let detail = match error {
        domain::review_v2::CommitHashError::Io { path, detail } => {
            format!("I/O error at {path}: {detail}")
        }
        domain::review_v2::CommitHashError::SymlinkDetected { path } => {
            format!("symlink detected at {path}")
        }
        domain::review_v2::CommitHashError::Format(detail) => {
            format!("format error: {detail}")
        }
    };
    diagnostic(format!("commit hash persistence failed: {detail}"), &[commit_hash_path])
}

fn branch_strategy_config_diagnostic(
    error: crate::branch_strategy::BranchStrategyConfigError,
    config_path: &Path,
) -> DiagnosticText {
    let message = match error {
        crate::branch_strategy::BranchStrategyConfigError::Io(error) => {
            format!("branch strategy configuration I/O failed: {error}")
        }
        crate::branch_strategy::BranchStrategyConfigError::Parse(error) => {
            format!("branch strategy configuration parse failed: {error}")
        }
    };
    diagnostic(message, &[config_path])
}

fn active_track_resolution_diagnostic(
    error: usecase::track_resolution::ActiveTrackResolveError,
    workspace_root: &Path,
) -> DiagnosticText {
    let detail = match error {
        usecase::track_resolution::ActiveTrackResolveError::BranchRead(error) => {
            format!("branch read failed: {error}")
        }
        usecase::track_resolution::ActiveTrackResolveError::Resolution(error) => {
            format!("track resolution failed: {error}")
        }
        usecase::track_resolution::ActiveTrackResolveError::BranchMismatch {
            explicit_id,
            branch_id,
        } => format!(
            "WRITE guard mismatch: explicit track-id '{explicit_id}' does not match branch-derived track-id '{branch_id}'"
        ),
    };
    diagnostic(format!("active track resolution failed: {detail}"), &[workspace_root])
}

fn git_head_resolution_diagnostic(
    error: GitWorkflowError,
    track_id: &TrackId,
    repository_root: &Path,
) -> DiagnosticText {
    let detail = match error {
        GitWorkflowError::Validation(detail) => format!("validation failed: {detail}"),
        GitWorkflowError::NoBranch => "current branch could not be determined".to_owned(),
        GitWorkflowError::DetachedHead(detail) => format!("detached HEAD: {detail}"),
        GitWorkflowError::BranchMismatch { current, expected } => {
            format!("branch mismatch: current '{current}' does not match expected '{expected}'")
        }
        GitWorkflowError::Message(detail) => format!("workflow message: {detail}"),
        GitWorkflowError::Unavailable(detail) => format!("workflow unavailable: {detail}"),
        GitWorkflowError::SyncUpstreamNotSet => "sync upstream is not configured".to_owned(),
        GitWorkflowError::SyncNonFastForward { stderr } => {
            format!("sync rejected as non-fast-forward: {stderr}")
        }
        GitWorkflowError::SyncWorktreeUnresolved { stderr } => {
            format!("sync worktree is unresolved: {stderr}")
        }
        GitWorkflowError::Fs { detail } => format!("filesystem failure: {detail}"),
        GitWorkflowError::SwitchFailed { branch, exit_code } => {
            format!("switch to '{branch}' failed with exit code {exit_code}")
        }
    };
    diagnostic(
        format!("git HEAD resolution failed for track '{track_id}': {detail}"),
        &[repository_root],
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod lifecycle_adapter_tests {
    use std::fs;
    use std::path::Path;

    use domain::branch_strategy::MergeMethod;
    use domain::{BranchStrategySnapshot, NonEmptyString, TrackId, TrackMetadata};
    use tempfile::tempdir;
    use usecase::track_lifecycle::{
        TrackBranchStrategyPort, TrackCommitHashPort, TrackItemsDirectory, TrackMetadataPort,
        TrackSelection, TrackSelectionPort, TrackViewsPort, TrackViewsScope, TrackWorkspaceRoot,
    };

    use super::{
        FsTrackBranchStrategyAdapter, FsTrackMetadataAdapter, FsTrackViewsAdapter,
        GitTrackCommitHashAdapter, GitTrackSelectionAdapter,
    };

    const COMMIT_HASH_CHILD_CASE_ENV: &str = "SOTOHE_TRACK_COMMIT_HASH_TEST_CASE";

    fn test_snapshot() -> BranchStrategySnapshot {
        BranchStrategySnapshot::new(
            NonEmptyString::try_new("main").expect("base branch is valid"),
            NonEmptyString::try_new("main").expect("merge target is valid"),
            MergeMethod::Squash,
        )
    }

    #[test]
    fn test_fs_track_branch_strategy_global_config_returns_snapshot() {
        let root = tempdir().expect("temporary root is created");
        let items_dir = root.path().join("track/items");
        fs::create_dir_all(root.path().join(".harness/config")).expect("config directory exists");
        fs::create_dir_all(&items_dir).expect("items directory exists");
        fs::write(
            root.path().join(".harness/config/branch-strategy.json"),
            r#"{"base_branch":"main","merge_target":"main","merge_method":"squash"}"#,
        )
        .expect("branch strategy config is written");

        let typed_items = TrackItemsDirectory::try_new(items_dir).expect("items path is valid");
        let snapshot = FsTrackBranchStrategyAdapter
            .global_for_items(&typed_items)
            .expect("global strategy loads");

        assert_eq!(snapshot.base_branch(), "main");
        assert_eq!(snapshot.merge_target(), "main");
        assert_eq!(snapshot.merge_method(), MergeMethod::Squash);
    }

    #[test]
    fn test_fs_track_metadata_save_then_find_returns_same_metadata() {
        let root = tempdir().expect("temporary root is created");
        let items_dir = root.path().join("track/items");
        fs::create_dir_all(&items_dir).expect("items directory exists");
        let typed_items = TrackItemsDirectory::try_new(items_dir).expect("items path is valid");
        let track_id = TrackId::try_new("adapter-test").expect("track id is valid");
        let metadata = TrackMetadata::new(track_id, "Adapter test", None, test_snapshot())
            .expect("metadata is valid");
        let adapter = FsTrackMetadataAdapter::new();

        adapter.save(&typed_items, metadata.clone()).expect("metadata saves");
        let loaded = adapter
            .find(&typed_items, metadata.id())
            .expect("metadata loads")
            .expect("saved metadata exists");

        assert_eq!(loaded, metadata);
    }

    #[test]
    fn test_fs_track_metadata_find_missing_returns_none() {
        let root = tempdir().expect("temporary root is created");
        let items_dir = root.path().join("track/items");
        fs::create_dir_all(&items_dir).expect("items directory exists");
        let typed_items = TrackItemsDirectory::try_new(items_dir).expect("items path is valid");
        let track_id = TrackId::try_new("missing-track").expect("track id is valid");

        let loaded = FsTrackMetadataAdapter::new()
            .find(&typed_items, &track_id)
            .expect("missing metadata lookup succeeds");

        assert!(loaded.is_none(), "absent metadata must not be invented");
    }

    #[test]
    fn test_fs_track_branch_strategy_snapshot_for_track_returns_saved_snapshot() {
        let root = tempdir().expect("temporary root is created");
        let items_dir = root.path().join("track/items");
        fs::create_dir_all(&items_dir).expect("items directory exists");
        let typed_items = TrackItemsDirectory::try_new(items_dir).expect("items path is valid");
        let track_id = TrackId::try_new("snapshot-track").expect("track id is valid");
        let metadata =
            TrackMetadata::new(track_id.clone(), "Snapshot track", None, test_snapshot())
                .expect("metadata is valid");
        FsTrackMetadataAdapter::new().save(&typed_items, metadata).expect("metadata saves");

        let workspace = TrackWorkspaceRoot::try_new(root.path().to_path_buf())
            .expect("workspace path is valid");
        let snapshot = FsTrackBranchStrategyAdapter
            .snapshot_for_track(&workspace, &track_id)
            .expect("saved snapshot loads");

        assert_eq!(snapshot, test_snapshot());
    }

    #[test]
    fn test_git_track_selection_explicit_selection_returns_same_track() {
        let root = tempdir().expect("temporary root is created");
        init_git_repo_on_branch(root.path(), "track/explicit-track");
        let items_path = root.path().join("track/items");
        fs::create_dir_all(&items_path).expect("items directory exists");
        let adapter = GitTrackSelectionAdapter;
        let items_dir = TrackItemsDirectory::try_new(items_path).expect("items path is valid");
        let track_id = TrackId::try_new("explicit-track").expect("track id is valid");
        let selection = TrackSelection::Explicit(track_id.clone());

        let resolved =
            adapter.resolve_required(&items_dir, &selection).expect("explicit selection resolves");
        assert_eq!(resolved, track_id);
    }

    #[test]
    fn test_git_track_selection_explicit_selection_rejects_branch_mismatch_for_write() {
        let root = tempdir().expect("temporary root is created");
        init_git_repo_on_branch(root.path(), "track/active-track");
        let items_path = root.path().join("track/items");
        fs::create_dir_all(&items_path).expect("items directory exists");
        let items_dir = TrackItemsDirectory::try_new(items_path).expect("items path is valid");
        let selection =
            TrackSelection::Explicit(TrackId::try_new("other-track").expect("track id is valid"));

        let error = GitTrackSelectionAdapter
            .resolve_required(&items_dir, &selection)
            .expect_err("explicit baseline-graph selection must enforce the write guard");

        assert!(error.to_string().contains("WRITE guard mismatch"));
    }

    #[test]
    fn test_git_track_selection_explicit_view_scope_returns_track_scope() {
        let root = tempdir().expect("temporary root is created");
        init_git_repo_on_branch(root.path(), "track/explicit-track");
        let adapter = GitTrackSelectionAdapter;
        let workspace = TrackWorkspaceRoot::try_new(root.path().to_path_buf())
            .expect("workspace path is valid");
        let track_id = TrackId::try_new("explicit-track").expect("track id is valid");

        let scope = adapter
            .resolve_views_scope(&workspace, &TrackSelection::Explicit(track_id.clone()))
            .expect("explicit view scope resolves");
        assert_eq!(scope, TrackViewsScope::Track(track_id));
    }

    #[test]
    fn test_git_track_selection_explicit_view_scope_rejects_branch_mismatch() {
        let root = tempdir().expect("temporary root is created");
        init_git_repo_on_branch(root.path(), "track/active-track");
        let workspace = TrackWorkspaceRoot::try_new(root.path().to_path_buf())
            .expect("workspace path is valid");
        let track_id = TrackId::try_new("other-track").expect("track id is valid");

        let error = GitTrackSelectionAdapter
            .resolve_views_scope(&workspace, &TrackSelection::Explicit(track_id))
            .expect_err("explicit view scope must enforce the write guard");

        assert!(error.to_string().contains("WRITE guard mismatch"));
    }

    #[test]
    fn test_git_track_selection_active_branch_returns_track() {
        let root = tempdir().expect("temporary root is created");
        init_git_repo_on_branch(root.path(), "track/active-track");
        let workspace =
            TrackWorkspaceRoot::try_new(root.path().to_path_buf()).expect("workspace is valid");

        let resolved =
            GitTrackSelectionAdapter.resolve_active(&workspace).expect("active branch resolves");

        assert_eq!(resolved, TrackId::try_new("active-track").expect("track id is valid"));
    }

    #[test]
    fn test_git_track_selection_active_on_non_track_branch_returns_registry_scope() {
        let root = tempdir().expect("temporary root is created");
        init_git_repo_on_branch(root.path(), "main");
        let workspace = TrackWorkspaceRoot::try_new(root.path().to_path_buf())
            .expect("workspace path is valid");

        let scope = GitTrackSelectionAdapter
            .resolve_views_scope(&workspace, &TrackSelection::Active)
            .expect("non-track branches use registry-only view synchronization");

        assert_eq!(scope, TrackViewsScope::RegistryOnly);
    }

    #[test]
    fn test_fs_track_views_validate_and_sync_registry_scope() {
        let root = tempdir().expect("temporary root is created");
        fs::create_dir_all(root.path().join("track/items")).expect("items directory exists");
        let workspace =
            TrackWorkspaceRoot::try_new(root.path().to_path_buf()).expect("workspace is valid");
        let adapter = FsTrackViewsAdapter::new();

        adapter.validate(&workspace).expect("empty workspace validates");
        let rendered = adapter
            .sync(&workspace, &TrackViewsScope::RegistryOnly)
            .expect("registry scope synchronizes");

        assert!(rendered.iter().any(|path| path.as_path().ends_with("registry.md")));
        assert!(root.path().join("track/registry.md").is_file());
    }

    #[test]
    fn test_fs_track_views_validate_invalid_metadata_returns_diagnostic() {
        let root = tempdir().expect("temporary root is created");
        let track_dir = root.path().join("track/items/bad-track");
        fs::create_dir_all(&track_dir).expect("track directory exists");
        fs::write(track_dir.join("metadata.json"), "{").expect("invalid metadata is written");
        let workspace =
            TrackWorkspaceRoot::try_new(root.path().to_path_buf()).expect("workspace is valid");

        let error = FsTrackViewsAdapter::new()
            .validate(&workspace)
            .expect_err("invalid metadata fails validation");

        assert!(error.as_str().contains("invalid"));
    }

    fn init_git_repo_on_branch(path: &Path, branch: &str) {
        let run = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(path)
                .env("GIT_AUTHOR_NAME", "test")
                .env("GIT_AUTHOR_EMAIL", "test@example.com")
                .env("GIT_COMMITTER_NAME", "test")
                .env("GIT_COMMITTER_EMAIL", "test@example.com")
                .status()
                .expect("git command runs");
            assert!(status.success(), "git command succeeds: {args:?}");
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "test"]);
        run(&["commit", "--allow-empty", "-m", "init", "--no-gpg-sign"]);
        run(&["checkout", "-B", branch]);
    }

    fn git_stdout(path: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .expect("git command runs");
        assert!(output.status.success(), "git command succeeds: {args:?}");
        String::from_utf8(output.stdout).expect("git output is UTF-8").trim().to_owned()
    }

    fn run_commit_hash_case_in_child(root: &Path, case: &str) {
        let executable = std::env::current_exe().expect("test executable is available");
        let status = std::process::Command::new(executable)
            .arg("--exact")
            .arg(
                "track::lifecycle_adapter_tests::test_git_track_commit_hash_adapter_child_process_entrypoint",
            )
            .arg("--nocapture")
            .current_dir(root)
            .env(COMMIT_HASH_CHILD_CASE_ENV, case)
            .status()
            .expect("child test process starts");
        assert!(status.success(), "child commit-hash case must pass: {case}");
    }

    #[test]
    fn test_git_track_commit_hash_adapter_child_process_entrypoint() {
        let Ok(case) = std::env::var(COMMIT_HASH_CHILD_CASE_ENV) else {
            return;
        };

        match case.as_str() {
            "mismatched_branch" => {
                let track_id = TrackId::try_new("different-track").expect("track id is valid");
                let error = GitTrackCommitHashAdapter::new()
                    .persist_current_for_track(&track_id)
                    .expect_err("mismatched branch is rejected");

                assert!(error.to_string().contains("does not match track branch"));
            }
            "persist_current_head" => {
                let track_dir = Path::new("track/items/commit-track");
                fs::create_dir_all(track_dir).expect("track directory exists");
                let expected = git_stdout(Path::new("."), &["rev-parse", "HEAD"]);
                let track_id = TrackId::try_new("commit-track").expect("track id is valid");

                let persisted = GitTrackCommitHashAdapter::new()
                    .persist_current_for_track(&track_id)
                    .expect("matching branch persists the current HEAD");

                assert_eq!(persisted.as_ref(), expected);
                assert_eq!(
                    fs::read_to_string(track_dir.join(".commit_hash"))
                        .expect("commit hash is persisted"),
                    expected
                );
            }
            "persistence_io_failure" => {
                let track_dir = Path::new("track/items/io-track");
                fs::create_dir_all(track_dir).expect("track directory exists");
                fs::create_dir(track_dir.join(".commit_hash"))
                    .expect("commit hash directory exists");
                let track_id = TrackId::try_new("io-track").expect("track id is valid");
                let root = std::env::current_dir().expect("current directory is available");

                let error = GitTrackCommitHashAdapter::new()
                    .persist_current_for_track(&track_id)
                    .expect_err("a directory at .commit_hash must fail the persistence write");

                assert!(error.to_string().contains("commit hash persistence failed"));
                assert!(error.to_string().contains("I/O error"));
                assert!(!error.to_string().contains(root.to_string_lossy().as_ref()));
            }
            "symlinked_track_directory" => {
                #[cfg(unix)]
                {
                    let items_dir = Path::new("track/items");
                    let outside = tempfile::tempdir().expect("outside directory is created");
                    fs::create_dir_all(outside.path()).expect("outside directory exists");
                    std::os::unix::fs::symlink(outside.path(), items_dir.join("linked-track"))
                        .expect("track directory symlink is created");
                    let track_id = TrackId::try_new("linked-track").expect("track id is valid");

                    let error = GitTrackCommitHashAdapter::new()
                        .persist_current_for_track(&track_id)
                        .expect_err("symlinked track directories must be rejected");

                    assert!(error.to_string().contains("track directory"));
                    assert!(error.to_string().contains("symlink"));
                }
                #[cfg(not(unix))]
                panic!("symlink test is only supported on Unix");
            }
            "symlinked_commit_hash" => {
                #[cfg(unix)]
                {
                    let track_dir = Path::new("track/items/symlink-track");
                    fs::create_dir_all(track_dir).expect("track directory exists");
                    let target = Path::new("target-commit-hash");
                    fs::write(target, "0".repeat(40)).expect("symlink target is written");
                    std::os::unix::fs::symlink(target, track_dir.join(".commit_hash"))
                        .expect("commit hash symlink is created");
                    let track_id = TrackId::try_new("symlink-track").expect("track id is valid");

                    let error = GitTrackCommitHashAdapter::new()
                        .persist_current_for_track(&track_id)
                        .expect_err("symlinked commit hash must be rejected");

                    assert!(error.to_string().contains("symlink detected"));
                }
                #[cfg(not(unix))]
                panic!("symlink test is only supported on Unix");
            }
            other => panic!("unknown commit-hash child case: {other}"),
        }
    }

    #[test]
    fn test_git_track_commit_hash_adapter_rejects_mismatched_branch() {
        let root = tempdir().expect("temporary root is created");
        init_git_repo_on_branch(root.path(), "main");
        run_commit_hash_case_in_child(root.path(), "mismatched_branch");
    }

    #[test]
    fn test_git_track_commit_hash_adapter_persists_current_head_for_matching_branch() {
        let root = tempdir().expect("temporary root is created");
        init_git_repo_on_branch(root.path(), "track/commit-track");
        let track_dir = root.path().join("track/items/commit-track");
        let expected = git_stdout(root.path(), &["rev-parse", "HEAD"]);
        run_commit_hash_case_in_child(root.path(), "persist_current_head");
        assert_eq!(
            fs::read_to_string(track_dir.join(".commit_hash")).expect("commit hash is persisted"),
            expected
        );
    }

    #[test]
    fn test_git_track_commit_hash_adapter_maps_persistence_io_failure() {
        let root = tempdir().expect("temporary root is created");
        init_git_repo_on_branch(root.path(), "track/io-track");
        run_commit_hash_case_in_child(root.path(), "persistence_io_failure");
    }

    #[cfg(unix)]
    #[test]
    fn test_git_track_commit_hash_adapter_rejects_symlinked_track_directory() {
        let root = tempdir().expect("temporary root is created");
        init_git_repo_on_branch(root.path(), "track/linked-track");
        fs::create_dir_all(root.path().join("track/items")).expect("items directory exists");
        run_commit_hash_case_in_child(root.path(), "symlinked_track_directory");
    }

    #[cfg(unix)]
    #[test]
    fn test_git_track_commit_hash_adapter_rejects_symlinked_commit_hash() {
        let root = tempdir().expect("temporary root is created");
        init_git_repo_on_branch(root.path(), "track/symlink-track");
        run_commit_hash_case_in_child(root.path(), "symlinked_commit_hash");
    }

    #[test]
    fn test_commit_hash_diagnostic_hides_environment_path() {
        let path = Path::new("/private/workspace/track/items/example/.commit_hash");
        let error = domain::review_v2::CommitHashError::Io {
            path: path.display().to_string(),
            detail: "permission denied".to_owned(),
        };

        let diagnostic = super::commit_hash_diagnostic(error, path);

        assert!(diagnostic.as_str().contains("permission denied"));
        assert!(diagnostic.as_str().contains("<path>"));
        assert!(!diagnostic.as_str().contains("/private/workspace"));
    }
}
