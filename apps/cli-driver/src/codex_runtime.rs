//! `codex-runtime` command family primary adapter.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::codex_runtime::CodexRuntimeProvisionService;

use crate::render::CommandOutcome;

/// Typed input for Codex runtime provisioning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexRuntimeInput {
    /// Repository root that receives the runtime link.
    pub project_root: PathBuf,
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
        match self.service.provision(input.project_root.as_path()) {
            Ok(()) => CommandOutcome::success(Some("[OK] Codex runtime provisioned".to_owned())),
            Err(error) => CommandOutcome::failure(Some(error.to_string())),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use usecase::{
        DiagnosticMessage,
        codex_runtime::{CodexRuntimeProvisionError, CodexRuntimeProvisionService},
    };

    use super::{CodexRuntimeDriver, CodexRuntimeInput};

    struct SucceedingService;

    impl CodexRuntimeProvisionService for SucceedingService {
        fn provision(&self, _project_root: &Path) -> Result<(), CodexRuntimeProvisionError> {
            Ok(())
        }
    }

    struct FailingService;

    impl CodexRuntimeProvisionService for FailingService {
        fn provision(&self, _project_root: &Path) -> Result<(), CodexRuntimeProvisionError> {
            let detail = DiagnosticMessage::try_new("no verified Codex candidate".to_owned())
                .expect("test diagnostic must be valid");
            Err(CodexRuntimeProvisionError::NoUsableCandidate(detail))
        }
    }

    #[test]
    fn test_handle_success_returns_provisioned_outcome() {
        let driver = CodexRuntimeDriver::new(Arc::new(SucceedingService));

        let outcome = driver.handle(CodexRuntimeInput { project_root: "/workspace".into() });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("[OK] Codex runtime provisioned"));
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    fn test_handle_service_failure_returns_diagnostic() {
        let driver = CodexRuntimeDriver::new(Arc::new(FailingService));

        let outcome = driver.handle(CodexRuntimeInput { project_root: "/workspace".into() });

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stdout, None);
        assert_eq!(outcome.stderr.as_deref(), Some("no verified Codex candidate"));
    }
}
