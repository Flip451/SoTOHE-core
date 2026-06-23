//! `verify` use-case port.
//!
//! Defines the secondary port for the `verify` command family.
//! Infrastructure implements [`VerifyPort`] by delegating to the
//! `infrastructure::verify::*` submodules.

use std::path::Path;

/// Outcome of a single verify subcommand.
pub struct VerifyOutcome {
    /// Human-readable output (stdout).
    pub stdout: Option<String>,
    /// Human-readable error output (stderr).
    pub stderr: Option<String>,
    /// Exit code: 0 = pass, non-zero = fail.
    pub exit_code: u8,
}

impl VerifyOutcome {
    /// Construct a passing outcome.
    pub fn success(stdout: Option<String>) -> Self {
        Self { stdout, stderr: None, exit_code: 0 }
    }

    /// Construct a failing outcome.
    pub fn failure(stderr: Option<String>) -> Self {
        Self { stdout: None, stderr, exit_code: 1 }
    }
}

/// Error returned by [`VerifyPort`] methods that can fail structurally
/// (e.g. adapter-level setup failures distinct from verify findings).
#[derive(Debug, thiserror::Error)]
pub enum VerifyPortError {
    /// The infrastructure layer could not fulfill the request.
    #[error("{0}")]
    Unavailable(String),
}

/// Secondary port for all `verify` subcommands.
///
/// Infrastructure implements this port by delegating to the
/// `infrastructure::verify::*` submodules.  Verify findings are encoded inside
/// [`VerifyOutcome`]; adapter-level failures use [`VerifyPortError`].
pub trait VerifyPort: Send + Sync {
    /// Check tech-stack.md for unresolved TODO markers.
    fn verify_tech_stack(&self, project_root: &Path) -> VerifyOutcome;

    /// Check latest track artifacts for completeness.
    fn verify_latest_track(&self, project_root: &Path) -> VerifyOutcome;

    /// Check architecture docs synchronization and text patterns.
    fn verify_arch_docs(&self, project_root: &Path) -> VerifyOutcome;

    /// Check workspace layer dependency rules via cargo metadata.
    fn verify_layers(&self, project_root: &Path) -> VerifyOutcome;

    /// Check local Git config uses `.githooks` as `core.hooksPath`.
    fn verify_hooks_path(&self, project_root: &Path) -> VerifyOutcome;

    /// Check spec.md requirement lines for `[source: ...]` attribution.
    fn verify_spec_attribution(&self, spec_path: &Path) -> VerifyOutcome;

    /// Check spec.md YAML frontmatter for required fields.
    fn verify_spec_frontmatter(&self, spec_path: &Path) -> VerifyOutcome;

    /// Check canonical module ownership.
    fn verify_canonical_modules(&self, project_root: &Path) -> VerifyOutcome;

    /// Check Rust source file sizes against module_limits thresholds.
    fn verify_module_size(&self, project_root: &Path) -> VerifyOutcome;

    /// Check libs/domain/src/ for hexagonal purity violations.
    fn verify_domain_purity(&self, project_root: &Path) -> VerifyOutcome;

    /// Check libs/domain/src/ for pub String fields (should be enums or newtypes).
    fn verify_domain_strings(&self, project_root: &Path) -> VerifyOutcome;

    /// Check libs/usecase/src/ for hexagonal purity violations.
    fn verify_usecase_purity(&self, project_root: &Path) -> VerifyOutcome;

    /// Check that local file links in Markdown documents resolve to existing files.
    fn verify_doc_links(&self, project_root: &Path) -> VerifyOutcome;

    /// Check that plan.md files are up-to-date with metadata.json renderings.
    fn verify_view_freshness(&self, project_root: &Path) -> VerifyOutcome;

    /// Check spec.md source tag signals match frontmatter and red == 0 gate.
    fn verify_spec_signals(&self, spec_path: &Path) -> VerifyOutcome;

    /// Validate structured-ref fields per ADR 2026-04-19-1242.
    ///
    /// Returns `Ok(None)` when the skip path fires (non-track branch).
    fn verify_plan_artifact_refs(&self, track_dir: Option<&Path>) -> VerifyOutcome;

    /// Verify catalogue-spec ref integrity (SoT Chain binary gate).
    ///
    /// Returns a skip outcome when not on a track branch.
    fn verify_catalogue_spec_refs(
        &self,
        track_id: Option<&str>,
        items_dir: &Path,
        workspace_root: &Path,
        skip_stale: bool,
    ) -> VerifyOutcome;
}
