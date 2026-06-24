//! Infrastructure adapters implementing [`usecase::telemetry`] ports.
//!
//! - [`FsTelemetryReportAdapter`]: implements [`TelemetryReportPort`].
//! - [`FsTelemetryEmitDynamicAdapter`]: implements [`TelemetryEmitDynamicPort`].
//!
//! Maps infra output types to the usecase boundary types so that `cli_driver`
//! never imports infrastructure directly.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use usecase::telemetry::{
    TelemetryEmitDynamicPort, TelemetryEmitDynamicPortError,
    TelemetryErrorEntry as UsecaseErrorEntry, TelemetryHookBlockEntry as UsecaseHookBlockEntry,
    TelemetryPhaseDuration, TelemetryReportError as UsecaseError, TelemetryReportOutput,
    TelemetryReportPort,
};

use usecase::{
    ArchivedTrackTelemetryCommand, ArchivedTrackTelemetryInteractor,
    ArchivedTrackTelemetryService as _,
};

use crate::telemetry::report::{TelemetryReport, TelemetryReportError as InfraError};

#[derive(Debug)]
#[allow(dead_code)]
struct TelemetryAdapterError(String);

impl std::fmt::Display for TelemetryAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Filesystem adapter implementing [`TelemetryReportPort`].
///
/// Stateless: the `items_dir` is accepted per-call so the same adapter
/// instance can serve different items directories without re-construction.
pub struct FsTelemetryReportAdapter;

impl FsTelemetryReportAdapter {
    /// Construct the adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsTelemetryReportAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetryReportPort for FsTelemetryReportAdapter {
    fn aggregate(
        &self,
        track_id: &str,
        items_dir: &Path,
    ) -> Result<TelemetryReportOutput, UsecaseError> {
        let trusted_items_dir = resolve_items_dir_under_current_repo(items_dir)
            .map_err(|e| UsecaseError::ReportUnavailable(e.to_string()))?;

        let report = TelemetryReport::new(trusted_items_dir);
        let infra_output = report.aggregate(track_id).map_err(|e| match e {
            InfraError::TrackNotFound { track_id: tid, .. } => UsecaseError::TrackNotFound(tid),
            InfraError::Io { path, message } => {
                UsecaseError::ReportUnavailable(format!("{path}: {message}"))
            }
        })?;

        let phase_durations = infra_output
            .phase_durations
            .into_iter()
            .map(|pd| TelemetryPhaseDuration {
                phase_name: pd.phase_name,
                total_ms: pd.total_ms,
                event_count: pd.event_count as usize,
            })
            .collect();

        let errors = infra_output
            .errors
            .into_iter()
            .map(|e| UsecaseErrorEntry {
                timestamp: e.timestamp,
                command: e.command,
                exit_code: e.exit_code,
                error_chain: e.error_chain,
            })
            .collect();

        let hook_blocks = infra_output
            .hook_blocks
            .into_iter()
            .map(|hb| UsecaseHookBlockEntry { timestamp: hb.timestamp, hook_name: hb.hook_name })
            .collect();

        Ok(TelemetryReportOutput {
            phase_durations,
            errors,
            hook_blocks,
            skipped_lines: infra_output.skipped_lines as usize,
        })
    }
}

// ---------------------------------------------------------------------------
// FsTelemetryEmitDynamicAdapter
// ---------------------------------------------------------------------------

/// Filesystem adapter implementing [`TelemetryEmitDynamicPort`].
///
/// Mirrors the logic in
/// `cli_composition::telemetry::TelemetryCompositionRoot::telemetry_emit_archived_track_subcommand`:
/// derives the telemetry directory from `items_dir` + git repo discovery,
/// then delegates to [`ArchivedTrackTelemetryInteractor`].
pub struct FsTelemetryEmitDynamicAdapter;

impl FsTelemetryEmitDynamicAdapter {
    /// Construct the adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsTelemetryEmitDynamicAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetryEmitDynamicPort for FsTelemetryEmitDynamicAdapter {
    fn emit_archived(
        &self,
        items_dir: &Path,
        track_id: &str,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> Result<(), TelemetryEmitDynamicPortError> {
        use crate::git_cli::{GitRepository as _, SystemGitRepo};
        use crate::telemetry::archived_track::FsArchivedTrackTelemetryAdapter;

        let project_root = resolve_project_root_from_items_dir(items_dir)
            .map_err(|e| TelemetryEmitDynamicPortError::EmitUnavailable(e.to_string()))?;
        let repo = SystemGitRepo::discover_from(&project_root).map_err(|e| {
            TelemetryEmitDynamicPortError::EmitUnavailable(format!(
                "failed to discover git repository: {e}"
            ))
        })?;
        let repo_root = repo.root().to_path_buf();
        ensure_trusted_root(&repo_root)
            .map_err(|e| TelemetryEmitDynamicPortError::EmitUnavailable(e.to_string()))?;

        let valid_track_id = domain::TrackId::try_new(track_id.to_owned()).map_err(|e| {
            TelemetryEmitDynamicPortError::EmitUnavailable(format!("invalid track ID: {e}"))
        })?;
        let archive_root = repo_root.join("track").join("archive");
        crate::track::symlink_guard::reject_symlinks_below(&archive_root, &repo_root).map_err(
            |e| {
                TelemetryEmitDynamicPortError::EmitUnavailable(format!(
                    "failed to validate archive root: {e}"
                ))
            },
        )?;
        let telemetry_dir = archive_root.join(valid_track_id.as_ref()).join("logs");
        if !telemetry_dir.starts_with(&archive_root) {
            return Err(TelemetryEmitDynamicPortError::EmitUnavailable(format!(
                "telemetry path escapes archive root: {}",
                telemetry_dir.display()
            )));
        }
        crate::track::symlink_guard::reject_symlinks_below(&telemetry_dir, &archive_root).map_err(
            |e| {
                TelemetryEmitDynamicPortError::EmitUnavailable(format!(
                    "failed to validate telemetry path: {e}"
                ))
            },
        )?;

        let adapter = Arc::new(FsArchivedTrackTelemetryAdapter::new(telemetry_dir));
        let interactor = ArchivedTrackTelemetryInteractor::new(adapter);

        interactor
            .emit(ArchivedTrackTelemetryCommand {
                subcommand,
                track_id: track_id.to_owned(),
                exit_code,
                duration_ms,
            })
            .map_err(|e| TelemetryEmitDynamicPortError::EmitUnavailable(e.to_string()))
    }
}

fn resolve_project_root_from_items_dir(items_dir: &Path) -> Result<PathBuf, TelemetryAdapterError> {
    reject_items_dir_escape(items_dir)?;

    let items_name = items_dir.file_name().and_then(|n| n.to_str());
    let track_dir = items_dir.parent();
    let track_name = track_dir.and_then(Path::file_name).and_then(|n| n.to_str());
    let project_root = track_dir.and_then(Path::parent);

    match (items_name, track_name, project_root) {
        (Some("items"), Some("track"), Some(root)) => {
            let root = normalize_project_root(root);
            ensure_trusted_root(&root)?;
            crate::track::symlink_guard::reject_symlinks_below(items_dir, &root)
                .map(|_| ())
                .map_err(|e| {
                    TelemetryAdapterError(format!(
                        "items_dir path rejected before use at '{}': {e}",
                        items_dir.display()
                    ))
                })?;
            ensure_current_repo_root(&root)
        }
        _ => Err(TelemetryAdapterError(format!(
            "--items-dir must point to '<project-root>/track/items'; got {}",
            items_dir.display()
        ))),
    }
}

fn normalize_project_root(root: &Path) -> PathBuf {
    if root.as_os_str().is_empty() { PathBuf::from(".") } else { root.to_path_buf() }
}

fn resolve_items_dir_under_current_repo(
    items_dir: &Path,
) -> Result<PathBuf, TelemetryAdapterError> {
    use crate::git_cli::{GitRepository as _, SystemGitRepo};

    reject_items_dir_escape(items_dir)?;

    let repo = SystemGitRepo::discover().map_err(|e| {
        TelemetryAdapterError(format!("cannot discover current git repository: {e}"))
    })?;
    let repo_root = repo.root().canonicalize().map_err(|e| {
        TelemetryAdapterError(format!(
            "failed to canonicalize current repository root {}: {e}",
            repo.root().display()
        ))
    })?;
    ensure_trusted_root(&repo_root)?;

    let absolute_items_dir =
        if items_dir.is_absolute() { items_dir.to_path_buf() } else { repo_root.join(items_dir) };
    if !absolute_items_dir.starts_with(&repo_root) {
        return Err(TelemetryAdapterError(format!(
            "--items-dir must resolve inside the current repository root {}; got {}",
            repo_root.display(),
            items_dir.display()
        )));
    }

    crate::track::symlink_guard::reject_symlinks_below(&absolute_items_dir, &repo_root)
        .map(|_| ())
        .map_err(|e| {
            TelemetryAdapterError(format!(
                "items_dir path rejected before use at '{}': {e}",
                items_dir.display()
            ))
        })?;

    let canonical_items_dir = absolute_items_dir.canonicalize().map_err(|e| {
        TelemetryAdapterError(format!(
            "failed to canonicalize items_dir {}: {e}",
            items_dir.display()
        ))
    })?;
    if !canonical_items_dir.starts_with(&repo_root) {
        return Err(TelemetryAdapterError(format!(
            "--items-dir resolves outside the current repository root {}; got {}",
            repo_root.display(),
            canonical_items_dir.display()
        )));
    }
    if !canonical_items_dir.is_dir() {
        return Err(TelemetryAdapterError(format!(
            "--items-dir is not a directory: {}",
            items_dir.display()
        )));
    }

    Ok(canonical_items_dir)
}

fn reject_items_dir_escape(items_dir: &Path) -> Result<(), TelemetryAdapterError> {
    if items_dir.as_os_str().is_empty() {
        return Err(TelemetryAdapterError("--items-dir must not be empty".to_owned()));
    }
    if items_dir
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(TelemetryAdapterError(format!(
            "--items-dir cannot escape the current repository root: {}",
            items_dir.display()
        )));
    }
    Ok(())
}

fn ensure_trusted_root(root: &Path) -> Result<(), TelemetryAdapterError> {
    match root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => Err(TelemetryAdapterError(format!(
            "refusing to use symlinked repository root: {}",
            root.display()
        ))),
        Ok(_) => Ok(()),
        Err(err) => Err(TelemetryAdapterError(format!(
            "failed to stat repository root {}: {err}",
            root.display()
        ))),
    }
}

fn ensure_current_repo_root(root: &Path) -> Result<PathBuf, TelemetryAdapterError> {
    use crate::git_cli::{GitRepository as _, SystemGitRepo};

    let canonical_root = root.canonicalize().map_err(|e| {
        TelemetryAdapterError(format!(
            "failed to canonicalize project root {}: {e}",
            root.display()
        ))
    })?;
    let repo = SystemGitRepo::discover().map_err(|e| {
        TelemetryAdapterError(format!("cannot discover current git repository: {e}"))
    })?;
    let canonical_repo_root = repo.root().canonicalize().map_err(|e| {
        TelemetryAdapterError(format!(
            "failed to canonicalize current repository root {}: {e}",
            repo.root().display()
        ))
    })?;
    if canonical_root != canonical_repo_root {
        return Err(TelemetryAdapterError(format!(
            "--items-dir must resolve to the current repository root {}; got {}",
            canonical_repo_root.display(),
            canonical_root.display()
        )));
    }
    Ok(canonical_root)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::Path;

    use super::{
        FsTelemetryReportAdapter, ensure_trusted_root, resolve_items_dir_under_current_repo,
        resolve_project_root_from_items_dir,
    };
    use crate::git_cli::GitRepository as _;
    use usecase::telemetry::{TelemetryReportError as UsecaseError, TelemetryReportPort as _};

    fn tempdir_in_current_repo() -> tempfile::TempDir {
        let repo = crate::git_cli::SystemGitRepo::discover().unwrap();
        let target_dir = repo.root().join("target").join("telemetry-report-adapter-tests");
        std::fs::create_dir_all(&target_dir).unwrap();
        tempfile::Builder::new().prefix("items-").tempdir_in(target_dir).unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn ensure_trusted_root_rejects_symlinked_root() {
        let real_root = tempfile::tempdir().unwrap();
        let link_parent = tempfile::tempdir().unwrap();
        let root_link = link_parent.path().join("workspace-link");
        std::os::unix::fs::symlink(real_root.path(), &root_link).unwrap();

        let err = ensure_trusted_root(&root_link).unwrap_err();

        assert!(err.to_string().contains("refusing to use symlinked repository root"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn resolve_project_root_from_items_dir_rejects_parent_dir_escape() {
        let err =
            resolve_project_root_from_items_dir(Path::new("../other/track/items")).unwrap_err();

        assert!(err.to_string().contains("cannot escape the current repository root"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn resolve_project_root_from_items_dir_rejects_symlinked_root() {
        let real_root = tempfile::tempdir().unwrap();
        let link_parent = tempfile::tempdir().unwrap();
        let root_link = link_parent.path().join("workspace-link");
        std::os::unix::fs::symlink(real_root.path(), &root_link).unwrap();
        let items_dir = root_link.join("track").join("items");

        let err = resolve_project_root_from_items_dir(&items_dir).unwrap_err();

        assert!(err.to_string().contains("refusing to use symlinked repository root"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn resolve_project_root_from_items_dir_rejects_symlinked_items_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let track_dir = tmp.path().join("track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::os::unix::fs::symlink(outside.path(), track_dir.join("items")).unwrap();
        let items_dir = track_dir.join("items");

        let err = resolve_project_root_from_items_dir(&items_dir).unwrap_err();

        assert!(err.to_string().contains("items_dir path rejected before use"), "{err}");
        assert!(err.to_string().contains("refusing to follow symlink"), "{err}");
    }

    #[test]
    fn resolve_project_root_from_items_dir_rejects_non_current_repo_root() {
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = tmp.path().join("track").join("items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let err = resolve_project_root_from_items_dir(&items_dir).unwrap_err();

        assert!(err.to_string().contains("current repository root"), "{err}");
    }

    #[test]
    fn aggregate_rejects_parent_dir_escape() {
        let err = FsTelemetryReportAdapter::new()
            .aggregate("some-track", Path::new("../other/track/items"))
            .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("cannot escape the current repository root"), "{message}");
    }

    #[test]
    fn aggregate_accepts_supplied_absolute_items_dir_inside_current_repo() {
        let tmp = tempdir_in_current_repo();
        std::fs::create_dir_all(tmp.path().join("some-track")).unwrap();

        let output = FsTelemetryReportAdapter::new().aggregate("some-track", tmp.path()).unwrap();

        assert!(output.phase_durations.is_empty());
        assert!(output.errors.is_empty());
        assert!(output.hook_blocks.is_empty());
        assert_eq!(output.skipped_lines, 0);
    }

    #[test]
    fn aggregate_rejects_supplied_absolute_items_dir_outside_current_repo() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("some-track")).unwrap();

        let err = FsTelemetryReportAdapter::new().aggregate("some-track", tmp.path()).unwrap_err();

        let message = err.to_string();
        assert!(message.contains("inside the current repository root"), "{message}");
    }

    #[test]
    fn resolve_items_dir_under_current_repo_rejects_absolute_outside_current_repo() {
        let tmp = tempfile::tempdir().unwrap();

        let err = resolve_items_dir_under_current_repo(tmp.path()).unwrap_err();

        assert!(err.to_string().contains("inside the current repository root"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn aggregate_rejects_symlinked_items_dir_root() {
        let tmp = tempdir_in_current_repo();
        let outside = tempfile::tempdir().unwrap();
        let track_dir = tmp.path().join("track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let items_link = track_dir.join("items");
        std::os::unix::fs::symlink(outside.path(), &items_link).unwrap();

        let err = FsTelemetryReportAdapter::new().aggregate("some-track", &items_link).unwrap_err();

        assert!(
            matches!(&err, UsecaseError::ReportUnavailable(_)),
            "expected ReportUnavailable; got {err:?}"
        );
        let message = err.to_string();
        assert!(message.contains("items_dir path rejected before use"), "{message}");
        assert!(message.contains("refusing to follow symlink"), "{message}");
    }

    #[cfg(unix)]
    #[test]
    fn aggregate_rejects_symlinked_items_dir_ancestor() {
        let tmp = tempdir_in_current_repo();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(outside.path().join("items")).unwrap();
        let track_link = tmp.path().join("track");
        std::os::unix::fs::symlink(outside.path(), &track_link).unwrap();
        let items_dir = track_link.join("items");

        let err = FsTelemetryReportAdapter::new().aggregate("some-track", &items_dir).unwrap_err();

        assert!(
            matches!(&err, UsecaseError::ReportUnavailable(_)),
            "expected ReportUnavailable; got {err:?}"
        );
        let message = err.to_string();
        assert!(message.contains("items_dir path rejected before use"), "{message}");
        assert!(message.contains("refusing to follow symlink"), "{message}");
    }
}
