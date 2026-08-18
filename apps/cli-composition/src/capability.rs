//! Composition root for generic capability dispatch.

use std::path::PathBuf;
use std::sync::Arc;

use domain::TrackId;
use infrastructure::agent_profiles::AGENT_PROFILES_PATH;
use infrastructure::capability_exec::{
    agent_profiles::AgentProfilesCapabilityAdapter, claude::ClaudeCapabilityAdapter,
    codex::CodexCapabilityAdapter, grok::GrokCapabilityAdapter, source::FsCapabilitySourceAdapter,
};
use infrastructure::conventions_resolve::FsConventionRequirementAdapter;
use infrastructure::git_cli::SystemGitRepo;
use infrastructure::provider_session::FsProviderSessionCacheAdapter;
use usecase::capability_exec::{
    CapabilityExecInteractor, CapabilityProfilePort, CapabilityProviderPort, CapabilitySourcePort,
};
use usecase::conventions_resolve::{
    ConventionRequirementPort, ConventionResolveInteractor, ConventionResolveService,
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
                session_cache.clone(),
                track_id.clone(),
            )),
            Arc::new(GrokCapabilityAdapter::new(
                self.repo_root.clone(),
                self.runtime_dir.clone(),
                session_cache,
                track_id,
            )),
        ];
        // Convention resolution for the dispatch preflight, assembled from the
        // same read-only scan the `conventions resolve` command uses. Both
        // callers go through one service, so the documents a dispatched
        // capability is told to read are the documents that command reports for
        // it; the filesystem side is the requirement scan and nothing else, so
        // this wiring gives the dispatcher no route to writing a convention
        // document.
        let conventions: Arc<dyn ConventionResolveService> = Arc::new(
            ConventionResolveInteractor::new(Arc::new(FsConventionRequirementAdapter::new())
                as Arc<dyn ConventionRequirementPort>),
        );
        // The preflight scans the discovered repository root, the same value the
        // adapters above resolve their repository-relative inputs against, so a
        // dispatch issued from a subdirectory reads the repository's conventions
        // rather than whatever tree the process happens to sit in.
        let service = Arc::new(CapabilityExecInteractor::new(
            profile,
            source,
            conventions,
            providers,
            self.repo_root.clone(),
        ));

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
    use std::ffi::OsString;
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
                "providers": {{
                    "{provider}": {{
                        "label": "Test provider",
                        "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"]
                    }}
                }},
                "capabilities": {{
                    "orchestrator": {{
                        "provider": "{provider}",
                        "model": "{model}",
                        "reasoning_effort": "high",
                        "execution_mode": "typed-pipeline"
                    }},
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
            host: Some(ProviderNameArg::from_str(host).expect("valid test provider")),
            briefing_file: CapabilityFilePathArg::from_str("tmp/briefing.md")
                .expect("valid test briefing path"),
            timeout_seconds: None,
            resume: cli_driver::capability::CapabilityResumeArg::Fresh,
        }
    }

    fn input() -> CapabilityExecDriverInput {
        input_for("implementer", "claude")
    }

    fn input_without_host(capability: &str) -> CapabilityExecDriverInput {
        CapabilityExecDriverInput {
            capability: CapabilityNameArg::from_str(capability).expect("valid test capability"),
            host: None,
            briefing_file: CapabilityFilePathArg::from_str("tmp/briefing.md")
                .expect("valid test briefing path"),
            timeout_seconds: None,
            resume: cli_driver::capability::CapabilityResumeArg::Fresh,
        }
    }

    fn process_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct PathGuard {
        previous: Option<OsString>,
    }

    impl PathGuard {
        fn prepend(directory: &Path) -> Self {
            let previous = std::env::var_os("PATH");
            let mut value = directory.as_os_str().to_os_string();
            value.push(":");
            value.push(previous.clone().unwrap_or_default());
            // Safety: the caller holds process_env_lock for this guard's full lifetime.
            unsafe { std::env::set_var("PATH", value) };
            Self { previous }
        }
    }

    impl Drop for PathGuard {
        fn drop(&mut self) {
            // Safety: the test that owns this guard holds process_env_lock until drop completes.
            unsafe {
                match self.previous.as_deref() {
                    Some(value) => std::env::set_var("PATH", value),
                    None => std::env::remove_var("PATH"),
                }
            }
        }
    }

    #[cfg(unix)]
    fn write_fake_claude_bin(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let bin_dir = root.join("fake-bin");
        fs::create_dir_all(&bin_dir)?;
        let claude = bin_dir.join("claude");
        fs::write(&claude, "#!/bin/sh\nexit 23\n")?;
        let mut permissions = fs::metadata(&claude)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&claude, permissions)?;
        Ok(bin_dir)
    }

    #[cfg(unix)]
    fn write_fake_grok_bin(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let bin_dir = root.join("fake-bin");
        fs::create_dir_all(&bin_dir)?;
        let grok = bin_dir.join("grok");
        fs::write(
            &grok,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > tmp/grok-args\nprintf '%s\\n' '{\"sessionId\":\"grok-test-session\",\"structured_output\":{\"result\":\"ok\"}}'\nexit 23\n",
        )?;
        let mut permissions = fs::metadata(&grok)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&grok, permissions)?;
        Ok(bin_dir)
    }

    fn write_grok_dispatch_fixture(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
        write_file(
            root,
            ".harness/config/agent-profiles.json",
            r#"{
                "schema_version": 1,
                "providers": {
                    "grok": {
                        "label": "Test Grok provider",
                        "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"]
                    }
                },
                "capabilities": {
                    "implementer": {
                        "provider": "grok",
                        "model": "grok-test-model",
                        "reasoning_effort": "high",
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
        write_file(
            root,
            ".agents/skills/implementer/SKILL.md",
            "---\nname: implementer\ndescription: Shared Grok adapter fixture.\nmodel: grok-test-model\ngrok-sandbox: workspace\n---\nshared adapter body\n",
        )?;
        Ok(())
    }

    #[test]
    fn test_capability_composition_root_builds_driver_without_a_caller_profile()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        initialize_git_repository(directory.path())?;
        write_file(
            directory.path(),
            ".harness/config/agent-profiles.json",
            r#"{
                "schema_version": 1,
                "providers": {},
                "capabilities": {
                    "implementer": {
                        "provider": "claude",
                        "model": "claude-opus",
                        "reasoning_effort": "high",
                        "execution_mode": "orchestrator-output"
                    }
                }
            }"#,
        )?;
        let root = CapabilityCompositionRoot::new(
            directory.path().to_owned(),
            directory.path().join("tmp/capability-runtime"),
        );

        let driver = root.capability_driver();

        assert!(std::mem::size_of_val(&driver) > 0);
        Ok(())
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
    fn test_capability_composition_root_driver_allows_a_root_without_a_repository()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = CapabilityCompositionRoot::new(
            directory.path().to_owned(),
            directory.path().join("tmp/capability-runtime"),
        );

        let driver = root.capability_driver();

        assert!(std::mem::size_of_val(&driver) > 0);
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
        let input = input();
        assert_eq!(
            input.host,
            Some(ProviderNameArg::from_str("claude").expect("valid test provider")),
            "the matching provider host is explicitly supplied rather than inferred"
        );
        let outcome = root.capability_driver().handle(input);

        assert_eq!(root.repo_root, repository.path());
        assert_eq!(root.runtime_dir, repository.path().join(CAPABILITY_RUNTIME_DIR));
        assert_eq!(outcome.exit_code, 0);
        Ok(())
    }

    #[test]
    fn test_capability_composition_root_dispatches_valid_claude_host_in_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        initialize_git_repository(directory.path())?;
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

    #[cfg(unix)]
    #[test]
    fn test_capability_composition_root_omitted_host_executes_requested_provider_subprocess()
    -> Result<(), Box<dyn std::error::Error>> {
        let _environment = process_env_lock().lock().expect("process environment lock is acquired");
        let directory = tempfile::tempdir()?;
        initialize_git_repository(directory.path())?;
        write_file(
            directory.path(),
            ".harness/config/agent-profiles.json",
            r#"{
                "schema_version": 1,
                "providers": {
                    "claude": {
                        "label": "Test provider",
                        "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"]
                    }
                },
                "capabilities": {
                    "orchestrator": {
                        "provider": "codex",
                        "model": "gpt-5",
                        "reasoning_effort": "high",
                        "execution_mode": "typed-pipeline"
                    },
                    "implementer": {
                        "provider": "claude",
                        "model": "claude-opus",
                        "reasoning_effort": "high",
                        "execution_mode": "orchestrator-output"
                    }
                }
            }"#,
        )?;
        write_file(
            directory.path(),
            ".harness/prompts/capability-exec-discipline.md",
            "Do not stage changes.",
        )?;
        write_file(directory.path(), "tmp/briefing.md", "Implement the assigned task.")?;
        write_file(
            directory.path(),
            ".claude/agents/implementer.md",
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools: Read\n---\nagent body\n",
        )?;
        let bin_dir = write_fake_claude_bin(directory.path())?;
        let _path = PathGuard::prepend(&bin_dir);
        let root = CapabilityCompositionRoot::new(
            directory.path().to_owned(),
            directory.path().join("tmp/capability-runtime"),
        );

        let outcome = root.capability_driver().handle(input_without_host("implementer"));
        let output = outcome.stdout.expect("subprocess outcome is rendered");

        assert_eq!(outcome.exit_code, 23);
        assert!(output.contains("CAPABILITY_EXEC_OUTCOME: executed"));
        assert!(output.contains("provider: claude"));
        assert!(!output.contains("delegate-in-host"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_capability_composition_root_omitted_host_executes_grok_adapter_subprocess()
    -> Result<(), Box<dyn std::error::Error>> {
        let _environment = process_env_lock().lock().expect("process environment lock is acquired");
        let directory = tempfile::tempdir()?;
        write_grok_dispatch_fixture(directory.path())?;
        let bin_dir = write_fake_grok_bin(directory.path())?;
        let _path = PathGuard::prepend(&bin_dir);
        let root = CapabilityCompositionRoot::new(
            directory.path().to_owned(),
            directory.path().join("tmp/capability-runtime"),
        );

        let outcome = root.capability_driver().handle(input_without_host("implementer"));
        let output = outcome.stdout.expect("subprocess outcome is rendered");
        let args = fs::read_to_string(directory.path().join("tmp/grok-args"))?;

        assert_eq!(outcome.exit_code, 23);
        assert!(output.contains("CAPABILITY_EXEC_OUTCOME: executed"));
        assert!(output.contains("provider: grok"));
        assert!(!output.contains("delegate-in-host"));
        assert!(args.contains("--model\ngrok-test-model\n"));
        assert!(args.contains("--reasoning-effort\nhigh\n"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_capability_composition_root_grok_host_executes_grok_adapter_subprocess_without_delegation()
    -> Result<(), Box<dyn std::error::Error>> {
        let _environment = process_env_lock().lock().expect("process environment lock is acquired");
        let directory = tempfile::tempdir()?;
        write_grok_dispatch_fixture(directory.path())?;
        let bin_dir = write_fake_grok_bin(directory.path())?;
        let _path = PathGuard::prepend(&bin_dir);
        let root = CapabilityCompositionRoot::new(
            directory.path().to_owned(),
            directory.path().join("tmp/capability-runtime"),
        );

        let outcome = root.capability_driver().handle(input_for("implementer", "grok"));
        let output = outcome.stdout.expect("subprocess outcome is rendered");
        let args = fs::read_to_string(directory.path().join("tmp/grok-args"))?;

        assert_eq!(outcome.exit_code, 23);
        assert!(output.contains("CAPABILITY_EXEC_OUTCOME: executed"));
        assert!(output.contains("provider: grok"));
        assert!(!output.contains("delegate-in-host"));
        assert!(args.contains("--model\ngrok-test-model\n"));
        assert!(args.contains("--reasoning-effort\nhigh\n"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_capability_composition_root_explicit_mismatched_host_executes_subprocess()
    -> Result<(), Box<dyn std::error::Error>> {
        let _environment = process_env_lock().lock().expect("process environment lock is acquired");
        let directory = tempfile::tempdir()?;
        write_file(
            directory.path(),
            ".harness/config/agent-profiles.json",
            r#"{
                "schema_version": 1,
                "providers": {
                    "claude": {
                        "label": "Test provider",
                        "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"]
                    }
                },
                "capabilities": {
                    "orchestrator": {
                        "provider": "codex",
                        "model": "gpt-5",
                        "reasoning_effort": "high",
                        "execution_mode": "typed-pipeline"
                    },
                    "implementer": {
                        "provider": "claude",
                        "model": "claude-opus",
                        "reasoning_effort": "high",
                        "execution_mode": "orchestrator-output"
                    }
                }
            }"#,
        )?;
        write_file(
            directory.path(),
            ".harness/prompts/capability-exec-discipline.md",
            "Do not stage changes.",
        )?;
        write_file(directory.path(), "tmp/briefing.md", "Implement the assigned task.")?;
        write_file(
            directory.path(),
            ".claude/agents/implementer.md",
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools: Read\n---\nagent body\n",
        )?;
        let bin_dir = write_fake_claude_bin(directory.path())?;
        let _path = PathGuard::prepend(&bin_dir);
        let root = CapabilityCompositionRoot::new(
            directory.path().to_owned(),
            directory.path().join("tmp/capability-runtime"),
        );
        let input = input_for("implementer", "codex");
        assert_eq!(
            input.host,
            Some(ProviderNameArg::from_str("codex").expect("valid test provider")),
            "the mismatched caller host reaches dispatch without normalization"
        );

        let outcome = root.capability_driver().handle(input);
        let output = outcome.stdout.expect("subprocess outcome is rendered");

        assert_eq!(outcome.exit_code, 23);
        assert!(output.contains("CAPABILITY_EXEC_OUTCOME: executed"));
        assert!(output.contains("provider: claude"));
        assert!(!output.contains("delegate-in-host"));
        Ok(())
    }

    #[test]
    fn test_capability_composition_root_dispatch_names_the_repository_conventions()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        initialize_git_repository(directory.path())?;
        write_dispatch_fixture(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools:\n  - Read\n---\nagent body\n",
        )?;
        // Declared for the dispatched capability, and named so that no document
        // of this repository could be mistaken for it: a path reaching the
        // delegation payload can only have come from the tree wired in as the
        // repository root.
        write_file(
            directory.path(),
            "knowledge/conventions/wiring-probe-capability-exec.md",
            "---\nrequired_for:\n  - implementer\n---\n\n# Probe\n",
        )?;
        let root = CapabilityCompositionRoot::new(
            directory.path().to_owned(),
            directory.path().join("tmp/capability-runtime"),
        );

        let outcome = root.capability_driver().handle(input());
        let output = outcome.stdout.expect("in-host instruction is rendered");

        assert_eq!(outcome.exit_code, 0);
        assert!(
            output.contains("knowledge/conventions/wiring-probe-capability-exec.md"),
            "the dispatch resolves conventions against the repository root this root was built \
             with, and the delegation payload carries what it found: {output}"
        );
        Ok(())
    }

    #[test]
    fn test_capability_composition_root_profile_resolution_yields_resolved_in_host_route()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        initialize_git_repository(directory.path())?;
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
        initialize_git_repository(directory.path())?;
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
