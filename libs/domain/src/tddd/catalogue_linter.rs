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

use crate::tddd::catalogue_v2::identifiers::TypeRef;
use crate::tddd::catalogue_v2::roles::{NonEmptyVec, SelfReceiver};
use crate::tddd::layer_id::LayerId;
use crate::tddd::primitive_occurrence_scanner::{
    PrimitiveName, PrimitiveOccurrencePosition, PrimitiveOccurrenceScanError,
};

// ---------------------------------------------------------------------------
// RoleKind — payload-free role discriminant (see catalogue_linter_role.rs)
// ---------------------------------------------------------------------------

#[path = "catalogue_linter_role.rs"]
mod role;

/// Re-export so that consumers of `catalogue_linter` see `RoleKind` at the
/// expected path without knowing about the `role` submodule.
pub use role::RoleKind;

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
// RolePayloadField — 8-variant closed field-name enum
// ---------------------------------------------------------------------------

/// Closed set of catalogue payload field names referenced by
/// `CatalogueLinterRuleKind`'s `target_field` payloads.
///
/// Exhaustively drawn from the field names historically accepted by
/// `validate_data_role_field` (`invariants`, `exclusive_members`,
/// `shared_value_objects`, `emits`, `handles`, `reacts_to`),
/// `validate_contract_role_field` (`aggregate`), and
/// `AccessorSignatureRequired`'s hardcoded `"identity"` check. Replacing the
/// former `target_field: String` payload with this enum makes "unknown field
/// name" an unrepresentable state instead of a runtime
/// `CatalogueLinterError::InvalidRuleConfig` rejection (D19 fail-closed, now
/// enforced by the type system rather than by a fallible string match).
///
/// Per-variant subset restrictions (e.g. `ReferencedRoleConstraint` rejecting
/// `Invariants` because invariants are not `TypeRef`-backed) remain necessary
/// runtime checks in `evaluate_catalogue_lint` — restated as enum-variant
/// equality instead of string equality, not eliminated.
///
/// The `Display` / `FromStr` format uses lowercase snake_case (`"invariants"`,
/// `"identity"`, `"exclusive_members"`, `"shared_value_objects"`, `"emits"`,
/// `"handles"`, `"reacts_to"`, `"aggregate"`) to match the field names already
/// used in catalogue JSON and existing rule configuration.
///
/// 8 variants: `Invariants`, `Identity`, `ExclusiveMembers`,
/// `SharedValueObjects`, `Emits`, `Handles`, `ReactsTo`, `Aggregate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum RolePayloadField {
    /// The `invariants` field on a `DataRole`.
    Invariants,
    /// The accessor-name field checked by `AccessorSignatureRequired`.
    Identity,
    /// The `exclusive_members` field on a `DataRole`.
    ExclusiveMembers,
    /// The `shared_value_objects` field on a `DataRole`.
    SharedValueObjects,
    /// The `emits` field on a `DataRole`.
    Emits,
    /// The `handles` field on a `DataRole`.
    Handles,
    /// The `reacts_to` field on a `DataRole`.
    ReactsTo,
    /// The `aggregate` field on a `ContractRole`.
    Aggregate,
}

// ---------------------------------------------------------------------------
// CatalogueLinterRuleKind — 13-variant rule category enum
// ---------------------------------------------------------------------------

/// Classifies what invariant a catalogue linter rule asserts (D15).
///
/// 13 variants: 12 data-carrying + 1 unit (`NoPublicField`).
///
/// Payloads use `RolePayloadField` for structured field-name references
/// (validated by construction), and `String` / `Vec<String>` / `Vec<RoleKind>`
/// for other data to stay serde-free. Codec layer converts JSON strings to
/// these types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogueLinterRuleKind {
    /// Rule asserts that the named field must be **empty** for matching entries.
    FieldEmpty {
        /// Catalogue field to check (e.g. `RolePayloadField::Emits`).
        target_field: RolePayloadField,
    },

    /// Rule asserts that the named field must be **non-empty** for matching
    /// entries.
    FieldNonEmpty {
        /// Catalogue field to check (e.g. `RolePayloadField::Emits`).
        target_field: RolePayloadField,
    },

    /// Rule constrains which layers entries of the target role may appear in.
    KindLayerConstraint {
        /// Layer IDs where the target role is permitted.
        permitted_layers: NonEmptyVec<LayerId>,
    },

    /// Rule asserts that the typed entries in `target_field` are declared with
    /// `expected_role` in the catalogue.
    ReferencedRoleConstraint {
        /// Field whose `TypeRef` entries are checked (e.g.
        /// `RolePayloadField::Emits`).
        target_field: RolePayloadField,
        /// The role that each referenced type must declare.
        expected_role: RoleKind,
    },

    /// Rule asserts that `trait_impls` contains all of `required_traits`.
    TraitImplRequired {
        /// Traits whose impl declarations are required (e.g. `TypeRef::new("PartialEq")`).
        required_traits: NonEmptyVec<TypeRef>,
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
        /// Field whose value is the referenced method name (currently only
        /// `RolePayloadField::Invariants` is supported).
        target_field: RolePayloadField,
    },

    /// Rule asserts that the entry has a public accessor getter matching the
    /// identity signature (`&self`, no params, non-unit return).
    AccessorSignatureRequired {
        /// Field that names the accessor (currently only
        /// `RolePayloadField::Identity` is supported).
        target_field: RolePayloadField,
    },

    /// Rule asserts that elements in `target_field` are unique across all
    /// entries of the target role (e.g. no two `AggregateRoot` share the same
    /// `exclusive_members` entry).
    FieldElementUniqueAcrossEntries {
        /// Field to check for cross-entry uniqueness (currently only
        /// `RolePayloadField::ExclusiveMembers` is supported).
        target_field: RolePayloadField,
    },

    /// Rule asserts that elements listed in `target_field` do not appear in
    /// any other entry's method signatures (external reference prohibition).
    NoExternalReferenceInMethods {
        /// Field whose listed types must not appear in other entries' method
        /// signatures (currently only `RolePayloadField::ExclusiveMembers` is
        /// supported).
        target_field: RolePayloadField,
    },

    /// Rule asserts that the entry has no public struct fields
    /// (`StructShape::Plain` / `StructShape::Tuple`). Unit variant.
    NoPublicField,

    /// Rule asserts that no method uses the given self-receiver kind.
    ForbiddenMethodReceiver {
        /// The receiver kind to forbid.
        forbidden_receiver: SelfReceiver,
    },

    /// Rule asserts that none of `primitives` occurs, at any of `positions`,
    /// inside the `TypeRef`-bearing catalogue-structural slots of entries
    /// (within `layers`) selected by the rule's `RuleTarget` (ADR
    /// `2026-07-01-0004` D1-D3).
    ///
    /// Unlike every other variant, this rule iterates its own `layers` field
    /// rather than being confined to a single evaluation-time target layer:
    /// each layer in `layers` is looked up in `all_catalogues` independently
    /// (erroring with [`CatalogueLinterError::UnknownLayer`] if absent), so a
    /// single rule can enforce a primitive-obsession ban across the whole
    /// workspace in one declaration.
    ///
    /// Role-axis filtering is deliberately omitted from this payload; it
    /// reuses [`RuleTarget::target_roles`] like every other rule kind (D1
    /// CN-03) rather than duplicating a role field here.
    ForbidPrimitiveInTypes {
        /// Primitive type names that must not occur (e.g. `String`).
        primitives: NonEmptyVec<PrimitiveName>,
        /// Layers to scan, independent of the evaluation-time target layer.
        layers: NonEmptyVec<LayerId>,
        /// Catalogue-structural positions to check for occurrences.
        positions: NonEmptyVec<PrimitiveOccurrencePosition>,
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
            Self::ForbidPrimitiveInTypes { .. } => "ForbidPrimitiveInTypes",
        }
    }
}

// ---------------------------------------------------------------------------
// FreeText — general-purpose free-text newtype (ADR 2026-07-01-0004 D4)
// ---------------------------------------------------------------------------

/// General-purpose newtype wrapping a genuinely free-text, unstructured
/// string with no finite value set or parseable format to validate (ADR
/// `2026-07-01-0004` D4).
///
/// Unlike [`crate::tddd::primitive_occurrence_scanner::PrimitiveName`] /
/// [`crate::tddd::catalogue_v2::identifiers::TypeRef`] / [`LayerId`]
/// (identifier-validated newtypes elsewhere in this catalogue, each rejecting
/// ill-formed input via a `Result`-returning constructor), `FreeText` has no
/// invariant beyond "is a string" to enforce, so [`FreeText::new`] is
/// infallible. Deliberately omits `PartialOrd` / `Ord` / `Copy`: it is never
/// stored as a sorted-collection element.
///
/// Used by [`CatalogueLinterError::InvalidRuleConfig`] and
/// [`CatalogueLinterRuleError::InvalidRuleConfig`] for ad hoc
/// rule-configuration-diagnostic messages assembled per call site with no
/// fixed vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FreeText(String);

impl FreeText {
    /// Constructs a `FreeText` from any string-like input. Infallible by
    /// design: unlike identifier-validated newtypes, there is no invariant to
    /// reject against.
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Returns the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FreeText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// CatalogueLinterRuleError — error type for constructor rejections
// ---------------------------------------------------------------------------

/// Errors that can be produced when constructing or validating a
/// [`CatalogueLinterRule`] or its constituent rule kinds.
///
/// `CatalogueLinterRule::new` never fails for `ForbiddenMethodReceiver` any
/// more: `forbidden_receiver` is `SelfReceiver`-typed (a closed 3-variant enum
/// with no invalid state), so an unparseable or empty receiver can no longer
/// reach this constructor at all — that class of failure now happens at the
/// usecase boundary, when a raw string is first parsed into a `SelfReceiver`.
///
/// The `EmptyPermittedLayers`, `EmptyRequiredTraits`, and `EmptyForbiddenRoles`
/// variants are not returned by `CatalogueLinterRule::new` itself, because
/// `KindLayerConstraint`, `TraitImplRequired`, and `NoRoleInMethodSignature`
/// carry `NonEmptyVec` payloads validated at variant-construction time.
/// These three variants exist for codec-layer conversions that need to signal
/// validation failures for the corresponding rule kinds.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CatalogueLinterRuleError {
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

    /// The rule configuration is internally inconsistent (e.g. a rule other
    /// than `KindLayerConstraint` targets a `FunctionRole` discriminant, or
    /// `NoRoleInMethodSignature` tries to forbid a `FunctionRole` discriminant
    /// that method-signature scanning cannot enforce).
    #[error("invalid rule configuration: {0}")]
    InvalidRuleConfig(FreeText),
}

// ---------------------------------------------------------------------------
// CatalogueLinterRule — value object
// ---------------------------------------------------------------------------

/// A single catalogue linter rule.
///
/// Constructed via [`CatalogueLinterRule::new`], which rejects ill-formed
/// combinations (e.g. a rule other than `KindLayerConstraint` targeting a
/// `FunctionRole` discriminant).
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
    /// Returns [`CatalogueLinterRuleError::InvalidRuleConfig`] when
    /// `NoRoleInMethodSignature` tries to forbid a `FunctionRole` discriminant
    /// that method-signature scanning cannot enforce, or when a rule other
    /// than `KindLayerConstraint` or `ForbidPrimitiveInTypes` targets a
    /// `FunctionRole` discriminant.
    pub fn new(
        target: RuleTarget,
        kind: CatalogueLinterRuleKind,
    ) -> Result<Self, CatalogueLinterRuleError> {
        if !matches!(
            &kind,
            CatalogueLinterRuleKind::KindLayerConstraint { .. }
                | CatalogueLinterRuleKind::ForbidPrimitiveInTypes { .. }
        ) {
            if let Some(function_role) = target
                .target_roles()
                .iter()
                .copied()
                .find(|role| RoleKind::FUNCTION_ROLES.contains(role))
            {
                return Err(CatalogueLinterRuleError::InvalidRuleConfig(FreeText::new(format!(
                    "{} cannot target FunctionRole '{}'; \
                     only KindLayerConstraint and ForbidPrimitiveInTypes support function entries",
                    kind.discriminant_name(),
                    function_role.variant_name()
                ))));
            }
        }

        // Validate per-kind invariants.
        match &kind {
            // `target_field` is now `RolePayloadField` (a closed enum),
            // `permitted_layers` / `required_traits` / `forbidden_roles` are
            // `NonEmptyVec`, `NoPublicField` is a unit variant, and
            // `forbidden_receiver` is `SelfReceiver` (a closed 3-variant enum
            // with no invalid state) — all validated by construction, so none
            // of these kinds needs an additional runtime check here.
            // `ForbidPrimitiveInTypes`'s `primitives` / `layers` / `positions`
            // are likewise `NonEmptyVec`, validated by construction.
            CatalogueLinterRuleKind::FieldEmpty { .. }
            | CatalogueLinterRuleKind::FieldNonEmpty { .. }
            | CatalogueLinterRuleKind::KindLayerConstraint { .. }
            | CatalogueLinterRuleKind::ReferencedRoleConstraint { .. }
            | CatalogueLinterRuleKind::TraitImplRequired { .. }
            | CatalogueLinterRuleKind::MethodReferenceSignature { .. }
            | CatalogueLinterRuleKind::AccessorSignatureRequired { .. }
            | CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries { .. }
            | CatalogueLinterRuleKind::NoExternalReferenceInMethods { .. }
            | CatalogueLinterRuleKind::NoPublicField
            | CatalogueLinterRuleKind::ForbiddenMethodReceiver { .. }
            | CatalogueLinterRuleKind::ForbidPrimitiveInTypes { .. } => {}
            CatalogueLinterRuleKind::NoRoleInMethodSignature { forbidden_roles } => {
                if let Some(function_role) = forbidden_roles
                    .as_slice()
                    .iter()
                    .copied()
                    .find(|role| RoleKind::FUNCTION_ROLES.contains(role))
                {
                    return Err(CatalogueLinterRuleError::InvalidRuleConfig(FreeText::new(
                        format!(
                            "NoRoleInMethodSignature cannot forbid FunctionRole '{}'; \
                         method signatures are checked against type and trait catalogue entries",
                            function_role.variant_name()
                        ),
                    )));
                }
            }
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
    InvalidRuleConfig(FreeText),

    /// The `all_catalogues` map does not contain an entry for the requested
    /// `target_layer_id`.
    #[error("unknown target layer '{layer_id}': not found in all_catalogues")]
    UnknownLayer { layer_id: LayerId },

    /// A [`crate::tddd::primitive_occurrence_scanner::PrimitiveOccurrenceScanner`]
    /// call made while evaluating `ForbidPrimitiveInTypes` failed.
    #[error(transparent)]
    ScanFailed(#[from] PrimitiveOccurrenceScanError),
}

// ---------------------------------------------------------------------------
// Internal helper functions and evaluation logic (split into submodules)
// ---------------------------------------------------------------------------

#[path = "catalogue_linter_helpers.rs"]
mod helpers;

#[path = "catalogue_linter_eval.rs"]
mod eval;

#[path = "catalogue_linter_eval_primitives.rs"]
mod eval_primitives;

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
    use crate::tddd::catalogue_v2::roles::{FunctionRole, NonEmptyVec};
    use crate::tddd::layer_id::LayerId;
    use crate::tddd::primitive_occurrence_scanner::{
        PrimitiveOccurrenceReport, PrimitiveOccurrenceScanner,
    };

    fn layer(name: &str) -> LayerId {
        LayerId::try_new(name.to_owned()).unwrap()
    }

    /// Test double for [`PrimitiveOccurrenceScanner`]: reports a requested
    /// primitive name as found, at the given call-site `position`, whenever
    /// it occurs as an exact substring of `type_ref`'s string form. No
    /// nested/recursive position reclassification (unlike the real
    /// `syn`-based adapter) -- this exists purely to exercise
    /// `evaluate_catalogue_lint`'s and `evaluate_forbid_primitive_in_types`'s
    /// own slot-collection and violation-emission logic in isolation, since
    /// domain tests cannot depend on the infrastructure-layer
    /// `SynPrimitiveOccurrenceScanner` (already covered by its own T004
    /// tests).
    struct StubPrimitiveScanner;

    impl PrimitiveOccurrenceScanner for StubPrimitiveScanner {
        fn scan(
            &self,
            type_ref: TypeRef,
            primitives: NonEmptyVec<PrimitiveName>,
            position: PrimitiveOccurrencePosition,
        ) -> Result<PrimitiveOccurrenceReport, PrimitiveOccurrenceScanError> {
            use std::collections::{BTreeMap, BTreeSet};

            let mut found = BTreeSet::new();
            for primitive in primitives.as_slice() {
                if type_ref.as_str().contains(primitive.as_str()) {
                    found.insert(primitive.clone());
                }
            }
            let mut occurrences = BTreeMap::new();
            if !found.is_empty() {
                occurrences.insert(position, found);
            }
            Ok(PrimitiveOccurrenceReport::new(occurrences))
        }
    }

    /// Test double for [`PrimitiveOccurrenceScanner`]: always fails with
    /// [`PrimitiveOccurrenceScanError::ParseFailure`], regardless of input.
    /// Exists solely to exercise `CatalogueLinterError::ScanFailed`
    /// propagation from `evaluate_forbid_primitive_in_types` through
    /// `evaluate_catalogue_lint`.
    struct FailingPrimitiveScanner;

    impl PrimitiveOccurrenceScanner for FailingPrimitiveScanner {
        fn scan(
            &self,
            type_ref: TypeRef,
            _primitives: NonEmptyVec<PrimitiveName>,
            _position: PrimitiveOccurrencePosition,
        ) -> Result<PrimitiveOccurrenceReport, PrimitiveOccurrenceScanError> {
            Err(PrimitiveOccurrenceScanError::ParseFailure { type_ref })
        }
    }

    /// Test double for [`PrimitiveOccurrenceScanner`]: fails with
    /// [`PrimitiveOccurrenceScanError::ParseFailure`] whenever called with
    /// `position == Bound`, and always succeeds (empty report) for every
    /// other position. Reproduces PR #179's finding: the real
    /// `syn`-based adapter parses every slot as a bare `syn::Type`, but a
    /// catalogue bound string may be a legal `syn::TypeParamBound` only
    /// (`?Sized`, a lifetime such as `'a`) that is not a valid `syn::Type` and
    /// so fails to parse -- this double reproduces that failure mode without
    /// pulling a `syn` dependency into a domain-only test.
    struct BoundOnlyFailingScanner;

    impl PrimitiveOccurrenceScanner for BoundOnlyFailingScanner {
        fn scan(
            &self,
            type_ref: TypeRef,
            _primitives: NonEmptyVec<PrimitiveName>,
            position: PrimitiveOccurrencePosition,
        ) -> Result<PrimitiveOccurrenceReport, PrimitiveOccurrenceScanError> {
            if position == PrimitiveOccurrencePosition::Bound {
                return Err(PrimitiveOccurrenceScanError::ParseFailure { type_ref });
            }
            Ok(PrimitiveOccurrenceReport::new(std::collections::BTreeMap::new()))
        }
    }

    /// Test double for [`PrimitiveOccurrenceScanner`]: rejects bound-only
    /// strings known not to be parseable as `syn::Type` (`?Sized`, lifetimes),
    /// but reports a requested primitive found inside a `Result<_, _>` bound at
    /// [`PrimitiveOccurrencePosition::ResultErr`].
    struct BoundResultErrScanner;

    impl PrimitiveOccurrenceScanner for BoundResultErrScanner {
        fn scan(
            &self,
            type_ref: TypeRef,
            primitives: NonEmptyVec<PrimitiveName>,
            position: PrimitiveOccurrencePosition,
        ) -> Result<PrimitiveOccurrenceReport, PrimitiveOccurrenceScanError> {
            if position == PrimitiveOccurrencePosition::Bound {
                let type_ref_text = type_ref.as_str();
                if type_ref_text.starts_with('?') || type_ref_text.starts_with('\'') {
                    return Err(PrimitiveOccurrenceScanError::ParseFailure { type_ref });
                }

                let mut found = std::collections::BTreeSet::new();
                for primitive in primitives.as_slice() {
                    if type_ref_text.contains("Result")
                        && type_ref_text.contains(primitive.as_str())
                    {
                        found.insert(primitive.clone());
                    }
                }

                let mut occurrences = std::collections::BTreeMap::new();
                if !found.is_empty() {
                    occurrences.insert(PrimitiveOccurrencePosition::ResultErr, found);
                }
                return Ok(PrimitiveOccurrenceReport::new(occurrences));
            }

            Ok(PrimitiveOccurrenceReport::new(std::collections::BTreeMap::new()))
        }
    }

    // ------------------------------------------------------------------
    // RoleKind — from_data_role covers all 17 DataRole variants
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
            ("CompositionRoot", RoleKind::CompositionRoot),
            ("PrimaryAdapter", RoleKind::PrimaryAdapter),
        ];
        assert_eq!(cases.len(), 17, "must cover all 17 DataRole variants");
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

    #[test]
    fn test_role_kind_from_function_role_covers_all_variants() {
        let cases = vec![
            (FunctionRole::FreeFunction, RoleKind::FreeFunction),
            (FunctionRole::UseCaseFunction, RoleKind::UseCaseFunction),
        ];

        assert_eq!(cases.len(), 2, "must cover all 2 FunctionRole variants");
        for (role, expected) in &cases {
            assert_eq!(
                RoleKind::from_function_role(role),
                *expected,
                "from_function_role mismatch for {:?}",
                role
            );
        }
    }

    // ------------------------------------------------------------------
    // CatalogueLinterRuleKind — 13 variants exist and discriminant_name works
    // ------------------------------------------------------------------

    #[test]
    fn test_catalogue_linter_rule_kind_has_13_variants_with_distinct_names() {
        let permitted = NonEmptyVec::new(layer("domain"), vec![]);
        let required_traits = NonEmptyVec::new(TypeRef::new("PartialEq").unwrap(), vec![]);
        let forbidden_roles = NonEmptyVec::new(RoleKind::Repository, vec![]);

        let kinds = vec![
            CatalogueLinterRuleKind::FieldEmpty { target_field: RolePayloadField::Invariants },
            CatalogueLinterRuleKind::FieldNonEmpty { target_field: RolePayloadField::Invariants },
            CatalogueLinterRuleKind::KindLayerConstraint { permitted_layers: permitted },
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: RolePayloadField::Emits,
                expected_role: RoleKind::DomainEvent,
            },
            CatalogueLinterRuleKind::TraitImplRequired { required_traits },
            CatalogueLinterRuleKind::NoRoleInMethodSignature { forbidden_roles },
            CatalogueLinterRuleKind::MethodReferenceSignature {
                target_field: RolePayloadField::Invariants,
            },
            CatalogueLinterRuleKind::AccessorSignatureRequired {
                target_field: RolePayloadField::Identity,
            },
            CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries {
                target_field: RolePayloadField::ExclusiveMembers,
            },
            CatalogueLinterRuleKind::NoExternalReferenceInMethods {
                target_field: RolePayloadField::ExclusiveMembers,
            },
            CatalogueLinterRuleKind::NoPublicField,
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: SelfReceiver::ExclusiveRef,
            },
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::NamedField, vec![]),
            },
        ];
        assert_eq!(kinds.len(), 13, "must have exactly 13 variants");

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
            "ForbidPrimitiveInTypes",
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
            CatalogueLinterRuleKind::FieldEmpty { target_field: RolePayloadField::Invariants },
        )
        .unwrap();
        assert_eq!(rule.kind().discriminant_name(), "FieldEmpty");
        assert!(rule.target().target_roles().is_empty(), "all_roles target should be empty vec");
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
    fn test_linter_rule_new_kind_layer_constraint_accepts_function_role_target() {
        let permitted = NonEmptyVec::new(layer("cli_driver"), vec![]);
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::FreeFunction]),
            CatalogueLinterRuleKind::KindLayerConstraint { permitted_layers: permitted },
        )
        .unwrap();

        assert_eq!(rule.target().target_roles(), &[RoleKind::FreeFunction]);
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
    fn test_linter_rule_new_no_public_field_rejects_function_role_target() {
        let result = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::UseCaseFunction]),
            CatalogueLinterRuleKind::NoPublicField,
        );

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterRuleError::InvalidRuleConfig(msg))
                    if msg.as_str().contains("NoPublicField") && msg.as_str().contains("UseCaseFunction")
            ),
            "expected InvalidRuleConfig for FunctionRole target, got: {result:?}"
        );
    }

    #[test]
    fn test_linter_rule_new_forbidden_method_receiver_succeeds() {
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::DomainEvent]),
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: SelfReceiver::ExclusiveRef,
            },
        )
        .unwrap();
        assert_eq!(rule.kind().discriminant_name(), "ForbiddenMethodReceiver");
    }

    #[test]
    fn test_linter_rule_new_forbidden_method_receiver_owned_round_trips() {
        // `SelfReceiver` is a closed 3-variant enum with no invalid state, so
        // construction always succeeds; this additionally verifies that the
        // exact payload variant (Owned, not just "some Ok value") round-trips
        // through `CatalogueLinterRule::new` / `rule.kind()` unchanged.
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::DomainEvent]),
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: SelfReceiver::Owned,
            },
        )
        .unwrap();
        assert_eq!(
            rule.kind(),
            &CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: SelfReceiver::Owned,
            }
        );
    }

    #[test]
    fn test_linter_rule_new_forbidden_method_receiver_shared_ref_round_trips() {
        // Same round-trip guarantee as the `Owned` case above, for `SharedRef`.
        // Typo'd / unsupported receiver strings (e.g. "&mutself") can no longer
        // reach this constructor at all — that parse failure now happens at the
        // usecase boundary (`parse_self_receiver`), before a domain value exists.
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::DomainEvent]),
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: SelfReceiver::SharedRef,
            },
        )
        .unwrap();
        assert_eq!(
            rule.kind(),
            &CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: SelfReceiver::SharedRef,
            }
        );
    }

    #[test]
    fn test_forbidden_method_receiver_accepts_all_self_receiver_variants() {
        // All three `SelfReceiver` variants must construct a valid rule.
        for variant in [SelfReceiver::Owned, SelfReceiver::SharedRef, SelfReceiver::ExclusiveRef] {
            let result = CatalogueLinterRule::new(
                RuleTarget::new(vec![RoleKind::DomainEvent]),
                CatalogueLinterRuleKind::ForbiddenMethodReceiver { forbidden_receiver: variant },
            );
            assert!(result.is_ok(), "expected Ok for receiver '{variant}', got: {result:?}");
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
            CatalogueLinterRuleKind::FieldNonEmpty { target_field: RolePayloadField::Emits },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner);
        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.as_str().contains("emits") && msg.as_str().contains("Entity")
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
            CatalogueLinterRuleKind::FieldEmpty { target_field: RolePayloadField::Emits },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner);
        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.as_str().contains("emits") && msg.as_str().contains("UseCase")
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
            CatalogueLinterRuleKind::FieldNonEmpty { target_field: RolePayloadField::Emits },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner);
        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.as_str().contains("emits") && msg.as_str().contains("all DataRole roles")
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
            CatalogueLinterRuleKind::FieldEmpty { target_field: RolePayloadField::Emits },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner);
        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.as_str().contains("emits") && msg.as_str().contains("all DataRole roles")
            ),
            "expected InvalidRuleConfig for all roles × emits FieldEmpty, got: {result:?}"
        );
    }

    #[test]
    fn test_linter_rule_new_trait_impl_required_succeeds_with_non_empty_vec() {
        // NonEmptyVec enforces non-emptiness at construction; CatalogueLinterRule::new
        // always succeeds for TraitImplRequired.
        let required_traits =
            NonEmptyVec::new(TypeRef::new("PartialEq").unwrap(), vec![TypeRef::new("Eq").unwrap()]);
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::TraitImplRequired { required_traits },
        )
        .unwrap();
        assert_eq!(rule.kind().discriminant_name(), "TraitImplRequired");
    }

    #[test]
    fn test_linter_rule_new_no_role_in_method_signature_succeeds_with_non_empty_vec() {
        // NonEmptyVec enforces non-emptiness at construction; data and contract
        // roles are valid for method-signature checks.
        let forbidden_roles = NonEmptyVec::new(RoleKind::Repository, vec![RoleKind::SecondaryPort]);
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::ValueObject, RoleKind::Entity]),
            CatalogueLinterRuleKind::NoRoleInMethodSignature { forbidden_roles },
        )
        .unwrap();
        assert_eq!(rule.kind().discriminant_name(), "NoRoleInMethodSignature");
    }

    #[test]
    fn test_linter_rule_new_no_role_in_method_signature_rejects_function_role() {
        let forbidden_roles = NonEmptyVec::new(RoleKind::FreeFunction, vec![]);
        let result = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::NoRoleInMethodSignature { forbidden_roles },
        );

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterRuleError::InvalidRuleConfig(msg))
                    if msg.as_str().contains("FreeFunction")
            ),
            "expected InvalidRuleConfig for FunctionRole forbidden role, got: {result:?}"
        );
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
        let violations =
            evaluate_catalogue_lint(&[rule], &all, &layer_id, &StubPrimitiveScanner).unwrap();
        assert!(violations.is_empty(), "T008 skeleton must return empty violations");
    }

    // ------------------------------------------------------------------
    // CatalogueLinterError::InvalidRuleConfig — constructor test
    // ------------------------------------------------------------------

    #[test]
    fn test_catalogue_linter_error_invalid_rule_config_stores_message() {
        let err = CatalogueLinterError::InvalidRuleConfig(FreeText::new("contradictory rule set"));
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
    use crate::tddd::catalogue_v2::entries::{
        FunctionEntry, InherentImplDeclV2, TraitEntry, TypeEntry,
    };
    use crate::tddd::catalogue_v2::identifiers::{
        CrateName, FieldName, FunctionName, FunctionPath, InvariantName, MethodName, ModulePath,
        ParamName, TraitName, TypeName, TypeRef, VariantName,
    };
    use crate::tddd::catalogue_v2::methods::{
        BoundOp, MethodDeclaration, MethodGenericParam, ParamDeclaration, WherePredicateDecl,
    };
    use crate::tddd::catalogue_v2::roles::{
        ContractRole, DataRole, IdentityAccessor, InvariantDecl, InvariantPredicate, ItemAction,
        SelfReceiver,
    };
    use crate::tddd::catalogue_v2::traits::TraitImplDeclV2;
    use crate::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl};

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

    fn make_function_entry(role: FunctionRole) -> FunctionEntry {
        FunctionEntry {
            action: ItemAction::Add,
            role,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![],
            where_predicates: vec![],
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
        evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner).unwrap()
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
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner);

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.as_str().contains("exclusive_members") && msg.as_str().contains("Entity")
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
            CatalogueLinterRuleKind::FieldEmpty { target_field: RolePayloadField::Emits },
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
            CatalogueLinterRuleKind::FieldEmpty { target_field: RolePayloadField::Emits },
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
            CatalogueLinterRuleKind::FieldEmpty { target_field: RolePayloadField::Emits },
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner);

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.as_str().contains("emits") && msg.as_str().contains("Repository")
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
            CatalogueLinterRuleKind::FieldNonEmpty { target_field: RolePayloadField::Handles },
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
            CatalogueLinterRuleKind::FieldNonEmpty { target_field: RolePayloadField::Handles },
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
            CatalogueLinterRuleKind::FieldNonEmpty { target_field: RolePayloadField::Emits },
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner);

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.as_str().contains("emits") && msg.as_str().contains("Repository")
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

    #[test]
    fn test_kind_layer_constraint_violation_when_function_layer_is_not_permitted() {
        let mut doc = make_doc("usecase");
        let function_path = FunctionPath::at_root(
            CrateName::new("domain").unwrap(),
            FunctionName::new("register_user").unwrap(),
        );
        doc.functions
            .insert(function_path.clone(), make_function_entry(FunctionRole::FreeFunction));

        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::FreeFunction]),
            CatalogueLinterRuleKind::KindLayerConstraint {
                permitted_layers: NonEmptyVec::new(layer("domain"), vec![]),
            },
        );

        assert_eq!(violations.len(), 1, "expected 1 violation for FreeFunction in usecase layer");
        assert_eq!(violations[0].rule_kind(), "KindLayerConstraint");
        let function_path_name = function_path.to_string();
        assert_eq!(violations[0].entry_name(), function_path_name.as_str());
        assert!(
            violations[0].message().contains("usecase"),
            "expected message to mention disallowed layer"
        );
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
                target_field: RolePayloadField::Emits,
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
                target_field: RolePayloadField::Emits,
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
                target_field: RolePayloadField::Emits,
                expected_role: RoleKind::DomainEvent,
            },
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner);

        assert!(
            matches!(&result, Err(CatalogueLinterError::InvalidRuleConfig(msg)) if msg.as_str().contains("emits")),
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
                required_traits: NonEmptyVec::new(
                    TypeRef::new("PartialEq").unwrap(),
                    vec![TypeRef::new("Eq").unwrap()],
                ),
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
                required_traits: NonEmptyVec::new(
                    TypeRef::new("PartialEq").unwrap(),
                    vec![TypeRef::new("Eq").unwrap()],
                ),
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
                target_field: RolePayloadField::Invariants,
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
                target_field: RolePayloadField::Invariants,
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
                target_field: RolePayloadField::Invariants,
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
                target_field: RolePayloadField::Invariants,
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
                target_field: RolePayloadField::Identity,
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
                target_field: RolePayloadField::Identity,
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
                target_field: RolePayloadField::Identity,
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
                target_field: RolePayloadField::ExclusiveMembers,
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
                target_field: RolePayloadField::ExclusiveMembers,
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
                target_field: RolePayloadField::Emits,
            },
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner);

        assert!(
            matches!(&result, Err(CatalogueLinterError::InvalidRuleConfig(msg)) if msg.as_str().contains("emits")),
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
                target_field: RolePayloadField::ExclusiveMembers,
            },
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner);

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.as_str().contains("exclusive_members") && msg.as_str().contains("Entity")
            ),
            "expected InvalidRuleConfig for Entity target with exclusive_members, got: {result:?}"
        );
    }

    #[test]
    fn test_field_element_unique_across_entries_rejects_mixed_target_role_that_does_not_carry_field()
     {
        assert_mixed_aggregate_entity_target_without_exclusive_members_rejected(
            CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries {
                target_field: RolePayloadField::ExclusiveMembers,
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
                target_field: RolePayloadField::ExclusiveMembers,
            },
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner);

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.as_str().contains("exclusive_members") && msg.as_str().contains("all roles")
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
                target_field: RolePayloadField::ExclusiveMembers,
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
                target_field: RolePayloadField::ExclusiveMembers,
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
                target_field: RolePayloadField::ExclusiveMembers,
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
                target_field: RolePayloadField::ExclusiveMembers,
            },
        )
        .unwrap();

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner);

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.as_str().contains("exclusive_members") && msg.as_str().contains("Entity")
            ),
            "expected InvalidRuleConfig for Entity target with exclusive_members, got: {result:?}"
        );
    }

    #[test]
    fn test_no_external_reference_in_methods_rejects_mixed_target_role_that_does_not_carry_field() {
        assert_mixed_aggregate_entity_target_without_exclusive_members_rejected(
            CatalogueLinterRuleKind::NoExternalReferenceInMethods {
                target_field: RolePayloadField::ExclusiveMembers,
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
                forbidden_receiver: SelfReceiver::ExclusiveRef,
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
                forbidden_receiver: SelfReceiver::ExclusiveRef,
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
                forbidden_receiver: SelfReceiver::ExclusiveRef,
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
                forbidden_receiver: SelfReceiver::ExclusiveRef,
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
                forbidden_receiver: SelfReceiver::ExclusiveRef,
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner);

        assert!(
            matches!(
                &result,
                Err(CatalogueLinterError::InvalidRuleConfig(msg))
                    if msg.as_str().contains("set_payload") && msg.as_str().contains("OrderPlaced")
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
                required_traits: NonEmptyVec::new(
                    TypeRef::new("PartialEq").unwrap(),
                    vec![TypeRef::new("Eq").unwrap()],
                ),
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let target_layer = layer("domain");
        let violations =
            evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner).unwrap();
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
                required_traits: NonEmptyVec::new(
                    TypeRef::new("PartialEq").unwrap(),
                    vec![TypeRef::new("Eq").unwrap()],
                ),
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let target_layer = layer("domain");
        let violations =
            evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner).unwrap();
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
                target_field: RolePayloadField::ExclusiveMembers,
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
                target_field: RolePayloadField::SharedValueObjects,
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
                target_field: RolePayloadField::Aggregate,
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
                target_field: RolePayloadField::Aggregate,
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
                target_field: RolePayloadField::ReactsTo,
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
                forbidden_receiver: SelfReceiver::ExclusiveRef,
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
                forbidden_receiver: SelfReceiver::ExclusiveRef,
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
                required_traits: NonEmptyVec::new(
                    TypeRef::new("PartialEq").unwrap(),
                    vec![TypeRef::new("Eq").unwrap()],
                ),
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
                target_field: RolePayloadField::Handles,
                expected_role: RoleKind::DomainEvent,
            },
        )
        .unwrap();
        let violations =
            evaluate_catalogue_lint(&[rule], &all, &usecase_layer, &StubPrimitiveScanner).unwrap();
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
                target_field: RolePayloadField::Handles,
                expected_role: RoleKind::DomainEvent,
            },
        )
        .unwrap();
        let violations =
            evaluate_catalogue_lint(&[rule], &all, &usecase_layer, &StubPrimitiveScanner).unwrap();
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
        let violations =
            evaluate_catalogue_lint(&[rule], &all, &usecase_layer, &StubPrimitiveScanner).unwrap();
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
        let result = evaluate_catalogue_lint(&[rule], &all, &target, &StubPrimitiveScanner);
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
                target_field: RolePayloadField::Handles,
                expected_role: RoleKind::DomainEvent,
            },
        )
        .unwrap();
        let violations =
            evaluate_catalogue_lint(&[rule], &all, &usecase_layer, &StubPrimitiveScanner).unwrap();
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
        let violations =
            evaluate_catalogue_lint(&[rule], &all, &usecase_layer, &StubPrimitiveScanner).unwrap();
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
        let violations =
            evaluate_catalogue_lint(&[rule], &all, &usecase_layer, &StubPrimitiveScanner).unwrap();
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
                required_traits: NonEmptyVec::new(TypeRef::new("PartialEq").unwrap(), vec![]),
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
                CatalogueLinterRuleKind::FieldNonEmpty {
                    target_field: RolePayloadField::Invariants,
                },
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

    #[test]
    fn test_type_entries_for_target_skips_reference_action_entry_for_trait_impl_required() {
        // A TypeEntry with action: Reference cites a pre-existing type without
        // restating its structure: in particular, the trait impls established
        // when the type was originally declared (Add/Modify) are not repeated
        // in this catalogue's `trait_impls` list for a Reference entry. This
        // reproduces the real-world false positive: a ValueObject reference
        // entry with no matching `trait_impls` entries in this catalogue must
        // not be flagged as missing PartialEq, because a Reference entry is
        // opaque to this catalogue's rule evaluations.
        let mut doc = make_doc("domain");
        let reference_entry = TypeEntry {
            action: ItemAction::Reference,
            role: DataRole::value_object(),
            kind: unit_struct_kind(),
            methods: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.types.insert(TypeName::new("ReferencedValue").unwrap(), reference_entry);
        // Deliberately no TraitImplDeclV2 pushed to doc.trait_impls: a
        // Reference entry's trait impls are not restated in this catalogue.

        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::ValueObject]),
            CatalogueLinterRuleKind::TraitImplRequired {
                required_traits: NonEmptyVec::new(TypeRef::new("PartialEq").unwrap(), vec![]),
            },
        );
        assert!(
            violations.is_empty(),
            "TraitImplRequired: expected no violations — Reference-action entry must be \
             skipped (its trait_impls are declared in the catalogue that originally added \
             the type, not restated here), got: {violations:?}"
        );
    }

    // ===========================================================================
    // D19 fail-closed: unknown target_field rejects with InvalidRuleConfig
    // ===========================================================================

    #[test]
    fn test_evaluate_catalogue_lint_wrong_category_target_field_returns_invalid_rule_config() {
        // `RolePayloadField` is a closed enum, so an arbitrary unrecognised string
        // (e.g. a typo like "emit") is no longer representable at the call site —
        // the type system rejects it at compile time instead of at runtime. The
        // residual, still-representable failure mode is a syntactically valid but
        // wrong-category field: `Aggregate` is a `ContractRole`-only concept, so
        // using it as a `FieldEmpty` target on a `DataRole` must still return
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
            CatalogueLinterRuleKind::FieldEmpty { target_field: RolePayloadField::Aggregate },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let result =
            evaluate_catalogue_lint(&[rule], &all, &doc.layer.clone(), &StubPrimitiveScanner);
        let is_invalid = matches!(&result, Err(CatalogueLinterError::InvalidRuleConfig(msg)) if msg.as_str().contains("aggregate"));
        assert!(
            is_invalid,
            "expected Err(InvalidRuleConfig) for wrong-category target_field 'aggregate', got: {result:?}"
        );
    }

    #[test]
    fn test_evaluate_catalogue_lint_wrong_category_target_field_for_referenced_role_constraint_returns_error()
     {
        // `RolePayloadField` is a closed enum, so an arbitrary unrecognised string
        // (e.g. a typo like "handle") is no longer representable at the call site.
        // The residual failure mode is a syntactically valid but wrong-category
        // field: `Identity` is neither a valid `DataRole` field (per
        // `validate_data_role_field`) nor the `ContractRole` field `aggregate`
        // (per `validate_contract_role_field`), so ReferencedRoleConstraint must
        // still return Err(InvalidRuleConfig) rather than silently reporting zero
        // violations.
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
                target_field: RolePayloadField::Identity,
                expected_role: RoleKind::DomainEvent,
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let result =
            evaluate_catalogue_lint(&[rule], &all, &doc.layer.clone(), &StubPrimitiveScanner);
        let is_invalid = matches!(&result, Err(CatalogueLinterError::InvalidRuleConfig(msg)) if msg.as_str().contains("identity"));
        assert!(
            is_invalid,
            "expected Err(InvalidRuleConfig) for wrong-category target_field 'identity', got: {result:?}"
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
                target_field: RolePayloadField::Invariants,
                expected_role: RoleKind::DomainEvent,
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let result =
            evaluate_catalogue_lint(&[rule], &all, &doc.layer.clone(), &StubPrimitiveScanner);
        let is_invalid = matches!(
            &result,
            Err(CatalogueLinterError::InvalidRuleConfig(msg))
                if msg.as_str().contains("ReferencedRoleConstraint") && msg.as_str().contains("invariants")
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
                target_field: RolePayloadField::Emits,
                expected_role: RoleKind::DomainEvent,
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let result =
            evaluate_catalogue_lint(&[rule], &all, &doc.layer.clone(), &StubPrimitiveScanner);
        let is_invalid = matches!(
            &result,
            Err(CatalogueLinterError::InvalidRuleConfig(msg))
                if msg.as_str().contains("emits") && msg.as_str().contains("Entity")
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
                target_field: RolePayloadField::Aggregate,
                expected_role: RoleKind::AggregateRoot,
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let result =
            evaluate_catalogue_lint(&[rule], &all, &doc.layer.clone(), &StubPrimitiveScanner);
        let is_invalid = matches!(
            &result,
            Err(CatalogueLinterError::InvalidRuleConfig(msg))
                if msg.as_str().contains("aggregate") && msg.as_str().contains("SpecificationPort")
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
        // field_type_refs must return an empty slice (not an error) so that a
        // ReferencedRoleConstraint rule whose RuleTarget covers both DataRole and
        // ContractRole entries can still evaluate the ContractRole trait entries.
        use super::helpers::field_type_refs;
        let role = DataRole::AggregateRoot {
            identity: identity_accessor("id"),
            invariants: vec![],
            exclusive_members: vec![TypeRef::new("OrderLine").unwrap()],
            shared_value_objects: vec![],
            emits: vec![],
        };
        let result = field_type_refs(&role, RolePayloadField::Aggregate);
        assert!(
            result.is_empty(),
            "expected an empty slice for ContractRole-only field 'aggregate' on DataRole, got: {result:?}"
        );
    }

    #[test]
    fn test_field_type_refs_field_not_carried_by_role_returns_empty_slice() {
        // `RolePayloadField` is a closed enum, so a "truly unknown field name"
        // (an arbitrary string not matching any variant) is no longer
        // representable at the call site — the type system rejects it at compile
        // time instead of at runtime, and `field_type_refs` is now infallible.
        // The residual, still-representable case is a syntactically valid field
        // that the given role simply does not carry (e.g. `ReactsTo`, which only
        // `EventPolicy` carries, applied to a `DomainService`), which returns an
        // empty slice rather than an error.
        use super::helpers::field_type_refs;
        let role = DataRole::DomainService { emits: vec![] };
        let result = field_type_refs(&role, RolePayloadField::ReactsTo);
        assert!(
            result.is_empty(),
            "expected an empty slice for field 'reacts_to' not carried by DomainService, got: {result:?}"
        );
    }

    #[test]
    fn test_evaluate_catalogue_lint_field_not_carried_by_repository_for_referenced_role_constraint_returns_error()
     {
        // `RolePayloadField` is a closed enum, so an arbitrary unrecognised string
        // (e.g. a typo like "aggregat") is no longer representable at the call
        // site, and `contract_role_type_ref` is now infallible — it can no longer
        // return an error for an unrecognised field name. The residual failure
        // mode for a trait (ContractRole) target is a syntactically valid field
        // that the target role cannot carry at all: `Emits` is only carried by
        // `AggregateRoot` / `DomainService` (via `carries_type_ref_field`), never
        // by `Repository`, so `ensure_target_can_produce_type_ref_checks` must
        // still reject it with Err(InvalidRuleConfig) rather than silently
        // reporting zero violations.
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
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::Repository]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: RolePayloadField::Emits,
                expected_role: RoleKind::AggregateRoot,
            },
        )
        .unwrap();
        let all = all_catalogues_single(&doc);
        let result =
            evaluate_catalogue_lint(&[rule], &all, &doc.layer.clone(), &StubPrimitiveScanner);
        let is_invalid = matches!(
            &result,
            Err(CatalogueLinterError::InvalidRuleConfig(msg))
                if msg.as_str().contains("emits") && msg.as_str().contains("Repository")
        );
        assert!(
            is_invalid,
            "expected Err(InvalidRuleConfig) for target_field 'emits' not carried by role \
             'Repository', got: {result:?}"
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
                target_field: RolePayloadField::ExclusiveMembers,
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

    // ===========================================================================
    // T005: Rule 13 — ForbidPrimitiveInTypes
    // ===========================================================================

    #[test]
    fn test_forbid_primitive_in_types_detects_named_field_occurrence() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Money").unwrap(),
            make_type_entry_with_kind(
                DataRole::value_object(),
                plain_struct_kind(vec![field_decl("amount", "String")]),
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::NamedField, vec![]),
            },
        );
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation for String named field, got: {violations:?}"
        );
        assert_eq!(violations[0].rule_kind(), "ForbidPrimitiveInTypes");
        assert_eq!(violations[0].entry_name(), "Money");
        assert!(violations[0].message().contains("String"));
    }

    #[test]
    fn test_forbid_primitive_in_types_no_violation_when_primitive_absent() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Money").unwrap(),
            make_type_entry_with_kind(
                DataRole::value_object(),
                plain_struct_kind(vec![field_decl("amount", "Decimal")]),
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::NamedField, vec![]),
            },
        );
        assert!(
            violations.is_empty(),
            "expected no violations when the field type does not contain the forbidden primitive"
        );
    }

    #[test]
    fn test_forbid_primitive_in_types_detects_variant_field_occurrence() {
        // Enum with both a Tuple-payload variant and a Struct-payload variant,
        // each carrying a String — both must be classified as VariantField.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("PaymentEvent").unwrap(),
            make_type_entry_with_kind(
                DataRole::DomainEvent,
                TypeKindV2::Enum {
                    variants: vec![
                        VariantDecl::tuple(
                            VariantName::new("Charged").unwrap(),
                            vec![TypeRef::new("String").unwrap()],
                        ),
                        VariantDecl::struct_variant(
                            VariantName::new("Refunded").unwrap(),
                            vec![field_decl("reason", "String")],
                        ),
                    ],
                },
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::VariantField, vec![]),
            },
        );
        assert_eq!(
            violations.len(),
            2,
            "expected 1 violation per variant (Tuple payload + Struct payload), got: {violations:?}"
        );
        assert!(violations.iter().all(|v| v.rule_kind() == "ForbidPrimitiveInTypes"));
        assert!(violations.iter().all(|v| v.entry_name() == "PaymentEvent"));
    }

    #[test]
    fn test_forbid_primitive_in_types_detects_type_alias_target_occurrence() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Description").unwrap(),
            make_type_entry_with_kind(
                DataRole::value_object(),
                TypeKindV2::TypeAlias { target: TypeRef::new("String").unwrap() },
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::TypeAliasTarget, vec![]),
            },
        );
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation for the type_alias target, got: {violations:?}"
        );
        assert_eq!(violations[0].entry_name(), "Description");
    }

    #[test]
    fn test_forbid_primitive_in_types_detects_param_and_return_on_type_method() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Money").unwrap(),
            make_type_entry_with_methods(
                DataRole::value_object(),
                vec![method_with_params(
                    "rename",
                    Some(SelfReceiver::ExclusiveRef),
                    vec![("new_name", "String")],
                    "String",
                )],
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(
                    PrimitiveOccurrencePosition::Param,
                    vec![PrimitiveOccurrencePosition::Return],
                ),
            },
        );
        assert_eq!(
            violations.len(),
            2,
            "expected 1 violation for the String param + 1 for the String return, got: {violations:?}"
        );
        assert!(violations.iter().all(|v| v.entry_name() == "Money"));
    }

    #[test]
    fn test_forbid_primitive_in_types_detects_param_and_return_on_trait_method() {
        let mut doc = make_doc("domain");
        let mut trait_entry = make_trait_entry(ContractRole::SpecificationPort);
        trait_entry.methods.push(method_with_params(
            "check",
            Some(SelfReceiver::SharedRef),
            vec![("input", "String")],
            "String",
        ));
        doc.traits.insert(TraitName::new("Checker").unwrap(), trait_entry);

        let violations = run_rule(
            &doc,
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(
                    PrimitiveOccurrencePosition::Param,
                    vec![PrimitiveOccurrencePosition::Return],
                ),
            },
        );
        assert_eq!(
            violations.len(),
            2,
            "expected 1 violation for the String param + 1 for the String return, got: {violations:?}"
        );
        assert!(violations.iter().all(|v| v.entry_name() == "Checker"));
    }

    #[test]
    fn test_forbid_primitive_in_types_detects_param_and_return_on_free_function() {
        let mut doc = make_doc("domain");
        let entry = FunctionEntry {
            params: vec![ParamDeclaration::new(
                ParamName::new("value").unwrap(),
                TypeRef::new("String").unwrap(),
            )],
            returns: TypeRef::new("String").unwrap(),
            ..make_function_entry(FunctionRole::FreeFunction)
        };
        doc.functions.insert(
            FunctionPath::at_root(
                CrateName::new("domain").unwrap(),
                FunctionName::new("do_thing").unwrap(),
            ),
            entry,
        );

        let violations = run_rule(
            &doc,
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(
                    PrimitiveOccurrencePosition::Param,
                    vec![PrimitiveOccurrencePosition::Return],
                ),
            },
        );
        assert_eq!(
            violations.len(),
            2,
            "expected 1 violation for the String param + 1 for the String return, got: {violations:?}"
        );
        assert!(violations.iter().all(|v| v.entry_name().contains("do_thing")));
    }

    #[test]
    fn test_forbid_primitive_in_types_detects_bound_occurrence() {
        // A generic bound (`T: Into<String>`), a where-predicate lhs
        // (`Vec<String>: Clone`), and a where-predicate rhs (`U: String`) on
        // the same method — all must be classified as Bound.
        let mut doc = make_doc("domain");
        let method = MethodDeclaration {
            generics: vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![TypeRef::new("Into<String>").unwrap()],
            }],
            where_predicates: vec![WherePredicateDecl {
                lhs: TypeRef::new("Vec<String>").unwrap(),
                rhs: vec![TypeRef::new("String").unwrap()],
                operator: BoundOp::Bound,
            }],
            ..method_shared_ref_no_params("do_thing", "()")
        };
        doc.types.insert(
            TypeName::new("Money").unwrap(),
            make_type_entry_with_methods(DataRole::value_object(), vec![method]),
        );

        let violations = run_rule(
            &doc,
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::Bound, vec![]),
            },
        );
        assert_eq!(
            violations.len(),
            3,
            "expected 1 violation for the generic bound + 1 for the where-predicate lhs \
             + 1 for the where-predicate rhs, \
             got: {violations:?}"
        );
        assert!(violations.iter().all(|v| v.entry_name() == "Money"));
    }

    #[test]
    fn test_forbid_primitive_in_types_respects_positions_filter() {
        // The String occurrence is at NamedField, but the rule only requests Param —
        // it must not be reported.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Money").unwrap(),
            make_type_entry_with_kind(
                DataRole::value_object(),
                plain_struct_kind(vec![field_decl("amount", "String")]),
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::Param, vec![]),
            },
        );
        assert!(
            violations.is_empty(),
            "a NamedField occurrence must not be reported when positions only requests Param"
        );
    }

    #[test]
    fn test_forbid_primitive_in_types_respects_role_target_filter() {
        // CN-03: role-axis filtering is deliberately omitted from the rule's own
        // payload, reusing RuleTarget.target_roles instead. A target that only
        // selects Entity must skip a ValueObject entry entirely.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Money").unwrap(),
            make_type_entry_with_kind(
                DataRole::value_object(),
                plain_struct_kind(vec![field_decl("amount", "String")]),
            ),
        );
        let violations = run_rule(
            &doc,
            RuleTarget::new(vec![RoleKind::Entity]),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::NamedField, vec![]),
            },
        );
        assert!(
            violations.is_empty(),
            "a ValueObject entry must be skipped when the target only selects Entity"
        );
    }

    #[test]
    fn test_forbid_primitive_in_types_fires_only_when_target_layer_in_rule_layers() {
        // A rule with layers=[domain, usecase] fires per-layer via the caller's
        // repeated invocation, once per target_layer_id. Each call yields the
        // violations for that layer only — the rule does NOT internally iterate
        // its own `layers` list (that would double-count when the composition
        // root already loops over layers).
        let mut domain_doc = make_doc("domain");
        domain_doc.types.insert(
            TypeName::new("Money").unwrap(),
            make_type_entry_with_kind(
                DataRole::value_object(),
                plain_struct_kind(vec![field_decl("amount", "String")]),
            ),
        );
        let mut usecase_doc = make_doc("usecase");
        usecase_doc.types.insert(
            TypeName::new("MoneyDto").unwrap(),
            make_type_entry_with_kind(
                DataRole::value_object(),
                plain_struct_kind(vec![field_decl("amount", "String")]),
            ),
        );
        let mut all = BTreeMap::new();
        all.insert(domain_doc.layer.clone(), domain_doc.clone());
        all.insert(usecase_doc.layer.clone(), usecase_doc.clone());

        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![layer("usecase")]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::NamedField, vec![]),
            },
        )
        .unwrap();

        let violations_domain = evaluate_catalogue_lint(
            std::slice::from_ref(&rule),
            &all,
            &layer("domain"),
            &StubPrimitiveScanner,
        )
        .unwrap();
        assert_eq!(violations_domain.len(), 1, "expected 1 domain violation");
        assert_eq!(violations_domain[0].entry_name(), "Money");

        let violations_usecase =
            evaluate_catalogue_lint(&[rule], &all, &layer("usecase"), &StubPrimitiveScanner)
                .unwrap();
        assert_eq!(violations_usecase.len(), 1, "expected 1 usecase violation");
        assert_eq!(violations_usecase[0].entry_name(), "MoneyDto");
    }

    #[test]
    fn test_forbid_primitive_in_types_skips_when_target_layer_not_in_rule_layers() {
        // A rule scoped to layers=[usecase] must be a no-op when the caller
        // targets layer_id=domain (the composition-root loop invokes the rule
        // once per layer; layers not in the rule's own list are skipped).
        let mut domain_doc = make_doc("domain");
        domain_doc.types.insert(
            TypeName::new("Money").unwrap(),
            make_type_entry_with_kind(
                DataRole::value_object(),
                plain_struct_kind(vec![field_decl("amount", "String")]),
            ),
        );
        let usecase_doc = make_doc("usecase");
        let mut all = all_catalogues_single(&domain_doc);
        all.insert(usecase_doc.layer.clone(), usecase_doc);
        let target_layer = domain_doc.layer.clone();
        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("usecase"), vec![]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::NamedField, vec![]),
            },
        )
        .unwrap();
        let violations =
            evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner).unwrap();
        assert!(
            violations.is_empty(),
            "rule with layers=[usecase] must not fire on target_layer=domain, got: {violations:?}"
        );
    }

    #[test]
    fn test_forbid_primitive_in_types_unknown_configured_layer_errors_before_skip() {
        // Every configured layer must exist in all_catalogues, even when the
        // current target_layer_id is outside the rule's layers. Otherwise a
        // typo in the rule config could be silently skipped by every per-layer
        // evaluation call.
        let domain_doc = make_doc("domain");
        let all = all_catalogues_single(&domain_doc);
        let target_layer = domain_doc.layer.clone();
        let missing_layer = layer("usecase");
        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(missing_layer.clone(), vec![]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::NamedField, vec![]),
            },
        )
        .unwrap();

        let result = evaluate_catalogue_lint(&[rule], &all, &target_layer, &StubPrimitiveScanner);

        match result {
            Err(CatalogueLinterError::UnknownLayer { layer_id }) => {
                assert_eq!(layer_id, missing_layer);
            }
            other => panic!("expected UnknownLayer for missing configured layer, got: {other:?}"),
        }
    }

    #[test]
    fn test_forbid_primitive_in_types_scan_failed_propagates_as_catalogue_linter_error() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Money").unwrap(),
            make_type_entry_with_kind(
                DataRole::value_object(),
                plain_struct_kind(vec![field_decl("amount", "String")]),
            ),
        );
        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::NamedField, vec![]),
            },
        )
        .unwrap();
        let result =
            evaluate_catalogue_lint(&[rule], &all, &target_layer, &FailingPrimitiveScanner);
        let is_scan_failed = matches!(&result, Err(CatalogueLinterError::ScanFailed(_)));
        assert!(
            is_scan_failed,
            "expected ScanFailed error propagated from the scanner, got: {result:?}"
        );
    }

    // ------------------------------------------------------------------
    // PR #179 P1 regression: a `?Sized` / lifetime bound must not be scanned
    // when the rule's `positions` does not request `Bound`. `push_generic_
    // and_where_slots` collected bound slots unconditionally, so a legal
    // catalogue bound like `?Sized` (a `syn::TypeParamBound`, not a
    // `syn::Type`) reached the scanner even for rules that never asked to
    // check `Bound`, failing the shipped default `result_err` /
    // `named_field`+`variant_field` rules with `ScanFailed`.
    // ------------------------------------------------------------------

    #[test]
    fn test_forbid_primitive_in_types_skips_method_generic_bound_when_bound_not_requested() {
        // `T: ?Sized` on a method generic is legal at the catalogue layer
        // (MethodGenericParam::bounds docs list `?Sized` as an example) but
        // is not parseable as a `syn::Type`. positions requests only
        // NamedField/VariantField/ResultErr (mirrors the shipped default
        // rules), so the Bound slot must be skipped at collection time and
        // never reach `BoundOnlyFailingScanner` with position == Bound.
        let mut doc = make_doc("domain");
        let method = MethodDeclaration {
            generics: vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![TypeRef::new("?Sized").unwrap()],
            }],
            ..method_shared_ref_no_params("do_thing", "()")
        };
        doc.types.insert(
            TypeName::new("Money").unwrap(),
            make_type_entry_with_methods(DataRole::value_object(), vec![method]),
        );

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(
                    PrimitiveOccurrencePosition::NamedField,
                    vec![
                        PrimitiveOccurrencePosition::VariantField,
                        PrimitiveOccurrencePosition::ResultErr,
                    ],
                ),
            },
        )
        .unwrap();

        let result =
            evaluate_catalogue_lint(&[rule], &all, &target_layer, &BoundOnlyFailingScanner);

        assert!(
            result.is_ok(),
            "a `?Sized` generic-bound slot must be skipped at collection time when Bound is \
             not requested, so evaluation must not fail with ScanFailed; got: {result:?}"
        );
    }

    #[test]
    fn test_forbid_primitive_in_types_skips_trait_supertrait_bound_when_bound_not_requested() {
        // A trait's `supertrait_bounds` (e.g. `trait Foo: ?Sized`) is pushed
        // directly in `collect_trait_entry_slots`, not via
        // `push_generic_and_where_slots` -- a distinct code path that must
        // also skip Bound slots when Bound is not requested.
        let mut doc = make_doc("domain");
        let trait_entry = TraitEntry {
            supertrait_bounds: vec![TypeRef::new("?Sized").unwrap()],
            ..make_trait_entry(ContractRole::SpecificationPort)
        };
        doc.traits.insert(TraitName::new("Checker").unwrap(), trait_entry);

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(
                    PrimitiveOccurrencePosition::NamedField,
                    vec![
                        PrimitiveOccurrencePosition::VariantField,
                        PrimitiveOccurrencePosition::ResultErr,
                    ],
                ),
            },
        )
        .unwrap();

        let result =
            evaluate_catalogue_lint(&[rule], &all, &target_layer, &BoundOnlyFailingScanner);

        assert!(
            result.is_ok(),
            "a `?Sized` supertrait-bound slot must be skipped at collection time when Bound is \
             not requested, so evaluation must not fail with ScanFailed; got: {result:?}"
        );
    }

    #[test]
    fn test_forbid_primitive_in_types_scans_result_err_inside_type_like_bound() {
        // The result_err default rule still needs to inspect type-like bounds:
        // `Into<Result<(), String>>` can contain a primitive Err slot even
        // though the rule does not request the top-level Bound position. The
        // adjacent `?Sized` bound proves the collector still skips bound-only
        // tokens that would fail a `syn::Type` scan.
        let mut doc = make_doc("domain");
        let method = MethodDeclaration {
            generics: vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![
                    TypeRef::new("?Sized").unwrap(),
                    TypeRef::new("Into<Result<(), String>>").unwrap(),
                ],
            }],
            ..method_shared_ref_no_params("do_thing", "()")
        };
        doc.types.insert(
            TypeName::new("Money").unwrap(),
            make_type_entry_with_methods(DataRole::value_object(), vec![method]),
        );

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::ResultErr, vec![]),
            },
        )
        .unwrap();

        let violations =
            evaluate_catalogue_lint(&[rule], &all, &target_layer, &BoundResultErrScanner).unwrap();

        assert_eq!(
            violations.len(),
            1,
            "expected exactly 1 ResultErr violation from the type-like bound, got: {violations:?}"
        );
        assert_eq!(violations[0].entry_name(), "Money");
        assert!(
            violations[0].message().contains("ResultErr"),
            "expected the violation to be attributed to ResultErr, got: {}",
            violations[0].message()
        );
    }

    // ------------------------------------------------------------------
    // PR #179 round 2 P1 regression: impl-block-level generic bounds
    // (`impl<T: Into<Result<(), String>>> Foo<T>`) are carried on
    // `InherentImplDeclV2.impl_generics` / `impl_where_predicates`, which
    // `collect_type_entry_slots` previously never inspected -- only the
    // *methods* of a matching `inherent_impls` block were merged in (via
    // `collect_methods_for_type`), so a primitive occurrence appearing only
    // in an impl-block-level bound was silently missed.
    // ------------------------------------------------------------------

    #[test]
    fn test_forbid_primitive_in_types_scans_result_err_inside_inherent_impl_generic_bound() {
        let mut doc = make_doc("domain");
        doc.types
            .insert(TypeName::new("Money").unwrap(), make_type_entry(DataRole::value_object()));
        doc.inherent_impls.push(InherentImplDeclV2 {
            type_name: TypeName::new("Money").unwrap(),
            impl_generics: vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![TypeRef::new("Into<Result<(), String>>").unwrap()],
            }],
            impl_where_predicates: vec![],
            methods: vec![],
        });

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::ResultErr, vec![]),
            },
        )
        .unwrap();

        let violations =
            evaluate_catalogue_lint(&[rule], &all, &target_layer, &BoundResultErrScanner).unwrap();

        assert_eq!(
            violations.len(),
            1,
            "expected exactly 1 ResultErr violation from the impl-block-level generic bound, \
             got: {violations:?}"
        );
        assert_eq!(violations[0].entry_name(), "Money");
    }

    #[test]
    fn test_forbid_primitive_in_types_scans_result_err_inside_inherent_impl_where_predicate() {
        let mut doc = make_doc("domain");
        doc.types
            .insert(TypeName::new("Money").unwrap(), make_type_entry(DataRole::value_object()));
        doc.inherent_impls.push(InherentImplDeclV2 {
            type_name: TypeName::new("Money").unwrap(),
            impl_generics: vec![],
            impl_where_predicates: vec![WherePredicateDecl {
                lhs: TypeRef::new("T").unwrap(),
                rhs: vec![TypeRef::new("Into<Result<(), String>>").unwrap()],
                operator: BoundOp::Bound,
            }],
            methods: vec![],
        });

        let all = all_catalogues_single(&doc);
        let target_layer = doc.layer.clone();
        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(layer("domain"), vec![]),
                positions: NonEmptyVec::new(PrimitiveOccurrencePosition::ResultErr, vec![]),
            },
        )
        .unwrap();

        let violations =
            evaluate_catalogue_lint(&[rule], &all, &target_layer, &BoundResultErrScanner).unwrap();

        assert_eq!(
            violations.len(),
            1,
            "expected exactly 1 ResultErr violation from the impl-block-level where-predicate \
             bound, got: {violations:?}"
        );
        assert_eq!(violations[0].entry_name(), "Money");
    }
}
