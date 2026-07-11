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
use crate::tddd::test_obligation::ids::DiagnosticMessage;

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

impl TargetEntryRoleKind {
    /// Returns the persistence canonical form used by obligation derivation; see IN-07.
    ///
    /// Catalogue roles may contain projection-only payloads, while persisted
    /// obligations retain only their stable role selector.
    pub fn canonical_form(&self) -> Result<Self, DiagnosticMessage> {
        match self {
            Self::DataRole(role) => role
                .variant_name()
                .parse::<DataRole>()
                .map(Self::DataRole)
                .map_err(|_| canonical_form_error("invalid data role for obligation artifact")),
            Self::ContractRole(role) => {
                role.variant_name().parse::<ContractRole>().map(Self::ContractRole).map_err(|_| {
                    canonical_form_error("invalid contract role for obligation artifact")
                })
            }
            Self::FunctionRole(role) => {
                role.to_string().parse::<FunctionRole>().map(Self::FunctionRole).map_err(|_| {
                    canonical_form_error("invalid function role for obligation artifact")
                })
            }
            Self::TraitImpl(role) => {
                role.variant_name().parse::<ContractRole>().map(Self::TraitImpl).map_err(|_| {
                    canonical_form_error(
                        "invalid trait implementation role for obligation artifact",
                    )
                })
            }
            Self::Pattern(pattern) => Ok(Self::Pattern(pattern.clone())),
        }
    }
}

/// Builds the non-empty diagnostic required by [`TargetEntryRoleKind::canonical_form`].
fn canonical_form_error(message: &str) -> DiagnosticMessage {
    let mut text = message.to_owned();
    loop {
        match DiagnosticMessage::try_new(text) {
            Ok(message) => return message,
            Err(_) => text = "invalid target entry role".to_owned(),
        }
    }
}

/// Drift classification for a test-obligation edge (IN-13 / AC-05).
///
/// Split into two families. *Existence* drifts are independent deterministic
/// checks on whether the edge still resolves: [`Missing`](Self::Missing) (an
/// obligation has no binding, or a bound test no longer exists — a renamed test
/// is caught here) and [`Orphaned`](Self::Orphaned) (a binding exists but no
/// obligation is derived for it). *Freshness* drifts are the display names of a
/// stale verdict, one per cache-key component: evidence-side
/// [`SpecChanged`](Self::SpecChanged) (anchor text hash changed) /
/// [`DeclChanged`](Self::DeclChanged) (entry declaration hash changed), and
/// claim-side [`TestChanged`](Self::TestChanged) (bound test body hash changed) /
/// [`ReasonChanged`](Self::ReasonChanged) (waived reason prose hash changed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestObligationDriftKind {
    /// An obligation has no binding (or its bound test no longer exists).
    Missing,
    /// A binding exists but no obligation is derived for it.
    Orphaned,
    /// The anchor text hash changed, staling the verdict.
    SpecChanged,
    /// The entry declaration hash changed, staling the verdict.
    DeclChanged,
    /// A bound test body hash changed, staling the verdict.
    TestChanged,
    /// The waived reason prose hash changed, staling the verdict.
    ReasonChanged,
}

impl TestObligationDriftKind {
    /// Returns the canonical kebab-case display name for this drift kind.
    #[must_use]
    pub fn as_kebab(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Orphaned => "orphaned",
            Self::SpecChanged => "spec_changed",
            Self::DeclChanged => "decl_changed",
            Self::TestChanged => "test_changed",
            Self::ReasonChanged => "reason_changed",
        }
    }

    /// Returns `true` for the existence family (`missing` / `orphaned`).
    #[must_use]
    pub fn is_existence(&self) -> bool {
        match self {
            Self::Missing | Self::Orphaned => true,
            Self::SpecChanged | Self::DeclChanged | Self::TestChanged | Self::ReasonChanged => {
                false
            }
        }
    }

    /// Returns `true` for the freshness family (the four `*_changed` kinds).
    #[must_use]
    pub fn is_freshness(&self) -> bool {
        match self {
            Self::SpecChanged | Self::DeclChanged | Self::TestChanged | Self::ReasonChanged => true,
            Self::Missing | Self::Orphaned => false,
        }
    }
}

/// Failure category for an obligation-fulfillment verdict (IN-12 / AC-08).
///
/// Names why bound tests failed to fulfill an anchor's promise:
/// [`Contradiction`](Self::Contradiction) — a test asserts the opposite of what
/// the anchor promises; [`Substitution`](Self::Substitution) — a test cites the
/// anchor but verifies unrelated content; [`CentralUnverified`](Self::CentralUnverified)
/// — no contradiction or irrelevance, but the anchor's central behavior is left
/// unverified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FulfillmentFailCategory {
    /// A test asserts the opposite of the anchor's promise.
    Contradiction,
    /// A test cites the anchor but verifies unrelated content.
    Substitution,
    /// The anchor's central behavior is left unverified.
    CentralUnverified,
}

impl FulfillmentFailCategory {
    /// Returns the canonical kebab-case spelling for this fail category.
    #[must_use]
    pub fn as_kebab(&self) -> &'static str {
        match self {
            Self::Contradiction => "contradiction",
            Self::Substitution => "substitution",
            Self::CentralUnverified => "central_unverified",
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::tddd::catalogue_v2::identifiers::TypeRef;

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

    #[test]
    fn test_target_entry_role_kind_canonical_form_with_data_role_discards_payload() {
        let role = TargetEntryRoleKind::DataRole(DataRole::UseCase {
            handles: vec![TypeRef::new("CreateOrder").unwrap()],
        });

        assert_eq!(
            role.canonical_form().unwrap(),
            TargetEntryRoleKind::DataRole(DataRole::use_case())
        );
    }

    #[test]
    fn test_target_entry_role_kind_canonical_form_with_contract_role_discards_payload() {
        let role = TargetEntryRoleKind::ContractRole(ContractRole::Repository {
            aggregate: TypeRef::new("Order").unwrap(),
        });

        assert_eq!(
            role.canonical_form().unwrap(),
            TargetEntryRoleKind::ContractRole(ContractRole::Repository {
                aggregate: TypeRef::new("AggregateRoot").unwrap(),
            })
        );
    }

    #[test]
    fn test_target_entry_role_kind_canonical_form_with_function_role_preserves_role() {
        let role = TargetEntryRoleKind::FunctionRole(FunctionRole::UseCaseFunction);

        assert_eq!(
            role.canonical_form().unwrap(),
            TargetEntryRoleKind::FunctionRole(FunctionRole::UseCaseFunction)
        );
    }

    #[test]
    fn test_target_entry_role_kind_canonical_form_with_trait_impl_discards_contract_payload() {
        let role = TargetEntryRoleKind::TraitImpl(ContractRole::Repository {
            aggregate: TypeRef::new("Order").unwrap(),
        });

        assert_eq!(
            role.canonical_form().unwrap(),
            TargetEntryRoleKind::TraitImpl(ContractRole::Repository {
                aggregate: TypeRef::new("AggregateRoot").unwrap(),
            })
        );
    }

    #[test]
    fn test_target_entry_role_kind_canonical_form_with_pattern_preserves_pattern() {
        let role = TargetEntryRoleKind::Pattern(TestObligationPatternKind::Typestate);

        assert_eq!(
            role.canonical_form().unwrap(),
            TargetEntryRoleKind::Pattern(TestObligationPatternKind::Typestate)
        );
    }

    const ALL_DRIFT_KINDS: &[TestObligationDriftKind] = &[
        TestObligationDriftKind::Missing,
        TestObligationDriftKind::Orphaned,
        TestObligationDriftKind::SpecChanged,
        TestObligationDriftKind::DeclChanged,
        TestObligationDriftKind::TestChanged,
        TestObligationDriftKind::ReasonChanged,
    ];

    #[test]
    fn test_drift_kind_families_partition_all_variants() {
        for kind in ALL_DRIFT_KINDS {
            assert_ne!(
                kind.is_existence(),
                kind.is_freshness(),
                "each drift kind belongs to exactly one family: {kind:?}"
            );
        }
    }

    #[test]
    fn test_drift_kind_existence_and_freshness_membership() {
        assert!(TestObligationDriftKind::Missing.is_existence());
        assert!(TestObligationDriftKind::Orphaned.is_existence());
        assert!(TestObligationDriftKind::SpecChanged.is_freshness());
        assert!(TestObligationDriftKind::DeclChanged.is_freshness());
        assert!(TestObligationDriftKind::TestChanged.is_freshness());
        assert!(TestObligationDriftKind::ReasonChanged.is_freshness());
    }

    #[test]
    fn test_drift_kind_kebab_spellings() {
        assert_eq!(TestObligationDriftKind::Missing.as_kebab(), "missing");
        assert_eq!(TestObligationDriftKind::Orphaned.as_kebab(), "orphaned");
        assert_eq!(TestObligationDriftKind::SpecChanged.as_kebab(), "spec_changed");
        assert_eq!(TestObligationDriftKind::DeclChanged.as_kebab(), "decl_changed");
        assert_eq!(TestObligationDriftKind::TestChanged.as_kebab(), "test_changed");
        assert_eq!(TestObligationDriftKind::ReasonChanged.as_kebab(), "reason_changed");
    }

    #[test]
    fn test_fulfillment_fail_category_kebab_spellings() {
        assert_eq!(FulfillmentFailCategory::Contradiction.as_kebab(), "contradiction");
        assert_eq!(FulfillmentFailCategory::Substitution.as_kebab(), "substitution");
        assert_eq!(FulfillmentFailCategory::CentralUnverified.as_kebab(), "central_unverified");
    }
}
