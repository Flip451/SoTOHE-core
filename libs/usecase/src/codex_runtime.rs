//! Codex runtime provisioning use case.

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use domain::tddd::test_obligation::ids::DiagnosticMessage;

/// Reports a failure while provisioning the repository-local Codex runtime link.
#[derive(Debug)]
pub enum CodexRuntimeProvisionError {
    /// The requested project root cannot host a runtime link.
    ProjectRootInvalid(DiagnosticMessage),
    /// Neither the PATH candidate nor the public npm candidate was usable.
    NoUsableCandidate(DiagnosticMessage),
    /// The public npm prefix query could not be completed.
    NpmQueryFailed(DiagnosticMessage),
    /// The verified runtime link could not be refreshed.
    LinkUpdateFailed(DiagnosticMessage),
    /// The implicit project root could not be discovered from the invocation directory.
    ProjectRootDiscoveryFailed(CodexRuntimeProjectRootDiscoveryError),
}

impl fmt::Display for CodexRuntimeProvisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let detail = match self {
            Self::ProjectRootInvalid(detail)
            | Self::NoUsableCandidate(detail)
            | Self::NpmQueryFailed(detail)
            | Self::LinkUpdateFailed(detail) => detail.as_str(),
            Self::ProjectRootDiscoveryFailed(error) => return error.fmt(formatter),
        };
        formatter.write_str(detail)
    }
}

impl std::error::Error for CodexRuntimeProvisionError {}

/// Reports a failure while discovering the repository root for an implicit
/// Codex runtime provisioning request.
#[derive(Debug)]
pub enum CodexRuntimeProjectRootDiscoveryError {
    /// The process current directory is not within a discoverable Git worktree.
    GitRootDiscoveryFailed(DiagnosticMessage),
}

impl fmt::Display for CodexRuntimeProjectRootDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitRootDiscoveryFailed(detail) => formatter.write_str(detail.as_str()),
        }
    }
}

impl std::error::Error for CodexRuntimeProjectRootDiscoveryError {}

/// Secondary port for discovering the Git repository root when provisioning
/// omits an explicit project root.
pub trait CodexRuntimeProjectRootDiscoveryPort: Send + Sync {
    /// Discover the repository root from `start_directory`.
    ///
    /// # Errors
    ///
    /// Returns [`CodexRuntimeProjectRootDiscoveryError`] when Git-root
    /// discovery fails.
    fn discover_from(
        &self,
        start_directory: &Path,
    ) -> Result<std::path::PathBuf, CodexRuntimeProjectRootDiscoveryError>;
}

/// Secondary port that resolves, verifies, and links a Codex runtime.
pub trait CodexRuntimeProvisionPort: Send + Sync {
    /// Provision the repository-local Codex runtime link for the requested root or the
    /// repository discovered from `invocation_directory`.
    ///
    /// # Errors
    ///
    /// Returns [`CodexRuntimeProvisionError`] when the project root is invalid,
    /// no candidate passes verification, npm cannot be queried, or the link cannot be updated.
    fn provision(&self, project_root: &Path) -> Result<(), CodexRuntimeProvisionError>;
}

/// Application service for one-shot Codex runtime provisioning.
pub trait CodexRuntimeProvisionService: Send + Sync {
    /// Provision the repository-local Codex runtime link for `project_root`.
    ///
    /// # Errors
    ///
    /// Returns [`CodexRuntimeProvisionError`] from root discovery or the provisioning port.
    fn provision(
        &self,
        project_root: Option<&Path>,
        invocation_directory: &Path,
    ) -> Result<(), CodexRuntimeProvisionError>;
}

/// Interactor that delegates Codex runtime provisioning to its secondary port.
pub struct CodexRuntimeProvisionInteractor {
    provisioner: Arc<dyn CodexRuntimeProvisionPort>,
    project_root_discovery: Arc<dyn CodexRuntimeProjectRootDiscoveryPort>,
}

impl CodexRuntimeProvisionInteractor {
    /// Create an interactor with the provisioning and project-root discovery ports.
    #[must_use]
    pub fn new(
        provisioner: Arc<dyn CodexRuntimeProvisionPort>,
        project_root_discovery: Arc<dyn CodexRuntimeProjectRootDiscoveryPort>,
    ) -> CodexRuntimeProvisionInteractor {
        CodexRuntimeProvisionInteractor { provisioner, project_root_discovery }
    }
}

impl CodexRuntimeProvisionService for CodexRuntimeProvisionInteractor {
    fn provision(
        &self,
        project_root: Option<&Path>,
        invocation_directory: &Path,
    ) -> Result<(), CodexRuntimeProvisionError> {
        let discovered_project_root;
        let project_root = match project_root {
            Some(project_root) => project_root,
            None => {
                discovered_project_root = self
                    .project_root_discovery
                    .discover_from(invocation_directory)
                    .map_err(CodexRuntimeProvisionError::ProjectRootDiscoveryFailed)?;
                discovered_project_root.as_path()
            }
        };
        self.provisioner.provision(project_root)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use domain::tddd::test_obligation::ids::DiagnosticMessage;

    use super::{
        CodexRuntimeProjectRootDiscoveryError, CodexRuntimeProjectRootDiscoveryPort,
        CodexRuntimeProvisionError, CodexRuntimeProvisionInteractor, CodexRuntimeProvisionPort,
        CodexRuntimeProvisionService,
    };

    struct RecordingProvisioner {
        received_root: Mutex<Option<PathBuf>>,
        result: Result<(), CodexRuntimeProvisionError>,
    }

    impl CodexRuntimeProvisionPort for RecordingProvisioner {
        fn provision(&self, project_root: &Path) -> Result<(), CodexRuntimeProvisionError> {
            *self.received_root.lock().expect("test mutex must not be poisoned") =
                Some(project_root.to_path_buf());
            match &self.result {
                Ok(()) => Ok(()),
                Err(CodexRuntimeProvisionError::ProjectRootInvalid(detail)) => {
                    Err(CodexRuntimeProvisionError::ProjectRootInvalid(detail.clone()))
                }
                Err(CodexRuntimeProvisionError::NoUsableCandidate(detail)) => {
                    Err(CodexRuntimeProvisionError::NoUsableCandidate(detail.clone()))
                }
                Err(CodexRuntimeProvisionError::NpmQueryFailed(detail)) => {
                    Err(CodexRuntimeProvisionError::NpmQueryFailed(detail.clone()))
                }
                Err(CodexRuntimeProvisionError::LinkUpdateFailed(detail)) => {
                    Err(CodexRuntimeProvisionError::LinkUpdateFailed(detail.clone()))
                }
                Err(CodexRuntimeProvisionError::ProjectRootDiscoveryFailed(error)) => {
                    Err(CodexRuntimeProvisionError::ProjectRootDiscoveryFailed(
                        CodexRuntimeProjectRootDiscoveryError::GitRootDiscoveryFailed(
                            match error {
                                CodexRuntimeProjectRootDiscoveryError::GitRootDiscoveryFailed(
                                    detail,
                                ) => detail.clone(),
                            },
                        ),
                    ))
                }
            }
        }
    }

    fn diagnostic(text: &str) -> DiagnosticMessage {
        DiagnosticMessage::try_new(text.to_owned()).expect("test diagnostic must be valid")
    }

    #[test]
    fn test_provision_forwards_project_root_to_port() {
        let provisioner =
            Arc::new(RecordingProvisioner { received_root: Mutex::new(None), result: Ok(()) });
        let service = CodexRuntimeProvisionInteractor::new(
            provisioner.clone(),
            Arc::new(FixedProjectRootDiscovery { result: Ok(PathBuf::from("/unused")) }),
        );
        let project_root = Path::new("/workspace/project");

        service
            .provision(Some(project_root), Path::new("/invocation"))
            .expect("provisioning must succeed");

        assert_eq!(
            *provisioner.received_root.lock().expect("test mutex must not be poisoned"),
            Some(project_root.to_path_buf())
        );
    }

    #[test]
    fn test_provision_preserves_no_usable_candidate_error() {
        let service = CodexRuntimeProvisionInteractor::new(
            Arc::new(RecordingProvisioner {
                received_root: Mutex::new(None),
                result: Err(CodexRuntimeProvisionError::NoUsableCandidate(diagnostic(
                    "PATH and npm candidates failed verification",
                ))),
            }),
            Arc::new(FixedProjectRootDiscovery { result: Ok(PathBuf::from("/unused")) }),
        );

        let error = service
            .provision(Some(Path::new("/workspace/project")), Path::new("/invocation"))
            .unwrap_err();

        assert!(matches!(error, CodexRuntimeProvisionError::NoUsableCandidate(_)));
        assert_eq!(error.to_string(), "PATH and npm candidates failed verification");
    }

    struct FixedProjectRootDiscovery {
        result: Result<PathBuf, CodexRuntimeProjectRootDiscoveryError>,
    }

    impl CodexRuntimeProjectRootDiscoveryPort for FixedProjectRootDiscovery {
        fn discover_from(
            &self,
            _start_directory: &Path,
        ) -> Result<PathBuf, CodexRuntimeProjectRootDiscoveryError> {
            match &self.result {
                Ok(root) => Ok(root.clone()),
                Err(CodexRuntimeProjectRootDiscoveryError::GitRootDiscoveryFailed(detail)) => Err(
                    CodexRuntimeProjectRootDiscoveryError::GitRootDiscoveryFailed(detail.clone()),
                ),
            }
        }
    }

    struct RecordingProjectRootDiscovery {
        received_start_directory: Mutex<Option<PathBuf>>,
        root: PathBuf,
    }

    impl CodexRuntimeProjectRootDiscoveryPort for RecordingProjectRootDiscovery {
        fn discover_from(
            &self,
            start_directory: &Path,
        ) -> Result<PathBuf, CodexRuntimeProjectRootDiscoveryError> {
            *self.received_start_directory.lock().expect("test mutex must not be poisoned") =
                Some(start_directory.to_path_buf());
            Ok(self.root.clone())
        }
    }

    #[test]
    fn test_provision_explicit_root_bypasses_discovery() {
        let provisioner =
            Arc::new(RecordingProvisioner { received_root: Mutex::new(None), result: Ok(()) });
        let service = CodexRuntimeProvisionInteractor::new(
            provisioner.clone(),
            Arc::new(FixedProjectRootDiscovery {
                result: Err(CodexRuntimeProjectRootDiscoveryError::GitRootDiscoveryFailed(
                    diagnostic("discovery must not run"),
                )),
            }),
        );
        let explicit_root = Path::new("/workspace/explicit");

        service
            .provision(Some(explicit_root), Path::new("/workspace/nested"))
            .expect("provisioning must succeed");

        assert_eq!(
            *provisioner.received_root.lock().expect("test mutex must not be poisoned"),
            Some(explicit_root.to_path_buf())
        );
    }

    #[test]
    fn test_provision_discovers_root_from_invocation_directory() {
        let provisioner =
            Arc::new(RecordingProvisioner { received_root: Mutex::new(None), result: Ok(()) });
        let discovery = Arc::new(RecordingProjectRootDiscovery {
            received_start_directory: Mutex::new(None),
            root: PathBuf::from("/workspace/repository"),
        });
        let service = CodexRuntimeProvisionInteractor::new(provisioner.clone(), discovery.clone());
        let invocation_directory = Path::new("/workspace/repository/nested");

        service.provision(None, invocation_directory).expect("provisioning must succeed");

        assert_eq!(
            *provisioner.received_root.lock().expect("test mutex must not be poisoned"),
            Some(PathBuf::from("/workspace/repository"))
        );
        assert_eq!(
            *discovery.received_start_directory.lock().expect("test mutex must not be poisoned"),
            Some(invocation_directory.to_path_buf())
        );
    }

    #[test]
    fn test_provision_wraps_discovery_failure() {
        let service = CodexRuntimeProvisionInteractor::new(
            Arc::new(RecordingProvisioner { received_root: Mutex::new(None), result: Ok(()) }),
            Arc::new(FixedProjectRootDiscovery {
                result: Err(CodexRuntimeProjectRootDiscoveryError::GitRootDiscoveryFailed(
                    diagnostic("not in a Git worktree"),
                )),
            }),
        );

        let error =
            service.provision(None, Path::new("/outside")).expect_err("discovery must fail");

        assert!(matches!(
            error,
            CodexRuntimeProvisionError::ProjectRootDiscoveryFailed(
                CodexRuntimeProjectRootDiscoveryError::GitRootDiscoveryFailed(_)
            )
        ));
        assert_eq!(error.to_string(), "not in a Git worktree");
    }
}
