//! Grok-backed implementation of the `Reviewer` usecase port.
//!
//! The reviewer uses the same provider-native launch contract as Grok capability
//! execution: every round is an isolated `grok` subprocess with explicit model,
//! reasoning effort, sandbox, JSON output, and a schema requiring a string
//! `result`. The review payload is read only from that structured-output result;
//! envelope text is never treated as a verdict channel.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use domain::review_v2::{
    FastVerdict, LogInfo, ReviewTarget, ReviewerFinding, RoundType, ScopeName, Verdict,
    VerdictError,
};
use domain::{CommitHash, TrackId};
use usecase::capability_exec::{GROK_PROVIDER_NAME, ModelName, ProviderName, ReasoningEffort};
use usecase::provider_session::{ProviderSessionCachePort, ReviewerPrompt};
use usecase::review_v2::{
    ResolvedReviewer, ResolvedReviewerAssignment, ReviewerError, ports::Reviewer,
};
use usecase::review_workflow::{
    ReviewFinalMessageState, ReviewPayloadVerdict, ReviewVerdict, classify_review_verdict,
    normalize_final_message, parse_review_final_message, render_review_payload,
};

use crate::capability_exec::grok::build_grok_args;
use crate::capability_exec::process::{ProviderProcessOutput, ProviderProcessRunner};
use crate::codex_common::REVIEW_RUNTIME_DIR;
use crate::grok_common::{GrokOutputEnvelope, GrokSandbox};

use super::session::ReviewerSession;

/// Grok-backed reviewer implementation for the `Reviewer` usecase port.
///
/// Each review attempt starts an independent Grok subprocess. A matching cached
/// session may be resumed, but the model, effort, and sandbox are supplied again
/// on every invocation. Resume failures are retried as a fresh session.
pub struct GrokReviewer {
    model: ModelName,
    sandbox: GrokSandbox,
    timeout: Duration,
    base_prompt: String,
    scope_label: String,
    session: ReviewerSession,
    assignment: ResolvedReviewerAssignment,
    repo_root: PathBuf,
    runtime_dir: PathBuf,
    provider: ProviderName,
    process_runner: Arc<dyn ProviderProcessRunner>,
}

impl GrokReviewer {
    /// Constructs a new `GrokReviewer` rooted at `repo_root`.
    ///
    /// `diff_base` scopes session reuse to the current review cycle. The model,
    /// effort, and Grok sandbox are resolved by the caller and remain explicit
    /// launch settings for both fresh and resumed rounds. The trusted
    /// repository root must come from the same discovery path used for
    /// definition admission so the subprocess never inherits process CWD.
    #[allow(clippy::too_many_arguments)] // signature is the catalogue-declared contract
    pub fn new(
        track_id: TrackId,
        scope: ScopeName,
        round_type: RoundType,
        diff_base: Option<CommitHash>,
        model: ModelName,
        effort: ReasoningEffort,
        sandbox: GrokSandbox,
        timeout: Duration,
        base_prompt: ReviewerPrompt,
        session_cache: Arc<dyn ProviderSessionCachePort>,
        repo_root: PathBuf,
    ) -> GrokReviewer {
        let runtime_dir = repo_root.join(REVIEW_RUNTIME_DIR);
        let assignment = ResolvedReviewerAssignment::new(
            track_id.clone(),
            scope.clone(),
            GROK_PROVIDER_NAME.clone(),
            model.clone(),
            effort,
        );
        Self {
            session: ReviewerSession::new(
                track_id,
                scope.clone(),
                round_type,
                diff_base,
                "grok",
                model.clone(),
                effort,
                session_cache,
            ),
            model,
            sandbox,
            timeout,
            base_prompt: base_prompt.as_str().to_owned(),
            scope_label: scope.to_string(),
            assignment,
            repo_root,
            runtime_dir,
            provider: GROK_PROVIDER_NAME.clone(),
            process_runner: crate::capability_exec::process::system_process_runner(),
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

    /// Builds the full review prompt by appending the selected scope's files.
    fn build_full_prompt(&self, target: &ReviewTarget) -> String {
        if target.is_empty() {
            return self.base_prompt.clone();
        }
        let file_list = target
            .files()
            .iter()
            .map(|file| format!("- {}", file.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "{base}\n\n\
             ## Review scope: `{scope}`\n\n\
             Review ONLY the following files (this is the `{scope}` scope).\n\
             Re-read the CURRENT file list and CURRENT diff, then fully re-adjudicate this entire scope.\n\n\
             Read the listed files and the current diff before deciding. A verdict produced\n\
             without reading them is invalid — in particular, returning `zero_findings` without\n\
             having inspected the diff is not a pass.\n\n\
             Files:\n{file_list}",
            base = self.base_prompt,
            scope = self.scope_label,
        )
    }

    fn run_process(
        &self,
        args: &[std::ffi::OsString],
    ) -> Result<ProviderProcessOutput, ReviewerError> {
        self.process_runner
            .run(
                "grok",
                None,
                args,
                &self.repo_root,
                &self.runtime_dir,
                &self.provider,
                Some(self.timeout),
                None,
            )
            .map_err(|error| {
                let detail = error.to_string();
                if detail.contains("timed out") {
                    ReviewerError::Timeout
                } else {
                    ReviewerError::Unexpected(detail)
                }
            })
    }

    fn run_attempt(
        &self,
        target: &ReviewTarget,
        resume_id: Option<&str>,
    ) -> Result<ReviewOutcomeRaw, ReviewerError> {
        let prompt = self.build_full_prompt(target);
        let args = build_grok_args(
            self.model.as_str(),
            self.session.effort(),
            &self.sandbox,
            resume_id,
            &prompt,
        );
        let output = self.run_process(&args)?;
        if output.exit_code != 0 {
            return Err(ReviewerError::ReviewerAbort);
        }
        let result = extract_review_result(&output)?;
        let normalized = normalize_final_message(&result);
        let final_message_state = parse_review_final_message(normalized.as_deref());
        let final_message = match &final_message_state {
            ReviewFinalMessageState::Parsed(payload) => Some(
                render_review_payload(payload)
                    .map_err(|error| ReviewerError::Unexpected(error.to_string()))?,
            ),
            _ => normalized,
        };
        let verdict = classify_review_verdict(false, output.exit_code == 0, &final_message_state);
        Ok(ReviewOutcomeRaw {
            verdict,
            final_message,
            session_id: output.session_id,
            log_info: self.runtime_dir.display().to_string(),
        })
    }

    fn run_review(&self, target: &ReviewTarget) -> Result<ReviewOutcomeRaw, ReviewerError> {
        let resume_id = self.session.resumable_id();
        let attempted = self.run_attempt(target, resume_id.as_deref());
        if resume_id.is_some()
            && !matches!(
                attempted.as_ref().map(|raw| &raw.verdict),
                Ok(ReviewVerdict::ZeroFindings | ReviewVerdict::FindingsRemain)
            )
        {
            return self.run_attempt(target, None);
        }
        attempted
    }
}

impl ResolvedReviewer for GrokReviewer {
    fn resolved_assignment(&self) -> &ResolvedReviewerAssignment {
        &self.assignment
    }
}

impl Reviewer for GrokReviewer {
    fn review(&self, target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError> {
        let raw = self.run_review(target)?;
        let session_id = raw.session_id.clone();
        let result = convert_raw_to_final(raw)?;
        self.session.save(session_id);
        Ok(result)
    }

    fn fast_review(&self, target: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError> {
        let raw = self.run_review(target)?;
        let session_id = raw.session_id.clone();
        let result = convert_raw_to_fast(raw)?;
        self.session.save(session_id);
        Ok(result)
    }
}

struct ReviewOutcomeRaw {
    verdict: ReviewVerdict,
    final_message: Option<String>,
    session_id: Option<String>,
    log_info: String,
}

fn extract_review_result(output: &ProviderProcessOutput) -> Result<String, ReviewerError> {
    let message = output.final_message.as_deref().ok_or(ReviewerError::IllegalVerdict)?;
    let envelope = serde_json::from_slice::<GrokOutputEnvelope>(message)
        .map_err(|_| ReviewerError::IllegalVerdict)?;
    let structured_output = envelope
        .into_structured_output()
        .map_err(|error| ReviewerError::Unexpected(error.to_string()))?;
    structured_output
        .get("result")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or(ReviewerError::IllegalVerdict)
}

fn convert_raw_to_final(raw: ReviewOutcomeRaw) -> Result<(Verdict, LogInfo), ReviewerError> {
    let payload = require_successful_payload(&raw)?;
    let verdict = match payload.verdict {
        ReviewPayloadVerdict::ZeroFindings => Verdict::ZeroFindings,
        ReviewPayloadVerdict::FindingsRemain => {
            let findings = convert_findings_to_domain(&payload.findings);
            Verdict::findings_remain(findings).map_err(|error: VerdictError| {
                ReviewerError::Unexpected(format!("verdict construction: {error}"))
            })?
        }
    };
    Ok((verdict, LogInfo::new(raw.log_info)))
}

fn convert_raw_to_fast(raw: ReviewOutcomeRaw) -> Result<(FastVerdict, LogInfo), ReviewerError> {
    let payload = require_successful_payload(&raw)?;
    let verdict = match payload.verdict {
        ReviewPayloadVerdict::ZeroFindings => FastVerdict::ZeroFindings,
        ReviewPayloadVerdict::FindingsRemain => {
            let findings = convert_findings_to_domain(&payload.findings);
            FastVerdict::findings_remain(findings).map_err(|error: VerdictError| {
                ReviewerError::Unexpected(format!("verdict construction: {error}"))
            })?
        }
    };
    Ok((verdict, LogInfo::new(raw.log_info)))
}

fn require_successful_payload(
    raw: &ReviewOutcomeRaw,
) -> Result<usecase::review_workflow::ReviewFinalPayload, ReviewerError> {
    match raw.verdict {
        ReviewVerdict::ZeroFindings | ReviewVerdict::FindingsRemain => {}
        ReviewVerdict::Timeout => return Err(ReviewerError::Timeout),
        ReviewVerdict::ProcessFailed => return Err(ReviewerError::ReviewerAbort),
        ReviewVerdict::LastMessageMissing => return Err(ReviewerError::IllegalVerdict),
    }

    let json = raw.final_message.as_deref().ok_or(ReviewerError::IllegalVerdict)?;
    match parse_review_final_message(Some(json)) {
        ReviewFinalMessageState::Parsed(payload) => Ok(payload),
        _ => Err(ReviewerError::IllegalVerdict),
    }
}

fn convert_findings_to_domain(
    findings: &[usecase::review_workflow::ReviewFinding],
) -> Vec<ReviewerFinding> {
    findings
        .iter()
        .filter_map(|finding| {
            ReviewerFinding::new(
                &finding.message,
                finding.severity.clone(),
                finding.file.clone(),
                finding.line,
                finding.category.clone(),
            )
            .ok()
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use super::*;
    use crate::capability_exec::grok::GROK_STRUCTURED_OUTPUT_SCHEMA;
    use crate::capability_exec::process::ProviderProcessRunner;
    use usecase::capability_exec::{CapabilityExecError, CapabilityFailureDetail};
    use usecase::provider_session::{
        ProviderSessionCacheEntry, ProviderSessionCacheError, ProviderSessionCacheKey,
        ProviderSessionId,
    };

    type RecordedInvocation = (String, Vec<OsString>, Option<Duration>);

    #[derive(Default)]
    struct RecordingProcessRunner {
        invocations: Mutex<Vec<RecordedInvocation>>,
        responses: Mutex<Vec<Result<ProviderProcessOutput, CapabilityExecError>>>,
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
            self.invocations.lock().expect("process recorder lock").push((
                binary.to_owned(),
                args.to_vec(),
                timeout,
            ));
            self.responses
                .lock()
                .expect("process response lock")
                .pop()
                .unwrap_or_else(|| Ok(successful_process_output()))
        }
    }

    struct MemorySessionCache {
        entries: Mutex<HashMap<ProviderSessionCacheKey, ProviderSessionCacheEntry>>,
    }

    impl MemorySessionCache {
        fn with_entry(entry: Option<ProviderSessionCacheEntry>) -> Self {
            let entries = Mutex::new(HashMap::new());
            if let Some(entry) = entry {
                // The reviewer always uses this stable test key.
                entries.lock().expect("session cache lock").insert(test_key(), entry);
            }
            Self { entries }
        }
    }

    impl ProviderSessionCachePort for MemorySessionCache {
        fn load(
            &self,
            key: &ProviderSessionCacheKey,
        ) -> Result<Option<ProviderSessionCacheEntry>, ProviderSessionCacheError> {
            Ok(self.entries.lock().expect("session cache lock").get(key).cloned())
        }

        fn save(
            &self,
            key: &ProviderSessionCacheKey,
            entry: &ProviderSessionCacheEntry,
        ) -> Result<(), ProviderSessionCacheError> {
            self.entries.lock().expect("session cache lock").insert(key.clone(), entry.clone());
            Ok(())
        }

        fn remove(&self, key: &ProviderSessionCacheKey) -> Result<(), ProviderSessionCacheError> {
            self.entries.lock().expect("session cache lock").remove(key);
            Ok(())
        }
    }

    fn test_key() -> ProviderSessionCacheKey {
        ProviderSessionCacheKey::Review {
            track_id: TrackId::try_new("grok-review-test").expect("track id is valid"),
            scope: ScopeName::Other,
            round_type: RoundType::Fast,
            diff_base: CommitHash::try_new("a1b2c3d").expect("diff base is valid"),
        }
    }

    fn session_entry_for(provider: &str, model: &str) -> ProviderSessionCacheEntry {
        ProviderSessionCacheEntry::new(
            ProviderSessionId::try_new("prior-session".to_owned()).expect("session is valid"),
            ProviderName::try_new(provider.to_owned()).expect("provider is valid"),
            ModelName::try_new(model.to_owned()).expect("model is valid"),
            ReasoningEffort::High,
        )
    }

    fn session_entry() -> ProviderSessionCacheEntry {
        session_entry_for(GROK_PROVIDER_NAME.as_str(), "grok-review-model")
    }

    fn resume_process_error() -> CapabilityExecError {
        CapabilityExecError::DispatchFailed {
            provider: GROK_PROVIDER_NAME.clone(),
            detail: CapabilityFailureDetail::new("resume process failed"),
        }
    }

    fn reviewer(
        runner: Arc<RecordingProcessRunner>,
        cache: Arc<dyn ProviderSessionCachePort>,
    ) -> GrokReviewer {
        GrokReviewer::new(
            TrackId::try_new("grok-review-test").expect("track id is valid"),
            ScopeName::Other,
            RoundType::Fast,
            Some(CommitHash::try_new("a1b2c3d").expect("diff base is valid")),
            ModelName::try_new("grok-review-model".to_owned()).expect("model is valid"),
            ReasoningEffort::High,
            GrokSandbox::Workspace,
            Duration::from_secs(10),
            ReviewerPrompt::try_new("Review this code.".to_owned()).expect("prompt is valid"),
            cache,
            PathBuf::from("/test/repository"),
        )
        .with_process_runner(
            PathBuf::from("/test/repository"),
            PathBuf::from("/test/repository/tmp/reviewer-runtime"),
            runner,
        )
    }

    fn successful_process_output() -> ProviderProcessOutput {
        ProviderProcessOutput {
            exit_code: 0,
            session_id: Some("new-grok-session".to_owned()),
            final_message: Some(
                br#"{"structured_output":{"result":"{\"verdict\":\"zero_findings\",\"findings\":[]}"}}"#
                    .to_vec(),
            ),
        }
    }

    fn assert_explicit_grok_settings(args: &[OsString], resume: Option<&str>) {
        assert!(
            args.windows(2).any(|pair| pair == ["-p", "Review this code."]),
            "briefing content must be supplied as the Grok -p prompt"
        );
        assert!(args.windows(2).any(|pair| pair == ["--model", "grok-review-model"]));
        assert!(args.windows(2).any(|pair| pair == ["--reasoning-effort", "high"]));
        assert!(args.windows(2).any(|pair| pair == ["--sandbox", "workspace"]));
        assert!(args.windows(2).any(|pair| pair == ["--output-format", "json"]));
        let schema = args
            .windows(2)
            .find_map(|pair| (pair[0] == "--json-schema").then(|| pair[1].to_string_lossy()))
            .expect("Grok schema is passed");
        let schema: serde_json::Value = serde_json::from_str(&schema).expect("schema is JSON");
        assert_eq!(schema["properties"]["result"]["type"], "string");
        assert_eq!(schema["required"], serde_json::json!(["result"]));
        match resume {
            Some(session) => assert!(args.windows(2).any(|pair| pair == ["--resume", session])),
            None => assert!(!args.iter().any(|arg| arg == "--resume")),
        }
        // `--resume` reuses a session id on a new subprocess. Shared-process
        // connection is `--leader` / `agent`, which this typed pipeline never sets.
        assert!(!args.iter().any(|arg| arg == "agent" || arg == "--leader"));
    }

    #[test]
    fn test_grok_reviewer_implements_reviewer_port() {
        fn accepts_port(_: &dyn Reviewer) {}

        let reviewer = reviewer(
            Arc::new(RecordingProcessRunner::default()),
            Arc::new(MemorySessionCache::with_entry(None)),
        );
        accepts_port(&reviewer);
    }

    #[test]
    fn test_grok_reviewer_resolved_assignment_returns_adapter_values() {
        let reviewer = reviewer(
            Arc::new(RecordingProcessRunner::default()),
            Arc::new(MemorySessionCache::with_entry(None)),
        );
        let assignment =
            <GrokReviewer as usecase::review_v2::ResolvedReviewer>::resolved_assignment(&reviewer);

        assert_eq!(assignment.track_id().as_ref(), "grok-review-test");
        assert_eq!(assignment.scope(), &ScopeName::Other);
        assert_eq!(assignment.provider().as_str(), GROK_PROVIDER_NAME.as_str());
        assert_eq!(assignment.model().as_str(), "grok-review-model");
        assert_eq!(assignment.reasoning_effort(), ReasoningEffort::High);
    }

    #[test]
    fn test_grok_reviewer_typed_pipeline_is_dedicated_path_not_capability_exec()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner::default());
        let reviewer = reviewer(runner.clone(), Arc::new(MemorySessionCache::with_entry(None)));

        reviewer.review(&ReviewTarget::new(vec![]))?;

        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 1);
        let (binary, args, _) = invocations.first().expect("typed-pipeline launch is recorded");
        assert_eq!(binary, "grok");
        assert!(
            !args.iter().any(|arg| arg == "capability" || arg == "exec" || arg == "agent"),
            "typed-pipeline reviewer must launch grok directly, not through capability exec"
        );
        Ok(())
    }

    #[test]
    fn test_grok_reviewer_accepts_briefing_content_and_structured_output_schema()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner::default());
        let reviewer = reviewer(runner.clone(), Arc::new(MemorySessionCache::with_entry(None)));

        reviewer.review(&ReviewTarget::new(vec![]))?;

        let invocations = runner.invocations.lock().expect("process recorder lock");
        let args = &invocations.first().expect("launch is recorded").1;
        assert!(
            args.windows(2).any(|pair| pair == ["-p", "Review this code."]),
            "Grok reviewer must accept the briefing content as its prompt"
        );
        assert!(
            args.windows(2).any(|pair| pair == ["--json-schema", GROK_STRUCTURED_OUTPUT_SCHEMA])
        );
        Ok(())
    }

    #[test]
    fn test_grok_reviewer_typed_pipeline_launch_uses_structured_result_schema_and_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner::default());
        let reviewer = reviewer(runner.clone(), Arc::new(MemorySessionCache::with_entry(None)));
        let target = ReviewTarget::new(vec![]);

        let (verdict, log_info) = reviewer.review(&target)?;

        assert!(matches!(verdict, Verdict::ZeroFindings));
        assert!(!log_info.as_str().is_empty());
        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 1);
        let (binary, args, timeout) = invocations.first().expect("launch is recorded");
        assert_eq!(binary, "grok");
        assert_eq!(*timeout, Some(Duration::from_secs(10)));
        assert!(args.windows(2).any(|pair| pair == ["--model", "grok-review-model"]));
        assert!(args.windows(2).any(|pair| pair == ["--reasoning-effort", "high"]));
        assert!(args.windows(2).any(|pair| pair == ["--sandbox", "workspace"]));
        assert!(args.windows(2).any(|pair| pair == ["--output-format", "json"]));
        assert!(
            args.windows(2).any(|pair| pair == ["--json-schema", GROK_STRUCTURED_OUTPUT_SCHEMA])
        );
        assert!(!args.iter().any(|arg| arg == "agent"));
        assert!(!args.iter().any(|arg| arg == "--leader"));
        assert_explicit_grok_settings(args, None);
        Ok(())
    }

    #[test]
    fn test_grok_reviewer_successful_review_uses_only_structured_result_not_envelope_text()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(vec![Ok(ProviderProcessOutput {
                exit_code: 0,
                session_id: Some("structured-session".to_owned()),
                final_message: Some(
                    br#"{"structured_output":{"result":"{\"verdict\":\"zero_findings\",\"findings\":[]}"},"text":"{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"text must be ignored\"}]}"}"#.to_vec(),
                ),
            })]),
            ..Default::default()
        });
        let reviewer = reviewer(runner, Arc::new(MemorySessionCache::with_entry(None)));

        let (verdict, _) = reviewer.review(&ReviewTarget::new(vec![]))?;

        assert!(matches!(verdict, Verdict::ZeroFindings));
        Ok(())
    }

    #[test]
    fn test_grok_reviewer_resume_failure_retries_fresh_with_all_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(vec![
                Ok(successful_process_output()),
                Ok(ProviderProcessOutput {
                    exit_code: 0,
                    session_id: None,
                    final_message: Some(br#"{"failure_reason":"resume expired"}"#.to_vec()),
                }),
            ]),
            ..Default::default()
        });
        let cache = Arc::new(MemorySessionCache::with_entry(Some(session_entry())));
        let reviewer = reviewer(runner.clone(), cache);
        let target = ReviewTarget::new(vec![]);

        let (verdict, _) = reviewer.review(&target)?;

        assert!(matches!(verdict, Verdict::ZeroFindings));
        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 2);
        let first = &invocations.first().expect("resume launch is recorded").1;
        let second = &invocations.get(1).expect("fresh fallback is recorded").1;
        assert_explicit_grok_settings(first, Some("prior-session"));
        assert_explicit_grok_settings(second, None);
        Ok(())
    }

    #[test]
    fn test_grok_reviewer_fast_review_uses_the_same_typed_pipeline()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner::default());
        let reviewer = reviewer(runner.clone(), Arc::new(MemorySessionCache::with_entry(None)));

        let (verdict, _) = reviewer.fast_review(&ReviewTarget::new(vec![]))?;

        assert!(matches!(verdict, FastVerdict::ZeroFindings));
        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 1);
        let (binary, args, timeout) = invocations.first().expect("fast launch is recorded");
        assert_eq!(binary, "grok");
        assert_eq!(*timeout, Some(Duration::from_secs(10)));
        assert!(args.windows(2).any(|pair| pair == ["--model", "grok-review-model"]));
        assert!(args.windows(2).any(|pair| pair == ["--reasoning-effort", "high"]));
        assert!(args.windows(2).any(|pair| pair == ["--sandbox", "workspace"]));
        assert!(args.windows(2).any(|pair| pair == ["--output-format", "json"]));
        let schema = args
            .windows(2)
            .find_map(|pair| (pair[0] == "--json-schema").then(|| pair[1].to_string_lossy()))
            .expect("fast review must pass the Grok schema");
        let schema: serde_json::Value = serde_json::from_str(&schema)?;
        assert_eq!(schema["properties"]["result"]["type"], "string");
        assert_eq!(schema["required"], serde_json::json!(["result"]));
        assert!(!args.iter().any(|arg| arg == "agent"));
        assert!(!args.iter().any(|arg| arg == "--leader"));
        Ok(())
    }

    #[test]
    fn test_grok_reviewer_failed_resume_retries_fresh_with_all_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(vec![
                Ok(successful_process_output()),
                Err(resume_process_error()),
            ]),
            ..Default::default()
        });
        let cache = Arc::new(MemorySessionCache::with_entry(Some(session_entry())));
        let reviewer = reviewer(runner.clone(), cache);

        let (verdict, _) = reviewer.review(&ReviewTarget::new(vec![]))?;

        assert!(matches!(verdict, Verdict::ZeroFindings));
        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 2);
        let first = &invocations.first().expect("resume launch is recorded").1;
        let second = &invocations.get(1).expect("fresh fallback is recorded").1;
        assert_explicit_grok_settings(first, Some("prior-session"));
        assert_explicit_grok_settings(second, None);
        Ok(())
    }

    #[test]
    fn test_grok_reviewer_provider_mismatched_resume_starts_fresh_with_all_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner::default());
        let cache = Arc::new(MemorySessionCache::with_entry(Some(session_entry_for(
            "codex",
            "grok-review-model",
        ))));
        let reviewer = reviewer(runner.clone(), cache);

        let (verdict, _) = reviewer.review(&ReviewTarget::new(vec![]))?;

        assert!(matches!(verdict, Verdict::ZeroFindings));
        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 1);
        let args = &invocations.first().expect("fresh launch is recorded").1;
        assert_explicit_grok_settings(args, None);
        Ok(())
    }

    #[test]
    fn test_grok_reviewer_model_mismatched_resume_starts_fresh_with_all_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner::default());
        let cache = Arc::new(MemorySessionCache::with_entry(Some(session_entry_for(
            "grok",
            "other-model",
        ))));
        let reviewer = reviewer(runner.clone(), cache);

        let (verdict, _) = reviewer.review(&ReviewTarget::new(vec![]))?;

        assert!(matches!(verdict, Verdict::ZeroFindings));
        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 1);
        let args = &invocations.first().expect("fresh launch is recorded").1;
        assert_explicit_grok_settings(args, None);
        Ok(())
    }

    #[test]
    fn test_grok_reviewer_missing_structured_output_fails_with_envelope_reason()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(vec![Ok(ProviderProcessOutput {
                exit_code: 0,
                session_id: None,
                final_message: Some(
                    br#"{"failure_reason":"provider declined structured output","text":"ignore"}"#
                        .to_vec(),
                ),
            })]),
            ..Default::default()
        });
        let reviewer = reviewer(runner, Arc::new(MemorySessionCache::with_entry(None)));

        let error = reviewer
            .review(&ReviewTarget::new(vec![]))
            .expect_err("failed Grok envelope must fail closed");

        assert!(error.to_string().contains("provider declined structured output"));
        assert!(!error.to_string().contains("ignore"));
        Ok(())
    }

    #[test]
    fn test_grok_reviewer_subprocess_timeout_is_reviewer_timeout()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(vec![Err(CapabilityExecError::DispatchFailed {
                provider: GROK_PROVIDER_NAME.clone(),
                detail: CapabilityFailureDetail::new("provider process timed out"),
            })]),
            ..Default::default()
        });
        let reviewer = reviewer(runner, Arc::new(MemorySessionCache::with_entry(None)));

        let error = reviewer
            .review(&ReviewTarget::new(vec![]))
            .expect_err("timed-out Grok subprocess must be a reviewer timeout");

        assert!(matches!(error, ReviewerError::Timeout));
        Ok(())
    }

    #[test]
    fn test_grok_reviewer_malformed_envelope_is_illegal_verdict()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(vec![Ok(ProviderProcessOutput {
                exit_code: 0,
                session_id: None,
                final_message: Some(b"not-json".to_vec()),
            })]),
            ..Default::default()
        });
        let reviewer = reviewer(runner, Arc::new(MemorySessionCache::with_entry(None)));

        let error = reviewer
            .review(&ReviewTarget::new(vec![]))
            .expect_err("malformed Grok envelope must be an illegal verdict");

        assert!(matches!(error, ReviewerError::IllegalVerdict));
        Ok(())
    }

    #[test]
    fn test_grok_reviewer_nonzero_exit_is_reviewer_abort_before_envelope_decode()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(vec![Ok(ProviderProcessOutput {
                exit_code: 9,
                session_id: None,
                final_message: Some(b"not-json".to_vec()),
            })]),
            ..Default::default()
        });
        let reviewer = reviewer(runner, Arc::new(MemorySessionCache::with_entry(None)));

        let error = reviewer
            .review(&ReviewTarget::new(vec![]))
            .expect_err("nonzero Grok exit must abort before envelope decode");

        assert!(matches!(error, ReviewerError::ReviewerAbort));
        Ok(())
    }

    #[test]
    fn test_grok_reviewer_effort_mismatched_resume_starts_fresh_with_all_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let runner = Arc::new(RecordingProcessRunner::default());
        let cache = Arc::new(MemorySessionCache::with_entry(Some(ProviderSessionCacheEntry::new(
            ProviderSessionId::try_new("prior-session".to_owned()).expect("session is valid"),
            GROK_PROVIDER_NAME.clone(),
            ModelName::try_new("grok-review-model".to_owned()).expect("model is valid"),
            ReasoningEffort::Low,
        ))));
        let reviewer = reviewer(runner.clone(), cache);

        let (verdict, _) = reviewer.review(&ReviewTarget::new(vec![]))?;

        assert!(matches!(verdict, Verdict::ZeroFindings));
        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 1);
        assert_explicit_grok_settings(&invocations.first().expect("fresh launch").1, None);
        Ok(())
    }
}
