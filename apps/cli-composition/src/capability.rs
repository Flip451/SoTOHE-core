//! Composition root for generic capability dispatch.

use std::path::PathBuf;
use std::sync::Arc;

use infrastructure::agent_profiles::AGENT_PROFILES_PATH;
use infrastructure::capability_exec::{
    agent_profiles::AgentProfilesCapabilityAdapter, claude::ClaudeCapabilityAdapter,
    codex::CodexCapabilityAdapter, source::FsCapabilitySourceAdapter,
};
use usecase::capability_exec::{
    CapabilityExecInteractor, CapabilityProfilePort, CapabilityProviderPort, CapabilitySourcePort,
};

/// Composition root for `sotp capability` commands.
pub struct CapabilityCompositionRoot {
    repo_root: PathBuf,
    runtime_dir: PathBuf,
}

impl CapabilityCompositionRoot {
    /// Creates a root using an explicit repository root and capability runtime directory.
    #[must_use]
    pub fn new(repo_root: PathBuf, runtime_dir: PathBuf) -> Self {
        Self { repo_root, runtime_dir }
    }

    /// Builds the generic capability driver with both supported provider adapters.
    #[must_use]
    pub fn capability_driver(&self) -> cli_driver::capability::CapabilityDriver {
        let profile: Arc<dyn CapabilityProfilePort> =
            Arc::new(AgentProfilesCapabilityAdapter::new(
                self.repo_root.clone(),
                self.repo_root.join(AGENT_PROFILES_PATH),
            ));
        let source: Arc<dyn CapabilitySourcePort> =
            Arc::new(FsCapabilitySourceAdapter::new(self.repo_root.clone()));
        let providers: Vec<Arc<dyn CapabilityProviderPort>> = vec![
            Arc::new(ClaudeCapabilityAdapter::new(
                self.repo_root.clone(),
                self.runtime_dir.clone(),
            )),
            Arc::new(CodexCapabilityAdapter::new(self.repo_root.clone(), self.runtime_dir.clone())),
        ];
        let service = Arc::new(CapabilityExecInteractor::new(profile, source, providers));

        cli_driver::capability::CapabilityDriver::new(service)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::str::FromStr;

    use super::CapabilityCompositionRoot;
    use cli_driver::capability::{
        CapabilityExecDriverInput, CapabilityFilePathArg, CapabilityNameArg, ProviderNameArg,
    };

    fn write_file(
        root: &std::path::Path,
        relative_path: &str,
        contents: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)?;
        Ok(())
    }

    fn write_dispatch_fixture(
        root: &std::path::Path,
        agent_definition: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        write_file(
            root,
            ".harness/config/agent-profiles.json",
            r#"{
                "schema_version": 1,
                "providers": {},
                "capabilities": {
                    "implementer": {
                        "provider": "claude",
                        "model": "claude-opus",
                        "execution_mode": "orchestrator-output"
                    }
                }
            }"#,
        )?;
        write_file(
            root,
            ".harness/prompts/capability-exec-discipline.md",
            "Do not stage changes.",
        )?;
        write_file(root, "tmp/briefing.md", "Implement the assigned task.")?;
        write_file(root, ".claude/agents/implementer.md", agent_definition)?;
        Ok(())
    }

    fn input() -> CapabilityExecDriverInput {
        CapabilityExecDriverInput {
            capability: CapabilityNameArg::from_str("implementer").expect("valid test capability"),
            host: ProviderNameArg::from_str("claude").expect("valid test provider"),
            briefing_file: CapabilityFilePathArg::from_str("tmp/briefing.md")
                .expect("valid test briefing path"),
        }
    }

    #[test]
    fn test_capability_composition_root_builds_generic_driver() {
        let root = CapabilityCompositionRoot::new(
            PathBuf::from("/repo"),
            PathBuf::from("/repo/tmp/capability-runtime"),
        );

        let driver = root.capability_driver();

        assert!(std::mem::size_of_val(&driver) > 0);
    }

    #[test]
    fn test_capability_composition_root_dispatches_valid_claude_host_in_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_dispatch_fixture(
            directory.path(),
            "---\nname: implementer\nmodel: claude-opus\ntools:\n  - Read\n---\nagent body\n",
        )?;
        let root = CapabilityCompositionRoot::new(
            directory.path().to_owned(),
            directory.path().join("tmp/capability-runtime"),
        );

        let outcome = root.capability_driver().handle(input());
        let output = outcome.stdout.expect("in-host instruction is rendered");

        assert_eq!(outcome.exit_code, 0);
        assert!(output.contains("CAPABILITY_EXEC_OUTCOME: delegate-in-host"));
        assert!(output.contains("capability: implementer"));
        assert!(output.contains("briefing_file: tmp/briefing.md"));
        assert!(output.contains("Do not stage changes."));
        Ok(())
    }

    #[test]
    fn test_capability_composition_root_rejects_claude_agent_without_tools()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_dispatch_fixture(
            directory.path(),
            "---\nname: implementer\nmodel: claude-opus\n---\nagent body\n",
        )?;
        let root = CapabilityCompositionRoot::new(
            directory.path().to_owned(),
            directory.path().join("tmp/capability-runtime"),
        );

        let outcome = root.capability_driver().handle(input());

        assert_ne!(outcome.exit_code, 0);
        assert!(outcome.stderr.as_deref().is_some_and(|error| error.contains("tools field")));
        Ok(())
    }
}
