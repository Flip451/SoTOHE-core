//! In-memory implementation of the `CatalogueLinter` secondary-port.
//!
//! `InMemoryCatalogueLinter` evaluates catalogue linter rules entirely in
//! memory — no I/O, no external dependencies. It is the concrete adapter that
//! backs the S3 linter framework described in ADR
//! `knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md`
//! §S3 / IN-05 / AC-05 / CN-06.
//!
//! ## T025: v3-native migration
//!
//! As of T025 the linter accepts `&CatalogueDocument` (v3 schema) instead of
//! `&TypeCatalogueDocument`. The v3 `DataRole` / `ContractRole` / `TypeKindV2`
//! values are converted to **v2-compatible** kind-tag strings that match the
//! signal evaluator's storage keys (mirroring `v3_stub::data_role_to_kind`):
//!
//! Type entries (`types` BTreeMap) — kind derived from both `TypeKindV2` and
//! `DataRole` (structural shape takes priority):
//!
//! | `TypeKindV2` + `DataRole`                        | kind tag           |
//! |--------------------------------------------------|-------------------|
//! | `PlainStruct { typestate: Some(_), .. }` (any role) | `typestate`    |
//! | `Enum { .. }` with `ErrorType` role              | `error_type`      |
//! | `Enum { .. }` with other role                    | `enum`            |
//! | Other shape, `ValueObject`/`Entity`/`AggregateRoot`/`Specification` | `value_object` |
//! | Other shape, `DomainService`                     | `domain_service`  |
//! | Other shape, `Factory`                           | `factory`         |
//! | Other shape, `UseCase`                           | `use_case`        |
//! | Other shape, `Interactor`                        | `interactor`      |
//! | Other shape, `Command`                           | `command`         |
//! | Other shape, `Query`                             | `query`           |
//! | Other shape, `Dto`                               | `dto`             |
//! | Other shape, `ErrorType`                         | `error_type`      |
//! | Other shape, `SecondaryAdapter`                  | `secondary_adapter`|
//!
//! Trait entries (`traits` BTreeMap):
//!
//! | `ContractRole` variant    | kind tag              |
//! |--------------------------|----------------------|
//! | `SpecificationPort`      | `secondary_port`     |
//! | `ApplicationService`     | `application_service`|
//! | `SecondaryPort`          | `secondary_port`     |
//!
//! Function entries (`functions` BTreeMap):
//!
//! | `FunctionRole` variant    | kind tag              |
//! |--------------------------|----------------------|
//! | `FreeFunction`           | `free_function`      |
//! | `UseCaseFunction`        | `free_function`      |
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
//! ## Supported field names (v3 mapping)
//!
//! | `target_field`              | v3 field checked                                      |
//! |-----------------------------|------------------------------------------------------|
//! | `expected_methods`          | `TypeEntry::methods` or `TraitEntry::methods` length  |
//! | `expected_members`          | field count from `TypeKindV2` (PlainStruct / TupleStruct fields) |
//! | `expected_variants`         | variant count from `TypeKindV2::Enum::variants`       |
//!
//! `expected_members` is derived from the `TypeKindV2` variant of `TypeEntry`:
//! - `PlainStruct { fields, .. }` → `fields.len()`
//! - `TupleStruct { fields, .. }` → `fields.len()`
//! - `UnitStruct` → `0`
//! - `Enum { .. }` / `TypeAlias { .. }` → `Ok(None)` (field concept not applicable)
//!
//! `expected_variants` is derived from `TypeKindV2::Enum { variants }`:
//! - `Enum { variants }` → `variants.len()`
//! - All other kinds → `Ok(None)`
//!
//! The legacy v2 `declares_application_service` field has no v3 equivalent (the
//! v3 `TypeEntry` carries no such field): a `FieldEmpty` / `FieldNonEmpty` rule
//! targeting it is **rejected** as an unsupported `target_field`
//! (`CatalogueLinterError::InvalidRuleConfig`) rather than silently passing —
//! the linter never reports a clean result for a constraint it cannot enforce.
//!
//! ## Disabled rules opt-out (CN-06)
//!
//! Construct with [`InMemoryCatalogueLinter::with_disabled_rules`] and pass a
//! list of rule-id strings (format: `"{rule_kind:?}::{target_kind}"`, e.g.
//! `"FieldEmpty::value_object"`). Rules whose id appears in this list are
//! silently skipped, allowing opt-out for false positives.

use domain::tddd::catalogue_linter::{
    CatalogueLintViolation, CatalogueLinter, CatalogueLinterError, CatalogueLinterRule,
    CatalogueLinterRuleKind,
};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, ContractRole, DataRole, FunctionRole, TypeKindV2,
};

// ---------------------------------------------------------------------------
// Kind-tag helpers
// ---------------------------------------------------------------------------

/// Derives the v2-compatible kind tag for a v3 `TypeEntry`.
///
/// Mirrors `v3_stub::data_role_to_kind` (the canonical reference) so that lint
/// rules targeting v2 kind tags fire for v3 entries equivalently:
///
/// 1. `PlainStruct { typestate: Some(_) }` → `"typestate"`
/// 2. `Enum { .. }` with `ErrorType` role → `"error_type"`
/// 3. `Enum { .. }` with other role → `"enum"`
/// 4. All other shapes → v2-compat role mapping via `data_role_kind_tag_v2compat`
///
/// The role mapping collapses `Entity`, `AggregateRoot`, and `Specification` to
/// `"value_object"` so that `value_object` lint rules cover those entries (the v2
/// stub rendered all three as `ValueObject` shape).
fn type_entry_kind_tag(role: DataRole, kind: &TypeKindV2) -> &'static str {
    match kind {
        TypeKindV2::PlainStruct { typestate: Some(_), .. } => "typestate",
        TypeKindV2::Enum { .. } if matches!(role, DataRole::ErrorType) => "error_type",
        TypeKindV2::Enum { .. } => "enum",
        _ => data_role_kind_tag_v2compat(role),
    }
}

/// Maps a v3 `DataRole` to its v2-compatible kind tag string.
///
/// `Entity`, `AggregateRoot`, and `Specification` collapse to `"value_object"`
/// (matching `v3_stub::data_role_to_kind`). All other roles map 1-to-1.
fn data_role_kind_tag_v2compat(role: DataRole) -> &'static str {
    match role {
        DataRole::ValueObject
        | DataRole::Entity
        | DataRole::AggregateRoot
        | DataRole::Specification => "value_object",
        DataRole::DomainService => "domain_service",
        DataRole::Factory => "factory",
        DataRole::UseCase => "use_case",
        DataRole::Interactor => "interactor",
        DataRole::Command => "command",
        DataRole::Query => "query",
        DataRole::Dto => "dto",
        DataRole::ErrorType => "error_type",
        DataRole::SecondaryAdapter => "secondary_adapter",
    }
}

fn contract_role_kind_tag(role: ContractRole) -> &'static str {
    match role {
        // SpecificationPort collapses to "secondary_port" for lint-rule compatibility:
        // the v2 stub (`v3_stub::contract_role_to_kind`) mapped both SecondaryPort and
        // SpecificationPort to `TypeDefinitionKind::SecondaryPort` → kind_tag
        // "secondary_port". Existing lint rules configured for "secondary_port" must
        // continue to fire for SpecificationPort entries so rule behaviour is equivalent
        // to the v2 implementation (briefing checklist requirement).
        ContractRole::SpecificationPort | ContractRole::SecondaryPort => "secondary_port",
        ContractRole::ApplicationService => "application_service",
    }
}

fn function_role_kind_tag(role: FunctionRole) -> &'static str {
    match role {
        // All FunctionRole variants collapse to "free_function" for lint-rule
        // compatibility: `v3_stub::function_role_to_kind` and `type_signals_evaluator`
        // both map all function roles to FreeFunction → "free_function". Lint rules
        // targeting "free_function" must continue to fire for UseCaseFunction entries.
        FunctionRole::FreeFunction | FunctionRole::UseCaseFunction => "free_function",
    }
}

// ---------------------------------------------------------------------------
// Field-length bundle
// ---------------------------------------------------------------------------

/// Field-length bundle passed to [`InMemoryCatalogueLinter::apply_rule_to_entry`].
///
/// Grouping these reduces the argument count below the `clippy::too_many_arguments`
/// threshold and keeps the call sites readable.
struct EntryFieldLengths {
    /// Length of the `methods` Vec, or `None` when the `expected_methods` field
    /// concept is not applicable to the entry kind (e.g. function entries have no
    /// `methods` field — `expected_methods` rules must skip them, not fire on `0`).
    methods: Option<usize>,
    /// Field count from `TypeKindV2` (`None` when not applicable — e.g. for
    /// traits, functions, enum types, and type aliases).
    members: Option<usize>,
    /// Variant count from `TypeKindV2::Enum` (`None` when not applicable).
    variants: Option<usize>,
}

// ---------------------------------------------------------------------------
// InMemoryCatalogueLinter
// ---------------------------------------------------------------------------

/// In-memory adapter for the [`CatalogueLinter`] secondary port.
///
/// Runs catalogue lint rules entirely in memory against a
/// [`CatalogueDocument`] (v3 schema). Instantiate with
/// [`InMemoryCatalogueLinter::new`] for a fully-active linter, or with
/// [`InMemoryCatalogueLinter::with_disabled_rules`] to suppress specific rules
/// by their rule-id (see CN-06).
#[derive(Default)]
pub struct InMemoryCatalogueLinter {
    disabled_rules: Vec<String>,
}

impl InMemoryCatalogueLinter {
    /// The set of `target_field` names recognized by the `FieldEmpty` and
    /// `FieldNonEmpty` rule primitives.
    const KNOWN_TARGET_FIELDS: &'static [&'static str] =
        &["expected_methods", "expected_members", "expected_variants"];

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
                 expected_methods, expected_members, expected_variants"
            )))
        }
    }

    /// Resolve the effective field length for a `FieldEmpty` / `FieldNonEmpty` rule.
    ///
    /// Returns `Ok(Some(len))` when the field is supported for this entry kind,
    /// `Ok(None)` when the field is not applicable (skip the entry silently),
    /// or `Err` for unknown field names.
    fn field_len_for_entry(
        lengths: &EntryFieldLengths,
        field_name: &str,
    ) -> Result<Option<usize>, CatalogueLinterError> {
        match field_name {
            "expected_methods" => Ok(lengths.methods),
            "expected_members" => Ok(lengths.members),
            "expected_variants" => Ok(lengths.variants),
            // The legacy v2 `declares_application_service` field has no v3
            // equivalent — reject the rule rather than silently skipping it.
            other => Err(CatalogueLinterError::InvalidRuleConfig(format!(
                "unknown target_field: \"{other}\"; supported values are \
                 expected_methods, expected_members, expected_variants"
            ))),
        }
    }

    /// Apply a single rule to a v3 entry.
    ///
    /// Appends to `violations` when the rule fires.
    ///
    /// # Errors
    ///
    /// Returns `CatalogueLinterError::InvalidRuleConfig` for unknown `target_field`.
    fn apply_rule_to_entry(
        rule: &CatalogueLinterRule,
        entry_name: &str,
        kind_tag: &str,
        lengths: &EntryFieldLengths,
        layer_id: &str,
        violations: &mut Vec<CatalogueLintViolation>,
    ) -> Result<(), CatalogueLinterError> {
        if kind_tag != rule.target_kind() {
            return Ok(());
        }

        match rule.rule_kind() {
            CatalogueLinterRuleKind::FieldEmpty => {
                let field_name = rule.target_field().unwrap_or("");
                match Self::field_len_for_entry(lengths, field_name)? {
                    None => {}
                    Some(len) if len > 0 => {
                        violations.push(CatalogueLintViolation::new(
                            CatalogueLinterRuleKind::FieldEmpty,
                            entry_name,
                            format!(
                                "FieldEmpty rule violated: `{field_name}` must be empty \
                                 for `{kind_tag}` entries, but has {len} item(s)"
                            ),
                        ));
                    }
                    Some(_) => {}
                }
            }

            CatalogueLinterRuleKind::FieldNonEmpty => {
                let field_name = rule.target_field().unwrap_or("");
                match Self::field_len_for_entry(lengths, field_name)? {
                    None => {}
                    Some(0) => {
                        violations.push(CatalogueLintViolation::new(
                            CatalogueLinterRuleKind::FieldNonEmpty,
                            entry_name,
                            format!(
                                "FieldNonEmpty rule violated: `{field_name}` must not be \
                                 empty for `{kind_tag}` entries"
                            ),
                        ));
                    }
                    Some(_) => {}
                }
            }

            CatalogueLinterRuleKind::KindLayerConstraint => {
                if !rule.permitted_layers().contains(&layer_id.to_owned()) {
                    violations.push(CatalogueLintViolation::new(
                        CatalogueLinterRuleKind::KindLayerConstraint,
                        entry_name,
                        format!(
                            "KindLayerConstraint rule violated: `{kind_tag}` is not permitted in \
                             layer `{layer_id}`; permitted layers: [{}]",
                            rule.permitted_layers().join(", "),
                        ),
                    ));
                }
            }
        }

        Ok(())
    }
}

impl CatalogueLinter for InMemoryCatalogueLinter {
    /// Run `rules` against `catalogue` (v3 `CatalogueDocument`) for the given `layer_id`.
    ///
    /// Iterates over all `types`, `traits`, and `functions` entries in the v3
    /// `CatalogueDocument`, mapping each entry's `DataRole` / `ContractRole` /
    /// `FunctionRole` to a snake_case kind tag for rule matching.
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
        catalogue: &CatalogueDocument,
        layer_id: &str,
    ) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> {
        let mut violations: Vec<CatalogueLintViolation> = Vec::new();

        for rule in rules {
            // CN-06: skip disabled rules.
            if self.disabled_rules.contains(&Self::rule_id(rule)) {
                continue;
            }

            // Validate field-based rules once per rule, before scanning entries.
            match rule.rule_kind() {
                CatalogueLinterRuleKind::FieldEmpty | CatalogueLinterRuleKind::FieldNonEmpty => {
                    let field_name = rule.target_field().unwrap_or("");
                    Self::validate_field_rule_target_field(field_name)?;
                }
                CatalogueLinterRuleKind::KindLayerConstraint => {}
            }

            // Walk v3 type entries.
            for (type_name, type_entry) in &catalogue.types {
                // Derive the v2-compatible kind tag from both role and structural
                // kind (TypeKindV2), matching the signal evaluator's storage key.
                // This ensures typestate structs fire `typestate` rules, enums
                // fire `enum` or `error_type` rules (not `value_object`/role rules),
                // and Entity/AggregateRoot/Specification entries fire `value_object`
                // rules — mirroring the v3_stub kind mapping.
                let kind_tag = type_entry_kind_tag(type_entry.role, &type_entry.kind);
                // Derive members_len and variants_len from TypeKindV2 so that
                // `expected_members` and `expected_variants` rules can fire on v3
                // entries equivalently to the v2 linter.
                let (members_len, variants_len) = match &type_entry.kind {
                    TypeKindV2::UnitStruct => (Some(0), None),
                    TypeKindV2::PlainStruct { fields, .. } => (Some(fields.len()), None),
                    TypeKindV2::TupleStruct { fields, .. } => (Some(fields.len()), None),
                    TypeKindV2::Enum { variants } => (None, Some(variants.len())),
                    TypeKindV2::TypeAlias { .. } => (None, None),
                };
                let lengths = EntryFieldLengths {
                    methods: Some(type_entry.methods.len()),
                    members: members_len,
                    variants: variants_len,
                };
                Self::apply_rule_to_entry(
                    rule,
                    type_name.as_str(),
                    kind_tag,
                    &lengths,
                    layer_id,
                    &mut violations,
                )?;
            }

            // Walk v3 trait entries.
            // Traits have no members or variants.
            for (trait_name, trait_entry) in &catalogue.traits {
                let kind_tag = contract_role_kind_tag(trait_entry.role);
                let lengths = EntryFieldLengths {
                    methods: Some(trait_entry.methods.len()),
                    members: None,
                    variants: None,
                };
                Self::apply_rule_to_entry(
                    rule,
                    trait_name.as_str(),
                    kind_tag,
                    &lengths,
                    layer_id,
                    &mut violations,
                )?;
            }

            // Walk v3 function entries.
            // Functions have no `methods`, `members`, or `variants` fields.
            // `expected_methods` is not applicable to function entries (`None`),
            // matching the v2 behaviour where `FreeFunction` returned `Ok(None)`
            // for `expected_methods` — so rules targeting that field skip functions
            // rather than falsely firing on a zero-length methods list.
            for (fn_path, fn_entry) in &catalogue.functions {
                let kind_tag = function_role_kind_tag(fn_entry.role);
                let fn_path_str = fn_path.to_string();
                let lengths = EntryFieldLengths { methods: None, members: None, variants: None };
                Self::apply_rule_to_entry(
                    rule,
                    &fn_path_str,
                    kind_tag,
                    &lengths,
                    layer_id,
                    &mut violations,
                )?;
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
    use domain::tddd::catalogue_linter::{CatalogueLinterRule, CatalogueLinterRuleKind};
    use domain::tddd::catalogue_v2::roles::ContractRole;
    use domain::tddd::catalogue_v2::{
        CatalogueDocument, CrateName, DataRole, ItemAction, MethodDeclaration, MethodName,
        ModulePath, SelfReceiver, TraitEntry, TraitName, TypeEntry, TypeKindV2, TypeName, TypeRef,
    };
    use domain::tddd::layer_id::LayerId;

    use super::InMemoryCatalogueLinter;
    use domain::tddd::catalogue_linter::CatalogueLinter as _;

    // ------------------------------------------------------------------
    // Test fixture helpers
    // ------------------------------------------------------------------

    fn layer(name: &str) -> LayerId {
        LayerId::try_new(name.to_owned()).unwrap()
    }

    fn empty_doc(crate_name: &str) -> CatalogueDocument {
        CatalogueDocument::new(3, CrateName::new(crate_name).unwrap(), layer(crate_name))
    }

    fn value_object_entry_empty_methods() -> TypeEntry {
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
    }

    fn value_object_entry_with_methods() -> TypeEntry {
        let method = MethodDeclaration::new(
            MethodName::new("is_valid").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("bool").unwrap(),
            false,
            None,
        );
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![method],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
    }

    fn secondary_port_with_methods() -> TraitEntry {
        let method = MethodDeclaration::new(
            MethodName::new("load").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            None,
        );
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SecondaryPort,
            methods: vec![method],
            supertrait_bounds: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
    }

    fn secondary_port_no_methods() -> TraitEntry {
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SecondaryPort,
            methods: vec![],
            supertrait_bounds: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
    }

    fn domain_service_entry() -> TypeEntry {
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::DomainService,
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
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
    // FieldEmpty rule tests (TypeEntry — maps to expected_methods via methods len)
    // ------------------------------------------------------------------

    #[test]
    fn test_field_empty_rule_with_empty_methods_produces_no_violation() {
        let linter = InMemoryCatalogueLinter::new();
        let mut catalogue = empty_doc("domain");
        catalogue.types.insert(TypeName::new("Email").unwrap(), value_object_entry_empty_methods());
        let rule = field_empty_rule("value_object", "expected_methods");

        let violations = linter.run(&[rule], &catalogue, "domain").unwrap();

        assert!(violations.is_empty(), "empty methods should not fire FieldEmpty");
    }

    #[test]
    fn test_field_empty_rule_with_non_empty_methods_generates_violation() {
        let linter = InMemoryCatalogueLinter::new();
        let mut catalogue = empty_doc("domain");
        catalogue.types.insert(TypeName::new("Money").unwrap(), value_object_entry_with_methods());
        let rule = field_empty_rule("value_object", "expected_methods");

        let violations = linter.run(&[rule], &catalogue, "domain").unwrap();

        assert_eq!(violations.len(), 1, "non-empty methods must fire FieldEmpty");
        assert_eq!(violations[0].entry_name(), "Money");
        assert_eq!(violations[0].rule_kind(), &CatalogueLinterRuleKind::FieldEmpty);
    }

    // ------------------------------------------------------------------
    // FieldNonEmpty rule tests (TraitEntry — secondary_port with methods)
    // ------------------------------------------------------------------

    #[test]
    fn test_field_non_empty_rule_with_non_empty_methods_on_trait_produces_no_violation() {
        let linter = InMemoryCatalogueLinter::new();
        let mut catalogue = empty_doc("domain");
        catalogue
            .traits
            .insert(TraitName::new("UserRepository").unwrap(), secondary_port_with_methods());
        let rule = field_non_empty_rule("secondary_port", "expected_methods");

        let violations = linter.run(&[rule], &catalogue, "domain").unwrap();

        assert!(violations.is_empty(), "non-empty methods should not fire FieldNonEmpty");
    }

    #[test]
    fn test_field_non_empty_rule_with_empty_methods_on_trait_generates_violation() {
        let linter = InMemoryCatalogueLinter::new();
        let mut catalogue = empty_doc("domain");
        catalogue.traits.insert(TraitName::new("LazyPort").unwrap(), secondary_port_no_methods());
        let rule = field_non_empty_rule("secondary_port", "expected_methods");

        let violations = linter.run(&[rule], &catalogue, "domain").unwrap();

        assert_eq!(violations.len(), 1, "empty methods must fire FieldNonEmpty");
        assert_eq!(violations[0].entry_name(), "LazyPort");
        assert_eq!(violations[0].rule_kind(), &CatalogueLinterRuleKind::FieldNonEmpty);
    }

    // ------------------------------------------------------------------
    // KindLayerConstraint rule tests
    // ------------------------------------------------------------------

    #[test]
    fn test_kind_layer_constraint_with_layer_not_in_permitted_generates_violation() {
        let linter = InMemoryCatalogueLinter::new();
        let mut catalogue = empty_doc("infrastructure");
        catalogue.types.insert(TypeName::new("TransferService").unwrap(), domain_service_entry());
        let rule = kind_layer_constraint_rule("domain_service", vec!["domain"]);

        let violations = linter.run(&[rule], &catalogue, "infrastructure").unwrap();

        assert_eq!(
            violations.len(),
            1,
            "layer not in permitted_layers must fire KindLayerConstraint"
        );
        assert_eq!(violations[0].entry_name(), "TransferService");
        assert_eq!(violations[0].rule_kind(), &CatalogueLinterRuleKind::KindLayerConstraint);
        assert!(violations[0].message().contains("infrastructure"));
    }

    #[test]
    fn test_kind_layer_constraint_with_layer_in_permitted_produces_no_violation() {
        let linter = InMemoryCatalogueLinter::new();
        let mut catalogue = empty_doc("domain");
        catalogue.types.insert(TypeName::new("TransferService").unwrap(), domain_service_entry());
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
        let disabled_id = "FieldEmpty::value_object".to_owned();
        let linter = InMemoryCatalogueLinter::with_disabled_rules(vec![disabled_id]);
        let mut catalogue = empty_doc("domain");
        catalogue.types.insert(TypeName::new("Money").unwrap(), value_object_entry_with_methods());
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
        let mut catalogue = empty_doc("domain");
        catalogue.types.insert(TypeName::new("Email").unwrap(), value_object_entry_empty_methods());
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
    // expected_members / expected_variants v3 support
    // ------------------------------------------------------------------

    #[test]
    fn test_expected_members_field_non_empty_rule_fires_when_plain_struct_has_no_fields() {
        // `expected_members` is now derived from `TypeKindV2`: a `PlainStruct` with
        // no fields has `members_len = Some(0)`, so a `FieldNonEmpty` rule fires.
        let linter = InMemoryCatalogueLinter::new();
        let mut catalogue = empty_doc("domain");
        catalogue
            .types
            .insert(TypeName::new("MyType").unwrap(), value_object_entry_empty_methods());
        // PlainStruct { fields: [] } → members_len = Some(0) → FieldNonEmpty fires.
        let rule = field_non_empty_rule("value_object", "expected_members");

        let violations = linter.run(&[rule], &catalogue, "domain").unwrap();

        assert_eq!(
            violations.len(),
            1,
            "FieldNonEmpty on expected_members must fire when PlainStruct has no fields: {violations:?}"
        );
        assert_eq!(violations[0].entry_name(), "MyType");
    }

    #[test]
    fn test_expected_members_field_empty_rule_fires_when_plain_struct_has_fields() {
        use domain::tddd::catalogue_v2::identifiers::FieldName;
        use domain::tddd::catalogue_v2::variants::FieldDecl;

        // A `PlainStruct` with one named field gives `members_len = Some(1)`,
        // so a `FieldEmpty` rule fires.
        let linter = InMemoryCatalogueLinter::new();
        let mut catalogue = empty_doc("domain");
        let field = FieldDecl::new(FieldName::new("inner").unwrap(), TypeRef::new("u64").unwrap());
        let entry = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![field],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        catalogue.types.insert(TypeName::new("MyType").unwrap(), entry);
        let rule = field_empty_rule("value_object", "expected_members");

        let violations = linter.run(&[rule], &catalogue, "domain").unwrap();

        assert_eq!(
            violations.len(),
            1,
            "FieldEmpty on expected_members must fire when PlainStruct has fields: {violations:?}"
        );
        assert_eq!(violations[0].entry_name(), "MyType");
    }

    #[test]
    fn test_expected_variants_field_non_empty_rule_fires_when_enum_has_no_variants() {
        // An `Enum { variants: [] }` gives `variants_len = Some(0)`,
        // so a `FieldNonEmpty` rule fires.
        let linter = InMemoryCatalogueLinter::new();
        let mut catalogue = empty_doc("domain");
        let entry = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ErrorType,
            kind: TypeKindV2::Enum { variants: vec![] },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        catalogue.types.insert(TypeName::new("MyError").unwrap(), entry);
        let rule = field_non_empty_rule("error_type", "expected_variants");

        let violations = linter.run(&[rule], &catalogue, "domain").unwrap();

        assert_eq!(
            violations.len(),
            1,
            "FieldNonEmpty on expected_variants must fire when Enum has no variants: {violations:?}"
        );
        assert_eq!(violations[0].entry_name(), "MyError");
    }

    #[test]
    fn test_declares_application_service_field_is_rejected_as_unsupported() {
        // The legacy v2 `declares_application_service` field has no v3 equivalent.
        // A rule targeting it is rejected as an unsupported `target_field`
        // (InvalidRuleConfig) rather than silently passing — the linter never
        // reports a clean result for a constraint it cannot enforce.
        let linter = InMemoryCatalogueLinter::new();
        let mut catalogue = empty_doc("domain");
        catalogue
            .types
            .insert(TypeName::new("MyType").unwrap(), value_object_entry_empty_methods());
        let rule = field_non_empty_rule("value_object", "declares_application_service");

        let result = linter.run(&[rule], &catalogue, "domain");

        match result {
            Err(domain::tddd::catalogue_linter::CatalogueLinterError::InvalidRuleConfig(msg)) => {
                assert!(
                    msg.contains("declares_application_service"),
                    "error must name the unsupported field: {msg}"
                );
            }
            other => panic!(
                "declares_application_service rule must return InvalidRuleConfig, got {other:?}"
            ),
        }
    }

    // ------------------------------------------------------------------
    // Multiple rules combined
    // ------------------------------------------------------------------

    #[test]
    fn test_multiple_rules_combined_collect_all_violations() {
        let linter = InMemoryCatalogueLinter::new();

        let mut catalogue = empty_doc("infrastructure");
        // value_object with non-empty methods → fires FieldEmpty
        catalogue.types.insert(TypeName::new("Money").unwrap(), value_object_entry_with_methods());
        // secondary_port (trait) with no methods → fires FieldNonEmpty
        catalogue.traits.insert(TraitName::new("LazyPort").unwrap(), secondary_port_no_methods());
        // domain_service in infrastructure layer → fires KindLayerConstraint
        catalogue.types.insert(TypeName::new("InfraService").unwrap(), domain_service_entry());

        let rules = vec![
            field_empty_rule("value_object", "expected_methods"),
            field_non_empty_rule("secondary_port", "expected_methods"),
            kind_layer_constraint_rule("domain_service", vec!["domain"]),
        ];

        let violations = linter.run(&rules, &catalogue, "infrastructure").unwrap();

        assert_eq!(violations.len(), 3, "all three rules must fire, collecting 3 violations");

        let entry_names: Vec<&str> = violations.iter().map(|v| v.entry_name()).collect();
        assert!(entry_names.contains(&"Money"), "Money must appear in violations");
        assert!(entry_names.contains(&"LazyPort"), "LazyPort must appear in violations");
        assert!(entry_names.contains(&"InfraService"), "InfraService must appear in violations");
    }
}
