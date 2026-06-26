//! Filesystem-backed adapter implementing [`usecase::verify::VerifyPort`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use usecase::verify::{VerifyOutcome, VerifyPort, VerifyPortError};

use crate::verify::VerifyOutcome as InfraVerifyOutcome;

// ---------------------------------------------------------------------------
// Render helper (mirrors cli_composition::cmd_outcome::render_outcome)
// ---------------------------------------------------------------------------

fn render_outcome(label: &str, outcome: &InfraVerifyOutcome) -> VerifyOutcome {
    let mut lines = vec![format!("--- {label} ---")];
    if outcome.findings().is_empty() {
        lines.push("[OK] All checks passed.".to_owned());
        lines.push(format!("--- {label} PASSED ---"));
        VerifyOutcome::success(Some(lines.join("\n")))
    } else {
        for finding in outcome.findings() {
            lines.push(finding.to_string());
        }
        if outcome.has_errors() {
            lines.push(format!("--- {label} FAILED ---"));
            VerifyOutcome { stdout: Some(lines.join("\n")), stderr: None, exit_code: 1 }
        } else {
            lines.push(format!("--- {label} PASSED ---"));
            VerifyOutcome::success(Some(lines.join("\n")))
        }
    }
}

fn render_skip(label: &str, reason: &str) -> VerifyOutcome {
    let stdout = format!("--- {label} ---\n[SKIP] {reason}\n--- {label} SKIPPED ---");
    VerifyOutcome::success(Some(stdout))
}

fn reject_symlinked_trusted_root(label: &str, root: &Path) -> Option<VerifyOutcome> {
    crate::verify::trusted_root::ensure_not_symlink_root(root.to_path_buf()).err().map(|e| {
        VerifyOutcome {
            stdout: None,
            stderr: Some(format!(
                "{label}: trusted root rejected before verification at '{}': {e}",
                root.display()
            )),
            exit_code: 1,
        }
    })
}

fn reject_unsafe_items_dir(
    label: &str,
    items_dir: &Path,
    workspace_root: &Path,
) -> Option<VerifyOutcome> {
    let absolute_workspace_root = crate::verify::path_safety::lexical_normalize(
        &crate::verify::trusted_root::absolutize(workspace_root),
    );
    let absolute_items_dir =
        crate::verify::path_safety::lexical_normalize(&if items_dir.is_absolute() {
            items_dir.to_path_buf()
        } else {
            absolute_workspace_root.join(items_dir)
        });

    if !absolute_items_dir.starts_with(&absolute_workspace_root) {
        return Some(VerifyOutcome {
            stdout: None,
            stderr: Some(format!(
                "{label}: items_dir '{}' resolves outside workspace root '{}'",
                items_dir.display(),
                workspace_root.display()
            )),
            exit_code: 1,
        });
    }

    match crate::track::symlink_guard::reject_symlinks_below(
        &absolute_items_dir,
        &absolute_workspace_root,
    ) {
        Ok(_) => None,
        Err(e) => Some(VerifyOutcome {
            stdout: None,
            stderr: Some(format!(
                "{label}: items_dir rejected before verification at '{}': {e}",
                items_dir.display()
            )),
            exit_code: 1,
        }),
    }
}

fn sibling_spec_json(spec_path: &Path) -> Option<PathBuf> {
    spec_path
        .parent()
        .map(|dir| if dir.as_os_str().is_empty() { Path::new(".") } else { dir })
        .map(|dir| dir.join("spec.json"))
}

fn reject_unsafe_sibling_spec_json(label: &str, spec_path: &Path) -> Option<VerifyOutcome> {
    let spec_json_path = sibling_spec_json(spec_path)?;
    let absolute_spec_json = crate::verify::path_safety::lexical_normalize(
        &crate::verify::trusted_root::absolutize(&spec_json_path),
    );

    let trusted_root = match crate::verify::trusted_root::resolve_trusted_root(&absolute_spec_json)
    {
        Ok(root) => crate::verify::path_safety::lexical_normalize(&root),
        Err(e) => {
            return Some(VerifyOutcome {
                stdout: None,
                stderr: Some(format!(
                    "{label}: trusted root resolution failed before sibling spec.json check at '{}': {e}",
                    spec_json_path.display()
                )),
                exit_code: 1,
            });
        }
    };

    if !absolute_spec_json.starts_with(&trusted_root) {
        return Some(VerifyOutcome {
            stdout: None,
            stderr: Some(format!(
                "{label}: sibling spec.json '{}' resolves outside trusted root '{}'",
                spec_json_path.display(),
                trusted_root.display()
            )),
            exit_code: 1,
        });
    }

    match crate::track::symlink_guard::reject_symlinks_below(&absolute_spec_json, &trusted_root) {
        Ok(_) => None,
        Err(e) => Some(VerifyOutcome {
            stdout: None,
            stderr: Some(format!(
                "{label}: sibling spec.json rejected before verification at '{}': {e}",
                spec_json_path.display()
            )),
            exit_code: 1,
        }),
    }
}

fn reject_unsafe_spec_markdown(label: &str, spec_path: &Path) -> Option<VerifyOutcome> {
    let absolute_spec = crate::verify::path_safety::lexical_normalize(
        &crate::verify::trusted_root::absolutize(spec_path),
    );

    let trusted_root = match crate::verify::trusted_root::resolve_trusted_root(&absolute_spec) {
        Ok(root) => crate::verify::path_safety::lexical_normalize(&root),
        Err(e) => {
            return Some(VerifyOutcome {
                stdout: None,
                stderr: Some(format!(
                    "{label}: trusted root resolution failed before spec.md check at '{}': {e}",
                    spec_path.display()
                )),
                exit_code: 1,
            });
        }
    };

    if !absolute_spec.starts_with(&trusted_root) {
        return Some(VerifyOutcome {
            stdout: None,
            stderr: Some(format!(
                "{label}: spec.md '{}' resolves outside trusted root '{}'",
                spec_path.display(),
                trusted_root.display()
            )),
            exit_code: 1,
        });
    }

    match crate::track::symlink_guard::reject_symlinks_below(&absolute_spec, &trusted_root) {
        Ok(_) => None,
        Err(e) => Some(VerifyOutcome {
            stdout: None,
            stderr: Some(format!(
                "{label}: spec.md rejected before verification at '{}': {e}",
                spec_path.display()
            )),
            exit_code: 1,
        }),
    }
}

// ---------------------------------------------------------------------------
// Git-resolution helpers (mirrors cli_composition/src/verify.rs)
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct VerifyAdapterError(String);

impl std::fmt::Display for VerifyAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

fn resolve_ci_verify_track_id() -> Result<Option<String>, VerifyAdapterError> {
    let repo = crate::git_cli::SystemGitRepo::discover()
        .map_err(|e| VerifyAdapterError(format!("cannot discover git repository: {e}")))?;
    resolve_ci_verify_track_id_with_reader(Arc::new(repo))
}

fn resolve_ci_verify_track_id_from_root(
    workspace_root: &std::path::Path,
) -> Result<Option<String>, VerifyAdapterError> {
    let repo = crate::git_cli::SystemGitRepo::discover_from(workspace_root)
        .map_err(|e| VerifyAdapterError(format!("cannot discover git repository: {e}")))?;
    resolve_ci_verify_track_id_with_reader(Arc::new(repo))
}

fn resolve_ci_verify_track_id_with_reader(
    branch_reader: Arc<dyn usecase::track_resolution::BranchReaderPort>,
) -> Result<Option<String>, VerifyAdapterError> {
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
        Err(e) => Err(VerifyAdapterError(e.to_string())),
    }
}

fn resolve_active_track_dir() -> Option<PathBuf> {
    use crate::git_cli::GitRepository as _;
    use usecase::track_resolution::{ActiveTrackResolveInteractor, ActiveTrackResolveService as _};
    let repo = crate::git_cli::SystemGitRepo::discover().ok()?;
    let repo_root = repo.root().to_path_buf();
    let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
    let track_id = interactor.resolve_active_track().ok()?;
    let track_dir = repo_root.join("track/items").join(&track_id);
    if track_dir.is_dir() { Some(track_dir) } else { None }
}

// ---------------------------------------------------------------------------
// FsVerifyAdapter
// ---------------------------------------------------------------------------

/// Filesystem-backed adapter implementing [`VerifyPort`].
///
/// Delegates each method to the appropriate `infrastructure::verify::*` submodule.
#[derive(Debug, Default)]
pub struct FsVerifyAdapter;

impl FsVerifyAdapter {
    /// Create a new `FsVerifyAdapter`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl VerifyPort for FsVerifyAdapter {
    fn verify_tech_stack(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) =
            reject_symlinked_trusted_root("verify tech stack readiness", project_root)
        {
            return Ok(outcome);
        }

        Ok(render_outcome(
            "verify tech stack readiness",
            &crate::verify::tech_stack::verify(project_root),
        ))
    }

    fn verify_latest_track(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) =
            reject_symlinked_trusted_root("verify latest track files", project_root)
        {
            return Ok(outcome);
        }

        Ok(render_outcome(
            "verify latest track files",
            &crate::verify::latest_track::verify(project_root),
        ))
    }

    fn verify_arch_docs(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) =
            reject_symlinked_trusted_root("verify architecture docs", project_root)
        {
            return Ok(outcome);
        }

        let mut outcome = crate::verify::architecture_rules::verify(project_root);
        outcome.merge(crate::verify::doc_patterns::verify(project_root));
        outcome.merge(crate::conventions::verify_convention_index(project_root));
        Ok(render_outcome("verify architecture docs", &outcome))
    }

    fn verify_layers(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) = reject_symlinked_trusted_root("verify layers", project_root) {
            return Ok(outcome);
        }

        Ok(render_outcome("verify layers", &crate::verify::layers::verify(project_root)))
    }

    fn verify_hooks_path(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) = reject_symlinked_trusted_root("verify hooks path", project_root) {
            return Ok(outcome);
        }

        Ok(render_outcome("verify hooks path", &crate::verify::hooks_path::verify(project_root)))
    }

    fn verify_spec_attribution(&self, spec_path: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) = reject_unsafe_sibling_spec_json("verify spec attribution", spec_path)
        {
            return Ok(outcome);
        }

        Ok(render_outcome(
            "verify spec attribution",
            &crate::verify::spec_attribution::verify(spec_path),
        ))
    }

    fn verify_spec_frontmatter(&self, spec_path: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) = reject_unsafe_sibling_spec_json("verify spec frontmatter", spec_path)
        {
            return Ok(outcome);
        }
        if let Some(outcome) = reject_unsafe_spec_markdown("verify spec frontmatter", spec_path) {
            return Ok(outcome);
        }

        Ok(render_outcome(
            "verify spec frontmatter",
            &crate::verify::spec_frontmatter::verify(spec_path),
        ))
    }

    fn verify_canonical_modules(
        &self,
        project_root: &Path,
    ) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) =
            reject_symlinked_trusted_root("verify canonical modules", project_root)
        {
            return Ok(outcome);
        }

        Ok(render_outcome(
            "verify canonical modules",
            &crate::verify::canonical_modules::verify(project_root),
        ))
    }

    fn verify_doc_hidden(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) = reject_symlinked_trusted_root("verify doc-hidden", project_root) {
            return Ok(outcome);
        }

        Ok(render_outcome("verify doc-hidden", &crate::verify::doc_hidden::verify(project_root)))
    }

    fn verify_module_size(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) = reject_symlinked_trusted_root("verify module size", project_root) {
            return Ok(outcome);
        }

        Ok(render_outcome("verify module size", &crate::verify::module_size::verify(project_root)))
    }

    fn verify_domain_purity(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) = reject_symlinked_trusted_root("verify domain purity", project_root) {
            return Ok(outcome);
        }

        Ok(render_outcome(
            "verify domain purity",
            &crate::verify::domain_purity::verify(project_root),
        ))
    }

    fn verify_domain_strings(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) = reject_symlinked_trusted_root("verify domain strings", project_root)
        {
            return Ok(outcome);
        }

        Ok(render_outcome(
            "verify domain strings",
            &crate::verify::domain_strings::verify(project_root),
        ))
    }

    fn verify_usecase_purity(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) = reject_symlinked_trusted_root("verify usecase purity", project_root)
        {
            return Ok(outcome);
        }

        Ok(render_outcome(
            "verify usecase purity",
            &crate::verify::usecase_purity::verify(project_root),
        ))
    }

    fn verify_doc_links(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) = reject_symlinked_trusted_root("verify doc links", project_root) {
            return Ok(outcome);
        }

        Ok(render_outcome("verify doc links", &crate::verify::doc_links::verify(project_root)))
    }

    fn verify_view_freshness(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) = reject_symlinked_trusted_root("verify view freshness", project_root)
        {
            return Ok(outcome);
        }

        Ok(render_outcome(
            "verify view freshness",
            &crate::verify::view_freshness::verify(project_root),
        ))
    }

    fn verify_spec_signals(&self, spec_path: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) = reject_unsafe_sibling_spec_json("verify spec signals", spec_path) {
            return Ok(outcome);
        }

        Ok(render_outcome("verify spec signals", &crate::verify::spec_signals::verify(spec_path)))
    }

    fn verify_plan_artifact_refs(
        &self,
        track_dir: Option<&Path>,
    ) -> Result<VerifyOutcome, VerifyPortError> {
        use crate::verify::VerifyFinding;

        if track_dir.is_none() {
            match resolve_ci_verify_track_id() {
                Ok(None) => {
                    return Ok(render_skip(
                        "verify plan artifact refs",
                        "not on a track branch; skipping",
                    ));
                }
                Ok(Some(_)) => {}
                Err(e) => {
                    return Ok(VerifyOutcome {
                        stdout: None,
                        stderr: Some(format!(
                            "verify plan artifact refs: git resolution failed: {e}"
                        )),
                        exit_code: 1,
                    });
                }
            }
        }

        let outcome = match track_dir {
            Some(dir) if dir.is_dir() => crate::verify::plan_artifact_refs::verify(dir),
            Some(dir) => crate::verify::VerifyOutcome::from_findings(vec![VerifyFinding::error(
                format!("Track directory does not exist: {}", dir.display()),
            )]),
            None => match resolve_active_track_dir() {
                Some(dir) => crate::verify::plan_artifact_refs::verify(&dir),
                None => crate::verify::VerifyOutcome::from_findings(vec![VerifyFinding::error(
                    "Cannot resolve active track directory: not on a track/* branch or \
                     directory does not exist. Use --track-dir <PATH> to specify the track \
                     directory explicitly."
                        .to_owned(),
                )]),
            },
        };
        Ok(render_outcome("verify plan artifact refs", &outcome))
    }

    fn verify_catalogue_spec_refs(
        &self,
        track_id: Option<&str>,
        items_dir: &Path,
        workspace_root: &Path,
        skip_stale: bool,
    ) -> Result<VerifyOutcome, VerifyPortError> {
        if let Some(outcome) =
            reject_symlinked_trusted_root("verify catalogue-spec-refs", workspace_root)
        {
            return Ok(outcome);
        }
        if let Some(outcome) =
            reject_symlinked_trusted_root("verify catalogue-spec-refs", items_dir)
        {
            return Ok(outcome);
        }
        if let Some(outcome) =
            reject_unsafe_items_dir("verify catalogue-spec-refs", items_dir, workspace_root)
        {
            return Ok(outcome);
        }

        if track_id.is_none() {
            match resolve_ci_verify_track_id_from_root(workspace_root) {
                Ok(None) => {
                    return Ok(render_skip(
                        "verify catalogue-spec-refs",
                        "not on a track branch; skipping",
                    ));
                }
                Ok(Some(_)) => {}
                Err(e) => {
                    return Ok(VerifyOutcome {
                        stdout: None,
                        stderr: Some(format!(
                            "verify catalogue-spec-refs: git resolution failed: {e}"
                        )),
                        exit_code: 1,
                    });
                }
            }
        }

        let resolved_id: String = match track_id {
            Some(id) => id.to_owned(),
            None => {
                // Resolve from git (already checked above that we're on a track branch).
                match resolve_ci_verify_track_id_from_root(workspace_root) {
                    Ok(Some(id)) => id,
                    Ok(None) => {
                        return Ok(render_skip(
                            "verify catalogue-spec-refs",
                            "not on a track branch; skipping",
                        ));
                    }
                    Err(e) => {
                        return Ok(VerifyOutcome {
                            stdout: None,
                            stderr: Some(format!(
                                "verify catalogue-spec-refs: git resolution failed: {e}"
                            )),
                            exit_code: 1,
                        });
                    }
                }
            }
        };

        let mut all_formatted_findings: Vec<String> = Vec::new();
        match crate::verify::catalogue_spec_refs::execute_verify_catalogue_spec_refs(
            items_dir.to_path_buf(),
            resolved_id,
            workspace_root.to_path_buf(),
            skip_stale,
            &mut all_formatted_findings,
        ) {
            Err(e) => Ok(VerifyOutcome {
                stdout: None,
                stderr: Some(format!("verify catalogue-spec-refs: infrastructure error: {e}")),
                exit_code: 1,
            }),
            Ok(no_findings) => {
                if no_findings {
                    Ok(VerifyOutcome::success(Some(
                        "[OK] catalogue-spec-refs: no findings".to_owned(),
                    )))
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
                    Ok(VerifyOutcome { stdout: None, stderr: Some(stderr), exit_code: 1 })
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::FsVerifyAdapter;
    use usecase::verify::VerifyPort as _;

    #[cfg(unix)]
    #[test]
    fn test_verify_tech_stack_rejects_symlinked_project_root() {
        let real_root = tempfile::tempdir().unwrap();
        let link_parent = tempfile::tempdir().unwrap();
        let root_link = link_parent.path().join("workspace-link");
        std::os::unix::fs::symlink(real_root.path(), &root_link).unwrap();

        let outcome = FsVerifyAdapter::new().verify_tech_stack(&root_link).unwrap();

        assert_eq!(outcome.exit_code, 1);
        let stderr = outcome.stderr.unwrap_or_default();
        assert!(stderr.contains("trusted root rejected before verification"), "{stderr}");
        assert!(stderr.contains("refusing to use symlinked trusted_root"), "{stderr}");
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_spec_attribution_symlinked_sibling_spec_json_errors() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real-spec.json");
        let spec_json = dir.path().join("spec.json");
        let spec_md = dir.path().join("spec.md");
        std::fs::write(&target, "{}").unwrap();
        std::fs::write(&spec_md, "# Spec\n").unwrap();
        std::os::unix::fs::symlink(&target, &spec_json).unwrap();

        let outcome = FsVerifyAdapter::new().verify_spec_attribution(&spec_md).unwrap();

        assert_eq!(outcome.exit_code, 1);
        let stderr = outcome.stderr.unwrap_or_default();
        assert!(stderr.contains("sibling spec.json rejected before verification"), "{stderr}");
        assert!(stderr.contains("refusing to follow symlink"), "{stderr}");
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_spec_frontmatter_symlinked_sibling_spec_json_errors() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real-spec.json");
        let spec_json = dir.path().join("spec.json");
        let spec_md = dir.path().join("spec.md");
        std::fs::write(&target, "{}").unwrap();
        std::fs::write(&spec_md, "# Spec\n").unwrap();
        std::os::unix::fs::symlink(&target, &spec_json).unwrap();

        let outcome = FsVerifyAdapter::new().verify_spec_frontmatter(&spec_md).unwrap();

        assert_eq!(outcome.exit_code, 1);
        let stderr = outcome.stderr.unwrap_or_default();
        assert!(stderr.contains("sibling spec.json rejected before verification"), "{stderr}");
        assert!(stderr.contains("refusing to follow symlink"), "{stderr}");
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_spec_frontmatter_symlinked_spec_md_errors() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real-spec.md");
        let spec_md = dir.path().join("spec.md");
        std::fs::write(&target, "---\nversion: \"1.0\"\n---\n# Spec\n").unwrap();
        std::os::unix::fs::symlink(&target, &spec_md).unwrap();

        let outcome = FsVerifyAdapter::new().verify_spec_frontmatter(&spec_md).unwrap();

        assert_eq!(outcome.exit_code, 1);
        let stderr = outcome.stderr.unwrap_or_default();
        assert!(stderr.contains("spec.md rejected before verification"), "{stderr}");
        assert!(stderr.contains("refusing to follow symlink"), "{stderr}");
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_spec_signals_symlinked_sibling_spec_json_errors() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real-spec.json");
        let spec_json = dir.path().join("spec.json");
        let spec_md = dir.path().join("spec.md");
        std::fs::write(&target, "{}").unwrap();
        std::fs::write(&spec_md, "# Spec\n").unwrap();
        std::os::unix::fs::symlink(&target, &spec_json).unwrap();

        let outcome = FsVerifyAdapter::new().verify_spec_signals(&spec_md).unwrap();

        assert_eq!(outcome.exit_code, 1);
        let stderr = outcome.stderr.unwrap_or_default();
        assert!(stderr.contains("sibling spec.json rejected before verification"), "{stderr}");
        assert!(stderr.contains("refusing to follow symlink"), "{stderr}");
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_catalogue_spec_refs_rejects_symlinked_items_dir_ancestor() {
        let workspace = tempfile::tempdir().unwrap();
        let real_track = workspace.path().join("real-track");
        let track_link = workspace.path().join("track");
        std::fs::create_dir_all(real_track.join("items/some-track")).unwrap();
        std::os::unix::fs::symlink(&real_track, &track_link).unwrap();

        let items_dir = track_link.join("items");
        let outcome = FsVerifyAdapter::new()
            .verify_catalogue_spec_refs(Some("some-track"), &items_dir, workspace.path(), false)
            .unwrap();

        assert_eq!(outcome.exit_code, 1);
        let stderr = outcome.stderr.unwrap_or_default();
        assert!(stderr.contains("items_dir rejected before verification"), "{stderr}");
        assert!(stderr.contains("refusing to follow symlink"), "{stderr}");
    }
}
