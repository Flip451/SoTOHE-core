//! Signal region and evaluation result types for Phase 2 3-way evaluation.
//!
//! ## Types
//!
//! - [`SignalRegion`]: 12 variants encoding the evaluation regions from ADR 3 D3.
//! - [`ThreeWaySignalKind`]: 4-variant signal kind (Skip / Blue / Yellow / Red).
//! - [`ThreeWaySignal`]: per-item signal result (item_name / region / signal).
//! - [`ThreeWayEvaluationReport`]: Phase 2 output containing all non-skip signals.
//!
//! ## Design (ADR 3 D3)
//!
//! Phase 2 evaluates S / D / C in 11 logical rows (the "S ∩ C + Match" row is
//! split into Add and Modify variants to distinguish the two actions), yielding
//! 12 `SignalRegion` variants.
//!
//! Signal table:
//!
//! | Region | Signal | Interpretation |
//! |--------|--------|----------------|
//! | SIntersectC_Match_Add | 🔵 Blue | add achieved |
//! | SIntersectC_Match_Modify | 🔵 Blue | modify achieved |
//! | SIntersectC_Match_Reference | Skip | maintained (suppressed to reduce noise) |
//! | SIntersectC_Mismatch_Reference | 🔴 Red | reference-only contract violated |
//! | SIntersectC_Mismatch_Add | 🟡 Yellow | add in progress |
//! | SIntersectC_Mismatch_Modify | 🟡 Yellow | modify in progress |
//! | SMinusC_Reference | 🔴 Red | reference contract violated — item vanished |
//! | SMinusC_Add | 🟡 Yellow | add not yet implemented |
//! | SMinusC_Modify | 🔴 Red | modify declared but item deleted |
//! | DIntersectC | 🟡 Yellow | delete in progress |
//! | DMinusC | 🔵 Blue | delete achieved |
//! | CMinusSUnionD | 🔴 Red | undeclared implementation |
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free.

// ---------------------------------------------------------------------------
// SignalRegion — 12 evaluation region variants (ADR 3 D3)
// ---------------------------------------------------------------------------

/// Evaluation region for a single item in Phase 2 3-way evaluation.
///
/// 12 variants encode the 11 logical rows of the ADR 3 D3 signal table
/// (the `S ∩ C + Match` row is split into `SIntersectC_Match_Add` and
/// `SIntersectC_Match_Modify` to distinguish the two action contexts).
///
/// Identity judgement uses `short name` for types/traits and `FunctionPath`
/// for functions (ADR 2 D3 / ADR 3 D2).
///
/// ## Naming convention
///
/// Variant names follow the set-notation naming used in the ADR:
/// - `SIntersectC_*` — item present in both S and C
/// - `SMinusC_*` — item present in S but absent from C
/// - `DIntersectC` / `DMinusC` — item in the Delete-set D vs C
/// - `CMinusSUnionD` — item in C that is not in S ∪ D
// Variant names use underscores to preserve the set-notation ADR naming
// (SIntersectC_Match_Add, SMinusC_Add, …).  The clippy `non_camel_case_types`
// lint fires on enum variants containing underscores; we suppress it at the
// enum level so individual variant docs stay clean.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalRegion {
    // --- S ∩ C ---
    /// S ∩ C: structure matches, `action = Add`. Signal: 🔵 Blue (achieved).
    SIntersectC_Match_Add,
    /// S ∩ C: structure matches, `action = Modify`. Signal: 🔵 Blue (achieved).
    SIntersectC_Match_Modify,
    /// S ∩ C: structure matches, `action = Reference`. Signal: Skip (suppressed).
    SIntersectC_Match_Reference,
    /// S ∩ C: structure mismatch, `action = Reference`. Signal: 🔴 Red (contract violated).
    SIntersectC_Mismatch_Reference,
    /// S ∩ C: structure mismatch, `action = Add`. Signal: 🟡 Yellow (add in progress).
    SIntersectC_Mismatch_Add,
    /// S ∩ C: structure mismatch, `action = Modify`. Signal: 🟡 Yellow (modify in progress).
    SIntersectC_Mismatch_Modify,
    // --- S \ C ---
    /// S \ C: `action = Reference`. Signal: 🔴 Red (reference-only item vanished).
    SMinusC_Reference,
    /// S \ C: `action = Add`. Signal: 🟡 Yellow (add not yet implemented).
    SMinusC_Add,
    /// S \ C: `action = Modify`. Signal: 🔴 Red (item deleted without delete declaration).
    SMinusC_Modify,
    // --- D vs C ---
    /// D ∩ C: delete-declared item still present in C. Signal: 🟡 Yellow (delete in progress).
    DIntersectC,
    /// D \ C: delete-declared item absent from C. Signal: 🔵 Blue (delete achieved).
    DMinusC,
    // --- C \ (S ∪ D) ---
    /// C \ (S ∪ D): undeclared implementation found in C. Signal: 🔴 Red (contract violated).
    CMinusSUnionD,
}

// ---------------------------------------------------------------------------
// ThreeWaySignalKind — 4 values
// ---------------------------------------------------------------------------

/// Signal kind for a single item in Phase 2 3-way evaluation.
///
/// Four values: `Skip` / `Blue` / `Yellow` / `Red`.
///
/// `Skip` corresponds to `SIntersectC_Match_Reference` (maintained, suppressed
/// in report output to reduce noise — ADR 3 D3).
/// `Blue` (🔵) = achieved. `Yellow` (🟡) = in progress. `Red` (🔴) = violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreeWaySignalKind {
    /// Suppressed: item is in S ∩ C with matching structure and `action = Reference`.
    ///
    /// Items with this signal are omitted from `ThreeWayEvaluationReport.signals`
    /// to reduce noise.
    Skip,
    /// 🔵 Achieved: declared goal has been fully met.
    Blue,
    /// 🟡 In progress: declared goal is partially met (implementation underway).
    Yellow,
    /// 🔴 Contract violated: structural mismatch, missing item, or undeclared
    /// implementation found.
    Red,
}

impl ThreeWaySignalKind {
    /// Returns `true` for the `Skip` variant (item excluded from the report).
    #[must_use]
    pub fn is_skip(self) -> bool {
        matches!(self, ThreeWaySignalKind::Skip)
    }

    /// Returns `true` for `Blue` (achieved).
    #[must_use]
    pub fn is_blue(self) -> bool {
        matches!(self, ThreeWaySignalKind::Blue)
    }

    /// Returns `true` for `Yellow` (in progress).
    #[must_use]
    pub fn is_yellow(self) -> bool {
        matches!(self, ThreeWaySignalKind::Yellow)
    }

    /// Returns `true` for `Red` (violation).
    #[must_use]
    pub fn is_red(self) -> bool {
        matches!(self, ThreeWaySignalKind::Red)
    }
}

/// Maps a `SignalRegion` to its canonical `ThreeWaySignalKind` per ADR 3 D3.
///
/// Crate-internal helper used by [`ThreeWaySignal::new`].
/// Callers outside this crate should construct [`ThreeWaySignal`] via
/// `ThreeWaySignal::new(item_name, region)` which automatically derives the
/// signal kind.
#[must_use]
pub(crate) fn signal_for_region(region: SignalRegion) -> ThreeWaySignalKind {
    match region {
        SignalRegion::SIntersectC_Match_Add => ThreeWaySignalKind::Blue,
        SignalRegion::SIntersectC_Match_Modify => ThreeWaySignalKind::Blue,
        SignalRegion::SIntersectC_Match_Reference => ThreeWaySignalKind::Skip,
        SignalRegion::SIntersectC_Mismatch_Reference => ThreeWaySignalKind::Red,
        SignalRegion::SIntersectC_Mismatch_Add => ThreeWaySignalKind::Yellow,
        SignalRegion::SIntersectC_Mismatch_Modify => ThreeWaySignalKind::Yellow,
        SignalRegion::SMinusC_Reference => ThreeWaySignalKind::Red,
        SignalRegion::SMinusC_Add => ThreeWaySignalKind::Yellow,
        SignalRegion::SMinusC_Modify => ThreeWaySignalKind::Red,
        SignalRegion::DIntersectC => ThreeWaySignalKind::Yellow,
        SignalRegion::DMinusC => ThreeWaySignalKind::Blue,
        SignalRegion::CMinusSUnionD => ThreeWaySignalKind::Red,
    }
}

// ---------------------------------------------------------------------------
// ThreeWaySignal — per-item evaluation result
// ---------------------------------------------------------------------------

/// Evaluation result for a single item in Phase 2 3-way evaluation.
///
/// Holds the item name (short name for types/traits, `FunctionPath` for functions),
/// the [`SignalRegion`] this item falls into, and the derived [`ThreeWaySignalKind`].
///
/// `ThreeWayEvaluationReport` collects all non-skip signals; items in
/// `SIntersectC_Match_Reference` are omitted from the report (ADR 3 D3).
///
/// ## Invariant
///
/// `signal` is always `signal_for_region(region)` — the only constructor is
/// [`ThreeWaySignal::new`], which derives the signal automatically.  All fields
/// are private so external code cannot construct an inconsistent value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreeWaySignal {
    /// Identity key of the item (short name for types/traits; `FunctionPath` for functions).
    item_name: String,
    /// Evaluation region this item falls into.
    region: SignalRegion,
    /// Canonical signal kind derived from `region` (see `signal_for_region` in this module).
    signal: ThreeWaySignalKind,
}

impl ThreeWaySignal {
    /// Constructs a `ThreeWaySignal`.
    ///
    /// The `signal` field is automatically derived from `region` to ensure
    /// consistency with the ADR 3 D3 signal table.
    ///
    /// # Examples
    ///
    /// ```
    /// use domain::tddd::signal_evaluator::region::{SignalRegion, ThreeWaySignal, ThreeWaySignalKind};
    /// let s = ThreeWaySignal::new("User".to_string(), SignalRegion::SIntersectC_Match_Add);
    /// assert_eq!(s.signal(), ThreeWaySignalKind::Blue);
    /// ```
    #[must_use]
    pub fn new(item_name: String, region: SignalRegion) -> Self {
        let signal = signal_for_region(region);
        Self { item_name, region, signal }
    }

    /// Returns the item identity key.
    #[must_use]
    pub fn item_name(&self) -> &str {
        &self.item_name
    }

    /// Returns the evaluation region.
    #[must_use]
    pub fn region(&self) -> SignalRegion {
        self.region
    }

    /// Returns the signal kind (always consistent with `region`).
    #[must_use]
    pub fn signal(&self) -> ThreeWaySignalKind {
        self.signal
    }
}

// ---------------------------------------------------------------------------
// ThreeWayEvaluationReport — Phase 2 output
// ---------------------------------------------------------------------------

/// Report produced by Phase 2 (S / D / C 3-way evaluation).
///
/// Contains all non-skip `ThreeWaySignal` entries — items that fall into
/// `SIntersectC_Match_Reference` (maintained, structure matches) are omitted
/// to reduce noise (ADR 3 D3).
///
/// Replaces `ConsistencyReport` for the new 3-way evaluator path.
///
/// ## Invariant
///
/// All signals stored in the report have `signal ∈ {Blue, Yellow, Red}` — the
/// `signals` field is private so callers cannot insert `Skip` entries or mutate
/// existing ones.  Use [`ThreeWayEvaluationReport::new`] to construct and
/// [`ThreeWayEvaluationReport::iter`] to inspect.
///
/// ## Interpreting the report
///
/// * All entries have `signal ∈ {Blue, Yellow, Red}`.
/// * `Blue` = declared goal achieved.
/// * `Yellow` = implementation in progress.
/// * `Red` = contract violation requiring immediate attention.
/// * An empty `signals` vec means every declared item is in the skip state
///   (all maintained) — this is the fully-converged state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreeWayEvaluationReport {
    /// Non-skip signals for all evaluated items.
    signals: Vec<ThreeWaySignal>,
}

impl ThreeWayEvaluationReport {
    /// Constructs a `ThreeWayEvaluationReport` from a list of signals.
    ///
    /// `Skip` signals are silently filtered out so that the report always
    /// contains only `Blue`, `Yellow`, and `Red` entries.  The caller need
    /// not pre-filter — the constructor enforces the invariant transparently.
    #[must_use]
    pub fn new(signals: Vec<ThreeWaySignal>) -> Self {
        let signals = signals.into_iter().filter(|s| !s.signal().is_skip()).collect();
        Self { signals }
    }

    /// Returns `true` if the report contains no non-skip signals.
    ///
    /// A fully-converged codebase (all items maintained, add/modify achieved,
    /// deletes completed) produces an empty report.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.signals.is_empty()
    }

    /// Returns the number of non-skip signals in the report.
    #[must_use]
    pub fn len(&self) -> usize {
        self.signals.len()
    }

    /// Returns an iterator over all non-skip signals.
    pub fn iter(&self) -> impl Iterator<Item = &ThreeWaySignal> {
        self.signals.iter()
    }

    /// Returns `true` if any signal is `Red`.
    #[must_use]
    pub fn has_violations(&self) -> bool {
        self.signals.iter().any(|s| s.signal().is_red())
    }

    /// Returns `true` if any signal is `Yellow` and none is `Red`.
    #[must_use]
    pub fn is_in_progress(&self) -> bool {
        !self.has_violations() && self.signals.iter().any(|s| s.signal().is_yellow())
    }
}

// ---------------------------------------------------------------------------
// Tests — structural + region→signal table coverage
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // -- SignalRegion coverage: all 12 variants map to a signal ---------------

    #[test]
    fn test_signal_for_region_s_intersect_c_match_add_returns_blue() {
        assert_eq!(
            signal_for_region(SignalRegion::SIntersectC_Match_Add),
            ThreeWaySignalKind::Blue
        );
    }

    #[test]
    fn test_signal_for_region_s_intersect_c_match_modify_returns_blue() {
        assert_eq!(
            signal_for_region(SignalRegion::SIntersectC_Match_Modify),
            ThreeWaySignalKind::Blue
        );
    }

    #[test]
    fn test_signal_for_region_s_intersect_c_match_reference_returns_skip() {
        assert_eq!(
            signal_for_region(SignalRegion::SIntersectC_Match_Reference),
            ThreeWaySignalKind::Skip
        );
    }

    #[test]
    fn test_signal_for_region_s_intersect_c_mismatch_reference_returns_red() {
        assert_eq!(
            signal_for_region(SignalRegion::SIntersectC_Mismatch_Reference),
            ThreeWaySignalKind::Red
        );
    }

    #[test]
    fn test_signal_for_region_s_intersect_c_mismatch_add_returns_yellow() {
        assert_eq!(
            signal_for_region(SignalRegion::SIntersectC_Mismatch_Add),
            ThreeWaySignalKind::Yellow
        );
    }

    #[test]
    fn test_signal_for_region_s_intersect_c_mismatch_modify_returns_yellow() {
        assert_eq!(
            signal_for_region(SignalRegion::SIntersectC_Mismatch_Modify),
            ThreeWaySignalKind::Yellow
        );
    }

    #[test]
    fn test_signal_for_region_s_minus_c_reference_returns_red() {
        assert_eq!(signal_for_region(SignalRegion::SMinusC_Reference), ThreeWaySignalKind::Red);
    }

    #[test]
    fn test_signal_for_region_s_minus_c_add_returns_yellow() {
        assert_eq!(signal_for_region(SignalRegion::SMinusC_Add), ThreeWaySignalKind::Yellow);
    }

    #[test]
    fn test_signal_for_region_s_minus_c_modify_returns_red() {
        assert_eq!(signal_for_region(SignalRegion::SMinusC_Modify), ThreeWaySignalKind::Red);
    }

    #[test]
    fn test_signal_for_region_d_intersect_c_returns_yellow() {
        assert_eq!(signal_for_region(SignalRegion::DIntersectC), ThreeWaySignalKind::Yellow);
    }

    #[test]
    fn test_signal_for_region_d_minus_c_returns_blue() {
        assert_eq!(signal_for_region(SignalRegion::DMinusC), ThreeWaySignalKind::Blue);
    }

    #[test]
    fn test_signal_for_region_c_minus_s_union_d_returns_red() {
        assert_eq!(signal_for_region(SignalRegion::CMinusSUnionD), ThreeWaySignalKind::Red);
    }

    // -- ThreeWaySignal constructor auto-derives signal from region -----------

    #[test]
    fn test_three_way_signal_new_derives_signal_from_region() {
        let s = ThreeWaySignal::new("User".to_string(), SignalRegion::SIntersectC_Match_Add);
        assert_eq!(s.signal(), ThreeWaySignalKind::Blue);
        assert_eq!(s.region(), SignalRegion::SIntersectC_Match_Add);
        assert_eq!(s.item_name(), "User");
    }

    #[test]
    fn test_three_way_signal_new_reference_match_yields_skip() {
        let s = ThreeWaySignal::new("Order".to_string(), SignalRegion::SIntersectC_Match_Reference);
        assert!(s.signal().is_skip());
    }

    // -- ThreeWayEvaluationReport helpers -------------------------------------

    #[test]
    fn test_report_empty_when_no_signals() {
        let report = ThreeWayEvaluationReport::new(vec![]);
        assert!(report.is_empty());
        assert_eq!(report.len(), 0);
        assert!(!report.has_violations());
        assert!(!report.is_in_progress());
    }

    #[test]
    fn test_report_new_filters_skip_signals() {
        // Skip signals must be excluded from the report, even if passed to `new`.
        let skip =
            ThreeWaySignal::new("Ref".to_string(), SignalRegion::SIntersectC_Match_Reference);
        let blue = ThreeWaySignal::new("Add".to_string(), SignalRegion::SIntersectC_Match_Add);
        let report = ThreeWayEvaluationReport::new(vec![skip, blue]);
        // Skip entry is filtered; only the Blue entry remains.
        assert_eq!(report.len(), 1);
        let collected: Vec<_> = report.iter().collect();
        assert_eq!(collected[0].item_name(), "Add");
        assert!(collected[0].signal().is_blue());
    }

    #[test]
    fn test_report_has_violations_when_red_present() {
        let signals = vec![ThreeWaySignal::new("Ghost".to_string(), SignalRegion::CMinusSUnionD)];
        let report = ThreeWayEvaluationReport::new(signals);
        assert!(report.has_violations());
        assert!(!report.is_in_progress());
    }

    #[test]
    fn test_report_is_in_progress_when_only_yellow_present() {
        let signals = vec![ThreeWaySignal::new("Feature".to_string(), SignalRegion::SMinusC_Add)];
        let report = ThreeWayEvaluationReport::new(signals);
        assert!(!report.has_violations());
        assert!(report.is_in_progress());
    }

    #[test]
    fn test_report_len_matches_signals_count() {
        let signals = vec![
            ThreeWaySignal::new("A".to_string(), SignalRegion::SIntersectC_Match_Add),
            ThreeWaySignal::new("B".to_string(), SignalRegion::DMinusC),
        ];
        let report = ThreeWayEvaluationReport::new(signals);
        assert_eq!(report.len(), 2);
    }

    #[test]
    fn test_report_iter_yields_all_signals() {
        let signals = vec![
            ThreeWaySignal::new("X".to_string(), SignalRegion::SIntersectC_Match_Modify),
            ThreeWaySignal::new("Y".to_string(), SignalRegion::DIntersectC),
        ];
        let report = ThreeWayEvaluationReport::new(signals.clone());
        let collected: Vec<_> = report.iter().collect();
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].item_name(), "X");
        assert_eq!(collected[1].item_name(), "Y");
    }

    // -- ThreeWaySignalKind helpers --------------------------------------------

    #[test]
    fn test_signal_kind_helpers_blue() {
        let k = ThreeWaySignalKind::Blue;
        assert!(k.is_blue());
        assert!(!k.is_skip());
        assert!(!k.is_yellow());
        assert!(!k.is_red());
    }

    #[test]
    fn test_signal_kind_helpers_yellow() {
        let k = ThreeWaySignalKind::Yellow;
        assert!(k.is_yellow());
        assert!(!k.is_blue());
        assert!(!k.is_red());
    }

    #[test]
    fn test_signal_kind_helpers_red() {
        let k = ThreeWaySignalKind::Red;
        assert!(k.is_red());
        assert!(!k.is_blue());
        assert!(!k.is_yellow());
    }

    #[test]
    fn test_signal_kind_helpers_skip() {
        let k = ThreeWaySignalKind::Skip;
        assert!(k.is_skip());
        assert!(!k.is_blue());
        assert!(!k.is_yellow());
        assert!(!k.is_red());
    }
}
