//! Composition root for generic capability dispatch.

use std::path::PathBuf;
use std::sync::Arc;

use domain::TrackId;
use infrastructure::agent_profiles::AGENT_PROFILES_PATH;
use infrastructure::capability_exec::{
    agent_profiles::AgentProfilesCapabilityAdapter, claude::ClaudeCapabilityAdapter,
    codex::CodexCapabilityAdapter, source::FsCapabilitySourceAdapter,
};
use infrastructure::git_cli::SystemGitRepo;
use infrastructure::provider_session::FsProviderSessionCacheAdapter;
use usecase::capability_exec::{
    CapabilityExecInteractor, CapabilityProfilePort, CapabilityProviderPort, CapabilitySourcePort,
};

const CAPABILITY_RUNTIME_DIR: &str = "tmp/capability-runtime";

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

    /// Discovers the current git worktree and builds the default root.
    ///
    /// # Errors
    ///
    /// Returns an infrastructure error when the current directory is not inside
    /// a discoverable git worktree.
    pub fn discover() -> Result<Self, crate::CompositionError> {
        let repo = SystemGitRepo::discover().map_err(|e| {
            crate::CompositionError::Infrastructure(format!("cannot discover git repo: {e}"))
        })?;
        let repo_root = repo.root().to_path_buf();
        Ok(Self::new(repo_root.clone(), repo_root.join(CAPABILITY_RUNTIME_DIR)))
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
        let session_cache = Arc::new(FsProviderSessionCacheAdapter::new(
            self.repo_root.clone(),
            self.runtime_dir.clone(),
        ));
        let track_id = self.current_track_id();
        let providers: Vec<Arc<dyn CapabilityProviderPort>> = vec![
            Arc::new(ClaudeCapabilityAdapter::new(
                self.repo_root.clone(),
                self.runtime_dir.clone(),
                session_cache.clone(),
                track_id.clone(),
            )),
            Arc::new(CodexCapabilityAdapter::new(
                self.repo_root.clone(),
                self.runtime_dir.clone(),
                session_cache,
                track_id,
            )),
        ];
        let service = Arc::new(CapabilityExecInteractor::new(profile, source, providers));

        cli_driver::capability::CapabilityDriver::new(service)
    }

    fn current_track_id(&self) -> Option<TrackId> {
        let branch =
            SystemGitRepo::discover_from(&self.repo_root).ok()?.current_branch().ok()??;
        TrackId::try_new(branch.strip_prefix("track/")?.to_owned()).ok()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::str::FromStr;
    use std::sync::{Mutex, OnceLock};

    use super::{CAPABILITY_RUNTIME_DIR, CapabilityCompositionRoot};
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
        write_profile_dispatch_fixture(
            root,
            "implementer",
            "claude",
            "claude-opus",
            agent_definition,
        )
    }

    fn write_profile_dispatch_fixture(
        root: &std::path::Path,
        capability: &str,
        provider: &str,
        model: &str,
        agent_definition: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        write_file(
            root,
            ".harness/config/agent-profiles.json",
            &format!(
                r#"{{
                "schema_version": 1,
                "providers": {{}},
                "capabilities": {{
                    "{capability}": {{
                        "provider": "{provider}",
                        "model": "{model}",
                        "reasoning_effort": "high",
                        "execution_mode": "orchestrator-output"
                    }}
                }}
            }}"#,
            ),
        )?;
        write_file(
            root,
            ".harness/prompts/capability-exec-discipline.md",
            "Do not stage changes.",
        )?;
        write_file(root, "tmp/briefing.md", "Implement the assigned task.")?;
        write_file(root, &format!(".claude/agents/{capability}.md"), agent_definition)?;
        Ok(())
    }

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct CurrentDirGuard {
        original: PathBuf,
    }

    impl CurrentDirGuard {
        fn change_to(path: &Path) -> Result<Self, std::io::Error> {
            let original = std::env::current_dir()?;
            std::env::set_current_dir(path)?;
            Ok(Self { original })
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    fn initialize_git_repository(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let status = Command::new("git").args(["init", "-q"]).current_dir(root).status()?;
        if status.success() { Ok(()) } else { Err(format!("git init failed with {status}").into()) }
    }

    fn input_for(capability: &str, host: &str) -> CapabilityExecDriverInput {
        CapabilityExecDriverInput {
            capability: CapabilityNameArg::from_str(capability).expect("valid test capability"),
            host: ProviderNameArg::from_str(host).expect("valid test provider"),
            briefing_file: CapabilityFilePathArg::from_str("tmp/briefing.md")
                .expect("valid test briefing path"),
            timeout_seconds: None,
            resume: cli_driver::capability::CapabilityResumeArg::Fresh,
        }
    }

    fn input() -> CapabilityExecDriverInput {
        input_for("implementer", "claude")
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
    fn test_capability_composition_root_discover_rejects_non_repository_directory()
    -> Result<(), Box<dyn std::error::Error>> {
        let _lock = cwd_lock().lock().expect("current directory lock is acquired");
        let directory = tempfile::tempdir()?;
        let _cwd = CurrentDirGuard::change_to(directory.path())?;

        let error = match CapabilityCompositionRoot::discover() {
            Ok(_) => return Err("directory without a git repository must be rejected".into()),
            Err(error) => error,
        };

        assert!(matches!(error, crate::CompositionError::Infrastructure(_)));
        assert!(error.to_string().contains("cannot discover git repo"));
        Ok(())
    }

    #[test]
    fn test_capability_composition_root_discover_uses_repository_root_from_subdirectory()
    -> Result<(), Box<dyn std::error::Error>> {
        let _lock = cwd_lock().lock().expect("current directory lock is acquired");
        let repository = tempfile::tempdir()?;
        initialize_git_repository(repository.path())?;
        write_dispatch_fixture(
            repository.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools:\n  - Read\n---\nagent body\n",
        )?;
        let nested = repository.path().join("nested/workdir");
        fs::create_dir_all(&nested)?;
        let _cwd = CurrentDirGuard::change_to(&nested)?;

        let root = CapabilityCompositionRoot::discover()?;
        let outcome = root.capability_driver().handle(input());

        assert_eq!(root.repo_root, repository.path());
        assert_eq!(root.runtime_dir, repository.path().join(CAPABILITY_RUNTIME_DIR));
        assert_eq!(outcome.exit_code, 0);
        Ok(())
    }

    #[test]
    fn test_capability_composition_root_dispatches_valid_claude_host_in_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_dispatch_fixture(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools:\n  - Read\n---\nagent body\n",
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
    fn test_capability_composition_root_profile_resolution_yields_resolved_in_host_route()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_profile_dispatch_fixture(
            directory.path(),
            "researcher",
            "claude",
            "claude-profile-model",
            "---\nname: researcher\ndescription: Researches the workspace.\nmodel: claude-profile-model\ntools:\n  - Read\n---\nagent body\n",
        )?;
        let root = CapabilityCompositionRoot::new(
            directory.path().to_owned(),
            directory.path().join("tmp/capability-runtime"),
        );

        let outcome = root.capability_driver().handle(input_for("researcher", "claude"));
        let output = outcome.stdout.expect("resolved in-host instruction is rendered");

        assert_eq!(outcome.exit_code, 0);
        assert!(output.contains("CAPABILITY_EXEC_OUTCOME: delegate-in-host"));
        assert!(output.contains("capability: researcher"));
        Ok(())
    }

    #[test]
    fn test_capability_composition_root_rejects_claude_agent_without_tools()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_dispatch_fixture(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\n---\nagent body\n",
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
