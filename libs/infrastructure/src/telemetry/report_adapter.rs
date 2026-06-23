//! Infrastructure adapters implementing [`usecase::telemetry`] ports.
//!
//! - [`FsTelemetryReportAdapter`]: implements [`TelemetryReportPort`].
//! - [`FsTelemetryEmitDynamicAdapter`]: implements [`TelemetryEmitDynamicPort`].
//!
//! Maps infra output types to the usecase boundary types so that `cli_driver`
//! never imports infrastructure directly.

use std::path::{Path, PathBuf};
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
        ensure_trusted_items_dir_root(items_dir).map_err(UsecaseError::ReportUnavailable)?;

        let report = TelemetryReport::new(items_dir.to_path_buf());
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
            .map_err(TelemetryEmitDynamicPortError::EmitUnavailable)?;
        let repo = SystemGitRepo::discover_from(&project_root).map_err(|e| {
            TelemetryEmitDynamicPortError::EmitUnavailable(format!(
                "failed to discover git repository: {e}"
            ))
        })?;
        let repo_root = repo.root().to_path_buf();
        ensure_trusted_root(&repo_root).map_err(TelemetryEmitDynamicPortError::EmitUnavailable)?;

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

fn resolve_project_root_from_items_dir(items_dir: &Path) -> Result<PathBuf, String> {
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
                    format!("items_dir path rejected before use at '{}': {e}", items_dir.display())
                })?;
            Ok(root)
        }
        _ => Err(format!(
            "--items-dir must point to '<project-root>/track/items'; got {}",
            items_dir.display()
        )),
    }
}

fn normalize_project_root(root: &Path) -> PathBuf {
    if root.as_os_str().is_empty() { PathBuf::from(".") } else { root.to_path_buf() }
}

fn ensure_trusted_root(root: &Path) -> Result<(), String> {
    match root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            Err(format!("refusing to use symlinked repository root: {}", root.display()))
        }
        Ok(_) => Ok(()),
        Err(err) => Err(format!("failed to stat repository root {}: {err}", root.display())),
    }
}

fn ensure_trusted_items_dir_root(items_dir: &Path) -> Result<(), String> {
    let absolute_items_dir = if items_dir.is_absolute() {
        items_dir.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("failed to resolve items_dir {}: {e}", items_dir.display()))?
            .join(items_dir)
    };

    crate::track::symlink_guard::reject_symlinks_below(&absolute_items_dir, Path::new("/"))
        .map(|_| ())
        .map_err(|e| {
            format!("items_dir path rejected before use at '{}': {e}", items_dir.display())
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{
        FsTelemetryReportAdapter, ensure_trusted_root, resolve_project_root_from_items_dir,
    };
    use usecase::telemetry::{TelemetryReportError as UsecaseError, TelemetryReportPort as _};

    #[cfg(unix)]
    #[test]
    fn ensure_trusted_root_rejects_symlinked_root() {
        let real_root = tempfile::tempdir().unwrap();
        let link_parent = tempfile::tempdir().unwrap();
        let root_link = link_parent.path().join("workspace-link");
        std::os::unix::fs::symlink(real_root.path(), &root_link).unwrap();

        let err = ensure_trusted_root(&root_link).unwrap_err();

        assert!(err.contains("refusing to use symlinked repository root"), "{err}");
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

        assert!(err.contains("refusing to use symlinked repository root"), "{err}");
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

        assert!(err.contains("items_dir path rejected before use"), "{err}");
        assert!(err.contains("refusing to follow symlink"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn aggregate_rejects_symlinked_items_dir_root() {
        let real_items = tempfile::tempdir().unwrap();
        let link_parent = tempfile::tempdir().unwrap();
        let items_link = link_parent.path().join("items-link");
        std::os::unix::fs::symlink(real_items.path(), &items_link).unwrap();

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
        let real_parent = tempfile::tempdir().unwrap();
        let real_items = real_parent.path().join("items");
        std::fs::create_dir(&real_items).unwrap();
        let link_parent = tempfile::tempdir().unwrap();
        let parent_link = link_parent.path().join("parent-link");
        std::os::unix::fs::symlink(real_parent.path(), &parent_link).unwrap();
        let items_dir = parent_link.join("items");

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
