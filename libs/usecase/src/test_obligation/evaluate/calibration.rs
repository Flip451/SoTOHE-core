//! Calibration-probe shapes for [`super::EvaluateTestObligationsInteractor`] (AC-08).
//!
//! AC-08 requires the calibration probe to exercise all three
//! fulfillment-fail categories — (a) contradiction, (b) substitution, and
//! (c) central-unverified — so the verifier's health signal reflects
//! detection across the whole failure taxonomy rather than a single
//! contradiction shape. Each shape here pairs a known-bad tests source with
//! a declaration and anchor text chosen so that a prompt honouring AC-08
//! must return a `Fail` verdict for that category. Categories are
//! distributed across probe indices deterministically via `index % 3`
//! (see [`probe_shape_for`]) so any `probe_count >= 3` exercises all three.
//!
//! D6 also requires positive and negative locality examples. The
//! [`local_responsibility_probe_shapes`] set uses a generic in-memory name
//! index and a separate persistence-path target to ensure the configured
//! provider sees both sides of each ownership boundary without any
//! external-project special case. These probes are dispatched by the existing
//! host-owned `test-obligation evaluate` path; the structural Rust tests only
//! verify that the probe set is deterministic and executable-looking. A skipped
//! calibration is intentionally not provider evidence: the compatibility rate
//! returned by the evaluator for a skipped run must not be rendered as a fresh
//! provider result.

use domain::tddd::test_obligation::vocab::FulfillmentFailCategory;

/// Deterministic calibration-probe shape (AC-08).
///
/// `category` is the AC-08 fulfillment-fail category this probe is
/// designed to elicit; the surrounding `tests_source` / `declaration` /
/// `anchor_text` triple is chosen so a verifier honouring the AC-08 prompt
/// must return a `Fail` verdict for `category`.
pub(super) struct CalibrationProbeShape {
    pub(super) tests_source: String,
    pub(super) declaration: &'static str,
    pub(super) anchor_text: &'static str,
    pub(super) category: FulfillmentFailCategory,
}

/// Expected result for a D6 locality probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LocalResponsibilityExpectation {
    /// The target-owned behaviour is completely exercised by the probe.
    Fulfilled,
    /// The target's central behaviour is contradicted or left unverified.
    Rejected,
}

/// Positive or negative D6 entry-local responsibility example.
pub(super) struct LocalResponsibilityProbeShape {
    pub(super) tests_source: String,
    pub(super) entry_key: &'static str,
    pub(super) item_identifier: &'static str,
    pub(super) obligation_brief: &'static str,
    pub(super) entry_declaration: &'static str,
    pub(super) anchor_text: &'static str,
    pub(super) expectation: LocalResponsibilityExpectation,
}

/// Whether the current evaluation has a configured-provider calibration pass.
///
/// This is private to the usecase implementation because the outer application
/// outcome owns the no-production-pairs state. Keeping the classification here
/// prevents the probe-count calculation from accidentally turning either an
/// empty production scope or the explicit opt-out into a provider call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CalibrationExecution {
    /// There are no production pairs, so there is nothing to calibrate.
    SkippedNoProductionPairs,
    /// Calibration was explicitly disabled by configuration.
    SkippedByConfiguration,
    /// The configured provider will receive this many known-bad probes.
    Provider { probe_count: usize },
}

impl CalibrationExecution {
    /// Classifies calibration without performing provider work.
    pub(super) fn for_inputs(production_pair_count: usize, injection_rate: u8) -> Self {
        if production_pair_count == 0 {
            return Self::SkippedNoProductionPairs;
        }
        if injection_rate == 0 {
            return Self::SkippedByConfiguration;
        }

        let scaled = production_pair_count.saturating_mul(usize::from(injection_rate));
        let count = scaled.saturating_add(99) / 100;
        Self::Provider { probe_count: count.max(3) }
    }

    /// Returns the number of provider probes for this execution plan.
    pub(super) const fn probe_count(self) -> usize {
        match self {
            Self::SkippedNoProductionPairs | Self::SkippedByConfiguration => 0,
            Self::Provider { probe_count } => probe_count,
        }
    }
}

/// AC-08 (a) — contradiction anchor: promises 🔴 always blocks.
const CONTRADICTION_DECLARATION: &str = "KnownBadGateRule { red_signal: \"must block the gate\" }";
const CONTRADICTION_ANCHOR_TEXT: &str = "A red signal must always block the gate.";

/// AC-08 (b) — substitution anchor: same red-signal contract as (a), but
/// paired with a probe that only verifies unrelated JSON serialization.
const SUBSTITUTION_DECLARATION: &str = "KnownBadGateRule { red_signal: \"must block the gate\" }";
const SUBSTITUTION_ANCHOR_TEXT: &str = "A red signal must always block the gate.";

/// AC-08 (c) — central-unverified anchor: the three-branch contract
/// (all-🔵 → pass / todo-🟡 → no effect / 🔴 → always block); paired with a
/// probe that verifies only the all-🔵 happy path.
const CENTRAL_UNVERIFIED_DECLARATION: &str = "GateEvaluator { \
     blue: \"pass\", \
     yellow_todo: \"no effect\", \
     red: \"always block\" \
     }";
const CENTRAL_UNVERIFIED_ANCHOR_TEXT: &str = "The gate passes when all signals are blue, ignores todo-attributed \
     yellow signals, and always blocks on red.";

/// Returns the calibration-probe shape for `index` (AC-08).
///
/// Distributes indices across the three fulfillment-fail categories via
/// `index % 3` — so any `probe_count >= 3` issues at least one probe per
/// category, and the caller can require per-category detection before
/// marking the verifier healthy.
pub(super) fn probe_shape_for(index: usize) -> CalibrationProbeShape {
    let category = category_for(index);
    match category {
        FulfillmentFailCategory::Contradiction => CalibrationProbeShape {
            tests_source: contradiction_probe_source(index),
            declaration: CONTRADICTION_DECLARATION,
            anchor_text: CONTRADICTION_ANCHOR_TEXT,
            category,
        },
        FulfillmentFailCategory::Substitution => CalibrationProbeShape {
            tests_source: substitution_probe_source(index),
            declaration: SUBSTITUTION_DECLARATION,
            anchor_text: SUBSTITUTION_ANCHOR_TEXT,
            category,
        },
        FulfillmentFailCategory::CentralUnverified => CalibrationProbeShape {
            tests_source: central_unverified_probe_source(index),
            declaration: CENTRAL_UNVERIFIED_DECLARATION,
            anchor_text: CENTRAL_UNVERIFIED_ANCHOR_TEXT,
            category,
        },
    }
}

/// Deterministic category assignment (AC-08): `index % 3`.
fn category_for(index: usize) -> FulfillmentFailCategory {
    match index % 3 {
        0 => FulfillmentFailCategory::Contradiction,
        1 => FulfillmentFailCategory::Substitution,
        _ => FulfillmentFailCategory::CentralUnverified,
    }
}

/// AC-08 (a): anchor promises 🔴 always blocks; probe asserts pass despite 🔴.
fn contradiction_probe_source(index: usize) -> String {
    format!(
        "#[test]\n\
         fn known_bad_calibration_probe_contradiction_{index}() {{\n    \
             // AC-08 (a): anchor promises \"🔴 always blocks\";\n    \
             // this probe asserts the gate passes despite a 🔴 signal.\n    \
             let red_signal_is_present = true;\n    \
             let gate_blocks = false;\n    \
             assert!(red_signal_is_present);\n    \
             assert!(!gate_blocks);\n\
         }}\n"
    )
}

/// AC-08 (b): anchor cited, but probe only verifies unrelated JSON serialization.
fn substitution_probe_source(index: usize) -> String {
    format!(
        "#[test]\n\
         fn known_bad_calibration_probe_substitution_{index}() {{\n    \
             // AC-08 (b): the anchor is cited, but the probe only checks\n    \
             // an unrelated JSON-serialization concern — the anchor's\n    \
             // 🔴-blocks-gate contract is never observed.\n    \
             let rendered = \"{{\\\"gate\\\":\\\"pass\\\"}}\";\n    \
             assert!(rendered.contains(\"\\\"gate\\\"\"));\n\
         }}\n"
    )
}

/// AC-08 (c): probe only exercises the all-🔵 happy path; todo-🟡 no-effect
/// and 🔴 always-block branches remain unverified.
fn central_unverified_probe_source(index: usize) -> String {
    format!(
        "#[test]\n\
         fn known_bad_calibration_probe_central_unverified_{index}() {{\n    \
             // AC-08 (c): the anchor promises three branches\n    \
             // (all-🔵 → pass / todo-🟡 → no effect / 🔴 → always block),\n    \
             // but this probe verifies only the all-🔵 happy path.\n    \
             let all_signals_are_blue = true;\n    \
             assert!(all_signals_are_blue);\n\
         }}\n"
    )
}

const LOCAL_RESPONSIBILITY_ANCHOR_TEXT: &str = "An in-memory name index keeps independently named values distinct and available; a separate persistence path target owns a project-local storage location and excludes user-global placement.";
const MEMORY_ENTRY_KEY: &str = "InMemoryNameIndex";
const MEMORY_ITEM_IDENTIFIER: &str = "method:lookup";
const MEMORY_OBLIGATION_BRIEF: &str =
    "verify independently named values remain distinct and available in memory";
const MEMORY_ENTRY_DECLARATION: &str =
    "InMemoryNameIndex { names: mapping of independently named values }";
const PERSISTENCE_ENTRY_KEY: &str = "PersistencePathTarget";
const PERSISTENCE_ITEM_IDENTIFIER: &str = "field:storage_location";
const PERSISTENCE_OBLIGATION_BRIEF: &str =
    "verify the storage location is project-local and excludes user-global placement";
const PERSISTENCE_ENTRY_DECLARATION: &str = "PersistencePathTarget { storage_location: project-local path; user-global placement: excluded }";

/// Returns the four D6 locality examples in stable order:
/// memory-positive, memory-negative, persistence-positive, persistence-negative.
pub(super) fn local_responsibility_probe_shapes() -> Vec<LocalResponsibilityProbeShape> {
    vec![
        LocalResponsibilityProbeShape {
            tests_source: memory_positive_probe_source(),
            entry_key: MEMORY_ENTRY_KEY,
            item_identifier: MEMORY_ITEM_IDENTIFIER,
            obligation_brief: MEMORY_OBLIGATION_BRIEF,
            entry_declaration: MEMORY_ENTRY_DECLARATION,
            anchor_text: LOCAL_RESPONSIBILITY_ANCHOR_TEXT,
            expectation: LocalResponsibilityExpectation::Fulfilled,
        },
        LocalResponsibilityProbeShape {
            tests_source: memory_negative_probe_source(),
            entry_key: MEMORY_ENTRY_KEY,
            item_identifier: MEMORY_ITEM_IDENTIFIER,
            obligation_brief: MEMORY_OBLIGATION_BRIEF,
            entry_declaration: MEMORY_ENTRY_DECLARATION,
            anchor_text: LOCAL_RESPONSIBILITY_ANCHOR_TEXT,
            expectation: LocalResponsibilityExpectation::Rejected,
        },
        LocalResponsibilityProbeShape {
            tests_source: persistence_positive_probe_source(),
            entry_key: PERSISTENCE_ENTRY_KEY,
            item_identifier: PERSISTENCE_ITEM_IDENTIFIER,
            obligation_brief: PERSISTENCE_OBLIGATION_BRIEF,
            entry_declaration: PERSISTENCE_ENTRY_DECLARATION,
            anchor_text: LOCAL_RESPONSIBILITY_ANCHOR_TEXT,
            expectation: LocalResponsibilityExpectation::Fulfilled,
        },
        LocalResponsibilityProbeShape {
            tests_source: persistence_negative_probe_source(),
            entry_key: PERSISTENCE_ENTRY_KEY,
            item_identifier: PERSISTENCE_ITEM_IDENTIFIER,
            obligation_brief: PERSISTENCE_OBLIGATION_BRIEF,
            entry_declaration: PERSISTENCE_ENTRY_DECLARATION,
            anchor_text: LOCAL_RESPONSIBILITY_ANCHOR_TEXT,
            expectation: LocalResponsibilityExpectation::Rejected,
        },
    ]
}

fn memory_positive_probe_source() -> String {
    "#[test]\nfn local_responsibility_probe_memory_positive() {\n    let mut names = std::collections::BTreeMap::new();\n    names.insert(\"alpha\", 1_u8);\n    names.insert(\"beta\", 2_u8);\n    assert_eq!(names.get(\"alpha\"), Some(&1_u8));\n    assert_eq!(names.get(\"beta\"), Some(&2_u8));\n}\n"
        .to_owned()
}

fn memory_negative_probe_source() -> String {
    "#[test]\nfn local_responsibility_probe_memory_negative() {\n    let mut names = std::collections::BTreeMap::new();\n    names.insert(\"alpha\", 1_u8);\n    // Only one name is exercised, so independent-name coexistence remains unverified.\n    assert_eq!(names.get(\"alpha\"), Some(&1_u8));\n}\n"
        .to_owned()
}

fn persistence_positive_probe_source() -> String {
    "#[test]\nfn local_responsibility_probe_persistence_positive() {\n    use std::path::{Path, PathBuf};\n\n    fn project_local_storage_location(project_root: &Path) -> PathBuf {\n        project_root.join(\".state\").join(\"values.data\")\n    }\n\n    let project_root = Path::new(\"project-root\");\n    let user_global_root = Path::new(\"user-global-root\");\n    let location = project_local_storage_location(project_root);\n    assert!(location.starts_with(project_root));\n    assert!(!location.starts_with(user_global_root));\n}\n"
        .to_owned()
}

fn persistence_negative_probe_source() -> String {
    "#[test]\nfn local_responsibility_probe_persistence_negative() {\n    use std::path::{Path, PathBuf};\n\n    fn user_global_storage_location(user_global_root: &Path) -> PathBuf {\n        user_global_root.join(\"values.data\")\n    }\n\n    let project_root = Path::new(\"project-root\");\n    let user_global_root = Path::new(\"user-global-root\");\n    let location = user_global_storage_location(user_global_root);\n    assert!(location.starts_with(user_global_root));\n    assert!(!location.starts_with(project_root));\n}\n"
        .to_owned()
}

/// Percentage-scaled probe count (AC-08 / IN-01).
///
/// A non-zero production run with calibration enabled must exercise every
/// fulfillment-fail category before the verifier can be declared healthy.
/// The percentage-derived count therefore has a floor of three probes; once
/// the rate-derived count exceeds that floor, its normal scaling is preserved.
pub(super) fn calibration_probe_count(production_pair_count: usize, injection_rate: u8) -> usize {
    CalibrationExecution::for_inputs(production_pair_count, injection_rate).probe_count()
}

/// Per-category detection tally for the calibration loop (AC-08).
///
/// Tracks `(issued, detected)` counts per fulfillment-fail category so the
/// caller can require at least one detection in every exercised category
/// before declaring the verifier healthy — an aggregate rate at or above
/// threshold cannot mask a category the verifier never flags.
#[derive(Debug, Default)]
pub(super) struct CategoryTally {
    contradiction: CategoryCounts,
    substitution: CategoryCounts,
    central_unverified: CategoryCounts,
}

#[derive(Debug, Default, Clone, Copy)]
struct CategoryCounts {
    issued: usize,
    detected: usize,
}

impl CategoryTally {
    /// Records a probe issued for `category`.
    pub(super) fn record_issued(&mut self, category: &FulfillmentFailCategory) {
        self.slot_mut(category).issued += 1;
    }

    /// Records a `Fail` verdict for a probe issued under `category`.
    pub(super) fn record_detected(&mut self, category: &FulfillmentFailCategory) {
        self.slot_mut(category).detected += 1;
    }

    /// Returns categories that were exercised but ended with zero detected
    /// probes — the AC-08 healthy-verifier gate refuses these.
    pub(super) fn undetected_categories(&self) -> Vec<&'static str> {
        let entries = [
            (&self.contradiction, FulfillmentFailCategory::Contradiction),
            (&self.substitution, FulfillmentFailCategory::Substitution),
            (&self.central_unverified, FulfillmentFailCategory::CentralUnverified),
        ];
        entries
            .into_iter()
            .filter(|(counts, _)| counts.issued > 0 && counts.detected == 0)
            .map(|(_, category)| category.as_kebab())
            .collect()
    }

    fn slot_mut(&mut self, category: &FulfillmentFailCategory) -> &mut CategoryCounts {
        match category {
            FulfillmentFailCategory::Contradiction => &mut self.contradiction,
            FulfillmentFailCategory::Substitution => &mut self.substitution,
            FulfillmentFailCategory::CentralUnverified => &mut self.central_unverified,
        }
    }
}
