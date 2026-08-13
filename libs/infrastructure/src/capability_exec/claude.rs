//! Claude provider-native adapter for generic capability dispatch.

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use domain::TrackId;
use usecase::capability_exec::{
    CLAUDE_PROVIDER_NAME, CapabilityDispatchOutcome, CapabilityDispatchRequest,
    CapabilityExecError, CapabilityProviderPort, ProviderName, ReasoningEffort,
};
use usecase::provider_session::ProviderSessionCachePort;

use super::path_guard::capability_name_path_segment;
use super::process::emit_provider_final_message;
use super::session::CapabilitySession;
use super::{
    ProviderProcessRunner, adapter_preflight_error, capability_prompt,
    parse_provider_definition_front_matter, read_front_matter, read_utf8_file,
    system_process_runner,
};

/// Dispatches through a named repository Claude agent definition.
pub struct ClaudeCapabilityAdapter {
    repo_root: PathBuf,
    runtime_dir: PathBuf,
    provider: ProviderName,
    process_runner: Arc<dyn ProviderProcessRunner>,
    session_cache: Arc<dyn ProviderSessionCachePort>,
    track_id: Option<TrackId>,
}

impl ClaudeCapabilityAdapter {
    /// Creates a Claude adapter rooted at `repo_root` with logs under `runtime_dir`.
    #[must_use]
    pub fn new(
        repo_root: PathBuf,
        runtime_dir: PathBuf,
        session_cache: Arc<dyn ProviderSessionCachePort>,
        track_id: Option<TrackId>,
    ) -> Self {
        Self {
            repo_root,
            runtime_dir,
            provider: CLAUDE_PROVIDER_NAME.clone(),
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
        Self {
            repo_root,
            runtime_dir,
            provider: CLAUDE_PROVIDER_NAME.clone(),
            process_runner,
            session_cache,
            track_id: None,
        }
    }

    fn agent_path(&self, capability: &str) -> PathBuf {
        self.repo_root.join(".claude").join("agents").join(format!("{capability}.md"))
    }

    fn agent_tools(
        &self,
        request: &CapabilityDispatchRequest,
    ) -> Result<Vec<String>, CapabilityExecError> {
        let capability = capability_name_path_segment(request.request.capability.as_str())
            .map_err(|detail| adapter_preflight_error(request, &self.provider, detail))?;
        let path = self.agent_path(capability);
        let definition = read_utf8_file(&path, &self.repo_root)
            .map_err(|detail| adapter_preflight_error(request, &self.provider, detail))?;
        agent_tools_from_definition(&definition, capability, request.profile.model.as_str())
            .map_err(|detail| adapter_preflight_error(request, &self.provider, detail))
    }
}

impl CapabilityProviderPort for ClaudeCapabilityAdapter {
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

impl ClaudeCapabilityAdapter {
    fn dispatch_with_stdout(
        &self,
        request: &CapabilityDispatchRequest,
        passthrough: &mut impl Write,
    ) -> Result<CapabilityDispatchOutcome, CapabilityExecError> {
        let allowed_tools = self.agent_tools(request)?;
        if request.request.host.as_ref() == Some(&self.provider) {
            return Ok(CapabilityDispatchOutcome::DelegateInHost {
                capability: request.request.capability.clone(),
                briefing_file: request.request.briefing_file.clone(),
                discipline: request.discipline.clone(),
            });
        }

        let prompt = capability_prompt(request);
        let session =
            CapabilitySession::new(request, self.track_id.as_ref(), self.session_cache.clone());
        let resume_id = session.resumable_id(&request.request.resume);
        let args = build_claude_args_with_resume(
            request.request.capability.as_str(),
            request.profile.model.as_str(),
            request.profile.effort,
            &allowed_tools,
            resume_id.as_deref(),
            &prompt,
        );
        let timeout = request.request.timeout.map(|timeout| Duration::from_secs(timeout.as_secs()));
        let result = self.process_runner.run(
            "claude",
            None,
            &args,
            &self.repo_root,
            &self.runtime_dir,
            &self.provider,
            timeout,
            None,
        );
        let output = match (resume_id, result) {
            (Some(_), Ok(output)) if output.exit_code != 0 => self.process_runner.run(
                "claude",
                None,
                &build_claude_args(
                    request.request.capability.as_str(),
                    request.profile.model.as_str(),
                    request.profile.effort,
                    &allowed_tools,
                    &prompt,
                ),
                &self.repo_root,
                &self.runtime_dir,
                &self.provider,
                timeout,
                None,
            )?,
            (Some(_), Err(_)) => self.process_runner.run(
                "claude",
                None,
                &build_claude_args(
                    request.request.capability.as_str(),
                    request.profile.model.as_str(),
                    request.profile.effort,
                    &allowed_tools,
                    &prompt,
                ),
                &self.repo_root,
                &self.runtime_dir,
                &self.provider,
                timeout,
                None,
            )?,
            (_, result) => result?,
        };
        emit_provider_final_message(&output, &self.provider, "claude", passthrough)?;
        if output.exit_code == 0 {
            session.save(output.session_id);
        }
        Ok(CapabilityDispatchOutcome::Executed {
            provider: self.provider.clone(),
            exit_code: output.exit_code,
        })
    }
}

fn agent_tools_from_definition(
    definition: &str,
    expected_capability: &str,
    profile_model: &str,
) -> Result<Vec<String>, String> {
    let front_matter = read_front_matter(definition)?
        .ok_or_else(|| "Claude agent definition has no YAML front matter".to_owned())?;
    let front_matter = parse_provider_definition_front_matter(front_matter)?;
    front_matter.validate_identity(expected_capability, "Claude agent definition")?;
    let tools = front_matter
        .tools()
        .ok_or_else(|| "Claude agent definition must declare a non-empty tools field".to_owned())?;
    let model = front_matter
        .model()
        .ok_or_else(|| "Claude agent definition must declare a model field".to_owned())?;
    if model != profile_model {
        return Err(format!(
            "Claude agent model '{model}' does not match profile model '{profile_model}'"
        ));
    }
    Ok(tools.into_iter().map(str::to_owned).collect())
}

fn build_claude_args(
    capability: &str,
    model: &str,
    effort: ReasoningEffort,
    allowed_tools: &[String],
    prompt: &str,
) -> Vec<OsString> {
    build_claude_args_with_resume(capability, model, effort, allowed_tools, None, prompt)
}

fn build_claude_args_with_resume(
    capability: &str,
    model: &str,
    effort: ReasoningEffort,
    allowed_tools: &[String],
    resume_id: Option<&str>,
    prompt: &str,
) -> Vec<OsString> {
    vec![OsString::from("-p")]
        .into_iter()
        .chain(
            resume_id.into_iter().flat_map(|id| [OsString::from("--resume"), OsString::from(id)]),
        )
        .chain([OsString::from("--permission-mode"), OsString::from("dontAsk")])
        .chain([OsString::from("--allowedTools")])
        .chain(allowed_tools.iter().map(OsString::from))
        .chain([
            OsString::from("--output-format"),
            OsString::from("json"),
            OsString::from("--agent"),
            OsString::from(capability),
            OsString::from("--model"),
            OsString::from(model),
            OsString::from("--effort"),
            OsString::from(reasoning_effort_value(effort)),
            OsString::from(prompt),
        ])
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
    use super::{ClaudeCapabilityAdapter, agent_tools_from_definition, build_claude_args};
    use crate::provider_session::FsProviderSessionCacheAdapter;
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
            Ok(ProviderProcessOutput { exit_code, session_id: None, final_message: None })
        }
    }

    fn request_with_capability_from_host(
        capability: &str,
        host: &str,
    ) -> Result<CapabilityDispatchRequest, Box<dyn std::error::Error>> {
        Ok(CapabilityDispatchRequest {
            request: CapabilityExecRequest {
                capability: CapabilityName::try_new(capability)?,
                host: Some(ProviderName::try_new(host)?),
                briefing_file: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                timeout: None,
                resume: usecase::capability_exec::CapabilityResumeRequest::Fresh,
            },
            profile: CapabilityProfile {
                provider: ProviderName::try_new("claude")?,
                model: ModelName::try_new("claude-opus")?,
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

    fn request_without_host() -> Result<CapabilityDispatchRequest, Box<dyn std::error::Error>> {
        let mut request = request_from_host("claude")?;
        request.request.host = None;
        Ok(request)
    }

    fn write_agent(root: &Path, definition: &str) -> Result<(), Box<dyn std::error::Error>> {
        let agent_dir = root.join(".claude/agents");
        fs::create_dir_all(&agent_dir)?;
        fs::write(agent_dir.join("implementer.md"), definition)?;
        Ok(())
    }

    #[test]
    fn test_claude_agent_matching_model_and_tools_is_valid() {
        let definition = "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools:\n  - Read\n---\nbody\n";

        assert_eq!(
            agent_tools_from_definition(definition, "implementer", "claude-opus"),
            Ok(vec!["Read".to_owned()])
        );
    }

    #[test]
    fn test_claude_agent_missing_tools_is_rejected() {
        let definition = "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\n---\nbody\n";

        assert!(agent_tools_from_definition(definition, "implementer", "claude-opus").is_err());
    }

    #[test]
    fn test_claude_agent_model_mismatch_is_rejected() {
        let definition = "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-haiku\ntools: Read\n---\nbody\n";

        assert!(agent_tools_from_definition(definition, "implementer", "claude-opus").is_err());
    }

    #[test]
    fn test_claude_agent_nested_model_and_tools_are_rejected() {
        let definition = "---\nname: implementer\ndescription: Implements assigned tasks.\nmetadata:\n  model: claude-opus\n  tools: Read\n---\nbody\n";

        assert!(agent_tools_from_definition(definition, "implementer", "claude-opus").is_err());
    }

    #[test]
    fn test_claude_agent_malformed_front_matter_is_rejected() {
        let definition = "---\nmodel: [claude-opus\ntools: Read\n---\nbody\n";

        assert!(agent_tools_from_definition(definition, "implementer", "claude-opus").is_err());
    }

    #[test]
    fn test_claude_args_use_native_agent_invocation_with_profile_model_and_effort() {
        let args = build_claude_args(
            "implementer",
            "claude-opus",
            ReasoningEffort::Max,
            &["Read".to_owned()],
            "Read tmp/briefing.md and perform the task.",
        );
        let values: Vec<_> = args.iter().map(|value| value.to_string_lossy()).collect();

        assert_eq!(
            values,
            [
                "-p",
                "--permission-mode",
                "dontAsk",
                "--allowedTools",
                "Read",
                "--output-format",
                "json",
                "--agent",
                "implementer",
                "--model",
                "claude-opus",
                "--effort",
                "max",
                "Read tmp/briefing.md and perform the task.",
            ]
        );
    }

    #[test]
    fn test_claude_capability_adapter_same_host_returns_in_host_instruction()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_agent(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools:\n  - Read\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = ClaudeCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );

        let outcome = adapter.dispatch(&request_from_host("claude")?)?;

        assert!(matches!(
            outcome,
            CapabilityDispatchOutcome::DelegateInHost {
                ref capability,
                ref briefing_file,
                ref discipline,
            } if capability.as_str() == "implementer"
                && briefing_file.as_path() == Path::new("tmp/briefing.md")
                && discipline.as_str() == "Do not stage changes."
        ));
        assert!(runner.invocations.lock().expect("test process recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_claude_capability_adapter_mismatched_supplied_host_uses_subprocess()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_agent(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools: Read\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner { exit_code: 9, ..Default::default() });
        let adapter = ClaudeCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );

        let outcome = adapter.dispatch(&request_from_host("codex")?)?;

        assert!(matches!(
            outcome,
            CapabilityDispatchOutcome::Executed { ref provider, exit_code: 9 }
                if provider.as_str() == "claude"
        ));
        let invocations = runner.invocations.lock().expect("test process recorder lock");
        assert_eq!(invocations.len(), 1);
        let invocation = invocations.first().expect("one process invocation is recorded");
        assert_eq!(invocation.0, "claude");
        let args: Vec<_> =
            invocation.1.iter().map(|value| value.to_string_lossy().into_owned()).collect();
        assert_eq!(
            args,
            [
                "-p",
                "--permission-mode",
                "dontAsk",
                "--allowedTools",
                "Read",
                "--output-format",
                "json",
                "--agent",
                "implementer",
                "--model",
                "claude-opus",
                "--effort",
                "high",
                "$implementer Briefing: Read tmp/briefing.md and perform the task.\n\nDo not stage changes.",
            ]
        );
        assert_eq!(invocation.2, None, "an omitted timeout runs the provider without a limit");
        Ok(())
    }

    #[test]
    fn test_claude_capability_adapter_omitted_host_uses_subprocess()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_agent(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools: Read\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner { exit_code: 9, ..Default::default() });
        let adapter = ClaudeCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );

        let outcome = adapter.dispatch(&request_without_host()?)?;

        assert!(matches!(
            outcome,
            CapabilityDispatchOutcome::Executed { ref provider, exit_code: 9 }
                if provider.as_str() == "claude"
        ));
        let invocations = runner.invocations.lock().expect("test process recorder lock");
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations.first().expect("one process invocation is recorded").0, "claude");
        Ok(())
    }

    #[test]
    fn test_claude_capability_adapter_resumes_workspace_session_with_explicit_flags()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_agent(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools: Read\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let mut adapter = ClaudeCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );
        let mut request = request_from_host("codex")?;
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
        assert!(args.windows(2).any(|pair| pair == ["--resume", "prior-session"]));
        assert!(args.windows(2).any(|pair| pair == ["--model", "claude-opus"]));
        assert!(args.windows(2).any(|pair| pair == ["--effort", "high"]));
        assert!(args.windows(2).any(|pair| pair == ["--permission-mode", "dontAsk"]));
        assert!(args.windows(2).any(|pair| pair == ["--allowedTools", "Read"]));
        Ok(())
    }

    #[test]
    fn test_claude_capability_adapter_provider_mismatch_starts_fresh_with_explicit_flags()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_agent(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools: Read\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let mut adapter = ClaudeCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );
        let mut request = request_from_host("codex")?;
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
        let recorded_provider = ProviderName::try_new("codex")?;
        assert_ne!(current_provider, recorded_provider);
        cache.save(
            &key,
            &ProviderSessionCacheEntry::new(
                ProviderSessionId::try_new("stale-codex-session".to_owned())?,
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
        assert!(!args.contains(&"--resume".to_owned()));
        assert!(args.windows(2).any(|pair| pair == ["--model", "claude-opus"]));
        assert!(args.windows(2).any(|pair| pair == ["--effort", "high"]));
        assert!(args.windows(2).any(|pair| pair == ["--permission-mode", "dontAsk"]));
        assert!(args.windows(2).any(|pair| pair == ["--allowedTools", "Read"]));
        Ok(())
    }

    #[test]
    fn test_claude_capability_adapter_model_mismatch_starts_fresh_with_explicit_flags()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_agent(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools: Read\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let mut adapter = ClaudeCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );
        let mut request = request_from_host("codex")?;
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
        let recorded_model = ModelName::try_new("claude-haiku")?;
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
        assert!(!args.contains(&"--resume".to_owned()));
        assert!(args.windows(2).any(|pair| pair == ["--model", "claude-opus"]));
        assert!(args.windows(2).any(|pair| pair == ["--effort", "high"]));
        assert!(args.windows(2).any(|pair| pair == ["--permission-mode", "dontAsk"]));
        assert!(args.windows(2).any(|pair| pair == ["--allowedTools", "Read"]));
        Ok(())
    }

    #[test]
    fn test_claude_capability_adapter_resume_nonzero_retries_fresh_with_explicit_flags()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_agent(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools: Read\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner {
            exit_codes: Mutex::new(vec![0, 7]),
            ..Default::default()
        });
        let mut adapter = ClaudeCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );
        let mut request = request_from_host("codex")?;
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
            assert_eq!(args.contains(&"--resume".to_owned()), index == 0);
            assert!(args.windows(2).any(|pair| pair == ["--model", "claude-opus"]));
            assert!(args.windows(2).any(|pair| pair == ["--effort", "high"]));
            assert!(args.windows(2).any(|pair| pair == ["--permission-mode", "dontAsk"]));
            assert!(args.windows(2).any(|pair| pair == ["--allowedTools", "Read"]));
            assert!(args.last().is_some_and(|prompt| prompt.contains(
                "check whether upstream artifacts changed; if they did, reread them before working"
            )));
        }
        Ok(())
    }

    #[test]
    fn test_claude_capability_adapter_forwards_requested_timeout()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_agent(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools: Read\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = ClaudeCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );
        let mut request = request_from_host("codex")?;
        request.request.timeout = Some(TimeoutSeconds::try_new(1800)?);

        adapter.dispatch(&request)?;

        let invocations = runner.invocations.lock().expect("test process recorder lock");
        let invocation = invocations.first().expect("one process invocation is recorded");
        assert_eq!(invocation.2, Some(Duration::from_secs(1800)));
        Ok(())
    }

    #[test]
    fn test_claude_capability_adapter_invalid_agent_rejected_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_agent(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = ClaudeCapabilityAdapter::with_process_runner(
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
    fn test_claude_capability_adapter_missing_name_rejected_before_in_host_delegation()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_agent(
            directory.path(),
            "---\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools: Read\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = ClaudeCapabilityAdapter::with_process_runner(
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
    fn test_claude_capability_adapter_mismatched_name_rejected_before_in_host_delegation()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_agent(
            directory.path(),
            "---\nname: researcher\ndescription: Researches the workspace.\nmodel: claude-opus\ntools: Read\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = ClaudeCapabilityAdapter::with_process_runner(
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
    fn test_claude_capability_adapter_parent_segment_name_rejected_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        fs::create_dir_all(directory.path().join(".claude/agents"))?;
        fs::write(
            directory.path().join(".claude/outside.md"),
            "---\nname: outside\ndescription: Outside agent.\nmodel: claude-opus\ntools: Read\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = ClaudeCapabilityAdapter::with_process_runner(
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
    fn test_claude_capability_adapter_missing_model_rejected_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_agent(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\ntools: Read\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = ClaudeCapabilityAdapter::with_process_runner(
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
    fn test_claude_capability_adapter_oversize_agent_rejected_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let agent = directory.path().join(".claude/agents/implementer.md");
        fs::create_dir_all(agent.parent().ok_or("agent path must have a parent")?)?;
        fs::File::create(&agent)?.set_len(MAX_CAPABILITY_EXEC_TEXT_BYTES + 1)?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = ClaudeCapabilityAdapter::with_process_runner(
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
    fn test_claude_capability_adapter_model_mismatch_rejected_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_agent(
            directory.path(),
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-haiku\ntools: Read\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = ClaudeCapabilityAdapter::with_process_runner(
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
    fn test_claude_capability_adapter_symlinked_agent_rejected_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = tempfile::tempdir()?;
        let repository = workspace.path().join("repository");
        fs::create_dir_all(repository.join(".claude/agents"))?;
        let external_agent = workspace.path().join("outside.md");
        fs::write(
            &external_agent,
            "---\nname: implementer\ndescription: Implements assigned tasks.\nmodel: claude-opus\ntools: Read\n---\nagent body\n",
        )?;
        std::os::unix::fs::symlink(
            &external_agent,
            repository.join(".claude/agents/implementer.md"),
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = ClaudeCapabilityAdapter::with_process_runner(
            repository,
            workspace.path().join("runtime"),
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
    fn test_claude_capability_adapter_traversal_name_rejected_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = tempfile::tempdir()?;
        let repository = workspace.path().join("repository");
        fs::create_dir_all(&repository)?;
        fs::write(
            workspace.path().join("outside.md"),
            "---\nname: outside\ndescription: Outside agent.\nmodel: claude-opus\ntools: Read\n---\nagent body\n",
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = ClaudeCapabilityAdapter::with_process_runner(
            repository,
            workspace.path().join("runtime"),
            runner.clone(),
        );

        assert!(matches!(
            adapter.dispatch(&request_with_capability_from_host("../../../outside", "claude")?),
            Err(CapabilityExecError::AdapterPreflight { .. })
        ));
        assert!(runner.invocations.lock().expect("test process recorder lock").is_empty());
        Ok(())
    }
}
