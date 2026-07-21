//! `verify` use-case port.
//!
//! Defines the secondary port for the `verify` command family.
//! Infrastructure implements [`VerifyPort`] by delegating to the
//! `infrastructure::verify::*` submodules.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::TrackId;

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
    /// Check latest track artifacts for completeness.
    fn verify_latest_track(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check the retention live surface for retired gate/document identifiers.
    fn verify_retention_gate(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Verify that the configured fixed SOTP version tag resolves on its remote.
    fn verify_sotp_version_tag(
        &self,
        project_root: &Path,
    ) -> Result<VerifyOutcome, VerifyPortError>;

    /// Verify that tracked repository files do not contain work-machine paths.
    fn verify_machine_paths(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Verify that shipped template files contain no concrete source references.
    fn verify_template_refs(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check architecture docs synchronization and text patterns.
    fn verify_arch_docs(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check workspace layer dependency rules via cargo metadata.
    fn verify_layers(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check local Git config uses `.githooks` as `core.hooksPath`.
    fn verify_hooks_path(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check spec.md requirement lines for `[source: ...]` attribution.
    fn verify_spec_attribution(&self, spec_path: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check spec.md YAML frontmatter for required fields.
    fn verify_spec_frontmatter(&self, spec_path: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check canonical module ownership.
    fn verify_canonical_modules(
        &self,
        project_root: &Path,
    ) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check Rust source file sizes against module_limits thresholds.
    fn verify_module_size(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check libs/domain/src/ for hexagonal purity violations.
    fn verify_domain_purity(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check libs/domain/src/ for pub String fields (should be enums or newtypes).
    fn verify_domain_strings(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check libs/usecase/src/ for hexagonal purity violations.
    fn verify_usecase_purity(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check that local file links in Markdown documents resolve to existing files.
    fn verify_doc_links(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check that plan.md files are up-to-date with metadata.json renderings.
    fn verify_view_freshness(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check spec.md source tag signals match frontmatter and red == 0 gate.
    fn verify_spec_signals(&self, spec_path: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Validate structured-ref fields per ADR 2026-04-19-1242.
    ///
    /// Returns a skip outcome (inside `Ok`) when the skip path fires (non-track branch).
    fn verify_plan_artifact_refs(
        &self,
        track_dir: Option<&Path>,
    ) -> Result<VerifyOutcome, VerifyPortError>;

    /// Verify catalogue-spec ref integrity (SoT Chain binary gate).
    ///
    /// Returns a skip outcome (inside `Ok`) when not on a track branch.
    fn verify_catalogue_spec_refs(
        &self,
        track_id: Option<&TrackId>,
        items_dir: &Path,
        workspace_root: &Path,
        skip_stale: bool,
    ) -> Result<VerifyOutcome, VerifyPortError>;
}

/// Application-level contract for all verify subcommands.
///
/// `PrimaryAdapter` (`VerifyDriver`) depends on this interface rather than directly on
/// `VerifyPort` (DIP). `VerifyInteractor` implements this service by delegating to the
/// injected `VerifyPort`.
pub trait VerifyService: Send + Sync {
    /// Check latest track artifacts for completeness.
    fn verify_latest_track(&self, project_root: PathBuf) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check the retention live surface for retired gate/document identifiers.
    fn verify_retention_gate(
        &self,
        project_root: PathBuf,
    ) -> Result<VerifyOutcome, VerifyPortError>;

    /// Verify that the configured fixed SOTP version tag resolves on its remote.
    fn verify_sotp_version_tag(
        &self,
        project_root: &Path,
    ) -> Result<VerifyOutcome, VerifyPortError>;

    /// Verify that tracked repository files do not contain work-machine paths.
    fn verify_machine_paths(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Verify that shipped template files contain no concrete source references.
    fn verify_template_refs(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check architecture docs synchronization and text patterns.
    fn verify_arch_docs(&self, project_root: PathBuf) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check workspace layer dependency rules via cargo metadata.
    fn verify_layers(&self, project_root: PathBuf) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check local Git config uses `.githooks` as `core.hooksPath`.
    fn verify_hooks_path(&self, project_root: PathBuf) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check spec.md requirement lines for `[source: ...]` attribution.
    fn verify_spec_attribution(&self, spec_path: PathBuf)
    -> Result<VerifyOutcome, VerifyPortError>;

    /// Check spec.md YAML frontmatter for required fields.
    fn verify_spec_frontmatter(&self, spec_path: PathBuf)
    -> Result<VerifyOutcome, VerifyPortError>;

    /// Check canonical module ownership.
    fn verify_canonical_modules(
        &self,
        project_root: PathBuf,
    ) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check Rust source file sizes against module_limits thresholds.
    fn verify_module_size(&self, project_root: PathBuf) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check libs/domain/src/ for hexagonal purity violations.
    fn verify_domain_purity(&self, project_root: PathBuf)
    -> Result<VerifyOutcome, VerifyPortError>;

    /// Check libs/domain/src/ for pub String fields.
    fn verify_domain_strings(
        &self,
        project_root: PathBuf,
    ) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check libs/usecase/src/ for hexagonal purity violations.
    fn verify_usecase_purity(
        &self,
        project_root: PathBuf,
    ) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check that local file links in Markdown documents resolve to existing files.
    fn verify_doc_links(&self, project_root: PathBuf) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check that plan.md files are up-to-date with metadata.json renderings.
    fn verify_view_freshness(
        &self,
        project_root: PathBuf,
    ) -> Result<VerifyOutcome, VerifyPortError>;

    /// Check spec.md source tag signals match frontmatter and red == 0 gate.
    fn verify_spec_signals(&self, spec_path: PathBuf) -> Result<VerifyOutcome, VerifyPortError>;

    /// Validate structured-ref fields per ADR 2026-04-19-1242.
    fn verify_plan_artifact_refs(
        &self,
        track_dir: Option<PathBuf>,
    ) -> Result<VerifyOutcome, VerifyPortError>;

    /// Verify catalogue-spec ref integrity (SoT Chain binary gate).
    fn verify_catalogue_spec_refs(
        &self,
        track_id: Option<TrackId>,
        items_dir: PathBuf,
        workspace_root: PathBuf,
        skip_stale: bool,
    ) -> Result<VerifyOutcome, VerifyPortError>;
}

/// Interactor that implements `VerifyService` by delegating to the injected `VerifyPort`.
pub struct VerifyInteractor {
    port: Arc<dyn VerifyPort>,
}

impl VerifyInteractor {
    /// Create a new `VerifyInteractor` wrapping the given `VerifyPort`.
    pub fn new(port: Arc<dyn VerifyPort>) -> Self {
        Self { port }
    }
}

impl VerifyService for VerifyInteractor {
    fn verify_latest_track(&self, project_root: PathBuf) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_latest_track(project_root.as_path())
    }

    fn verify_retention_gate(
        &self,
        project_root: PathBuf,
    ) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_retention_gate(project_root.as_path())
    }

    fn verify_sotp_version_tag(
        &self,
        project_root: &Path,
    ) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_sotp_version_tag(project_root)
    }

    fn verify_machine_paths(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_machine_paths(project_root)
    }

    fn verify_template_refs(&self, project_root: &Path) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_template_refs(project_root)
    }

    fn verify_arch_docs(&self, project_root: PathBuf) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_arch_docs(project_root.as_path())
    }

    fn verify_layers(&self, project_root: PathBuf) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_layers(project_root.as_path())
    }

    fn verify_hooks_path(&self, project_root: PathBuf) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_hooks_path(project_root.as_path())
    }

    fn verify_spec_attribution(
        &self,
        spec_path: PathBuf,
    ) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_spec_attribution(spec_path.as_path())
    }

    fn verify_spec_frontmatter(
        &self,
        spec_path: PathBuf,
    ) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_spec_frontmatter(spec_path.as_path())
    }

    fn verify_canonical_modules(
        &self,
        project_root: PathBuf,
    ) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_canonical_modules(project_root.as_path())
    }

    fn verify_module_size(&self, project_root: PathBuf) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_module_size(project_root.as_path())
    }

    fn verify_domain_purity(
        &self,
        project_root: PathBuf,
    ) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_domain_purity(project_root.as_path())
    }

    fn verify_domain_strings(
        &self,
        project_root: PathBuf,
    ) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_domain_strings(project_root.as_path())
    }

    fn verify_usecase_purity(
        &self,
        project_root: PathBuf,
    ) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_usecase_purity(project_root.as_path())
    }

    fn verify_doc_links(&self, project_root: PathBuf) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_doc_links(project_root.as_path())
    }

    fn verify_view_freshness(
        &self,
        project_root: PathBuf,
    ) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_view_freshness(project_root.as_path())
    }

    fn verify_spec_signals(&self, spec_path: PathBuf) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_spec_signals(spec_path.as_path())
    }

    fn verify_plan_artifact_refs(
        &self,
        track_dir: Option<PathBuf>,
    ) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_plan_artifact_refs(track_dir.as_deref())
    }

    fn verify_catalogue_spec_refs(
        &self,
        track_id: Option<TrackId>,
        items_dir: PathBuf,
        workspace_root: PathBuf,
        skip_stale: bool,
    ) -> Result<VerifyOutcome, VerifyPortError> {
        self.port.verify_catalogue_spec_refs(
            track_id.as_ref(),
            items_dir.as_path(),
            workspace_root.as_path(),
            skip_stale,
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Default)]
    struct RecordingPort {
        retention_root: Mutex<Option<PathBuf>>,
        verifier_roots: Mutex<Vec<(&'static str, PathBuf)>>,
        fail_verifier_routes: bool,
    }

    impl RecordingPort {
        fn unused(&self) -> Result<VerifyOutcome, VerifyPortError> {
            Err(VerifyPortError::Unavailable("unused test route".to_owned()))
        }

        fn record_verifier_route(
            &self,
            route: &'static str,
            project_root: &Path,
        ) -> Result<VerifyOutcome, VerifyPortError> {
            self.verifier_roots.lock().unwrap().push((route, project_root.to_path_buf()));
            if self.fail_verifier_routes {
                Err(VerifyPortError::Unavailable(format!("{route} unavailable")))
            } else {
                Ok(VerifyOutcome::success(Some(route.to_owned())))
            }
        }
    }

    macro_rules! unused_path_method {
        ($name:ident) => {
            fn $name(&self, _: &Path) -> Result<VerifyOutcome, VerifyPortError> {
                self.unused()
            }
        };
    }

    impl VerifyPort for RecordingPort {
        unused_path_method!(verify_latest_track);
        unused_path_method!(verify_arch_docs);
        unused_path_method!(verify_layers);
        unused_path_method!(verify_hooks_path);
        unused_path_method!(verify_spec_attribution);
        unused_path_method!(verify_spec_frontmatter);
        unused_path_method!(verify_canonical_modules);
        unused_path_method!(verify_module_size);
        unused_path_method!(verify_domain_purity);
        unused_path_method!(verify_domain_strings);
        unused_path_method!(verify_usecase_purity);
        unused_path_method!(verify_doc_links);
        unused_path_method!(verify_view_freshness);
        unused_path_method!(verify_spec_signals);

        fn verify_retention_gate(
            &self,
            project_root: &Path,
        ) -> Result<VerifyOutcome, VerifyPortError> {
            *self.retention_root.lock().unwrap() = Some(project_root.to_path_buf());
            Ok(VerifyOutcome::success(Some("ok".to_owned())))
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
            _: Option<&Path>,
        ) -> Result<VerifyOutcome, VerifyPortError> {
            self.unused()
        }

        fn verify_catalogue_spec_refs(
            &self,
            _: Option<&TrackId>,
            _: &Path,
            _: &Path,
            _: bool,
        ) -> Result<VerifyOutcome, VerifyPortError> {
            self.unused()
        }
    }

    #[test]
    fn test_verify_interactor_retention_gate_delegates_project_root() {
        let port = Arc::new(RecordingPort::default());
        let service_port: Arc<dyn VerifyPort> = port.clone();
        let interactor = VerifyInteractor::new(service_port);
        let root = PathBuf::from("workspace-root");

        let outcome = interactor.verify_retention_gate(root.clone()).unwrap();

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(port.retention_root.lock().unwrap().as_deref(), Some(root.as_path()));
    }

    #[test]
    fn test_verify_interactor_new_verifier_routes_delegate_borrowed_project_root() {
        let port = Arc::new(RecordingPort::default());
        let service_port: Arc<dyn VerifyPort> = port.clone();
        let interactor = VerifyInteractor::new(service_port);
        let root = PathBuf::from("workspace-root");

        assert_eq!(interactor.verify_sotp_version_tag(&root).unwrap().exit_code, 0);
        assert_eq!(interactor.verify_machine_paths(&root).unwrap().exit_code, 0);
        assert_eq!(interactor.verify_template_refs(&root).unwrap().exit_code, 0);

        assert_eq!(
            *port.verifier_roots.lock().unwrap(),
            vec![
                ("sotp_version_tag", root.clone()),
                ("machine_paths", root.clone()),
                ("template_refs", root),
            ]
        );
    }

    #[test]
    fn test_verify_interactor_new_verifier_route_transports_port_error() {
        let port = Arc::new(RecordingPort {
            retention_root: Mutex::new(None),
            verifier_roots: Mutex::new(Vec::new()),
            fail_verifier_routes: true,
        });
        let service_port: Arc<dyn VerifyPort> = port;
        let interactor = VerifyInteractor::new(service_port);

        let result = interactor.verify_machine_paths(Path::new("workspace-root"));

        assert!(
            matches!(result, Err(VerifyPortError::Unavailable(message)) if message == "machine_paths unavailable")
        );
    }
}
