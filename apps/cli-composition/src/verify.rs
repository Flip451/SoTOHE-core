//! `verify` command family — CliApp impl methods.

use std::path::PathBuf;

use crate::{CliApp, CommandOutcome};

impl CliApp {
    /// Check tech-stack.md for unresolved TODO markers.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_tech_stack(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let _ = project_root;
        Err(String::from("not implemented"))
    }

    /// Check latest track artifacts for completeness.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_latest_track(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let _ = project_root;
        Err(String::from("not implemented"))
    }

    /// Check architecture docs synchronization and text patterns.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_arch_docs(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let _ = project_root;
        Err(String::from("not implemented"))
    }

    /// Check workspace layer dependency rules via cargo metadata.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_layers(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let _ = project_root;
        Err(String::from("not implemented"))
    }

    /// Check .claude/settings.json structural guardrails.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_orchestra(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let _ = project_root;
        Err(String::from("not implemented"))
    }

    /// Check spec.md requirement lines for `[source: ...]` attribution.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_spec_attribution(&self, spec_path: PathBuf) -> Result<CommandOutcome, String> {
        let _ = spec_path;
        Err(String::from("not implemented"))
    }

    /// Check spec.md YAML frontmatter for required fields.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_spec_frontmatter(&self, spec_path: PathBuf) -> Result<CommandOutcome, String> {
        let _ = spec_path;
        Err(String::from("not implemented"))
    }

    /// Check canonical module ownership.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_canonical_modules(
        &self,
        project_root: PathBuf,
    ) -> Result<CommandOutcome, String> {
        let _ = project_root;
        Err(String::from("not implemented"))
    }

    /// Check Rust source file sizes against module_limits thresholds.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_module_size(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let _ = project_root;
        Err(String::from("not implemented"))
    }

    /// Check libs/domain/src/ for hexagonal purity violations.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_domain_purity(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let _ = project_root;
        Err(String::from("not implemented"))
    }

    /// Check libs/domain/src/ for pub String fields (should be enums or newtypes).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_domain_strings(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let _ = project_root;
        Err(String::from("not implemented"))
    }

    /// Check libs/usecase/src/ for hexagonal purity violations.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_usecase_purity(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let _ = project_root;
        Err(String::from("not implemented"))
    }

    /// Check that local file links in Markdown documents resolve to existing files.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_doc_links(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let _ = project_root;
        Err(String::from("not implemented"))
    }

    /// Check that plan.md files are up-to-date with metadata.json renderings.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_view_freshness(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let _ = project_root;
        Err(String::from("not implemented"))
    }

    /// Check spec.md source tag signals match frontmatter and red == 0 gate.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_spec_signals(&self, spec_path: PathBuf) -> Result<CommandOutcome, String> {
        let _ = spec_path;
        Err(String::from("not implemented"))
    }

    /// Check spec.md contains a Domain States section with table data rows.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_spec_states(
        &self,
        spec_path: Option<PathBuf>,
        strict: bool,
    ) -> Result<CommandOutcome, String> {
        let _ = (spec_path, strict);
        Err(String::from("not implemented"))
    }

    /// Validate structured-ref fields per ADR 2026-04-19-1242.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_plan_artifact_refs(
        &self,
        track_dir: Option<PathBuf>,
    ) -> Result<CommandOutcome, String> {
        let _ = track_dir;
        Err(String::from("not implemented"))
    }

    /// Verify catalogue-spec ref integrity (SoT Chain binary gate).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_catalogue_spec_refs(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
        workspace_root: PathBuf,
        skip_stale: bool,
    ) -> Result<CommandOutcome, String> {
        let _ = (track_id, items_dir, workspace_root, skip_stale);
        Err(String::from("not implemented"))
    }

    /// Check catalogue-spec signal gate results for each tddd-enabled layer.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_catalogue_spec_signals(
        &self,
        items_dir: PathBuf,
        workspace_root: PathBuf,
        strict: bool,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, workspace_root, strict);
        Err(String::from("not implemented"))
    }

    /// Verify ADR decision signal grounds across knowledge/adr/.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn verify_adr_signals(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let _ = project_root;
        Err(String::from("not implemented"))
    }
}
