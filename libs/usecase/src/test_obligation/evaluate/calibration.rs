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
//! Kept private to the usecase crate: no `pub` surface leaks — the
//! interactor consumes [`CalibrationProbeShape`] internally, and tests
//! reconstruct the same shape via [`probe_shape_for`].

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
             assert!(gate_passes_when_red_signal_exists());\n\
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
             let rendered = serde_json::to_string(&GateReport::default()).unwrap();\n    \
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
             assert!(gate_passes_when_all_signals_are_blue());\n\
         }}\n"
    )
}

/// Percentage-scaled probe count (AC-08 / IN-01).
///
/// A non-zero production run with calibration enabled must exercise every
/// fulfillment-fail category before the verifier can be declared healthy.
/// The percentage-derived count therefore has a floor of three probes; once
/// the rate-derived count exceeds that floor, its normal scaling is preserved.
pub(super) fn calibration_probe_count(production_pair_count: usize, injection_rate: u8) -> usize {
    if production_pair_count == 0 || injection_rate == 0 {
        return 0;
    }
    let scaled = production_pair_count.saturating_mul(usize::from(injection_rate));
    let count = scaled.saturating_add(99) / 100;
    count.max(3)
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
