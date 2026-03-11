//! Guard verdict types for shell command checking.

use std::fmt;

/// The decision of a guard check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The command is allowed to proceed.
    Allow,
    /// The command is blocked.
    Block,
}

impl fmt::Display for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Decision::Allow => write!(f, "allow"),
            Decision::Block => write!(f, "block"),
        }
    }
}

/// The result of checking a shell command against the guard policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardVerdict {
    /// Whether the command is allowed or blocked.
    pub decision: Decision,
    /// Human-readable reason for the decision.
    pub reason: String,
}

impl GuardVerdict {
    /// Creates an allow verdict.
    pub fn allow() -> Self {
        Self { decision: Decision::Allow, reason: String::new() }
    }

    /// Creates a block verdict with the given reason.
    pub fn block(reason: impl Into<String>) -> Self {
        Self { decision: Decision::Block, reason: reason.into() }
    }

    /// Returns `true` if the command is blocked.
    pub fn is_blocked(&self) -> bool {
        self.decision == Decision::Block
    }
}

/// Errors that can occur during shell command parsing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// Nesting depth exceeded the maximum allowed.
    #[error("nesting depth exceeded maximum of {max}")]
    NestingDepthExceeded {
        /// The maximum nesting depth that was exceeded.
        max: usize,
    },
    /// An unmatched quote was found in the command.
    #[error("unmatched quote in command")]
    UnmatchedQuote,
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_verdict_is_not_blocked() {
        let v = GuardVerdict::allow();
        assert!(!v.is_blocked());
        assert_eq!(v.decision, Decision::Allow);
    }

    #[test]
    fn test_block_verdict_is_blocked() {
        let v = GuardVerdict::block("git add is not allowed");
        assert!(v.is_blocked());
        assert_eq!(v.reason, "git add is not allowed");
    }

    #[test]
    fn test_decision_display() {
        assert_eq!(Decision::Allow.to_string(), "allow");
        assert_eq!(Decision::Block.to_string(), "block");
    }
}
