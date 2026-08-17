//! Application boundary for archiving a completed track.

use std::path::PathBuf;
use std::sync::Arc;

use domain::TrackId;

use crate::git_workflow::{
    DiagnosticText, GitPrimitivePort, TrackArchiveFsPort, TrackGitInteractor,
};

use super::{TrackDirectoryPath, TrackItemsDirectory, TrackLifecycleIdInput};

/// Typed input for archiving a track directory.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackArchiveCommand {
    /// Directory containing the track to archive.
    pub items_dir: TrackItemsDirectory,
    /// Validated track identity.
    pub track_id: TrackId,
}

impl TrackArchiveCommand {
    /// Creates an archive command from validated primary-adapter inputs.
    #[must_use]
    pub fn new(items_dir: TrackItemsDirectory, track_id: TrackLifecycleIdInput) -> Self {
        Self { items_dir, track_id: track_id.into_track_id() }
    }
}

/// Presentation-free result of track archiving.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackArchiveResult {
    /// Identity of the archived track.
    pub track_id: TrackId,
    /// Source directory moved by the archive operation.
    pub source: TrackDirectoryPath,
    /// Destination directory created by the archive operation.
    pub destination: TrackDirectoryPath,
}

/// Error returned by the track-archive boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackArchiveError {
    /// A repository, git, filesystem, or result-construction operation failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackArchiveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackArchiveError {}

/// Application service for archiving a completed track.
pub trait TrackArchiveService: Send + Sync {
    /// Moves the track directory and its optional gitignored logs directory.
    fn execute(
        &self,
        command: TrackArchiveCommand,
    ) -> Result<TrackArchiveResult, TrackArchiveError>;
}

/// Interactor for the track-archive command context.
pub struct TrackArchiveInteractor {
    git: Arc<dyn GitPrimitivePort>,
    fs: Arc<dyn TrackArchiveFsPort>,
}

impl TrackArchiveInteractor {
    /// Creates an interactor from the existing git and archive-filesystem ports.
    #[must_use]
    pub fn new(git: Arc<dyn GitPrimitivePort>, fs: Arc<dyn TrackArchiveFsPort>) -> Self {
        Self { git, fs }
    }
}

impl TrackArchiveService for TrackArchiveInteractor {
    fn execute(
        &self,
        command: TrackArchiveCommand,
    ) -> Result<TrackArchiveResult, TrackArchiveError> {
        let repo_anchor = project_root_hint(&command.items_dir)?;
        let project_root = self
            .git
            .resolve_repo_root(Some(&repo_anchor))
            .map_err(|error| execution_failed(error.to_string()))?;
        let source = TrackDirectoryPath::try_new(
            project_root.join("track").join("items").join(command.track_id.as_ref()),
        )
        .map_err(|error| execution_failed(error.to_string()))?;
        let destination = TrackDirectoryPath::try_new(
            project_root.join("track").join("archive").join(command.track_id.as_ref()),
        )
        .map_err(|error| execution_failed(error.to_string()))?;

        TrackGitInteractor::new(Arc::clone(&self.git), Arc::clone(&self.fs))
            .archive_track(&project_root, &command.track_id)
            .map_err(|error| execution_failed(error.to_string()))?;

        Ok(TrackArchiveResult { track_id: command.track_id, source, destination })
    }
}

fn execution_failed(error: impl Into<String>) -> TrackArchiveError {
    TrackArchiveError::ExecutionFailed(DiagnosticText::new(error))
}

fn project_root_hint(items_dir: &TrackItemsDirectory) -> Result<PathBuf, TrackArchiveError> {
    let track_dir = items_dir
        .as_path()
        .parent()
        .ok_or_else(|| execution_failed("track items directory has no track parent"))?;
    let root = track_dir
        .parent()
        .ok_or_else(|| execution_failed("track items directory has no workspace root"))?;
    Ok(if root.as_os_str().is_empty() { PathBuf::from(".") } else { root.to_path_buf() })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};

    use mockall::mock;

    use super::*;
    use crate::git_workflow::{ExplicitTrackBranch, GitWorkflowError, TrackBranchClaim};

    mock! {
        Git {}
        impl GitPrimitivePort for Git {
            fn current_branch<'a>(
                &self,
                project_root: Option<&'a Path>,
            ) -> Result<Option<String>, GitWorkflowError>;
            fn sync_current_branch<'a>(
                &self,
                project_root: Option<&'a Path>,
            ) -> Result<(), GitWorkflowError>;
            fn switch_branch<'a>(
                &self,
                project_root: Option<&'a Path>,
                branch: &str,
            ) -> Result<(), GitWorkflowError>;
            fn create_branch<'a>(
                &self,
                project_root: Option<&'a Path>,
                new_branch: &str,
                base_branch: &str,
            ) -> Result<(), GitWorkflowError>;
            fn branch_exists<'a>(
                &self,
                project_root: Option<&'a Path>,
                branch: &str,
            ) -> Result<bool, GitWorkflowError>;
            fn move_path<'a>(
                &self,
                project_root: Option<&'a Path>,
                src: &Path,
                dst: &Path,
            ) -> Result<(), GitWorkflowError>;
            fn fetch_branch<'a>(
                &self,
                project_root: Option<&'a Path>,
                branch: &str,
            ) -> Result<(), GitWorkflowError>;
            fn show_file_at_ref<'a>(
                &self,
                project_root: Option<&'a Path>,
                git_ref: &str,
                path: &Path,
            ) -> Result<String, GitWorkflowError>;
            fn resolve_commit<'a>(
                &self,
                project_root: Option<&'a Path>,
                rev: &str,
            ) -> Result<Option<domain::CommitHash>, GitWorkflowError>;
            fn resolve_repo_root<'a>(
                &self,
                project_root: Option<&'a Path>,
            ) -> Result<PathBuf, GitWorkflowError>;
            fn stage_all<'a>(&self, project_root: Option<&'a Path>)
                -> Result<(), GitWorkflowError>;
            fn stage_from_file<'a>(
                &self,
                project_root: Option<&'a Path>,
                path: &Path,
                cleanup: bool,
            ) -> Result<(), GitWorkflowError>;
            fn commit_from_message_file<'a>(
                &self,
                project_root: Option<&'a Path>,
                path: &Path,
                cleanup: bool,
            ) -> Result<(), GitWorkflowError>;
            fn note_from_file<'a>(
                &self,
                project_root: Option<&'a Path>,
                path: &Path,
                cleanup: bool,
            ) -> Result<(), GitWorkflowError>;
            fn unstage<'a>(
                &self,
                project_root: Option<&'a Path>,
                paths: &[PathBuf],
            ) -> Result<(), GitWorkflowError>;
            fn read_explicit_track_branch<'a>(
                &self,
                project_root: Option<&'a Path>,
                track_dir: &Path,
            ) -> Result<ExplicitTrackBranch, GitWorkflowError>;
            fn collect_track_branch_claims<'a>(
                &self,
                project_root: Option<&'a Path>,
            ) -> Result<Vec<TrackBranchClaim>, GitWorkflowError>;
        }
    }

    mock! {
        Fs {}
        impl TrackArchiveFsPort for Fs {
            fn path_is_dir(&self, path: &Path) -> Result<bool, GitWorkflowError>;
            fn path_exists(&self, path: &Path) -> Result<bool, GitWorkflowError>;
            fn create_dir_all(&self, path: &Path) -> Result<(), GitWorkflowError>;
            fn rename_path(&self, src: &Path, dst: &Path) -> Result<(), GitWorkflowError>;
            fn list_dir_file_names(&self, path: &Path) -> Result<Vec<PathBuf>, GitWorkflowError>;
            fn remove_dir(&self, path: &Path) -> Result<(), GitWorkflowError>;
        }
    }

    fn command() -> TrackArchiveCommand {
        TrackArchiveCommand::new(
            TrackItemsDirectory::try_new(PathBuf::from("workspace/track/items"))
                .expect("items directory is valid"),
            TrackLifecycleIdInput::try_new("archive-track".to_owned()).expect("track id is valid"),
        )
    }

    fn configure_success_git(git: &mut MockGit) {
        git.expect_resolve_repo_root()
            .withf(|root| *root == Some(Path::new("workspace")))
            .returning(|_| Ok(PathBuf::from("/workspace")));
        git.expect_move_path().returning(|_, _, _| Ok(()));
    }

    #[test]
    fn test_track_archive_interactor_valid_command_returns_typed_archive_paths() {
        let mut git = MockGit::new();
        configure_success_git(&mut git);
        let mut fs = MockFs::new();
        fs.expect_path_is_dir()
            .returning(|path| Ok(path.ends_with("archive-track") && !path.ends_with("logs")));
        fs.expect_path_exists().returning(|_| Ok(false));
        fs.expect_create_dir_all().returning(|_| Ok(()));

        let interactor = TrackArchiveInteractor::new(Arc::new(git), Arc::new(fs));
        let result = interactor.execute(command()).expect("archive succeeds");

        assert_eq!(result.track_id.as_ref(), "archive-track");
        assert_eq!(result.source.as_path(), Path::new("/workspace/track/items/archive-track"));
        assert_eq!(
            result.destination.as_path(),
            Path::new("/workspace/track/archive/archive-track")
        );
    }

    #[test]
    fn test_track_archive_interactor_missing_source_maps_to_execution_error() {
        let mut git = MockGit::new();
        configure_success_git(&mut git);
        let mut fs = MockFs::new();
        fs.expect_path_is_dir().returning(|_| Ok(false));

        let interactor = TrackArchiveInteractor::new(Arc::new(git), Arc::new(fs));
        let error = interactor.execute(command()).expect_err("missing source must fail");

        assert!(error.to_string().contains("track directory not found"));
    }
}
