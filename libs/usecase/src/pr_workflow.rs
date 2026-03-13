//! Pure workflow rules for pull-request status evaluation and merge polling.

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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::{
        CheckSummary, PrCheckStatus, PrCheckView, WaitDecision, decide_wait_action,
        summarize_checks,
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
}
