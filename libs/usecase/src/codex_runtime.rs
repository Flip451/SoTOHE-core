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
}

impl fmt::Display for CodexRuntimeProvisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let detail = match self {
            Self::ProjectRootInvalid(detail)
            | Self::NoUsableCandidate(detail)
            | Self::NpmQueryFailed(detail)
            | Self::LinkUpdateFailed(detail) => detail.as_str(),
        };
        formatter.write_str(detail)
    }
}

impl std::error::Error for CodexRuntimeProvisionError {}

/// Secondary port that resolves, verifies, and links a Codex runtime.
pub trait CodexRuntimeProvisionPort: Send + Sync {
    /// Provision the repository-local Codex runtime link for `project_root`.
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
    /// Returns [`CodexRuntimeProvisionError`] from the provisioning port.
    fn provision(&self, project_root: &Path) -> Result<(), CodexRuntimeProvisionError>;
}

/// Interactor that delegates Codex runtime provisioning to its secondary port.
pub struct CodexRuntimeProvisionInteractor {
    provisioner: Arc<dyn CodexRuntimeProvisionPort>,
}

impl CodexRuntimeProvisionInteractor {
    /// Create an interactor with the provisioning port implementation.
    #[must_use]
    pub fn new(provisioner: Arc<dyn CodexRuntimeProvisionPort>) -> CodexRuntimeProvisionInteractor {
        CodexRuntimeProvisionInteractor { provisioner }
    }
}

impl CodexRuntimeProvisionService for CodexRuntimeProvisionInteractor {
    fn provision(&self, project_root: &Path) -> Result<(), CodexRuntimeProvisionError> {
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
        let service = CodexRuntimeProvisionInteractor::new(provisioner.clone());
        let project_root = Path::new("/workspace/project");

        service.provision(project_root).expect("provisioning must succeed");

        assert_eq!(
            *provisioner.received_root.lock().expect("test mutex must not be poisoned"),
            Some(project_root.to_path_buf())
        );
    }

    #[test]
    fn test_provision_preserves_no_usable_candidate_error() {
        let service = CodexRuntimeProvisionInteractor::new(Arc::new(RecordingProvisioner {
            received_root: Mutex::new(None),
            result: Err(CodexRuntimeProvisionError::NoUsableCandidate(diagnostic(
                "PATH and npm candidates failed verification",
            ))),
        }));

        let error = service.provision(Path::new("/workspace/project")).unwrap_err();

        assert!(matches!(error, CodexRuntimeProvisionError::NoUsableCandidate(_)));
        assert_eq!(error.to_string(), "PATH and npm candidates failed verification");
    }
}
