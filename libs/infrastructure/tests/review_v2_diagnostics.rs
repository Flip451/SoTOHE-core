//! Boundary regressions for typed reviewer-process failures.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use usecase::capability_exec::ProviderName;
use usecase::program_runner::ProgramExitCode;
use usecase::review_v2::{
    ReviewCycleError, ReviewerDiagnostic, ReviewerError, ReviewerProcessDiagnostic,
};

#[test]
fn typed_reviewer_process_failure_preserves_provider_exit_and_safe_diagnostic() {
    let diagnostic = ReviewerDiagnostic::try_new("redacted provider failure".to_owned())
        .expect("fixture diagnostic is bounded");
    let reviewer_error = ReviewerError::ProcessFailed {
        provider: ProviderName::try_new("codex").expect("fixture provider is valid"),
        exit_code: Some(ProgramExitCode::new(17)),
        diagnostic: ReviewerProcessDiagnostic::Available(diagnostic),
    };

    let cycle_error: ReviewCycleError = reviewer_error.into();
    let rendered = cycle_error.to_string();

    assert!(rendered.contains("provider=codex"));
    assert!(rendered.contains("exit_code=17"));
    assert!(rendered.contains("redacted provider failure"));
}

#[test]
fn unavailable_reviewer_diagnostic_is_fixed_and_does_not_expose_process_text() {
    let error = ReviewerError::ProcessFailed {
        provider: ProviderName::try_new("claude").expect("fixture provider is valid"),
        exit_code: None,
        diagnostic: ReviewerProcessDiagnostic::Unavailable,
    };

    assert_eq!(
        error.to_string(),
        "reviewer process failed: provider=claude, exit_code=unavailable, diagnostic=diagnostic_unavailable"
    );
}
