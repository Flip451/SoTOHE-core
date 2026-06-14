//! Catalogue linter — S3 linter framework (Stage 3 framework bundle).
//!
//! Defines the expanded rule vocabulary, violation value object, and the
//! `evaluate_catalogue_lint` pure free-function entry point described in ADR
//! `knowledge/adr/2026-05-25-0000-tddd-pattern-semantics-extension.md`
//! §D15 / D17.
//!
//! ## Design overview
//!
//! - `RoleKind` — payload-free discriminant covering `DataRole` /
//!   `ContractRole` variants for use in rule targeting.
//! - `RuleTarget` — selector that specifies which role(s) a rule applies to.
//! - `CatalogueLinterRuleKind` — 12-variant enum of rule categories (D15).
//! - `CatalogueLinterRule` — value object with `target: RuleTarget` and
//!   `kind: CatalogueLinterRuleKind`; constructed via `CatalogueLinterRule::new`.
//! - `CatalogueLinterRuleError` — error type for constructor rejections.
//! - `CatalogueLintViolation` — value object produced when a rule fires.
//! - `CatalogueLinterError` — error type for `evaluate_catalogue_lint` failures.
//! - `evaluate_catalogue_lint` — pure free-function entry point (D17).
//!
//! The former `CatalogueLinter` trait (secondary port) has been removed (D17):
//! pure evaluation logic belongs in the domain core, not infrastructure.
//!
//! No `serde` derives are attached here — ADR
//! `knowledge/adr/2026-04-14-1531-…` forbids serde inside `libs/domain`;
//! codec / serde support lives in the infrastructure codec.

use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::roles::{ContractRole, DataRole, NonEmptyVec};
use crate::tddd::layer_id::LayerId;

// ---------------------------------------------------------------------------
// RoleKind — payload-free role discriminant
// ---------------------------------------------------------------------------

/// Payload-free discriminant that covers every `DataRole` and `ContractRole`
/// variant (D15 / D17).
///
/// Used in [`RuleTarget`] and in rule kind payloads such as
/// [`CatalogueLinterRuleKind::NoRoleInMethodSignature`] where the rule must
/// reference a role across both `DataRole` and `ContractRole` (e.g.
/// `RoleKind::Repository` is a `ContractRole` variant, not a `DataRole`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoleKind {
    // --- DataRole variants (15) ---
    /// `DataRole::ValueObject`
    ValueObject,
    /// `DataRole::Entity`
    Entity,
    /// `DataRole::AggregateRoot`
    AggregateRoot,
    /// `DataRole::DomainService`
    DomainService,
    /// `DataRole::Specification`
    Specification,
    /// `DataRole::Factory`
    Factory,
    /// `DataRole::UseCase`
    UseCase,
    /// `DataRole::Interactor`
    Interactor,
    /// `DataRole::Command`
    Command,
    /// `DataRole::Query`
    Query,
    /// `DataRole::Dto`
    Dto,
    /// `DataRole::ErrorType`
    ErrorType,
    /// `DataRole::SecondaryAdapter`
    SecondaryAdapter,
    /// `DataRole::EventPolicy`
    EventPolicy,
    /// `DataRole::DomainEvent`
    DomainEvent,
    // --- ContractRole variants (4) ---
    /// `ContractRole::SpecificationPort`
    SpecificationPort,
    /// `ContractRole::ApplicationService`
    ApplicationService,
    /// `ContractRole::SecondaryPort`
    SecondaryPort,
    /// `ContractRole::Repository`
    Repository,
}

impl RoleKind {
    /// Returns the payload-free discriminant for a `DataRole`.
    #[must_use]
    pub fn from_data_role(role: &DataRole) -> Self {
        match role {
            DataRole::ValueObject { .. } => Self::ValueObject,
            DataRole::Entity { .. } => Self::Entity,
            DataRole::AggregateRoot { .. } => Self::AggregateRoot,
            DataRole::DomainService { .. } => Self::DomainService,
            DataRole::Specification => Self::Specification,
            DataRole::Factory => Self::Factory,
            DataRole::UseCase { .. } => Self::UseCase,
            DataRole::Interactor => Self::Interactor,
            DataRole::Command => Self::Command,
            DataRole::Query => Self::Query,
            DataRole::Dto => Self::Dto,
            DataRole::ErrorType => Self::ErrorType,
            DataRole::SecondaryAdapter => Self::SecondaryAdapter,
            DataRole::EventPolicy { .. } => Self::EventPolicy,
            DataRole::DomainEvent => Self::DomainEvent,
        }
    }

    /// Returns the payload-free discriminant for a `ContractRole`.
    #[must_use]
    pub fn from_contract_role(role: &ContractRole) -> Self {
        match role {
            ContractRole::SpecificationPort => Self::SpecificationPort,
            ContractRole::ApplicationService => Self::ApplicationService,
            ContractRole::SecondaryPort => Self::SecondaryPort,
            ContractRole::Repository { .. } => Self::Repository,
        }
    }

    /// Returns a stable display name for this discriminant.
    #[must_use]
    pub fn variant_name(self) -> &'static str {
        match self {
            Self::ValueObject => "ValueObject",
            Self::Entity => "Entity",
            Self::AggregateRoot => "AggregateRoot",
            Self::DomainService => "DomainService",
            Self::Specification => "Specification",
            Self::Factory => "Factory",
            Self::UseCase => "UseCase",
            Self::Interactor => "Interactor",
            Self::Command => "Command",
            Self::Query => "Query",
            Self::Dto => "Dto",
            Self::ErrorType => "ErrorType",
            Self::SecondaryAdapter => "SecondaryAdapter",
            Self::EventPolicy => "EventPolicy",
            Self::DomainEvent => "DomainEvent",
            Self::SpecificationPort => "SpecificationPort",
            Self::ApplicationService => "ApplicationService",
            Self::SecondaryPort => "SecondaryPort",
            Self::Repository => "Repository",
        }
    }
}

// ---------------------------------------------------------------------------
// RuleTarget — rule application target selector
// ---------------------------------------------------------------------------

/// Selector that specifies which role(s) a linter rule applies to.
///
/// An empty `target_roles` vector means the rule applies to **all** roles.
/// The caller supplies the vector; `RuleTarget::new` always succeeds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleTarget {
    target_roles: Vec<RoleKind>,
}

impl RuleTarget {
    /// Creates a new `RuleTarget` from the given role list.
    ///
    /// An empty `target_roles` means "apply to all roles".
    #[must_use]
    pub fn new(target_roles: Vec<RoleKind>) -> Self {
        Self { target_roles }
    }

    /// Creates a `RuleTarget` that matches all roles.
    #[must_use]
    pub fn all_roles() -> Self {
        Self::new(vec![])
    }

    /// Returns the target roles. An empty slice means "all roles".
    #[must_use]
    pub fn target_roles(&self) -> &[RoleKind] {
        &self.target_roles
    }

    /// Returns `true` if the given `RoleKind` is in scope for this target.
    #[must_use]
    pub fn matches(&self, role: RoleKind) -> bool {
        self.target_roles.is_empty() || self.target_roles.contains(&role)
    }
}

// ---------------------------------------------------------------------------
// CatalogueLinterRuleKind — 12-variant rule category enum
// ---------------------------------------------------------------------------

/// Classifies what invariant a catalogue linter rule asserts (D15).
///
/// 12 variants: 11 data-carrying + 1 unit (`NoPublicField`).
///
/// Payloads use `String` / `Vec<String>` / `Vec<RoleKind>` at the domain
/// level to stay serde-free. Codec layer converts JSON strings to these types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogueLinterRuleKind {
    /// Rule asserts that the named field must be **empty** for matching entries.
    FieldEmpty {
        /// Name of the catalogue field to check (e.g. `"expected_methods"`).
        target_field: String,
    },

    /// Rule asserts that the named field must be **non-empty** for matching
    /// entries.
    FieldNonEmpty {
        /// Name of the catalogue field to check (e.g. `"expected_methods"`).
        target_field: String,
    },

    /// Rule constrains which layers entries of the target role may appear in.
    KindLayerConstraint {
        /// Layer IDs where the target role is permitted.
        permitted_layers: NonEmptyVec<LayerId>,
    },

    /// Rule asserts that the typed entries in `target_field` are declared with
    /// `expected_role` in the catalogue.
    ReferencedRoleConstraint {
        /// Field whose `TypeRef` entries are checked (e.g. `"emits"`).
        target_field: String,
        /// The role that each referenced type must declare.
        expected_role: RoleKind,
    },

    /// Rule asserts that `trait_impls` contains all of `required_traits`.
    TraitImplRequired {
        /// Traits whose impl declarations are required (e.g. `"PartialEq"`).
        required_traits: NonEmptyVec<String>,
    },

    /// Rule asserts that no method signature contains a type with a forbidden
    /// role.
    NoRoleInMethodSignature {
        /// Roles that must not appear in any method parameter or return type.
        forbidden_roles: NonEmptyVec<RoleKind>,
    },

    /// Rule asserts that the method referenced by `target_field` exists in the
    /// entry's public method set and satisfies the expected signature.
    MethodReferenceSignature {
        /// Field whose value is the referenced method name (e.g.
        /// `"invariants"`, `"identity"`).
        target_field: String,
    },

    /// Rule asserts that the entry has a public accessor getter matching the
    /// identity signature (`&self`, no params, non-unit return).
    AccessorSignatureRequired {
        /// Field that names the accessor (e.g. `"identity"`).
        target_field: String,
    },

    /// Rule asserts that elements in `target_field` are unique across all
    /// entries of the target role (e.g. no two `AggregateRoot` share the same
    /// `exclusive_members` entry).
    FieldElementUniqueAcrossEntries {
        /// Field to check for cross-entry uniqueness (e.g.
        /// `"exclusive_members"`).
        target_field: String,
    },

    /// Rule asserts that elements listed in `target_field` do not appear in
    /// any other entry's method signatures (external reference prohibition).
    NoExternalReferenceInMethods {
        /// Field whose listed types must not appear in other entries' method
        /// signatures (e.g. `"exclusive_members"`).
        target_field: String,
    },

    /// Rule asserts that the entry has no public struct fields
    /// (`StructShape::Plain` / `StructShape::Tuple`). Unit variant.
    NoPublicField,

    /// Rule asserts that no method uses the given self-receiver kind.
    ForbiddenMethodReceiver {
        /// The receiver kind to forbid (e.g. `"&mut self"`).
        forbidden_receiver: String,
    },
}

impl CatalogueLinterRuleKind {
    /// Returns the discriminant name for display / violation reporting.
    #[must_use]
    pub fn discriminant_name(&self) -> &'static str {
        match self {
            Self::FieldEmpty { .. } => "FieldEmpty",
            Self::FieldNonEmpty { .. } => "FieldNonEmpty",
            Self::KindLayerConstraint { .. } => "KindLayerConstraint",
            Self::ReferencedRoleConstraint { .. } => "ReferencedRoleConstraint",
            Self::TraitImplRequired { .. } => "TraitImplRequired",
            Self::NoRoleInMethodSignature { .. } => "NoRoleInMethodSignature",
            Self::MethodReferenceSignature { .. } => "MethodReferenceSignature",
            Self::AccessorSignatureRequired { .. } => "AccessorSignatureRequired",
            Self::FieldElementUniqueAcrossEntries { .. } => "FieldElementUniqueAcrossEntries",
            Self::NoExternalReferenceInMethods { .. } => "NoExternalReferenceInMethods",
            Self::NoPublicField => "NoPublicField",
            Self::ForbiddenMethodReceiver { .. } => "ForbiddenMethodReceiver",
        }
    }
}

// ---------------------------------------------------------------------------
// CatalogueLinterRuleError — error type for constructor rejections
// ---------------------------------------------------------------------------

/// Errors that can be produced when constructing or validating a
/// [`CatalogueLinterRule`] or its constituent rule kinds.
///
/// [`CatalogueLinterRule::new`] returns [`Self::EmptyTargetField`] for any
/// `kind` that carries a required string payload (`target_field` or
/// `forbidden_receiver`) when that payload is empty.
///
/// The `EmptyPermittedLayers`, `EmptyRequiredTraits`, and `EmptyForbiddenRoles`
/// variants are not returned by `CatalogueLinterRule::new` itself, because
/// `KindLayerConstraint`, `TraitImplRequired`, and `NoRoleInMethodSignature`
/// carry `NonEmptyVec` payloads validated at variant-construction time.
/// These three variants exist for codec-layer conversions that need to signal
/// validation failures for the corresponding rule kinds.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CatalogueLinterRuleError {
    /// `target_field` is empty for a rule kind that requires it.
    #[error("target_field must not be empty")]
    EmptyTargetField,

    /// `permitted_layers` is empty for a `KindLayerConstraint` rule.
    /// Not returned by [`CatalogueLinterRule::new`] — reserved for
    /// codec-layer conversions.
    #[error("permitted_layers must not be empty for KindLayerConstraint rules")]
    EmptyPermittedLayers,

    /// `required_traits` is empty for a `TraitImplRequired` rule.
    /// Not returned by [`CatalogueLinterRule::new`] — reserved for
    /// codec-layer conversions.
    #[error("required_traits must not be empty for TraitImplRequired rules")]
    EmptyRequiredTraits,

    /// `forbidden_roles` is empty for a `NoRoleInMethodSignature` rule.
    /// Not returned by [`CatalogueLinterRule::new`] — reserved for
    /// codec-layer conversions.
    #[error("forbidden_roles must not be empty for NoRoleInMethodSignature rules")]
    EmptyForbiddenRoles,
}

// ---------------------------------------------------------------------------
// CatalogueLinterRule — value object
// ---------------------------------------------------------------------------

/// A single catalogue linter rule.
///
/// Constructed via [`CatalogueLinterRule::new`], which rejects ill-formed
/// combinations (e.g. empty `target_field` for `FieldEmpty` rules).
///
/// All fields are private; read access is via accessor methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueLinterRule {
    target: RuleTarget,
    kind: CatalogueLinterRuleKind,
}

impl CatalogueLinterRule {
    /// Creates a new `CatalogueLinterRule`.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogueLinterRuleError::EmptyTargetField`] when `kind` is
    /// `FieldEmpty`, `FieldNonEmpty`, `ReferencedRoleConstraint`,
    /// `MethodReferenceSignature`, `AccessorSignatureRequired`,
    /// `FieldElementUniqueAcrossEntries`, `NoExternalReferenceInMethods`, or
    /// `ForbiddenMethodReceiver` and the associated string payload is empty.
    ///
    /// For the remaining `kind` variants (`KindLayerConstraint`,
    /// `TraitImplRequired`, `NoRoleInMethodSignature`, `NoPublicField`) this
    /// function always succeeds, because their payloads are either unit or backed
    /// by `NonEmptyVec` (validated at variant-construction time).
    pub fn new(
        target: RuleTarget,
        kind: CatalogueLinterRuleKind,
    ) -> Result<Self, CatalogueLinterRuleError> {
        // Validate per-kind invariants.
        match &kind {
            CatalogueLinterRuleKind::FieldEmpty { target_field }
            | CatalogueLinterRuleKind::FieldNonEmpty { target_field }
            | CatalogueLinterRuleKind::MethodReferenceSignature { target_field }
            | CatalogueLinterRuleKind::AccessorSignatureRequired { target_field }
            | CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries { target_field }
            | CatalogueLinterRuleKind::NoExternalReferenceInMethods { target_field } => {
                if target_field.is_empty() {
                    return Err(CatalogueLinterRuleError::EmptyTargetField);
                }
            }
            CatalogueLinterRuleKind::ReferencedRoleConstraint { target_field, .. } => {
                if target_field.is_empty() {
                    return Err(CatalogueLinterRuleError::EmptyTargetField);
                }
            }
            CatalogueLinterRuleKind::ForbiddenMethodReceiver { forbidden_receiver } => {
                if forbidden_receiver.is_empty() {
                    return Err(CatalogueLinterRuleError::EmptyTargetField);
                }
            }
            // KindLayerConstraint — permitted_layers is NonEmptyVec, already validated
            // by the caller when constructing the variant.
            CatalogueLinterRuleKind::KindLayerConstraint { .. } => {}
            // TraitImplRequired — required_traits is NonEmptyVec, already validated.
            CatalogueLinterRuleKind::TraitImplRequired { .. } => {}
            // NoRoleInMethodSignature — forbidden_roles is NonEmptyVec, already validated.
            CatalogueLinterRuleKind::NoRoleInMethodSignature { .. } => {}
            // NoPublicField has no extra invariants — it is a unit variant.
            CatalogueLinterRuleKind::NoPublicField => {}
        }
        Ok(Self { target, kind })
    }

    /// Returns the rule target selector.
    #[must_use]
    pub fn target(&self) -> &RuleTarget {
        &self.target
    }

    /// Returns the rule kind.
    #[must_use]
    pub fn kind(&self) -> &CatalogueLinterRuleKind {
        &self.kind
    }
}

// ---------------------------------------------------------------------------
// CatalogueLintViolation — value object produced when a rule fires
// ---------------------------------------------------------------------------

/// A single violation produced when a catalogue linter rule fires against an
/// entry.
///
/// All fields are private; read access is via accessor methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueLintViolation {
    rule_kind: &'static str,
    entry_name: String,
    message: String,
}

impl CatalogueLintViolation {
    /// Creates a new `CatalogueLintViolation`.
    ///
    /// `rule_kind` is the discriminant name from
    /// [`CatalogueLinterRuleKind::discriminant_name`].
    ///
    /// All three parameters are required; no validation is performed because
    /// violations are constructed only by a trusted linter implementation.
    #[must_use]
    pub fn new(
        rule_kind: &'static str,
        entry_name: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self { rule_kind, entry_name: entry_name.into(), message: message.into() }
    }

    /// Returns the discriminant name of the rule kind that generated this
    /// violation.
    #[must_use]
    pub fn rule_kind(&self) -> &'static str {
        self.rule_kind
    }

    /// Returns the catalogue entry name that triggered the violation.
    #[must_use]
    pub fn entry_name(&self) -> &str {
        &self.entry_name
    }

    /// Returns the human-readable violation message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

// ---------------------------------------------------------------------------
// CatalogueLinterError — error type for evaluate_catalogue_lint failures
// ---------------------------------------------------------------------------

/// Errors returned by [`evaluate_catalogue_lint`].
#[derive(Debug, thiserror::Error)]
pub enum CatalogueLinterError {
    /// The linter rule configuration is invalid and prevents execution.
    #[error("invalid linter rule configuration: {0}")]
    InvalidRuleConfig(String),
}

// ---------------------------------------------------------------------------
// evaluate_catalogue_lint — pure free-function entry point (D17 / T008)
// ---------------------------------------------------------------------------

/// Evaluate `rules` against `catalogue` for the given `layer_id`.
///
/// Returns the full list of violations found. An empty `Vec` means no rules
/// fired.
///
/// This is the pure domain-layer entry point (D17): no I/O, no trait object,
/// no infrastructure dependency. The per-rule evaluation logic is deferred
/// to T014; this skeleton establishes the callable surface and the exhaustive
/// match over all 12 `CatalogueLinterRuleKind` variants.
///
/// # Errors
///
/// Returns [`CatalogueLinterError::InvalidRuleConfig`] if the provided rule
/// configuration is internally inconsistent and prevents execution.
pub fn evaluate_catalogue_lint(
    rules: &[CatalogueLinterRule],
    catalogue: &CatalogueDocument,
    layer_id: &LayerId,
) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> {
    let violations: Vec<CatalogueLintViolation> = Vec::new();
    for rule in rules {
        // TODO(T014): implement per-kind evaluation logic.
        match rule.kind() {
            CatalogueLinterRuleKind::FieldEmpty { .. } => {}
            CatalogueLinterRuleKind::FieldNonEmpty { .. } => {}
            CatalogueLinterRuleKind::KindLayerConstraint { .. } => {}
            CatalogueLinterRuleKind::ReferencedRoleConstraint { .. } => {}
            CatalogueLinterRuleKind::TraitImplRequired { .. } => {}
            CatalogueLinterRuleKind::NoRoleInMethodSignature { .. } => {}
            CatalogueLinterRuleKind::MethodReferenceSignature { .. } => {}
            CatalogueLinterRuleKind::AccessorSignatureRequired { .. } => {}
            CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries { .. } => {}
            CatalogueLinterRuleKind::NoExternalReferenceInMethods { .. } => {}
            CatalogueLinterRuleKind::NoPublicField => {}
            CatalogueLinterRuleKind::ForbiddenMethodReceiver { .. } => {}
        }
    }
    // Suppress unused-variable warnings until T014 fills in logic.
    let _ = catalogue;
    let _ = layer_id;
    Ok(violations)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use crate::tddd::catalogue_v2::roles::NonEmptyVec;
    use crate::tddd::layer_id::LayerId;

    fn layer(name: &str) -> LayerId {
        LayerId::try_new(name.to_owned()).unwrap()
    }

    // ------------------------------------------------------------------
    // RoleKind — from_data_role covers all 15 DataRole variants
    // ------------------------------------------------------------------

    #[test]
    fn test_role_kind_from_data_role_covers_all_variants() {
        use crate::tddd::catalogue_v2::identifiers::TypeRef;
        let cases: Vec<(&str, RoleKind)> = vec![
            ("ValueObject", RoleKind::ValueObject),
            ("Entity", RoleKind::Entity),
            ("AggregateRoot", RoleKind::AggregateRoot),
            ("DomainService", RoleKind::DomainService),
            ("Specification", RoleKind::Specification),
            ("Factory", RoleKind::Factory),
            ("UseCase", RoleKind::UseCase),
            ("Interactor", RoleKind::Interactor),
            ("Command", RoleKind::Command),
            ("Query", RoleKind::Query),
            ("Dto", RoleKind::Dto),
            ("ErrorType", RoleKind::ErrorType),
            ("SecondaryAdapter", RoleKind::SecondaryAdapter),
            ("EventPolicy", RoleKind::EventPolicy),
            ("DomainEvent", RoleKind::DomainEvent),
        ];
        assert_eq!(cases.len(), 15, "must cover all 15 DataRole variants");
        for (name, expected) in &cases {
            let role: crate::tddd::catalogue_v2::roles::DataRole = name.parse().unwrap();
            assert_eq!(
                RoleKind::from_data_role(&role),
                *expected,
                "from_data_role mismatch for {name}"
            );
        }
        // EventPolicy needs a TypeRef to parse via FromStr, verify separately:
        let event_ref = TypeRef::new("OrderPlaced").unwrap();
        let ep = crate::tddd::catalogue_v2::roles::DataRole::EventPolicy {
            reacts_to: NonEmptyVec::new(event_ref, vec![]),
        };
        assert_eq!(RoleKind::from_data_role(&ep), RoleKind::EventPolicy);
    }

    #[test]
    fn test_role_kind_from_contract_role_covers_all_variants() {
        use crate::tddd::catalogue_v2::identifiers::TypeRef;
        let agg = TypeRef::new("Order").unwrap();
        let cases = vec![
            (ContractRole::SpecificationPort, RoleKind::SpecificationPort),
            (ContractRole::ApplicationService, RoleKind::ApplicationService),
            (ContractRole::SecondaryPort, RoleKind::SecondaryPort),
            (ContractRole::Repository { aggregate: agg }, RoleKind::Repository),
        ];
        assert_eq!(cases.len(), 4, "must cover all 4 ContractRole variants");
        for (role, expected) in &cases {
            assert_eq!(
                RoleKind::from_contract_role(role),
                *expected,
                "from_contract_role mismatch for {:?}",
                role.variant_name()
            );
        }
    }

    // ------------------------------------------------------------------
    // CatalogueLinterRuleKind — 12 variants exist and discriminant_name works
    // ------------------------------------------------------------------

    #[test]
    fn test_catalogue_linter_rule_kind_has_12_variants_with_distinct_names() {
        let permitted = NonEmptyVec::new(layer("domain"), vec![]);
        let required_traits = NonEmptyVec::new("PartialEq".to_owned(), vec![]);
        let forbidden_roles = NonEmptyVec::new(RoleKind::Repository, vec![]);

        let kinds = vec![
            CatalogueLinterRuleKind::FieldEmpty { target_field: "f".to_owned() },
            CatalogueLinterRuleKind::FieldNonEmpty { target_field: "f".to_owned() },
            CatalogueLinterRuleKind::KindLayerConstraint { permitted_layers: permitted },
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "emits".to_owned(),
                expected_role: RoleKind::DomainEvent,
            },
            CatalogueLinterRuleKind::TraitImplRequired { required_traits },
            CatalogueLinterRuleKind::NoRoleInMethodSignature { forbidden_roles },
            CatalogueLinterRuleKind::MethodReferenceSignature {
                target_field: "invariants".to_owned(),
            },
            CatalogueLinterRuleKind::AccessorSignatureRequired {
                target_field: "identity".to_owned(),
            },
            CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries {
                target_field: "exclusive_members".to_owned(),
            },
            CatalogueLinterRuleKind::NoExternalReferenceInMethods {
                target_field: "exclusive_members".to_owned(),
            },
            CatalogueLinterRuleKind::NoPublicField,
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: "&mut self".to_owned(),
            },
        ];
        assert_eq!(kinds.len(), 12, "must have exactly 12 variants");

        let names: Vec<&str> = kinds.iter().map(|k| k.discriminant_name()).collect();
        let expected = [
            "FieldEmpty",
            "FieldNonEmpty",
            "KindLayerConstraint",
            "ReferencedRoleConstraint",
            "TraitImplRequired",
            "NoRoleInMethodSignature",
            "MethodReferenceSignature",
            "AccessorSignatureRequired",
            "FieldElementUniqueAcrossEntries",
            "NoExternalReferenceInMethods",
            "NoPublicField",
            "ForbiddenMethodReceiver",
        ];
        assert_eq!(names, expected);
    }

    // ------------------------------------------------------------------
    // CatalogueLinterRule::new — happy path and rejection cases
    // ------------------------------------------------------------------

    #[test]
    fn test_linter_rule_new_field_empty_succeeds_with_valid_target_field() {
        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::FieldEmpty { target_field: "expected_methods".to_owned() },
        )
        .unwrap();
        assert_eq!(rule.kind().discriminant_name(), "FieldEmpty");
        assert!(rule.target().target_roles().is_empty(), "all_roles target should be empty vec");
    }

    #[test]
    fn test_linter_rule_new_field_empty_rejects_empty_target_field() {
        let result = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::FieldEmpty { target_field: String::new() },
        );
        assert_eq!(result, Err(CatalogueLinterRuleError::EmptyTargetField));
    }

    #[test]
    fn test_linter_rule_new_field_non_empty_rejects_empty_target_field() {
        let result = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::FieldNonEmpty { target_field: String::new() },
        );
        assert_eq!(result, Err(CatalogueLinterRuleError::EmptyTargetField));
    }

    #[test]
    fn test_linter_rule_new_kind_layer_constraint_succeeds() {
        let permitted = NonEmptyVec::new(layer("domain"), vec![]);
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::EventPolicy]),
            CatalogueLinterRuleKind::KindLayerConstraint { permitted_layers: permitted },
        )
        .unwrap();
        assert_eq!(rule.kind().discriminant_name(), "KindLayerConstraint");
        assert_eq!(rule.target().target_roles(), &[RoleKind::EventPolicy]);
    }

    #[test]
    fn test_linter_rule_new_no_public_field_succeeds_as_unit_variant() {
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::DomainEvent, RoleKind::ValueObject]),
            CatalogueLinterRuleKind::NoPublicField,
        )
        .unwrap();
        assert_eq!(rule.kind().discriminant_name(), "NoPublicField");
    }

    #[test]
    fn test_linter_rule_new_referenced_role_constraint_rejects_empty_target_field() {
        let result = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: String::new(),
                expected_role: RoleKind::DomainEvent,
            },
        );
        assert_eq!(result, Err(CatalogueLinterRuleError::EmptyTargetField));
    }

    #[test]
    fn test_linter_rule_new_forbidden_method_receiver_succeeds() {
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::DomainEvent]),
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: "&mut self".to_owned(),
            },
        )
        .unwrap();
        assert_eq!(rule.kind().discriminant_name(), "ForbiddenMethodReceiver");
    }

    #[test]
    fn test_linter_rule_new_forbidden_method_receiver_with_empty_string_returns_error() {
        // An empty forbidden_receiver is invalid — it would create a silent no-op rule
        // because no receiver can match the empty string.
        let result = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::DomainEvent]),
            CatalogueLinterRuleKind::ForbiddenMethodReceiver { forbidden_receiver: String::new() },
        );
        assert_eq!(result, Err(CatalogueLinterRuleError::EmptyTargetField));
    }

    #[test]
    fn test_linter_rule_new_trait_impl_required_succeeds_with_non_empty_vec() {
        // NonEmptyVec enforces non-emptiness at construction; CatalogueLinterRule::new
        // always succeeds for TraitImplRequired.
        let required_traits = NonEmptyVec::new("PartialEq".to_owned(), vec!["Eq".to_owned()]);
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::TraitImplRequired { required_traits },
        )
        .unwrap();
        assert_eq!(rule.kind().discriminant_name(), "TraitImplRequired");
    }

    #[test]
    fn test_linter_rule_new_no_role_in_method_signature_succeeds_with_non_empty_vec() {
        // NonEmptyVec enforces non-emptiness at construction; CatalogueLinterRule::new
        // always succeeds for NoRoleInMethodSignature.
        let forbidden_roles = NonEmptyVec::new(RoleKind::Repository, vec![RoleKind::SecondaryPort]);
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::ValueObject, RoleKind::Entity]),
            CatalogueLinterRuleKind::NoRoleInMethodSignature { forbidden_roles },
        )
        .unwrap();
        assert_eq!(rule.kind().discriminant_name(), "NoRoleInMethodSignature");
    }

    // ------------------------------------------------------------------
    // CatalogueLintViolation — constructor + accessor methods
    // ------------------------------------------------------------------

    #[test]
    fn test_catalogue_lint_violation_constructor_and_accessors() {
        let violation = CatalogueLintViolation::new(
            "FieldEmpty",
            "MyValueObject",
            "expected_methods must be empty for value_object entries",
        );
        assert_eq!(violation.rule_kind(), "FieldEmpty");
        assert_eq!(violation.entry_name(), "MyValueObject");
        assert_eq!(violation.message(), "expected_methods must be empty for value_object entries");
    }

    // ------------------------------------------------------------------
    // evaluate_catalogue_lint — skeleton returns empty violations
    // ------------------------------------------------------------------

    #[test]
    fn test_evaluate_catalogue_lint_skeleton_returns_empty_violations() {
        use crate::tddd::catalogue_v2::document::CatalogueDocument;
        use crate::tddd::catalogue_v2::identifiers::CrateName;

        let doc = CatalogueDocument::new(3, CrateName::new("domain").unwrap(), layer("domain"));
        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::NoPublicField,
        )
        .unwrap();
        let layer_id = layer("domain");
        let violations = evaluate_catalogue_lint(&[rule], &doc, &layer_id).unwrap();
        assert!(violations.is_empty(), "T008 skeleton must return empty violations");
    }

    // ------------------------------------------------------------------
    // CatalogueLinterError::InvalidRuleConfig — constructor test
    // ------------------------------------------------------------------

    #[test]
    fn test_catalogue_linter_error_invalid_rule_config_stores_message() {
        let err = CatalogueLinterError::InvalidRuleConfig("contradictory rule set".to_owned());
        assert!(err.to_string().contains("contradictory rule set"));
    }

    // ------------------------------------------------------------------
    // RuleTarget::matches
    // ------------------------------------------------------------------

    #[test]
    fn test_rule_target_all_roles_matches_any_role_kind() {
        let target = RuleTarget::all_roles();
        assert!(target.matches(RoleKind::ValueObject));
        assert!(target.matches(RoleKind::Repository));
        assert!(target.matches(RoleKind::DomainEvent));
    }

    #[test]
    fn test_rule_target_specific_roles_matches_only_listed_roles() {
        let target = RuleTarget::new(vec![RoleKind::ValueObject, RoleKind::Entity]);
        assert!(target.matches(RoleKind::ValueObject));
        assert!(target.matches(RoleKind::Entity));
        assert!(!target.matches(RoleKind::AggregateRoot));
        assert!(!target.matches(RoleKind::Repository));
    }
}
