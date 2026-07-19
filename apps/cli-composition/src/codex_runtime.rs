//! `codex-runtime` command-family composition root.

use std::sync::Arc;

/// Composition root for filesystem-backed Codex runtime provisioning.
pub struct CodexRuntimeCompositionRoot;

impl CodexRuntimeCompositionRoot {
    /// Create a new `CodexRuntimeCompositionRoot`.
    #[must_use]
    pub fn new() -> CodexRuntimeCompositionRoot {
        CodexRuntimeCompositionRoot
    }

    /// Wire the filesystem provisioning stack into the Codex runtime driver and
    /// return it without invoking it.
    #[must_use]
    pub fn codex_runtime_driver(&self) -> cli_driver::codex_runtime::CodexRuntimeDriver {
        use infrastructure::codex_runtime::{
            FsCodexRuntimeProvisioner, GitCodexRuntimeProjectRootDiscoveryAdapter,
        };
        use usecase::codex_runtime::{
            CodexRuntimeProjectRootDiscoveryPort, CodexRuntimeProvisionInteractor,
            CodexRuntimeProvisionPort, CodexRuntimeProvisionService,
        };

        let provisioner =
            Arc::new(FsCodexRuntimeProvisioner::new()) as Arc<dyn CodexRuntimeProvisionPort>;
        let project_root_discovery_port = Arc::new(GitCodexRuntimeProjectRootDiscoveryAdapter::new())
            as Arc<dyn CodexRuntimeProjectRootDiscoveryPort>;
        let service = Arc::new(CodexRuntimeProvisionInteractor::new(
            provisioner,
            project_root_discovery_port,
        )) as Arc<dyn CodexRuntimeProvisionService>;
        cli_driver::codex_runtime::CodexRuntimeDriver::new(service)
    }
}

impl Default for CodexRuntimeCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use cli_driver::codex_runtime::CodexRuntimeInput;

    use super::CodexRuntimeCompositionRoot;

    #[test]
    fn test_codex_runtime_driver_wires_filesystem_adapter_and_reports_invalid_root() {
        let fixture = tempfile::tempdir().expect("fixture must be created");
        let outcome = CodexRuntimeCompositionRoot::new()
            .codex_runtime_driver()
            .handle(CodexRuntimeInput { project_root: Some(fixture.path().join("missing")) });

        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stdout.is_none());
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|message| message.contains("project root is not a directory"))
        );
    }
}
