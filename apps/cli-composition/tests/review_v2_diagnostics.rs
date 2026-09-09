//! CLI-composition rendering regressions for typed reviewer diagnostics.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use cli_composition::CompositionError;
use usecase::capability_exec::ProviderName;
use usecase::program_runner::ProgramExitCode;
use usecase::review_v2::{
    ReviewerDiagnostic, ReviewerError, ReviewerExecutionProvenance, ReviewerProcessDiagnostic,
};

#[test]
fn composition_error_keeps_provider_exit_and_bounded_diagnostic_for_cli() {
    let diagnostic = ReviewerDiagnostic::try_new("safe reviewer detail".to_owned())
        .expect("fixture diagnostic is bounded");
    let reviewer_error = ReviewerError::ProcessFailed {
        provider: ProviderName::try_new("grok").expect("fixture provider is valid"),
        exit_code: Some(ProgramExitCode::new(9)),
        diagnostic: ReviewerProcessDiagnostic::Available(diagnostic),
    };
    let composition_error = CompositionError::Infrastructure(reviewer_error.to_string());

    let rendered = composition_error.to_string();
    assert!(rendered.contains("provider=grok"));
    assert!(rendered.contains("exit_code=9"));
    assert!(rendered.contains("safe reviewer detail"));
}

#[test]
fn composition_error_uses_fixed_unavailable_classification() {
    let reviewer_error = ReviewerError::Unexpected {
        provenance: ReviewerExecutionProvenance::PreSpawn,
        diagnostic: ReviewerProcessDiagnostic::Unavailable,
    };
    let composition_error = CompositionError::Usecase(reviewer_error.to_string());

    assert_eq!(composition_error.to_string(), "unexpected reviewer error: diagnostic_unavailable");
}
