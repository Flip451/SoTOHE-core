use std::fs;
use std::path::Path;
use std::sync::Arc;

use domain::CommitHash;
use domain::tddd::catalogue_v2::RustdocBaselineCapturePort;
use usecase::base_merge::{BaseMergeCleanupRequest, BaselineReplacementError};
use usecase::git_workflow::DiagnosticText;

use super::baseline_support::{
    add_commit_pinned_worktree, create_unique_directory, remove_commit_pinned_worktree,
};
use super::cleanup_tree::{
    capture_baselines_in_worktree, collect_validated_baselines, copy_cleanup_inputs_with_baselines,
    copy_tree_with_baselines, remove_tree_bounded,
};
use super::publication::{
    generated_baseline_file_names, path_exists, promote_baseline_recovery_slot,
    publish_baseline_replacements, reconcile_interrupted_replacement,
    write_replacement_phase_marker,
};
use super::{
    BASELINE_REPLACEMENT_PHASE_MARKER, MAX_BASE_MERGE_GIT_OUTPUT_BYTES,
    resolve_workspace_repository_root,
};
use crate::git_cli::isolated_bounded_git_output;
use crate::track::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};

#[derive(Debug)]
pub(super) enum ExactCommitReplacementError {
    Baseline(BaselineReplacementError),
    WorktreeLifecycle(DiagnosticText),
    HeadMismatch { expected: CommitHash, actual: CommitHash },
}

pub(super) fn replace_baselines_from_exact_commit(
    request: &BaseMergeCleanupRequest,
    baseline_capture: Arc<dyn RustdocBaselineCapturePort>,
) -> Result<(), BaselineReplacementError> {
    run_exact_commit_replacement(request, baseline_capture).map_err(|error| match error {
        ExactCommitReplacementError::Baseline(error) => error,
        ExactCommitReplacementError::WorktreeLifecycle(detail) => {
            BaselineReplacementError::Isolation(detail)
        }
        ExactCommitReplacementError::HeadMismatch { expected, actual } => {
            BaselineReplacementError::Isolation(DiagnosticText::new(format!(
                "pinned baseline worktree HEAD mismatch: expected {expected}, got {actual}"
            )))
        }
    })
}

fn run_exact_commit_replacement(
    request: &BaseMergeCleanupRequest,
    baseline_capture: Arc<dyn RustdocBaselineCapturePort>,
) -> Result<(), ExactCommitReplacementError> {
    let baseline_error = |error| ExactCommitReplacementError::Baseline(error);
    let repository_root =
        resolve_workspace_repository_root(&request.workspace_root).map_err(|detail| {
            baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(detail)))
        })?;
    let track_dir = request.workspace_root.join("track/items").join(request.track_id.as_ref());
    let items_dir = request.workspace_root.join("track/items");
    reject_symlinks_up_to_root(&items_dir).map_err(|error| {
        baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(error.to_string())))
    })?;
    reject_symlinks_below(&track_dir, &items_dir).map_err(|error| {
        baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(error.to_string())))
    })?;
    if !track_dir.is_dir() {
        return Err(baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(
            "active track directory is unavailable",
        ))));
    }
    let generated_baseline_files =
        generated_baseline_file_names(&request.workspace_root).map_err(|error| {
            baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(error)))
        })?;

    let track_parent = track_dir.parent().ok_or_else(|| {
        baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(
            "active track directory has no parent directory",
        )))
    })?;
    let track_root = track_parent.parent().ok_or_else(|| {
        baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(
            "track items directory has no recovery root",
        )))
    })?;
    reject_symlinks_up_to_root(track_root).map_err(|error| {
        baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot inspect track recovery root: {error}"
        ))))
    })?;
    let recovery_root = track_root.join(".sotp-baseline-recovery");
    fs::create_dir_all(&recovery_root).map_err(|error| {
        baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot create baseline recovery root: {error}"
        ))))
    })?;
    fs::File::open(track_root).and_then(|directory| directory.sync_all()).map_err(|error| {
        baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot persist baseline recovery root entry: {error}"
        ))))
    })?;
    reject_symlinks_below(&recovery_root, track_root).map_err(|error| {
        baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot inspect baseline recovery root: {error}"
        ))))
    })?;
    let recovery_slot = recovery_root.join(request.track_id.as_ref());
    let replacement =
        recovery_root.join(format!(".sotp-baseline-replacement-{}", request.track_id));
    let active_phase_marker = track_dir.join(BASELINE_REPLACEMENT_PHASE_MARKER);
    reject_symlinks_below(&active_phase_marker, &items_dir).map_err(|error| {
        baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot inspect active baseline replacement phase marker: {error}"
        ))))
    })?;
    reject_symlinks_below(&replacement, &recovery_root).map_err(|error| {
        baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot inspect baseline replacement staging slot: {error}"
        ))))
    })?;
    let had_replacement = path_exists(&replacement).map_err(|error| {
        baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(error)))
    })?;
    if had_replacement {
        reconcile_interrupted_replacement(
            &replacement,
            &recovery_slot,
            &recovery_root,
            &track_dir,
            &generated_baseline_files,
        )
        .map_err(|error| {
            baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(error)))
        })?;
    }
    if path_exists(&active_phase_marker).map_err(|error| {
        baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(error)))
    })? {
        let recovered_canonical_slot = path_exists(&recovery_slot).map_err(|error| {
            baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(error)))
        })?;
        if !had_replacement && !recovered_canonical_slot {
            return Err(baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(
                "active track contains an unreconciled baseline replacement phase marker",
            ))));
        }
        fs::remove_file(&active_phase_marker).map_err(|error| {
            baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(format!(
                "cannot clear recovered baseline replacement phase marker: {error}"
            ))))
        })?;
        fs::File::open(&track_dir).and_then(|directory| directory.sync_all()).map_err(|error| {
            baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(format!(
                "cannot persist recovered baseline replacement phase marker removal: {error}"
            ))))
        })?;
    }
    fs::create_dir(&replacement).map_err(|error| {
        baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot create baseline replacement staging slot: {error}"
        ))))
    })?;

    let worktree = match create_unique_directory(&repository_root, ".sotp-base-merge-worktree-") {
        Ok(worktree) => worktree,
        Err(error) => {
            let detail =
                match cleanup_directory(&replacement, "baseline replacement staging directory") {
                    Ok(()) => error.to_string(),
                    Err(cleanup) => format!("{error}; {cleanup}"),
                };
            return Err(ExactCommitReplacementError::WorktreeLifecycle(DiagnosticText::new(
                detail,
            )));
        }
    };
    let mut published = false;
    let mut exchanged = false;
    let result: Result<(), ExactCommitReplacementError> = (|| {
        add_commit_pinned_worktree(&repository_root, &worktree, &request.base_commit).map_err(
            |error| ExactCommitReplacementError::WorktreeLifecycle(DiagnosticText::new(error)),
        )?;
        let actual = read_worktree_head(&worktree).map_err(|error| {
            ExactCommitReplacementError::WorktreeLifecycle(DiagnosticText::new(error))
        })?;
        if actual != request.base_commit {
            return Err(ExactCommitReplacementError::HeadMismatch {
                expected: request.base_commit.clone(),
                actual,
            });
        }
        copy_cleanup_inputs_with_baselines(
            &request.workspace_root,
            &worktree,
            request.track_id.as_ref(),
            &generated_baseline_files,
        )
        .map_err(|error| {
            baseline_error(BaselineReplacementError::Isolation(DiagnosticText::new(error)))
        })?;
        capture_baselines_in_worktree(
            &worktree,
            request.track_id.as_ref(),
            Arc::clone(&baseline_capture),
        )
        .map_err(baseline_error)?;
        collect_validated_baselines(&worktree, request.track_id.as_ref()).map_err(|error| {
            baseline_error(BaselineReplacementError::Validation(DiagnosticText::new(error)))
        })?;
        write_replacement_phase_marker(&replacement).map_err(baseline_error)?;
        copy_tree_with_baselines(
            &worktree.join("track/items").join(request.track_id.as_ref()),
            &replacement,
            true,
            &recovery_root,
            &generated_baseline_files,
        )
        .map_err(|error| {
            baseline_error(BaselineReplacementError::Validation(DiagnosticText::new(error)))
        })?;
        publish_baseline_replacements(
            &track_dir,
            &replacement,
            &generated_baseline_files,
            &mut exchanged,
        )
        .map_err(baseline_error)?;
        published = true;
        promote_baseline_recovery_slot(&replacement, &recovery_slot, &recovery_root)
            .map_err(baseline_error)?;
        Ok(())
    })();

    let removal = remove_commit_pinned_worktree(&repository_root, &worktree)
        .map_err(|error| format!("cannot unregister detached cleanup worktree: {error}"));
    let directory_cleanup = cleanup_directory(&worktree, "detached cleanup worktree");
    // Clean merge retains the exchanged prior tree for the later sync-base
    // transaction.
    let replacement_cleanup = if result.is_err()
        && !published
        && !exchanged
        && !matches!(
            &result,
            Err(ExactCommitReplacementError::Baseline(
                BaselineReplacementError::Restoration { .. }
            ))
        ) {
        cleanup_directory(&replacement, "baseline replacement staging directory")
    } else {
        Ok(())
    };
    combine_exact_commit_cleanup_result(result, [removal, directory_cleanup, replacement_cleanup])
}

fn read_worktree_head(worktree: &Path) -> Result<CommitHash, String> {
    let output = isolated_bounded_git_output(
        worktree,
        &["rev-parse", "--verify", "HEAD^{commit}"],
        MAX_BASE_MERGE_GIT_OUTPUT_BYTES,
    )
    .map_err(|error| format!("cannot inspect disposable worktree HEAD: {error}"))?;
    if !output.status.success() {
        return Err("cannot inspect disposable worktree HEAD".to_owned());
    }
    let actual = std::str::from_utf8(&output.stdout)
        .map_err(|_| "disposable worktree HEAD is invalid".to_owned())?;
    CommitHash::try_new(actual.trim().to_owned())
        .map_err(|_| "disposable worktree HEAD is invalid".to_owned())
}

fn cleanup_directory(path: &Path, label: &str) -> Result<(), String> {
    let trusted_root = path
        .parent()
        .ok_or_else(|| format!("cannot remove {label} {}: no trusted parent", path.display()))?;
    remove_tree_bounded(path, trusted_root)
        .map_err(|error| format!("cannot remove {label} {}: {error}", path.display()))
}

pub(super) fn combine_exact_commit_cleanup_result(
    result: Result<(), ExactCommitReplacementError>,
    cleanup_results: [Result<(), String>; 3],
) -> Result<(), ExactCommitReplacementError> {
    let [removal, directory_cleanup, replacement_cleanup] = cleanup_results;
    let worktree_details =
        [removal, directory_cleanup].into_iter().filter_map(Result::err).collect::<Vec<_>>();
    let replacement_details = match replacement_cleanup {
        Ok(()) => Vec::new(),
        Err(error) => vec![error],
    };
    let all_cleanup = worktree_details
        .iter()
        .chain(replacement_details.iter())
        .map(String::as_str)
        .collect::<Vec<_>>();
    if all_cleanup.is_empty() {
        return result;
    }
    let cleanup = all_cleanup.join("; ");
    if !worktree_details.is_empty() {
        return Err(ExactCommitReplacementError::WorktreeLifecycle(DiagnosticText::new(
            match result {
                Ok(()) => format!("cleanup after baseline replacement failed: {cleanup}"),
                Err(error) => format!("{error:?}; cleanup also failed: {cleanup}"),
            },
        )));
    }
    match result {
        Ok(()) => Err(ExactCommitReplacementError::Baseline(BaselineReplacementError::Publish(
            DiagnosticText::new(format!("cleanup after baseline replacement failed: {cleanup}")),
        ))),
        Err(ExactCommitReplacementError::Baseline(error)) => {
            Err(ExactCommitReplacementError::Baseline(append_cleanup_failure(error, cleanup)))
        }
        Err(error @ ExactCommitReplacementError::HeadMismatch { .. }) => Err(error),
        Err(ExactCommitReplacementError::WorktreeLifecycle(detail)) => {
            Err(ExactCommitReplacementError::WorktreeLifecycle(DiagnosticText::new(format!(
                "{detail}; cleanup also failed: {cleanup}"
            ))))
        }
    }
}

fn append_cleanup_failure(
    error: BaselineReplacementError,
    cleanup: String,
) -> BaselineReplacementError {
    let append = |detail: DiagnosticText| {
        DiagnosticText::new(format!("{detail}; cleanup also failed: {cleanup}"))
    };
    match error {
        BaselineReplacementError::Isolation(detail) => {
            BaselineReplacementError::Isolation(append(detail))
        }
        BaselineReplacementError::Generation(detail) => {
            BaselineReplacementError::Generation(append(detail))
        }
        BaselineReplacementError::Validation(detail) => {
            BaselineReplacementError::Validation(append(detail))
        }
        BaselineReplacementError::Publish(detail) => {
            BaselineReplacementError::Publish(append(detail))
        }
        BaselineReplacementError::Restoration { publish, restoration } => {
            BaselineReplacementError::Restoration { publish, restoration: append(restoration) }
        }
    }
}
