//! Shared contracts for the Track lifecycle command contexts.
//!
//! The command-specific interactors are introduced by the lifecycle slices. This
//! module owns the values and secondary ports that are shared by those slices so
//! the composition root can wire them without exposing raw paths, ids, or CLI
//! presentation values at the usecase boundary.

use std::path::{Path, PathBuf};

use domain::{CommitHash, TrackId};

use crate::git_workflow::DiagnosticText;
pub mod resolution_compat;
pub mod tddd;
pub mod track_add_task;
pub mod track_clear_override;
pub mod track_init;
pub mod track_next_task;
pub mod track_set_override;
pub mod track_task_counts;
pub mod track_transition;

pub use tddd::{
    TrackCatalogueEntryCount, TrackCatalogueImplLayerResult, TrackCataloguePath, TrackLayerFilter,
    TrackLayerSelection, TrackLayerSignalResult, TrackRenderedLayerCount, TrackSourceWorkspace,
    TrackSpecAnchorSelection, TrackWrittenFileCount,
};
pub use track_add_task::TrackTaskAddPort;
pub use track_clear_override::TrackOverrideClearPort;
pub use track_next_task::TrackNextTaskQueryPort;
pub use track_set_override::TrackOverrideSetPort;
pub use track_task_counts::TrackTaskCountsQueryPort;
pub use track_transition::TrackTaskTransitionPort;
/// The directory containing a track's item directories.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackItemsDirectory(PathBuf);

impl TrackItemsDirectory {
    /// Validates and wraps an items directory path.
    ///
    /// The path may be relative or absolute, but it must name the conventional
    /// `track/items` directory and may not contain parent traversal.
    pub fn try_new(value: PathBuf) -> Result<Self, DiagnosticText> {
        validate_non_traversing_path(&value, "track items directory")?;
        let is_items_dir = value.file_name().and_then(|name| name.to_str()) == Some("items")
            && value.parent().and_then(Path::file_name).and_then(|name| name.to_str())
                == Some("track");
        if !is_items_dir {
            return Err(DiagnosticText::new(format!(
                "track items directory must end in 'track/items': {}",
                value.display()
            )));
        }
        Ok(Self(value))
    }

    /// Returns the wrapped path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// The root directory of the workspace containing `track/`.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackWorkspaceRoot(PathBuf);

impl TrackWorkspaceRoot {
    /// Validates and wraps a workspace root path.
    pub fn try_new(value: PathBuf) -> Result<Self, DiagnosticText> {
        validate_non_traversing_path(&value, "track workspace root")?;
        Ok(Self(value))
    }

    /// Returns the wrapped path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// A validated path to a track directory.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackDirectoryPath(PathBuf);

impl TrackDirectoryPath {
    /// Validates and wraps a track directory path.
    pub fn try_new(value: PathBuf) -> Result<Self, DiagnosticText> {
        validate_non_traversing_path(&value, "track directory")?;
        Ok(Self(value))
    }

    /// Returns the wrapped path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// A path returned by the rendered-view adapter.
#[derive(Debug, PartialEq, Eq)]
pub struct RenderedViewPath(PathBuf);

impl RenderedViewPath {
    /// Wraps a rendered-view path returned by infrastructure.
    #[must_use]
    pub fn new(value: PathBuf) -> Self {
        Self(value)
    }

    /// Returns the wrapped path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// A validated transport value for a process exit code.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ProcessExitCode(u8);

impl ProcessExitCode {
    /// Wraps an exit code.
    #[must_use]
    pub fn new(value: u8) -> Self {
        Self(value)
    }

    /// Returns the numeric exit code.
    #[must_use]
    pub fn value(&self) -> u8 {
        self.0
    }
}

/// A task count used by presentation-free result DTOs.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TaskCount(u64);

impl TaskCount {
    /// Wraps a task count.
    #[must_use]
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric count.
    #[must_use]
    pub fn value(&self) -> u64 {
        self.0
    }
}

/// A validated CLI track-id input.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackLifecycleIdInput(TrackId);

impl TrackLifecycleIdInput {
    /// Validates and wraps a track id supplied by a primary adapter.
    pub fn try_new(value: String) -> Result<Self, DiagnosticText> {
        TrackId::try_new(value)
            .map(Self)
            .map_err(|error| DiagnosticText::new(format!("invalid track id: {error}")))
    }

    /// Returns the validated id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    /// Converts the validated input into its domain id.
    #[must_use]
    fn into_track_id(self) -> TrackId {
        self.0
    }
}

/// A track selection supplied to a lifecycle use case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackSelection {
    /// Resolve the active track from the current branch.
    Active,
    /// Use the already validated explicit track id.
    Explicit(TrackId),
}

impl TrackSelection {
    /// Converts optional CLI input into a validated selection.
    #[must_use]
    pub fn from_input(value: Option<TrackLifecycleIdInput>) -> Self {
        match value {
            Some(input) => Self::Explicit(input.into_track_id()),
            None => Self::Active,
        }
    }
}

/// A validated task transition requested by the transition use case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackTaskTransition {
    /// Reset a task to todo.
    Todo,
    /// Start or reopen a task.
    InProgress,
    /// Complete a task, optionally recording a commit hash.
    Done { commit_hash: Option<CommitHash> },
    /// Skip a task.
    Skipped,
}

impl TrackTaskTransition {
    /// Validates a target-status token and its optional commit hash.
    pub fn try_new(
        target_status: String,
        commit_hash: Option<String>,
    ) -> Result<Self, DiagnosticText> {
        match target_status.as_str() {
            "todo" if commit_hash.is_none() => Ok(Self::Todo),
            "in_progress" if commit_hash.is_none() => Ok(Self::InProgress),
            "skipped" if commit_hash.is_none() => Ok(Self::Skipped),
            "done" => commit_hash
                .map(|value| {
                    CommitHash::try_new(value).map_err(|error| {
                        DiagnosticText::new(format!("invalid commit hash: {error}"))
                    })
                })
                .transpose()
                .map(|hash| Self::Done { commit_hash: hash }),
            "todo" | "in_progress" | "skipped" => {
                Err(DiagnosticText::new("commit hash is only valid for the done transition"))
            }
            other => Err(DiagnosticText::new(format!("unsupported target status: {other}"))),
        }
    }
}

/// The scope used by the rendered-view secondary port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackViewsScope {
    /// Refresh only the registry view.
    RegistryOnly,
    /// Refresh the registry and one track's rendered view.
    Track(TrackId),
}

/// The presentation-free result of view synchronization.
#[derive(Debug, PartialEq, Eq)]
pub enum TrackViewSyncOutcome {
    /// All requested views were synchronized.
    Synchronized(Vec<RenderedViewPath>),
    /// The track operation succeeded but view synchronization emitted a warning.
    Warning { rendered_views: Vec<RenderedViewPath>, diagnostic: DiagnosticText },
}

/// Secondary port for track branch-strategy configuration and snapshots.
pub trait TrackBranchStrategyPort: Send + Sync {
    /// Loads the global strategy used before a track metadata snapshot exists.
    fn global_for_items(
        &self,
        items_dir: &TrackItemsDirectory,
    ) -> Result<domain::BranchStrategySnapshot, DiagnosticText>;

    /// Loads the captured strategy for an existing track.
    fn snapshot_for_track(
        &self,
        workspace_root: &TrackWorkspaceRoot,
        track_id: &TrackId,
    ) -> Result<domain::BranchStrategySnapshot, DiagnosticText>;
}

/// Secondary port for persisting a track's current commit hash.
pub trait TrackCommitHashPort: Send + Sync {
    /// Resolves and persists the current HEAD for `track_id`.
    fn persist_current_for_track(&self, track_id: &TrackId) -> Result<CommitHash, DiagnosticText>;
}

/// Secondary port for track metadata storage.
pub trait TrackMetadataPort: Send + Sync {
    /// Persists metadata in `items_dir`.
    fn save(
        &self,
        items_dir: &TrackItemsDirectory,
        metadata: domain::TrackMetadata,
    ) -> Result<(), DiagnosticText>;

    /// Finds metadata by id in `items_dir`.
    fn find(
        &self,
        items_dir: &TrackItemsDirectory,
        track_id: &TrackId,
    ) -> Result<Option<domain::TrackMetadata>, DiagnosticText>;
}

/// Secondary port for resolving track selections and view scopes.
pub trait TrackSelectionPort: Send + Sync {
    /// Resolves a required selection against an items directory.
    fn resolve_required(
        &self,
        items_dir: &TrackItemsDirectory,
        selection: &TrackSelection,
    ) -> Result<TrackId, DiagnosticText>;

    /// Resolves the active track from a workspace root.
    fn resolve_active(
        &self,
        workspace_root: &TrackWorkspaceRoot,
    ) -> Result<TrackId, DiagnosticText>;

    /// Resolves a selection into the view adapter's scope.
    fn resolve_views_scope(
        &self,
        workspace_root: &TrackWorkspaceRoot,
        selection: &TrackSelection,
    ) -> Result<TrackViewsScope, DiagnosticText>;
}

/// Secondary port for rendered-view validation and synchronization.
pub trait TrackViewsPort: Send + Sync {
    /// Validates rendered views below a workspace root.
    fn validate(&self, workspace_root: &TrackWorkspaceRoot) -> Result<(), DiagnosticText>;

    /// Synchronizes the selected rendered views.
    fn sync(
        &self,
        workspace_root: &TrackWorkspaceRoot,
        scope: &TrackViewsScope,
    ) -> Result<Vec<RenderedViewPath>, DiagnosticText>;
}

fn validate_non_traversing_path(value: &Path, label: &str) -> Result<(), DiagnosticText> {
    if value.as_os_str().is_empty() {
        return Err(DiagnosticText::new(format!("{label} must not be empty")));
    }
    if value.components().any(|component| component == std::path::Component::ParentDir) {
        return Err(DiagnosticText::new(format!(
            "{label} must not contain parent traversal: {}",
            value.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_track_items_directory_accepts_canonical_items_path() {
        let value = TrackItemsDirectory::try_new(PathBuf::from("track/items")).unwrap();
        assert_eq!(value.as_path(), Path::new("track/items"));
    }

    #[test]
    fn test_track_items_directory_rejects_noncanonical_path() {
        let error = TrackItemsDirectory::try_new(PathBuf::from("items")).unwrap_err();
        assert!(error.to_string().contains("track/items"));
    }

    #[test]
    fn test_track_id_input_rejects_invalid_id() {
        let error = TrackLifecycleIdInput::try_new("../escape".to_owned()).unwrap_err();
        assert!(error.to_string().contains("invalid track id"));
    }

    #[test]
    fn test_track_selection_from_input_preserves_valid_explicit_id() {
        let input = TrackLifecycleIdInput::try_new("feature-2026".to_owned()).unwrap();
        assert_eq!(
            TrackSelection::from_input(Some(input)),
            TrackSelection::Explicit(TrackId::try_new("feature-2026").unwrap())
        );
    }

    #[test]
    fn test_track_task_transition_rejects_hash_on_non_done_status() {
        let error =
            TrackTaskTransition::try_new("in_progress".to_owned(), Some("abc1234".to_owned()))
                .unwrap_err();
        assert!(error.to_string().contains("only valid for the done"));
    }

    #[test]
    fn test_track_task_transition_accepts_done_with_commit_hash() {
        let transition =
            TrackTaskTransition::try_new("done".to_owned(), Some("abc1234".to_owned())).unwrap();
        assert!(matches!(transition, TrackTaskTransition::Done { commit_hash: Some(_) }));
    }
}
