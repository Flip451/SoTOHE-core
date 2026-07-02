//! Catalogue lint workflow (ADR 2026-05-25-0000-tddd-pattern-semantics-extension §D15 / D17).
//!
//! Hexagonal composition:
//!
//! * [`RunCatalogueLint`] — **primary port** (application service trait).
//!   CLI and future adapters depend on this trait, not on the concrete
//!   interactor below.
//! * [`RunCatalogueLintInteractor`] — orchestrates the `CatalogueLoader`
//!   secondary port and calls `domain::evaluate_catalogue_lint` directly
//!   (D17: pure evaluation logic lives in domain, not infrastructure).
//!   It implements [`RunCatalogueLint`].
//!
//! The usecase layer stays pure-orchestrator per
//! `knowledge/conventions/hexagonal-architecture.md` §Usecase Purity:
//! no `std::fs`, no `println!`, no `chrono`, no env access.
//! All I/O flows through the domain ports.

use domain::TrackId;
use domain::tddd::catalogue_linter::{
    CatalogueLintViolation, CatalogueLinterError, CatalogueLinterRule, CatalogueLinterRuleKind,
    RoleKind, RolePayloadField, RuleTarget, evaluate_catalogue_lint,
};
use domain::tddd::catalogue_ports::{CatalogueLoader, CatalogueLoaderError};
use domain::tddd::catalogue_v2::identifiers::TypeRef;
use domain::tddd::catalogue_v2::roles::{NonEmptyVec, SelfReceiver};
use domain::tddd::layer_id::LayerId;
use domain::tddd::primitive_occurrence_scanner::{
    PrimitiveName, PrimitiveOccurrencePosition, PrimitiveOccurrenceScanner,
};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Config file support (D19)
// ---------------------------------------------------------------------------

/// A loaded, validated lint configuration from a config file.
///
/// Carries the rule set to evaluate. Constructed by [`LintConfigLoader::load`].
#[derive(Debug, Clone)]
pub struct LintConfig {
    rules: Vec<LintRuleSpec>,
}

impl LintConfig {
    /// Creates a new `LintConfig` with the given rule set.
    #[must_use]
    pub fn new(rules: Vec<LintRuleSpec>) -> Self {
        Self { rules }
    }

    /// Returns the lint rules declared in this config.
    #[must_use]
    pub fn rules(&self) -> &[LintRuleSpec] {
        &self.rules
    }
}

/// Error variants returned by [`LintConfigLoader::load`].
#[derive(thiserror::Error, Debug)]
pub enum LintConfigLoaderError {
    /// The config file was not found at the expected path.
    #[error("lint config file not found: {path}")]
    MissingFile {
        /// Path that was searched.
        path: std::path::PathBuf,
    },
    /// The config file could not be parsed.
    #[error("failed to parse lint config at {path}: {reason}")]
    ParseError {
        /// Path of the file that failed to parse.
        path: std::path::PathBuf,
        /// Human-readable parse failure description.
        reason: String,
    },
    /// The config file declares an unsupported schema version.
    #[error("lint config schema_version mismatch: expected {expected}, got {actual}")]
    SchemaVersionMismatch {
        /// The only version this loader supports.
        expected: u32,
        /// The version found in the file.
        actual: u32,
    },
}

/// Secondary port for loading lint configuration from a config file (D19).
///
/// The path is baked into the adapter at construction time; `load()` takes no
/// path argument.
pub trait LintConfigLoader: Send + Sync {
    /// Load and validate the lint configuration.
    ///
    /// # Errors
    ///
    /// Returns [`LintConfigLoaderError::MissingFile`] when the config file is absent.
    /// Returns [`LintConfigLoaderError::ParseError`] when the file cannot be parsed.
    /// Returns [`LintConfigLoaderError::SchemaVersionMismatch`] when the file uses an
    /// unsupported schema version.
    fn load(&self) -> Result<LintConfig, LintConfigLoaderError>;
}

// ---------------------------------------------------------------------------
// Usecase-owned lint rule types (no domain imports in CLI / callers)
// ---------------------------------------------------------------------------

/// Usecase-owned mirror of `domain::tddd::catalogue_linter::CatalogueLinterRuleKind`.
///
/// Callers (e.g. CLI) use this enum so they never import domain types
/// directly (CN-01 / AC-03). All payload fields use `String` / `Vec<String>`
/// as boundary representations; the interactor converts them to domain types.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub enum LintRuleKind {
    /// Rule asserts that the named field must be empty for matching entries.
    FieldEmpty { target_field: String },
    /// Rule asserts that the named field must be non-empty for matching entries.
    FieldNonEmpty { target_field: String },
    /// Rule constrains which layers entries of the target role may appear in.
    KindLayerConstraint { permitted_layers: Vec<String> },
    /// Rule asserts that typed entries in `target_field` are declared with
    /// `expected_role` in the catalogue.
    ReferencedRoleConstraint { target_field: String, expected_role: String },
    /// Rule asserts that `trait_impls` contains all of `required_traits`.
    TraitImplRequired { required_traits: Vec<String> },
    /// Rule asserts that no method signature contains a type with a forbidden
    /// role.
    NoRoleInMethodSignature { forbidden_roles: Vec<String> },
    /// Rule asserts that the method referenced by `target_field` exists in the
    /// entry's public method set and satisfies the expected signature.
    MethodReferenceSignature { target_field: String },
    /// Rule asserts that the entry has a public accessor getter matching the
    /// identity signature.
    AccessorSignatureRequired { target_field: String },
    /// Rule asserts that elements in `target_field` are unique across all
    /// entries of the target role.
    FieldElementUniqueAcrossEntries { target_field: String },
    /// Rule asserts that elements listed in `target_field` do not appear in
    /// any other entry's method signatures.
    NoExternalReferenceInMethods { target_field: String },
    /// Rule asserts that the entry has no public struct fields. Unit variant.
    NoPublicField,
    /// Rule asserts that no method uses the given self-receiver kind.
    ForbiddenMethodReceiver { forbidden_receiver: String },
    /// Rule asserts that none of `primitives` occurs at any of `positions`
    /// within catalogue entries in `layers`. Role-axis filtering is not part
    /// of this variant's payload; it reuses `target_roles` on the enclosing
    /// [`LintRuleSpec`] instead.
    ForbidPrimitiveInTypes { primitives: Vec<String>, layers: Vec<String>, positions: Vec<String> },
}

/// Usecase-owned string-only description of a single lint rule.
///
/// Callers (e.g. CLI) construct `LintRuleSpec` values without importing
/// domain types. The interactor converts them to `CatalogueLinterRule`
/// internally.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LintRuleSpec {
    /// Roles to which this rule applies. An empty vec means "all roles".
    pub target_roles: Vec<String>,
    pub kind: LintRuleKind,
}

/// Command input for [`RunCatalogueLint::execute`].
///
/// Carries the track, layer, and rule set to lint against.
#[derive(Debug, Clone)]
pub struct RunCatalogueLintCommand {
    /// Track identifier (used to locate the layer catalogue file).
    pub track_id: String,
    /// Layer identifier (e.g. `"domain"`, `"usecase"`, `"infrastructure"`).
    /// Must be one of the TDDD-enabled layers known to the [`CatalogueLoader`].
    pub layer_id: String,
    /// Set of lint rules to evaluate against the catalogue.
    /// Use [`LintRuleSpec`] so callers never import domain types directly.
    pub rules: Vec<LintRuleSpec>,
}

/// Error variants returned by [`RunCatalogueLintInteractor::execute`].
#[derive(Debug, Error)]
pub enum RunCatalogueLintError {
    /// The catalogue loader failed to load the layer catalogue.
    #[error(transparent)]
    CatalogueLoad(#[from] CatalogueLoaderError),

    /// The linter returned an execution error.
    #[error(transparent)]
    LintExecution(#[from] CatalogueLinterError),

    /// The specified `layer_id` is not a TDDD-enabled layer (not present in
    /// the set returned by [`CatalogueLoader::load_all`]).
    #[error("layer '{0}' is not a TDDD-enabled layer")]
    InvalidLayer(String),

    /// A [`LintRuleSpec`] could not be converted to a domain rule.
    #[error("invalid lint rule spec: {0}")]
    InvalidRuleSpec(String),

    /// No lint rules were provided via CLI and the config file was absent
    /// (D19 fail-closed: missing config is an error, not a no-op).
    #[error("lint config file not found: {path}")]
    ConfigMissing {
        /// The path that was searched for the config file.
        path: std::path::PathBuf,
    },

    /// The config file was found but could not be parsed or has an unsupported
    /// schema version.
    #[error(transparent)]
    ConfigInvalid(#[from] LintConfigLoaderError),
}

/// Primary port for the catalogue lint use case.
///
/// The CLI `sotp track lint` subcommand drives the workflow through
/// this trait, keeping the composition root substitutable.
pub trait RunCatalogueLint: Send + Sync {
    /// Run catalogue lint rules for `cmd.layer_id` within `cmd.track_id`.
    ///
    /// # Errors
    ///
    /// Returns [`RunCatalogueLintError::CatalogueLoad`] on loader failure.
    /// Returns [`RunCatalogueLintError::InvalidLayer`] when `cmd.layer_id`
    /// is not present in the TDDD-enabled layer set returned by the loader.
    /// Returns [`RunCatalogueLintError::LintExecution`] on linter failure.
    /// Returns [`RunCatalogueLintError::InvalidRuleSpec`] when a
    /// [`LintRuleSpec`] cannot be converted to a domain rule.
    fn execute(
        &self,
        cmd: RunCatalogueLintCommand,
    ) -> Result<Vec<CatalogueLintViolation>, RunCatalogueLintError>;
}

/// Default [`RunCatalogueLint`] implementation that composes
/// [`CatalogueLoader`] and calls [`evaluate_catalogue_lint`] directly (D17).
///
/// Generic over `L: CatalogueLoader`, `C: LintConfigLoader`, and
/// `S: PrimitiveOccurrenceScanner` so callers (e.g. the CLI composition root)
/// pass concrete types without importing domain port traits or usecase config
/// port traits directly. `S` backs the `ForbidPrimitiveInTypes` rule kind
/// (T005); it is threaded straight through to [`evaluate_catalogue_lint`] and
/// otherwise has no effect on the other rule kinds.
///
/// Rule source priority (D19 fail-closed precedence):
/// 1. `command.rules` non-empty → use CLI-supplied rules.
/// 2. `command.rules` empty → load from `config_loader.load()`.
///    - [`LintConfigLoaderError::MissingFile`] → [`RunCatalogueLintError::ConfigMissing`].
///    - Other load errors → [`RunCatalogueLintError::ConfigInvalid`].
pub struct RunCatalogueLintInteractor<
    L: CatalogueLoader,
    C: LintConfigLoader,
    S: PrimitiveOccurrenceScanner,
> {
    loader: L,
    config_loader: C,
    scanner: S,
}

impl<L: CatalogueLoader, C: LintConfigLoader, S: PrimitiveOccurrenceScanner>
    RunCatalogueLintInteractor<L, C, S>
{
    /// Creates a new interactor wrapping the supplied catalogue loader,
    /// config loader, and primitive-occurrence scanner.
    #[must_use]
    pub fn new(loader: L, config_loader: C, scanner: S) -> Self {
        Self { loader, config_loader, scanner }
    }
}

impl<
    L: CatalogueLoader + Send + Sync,
    C: LintConfigLoader + Send + Sync,
    S: PrimitiveOccurrenceScanner + Send + Sync,
> RunCatalogueLint for RunCatalogueLintInteractor<L, C, S>
{
    fn execute(
        &self,
        cmd: RunCatalogueLintCommand,
    ) -> Result<Vec<CatalogueLintViolation>, RunCatalogueLintError> {
        // Step 1: parse track_id into domain type.
        let track_id = TrackId::try_new(&cmd.track_id).map_err(|e| {
            CatalogueLoaderError::LayerDiscoveryFailed {
                reason: format!("invalid track_id '{}': {e}", cmd.track_id),
            }
        })?;

        // Step 2: resolve lint rule specs.
        // Priority: (1) CLI-explicit rules, (2) config file (fail-closed on missing).
        let lint_rule_specs: Vec<LintRuleSpec> = if !cmd.rules.is_empty() {
            cmd.rules
        } else {
            match self.config_loader.load() {
                Ok(config) => config.rules().to_vec(),
                Err(LintConfigLoaderError::MissingFile { path }) => {
                    return Err(RunCatalogueLintError::ConfigMissing { path });
                }
                Err(other) => return Err(RunCatalogueLintError::ConfigInvalid(other)),
            }
        };

        // Step 3: convert LintRuleSpec values to domain CatalogueLinterRule values.
        let rules: Vec<CatalogueLinterRule> = lint_rule_specs
            .into_iter()
            .map(lint_rule_spec_to_domain)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e: CatalogueLintError| {
                RunCatalogueLintError::InvalidRuleSpec(e.to_string())
            })?;

        // Step 4: load all TDDD-enabled layers for this track.
        let (layer_order, catalogues) = self.loader.load_all(&track_id)?;

        // Step 5: validate that the requested layer_id is TDDD-enabled.
        let target_layer = layer_order
            .iter()
            .find(|l| l.as_ref() == cmd.layer_id.as_str())
            .ok_or_else(|| RunCatalogueLintError::InvalidLayer(cmd.layer_id.clone()))?;

        // Step 6: evaluate rules via the domain pure function (D17).
        // Pass all catalogues so that cross-layer role references
        // (e.g. `UseCase.handles: ["domain::OrderPlaced"]`) are resolved
        // correctly against every enabled layer, not just the target layer.
        let violations = evaluate_catalogue_lint(&rules, &catalogues, target_layer, &self.scanner)?;

        Ok(violations)
    }
}

// ── CatalogueLintError ────────────────────────────────────────────────────────

/// Error type for [`lint_rule_spec_to_domain`] and [`parse_role_kind`].
///
/// Implemented as a transparent newtype over [`String`] so that test assertions
/// can use `.contains()` on the unwrapped error value (the private helpers are
/// called directly in tests). The newtype satisfies the typed-error requirement
/// while keeping the existing test surface intact.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct CatalogueLintError(String);

impl std::ops::Deref for CatalogueLintError {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Convert a [`LintRuleSpec`] to a domain [`CatalogueLinterRule`].
///
/// Returns [`CatalogueLintError`] when the spec is rejected by the domain
/// constructors (e.g. empty `target_field`, unknown role string, empty
/// required_traits).
fn lint_rule_spec_to_domain(spec: LintRuleSpec) -> Result<CatalogueLinterRule, CatalogueLintError> {
    // Convert target_roles strings to RoleKind.
    let target_roles =
        spec.target_roles.iter().map(|s| parse_role_kind(s)).collect::<Result<Vec<_>, _>>()?;
    let target = RuleTarget::new(target_roles);

    // Convert LintRuleKind to CatalogueLinterRuleKind.
    let kind = match spec.kind {
        LintRuleKind::FieldEmpty { target_field } => CatalogueLinterRuleKind::FieldEmpty {
            target_field: parse_role_payload_field(&target_field)?,
        },
        LintRuleKind::FieldNonEmpty { target_field } => CatalogueLinterRuleKind::FieldNonEmpty {
            target_field: parse_role_payload_field(&target_field)?,
        },
        LintRuleKind::KindLayerConstraint { permitted_layers } => {
            let layers: Vec<LayerId> = permitted_layers
                .into_iter()
                .map(|s| {
                    LayerId::try_new(s.clone())
                        .map_err(|e| CatalogueLintError(format!("invalid layer_id '{s}': {e}")))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let non_empty = NonEmptyVec::try_new(layers)
                .map_err(|_| CatalogueLintError("permitted_layers must not be empty".to_owned()))?;
            CatalogueLinterRuleKind::KindLayerConstraint { permitted_layers: non_empty }
        }
        LintRuleKind::ReferencedRoleConstraint { target_field, expected_role } => {
            let role = parse_role_kind(&expected_role)?;
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: parse_role_payload_field(&target_field)?,
                expected_role: role,
            }
        }
        LintRuleKind::TraitImplRequired { required_traits } => {
            let refs: Vec<TypeRef> = required_traits
                .into_iter()
                .map(|s| {
                    TypeRef::new(s.clone()).map_err(|e| {
                        CatalogueLintError(format!("invalid required trait '{s}': {e}"))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let non_empty = NonEmptyVec::try_new(refs)
                .map_err(|_| CatalogueLintError("required_traits must not be empty".to_owned()))?;
            CatalogueLinterRuleKind::TraitImplRequired { required_traits: non_empty }
        }
        LintRuleKind::NoRoleInMethodSignature { forbidden_roles } => {
            let roles: Vec<RoleKind> = forbidden_roles
                .iter()
                .map(|s| parse_role_kind(s))
                .collect::<Result<Vec<_>, _>>()?;
            let non_empty = NonEmptyVec::try_new(roles)
                .map_err(|_| CatalogueLintError("forbidden_roles must not be empty".to_owned()))?;
            CatalogueLinterRuleKind::NoRoleInMethodSignature { forbidden_roles: non_empty }
        }
        LintRuleKind::MethodReferenceSignature { target_field } => {
            CatalogueLinterRuleKind::MethodReferenceSignature {
                target_field: parse_role_payload_field(&target_field)?,
            }
        }
        LintRuleKind::AccessorSignatureRequired { target_field } => {
            CatalogueLinterRuleKind::AccessorSignatureRequired {
                target_field: parse_role_payload_field(&target_field)?,
            }
        }
        LintRuleKind::FieldElementUniqueAcrossEntries { target_field } => {
            CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries {
                target_field: parse_role_payload_field(&target_field)?,
            }
        }
        LintRuleKind::NoExternalReferenceInMethods { target_field } => {
            CatalogueLinterRuleKind::NoExternalReferenceInMethods {
                target_field: parse_role_payload_field(&target_field)?,
            }
        }
        LintRuleKind::NoPublicField => CatalogueLinterRuleKind::NoPublicField,
        LintRuleKind::ForbiddenMethodReceiver { forbidden_receiver } => {
            CatalogueLinterRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: parse_self_receiver(&forbidden_receiver)?,
            }
        }
        LintRuleKind::ForbidPrimitiveInTypes { primitives, layers, positions } => {
            let primitive_names: Vec<PrimitiveName> = primitives
                .into_iter()
                .map(|s| {
                    PrimitiveName::new(s.clone())
                        .map_err(|e| CatalogueLintError(format!("invalid primitive '{s}': {e}")))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let non_empty_primitives = NonEmptyVec::try_new(primitive_names)
                .map_err(|_| CatalogueLintError("primitives must not be empty".to_owned()))?;

            let layer_ids: Vec<LayerId> = layers
                .into_iter()
                .map(|s| {
                    LayerId::try_new(s.clone())
                        .map_err(|e| CatalogueLintError(format!("invalid layer_id '{s}': {e}")))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let non_empty_layers = NonEmptyVec::try_new(layer_ids)
                .map_err(|_| CatalogueLintError("layers must not be empty".to_owned()))?;

            let position_values: Vec<PrimitiveOccurrencePosition> = positions
                .iter()
                .map(|s| parse_primitive_occurrence_position(s))
                .collect::<Result<Vec<_>, _>>()?;
            let non_empty_positions = NonEmptyVec::try_new(position_values)
                .map_err(|_| CatalogueLintError("positions must not be empty".to_owned()))?;

            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: non_empty_primitives,
                layers: non_empty_layers,
                positions: non_empty_positions,
            }
        }
    };

    CatalogueLinterRule::new(target, kind).map_err(|e| CatalogueLintError(e.to_string()))
}

/// Parse a role kind string into a [`RoleKind`].
fn parse_role_kind(s: &str) -> Result<RoleKind, CatalogueLintError> {
    match s {
        "ValueObject" => Ok(RoleKind::ValueObject),
        "Entity" => Ok(RoleKind::Entity),
        "AggregateRoot" => Ok(RoleKind::AggregateRoot),
        "DomainService" => Ok(RoleKind::DomainService),
        "Specification" => Ok(RoleKind::Specification),
        "Factory" => Ok(RoleKind::Factory),
        "UseCase" => Ok(RoleKind::UseCase),
        "Interactor" => Ok(RoleKind::Interactor),
        "Command" => Ok(RoleKind::Command),
        "Query" => Ok(RoleKind::Query),
        "Dto" => Ok(RoleKind::Dto),
        "ErrorType" => Ok(RoleKind::ErrorType),
        "SecondaryAdapter" => Ok(RoleKind::SecondaryAdapter),
        "EventPolicy" => Ok(RoleKind::EventPolicy),
        "DomainEvent" => Ok(RoleKind::DomainEvent),
        "SpecificationPort" => Ok(RoleKind::SpecificationPort),
        "ApplicationService" => Ok(RoleKind::ApplicationService),
        "SecondaryPort" => Ok(RoleKind::SecondaryPort),
        "Repository" => Ok(RoleKind::Repository),
        "CompositionRoot" => Ok(RoleKind::CompositionRoot),
        "PrimaryAdapter" => Ok(RoleKind::PrimaryAdapter),
        "FreeFunction" => Ok(RoleKind::FreeFunction),
        "UseCaseFunction" => Ok(RoleKind::UseCaseFunction),
        other => Err(CatalogueLintError(format!("unknown role kind: '{other}'"))),
    }
}

/// Parse a target-field string into a [`RolePayloadField`].
///
/// `RolePayloadField` is a closed enum (strum `EnumString`), so this rejects
/// any string that does not match one of its variant names exactly
/// (snake_case), replacing what used to be an unchecked `String` passthrough.
fn parse_role_payload_field(s: &str) -> Result<RolePayloadField, CatalogueLintError> {
    s.parse::<RolePayloadField>()
        .map_err(|e| CatalogueLintError(format!("invalid target_field '{s}': {e}")))
}

/// Parse a self-receiver string into a [`SelfReceiver`].
///
/// `SelfReceiver` is a closed enum (strum `EnumString`) accepting exactly
/// `"self"`, `"&self"`, or `"&mut self"`; anything else (including the empty
/// string) is rejected here, at the usecase boundary, before a domain
/// `CatalogueLinterRuleKind::ForbiddenMethodReceiver` value can be constructed.
fn parse_self_receiver(s: &str) -> Result<SelfReceiver, CatalogueLintError> {
    s.parse::<SelfReceiver>()
        .map_err(|e| CatalogueLintError(format!("invalid forbidden_receiver '{s}': {e}")))
}

/// Parse a primitive-occurrence position string into a
/// [`PrimitiveOccurrencePosition`].
///
/// `PrimitiveOccurrencePosition` is a closed 7-variant enum but, unlike
/// [`RolePayloadField`] / [`SelfReceiver`], does not derive `strum`'s
/// `EnumString`: `domain-types.json`'s `trait_impls` for this type list only
/// the `Debug` / `Clone` / `Copy` / `PartialEq` / `Eq` / `PartialOrd` / `Ord`
/// / `Hash` derives it already had from T001 -- adding a `strum`-based
/// `FromStr` here would be an undeclared trait-impl addition against that
/// catalogue. This mirrors [`parse_role_kind`]'s exhaustive match instead,
/// using the same canonical snake_case names the ADR's default config uses
/// (e.g. `"result_err"`).
fn parse_primitive_occurrence_position(
    s: &str,
) -> Result<PrimitiveOccurrencePosition, CatalogueLintError> {
    match s {
        "named_field" => Ok(PrimitiveOccurrencePosition::NamedField),
        "variant_field" => Ok(PrimitiveOccurrencePosition::VariantField),
        "param" => Ok(PrimitiveOccurrencePosition::Param),
        "return" => Ok(PrimitiveOccurrencePosition::Return),
        "bound" => Ok(PrimitiveOccurrencePosition::Bound),
        "type_alias_target" => Ok(PrimitiveOccurrencePosition::TypeAliasTarget),
        "result_err" => Ok(PrimitiveOccurrencePosition::ResultErr),
        other => {
            Err(CatalogueLintError(format!("unknown primitive occurrence position: '{other}'")))
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::BTreeMap;

    use domain::TrackId;
    use domain::tddd::catalogue_ports::{CatalogueLoader, CatalogueLoaderError};
    use domain::tddd::catalogue_v2::document::CatalogueDocument;
    use domain::tddd::catalogue_v2::entries::TypeEntry;
    use domain::tddd::catalogue_v2::identifiers::{CrateName, FieldName, ModulePath, TypeName};
    use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction, NonEmptyVec};
    use domain::tddd::catalogue_v2::variants::FieldDecl;
    use domain::tddd::catalogue_v2::{StructKind, StructShape, TypeKindV2};
    use domain::tddd::layer_id::LayerId;
    use domain::tddd::primitive_occurrence_scanner::{
        PrimitiveOccurrenceReport, PrimitiveOccurrenceScanError,
    };
    use mockall::mock;

    use super::*;

    // ------------------------------------------------------------------
    // Test double for PrimitiveOccurrenceScanner: reports a requested
    // primitive name as found, at the given call-site position, whenever it
    // occurs as an exact substring of type_ref's string form. Mirrors the
    // domain-layer StubPrimitiveScanner in catalogue_linter.rs tests, since
    // domain's stub is private and not reusable from the usecase crate.
    // ------------------------------------------------------------------

    struct StubScanner;

    impl PrimitiveOccurrenceScanner for StubScanner {
        fn scan(
            &self,
            type_ref: TypeRef,
            primitives: NonEmptyVec<PrimitiveName>,
            position: PrimitiveOccurrencePosition,
        ) -> Result<PrimitiveOccurrenceReport, PrimitiveOccurrenceScanError> {
            use std::collections::BTreeSet;

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

    // ------------------------------------------------------------------
    // mockall mock for CatalogueLoader
    // ------------------------------------------------------------------

    mock! {
        pub Loader {}
        impl CatalogueLoader for Loader {
            fn load_all(
                &self,
                track_id: &TrackId,
            ) -> Result<
                (Vec<LayerId>, BTreeMap<LayerId, CatalogueDocument>),
                CatalogueLoaderError,
            >;
        }
    }

    // ------------------------------------------------------------------
    // Stub LintConfigLoader that always returns MissingFile.
    // Used in existing unit tests where rules are supplied via command.rules
    // (CLI-explicit path), so the config loader is never reached.
    // ------------------------------------------------------------------

    struct StubMissingConfigLoader;

    impl LintConfigLoader for StubMissingConfigLoader {
        fn load(&self) -> Result<LintConfig, LintConfigLoaderError> {
            Err(LintConfigLoaderError::MissingFile {
                path: std::path::PathBuf::from("/stub/config.json"),
            })
        }
    }

    // ------------------------------------------------------------------
    // Stub LintConfigLoader that returns a successful config with one rule.
    // ------------------------------------------------------------------

    struct StubSuccessConfigLoader {
        rules: Vec<LintRuleSpec>,
    }

    impl LintConfigLoader for StubSuccessConfigLoader {
        fn load(&self) -> Result<LintConfig, LintConfigLoaderError> {
            Ok(LintConfig::new(self.rules.clone()))
        }
    }

    // ------------------------------------------------------------------
    // Stub LintConfigLoader that returns a ParseError.
    // ------------------------------------------------------------------

    struct StubParseErrorConfigLoader;

    impl LintConfigLoader for StubParseErrorConfigLoader {
        fn load(&self) -> Result<LintConfig, LintConfigLoaderError> {
            Err(LintConfigLoaderError::ParseError {
                path: std::path::PathBuf::from("/stub/config.json"),
                reason: "unexpected token".to_owned(),
            })
        }
    }

    // ------------------------------------------------------------------
    // Stub LintConfigLoader that panics if called.
    // Used to assert the config loader is NOT called when CLI rules are provided.
    // ------------------------------------------------------------------

    struct StubNeverCalledConfigLoader;

    impl LintConfigLoader for StubNeverCalledConfigLoader {
        fn load(&self) -> Result<LintConfig, LintConfigLoaderError> {
            panic!("config_loader.load() must not be called when CLI rules are provided");
        }
    }

    // ------------------------------------------------------------------
    // helpers
    // ------------------------------------------------------------------

    fn layer(name: &str) -> LayerId {
        LayerId::try_new(name.to_owned()).unwrap()
    }

    fn empty_doc(crate_name: &str) -> CatalogueDocument {
        CatalogueDocument::new(3, CrateName::new(crate_name).unwrap(), layer(crate_name))
    }

    fn single_entry_doc(crate_name: &str) -> CatalogueDocument {
        let mut doc = empty_doc(crate_name);
        doc.types.insert(
            TypeName::new("SentinelType").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        doc
    }

    fn three_layer_result(target: &str) -> (Vec<LayerId>, BTreeMap<LayerId, CatalogueDocument>) {
        let domain = layer("domain");
        let usecase = layer("usecase");
        let infra = layer("infrastructure");
        let order = vec![domain.clone(), usecase.clone(), infra.clone()];
        let mut catalogues = BTreeMap::new();
        catalogues.insert(domain.clone(), empty_doc("domain"));
        catalogues.insert(usecase.clone(), empty_doc("usecase"));
        catalogues.insert(infra.clone(), empty_doc("infrastructure"));
        let target_layer = layer(target);
        catalogues.insert(target_layer, single_entry_doc(target));
        (order, catalogues)
    }

    fn no_public_field_rule_spec() -> LintRuleSpec {
        LintRuleSpec { target_roles: vec![], kind: LintRuleKind::NoPublicField }
    }

    fn cmd(track: &str, layer_name: &str) -> RunCatalogueLintCommand {
        RunCatalogueLintCommand {
            track_id: track.to_owned(),
            layer_id: layer_name.to_owned(),
            rules: vec![no_public_field_rule_spec()],
        }
    }

    /// Build a command with no CLI-supplied rules (triggers config-file path).
    fn cmd_no_rules(track: &str, layer_name: &str) -> RunCatalogueLintCommand {
        RunCatalogueLintCommand {
            track_id: track.to_owned(),
            layer_id: layer_name.to_owned(),
            rules: vec![],
        }
    }

    // ------------------------------------------------------------------
    // T001: Happy path — evaluate_catalogue_lint skeleton returns empty violations
    // ------------------------------------------------------------------

    #[test]
    fn test_execute_happy_path_empty_violations() {
        let (order, catalogues) = three_layer_result("domain");
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .with(mockall::predicate::function(|t: &TrackId| t.as_ref() == "my-track"))
            .times(1)
            .returning(move |_| Ok((order.clone(), catalogues.clone())));

        let interactor =
            RunCatalogueLintInteractor::new(loader, StubMissingConfigLoader, StubScanner);
        let result = interactor.execute(cmd("my-track", "domain"));

        assert!(result.is_ok(), "expected Ok, got: {result:?}");
        assert_eq!(result.unwrap(), vec![]);
    }

    // ------------------------------------------------------------------
    // T002: CatalogueLoader error propagation
    // ------------------------------------------------------------------

    #[test]
    fn test_execute_catalogue_loader_error_propagates() {
        let mut loader = MockLoader::new();
        loader.expect_load_all().times(1).returning(|_| {
            Err(CatalogueLoaderError::LayerDiscoveryFailed { reason: "boom".to_owned() })
        });

        let interactor =
            RunCatalogueLintInteractor::new(loader, StubMissingConfigLoader, StubScanner);
        let result = interactor.execute(cmd("my-track", "domain"));

        assert!(
            matches!(result, Err(RunCatalogueLintError::CatalogueLoad(_))),
            "expected CatalogueLoad error"
        );
    }

    // ------------------------------------------------------------------
    // T003: InvalidLayer — layer_id not in TDDD-enabled layers
    // ------------------------------------------------------------------

    #[test]
    fn test_execute_invalid_layer_returns_error() {
        let (order, catalogues) = three_layer_result("domain");
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .times(1)
            .returning(move |_| Ok((order.clone(), catalogues.clone())));

        let interactor =
            RunCatalogueLintInteractor::new(loader, StubMissingConfigLoader, StubScanner);
        let result = interactor.execute(cmd("my-track", "presentation")); // not in set

        match result {
            Err(RunCatalogueLintError::InvalidLayer(layer_id)) => {
                assert_eq!(layer_id, "presentation");
            }
            other => panic!("expected InvalidLayer, got {other:?}"),
        }
    }

    // ------------------------------------------------------------------
    // T004: lint_rule_spec_to_domain — all 12 LintRuleKind variants convert
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_converts_all_12_kinds() {
        let specs: Vec<LintRuleSpec> = vec![
            LintRuleSpec {
                target_roles: vec![],
                kind: LintRuleKind::FieldEmpty { target_field: "invariants".to_owned() },
            },
            LintRuleSpec {
                target_roles: vec![],
                kind: LintRuleKind::FieldNonEmpty { target_field: "emits".to_owned() },
            },
            LintRuleSpec {
                target_roles: vec!["EventPolicy".to_owned()],
                kind: LintRuleKind::KindLayerConstraint {
                    permitted_layers: vec!["domain".to_owned()],
                },
            },
            LintRuleSpec {
                target_roles: vec!["AggregateRoot".to_owned()],
                kind: LintRuleKind::ReferencedRoleConstraint {
                    target_field: "emits".to_owned(),
                    expected_role: "DomainEvent".to_owned(),
                },
            },
            LintRuleSpec {
                target_roles: vec!["ValueObject".to_owned()],
                kind: LintRuleKind::TraitImplRequired {
                    required_traits: vec!["PartialEq".to_owned()],
                },
            },
            LintRuleSpec {
                target_roles: vec!["EventPolicy".to_owned()],
                kind: LintRuleKind::NoRoleInMethodSignature {
                    forbidden_roles: vec!["Repository".to_owned()],
                },
            },
            LintRuleSpec {
                target_roles: vec![],
                kind: LintRuleKind::MethodReferenceSignature {
                    target_field: "invariants".to_owned(),
                },
            },
            LintRuleSpec {
                target_roles: vec![],
                kind: LintRuleKind::AccessorSignatureRequired {
                    target_field: "identity".to_owned(),
                },
            },
            LintRuleSpec {
                target_roles: vec![],
                kind: LintRuleKind::FieldElementUniqueAcrossEntries {
                    target_field: "exclusive_members".to_owned(),
                },
            },
            LintRuleSpec {
                target_roles: vec![],
                kind: LintRuleKind::NoExternalReferenceInMethods {
                    target_field: "exclusive_members".to_owned(),
                },
            },
            LintRuleSpec { target_roles: vec![], kind: LintRuleKind::NoPublicField },
            LintRuleSpec {
                target_roles: vec![],
                kind: LintRuleKind::ForbiddenMethodReceiver {
                    forbidden_receiver: "&mut self".to_owned(),
                },
            },
        ];
        assert_eq!(specs.len(), 12, "must cover all 12 LintRuleKind variants");
        for spec in specs {
            let kind_name = format!("{:?}", spec.kind).split(' ').next().unwrap().to_owned();
            let result = lint_rule_spec_to_domain(spec);
            assert!(
                result.is_ok(),
                "conversion failed for kind starting with {kind_name}: {result:?}"
            );
        }
    }

    // ------------------------------------------------------------------
    // T005: CatalogueLintViolation re-export / interoperability
    // ------------------------------------------------------------------

    #[test]
    fn test_catalogue_lint_violation_accessible_from_usecase_boundary() {
        // CatalogueLintViolation is used as the output type of RunCatalogueLint::execute.
        // Verify it can be constructed at the usecase boundary.
        let v = CatalogueLintViolation::new("NoPublicField", "MyType", "has public fields");
        assert_eq!(v.rule_kind(), "NoPublicField");
        assert_eq!(v.entry_name(), "MyType");
    }

    // ------------------------------------------------------------------
    // T006: lint_rule_spec_to_domain — unknown target_roles entry is rejected
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_rejects_unknown_role() {
        let spec = LintRuleSpec {
            target_roles: vec!["NotARealRole".to_owned()],
            kind: LintRuleKind::NoPublicField,
        };
        let result = lint_rule_spec_to_domain(spec);
        assert!(result.is_err(), "expected Err for unknown role, got Ok");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("NotARealRole"),
            "error message should mention the bad role, got: {msg}"
        );
    }

    // ------------------------------------------------------------------
    // T007: lint_rule_spec_to_domain — empty permitted_layers is rejected
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_rejects_empty_permitted_layers() {
        let spec = LintRuleSpec {
            target_roles: vec![],
            kind: LintRuleKind::KindLayerConstraint { permitted_layers: vec![] },
        };
        let result = lint_rule_spec_to_domain(spec);
        assert!(result.is_err(), "expected Err for empty permitted_layers, got Ok");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("permitted_layers"),
            "error message should mention permitted_layers, got: {msg}"
        );
    }

    // ------------------------------------------------------------------
    // T008: lint_rule_spec_to_domain — empty required_traits is rejected
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_rejects_empty_required_traits() {
        let spec = LintRuleSpec {
            target_roles: vec![],
            kind: LintRuleKind::TraitImplRequired { required_traits: vec![] },
        };
        let result = lint_rule_spec_to_domain(spec);
        assert!(result.is_err(), "expected Err for empty required_traits, got Ok");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("required_traits"),
            "error message should mention required_traits, got: {msg}"
        );
    }

    // ------------------------------------------------------------------
    // T009: lint_rule_spec_to_domain — empty forbidden_roles is rejected
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_rejects_empty_forbidden_roles() {
        let spec = LintRuleSpec {
            target_roles: vec![],
            kind: LintRuleKind::NoRoleInMethodSignature { forbidden_roles: vec![] },
        };
        let result = lint_rule_spec_to_domain(spec);
        assert!(result.is_err(), "expected Err for empty forbidden_roles, got Ok");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("forbidden_roles"),
            "error message should mention forbidden_roles, got: {msg}"
        );
    }

    // ------------------------------------------------------------------
    // T010: lint_rule_spec_to_domain — empty target_field is rejected
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_rejects_empty_target_field() {
        // FieldNonEmpty with empty target_field should be rejected by
        // parse_role_payload_field (an empty string parses as no RolePayloadField
        // variant), before a domain value is ever constructed.
        let spec = LintRuleSpec {
            target_roles: vec![],
            kind: LintRuleKind::FieldNonEmpty { target_field: "".to_owned() },
        };
        let result = lint_rule_spec_to_domain(spec);
        assert!(result.is_err(), "expected Err for empty target_field, got Ok");
    }

    // ------------------------------------------------------------------
    // T011: execute — InvalidRuleSpec propagates when a rule spec is malformed
    // ------------------------------------------------------------------

    #[test]
    fn test_execute_invalid_rule_spec_returns_error() {
        let (order, catalogues) = three_layer_result("domain");
        let mut loader = MockLoader::new();
        loader.expect_load_all().times(0); // loader should not be called; spec conversion fails first

        // We need to construct the command with a malformed spec directly
        // without going through execute (which calls load_all before rules conversion).
        // Instead create a loader that should never be called (times(0)) by passing
        // the bad rules before load_all.
        // Actually, per the execute() code, rules are converted BEFORE load_all
        // (step 2 precedes step 3). So the loader mock with times(0) is correct.
        let _ = loader; // satisfy the borrow checker

        // Rebuild with a fresh mock that enforces times(0).
        let mut loader2 = MockLoader::new();
        loader2.expect_load_all().times(0);
        // Insert the target back so it compiles; we won't hit load_all anyway.
        let _ = (order, catalogues);

        let interactor =
            RunCatalogueLintInteractor::new(loader2, StubMissingConfigLoader, StubScanner);
        let bad_cmd = RunCatalogueLintCommand {
            track_id: "my-track".to_owned(),
            layer_id: "domain".to_owned(),
            rules: vec![LintRuleSpec {
                target_roles: vec!["UnknownRoleXYZ".to_owned()],
                kind: LintRuleKind::NoPublicField,
            }],
        };
        let result = interactor.execute(bad_cmd);
        assert!(
            matches!(result, Err(RunCatalogueLintError::InvalidRuleSpec(_))),
            "expected InvalidRuleSpec, got: {result:?}"
        );
    }

    // ------------------------------------------------------------------
    // T012: config-driven path — MissingFile loader error yields ConfigMissing
    // ------------------------------------------------------------------

    #[test]
    fn test_execute_with_empty_rules_and_missing_config_returns_config_missing() {
        let mut loader = MockLoader::new();
        loader.expect_load_all().times(0); // load_all is not reached before config check
        // (Actually: track_id parse happens first, then config load, then load_all.)
        // Re-check the execute() order: track_id parse → rules check → if empty: config load → load_all.
        // So load_all IS NOT called when config is missing — loader times(0) is correct.

        let interactor =
            RunCatalogueLintInteractor::new(loader, StubMissingConfigLoader, StubScanner);
        let result = interactor.execute(cmd_no_rules("my-track", "domain"));

        match result {
            Err(RunCatalogueLintError::ConfigMissing { path }) => {
                assert_eq!(path, std::path::PathBuf::from("/stub/config.json"));
            }
            other => panic!("expected ConfigMissing, got: {other:?}"),
        }
    }

    // ------------------------------------------------------------------
    // T013: config-driven path — non-MissingFile loader error yields ConfigInvalid
    // ------------------------------------------------------------------

    #[test]
    fn test_execute_with_empty_rules_and_parse_error_returns_config_invalid() {
        let mut loader = MockLoader::new();
        loader.expect_load_all().times(0);

        let interactor =
            RunCatalogueLintInteractor::new(loader, StubParseErrorConfigLoader, StubScanner);
        let result = interactor.execute(cmd_no_rules("my-track", "domain"));

        assert!(
            matches!(result, Err(RunCatalogueLintError::ConfigInvalid(_))),
            "expected ConfigInvalid, got: {result:?}"
        );
    }

    // ------------------------------------------------------------------
    // T014: config-driven path — successful config load runs lint with config rules
    // ------------------------------------------------------------------

    #[test]
    fn test_execute_with_empty_rules_and_valid_config_uses_config_rules() {
        let (order, catalogues) = three_layer_result("domain");
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .with(mockall::predicate::function(|t: &TrackId| t.as_ref() == "my-track"))
            .times(1)
            .returning(move |_| Ok((order.clone(), catalogues.clone())));

        let config_loader = StubSuccessConfigLoader { rules: vec![no_public_field_rule_spec()] };

        let interactor = RunCatalogueLintInteractor::new(loader, config_loader, StubScanner);
        let result = interactor.execute(cmd_no_rules("my-track", "domain"));

        assert!(result.is_ok(), "expected Ok when config provides rules, got: {result:?}");
    }

    // ------------------------------------------------------------------
    // T015: CLI precedence — non-empty cmd.rules skips config loader entirely
    // ------------------------------------------------------------------

    #[test]
    fn test_execute_with_cli_rules_does_not_call_config_loader() {
        let (order, catalogues) = three_layer_result("domain");
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .times(1)
            .returning(move |_| Ok((order.clone(), catalogues.clone())));

        // StubNeverCalledConfigLoader panics if load() is invoked.
        let interactor =
            RunCatalogueLintInteractor::new(loader, StubNeverCalledConfigLoader, StubScanner);
        // cmd() provides non-empty rules via CLI, so config_loader must not be called.
        let result = interactor.execute(cmd("my-track", "domain"));

        assert!(result.is_ok(), "expected Ok with CLI rules bypassing config, got: {result:?}");
    }

    // ------------------------------------------------------------------
    // T016: lint_rule_spec_to_domain — unrecognised target_field is rejected
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_rejects_unknown_target_field() {
        // `RolePayloadField` is a closed enum; a typo'd target_field string (e.g.
        // "emit" instead of "emits") must be rejected by `parse_role_payload_field`
        // at the usecase boundary, before it ever reaches the domain constructor.
        let spec = LintRuleSpec {
            target_roles: vec![],
            kind: LintRuleKind::FieldEmpty { target_field: "emit".to_owned() }, // typo
        };
        let result = lint_rule_spec_to_domain(spec);
        assert!(result.is_err(), "expected Err for unknown target_field 'emit', got Ok");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("emit"),
            "error message should mention the bad target_field, got: {msg}"
        );
    }

    // ------------------------------------------------------------------
    // T017: lint_rule_spec_to_domain — unrecognised forbidden_receiver is rejected
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_rejects_unknown_forbidden_receiver() {
        // `SelfReceiver` is a closed enum; a typo'd receiver string (e.g.
        // "&mutself" instead of "&mut self") must be rejected by
        // `parse_self_receiver` at the usecase boundary, before it ever reaches
        // the domain constructor.
        let spec = LintRuleSpec {
            target_roles: vec![],
            kind: LintRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: "&mutself".to_owned(), // typo
            },
        };
        let result = lint_rule_spec_to_domain(spec);
        assert!(result.is_err(), "expected Err for unknown forbidden_receiver '&mutself', got Ok");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("&mutself"),
            "error message should mention the bad forbidden_receiver, got: {msg}"
        );
    }

    // ------------------------------------------------------------------
    // T018: lint_rule_spec_to_domain — empty-string required_traits element is rejected
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_rejects_empty_required_trait_element() {
        // A non-empty Vec containing an empty-string element would previously
        // have silently produced a TraitImplRequired rule that could never be
        // satisfied by any impl (D19 fail-closed). `TypeRef::new` now rejects
        // the empty string per-element, before NonEmptyVec::try_new even runs.
        let spec = LintRuleSpec {
            target_roles: vec![],
            kind: LintRuleKind::TraitImplRequired { required_traits: vec![String::new()] },
        };
        let result = lint_rule_spec_to_domain(spec);
        assert!(result.is_err(), "expected Err for empty-string required trait element, got Ok");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("required trait"),
            "error message should mention the invalid required trait, got: {msg}"
        );
    }

    // ------------------------------------------------------------------
    // T019: PrimitiveOccurrenceScanner threading (catalogue-primitive-
    // obsession-guard-2026-07-01 T005) — execute() must not invoke the
    // scanner for any of the 12 pre-existing LintRuleKind variants, since
    // none of them lower to CatalogueLinterRuleKind::ForbidPrimitiveInTypes.
    // ------------------------------------------------------------------

    /// Scanner double that panics if `scan()` is ever called. Proves that
    /// wiring a third `S: PrimitiveOccurrenceScanner` generic into
    /// `RunCatalogueLintInteractor` does not change `execute()`'s existing
    /// behaviour for any of the 12 pre-existing rule kinds — `track_lint`'s
    /// behaviour for them is unchanged (T005 SSoT).
    struct PanicIfCalledScanner;

    impl PrimitiveOccurrenceScanner for PanicIfCalledScanner {
        fn scan(
            &self,
            _type_ref: TypeRef,
            _primitives: NonEmptyVec<PrimitiveName>,
            _position: PrimitiveOccurrencePosition,
        ) -> Result<PrimitiveOccurrenceReport, PrimitiveOccurrenceScanError> {
            panic!(
                "scanner.scan() must not be invoked for rule kinds other than ForbidPrimitiveInTypes"
            );
        }
    }

    #[test]
    fn test_execute_does_not_invoke_scanner_for_pre_existing_rule_kinds() {
        let (order, catalogues) = three_layer_result("domain");
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .times(1)
            .returning(move |_| Ok((order.clone(), catalogues.clone())));

        let interactor =
            RunCatalogueLintInteractor::new(loader, StubMissingConfigLoader, PanicIfCalledScanner);
        let result = interactor.execute(cmd("my-track", "domain"));

        assert!(
            result.is_ok(),
            "expected Ok without the scanner ever being invoked, got: {result:?}"
        );
    }

    // ------------------------------------------------------------------
    // T020: ForbidPrimitiveInTypes evaluation path (catalogue-primitive-
    // obsession-guard-2026-07-01 T005) — exercises the same
    // evaluate_catalogue_lint(...) call that execute() makes internally,
    // using the interactor's own scanner double, across a NamedField and a
    // type_alias/TypeAliasTarget catalogue entry.
    //
    // This intentionally does not go through
    // RunCatalogueLintInteractor::execute() / RunCatalogueLintCommand.rules:
    // LintRuleKind's own ForbidPrimitiveInTypes Dto variant and its
    // lint_rule_spec_to_domain conversion are deferred to T006 (SSoT:
    // impl-plan.json), so a ForbidPrimitiveInTypes rule cannot yet be
    // supplied through that public, string-based entry point. Domain-level
    // behavioural coverage (including TypeAliasTarget) already lives in
    // libs/domain/src/tddd/catalogue_linter.rs; this test instead confirms
    // the usecase crate's own catalogue fixtures and re-exported types
    // compose correctly with evaluate_catalogue_lint end to end.
    // ------------------------------------------------------------------

    #[test]
    fn test_forbid_primitive_in_types_named_field_and_type_alias_target() {
        let mut doc = empty_doc("domain");
        doc.types.insert(
            TypeName::new("Money").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain {
                        fields: vec![FieldDecl::new(
                            FieldName::new("amount").unwrap(),
                            TypeRef::new("String").unwrap(),
                        )],
                        has_stripped_fields: false,
                    },
                    None,
                )),
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        doc.types.insert(
            TypeName::new("Description").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
                kind: TypeKindV2::TypeAlias { target: TypeRef::new("String").unwrap() },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        let target_layer = layer("domain");
        let mut catalogues = BTreeMap::new();
        catalogues.insert(target_layer.clone(), doc);

        let rule = CatalogueLinterRule::new(
            RuleTarget::all_roles(),
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes {
                primitives: NonEmptyVec::new(PrimitiveName::new("String").unwrap(), vec![]),
                layers: NonEmptyVec::new(target_layer.clone(), vec![]),
                positions: NonEmptyVec::new(
                    PrimitiveOccurrencePosition::NamedField,
                    vec![PrimitiveOccurrencePosition::TypeAliasTarget],
                ),
            },
        )
        .unwrap();

        let violations =
            evaluate_catalogue_lint(&[rule], &catalogues, &target_layer, &StubScanner).unwrap();

        assert_eq!(
            violations.len(),
            2,
            "expected 1 violation for Money's NamedField and 1 for Description's \
             TypeAliasTarget, got: {violations:?}"
        );
        assert!(
            violations.iter().any(|v| v.entry_name() == "Money"),
            "missing Money NamedField violation: {violations:?}"
        );
        assert!(
            violations.iter().any(|v| v.entry_name() == "Description"),
            "missing Description TypeAliasTarget violation: {violations:?}"
        );
        assert!(violations.iter().all(|v| v.rule_kind() == "ForbidPrimitiveInTypes"));
    }

    // ------------------------------------------------------------------
    // T021: LintRuleKind::ForbidPrimitiveInTypes -- JSON wire-format round
    // trip (catalogue-primitive-obsession-guard-2026-07-01 T006). Confirms
    // the externally-tagged serde representation (PascalCase variant name as
    // the `kind` object's single key, matching every other LintRuleKind
    // variant and .harness/catalogue-lint/config.json's existing rules) --
    // not a `#[serde(tag = "kind")]`-style snake_case string tag.
    // ------------------------------------------------------------------

    #[test]
    fn test_forbid_primitive_in_types_json_round_trips_through_lint_rule_kind() {
        let json = serde_json::json!({
            "ForbidPrimitiveInTypes": {
                "primitives": ["String"],
                "layers": ["domain"],
                "positions": ["result_err"]
            }
        });
        let kind: LintRuleKind =
            serde_json::from_value(json.clone()).expect("valid ForbidPrimitiveInTypes JSON");
        assert_eq!(
            kind,
            LintRuleKind::ForbidPrimitiveInTypes {
                primitives: vec!["String".to_owned()],
                layers: vec!["domain".to_owned()],
                positions: vec!["result_err".to_owned()],
            }
        );

        let round_tripped = serde_json::to_value(&kind).unwrap();
        assert_eq!(round_tripped, json, "serialized form must match the wire format exactly");
    }

    // ------------------------------------------------------------------
    // T022: lint_rule_spec_to_domain -- ForbidPrimitiveInTypes converts
    // successfully into the domain CatalogueLinterRuleKind variant.
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_converts_forbid_primitive_in_types() {
        let spec = LintRuleSpec {
            target_roles: vec![],
            kind: LintRuleKind::ForbidPrimitiveInTypes {
                primitives: vec!["String".to_owned(), "i32".to_owned()],
                layers: vec!["domain".to_owned(), "usecase".to_owned()],
                positions: vec!["named_field".to_owned(), "result_err".to_owned()],
            },
        };
        let rule = lint_rule_spec_to_domain(spec).expect("expected successful conversion");

        match rule.kind() {
            CatalogueLinterRuleKind::ForbidPrimitiveInTypes { primitives, layers, positions } => {
                assert_eq!(
                    primitives.as_slice().to_vec(),
                    vec![PrimitiveName::new("String").unwrap(), PrimitiveName::new("i32").unwrap()]
                );
                assert_eq!(layers.as_slice().to_vec(), vec![layer("domain"), layer("usecase")]);
                assert_eq!(
                    positions.as_slice().to_vec(),
                    vec![
                        PrimitiveOccurrencePosition::NamedField,
                        PrimitiveOccurrencePosition::ResultErr
                    ]
                );
            }
            other => panic!("expected ForbidPrimitiveInTypes, got {other:?}"),
        }
    }

    // ------------------------------------------------------------------
    // T023: lint_rule_spec_to_domain -- ForbidPrimitiveInTypes rejects an
    // empty `primitives` list.
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_rejects_empty_forbid_primitive_in_types_primitives() {
        let spec = LintRuleSpec {
            target_roles: vec![],
            kind: LintRuleKind::ForbidPrimitiveInTypes {
                primitives: vec![],
                layers: vec!["domain".to_owned()],
                positions: vec!["named_field".to_owned()],
            },
        };
        let result = lint_rule_spec_to_domain(spec);
        assert!(result.is_err(), "expected Err for empty primitives, got Ok");
        let msg = result.unwrap_err();
        assert!(msg.contains("primitives"), "error message should mention primitives, got: {msg}");
    }

    // ------------------------------------------------------------------
    // T024: lint_rule_spec_to_domain -- ForbidPrimitiveInTypes rejects an
    // empty `layers` list.
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_rejects_empty_forbid_primitive_in_types_layers() {
        let spec = LintRuleSpec {
            target_roles: vec![],
            kind: LintRuleKind::ForbidPrimitiveInTypes {
                primitives: vec!["String".to_owned()],
                layers: vec![],
                positions: vec!["named_field".to_owned()],
            },
        };
        let result = lint_rule_spec_to_domain(spec);
        assert!(result.is_err(), "expected Err for empty layers, got Ok");
        let msg = result.unwrap_err();
        assert!(msg.contains("layers"), "error message should mention layers, got: {msg}");
    }

    // ------------------------------------------------------------------
    // T025: lint_rule_spec_to_domain -- ForbidPrimitiveInTypes rejects an
    // empty `positions` list.
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_rejects_empty_forbid_primitive_in_types_positions() {
        let spec = LintRuleSpec {
            target_roles: vec![],
            kind: LintRuleKind::ForbidPrimitiveInTypes {
                primitives: vec!["String".to_owned()],
                layers: vec!["domain".to_owned()],
                positions: vec![],
            },
        };
        let result = lint_rule_spec_to_domain(spec);
        assert!(result.is_err(), "expected Err for empty positions, got Ok");
        let msg = result.unwrap_err();
        assert!(msg.contains("positions"), "error message should mention positions, got: {msg}");
    }

    // ------------------------------------------------------------------
    // T026: lint_rule_spec_to_domain -- ForbidPrimitiveInTypes rejects an
    // unparseable position string.
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_rejects_unknown_forbid_primitive_in_types_position() {
        let spec = LintRuleSpec {
            target_roles: vec![],
            kind: LintRuleKind::ForbidPrimitiveInTypes {
                primitives: vec!["String".to_owned()],
                layers: vec!["domain".to_owned()],
                positions: vec!["result_ok".to_owned()], // not a real position
            },
        };
        let result = lint_rule_spec_to_domain(spec);
        assert!(result.is_err(), "expected Err for unknown position 'result_ok', got Ok");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("result_ok"),
            "error message should mention the bad position, got: {msg}"
        );
    }

    // ------------------------------------------------------------------
    // T027: lint_rule_spec_to_domain -- ForbidPrimitiveInTypes rejects an
    // invalid primitive name (non-identifier).
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_rejects_invalid_forbid_primitive_in_types_primitive_name() {
        let spec = LintRuleSpec {
            target_roles: vec![],
            kind: LintRuleKind::ForbidPrimitiveInTypes {
                primitives: vec!["Vec<String>".to_owned()], // not a bare identifier
                layers: vec!["domain".to_owned()],
                positions: vec!["named_field".to_owned()],
            },
        };
        let result = lint_rule_spec_to_domain(spec);
        assert!(result.is_err(), "expected Err for invalid primitive name, got Ok");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("Vec<String>"),
            "error message should mention the bad primitive name, got: {msg}"
        );
    }

    // ------------------------------------------------------------------
    // T028: lint_rule_spec_to_domain -- ForbidPrimitiveInTypes rejects an
    // empty-string primitive name element.
    // ------------------------------------------------------------------

    #[test]
    fn test_lint_rule_spec_to_domain_rejects_empty_forbid_primitive_in_types_primitive_element() {
        let spec = LintRuleSpec {
            target_roles: vec![],
            kind: LintRuleKind::ForbidPrimitiveInTypes {
                primitives: vec![String::new()],
                layers: vec!["domain".to_owned()],
                positions: vec!["named_field".to_owned()],
            },
        };
        let result = lint_rule_spec_to_domain(spec);
        assert!(result.is_err(), "expected Err for empty-string primitive element, got Ok");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("invalid primitive"),
            "error message should mention the invalid primitive, got: {msg}"
        );
    }
}
