//! Safe reviewer-process diagnostic construction.
//!
//! Reviewer adapters receive arbitrary provider bytes. This module keeps the
//! existing credential-redaction boundary in infrastructure and converts only
//! valid, redacted, bounded text into the usecase diagnostic types.

use usecase::capability_exec::ProviderName;
use usecase::program_runner::ProgramExitCode;
use usecase::review_v2::{
    ReviewerDiagnostic, ReviewerError, ReviewerExecutionProvenance, ReviewerProcessDiagnostic,
};

use super::review_fix_runner::redact_credentials;

pub(super) fn diagnostic_from_bytes(bytes: &[u8]) -> ReviewerProcessDiagnostic {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return ReviewerProcessDiagnostic::Unavailable;
    };
    diagnostic_from_text(text)
}

pub(super) fn diagnostic_from_text(text: &str) -> ReviewerProcessDiagnostic {
    let redacted = redact_credentials(text);
    ReviewerDiagnostic::try_new(redacted)
        .map(ReviewerProcessDiagnostic::Available)
        .unwrap_or(ReviewerProcessDiagnostic::Unavailable)
}

pub(super) fn safe_log_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    match diagnostic_from_text(text) {
        ReviewerProcessDiagnostic::Available(value) => value.as_str().to_owned(),
        ReviewerProcessDiagnostic::Unavailable => "diagnostic_unavailable".to_owned(),
    }
}

pub(super) fn unexpected_from_text(
    provenance: ReviewerExecutionProvenance,
    text: impl AsRef<str>,
) -> ReviewerError {
    ReviewerError::Unexpected { provenance, diagnostic: diagnostic_from_text(text.as_ref()) }
}

pub(super) fn process_failed_from_bytes(
    provider: &ProviderName,
    exit_code: Option<i32>,
    bytes: Option<&[u8]>,
) -> ReviewerError {
    ReviewerError::ProcessFailed {
        provider: provider.clone(),
        exit_code: exit_code.map(ProgramExitCode::new),
        diagnostic: bytes
            .map(diagnostic_from_bytes)
            .unwrap_or(ReviewerProcessDiagnostic::Unavailable),
    }
}

pub(super) fn process_failed_from_text(
    provider: &ProviderName,
    exit_code: Option<i32>,
    text: Option<&str>,
) -> ReviewerError {
    ReviewerError::ProcessFailed {
        provider: provider.clone(),
        exit_code: exit_code.map(ProgramExitCode::new),
        diagnostic: text
            .map(diagnostic_from_text)
            .unwrap_or(ReviewerProcessDiagnostic::Unavailable),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn raw_diagnostic_boundary_suppresses_invalid_and_oversized_input_preserving_exit_code() {
        let provider = ProviderName::try_new("codex").unwrap();
        let oversized = vec![b'x'; 4097];
        let invalid_utf8 = vec![0xff, 0xfe, 0xfd];

        for raw in [oversized, invalid_utf8] {
            let error = process_failed_from_bytes(&provider, Some(23), Some(raw.as_slice()));
            assert_eq!(
                error.to_string(),
                "reviewer process failed: provider=codex, exit_code=23, diagnostic=diagnostic_unavailable"
            );
        }

        // The safe path retains bounded, already-redacted text. The pure
        // credential replacement behavior is covered by the existing
        // review-fix-runner redaction tests; this assertion proves the
        // production diagnostic adapter consumes that boundary.
        let safe = process_failed_from_text(
            &provider,
            Some(7),
            Some("safe [REDACTED:OPENAI_API_KEY] provider detail"),
        );
        assert_eq!(
            safe.to_string(),
            "reviewer process failed: provider=codex, exit_code=7, diagnostic=safe [REDACTED:OPENAI_API_KEY] provider detail"
        );
    }

    #[test]
    fn test_diagnostic_redaction_covers_claude_and_grok_credentials() {
        let anthropic_key = "sk-ant-FAKE-DIAGNOSTIC-KEY";
        let anthropic_token = "anthropic-FAKE-DIAGNOSTIC-TOKEN";
        let xai_key = "xai-FAKE-DIAGNOSTIC-KEY";

        temp_env::with_vars(
            [
                ("ANTHROPIC_API_KEY", Some(anthropic_key)),
                ("ANTHROPIC_AUTH_TOKEN", Some(anthropic_token)),
                ("XAI_API_KEY", Some(xai_key)),
            ],
            || {
                let raw = format!(
                    "claude_key={anthropic_key} claude_token={anthropic_token} grok_key={xai_key}"
                );
                let diagnostic = diagnostic_from_text(&raw);
                let ReviewerProcessDiagnostic::Available(value) = diagnostic else {
                    panic!("provider credentials should be redacted before validation");
                };

                assert!(!value.as_str().contains(anthropic_key));
                assert!(!value.as_str().contains(anthropic_token));
                assert!(!value.as_str().contains(xai_key));
                assert!(value.as_str().contains("[REDACTED:ANTHROPIC_API_KEY]"));
                assert!(value.as_str().contains("[REDACTED:ANTHROPIC_AUTH_TOKEN]"));
                assert!(value.as_str().contains("[REDACTED:XAI_API_KEY]"));
            },
        );
    }
}
