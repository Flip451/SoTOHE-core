//! One task's declared line estimates and its decomposition state.

use crate::review_v2::ScopeName;
use crate::{NonEmptyString, TaskId};

use super::error::BatchPlanValidationError;
use super::line_count::LineCount;

// ── IndivisibilityJustification ───────────────────────────────────────────────

/// The planner's stated reason why a task cannot be split further
/// (IN-04 / AC-03).
///
/// Non-emptiness is the constraint the plan relies on, so the reason is carried
/// as a validated value rather than as free text. Surrounding whitespace is
/// trimmed before the non-empty check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndivisibilityJustification(NonEmptyString);

impl IndivisibilityJustification {
    /// Validates `text` as a stated indivisibility reason.
    ///
    /// # Errors
    ///
    /// Returns [`BatchPlanValidationError::EmptyJustification`] when `text` is
    /// empty or whitespace-only.
    pub fn try_new(text: &str) -> Result<IndivisibilityJustification, BatchPlanValidationError> {
        NonEmptyString::try_new(text)
            .map(IndivisibilityJustification)
            .map_err(|_| BatchPlanValidationError::EmptyJustification)
    }

    /// Returns the stated reason.
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

// ── TaskDecomposition ─────────────────────────────────────────────────────────

/// Whether a task can be split further, and why not when it cannot
/// (IN-04 / AC-03).
///
/// A decomposable task cannot carry a justification and an indivisible one
/// cannot lack it, so the machine-readable distinction the plan promises holds
/// by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskDecomposition {
    /// The task can be split into smaller tasks.
    Decomposable,
    /// The task cannot be split, for the stated reason.
    Indivisible(IndivisibilityJustification),
}

impl TaskDecomposition {
    /// Returns the stated reason, or `None` for a decomposable task.
    pub fn justification(&self) -> Option<&IndivisibilityJustification> {
        match self {
            TaskDecomposition::Decomposable => None,
            TaskDecomposition::Indivisible(justification) => Some(justification),
        }
    }

    /// Reports whether the task was declared indivisible.
    pub fn is_indivisible(&self) -> bool {
        matches!(self, TaskDecomposition::Indivisible(_))
    }
}

// ── ScopeLineEstimate ─────────────────────────────────────────────────────────

/// One task's declared estimate for one review scope (IN-02 / AC-02).
///
/// Production and test lines are declared separately — the test figure is the
/// sum of the task's test code, obligation-derived tests included — while
/// [`ScopeLineEstimate::total`] is the only quantity the ceiling comparison
/// consumes, so the split stays informational.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeLineEstimate {
    scope: ScopeName,
    production_lines: LineCount,
    test_lines: LineCount,
}

impl ScopeLineEstimate {
    /// Declares the estimate a task expects to add to `scope`.
    pub fn new(
        scope: ScopeName,
        production_lines: LineCount,
        test_lines: LineCount,
    ) -> ScopeLineEstimate {
        ScopeLineEstimate { scope, production_lines, test_lines }
    }

    /// Returns the scope this estimate is declared for.
    pub fn scope(&self) -> &ScopeName {
        &self.scope
    }

    /// Returns the declared production-code figure.
    pub fn production_lines(&self) -> LineCount {
        self.production_lines
    }

    /// Returns the declared test-code figure.
    pub fn test_lines(&self) -> LineCount {
        self.test_lines
    }

    /// Returns the estimate's total: production plus test lines.
    pub fn total(&self) -> LineCount {
        self.production_lines.saturating_add(&self.test_lines)
    }
}

// ── TaskEstimate ──────────────────────────────────────────────────────────────

/// One task's per-scope estimates plus its decomposition state
/// (IN-02 / IN-04 / AC-02 / AC-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskEstimate {
    task_id: TaskId,
    scope_estimates: Vec<ScopeLineEstimate>,
    decomposition: TaskDecomposition,
}

impl TaskEstimate {
    /// Declares the estimates and decomposition state of one task.
    ///
    /// # Errors
    ///
    /// Returns [`BatchPlanValidationError::DuplicateScopeEstimate`] when
    /// `scope_estimates` declares two competing figures for one scope.
    pub fn new(
        task_id: TaskId,
        scope_estimates: Vec<ScopeLineEstimate>,
        decomposition: TaskDecomposition,
    ) -> Result<TaskEstimate, BatchPlanValidationError> {
        let mut seen: Vec<&ScopeName> = Vec::new();
        for estimate in &scope_estimates {
            if seen.contains(&estimate.scope()) {
                return Err(BatchPlanValidationError::DuplicateScopeEstimate {
                    task_id,
                    scope: estimate.scope().clone(),
                });
            }
            seen.push(estimate.scope());
        }
        Ok(TaskEstimate { task_id, scope_estimates, decomposition })
    }

    /// Returns the estimated task.
    pub fn task_id(&self) -> &TaskId {
        &self.task_id
    }

    /// Returns the declared estimates, in declaration order.
    pub fn scope_estimates(&self) -> &[ScopeLineEstimate] {
        &self.scope_estimates
    }

    /// Returns the task's decomposition state.
    pub fn decomposition(&self) -> &TaskDecomposition {
        &self.decomposition
    }

    /// Returns the estimate declared for `scope`, or `None` when the task
    /// declares none — an untouched scope.
    pub fn estimate_for(&self, scope: &ScopeName) -> Option<&ScopeLineEstimate> {
        self.scope_estimates.iter().find(|estimate| estimate.scope() == scope)
    }
}
