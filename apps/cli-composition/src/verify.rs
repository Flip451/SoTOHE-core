//! `verify` command family — CliApp impl methods.

use std::path::PathBuf;

use crate::{CliApp, CommandOutcome};

/// Render a `VerifyOutcome` into a `CommandOutcome`.
fn render_outcome(label: &str, outcome: &infrastructure::verify::VerifyOutcome) -> CommandOutcome {
    let mut lines = vec![format!("--- {label} ---")];
    if outcome.findings().is_empty() {
        lines.push("[OK] All checks passed.".to_owned());
        lines.push(format!("--- {label} PASSED ---"));
        CommandOutcome::success(Some(lines.join("\n")))
    } else {
        for finding in outcome.findings() {
            lines.push(finding.to_string());
        }
        if outcome.has_errors() {
            lines.push(format!("--- {label} FAILED ---"));
            CommandOutcome { stdout: Some(lines.join("\n")), stderr: None, exit_code: 1 }
        } else {
            lines.push(format!("--- {label} PASSED ---"));
            CommandOutcome::success(Some(lines.join("\n")))
        }
    }
}

/// Render a skip outcome (non-track branch, AC-16).
fn render_skip(label: &str, reason: &str) -> CommandOutcome {
    let stdout = format!("--- {label} ---\n[SKIP] {reason}\n--- {label} SKIPPED ---");
    CommandOutcome::success(Some(stdout))
}

/// Resolve the active track id for CI verify subcommands.
///
/// Returns:
/// - `Ok(Some(track_id))` when on a valid `track/<id>` branch.
/// - `Ok(None)` when on a non-track branch (skip path — AC-16).
/// - `Err(msg)` for real infrastructure failures (fail-closed).
fn resolve_ci_verify_track_id() -> Result<Option<String>, String> {
    use std::sync::Arc;

    let repo = infrastructure::git_cli::SystemGitRepo::discover()
        .map_err(|e| format!("cannot discover git repository: {e}"))?;
    resolve_ci_verify_track_id_with_reader(Arc::new(repo))
}

fn resolve_ci_verify_track_id_from_root(
    workspace_root: &std::path::Path,
) -> Result<Option<String>, String> {
    use std::sync::Arc;

    let repo = infrastructure::git_cli::SystemGitRepo::discover_from(workspace_root)
        .map_err(|e| format!("cannot discover git repository: {e}"))?;
    resolve_ci_verify_track_id_with_reader(Arc::new(repo))
}

fn resolve_ci_verify_track_id_with_reader(
    branch_reader: std::sync::Arc<dyn usecase::track_resolution::BranchReaderPort>,
) -> Result<Option<String>, String> {
    use usecase::track_resolution::{
        ActiveTrackResolveError, ActiveTrackResolveInteractor, ActiveTrackResolveService as _,
        TrackResolutionError,
    };

    let interactor = ActiveTrackResolveInteractor::new(branch_reader);
    match interactor.resolve_active_track() {
        Ok(track_id) => Ok(Some(track_id)),
        Err(ActiveTrackResolveError::Resolution(
            TrackResolutionError::NotTrackBranch(_)
            | TrackResolutionError::DetachedHead
            | TrackResolutionError::NoBranch,
        )) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Build the `spec.md` path from a resolved track ID, with presence checking.
///
/// Returns:
/// - `Ok(Some(spec_md_path))` when at least one of `spec.json` or `spec.md` exists under
///   `track/items/<track_id>/`. The returned path is always `spec.md`; the downstream
///   `infrastructure::verify::spec_states::verify` prefers the sibling `spec.json` when
///   present and falls back to reading `spec.md`, so passing the `spec.md` path is correct
///   as long as at least one of the two exists.
/// - `Ok(None)` (skip-signal) when the track artifact directory exists but **neither**
///   `spec.json` **nor** `spec.md` exists — the spec artifacts have not yet been generated
///   (Phase 0). Callers should render a SKIP outcome.
/// - `Err(msg)` for infrastructure failures (fail-closed).
fn build_spec_path_from_track_id(track_id: &str) -> Result<Option<PathBuf>, String> {
    use infrastructure::git_cli::GitRepository as _;
    let repo = infrastructure::git_cli::SystemGitRepo::discover()
        .map_err(|e| format!("cannot discover git repository: {e}"))?;
    let repo_root = repo.root().to_path_buf();
    let track_dir = repo_root.join("track/items").join(track_id);
    require_track_artifact_dir(track_id, &track_dir)?;
    let spec_json_path = track_dir.join("spec.json");
    let spec_md_path = track_dir.join("spec.md");
    if spec_artifact_entry_exists(&spec_json_path)? || spec_artifact_entry_exists(&spec_md_path)? {
        Ok(Some(spec_md_path))
    } else {
        // Neither spec artifact exists yet (Phase 0) — return skip-signal.
        Ok(None)
    }
}

fn require_track_artifact_dir(track_id: &str, track_dir: &std::path::Path) -> Result<(), String> {
    match std::fs::symlink_metadata(track_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "track artifact directory {} for track '{track_id}' is a symlink",
            track_dir.display()
        )),
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(format!(
            "track artifact path {} for track '{track_id}' is not a directory",
            track_dir.display()
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(format!(
            "track artifact directory {} for track '{track_id}' does not exist",
            track_dir.display()
        )),
        Err(e) => Err(format!(
            "failed to inspect track artifact directory {} for track '{track_id}': {e}",
            track_dir.display()
        )),
    }
}

fn spec_artifact_entry_exists(path: &std::path::Path) -> Result<bool, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "spec artifact {} is a symlink; refusing to skip spec-state verification",
            path.display()
        )),
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => Err(format!(
            "spec artifact {} is not a regular file; refusing to skip spec-state verification",
            path.display()
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("failed to inspect spec artifact {}: {e}", path.display())),
    }
}

fn resolve_active_track_dir() -> Option<PathBuf> {
    use std::sync::Arc;

    use infrastructure::git_cli::GitRepository as _;
    use usecase::track_resolution::{ActiveTrackResolveInteractor, ActiveTrackResolveService as _};
    let repo = infrastructure::git_cli::SystemGitRepo::discover().ok()?;
    let repo_root = repo.root().to_path_buf();
    let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
    let track_id = interactor.resolve_active_track().ok()?;
    let track_dir = repo_root.join("track/items").join(&track_id);
    if track_dir.is_dir() { Some(track_dir) } else { None }
}

/// Execute catalogue-spec-refs verification, returning a `CommandOutcome`.
///
/// Validates the track id, delegates I/O to the infrastructure layer,
/// and maps the result to a `CommandOutcome`.
pub(crate) fn execute_catalogue_spec_refs(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    skip_stale: bool,
) -> Result<CommandOutcome, String> {
    // Validate track id (path traversal guard) without importing domain types (CN-01 / AC-03).
    validate_track_id_str_local(&track_id).map_err(|e| format!("invalid track ID: {e}"))?;

    let mut all_formatted_findings: Vec<String> = Vec::new();
    let no_findings =
        infrastructure::verify::catalogue_spec_refs::execute_verify_catalogue_spec_refs(
            items_dir,
            track_id,
            workspace_root,
            skip_stale,
            &mut all_formatted_findings,
        )
        .map_err(|e| e.to_string())?;

    if no_findings {
        Ok(CommandOutcome::success(Some("[OK] catalogue-spec-refs: no findings".to_owned())))
    } else {
        let stderr = all_formatted_findings
            .iter()
            .chain(std::iter::once(&format!(
                "[FAIL] catalogue-spec-refs: {} finding(s)",
                all_formatted_findings.len()
            )))
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        Ok(CommandOutcome { stdout: None, stderr: Some(stderr), exit_code: 1 })
    }
}

/// Validates a track ID string without importing domain types.
fn validate_track_id_str_local(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("invalid track id: '{value}' (must not be empty)"));
    }
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => {
            return Err(format!(
                "invalid track id: '{value}' (must start with lowercase letter or digit)"
            ));
        }
    }
    let mut previous_was_hyphen = false;
    for ch in chars {
        let is_valid = ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-';
        if !is_valid {
            return Err(format!("invalid track id: '{value}' (invalid character '{ch}')"));
        }
        if ch == '-' && previous_was_hyphen {
            return Err(format!("invalid track id: '{value}' (double hyphen not allowed)"));
        }
        previous_was_hyphen = ch == '-';
    }
    if previous_was_hyphen {
        return Err(format!("invalid track id: '{value}' (must not end with hyphen)"));
    }
    Ok(())
}

impl CliApp {
    /// Check tech-stack.md for unresolved TODO markers.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_tech_stack(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::tech_stack::verify(&project_root);
        Ok(render_outcome("verify tech stack readiness", &outcome))
    }

    /// Check latest track artifacts for completeness.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_latest_track(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::latest_track::verify(&project_root);
        Ok(render_outcome("verify latest track files", &outcome))
    }

    /// Check architecture docs synchronization and text patterns.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_arch_docs(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let mut outcome = infrastructure::verify::architecture_rules::verify(&project_root);
        outcome.merge(infrastructure::verify::doc_patterns::verify(&project_root));
        outcome.merge(infrastructure::verify::convention_docs::verify(&project_root));
        Ok(render_outcome("verify architecture docs", &outcome))
    }

    /// Check workspace layer dependency rules via cargo metadata.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_layers(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::layers::verify(&project_root);
        Ok(render_outcome("verify layers", &outcome))
    }

    /// Check .claude/settings.json structural guardrails.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_orchestra(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::orchestra::verify(&project_root);
        Ok(render_outcome("verify orchestra guardrails", &outcome))
    }

    /// Check spec.md requirement lines for `[source: ...]` attribution.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_spec_attribution(&self, spec_path: PathBuf) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::spec_attribution::verify(&spec_path);
        Ok(render_outcome("verify spec attribution", &outcome))
    }

    /// Check spec.md YAML frontmatter for required fields.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_spec_frontmatter(&self, spec_path: PathBuf) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::spec_frontmatter::verify(&spec_path);
        Ok(render_outcome("verify spec frontmatter", &outcome))
    }

    /// Check canonical module ownership.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_canonical_modules(
        &self,
        project_root: PathBuf,
    ) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::canonical_modules::verify(&project_root);
        Ok(render_outcome("verify canonical modules", &outcome))
    }

    /// Check Rust source file sizes against module_limits thresholds.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_module_size(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::module_size::verify(&project_root);
        Ok(render_outcome("verify module size", &outcome))
    }

    /// Check libs/domain/src/ for hexagonal purity violations.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_domain_purity(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::domain_purity::verify(&project_root);
        Ok(render_outcome("verify domain purity", &outcome))
    }

    /// Check libs/domain/src/ for pub String fields (should be enums or newtypes).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_domain_strings(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::domain_strings::verify(&project_root);
        Ok(render_outcome("verify domain strings", &outcome))
    }

    /// Check libs/usecase/src/ for hexagonal purity violations.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_usecase_purity(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::usecase_purity::verify(&project_root);
        Ok(render_outcome("verify usecase purity", &outcome))
    }

    /// Check that local file links in Markdown documents resolve to existing files.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_doc_links(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::doc_links::verify(&project_root);
        Ok(render_outcome("verify doc links", &outcome))
    }

    /// Check that plan.md files are up-to-date with metadata.json renderings.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_view_freshness(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::view_freshness::verify(&project_root);
        Ok(render_outcome("verify view freshness", &outcome))
    }

    /// Check spec.md source tag signals match frontmatter and red == 0 gate.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_spec_signals(&self, spec_path: PathBuf) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::spec_signals::verify(&spec_path);
        Ok(render_outcome("verify spec signals", &outcome))
    }

    /// Check spec.md contains a Domain States section with table data rows.
    ///
    /// When `spec_path` is `None`, resolves from the active track branch (AC-16: skip on
    /// non-track branches).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_spec_states(
        &self,
        spec_path: Option<PathBuf>,
        strict: bool,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::verify::VerifyFinding;

        let resolved_path = match spec_path {
            Some(p) => p,
            None => match resolve_ci_verify_track_id()? {
                None => {
                    return Ok(render_skip(
                        "verify spec states",
                        "not on a track branch; skipping",
                    ));
                }
                Some(track_id) => match build_spec_path_from_track_id(&track_id)? {
                    Some(path) => path,
                    None => {
                        return Ok(render_skip(
                            "verify spec states",
                            "spec artifacts not yet generated (Phase 0); skipping",
                        ));
                    }
                },
            },
        };

        let outcome =
            match infrastructure::verify::trusted_root::resolve_trusted_root(&resolved_path) {
                Ok(trusted_root) => infrastructure::verify::spec_states::verify(
                    &resolved_path,
                    strict,
                    &trusted_root,
                ),
                Err(e) => infrastructure::verify::VerifyOutcome::from_findings(vec![
                    VerifyFinding::error(format!(
                        "cannot resolve trusted_root for {}: {e}",
                        resolved_path.display()
                    )),
                ]),
            };
        Ok(render_outcome("verify spec states", &outcome))
    }

    /// Validate structured-ref fields per ADR 2026-04-19-1242.
    ///
    /// When `track_dir` is `None`, resolves from the active track branch (AC-16: skip on
    /// non-track branches).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_plan_artifact_refs(
        &self,
        track_dir: Option<PathBuf>,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::verify::VerifyFinding;

        if track_dir.is_none() && resolve_ci_verify_track_id()?.is_none() {
            return Ok(render_skip("verify plan artifact refs", "not on a track branch; skipping"));
        }

        let outcome = match &track_dir {
            Some(dir) if dir.is_dir() => infrastructure::verify::plan_artifact_refs::verify(dir),
            Some(dir) => {
                infrastructure::verify::VerifyOutcome::from_findings(vec![VerifyFinding::error(
                    format!("Track directory does not exist: {}", dir.display()),
                )])
            }
            None => match resolve_active_track_dir() {
                Some(dir) => infrastructure::verify::plan_artifact_refs::verify(&dir),
                None => infrastructure::verify::VerifyOutcome::from_findings(vec![
                    VerifyFinding::error(
                        "Cannot resolve active track directory: not on a track/* branch or \
                         directory does not exist. Use --track-dir <PATH> to specify the track \
                         directory explicitly."
                            .to_owned(),
                    ),
                ]),
            },
        };
        Ok(render_outcome("verify plan artifact refs", &outcome))
    }

    /// Verify catalogue-spec ref integrity (SoT Chain binary gate).
    ///
    /// When `track_id` is `None`, resolves from the active track branch (AC-16: skip on
    /// non-track branches).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_catalogue_spec_refs(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
        workspace_root: PathBuf,
        skip_stale: bool,
    ) -> Result<CommandOutcome, String> {
        if track_id.is_none() && resolve_ci_verify_track_id_from_root(&workspace_root)?.is_none() {
            return Ok(render_skip(
                "verify catalogue-spec-refs",
                "not on a track branch; skipping",
            ));
        }

        let resolved_track_id = match track_id {
            Some(id) => id,
            None => {
                use crate::track::resolve_track_id_from_root;
                resolve_track_id_from_root(None, &workspace_root)?
            }
        };

        execute_catalogue_spec_refs(items_dir, resolved_track_id, workspace_root, skip_stale)
    }

    /// Check catalogue-spec signal gate results for each tddd-enabled layer.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_catalogue_spec_signals(
        &self,
        items_dir: PathBuf,
        workspace_root: PathBuf,
        strict: bool,
    ) -> Result<CommandOutcome, String> {
        // AC-16: check for non-track branch at the composition layer before delegating
        // to infrastructure (which has its own track resolution).
        //
        // Uses resolve_ci_verify_track_id() (CWD-anchored) to match the behaviour of
        // execute_catalogue_spec_signals_check(), which also discovers the git repo from
        // the process CWD via SystemGitRepo::discover().
        // Using the workspace_root-anchored variant here while the check resolves from
        // CWD would create a mismatch when --workspace-root differs from CWD.
        match resolve_ci_verify_track_id()? {
            None => {
                Ok(render_skip("verify catalogue-spec signals", "not on a track branch; skipping"))
            }
            Some(_) => {
                let outcome = infrastructure::verify::catalogue_spec_signals::execute_catalogue_spec_signals_check(
                    items_dir,
                    workspace_root,
                    strict,
                );
                Ok(render_outcome("verify catalogue-spec signals", &outcome))
            }
        }
    }

    /// Verify ADR decision signal grounds across knowledge/adr/.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn verify_adr_signals(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let outcome =
            infrastructure::verify::adr_signals::execute_verify_adr_signals(&project_root);
        Ok(render_outcome("verify adr signals", &outcome))
    }

    // -----------------------------------------------------------------------
    // CI verify helpers — exposed so apps/cli can avoid direct infra imports
    // -----------------------------------------------------------------------

    /// Resolve the active track ID for CI verify subcommands (CWD-anchored).
    ///
    /// - `Ok(Some(id))` → on a valid track branch.
    /// - `Ok(None)`     → on a non-track branch (skip, AC-16).
    /// - `Err(msg)`     → infrastructure failure (fail-closed).
    ///
    /// # Errors
    /// Returns a human-readable error string for non-skip failures.
    pub fn verify_ci_resolve_track_id(&self) -> Result<Option<String>, String> {
        resolve_ci_verify_track_id()
    }

    /// Resolve the active track ID using workspace_root for git discovery.
    ///
    /// # Errors
    /// Returns a human-readable error string for non-skip failures.
    pub fn verify_ci_resolve_track_id_from_root(
        &self,
        workspace_root: PathBuf,
    ) -> Result<Option<String>, String> {
        resolve_ci_verify_track_id_from_root(&workspace_root)
    }

    /// Resolve the active track directory from the current git branch.
    ///
    /// Returns `None` when not in a git repo, not on a track branch,
    /// or the resolved directory does not exist.
    pub fn verify_resolve_active_track_dir(&self) -> Option<PathBuf> {
        resolve_active_track_dir()
    }

    /// Build the spec.md path from a resolved track ID.
    ///
    /// Returns `Ok(path)` when at least one of `spec.json` or `spec.md` exists.
    /// Returns `Err` when neither spec artifact exists (Phase 0) or on infrastructure failure.
    /// Use `verify_spec_states` for the track-resolution path that emits a SKIP outcome
    /// instead of an error when spec artifacts are absent.
    ///
    /// # Errors
    /// Returns a human-readable error string when neither spec artifact exists or on
    /// infrastructure failure.
    pub fn verify_build_spec_path_from_track_id(&self, track_id: &str) -> Result<PathBuf, String> {
        match build_spec_path_from_track_id(track_id)? {
            Some(path) => Ok(path),
            None => Err(format!(
                "spec artifacts not found for track '{track_id}': \
                 neither spec.json nor spec.md exists under track/items/{track_id}/"
            )),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_artifact_entry_exists_missing_path_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing-spec.json");

        assert!(!spec_artifact_entry_exists(&path).unwrap());
    }

    #[test]
    fn test_spec_artifact_entry_exists_existing_file_returns_true() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.json");
        std::fs::write(&path, "{}").unwrap();

        assert!(spec_artifact_entry_exists(&path).unwrap());
    }

    #[test]
    fn test_spec_artifact_entry_exists_directory_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.json");
        std::fs::create_dir(&path).unwrap();

        let message = spec_artifact_entry_exists(&path).unwrap_err();

        assert!(
            message.contains("is not a regular file"),
            "artifact directories must fail closed, got: {message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_spec_artifact_entry_exists_dangling_symlink_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("missing-spec.json");
        let link = dir.path().join("spec.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let message = spec_artifact_entry_exists(&link).unwrap_err();

        assert!(
            message.contains("is a symlink"),
            "dangling artifact symlinks must fail closed, got: {message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_spec_artifact_entry_exists_not_directory_error_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let not_dir = dir.path().join("not-dir");
        std::fs::write(&not_dir, "not a directory").unwrap();
        let path = not_dir.join("spec.json");
        let raw_error = std::fs::symlink_metadata(&path).unwrap_err();
        assert_eq!(
            raw_error.kind(),
            std::io::ErrorKind::NotADirectory,
            "test setup must trigger a metadata error, not an absent artifact"
        );

        let message = spec_artifact_entry_exists(&path).unwrap_err();

        assert!(
            message.contains("failed to inspect spec artifact"),
            "metadata errors must fail closed, got: {message}"
        );
    }

    #[test]
    fn test_require_track_artifact_dir_missing_dir_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("missing-track");

        let message = require_track_artifact_dir("missing-track", &track_dir).unwrap_err();
        assert!(
            message.contains("does not exist"),
            "missing track artifact directory must fail closed, got: {message}"
        );
    }

    #[test]
    fn test_require_track_artifact_dir_empty_dir_allows_phase0_skip() {
        let dir = tempfile::tempdir().unwrap();

        require_track_artifact_dir("phase0-track", dir.path()).unwrap();
    }
}
