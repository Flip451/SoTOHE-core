//! Codex provider-native adapter for generic capability dispatch.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use usecase::capability_exec::{
    CODEX_PROVIDER_NAME, CapabilityDispatchOutcome, CapabilityDispatchRequest, CapabilityExecError,
    CapabilityProviderPort, ProviderName,
};

use super::{
    ProviderProcessRunner, adapter_preflight_error, capability_prompt, dispatch_error,
    parse_provider_definition_front_matter, read_front_matter, read_utf8_file,
    system_process_runner,
};

/// Codex sandbox vocabulary declared by a provider-native skill definition.
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    /// Codex receives no workspace-write permission.
    ReadOnly,
    /// Codex may write within the workspace sandbox.
    WorkspaceWrite,
}

impl Copy for SandboxMode {}

impl Clone for SandboxMode {
    fn clone(&self) -> Self {
        *self
    }
}

impl SandboxMode {
    fn as_cli_value(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
        }
    }
}

/// Dispatches through a named repository Codex skill.
pub struct CodexCapabilityAdapter {
    repo_root: PathBuf,
    runtime_dir: PathBuf,
    provider: ProviderName,
    process_runner: Arc<dyn ProviderProcessRunner>,
}

impl CodexCapabilityAdapter {
    /// Creates a Codex adapter rooted at `repo_root` with logs under `runtime_dir`.
    #[must_use]
    pub fn new(repo_root: PathBuf, runtime_dir: PathBuf) -> Self {
        Self {
            repo_root,
            runtime_dir,
            provider: CODEX_PROVIDER_NAME.clone(),
            process_runner: system_process_runner(),
        }
    }

    #[cfg(test)]
    fn with_process_runner(
        repo_root: PathBuf,
        runtime_dir: PathBuf,
        process_runner: Arc<dyn ProviderProcessRunner>,
    ) -> Self {
        Self { repo_root, runtime_dir, provider: CODEX_PROVIDER_NAME.clone(), process_runner }
    }

    fn skill_path(&self, request: &CapabilityDispatchRequest) -> PathBuf {
        self.repo_root
            .join(".agents")
            .join("skills")
            .join(request.request.capability.as_str())
            .join("SKILL.md")
    }

    fn sandbox_mode(
        &self,
        request: &CapabilityDispatchRequest,
    ) -> Result<SandboxMode, CapabilityExecError> {
        let path = self.skill_path(request);
        let definition = read_utf8_file(&path, &self.repo_root)
            .map_err(|detail| adapter_preflight_error(request, &self.provider, detail))?;
        sandbox_mode_from_skill(&definition)
            .map_err(|detail| adapter_preflight_error(request, &self.provider, detail))
    }
}

impl CapabilityProviderPort for CodexCapabilityAdapter {
    fn provider(&self) -> &ProviderName {
        &self.provider
    }

    fn dispatch(
        &self,
        request: &CapabilityDispatchRequest,
    ) -> Result<CapabilityDispatchOutcome, CapabilityExecError> {
        let sandbox = self.sandbox_mode(request)?;
        let prompt = capability_prompt(request);
        let args = build_codex_args(request.profile.model.as_str(), sandbox, &prompt);
        let exit_code = self
            .process_runner
            .run("codex", &args, &self.repo_root, &self.runtime_dir, &self.provider)
            .map_err(|error| match error {
                CapabilityExecError::DispatchFailed { .. } => error,
                other => dispatch_error(&self.provider, other.to_string()),
            })?;

        Ok(CapabilityDispatchOutcome::Executed { provider: self.provider.clone(), exit_code })
    }
}

fn sandbox_mode_from_skill(definition: &str) -> Result<SandboxMode, String> {
    let Some(front_matter) = read_front_matter(definition)? else {
        return Ok(SandboxMode::ReadOnly);
    };
    let front_matter = parse_provider_definition_front_matter(front_matter)?;
    match front_matter.sandbox()? {
        None => Ok(SandboxMode::ReadOnly),
        Some("read-only") => Ok(SandboxMode::ReadOnly),
        Some("workspace-write") => Ok(SandboxMode::WorkspaceWrite),
        Some(value) => Err(format!("unsupported Codex skill sandbox declaration '{value}'")),
    }
}

fn build_codex_args(model: &str, sandbox: SandboxMode, prompt: &str) -> Vec<OsString> {
    vec![
        OsString::from("exec"),
        OsString::from("-m"),
        OsString::from(model),
        OsString::from("--sandbox"),
        OsString::from(sandbox.as_cli_value()),
        OsString::from(prompt),
    ]
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use super::super::{MAX_CAPABILITY_EXEC_TEXT_BYTES, ProviderProcessRunner};
    use super::{CodexCapabilityAdapter, SandboxMode, build_codex_args, sandbox_mode_from_skill};
    use usecase::capability_exec::{
        BriefingText, CapabilityDispatchOutcome, CapabilityDispatchRequest, CapabilityExecError,
        CapabilityExecRequest, CapabilityFilePath, CapabilityProfile, CapabilityProviderPort,
        DisciplineText, ExecutionMode, ModelName, ProviderName,
    };
    use usecase::dry_write_driver::CapabilityName;

    #[derive(Default)]
    struct RecordingProcessRunner {
        invocations: Mutex<Vec<(String, Vec<OsString>)>>,
        exit_code: u8,
    }

    impl ProviderProcessRunner for RecordingProcessRunner {
        fn run(
            &self,
            binary: &str,
            args: &[OsString],
            _repo_root: &Path,
            _runtime_dir: &Path,
            _provider: &ProviderName,
        ) -> Result<u8, CapabilityExecError> {
            self.invocations
                .lock()
                .expect("test process recorder lock")
                .push((binary.to_owned(), args.to_vec()));
            Ok(self.exit_code)
        }
    }

    fn request_with_capability_from_host(
        capability: &str,
        host: &str,
    ) -> Result<CapabilityDispatchRequest, Box<dyn std::error::Error>> {
        Ok(CapabilityDispatchRequest {
            request: CapabilityExecRequest {
                capability: CapabilityName::try_new(capability)?,
                host: ProviderName::try_new(host)?,
                briefing_file: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
            },
            profile: CapabilityProfile {
                provider: ProviderName::try_new("codex")?,
                model: ModelName::try_new("gpt-5")?,
                execution_mode: ExecutionMode::OrchestratorOutput,
            },
            briefing: BriefingText::try_new("Implement the assigned task.".to_owned())?,
            discipline: DisciplineText::try_new("Do not stage changes.".to_owned())?,
        })
    }

    fn request_from_host(
        host: &str,
    ) -> Result<CapabilityDispatchRequest, Box<dyn std::error::Error>> {
        request_with_capability_from_host("implementer", host)
    }

    fn write_skill(root: &Path, definition: &str) -> Result<(), Box<dyn std::error::Error>> {
        let skill_dir = root.join(".agents/skills/implementer");
        fs::create_dir_all(&skill_dir)?;
        fs::write(skill_dir.join("SKILL.md"), definition)?;
        Ok(())
    }

    #[test]
    fn test_codex_skill_missing_sandbox_defaults_to_read_only() {
        let definition = "---\nname: implementer\n---\nbody\n";

        assert_eq!(sandbox_mode_from_skill(definition), Ok(SandboxMode::ReadOnly));
    }

    #[test]
    fn test_codex_skill_workspace_write_sandbox_is_retained() {
        let definition = "---\nname: implementer\nsandbox: workspace-write\n---\nbody\n";

        assert_eq!(sandbox_mode_from_skill(definition), Ok(SandboxMode::WorkspaceWrite));
    }

    #[test]
    fn test_codex_skill_nested_sandbox_is_rejected() {
        let definition = "---\nmetadata:\n  sandbox: workspace-write\n---\nbody\n";

        assert!(sandbox_mode_from_skill(definition).is_err());
    }

    #[test]
    fn test_codex_skill_malformed_front_matter_is_rejected() {
        let definition = "---\nsandbox: [workspace-write\n---\nbody\n";

        assert!(sandbox_mode_from_skill(definition).is_err());
    }

    #[test]
    fn test_codex_args_explicit_skill_prompt_uses_profile_model_and_sandbox() {
        let args = build_codex_args(
            "gpt-5",
            SandboxMode::WorkspaceWrite,
            "$implementer Briefing: Read tmp/briefing.md and perform the task.",
        );
        let values: Vec<_> = args.iter().map(|value| value.to_string_lossy()).collect();

        assert_eq!(
            values,
            [
                "exec",
                "-m",
                "gpt-5",
                "--sandbox",
                "workspace-write",
                "$implementer Briefing: Read tmp/briefing.md and perform the task.",
            ]
        );
    }

    #[test]
    fn test_codex_capability_adapter_dispatches_native_skill_with_profile_model_and_prompt()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_skill(
            directory.path(),
            "---\nname: implementer\nsandbox: workspace-write\n---\nskill body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner { exit_code: 17, ..Default::default() });
        let adapter = CodexCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );

        let outcome = adapter.dispatch(&request_from_host("codex")?)?;

        assert!(matches!(
            outcome,
            CapabilityDispatchOutcome::Executed { ref provider, exit_code: 17 }
                if provider.as_str() == "codex"
        ));
        let invocations = runner.invocations.lock().expect("test process recorder lock");
        assert_eq!(invocations.len(), 1);
        let invocation = invocations.first().expect("one process invocation is recorded");
        assert_eq!(invocation.0, "codex");
        let args: Vec<_> =
            invocation.1.iter().map(|value| value.to_string_lossy().into_owned()).collect();
        let [command, model_flag, model, sandbox_flag, sandbox, prompt] = args.as_slice() else {
            return Err("Codex invocation must have six arguments".into());
        };
        assert_eq!(command, "exec");
        assert_eq!(model_flag, "-m");
        assert_eq!(model, "gpt-5");
        assert_eq!(sandbox_flag, "--sandbox");
        assert_eq!(sandbox, "workspace-write");
        assert!(
            prompt.contains("$implementer Briefing: Read tmp/briefing.md and perform the task.")
        );
        assert!(prompt.contains("Do not stage changes."));
        assert!(!prompt.contains("sandbox: workspace-write"));
        Ok(())
    }

    #[test]
    fn test_codex_capability_adapter_cross_provider_dispatches_native_skill()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_skill(
            directory.path(),
            "---\nname: implementer\nsandbox: workspace-write\n---\nskill body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = CodexCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );

        let outcome = adapter.dispatch(&request_from_host("claude")?)?;

        assert!(matches!(
            outcome,
            CapabilityDispatchOutcome::Executed { ref provider, exit_code: 0 }
                if provider.as_str() == "codex"
        ));
        let invocations = runner.invocations.lock().expect("test process recorder lock");
        assert_eq!(invocations.len(), 1);
        let invocation = invocations.first().expect("one process invocation is recorded");
        assert_eq!(invocation.0, "codex");
        Ok(())
    }

    #[test]
    fn test_codex_capability_adapter_missing_skill_rejected_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = CodexCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );

        assert!(matches!(
            adapter.dispatch(&request_from_host("claude")?),
            Err(CapabilityExecError::AdapterPreflight { .. })
        ));
        assert!(runner.invocations.lock().expect("test process recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_codex_capability_adapter_oversize_skill_rejected_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let skill = directory.path().join(".agents/skills/implementer/SKILL.md");
        fs::create_dir_all(skill.parent().ok_or("skill path must have a parent")?)?;
        fs::File::create(&skill)?.set_len(MAX_CAPABILITY_EXEC_TEXT_BYTES + 1)?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = CodexCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );

        assert!(matches!(
            adapter.dispatch(&request_from_host("codex")?),
            Err(CapabilityExecError::AdapterPreflight { .. })
        ));
        assert!(runner.invocations.lock().expect("test process recorder lock").is_empty());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_codex_capability_adapter_symlinked_skill_rejected_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = tempfile::tempdir()?;
        let repository = workspace.path().join("repository");
        fs::create_dir_all(repository.join(".agents/skills/implementer"))?;
        let external_skill = workspace.path().join("outside-skill.md");
        fs::write(&external_skill, "---\nname: implementer\n---\nskill body\n")?;
        std::os::unix::fs::symlink(
            &external_skill,
            repository.join(".agents/skills/implementer/SKILL.md"),
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = CodexCapabilityAdapter::with_process_runner(
            repository,
            workspace.path().join("runtime"),
            runner.clone(),
        );

        assert!(matches!(
            adapter.dispatch(&request_from_host("codex")?),
            Err(CapabilityExecError::AdapterPreflight { .. })
        ));
        assert!(runner.invocations.lock().expect("test process recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_codex_capability_adapter_traversal_name_rejected_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = tempfile::tempdir()?;
        let repository = workspace.path().join("repository");
        fs::create_dir_all(&repository)?;
        let external_skill = workspace.path().join("outside/SKILL.md");
        fs::create_dir_all(external_skill.parent().expect("skill has parent"))?;
        fs::write(&external_skill, "---\nname: outside\n---\nskill body\n")?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = CodexCapabilityAdapter::with_process_runner(
            repository,
            workspace.path().join("runtime"),
            runner.clone(),
        );

        assert!(matches!(
            adapter.dispatch(&request_with_capability_from_host("../../../outside", "codex")?),
            Err(CapabilityExecError::AdapterPreflight { .. })
        ));
        assert!(runner.invocations.lock().expect("test process recorder lock").is_empty());
        Ok(())
    }
}
