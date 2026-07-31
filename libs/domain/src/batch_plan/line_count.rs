//! Per-scope source-line quantities: the count itself, the ceiling it is
//! compared against, and a measured diff figure.

use crate::review_v2::ScopeName;

// ── LineCount ─────────────────────────────────────────────────────────────────

/// Source-line quantity (IN-02 / AC-02).
///
/// Declared estimates, measured diffs and resolved ceilings are all counted in
/// this unit, so none of them travels as a bare `u32`. Ordering is the
/// comparison the ceiling check consumes; [`LineCount::saturating_add`] is the
/// accumulation used to build a per-scope total. Internally, totals retain one
/// wider arithmetic range so an overflow past the largest declarable figure
/// remains above every configured ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineCount(u64);

impl LineCount {
    /// Wraps `value` as a line quantity. Every `u32` is a valid count, so the
    /// constructor is total.
    pub fn new(value: u32) -> LineCount {
        LineCount(u64::from(value))
    }

    /// Returns the declarable quantity, saturating an internally derived total
    /// at `u32::MAX` because the public contract represents figures as `u32`.
    pub fn value(&self) -> u32 {
        u32::try_from(self.0).unwrap_or(u32::MAX)
    }

    /// Adds `other`, saturating only at the wider internal maximum instead of
    /// wrapping or collapsing an over-limit total to `u32::MAX`.
    ///
    /// A batch total is a sum of declared figures; saturating keeps an absurd
    /// declaration reported as an enormous total rather than a small one; the
    /// retained magnitude lets a `u32::MAX` ceiling reject a larger total.
    pub fn saturating_add(&self, other: &LineCount) -> LineCount {
        LineCount(self.0.saturating_add(other.0))
    }
}

// ── ScopeCeiling ──────────────────────────────────────────────────────────────

/// Resolved per-scope diff ceiling (IN-06 / AC-08).
///
/// The scope configuration reports a ceiling as an optional number; this enum
/// names the two resulting states, so "no ceiling configured for this scope" is
/// a state rather than a missing number every caller has to re-interpret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeCeiling {
    /// No ceiling is configured for the scope; every total is admitted.
    Unconstrained,
    /// The scope admits totals up to and including this quantity.
    Limited(LineCount),
}

impl ScopeCeiling {
    /// Resolves the scope configuration's optional ceiling into a named state.
    ///
    /// This is the single conversion point from the configuration boundary into
    /// [`LineCount`].
    pub fn resolve(configured: Option<u32>) -> ScopeCeiling {
        match configured {
            Some(limit) => ScopeCeiling::Limited(LineCount::new(limit)),
            None => ScopeCeiling::Unconstrained,
        }
    }

    /// Reports whether `total` stays within the ceiling.
    ///
    /// An unconstrained scope admits every total; a limited scope admits totals
    /// up to and including its limit.
    pub fn admits(&self, total: &LineCount) -> bool {
        match self {
            ScopeCeiling::Unconstrained => true,
            ScopeCeiling::Limited(limit) => total <= limit,
        }
    }

    /// Returns the resolved limit, or `None` when the scope is unconstrained.
    pub fn limit(&self) -> Option<LineCount> {
        match self {
            ScopeCeiling::Unconstrained => None,
            ScopeCeiling::Limited(limit) => Some(*limit),
        }
    }
}

// ── MeasuredScopeDiff ─────────────────────────────────────────────────────────

/// Measured diff size for one review scope (IN-10).
///
/// Carries only the resulting figure: how the lines are counted and which base
/// they are measured from belong to the measuring adapter, not to this value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredScopeDiff {
    scope: ScopeName,
    lines: LineCount,
}

impl MeasuredScopeDiff {
    /// Pairs a measured quantity with the scope it was measured for.
    pub fn new(scope: ScopeName, lines: LineCount) -> MeasuredScopeDiff {
        MeasuredScopeDiff { scope, lines }
    }

    /// Returns the scope the figure was measured for.
    pub fn scope(&self) -> &ScopeName {
        &self.scope
    }

    /// Returns the measured quantity.
    pub fn lines(&self) -> LineCount {
        self.lines
    }
}
