//! Hook verdict type using the shared `Decision` enum.

use crate::Decision;

/// The result of processing a hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookVerdict {
    /// Whether the operation is allowed or blocked.
    pub decision: Decision,
    /// Human-readable reason for the decision.
    pub reason: Option<String>,
    /// Additional context for the hook output (e.g., hookSpecificOutput fields).
    pub additional_context: Option<String>,
}

impl HookVerdict {
    /// Creates an allow verdict with no reason.
    #[must_use]
    pub fn allow() -> Self {
        Self { decision: Decision::Allow, reason: None, additional_context: None }
    }

    /// Creates a block verdict with the given reason.
    #[must_use]
    pub fn block(reason: impl Into<String>) -> Self {
        Self { decision: Decision::Block, reason: Some(reason.into()), additional_context: None }
    }

    /// Returns `true` if the hook blocks the operation.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        self.decision == Decision::Block
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_verdict_allow_is_not_blocked() {
        let v = HookVerdict::allow();
        assert!(!v.is_blocked());
        assert_eq!(v.decision, Decision::Allow);
        assert!(v.reason.is_none());
    }

    #[test]
    fn test_hook_verdict_block_is_blocked() {
        let v = HookVerdict::block("forbidden");
        assert!(v.is_blocked());
        assert_eq!(v.reason.as_deref(), Some("forbidden"));
    }
}
