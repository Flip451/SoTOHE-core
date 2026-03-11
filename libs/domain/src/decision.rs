//! Shared binary policy outcome used by guard and hook subdomains (SRP).

use std::fmt;

/// A binary policy outcome: allow or block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// The operation is allowed to proceed.
    Allow,
    /// The operation is blocked.
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

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_display_allow() {
        assert_eq!(Decision::Allow.to_string(), "allow");
    }

    #[test]
    fn test_decision_display_block() {
        assert_eq!(Decision::Block.to_string(), "block");
    }

    #[test]
    fn test_decision_equality() {
        assert_eq!(Decision::Allow, Decision::Allow);
        assert_eq!(Decision::Block, Decision::Block);
        assert_ne!(Decision::Allow, Decision::Block);
    }

    #[test]
    fn test_decision_copy() {
        let d = Decision::Allow;
        let d2 = d;
        assert_eq!(d, d2);
    }
}
