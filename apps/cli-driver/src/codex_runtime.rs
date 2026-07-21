//! `codex-runtime` command family primary adapter.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::codex_runtime::CodexRuntimeProvisionService;

use crate::render::CommandOutcome;

/// Typed input for Codex runtime provisioning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexRuntimeInput {
    /// Explicit repository root that receives the runtime link, when supplied.
    pub project_root: Option<PathBuf>,
}

/// Primary adapter for the Codex runtime provisioning command.
pub struct CodexRuntimeDriver {
    service: Arc<dyn CodexRuntimeProvisionService>,
}

impl CodexRuntimeDriver {
    /// Create a driver backed by the provisioning application service.
    #[must_use]
    pub fn new(service: Arc<dyn CodexRuntimeProvisionService>) -> CodexRuntimeDriver {
        CodexRuntimeDriver { service }
    }

    /// Handle a Codex runtime provisioning request.
    #[must_use]
    pub fn handle(&self, input: CodexRuntimeInput) -> CommandOutcome {
        let invocation_directory = match std::env::current_dir() {
            Ok(directory) => directory,
            Err(error) => {
                return CommandOutcome::failure(Some(format!(
                    "failed to determine Codex runtime invocation directory: {error}"
                )));
            }
        };
        match self.service.provision(input.project_root.as_deref(), &invocation_directory) {
            Ok(()) => CommandOutcome::success(Some("[OK] Codex runtime provisioned".to_owned())),
            Err(error) => CommandOutcome::failure(Some(error.to_string())),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use usecase::{
        DiagnosticMessage,
        codex_runtime::{CodexRuntimeProvisionError, CodexRuntimeProvisionService},
    };

    use super::{CodexRuntimeDriver, CodexRuntimeInput};

    struct RecordingService {
        received_request: Mutex<Option<(Option<PathBuf>, PathBuf)>>,
    }

    impl CodexRuntimeProvisionService for RecordingService {
        fn provision(
            &self,
            project_root: Option<&Path>,
            invocation_directory: &Path,
        ) -> Result<(), CodexRuntimeProvisionError> {
            *self.received_request.lock().expect("test mutex must not be poisoned") =
                Some((project_root.map(Path::to_path_buf), invocation_directory.to_path_buf()));
            Ok(())
        }
    }

    struct FailingService;

    impl CodexRuntimeProvisionService for FailingService {
        fn provision(
            &self,
            _project_root: Option<&Path>,
            _invocation_directory: &Path,
        ) -> Result<(), CodexRuntimeProvisionError> {
            let detail = DiagnosticMessage::try_new("no verified Codex candidate".to_owned())
                .expect("test diagnostic must be valid");
            Err(CodexRuntimeProvisionError::NoUsableCandidate(detail))
        }
    }

    #[test]
    fn test_handle_forwards_explicit_root_and_invocation_directory_once() {
        let service = Arc::new(RecordingService { received_request: Mutex::new(None) });
        let driver = CodexRuntimeDriver::new(service.clone());
        let invocation_directory =
            std::env::current_dir().expect("test invocation directory must be readable");

        let outcome = driver
            .handle(CodexRuntimeInput { project_root: Some(PathBuf::from("/explicit/project")) });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("[OK] Codex runtime provisioned"));
        assert_eq!(outcome.stderr, None);
        assert_eq!(
            *service.received_request.lock().expect("test mutex must not be poisoned"),
            Some((Some(PathBuf::from("/explicit/project")), invocation_directory))
        );
    }

    #[test]
    fn test_handle_forwards_omitted_root_and_invocation_directory_once() {
        let service = Arc::new(RecordingService { received_request: Mutex::new(None) });
        let driver = CodexRuntimeDriver::new(service.clone());
        let invocation_directory =
            std::env::current_dir().expect("test invocation directory must be readable");

        let outcome = driver.handle(CodexRuntimeInput { project_root: None });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            *service.received_request.lock().expect("test mutex must not be poisoned"),
            Some((None, invocation_directory))
        );
    }

    #[test]
    fn test_handle_service_failure_returns_typed_diagnostic() {
        let driver = CodexRuntimeDriver::new(Arc::new(FailingService));

        let outcome = driver.handle(CodexRuntimeInput { project_root: None });

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stdout, None);
        assert_eq!(outcome.stderr.as_deref(), Some("no verified Codex candidate"));
    }
}
