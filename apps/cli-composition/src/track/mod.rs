//! `track` command family — core composition-root wiring helpers.
mod branch_strategy;
pub mod composition_root;
mod resolution;
mod set_commit_hash;
mod tddd;
use crate::error::CompositionError;
pub use composition_root::TrackCompositionRoot;
use std::path::{Path, PathBuf};
use std::sync::Arc;
/// Resolves `<project-root>/track/items` → `<project-root>`.
pub(crate) fn resolve_project_root(items_dir: &Path) -> Result<PathBuf, CompositionError> {
    let items_name = items_dir.file_name().and_then(|n| n.to_str());
    let track_dir = items_dir.parent();
    let track_name = track_dir.and_then(Path::file_name).and_then(|n| n.to_str());
    let project_root = track_dir.and_then(Path::parent);
    match (items_name, track_name, project_root) {
        (Some("items"), Some("track"), Some(root)) => {
            if root.as_os_str().is_empty() {
                Ok(PathBuf::from("."))
            } else {
                Ok(root.to_path_buf())
            }
        }
        _ => Err(CompositionError::WiringFailed(format!(
            "--items-dir must point to '<project-root>/track/items'; got {}",
            items_dir.display()
        ))),
    }
}
pub(crate) fn build_branch_reader(
    project_root: &Path,
) -> Option<Arc<dyn usecase::track_resolution::BranchReaderPort>> {
    use infrastructure::git_cli::SystemGitRepo;
    use usecase::track_resolution::BranchReaderPort;
    match SystemGitRepo::discover_from(project_root) {
        Ok(repo) => Some(Arc::new(repo) as Arc<dyn BranchReaderPort>),
        Err(_) => None,
    }
}
/// Wires the task-operation interactor together with the admission
/// collaborators every transition is judged through and the verifier every
/// recorded commit hash is checked by.
pub(crate) fn build_task_operation_interactor(
    store: Arc<infrastructure::track::fs_store::FsTrackStore>,
    branch_reader: Option<Arc<dyn usecase::track_resolution::BranchReaderPort>>,
) -> usecase::task_ops::TaskOperationInteractor<infrastructure::track::fs_store::FsTrackStore> {
    usecase::task_ops::TaskOperationInteractor::new(
        store,
        branch_reader,
        Arc::new(infrastructure::batch_plan_reader::FsBatchPlanReader::new()),
        Arc::new(infrastructure::scope_diff_measure::GitScopeDiffMeasurer::new()),
        Arc::new(infrastructure::review_scope_config_reader::FsReviewScopeConfigReader::new()),
        Arc::new(infrastructure::commit_record_verifier::GitCommitRecordVerifier::new()),
    )
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};

    use cli_driver::track::TrackInput;

    use cli_driver::adr_baseline::TrackIdInput;
    use cli_driver::track_resolution::{
        TrackItemsDirectoryInput, TrackResolutionInput, TrackResolutionOutcome,
    };

    use super::resolve_project_root;
    use crate::review_v2::process_guards::{CwdGuard, GitRunner};

    fn change_to(path: &Path) -> CwdGuard {
        let guard = CwdGuard::save_current();
        std::env::set_current_dir(path).unwrap();
        guard
    }

    fn init_git_repo(root: &Path) {
        GitRunner::at(root).assert_success(&["init", "-q"]);
        GitRunner::at(root).assert_success(&["config", "user.email", "test@test.com"]);
        GitRunner::at(root).assert_success(&["config", "user.name", "Test"]);
        GitRunner::at(root).assert_success(&["checkout", "-B", "main"]);
    }

    /// `resolve_project_root` strips the trailing `track/items` from a relative path.
    ///
    /// For `"track/items"` the parent of `track` is empty, so the function returns
    /// `"."` (the current-working-directory anchor).  This is the key property used
    /// by `resolve_track_id_from_branch` and `resolve_track_id_or_branch_write` in
    /// `review_v2/mod.rs`: they call `resolve_project_root` first and then pass the
    /// result to `SystemGitRepo::discover_from`, which means a relative items_dir
    /// always discovers from `"."` (the CWD) rather than from `"track/items"` which
    /// may not exist as a filesystem path when the process is inside a subdirectory.
    #[test]
    fn resolve_project_root_returns_dot_for_relative_items_dir() {
        let root = resolve_project_root(Path::new("track/items")).unwrap();
        assert_eq!(root, std::path::Path::new("."));
    }

    /// `resolve_project_root` returns the absolute parent when given an absolute path.
    #[test]
    fn resolve_project_root_strips_track_items_from_absolute_path() {
        let root = resolve_project_root(Path::new("/some/project/track/items")).unwrap();
        assert_eq!(root, std::path::Path::new("/some/project"));
    }

    /// `resolve_project_root` returns an error when the path does not end in `track/items`.
    #[test]
    fn resolve_project_root_rejects_non_canonical_path() {
        let result = resolve_project_root(Path::new("wrong/path"));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("track/items"), "error should mention 'track/items': {msg}");
    }

    /// When git discovery fails (anchor path does not exist → `current_dir` fails)
    /// AND an explicit track id is supplied, `resolve_track_id_for_write` must
    /// return `Err` rather than the bare id.  This is the fail-closed branch-guard
    /// contract: WRITE operations may not proceed without branch proof.
    #[test]
    fn test_resolve_track_id_for_write_with_git_failure_and_explicit_id_returns_error() {
        // Use a path that satisfies resolve_project_root's structural check
        // (items_dir must end in "track/items") but points to a directory that
        // does not exist, so git discovery returns an error.
        let items_dir = Path::new("/tmp/sotp-test-no-git-repo/track/items");
        let items_dir = TrackItemsDirectoryInput::try_new(items_dir.to_path_buf()).unwrap();
        let track_id = Some("my-track-2026".parse::<TrackIdInput>().unwrap());
        let outcome = crate::TrackCompositionRoot::new()
            .track_resolution_driver()
            .resolve(TrackResolutionInput::WriteFromItems { track_id, items_dir });
        let result = match outcome {
            TrackResolutionOutcome::Failed(diagnostic) => Err(diagnostic.to_string()),
            other => Ok(other),
        };
        assert!(
            result.is_err(),
            "expected Err when git discovery fails with explicit track id, got Ok({:?})",
            result.ok()
        );
        let msg = result.unwrap_err();
        assert!(
            msg.contains("cannot discover git repository")
                || msg.contains("write operations require a git repository")
                || msg.contains("failed to run git")
                || msg.contains("No such file or directory")
                || msg.contains("rev-parse"),
            "expected error message to mention git failure, got: {msg:?}"
        );
    }

    #[test]
    fn test_track_init_missing_items_root_creates_metadata() {
        let root = tempfile::tempdir().unwrap();
        let items_dir = root.path().join("track").join("items");
        let config_dir = root.path().join(".harness").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("branch-strategy.json"),
            r#"{
  "base_branch": "main",
  "merge_target": "main",
  "merge_method": "merge"
}"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("architecture-rules.json"),
            include_str!("../../../../architecture-rules.json"),
        )
        .unwrap();

        assert!(!items_dir.exists(), "fixture must start without track/items");

        let outcome =
            crate::track::TrackCompositionRoot::new().track_driver().handle(TrackInput::Init {
                items_dir: items_dir.clone(),
                track_id: "new-track".to_owned(),
                description: "New Track".to_owned(),
            });

        assert_eq!(outcome.exit_code, 0);
        assert!(
            items_dir.join("new-track").join("metadata.json").is_file(),
            "track init must bootstrap track/items and write metadata"
        );
    }

    #[test]
    fn test_track_init_call_site_preserves_cli_contract_for_branch_strategy_adapter() {
        let root = tempfile::tempdir().unwrap();
        let items_dir = root.path().join("track").join("items");
        let config_dir = root.path().join(".harness").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("branch-strategy.json"),
            r#"{
  "base_branch": "main",
  "merge_target": "main",
  "merge_method": "merge"
}"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("architecture-rules.json"),
            include_str!("../../../../architecture-rules.json"),
        )
        .unwrap();

        let argv_items_dir = items_dir.clone();
        let argv_track_id = "adapter-track".to_owned();
        let argv_description = "Adapter Track".to_owned();
        let outcome =
            crate::track::TrackCompositionRoot::new().track_driver().handle(TrackInput::Init {
                items_dir: argv_items_dir.clone(),
                track_id: argv_track_id.clone(),
                description: argv_description.clone(),
            });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, None);
        assert_eq!(outcome.stderr, None);
        assert_eq!(argv_items_dir, items_dir);
        assert_eq!(argv_track_id, "adapter-track");
        assert_eq!(argv_description, "Adapter Track");
        assert!(
            items_dir.join("adapter-track").join("metadata.json").is_file(),
            "branch-strategy adapter call site must persist metadata"
        );
    }

    #[test]
    fn test_track_init_date_prefixed_ids_create_sorted_item_directories() {
        let root = tempfile::tempdir().unwrap();
        let items_dir = root.path().join("track").join("items");
        let config_dir = root.path().join(".harness").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("branch-strategy.json"),
            r#"{
  "base_branch": "main",
  "merge_target": "main",
  "merge_method": "merge"
}"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("architecture-rules.json"),
            include_str!("../../../../architecture-rules.json"),
        )
        .unwrap();

        let composition = crate::track::TrackCompositionRoot::new();
        for (track_id, title) in [
            ("2026-07-01-earlier-track", "Earlier Track"),
            ("2026-07-31-later-track", "Later Track"),
        ] {
            let outcome = composition.track_driver().handle(TrackInput::Init {
                items_dir: items_dir.clone(),
                track_id: track_id.to_owned(),
                description: title.to_owned(),
            });
            assert_eq!(outcome.exit_code, 0);
            assert!(
                items_dir.join(track_id).join("metadata.json").is_file(),
                "date-prefixed track ID must remain the item-directory name"
            );
        }

        let mut listed_ids = std::fs::read_dir(&items_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        listed_ids.sort_unstable();

        assert_eq!(
            listed_ids,
            ["2026-07-01-earlier-track", "2026-07-31-later-track"],
            "ascending item-directory listing must put the earlier date first"
        );
    }

    #[test]
    fn test_track_init_suffix_form_id_preserves_item_directory() {
        let root = tempfile::tempdir().unwrap();
        let items_dir = root.path().join("track").join("items");
        let config_dir = root.path().join(".harness").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("branch-strategy.json"),
            r#"{
  "base_branch": "main",
  "merge_target": "main",
  "merge_method": "merge"
}"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("architecture-rules.json"),
            include_str!("../../../../architecture-rules.json"),
        )
        .unwrap();

        let track_id = "legacy-suffix-track-2026-07-31";
        let outcome =
            crate::track::TrackCompositionRoot::new().track_driver().handle(TrackInput::Init {
                items_dir: items_dir.clone(),
                track_id: track_id.to_owned(),
                description: "Legacy Suffix Track".to_owned(),
            });

        assert_eq!(outcome.exit_code, 0);
        assert!(
            items_dir.join(track_id).join("metadata.json").is_file(),
            "a suffix-form ID must remain the item-directory name"
        );
    }

    #[test]
    fn test_track_archive_from_subdir_moves_track_and_logs_under_repo_root() {
        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_id = "my-track-2026";
        let track_dir = root.join("track").join("items").join(track_id);
        let logs_dir = track_dir.join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        std::fs::write(root.join(".gitignore"), "track/items/*/logs/\n").unwrap();
        std::fs::write(track_dir.join("tracked.txt"), "archive fixture\n").unwrap();
        std::fs::write(logs_dir.join("telemetry.jsonl"), "{}\n").unwrap();

        init_git_repo(root);
        GitRunner::at(root).assert_success(&[
            "add",
            ".gitignore",
            "track/items/my-track-2026/tracked.txt",
        ]);
        GitRunner::at(root).assert_success(&["commit", "-m", "add track", "--no-gpg-sign"]);

        let subdir = root.join("nested").join("workdir");
        std::fs::create_dir_all(&subdir).unwrap();
        let _cwd = change_to(&subdir);

        let outcome =
            crate::track::TrackCompositionRoot::new().track_driver().handle(TrackInput::Archive {
                items_dir: PathBuf::from("track/items"),
                track_id: track_id.to_owned(),
            });

        assert_eq!(outcome.exit_code, 0);
        let archived_dir = root.join("track").join("archive").join(track_id);
        assert!(archived_dir.join("tracked.txt").is_file());
        assert!(archived_dir.join("logs").join("telemetry.jsonl").is_file());
        assert!(!root.join("track").join("items").join(track_id).join("logs").exists());
    }

    #[test]
    fn test_track_archive_without_logs_from_subdir_succeeds_silently() {
        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_id = "no-logs-track-2026";
        let track_dir = root.join("track").join("items").join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("tracked.txt"), "archive fixture\n").unwrap();

        init_git_repo(root);
        GitRunner::at(root).assert_success(&["add", "track/items/no-logs-track-2026/tracked.txt"]);
        GitRunner::at(root).assert_success(&["commit", "-m", "add track", "--no-gpg-sign"]);

        let subdir = root.join("nested").join("workdir");
        std::fs::create_dir_all(&subdir).unwrap();
        let _cwd = change_to(&subdir);

        let outcome =
            crate::track::TrackCompositionRoot::new().track_driver().handle(TrackInput::Archive {
                items_dir: PathBuf::from("track/items"),
                track_id: track_id.to_owned(),
            });

        assert_eq!(outcome.exit_code, 0);
        let archived_dir = root.join("track").join("archive").join(track_id);
        assert!(archived_dir.join("tracked.txt").is_file());
        assert!(!archived_dir.join("logs").exists());
    }

    #[test]
    fn test_track_archive_missing_track_returns_error() {
        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("track").join("items")).unwrap();
        init_git_repo(root);

        let subdir = root.join("nested").join("workdir");
        std::fs::create_dir_all(&subdir).unwrap();
        let _cwd = change_to(&subdir);

        let outcome =
            crate::track::TrackCompositionRoot::new().track_driver().handle(TrackInput::Archive {
                items_dir: PathBuf::from("track/items"),
                track_id: "missing-track-2026".to_owned(),
            });
        let err = outcome.stderr.as_deref().or(outcome.stdout.as_deref()).unwrap_or_default();

        assert_ne!(outcome.exit_code, 0);
        assert!(err.contains("track directory not found"), "unexpected error: {err}");
        assert!(!root.join("track").join("archive").exists());
    }

    #[test]
    fn test_track_archive_untracked_source_returns_git_mv_error() {
        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_id = "untracked-track-2026";
        let track_dir = root.join("track").join("items").join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("untracked.txt"), "archive fixture\n").unwrap();
        init_git_repo(root);

        let subdir = root.join("nested").join("workdir");
        std::fs::create_dir_all(&subdir).unwrap();
        let _cwd = change_to(&subdir);

        let outcome =
            crate::track::TrackCompositionRoot::new().track_driver().handle(TrackInput::Archive {
                items_dir: PathBuf::from("track/items"),
                track_id: track_id.to_owned(),
            });
        let err = outcome.stderr.as_deref().or(outcome.stdout.as_deref()).unwrap_or_default();

        assert_ne!(outcome.exit_code, 0);
        assert!(err.contains("git mv failed"), "unexpected error: {err}");
        assert!(track_dir.join("untracked.txt").is_file());
    }

    // The former `test_rollback_archive_contents_after_logs_error_restores_source_tree`
    // test targeted the private `rollback_archive_contents_after_logs_error`
    // helper, which was deleted as part of the T006 cutover.
    // `TrackGitInteractor::archive_track` now performs the fs-side rename
    // directly through the port; its unit coverage lives in
    // `libs/usecase/src/git_workflow.rs::tests` (T003 mock-port tests).
}
