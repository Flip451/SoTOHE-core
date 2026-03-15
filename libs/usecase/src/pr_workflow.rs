//! Pure workflow rules for pull-request status evaluation and merge polling.

use thiserror::Error;

/// Errors returned by PR workflow functions.
#[derive(Debug, Error)]
pub enum PrWorkflowError {
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrCheckView {
    pub name: String,
    pub status: PrCheckStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrCheckStatus {
    Passed,
    Failed,
    Pending,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CheckSummary {
    AllPassed,
    Failed(Vec<String>),
    Pending(Vec<String>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum WaitDecision {
    MergeNow,
    FailChecks(Vec<String>),
    Timeout(Vec<String>),
    Wait { pending: Vec<String>, delay_seconds: u64 },
}

pub fn summarize_checks(checks: &[PrCheckView]) -> CheckSummary {
    if checks.is_empty() {
        return CheckSummary::Pending(vec!["(no checks found)".to_owned()]);
    }

    let mut pending = Vec::new();
    let mut failed = Vec::new();
    for check in checks {
        let name = if check.name.is_empty() { "unknown".to_owned() } else { check.name.clone() };
        match check.status {
            PrCheckStatus::Passed => continue,
            PrCheckStatus::Failed => failed.push(name),
            PrCheckStatus::Pending => pending.push(name),
        }
    }

    if !failed.is_empty() {
        return CheckSummary::Failed(failed);
    }
    if !pending.is_empty() {
        return CheckSummary::Pending(pending);
    }
    CheckSummary::AllPassed
}

pub fn decide_wait_action(
    summary: CheckSummary,
    elapsed_seconds: u64,
    timeout_seconds: u64,
    interval_seconds: u64,
) -> WaitDecision {
    match summary {
        CheckSummary::AllPassed => WaitDecision::MergeNow,
        CheckSummary::Failed(names) => WaitDecision::FailChecks(names),
        CheckSummary::Pending(names) => {
            if elapsed_seconds >= timeout_seconds {
                WaitDecision::Timeout(names)
            } else {
                let remaining = timeout_seconds.saturating_sub(elapsed_seconds);
                WaitDecision::Wait {
                    pending: names,
                    delay_seconds: interval_seconds.min(remaining),
                }
            }
        }
    }
}

/// Context resolved from the current git branch for PR operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrBranchContext {
    /// The git branch name (e.g., `track/my-feature` or `plan/my-feature`).
    pub branch: String,
    /// The track ID extracted from the branch name.
    pub track_id: String,
    /// Whether this is a planning-only branch (`plan/<id>`).
    pub is_plan_branch: bool,
}

/// Resolve the current branch for PR operations.
///
/// # Rules
///
/// - `track/<id>` branches: auto-resolve `track_id` from branch name.
///   `explicit_track_id` is ignored.
/// - `plan/<id>` branches: require `explicit_track_id` (fail-closed).
///   If missing, return error.
/// - Other branches: return error (PR operations not supported).
///
/// # Errors
///
/// Returns an error description if the branch is not a `track/` or `plan/`
/// branch, or if a `plan/` branch is used without an explicit track ID.
pub fn resolve_pr_branch(
    branch: &str,
    explicit_track_id: Option<&str>,
) -> Result<PrBranchContext, PrWorkflowError> {
    if let Some(id) = branch.strip_prefix("track/") {
        if id.is_empty() {
            return Err(PrWorkflowError::Message("track branch name is empty".to_owned()));
        }
        return Ok(PrBranchContext {
            branch: branch.to_owned(),
            track_id: id.to_owned(),
            is_plan_branch: false,
        });
    }

    if let Some(id) = branch.strip_prefix("plan/") {
        if id.is_empty() {
            return Err(PrWorkflowError::Message("plan branch name is empty".to_owned()));
        }
        let track_id = match explicit_track_id {
            Some(tid) if !tid.is_empty() => {
                if tid != id {
                    return Err(PrWorkflowError::Message(format!(
                        "--track-id '{tid}' does not match plan branch suffix '{id}'. \
                         The explicit selector must match the branch name."
                    )));
                }
                tid.to_owned()
            }
            _ => {
                return Err(PrWorkflowError::Message(format!(
                    "plan/{id} branch requires explicit --track-id argument. \
                     Auto-detection is not supported for non-track branches."
                )));
            }
        };
        return Ok(PrBranchContext { branch: branch.to_owned(), track_id, is_plan_branch: true });
    }

    Err(PrWorkflowError::Message(format!(
        "Not on a track or plan branch (current: {branch}). \
         PR operations require a track/<id> or plan/<id> branch."
    )))
}

/// Generate a PR title based on the branch context.
pub fn pr_title(ctx: &PrBranchContext) -> String {
    if ctx.is_plan_branch {
        format!("Plan: {}", ctx.track_id)
    } else {
        format!("track: {}", ctx.track_id)
    }
}

/// Generate a default PR body based on the branch context.
pub fn pr_body(ctx: &PrBranchContext) -> String {
    if ctx.is_plan_branch {
        format!(
            "Planning artifacts for `{}`.\n\n\
             After merge, run `/track:activate {}` to create the implementation branch.",
            ctx.track_id, ctx.track_id
        )
    } else {
        format!("Track implementation for `{}`.", ctx.track_id)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::{
        CheckSummary, PrBranchContext, PrCheckStatus, PrCheckView, WaitDecision,
        decide_wait_action, pr_body, pr_title, resolve_pr_branch, summarize_checks,
    };

    #[test]
    fn summarize_checks_reports_success_when_all_checks_pass() {
        let checks = vec![
            PrCheckView { name: "ci".to_owned(), status: PrCheckStatus::Passed },
            PrCheckView { name: "lint".to_owned(), status: PrCheckStatus::Passed },
        ];

        assert_eq!(summarize_checks(&checks), CheckSummary::AllPassed);
    }

    #[test]
    fn summarize_checks_reports_failed_checks() {
        let checks = vec![
            PrCheckView { name: "ci".to_owned(), status: PrCheckStatus::Passed },
            PrCheckView { name: "review".to_owned(), status: PrCheckStatus::Failed },
        ];

        assert_eq!(summarize_checks(&checks), CheckSummary::Failed(vec!["review".to_owned()]));
    }

    #[test]
    fn summarize_checks_reports_pending_checks_and_empty_sets() {
        let checks = vec![
            PrCheckView { name: "ci".to_owned(), status: PrCheckStatus::Pending },
            PrCheckView { name: String::new(), status: PrCheckStatus::Pending },
        ];

        assert_eq!(
            summarize_checks(&checks),
            CheckSummary::Pending(vec!["ci".to_owned(), "unknown".to_owned()])
        );
        assert_eq!(
            summarize_checks(&[]),
            CheckSummary::Pending(vec!["(no checks found)".to_owned()])
        );
    }

    #[test]
    fn summarize_checks_uses_normalized_status() {
        let checks = vec![
            PrCheckView { name: "ci".to_owned(), status: PrCheckStatus::Passed },
            PrCheckView { name: "lint".to_owned(), status: PrCheckStatus::Pending },
            PrCheckView { name: "review".to_owned(), status: PrCheckStatus::Failed },
        ];

        assert_eq!(summarize_checks(&checks), CheckSummary::Failed(vec!["review".to_owned()]));
    }

    #[test]
    fn decide_wait_action_returns_timeout_when_pending_exceeds_deadline() {
        let decision =
            decide_wait_action(CheckSummary::Pending(vec!["ci".to_owned()]), 600, 600, 15);

        assert_eq!(decision, WaitDecision::Timeout(vec!["ci".to_owned()]));
    }

    #[test]
    fn decide_wait_action_returns_delay_capped_by_remaining_time() {
        let decision =
            decide_wait_action(CheckSummary::Pending(vec!["ci".to_owned()]), 595, 600, 15);

        assert_eq!(
            decision,
            WaitDecision::Wait { pending: vec!["ci".to_owned()], delay_seconds: 5 }
        );
    }

    // --- resolve_pr_branch tests ---

    #[test]
    fn resolve_pr_branch_auto_resolves_track_branch() {
        let ctx = resolve_pr_branch("track/my-feature", None).unwrap();
        assert_eq!(
            ctx,
            PrBranchContext {
                branch: "track/my-feature".to_owned(),
                track_id: "my-feature".to_owned(),
                is_plan_branch: false,
            }
        );
    }

    #[test]
    fn resolve_pr_branch_ignores_explicit_id_on_track_branch() {
        let ctx = resolve_pr_branch("track/my-feature", Some("other-id")).unwrap();
        assert_eq!(ctx.track_id, "my-feature");
    }

    #[test]
    fn resolve_pr_branch_requires_explicit_id_on_plan_branch() {
        let err = resolve_pr_branch("plan/my-plan", None).unwrap_err();
        assert!(err.to_string().contains("--track-id"), "expected --track-id hint, got: {err}");
    }

    #[test]
    fn resolve_pr_branch_accepts_plan_branch_with_explicit_id() {
        let ctx = resolve_pr_branch("plan/my-plan", Some("my-plan")).unwrap();
        assert_eq!(
            ctx,
            PrBranchContext {
                branch: "plan/my-plan".to_owned(),
                track_id: "my-plan".to_owned(),
                is_plan_branch: true,
            }
        );
    }

    #[test]
    fn resolve_pr_branch_rejects_main_branch() {
        let err = resolve_pr_branch("main", None).unwrap_err();
        assert!(err.to_string().contains("Not on a track or plan branch"), "got: {err}");
    }

    #[test]
    fn resolve_pr_branch_rejects_empty_track_id() {
        let err = resolve_pr_branch("track/", None).unwrap_err();
        assert!(err.to_string().contains("empty"), "got: {err}");
    }

    #[test]
    fn resolve_pr_branch_rejects_mismatched_explicit_id_on_plan() {
        let err = resolve_pr_branch("plan/my-plan", Some("other-id")).unwrap_err();
        assert!(err.to_string().contains("does not match"), "got: {err}");
    }

    #[test]
    fn resolve_pr_branch_rejects_empty_explicit_id_on_plan() {
        let err = resolve_pr_branch("plan/my-plan", Some("")).unwrap_err();
        assert!(err.to_string().contains("--track-id"), "got: {err}");
    }

    #[test]
    fn pr_title_uses_plan_prefix_for_plan_branch() {
        let ctx = PrBranchContext {
            branch: "plan/my-plan".to_owned(),
            track_id: "my-plan".to_owned(),
            is_plan_branch: true,
        };
        assert_eq!(pr_title(&ctx), "Plan: my-plan");
    }

    #[test]
    fn pr_title_uses_track_prefix_for_track_branch() {
        let ctx = PrBranchContext {
            branch: "track/my-feature".to_owned(),
            track_id: "my-feature".to_owned(),
            is_plan_branch: false,
        };
        assert_eq!(pr_title(&ctx), "track: my-feature");
    }

    #[test]
    fn pr_body_includes_activate_hint_for_plan_branch() {
        let ctx = PrBranchContext {
            branch: "plan/my-plan".to_owned(),
            track_id: "my-plan".to_owned(),
            is_plan_branch: true,
        };
        assert!(pr_body(&ctx).contains("/track:activate my-plan"));
    }
}
