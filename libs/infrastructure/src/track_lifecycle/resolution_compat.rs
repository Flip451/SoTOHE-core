//! System adapter for the compatibility track-resolution boundary.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::TrackId;
use usecase::track_lifecycle::resolution_compat::{
    TrackResolutionCommand, TrackResolutionCompatError, TrackResolutionPort, TrackResolutionResult,
};
use usecase::track_lifecycle::{TrackItemsDirectory, TrackSelection, TrackWorkspaceRoot};
use usecase::track_resolution::{
    ActiveTrackResolveError, ActiveTrackResolveInteractor, ActiveTrackResolveService,
    BranchReadError, BranchReaderPort, TrackResolutionError,
};

use crate::git_cli::SystemGitRepo;

const MAX_CURRENT_BRANCH_OUTPUT_BYTES: usize = 4096;

/// System secondary adapter for compatibility track resolution.
pub struct SystemTrackResolutionAdapter;

/// Branch reader for resolution guards that keeps repository selection isolated
/// for the branch read as well as for repository discovery.
struct IsolatedBranchReader {
    repo_root: PathBuf,
}

impl IsolatedBranchReader {
    fn new(repo: &SystemGitRepo) -> Self {
        Self { repo_root: repo.root().to_path_buf() }
    }
}

impl BranchReaderPort for IsolatedBranchReader {
    fn current_branch(&self) -> Result<Option<String>, BranchReadError> {
        let output = crate::git_cli::isolated_bounded_git_output(
            &self.repo_root,
            &["rev-parse", "--abbrev-ref", "HEAD"],
            MAX_CURRENT_BRANCH_OUTPUT_BYTES,
        )
        .map_err(|error| BranchReadError::ReadFailed(error.to_string()))?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_owned()))
    }
}

impl TrackResolutionPort for SystemTrackResolutionAdapter {
    fn execute(
        &self,
        command: TrackResolutionCommand,
    ) -> Result<TrackResolutionResult, TrackResolutionCompatError> {
        match command {
            TrackResolutionCommand::ReadFromItems { track, items_dir } => {
                if let TrackSelection::Explicit(track_id) = track {
                    return Ok(TrackResolutionResult::Resolved(track_id));
                }
                let workspace_root = workspace_root_from_items(&items_dir)?;
                resolve_from_root(workspace_root, None, false, false)
            }
            TrackResolutionCommand::ReadFromRoot { track, workspace_root } => {
                if let TrackSelection::Explicit(track_id) = track {
                    return Ok(TrackResolutionResult::Resolved(track_id));
                }
                resolve_from_root(workspace_root, None, false, false)
            }
            TrackResolutionCommand::WriteFromItems { track, items_dir } => {
                let workspace_root = workspace_root_from_items(&items_dir)?;
                let explicit_write = matches!(&track, TrackSelection::Explicit(_));
                resolve_from_root(workspace_root, explicit_track_id(track), true, explicit_write)
            }
            TrackResolutionCommand::WriteFromRoot { track, workspace_root } => {
                let explicit_write = matches!(&track, TrackSelection::Explicit(_));
                resolve_from_root(workspace_root, explicit_track_id(track), true, explicit_write)
            }
            TrackResolutionCommand::DetectActive { workspace_root } => {
                detect_active(workspace_root)
            }
        }
    }
}

fn explicit_track_id(track: TrackSelection) -> Option<String> {
    match track {
        TrackSelection::Active => None,
        TrackSelection::Explicit(track_id) => Some(track_id.to_string()),
    }
}

fn workspace_root_from_items(
    items_dir: &TrackItemsDirectory,
) -> Result<TrackWorkspaceRoot, TrackResolutionCompatError> {
    let path = items_dir.as_path();
    reject_symlinked_path(path, "track items directory")
        .map_err(|error| unavailable(error.to_string()))?;
    let root = path
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."));
    TrackWorkspaceRoot::try_new(root).map_err(|error| unavailable(error.to_string()))
}

fn reject_symlinked_path(path: &Path, label: &str) -> Result<(), std::io::Error> {
    let absolute_path =
        if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir()?.join(path) };
    crate::track::symlink_guard::reject_symlinks_up_to_root(&absolute_path).map_err(|error| {
        std::io::Error::new(
            error.kind(),
            format!("refusing to use {label} '{}': {error}", path.display()),
        )
    })
}

fn discover_repository(
    workspace_root: &TrackWorkspaceRoot,
) -> Result<SystemGitRepo, std::io::Error> {
    let path = workspace_root.as_path();
    reject_symlinked_path(path, "workspace root")?;
    let canonical_root = path.canonicalize()?;
    SystemGitRepo::discover_from_isolated(&canonical_root)
        .map_err(|error| std::io::Error::other(error.to_string()))
}

fn resolve_from_root(
    workspace_root: TrackWorkspaceRoot,
    explicit_track_id: Option<String>,
    write_mode: bool,
    explicit_write: bool,
) -> Result<TrackResolutionResult, TrackResolutionCompatError> {
    let repo = discover_repository(&workspace_root).map_err(|error| {
        let write_suffix =
            if explicit_write { " (write operations require a git repository)" } else { "" };
        unavailable(format!(
            "cannot discover git repository from {}: {error}{write_suffix}",
            workspace_root.as_path().display()
        ))
    })?;
    let branch_reader = IsolatedBranchReader::new(&repo);
    let interactor = ActiveTrackResolveInteractor::new(Arc::new(branch_reader));
    let resolved = if write_mode {
        interactor
            .resolve_for_write(explicit_track_id)
            .map_err(|error| unavailable(error.to_string()))?
    } else {
        interactor.resolve_for_read(None).map_err(|error| unavailable(error.to_string()))?
    };
    TrackId::try_new(resolved)
        .map(TrackResolutionResult::Resolved)
        .map_err(|error| unavailable(format!("invalid track id: {error}")))
}

fn detect_active(
    workspace_root: TrackWorkspaceRoot,
) -> Result<TrackResolutionResult, TrackResolutionCompatError> {
    let repo = discover_repository(&workspace_root).map_err(|error| {
        unavailable(format!(
            "cannot discover git repository from {}: {error}",
            workspace_root.as_path().display()
        ))
    })?;
    let branch_reader = IsolatedBranchReader::new(&repo);
    let interactor = ActiveTrackResolveInteractor::new(Arc::new(branch_reader));
    match interactor.resolve_active_track() {
        Ok(track_id) => TrackId::try_new(track_id)
            .map(TrackResolutionResult::Resolved)
            .map_err(|error| unavailable(format!("invalid track id: {error}"))),
        Err(ActiveTrackResolveError::BranchRead(error)) => Err(unavailable(error.to_string())),
        Err(ActiveTrackResolveError::Resolution(error)) if is_inactive(&error) => {
            Ok(TrackResolutionResult::Inactive)
        }
        Err(error) => Err(unavailable(error.to_string())),
    }
}

fn is_inactive(error: &TrackResolutionError) -> bool {
    matches!(
        error,
        TrackResolutionError::DetachedHead
            | TrackResolutionError::NoBranch
            | TrackResolutionError::NotTrackBranch(_)
    )
}

fn unavailable(message: impl Into<String>) -> TrackResolutionCompatError {
    TrackResolutionCompatError::Unavailable(usecase::git_workflow::DiagnosticText::new(message))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::process::Command;

    fn run_git(root: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .args(arguments)
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .status()
            .unwrap();
        assert!(status.success(), "git command failed: git {arguments:?}");
    }

    fn repository(branch: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        run_git(root.path(), &["init", "-q"]);
        run_git(root.path(), &["checkout", "-q", "-B", branch]);
        run_git(root.path(), &["commit", "--allow-empty", "-m", "init", "--no-gpg-sign"]);
        root
    }

    fn items(root: &Path) -> TrackItemsDirectory {
        TrackItemsDirectory::try_new(root.join("track/items")).unwrap()
    }

    #[test]
    fn test_system_track_resolution_adapter_read_resolves_track_branch() {
        let root = repository("track/compat-track");
        let result = SystemTrackResolutionAdapter.execute(TrackResolutionCommand::ReadFromItems {
            track: TrackSelection::Active,
            items_dir: items(root.path()),
        });

        assert_eq!(
            result,
            Ok(TrackResolutionResult::Resolved(TrackId::try_new("compat-track").unwrap()))
        );
    }

    #[test]
    fn test_track_resolution_port_execute_preserves_read_resolution_for_active_branch() {
        let root = repository("track/port-contract-track");
        let workspace_root = TrackWorkspaceRoot::try_new(root.path().to_path_buf()).unwrap();
        let port: &dyn TrackResolutionPort = &SystemTrackResolutionAdapter;

        let result = port.execute(TrackResolutionCommand::ReadFromRoot {
            track: TrackSelection::Active,
            workspace_root,
        });

        assert_eq!(
            result,
            Ok(TrackResolutionResult::Resolved(TrackId::try_new("port-contract-track").unwrap()))
        );
    }

    #[test]
    fn test_system_track_resolution_adapter_detect_active_on_main_returns_inactive() {
        let root = repository("main");
        let workspace_root = TrackWorkspaceRoot::try_new(root.path().to_path_buf()).unwrap();

        let result = SystemTrackResolutionAdapter
            .execute(TrackResolutionCommand::DetectActive { workspace_root });

        assert_eq!(result, Ok(TrackResolutionResult::Inactive));
    }

    #[test]
    fn test_system_track_resolution_adapter_detect_active_invalid_track_branch_returns_unavailable()
    {
        let root = repository("track/Bad-branch");
        let workspace_root = TrackWorkspaceRoot::try_new(root.path().to_path_buf()).unwrap();

        let result = SystemTrackResolutionAdapter
            .execute(TrackResolutionCommand::DetectActive { workspace_root });

        let Err(TrackResolutionCompatError::Unavailable(message)) = result else {
            panic!("expected malformed track branch diagnostic");
        };
        assert!(message.to_string().contains("invalid track id from branch 'Bad-branch'"));
    }

    #[test]
    fn test_system_track_resolution_adapter_write_mismatch_returns_branch_diagnostic() {
        let root = repository("track/other-track");
        let result = SystemTrackResolutionAdapter.execute(TrackResolutionCommand::WriteFromItems {
            track: TrackSelection::Explicit(TrackId::try_new("requested-track").unwrap()),
            items_dir: items(root.path()),
        });

        let Err(TrackResolutionCompatError::Unavailable(message)) = result else {
            panic!("expected write mismatch diagnostic");
        };
        assert!(message.to_string().contains("WRITE guard mismatch"));
        assert!(message.to_string().contains("requested-track"));
    }

    #[test]
    fn test_system_track_resolution_adapter_write_guard_ignores_ambient_repository_selection() {
        let requested = repository("track/requested-track");
        let elsewhere = repository("track/elsewhere-track");
        let workspace_root = TrackWorkspaceRoot::try_new(requested.path().to_path_buf()).unwrap();
        let command = TrackResolutionCommand::WriteFromRoot {
            track: TrackSelection::Explicit(TrackId::try_new("requested-track").unwrap()),
            workspace_root,
        };

        let result =
            temp_env::with_var("GIT_DIR", Some(elsewhere.path().join(".git").as_os_str()), || {
                SystemTrackResolutionAdapter.execute(command)
            });

        assert_eq!(
            result,
            Ok(TrackResolutionResult::Resolved(TrackId::try_new("requested-track").unwrap()))
        );
    }

    #[test]
    fn test_system_track_resolution_adapter_missing_repository_returns_diagnostic() {
        let root = tempfile::tempdir().unwrap();
        let workspace_root = TrackWorkspaceRoot::try_new(root.path().to_path_buf()).unwrap();

        let result = SystemTrackResolutionAdapter.execute(TrackResolutionCommand::ReadFromRoot {
            track: TrackSelection::Active,
            workspace_root,
        });

        let Err(TrackResolutionCompatError::Unavailable(message)) = result else {
            panic!("expected repository discovery diagnostic");
        };
        assert!(message.to_string().contains("cannot discover git repository"));
    }

    #[cfg(unix)]
    #[test]
    fn test_system_track_resolution_adapter_write_from_root_rejects_symlinked_workspace_root() {
        let repository = repository("track/safe-track");
        let holder = tempfile::tempdir().unwrap();
        let linked_root = holder.path().join("workspace");
        std::os::unix::fs::symlink(repository.path(), &linked_root).unwrap();
        let workspace_root = TrackWorkspaceRoot::try_new(linked_root).unwrap();

        let result = SystemTrackResolutionAdapter.execute(TrackResolutionCommand::WriteFromRoot {
            track: TrackSelection::Explicit(TrackId::try_new("safe-track").unwrap()),
            workspace_root,
        });

        let Err(TrackResolutionCompatError::Unavailable(message)) = result else {
            panic!("expected symlinked workspace root to be rejected");
        };
        assert!(message.to_string().contains("symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn test_system_track_resolution_adapter_write_from_items_rejects_symlinked_items_path() {
        let repository = repository("track/safe-track");
        let real_track = repository.path().join("track");
        std::fs::create_dir_all(real_track.join("items")).unwrap();
        let holder = tempfile::tempdir().unwrap();
        let linked_track = holder.path().join("track");
        std::os::unix::fs::symlink(real_track, &linked_track).unwrap();
        let items_dir = TrackItemsDirectory::try_new(linked_track.join("items")).unwrap();

        let result = SystemTrackResolutionAdapter.execute(TrackResolutionCommand::WriteFromItems {
            track: TrackSelection::Explicit(TrackId::try_new("safe-track").unwrap()),
            items_dir,
        });

        let Err(TrackResolutionCompatError::Unavailable(message)) = result else {
            panic!("expected symlinked items path to be rejected");
        };
        assert!(message.to_string().contains("symlink"));
    }
}
