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
    RoleKind, RuleTarget, evaluate_catalogue_lint,
};
use domain::tddd::catalogue_ports::{CatalogueLoader, CatalogueLoaderError};
use domain::tddd::catalogue_v2::roles::NonEmptyVec;
use domain::tddd::layer_id::LayerId;
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
/// Generic over `L: CatalogueLoader` and `C: LintConfigLoader` so callers
/// (e.g. the CLI composition root) pass concrete types without importing
/// domain port traits or usecase config port traits directly.
///
/// Rule source priority (D19 fail-closed precedence):
/// 1. `command.rules` non-empty → use CLI-supplied rules.
/// 2. `command.rules` empty → load from `config_loader.load()`.
///    - [`LintConfigLoaderError::MissingFile`] → [`RunCatalogueLintError::ConfigMissing`].
///    - Other load errors → [`RunCatalogueLintError::ConfigInvalid`].
pub struct RunCatalogueLintInteractor<L: CatalogueLoader, C: LintConfigLoader> {
    loader: L,
    config_loader: C,
}

impl<L: CatalogueLoader, C: LintConfigLoader> RunCatalogueLintInteractor<L, C> {
    /// Creates a new interactor wrapping the supplied catalogue loader and
    /// config loader.
    #[must_use]
    pub fn new(loader: L, config_loader: C) -> Self {
        Self { loader, config_loader }
    }
}

impl<L: CatalogueLoader + Send + Sync, C: LintConfigLoader + Send + Sync> RunCatalogueLint
    for RunCatalogueLintInteractor<L, C>
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
            .map_err(RunCatalogueLintError::InvalidRuleSpec)?;

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
        let violations = evaluate_catalogue_lint(&rules, &catalogues, target_layer)?;

        Ok(violations)
    }
}

/// Convert a [`LintRuleSpec`] to a domain [`CatalogueLinterRule`].
///
/// Returns `Err(String)` when the spec is rejected by the domain constructors
/// (e.g. empty `target_field`, unknown role string, empty required_traits).
fn lint_rule_spec_to_domain(spec: LintRuleSpec) -> Result<CatalogueLinterRule, String> {
    // Convert target_roles strings to RoleKind.
    let target_roles =
        spec.target_roles.iter().map(|s| parse_role_kind(s)).collect::<Result<Vec<_>, _>>()?;
    let target = RuleTarget::new(target_roles);

    // Convert LintRuleKind to CatalogueLinterRuleKind.
    let kind = match spec.kind {
        LintRuleKind::FieldEmpty { target_field } => {
            CatalogueLinterRuleKind::FieldEmpty { target_field }
        }
        LintRuleKind::FieldNonEmpty { target_field } => {
            CatalogueLinterRuleKind::FieldNonEmpty { target_field }
        }
        LintRuleKind::KindLayerConstraint { permitted_layers } => {
            let layers: Vec<LayerId> = permitted_layers
                .into_iter()
                .map(|s| {
                    LayerId::try_new(s.clone()).map_err(|e| format!("invalid layer_id '{s}': {e}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let non_empty = NonEmptyVec::try_new(layers)
                .map_err(|_| "permitted_layers must not be empty".to_owned())?;
            CatalogueLinterRuleKind::KindLayerConstraint { permitted_layers: non_empty }
        }
        LintRuleKind::ReferencedRoleConstraint { target_field, expected_role } => {
            let role = parse_role_kind(&expected_role)?;
            CatalogueLinterRuleKind::ReferencedRoleConstraint { target_field, expected_role: role }
        }
        LintRuleKind::TraitImplRequired { required_traits } => {
            let non_empty = NonEmptyVec::try_new(required_traits)
                .map_err(|_| "required_traits must not be empty".to_owned())?;
            CatalogueLinterRuleKind::TraitImplRequired { required_traits: non_empty }
        }
        LintRuleKind::NoRoleInMethodSignature { forbidden_roles } => {
            let roles: Vec<RoleKind> = forbidden_roles
                .iter()
                .map(|s| parse_role_kind(s))
                .collect::<Result<Vec<_>, _>>()?;
            let non_empty = NonEmptyVec::try_new(roles)
                .map_err(|_| "forbidden_roles must not be empty".to_owned())?;
            CatalogueLinterRuleKind::NoRoleInMethodSignature { forbidden_roles: non_empty }
        }
        LintRuleKind::MethodReferenceSignature { target_field } => {
            CatalogueLinterRuleKind::MethodReferenceSignature { target_field }
        }
        LintRuleKind::AccessorSignatureRequired { target_field } => {
            CatalogueLinterRuleKind::AccessorSignatureRequired { target_field }
        }
        LintRuleKind::FieldElementUniqueAcrossEntries { target_field } => {
            CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries { target_field }
        }
        LintRuleKind::NoExternalReferenceInMethods { target_field } => {
            CatalogueLinterRuleKind::NoExternalReferenceInMethods { target_field }
        }
        LintRuleKind::NoPublicField => CatalogueLinterRuleKind::NoPublicField,
        LintRuleKind::ForbiddenMethodReceiver { forbidden_receiver } => {
            CatalogueLinterRuleKind::ForbiddenMethodReceiver { forbidden_receiver }
        }
    };

    CatalogueLinterRule::new(target, kind).map_err(|e| e.to_string())
}

/// Parse a role kind string into a [`RoleKind`].
fn parse_role_kind(s: &str) -> Result<RoleKind, String> {
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
        other => Err(format!("unknown role kind: '{other}'")),
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
    use domain::tddd::catalogue_linter::CatalogueLintViolation;
    use domain::tddd::catalogue_ports::{CatalogueLoader, CatalogueLoaderError};
    use domain::tddd::catalogue_v2::document::CatalogueDocument;
    use domain::tddd::catalogue_v2::entries::TypeEntry;
    use domain::tddd::catalogue_v2::identifiers::{CrateName, ModulePath, TypeName};
    use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
    use domain::tddd::catalogue_v2::{StructKind, StructShape, TypeKindV2};
    use domain::tddd::layer_id::LayerId;
    use mockall::mock;

    use super::*;

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

        let interactor = RunCatalogueLintInteractor::new(loader, StubMissingConfigLoader);
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

        let interactor = RunCatalogueLintInteractor::new(loader, StubMissingConfigLoader);
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

        let interactor = RunCatalogueLintInteractor::new(loader, StubMissingConfigLoader);
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
                kind: LintRuleKind::FieldEmpty { target_field: "f".to_owned() },
            },
            LintRuleSpec {
                target_roles: vec![],
                kind: LintRuleKind::FieldNonEmpty { target_field: "f".to_owned() },
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
        // CatalogueLinterRule::new (EmptyTargetField).
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

        let interactor = RunCatalogueLintInteractor::new(loader2, StubMissingConfigLoader);
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

        let interactor = RunCatalogueLintInteractor::new(loader, StubMissingConfigLoader);
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

        let interactor = RunCatalogueLintInteractor::new(loader, StubParseErrorConfigLoader);
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

        let interactor = RunCatalogueLintInteractor::new(loader, config_loader);
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
        let interactor = RunCatalogueLintInteractor::new(loader, StubNeverCalledConfigLoader);
        // cmd() provides non-empty rules via CLI, so config_loader must not be called.
        let result = interactor.execute(cmd("my-track", "domain"));

        assert!(result.is_ok(), "expected Ok with CLI rules bypassing config, got: {result:?}");
    }
}
