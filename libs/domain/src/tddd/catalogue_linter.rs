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

use crate::tddd::catalogue_v2::roles::{ContractRole, DataRole, NonEmptyVec, SelfReceiver};
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
    /// Every role discriminant that a rule target can name.
    pub(crate) const ALL: [Self; 19] = [
        Self::ValueObject,
        Self::Entity,
        Self::AggregateRoot,
        Self::DomainService,
        Self::Specification,
        Self::Factory,
        Self::UseCase,
        Self::Interactor,
        Self::Command,
        Self::Query,
        Self::Dto,
        Self::ErrorType,
        Self::SecondaryAdapter,
        Self::EventPolicy,
        Self::DomainEvent,
        Self::SpecificationPort,
        Self::ApplicationService,
        Self::SecondaryPort,
        Self::Repository,
    ];

    /// All `DataRole` discriminants that a type-entry field rule can scan.
    pub(crate) const DATA_ROLES: [Self; 15] = [
        Self::ValueObject,
        Self::Entity,
        Self::AggregateRoot,
        Self::DomainService,
        Self::Specification,
        Self::Factory,
        Self::UseCase,
        Self::Interactor,
        Self::Command,
        Self::Query,
        Self::Dto,
        Self::ErrorType,
        Self::SecondaryAdapter,
        Self::EventPolicy,
        Self::DomainEvent,
    ];

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

    /// Returns `true` when this discriminant carries the named `TypeRef` field.
    ///
    /// Used by the `ReferencedRoleConstraint` pre-check to reject
    /// `target_role × target_field` combinations that cannot produce any role
    /// reference checks (D19 fail-closed).
    ///
    /// `pub(crate)` — internal helper; not part of the public API surface.
    #[must_use]
    pub(crate) fn carries_type_ref_field(self, field: &str) -> bool {
        match field {
            "exclusive_members" | "shared_value_objects" => matches!(self, Self::AggregateRoot),
            "emits" => matches!(self, Self::AggregateRoot | Self::DomainService),
            "handles" => matches!(self, Self::UseCase),
            "reacts_to" => matches!(self, Self::EventPolicy),
            "aggregate" => matches!(self, Self::Repository),
            _ => false,
        }
    }

    /// Returns `true` when this discriminant carries the named `DataRole` field.
    ///
    /// Covers both `TypeRef` fields (delegating to [`carries_type_ref_field`])
    /// and `InvariantDecl` fields (`"invariants"`).
    ///
    /// Used by `FieldEmpty` / `FieldNonEmpty` pre-checks to reject
    /// `target_role × target_field` combinations where any target role does not
    /// carry the field (D19 fail-closed).  Both rules iterate type entries and
    /// inspect a field on each entry's `DataRole`; a role that never carries
    /// the field would silently treat every entry as having an empty vec,
    /// producing false positives.
    ///
    /// `pub(crate)` — internal helper; not part of the public API surface.
    #[must_use]
    pub(crate) fn carries_data_role_field(self, field: &str) -> bool {
        match field {
            "invariants" => {
                matches!(self, Self::ValueObject | Self::Entity | Self::AggregateRoot)
            }
            other => self.carries_type_ref_field(other),
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

    /// The rule configuration is internally inconsistent (e.g. an unsupported
    /// `forbidden_receiver` string that cannot be parsed as a canonical
    /// `SelfReceiver` form).
    #[error("invalid rule configuration: {0}")]
    InvalidRuleConfig(String),
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
    /// Returns [`CatalogueLinterRuleError::InvalidRuleConfig`] when `kind` is
    /// `ForbiddenMethodReceiver` and `forbidden_receiver` is not a canonical
    /// `SelfReceiver` form (`"self"`, `"&self"`, `"&mut self"`).
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
                // Validate that `forbidden_receiver` is a canonical SelfReceiver
                // rendered form: "self", "&self", or "&mut self".  Any other value
                // (e.g. typos like "&mutself", "self mut") would cause the rule to
                // never fire against any entry — a silent disable (D19 fail-closed).
                if forbidden_receiver.parse::<SelfReceiver>().is_err() {
                    return Err(CatalogueLinterRuleError::InvalidRuleConfig(format!(
                        "unsupported forbidden_receiver '{forbidden_receiver}'; \
                         expected one of 'self', '&self', '&mut self'"
                    )));
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

    /// The `all_catalogues` map does not contain an entry for the requested
    /// `target_layer_id`.
    #[error("unknown target layer '{layer_id}': not found in all_catalogues")]
    UnknownLayer { layer_id: String },
}

// ---------------------------------------------------------------------------
// Internal helper functions and evaluation logic (split into submodules)
// ---------------------------------------------------------------------------

#[path = "catalogue_linter_helpers.rs"]
mod helpers;

#[path = "catalogue_linter_eval.rs"]
mod eval;

/// Re-export so that consumers of `catalogue_linter` see `evaluate_catalogue_lint`
/// at the expected path without knowing about the `eval` submodule.
pub use eval::evaluate_catalogue_lint;

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
    fn test_forbidden_method_receiver_rejects_unsupported_string() {
        // A typo like "&mutself" is not a canonical SelfReceiver rendered form.
        // The rule must be rejected at construction time so the evaluator never
        // runs with a receiver string that can never match any entry (D19
        // fail-closed).
        let result = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::DomainEvent]),
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: "&mutself".to_owned(),
            },
        );
        assert!(
            matches!(
                &result,
                Err(CatalogueLinterRuleError::InvalidRuleConfig(msg))
                    if msg.contains("&mutself")
            ),
            "expected InvalidRuleConfig for unsupported receiver, got: {result:?}"
        );
    }

    #[test]
    fn test_forbidden_method_receiver_accepts_canonical_forms() {
        // All three canonical SelfReceiver forms must be accepted.
        for canonical in &["self", "&self", "&mut self"] {
            let result = CatalogueLinterRule::new(
                RuleTarget::new(vec![RoleKind::DomainEvent]),
                CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                    forbidden_receiver: (*canonical).to_owned(),
                },
            );
            assert!(
                result.is_ok(),
                "expected Ok for canonical receiver '{canonical}', got: {result:?}"
            );
        }
    }

    #[test]
    fn test_field_non_empty_rejects_target_role_that_does_not_carry_field() {
        // Entity does not carry "emits"; targeting it with FieldNonEmpty "emits"
        // would produce a false positive for every Entity entry (the field is
        // always empty for that role).  The evaluator must return InvalidRuleConfig
        // rather than silently treating every Entity as a violation (D19 fail-closed).
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("MyEntity").unwrap(),
            make_type_entry(DataRole::entity().unwrap()),
        );
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::Entity]),
            CatalogueLinterRuleKind::FieldNonEmpty { target_field: "emits".to_owned() },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer);
        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.contains("emits") && msg.contains("Entity")
            ),
            "expected InvalidRuleConfig for Entity × emits FieldNonEmpty, got: {result:?}"
        );
    }

    #[test]
    fn test_field_empty_rejects_target_role_that_does_not_carry_field() {
        // UseCase does not carry "emits"; targeting it with FieldEmpty "emits"
        // would silently pass for every UseCase entry (the field is always empty
        // for that role, so the rule trivially fires false negatives).  The
        // evaluator must return InvalidRuleConfig (D19 fail-closed).
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("MyUseCase").unwrap(),
            make_type_entry(DataRole::UseCase { handles: vec![] }),
        );
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::UseCase]),
            CatalogueLinterRuleKind::FieldEmpty { target_field: "emits".to_owned() },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer);
        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.contains("emits") && msg.contains("UseCase")
            ),
            "expected InvalidRuleConfig for UseCase × emits FieldEmpty, got: {result:?}"
        );
    }

    #[test]
    fn test_field_non_empty_rejects_all_roles_target_when_some_roles_do_not_carry_field() {
        // An empty RuleTarget means all roles. FieldNonEmpty "emits" must not be
        // accepted because many DataRole variants never carry "emits" and would
        // be false positives if those entries were present.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("MyService").unwrap(),
            make_type_entry(DataRole::DomainService {
                emits: vec![TypeRef::new("OrderPlaced").unwrap()],
            }),
        );
        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::FieldNonEmpty { target_field: "emits".to_owned() },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer);
        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.contains("emits") && msg.contains("all DataRole roles")
            ),
            "expected InvalidRuleConfig for all roles × emits FieldNonEmpty, got: {result:?}"
        );
    }

    #[test]
    fn test_field_empty_rejects_all_roles_target_when_some_roles_do_not_carry_field() {
        // An empty RuleTarget means all roles. FieldEmpty "emits" must not be
        // accepted because roles that never carry "emits" would silently pass.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("MyService").unwrap(),
            make_type_entry(DataRole::DomainService { emits: vec![] }),
        );
        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::FieldEmpty { target_field: "emits".to_owned() },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer);
        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.contains("emits") && msg.contains("all DataRole roles")
            ),
            "expected InvalidRuleConfig for all roles × emits FieldEmpty, got: {result:?}"
        );
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
        use std::collections::BTreeMap;

        use crate::tddd::catalogue_v2::document::CatalogueDocument;
        use crate::tddd::catalogue_v2::identifiers::CrateName;

        let layer_id = layer("domain");
        let doc = CatalogueDocument::new(3, CrateName::new("domain").unwrap(), layer_id.clone());
        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::NoPublicField,
        )
        .unwrap();
        let mut all = BTreeMap::new();
        all.insert(layer_id.clone(), doc);
        let violations = evaluate_catalogue_lint(&[rule], &all, &layer_id).unwrap();
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

    // ===========================================================================
    // T016: Test fixture helpers
    // ===========================================================================

    use std::collections::BTreeMap;

    use crate::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
    use crate::tddd::catalogue_v2::document::CatalogueDocument;
    use crate::tddd::catalogue_v2::entries::{InherentImplDeclV2, TraitEntry, TypeEntry};
    use crate::tddd::catalogue_v2::identifiers::{
        CrateName, FieldName, InvariantName, MethodName, ModulePath, ParamName, TraitName,
        TypeName, TypeRef,
    };
    use crate::tddd::catalogue_v2::methods::{MethodDeclaration, ParamDeclaration};
    use crate::tddd::catalogue_v2::roles::{
        ContractRole, DataRole, IdentityAccessor, InvariantDecl, InvariantPredicate, ItemAction,
        SelfReceiver,
    };
    use crate::tddd::catalogue_v2::traits::TraitImplDeclV2;
    use crate::tddd::catalogue_v2::variants::FieldDecl;

    fn make_doc(layer_name: &str) -> CatalogueDocument {
        CatalogueDocument::new(3, CrateName::new("domain").unwrap(), layer(layer_name))
    }

    /// Wrap a single `CatalogueDocument` in a `BTreeMap` keyed by its layer and
    /// call `evaluate_catalogue_lint`. Helper for single-layer tests.
    fn all_catalogues_single(doc: &CatalogueDocument) -> BTreeMap<LayerId, CatalogueDocument> {
        let mut map = BTreeMap::new();
        map.insert(doc.layer.clone(), doc.clone());
        map
    }

    fn plain_struct_kind(fields: Vec<FieldDecl>) -> TypeKindV2 {
        TypeKindV2::Struct(StructKind::new(
            StructShape::Plain { fields, has_stripped_fields: false },
            None,
        ))
    }

    fn unit_struct_kind() -> TypeKindV2 {
        TypeKindV2::Struct(StructKind::new(StructShape::Unit, None))
    }

    fn make_type_entry(role: DataRole) -> TypeEntry {
        make_type_entry_with_kind(role, unit_struct_kind())
    }

    fn make_type_entry_with_kind(role: DataRole, kind: TypeKindV2) -> TypeEntry {
        TypeEntry {
            action: ItemAction::Add,
            role,
            kind,
            methods: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
    }

    fn make_type_entry_with_methods(role: DataRole, methods: Vec<MethodDeclaration>) -> TypeEntry {
        TypeEntry {
            action: ItemAction::Add,
            role,
            kind: unit_struct_kind(),
            methods,
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
    }

    fn make_trait_entry(role: ContractRole) -> TraitEntry {
        TraitEntry {
            action: ItemAction::Add,
            role,
            methods: vec![],
            assoc_types: vec![],
            assoc_consts: vec![],
            supertrait_bounds: vec![],
            generics: vec![],
            where_predicates: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
    }

    fn method_ref_no_params(
        name: &str,
        returns: &str,
        receiver: SelfReceiver,
    ) -> MethodDeclaration {
        MethodDeclaration::new(
            MethodName::new(name).unwrap(),
            Some(receiver),
            vec![],
            TypeRef::new(returns).unwrap(),
            false,
            None,
        )
    }

    fn method_shared_ref_no_params(name: &str, returns: &str) -> MethodDeclaration {
        method_ref_no_params(name, returns, SelfReceiver::SharedRef)
    }

    fn method_exclusive_ref_no_params(name: &str, returns: &str) -> MethodDeclaration {
        method_ref_no_params(name, returns, SelfReceiver::ExclusiveRef)
    }

    fn method_with_params(
        name: &str,
        receiver: Option<SelfReceiver>,
        params: Vec<(&str, &str)>,
        returns: &str,
    ) -> MethodDeclaration {
        let params = params
            .into_iter()
            .map(|(pname, pty)| {
                ParamDeclaration::new(ParamName::new(pname).unwrap(), TypeRef::new(pty).unwrap())
            })
            .collect();
        MethodDeclaration::new(
            MethodName::new(name).unwrap(),
            receiver,
            params,
            TypeRef::new(returns).unwrap(),
            false,
            None,
        )
    }

    fn invariant_decl(method_name: &str) -> InvariantDecl {
        InvariantDecl::new(
            InvariantName::new(method_name).unwrap(),
            InvariantPredicate::SelfMethod(MethodName::new(method_name).unwrap()),
        )
    }

    fn identity_accessor(method_name: &str) -> IdentityAccessor {
        IdentityAccessor::new(MethodName::new(method_name).unwrap())
    }

    fn field_decl(name: &str, ty: &str) -> FieldDecl {
        FieldDecl::new(FieldName::new(name).unwrap(), TypeRef::new(ty).unwrap())
    }

    fn run_rule(
        doc: &CatalogueDocument,
        target: RuleTarget,
        kind: CatalogueLinterRuleKind,
    ) -> Vec<CatalogueLintViolation> {
        let rule = CatalogueLinterRule::new(target, kind).unwrap();
        let all = all_catalogues_single(doc);
        let target_layer = doc.layer.clone();
        evaluate_catalogue_lint(&[rule], &all, &target_layer).unwrap()
    }

    fn assert_mixed_aggregate_entity_target_without_exclusive_members_rejected(
        kind: CatalogueLinterRuleKind,
    ) {
        let rule_kind = kind.discriminant_name();
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderAgg").unwrap(),
            make_type_entry(DataRole::aggregate_root().unwrap()),
        );
        doc.types.insert(
            TypeName::new("OrderEntity").unwrap(),
            make_type_entry(DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![],
            }),
        );

        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::AggregateRoot, RoleKind::Entity]),
            kind,
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer);

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.contains("exclusive_members") && msg.contains("Entity")
            ),
            "expected InvalidRuleConfig for {rule_kind} mixed AggregateRoot/Entity target, got: {result:?}"
        );
    }

    // ===========================================================================
    // T016: Rule 1 — FieldEmpty
    // ===========================================================================

    #[test]
    fn test_field_empty_happy_path_when_field_is_empty() {
        // DomainService with emits: [] → FieldEmpty "emits" → no violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("MyService").unwrap(),
            make_type_entry(DataRole::DomainService { emits: vec![] }),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::DomainService]),
            CatalogueLinterRuleKind::FieldEmpty { target_field: "emits".to_owned() },
        );
        assert!(violations.is_empty(), "expected no violations when emits is empty");
    }

    #[test]
    fn test_field_empty_violation_when_field_is_not_empty() {
        // DomainService with emits: ["OrderPlaced"] → FieldEmpty "emits" → 1 violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("MyService").unwrap(),
            make_type_entry(DataRole::DomainService {
                emits: vec![TypeRef::new("OrderPlaced").unwrap()],
            }),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::DomainService]),
            CatalogueLinterRuleKind::FieldEmpty { target_field: "emits".to_owned() },
        );
        assert_eq!(violations.len(), 1, "expected 1 violation when emits is non-empty");
        assert_eq!(violations[0].rule_kind(), "FieldEmpty");
        assert_eq!(violations[0].entry_name(), "MyService");
        assert!(violations[0].message().contains("emits"));
    }

    #[test]
    fn test_field_empty_rejects_contract_only_target_role() {
        // FieldEmpty only evaluates TypeEntry/DataRole entries. A ContractRole-only
        // target would otherwise iterate no entries and return success.
        let mut doc = make_doc("domain");
        doc.traits.insert(
            TraitName::new("OrderRepo").unwrap(),
            make_trait_entry(ContractRole::Repository {
                aggregate: TypeRef::new("Order").unwrap(),
            }),
        );

        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::Repository]),
            CatalogueLinterRuleKind::FieldEmpty { target_field: "emits".to_owned() },
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer);

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.contains("emits") && msg.contains("Repository")
            ),
            "expected InvalidRuleConfig for Repository FieldEmpty target, got: {result:?}"
        );
    }

    // ===========================================================================
    // T016: Rule 2 — FieldNonEmpty
    // ===========================================================================

    #[test]
    fn test_field_non_empty_happy_path_when_field_is_non_empty() {
        // UseCase with handles: ["OrderCommand"] → FieldNonEmpty "handles" → no violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("MyUseCase").unwrap(),
            make_type_entry(DataRole::UseCase {
                handles: vec![TypeRef::new("OrderCommand").unwrap()],
            }),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::UseCase]),
            CatalogueLinterRuleKind::FieldNonEmpty { target_field: "handles".to_owned() },
        );
        assert!(violations.is_empty(), "expected no violations when handles is non-empty");
    }

    #[test]
    fn test_field_non_empty_violation_when_field_is_empty() {
        // UseCase with handles: [] → FieldNonEmpty "handles" → 1 violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("MyUseCase").unwrap(),
            make_type_entry(DataRole::UseCase { handles: vec![] }),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::UseCase]),
            CatalogueLinterRuleKind::FieldNonEmpty { target_field: "handles".to_owned() },
        );
        assert_eq!(violations.len(), 1, "expected 1 violation when handles is empty");
        assert_eq!(violations[0].rule_kind(), "FieldNonEmpty");
        assert_eq!(violations[0].entry_name(), "MyUseCase");
        assert!(violations[0].message().contains("handles"));
    }

    #[test]
    fn test_field_non_empty_rejects_contract_only_target_role() {
        // FieldNonEmpty only evaluates TypeEntry/DataRole entries. A ContractRole-only
        // target would otherwise iterate no entries and return success.
        let mut doc = make_doc("domain");
        doc.traits.insert(
            TraitName::new("OrderRepo").unwrap(),
            make_trait_entry(ContractRole::Repository {
                aggregate: TypeRef::new("Order").unwrap(),
            }),
        );

        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::Repository]),
            CatalogueLinterRuleKind::FieldNonEmpty { target_field: "emits".to_owned() },
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer);

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.contains("emits") && msg.contains("Repository")
            ),
            "expected InvalidRuleConfig for Repository FieldNonEmpty target, got: {result:?}"
        );
    }

    // ===========================================================================
    // T016: Rule 3 — KindLayerConstraint
    // ===========================================================================

    #[test]
    fn test_kind_layer_constraint_happy_path_when_layer_is_permitted() {
        // EventPolicy in domain layer → permitted_layers: [domain] → no violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("MyEventPolicy").unwrap(),
            make_type_entry(DataRole::EventPolicy {
                reacts_to: NonEmptyVec::new(TypeRef::new("OrderPlaced").unwrap(), vec![]),
            }),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::EventPolicy]),
            CatalogueLinterRuleKind::KindLayerConstraint {
                permitted_layers: NonEmptyVec::new(layer("domain"), vec![]),
            },
        );
        assert!(violations.is_empty(), "expected no violations when layer is permitted");
    }

    #[test]
    fn test_kind_layer_constraint_violation_when_layer_is_not_permitted() {
        // Table-driven: EventPolicy in various non-domain layers → 1 violation each
        for disallowed_layer in ["usecase", "infrastructure"] {
            let mut doc = make_doc(disallowed_layer);
            doc.types.insert(
                TypeName::new("MyEventPolicy").unwrap(),
                make_type_entry(DataRole::EventPolicy {
                    reacts_to: NonEmptyVec::new(TypeRef::new("OrderPlaced").unwrap(), vec![]),
                }),
            );
            let violations = run_rule(
                &doc,
                RuleTarget::new(vec![RoleKind::EventPolicy]),
                CatalogueLinterRuleKind::KindLayerConstraint {
                    permitted_layers: NonEmptyVec::new(layer("domain"), vec![]),
                },
            );
            assert_eq!(
                violations.len(),
                1,
                "expected 1 violation for EventPolicy in {disallowed_layer} layer"
            );
            assert_eq!(violations[0].rule_kind(), "KindLayerConstraint");
            assert_eq!(violations[0].entry_name(), "MyEventPolicy");
            assert!(
                violations[0].message().contains(disallowed_layer),
                "expected message to mention layer {disallowed_layer}"
            );
        }
    }

    // ===========================================================================
    // T016: Rule 4 — ReferencedRoleConstraint
    // ===========================================================================

    #[test]
    fn test_referenced_role_constraint_happy_path_when_role_matches() {
        // AggregateRoot emits→["OrderPlaced"] where OrderPlaced is DomainEvent → no violation
        let mut doc = make_doc("domain");
        doc.types
            .insert(TypeName::new("OrderPlaced").unwrap(), make_type_entry(DataRole::DomainEvent));
        doc.types.insert(
            TypeName::new("OrderAgg").unwrap(),
            make_type_entry(DataRole::AggregateRoot {
                identity: identity_accessor("id"),
                invariants: vec![],
                exclusive_members: vec![],
                shared_value_objects: vec![],
                emits: vec![TypeRef::new("OrderPlaced").unwrap()],
            }),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::AggregateRoot]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "emits".to_owned(),
                expected_role: RoleKind::DomainEvent,
            },
        );
        assert!(violations.is_empty(), "expected no violations when emits → DomainEvent");
    }

    #[test]
    fn test_referenced_role_constraint_violation_when_wrong_role() {
        // AggregateRoot emits→["OrderEntity"] where OrderEntity is Entity (not DomainEvent) → 1 violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderEntity").unwrap(),
            make_type_entry(DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![],
            }),
        );
        doc.types.insert(
            TypeName::new("OrderAgg").unwrap(),
            make_type_entry(DataRole::AggregateRoot {
                identity: identity_accessor("id"),
                invariants: vec![],
                exclusive_members: vec![],
                shared_value_objects: vec![],
                emits: vec![TypeRef::new("OrderEntity").unwrap()],
            }),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::AggregateRoot]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "emits".to_owned(),
                expected_role: RoleKind::DomainEvent,
            },
        );
        assert_eq!(violations.len(), 1, "expected 1 violation when emits has wrong role");
        assert_eq!(violations[0].rule_kind(), "ReferencedRoleConstraint");
        assert_eq!(violations[0].entry_name(), "OrderAgg");
        assert!(violations[0].message().contains("OrderEntity"));
        assert!(violations[0].message().contains("DomainEvent"));
    }

    #[test]
    fn test_referenced_role_constraint_rejects_role_field_mismatch() {
        // Repository × "emits" must fail-closed with InvalidRuleConfig.
        //
        // "emits" is a DataRole-only field (AggregateRoot / DomainService).
        // Repository is a ContractRole-kind role.  Supplying a DataRole-only
        // target_field together with an exclusively ContractRole target_roles list
        // is an incoherent configuration — no catalogue entry can ever carry that
        // field — so the evaluator must return Err(InvalidRuleConfig) rather than
        // silently iterating zero entries (D19 fail-closed).
        let mut doc = make_doc("domain");
        // Add a Repository trait entry so the rule has at least one candidate entry
        // to iterate if the pre-check were absent.
        doc.traits.insert(
            TraitName::new("OrderRepo").unwrap(),
            make_trait_entry(ContractRole::Repository {
                aggregate: TypeRef::new("Order").unwrap(),
            }),
        );

        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::Repository]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "emits".to_owned(),
                expected_role: RoleKind::DomainEvent,
            },
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer);

        assert!(
            matches!(&result, Err(CatalogueLinterError::InvalidRuleConfig(msg)) if msg.contains("emits")),
            "expected InvalidRuleConfig for Repository × emits mismatch, got: {result:?}"
        );
    }

    // ===========================================================================
    // T016: Rule 5 — TraitImplRequired
    // ===========================================================================

    #[test]
    fn test_trait_impl_required_happy_path_when_all_traits_present() {
        // ValueObject with PartialEq + Eq in trait_impls → no violation
        let mut doc = make_doc("domain");
        doc.types
            .insert(TypeName::new("MyValue").unwrap(), make_type_entry(DataRole::value_object()));
        doc.trait_impls.push(TraitImplDeclV2::new(
            TypeRef::new("PartialEq").unwrap(),
            TypeRef::new("MyValue").unwrap(),
        ));
        doc.trait_impls.push(TraitImplDeclV2::new(
            TypeRef::new("Eq").unwrap(),
            TypeRef::new("MyValue").unwrap(),
        ));
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::TraitImplRequired {
                required_traits: NonEmptyVec::new("PartialEq".to_owned(), vec!["Eq".to_owned()]),
            },
        );
        assert!(violations.is_empty(), "expected no violations when PartialEq + Eq are present");
    }

    #[test]
    fn test_trait_impl_required_violation_when_trait_missing() {
        // ValueObject with only PartialEq (missing Eq) → 1 violation
        let mut doc = make_doc("domain");
        doc.types
            .insert(TypeName::new("MyValue").unwrap(), make_type_entry(DataRole::value_object()));
        doc.trait_impls.push(TraitImplDeclV2::new(
            TypeRef::new("PartialEq").unwrap(),
            TypeRef::new("MyValue").unwrap(),
        ));
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::TraitImplRequired {
                required_traits: NonEmptyVec::new("PartialEq".to_owned(), vec!["Eq".to_owned()]),
            },
        );
        assert_eq!(violations.len(), 1, "expected 1 violation for missing Eq");
        assert_eq!(violations[0].rule_kind(), "TraitImplRequired");
        assert_eq!(violations[0].entry_name(), "MyValue");
        assert!(violations[0].message().contains("Eq"));
    }

    // ===========================================================================
    // T016: Rule 6 — NoRoleInMethodSignature
    // ===========================================================================

    #[test]
    fn test_no_role_in_method_signature_happy_path_when_no_forbidden_role_in_sig() {
        // ValueObject method returns "String" (not Entity) → no violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("MyValue").unwrap(),
            make_type_entry_with_methods(
                DataRole::value_object(),
                vec![method_shared_ref_no_params("as_str", "String")],
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::NoRoleInMethodSignature {
                forbidden_roles: NonEmptyVec::new(RoleKind::Entity, vec![RoleKind::AggregateRoot]),
            },
        );
        assert!(
            violations.is_empty(),
            "expected no violations when no forbidden role in signature"
        );
    }

    #[test]
    fn test_no_role_in_method_signature_violation_when_forbidden_role_in_return() {
        // ValueObject method returns "OrderEntity" which is Entity → 1 violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderEntity").unwrap(),
            make_type_entry(DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![],
            }),
        );
        doc.types.insert(
            TypeName::new("MyValue").unwrap(),
            make_type_entry_with_methods(
                DataRole::value_object(),
                vec![method_shared_ref_no_params("entity_ref", "OrderEntity")],
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::NoRoleInMethodSignature {
                forbidden_roles: NonEmptyVec::new(RoleKind::Entity, vec![RoleKind::AggregateRoot]),
            },
        );
        assert_eq!(violations.len(), 1, "expected 1 violation for Entity in return type");
        assert_eq!(violations[0].rule_kind(), "NoRoleInMethodSignature");
        assert_eq!(violations[0].entry_name(), "MyValue");
        assert!(violations[0].message().contains("entity_ref"));
    }

    // ===========================================================================
    // T016: Rule 7 — MethodReferenceSignature (AC-13)
    // ===========================================================================

    #[test]
    fn test_method_reference_signature_ac13_pass_when_invariant_method_valid() {
        // Entity with invariant "is_valid" → method &self, no params, bool return → pass
        let mut doc = make_doc("domain");
        let entity = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![invariant_decl("is_valid")],
            },
            kind: unit_struct_kind(),
            methods: vec![method_shared_ref_no_params("is_valid", "bool")],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.types.insert(TypeName::new("Order").unwrap(), entity);
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::Entity]),
            CatalogueLinterRuleKind::MethodReferenceSignature {
                target_field: "invariants".to_owned(),
            },
        );
        assert!(violations.is_empty(), "expected no violations for valid invariant method");
    }

    #[test]
    fn test_method_reference_signature_ac13_violation_when_method_missing() {
        // Entity with invariant "is_valid" → method not in public methods → 1 violation
        let mut doc = make_doc("domain");
        let entity = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![invariant_decl("is_valid")],
            },
            kind: unit_struct_kind(),
            methods: vec![], // method missing
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.types.insert(TypeName::new("Order").unwrap(), entity);
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::Entity]),
            CatalogueLinterRuleKind::MethodReferenceSignature {
                target_field: "invariants".to_owned(),
            },
        );
        assert_eq!(violations.len(), 1, "expected 1 violation when invariant method is missing");
        assert_eq!(violations[0].rule_kind(), "MethodReferenceSignature");
        assert_eq!(violations[0].entry_name(), "Order");
        assert!(violations[0].message().contains("is_valid"));
    }

    #[test]
    fn test_method_reference_signature_ac13_violation_when_wrong_receiver() {
        // Entity with invariant "is_valid" → method has &mut self → 1 violation
        let mut doc = make_doc("domain");
        let entity = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![invariant_decl("is_valid")],
            },
            kind: unit_struct_kind(),
            methods: vec![method_exclusive_ref_no_params("is_valid", "bool")],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.types.insert(TypeName::new("Order").unwrap(), entity);
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::Entity]),
            CatalogueLinterRuleKind::MethodReferenceSignature {
                target_field: "invariants".to_owned(),
            },
        );
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation when invariant method has wrong receiver"
        );
        assert_eq!(violations[0].rule_kind(), "MethodReferenceSignature");
        assert!(violations[0].message().contains("is_valid"));
    }

    #[test]
    fn test_method_reference_signature_ac13_violation_when_has_params() {
        // Entity with invariant "is_valid" → method has a param → 1 violation
        let mut doc = make_doc("domain");
        let entity = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![invariant_decl("is_valid")],
            },
            kind: unit_struct_kind(),
            methods: vec![method_with_params(
                "is_valid",
                Some(SelfReceiver::SharedRef),
                vec![("x", "i32")],
                "bool",
            )],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.types.insert(TypeName::new("Order").unwrap(), entity);
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::Entity]),
            CatalogueLinterRuleKind::MethodReferenceSignature {
                target_field: "invariants".to_owned(),
            },
        );
        assert_eq!(violations.len(), 1, "expected 1 violation when invariant method has params");
        assert_eq!(violations[0].rule_kind(), "MethodReferenceSignature");
        assert!(violations[0].message().contains("is_valid"));
    }

    // ===========================================================================
    // T016: Rule 8 — AccessorSignatureRequired (AC-14)
    // ===========================================================================

    #[test]
    fn test_accessor_signature_required_ac14_pass_when_valid_getter() {
        // Entity with identity "id" → method &self, no params, non-unit return → pass
        let mut doc = make_doc("domain");
        let entity = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::Entity { identity: identity_accessor("id"), invariants: vec![] },
            kind: unit_struct_kind(),
            methods: vec![method_shared_ref_no_params("id", "OrderId")],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.types.insert(TypeName::new("Order").unwrap(), entity);
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::Entity]),
            CatalogueLinterRuleKind::AccessorSignatureRequired {
                target_field: "identity".to_owned(),
            },
        );
        assert!(violations.is_empty(), "expected no violations for valid identity getter");
    }

    #[test]
    fn test_accessor_signature_required_ac14_violation_when_getter_missing() {
        // Entity with identity "id" → method not present → 1 violation
        let mut doc = make_doc("domain");
        let entity = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::Entity { identity: identity_accessor("id"), invariants: vec![] },
            kind: unit_struct_kind(),
            methods: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.types.insert(TypeName::new("Order").unwrap(), entity);
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::Entity]),
            CatalogueLinterRuleKind::AccessorSignatureRequired {
                target_field: "identity".to_owned(),
            },
        );
        assert_eq!(violations.len(), 1, "expected 1 violation when identity getter is missing");
        assert_eq!(violations[0].rule_kind(), "AccessorSignatureRequired");
        assert_eq!(violations[0].entry_name(), "Order");
        assert!(violations[0].message().contains("id"));
    }

    #[test]
    fn test_accessor_signature_required_ac14_violation_when_unit_return() {
        // Entity identity getter returns "()" → 1 violation
        let mut doc = make_doc("domain");
        let entity = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::Entity { identity: identity_accessor("id"), invariants: vec![] },
            kind: unit_struct_kind(),
            methods: vec![method_shared_ref_no_params("id", "()")],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.types.insert(TypeName::new("Order").unwrap(), entity);
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::Entity]),
            CatalogueLinterRuleKind::AccessorSignatureRequired {
                target_field: "identity".to_owned(),
            },
        );
        assert_eq!(violations.len(), 1, "expected 1 violation when getter returns ()");
        assert_eq!(violations[0].rule_kind(), "AccessorSignatureRequired");
        assert!(violations[0].message().contains("id"));
    }

    // ===========================================================================
    // T016: Rule 9 — FieldElementUniqueAcrossEntries (AC-16 boundary)
    // ===========================================================================

    #[test]
    fn test_field_element_unique_across_entries_happy_path_when_no_overlap() {
        // Two AggregateRoots with disjoint exclusive_members → no violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("AggA").unwrap(),
            make_type_entry(DataRole::AggregateRoot {
                identity: identity_accessor("id"),
                invariants: vec![],
                exclusive_members: vec![TypeRef::new("EntityA").unwrap()],
                shared_value_objects: vec![],
                emits: vec![],
            }),
        );
        doc.types.insert(
            TypeName::new("AggB").unwrap(),
            make_type_entry(DataRole::AggregateRoot {
                identity: identity_accessor("id"),
                invariants: vec![],
                exclusive_members: vec![TypeRef::new("EntityB").unwrap()],
                shared_value_objects: vec![],
                emits: vec![],
            }),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::AggregateRoot]),
            CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries {
                target_field: "exclusive_members".to_owned(),
            },
        );
        assert!(
            violations.is_empty(),
            "expected no violations when exclusive_members are disjoint"
        );
    }

    #[test]
    fn test_field_element_unique_across_entries_violation_when_overlap() {
        // Two AggregateRoots sharing "EntityA" → 1 violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("AggA").unwrap(),
            make_type_entry(DataRole::AggregateRoot {
                identity: identity_accessor("id"),
                invariants: vec![],
                exclusive_members: vec![TypeRef::new("EntityA").unwrap()],
                shared_value_objects: vec![],
                emits: vec![],
            }),
        );
        doc.types.insert(
            TypeName::new("AggB").unwrap(),
            make_type_entry(DataRole::AggregateRoot {
                identity: identity_accessor("id"),
                invariants: vec![],
                exclusive_members: vec![TypeRef::new("EntityA").unwrap()], // duplicate!
                shared_value_objects: vec![],
                emits: vec![],
            }),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::AggregateRoot]),
            CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries {
                target_field: "exclusive_members".to_owned(),
            },
        );
        assert_eq!(violations.len(), 1, "expected 1 violation when exclusive_members overlap");
        assert_eq!(violations[0].rule_kind(), "FieldElementUniqueAcrossEntries");
        assert!(violations[0].message().contains("EntityA"));
    }

    #[test]
    fn test_field_element_unique_across_entries_rejects_non_exclusive_members_target_field() {
        // FieldElementUniqueAcrossEntries is defined only for "exclusive_members" (ADR D6/D11).
        // Supplying any other DataRole field (e.g. "emits") must return
        // Err(InvalidRuleConfig) — the evaluator must not silently iterate zero
        // entries (D19 fail-closed).
        let doc = make_doc("domain");

        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::AggregateRoot]),
            CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries {
                target_field: "emits".to_owned(),
            },
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer);

        assert!(
            matches!(&result, Err(CatalogueLinterError::InvalidRuleConfig(msg)) if msg.contains("emits")),
            "expected InvalidRuleConfig for FieldElementUniqueAcrossEntries with target_field 'emits', \
             got: {result:?}"
        );
    }

    #[test]
    fn test_field_element_unique_across_entries_rejects_non_aggregate_target_role() {
        // FieldElementUniqueAcrossEntries inspects AggregateRoot.exclusive_members only.
        // Targeting a role that cannot carry that field must fail closed instead of
        // silently seeing an empty refs slice.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderEntity").unwrap(),
            make_type_entry(DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![],
            }),
        );

        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::Entity]),
            CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries {
                target_field: "exclusive_members".to_owned(),
            },
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer);

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.contains("exclusive_members") && msg.contains("Entity")
            ),
            "expected InvalidRuleConfig for Entity target with exclusive_members, got: {result:?}"
        );
    }

    #[test]
    fn test_field_element_unique_across_entries_rejects_mixed_target_role_that_does_not_carry_field()
     {
        assert_mixed_aggregate_entity_target_without_exclusive_members_rejected(
            CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries {
                target_field: "exclusive_members".to_owned(),
            },
        );
    }

    #[test]
    fn test_field_element_unique_across_entries_rejects_all_roles_target() {
        // RuleTarget::all_roles includes roles that do not carry exclusive_members.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderAgg").unwrap(),
            make_type_entry(DataRole::aggregate_root().unwrap()),
        );

        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries {
                target_field: "exclusive_members".to_owned(),
            },
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer);

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.contains("exclusive_members") && msg.contains("all roles")
            ),
            "expected InvalidRuleConfig for all roles target with exclusive_members, got: {result:?}"
        );
    }

    // Note: AC-16 exclusive_members uniqueness is covered by
    // test_field_element_unique_across_entries_violation_when_overlap above.

    // ===========================================================================
    // T016: Rule 10 — NoExternalReferenceInMethods
    // ===========================================================================

    #[test]
    fn test_no_external_reference_in_methods_happy_path_when_no_external_ref() {
        // AggA with exclusive_member EntityA; no other type references EntityA in methods → no violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("AggA").unwrap(),
            make_type_entry(DataRole::AggregateRoot {
                identity: identity_accessor("id"),
                invariants: vec![],
                exclusive_members: vec![TypeRef::new("EntityA").unwrap()],
                shared_value_objects: vec![],
                emits: vec![],
            }),
        );
        doc.types.insert(
            TypeName::new("EntityA").unwrap(),
            make_type_entry_with_methods(
                DataRole::Entity { identity: identity_accessor("id"), invariants: vec![] },
                vec![method_shared_ref_no_params("id", "EntityAId")],
            ),
        );
        // EntityA method returns "EntityAId" — not a reference to EntityA → pass
        doc.types.insert(
            TypeName::new("AnotherService").unwrap(),
            make_type_entry_with_methods(
                DataRole::DomainService { emits: vec![] },
                vec![method_shared_ref_no_params("do_work", "String")],
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::AggregateRoot]),
            CatalogueLinterRuleKind::NoExternalReferenceInMethods {
                target_field: "exclusive_members".to_owned(),
            },
        );
        assert!(
            violations.is_empty(),
            "expected no violations when exclusive members not referenced externally"
        );
    }

    #[test]
    fn test_no_external_reference_in_methods_violation_when_external_ref_exists() {
        // AggA exclusive_member EntityA; ExternalService has method referencing EntityA → 1 violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("AggA").unwrap(),
            make_type_entry(DataRole::AggregateRoot {
                identity: identity_accessor("id"),
                invariants: vec![],
                exclusive_members: vec![TypeRef::new("EntityA").unwrap()],
                shared_value_objects: vec![],
                emits: vec![],
            }),
        );
        doc.types.insert(
            TypeName::new("EntityA").unwrap(),
            make_type_entry(DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![],
            }),
        );
        // ExternalService references EntityA in its method signature (violation of boundary)
        doc.types.insert(
            TypeName::new("ExternalService").unwrap(),
            make_type_entry_with_methods(
                DataRole::DomainService { emits: vec![] },
                vec![method_shared_ref_no_params("illegal_ref", "EntityA")],
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::AggregateRoot]),
            CatalogueLinterRuleKind::NoExternalReferenceInMethods {
                target_field: "exclusive_members".to_owned(),
            },
        );
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation when external entry references exclusive member"
        );
        assert_eq!(violations[0].rule_kind(), "NoExternalReferenceInMethods");
        assert_eq!(violations[0].entry_name(), "AggA");
        assert!(violations[0].message().contains("EntityA"));
        assert!(violations[0].message().contains("ExternalService"));
    }

    #[test]
    fn test_no_external_reference_in_methods_detects_external_ref_in_inherent_impl() {
        // AggA exclusive_member EntityA; ExternalService has no legacy methods, but
        // its inherent impl references EntityA. The rule must scan both method sources.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("AggA").unwrap(),
            make_type_entry(DataRole::AggregateRoot {
                identity: identity_accessor("id"),
                invariants: vec![],
                exclusive_members: vec![TypeRef::new("EntityA").unwrap()],
                shared_value_objects: vec![],
                emits: vec![],
            }),
        );
        doc.types.insert(
            TypeName::new("EntityA").unwrap(),
            make_type_entry(DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![],
            }),
        );
        doc.types.insert(
            TypeName::new("ExternalService").unwrap(),
            make_type_entry(DataRole::DomainService { emits: vec![] }),
        );
        doc.inherent_impls.push(InherentImplDeclV2 {
            type_name: TypeName::new("ExternalService").unwrap(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![method_shared_ref_no_params("illegal_ref", "EntityA")],
        });

        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::AggregateRoot]),
            CatalogueLinterRuleKind::NoExternalReferenceInMethods {
                target_field: "exclusive_members".to_owned(),
            },
        );

        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation when an inherent impl method references an exclusive member"
        );
        assert_eq!(violations[0].rule_kind(), "NoExternalReferenceInMethods");
        assert_eq!(violations[0].entry_name(), "AggA");
        assert!(violations[0].message().contains("EntityA"));
        assert!(violations[0].message().contains("ExternalService"));
    }

    #[test]
    fn test_no_external_reference_in_methods_rejects_non_aggregate_target_role() {
        // NoExternalReferenceInMethods inspects AggregateRoot.exclusive_members only.
        // Targeting a role that cannot carry that field must fail closed instead of
        // silently building an empty aggregate boundary set.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderEntity").unwrap(),
            make_type_entry(DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![],
            }),
        );

        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::Entity]),
            CatalogueLinterRuleKind::NoExternalReferenceInMethods {
                target_field: "exclusive_members".to_owned(),
            },
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer);

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.contains("exclusive_members") && msg.contains("Entity")
            ),
            "expected InvalidRuleConfig for Entity target with exclusive_members, got: {result:?}"
        );
    }

    #[test]
    fn test_no_external_reference_in_methods_rejects_mixed_target_role_that_does_not_carry_field() {
        assert_mixed_aggregate_entity_target_without_exclusive_members_rejected(
            CatalogueLinterRuleKind::NoExternalReferenceInMethods {
                target_field: "exclusive_members".to_owned(),
            },
        );
    }

    // ===========================================================================
    // T016: Rule 11 — NoPublicField
    // ===========================================================================

    #[test]
    fn test_no_public_field_happy_path_when_struct_is_unit() {
        // DomainEvent with Unit struct → no violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderPlaced").unwrap(),
            make_type_entry_with_kind(DataRole::DomainEvent, unit_struct_kind()),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::DomainEvent]),
            CatalogueLinterRuleKind::NoPublicField,
        );
        assert!(violations.is_empty(), "expected no violations for unit struct DomainEvent");
    }

    #[test]
    fn test_no_public_field_violation_when_struct_has_public_fields() {
        // DomainEvent with Plain struct + field → 1 violation
        let mut doc = make_doc("domain");
        let kind = plain_struct_kind(vec![field_decl("order_id", "OrderId")]);
        doc.types.insert(
            TypeName::new("OrderPlaced").unwrap(),
            make_type_entry_with_kind(DataRole::DomainEvent, kind),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::DomainEvent]),
            CatalogueLinterRuleKind::NoPublicField,
        );
        assert_eq!(violations.len(), 1, "expected 1 violation when struct has public field");
        assert_eq!(violations[0].rule_kind(), "NoPublicField");
        assert_eq!(violations[0].entry_name(), "OrderPlaced");
        assert!(violations[0].message().contains("public"));
    }

    // ===========================================================================
    // T016: Rule 12 — ForbiddenMethodReceiver
    // ===========================================================================

    #[test]
    fn test_forbidden_method_receiver_happy_path_when_no_forbidden_receiver() {
        // DomainEvent with &self method → ForbiddenMethodReceiver "&mut self" → no violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderPlaced").unwrap(),
            make_type_entry_with_methods(
                DataRole::DomainEvent,
                vec![method_shared_ref_no_params("event_id", "EventId")],
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::DomainEvent]),
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: "&mut self".to_owned(),
            },
        );
        assert!(violations.is_empty(), "expected no violations when no &mut self method");
    }

    #[test]
    fn test_forbidden_method_receiver_violation_when_forbidden_receiver_used() {
        // DomainEvent with &mut self method → ForbiddenMethodReceiver "&mut self" → 1 violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderPlaced").unwrap(),
            make_type_entry_with_methods(
                DataRole::DomainEvent,
                vec![method_exclusive_ref_no_params("mutate", "()")],
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::DomainEvent]),
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: "&mut self".to_owned(),
            },
        );
        assert_eq!(violations.len(), 1, "expected 1 violation when &mut self method exists");
        assert_eq!(violations[0].rule_kind(), "ForbiddenMethodReceiver");
        assert_eq!(violations[0].entry_name(), "OrderPlaced");
        assert!(violations[0].message().contains("mutate"));
        assert!(violations[0].message().contains("&mut self"));
    }

    // ===========================================================================
    // T016: Rule 6/12 — inherent_impls coverage
    // NoRoleInMethodSignature and ForbiddenMethodReceiver must scan methods
    // declared in CatalogueDocument::inherent_impls, not only TypeEntry::methods.
    // ===========================================================================

    #[test]
    fn test_no_role_in_method_signature_detects_forbidden_role_in_inherent_impl() {
        // ValueObject `Money` has an empty `TypeEntry::methods`.
        // An inherent impl block for `Money` declares a method whose param type is
        // `OrderEntity` — which has role Entity (forbidden).
        // The rule must detect the violation sourced from the inherent impl.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderEntity").unwrap(),
            make_type_entry(DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![],
            }),
        );
        doc.types.insert(
            TypeName::new("Money").unwrap(),
            make_type_entry(DataRole::value_object()), // methods: vec![]
        );
        // The method lives in an inherent impl, not in TypeEntry::methods.
        doc.inherent_impls.push(InherentImplDeclV2 {
            type_name: TypeName::new("Money").unwrap(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![method_with_params(
                "from_entity",
                Some(SelfReceiver::SharedRef),
                vec![("entity", "OrderEntity")],
                "String",
            )],
        });
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::NoRoleInMethodSignature {
                forbidden_roles: NonEmptyVec::new(RoleKind::Entity, vec![RoleKind::AggregateRoot]),
            },
        );
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation: forbidden Entity role found in inherent impl method param"
        );
        assert_eq!(violations[0].rule_kind(), "NoRoleInMethodSignature");
        assert_eq!(violations[0].entry_name(), "Money");
        assert!(
            violations[0].message().contains("from_entity"),
            "violation message should name the method"
        );
    }

    #[test]
    fn test_forbidden_method_receiver_detects_in_inherent_impl() {
        // DomainEvent `OrderPlaced` has an empty `TypeEntry::methods`.
        // An inherent impl block for `OrderPlaced` declares a method with `&mut self`.
        // The ForbiddenMethodReceiver rule must detect the violation from the impl block.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderPlaced").unwrap(),
            make_type_entry(DataRole::DomainEvent), // methods: vec![]
        );
        // The &mut self method lives in an inherent impl, not in TypeEntry::methods.
        doc.inherent_impls.push(InherentImplDeclV2 {
            type_name: TypeName::new("OrderPlaced").unwrap(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![method_exclusive_ref_no_params("set_payload", "()")],
        });
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::DomainEvent]),
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: "&mut self".to_owned(),
            },
        );
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation: &mut self found in inherent impl method"
        );
        assert_eq!(violations[0].rule_kind(), "ForbiddenMethodReceiver");
        assert_eq!(violations[0].entry_name(), "OrderPlaced");
        assert!(
            violations[0].message().contains("set_payload"),
            "violation message should name the method"
        );
        assert!(violations[0].message().contains("&mut self"));
    }

    #[test]
    fn test_forbidden_method_receiver_deduplicates_legacy_and_inherent_methods() {
        // Legacy catalogues can still carry methods in `TypeEntry.methods`, while
        // newer catalogues can also carry the same logical method in top-level
        // `inherent_impls`. The linter treats the method name as the identity, so
        // duplicate source representations must not double-report one Rust method.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderPlaced").unwrap(),
            make_type_entry_with_methods(
                DataRole::DomainEvent,
                vec![method_exclusive_ref_no_params("set_payload", "()")],
            ),
        );
        doc.inherent_impls.push(InherentImplDeclV2 {
            type_name: TypeName::new("OrderPlaced").unwrap(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![method_exclusive_ref_no_params("set_payload", "()")],
        });

        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::DomainEvent]),
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: "&mut self".to_owned(),
            },
        );

        assert_eq!(violations.len(), 1, "expected one violation for one logical method");
        assert_eq!(violations[0].rule_kind(), "ForbiddenMethodReceiver");
        assert_eq!(violations[0].entry_name(), "OrderPlaced");
        assert!(violations[0].message().contains("set_payload"));
    }

    #[test]
    fn test_inconsistent_legacy_and_inherent_method_duplicates_return_invalid_rule_config() {
        // A stale legacy method declaration must not hide a different declaration
        // for the same Rust method in `inherent_impls`.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderPlaced").unwrap(),
            make_type_entry_with_methods(
                DataRole::DomainEvent,
                vec![method_shared_ref_no_params("set_payload", "()")],
            ),
        );
        doc.inherent_impls.push(InherentImplDeclV2 {
            type_name: TypeName::new("OrderPlaced").unwrap(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![method_exclusive_ref_no_params("set_payload", "()")],
        });

        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::DomainEvent]),
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: "&mut self".to_owned(),
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer);

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.contains("set_payload") && msg.contains("OrderPlaced")
            ),
            "expected InvalidRuleConfig for inconsistent duplicate method declarations, got: {result:?}"
        );
    }

    // ===========================================================================
    // T016: AC-15 — TraitImplRequired: equality traits for Entity / AggregateRoot
    // ===========================================================================

    #[test]
    fn test_ac15_entity_requires_partial_eq_and_eq_trait_impls() {
        // Entity without PartialEq/Eq trait_impls → 2 violations
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderEntity").unwrap(),
            make_type_entry(DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![],
            }),
        );
        // No trait_impls added
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::Entity, RoleKind::AggregateRoot]),
            CatalogueLinterRuleKind::TraitImplRequired {
                required_traits: NonEmptyVec::new("PartialEq".to_owned(), vec!["Eq".to_owned()]),
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let target_layer = layer("domain");
        let violations = evaluate_catalogue_lint(&[rule], &all, &target_layer).unwrap();
        assert_eq!(violations.len(), 2, "expected 2 violations (missing PartialEq + missing Eq)");
        assert!(violations.iter().any(|v| v.message().contains("PartialEq")));
        assert!(violations.iter().any(|v| v.message().contains("Eq")));
    }

    #[test]
    fn test_ac15_aggregate_root_requires_partial_eq_and_eq_trait_impls() {
        // AggregateRoot with PartialEq + Eq → no violations
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderAgg").unwrap(),
            make_type_entry(DataRole::aggregate_root().unwrap()),
        );
        doc.trait_impls.push(TraitImplDeclV2::new(
            TypeRef::new("PartialEq").unwrap(),
            TypeRef::new("OrderAgg").unwrap(),
        ));
        doc.trait_impls.push(TraitImplDeclV2::new(
            TypeRef::new("Eq").unwrap(),
            TypeRef::new("OrderAgg").unwrap(),
        ));
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::Entity, RoleKind::AggregateRoot]),
            CatalogueLinterRuleKind::TraitImplRequired {
                required_traits: NonEmptyVec::new("PartialEq".to_owned(), vec!["Eq".to_owned()]),
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let target_layer = layer("domain");
        let violations = evaluate_catalogue_lint(&[rule], &all, &target_layer).unwrap();
        assert!(violations.is_empty(), "expected no violations when PartialEq + Eq are present");
    }

    // ===========================================================================
    // T016: AC-16 — 5 Aggregate Boundary rules
    // ===========================================================================

    #[test]
    fn test_ac16_aggregate_boundary_exclusive_members_must_be_entities() {
        // AggregateRoot exclusive_member is a ValueObject (not Entity) → 1 violation
        let mut doc = make_doc("domain");
        doc.types
            .insert(TypeName::new("Price").unwrap(), make_type_entry(DataRole::value_object()));
        doc.types.insert(
            TypeName::new("OrderAgg").unwrap(),
            make_type_entry(DataRole::AggregateRoot {
                identity: identity_accessor("id"),
                invariants: vec![],
                exclusive_members: vec![TypeRef::new("Price").unwrap()], // wrong role
                shared_value_objects: vec![],
                emits: vec![],
            }),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::AggregateRoot]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "exclusive_members".to_owned(),
                expected_role: RoleKind::Entity,
            },
        );
        assert_eq!(violations.len(), 1, "expected 1 violation for non-Entity exclusive_member");
        assert_eq!(violations[0].rule_kind(), "ReferencedRoleConstraint");
        assert!(violations[0].message().contains("Price"));
    }

    #[test]
    fn test_ac16_aggregate_boundary_shared_value_objects_must_be_value_objects() {
        // AggregateRoot shared_value_objects includes Entity → 1 violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderLine").unwrap(),
            make_type_entry(DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![],
            }),
        );
        doc.types.insert(
            TypeName::new("OrderAgg").unwrap(),
            make_type_entry(DataRole::AggregateRoot {
                identity: identity_accessor("id"),
                invariants: vec![],
                exclusive_members: vec![],
                shared_value_objects: vec![TypeRef::new("OrderLine").unwrap()], // wrong role
                emits: vec![],
            }),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::AggregateRoot]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "shared_value_objects".to_owned(),
                expected_role: RoleKind::ValueObject,
            },
        );
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation for non-ValueObject in shared_value_objects"
        );
        assert!(violations[0].message().contains("OrderLine"));
    }

    #[test]
    fn test_ac16_value_object_no_entity_in_method_signature() {
        // ValueObject method param is an Entity → 1 violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderEntity").unwrap(),
            make_type_entry(DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![],
            }),
        );
        doc.types.insert(
            TypeName::new("MyValue").unwrap(),
            make_type_entry_with_methods(
                DataRole::value_object(),
                vec![method_with_params(
                    "bad_method",
                    Some(SelfReceiver::SharedRef),
                    vec![("entity", "OrderEntity")],
                    "()",
                )],
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::NoRoleInMethodSignature {
                forbidden_roles: NonEmptyVec::new(RoleKind::Entity, vec![RoleKind::AggregateRoot]),
            },
        );
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation for Entity in ValueObject method sig"
        );
        assert_eq!(violations[0].rule_kind(), "NoRoleInMethodSignature");
    }

    // ===========================================================================
    // T016: AC-17 — DomainEvent mutation rules
    // (covered by Rule 11 + Rule 12 tests above; AC-17 NoPublicField tested below)
    // ===========================================================================

    // Note: DomainEvent NoPublicField is covered by
    // test_no_public_field_violation_when_struct_has_public_fields above.

    // ===========================================================================
    // T016: AC-18 — Repository aggregate role check
    // ===========================================================================

    #[test]
    fn test_ac18_repository_aggregate_field_must_be_aggregate_root_happy_path() {
        // Repository TraitEntry with aggregate → AggregateRoot: no violation.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderAgg").unwrap(),
            make_type_entry(DataRole::aggregate_root().unwrap()),
        );
        doc.traits.insert(
            TraitName::new("OrderRepository").unwrap(),
            make_trait_entry(ContractRole::Repository {
                aggregate: TypeRef::new("OrderAgg").unwrap(),
            }),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::Repository]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "aggregate".to_owned(),
                expected_role: RoleKind::AggregateRoot,
            },
        );
        assert!(violations.is_empty(), "expected 0 violations: OrderAgg is declared AggregateRoot");
    }

    #[test]
    fn test_ac18_repository_aggregate_pointing_to_non_aggregate_root_is_violation() {
        // Repository TraitEntry with aggregate → Entity (wrong role): 1 violation.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderItem").unwrap(),
            make_type_entry(DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![],
            }),
        );
        doc.traits.insert(
            TraitName::new("OrderItemRepository").unwrap(),
            make_trait_entry(ContractRole::Repository {
                aggregate: TypeRef::new("OrderItem").unwrap(),
            }),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::Repository]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "aggregate".to_owned(),
                expected_role: RoleKind::AggregateRoot,
            },
        );
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation: OrderItem is Entity, not AggregateRoot"
        );
        assert_eq!(violations[0].rule_kind(), "ReferencedRoleConstraint");
        assert!(
            violations[0].message().contains("OrderItem"),
            "message should name the referenced type"
        );
        assert_eq!(
            violations[0].entry_name(),
            "OrderItemRepository",
            "entry_name should be the Repository trait name"
        );
    }

    // ===========================================================================
    // T016: AC-19 — EventPolicy 4 rules
    // ===========================================================================

    #[test]
    fn test_ac19_event_policy_reacts_to_must_be_domain_event() {
        // EventPolicy reacts_to references a UseCase (wrong role) → 1 violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("PlaceOrderUseCase").unwrap(),
            make_type_entry(DataRole::use_case()),
        );
        doc.types.insert(
            TypeName::new("OrderPolicy").unwrap(),
            make_type_entry(DataRole::EventPolicy {
                reacts_to: NonEmptyVec::new(TypeRef::new("PlaceOrderUseCase").unwrap(), vec![]),
            }),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::EventPolicy]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "reacts_to".to_owned(),
                expected_role: RoleKind::DomainEvent,
            },
        );
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation for EventPolicy reacts_to wrong role"
        );
        assert_eq!(violations[0].rule_kind(), "ReferencedRoleConstraint");
        assert!(violations[0].message().contains("PlaceOrderUseCase"));
    }

    // Note: AC-19 KindLayerConstraint (infrastructure layer) is covered by
    // test_kind_layer_constraint_violation_when_layer_is_not_permitted above.

    #[test]
    fn test_ac19_event_policy_no_mut_self_method() {
        // EventPolicy with &mut self method → 1 violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderPolicy").unwrap(),
            make_type_entry_with_methods(
                DataRole::EventPolicy {
                    reacts_to: NonEmptyVec::new(TypeRef::new("OrderPlaced").unwrap(), vec![]),
                },
                vec![method_exclusive_ref_no_params("on_event", "()")],
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::EventPolicy]),
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: "&mut self".to_owned(),
            },
        );
        assert_eq!(violations.len(), 1, "expected 1 violation for &mut self on EventPolicy");
        assert_eq!(violations[0].rule_kind(), "ForbiddenMethodReceiver");
    }

    #[test]
    fn test_ac19_event_policy_no_repository_or_usecase_in_method_sig() {
        // EventPolicy method param is a Repository role (TraitEntry) → 1 violation
        let mut doc = make_doc("domain");
        doc.traits.insert(
            TraitName::new("OrderRepo").unwrap(),
            make_trait_entry(ContractRole::Repository {
                aggregate: TypeRef::new("Order").unwrap(),
            }),
        );
        doc.types.insert(
            TypeName::new("OrderPolicy").unwrap(),
            make_type_entry_with_methods(
                DataRole::EventPolicy {
                    reacts_to: NonEmptyVec::new(TypeRef::new("OrderPlaced").unwrap(), vec![]),
                },
                vec![method_with_params(
                    "on_event",
                    Some(SelfReceiver::SharedRef),
                    vec![("repo", "OrderRepo")],
                    "()",
                )],
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::EventPolicy]),
            CatalogueLinterRuleKind::NoRoleInMethodSignature {
                forbidden_roles: NonEmptyVec::new(RoleKind::Repository, vec![RoleKind::UseCase]),
            },
        );
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation for Repository in EventPolicy method sig"
        );
        assert_eq!(violations[0].rule_kind(), "NoRoleInMethodSignature");
    }

    // ===========================================================================
    // T016: AC-20 — ValueObject mutation checks
    // ===========================================================================

    #[test]
    fn test_ac20_value_object_no_mut_self_method() {
        // ValueObject with &mut self method → 1 violation
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Money").unwrap(),
            make_type_entry_with_methods(
                DataRole::value_object(),
                vec![method_exclusive_ref_no_params("set_amount", "()")],
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: "&mut self".to_owned(),
            },
        );
        assert_eq!(violations.len(), 1, "expected 1 violation for &mut self on ValueObject");
        assert_eq!(violations[0].rule_kind(), "ForbiddenMethodReceiver");
    }

    #[test]
    fn test_ac20_value_object_no_public_field() {
        // ValueObject with public field → 1 violation
        let mut doc = make_doc("domain");
        let kind = plain_struct_kind(vec![field_decl("amount", "i64")]);
        doc.types.insert(
            TypeName::new("Money").unwrap(),
            make_type_entry_with_kind(DataRole::value_object(), kind),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::NoPublicField,
        );
        assert_eq!(violations.len(), 1, "expected 1 violation for public field on ValueObject");
        assert_eq!(violations[0].rule_kind(), "NoPublicField");
    }

    #[test]
    fn test_ac20_value_object_equality_trait_required() {
        // ValueObject without PartialEq → 1 violation
        let mut doc = make_doc("domain");
        doc.types
            .insert(TypeName::new("Money").unwrap(), make_type_entry(DataRole::value_object()));
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::TraitImplRequired {
                required_traits: NonEmptyVec::new("PartialEq".to_owned(), vec!["Eq".to_owned()]),
            },
        );
        assert_eq!(
            violations.len(),
            2,
            "expected 2 violations for missing PartialEq + Eq on ValueObject"
        );
    }

    // ===========================================================================
    // Cross-layer lookup tests
    // ===========================================================================

    /// Build a two-layer BTreeMap for cross-layer testing.
    /// `domain` layer contains `OrderPlaced` as `DomainEvent`,
    /// `usecase` layer contains `PlaceOrder` as `UseCase` with `handles: ["domain::OrderPlaced"]`.
    fn two_layer_catalogues() -> (BTreeMap<LayerId, CatalogueDocument>, LayerId, LayerId) {
        let domain_layer = layer("domain");
        let usecase_layer = layer("usecase");

        let mut domain_doc = make_doc("domain");
        domain_doc
            .types
            .insert(TypeName::new("OrderPlaced").unwrap(), make_type_entry(DataRole::DomainEvent));

        let mut usecase_doc =
            CatalogueDocument::new(3, CrateName::new("usecase").unwrap(), usecase_layer.clone());
        usecase_doc.types.insert(
            TypeName::new("PlaceOrder").unwrap(),
            make_type_entry(DataRole::UseCase {
                handles: vec![TypeRef::new("domain::OrderPlaced").unwrap()],
            }),
        );

        let mut all = BTreeMap::new();
        all.insert(domain_layer.clone(), domain_doc);
        all.insert(usecase_layer.clone(), usecase_doc);

        (all, domain_layer, usecase_layer)
    }

    #[test]
    fn test_cross_layer_referenced_role_constraint_resolves_domain_event_from_domain_layer() {
        // UseCase in usecase layer handles "domain::OrderPlaced" which is DomainEvent in domain
        // layer. Rule: ReferencedRoleConstraint on UseCase.handles must be DomainEvent.
        // Expected: no violation (OrderPlaced is correctly declared DomainEvent in domain layer).
        let (all, _domain_layer, usecase_layer) = two_layer_catalogues();
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::UseCase]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "handles".to_owned(),
                expected_role: RoleKind::DomainEvent,
            },
        )
        .unwrap();
        let violations = evaluate_catalogue_lint(&[rule], &all, &usecase_layer).unwrap();
        assert!(
            violations.is_empty(),
            "expected no violations: domain::OrderPlaced is DomainEvent in domain layer, \
             got: {violations:?}"
        );
    }

    #[test]
    fn test_cross_layer_referenced_role_constraint_violation_when_wrong_role() {
        // Put OrderPlaced in usecase layer as ValueObject (wrong role) and check that the
        // rule fires a violation when the UseCase.handles entry declares wrong role.
        let domain_layer = layer("domain");
        let usecase_layer = layer("usecase");

        let domain_doc = make_doc("domain");
        // domain layer has NO OrderPlaced at all; usecase has it as ValueObject (wrong)
        let mut usecase_doc =
            CatalogueDocument::new(3, CrateName::new("usecase").unwrap(), usecase_layer.clone());
        usecase_doc.types.insert(
            TypeName::new("OrderPlaced").unwrap(),
            make_type_entry(DataRole::value_object()),
        );
        usecase_doc.types.insert(
            TypeName::new("PlaceOrder").unwrap(),
            make_type_entry(DataRole::UseCase {
                handles: vec![TypeRef::new("OrderPlaced").unwrap()],
            }),
        );

        let mut all = BTreeMap::new();
        all.insert(domain_layer, domain_doc);
        all.insert(usecase_layer.clone(), usecase_doc);

        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::UseCase]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "handles".to_owned(),
                expected_role: RoleKind::DomainEvent,
            },
        )
        .unwrap();
        let violations = evaluate_catalogue_lint(&[rule], &all, &usecase_layer).unwrap();
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation: OrderPlaced is ValueObject, not DomainEvent, \
             got: {violations:?}"
        );
        assert_eq!(violations[0].rule_kind(), "ReferencedRoleConstraint");
        assert_eq!(violations[0].entry_name(), "PlaceOrder");
        assert!(violations[0].message().contains("OrderPlaced"));
        assert!(violations[0].message().contains("DomainEvent"));
    }

    #[test]
    fn test_cross_layer_no_role_in_method_signature_detects_forbidden_role_from_other_layer() {
        // ValueObject in usecase layer has a method returning "OrderRepo" which is a
        // Repository declared in domain layer. Rule: no Repository in method sig.
        let domain_layer = layer("domain");
        let usecase_layer = layer("usecase");

        let mut domain_doc = make_doc("domain");
        domain_doc.traits.insert(
            crate::tddd::catalogue_v2::identifiers::TraitName::new("OrderRepo").unwrap(),
            make_trait_entry(ContractRole::Repository {
                aggregate: TypeRef::new("OrderAgg").unwrap(),
            }),
        );

        let mut usecase_doc =
            CatalogueDocument::new(3, CrateName::new("usecase").unwrap(), usecase_layer.clone());
        usecase_doc.types.insert(
            TypeName::new("MyDto").unwrap(),
            make_type_entry_with_methods(
                DataRole::value_object(),
                vec![method_with_params(
                    "with_repo",
                    Some(SelfReceiver::SharedRef),
                    vec![("repo", "OrderRepo")],
                    "()",
                )],
            ),
        );

        let mut all = BTreeMap::new();
        all.insert(domain_layer, domain_doc);
        all.insert(usecase_layer.clone(), usecase_doc);

        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::NoRoleInMethodSignature {
                forbidden_roles: NonEmptyVec::new(RoleKind::Repository, vec![]),
            },
        )
        .unwrap();
        let violations = evaluate_catalogue_lint(&[rule], &all, &usecase_layer).unwrap();
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation: OrderRepo (Repository, declared in domain) in method sig, \
             got: {violations:?}"
        );
        assert_eq!(violations[0].rule_kind(), "NoRoleInMethodSignature");
        assert_eq!(violations[0].entry_name(), "MyDto");
        assert!(violations[0].message().contains("OrderRepo"));
        assert!(violations[0].message().contains("Repository"));
    }

    #[test]
    fn test_evaluate_catalogue_lint_unknown_layer_returns_error() {
        // Requesting evaluation for a layer not present in all_catalogues must return
        // CatalogueLinterError::UnknownLayer.
        let all: BTreeMap<LayerId, CatalogueDocument> = BTreeMap::new();
        let target = layer("nonexistent");
        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::NoPublicField,
        )
        .unwrap();
        let result = evaluate_catalogue_lint(&[rule], &all, &target);
        let is_unknown_layer = matches!(&result, Err(CatalogueLinterError::UnknownLayer { .. }));
        assert!(is_unknown_layer, "expected UnknownLayer error, got: {result:?}");
    }

    // ===========================================================================
    // Fail-closed semantics: action: Delete entries are excluded from lookups
    // ===========================================================================

    #[test]
    fn test_resolve_type_role_skips_delete_action_type_entry() {
        // A TypeEntry with action: Delete in the domain layer must not be found by
        // find_in_catalogue, so resolve_type_role returns None for it.
        // As a result, ReferencedRoleConstraint treats the reference as unresolvable
        // and emits a violation (fail-closed semantics).
        let domain_layer = layer("domain");
        let usecase_layer = layer("usecase");

        // domain layer: OrderPlaced is Delete-marked — must be invisible to lookups
        let mut domain_doc = make_doc("domain");
        let mut deleted_entry = make_type_entry(DataRole::DomainEvent);
        deleted_entry.action = ItemAction::Delete;
        domain_doc.types.insert(TypeName::new("OrderPlaced").unwrap(), deleted_entry);

        // usecase layer: PlaceOrder.handles references "domain::OrderPlaced"
        let mut usecase_doc =
            CatalogueDocument::new(3, CrateName::new("usecase").unwrap(), usecase_layer.clone());
        usecase_doc.types.insert(
            TypeName::new("PlaceOrder").unwrap(),
            make_type_entry(DataRole::UseCase {
                handles: vec![TypeRef::new("domain::OrderPlaced").unwrap()],
            }),
        );

        let mut all = BTreeMap::new();
        all.insert(domain_layer, domain_doc);
        all.insert(usecase_layer.clone(), usecase_doc);

        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::UseCase]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "handles".to_owned(),
                expected_role: RoleKind::DomainEvent,
            },
        )
        .unwrap();
        let violations = evaluate_catalogue_lint(&[rule], &all, &usecase_layer).unwrap();
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation: Delete-marked TypeEntry must not satisfy role lookup, \
             got: {violations:?}"
        );
        assert_eq!(violations[0].rule_kind(), "ReferencedRoleConstraint");
        assert_eq!(violations[0].entry_name(), "PlaceOrder");
        assert!(violations[0].message().contains("OrderPlaced"));
    }

    #[test]
    fn test_resolve_type_role_skips_delete_action_trait_entry() {
        // A TraitEntry with action: Delete in the domain layer must not be iterated
        // by the NoRoleInMethodSignature trait loop in catalogue_linter_eval.rs.
        //
        // The method signature uses the layer-qualified form "domain::OrderRepo" so
        // that sig_type_contains_entry takes Rule 1 (layer_qualified_name_in_sig →
        // true) without consulting find_in_catalogue. This means the ONLY guard that
        // prevents a false violation is the `action != Delete` filter on
        // cat.traits.iter() in catalogue_linter_eval.rs — not find_in_catalogue.
        // Without that filter the deleted trait would be iterated, sig_type_contains_entry
        // would return true, and a forbidden-role violation would be emitted.
        let domain_layer = layer("domain");
        let usecase_layer = layer("usecase");

        // domain layer: OrderRepo is Delete-marked — must not be iterated by the
        // NoRoleInMethodSignature trait loop.
        let mut domain_doc = make_doc("domain");
        let mut deleted_trait = make_trait_entry(ContractRole::Repository {
            aggregate: TypeRef::new("Order").unwrap(),
        });
        deleted_trait.action = ItemAction::Delete;
        domain_doc.traits.insert(TraitName::new("OrderRepo").unwrap(), deleted_trait);

        // usecase layer: MyDto method param uses the qualified form "domain::OrderRepo"
        // so that sig_type_contains_entry resolves via Rule 1 (qualified layer match)
        // without calling find_in_catalogue — making the eval.rs outer filter the
        // decisive guard.
        let mut usecase_doc =
            CatalogueDocument::new(3, CrateName::new("usecase").unwrap(), usecase_layer.clone());
        usecase_doc.types.insert(
            TypeName::new("MyDto").unwrap(),
            make_type_entry_with_methods(
                DataRole::value_object(),
                vec![method_with_params(
                    "with_repo",
                    Some(SelfReceiver::SharedRef),
                    vec![("repo", "domain::OrderRepo")],
                    "()",
                )],
            ),
        );

        let mut all = BTreeMap::new();
        all.insert(domain_layer, domain_doc);
        all.insert(usecase_layer.clone(), usecase_doc);

        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::NoRoleInMethodSignature {
                forbidden_roles: NonEmptyVec::new(RoleKind::Repository, vec![]),
            },
        )
        .unwrap();
        let violations = evaluate_catalogue_lint(&[rule], &all, &usecase_layer).unwrap();
        assert!(
            violations.is_empty(),
            "expected no violations: Delete-marked TraitEntry must be skipped by the \
             action != Delete filter in the NoRoleInMethodSignature trait loop — \
             the qualified 'domain::OrderRepo' reference bypasses find_in_catalogue \
             so the outer loop filter is the decisive guard, got: {violations:?}"
        );
    }

    #[test]
    fn test_no_role_in_method_signature_skips_delete_action_type_entry() {
        // A TypeEntry with action: Delete in the domain layer must not be iterated
        // by the NoRoleInMethodSignature TYPE loop in catalogue_linter_eval.rs.
        //
        // The method signature uses the layer-qualified form "domain::OrderPlaced" so
        // that sig_type_contains_entry takes Rule 1 (layer_qualified_name_in_sig →
        // true) without consulting find_in_catalogue. This means the ONLY guard that
        // prevents a false violation is the `action != Delete` filter on
        // cat.types.iter() in catalogue_linter_eval.rs — not find_in_catalogue.
        // Without that filter the deleted type would be iterated, entry_role_kind
        // would return DomainEvent, and a forbidden-role violation would be emitted.
        let domain_layer = layer("domain");
        let usecase_layer = layer("usecase");

        // domain layer: OrderPlaced is Delete-marked — must not be iterated by the
        // NoRoleInMethodSignature TYPE loop.
        let mut domain_doc = make_doc("domain");
        let mut deleted_type = make_type_entry(DataRole::DomainEvent);
        deleted_type.action = ItemAction::Delete;
        domain_doc.types.insert(TypeName::new("OrderPlaced").unwrap(), deleted_type);

        // usecase layer: MyUseCase method return type uses the qualified form
        // "domain::OrderPlaced" so that sig_type_contains_entry resolves via
        // Rule 1 (qualified layer match) without calling find_in_catalogue —
        // making the eval.rs outer cat.types.iter() filter the decisive guard.
        let mut usecase_doc =
            CatalogueDocument::new(3, CrateName::new("usecase").unwrap(), usecase_layer.clone());
        usecase_doc.types.insert(
            TypeName::new("MyUseCase").unwrap(),
            make_type_entry_with_methods(
                DataRole::UseCase { handles: vec![] },
                vec![method_shared_ref_no_params("last_event", "domain::OrderPlaced")],
            ),
        );

        let mut all = BTreeMap::new();
        all.insert(domain_layer, domain_doc);
        all.insert(usecase_layer.clone(), usecase_doc);

        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::UseCase]),
            CatalogueLinterRuleKind::NoRoleInMethodSignature {
                forbidden_roles: NonEmptyVec::new(RoleKind::DomainEvent, vec![]),
            },
        )
        .unwrap();
        let violations = evaluate_catalogue_lint(&[rule], &all, &usecase_layer).unwrap();
        assert!(
            violations.is_empty(),
            "expected no violations: Delete-marked TypeEntry must be skipped by the \
             action != Delete filter in the NoRoleInMethodSignature type loop — \
             the qualified 'domain::OrderPlaced' reference bypasses find_in_catalogue \
             so the outer cat.types.iter() filter is the decisive guard, \
             got: {violations:?}"
        );
    }

    #[test]
    fn test_has_trait_impl_skips_delete_action_impl() {
        // A TraitImplDeclV2 with action: Delete must not be treated as a present impl.
        // TraitImplRequired for "PartialEq" fires a violation because the only
        // PartialEq impl entry is Delete-marked.
        let mut doc = make_doc("domain");
        doc.types
            .insert(TypeName::new("MyValue").unwrap(), make_type_entry(DataRole::value_object()));

        // Only a Delete-marked PartialEq impl exists — must be treated as absent.
        let mut deleted_impl = TraitImplDeclV2::new(
            TypeRef::new("PartialEq").unwrap(),
            TypeRef::new("MyValue").unwrap(),
        );
        deleted_impl.action = ItemAction::Delete;
        doc.trait_impls.push(deleted_impl);

        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::TraitImplRequired {
                required_traits: NonEmptyVec::new("PartialEq".to_owned(), vec![]),
            },
        );
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation: Delete-marked TraitImplDeclV2 must not satisfy TraitImplRequired, \
             got: {violations:?}"
        );
        assert_eq!(violations[0].rule_kind(), "TraitImplRequired");
        assert_eq!(violations[0].entry_name(), "MyValue");
        assert!(violations[0].message().contains("PartialEq"));
    }

    #[test]
    fn test_type_entries_for_target_skips_delete_action_entry_for_field_non_empty_and_no_public_field()
     {
        // A TypeEntry with action: Delete must not be scanned by any rule that
        // routes through type_entries_for_target (FieldNonEmpty, NoPublicField, …).
        //
        // Each sub-case uses its own catalogue with only the Delete-action entry.
        // Because the entry is Delete-marked, type_entries_for_target must exclude
        // it, and no violation may appear regardless of how badly the entry would
        // violate the rule if it were active.

        // --- Sub-case 1: FieldNonEmpty ---
        // The Delete-action entry has empty invariants, which would normally
        // trigger a FieldNonEmpty("invariants") violation.  With the filter in
        // place, no violation fires.
        {
            let mut doc = make_doc("domain");
            let deleted_entry = TypeEntry {
                action: ItemAction::Delete,
                role: DataRole::ValueObject { invariants: vec![] }, // empty — would violate
                kind: unit_struct_kind(),
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            };
            doc.types.insert(TypeName::new("DeletedValue").unwrap(), deleted_entry);

            let violations = run_rule(
                &doc,
                RuleTarget::new(vec![RoleKind::ValueObject]),
                CatalogueLinterRuleKind::FieldNonEmpty { target_field: "invariants".to_owned() },
            );
            assert!(
                violations.is_empty(),
                "FieldNonEmpty: expected no violations — Delete-action entry must be skipped, \
                 got: {violations:?}"
            );
        }

        // --- Sub-case 2: NoPublicField ---
        // The Delete-action entry has a public field, which would normally
        // trigger a NoPublicField violation.  With the filter in place, no
        // violation fires.
        {
            let mut doc = make_doc("domain");
            let deleted_entry = TypeEntry {
                action: ItemAction::Delete,
                role: DataRole::value_object(),
                kind: plain_struct_kind(vec![field_decl("pub_field", "String")]), // public — would violate
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            };
            doc.types.insert(TypeName::new("DeletedValue").unwrap(), deleted_entry);

            let violations = run_rule(
                &doc,
                RuleTarget::new(vec![RoleKind::ValueObject]),
                CatalogueLinterRuleKind::NoPublicField,
            );
            assert!(
                violations.is_empty(),
                "NoPublicField: expected no violations — Delete-action entry must be skipped, \
                 got: {violations:?}"
            );
        }
    }

    // ===========================================================================
    // D19 fail-closed: unknown target_field rejects with InvalidRuleConfig
    // ===========================================================================

    #[test]
    fn test_evaluate_catalogue_lint_unknown_target_field_returns_invalid_rule_config() {
        // FieldEmpty with target_field "emit" (typo for "emits") must return
        // Err(InvalidRuleConfig) rather than silently treating the field as empty.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("MyService").unwrap(),
            make_type_entry(DataRole::DomainService {
                emits: vec![TypeRef::new("OrderPlaced").unwrap()],
            }),
        );
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::DomainService]),
            CatalogueLinterRuleKind::FieldEmpty { target_field: "emit".to_owned() }, // typo
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let result = evaluate_catalogue_lint(&[rule], &all, &doc.layer.clone());
        let is_invalid = matches!(&result, Err(CatalogueLinterError::InvalidRuleConfig(msg)) if msg.contains("emit"));
        assert!(
            is_invalid,
            "expected Err(InvalidRuleConfig) for unknown target_field 'emit', got: {result:?}"
        );
    }

    #[test]
    fn test_evaluate_catalogue_lint_unknown_target_field_for_referenced_role_constraint_returns_error()
     {
        // ReferencedRoleConstraint with target_field "handle" (typo for "handles") must
        // return Err(InvalidRuleConfig) rather than silently reporting zero violations.
        let mut doc = make_doc("domain");
        doc.types
            .insert(TypeName::new("OrderPlaced").unwrap(), make_type_entry(DataRole::DomainEvent));
        doc.types.insert(
            TypeName::new("PlaceOrder").unwrap(),
            make_type_entry(DataRole::UseCase {
                handles: vec![TypeRef::new("OrderPlaced").unwrap()],
            }),
        );
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::UseCase]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "handle".to_owned(), // typo — should be "handles"
                expected_role: RoleKind::DomainEvent,
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let result = evaluate_catalogue_lint(&[rule], &all, &doc.layer.clone());
        let is_invalid = matches!(&result, Err(CatalogueLinterError::InvalidRuleConfig(msg)) if msg.contains("handle"));
        assert!(
            is_invalid,
            "expected Err(InvalidRuleConfig) for unknown target_field 'handle', got: {result:?}"
        );
    }

    #[test]
    fn test_evaluate_catalogue_lint_referenced_role_constraint_invariants_returns_error() {
        // "invariants" is a valid DataRole field for FieldEmpty / FieldNonEmpty,
        // but it contains predicate declarations rather than TypeRef role references.
        // ReferencedRoleConstraint must reject it instead of silently checking no refs.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Order").unwrap(),
            make_type_entry(DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![invariant_decl("is_valid")],
            }),
        );
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::Entity]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "invariants".to_owned(),
                expected_role: RoleKind::DomainEvent,
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let result = evaluate_catalogue_lint(&[rule], &all, &doc.layer.clone());
        let is_invalid = matches!(
            &result,
            Err(CatalogueLinterError::InvalidRuleConfig(msg))
                if msg.contains("ReferencedRoleConstraint") && msg.contains("invariants")
        );
        assert!(
            is_invalid,
            "expected Err(InvalidRuleConfig) for ReferencedRoleConstraint target_field \
             'invariants', got: {result:?}"
        );
    }

    #[test]
    fn test_referenced_role_constraint_rejects_field_not_carried_by_data_target_role() {
        // Entity does not carry "emits"; an explicit Entity target must not silently
        // evaluate zero refs for ReferencedRoleConstraint.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderEntity").unwrap(),
            make_type_entry(DataRole::Entity {
                identity: identity_accessor("id"),
                invariants: vec![],
            }),
        );
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::Entity]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "emits".to_owned(),
                expected_role: RoleKind::DomainEvent,
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let result = evaluate_catalogue_lint(&[rule], &all, &doc.layer.clone());
        let is_invalid = matches!(
            &result,
            Err(CatalogueLinterError::InvalidRuleConfig(msg))
                if msg.contains("emits") && msg.contains("Entity")
        );
        assert!(
            is_invalid,
            "expected Err(InvalidRuleConfig) for Entity target_field 'emits', got: {result:?}"
        );
    }

    #[test]
    fn test_referenced_role_constraint_rejects_field_not_carried_by_contract_target_role() {
        // SpecificationPort does not carry "aggregate"; only Repository does.
        let mut doc = make_doc("domain");
        doc.traits.insert(
            TraitName::new("OrderSpecPort").unwrap(),
            make_trait_entry(ContractRole::SpecificationPort),
        );
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::SpecificationPort]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "aggregate".to_owned(),
                expected_role: RoleKind::AggregateRoot,
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let result = evaluate_catalogue_lint(&[rule], &all, &doc.layer.clone());
        let is_invalid = matches!(
            &result,
            Err(CatalogueLinterError::InvalidRuleConfig(msg))
                if msg.contains("aggregate") && msg.contains("SpecificationPort")
        );
        assert!(
            is_invalid,
            "expected Err(InvalidRuleConfig) for SpecificationPort target_field \
             'aggregate', got: {result:?}"
        );
    }

    #[test]
    fn test_field_type_refs_aggregate_on_data_role_returns_empty_slice() {
        // "aggregate" is a ContractRole-only field; DataRole does not carry it.
        // field_type_refs must return Ok(&[]) (not an error) so that a
        // ReferencedRoleConstraint rule whose RuleTarget covers both DataRole and
        // ContractRole entries can still evaluate the ContractRole trait entries.
        // Only field names that are unrecognised in any role's vocabulary are rejected.
        use super::helpers::field_type_refs;
        let role = DataRole::AggregateRoot {
            identity: identity_accessor("id"),
            invariants: vec![],
            exclusive_members: vec![TypeRef::new("OrderLine").unwrap()],
            shared_value_objects: vec![],
            emits: vec![],
        };
        let result = field_type_refs(&role, "aggregate");
        assert!(
            matches!(result, Ok(slice) if slice.is_empty()),
            "expected Ok(&[]) for ContractRole-only field 'aggregate' on DataRole, got: {result:?}"
        );
    }

    #[test]
    fn test_field_type_refs_truly_unknown_field_returns_error() {
        // A field name that is not in any role's vocabulary must return Err(InvalidRuleConfig).
        // (As opposed to cross-role fields like "aggregate" which return Ok(&[]) for DataRole.)
        use super::helpers::field_type_refs;
        let role = DataRole::DomainService { emits: vec![] };
        let result = field_type_refs(&role, "no_such_field_xyz");
        let is_invalid = matches!(&result, Err(CatalogueLinterError::InvalidRuleConfig(msg)) if msg.contains("no_such_field_xyz"));
        assert!(
            is_invalid,
            "expected Err(InvalidRuleConfig) for truly unknown field 'no_such_field_xyz', got: {result:?}"
        );
    }

    #[test]
    fn test_evaluate_catalogue_lint_unknown_target_field_for_referenced_role_constraint_on_trait_returns_error()
     {
        // ReferencedRoleConstraint with an unknown target_field for a trait (ContractRole) target
        // must return Err(InvalidRuleConfig).  Previously, contract_role_type_ref returned None
        // for unrecognised field names, causing a silent skip instead of a fail-closed error.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("OrderAgg").unwrap(),
            make_type_entry(DataRole::aggregate_root().unwrap()),
        );
        doc.traits.insert(
            TraitName::new("OrderRepository").unwrap(),
            make_trait_entry(ContractRole::Repository {
                aggregate: TypeRef::new("OrderAgg").unwrap(),
            }),
        );
        // "aggregat" is a typo for "aggregate" — unknown ContractRole field name.
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::Repository]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: "aggregat".to_owned(), // typo
                expected_role: RoleKind::AggregateRoot,
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let result = evaluate_catalogue_lint(&[rule], &all, &doc.layer.clone());
        let is_invalid = matches!(&result, Err(CatalogueLinterError::InvalidRuleConfig(msg)) if msg.contains("aggregat"));
        assert!(
            is_invalid,
            "expected Err(InvalidRuleConfig) for unknown ContractRole target_field 'aggregat', got: {result:?}"
        );
    }

    #[test]
    fn test_no_external_reference_in_methods_ignores_delete_action_aggregate_shared_value_objects()
    {
        // A delete-action AggregateRoot must not be scanned by the
        // NoExternalReferenceInMethods rule — neither as an aggregate source (boundary
        // builder) nor as an external entry whose methods are checked.
        //
        // Setup:
        //   - OrderAgg (Add): exclusive_members: [OrderLine], shared_value_objects: [Money]
        //   - DeletedAgg (Delete): exclusive_members: [OrderLine], shared_value_objects: [Money]
        //     ALSO carries a method `get_line() -> OrderLine` (references the exclusive member).
        //     If DeletedAgg were scanned as an external entry, that method reference
        //     would trigger a second NoExternalReferenceInMethods violation.
        //   - ExternalEntry (Add, ValueObject): method references "OrderLine" in return type.
        //     With the active OrderAgg, this is an external-reference violation.
        //   - ExternalEntry2 (Add, ValueObject): method references "Money" in return type.
        //     Money is in OrderAgg.shared_value_objects → inside boundary → no violation.
        //     Crucially, DeletedAgg also lists Money — but the fix ensures the deleted
        //     aggregate does not pollute the active aggregate's boundary computation.
        //
        // The main assertion: the rule fires exactly ONE violation (OrderAgg / ExternalEntry
        // for OrderLine), confirming that:
        //   (a) DeletedAgg's shared_value_objects do not pollute the boundary set, AND
        //   (b) DeletedAgg is not scanned in the other_entry.methods loop — if it were,
        //       a second violation for OrderAgg / DeletedAgg would fire.
        let mut doc = make_doc("domain");

        // Active aggregate: exclusive_members = [OrderLine], shared_value_objects = [Money]
        let order_agg = make_type_entry(DataRole::AggregateRoot {
            identity: IdentityAccessor::new(MethodName::new("id").unwrap()),
            invariants: vec![],
            exclusive_members: vec![TypeRef::new("OrderLine").unwrap()],
            shared_value_objects: vec![TypeRef::new("Money").unwrap()],
            emits: vec![],
        });
        doc.types.insert(TypeName::new("OrderAgg").unwrap(), order_agg);

        // Delete-action aggregate: also has exclusive_members=[OrderLine],
        // shared_value_objects=[Money], and a method that references OrderLine.
        // Must not be scanned as either an aggregate source or an external entry.
        // Without the `action != Delete` filter in the other_entry.methods scan,
        // this method would cause a second violation to fire for OrderAgg/DeletedAgg.
        let deleted_agg = TypeEntry {
            action: ItemAction::Delete,
            role: DataRole::AggregateRoot {
                identity: IdentityAccessor::new(MethodName::new("id").unwrap()),
                invariants: vec![],
                exclusive_members: vec![TypeRef::new("OrderLine").unwrap()],
                shared_value_objects: vec![TypeRef::new("Money").unwrap()],
                emits: vec![],
            },
            kind: unit_struct_kind(),
            // Method references OrderLine — would trigger a second violation if
            // the other_entry.methods scan were not guarded by action != Delete.
            methods: vec![method_shared_ref_no_params("get_line", "OrderLine")],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.types.insert(TypeName::new("DeletedAgg").unwrap(), deleted_agg);

        // ExternalEntry: references OrderLine (exclusive member) — should be a violation.
        let external_entry = make_type_entry_with_methods(
            DataRole::value_object(),
            vec![method_shared_ref_no_params("get_line", "OrderLine")],
        );
        doc.types.insert(TypeName::new("ExternalEntry").unwrap(), external_entry);

        // ExternalEntry2: references Money (shared_value_object of active OrderAgg) —
        // inside the boundary, so no violation expected.
        let external_entry2 = make_type_entry_with_methods(
            DataRole::value_object(),
            vec![method_shared_ref_no_params("get_money", "Money")],
        );
        doc.types.insert(TypeName::new("ExternalEntry2").unwrap(), external_entry2);

        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::AggregateRoot]),
            CatalogueLinterRuleKind::NoExternalReferenceInMethods {
                target_field: "exclusive_members".to_owned(),
            },
        );

        // Expect exactly 1 violation: ExternalEntry leaks OrderLine from the active OrderAgg.
        // DeletedAgg must not produce a second violation — neither as an aggregate source
        // (boundary pollution) nor as an external entry (methods scan).
        assert_eq!(
            violations.len(),
            1,
            "expected exactly 1 violation (OrderAgg/OrderLine via ExternalEntry); \
             DeletedAgg must be skipped as a scan target, got: {violations:?}"
        );
        assert_eq!(violations[0].rule_kind(), "NoExternalReferenceInMethods");
        assert_eq!(violations[0].entry_name(), "OrderAgg");
        assert!(
            violations[0].message().contains("OrderLine"),
            "violation message must mention the exclusive member 'OrderLine'"
        );
        assert!(
            violations[0].message().contains("ExternalEntry"),
            "violation message must name the external entry 'ExternalEntry'"
        );
    }
}
