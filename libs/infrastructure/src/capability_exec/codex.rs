//! Codex provider-native adapter for generic capability dispatch.

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use domain::TrackId;
use serde::Deserialize;
use usecase::capability_exec::{
    CODEX_PROVIDER_NAME, CapabilityDispatchOutcome, CapabilityDispatchRequest, CapabilityExecError,
    CapabilityProviderPort, ProviderName, ReasoningEffort,
};
use usecase::provider_session::ProviderSessionCachePort;

use super::path_guard::capability_name_path_segment;
use super::process::emit_provider_final_message;
use super::session::CapabilitySession;
use super::{
    ProviderProcessRunner, adapter_preflight_error, capability_prompt, dispatch_error,
    parse_provider_definition_front_matter, read_front_matter, read_utf8_file,
    system_process_runner,
};

static OUTPUT_LAST_MESSAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
    session_cache: Arc<dyn ProviderSessionCachePort>,
    track_id: Option<TrackId>,
}

impl CodexCapabilityAdapter {
    /// Creates a Codex adapter rooted at `repo_root` with logs under `runtime_dir`.
    #[must_use]
    pub fn new(
        repo_root: PathBuf,
        runtime_dir: PathBuf,
        session_cache: Arc<dyn ProviderSessionCachePort>,
        track_id: Option<TrackId>,
    ) -> CodexCapabilityAdapter {
        Self {
            repo_root,
            runtime_dir,
            provider: CODEX_PROVIDER_NAME.clone(),
            process_runner: system_process_runner(),
            session_cache,
            track_id,
        }
    }

    #[cfg(test)]
    fn with_process_runner(
        repo_root: PathBuf,
        runtime_dir: PathBuf,
        process_runner: Arc<dyn ProviderProcessRunner>,
    ) -> Self {
        let session_cache = Arc::new(crate::provider_session::FsProviderSessionCacheAdapter::new(
            repo_root.clone(),
            runtime_dir.clone(),
        ));
        Self::with_process_runner_and_session_cache(
            repo_root,
            runtime_dir,
            process_runner,
            session_cache,
            None,
        )
    }

    #[cfg(test)]
    fn with_process_runner_and_session_cache(
        repo_root: PathBuf,
        runtime_dir: PathBuf,
        process_runner: Arc<dyn ProviderProcessRunner>,
        session_cache: Arc<dyn ProviderSessionCachePort>,
        track_id: Option<TrackId>,
    ) -> Self {
        Self {
            repo_root,
            runtime_dir,
            provider: CODEX_PROVIDER_NAME.clone(),
            process_runner,
            session_cache,
            track_id,
        }
    }

    fn skill_path(&self, capability: &str) -> PathBuf {
        self.repo_root.join(".agents").join("skills").join(capability).join("SKILL.md")
    }

    fn sandbox_mode(
        &self,
        request: &CapabilityDispatchRequest,
    ) -> Result<SandboxMode, CapabilityExecError> {
        let capability = capability_name_path_segment(request.request.capability.as_str())
            .map_err(|detail| adapter_preflight_error(request, &self.provider, detail))?;
        let path = self.skill_path(capability);
        let definition = read_utf8_file(&path, &self.repo_root)
            .map_err(|detail| adapter_preflight_error(request, &self.provider, detail))?;
        sandbox_mode_from_skill(&definition, capability)
            .map_err(|detail| adapter_preflight_error(request, &self.provider, detail))
    }

    fn output_last_message_path(&self) -> PathBuf {
        self.runtime_dir.join(format!(
            "capability-codex-last-message-{}-{}.txt",
            std::process::id(),
            OUTPUT_LAST_MESSAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ))
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
        self.dispatch_with_stdout(request, &mut std::io::stdout())
    }
}

impl CodexCapabilityAdapter {
    fn dispatch_with_stdout(
        &self,
        request: &CapabilityDispatchRequest,
        passthrough: &mut impl Write,
    ) -> Result<CapabilityDispatchOutcome, CapabilityExecError> {
        let sandbox = self.sandbox_mode(request)?;
        #[cfg(test)]
        let (binary, path_prefix) = ("codex".to_owned(), None::<PathBuf>);
        #[cfg(not(test))]
        let (binary, path_prefix) = {
            let runtime = crate::codex_common::resolve_codex_runtime(&self.repo_root)
                .map_err(|error| dispatch_error(&self.provider, error.to_string()))?;
            (
                runtime.executable().to_string_lossy().into_owned(),
                runtime.path_prefix().map(std::path::Path::to_path_buf),
            )
        };
        let prompt = capability_prompt(request);
        let session =
            CapabilitySession::new(request, self.track_id.as_ref(), self.session_cache.clone());
        let resume_id = session.resumable_id(&request.request.resume);
        let output_last_message = self.output_last_message_path();
        let args = build_codex_args_with_resume(
            request.profile.model.as_str(),
            request.profile.effort,
            sandbox,
            resume_id.as_deref(),
            &prompt,
            &output_last_message,
        );
        let timeout = request.request.timeout.map(|timeout| Duration::from_secs(timeout.as_secs()));
        let result = self
            .process_runner
            .run(
                &binary,
                path_prefix.as_deref(),
                &args,
                &self.repo_root,
                &self.runtime_dir,
                &self.provider,
                timeout,
                Some(&output_last_message),
            )
            .map_err(|error| match error {
                CapabilityExecError::DispatchFailed { .. } => error,
                other => dispatch_error(&self.provider, other.to_string()),
            });
        let output = match (resume_id, result) {
            (Some(_), Ok(output)) if output.exit_code != 0 => self.process_runner.run(
                &binary,
                path_prefix.as_deref(),
                &build_codex_args(
                    request.profile.model.as_str(),
                    request.profile.effort,
                    sandbox,
                    &prompt,
                    &output_last_message,
                ),
                &self.repo_root,
                &self.runtime_dir,
                &self.provider,
                timeout,
                Some(&output_last_message),
            )?,
            (Some(_), Err(_)) => self.process_runner.run(
                &binary,
                path_prefix.as_deref(),
                &build_codex_args(
                    request.profile.model.as_str(),
                    request.profile.effort,
                    sandbox,
                    &prompt,
                    &output_last_message,
                ),
                &self.repo_root,
                &self.runtime_dir,
                &self.provider,
                timeout,
                Some(&output_last_message),
            )?,
            (_, result) => result?,
        };
        emit_provider_final_message(&output, &self.provider, "codex", passthrough)?;
        if output.exit_code == 0 {
            session.save(output.session_id);
        }
        Ok(CapabilityDispatchOutcome::Executed {
            provider: self.provider.clone(),
            exit_code: output.exit_code,
        })
    }
}

fn sandbox_mode_from_skill(
    definition: &str,
    expected_capability: &str,
) -> Result<SandboxMode, String> {
    let front_matter = read_front_matter(definition)?
        .ok_or_else(|| "Codex skill definition has no YAML front matter".to_owned())?;
    let front_matter = parse_provider_definition_front_matter(front_matter)?;
    front_matter.validate_identity(expected_capability, "Codex skill definition")?;
    match front_matter.sandbox()? {
        None => Ok(SandboxMode::ReadOnly),
        Some("read-only") => Ok(SandboxMode::ReadOnly),
        Some("workspace-write") => Ok(SandboxMode::WorkspaceWrite),
        Some(value) => Err(format!("unsupported Codex skill sandbox declaration '{value}'")),
    }
}

fn build_codex_args(
    model: &str,
    effort: ReasoningEffort,
    sandbox: SandboxMode,
    prompt: &str,
    output_last_message: &std::path::Path,
) -> Vec<OsString> {
    build_codex_args_with_resume(model, effort, sandbox, None, prompt, output_last_message)
}

fn build_codex_args_with_resume(
    model: &str,
    effort: ReasoningEffort,
    sandbox: SandboxMode,
    resume_id: Option<&str>,
    prompt: &str,
    output_last_message: &std::path::Path,
) -> Vec<OsString> {
    vec![
        OsString::from("exec"),
        OsString::from("-m"),
        OsString::from(model),
        OsString::from("--config"),
        OsString::from(format!("model_reasoning_effort=\"{}\"", reasoning_effort_value(effort))),
        OsString::from("--sandbox"),
        OsString::from(sandbox.as_cli_value()),
        OsString::from("--json"),
        OsString::from("--output-last-message"),
        output_last_message.as_os_str().to_owned(),
    ]
    .into_iter()
    .chain(resume_id.into_iter().flat_map(|id| [OsString::from("resume"), OsString::from(id)]))
    .chain([OsString::from(prompt)])
    .collect()
}

fn reasoning_effort_value(effort: ReasoningEffort) -> &'static str {
    match effort {
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::XHigh => "xhigh",
        ReasoningEffort::Max => "max",
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::super::process::ProviderProcessOutput;
    use super::super::{MAX_CAPABILITY_EXEC_TEXT_BYTES, ProviderProcessRunner};
    use super::{CodexCapabilityAdapter, SandboxMode, build_codex_args, sandbox_mode_from_skill};
    use crate::provider_session::FsProviderSessionCacheAdapter;
    use domain::TrackId;
    use usecase::capability_exec::{
        BriefingText, CapabilityDispatchOutcome, CapabilityDispatchRequest, CapabilityExecError,
        CapabilityExecRequest, CapabilityFilePath, CapabilityProfile, CapabilityProviderPort,
        CapabilityResumeRequest, DisciplineText, ExecutionMode, ModelName, ProviderName,
        ReasoningEffort, TargetArtifactPath, TargetArtifactSet, TimeoutSeconds,
    };
    use usecase::dry_write_driver::CapabilityName;
    use usecase::provider_session::{
        ProviderSessionCacheEntry, ProviderSessionCacheKey, ProviderSessionCachePort,
        ProviderSessionId,
    };

    type RecordedInvocation = (String, Vec<OsString>, Option<Duration>);

    #[derive(Default)]
    struct RecordingProcessRunner {
        invocations: Mutex<Vec<RecordedInvocation>>,
        exit_code: u8,
        exit_codes: Mutex<Vec<u8>>,
        session_ids: Mutex<Vec<Option<String>>>,
        final_messages: Mutex<Vec<Option<Vec<u8>>>>,
    }

    impl ProviderProcessRunner for RecordingProcessRunner {
        fn run(
            &self,
            binary: &str,
            _path_prefix: Option<&Path>,
            args: &[OsString],
            _repo_root: &Path,
            _runtime_dir: &Path,
            _provider: &ProviderName,
            timeout: Option<Duration>,
            _output_last_message: Option<&Path>,
        ) -> Result<ProviderProcessOutput, CapabilityExecError> {
            self.invocations.lock().expect("test process recorder lock").push((
                binary.to_owned(),
                args.to_vec(),
                timeout,
            ));
            let exit_code = self
                .exit_codes
                .lock()
                .expect("test process exit-code lock")
                .pop()
                .unwrap_or(self.exit_code);
            let session_id = self
                .session_ids
                .lock()
                .expect("test process session-id lock")
                .pop()
                .unwrap_or(None);
            let final_message = self
                .final_messages
                .lock()
                .expect("test process final-message lock")
                .pop()
                .unwrap_or(None);
            Ok(ProviderProcessOutput { exit_code, session_id, final_message })
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
                timeout: None,
                resume: usecase::capability_exec::CapabilityResumeRequest::Fresh,
            },
            profile: CapabilityProfile {
                provider: ProviderName::try_new("codex")?,
                model: ModelName::try_new("gpt-5")?,
                effort: ReasoningEffort::High,
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
        let definition =
            "---\nname: implementer\ndescription: Implements assigned tasks.\n---\nbody\n";

        assert_eq!(sandbox_mode_from_skill(definition, "implementer"), Ok(SandboxMode::ReadOnly));
    }

    #[test]
    fn test_codex_skill_workspace_write_sandbox_is_retained() {
        let definition = "---\nname: implementer\ndescription: Implements assigned tasks.\nsandbox: workspace-write\n---\nbody\n";

        assert_eq!(
            sandbox_mode_from_skill(definition, "implementer"),
            Ok(SandboxMode::WorkspaceWrite)
        );
    }

    #[test]
    fn test_codex_skill_nested_sandbox_is_rejected() {
        let definition = "---\nname: implementer\ndescription: Implements assigned tasks.\nmetadata:\n  sandbox: workspace-write\n---\nbody\n";

        assert!(sandbox_mode_from_skill(definition, "implementer").is_err());
    }

    #[test]
    fn test_codex_skill_malformed_front_matter_is_rejected() {
        let definition = "---\nsandbox: [workspace-write\n---\nbody\n";

        assert!(sandbox_mode_from_skill(definition, "implementer").is_err());
    }

    #[test]
    fn test_codex_args_explicit_skill_prompt_uses_profile_model_effort_and_sandbox() {
        let args = build_codex_args(
            "gpt-5",
            ReasoningEffort::Max,
            SandboxMode::WorkspaceWrite,
            "$implementer Briefing: Read tmp/briefing.md and perform the task.",
            Path::new("tmp/runtime/final-message.txt"),
        );
        let values: Vec<_> = args.iter().map(|value| value.to_string_lossy()).collect();

        assert_eq!(
            values,
            [
                "exec",
                "-m",
                "gpt-5",
                "--config",
                "model_reasoning_effort=\"max\"",
                "--sandbox",
                "workspace-write",
                "--json",
                "--output-last-message",
                "tmp/runtime/final-message.txt",
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
            "---\nname: implementer\ndescription: Implements assigned tasks.\nsandbox: workspace-write\n---\nskill body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner { exit_code: 17, ..Default::default() });
        let adapter = CodexCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );

        let mut request = request_from_host("codex")?;
        request.profile.effort = ReasoningEffort::Max;

        let outcome = adapter.dispatch(&request)?;

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
        let [
            command,
            model_flag,
            model,
            config_flag,
            config,
            sandbox_flag,
            sandbox,
            json,
            output_flag,
            output_path,
            prompt,
        ] = args.as_slice()
        else {
            return Err("Codex invocation must have eleven arguments".into());
        };
        assert_eq!(command, "exec");
        assert_eq!(model_flag, "-m");
        assert_eq!(model, "gpt-5");
        assert_eq!(config_flag, "--config");
        assert_eq!(config, "model_reasoning_effort=\"max\"");
        assert_eq!(sandbox_flag, "--sandbox");
        assert_eq!(sandbox, "workspace-write");
        assert_eq!(json, "--json");
        assert_eq!(output_flag, "--output-last-message");
        assert!(output_path.ends_with(".txt"));
        assert!(
            prompt.contains("$implementer Briefing: Read tmp/briefing.md and perform the task.")
        );
        assert!(prompt.contains("Do not stage changes."));
        assert!(!prompt.contains("sandbox: workspace-write"));
        assert_eq!(invocation.2, None, "an omitted timeout runs the provider without a limit");
        Ok(())
    }

    #[test]
    fn test_codex_capability_adapter_resumes_workspace_session_with_explicit_flags()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_skill(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nsandbox: workspace-write\n---\nskill body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let mut adapter = CodexCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );
        let mut request = request_from_host("claude")?;
        let targets = TargetArtifactSet::try_new(vec![TargetArtifactPath::try_new(
            PathBuf::from("track/items/a/spec.json"),
        )?])?;
        request.request.resume = CapabilityResumeRequest::Resume(targets.clone());
        let cache = Arc::new(FsProviderSessionCacheAdapter::new(
            directory.path().to_owned(),
            directory.path().join("runtime"),
        ));
        let key = ProviderSessionCacheKey::WorkspaceCapability {
            capability: request.request.capability.clone(),
            target_artifacts: targets,
        };
        cache.save(
            &key,
            &ProviderSessionCacheEntry::new(
                ProviderSessionId::try_new("prior-session".to_owned())?,
                request.profile.provider.clone(),
                request.profile.model.clone(),
                request.profile.effort,
            ),
        )?;
        adapter.session_cache = cache;

        adapter.dispatch(&request)?;

        let invocations = runner.invocations.lock().expect("test process recorder lock");
        let first = invocations.first().expect("one invocation recorded");
        let args: Vec<_> =
            first.1.iter().map(|value| value.to_string_lossy().into_owned()).collect();
        assert!(args.windows(2).any(|pair| pair == ["resume", "prior-session"]));
        assert!(args.windows(2).any(|pair| pair == ["-m", "gpt-5"]));
        assert!(args.contains(&"model_reasoning_effort=\"high\"".to_owned()));
        assert!(args.windows(2).any(|pair| pair == ["--sandbox", "workspace-write"]));
        Ok(())
    }

    #[test]
    fn test_codex_capability_adapter_injected_cache_persists_and_resumes_track_session()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_skill(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nsandbox: workspace-write\n---\nskill body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner {
            session_ids: Mutex::new(vec![None, Some("persisted-codex-session".to_owned())]),
            ..Default::default()
        });
        let cache = Arc::new(FsProviderSessionCacheAdapter::new(
            directory.path().to_owned(),
            directory.path().join("tmp/runtime"),
        ));
        let track_id = TrackId::try_new("track-a")?;
        let adapter = CodexCapabilityAdapter::with_process_runner_and_session_cache(
            directory.path().to_owned(),
            directory.path().join("tmp/runtime"),
            runner.clone(),
            cache.clone(),
            Some(track_id.clone()),
        );
        let mut request = request_from_host("claude")?;
        request.request.resume =
            CapabilityResumeRequest::Resume(TargetArtifactSet::try_new(vec![
                TargetArtifactPath::try_new(PathBuf::from("track/items/a/spec.json"))?,
            ])?);
        let key = ProviderSessionCacheKey::TrackCapability {
            track_id,
            capability: request.request.capability.clone(),
        };

        adapter.dispatch(&request)?;
        assert_eq!(
            cache.load(&key)?.map(|entry| entry.session_id().as_str().to_owned()),
            Some("persisted-codex-session".to_owned())
        );
        adapter.dispatch(&request)?;

        let invocations = runner.invocations.lock().expect("test process recorder lock");
        assert_eq!(invocations.len(), 2);
        let resumed = invocations.get(1).expect("second invocation resumes the saved session");
        let resumed_args: Vec<_> =
            resumed.1.iter().map(|value| value.to_string_lossy().into_owned()).collect();
        assert!(resumed_args.windows(2).any(|pair| pair == ["resume", "persisted-codex-session"]));
        assert!(resumed_args.windows(2).any(|pair| pair == ["-m", "gpt-5"]));
        assert!(resumed_args.contains(&"model_reasoning_effort=\"high\"".to_owned()));
        assert!(resumed_args.windows(2).any(|pair| pair == ["--sandbox", "workspace-write"]));
        Ok(())
    }

    #[test]
    fn test_codex_capability_adapter_provider_mismatch_starts_fresh_with_explicit_flags()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_skill(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nsandbox: workspace-write\n---\nskill body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let mut adapter = CodexCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );
        let mut request = request_from_host("claude")?;
        let targets = TargetArtifactSet::try_new(vec![TargetArtifactPath::try_new(
            PathBuf::from("track/items/a/spec.json"),
        )?])?;
        request.request.resume = CapabilityResumeRequest::Resume(targets.clone());
        let cache = Arc::new(FsProviderSessionCacheAdapter::new(
            directory.path().to_owned(),
            directory.path().join("runtime"),
        ));
        let key = ProviderSessionCacheKey::WorkspaceCapability {
            capability: request.request.capability.clone(),
            target_artifacts: targets,
        };
        let current_provider = request.profile.provider.clone();
        let recorded_provider = ProviderName::try_new("claude")?;
        assert_ne!(current_provider, recorded_provider);
        cache.save(
            &key,
            &ProviderSessionCacheEntry::new(
                ProviderSessionId::try_new("stale-claude-session".to_owned())?,
                recorded_provider,
                request.profile.model.clone(),
                request.profile.effort,
            ),
        )?;
        adapter.session_cache = cache;

        adapter.dispatch(&request)?;

        let invocations = runner.invocations.lock().expect("test process recorder lock");
        let args: Vec<_> = invocations
            .first()
            .expect("one fresh invocation recorded")
            .1
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert!(!args.contains(&"resume".to_owned()));
        assert!(args.windows(2).any(|pair| pair == ["-m", "gpt-5"]));
        assert!(args.contains(&"model_reasoning_effort=\"high\"".to_owned()));
        assert!(args.windows(2).any(|pair| pair == ["--sandbox", "workspace-write"]));
        Ok(())
    }

    #[test]
    fn test_codex_capability_adapter_model_mismatch_starts_fresh_with_explicit_flags()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_skill(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nsandbox: workspace-write\n---\nskill body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let mut adapter = CodexCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );
        let mut request = request_from_host("claude")?;
        let targets = TargetArtifactSet::try_new(vec![TargetArtifactPath::try_new(
            PathBuf::from("track/items/a/spec.json"),
        )?])?;
        request.request.resume = CapabilityResumeRequest::Resume(targets.clone());
        let cache = Arc::new(FsProviderSessionCacheAdapter::new(
            directory.path().to_owned(),
            directory.path().join("runtime"),
        ));
        let key = ProviderSessionCacheKey::WorkspaceCapability {
            capability: request.request.capability.clone(),
            target_artifacts: targets,
        };
        let recorded_model = ModelName::try_new("gpt-5-mini")?;
        assert_ne!(request.profile.model, recorded_model);
        cache.save(
            &key,
            &ProviderSessionCacheEntry::new(
                ProviderSessionId::try_new("stale-model-session".to_owned())?,
                request.profile.provider.clone(),
                recorded_model,
                request.profile.effort,
            ),
        )?;
        adapter.session_cache = cache;

        adapter.dispatch(&request)?;

        let invocations = runner.invocations.lock().expect("test process recorder lock");
        assert_eq!(invocations.len(), 1);
        let args: Vec<_> = invocations
            .first()
            .expect("one fresh invocation recorded")
            .1
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert!(!args.contains(&"resume".to_owned()));
        assert!(args.windows(2).any(|pair| pair == ["-m", "gpt-5"]));
        assert!(args.contains(&"model_reasoning_effort=\"high\"".to_owned()));
        assert!(args.windows(2).any(|pair| pair == ["--sandbox", "workspace-write"]));
        Ok(())
    }

    #[test]
    fn test_codex_capability_adapter_resume_nonzero_retries_fresh_with_explicit_flags()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_skill(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nsandbox: workspace-write\n---\nskill body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner {
            exit_codes: Mutex::new(vec![0, 7]),
            ..Default::default()
        });
        let mut adapter = CodexCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );
        let mut request = request_from_host("claude")?;
        let targets = TargetArtifactSet::try_new(vec![TargetArtifactPath::try_new(
            PathBuf::from("track/items/a/spec.json"),
        )?])?;
        request.request.resume = CapabilityResumeRequest::Resume(targets.clone());
        let cache = Arc::new(FsProviderSessionCacheAdapter::new(
            directory.path().to_owned(),
            directory.path().join("runtime"),
        ));
        let key = ProviderSessionCacheKey::WorkspaceCapability {
            capability: request.request.capability.clone(),
            target_artifacts: targets,
        };
        cache.save(
            &key,
            &ProviderSessionCacheEntry::new(
                ProviderSessionId::try_new("prior-session".to_owned())?,
                request.profile.provider.clone(),
                request.profile.model.clone(),
                request.profile.effort,
            ),
        )?;
        adapter.session_cache = cache;

        let outcome = adapter.dispatch(&request)?;

        assert!(matches!(outcome, CapabilityDispatchOutcome::Executed { exit_code: 0, .. }));
        let invocations = runner.invocations.lock().expect("test process recorder lock");
        assert_eq!(invocations.len(), 2);
        for (index, invocation) in invocations.iter().enumerate() {
            let args: Vec<_> =
                invocation.1.iter().map(|value| value.to_string_lossy().into_owned()).collect();
            assert_eq!(args.contains(&"resume".to_owned()), index == 0);
            assert!(args.windows(2).any(|pair| pair == ["-m", "gpt-5"]));
            assert!(args.contains(&"model_reasoning_effort=\"high\"".to_owned()));
            assert!(args.windows(2).any(|pair| pair == ["--sandbox", "workspace-write"]));
            assert!(args.last().is_some_and(|prompt| prompt.contains(
                "check whether upstream artifacts changed; if they did, reread them before working"
            )));
        }
        Ok(())
    }

    #[test]
    fn test_codex_capability_adapter_resume_failure_emits_only_fresh_final_message()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_skill(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nsandbox: workspace-write\n---\nskill body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner {
            exit_codes: Mutex::new(vec![0, 7]),
            final_messages: Mutex::new(vec![
                Some(b"fresh final message".to_vec()),
                Some(b"failed partial message".to_vec()),
            ]),
            ..Default::default()
        });
        let mut adapter = CodexCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner,
        );
        let mut request = request_from_host("claude")?;
        let targets = TargetArtifactSet::try_new(vec![TargetArtifactPath::try_new(
            PathBuf::from("track/items/a/spec.json"),
        )?])?;
        request.request.resume = CapabilityResumeRequest::Resume(targets.clone());
        let cache = Arc::new(FsProviderSessionCacheAdapter::new(
            directory.path().to_owned(),
            directory.path().join("runtime"),
        ));
        let key = ProviderSessionCacheKey::WorkspaceCapability {
            capability: request.request.capability.clone(),
            target_artifacts: targets,
        };
        cache.save(
            &key,
            &ProviderSessionCacheEntry::new(
                ProviderSessionId::try_new("prior-session".to_owned())?,
                request.profile.provider.clone(),
                request.profile.model.clone(),
                request.profile.effort,
            ),
        )?;
        adapter.session_cache = cache;

        let mut stdout = Vec::new();
        let outcome = adapter.dispatch_with_stdout(&request, &mut stdout)?;

        assert!(matches!(outcome, CapabilityDispatchOutcome::Executed { exit_code: 0, .. }));
        assert_eq!(stdout, b"fresh final message");
        Ok(())
    }

    #[test]
    fn test_codex_capability_adapter_forwards_requested_timeout()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_skill(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nsandbox: workspace-write\n---\nskill body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = CodexCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );
        let mut request = request_from_host("claude")?;
        request.request.timeout = Some(TimeoutSeconds::try_new(1800)?);

        adapter.dispatch(&request)?;

        let invocations = runner.invocations.lock().expect("test process recorder lock");
        let invocation = invocations.first().expect("one process invocation is recorded");
        assert_eq!(invocation.2, Some(Duration::from_secs(1800)));
        Ok(())
    }

    #[test]
    fn test_codex_capability_adapter_cross_provider_dispatches_native_skill()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_skill(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nsandbox: workspace-write\n---\nskill body\n",
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
    fn test_codex_capability_adapter_missing_name_rejected_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_skill(
            directory.path(),
            "---\ndescription: Implements assigned tasks.\nsandbox: workspace-write\n---\nskill body\n",
        )?;
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

    #[test]
    fn test_codex_capability_adapter_mismatched_name_rejected_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_skill(
            directory.path(),
            "---\nname: researcher\ndescription: Researches the workspace.\nsandbox: workspace-write\n---\nskill body\n",
        )?;
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
    fn test_codex_capability_adapter_missing_skill_rejects_without_raw_definition_prompt_fallback()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let raw_definition = directory.path().join(".harness/capabilities/implementer.md");
        fs::create_dir_all(raw_definition.parent().ok_or("raw definition has a parent")?)?;
        fs::write(&raw_definition, "raw capability definition")?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = CodexCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );

        let error = adapter
            .dispatch(&request_from_host("claude")?)
            .expect_err("a missing native skill must reject at preflight");

        assert!(matches!(error, CapabilityExecError::AdapterPreflight { .. }));
        assert!(
            runner.invocations.lock().expect("test process recorder lock").is_empty(),
            "a raw definition must not be injected into a fallback provider prompt"
        );
        Ok(())
    }

    #[test]
    fn test_codex_capability_adapter_declared_and_default_sandbox_reach_subprocess()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace_write_directory = tempfile::tempdir()?;
        write_skill(
            workspace_write_directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nsandbox: workspace-write\n---\nskill body\n",
        )?;
        let workspace_write_runner = Arc::new(RecordingProcessRunner::default());
        let workspace_write_adapter = CodexCapabilityAdapter::with_process_runner(
            workspace_write_directory.path().to_owned(),
            workspace_write_directory.path().join("runtime"),
            workspace_write_runner.clone(),
        );

        workspace_write_adapter.dispatch(&request_from_host("codex")?)?;

        let workspace_write_invocations =
            workspace_write_runner.invocations.lock().expect("test process recorder lock");
        let workspace_write_invocation = workspace_write_invocations
            .first()
            .ok_or("workspace-write dispatch must invoke the provider")?;
        let workspace_write_args: Vec<_> = workspace_write_invocation
            .1
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        let [
            _,
            _,
            _,
            config_flag,
            config,
            workspace_write_flag,
            workspace_write_sandbox,
            json,
            output_flag,
            output_path,
            _,
        ] = workspace_write_args.as_slice()
        else {
            return Err("workspace-write invocation must have eleven arguments".into());
        };
        assert_eq!(config_flag, "--config");
        assert_eq!(config, "model_reasoning_effort=\"high\"");
        assert_eq!(workspace_write_flag, "--sandbox");
        assert_eq!(workspace_write_sandbox, "workspace-write");
        assert_eq!(json, "--json");
        assert_eq!(output_flag, "--output-last-message");
        assert!(output_path.ends_with(".txt"));

        let default_directory = tempfile::tempdir()?;
        write_skill(
            default_directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\n---\nskill body\n",
        )?;
        let default_runner = Arc::new(RecordingProcessRunner::default());
        let default_adapter = CodexCapabilityAdapter::with_process_runner(
            default_directory.path().to_owned(),
            default_directory.path().join("runtime"),
            default_runner.clone(),
        );

        default_adapter.dispatch(&request_from_host("codex")?)?;

        let default_invocations =
            default_runner.invocations.lock().expect("test process recorder lock");
        let default_invocation = default_invocations
            .first()
            .ok_or("undeclared sandbox dispatch must invoke the provider")?;
        let default_args: Vec<_> =
            default_invocation.1.iter().map(|value| value.to_string_lossy().into_owned()).collect();
        let [
            _,
            _,
            _,
            config_flag,
            config,
            default_flag,
            default_sandbox,
            json,
            output_flag,
            output_path,
            _,
        ] = default_args.as_slice()
        else {
            return Err("undeclared sandbox invocation must have eleven arguments".into());
        };
        assert_eq!(config_flag, "--config");
        assert_eq!(config, "model_reasoning_effort=\"high\"");
        assert_eq!(default_flag, "--sandbox");
        assert_eq!(default_sandbox, "read-only");
        assert_eq!(json, "--json");
        assert_eq!(output_flag, "--output-last-message");
        assert!(output_path.ends_with(".txt"));
        Ok(())
    }

    #[test]
    fn test_codex_capability_adapter_parent_segment_name_rejected_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let escaped_skill = directory.path().join(".agents/outside/SKILL.md");
        fs::create_dir_all(directory.path().join(".agents/skills"))?;
        fs::create_dir_all(escaped_skill.parent().ok_or("skill path must have a parent")?)?;
        fs::write(
            &escaped_skill,
            "---\nname: outside\ndescription: Outside skill.\n---\nskill body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = CodexCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );

        let error = adapter
            .dispatch(&request_with_capability_from_host("../outside", "codex")?)
            .expect_err("parent component capability names must fail preflight");

        assert!(matches!(error, CapabilityExecError::AdapterPreflight { .. }));
        assert!(error.to_string().contains("single path segment"));
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
        fs::write(
            &external_skill,
            "---\nname: implementer\ndescription: Implements assigned tasks.\n---\nskill body\n",
        )?;
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
        fs::write(
            &external_skill,
            "---\nname: outside\ndescription: Outside skill.\n---\nskill body\n",
        )?;
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
