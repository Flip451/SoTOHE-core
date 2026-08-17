//! Infrastructure adapters implementing [`usecase::telemetry`] ports.
//!
//! - [`FsTelemetryReportAdapter`]: implements [`TelemetryReportPort`].
//! - [`FsTelemetryEmitDynamicAdapter`]: implements [`TelemetryEmitDynamicPort`].
//!
//! Maps infra output types to the usecase boundary types so that `cli_driver`
//! never imports infrastructure directly.

use std::path::{Component, Path, PathBuf};

use domain::TrackId;
use usecase::telemetry::{
    TelemetryEmitDynamicPort, TelemetryEmitDynamicPortError,
    TelemetryErrorEntry as UsecaseErrorEntry, TelemetryHookBlockEntry as UsecaseHookBlockEntry,
    TelemetryPhaseDuration, TelemetryReportError as UsecaseError, TelemetryReportOutput,
    TelemetryReportPort,
};

use crate::telemetry::report::{TelemetryReport, TelemetryReportError as InfraError};
use crate::telemetry::{TelemetryConfig, TelemetryEvent, TelemetryWriter};

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

        let valid_track_id = TrackId::try_new(track_id.to_owned())
            .map_err(|_| UsecaseError::TrackNotFound(track_id.to_owned()))?;
        let report = TelemetryReport::new(trusted_items_dir);
        let infra_output = report.aggregate(&valid_track_id).map_err(|e| match e {
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
            skipped_lines: infra_output.skipped_lines,
            command_metrics: infra_output.command_metrics,
            review_yield_metrics: infra_output.review_yield_metrics,
        })
    }
}

// ---------------------------------------------------------------------------
// FsTelemetryEmitDynamicAdapter
// ---------------------------------------------------------------------------

/// Filesystem adapter implementing [`TelemetryEmitDynamicPort`].
///
/// Resolves the repository from `items_dir` and delegates active
/// command-completion persistence to the existing telemetry writer. The
/// caller supplies the track captured before dispatch; a missing context is a
/// branch-bound no-op.
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
    fn emit_active(
        &self,
        items_dir: &Path,
        source_track_id: Option<&str>,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
        error_chain: Option<String>,
    ) -> Result<(), TelemetryEmitDynamicPortError> {
        // `None` is the captured pre-dispatch non-track context. Never
        // re-read the branch after dispatch: a command may have switched to a
        // track branch, but that destination must not receive the event.
        let Some(source_track_id) = source_track_id else {
            return Ok(());
        };
        // Resolve the repository from the supplied items directory, never from
        // the process CWD. Invalid paths and unavailable repository state are
        // typed adapter failures; the driver deliberately swallows those
        // diagnostic failures so the original command outcome remains unchanged.
        let anchor_root = resolve_project_root_from_items_dir(items_dir)
            .map_err(|e| TelemetryEmitDynamicPortError::EmitUnavailable(e.to_string()))?;
        let resolved_track_id = source_track_id.to_owned();
        let track_id = TrackId::try_new(resolved_track_id).map_err(|error| {
            TelemetryEmitDynamicPortError::EmitUnavailable(format!(
                "invalid captured track id: {error}"
            ))
        })?;
        let anchored_items_dir = anchor_root.join("track").join("items");
        let writer =
            TelemetryWriter::new(TelemetryConfig::from_env(), track_id.clone(), anchored_items_dir);

        let timestamp = chrono::Utc::now().to_rfc3339();
        // The existing TelemetryWriter is the sole active-track sink. Its
        // kill-switch, override path, append semantics, and fail-open write
        // behavior therefore remain unchanged.
        let mut write_error = None;
        if let Err(error) = writer.write(TelemetryEvent::TrackSubcommand {
            schema_version: 1,
            track_id: track_id.as_ref().to_owned(),
            command: subcommand.clone(),
            exit_code,
            duration_ms,
            timestamp: timestamp.clone(),
        }) {
            write_error = Some(error.to_string());
        }
        if exit_code != 0 {
            if let Err(error) = writer.write(TelemetryEvent::NonZeroExit {
                schema_version: 1,
                track_id: track_id.as_ref().to_owned(),
                command: subcommand,
                exit_code,
                error_chain: error_chain.unwrap_or_default(),
                timestamp,
            }) {
                if write_error.is_none() {
                    write_error = Some(error.to_string());
                }
            }
        }
        write_error
            .map_or(Ok(()), |error| Err(TelemetryEmitDynamicPortError::EmitUnavailable(error)))
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
            let default_items_dir =
                !items_dir.is_absolute() && items_dir == Path::new("track/items");
            let root = normalize_project_root(root);
            ensure_trusted_root(&root)?;
            let absolute_root = if root.is_absolute() {
                root.clone()
            } else {
                std::env::current_dir()
                    .map_err(|e| {
                        TelemetryAdapterError(format!(
                            "cannot resolve current directory for relative items_dir: {e}"
                        ))
                    })?
                    .join(&root)
            };
            crate::track::symlink_guard::reject_symlinks_up_to_root(&absolute_root).map_err(
                |e| {
                    TelemetryAdapterError(format!(
                        "items_dir path rejected before use at '{}': {e}",
                        items_dir.display()
                    ))
                },
            )?;
            if !default_items_dir {
                let supplied_absolute_items_dir = if items_dir.is_absolute() {
                    items_dir.to_path_buf()
                } else {
                    absolute_root.join("track").join("items")
                };
                crate::track::symlink_guard::reject_symlinks_below(
                    &supplied_absolute_items_dir,
                    &absolute_root,
                )
                .map(|_| ())
                .map_err(|e| {
                    TelemetryAdapterError(format!(
                        "items_dir path rejected before use at '{}': {e}",
                        items_dir.display()
                    ))
                })?;
            }
            let canonical_project_root = absolute_root.canonicalize().map_err(|e| {
                TelemetryAdapterError(format!(
                    "failed to canonicalize project root {}: {e}",
                    absolute_root.display()
                ))
            })?;
            let repo =
                crate::git_cli::SystemGitRepo::discover_from_isolated(&canonical_project_root)
                    .map_err(|e| {
                        TelemetryAdapterError(format!(
                            "cannot discover git repository from supplied project root {}: {e}",
                            canonical_project_root.display()
                        ))
                    })?;
            let repo_root = repo.root().canonicalize().map_err(|e| {
                TelemetryAdapterError(format!(
                    "failed to canonicalize discovered repository root {}: {e}",
                    repo.root().display()
                ))
            })?;
            ensure_trusted_root(&repo_root)?;
            // Explicit non-default items roots pass through unchanged,
            // including nested in-repository layouts such as
            // `<repo>/custom/track/items`, matching the pre-dispatch context
            // resolver. Only roots outside the repository stay fail-closed.
            if !canonical_project_root.starts_with(&repo_root) {
                return Err(TelemetryAdapterError(format!(
                    "--items-dir must resolve inside the discovered repository root {}; got {}",
                    repo_root.display(),
                    canonical_project_root.display()
                )));
            }
            crate::track::symlink_guard::reject_symlinks_up_to_root(&repo_root).map_err(|e| {
                TelemetryAdapterError(format!(
                    "items_dir path rejected before use at '{}': {e}",
                    items_dir.display()
                ))
            })?;
            let absolute_items_dir = if items_dir.is_absolute() {
                items_dir.to_path_buf()
            } else if default_items_dir {
                // The CLI's default `track/items` path is repository-relative,
                // even when the process starts in a nested working directory.
                repo_root.join("track").join("items")
            } else {
                std::env::current_dir()
                    .map_err(|e| {
                        TelemetryAdapterError(format!(
                            "cannot resolve current directory for relative items_dir: {e}"
                        ))
                    })?
                    .join(items_dir)
            };
            crate::track::symlink_guard::reject_symlinks_below(&absolute_items_dir, &repo_root)
                .map(|_| ())
                .map_err(|e| {
                    TelemetryAdapterError(format!(
                        "items_dir path rejected before use at '{}': {e}",
                        items_dir.display()
                    ))
                })?;
            // Return the anchor the writer must join `track/items` onto: the
            // repository root for the repository-relative default, and the
            // validated supplied project root (possibly nested) otherwise, so
            // emission preserves the caller's items directory.
            if default_items_dir { Ok(repo_root) } else { Ok(canonical_project_root) }
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
    use crate::git_cli::SystemGitRepo;

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
    #[cfg(not(windows))]
    if !absolute_items_dir.starts_with(&repo_root) {
        return Err(TelemetryAdapterError(format!(
            "--items-dir must resolve inside the current repository root {}; got {}",
            repo_root.display(),
            items_dir.display()
        )));
    }

    // Inspect the supplied path before canonicalization so symlink leaves and
    // ancestors are rejected before any filesystem read follows them. Windows
    // may represent the canonical repository root with an extended-length
    // prefix (`\\?\\C:`), so lexical prefix checks on the raw path are not
    // reliable across drive-letter and UNC spellings.
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
            "--items-dir must resolve inside the current repository root {}; got {}",
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
    if items_dir.components().any(|component| matches!(component, Component::ParentDir)) {
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::{
        FsTelemetryEmitDynamicAdapter, FsTelemetryReportAdapter, ensure_trusted_root,
        resolve_items_dir_under_current_repo, resolve_project_root_from_items_dir,
    };
    use usecase::telemetry::review_yield::ReviewYieldValue;
    use usecase::telemetry::{
        TelemetryEmitDynamicPort as _, TelemetryEmitDynamicPortError,
        TelemetryReportError as UsecaseError, TelemetryReportPort as _,
    };

    fn tempdir_in_current_repo() -> tempfile::TempDir {
        let repo = crate::git_cli::SystemGitRepo::discover().unwrap();
        let target_dir = repo.root().join("target").join("telemetry-report-adapter-tests");
        std::fs::create_dir_all(&target_dir).unwrap();
        tempfile::Builder::new().prefix("items-").tempdir_in(target_dir).unwrap()
    }

    fn run_git(path: &Path, args: &[&str]) {
        let output = Command::new("git").args(args).current_dir(path).output().unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init_repository(path: &Path) {
        run_git(path, &["init", "--quiet", "--initial-branch=main"]);
        run_git(path, &["config", "user.email", "test@example.invalid"]);
        run_git(path, &["config", "user.name", "Telemetry Adapter Test"]);
        std::fs::create_dir_all(path.join("track/items")).unwrap();
        std::fs::write(path.join("README.md"), "fixture\n").unwrap();
        run_git(path, &["add", "."]);
        run_git(path, &["commit", "--quiet", "-m", "fixture"]);
    }

    #[test]
    fn test_resolve_project_root_from_items_dir_anchors_relative_root_to_current_directory() {
        if std::env::var_os("SOTP_REPORT_RELATIVE_CHILD").is_some() {
            let repo_name = std::env::var_os("SOTP_REPORT_RELATIVE_ROOT")
                .and_then(|path| Path::new(&path).file_name().map(ToOwned::to_owned))
                .unwrap();
            let root = resolve_project_root_from_items_dir(
                &std::path::PathBuf::from(repo_name).join("track/items"),
            )
            .unwrap();
            let expected =
                std::path::PathBuf::from(std::env::var_os("SOTP_REPORT_RELATIVE_ROOT").unwrap())
                    .canonicalize()
                    .unwrap();
            assert_eq!(root, expected);
            return;
        }

        let stable_parent = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo = tempfile::tempdir_in(stable_parent).unwrap();
        init_repository(repo.path());
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "telemetry::report_adapter::tests::test_resolve_project_root_from_items_dir_anchors_relative_root_to_current_directory",
                "--nocapture",
            ])
            .current_dir(stable_parent)
            .env("SOTP_REPORT_RELATIVE_CHILD", "1")
            .env("SOTP_REPORT_RELATIVE_ROOT", repo.path())
            .status()
            .unwrap();
        assert!(status.success(), "relative report subprocess failed: {status}");
    }

    #[test]
    fn test_resolve_project_root_from_items_dir_anchors_default_path_to_enclosing_repository() {
        if std::env::var_os("SOTP_REPORT_DEFAULT_CHILD").is_some() {
            let root = resolve_project_root_from_items_dir(Path::new("track/items")).unwrap();
            let expected =
                std::path::PathBuf::from(std::env::var_os("SOTP_REPORT_DEFAULT_ROOT").unwrap())
                    .canonicalize()
                    .unwrap();
            assert_eq!(root, expected);
            return;
        }

        let stable_parent = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo = tempfile::tempdir_in(stable_parent).unwrap();
        init_repository(repo.path());
        let ambient_repo = tempfile::tempdir_in(stable_parent).unwrap();
        init_repository(ambient_repo.path());
        let nested_dir = repo.path().join("nested");
        std::fs::create_dir_all(&nested_dir).unwrap();
        std::fs::write(nested_dir.join("track"), "shadow\n").unwrap();
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "telemetry::report_adapter::tests::test_resolve_project_root_from_items_dir_anchors_default_path_to_enclosing_repository",
                "--nocapture",
            ])
            .current_dir(nested_dir)
            .env("SOTP_REPORT_DEFAULT_CHILD", "1")
            .env("SOTP_REPORT_DEFAULT_ROOT", repo.path())
            .env("GIT_DIR", ambient_repo.path().join(".git"))
            .status()
            .unwrap();
        assert!(status.success(), "default report subprocess failed: {status}");
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
    fn resolve_project_root_from_items_dir_rejects_symlinked_ancestor() {
        let repo = crate::git_cli::SystemGitRepo::discover().unwrap();
        let parent = tempfile::tempdir().unwrap();
        let root_link = parent.path().join("repo-link");
        std::os::unix::fs::symlink(repo.root(), &root_link).unwrap();
        let items_dir = root_link.join("track").join("items");

        let err = resolve_project_root_from_items_dir(&items_dir).unwrap_err();

        let message = err.to_string();
        assert!(
            message.contains("refusing to follow symlink")
                || message.contains("refusing to use symlinked repository root"),
            "{message}"
        );
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
    fn resolve_project_root_from_items_dir_rejects_non_repository_root() {
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = tmp.path().join("track").join("items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let err = resolve_project_root_from_items_dir(&items_dir).unwrap_err();

        assert!(err.to_string().contains("git repository"), "{err}");
    }

    #[test]
    fn test_resolve_project_root_from_items_dir_preserves_nested_in_repository_root() {
        let tmp = tempdir_in_current_repo();
        let items_dir = tmp.path().join("track").join("items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let anchor = resolve_project_root_from_items_dir(&items_dir).unwrap();

        assert_eq!(
            anchor,
            tmp.path().canonicalize().unwrap(),
            "a nested in-repository items root must keep its supplied project root"
        );
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
        assert_eq!(*output.skipped_lines.as_ref(), 0);
        assert!(output.review_yield_metrics.is_empty());
    }

    #[test]
    fn aggregate_converts_persisted_command_metrics_to_usecase_output() {
        let tmp = tempdir_in_current_repo();
        let logs_dir = tmp.path().join("some-track").join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        std::fs::write(
            logs_dir.join("telemetry.jsonl"),
            concat!(
                r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"some-track","command":"track plan","exit_code":0,"duration_ms":120,"timestamp":"2026-06-10T00:00:00Z"}"#,
                "\n",
                r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"some-track","command":"track plan","exit_code":17,"duration_ms":80,"timestamp":"2026-06-10T00:00:01Z"}"#,
                "\n"
            ),
        )
        .unwrap();

        let output = FsTelemetryReportAdapter::new().aggregate("some-track", tmp.path()).unwrap();

        assert_eq!(output.command_metrics.len(), 1);
        let metric = output.command_metrics.first().unwrap();
        assert_eq!(metric.command().as_str(), "track plan");
        assert_eq!(*metric.executions().as_ref(), 2);
        assert_eq!(*metric.failures().as_ref(), 1);
        assert_eq!(*metric.total_duration().as_ref(), 200);
        assert_eq!(metric.failure_rate().value(), 5_000);
    }

    #[test]
    fn test_aggregate_projects_review_yield_metrics_to_usecase_output() {
        let tmp = tempdir_in_current_repo();
        let logs_dir = tmp.path().join("some-track").join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        std::fs::write(
            logs_dir.join("telemetry.jsonl"),
            concat!(
                r#"{"event_type":"ReviewRound","schema_version":1,"track_id":"some-track","round":{"scope":"architecture","round_type":"fast","provider":"codex","model":"gpt-5","reasoning_effort":"high","findings_count":1},"duration_ms":120,"timestamp":"2026-06-10T00:00:00Z"}"#,
                "\n",
                r#"{"event_type":"ReviewRound","schema_version":1,"track_id":"some-track","round":{"scope":"architecture","round_type":"fast","provider":"codex","model":"gpt-5","reasoning_effort":"high","findings_count":0},"duration_ms":80,"timestamp":"2026-06-10T00:00:01Z"}"#,
                "\n"
            ),
        )
        .unwrap();

        let output = FsTelemetryReportAdapter::new().aggregate("some-track", tmp.path()).unwrap();

        assert_eq!(output.review_yield_metrics.len(), 5);
        let provider_metric = output
            .review_yield_metrics
            .iter()
            .find(|metric| {
                metric.value
                    == ReviewYieldValue::Provider(
                        usecase::capability_exec::ProviderName::try_new("codex").unwrap(),
                    )
            })
            .unwrap();
        assert_eq!(provider_metric.execution_count.to_string(), "2");
        assert_eq!(provider_metric.detection_rate.to_string(), "5000");
    }

    #[test]
    fn test_telemetry_report_output_aggregation_preserves_all_observed_state() {
        let tmp = tempdir_in_current_repo();
        let track_dir = tmp.path().join("some-track");
        let logs_dir = track_dir.join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        let telemetry_path = logs_dir.join("telemetry.jsonl");
        std::fs::write(
            &telemetry_path,
            concat!(
                r#"{"event_type":"ReviewRound","schema_version":1,"track_id":"some-track","round":{"scope":"architecture","round_type":"fast","provider":"codex","model":"gpt-5","reasoning_effort":"high","findings_count":1},"duration_ms":120,"timestamp":"2026-06-10T00:00:00Z"}"#,
                "\n",
                r#"{"event_type":"ReviewRound","schema_version":1,"track_id":"some-track","round":{"scope":"architecture","round_type":"fast","provider":"codex","model":"gpt-5","reasoning_effort":"high","findings_count":0},"duration_ms":80,"timestamp":"2026-06-10T00:00:01Z"}"#,
                "\n"
            ),
        )
        .unwrap();

        let review_results_path = track_dir.join("review.json");
        std::fs::write(&review_results_path, br#"{"schema_version":2,"scopes":{}}"#).unwrap();
        let configuration_path = tmp.path().join(".harness/config/review-scope.json");
        std::fs::create_dir_all(configuration_path.parent().unwrap()).unwrap();
        std::fs::write(&configuration_path, br#"{"scopes":{"architecture":{"required":true}}}"#)
            .unwrap();

        // The adapter projects a TelemetryReportOutput only; aggregation must not
        // write telemetry, review results, configuration, or inspection state.
        let telemetry_before = std::fs::read(&telemetry_path).unwrap();
        let review_results_before = std::fs::read(&review_results_path).unwrap();
        let configuration_before = std::fs::read(&configuration_path).unwrap();
        let inspection_rounds_before = String::from_utf8_lossy(&telemetry_before)
            .matches("\"event_type\":\"ReviewRound\"")
            .count();

        let output = FsTelemetryReportAdapter::new().aggregate("some-track", tmp.path()).unwrap();

        let telemetry_after = std::fs::read(&telemetry_path).unwrap();
        let review_results_after = std::fs::read(&review_results_path).unwrap();
        let configuration_after = std::fs::read(&configuration_path).unwrap();
        let inspection_rounds_after = String::from_utf8_lossy(&telemetry_after)
            .matches("\"event_type\":\"ReviewRound\"")
            .count();

        assert_eq!(output.review_yield_metrics.len(), 5);
        assert_eq!(telemetry_after, telemetry_before, "aggregation must not mutate telemetry");
        assert_eq!(
            review_results_after, review_results_before,
            "aggregation must not mutate review results"
        );
        assert_eq!(
            configuration_after, configuration_before,
            "aggregation must not mutate configuration"
        );
        assert_eq!(
            inspection_rounds_after, inspection_rounds_before,
            "aggregation must not alter inspection behavior by adding or removing review rounds"
        );
    }

    #[test]
    fn test_emit_active_appends_completed_command_to_existing_track_telemetry_jsonl() {
        let repo = tempfile::tempdir().unwrap();
        init_repository(repo.path());
        run_git(repo.path(), &["switch", "--quiet", "--create", "track/telemetry-test"]);
        let track_id = "telemetry-test";
        let items_dir = repo.path().join("track").join("items");
        let path = items_dir.join(track_id).join("logs").join("telemetry.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"{\"event_type\":\"Existing\"}\n").unwrap();
        let adapter = FsTelemetryEmitDynamicAdapter::new();
        temp_env::with_vars([("SOTP_TELEMETRY", Some("1")), ("SOTP_TELEMETRY_DIR", None)], || {
            adapter
                .emit_active(
                    &items_dir,
                    Some(track_id),
                    "sotp dry".to_owned(),
                    17,
                    240,
                    Some("command failed".to_owned()),
                )
                .unwrap();
        });

        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("\"event_type\":\"Existing\""));
        assert!(content.contains("\"event_type\":\"TrackSubcommand\""));
        assert!(content.contains("\"command\":\"sotp dry\""));
        assert!(content.contains("\"exit_code\":17"));
        assert!(content.contains("\"duration_ms\":240"));
        assert!(content.contains("\"event_type\":\"NonZeroExit\""));
        assert!(content.contains("command failed"));
    }

    #[test]
    fn test_emit_active_rejects_malformed_items_dir() {
        let adapter = FsTelemetryEmitDynamicAdapter::new();
        let error = adapter
            .emit_active(
                Path::new("wrong/path"),
                Some("track-id"),
                "sotp dry".to_owned(),
                0,
                1,
                None,
            )
            .unwrap_err();

        assert!(matches!(error, TelemetryEmitDynamicPortError::EmitUnavailable(_)));
        assert!(error.to_string().contains("--items-dir"));
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
