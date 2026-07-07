//! Value objects for the test-obligation decision table (rules document).
//!
//! Models the role → obligation-generation rules that
//! `.harness/config/test-obligation-rules.json` encodes (IN-02 / IN-03 / CN-05 /
//! CN-10 / AC-01 / AC-02). [`TestObligationRulesDocument::try_new`] enforces the
//! fail-closed load-time totality guarantee: every role enum variant must be
//! explicitly declared (a zero-obligation role as an explicit empty list), so a
//! newly added role can never be silently omitted.

use crate::ValidationError;
use crate::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole};
use crate::tddd::test_obligation::errors::TestObligationRulesLoadError;
use crate::tddd::test_obligation::ids::{DiagnosticMessage, RoleName};
use crate::tddd::test_obligation::vocab::{
    TestObligationKind, TestObligationPatternKind, TestObligationPerAxis,
};

/// Canonical `DataRole` variant names the rules document must cover.
const EXPECTED_DATA_ROLE_NAMES: &[&str] = &[
    "ValueObject",
    "Entity",
    "AggregateRoot",
    "DomainService",
    "Specification",
    "Factory",
    "UseCase",
    "Interactor",
    "Command",
    "Query",
    "Dto",
    "ErrorType",
    "SecondaryAdapter",
    "EventPolicy",
    "DomainEvent",
    "CompositionRoot",
    "PrimaryAdapter",
];

/// Canonical `ContractRole` variant names the rules document must cover.
const EXPECTED_CONTRACT_ROLE_NAMES: &[&str] =
    &["SpecificationPort", "ApplicationService", "SecondaryPort", "Repository"];

/// Canonical `FunctionRole` variant names the rules document must cover.
const EXPECTED_FUNCTION_ROLE_NAMES: &[&str] = &["FreeFunction", "UseCaseFunction"];

/// Canonical pattern-section keys the rules document must cover.
const EXPECTED_PATTERN_NAMES: &[&str] = &["Typestate"];

/// Minimum obligation count for a rule, guaranteed to be at least one.
///
/// Encodes the `"min"` field in the decision table (e.g. `UseCase`'s
/// `{ "kind": "result", "per": "handles", "min": 1 }`): even when the per-axis
/// yields no items, at least this many obligations are required (IN-02 / AC-01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationMinimum(usize);

impl TestObligationMinimum {
    /// Validate and wrap `value` as a [`TestObligationMinimum`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidObligationMinimum`] when `value` is `0`;
    /// a minimum of zero is indistinguishable from omitting the field.
    pub fn try_new(value: usize) -> Result<Self, ValidationError> {
        if value == 0 {
            return Err(ValidationError::InvalidObligationMinimum(value));
        }
        Ok(Self(value))
    }

    /// Returns the minimum count.
    #[must_use]
    pub fn as_usize(&self) -> usize {
        self.0
    }
}

/// A brief-generation template attached to a rule.
///
/// Free-form, non-empty text that the derive interactor expands into a per
/// obligation brief for the implementer (IN-02 / CN-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationBriefTemplate(String);

impl TestObligationBriefTemplate {
    /// Validate and wrap `template` as a [`TestObligationBriefTemplate`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `template` is empty or
    /// whitespace-only.
    pub fn try_new(template: String) -> Result<Self, ValidationError> {
        if template.trim().is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self(template))
    }

    /// Borrow the inner template string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A single obligation-generation rule (`kind` × `per` × optional `min` / brief).
///
/// Names the category of test to require ([`TestObligationKind`]), the
/// declaration facet to iterate ([`TestObligationPerAxis`]), an optional floor
/// count, and an optional brief template (IN-02 / CN-10 / AC-01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationRule {
    kind: TestObligationKind,
    per_axis: TestObligationPerAxis,
    minimum: Option<TestObligationMinimum>,
    // Consumed by the derive interactor (T016) when it expands per-obligation
    // briefs; no read accessor is declared in this batch's type contract.
    #[allow(dead_code)]
    brief_template: Option<TestObligationBriefTemplate>,
}

impl TestObligationRule {
    /// Creates a new [`TestObligationRule`].
    #[must_use]
    pub fn new(
        kind: TestObligationKind,
        per_axis: TestObligationPerAxis,
        minimum: Option<TestObligationMinimum>,
        brief_template: Option<TestObligationBriefTemplate>,
    ) -> Self {
        Self { kind, per_axis, minimum, brief_template }
    }

    /// Returns the obligation kind this rule generates.
    #[must_use]
    pub fn kind(&self) -> &TestObligationKind {
        &self.kind
    }

    /// Returns the per-axis facet this rule iterates.
    #[must_use]
    pub fn per_axis(&self) -> &TestObligationPerAxis {
        &self.per_axis
    }

    /// Returns the optional minimum obligation count.
    #[must_use]
    pub fn minimum(&self) -> Option<&TestObligationMinimum> {
        self.minimum.as_ref()
    }
}

/// The rule list declared for a single role (or pattern / trait-impl key).
///
/// A role with no obligations is represented by an explicit empty list, which is
/// distinct from omitting the entry entirely (the omission is rejected earlier
/// as a load error). [`RoleObligationRules::is_empty_explicitly`] reports the
/// explicit zero-obligation case (IN-02 / CN-05 / AC-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleObligationRules {
    obligations: Vec<TestObligationRule>,
}

impl RoleObligationRules {
    /// Creates a [`RoleObligationRules`] from an explicit obligation list.
    #[must_use]
    pub fn new(obligations: Vec<TestObligationRule>) -> Self {
        Self { obligations }
    }

    /// Returns the declared obligation rules.
    #[must_use]
    pub fn obligations(&self) -> &[TestObligationRule] {
        &self.obligations
    }

    /// Returns `true` when the role explicitly declares zero obligations.
    #[must_use]
    pub fn is_empty_explicitly(&self) -> bool {
        self.obligations.is_empty()
    }
}

/// The whole decision table: rules keyed by role / pattern / trait-impl section.
///
/// Constructed via [`TestObligationRulesDocument::try_new`], which enforces the
/// load-time totality guarantee (every role enum variant present) before the
/// document is usable (IN-02 / IN-03 / CN-05 / AC-01 / AC-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationRulesDocument {
    data_roles: Vec<(DataRole, RoleObligationRules)>,
    contract_roles: Vec<(ContractRole, RoleObligationRules)>,
    function_roles: Vec<(FunctionRole, RoleObligationRules)>,
    patterns: Vec<(TestObligationPatternKind, RoleObligationRules)>,
    trait_impls: Vec<(ContractRole, RoleObligationRules)>,
}

impl TestObligationRulesDocument {
    /// Builds a [`TestObligationRulesDocument`], enforcing role totality.
    ///
    /// Every `DataRole`, `ContractRole`, and `FunctionRole` variant must appear
    /// in its section, the pattern section must cover every pattern kind, and
    /// the `trait_impls` section must cover every `ContractRole` variant.
    ///
    /// # Errors
    ///
    /// - [`TestObligationRulesLoadError::RoleNotCovered`] when a section omits a
    ///   role / pattern the enum requires.
    /// - [`TestObligationRulesLoadError::InvalidRuleValue`] when a section
    ///   declares the same role more than once.
    pub fn try_new(
        data_roles: Vec<(DataRole, RoleObligationRules)>,
        contract_roles: Vec<(ContractRole, RoleObligationRules)>,
        function_roles: Vec<(FunctionRole, RoleObligationRules)>,
        patterns: Vec<(TestObligationPatternKind, RoleObligationRules)>,
        trait_impls: Vec<(ContractRole, RoleObligationRules)>,
    ) -> Result<Self, TestObligationRulesLoadError> {
        let data_names: Vec<String> =
            data_roles.iter().map(|(role, _)| role.variant_name().to_owned()).collect();
        let contract_names: Vec<String> =
            contract_roles.iter().map(|(role, _)| role.variant_name().to_owned()).collect();
        let function_names: Vec<String> =
            function_roles.iter().map(|(role, _)| function_role_name(*role).to_owned()).collect();
        let pattern_names: Vec<String> =
            patterns.iter().map(|(pattern, _)| pattern_name(pattern).to_owned()).collect();
        let trait_impl_names: Vec<String> =
            trait_impls.iter().map(|(role, _)| role.variant_name().to_owned()).collect();

        check_no_duplicates(&data_names)?;
        check_no_duplicates(&contract_names)?;
        check_no_duplicates(&function_names)?;
        check_no_duplicates(&pattern_names)?;
        check_no_duplicates(&trait_impl_names)?;

        check_totality(EXPECTED_DATA_ROLE_NAMES, &data_names)?;
        check_totality(EXPECTED_CONTRACT_ROLE_NAMES, &contract_names)?;
        check_totality(EXPECTED_FUNCTION_ROLE_NAMES, &function_names)?;
        check_totality(EXPECTED_PATTERN_NAMES, &pattern_names)?;
        check_totality(EXPECTED_CONTRACT_ROLE_NAMES, &trait_impl_names)?;

        Ok(Self { data_roles, contract_roles, function_roles, patterns, trait_impls })
    }

    /// Returns the rules keyed by `DataRole`.
    #[must_use]
    pub fn data_roles(&self) -> &[(DataRole, RoleObligationRules)] {
        &self.data_roles
    }

    /// Returns the rules keyed by `ContractRole`.
    #[must_use]
    pub fn contract_roles(&self) -> &[(ContractRole, RoleObligationRules)] {
        &self.contract_roles
    }

    /// Returns the rules keyed by `FunctionRole`.
    #[must_use]
    pub fn function_roles(&self) -> &[(FunctionRole, RoleObligationRules)] {
        &self.function_roles
    }

    /// Returns the rules keyed by pattern kind.
    #[must_use]
    pub fn patterns(&self) -> &[(TestObligationPatternKind, RoleObligationRules)] {
        &self.patterns
    }

    /// Returns the trait-implementation conformance rules keyed by `ContractRole`.
    #[must_use]
    pub fn trait_impls(&self) -> &[(ContractRole, RoleObligationRules)] {
        &self.trait_impls
    }
}

/// Returns the canonical variant name for a `FunctionRole`.
fn function_role_name(role: FunctionRole) -> &'static str {
    match role {
        FunctionRole::FreeFunction => "FreeFunction",
        FunctionRole::UseCaseFunction => "UseCaseFunction",
    }
}

/// Returns the canonical section key for a pattern kind.
fn pattern_name(pattern: &TestObligationPatternKind) -> &'static str {
    match pattern {
        TestObligationPatternKind::Typestate => "Typestate",
    }
}

/// Fails when any `expected` name is absent from `present`.
fn check_totality(
    expected: &[&str],
    present: &[String],
) -> Result<(), TestObligationRulesLoadError> {
    if let Some(role_name) = expected
        .iter()
        .find(|name| !present.iter().any(|p| p == *name))
        .and_then(|name| RoleName::try_new((*name).to_owned()).ok())
    {
        return Err(TestObligationRulesLoadError::RoleNotCovered { role_name });
    }
    Ok(())
}

/// Fails when `present` declares the same role name more than once.
fn check_no_duplicates(present: &[String]) -> Result<(), TestObligationRulesLoadError> {
    for (index, name) in present.iter().enumerate() {
        if present.iter().skip(index + 1).any(|other| other == name) {
            // `name` is a non-empty variant name, so both constructions succeed;
            // the guard keeps the path panic-free without an unreachable fallback.
            if let (Ok(role_name), Ok(message)) = (
                RoleName::try_new(name.clone()),
                DiagnosticMessage::try_new(format!(
                    "role '{name}' is declared more than once in its section"
                )),
            ) {
                return Err(TestObligationRulesLoadError::InvalidRuleValue { role_name, message });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn empty_rules() -> RoleObligationRules {
        RoleObligationRules::new(vec![])
    }

    fn all_data_roles() -> Vec<(DataRole, RoleObligationRules)> {
        EXPECTED_DATA_ROLE_NAMES
            .iter()
            .map(|name| (name.parse::<DataRole>().unwrap(), empty_rules()))
            .collect()
    }

    fn all_contract_roles() -> Vec<(ContractRole, RoleObligationRules)> {
        EXPECTED_CONTRACT_ROLE_NAMES
            .iter()
            .map(|name| (name.parse::<ContractRole>().unwrap(), empty_rules()))
            .collect()
    }

    fn all_function_roles() -> Vec<(FunctionRole, RoleObligationRules)> {
        EXPECTED_FUNCTION_ROLE_NAMES
            .iter()
            .map(|name| (name.parse::<FunctionRole>().unwrap(), empty_rules()))
            .collect()
    }

    fn all_patterns() -> Vec<(TestObligationPatternKind, RoleObligationRules)> {
        vec![(TestObligationPatternKind::Typestate, empty_rules())]
    }

    fn complete_document() -> Result<TestObligationRulesDocument, TestObligationRulesLoadError> {
        TestObligationRulesDocument::try_new(
            all_data_roles(),
            all_contract_roles(),
            all_function_roles(),
            all_patterns(),
            all_contract_roles(),
        )
    }

    // Compile-time totality anchor: adding a `DataRole` variant breaks this
    // exhaustive match, forcing `EXPECTED_DATA_ROLE_NAMES` to be updated so a new
    // role can never be silently omitted from the load-time coverage check.
    fn _data_role_totality_anchor(role: &DataRole) -> &'static str {
        match role {
            DataRole::ValueObject { .. } => "ValueObject",
            DataRole::Entity { .. } => "Entity",
            DataRole::AggregateRoot { .. } => "AggregateRoot",
            DataRole::DomainService { .. } => "DomainService",
            DataRole::Specification => "Specification",
            DataRole::Factory => "Factory",
            DataRole::UseCase { .. } => "UseCase",
            DataRole::Interactor => "Interactor",
            DataRole::Command => "Command",
            DataRole::Query => "Query",
            DataRole::Dto => "Dto",
            DataRole::ErrorType => "ErrorType",
            DataRole::SecondaryAdapter => "SecondaryAdapter",
            DataRole::EventPolicy { .. } => "EventPolicy",
            DataRole::DomainEvent => "DomainEvent",
            DataRole::CompositionRoot => "CompositionRoot",
            DataRole::PrimaryAdapter => "PrimaryAdapter",
        }
    }

    #[test]
    fn test_minimum_rejects_zero() {
        assert_eq!(
            TestObligationMinimum::try_new(0),
            Err(ValidationError::InvalidObligationMinimum(0))
        );
    }

    #[test]
    fn test_minimum_accepts_one() {
        assert_eq!(TestObligationMinimum::try_new(1).unwrap().as_usize(), 1);
    }

    #[test]
    fn test_brief_template_rejects_blank() {
        assert_eq!(
            TestObligationBriefTemplate::try_new("  ".to_owned()),
            Err(ValidationError::EmptyString)
        );
    }

    #[test]
    fn test_rule_accessors() {
        let rule = TestObligationRule::new(
            TestObligationKind::Result,
            TestObligationPerAxis::Handles,
            Some(TestObligationMinimum::try_new(1).unwrap()),
            None,
        );
        assert_eq!(rule.kind(), &TestObligationKind::Result);
        assert_eq!(rule.per_axis(), &TestObligationPerAxis::Handles);
        assert_eq!(rule.minimum().map(TestObligationMinimum::as_usize), Some(1));
    }

    #[test]
    fn test_role_obligation_rules_empty_explicitly() {
        assert!(RoleObligationRules::new(vec![]).is_empty_explicitly());
        let non_empty = RoleObligationRules::new(vec![TestObligationRule::new(
            TestObligationKind::Boundary,
            TestObligationPerAxis::Invariant,
            None,
            None,
        )]);
        assert!(!non_empty.is_empty_explicitly());
    }

    #[test]
    fn test_data_role_totality_anchor_matches_variant_name() {
        // Exercises the compile-time totality anchor and confirms its names line
        // up with the coverage list the document constructor checks against.
        let role = DataRole::value_object();
        assert_eq!(_data_role_totality_anchor(&role), role.variant_name());
        assert!(EXPECTED_DATA_ROLE_NAMES.contains(&_data_role_totality_anchor(&role)));
    }

    #[test]
    fn test_document_accepts_complete_table() {
        let doc = complete_document().unwrap();
        assert_eq!(doc.data_roles().len(), 17);
        assert_eq!(doc.contract_roles().len(), 4);
        assert_eq!(doc.function_roles().len(), 2);
        assert_eq!(doc.patterns().len(), 1);
        assert_eq!(doc.trait_impls().len(), 4);
    }

    #[test]
    fn test_document_rejects_missing_data_role() {
        let mut data = all_data_roles();
        data.pop(); // drop PrimaryAdapter
        let result = TestObligationRulesDocument::try_new(
            data,
            all_contract_roles(),
            all_function_roles(),
            all_patterns(),
            all_contract_roles(),
        );
        match result {
            Err(TestObligationRulesLoadError::RoleNotCovered { role_name }) => {
                assert_eq!(role_name.as_str(), "PrimaryAdapter");
            }
            other => panic!("expected RoleNotCovered, got {other:?}"),
        }
    }

    #[test]
    fn test_document_rejects_missing_trait_impl_role() {
        let result = TestObligationRulesDocument::try_new(
            all_data_roles(),
            all_contract_roles(),
            all_function_roles(),
            all_patterns(),
            vec![], // trait_impls omits every ContractRole
        );
        assert!(matches!(result, Err(TestObligationRulesLoadError::RoleNotCovered { .. })));
    }

    #[test]
    fn test_document_rejects_duplicate_role() {
        let mut data = all_data_roles();
        data.push((DataRole::value_object(), empty_rules()));
        let result = TestObligationRulesDocument::try_new(
            data,
            all_contract_roles(),
            all_function_roles(),
            all_patterns(),
            all_contract_roles(),
        );
        match result {
            Err(TestObligationRulesLoadError::InvalidRuleValue { role_name, .. }) => {
                assert_eq!(role_name.as_str(), "ValueObject");
            }
            other => panic!("expected InvalidRuleValue, got {other:?}"),
        }
    }
}
