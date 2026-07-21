//! `verify` command family — primary adapter driver.
//!
//! `VerifyDriver` holds an injected [`usecase::verify::VerifyService`] and exposes
//! `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

pub use usecase::TrackId;
use usecase::verify::VerifyService;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `verify` command family.
pub enum VerifyInput {
    /// Check latest track artifacts for completeness.
    LatestTrack {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Check the retention live surface for retired gate/document identifiers.
    RetentionGate {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Verify that the configured fixed SOTP version tag resolves on its remote.
    SotpVersionTag {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Verify that tracked repository files do not contain work-machine paths.
    MachinePaths {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Verify that shipped template files contain no concrete source references.
    TemplateRefs {
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
        track_id: Option<TrackId>,
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
/// Holds an injected [`VerifyService`]; exposes `handle(input) -> CommandOutcome`.
pub struct VerifyDriver {
    service: Arc<dyn VerifyService>,
}

impl VerifyDriver {
    /// Create a new `VerifyDriver` with the given service.
    pub fn new(service: Arc<dyn VerifyService>) -> Self {
        Self { service }
    }

    /// Handle a verify command.
    pub fn handle(&self, input: VerifyInput) -> CommandOutcome {
        match input {
            VerifyInput::LatestTrack { project_root } => {
                map_result(self.service.verify_latest_track(project_root))
            }
            VerifyInput::RetentionGate { project_root } => {
                map_result(self.service.verify_retention_gate(project_root))
            }
            VerifyInput::SotpVersionTag { project_root } => {
                map_result(self.service.verify_sotp_version_tag(&project_root))
            }
            VerifyInput::MachinePaths { project_root } => {
                map_result(self.service.verify_machine_paths(&project_root))
            }
            VerifyInput::TemplateRefs { project_root } => {
                map_result(self.service.verify_template_refs(&project_root))
            }
            VerifyInput::ArchDocs { project_root } => {
                map_result(self.service.verify_arch_docs(project_root))
            }
            VerifyInput::Layers { project_root } => {
                map_result(self.service.verify_layers(project_root))
            }
            VerifyInput::HooksPath { project_root } => {
                map_result(self.service.verify_hooks_path(project_root))
            }
            VerifyInput::SpecAttribution { spec_path } => {
                map_result(self.service.verify_spec_attribution(spec_path))
            }
            VerifyInput::SpecFrontmatter { spec_path } => {
                map_result(self.service.verify_spec_frontmatter(spec_path))
            }
            VerifyInput::CanonicalModules { project_root } => {
                map_result(self.service.verify_canonical_modules(project_root))
            }
            VerifyInput::ModuleSize { project_root } => {
                map_result(self.service.verify_module_size(project_root))
            }
            VerifyInput::DomainPurity { project_root } => {
                map_result(self.service.verify_domain_purity(project_root))
            }
            VerifyInput::DomainStrings { project_root } => {
                map_result(self.service.verify_domain_strings(project_root))
            }
            VerifyInput::UsecasePurity { project_root } => {
                map_result(self.service.verify_usecase_purity(project_root))
            }
            VerifyInput::DocLinks { project_root } => {
                map_result(self.service.verify_doc_links(project_root))
            }
            VerifyInput::ViewFreshness { project_root } => {
                map_result(self.service.verify_view_freshness(project_root))
            }
            VerifyInput::SpecSignals { spec_path } => {
                map_result(self.service.verify_spec_signals(spec_path))
            }
            VerifyInput::PlanArtifactRefs { track_dir } => {
                map_result(self.service.verify_plan_artifact_refs(track_dir))
            }
            VerifyInput::CatalogueSpecRefs { track_id, items_dir, workspace_root, skip_stale } => {
                map_result(self.service.verify_catalogue_spec_refs(
                    track_id,
                    items_dir,
                    workspace_root,
                    skip_stale,
                ))
            }
        }
    }
}

/// Map a `Result<VerifyOutcome, VerifyPortError>` to a [`CommandOutcome`].
///
/// Adapter-level errors (`Err`) are rendered as a failing `CommandOutcome` with
/// the error message in `stderr`.
fn map_result(
    result: Result<usecase::verify::VerifyOutcome, usecase::verify::VerifyPortError>,
) -> CommandOutcome {
    match result {
        Ok(outcome) => CommandOutcome {
            stdout: outcome.stdout,
            stderr: outcome.stderr,
            exit_code: outcome.exit_code,
        },
        Err(e) => CommandOutcome { stdout: None, stderr: Some(e.to_string()), exit_code: 1 },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use usecase::verify::{VerifyOutcome, VerifyPortError};

    use super::*;

    #[derive(Default)]
    struct RecordingService {
        retention_root: Mutex<Option<PathBuf>>,
        verifier_roots: Mutex<Vec<(&'static str, PathBuf)>>,
        fail: bool,
    }

    impl RecordingService {
        fn unused(&self) -> Result<VerifyOutcome, VerifyPortError> {
            Err(VerifyPortError::Unavailable("unused test route".to_owned()))
        }

        fn record_verifier_route(
            &self,
            route: &'static str,
            project_root: &Path,
        ) -> Result<VerifyOutcome, VerifyPortError> {
            self.verifier_roots.lock().unwrap().push((route, project_root.to_path_buf()));
            Ok(VerifyOutcome::success(Some(route.to_owned())))
        }
    }

    macro_rules! unused_pathbuf_method {
        ($name:ident) => {
            fn $name(&self, _: PathBuf) -> Result<VerifyOutcome, VerifyPortError> {
                self.unused()
            }
        };
    }

    impl VerifyService for RecordingService {
        unused_pathbuf_method!(verify_latest_track);
        unused_pathbuf_method!(verify_arch_docs);
        unused_pathbuf_method!(verify_layers);
        unused_pathbuf_method!(verify_hooks_path);
        unused_pathbuf_method!(verify_spec_attribution);
        unused_pathbuf_method!(verify_spec_frontmatter);
        unused_pathbuf_method!(verify_canonical_modules);
        unused_pathbuf_method!(verify_module_size);
        unused_pathbuf_method!(verify_domain_purity);
        unused_pathbuf_method!(verify_domain_strings);
        unused_pathbuf_method!(verify_usecase_purity);
        unused_pathbuf_method!(verify_doc_links);
        unused_pathbuf_method!(verify_view_freshness);
        unused_pathbuf_method!(verify_spec_signals);

        fn verify_retention_gate(
            &self,
            project_root: PathBuf,
        ) -> Result<VerifyOutcome, VerifyPortError> {
            *self.retention_root.lock().unwrap() = Some(project_root);
            if self.fail {
                Ok(VerifyOutcome::failure(Some("finding".to_owned())))
            } else {
                Ok(VerifyOutcome::success(Some("ok".to_owned())))
            }
        }

        fn verify_sotp_version_tag(
            &self,
            project_root: &Path,
        ) -> Result<VerifyOutcome, VerifyPortError> {
            self.record_verifier_route("sotp_version_tag", project_root)
        }

        fn verify_machine_paths(
            &self,
            project_root: &Path,
        ) -> Result<VerifyOutcome, VerifyPortError> {
            self.record_verifier_route("machine_paths", project_root)
        }

        fn verify_template_refs(
            &self,
            project_root: &Path,
        ) -> Result<VerifyOutcome, VerifyPortError> {
            self.record_verifier_route("template_refs", project_root)
        }

        fn verify_plan_artifact_refs(
            &self,
            _: Option<PathBuf>,
        ) -> Result<VerifyOutcome, VerifyPortError> {
            self.unused()
        }

        fn verify_catalogue_spec_refs(
            &self,
            _: Option<TrackId>,
            _: PathBuf,
            _: PathBuf,
            _: bool,
        ) -> Result<VerifyOutcome, VerifyPortError> {
            self.unused()
        }
    }

    #[test]
    fn test_verify_driver_retention_gate_routes_to_service() {
        let service = Arc::new(RecordingService::default());
        let driver = VerifyDriver::new(service.clone());
        let root = PathBuf::from("workspace-root");

        let outcome = driver.handle(VerifyInput::RetentionGate { project_root: root.clone() });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(service.retention_root.lock().unwrap().as_deref(), Some(root.as_path()));
    }

    #[test]
    fn test_verify_driver_retention_gate_preserves_failure_outcome() {
        let service = Arc::new(RecordingService {
            retention_root: Mutex::new(None),
            verifier_roots: Mutex::new(Vec::new()),
            fail: true,
        });
        let driver = VerifyDriver::new(service);

        let outcome =
            driver.handle(VerifyInput::RetentionGate { project_root: PathBuf::from(".") });

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stderr.as_deref(), Some("finding"));
    }

    #[test]
    fn test_verify_driver_new_verifier_routes_route_project_root_to_service() {
        let service = Arc::new(RecordingService::default());
        let driver = VerifyDriver::new(service.clone());
        let root = PathBuf::from("workspace-root");

        assert_eq!(
            driver.handle(VerifyInput::SotpVersionTag { project_root: root.clone() }).exit_code,
            0
        );
        assert_eq!(
            driver.handle(VerifyInput::MachinePaths { project_root: root.clone() }).exit_code,
            0
        );
        assert_eq!(
            driver.handle(VerifyInput::TemplateRefs { project_root: root.clone() }).exit_code,
            0
        );

        assert_eq!(
            *service.verifier_roots.lock().unwrap(),
            vec![
                ("sotp_version_tag", root.clone()),
                ("machine_paths", root.clone()),
                ("template_refs", root),
            ]
        );
    }
}
