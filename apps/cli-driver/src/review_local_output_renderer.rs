//! Rendering for the local review runner's presentation-neutral output.

use usecase::review_v2::ReviewRunLocalOutput;

use crate::render::CommandOutcome;

pub(super) fn review_run_local_output_to_outcome(out: ReviewRunLocalOutput) -> CommandOutcome {
    let stderr = (!out.diagnostics.is_empty()).then(|| {
        out.diagnostics
            .iter()
            .map(usecase::git_workflow::DiagnosticText::as_str)
            .collect::<Vec<_>>()
            .join("\n")
    });
    CommandOutcome { stdout: out.summary, stderr, exit_code: out.exit_code }
}
