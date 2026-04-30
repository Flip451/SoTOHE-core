//! In-memory implementation of the `CatalogueLinter` secondary-port.
//!
//! `InMemoryCatalogueLinter` evaluates catalogue linter rules entirely in
//! memory — no I/O, no external dependencies. It is the concrete adapter that
//! backs the S3 linter framework described in ADR
//! `knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md`
//! §S3 / IN-05 / AC-05 / CN-06.
//!
//! ## Rule primitives
//!
//! - `FieldEmpty` — violation when the named field is **non-empty** for entries
//!   of `target_kind`.
//! - `FieldNonEmpty` — violation when the named field is **empty** for entries
//!   of `target_kind`.
//! - `KindLayerConstraint` — violation when the injected `layer_id` is **not**
//!   in `permitted_layers` for entries of `target_kind`.
//!
//! ## Disabled rules opt-out (CN-06)
//!
//! Construct with [`InMemoryCatalogueLinter::with_disabled_rules`] and pass a
//! list of rule-id strings (format: `"{rule_kind:?}::{target_kind}"`, e.g.
//! `"FieldEmpty::value_object"`). Rules whose id appears in this list are
//! silently skipped, allowing opt-out for false positives.
//!
//! ## Supported field names
//!
//! The `FieldEmpty` / `FieldNonEmpty` primitives recognise the following
//! `target_field` values:
//!
//! | `target_field` | Applicable `target_kind`(s) |
//! |---|---|
//! | `expected_methods` | All struct-based kinds (typestate, value_object, use_case, interactor, dto, command, query, factory, secondary_adapter, domain_service, secondary_port, application_service) |
//! | `expected_members` | typestate, value_object, use_case, interactor, dto, command, query, factory, secondary_adapter, domain_service |
//! | `expected_variants` | enum, error_type |
//! | `declares_application_service` | interactor |
//!
//! Any other `target_field` name triggers
//! [`CatalogueLinterError::InvalidRuleConfig`].

use domain::TypeCatalogueDocument;
use domain::tddd::catalogue::TypeDefinitionKind;
use domain::tddd::catalogue_linter::{
    CatalogueLintViolation, CatalogueLinter, CatalogueLinterError, CatalogueLinterRule,
    CatalogueLinterRuleKind,
};

// ---------------------------------------------------------------------------
// InMemoryCatalogueLinter
// ---------------------------------------------------------------------------

/// In-memory adapter for the [`CatalogueLinter`] secondary port.
///
/// Runs catalogue lint rules entirely in memory against a
/// [`TypeCatalogueDocument`]. Instantiate with [`InMemoryCatalogueLinter::new`]
/// for a fully-active linter, or with
/// [`InMemoryCatalogueLinter::with_disabled_rules`] to suppress specific rules
/// by their rule-id (see CN-06).
pub struct InMemoryCatalogueLinter {
    disabled_rules: Vec<String>,
}

impl InMemoryCatalogueLinter {
    /// The set of `target_field` names recognized by the `FieldEmpty` and
    /// `FieldNonEmpty` rule primitives.
    const KNOWN_TARGET_FIELDS: &'static [&'static str] = &[
        "expected_methods",
        "expected_members",
        "expected_variants",
        "declares_application_service",
    ];

    /// Creates a new `InMemoryCatalogueLinter` with no disabled rules.
    #[must_use]
    pub fn new() -> Self {
        Self { disabled_rules: Vec::new() }
    }

    /// Creates a new `InMemoryCatalogueLinter` with the given disabled rule ids.
    ///
    /// Rule ids have the format `"{rule_kind:?}::{target_kind}"`, for example
    /// `"FieldEmpty::value_object"`. Rules whose id appears in
    /// `disabled_rules` are silently skipped during [`CatalogueLinter::run`].
    #[must_use]
    pub fn with_disabled_rules(disabled_rules: Vec<String>) -> Self {
        Self { disabled_rules }
    }

    /// Computes the rule-id string for a given rule.
    ///
    /// Format: `"{rule_kind:?}::{target_kind}"`.
    fn rule_id(rule: &CatalogueLinterRule) -> String {
        format!("{:?}::{}", rule.rule_kind(), rule.target_kind())
    }

    /// Validates that the `target_field` of a `FieldEmpty` or `FieldNonEmpty`
    /// rule is a known field name, independently of whether any catalogue
    /// entries match `target_kind`.
    ///
    /// This eager check ensures that an invalid `target_field` always returns
    /// `Err(CatalogueLinterError::InvalidRuleConfig)` even when the catalogue
    /// contains no entries of the targeted kind.
    fn validate_field_rule_target_field(field_name: &str) -> Result<(), CatalogueLinterError> {
        if Self::KNOWN_TARGET_FIELDS.contains(&field_name) {
            Ok(())
        } else {
            Err(CatalogueLinterError::InvalidRuleConfig(format!(
                "unknown target_field: \"{field_name}\"; supported values are \
                 expected_methods, expected_members, expected_variants, declares_application_service"
            )))
        }
    }

    /// Extracts the length of the named field from a `TypeDefinitionKind`.
    ///
    /// Returns:
    /// - `Ok(Some(len))` — field found; `len` is the number of elements.
    /// - `Ok(None)` — the entry kind does not carry this field at all (field
    ///   not applicable to this kind — skip the entry rather than error).
    /// - `Err(...)` — the field name is unknown across all kinds; this
    ///   indicates an invalid rule configuration.
    fn field_len(
        kind: &TypeDefinitionKind,
        field_name: &str,
    ) -> Result<Option<usize>, CatalogueLinterError> {
        match field_name {
            "expected_methods" => {
                let len = match kind {
                    TypeDefinitionKind::Typestate { expected_methods, .. } => {
                        expected_methods.len()
                    }
                    TypeDefinitionKind::ValueObject { expected_methods, .. } => {
                        expected_methods.len()
                    }
                    TypeDefinitionKind::SecondaryPort { expected_methods } => {
                        expected_methods.len()
                    }
                    TypeDefinitionKind::ApplicationService { expected_methods } => {
                        expected_methods.len()
                    }
                    TypeDefinitionKind::UseCase { expected_methods, .. } => expected_methods.len(),
                    TypeDefinitionKind::Interactor { expected_methods, .. } => {
                        expected_methods.len()
                    }
                    TypeDefinitionKind::Dto { expected_methods, .. } => expected_methods.len(),
                    TypeDefinitionKind::Command { expected_methods, .. } => expected_methods.len(),
                    TypeDefinitionKind::Query { expected_methods, .. } => expected_methods.len(),
                    TypeDefinitionKind::Factory { expected_methods, .. } => expected_methods.len(),
                    TypeDefinitionKind::SecondaryAdapter { expected_methods, .. } => {
                        expected_methods.len()
                    }
                    TypeDefinitionKind::DomainService { expected_methods, .. } => {
                        expected_methods.len()
                    }
                    // Kinds that do not carry expected_methods — skip entry.
                    TypeDefinitionKind::Enum { .. }
                    | TypeDefinitionKind::ErrorType { .. }
                    | TypeDefinitionKind::FreeFunction { .. } => return Ok(None),
                };
                Ok(Some(len))
            }

            "expected_members" => {
                let len = match kind {
                    TypeDefinitionKind::Typestate { expected_members, .. } => {
                        expected_members.len()
                    }
                    TypeDefinitionKind::ValueObject { expected_members, .. } => {
                        expected_members.len()
                    }
                    TypeDefinitionKind::UseCase { expected_members, .. } => expected_members.len(),
                    TypeDefinitionKind::Interactor { expected_members, .. } => {
                        expected_members.len()
                    }
                    TypeDefinitionKind::Dto { expected_members, .. } => expected_members.len(),
                    TypeDefinitionKind::Command { expected_members, .. } => expected_members.len(),
                    TypeDefinitionKind::Query { expected_members, .. } => expected_members.len(),
                    TypeDefinitionKind::Factory { expected_members, .. } => expected_members.len(),
                    TypeDefinitionKind::SecondaryAdapter { expected_members, .. } => {
                        expected_members.len()
                    }
                    TypeDefinitionKind::DomainService { expected_members, .. } => {
                        expected_members.len()
                    }
                    // Kinds that do not carry expected_members — skip entry.
                    TypeDefinitionKind::SecondaryPort { .. }
                    | TypeDefinitionKind::ApplicationService { .. }
                    | TypeDefinitionKind::Enum { .. }
                    | TypeDefinitionKind::ErrorType { .. }
                    | TypeDefinitionKind::FreeFunction { .. } => return Ok(None),
                };
                Ok(Some(len))
            }

            "expected_variants" => {
                let len = match kind {
                    TypeDefinitionKind::Enum { expected_variants } => expected_variants.len(),
                    TypeDefinitionKind::ErrorType { expected_variants } => expected_variants.len(),
                    // Kinds that do not carry expected_variants — skip entry.
                    _ => return Ok(None),
                };
                Ok(Some(len))
            }

            "declares_application_service" => {
                let len = match kind {
                    TypeDefinitionKind::Interactor { declares_application_service, .. } => {
                        declares_application_service.len()
                    }
                    // Kinds that do not carry declares_application_service — skip entry.
                    _ => return Ok(None),
                };
                Ok(Some(len))
            }

            other => Err(CatalogueLinterError::InvalidRuleConfig(format!(
                "unknown target_field: \"{other}\"; supported values are \
                 expected_methods, expected_members, expected_variants, declares_application_service"
            ))),
        }
    }
}

impl Default for InMemoryCatalogueLinter {
    fn default() -> Self {
        Self::new()
    }
}

impl CatalogueLinter for InMemoryCatalogueLinter {
    /// Run `rules` against `catalogue` for the given `layer_id`.
    ///
    /// Returns the full list of violations found. An empty `Vec` means no
    /// rules fired.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogueLinterError::InvalidRuleConfig`] if a `FieldEmpty` or
    /// `FieldNonEmpty` rule references an unknown `target_field` name.
    fn run(
        &self,
        rules: &[CatalogueLinterRule],
        catalogue: &TypeCatalogueDocument,
        layer_id: &str,
    ) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> {
        let mut violations: Vec<CatalogueLintViolation> = Vec::new();

        for rule in rules {
            // CN-06: skip disabled rules.
            if self.disabled_rules.contains(&Self::rule_id(rule)) {
                continue;
            }

            // Validate field-based rules once per rule, before scanning entries.
            // This guarantees InvalidRuleConfig is returned for unknown
            // target_field values even when no catalogue entries match
            // target_kind (so the per-entry check would never run).
            match rule.rule_kind() {
                CatalogueLinterRuleKind::FieldEmpty | CatalogueLinterRuleKind::FieldNonEmpty => {
                    let field_name = rule.target_field().unwrap_or("");
                    Self::validate_field_rule_target_field(field_name)?;
                }
                CatalogueLinterRuleKind::KindLayerConstraint => {
                    // No target_field to validate for layer constraint rules.
                }
            }

            for entry in catalogue.entries() {
                // Only process entries whose kind matches the rule's target_kind.
                if entry.kind().kind_tag() != rule.target_kind() {
                    continue;
                }

                match rule.rule_kind() {
                    CatalogueLinterRuleKind::FieldEmpty => {
                        // target_field is validated to be Some (non-empty) by try_new.
                        let field_name = rule.target_field().unwrap_or("");
                        match Self::field_len(entry.kind(), field_name)? {
                            None => {
                                // Field not applicable to this kind — skip.
                            }
                            Some(len) if len > 0 => {
                                // Field is non-empty — rule fires.
                                violations.push(CatalogueLintViolation::new(
                                    CatalogueLinterRuleKind::FieldEmpty,
                                    entry.name(),
                                    format!(
                                        "FieldEmpty rule violated: `{field_name}` must be empty \
                                         for `{}` entries, but has {len} item(s)",
                                        rule.target_kind(),
                                    ),
                                ));
                            }
                            Some(_) => {
                                // Field is empty — rule satisfied.
                            }
                        }
                    }

                    CatalogueLinterRuleKind::FieldNonEmpty => {
                        let field_name = rule.target_field().unwrap_or("");
                        match Self::field_len(entry.kind(), field_name)? {
                            None => {
                                // Field not applicable to this kind — skip.
                            }
                            Some(0) => {
                                // Field is empty — rule fires.
                                violations.push(CatalogueLintViolation::new(
                                    CatalogueLinterRuleKind::FieldNonEmpty,
                                    entry.name(),
                                    format!(
                                        "FieldNonEmpty rule violated: `{field_name}` must not be \
                                         empty for `{}` entries",
                                        rule.target_kind(),
                                    ),
                                ));
                            }
                            Some(_) => {
                                // Field is non-empty — rule satisfied.
                            }
                        }
                    }

                    CatalogueLinterRuleKind::KindLayerConstraint => {
                        if !rule.permitted_layers().contains(&layer_id.to_owned()) {
                            violations.push(CatalogueLintViolation::new(
                                CatalogueLinterRuleKind::KindLayerConstraint,
                                entry.name(),
                                format!(
                                    "KindLayerConstraint rule violated: `{}` is not permitted in \
                                     layer `{layer_id}`; permitted layers: [{}]",
                                    rule.target_kind(),
                                    rule.permitted_layers().join(", "),
                                ),
                            ));
                        }
                    }
                }
            }
        }

        Ok(violations)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use domain::tddd::catalogue::{
        MemberDeclaration, MethodDeclaration, TypeAction, TypeCatalogueDocument,
        TypeCatalogueEntry, TypeDefinitionKind,
    };
    use domain::tddd::catalogue_linter::{CatalogueLinterRule, CatalogueLinterRuleKind};

    use super::InMemoryCatalogueLinter;
    use domain::tddd::catalogue_linter::CatalogueLinter as _;

    // ------------------------------------------------------------------
    // Test fixture helpers
    // ------------------------------------------------------------------

    fn value_object_entry_empty_methods(name: &str) -> TypeCatalogueEntry {
        TypeCatalogueEntry::new(
            name,
            "A value object with no methods",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap()
    }

    fn value_object_entry_with_methods(name: &str) -> TypeCatalogueEntry {
        let method =
            MethodDeclaration::new("is_valid", Some("&self".into()), vec![], "bool", false);
        TypeCatalogueEntry::new(
            name,
            "A value object with a behavioral method",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: vec![method],
            },
            TypeAction::Add,
            true,
        )
        .unwrap()
    }

    fn interactor_entry_with_app_service(name: &str) -> TypeCatalogueEntry {
        TypeCatalogueEntry::new(
            name,
            "Interactor implementing an ApplicationService",
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: vec!["SaveTrackApplicationService".into()],
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap()
    }

    fn interactor_entry_no_app_service(name: &str) -> TypeCatalogueEntry {
        TypeCatalogueEntry::new(
            name,
            "Interactor with no declared ApplicationService",
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap()
    }

    fn domain_service_entry(name: &str) -> TypeCatalogueEntry {
        TypeCatalogueEntry::new(
            name,
            "A domain service",
            TypeDefinitionKind::DomainService {
                expected_members: vec![MemberDeclaration::field("repo", "UserRepository")],
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap()
    }

    fn field_empty_rule(target_kind: &str, target_field: &str) -> CatalogueLinterRule {
        CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::FieldEmpty,
            target_kind,
            Some(target_field.to_owned()),
            vec![],
        )
        .unwrap()
    }

    fn field_non_empty_rule(target_kind: &str, target_field: &str) -> CatalogueLinterRule {
        CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::FieldNonEmpty,
            target_kind,
            Some(target_field.to_owned()),
            vec![],
        )
        .unwrap()
    }

    fn kind_layer_constraint_rule(
        target_kind: &str,
        permitted_layers: Vec<&str>,
    ) -> CatalogueLinterRule {
        CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::KindLayerConstraint,
            target_kind,
            None,
            permitted_layers.into_iter().map(String::from).collect(),
        )
        .unwrap()
    }

    // ------------------------------------------------------------------
    // FieldEmpty rule tests
    // ------------------------------------------------------------------

    #[test]
    fn test_field_empty_rule_with_empty_expected_methods_produces_no_violation() {
        let linter = InMemoryCatalogueLinter::new();
        let entry = value_object_entry_empty_methods("Email");
        let catalogue = TypeCatalogueDocument::new(1, vec![entry]);
        let rule = field_empty_rule("value_object", "expected_methods");

        let violations = linter.run(&[rule], &catalogue, "domain").unwrap();

        assert!(violations.is_empty(), "empty expected_methods should not fire FieldEmpty");
    }

    #[test]
    fn test_field_empty_rule_with_non_empty_expected_methods_generates_violation() {
        let linter = InMemoryCatalogueLinter::new();
        let entry = value_object_entry_with_methods("Money");
        let catalogue = TypeCatalogueDocument::new(1, vec![entry]);
        let rule = field_empty_rule("value_object", "expected_methods");

        let violations = linter.run(&[rule], &catalogue, "domain").unwrap();

        assert_eq!(violations.len(), 1, "non-empty expected_methods must fire FieldEmpty");
        assert_eq!(violations[0].entry_name(), "Money");
        assert_eq!(violations[0].rule_kind(), &CatalogueLinterRuleKind::FieldEmpty);
    }

    // ------------------------------------------------------------------
    // FieldNonEmpty rule tests
    // ------------------------------------------------------------------

    #[test]
    fn test_field_non_empty_rule_with_non_empty_field_produces_no_violation() {
        let linter = InMemoryCatalogueLinter::new();
        let entry = interactor_entry_with_app_service("SaveTrackInteractor");
        let catalogue = TypeCatalogueDocument::new(1, vec![entry]);
        let rule = field_non_empty_rule("interactor", "declares_application_service");

        let violations = linter.run(&[rule], &catalogue, "usecase").unwrap();

        assert!(
            violations.is_empty(),
            "non-empty declares_application_service should not fire FieldNonEmpty"
        );
    }

    #[test]
    fn test_field_non_empty_rule_with_empty_field_generates_violation() {
        let linter = InMemoryCatalogueLinter::new();
        let entry = interactor_entry_no_app_service("LazyInteractor");
        let catalogue = TypeCatalogueDocument::new(1, vec![entry]);
        let rule = field_non_empty_rule("interactor", "declares_application_service");

        let violations = linter.run(&[rule], &catalogue, "usecase").unwrap();

        assert_eq!(
            violations.len(),
            1,
            "empty declares_application_service must fire FieldNonEmpty"
        );
        assert_eq!(violations[0].entry_name(), "LazyInteractor");
        assert_eq!(violations[0].rule_kind(), &CatalogueLinterRuleKind::FieldNonEmpty);
    }

    // ------------------------------------------------------------------
    // KindLayerConstraint rule tests
    // ------------------------------------------------------------------

    #[test]
    fn test_kind_layer_constraint_with_layer_not_in_permitted_generates_violation() {
        let linter = InMemoryCatalogueLinter::new();
        let entry = domain_service_entry("TransferService");
        let catalogue = TypeCatalogueDocument::new(1, vec![entry]);
        let rule = kind_layer_constraint_rule("domain_service", vec!["domain"]);

        let violations = linter.run(&[rule], &catalogue, "usecase").unwrap();

        assert_eq!(
            violations.len(),
            1,
            "layer not in permitted_layers must fire KindLayerConstraint"
        );
        assert_eq!(violations[0].entry_name(), "TransferService");
        assert_eq!(violations[0].rule_kind(), &CatalogueLinterRuleKind::KindLayerConstraint);
        assert!(violations[0].message().contains("usecase"));
    }

    #[test]
    fn test_kind_layer_constraint_with_layer_in_permitted_produces_no_violation() {
        let linter = InMemoryCatalogueLinter::new();
        let entry = domain_service_entry("TransferService");
        let catalogue = TypeCatalogueDocument::new(1, vec![entry]);
        let rule = kind_layer_constraint_rule("domain_service", vec!["domain"]);

        let violations = linter.run(&[rule], &catalogue, "domain").unwrap();

        assert!(
            violations.is_empty(),
            "layer in permitted_layers must not fire KindLayerConstraint"
        );
    }

    // ------------------------------------------------------------------
    // disabled_rules opt-out (CN-06)
    // ------------------------------------------------------------------

    #[test]
    fn test_disabled_rules_skips_matching_rule() {
        // A rule that would normally fire is skipped because its id is disabled.
        let disabled_id = "FieldEmpty::value_object".to_owned();
        let linter = InMemoryCatalogueLinter::with_disabled_rules(vec![disabled_id]);
        let entry = value_object_entry_with_methods("Money");
        let catalogue = TypeCatalogueDocument::new(1, vec![entry]);
        let rule = field_empty_rule("value_object", "expected_methods");

        let violations = linter.run(&[rule], &catalogue, "domain").unwrap();

        assert!(
            violations.is_empty(),
            "disabled rule must not produce a violation even when it would normally fire"
        );
    }

    // ------------------------------------------------------------------
    // InvalidRuleConfig error path
    // ------------------------------------------------------------------

    #[test]
    fn test_unknown_target_field_returns_invalid_rule_config_error() {
        let linter = InMemoryCatalogueLinter::new();
        let entry = value_object_entry_empty_methods("Email");
        let catalogue = TypeCatalogueDocument::new(1, vec![entry]);
        let rule = field_empty_rule("value_object", "nonexistent_field");

        let result = linter.run(&[rule], &catalogue, "domain");

        assert!(
            matches!(
                result,
                Err(domain::tddd::catalogue_linter::CatalogueLinterError::InvalidRuleConfig(_))
            ),
            "unknown target_field must return InvalidRuleConfig"
        );
    }

    // ------------------------------------------------------------------
    // Multiple rules combined
    // ------------------------------------------------------------------

    #[test]
    fn test_multiple_rules_combined_collect_all_violations() {
        let linter = InMemoryCatalogueLinter::new();

        // Entry 1: value_object with non-empty expected_methods → fires FieldEmpty
        let vo_with_methods = value_object_entry_with_methods("Money");
        // Entry 2: interactor with empty declares_application_service → fires FieldNonEmpty
        let interactor_no_service = interactor_entry_no_app_service("BadInteractor");
        // Entry 3: domain_service in infrastructure layer → fires KindLayerConstraint
        let domain_svc = domain_service_entry("InfraService");

        let catalogue =
            TypeCatalogueDocument::new(1, vec![vo_with_methods, interactor_no_service, domain_svc]);

        let rules = vec![
            field_empty_rule("value_object", "expected_methods"),
            field_non_empty_rule("interactor", "declares_application_service"),
            kind_layer_constraint_rule("domain_service", vec!["domain"]),
        ];

        let violations = linter.run(&rules, &catalogue, "infrastructure").unwrap();

        assert_eq!(violations.len(), 3, "all three rules must fire, collecting 3 violations");

        let entry_names: Vec<&str> = violations.iter().map(|v| v.entry_name()).collect();
        assert!(entry_names.contains(&"Money"), "Money must appear in violations");
        assert!(entry_names.contains(&"BadInteractor"), "BadInteractor must appear in violations");
        assert!(entry_names.contains(&"InfraService"), "InfraService must appear in violations");
    }
}
