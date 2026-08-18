//! Codex-backed implementation of the `Reviewer` usecase port.

use std::sync::Arc;
use std::time::Duration;

use domain::review_v2::{
    FastVerdict, LogInfo, ReviewTarget, ReviewerFinding, RoundType, ScopeName, Verdict,
    VerdictError,
};
use domain::{CommitHash, TrackId};
use usecase::capability_exec::{CODEX_PROVIDER_NAME, ModelName, ReasoningEffort};
use usecase::provider_session::{ProviderSessionCachePort, ReviewerPrompt};
use usecase::review_v2::{
    ResolvedReviewer, ResolvedReviewerAssignment, ReviewerError, ports::Reviewer,
};
use usecase::review_workflow::{
    REVIEW_OUTPUT_SCHEMA_JSON, ReviewFinalMessageState, ReviewPayloadVerdict, ReviewVerdict,
    parse_review_final_message,
};

use super::codex_process::{
    AutoManagedArtifacts, ReviewOutcomeRaw, build_codex_reviewer_invocation,
    initialize_output_last_message, prepare_output_last_message_path, run_codex_child,
    spawn_codex_reviewer, write_runtime_artifact,
};
use super::session::{ReviewerSession, effort_value};
use crate::codex_common::{
    REVIEW_RUNTIME_DIR, resolve_codex_runtime_for_current_repository, runtime_path,
};

/// Codex-backed reviewer implementation for the `Reviewer` usecase port.
///
/// Spawns a `codex exec --sandbox read-only` subprocess, feeds it a review
/// prompt (base prompt + scope file list), polls for completion, and parses
/// the structured JSON verdict written to `--output-last-message`.
pub struct CodexReviewer {
    /// Codex model name (e.g., `"gpt-5.4"` or `"gpt-5.4-mini"`).
    model: ModelName,
    /// Maximum time to wait for the Codex subprocess.
    timeout: Duration,
    /// Base review prompt to send to Codex (before the file list is appended).
    base_prompt: String,
    /// Scope label injected into the prompt (e.g., `"cli"`, `"infrastructure"`).
    scope_label: String,
    session: ReviewerSession,
    /// Validated assignment owned by this reviewer adapter.
    assignment: ResolvedReviewerAssignment,
    /// Test-only: override the Codex binary path (avoids unsafe env var mutation).
    #[cfg(test)]
    bin_override: Option<std::ffi::OsString>,
}

impl CodexReviewer {
    /// Constructs a new `CodexReviewer`.
    ///
    /// # Arguments
    /// - `diff_base`: optional persisted review-cycle base that scopes session reuse.
    /// - `model`: Codex model name.
    /// - `timeout`: Maximum time allowed for the review subprocess.
    /// - `base_prompt`: Review instructions without the scope file list.
    #[allow(clippy::too_many_arguments)] // signature is the catalogue-declared contract
    pub fn new(
        track_id: TrackId,
        scope: ScopeName,
        round_type: RoundType,
        diff_base: Option<CommitHash>,
        model: ModelName,
        effort: ReasoningEffort,
        timeout: Duration,
        base_prompt: ReviewerPrompt,
        session_cache: Arc<dyn ProviderSessionCachePort>,
    ) -> CodexReviewer {
        let assignment = ResolvedReviewerAssignment::new(
            track_id.clone(),
            scope.clone(),
            CODEX_PROVIDER_NAME.clone(),
            model.clone(),
            effort,
        );
        Self {
            session: ReviewerSession::new(
                track_id,
                scope.clone(),
                round_type,
                diff_base,
                "codex",
                model.clone(),
                effort,
                session_cache,
            ),
            model,
            timeout,
            base_prompt: base_prompt.as_str().to_owned(),
            scope_label: scope.to_string(),
            assignment,
            #[cfg(test)]
            bin_override: None,
        }
    }

    /// Sets the scope label injected into the review prompt.
    pub fn with_scope_label(mut self, label: impl Into<String>) -> CodexReviewer {
        self.scope_label = label.into();
        self
    }

    /// Test-only: set a custom binary path instead of the default `codex`.
    #[cfg(test)]
    pub(crate) fn with_bin(mut self, bin: impl Into<std::ffi::OsString>) -> Self {
        self.bin_override = Some(bin.into());
        self
    }

    /// Builds the full prompt by appending the scope file list to the base prompt.
    fn build_full_prompt(&self, target: &ReviewTarget, scope_label: &str) -> String {
        if target.is_empty() {
            return self.base_prompt.clone();
        }
        let file_list = target
            .files()
            .iter()
            .map(|f| format!("- {}", f.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "{base}\n\n\
             ## Review scope: `{scope}`\n\n\
             Review ONLY the following files (this is the `{scope}` scope).\n\
             Re-read the CURRENT file list and CURRENT diff, then fully re-adjudicate this entire scope.\n\n\
             ## How to inspect the repository\n\n\
             You have read-only access to the working tree. Shell commands are available, but the\n\
             sandbox rejects any command wrapped as `bash -lc \"...\"` or `/bin/bash -lc \"...\"`.\n\
             Issue commands directly instead — for example `git diff -- <path>` or\n\
             `rg -n <pattern> <path>`. If a command is rejected for that reason, reissue it in\n\
             direct form; do not abandon the investigation.\n\n\
             Read the listed files and the current diff before deciding. A verdict produced\n\
             without reading them is invalid — in particular, returning `zero_findings` without\n\
             having inspected the diff is not a pass.\n\n\
             Files:\n{file_list}",
            base = self.base_prompt,
            scope = scope_label,
        )
    }

    /// Runs the Codex review and returns a `(verdict_str, log_info)` pair.
    ///
    /// `verdict_str` is the raw JSON string of the final verdict.
    fn run_review(
        &self,
        target: &ReviewTarget,
        scope_label: &str,
    ) -> Result<ReviewOutcomeRaw, ReviewerError> {
        let prompt = self.build_full_prompt(target, scope_label);

        let output_last_message =
            prepare_output_last_message_path(None).map_err(ReviewerError::Unexpected)?;
        let output_schema = runtime_path(REVIEW_RUNTIME_DIR, "codex-output-schema", "json")
            .map_err(ReviewerError::Unexpected)?;
        let session_log = runtime_path(REVIEW_RUNTIME_DIR, "codex-session", "log")
            .map_err(ReviewerError::Unexpected)?;

        // Auto-managed: output-last-message and output-schema are cleaned up on drop.
        // Session log is NOT auto-managed — it persists for post-run debugging.
        let _cleanup = AutoManagedArtifacts::new([&output_last_message, &output_schema]);

        // Write output schema file.
        write_runtime_artifact(&output_schema, REVIEW_OUTPUT_SCHEMA_JSON.as_bytes()).map_err(
            |e| ReviewerError::Unexpected(format!("failed to write output-schema: {e}")),
        )?;

        #[cfg(test)]
        let runtime = if self.bin_override.is_none() {
            Some(
                resolve_codex_runtime_for_current_repository()
                    .map_err(ReviewerError::Unexpected)?,
            )
        } else {
            None
        };
        #[cfg(test)]
        let (bin, runtime_for_spawn) = match (&self.bin_override, runtime.as_ref()) {
            (Some(bin), _) => (bin.clone(), None),
            (None, Some(runtime)) => (runtime.executable().to_os_string(), Some(runtime)),
            (None, None) => {
                return Err(ReviewerError::Unexpected("test Codex runtime missing".to_owned()));
            }
        };
        #[cfg(not(test))]
        let runtime =
            resolve_codex_runtime_for_current_repository().map_err(ReviewerError::Unexpected)?;
        #[cfg(not(test))]
        let (bin, runtime_for_spawn) = (runtime.executable().to_os_string(), Some(&runtime));

        let resume_id = self.session.resumable_id();
        let run = |resume_id: Option<&str>| {
            // Both resumed and fresh attempts share this path. Reset the
            // authoritative output before every child so a failed resume
            // cannot donate its verdict to the fresh retry.
            initialize_output_last_message(&output_last_message).map_err(|e| {
                ReviewerError::Unexpected(format!("failed to initialize output-last-message: {e}"))
            })?;
            let invocation = build_codex_reviewer_invocation(
                self.model.as_str(),
                effort_value(self.session.effort()),
                resume_id,
                &prompt,
                &output_last_message,
                &output_schema,
            );
            let (child, stderr, stdout) =
                spawn_codex_reviewer(&bin, &invocation, &session_log, runtime_for_spawn)
                    .map_err(ReviewerError::Unexpected)?;
            run_codex_child(
                child,
                stderr,
                stdout,
                self.timeout,
                output_last_message.clone(),
                &session_log,
            )
        };
        let attempted = run(resume_id.as_deref());
        if resume_id.is_some()
            && !matches!(
                attempted.as_ref().map(|raw| &raw.verdict),
                Ok(ReviewVerdict::ZeroFindings | ReviewVerdict::FindingsRemain)
            )
        {
            return run(None);
        }
        attempted
    }
}

impl ResolvedReviewer for CodexReviewer {
    fn resolved_assignment(&self) -> &ResolvedReviewerAssignment {
        &self.assignment
    }
}

impl Reviewer for CodexReviewer {
    fn review(&self, target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError> {
        let raw = self.run_review(target, &self.scope_label)?;
        let session_id = raw.session_id.clone();
        let (verdict, log_info) = convert_raw_to_final(raw)?;
        self.session.save(session_id);
        Ok((verdict, log_info))
    }

    fn fast_review(&self, target: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError> {
        let raw = self.run_review(target, &self.scope_label)?;
        let session_id = raw.session_id.clone();
        let (verdict, log_info) = convert_raw_to_fast(raw)?;
        self.session.save(session_id);
        Ok((verdict, log_info))
    }
}

/// Converts a raw Codex outcome to a final `(Verdict, LogInfo)`.
///
/// # Errors
/// Returns `ReviewerError` if the verdict indicates failure or the payload cannot be parsed.
fn convert_raw_to_final(raw: ReviewOutcomeRaw) -> Result<(Verdict, LogInfo), ReviewerError> {
    let payload = require_successful_payload(&raw)?;
    let log_info = LogInfo::new(raw.session_log_path.display().to_string());

    let verdict = match payload.verdict {
        ReviewPayloadVerdict::ZeroFindings => Verdict::ZeroFindings,
        ReviewPayloadVerdict::FindingsRemain => {
            let findings = convert_findings_to_domain(&payload.findings);
            Verdict::findings_remain(findings).map_err(|e: VerdictError| {
                ReviewerError::Unexpected(format!("verdict construction: {e}"))
            })?
        }
    };
    Ok((verdict, log_info))
}

/// Converts a raw Codex outcome to a fast `(FastVerdict, LogInfo)`.
///
/// # Errors
/// Returns `ReviewerError` if the verdict indicates failure or the payload cannot be parsed.
fn convert_raw_to_fast(raw: ReviewOutcomeRaw) -> Result<(FastVerdict, LogInfo), ReviewerError> {
    let payload = require_successful_payload(&raw)?;
    let log_info = LogInfo::new(raw.session_log_path.display().to_string());

    let verdict = match payload.verdict {
        ReviewPayloadVerdict::ZeroFindings => FastVerdict::ZeroFindings,
        ReviewPayloadVerdict::FindingsRemain => {
            let findings = convert_findings_to_domain(&payload.findings);
            FastVerdict::findings_remain(findings).map_err(|e: VerdictError| {
                ReviewerError::Unexpected(format!("verdict construction: {e}"))
            })?
        }
    };
    Ok((verdict, log_info))
}

/// Extracts the parsed payload from the raw outcome, mapping error variants to `ReviewerError`.
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
        ReviewFinalMessageState::Parsed(p) => Ok(p),
        _ => Err(ReviewerError::IllegalVerdict),
    }
}

/// Converts `usecase::review_workflow::ReviewFinding` slice to domain `ReviewerFinding` vec.
fn convert_findings_to_domain(
    findings: &[usecase::review_workflow::ReviewFinding],
) -> Vec<ReviewerFinding> {
    findings
        .iter()
        .filter_map(|f| {
            ReviewerFinding::new(
                &f.message,
                f.severity.clone(),
                f.file.clone(),
                f.line,
                f.category.clone(),
            )
            .ok()
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::super::codex_process::{
        MAX_CODEX_EVENT_BYTES, collect_codex_session_id, read_bounded_output_last_message,
    };
    use super::*;
    use std::path::Path;

    struct StaticSessionCache {
        entry: Option<usecase::provider_session::ProviderSessionCacheEntry>,
    }

    impl ProviderSessionCachePort for StaticSessionCache {
        fn load(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
        ) -> Result<
            Option<usecase::provider_session::ProviderSessionCacheEntry>,
            usecase::provider_session::ProviderSessionCacheError,
        > {
            Ok(self.entry.clone())
        }

        fn save(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
            _: &usecase::provider_session::ProviderSessionCacheEntry,
        ) -> Result<(), usecase::provider_session::ProviderSessionCacheError> {
            Ok(())
        }

        fn remove(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
        ) -> Result<(), usecase::provider_session::ProviderSessionCacheError> {
            Ok(())
        }
    }

    struct EmptySessionCache;

    impl ProviderSessionCachePort for EmptySessionCache {
        fn load(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
        ) -> Result<
            Option<usecase::provider_session::ProviderSessionCacheEntry>,
            usecase::provider_session::ProviderSessionCacheError,
        > {
            Ok(None)
        }
        fn save(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
            _: &usecase::provider_session::ProviderSessionCacheEntry,
        ) -> Result<(), usecase::provider_session::ProviderSessionCacheError> {
            Ok(())
        }
        fn remove(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
        ) -> Result<(), usecase::provider_session::ProviderSessionCacheError> {
            Ok(())
        }
    }

    struct KeyedSessionCache {
        entry: Option<usecase::provider_session::ProviderSessionCacheEntry>,
        expected_key: usecase::provider_session::ProviderSessionCacheKey,
    }

    impl ProviderSessionCachePort for KeyedSessionCache {
        fn load(
            &self,
            key: &usecase::provider_session::ProviderSessionCacheKey,
        ) -> Result<
            Option<usecase::provider_session::ProviderSessionCacheEntry>,
            usecase::provider_session::ProviderSessionCacheError,
        > {
            Ok((key == &self.expected_key).then(|| self.entry.clone()).flatten())
        }

        fn save(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
            _: &usecase::provider_session::ProviderSessionCacheEntry,
        ) -> Result<(), usecase::provider_session::ProviderSessionCacheError> {
            Ok(())
        }

        fn remove(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
        ) -> Result<(), usecase::provider_session::ProviderSessionCacheError> {
            Ok(())
        }
    }

    fn test_reviewer(timeout: Duration, prompt: &str) -> CodexReviewer {
        test_reviewer_with_cache(timeout, prompt, Arc::new(EmptySessionCache))
    }

    fn test_reviewer_with_cache(
        timeout: Duration,
        prompt: &str,
        cache: Arc<dyn ProviderSessionCachePort>,
    ) -> CodexReviewer {
        CodexReviewer::new(
            TrackId::try_new("test-track").unwrap(),
            ScopeName::Other,
            RoundType::Fast,
            Some(CommitHash::try_new("a1b2c3d").unwrap()),
            ModelName::try_new("gpt-5.4").unwrap(),
            ReasoningEffort::High,
            timeout,
            ReviewerPrompt::try_new(prompt.to_owned()).unwrap(),
            cache,
        )
    }

    #[test]
    fn test_codex_reviewer_resolved_assignment_returns_adapter_values() {
        let mut reviewer: CodexReviewer = test_reviewer(Duration::from_secs(10), "Review.");
        let assignment_before =
            <CodexReviewer as usecase::review_v2::ResolvedReviewer>::resolved_assignment(&reviewer)
                .clone();

        // `model` and `scope_label` are mutable invocation configuration. The
        // constructor stores a separate resolved assignment snapshot, so a
        // later configuration change must not alter values persisted to telemetry.
        reviewer.model = ModelName::try_new("profile-mutated-model").unwrap();
        reviewer.scope_label = "profile-mutated-scope".to_owned();
        let assignment =
            <CodexReviewer as usecase::review_v2::ResolvedReviewer>::resolved_assignment(&reviewer);

        assert_eq!(reviewer.model.as_str(), "profile-mutated-model");
        assert_eq!(reviewer.scope_label, "profile-mutated-scope");
        assert_eq!(assignment, &assignment_before);
        assert_eq!(assignment.track_id().as_ref(), "test-track");
        assert_eq!(assignment.scope(), &ScopeName::Other);
        assert_eq!(assignment.provider().as_str(), "codex");
        assert_eq!(assignment.model().as_str(), "gpt-5.4");
        assert_eq!(assignment.reasoning_effort(), ReasoningEffort::High);
    }

    fn session_entry(provider: &str) -> usecase::provider_session::ProviderSessionCacheEntry {
        session_entry_with_model(provider, "gpt-5.4")
    }

    fn session_entry_with_model(
        provider: &str,
        model: &str,
    ) -> usecase::provider_session::ProviderSessionCacheEntry {
        usecase::provider_session::ProviderSessionCacheEntry::new(
            usecase::provider_session::ProviderSessionId::try_new("prior-session".to_owned())
                .unwrap(),
            usecase::capability_exec::ProviderName::try_new(provider.to_owned()).unwrap(),
            ModelName::try_new(model.to_owned()).unwrap(),
            ReasoningEffort::High,
        )
    }

    #[test]
    fn test_codex_reviewer_build_full_prompt_with_files() {
        let reviewer = test_reviewer(Duration::from_secs(600), "Review this code.");
        let files = vec![
            domain::review_v2::FilePath::new("src/lib.rs").unwrap(),
            domain::review_v2::FilePath::new("src/main.rs").unwrap(),
        ];
        let target = ReviewTarget::new(files);
        let prompt = reviewer.build_full_prompt(&target, "domain");

        assert!(prompt.starts_with("Review this code."));
        assert!(prompt.contains("## Review scope: `domain`"));
        assert!(prompt.contains("- src/lib.rs"));
        assert!(prompt.contains("- src/main.rs"));
        assert!(prompt.contains("CURRENT file list"));
        assert!(prompt.contains("CURRENT diff"));
        assert!(prompt.contains("fully re-adjudicate this entire scope"));
    }

    #[cfg(unix)]
    #[test]
    fn test_codex_reviewer_resume_and_fresh_rounds_reinject_flags_reread_scope_and_preserve_verdict_unit()
     {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let write_bin = |name: &str, expected_resume: bool| {
            let script = dir.path().join(name);
            let resume_check = if expected_resume {
                "[ \"$has_resume\" -eq 1 ]"
            } else {
                "[ \"$has_resume\" -eq 0 ]"
            };
            std::fs::write(
                &script,
                format!(
                    r#"#!/bin/sh
has_model=0
has_sandbox=0
has_effort=0
has_resume=0
output=""
last=""
previous=""
for argument in "$@"; do
  [ "$previous" = "--model" ] && [ "$argument" = "gpt-5.4" ] && has_model=1
  [ "$previous" = "--sandbox" ] && [ "$argument" = "read-only" ] && has_sandbox=1
  [ "$previous" = "--output-last-message" ] && output="$argument"
  [ "$argument" = "model_reasoning_effort=\"high\"" ] && has_effort=1
  [ "$argument" = "resume" ] && has_resume=1
  previous="$argument"
  last="$argument"
done
[ "$has_model" -eq 1 ] && [ "$has_sandbox" -eq 1 ] && [ "$has_effort" -eq 1 ] && [ -n "$output" ] && {resume_check} || exit 9
case "$last" in
  *"CURRENT file list"*"CURRENT diff"*"fully re-adjudicate this entire scope"*"src/lib.rs"*) ;;
  *) exit 10 ;;
esac
printf '{{"verdict":"zero_findings","findings":[]}}\n' > "$output"
printf '{{"thread_id":"new-session"}}\n'
"#
                ),
            )
            .unwrap();
            let mut permissions = std::fs::metadata(&script).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script, permissions).unwrap();
            script
        };

        let target =
            ReviewTarget::new(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()]);
        let resumed = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(StaticSessionCache { entry: Some(session_entry("codex")) }),
        )
        .with_bin(write_bin("resume-codex.sh", true))
        .review(&target)
        .unwrap();
        let model_mismatch = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(StaticSessionCache {
                entry: Some(session_entry_with_model("codex", "previous-model")),
            }),
        )
        .with_bin(write_bin("model-mismatch-codex.sh", false))
        .review(&target)
        .unwrap();
        let provider_mismatch = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(StaticSessionCache { entry: Some(session_entry("claude")) }),
        )
        .with_bin(write_bin("provider-mismatch-codex.sh", false))
        .review(&target)
        .unwrap();
        let first_round = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(EmptySessionCache),
        )
        .with_bin(write_bin("first-round-codex.sh", false))
        .review(&target)
        .unwrap();

        assert_eq!(resumed.0, model_mismatch.0);
        assert_eq!(resumed.0, provider_mismatch.0);
        assert_eq!(resumed.0, first_round.0);
        assert!(matches!(resumed.0, Verdict::ZeroFindings));
        // Codex uses distinct log paths per subprocess, but resumed and fresh
        // results both preserve the record's verdict + non-empty log-info unit.
        assert!(!resumed.1.as_str().is_empty());
        assert!(!model_mismatch.1.as_str().is_empty());
        assert!(!provider_mismatch.1.as_str().is_empty());
        assert!(!first_round.1.as_str().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn test_codex_reviewer_resume_failure_retries_fresh_invocation() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let attempts = dir.path().join("attempts.log");
        let script = dir.path().join("resume-fallback-codex.sh");
        std::fs::write(
            &script,
            format!(
                r#"#!/bin/sh
has_resume=0
has_model=0
has_sandbox=0
has_effort=0
output=""
previous=""
for argument in "$@"; do
  [ "$argument" = "resume" ] && has_resume=1
  [ "$previous" = "--model" ] && [ "$argument" = "gpt-5.4" ] && has_model=1
  [ "$previous" = "--sandbox" ] && [ "$argument" = "read-only" ] && has_sandbox=1
  [ "$previous" = "--output-last-message" ] && output="$argument"
  [ "$argument" = "model_reasoning_effort=\"high\"" ] && has_effort=1
  previous="$argument"
done
[ "$has_model" -eq 1 ] && [ "$has_sandbox" -eq 1 ] && [ "$has_effort" -eq 1 ] || exit 9
if [ "$has_resume" -eq 1 ]; then
  printf 'resume\n' >> '{}'
  exit 7
fi
printf 'fresh\n' >> '{}'
printf '{{"verdict":"zero_findings","findings":[]}}\n' > "$output"
printf '{{"thread_id":"new-session"}}\n'
"#,
                attempts.display(),
                attempts.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();

        let target =
            ReviewTarget::new(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()]);
        let record = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(StaticSessionCache { entry: Some(session_entry("codex")) }),
        )
        .with_bin(script)
        .review(&target)
        .unwrap();

        assert!(matches!(record.0, Verdict::ZeroFindings));
        assert_eq!(std::fs::read_to_string(attempts).unwrap(), "resume\nfresh\n");
    }

    #[cfg(unix)]
    #[test]
    fn test_codex_reviewer_fresh_retry_does_not_adopt_failed_resume_verdict() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let attempts = dir.path().join("attempts.log");
        let script = dir.path().join("stale-resume-verdict-codex.sh");
        std::fs::write(
            &script,
            format!(
                r#"#!/bin/sh
has_resume=0
output=""
previous=""
for argument in "$@"; do
  [ "$argument" = "resume" ] && has_resume=1
  [ "$previous" = "--output-last-message" ] && output="$argument"
  previous="$argument"
done
if [ "$has_resume" -eq 1 ]; then
  printf 'resume\n' >> '{}'
  printf '{{"verdict":"zero_findings","findings":[]}}\n' > "$output"
  exit 7
fi
printf 'fresh\n' >> '{}'
exit 0
"#,
                attempts.display(),
                attempts.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();

        let target =
            ReviewTarget::new(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()]);
        let result = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(StaticSessionCache { entry: Some(session_entry("codex")) }),
        )
        .with_bin(script)
        .review(&target);

        assert!(matches!(result, Err(ReviewerError::IllegalVerdict)));
        assert_eq!(std::fs::read_to_string(attempts).unwrap(), "resume\nfresh\n");
    }

    #[cfg(unix)]
    #[test]
    fn test_codex_reviewer_expired_session_starts_fresh_with_explicit_flags() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let attempts = dir.path().join("attempts.log");
        let script = dir.path().join("expired-session-codex.sh");
        std::fs::write(
            &script,
            format!(
                r#"#!/bin/sh
has_resume=0
has_model=0
has_sandbox=0
has_effort=0
output=""
previous=""
for argument in "$@"; do
  [ "$argument" = "resume" ] && has_resume=1
  [ "$previous" = "--model" ] && [ "$argument" = "gpt-5.4" ] && has_model=1
  [ "$previous" = "--sandbox" ] && [ "$argument" = "read-only" ] && has_sandbox=1
  [ "$previous" = "--output-last-message" ] && output="$argument"
  [ "$argument" = "model_reasoning_effort=\"high\"" ] && has_effort=1
  previous="$argument"
done
[ "$has_model" -eq 1 ] && [ "$has_sandbox" -eq 1 ] && [ "$has_effort" -eq 1 ] && [ -n "$output" ] || exit 9
if [ "$has_resume" -eq 1 ]; then
  printf 'resume\n' >> '{}'
  printf 'expired or unknown session\n' >&2
  exit 7
fi
printf 'fresh\n' >> '{}'
printf '{{"verdict":"zero_findings","findings":[]}}\n' > "$output"
printf '{{"thread_id":"new-session"}}\n'
"#,
                attempts.display(),
                attempts.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();

        let target =
            ReviewTarget::new(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()]);
        let record = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(StaticSessionCache { entry: Some(session_entry("codex")) }),
        )
        .with_bin(script)
        .review(&target)
        .unwrap();

        assert!(matches!(record.0, Verdict::ZeroFindings));
        assert_eq!(std::fs::read_to_string(attempts).unwrap(), "resume\nfresh\n");
    }

    #[cfg(unix)]
    #[test]
    fn test_codex_reviewer_resumes_only_matching_track_scope_and_round_key_unit() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let write_bin = |name: &str, expected_resume: bool| {
            let script = dir.path().join(name);
            let resume_check = if expected_resume {
                "[ \"$has_resume\" -eq 1 ]"
            } else {
                "[ \"$has_resume\" -eq 0 ]"
            };
            std::fs::write(
                &script,
                format!(
                    r#"#!/bin/sh
has_resume=0
has_model=0
has_sandbox=0
has_effort=0
output=""
previous=""
for argument in "$@"; do
  [ "$argument" = "resume" ] && has_resume=1
  [ "$previous" = "--model" ] && [ "$argument" = "gpt-5.4" ] && has_model=1
  [ "$previous" = "--sandbox" ] && [ "$argument" = "read-only" ] && has_sandbox=1
  [ "$previous" = "--output-last-message" ] && output="$argument"
  [ "$argument" = "model_reasoning_effort=\"high\"" ] && has_effort=1
  previous="$argument"
done
[ "$has_model" -eq 1 ] && [ "$has_sandbox" -eq 1 ] && [ "$has_effort" -eq 1 ] && [ -n "$output" ] && {resume_check} || exit 9
printf '{{"verdict":"zero_findings","findings":[]}}\n' > "$output"
printf '{{"thread_id":"new-session"}}\n'
"#
                ),
            )
            .unwrap();
            let mut permissions = std::fs::metadata(&script).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script, permissions).unwrap();
            script
        };
        let target =
            ReviewTarget::new(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()]);
        let track_id = TrackId::try_new("test-track").unwrap();
        let scope = ScopeName::Other;
        let diff_base = CommitHash::try_new("a1b2c3d").unwrap();
        let matched_key = usecase::provider_session::ProviderSessionCacheKey::Review {
            track_id: track_id.clone(),
            scope: scope.clone(),
            round_type: RoundType::Fast,
            diff_base: diff_base.clone(),
        };
        let wrong_track_key = usecase::provider_session::ProviderSessionCacheKey::Review {
            track_id: TrackId::try_new("other-track").unwrap(),
            scope: scope.clone(),
            round_type: RoundType::Fast,
            diff_base: diff_base.clone(),
        };
        let wrong_scope_key = usecase::provider_session::ProviderSessionCacheKey::Review {
            track_id: track_id.clone(),
            scope: ScopeName::Main(
                domain::review_v2::MainScopeName::new("infrastructure").unwrap(),
            ),
            round_type: RoundType::Fast,
            diff_base: diff_base.clone(),
        };

        let run = |round_type, expected_key, name, expects_resume| {
            CodexReviewer::new(
                track_id.clone(),
                scope.clone(),
                round_type,
                Some(diff_base.clone()),
                ModelName::try_new("gpt-5.4").unwrap(),
                ReasoningEffort::High,
                Duration::from_secs(10),
                ReviewerPrompt::try_new("Review this code.".to_owned()).unwrap(),
                Arc::new(KeyedSessionCache { entry: Some(session_entry("codex")), expected_key }),
            )
            .with_bin(write_bin(name, expects_resume))
            .review(&target)
            .unwrap()
        };

        assert!(matches!(
            run(RoundType::Fast, wrong_track_key, "wrong-track-codex.sh", false).0,
            Verdict::ZeroFindings
        ));
        assert!(matches!(
            run(RoundType::Fast, wrong_scope_key, "wrong-scope-codex.sh", false).0,
            Verdict::ZeroFindings
        ));
        assert!(matches!(
            run(RoundType::Final, matched_key.clone(), "fast-keyed-final-codex.sh", false).0,
            Verdict::ZeroFindings
        ));
        assert!(matches!(
            run(RoundType::Fast, matched_key, "matching-key-codex.sh", true).0,
            Verdict::ZeroFindings
        ));
    }

    #[test]
    fn test_codex_reviewer_build_full_prompt_empty_target_returns_base_prompt() {
        let reviewer = test_reviewer(Duration::from_secs(600), "Review this code.");
        let target = ReviewTarget::new(vec![]);
        let prompt = reviewer.build_full_prompt(&target, "domain");

        assert_eq!(prompt, "Review this code.");
    }

    #[test]
    fn test_codex_resume_args_reinject_all_execution_flags() {
        let args = build_codex_reviewer_invocation(
            "codex-model",
            "xhigh",
            Some("prior-session"),
            "Review.",
            Path::new("tmp/last-message.json"),
            Path::new("tmp/schema.json"),
        );
        let args: Vec<&str> = args.iter().filter_map(|arg| arg.to_str()).collect();
        assert!(args.windows(2).any(|pair| pair == ["resume", "prior-session"]));
        assert!(args.windows(2).any(|pair| pair == ["--model", "codex-model"]));
        assert!(args.windows(2).any(|pair| pair == ["--sandbox", "read-only"]));
        assert!(args.contains(&"model_reasoning_effort=\"xhigh\""));
    }

    #[test]
    fn test_collect_codex_session_id_discards_oversized_event_and_keeps_draining() {
        let mut stdout = vec![b'x'; MAX_CODEX_EVENT_BYTES.saturating_add(1)];
        stdout.push(b'\n');
        stdout.extend_from_slice(br#"{"thread_id":"captured-session"}"#);
        stdout.push(b'\n');
        stdout.extend_from_slice(b"discarded-after-session\n");

        assert_eq!(
            collect_codex_session_id(std::io::Cursor::new(stdout)),
            Some("captured-session".to_owned())
        );
    }

    #[test]
    fn test_read_bounded_output_last_message_over_limit_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("last-message.json");
        std::fs::write(&path, b"abcde").unwrap();

        let error = read_bounded_output_last_message(&path, 4).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("exceeds maximum size of 4 bytes"));
    }

    #[cfg(unix)]
    #[test]
    fn test_read_bounded_output_last_message_symlink_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real-last-message.json");
        let link = dir.path().join("last-message.json");
        std::fs::write(&target, b"{}\n").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let error = read_bounded_output_last_message(&link, 4).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("symlinked output-last-message"));
    }

    #[cfg(unix)]
    #[test]
    fn test_initialize_output_last_message_symlink_returns_error_without_truncating_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("verdict.json");
        let link = dir.path().join("last-message.json");
        std::fs::write(&target, b"preserve this verdict").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let error = initialize_output_last_message(&link).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("symlinked output-last-message"));
        assert_eq!(std::fs::read(&target).unwrap(), b"preserve this verdict");
    }

    #[cfg(unix)]
    #[test]
    fn test_initialize_output_last_message_symlinked_parent_preserves_target() {
        let dir = tempfile::tempdir().unwrap();
        let redirected_parent = dir.path().join("redirected");
        let link_parent = dir.path().join("runtime");
        std::fs::create_dir_all(&redirected_parent).unwrap();
        std::fs::write(redirected_parent.join("last-message.json"), b"preserve this verdict")
            .unwrap();
        std::os::unix::fs::symlink(&redirected_parent, &link_parent).unwrap();

        let error =
            initialize_output_last_message(&link_parent.join("last-message.json")).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("refusing to follow symlink"));
        assert_eq!(
            std::fs::read(redirected_parent.join("last-message.json")).unwrap(),
            b"preserve this verdict"
        );
    }

    #[test]
    fn test_convert_findings_to_domain_skips_empty_message() {
        // ReviewerFindingError::EmptyMessage causes filter_map to skip the item
        let findings = vec![usecase::review_workflow::ReviewFinding {
            message: "  ".to_owned(), // whitespace-only → empty after trim
            severity: None,
            file: None,
            line: None,
            category: None,
        }];
        let result = convert_findings_to_domain(&findings);
        assert!(result.is_empty(), "empty-message findings must be filtered out: {result:?}");
    }

    #[test]
    fn test_runtime_path_is_unique_across_calls() {
        let p1 = runtime_path(REVIEW_RUNTIME_DIR, "test-unique", "txt").unwrap();
        let p2 = runtime_path(REVIEW_RUNTIME_DIR, "test-unique", "txt").unwrap();
        assert_ne!(p1, p2, "sequential runtime_path calls must produce unique names");
        // Cleanup
        let _ = std::fs::remove_file(&p1);
        let _ = std::fs::remove_file(&p2);
    }

    #[cfg(unix)]
    #[test]
    fn test_review_with_fake_codex_zero_findings() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-codex.sh");
        std::fs::write(
            &script,
            r#"#!/bin/sh
out=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-last-message) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
if [ -n "$out" ]; then
  printf '{"verdict":"zero_findings","findings":[]}\n' > "$out"
fi
exit 0
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let reviewer = test_reviewer(Duration::from_secs(10), "Review.").with_bin(&script);
        let target =
            ReviewTarget::new(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()]);
        let result = reviewer.review(&target);

        let (verdict, _log) = result.expect("review should succeed");
        assert!(
            matches!(verdict, domain::review_v2::Verdict::ZeroFindings),
            "expected ZeroFindings, got: {verdict:?}"
        );
    }

    #[test]
    fn test_convert_findings_to_domain_converts_valid_finding() {
        let findings = vec![usecase::review_workflow::ReviewFinding {
            message: "Missing error handling".to_owned(),
            severity: Some("P1".to_owned()),
            file: Some("src/lib.rs".to_owned()),
            line: Some(42),
            category: Some("error_handling".to_owned()),
        }];
        let result = convert_findings_to_domain(&findings);
        assert_eq!(result.len(), 1);
        let finding = result.first().expect("expected one finding");
        assert_eq!(finding.message(), "Missing error handling");
        assert_eq!(finding.severity(), Some("P1"));
        assert_eq!(finding.file(), Some("src/lib.rs"));
        assert_eq!(finding.line(), Some(42));
        assert_eq!(finding.category(), Some("error_handling"));
    }
}
