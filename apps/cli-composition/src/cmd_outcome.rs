//! Shared render helpers for CLI command outcomes.
//!
//! Private helpers used by multiple command families (`signal`, `verify`, …).
//! Nothing in this module is exported from the crate.

use crate::CommandOutcome;

/// Render a `VerifyOutcome` into a `CommandOutcome`.
///
/// Formats findings with a header/footer label and sets `exit_code = 1` when
/// any finding has error severity.
pub(crate) fn render_outcome(
    label: &str,
    outcome: &infrastructure::verify::VerifyOutcome,
) -> CommandOutcome {
    let mut lines = vec![format!("--- {label} ---")];
    if outcome.findings().is_empty() {
        lines.push("[OK] All checks passed.".to_owned());
        lines.push(format!("--- {label} PASSED ---"));
        CommandOutcome::success(Some(lines.join("\n")))
    } else {
        for finding in outcome.findings() {
            lines.push(finding.to_string());
        }
        if outcome.has_errors() {
            lines.push(format!("--- {label} FAILED ---"));
            CommandOutcome { stdout: Some(lines.join("\n")), stderr: None, exit_code: 1 }
        } else {
            lines.push(format!("--- {label} PASSED ---"));
            CommandOutcome::success(Some(lines.join("\n")))
        }
    }
}
