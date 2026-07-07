//! Closed vocabularies for the test-obligation decision table.
//!
//! Defines the obligation-kind axis ([`TestObligationKind`]), the per-axis
//! vocabulary ([`TestObligationPerAxis`]), the pattern-section vocabulary
//! ([`TestObligationPatternKind`]), and the target-section discriminant
//! ([`TargetEntryRoleKind`]) that the derivation engine applies decision-table
//! rules against (IN-02 / IN-07 / IN-17 / CN-10).
//!
//! The `as_kebab` projections are the canonical wire spelling used by the
//! config codec (`.harness/config/test-obligation-rules.json`). The interpreter
//! for these vocabularies is an exhaustive Rust `match`, so extending a
//! vocabulary is a template-side change (CN-10).

use crate::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole};

/// Obligation-kind vocabulary produced by the decision-table rules.
///
/// Each variant names a category of test the derivation engine may require for
/// an entry. The projection [`TestObligationKind::as_kebab`] is the canonical
/// spelling used in the rules config (IN-05 / CN-10 / CN-16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestObligationKind {
    /// Boundary tests (valid accepted / invalid rejected) for an invariant.
    Boundary,
    /// Invariant-preservation test across a mutating operation.
    InvariantPreservation,
    /// Event-emission test for a declared `emits` event.
    EventEmission,
    /// Logic-result test for a domain-service method.
    LogicResult,
    /// Predicate test covering both branches (satisfied / not satisfied).
    PredicateBothBranches,
    /// Construction-result test for a factory.
    ConstructionResult,
    /// Result test for a use-case / application-service entry.
    Result,
    /// Reaction test for an event policy.
    Reaction,
    /// Transition test for a typestate transition.
    Transition,
    /// Contract test for a port trait method.
    Contract,
    /// Contract-conformance test for a trait implementation.
    ContractConformance,
    /// Generic logic test for a free function.
    Logic,
}

impl TestObligationKind {
    /// Returns the canonical kebab-case spelling used in the rules config.
    #[must_use]
    pub fn as_kebab(&self) -> &'static str {
        match self {
            Self::Boundary => "boundary",
            Self::InvariantPreservation => "invariant_preservation",
            Self::EventEmission => "event_emission",
            Self::LogicResult => "logic_result",
            Self::PredicateBothBranches => "predicate_both_branches",
            Self::ConstructionResult => "construction_result",
            Self::Result => "result",
            Self::Reaction => "reaction",
            Self::Transition => "transition",
            Self::Contract => "contract",
            Self::ContractConformance => "contract_conformance",
            Self::Logic => "logic",
        }
    }
}

/// Per-axis vocabulary — the declaration facet an obligation is generated over.
///
/// The closed set corresponds to declaration payload fields (CN-10):
/// `invariant` / `method` / `handles` / `reacts_to` / `transition` /
/// `trait_method` / `entry` / `emits` / `trait_impl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestObligationPerAxis {
    /// One obligation per declared invariant.
    Invariant,
    /// One obligation per method.
    Method,
    /// One obligation per `handles` type.
    Handles,
    /// One obligation per `reacts_to` event.
    ReactsTo,
    /// One obligation per typestate transition.
    Transition,
    /// One obligation per trait method.
    TraitMethod,
    /// One obligation for the entry as a whole.
    Entry,
    /// One obligation per declared `emits` event.
    Emits,
    /// One obligation per trait implementation.
    TraitImpl,
}

impl TestObligationPerAxis {
    /// Returns the canonical kebab-case spelling used in the rules config.
    #[must_use]
    pub fn as_kebab(&self) -> &'static str {
        match self {
            Self::Invariant => "invariant",
            Self::Method => "method",
            Self::Handles => "handles",
            Self::ReactsTo => "reacts_to",
            Self::Transition => "transition",
            Self::TraitMethod => "trait_method",
            Self::Entry => "entry",
            Self::Emits => "emits",
            Self::TraitImpl => "trait_impl",
        }
    }
}

/// Pattern-section vocabulary for the decision table (IN-03 / AC-01).
///
/// The only pattern the default config ships is `typestate`; the enum is kept
/// so new patterns are added as a template-side vocabulary extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestObligationPatternKind {
    /// The typestate pattern (transition methods carry obligations).
    Typestate,
}

/// Target-section discriminant for applying decision-table rules (IN-07 / IN-17).
///
/// Names which section of the rules document a target entry resolves to: one of
/// the three role sections, the trait-impl section, or the pattern section. The
/// role sections carry their section-specific role enum; the pattern section
/// carries a [`TestObligationPatternKind`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetEntryRoleKind {
    /// A `TypeEntry` resolved by its `DataRole`.
    DataRole(DataRole),
    /// A `TraitEntry` resolved by its `ContractRole`.
    ContractRole(ContractRole),
    /// A `FunctionEntry` resolved by its `FunctionRole`.
    FunctionRole(FunctionRole),
    /// A `TraitImpl` entry resolved by the `ContractRole` of its target trait.
    TraitImpl(ContractRole),
    /// A pattern-driven target resolved by its pattern kind.
    Pattern(TestObligationPatternKind),
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    const ALL_KINDS: &[TestObligationKind] = &[
        TestObligationKind::Boundary,
        TestObligationKind::InvariantPreservation,
        TestObligationKind::EventEmission,
        TestObligationKind::LogicResult,
        TestObligationKind::PredicateBothBranches,
        TestObligationKind::ConstructionResult,
        TestObligationKind::Result,
        TestObligationKind::Reaction,
        TestObligationKind::Transition,
        TestObligationKind::Contract,
        TestObligationKind::ContractConformance,
        TestObligationKind::Logic,
    ];

    const ALL_PER_AXES: &[TestObligationPerAxis] = &[
        TestObligationPerAxis::Invariant,
        TestObligationPerAxis::Method,
        TestObligationPerAxis::Handles,
        TestObligationPerAxis::ReactsTo,
        TestObligationPerAxis::Transition,
        TestObligationPerAxis::TraitMethod,
        TestObligationPerAxis::Entry,
        TestObligationPerAxis::Emits,
        TestObligationPerAxis::TraitImpl,
    ];

    #[test]
    fn test_obligation_kind_has_12_variants() {
        assert_eq!(ALL_KINDS.len(), 12);
    }

    #[test]
    fn test_obligation_kind_kebab_is_unique_and_snake_case() {
        let mut kebabs: Vec<&str> = ALL_KINDS.iter().map(TestObligationKind::as_kebab).collect();
        kebabs.sort_unstable();
        let unique_count = {
            let mut deduped = kebabs.clone();
            deduped.dedup();
            deduped.len()
        };
        assert_eq!(kebabs.len(), unique_count, "kebab spellings must be unique");
        assert!(kebabs.iter().all(|k| k.chars().all(|c| c.is_ascii_lowercase() || c == '_')));
    }

    #[test]
    fn test_obligation_kind_kebab_roundtrip_examples() {
        assert_eq!(TestObligationKind::InvariantPreservation.as_kebab(), "invariant_preservation");
        assert_eq!(TestObligationKind::ContractConformance.as_kebab(), "contract_conformance");
        assert_eq!(TestObligationKind::Result.as_kebab(), "result");
    }

    #[test]
    fn test_per_axis_has_9_variants() {
        assert_eq!(ALL_PER_AXES.len(), 9);
    }

    #[test]
    fn test_per_axis_kebab_examples() {
        assert_eq!(TestObligationPerAxis::ReactsTo.as_kebab(), "reacts_to");
        assert_eq!(TestObligationPerAxis::TraitMethod.as_kebab(), "trait_method");
        assert_eq!(TestObligationPerAxis::TraitImpl.as_kebab(), "trait_impl");
        assert_eq!(TestObligationPerAxis::Entry.as_kebab(), "entry");
    }

    #[test]
    fn test_target_entry_role_kind_wraps_section_specific_role_and_pattern() {
        let data_role = DataRole::value_object();
        let data = TargetEntryRoleKind::DataRole(data_role.clone());
        let contract = TargetEntryRoleKind::ContractRole(ContractRole::SecondaryPort);
        let function = TargetEntryRoleKind::FunctionRole(FunctionRole::FreeFunction);
        let trait_impl = TargetEntryRoleKind::TraitImpl(ContractRole::ApplicationService);
        let pattern = TargetEntryRoleKind::Pattern(TestObligationPatternKind::Typestate);

        assert_eq!(data, TargetEntryRoleKind::DataRole(data_role));
        assert_eq!(contract, TargetEntryRoleKind::ContractRole(ContractRole::SecondaryPort));
        assert_eq!(function, TargetEntryRoleKind::FunctionRole(FunctionRole::FreeFunction));
        assert_eq!(trait_impl, TargetEntryRoleKind::TraitImpl(ContractRole::ApplicationService));
        assert_ne!(data, pattern);
    }
}
