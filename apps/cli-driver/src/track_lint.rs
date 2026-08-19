//! Primary-adapter rendering for single-layer catalogue linting.

use usecase::track_lifecycle::tddd::catalogue_lint_active::{
    TrackCatalogueLintActiveError, TrackCatalogueLintActiveResult,
};
use usecase::track_lifecycle::tddd::lint::{TrackLintError, TrackLintResult};

use crate::render::CommandOutcome;

pub(crate) fn render_lint_result(result: TrackLintResult) -> CommandOutcome {
    let mut stdout_lines = Vec::new();
    for violation in &result.violations {
        stdout_lines.push(format!(
            "{} on {}: {}",
            violation.rule_kind(),
            violation.entry_name(),
            violation.message(),
        ));
    }
    let count = result.violations.len();
    let stderr_msg = format!("Found {count} violation(s)");
    if count > 0 {
        CommandOutcome {
            stdout: Some(stdout_lines.join("\n")),
            stderr: Some(stderr_msg),
            exit_code: 1,
        }
    } else {
        CommandOutcome { stdout: None, stderr: Some(stderr_msg), exit_code: 0 }
    }
}

pub(crate) fn lint_error_to_outcome(error: TrackLintError) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}

pub(crate) fn render_catalogue_lint_active_result(
    result: TrackCatalogueLintActiveResult,
) -> CommandOutcome {
    match result {
        TrackCatalogueLintActiveResult::Skipped { layer, path } => CommandOutcome {
            stdout: None,
            stderr: Some(format!(
                "catalogue-lint skipped: layer '{}' has no catalogue file yet at {} (tolerated before/during Phase 2 type-design)",
                layer,
                path.as_path().display(),
            )),
            exit_code: 0,
        },
        TrackCatalogueLintActiveResult::Checked { layers } => {
            let mut stdout_lines = Vec::new();
            let mut total_violations = 0usize;
            for layer in &layers {
                for violation in &layer.violations {
                    stdout_lines.push(format!(
                        "[{}] {} on {}: {}",
                        layer.layer,
                        violation.rule_kind(),
                        violation.entry_name(),
                        violation.message(),
                    ));
                }
                total_violations = total_violations.saturating_add(layer.violations.len());
            }
            if total_violations == 0 {
                CommandOutcome {
                    stdout: None,
                    stderr: Some(format!("Found 0 violation(s) across {} layer(s)", layers.len())),
                    exit_code: 0,
                }
            } else {
                CommandOutcome {
                    stdout: Some(stdout_lines.join("\n")),
                    stderr: Some(format!(
                        "Found {total_violations} violation(s) across {} layer(s)",
                        layers.len()
                    )),
                    exit_code: 1,
                }
            }
        }
    }
}

pub(crate) fn catalogue_lint_active_error_to_outcome(
    error: TrackCatalogueLintActiveError,
) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}
