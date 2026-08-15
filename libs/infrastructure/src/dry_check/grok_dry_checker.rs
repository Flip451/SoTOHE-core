//! Grok-backed implementation of the `DryCheckAgentPort` usecase port.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use domain::semantic_dup::CodeFragment;
use usecase::capability_exec::{
    CapabilityExecError, GROK_PROVIDER_NAME, ModelName, ProviderName, ReasoningEffort,
};
use usecase::dry_check::{
    DryCheckAgentError, DryCheckAgentJudgment, DryCheckAgentPort, DryCheckJudgeTier,
};
use usecase::dry_write_driver::CapabilityName;
use usecase::provider_session::ProviderSessionId;

use crate::capability_exec::grok::build_grok_args;
use crate::capability_exec::process::ProviderProcessOutput;
use crate::capability_exec::{ProviderProcessRunner, system_process_runner};
use crate::grok_common::GrokOutputEnvelope;
use crate::grok_common::GrokSandbox;

const GROK_DRY_CHECK_TIMEOUT: Duration = Duration::from_secs(600);

/// Grok-backed implementation of [`DryCheckAgentPort`].
///
/// Each judgment is sent to an independent `grok` subprocess. The subprocess is
/// given the profile-selected model, reasoning effort, sandbox, and typed-output
/// schema explicitly. Only the JSON stored in the Grok envelope's
/// `structured_output.result` field is accepted as the dry-check judgment.
pub struct GrokDryChecker {
    fast_model: ModelName,
    fast_reasoning_effort: ReasoningEffort,
    final_model: ModelName,
    final_reasoning_effort: ReasoningEffort,
    capability_name: CapabilityName,
    sandbox: GrokSandbox,
    repo_root: PathBuf,
    runtime_dir: PathBuf,
    timeout: Duration,
    provider: ProviderName,
    process_runner: Arc<dyn ProviderProcessRunner>,
    session: Mutex<Option<GrokDryCheckSession>>,
}

struct GrokDryCheckSession {
    session_id: ProviderSessionId,
    provider: ProviderName,
    model: ModelName,
    effort: ReasoningEffort,
}

impl std::fmt::Debug for GrokDryChecker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GrokDryChecker")
            .field("fast_model", &self.fast_model)
            .field("fast_reasoning_effort", &self.fast_reasoning_effort)
            .field("final_model", &self.final_model)
            .field("final_reasoning_effort", &self.final_reasoning_effort)
            .field("capability_name", &self.capability_name)
            .field("sandbox", &self.sandbox)
            .field("repo_root", &self.repo_root)
            .field("runtime_dir", &self.runtime_dir)
            .field("timeout", &self.timeout)
            .field("provider", &self.provider)
            .finish_non_exhaustive()
    }
}

impl GrokDryChecker {
    /// Constructs a Grok dry-check adapter using the current repository and its
    /// capability runtime directory for provider subprocess execution.
    #[must_use]
    pub fn new(
        fast_model: ModelName,
        fast_reasoning_effort: ReasoningEffort,
        final_model: ModelName,
        final_reasoning_effort: ReasoningEffort,
        capability_name: CapabilityName,
        sandbox: GrokSandbox,
    ) -> GrokDryChecker {
        let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let runtime_dir = repo_root.join("tmp/capability-runtime");
        Self {
            fast_model,
            fast_reasoning_effort,
            final_model,
            final_reasoning_effort,
            capability_name,
            sandbox,
            repo_root,
            runtime_dir,
            timeout: GROK_DRY_CHECK_TIMEOUT,
            provider: GROK_PROVIDER_NAME.clone(),
            process_runner: system_process_runner(),
            session: Mutex::new(None),
        }
    }

    #[cfg(test)]
    fn with_process_runner(
        mut self,
        repo_root: PathBuf,
        runtime_dir: PathBuf,
        process_runner: Arc<dyn ProviderProcessRunner>,
    ) -> Self {
        self.repo_root = repo_root;
        self.runtime_dir = runtime_dir;
        self.process_runner = process_runner;
        self
    }

    fn model_and_effort(&self, tier: DryCheckJudgeTier) -> (&ModelName, ReasoningEffort) {
        match tier {
            DryCheckJudgeTier::Fast => (&self.fast_model, self.fast_reasoning_effort),
            DryCheckJudgeTier::Final => (&self.final_model, self.final_reasoning_effort),
        }
    }

    fn resumable_session_id(
        &self,
        model: &ModelName,
        effort: ReasoningEffort,
    ) -> Result<Option<String>, DryCheckAgentError> {
        let mut session = self.session.lock().map_err(|_| {
            DryCheckAgentError::Unexpected("dry-check session state is poisoned".to_owned())
        })?;
        let Some(active) = session.as_ref() else {
            return Ok(None);
        };
        if active.provider.as_str() == self.provider.as_str()
            && active.model.as_str() == model.as_str()
            && active.effort == effort
        {
            return Ok(Some(active.session_id.as_str().to_owned()));
        }
        *session = None;
        Ok(None)
    }

    fn clear_session(&self) -> Result<(), DryCheckAgentError> {
        let mut session = self.session.lock().map_err(|_| {
            DryCheckAgentError::Unexpected("dry-check session state is poisoned".to_owned())
        })?;
        *session = None;
        Ok(())
    }

    fn save_session(
        &self,
        session_id: Option<String>,
        model: &ModelName,
        effort: ReasoningEffort,
    ) -> Result<(), DryCheckAgentError> {
        let session_id = session_id.and_then(|value| ProviderSessionId::try_new(value).ok());
        let mut session = self.session.lock().map_err(|_| {
            DryCheckAgentError::Unexpected("dry-check session state is poisoned".to_owned())
        })?;
        *session = session_id.map(|session_id| GrokDryCheckSession {
            session_id,
            provider: self.provider.clone(),
            model: model.clone(),
            effort,
        });
        Ok(())
    }

    #[cfg(test)]
    #[allow(clippy::expect_used)]
    fn seed_session(
        &self,
        session_id: &str,
        provider: ProviderName,
        model: ModelName,
        effort: ReasoningEffort,
    ) {
        let session_id =
            ProviderSessionId::try_new(session_id.to_owned()).expect("session is valid");
        *self.session.lock().expect("session lock") =
            Some(GrokDryCheckSession { session_id, provider, model, effort });
    }

    fn build_prompt(&self, changed: &CodeFragment, candidate: &CodeFragment) -> String {
        format!(
            "You are the `{capability}` capability. Determine whether the following two code \
             fragments constitute a DRY (Don't Repeat Yourself) violation.\n\n\
             ## Changed fragment (diff side)\n\n\
             File: {changed_path}\n\
             Lines: {changed_start}–{changed_end}\n\n\
             ```\n{changed_content}\n```\n\n\
             ## Candidate fragment (existing code)\n\n\
             File: {candidate_path}\n\
             Lines: {candidate_start}–{candidate_end}\n\n\
             ```\n{candidate_content}\n```\n\n\
             Return the dry-check JSON object as a string in the required `result` \
             structured-output field. The object must contain `verdict`, `rationale`, \
             and `refactor_proposal`.",
            capability = self.capability_name.as_str(),
            changed_path = changed.source_path.display(),
            changed_start = changed.start_line(),
            changed_end = changed.end_line(),
            changed_content = changed.content(),
            candidate_path = candidate.source_path.display(),
            candidate_start = candidate.start_line(),
            candidate_end = candidate.end_line(),
            candidate_content = candidate.content(),
        )
    }

    fn run_process(
        &self,
        model: &ModelName,
        effort: ReasoningEffort,
        resume_id: Option<&str>,
        prompt: &str,
    ) -> Result<ProviderProcessOutput, CapabilityExecError> {
        let args = build_grok_args(model.as_str(), effort, &self.sandbox, resume_id, prompt);
        self.process_runner.run(
            "grok",
            None,
            &args,
            &self.repo_root,
            &self.runtime_dir,
            &self.provider,
            Some(self.timeout),
            None,
        )
    }
}

impl DryCheckAgentPort for GrokDryChecker {
    fn judge(
        &self,
        changed_fragment: &CodeFragment,
        candidate_fragment: &CodeFragment,
        tier: DryCheckJudgeTier,
    ) -> Result<DryCheckAgentJudgment, DryCheckAgentError> {
        let (model, effort) = self.model_and_effort(tier);
        let prompt = self.build_prompt(changed_fragment, candidate_fragment);
        let resume_id = self.resumable_session_id(model, effort)?;
        let output = match resume_id.as_deref() {
            Some(resume_id) => match self.run_process(model, effort, Some(resume_id), &prompt) {
                Ok(output) if !resume_attempt_needs_fresh_session(&output) => output,
                Ok(_) | Err(_) => {
                    self.clear_session()?;
                    self.run_process(model, effort, None, &prompt).map_err(map_process_error)?
                }
            },
            None => self.run_process(model, effort, None, &prompt).map_err(map_process_error)?,
        };
        let judgment = parse_grok_judgment(&output, changed_fragment, candidate_fragment)?;
        if output.exit_code == 0 {
            self.save_session(output.session_id.clone(), model, effort)?;
        }
        Ok(judgment)
    }
}

fn resume_attempt_needs_fresh_session(output: &ProviderProcessOutput) -> bool {
    if output.exit_code != 0 {
        return true;
    }
    !matches!(
        output
            .final_message
            .as_deref()
            .and_then(|message| serde_json::from_slice::<GrokOutputEnvelope>(message).ok()),
        Some(GrokOutputEnvelope::Succeeded { .. })
    )
}

fn map_process_error(error: CapabilityExecError) -> DryCheckAgentError {
    let detail = error.to_string();
    if detail.contains("timed out") {
        DryCheckAgentError::Timeout
    } else {
        DryCheckAgentError::Unexpected(detail)
    }
}

fn parse_grok_judgment(
    output: &ProviderProcessOutput,
    changed_fragment: &CodeFragment,
    candidate_fragment: &CodeFragment,
) -> Result<DryCheckAgentJudgment, DryCheckAgentError> {
    if output.exit_code != 0 {
        return Err(DryCheckAgentError::AgentAbort);
    }
    let message = output.final_message.as_deref().ok_or(DryCheckAgentError::IllegalOutput)?;
    let envelope = serde_json::from_slice::<GrokOutputEnvelope>(message)
        .map_err(|_| DryCheckAgentError::IllegalOutput)?;
    let structured_output = envelope
        .into_structured_output()
        .map_err(|error| DryCheckAgentError::Unexpected(error.to_string()))?;
    let result = structured_output
        .get("result")
        .and_then(serde_json::Value::as_str)
        .ok_or(DryCheckAgentError::IllegalOutput)?;

    super::codex_dry_checker::parse_agent_json_and_build_judgment(
        result,
        changed_fragment,
        candidate_fragment,
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::VecDeque;
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::*;
    use crate::capability_exec::grok::GROK_STRUCTURED_OUTPUT_SCHEMA;
    use crate::capability_exec::process::ProviderProcessRunner;
    use domain::semantic_dup::CodeFragment;
    use usecase::capability_exec::{CapabilityFailureDetail, ProviderName};

    struct RecordedInvocation {
        binary: String,
        path_prefix: Option<PathBuf>,
        args: Vec<OsString>,
        output_last_message: Option<PathBuf>,
    }

    #[derive(Default)]
    struct RecordingProcessRunner {
        invocations: Mutex<Vec<RecordedInvocation>>,
        responses: Mutex<VecDeque<Result<ProviderProcessOutput, CapabilityExecError>>>,
    }

    impl ProviderProcessRunner for RecordingProcessRunner {
        fn run(
            &self,
            binary: &str,
            path_prefix: Option<&Path>,
            args: &[OsString],
            _repo_root: &Path,
            _runtime_dir: &Path,
            _provider: &ProviderName,
            _timeout: Option<Duration>,
            output_last_message: Option<&Path>,
        ) -> Result<ProviderProcessOutput, CapabilityExecError> {
            assert_eq!(binary, "grok");
            self.invocations.lock().expect("invocation lock").push(RecordedInvocation {
                binary: binary.to_owned(),
                path_prefix: path_prefix.map(Path::to_path_buf),
                args: args.to_vec(),
                output_last_message: output_last_message.map(Path::to_path_buf),
            });
            self.responses
                .lock()
                .expect("response lock")
                .pop_front()
                .unwrap_or_else(|| Ok(successful_output()))
        }
    }

    fn model(value: &str) -> ModelName {
        ModelName::try_new(value).expect("model is valid")
    }

    fn capability(value: &str) -> CapabilityName {
        CapabilityName::try_new(value).expect("capability is valid")
    }

    fn fragments() -> (CodeFragment, CodeFragment) {
        (
            CodeFragment::new(
                "src/changed.rs".into(),
                "fn changed() { let value = 1; }".to_owned(),
                2,
                2,
            )
            .expect("changed fragment is valid"),
            CodeFragment::new(
                "src/candidate.rs".into(),
                "fn candidate() { let value = 1; }".to_owned(),
                8,
                8,
            )
            .expect("candidate fragment is valid"),
        )
    }

    fn checker(runner: Arc<RecordingProcessRunner>) -> GrokDryChecker {
        GrokDryChecker::new(
            model("grok-fast"),
            ReasoningEffort::Medium,
            model("grok-final"),
            ReasoningEffort::High,
            capability("dry-checker"),
            GrokSandbox::Workspace,
        )
        .with_process_runner(PathBuf::from("."), PathBuf::from("tmp/runtime"), runner)
    }

    fn successful_output() -> ProviderProcessOutput {
        successful_output_with_session("grok-session")
    }

    fn successful_output_with_session(session_id: &str) -> ProviderProcessOutput {
        ProviderProcessOutput {
            exit_code: 0,
            session_id: Some(session_id.to_owned()),
            final_message: Some(
                br#"{"structured_output":{"result":"{\"verdict\":\"not_a_violation\",\"rationale\":\"The fragments have distinct responsibilities.\",\"refactor_proposal\":null}"},"text":"{\"verdict\":\"violation\",\"rationale\":\"text channel must be ignored\",\"refactor_proposal\":\"do not use this\"}"}"#.to_vec(),
            ),
        }
    }

    fn assert_settings(args: &[OsString], model: &str, effort: &str) {
        assert!(args.windows(2).any(|pair| pair == ["--model", model]));
        assert!(args.windows(2).any(|pair| pair == ["--reasoning-effort", effort]));
        assert!(
            args.windows(2).any(|pair| pair == ["--sandbox", "workspace"]),
            "permission settings must be restated as --sandbox on every Grok launch"
        );
        assert!(args.windows(2).any(|pair| pair == ["--output-format", "json"]));
        assert!(
            args.windows(2).any(|pair| pair == ["--json-schema", GROK_STRUCTURED_OUTPUT_SCHEMA])
        );
        // `--resume` is session-id reuse on a new subprocess. Shared-process
        // connection is `--leader` / `agent`, which this typed pipeline never sets.
        assert!(!args.iter().any(|arg| arg == "--leader" || arg == "agent"));
    }

    #[test]
    fn test_grok_dry_checker_implements_dry_check_agent_port() {
        fn accepts_port(_: &dyn DryCheckAgentPort) {}

        let checker = checker(Arc::new(RecordingProcessRunner::default()));
        accepts_port(&checker);
    }

    #[test]
    fn test_grok_dry_checker_typed_pipeline_uses_dedicated_path_not_capability_exec()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner::default());
        let checker = checker(runner.clone());
        let (changed, candidate) = fragments();

        checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;

        let invocations = runner.invocations.lock().expect("invocation lock");
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations[0].binary, "grok");
        assert!(
            !invocations[0]
                .args
                .iter()
                .any(|arg| arg == "capability" || arg == "exec" || arg == "agent"),
            "typed-pipeline dry-check must launch grok directly, not through capability exec"
        );
        assert_settings(&invocations[0].args, "grok-fast", "medium");
        Ok(())
    }

    #[test]
    fn test_grok_dry_checker_fast_tier_launches_isolated_typed_pipeline_with_explicit_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner::default());
        let checker = checker(runner.clone());
        let (changed, candidate) = fragments();

        let judgment = checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;

        assert!(matches!(judgment, DryCheckAgentJudgment::NotAViolation { .. }));
        let invocations = runner.invocations.lock().expect("invocation lock");
        assert_eq!(invocations.len(), 1);
        let invocation = &invocations[0];
        assert_eq!(invocation.binary, "grok");
        assert!(invocation.path_prefix.is_none());
        assert!(invocation.output_last_message.is_none());
        assert!(!invocation.args.iter().any(|arg| arg == "--resume"));
        assert_settings(&invocation.args, "grok-fast", "medium");
        Ok(())
    }

    #[test]
    fn test_grok_dry_checker_final_tier_launches_with_final_model_and_effort()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner::default());
        let checker = checker(runner.clone());
        let (changed, candidate) = fragments();

        checker.judge(&changed, &candidate, DryCheckJudgeTier::Final)?;

        let invocations = runner.invocations.lock().expect("invocation lock");
        assert_eq!(invocations.len(), 1);
        assert_settings(&invocations[0].args, "grok-final", "high");
        Ok(())
    }

    #[test]
    fn test_grok_dry_checker_fresh_and_resumed_launches_restate_permission_sandbox()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(VecDeque::from([
                Ok(successful_output()),
                Ok(successful_output()),
            ])),
            ..Default::default()
        });
        let checker = checker(runner.clone());
        let (changed, candidate) = fragments();

        checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;
        checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;

        let invocations = runner.invocations.lock().expect("invocation lock");
        assert_eq!(invocations.len(), 2);
        assert!(!invocations[0].args.iter().any(|arg| arg == "--resume"));
        assert!(invocations[1].args.windows(2).any(|pair| pair == ["--resume", "grok-session"]));
        for invocation in invocations.iter() {
            assert!(
                invocation.args.windows(2).any(|pair| pair == ["--sandbox", "workspace"]),
                "permission settings (--sandbox) must be passed on fresh and resumed Grok launches"
            );
        }
        Ok(())
    }

    #[test]
    fn test_grok_dry_checker_reentry_restates_explicit_settings_on_each_launch()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(VecDeque::from([
                Ok(successful_output()),
                Ok(successful_output()),
            ])),
            ..Default::default()
        });
        let checker = checker(runner.clone());
        let (changed, candidate) = fragments();

        checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;
        checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;

        let invocations = runner.invocations.lock().expect("invocation lock");
        assert_eq!(invocations.len(), 2);
        assert!(!invocations[0].args.iter().any(|arg| arg == "--resume"));
        assert!(invocations[1].args.windows(2).any(|pair| pair == ["--resume", "grok-session"]));
        for invocation in invocations.iter() {
            assert_eq!(invocation.binary, "grok");
            assert!(invocation.path_prefix.is_none());
            assert!(invocation.output_last_message.is_none());
            assert_settings(&invocation.args, "grok-fast", "medium");
        }
        Ok(())
    }

    #[test]
    fn test_grok_dry_checker_extracts_only_structured_result()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner::default());
        let checker = checker(runner);
        let (changed, candidate) = fragments();

        let judgment = checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;

        match judgment {
            DryCheckAgentJudgment::NotAViolation { rationale } => {
                assert_eq!(rationale.as_str(), "The fragments have distinct responsibilities.")
            }
            other => {
                return Err(format!(
                    "successful judgment must come only from structured_output.result, got {other:?}"
                )
                .into());
            }
        }
        Ok(())
    }

    #[test]
    fn test_grok_dry_checker_incompatible_resume_starts_fresh_with_explicit_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner::default());
        let checker = checker(runner.clone());
        checker.seed_session(
            "incompatible-session",
            GROK_PROVIDER_NAME.clone(),
            model("grok-fast"),
            ReasoningEffort::Low,
        );
        let (changed, candidate) = fragments();

        checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;

        let invocations = runner.invocations.lock().expect("invocation lock");
        assert_eq!(invocations.len(), 1);
        assert!(
            !invocations[0].args.iter().any(|arg| arg == "--resume"),
            "incompatible resume must fall back to a fresh isolated subprocess"
        );
        assert_settings(&invocations[0].args, "grok-fast", "medium");
        Ok(())
    }

    #[test]
    fn test_grok_dry_checker_provider_mismatched_resume_starts_fresh_with_explicit_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner::default());
        let checker = checker(runner.clone());
        checker.seed_session(
            "foreign-provider-session",
            ProviderName::try_new("codex".to_owned()).expect("provider is valid"),
            model("grok-fast"),
            ReasoningEffort::Medium,
        );
        let (changed, candidate) = fragments();

        checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;

        let invocations = runner.invocations.lock().expect("invocation lock");
        assert_eq!(invocations.len(), 1);
        assert!(
            !invocations[0].args.iter().any(|arg| arg == "--resume"),
            "provider-mismatched resume must fall back to a fresh isolated subprocess"
        );
        assert_settings(&invocations[0].args, "grok-fast", "medium");
        Ok(())
    }

    #[test]
    fn test_grok_dry_checker_expired_resume_retries_fresh_with_explicit_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(VecDeque::from([
                Ok(successful_output_with_session("prior-session")),
                Ok(ProviderProcessOutput {
                    exit_code: 0,
                    session_id: None,
                    final_message: Some(br#"{"failure_reason":"resume session expired"}"#.to_vec()),
                }),
                Ok(successful_output_with_session("fresh-session")),
                Ok(successful_output_with_session("fresh-session")),
            ])),
            ..Default::default()
        });
        let checker = checker(runner.clone());
        let (changed, candidate) = fragments();

        checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;
        checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;
        checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;

        let invocations = runner.invocations.lock().expect("invocation lock");
        assert_eq!(invocations.len(), 4);
        assert!(!invocations[0].args.iter().any(|arg| arg == "--resume"));
        assert!(invocations[1].args.windows(2).any(|pair| pair == ["--resume", "prior-session"]));
        assert!(!invocations[2].args.iter().any(|arg| arg == "--resume"));
        assert!(invocations[3].args.windows(2).any(|pair| pair == ["--resume", "fresh-session"]));
        for invocation in invocations.iter() {
            assert_eq!(invocation.binary, "grok");
            assert!(invocation.path_prefix.is_none());
            assert!(invocation.output_last_message.is_none());
            assert_settings(&invocation.args, "grok-fast", "medium");
        }
        Ok(())
    }

    #[test]
    fn test_grok_dry_checker_unavailable_resume_retries_fresh_with_explicit_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(VecDeque::from([
                Ok(successful_output_with_session("prior-session")),
                Err(CapabilityExecError::DispatchFailed {
                    provider: GROK_PROVIDER_NAME.clone(),
                    detail: CapabilityFailureDetail::new("resume unavailable"),
                }),
                Ok(successful_output_with_session("fresh-session")),
            ])),
            ..Default::default()
        });
        let checker = checker(runner.clone());
        let (changed, candidate) = fragments();

        checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;
        checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;

        let invocations = runner.invocations.lock().expect("invocation lock");
        assert_eq!(invocations.len(), 3);
        assert!(invocations[1].args.windows(2).any(|pair| pair == ["--resume", "prior-session"]));
        assert!(!invocations[2].args.iter().any(|arg| arg == "--resume"));
        assert_settings(&invocations[1].args, "grok-fast", "medium");
        assert_settings(&invocations[2].args, "grok-fast", "medium");
        Ok(())
    }

    #[test]
    fn test_grok_dry_checker_model_mismatch_starts_fresh_with_explicit_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(VecDeque::from([
                Ok(successful_output_with_session("fast-session")),
                Ok(successful_output_with_session("final-session")),
            ])),
            ..Default::default()
        });
        let checker = checker(runner.clone());
        let (changed, candidate) = fragments();

        checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast)?;
        checker.judge(&changed, &candidate, DryCheckJudgeTier::Final)?;

        let invocations = runner.invocations.lock().expect("invocation lock");
        assert_eq!(invocations.len(), 2);
        assert!(!invocations[0].args.iter().any(|arg| arg == "--resume"));
        assert!(!invocations[1].args.iter().any(|arg| arg == "--resume"));
        assert_settings(&invocations[0].args, "grok-fast", "medium");
        assert_settings(&invocations[1].args, "grok-final", "high");
        Ok(())
    }

    #[test]
    fn test_grok_dry_checker_missing_structured_result_reports_envelope_failure_reason() {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(VecDeque::from([Ok(ProviderProcessOutput {
                exit_code: 0,
                session_id: None,
                final_message: Some(
                    br#"{"failure_reason":"Grok declined structured output","text":"ignore-me"}"#
                        .to_vec(),
                ),
            })])),
            ..Default::default()
        });
        let checker = checker(runner);
        let (changed, candidate) = fragments();

        let error = checker
            .judge(&changed, &candidate, DryCheckJudgeTier::Final)
            .expect_err("missing structured output must fail closed");

        assert!(
            matches!(error, DryCheckAgentError::Unexpected(ref detail) if detail.contains("Grok declined structured output"))
        );
        assert!(!error.to_string().contains("ignore-me"));
    }

    #[test]
    fn test_grok_dry_checker_missing_result_field_returns_illegal_output() {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(VecDeque::from([Ok(ProviderProcessOutput {
                exit_code: 0,
                session_id: None,
                final_message: Some(br#"{"structured_output":{"text":"not-a-result"}}"#.to_vec()),
            })])),
            ..Default::default()
        });
        let checker = checker(runner);
        let (changed, candidate) = fragments();

        let error = checker
            .judge(&changed, &candidate, DryCheckJudgeTier::Fast)
            .expect_err("missing result field must fail closed");

        assert!(matches!(error, DryCheckAgentError::IllegalOutput));
    }

    #[test]
    fn test_grok_dry_checker_nonzero_provider_exit_returns_agent_abort() {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(VecDeque::from([Ok(ProviderProcessOutput {
                exit_code: 9,
                session_id: None,
                final_message: None,
            })])),
            ..Default::default()
        });
        let checker = checker(runner);
        let (changed, candidate) = fragments();

        let error = checker
            .judge(&changed, &candidate, DryCheckJudgeTier::Fast)
            .expect_err("nonzero provider exit must fail closed");

        assert!(matches!(error, DryCheckAgentError::AgentAbort));
    }
}
