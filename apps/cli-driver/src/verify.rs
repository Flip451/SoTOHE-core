// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `verify` command family — primary adapter driver.
//!
//! `VerifyDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The render helpers here mirror
//! `apps/cli-composition/src/verify.rs` and `cmd_outcome.rs`;
//! T021 removes the `cli_composition` duplicates when the live path is flipped.

// TODO(T021): add use-case + infrastructure imports once Cargo.toml is materialized.
// use std::path::PathBuf;
// use infrastructure::verify::{VerifyFinding, VerifyOutcome};
// use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
// use usecase::track_resolution::{
//     ActiveTrackResolveError, ActiveTrackResolveInteractor, ActiveTrackResolveService as _,
//     TrackResolutionError,
// };

use std::path::PathBuf;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `verify` command family.
pub enum VerifyInput {
    /// Check tech-stack.md for unresolved TODO markers.
    TechStack {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Check latest track artifacts for completeness.
    LatestTrack {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Check architecture docs synchronization and text patterns.
    ArchDocs {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Check workspace layer dependency rules via cargo metadata.
    Layers {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Check local Git config uses `.githooks` as `core.hooksPath`.
    HooksPath {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Check spec.md requirement lines for `[source: ...]` attribution.
    SpecAttribution {
        /// Path to the spec file.
        spec_path: PathBuf,
    },
    /// Check spec.md YAML frontmatter for required fields.
    SpecFrontmatter {
        /// Path to the spec file.
        spec_path: PathBuf,
    },
    /// Check canonical module ownership.
    CanonicalModules {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Check Rust source file sizes against module_limits thresholds.
    ModuleSize {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Check libs/domain/src/ for hexagonal purity violations.
    DomainPurity {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Check libs/domain/src/ for pub String fields (should be enums or newtypes).
    DomainStrings {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Check libs/usecase/src/ for hexagonal purity violations.
    UsecasePurity {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Check that local file links in Markdown documents resolve to existing files.
    DocLinks {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Check that plan.md files are up-to-date with metadata.json renderings.
    ViewFreshness {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Check spec.md source tag signals match frontmatter and red == 0 gate.
    SpecSignals {
        /// Path to the spec file.
        spec_path: PathBuf,
    },
    /// Validate structured-ref fields per ADR 2026-04-19-1242.
    PlanArtifactRefs {
        /// Optional track directory (resolved from active branch if `None`).
        track_dir: Option<PathBuf>,
    },
    /// Verify catalogue-spec ref integrity (SoT Chain binary gate).
    CatalogueSpecRefs {
        /// Optional track ID (resolved from active branch if `None`).
        track_id: Option<String>,
        /// Path to the track items directory.
        items_dir: PathBuf,
        /// Workspace root directory.
        workspace_root: PathBuf,
        /// Skip stale entries.
        skip_stale: bool,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `verify` command family.
///
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct VerifyDriver {
    // TODO(T021): inject use-case interactors here (currently this family has
    // no injectable adapter dependencies — infrastructure functions are called
    // inline, same as cli_composition::VerifyCompositionRoot).
}

impl VerifyDriver {
    /// Create a new `VerifyDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a verify command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
    pub fn handle(&self, input: VerifyInput) -> CommandOutcome {
        match input {
            VerifyInput::TechStack { project_root } => self.verify_tech_stack(project_root),
            VerifyInput::LatestTrack { project_root } => self.verify_latest_track(project_root),
            VerifyInput::ArchDocs { project_root } => self.verify_arch_docs(project_root),
            VerifyInput::Layers { project_root } => self.verify_layers(project_root),
            VerifyInput::HooksPath { project_root } => self.verify_hooks_path(project_root),
            VerifyInput::SpecAttribution { spec_path } => self.verify_spec_attribution(spec_path),
            VerifyInput::SpecFrontmatter { spec_path } => self.verify_spec_frontmatter(spec_path),
            VerifyInput::CanonicalModules { project_root } => {
                self.verify_canonical_modules(project_root)
            }
            VerifyInput::ModuleSize { project_root } => self.verify_module_size(project_root),
            VerifyInput::DomainPurity { project_root } => self.verify_domain_purity(project_root),
            VerifyInput::DomainStrings { project_root } => self.verify_domain_strings(project_root),
            VerifyInput::UsecasePurity { project_root } => self.verify_usecase_purity(project_root),
            VerifyInput::DocLinks { project_root } => self.verify_doc_links(project_root),
            VerifyInput::ViewFreshness { project_root } => self.verify_view_freshness(project_root),
            VerifyInput::SpecSignals { spec_path } => self.verify_spec_signals(spec_path),
            VerifyInput::PlanArtifactRefs { track_dir } => {
                self.verify_plan_artifact_refs(track_dir)
            }
            VerifyInput::CatalogueSpecRefs { track_id, items_dir, workspace_root, skip_stale } => {
                self.verify_catalogue_spec_refs(track_id, items_dir, workspace_root, skip_stale)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Render helpers (logic duplicated from cli_composition/src/verify.rs;
    // T021 removes the cli_composition copy).
    // -----------------------------------------------------------------------

    fn verify_tech_stack(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::tech_stack::verify and
        // render_outcome("verify tech stack readiness", &outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_tech_stack.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_latest_track(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::latest_track::verify and
        // render_outcome("verify latest track files", &outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_latest_track.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_arch_docs(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::architecture_rules::verify,
        // doc_patterns::verify, conventions::verify_convention_index,
        // and render_outcome("verify architecture docs", &merged_outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_arch_docs.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_layers(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::layers::verify and
        // render_outcome("verify layers", &outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_layers.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_hooks_path(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::hooks_path::verify and
        // render_outcome("verify hooks path", &outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_hooks_path.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_spec_attribution(&self, _spec_path: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::spec_attribution::verify and
        // render_outcome("verify spec attribution", &outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_spec_attribution.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_spec_frontmatter(&self, _spec_path: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::spec_frontmatter::verify and
        // render_outcome("verify spec frontmatter", &outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_spec_frontmatter.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_canonical_modules(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::canonical_modules::verify and
        // render_outcome("verify canonical modules", &outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_canonical_modules.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_module_size(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::module_size::verify and
        // render_outcome("verify module size", &outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_module_size.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_domain_purity(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::domain_purity::verify and
        // render_outcome("verify domain purity", &outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_domain_purity.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_domain_strings(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::domain_strings::verify and
        // render_outcome("verify domain strings", &outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_domain_strings.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_usecase_purity(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::usecase_purity::verify and
        // render_outcome("verify usecase purity", &outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_usecase_purity.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_doc_links(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::doc_links::verify and
        // render_outcome("verify doc links", &outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_doc_links.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_view_freshness(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::view_freshness::verify and
        // render_outcome("verify view freshness", &outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_view_freshness.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_spec_signals(&self, _spec_path: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::spec_signals::verify and
        // render_outcome("verify spec signals", &outcome) here.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_spec_signals.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_plan_artifact_refs(&self, _track_dir: Option<PathBuf>) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::plan_artifact_refs::verify and
        // render_outcome("verify plan artifact refs", &outcome) here.
        // When track_dir is None, use resolve_ci_verify_track_id() for AC-16 skip path
        // via render_skip("verify plan artifact refs", "not on a track branch; skipping").
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_plan_artifact_refs.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn verify_catalogue_spec_refs(
        &self,
        _track_id: Option<String>,
        _items_dir: PathBuf,
        _workspace_root: PathBuf,
        _skip_stale: bool,
    ) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::catalogue_spec_refs::execute_verify_catalogue_spec_refs
        // and render_skip("verify catalogue-spec-refs", "not on a track branch; skipping") for AC-16 path.
        // Mirrors cli_composition/src/verify.rs VerifyCompositionRoot::verify_catalogue_spec_refs.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }
}

impl Default for VerifyDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Render helper (duplicated from cli_composition/src/verify.rs lines 8-11;
// T021 removes the cli_composition copy and uses cli_driver::render::render_skip).
// ---------------------------------------------------------------------------

/// Render a skip outcome (non-track branch, AC-16).
///
/// Mirrors `cli_composition::verify::render_skip` (lines 8-11).
#[allow(dead_code)]
fn render_skip(_label: &str, _reason: &str) -> CommandOutcome {
    // TODO(T021): implement — format!("--- {label} ---\n[SKIP] {reason}\n--- {label} SKIPPED ---")
    // and CommandOutcome::success(Some(stdout)).
    // Kept as a named stub so callers reference it at the right site.
    CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
}
