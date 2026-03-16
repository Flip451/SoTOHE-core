//! Auto mode phase state machine for `/track:auto`.
//!
//! Defines the 6-phase cycle that `/track:auto` drives per commit unit:
//! Plan → PlanReview → TypeDesign → TypeReview → Implement → CodeReview → Committed.
//!
//! Each phase can advance forward, roll back to an earlier phase based on
//! reviewer findings severity, or escalate to human decision.

use std::fmt;

use thiserror::Error;

/// The six execution phases plus terminal/escalation states.
///
/// The cycle for one commit unit:
/// ```text
/// Plan → PlanReview → TypeDesign → TypeReview → Implement → CodeReview → Committed
///   ↑         ↓            ↑            ↓           ↑           ↓
///   └── rollback ──────────┴── rollback ────────────┴── rollback
///                                                         ↓
///                                                    Escalated
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutoPhase {
    /// Task-level implementation planning.
    Plan,
    /// Review of the implementation plan.
    PlanReview,
    /// Type definitions: trait, struct, enum signatures (no method bodies) in `.rs` files.
    TypeDesign,
    /// Review of type definitions for API ergonomics and correctness.
    TypeReview,
    /// TDD implementation: Red → Green → Refactor.
    Implement,
    /// Code review for correctness, performance, idiomatic Rust.
    CodeReview,
    /// Human intervention required — state persisted to `auto-state.json`.
    Escalated,
    /// All reviews passed, commit completed for this cycle.
    Committed,
}

impl fmt::Display for AutoPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Plan => "plan",
            Self::PlanReview => "plan_review",
            Self::TypeDesign => "type_design",
            Self::TypeReview => "type_review",
            Self::Implement => "implement",
            Self::CodeReview => "code_review",
            Self::Escalated => "escalated",
            Self::Committed => "committed",
        };
        f.write_str(label)
    }
}

/// Rollback target determined by reviewer findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollbackTarget {
    /// Design-level issue → roll back to Plan.
    Plan,
    /// Type signature change needed → roll back to TypeDesign.
    TypeDesign,
    /// Implementation fix only → roll back to Implement.
    Implement,
}

/// Commands that drive the auto-phase state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoPhaseTransition {
    /// Advance to the next phase in the cycle.
    Advance,
    /// Roll back to an earlier phase based on reviewer findings.
    Rollback(RollbackTarget),
    /// Escalate to human — requires a reason.
    Escalate { reason: String },
    /// Resume from escalation with a human decision.
    Resume { decision: String },
}

/// Severity of a reviewer finding, used to determine rollback target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FindingSeverity {
    /// Implementation fix only (e.g., logic error, missing test).
    P1,
    /// Type signature change needed (e.g., wrong trait bound, missing field).
    P2,
    /// Design-level issue (e.g., wrong abstraction, missing module).
    P3,
}

/// Errors from auto-phase state transitions.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AutoPhaseError {
    #[error("invalid auto-phase transition: {from} cannot {action}")]
    InvalidTransition { from: String, action: String },

    #[error("cannot resume: phase is '{phase}', not escalated")]
    NotEscalated { phase: String },

    #[error("rollback from '{from}' to '{to}' is not allowed")]
    InvalidRollback { from: String, to: String },
}

/// Determines the rollback target based on the current review phase and finding severity.
///
/// Phase-specific rules (from the auto-mode design):
/// - PlanReview: P2+ → Plan, P1 → Plan (fix in place, re-review)
/// - TypeReview: P3 → Plan, P2 → TypeDesign, P1 → TypeDesign (fix in place, re-review)
/// - CodeReview: P3 → Plan, P2 → TypeDesign, P1 → Implement
///
/// # Errors
///
/// Returns `AutoPhaseError::InvalidRollback` if the current phase is not a review phase.
pub fn rollback_target(
    current_phase: AutoPhase,
    severity: FindingSeverity,
) -> Result<RollbackTarget, AutoPhaseError> {
    match current_phase {
        AutoPhase::PlanReview => {
            // All severities roll back to Plan (the preceding authoring phase)
            Ok(RollbackTarget::Plan)
        }
        AutoPhase::TypeReview => match severity {
            FindingSeverity::P3 => Ok(RollbackTarget::Plan),
            FindingSeverity::P2 | FindingSeverity::P1 => Ok(RollbackTarget::TypeDesign),
        },
        AutoPhase::CodeReview => match severity {
            FindingSeverity::P3 => Ok(RollbackTarget::Plan),
            FindingSeverity::P2 => Ok(RollbackTarget::TypeDesign),
            FindingSeverity::P1 => Ok(RollbackTarget::Implement),
        },
        _ => Err(AutoPhaseError::InvalidRollback {
            from: current_phase.to_string(),
            to: "rollback (not a review phase)".to_string(),
        }),
    }
}

/// The ordered list of the six core phases (excluding Escalated and Committed).
pub const PHASE_ORDER: [AutoPhase; 6] = [
    AutoPhase::Plan,
    AutoPhase::PlanReview,
    AutoPhase::TypeDesign,
    AutoPhase::TypeReview,
    AutoPhase::Implement,
    AutoPhase::CodeReview,
];

/// Returns the next phase in the cycle, or `None` for terminal/escalation states.
#[must_use]
pub fn next_phase(current: AutoPhase) -> Option<AutoPhase> {
    match current {
        AutoPhase::Plan => Some(AutoPhase::PlanReview),
        AutoPhase::PlanReview => Some(AutoPhase::TypeDesign),
        AutoPhase::TypeDesign => Some(AutoPhase::TypeReview),
        AutoPhase::TypeReview => Some(AutoPhase::Implement),
        AutoPhase::Implement => Some(AutoPhase::CodeReview),
        AutoPhase::CodeReview => Some(AutoPhase::Committed),
        AutoPhase::Escalated | AutoPhase::Committed => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_display_roundtrip() {
        assert_eq!(AutoPhase::Plan.to_string(), "plan");
        assert_eq!(AutoPhase::PlanReview.to_string(), "plan_review");
        assert_eq!(AutoPhase::TypeDesign.to_string(), "type_design");
        assert_eq!(AutoPhase::TypeReview.to_string(), "type_review");
        assert_eq!(AutoPhase::Implement.to_string(), "implement");
        assert_eq!(AutoPhase::CodeReview.to_string(), "code_review");
        assert_eq!(AutoPhase::Escalated.to_string(), "escalated");
        assert_eq!(AutoPhase::Committed.to_string(), "committed");
    }

    #[test]
    fn test_next_phase_follows_cycle_order() {
        assert_eq!(next_phase(AutoPhase::Plan), Some(AutoPhase::PlanReview));
        assert_eq!(next_phase(AutoPhase::PlanReview), Some(AutoPhase::TypeDesign));
        assert_eq!(next_phase(AutoPhase::TypeDesign), Some(AutoPhase::TypeReview));
        assert_eq!(next_phase(AutoPhase::TypeReview), Some(AutoPhase::Implement));
        assert_eq!(next_phase(AutoPhase::Implement), Some(AutoPhase::CodeReview));
        assert_eq!(next_phase(AutoPhase::CodeReview), Some(AutoPhase::Committed));
    }

    #[test]
    fn test_terminal_phases_have_no_next() {
        assert_eq!(next_phase(AutoPhase::Escalated), None);
        assert_eq!(next_phase(AutoPhase::Committed), None);
    }

    #[test]
    fn test_rollback_target_plan_review_all_severities_go_to_plan() {
        assert_eq!(
            rollback_target(AutoPhase::PlanReview, FindingSeverity::P3).unwrap(),
            RollbackTarget::Plan
        );
        assert_eq!(
            rollback_target(AutoPhase::PlanReview, FindingSeverity::P2).unwrap(),
            RollbackTarget::Plan
        );
        assert_eq!(
            rollback_target(AutoPhase::PlanReview, FindingSeverity::P1).unwrap(),
            RollbackTarget::Plan
        );
    }

    #[test]
    fn test_rollback_target_type_review_phase_specific() {
        assert_eq!(
            rollback_target(AutoPhase::TypeReview, FindingSeverity::P3).unwrap(),
            RollbackTarget::Plan
        );
        assert_eq!(
            rollback_target(AutoPhase::TypeReview, FindingSeverity::P2).unwrap(),
            RollbackTarget::TypeDesign
        );
        assert_eq!(
            rollback_target(AutoPhase::TypeReview, FindingSeverity::P1).unwrap(),
            RollbackTarget::TypeDesign
        );
    }

    #[test]
    fn test_rollback_target_code_review_phase_specific() {
        assert_eq!(
            rollback_target(AutoPhase::CodeReview, FindingSeverity::P3).unwrap(),
            RollbackTarget::Plan
        );
        assert_eq!(
            rollback_target(AutoPhase::CodeReview, FindingSeverity::P2).unwrap(),
            RollbackTarget::TypeDesign
        );
        assert_eq!(
            rollback_target(AutoPhase::CodeReview, FindingSeverity::P1).unwrap(),
            RollbackTarget::Implement
        );
    }

    #[test]
    fn test_rollback_target_non_review_phase_returns_error() {
        assert!(rollback_target(AutoPhase::Plan, FindingSeverity::P1).is_err());
        assert!(rollback_target(AutoPhase::TypeDesign, FindingSeverity::P1).is_err());
        assert!(rollback_target(AutoPhase::Implement, FindingSeverity::P1).is_err());
    }

    #[test]
    fn test_phase_order_has_six_entries() {
        assert_eq!(PHASE_ORDER.len(), 6);
        assert_eq!(PHASE_ORDER[0], AutoPhase::Plan);
        assert_eq!(PHASE_ORDER[5], AutoPhase::CodeReview);
    }

    #[test]
    fn test_auto_phase_error_display() {
        let err = AutoPhaseError::InvalidTransition {
            from: "committed".to_string(),
            action: "advance".to_string(),
        };
        assert_eq!(err.to_string(), "invalid auto-phase transition: committed cannot advance");
    }
}
