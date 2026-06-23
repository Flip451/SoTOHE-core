//! `verify` command family — primary adapter driver.
//!
//! `VerifyDriver` holds an injected [`usecase::verify::VerifyPort`] and exposes
//! `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::verify::VerifyPort;

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
/// Holds an injected [`VerifyPort`]; exposes `handle(input) -> CommandOutcome`.
pub struct VerifyDriver {
    port: Arc<dyn VerifyPort>,
}

impl VerifyDriver {
    /// Create a new `VerifyDriver` with the given port.
    pub fn new(port: Arc<dyn VerifyPort>) -> Self {
        Self { port }
    }

    /// Handle a verify command.
    pub fn handle(&self, input: VerifyInput) -> CommandOutcome {
        match input {
            VerifyInput::TechStack { project_root } => {
                map_outcome(self.port.verify_tech_stack(&project_root))
            }
            VerifyInput::LatestTrack { project_root } => {
                map_outcome(self.port.verify_latest_track(&project_root))
            }
            VerifyInput::ArchDocs { project_root } => {
                map_outcome(self.port.verify_arch_docs(&project_root))
            }
            VerifyInput::Layers { project_root } => {
                map_outcome(self.port.verify_layers(&project_root))
            }
            VerifyInput::HooksPath { project_root } => {
                map_outcome(self.port.verify_hooks_path(&project_root))
            }
            VerifyInput::SpecAttribution { spec_path } => {
                map_outcome(self.port.verify_spec_attribution(&spec_path))
            }
            VerifyInput::SpecFrontmatter { spec_path } => {
                map_outcome(self.port.verify_spec_frontmatter(&spec_path))
            }
            VerifyInput::CanonicalModules { project_root } => {
                map_outcome(self.port.verify_canonical_modules(&project_root))
            }
            VerifyInput::ModuleSize { project_root } => {
                map_outcome(self.port.verify_module_size(&project_root))
            }
            VerifyInput::DomainPurity { project_root } => {
                map_outcome(self.port.verify_domain_purity(&project_root))
            }
            VerifyInput::DomainStrings { project_root } => {
                map_outcome(self.port.verify_domain_strings(&project_root))
            }
            VerifyInput::UsecasePurity { project_root } => {
                map_outcome(self.port.verify_usecase_purity(&project_root))
            }
            VerifyInput::DocLinks { project_root } => {
                map_outcome(self.port.verify_doc_links(&project_root))
            }
            VerifyInput::ViewFreshness { project_root } => {
                map_outcome(self.port.verify_view_freshness(&project_root))
            }
            VerifyInput::SpecSignals { spec_path } => {
                map_outcome(self.port.verify_spec_signals(&spec_path))
            }
            VerifyInput::PlanArtifactRefs { track_dir } => {
                map_outcome(self.port.verify_plan_artifact_refs(track_dir.as_deref()))
            }
            VerifyInput::CatalogueSpecRefs { track_id, items_dir, workspace_root, skip_stale } => {
                map_outcome(self.port.verify_catalogue_spec_refs(
                    track_id.as_deref(),
                    &items_dir,
                    &workspace_root,
                    skip_stale,
                ))
            }
        }
    }
}

/// Map a [`usecase::verify::VerifyOutcome`] to a [`CommandOutcome`].
fn map_outcome(outcome: usecase::verify::VerifyOutcome) -> CommandOutcome {
    CommandOutcome { stdout: outcome.stdout, stderr: outcome.stderr, exit_code: outcome.exit_code }
}
