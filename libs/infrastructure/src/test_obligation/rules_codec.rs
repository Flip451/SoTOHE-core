//! JSON codec for the test-obligation decision-table config.
//!
//! Deserialises `.harness/config/test-obligation-rules.json` into serde DTOs and
//! projects them onto the domain [`TestObligationRulesDocument`], enforcing the
//! fail-closed load-time validation (IN-02 / IN-04 / AC-02 / CN-05 / CN-10):
//! unknown role keys, omitted `obligations` fields, invalid rule values, and
//! (via the domain constructor) missing role coverage are each reported with a
//! precise [`TestObligationRulesLoadError`].
//!
//! The role-key resolution matches (`*_key_to_domain`) are exhaustive over the
//! `*Key` DTO enums, so a newly added role variant forces the codec to grow —
//! the config-load counterpart of the compile-time totality guarantee (OS-05).

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole};
use domain::tddd::test_obligation::errors::TestObligationRulesLoadError;
use domain::tddd::test_obligation::ids::{DiagnosticMessage, RoleName};
use domain::tddd::test_obligation::ports::TestObligationRulesLoaderPort;
use domain::tddd::test_obligation::rules::{
    RoleObligationRules, TestObligationBriefTemplate, TestObligationMinimum, TestObligationRule,
    TestObligationRulesDocument,
};
use domain::tddd::test_obligation::vocab::{
    TestObligationKind, TestObligationPatternKind, TestObligationPerAxis,
};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Serde-map key DTOs (role-name keys in the config sections)
// ---------------------------------------------------------------------------

/// Config-key DTO mirroring the domain `DataRole` variants (IN-03 / CN-05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataRoleKey {
    /// `DataRole::ValueObject`.
    ValueObject,
    /// `DataRole::Entity`.
    Entity,
    /// `DataRole::AggregateRoot`.
    AggregateRoot,
    /// `DataRole::DomainService`.
    DomainService,
    /// `DataRole::Specification`.
    Specification,
    /// `DataRole::Factory`.
    Factory,
    /// `DataRole::UseCase`.
    UseCase,
    /// `DataRole::Interactor`.
    Interactor,
    /// `DataRole::Command`.
    Command,
    /// `DataRole::Query`.
    Query,
    /// `DataRole::Dto`.
    Dto,
    /// `DataRole::ErrorType`.
    ErrorType,
    /// `DataRole::SecondaryAdapter`.
    SecondaryAdapter,
    /// `DataRole::EventPolicy`.
    EventPolicy,
    /// `DataRole::DomainEvent`.
    DomainEvent,
    /// `DataRole::CompositionRoot`.
    CompositionRoot,
    /// `DataRole::PrimaryAdapter`.
    PrimaryAdapter,
}

/// Config-key DTO mirroring the domain `ContractRole` variants (IN-03 / CN-05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractRoleKey {
    /// `ContractRole::SecondaryPort`.
    SecondaryPort,
    /// `ContractRole::SpecificationPort`.
    SpecificationPort,
    /// `ContractRole::Repository`.
    Repository,
    /// `ContractRole::ApplicationService`.
    ApplicationService,
}

/// Config-key DTO mirroring the domain `FunctionRole` variants (IN-03 / CN-05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionRoleKey {
    /// `FunctionRole::UseCaseFunction`.
    UseCaseFunction,
    /// `FunctionRole::FreeFunction`.
    FreeFunction,
}

/// Config-key DTO mirroring the domain `TestObligationPatternKind` (IN-03 / CN-05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternKey {
    /// The typestate pattern.
    Typestate,
}

// ---------------------------------------------------------------------------
// Rule DTOs
// ---------------------------------------------------------------------------

/// Serde DTO for the `per` axis of a rule (IN-02 / CN-10).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestObligationPerAxisDto {
    /// One obligation per invariant.
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
    /// One obligation per `emits` event.
    Emits,
    /// One obligation per trait implementation.
    TraitImpl,
}

/// Serde DTO for a single obligation-generation rule (IN-02 / CN-10).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestObligationRuleDto {
    kind: String,
    per: TestObligationPerAxisDto,
    #[serde(default)]
    min: Option<usize>,
    #[serde(default)]
    brief_template: Option<String>,
}

/// Serde DTO for a role's rule list (IN-02 / CN-05).
///
/// `obligations` is optional so an omitted field (`{}`) is distinguishable from
/// an explicit empty list (`{ "obligations": [] }`); the omission is rejected as
/// [`TestObligationRulesLoadError::ObligationsFieldOmitted`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleObligationRulesDto {
    #[serde(default)]
    obligations: Option<Vec<TestObligationRuleDto>>,
}

/// Serde DTO for the whole decision table (IN-02 / IN-03 / CN-05 / AC-01).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestObligationRulesDocumentDto {
    data_roles: BTreeMap<String, RoleObligationRulesDto>,
    contract_roles: BTreeMap<String, RoleObligationRulesDto>,
    function_roles: BTreeMap<String, RoleObligationRulesDto>,
    patterns: BTreeMap<String, RoleObligationRulesDto>,
    trait_impls: BTreeMap<String, RoleObligationRulesDto>,
}

impl TestObligationRulesDocumentDto {
    /// Projects the parsed config onto the domain rules document.
    ///
    /// # Errors
    ///
    /// Returns a [`TestObligationRulesLoadError`] on an unknown role key, an
    /// omitted `obligations` field, an invalid rule value, or (via the domain
    /// constructor) a role-coverage totality violation.
    pub fn into_domain(self) -> Result<TestObligationRulesDocument, TestObligationRulesLoadError> {
        let data_roles = resolve_data_roles(&self.data_roles)?;
        let contract_roles = resolve_contract_roles(&self.contract_roles)?;
        let function_roles = resolve_function_roles(&self.function_roles)?;
        let patterns = resolve_patterns(&self.patterns)?;
        let trait_impls = resolve_contract_roles(&self.trait_impls)?;
        TestObligationRulesDocument::try_new(
            data_roles,
            contract_roles,
            function_roles,
            patterns,
            trait_impls,
        )
    }
}

// ---------------------------------------------------------------------------
// Loader adapter
// ---------------------------------------------------------------------------

/// JSON loader adapter for [`TestObligationRulesLoaderPort`] (IN-02 / IN-04 / AC-02).
#[derive(Debug, Clone)]
pub struct JsonTestObligationRulesLoader {
    config_path: PathBuf,
    trusted_root: PathBuf,
}

impl JsonTestObligationRulesLoader {
    /// Creates a loader that reads the decision table from `config_path`.
    ///
    /// `trusted_root` is the workspace root that owns `.harness/config`; the
    /// loader rejects paths that resolve outside that config directory.
    #[must_use]
    pub fn new(config_path: PathBuf, trusted_root: PathBuf) -> Self {
        Self { config_path, trusted_root }
    }
}

impl TestObligationRulesLoaderPort for JsonTestObligationRulesLoader {
    fn load(&self) -> Result<TestObligationRulesDocument, TestObligationRulesLoadError> {
        let guarded_path = guard_config_path(&self.config_path, &self.trusted_root)?;
        let content = std::fs::read_to_string(&guarded_path).map_err(|e| {
            TestObligationRulesLoadError::IoError(diag(&format!(
                "failed to read {}: {e}",
                self.config_path.display()
            )))
        })?;
        let dto: TestObligationRulesDocumentDto = serde_json::from_str(&content)
            .map_err(|e| TestObligationRulesLoadError::MalformedJson(diag(&e.to_string())))?;
        dto.into_domain()
    }
}

fn guard_config_path(
    config_path: &Path,
    trusted_root: &Path,
) -> Result<PathBuf, TestObligationRulesLoadError> {
    let trusted_root = absolutize_lexical(trusted_root);
    match trusted_root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(TestObligationRulesLoadError::IoError(diag(&format!(
                "refusing to use symlinked rules-config trusted root: {}",
                trusted_root.display()
            ))));
        }
        Ok(_) => {}
        Err(source) => {
            return Err(TestObligationRulesLoadError::IoError(diag(&format!(
                "failed to stat rules-config trusted root {}: {source}",
                trusted_root.display()
            ))));
        }
    }

    let guarded_config_root = trusted_root.join(".harness/config");
    let guarded_path = absolutize_under_root(config_path, &trusted_root);
    if !guarded_path.starts_with(&trusted_root) {
        return Err(TestObligationRulesLoadError::IoError(diag(&format!(
            "rules config path {} escapes trusted root {}",
            config_path.display(),
            trusted_root.display()
        ))));
    }

    guard_config_symlinks(&guarded_config_root, &trusted_root)?;
    guard_config_symlinks(&guarded_path, &trusted_root)?;

    let canonical_config_root = guarded_config_root.canonicalize().map_err(|source| {
        TestObligationRulesLoadError::IoError(diag(&format!(
            "failed to canonicalize trusted config root {}: {source}",
            guarded_config_root.display()
        )))
    })?;
    let canonical_path = guarded_path.canonicalize().map_err(|source| {
        TestObligationRulesLoadError::IoError(diag(&format!(
            "failed to canonicalize rules config {}: {source}",
            guarded_path.display()
        )))
    })?;
    if !canonical_path.starts_with(&canonical_config_root) {
        return Err(TestObligationRulesLoadError::IoError(diag(&format!(
            "rules config path {} escapes trusted config root {}",
            config_path.display(),
            canonical_config_root.display()
        ))));
    }

    Ok(canonical_path)
}

fn guard_config_symlinks(
    config_path: &Path,
    trusted_root: &Path,
) -> Result<(), TestObligationRulesLoadError> {
    match crate::track::symlink_guard::reject_symlinks_below(config_path, trusted_root) {
        Ok(true) => Ok(()),
        Ok(false) => Err(TestObligationRulesLoadError::IoError(diag(&format!(
            "rules config not found: {}",
            config_path.display()
        )))),
        Err(source) => Err(TestObligationRulesLoadError::IoError(diag(&format!(
            "refusing to read rules config {}: {source}",
            config_path.display()
        )))),
    }
}

fn absolutize_lexical(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map(|cwd| cwd.join(path)).unwrap_or_else(|_| path.to_path_buf())
    };
    lexical_normalize(&absolute)
}

fn absolutize_under_root(path: &Path, trusted_root: &Path) -> PathBuf {
    if path.is_absolute() {
        lexical_normalize(path)
    } else {
        lexical_normalize(&trusted_root.join(path))
    }
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut components: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => match components.last() {
                Some(Component::Normal(_)) => {
                    components.pop();
                }
                _ => components.push(component),
            },
            Component::CurDir => {}
            _ => components.push(component),
        }
    }
    components.iter().collect()
}

// ---------------------------------------------------------------------------
// Section resolution (private helpers)
// ---------------------------------------------------------------------------

/// Resolves the `data_roles` section into domain role/rule pairs.
fn resolve_data_roles(
    section: &BTreeMap<String, RoleObligationRulesDto>,
) -> Result<Vec<(DataRole, RoleObligationRules)>, TestObligationRulesLoadError> {
    let mut out = Vec::with_capacity(section.len());
    for (key, dto) in section {
        let role_key = data_role_key_from_config(key).ok_or_else(|| unknown_role_error(key))?;
        let role = data_role_key_to_domain(&role_key)?;
        out.push((role, rules_from_dto(dto, key)?));
    }
    Ok(out)
}

/// Resolves a `ContractRole`-keyed section (`contract_roles` / `trait_impls`).
fn resolve_contract_roles(
    section: &BTreeMap<String, RoleObligationRulesDto>,
) -> Result<Vec<(ContractRole, RoleObligationRules)>, TestObligationRulesLoadError> {
    let mut out = Vec::with_capacity(section.len());
    for (key, dto) in section {
        let role_key = contract_role_key_from_config(key).ok_or_else(|| unknown_role_error(key))?;
        let role = contract_role_key_to_domain(&role_key)?;
        out.push((role, rules_from_dto(dto, key)?));
    }
    Ok(out)
}

/// Resolves the `function_roles` section into domain role/rule pairs.
fn resolve_function_roles(
    section: &BTreeMap<String, RoleObligationRulesDto>,
) -> Result<Vec<(FunctionRole, RoleObligationRules)>, TestObligationRulesLoadError> {
    let mut out = Vec::with_capacity(section.len());
    for (key, dto) in section {
        let role_key = function_role_key_from_config(key).ok_or_else(|| unknown_role_error(key))?;
        let role = function_role_key_to_domain(&role_key);
        out.push((role, rules_from_dto(dto, key)?));
    }
    Ok(out)
}

/// Resolves the `patterns` section into domain pattern/rule pairs.
fn resolve_patterns(
    section: &BTreeMap<String, RoleObligationRulesDto>,
) -> Result<Vec<(TestObligationPatternKind, RoleObligationRules)>, TestObligationRulesLoadError> {
    let mut out = Vec::with_capacity(section.len());
    for (key, dto) in section {
        let pattern_key = pattern_key_from_config(key).ok_or_else(|| unknown_role_error(key))?;
        let pattern = pattern_key_to_domain(&pattern_key);
        out.push((pattern, rules_from_dto(dto, key)?));
    }
    Ok(out)
}

/// Converts a role's rule-list DTO into the domain [`RoleObligationRules`].
fn rules_from_dto(
    dto: &RoleObligationRulesDto,
    role: &str,
) -> Result<RoleObligationRules, TestObligationRulesLoadError> {
    let Some(obligations) = dto.obligations.as_ref() else {
        return Err(omitted_obligations_error(role));
    };
    let mut rules = Vec::with_capacity(obligations.len());
    for rule_dto in obligations {
        rules.push(rule_from_dto(rule_dto, role)?);
    }
    Ok(RoleObligationRules::new(rules))
}

/// Converts a single rule DTO into the domain [`TestObligationRule`].
fn rule_from_dto(
    dto: &TestObligationRuleDto,
    role: &str,
) -> Result<TestObligationRule, TestObligationRulesLoadError> {
    let kind = kind_from_kebab(&dto.kind).ok_or_else(|| {
        invalid_rule_error(role, &format!("unknown obligation kind '{}'", dto.kind))
    })?;
    let per_axis = per_axis_dto_to_domain(&dto.per);
    let minimum = match dto.min {
        Some(value) => Some(
            TestObligationMinimum::try_new(value)
                .map_err(|e| invalid_rule_error(role, &e.to_string()))?,
        ),
        None => None,
    };
    let brief_template = match &dto.brief_template {
        Some(template) => Some(
            TestObligationBriefTemplate::try_new(template.clone())
                .map_err(|e| invalid_rule_error(role, &e.to_string()))?,
        ),
        None => None,
    };
    Ok(TestObligationRule::new(kind, per_axis, minimum, brief_template))
}

// ---------------------------------------------------------------------------
// Vocabulary mapping (private helpers)
// ---------------------------------------------------------------------------

/// Maps a config `kind` kebab string onto the domain obligation kind.
fn kind_from_kebab(kebab: &str) -> Option<TestObligationKind> {
    Some(match kebab {
        "boundary" => TestObligationKind::Boundary,
        "invariant_preservation" => TestObligationKind::InvariantPreservation,
        "event_emission" => TestObligationKind::EventEmission,
        "logic_result" => TestObligationKind::LogicResult,
        "predicate_both_branches" => TestObligationKind::PredicateBothBranches,
        "construction_result" => TestObligationKind::ConstructionResult,
        "result" => TestObligationKind::Result,
        "reaction" => TestObligationKind::Reaction,
        "transition" => TestObligationKind::Transition,
        "contract" => TestObligationKind::Contract,
        "contract_conformance" => TestObligationKind::ContractConformance,
        "logic" => TestObligationKind::Logic,
        _ => return None,
    })
}

/// Projects the `per` axis DTO onto the domain per-axis vocabulary.
fn per_axis_dto_to_domain(dto: &TestObligationPerAxisDto) -> TestObligationPerAxis {
    match dto {
        TestObligationPerAxisDto::Invariant => TestObligationPerAxis::Invariant,
        TestObligationPerAxisDto::Method => TestObligationPerAxis::Method,
        TestObligationPerAxisDto::Handles => TestObligationPerAxis::Handles,
        TestObligationPerAxisDto::ReactsTo => TestObligationPerAxis::ReactsTo,
        TestObligationPerAxisDto::Transition => TestObligationPerAxis::Transition,
        TestObligationPerAxisDto::TraitMethod => TestObligationPerAxis::TraitMethod,
        TestObligationPerAxisDto::Entry => TestObligationPerAxis::Entry,
        TestObligationPerAxisDto::Emits => TestObligationPerAxis::Emits,
        TestObligationPerAxisDto::TraitImpl => TestObligationPerAxis::TraitImpl,
    }
}

// ---------------------------------------------------------------------------
// Role-key resolution (private helpers)
// ---------------------------------------------------------------------------

/// Parses a `data_roles` config key into a [`DataRoleKey`].
fn data_role_key_from_config(key: &str) -> Option<DataRoleKey> {
    Some(match key {
        "ValueObject" => DataRoleKey::ValueObject,
        "Entity" => DataRoleKey::Entity,
        "AggregateRoot" => DataRoleKey::AggregateRoot,
        "DomainService" => DataRoleKey::DomainService,
        "Specification" => DataRoleKey::Specification,
        "Factory" => DataRoleKey::Factory,
        "UseCase" => DataRoleKey::UseCase,
        "Interactor" => DataRoleKey::Interactor,
        "Command" => DataRoleKey::Command,
        "Query" => DataRoleKey::Query,
        "Dto" => DataRoleKey::Dto,
        "ErrorType" => DataRoleKey::ErrorType,
        "SecondaryAdapter" => DataRoleKey::SecondaryAdapter,
        "EventPolicy" => DataRoleKey::EventPolicy,
        "DomainEvent" => DataRoleKey::DomainEvent,
        "CompositionRoot" => DataRoleKey::CompositionRoot,
        "PrimaryAdapter" => DataRoleKey::PrimaryAdapter,
        _ => return None,
    })
}

/// Converts a [`DataRoleKey`] into a placeholder-payload domain `DataRole`.
///
/// Only the role discriminant is meaningful for the rules table, so payload
/// variants use the domain's placeholder constructors. The exhaustive match
/// forces this codec to grow when a `DataRoleKey` variant is added.
fn data_role_key_to_domain(key: &DataRoleKey) -> Result<DataRole, TestObligationRulesLoadError> {
    let role = match key {
        DataRoleKey::ValueObject => DataRole::value_object(),
        DataRoleKey::DomainService => DataRole::domain_service(),
        DataRoleKey::UseCase => DataRole::use_case(),
        DataRoleKey::Specification => DataRole::Specification,
        DataRoleKey::Factory => DataRole::Factory,
        DataRoleKey::Interactor => DataRole::Interactor,
        DataRoleKey::Command => DataRole::Command,
        DataRoleKey::Query => DataRole::Query,
        DataRoleKey::Dto => DataRole::Dto,
        DataRoleKey::ErrorType => DataRole::ErrorType,
        DataRoleKey::SecondaryAdapter => DataRole::SecondaryAdapter,
        DataRoleKey::DomainEvent => DataRole::DomainEvent,
        DataRoleKey::CompositionRoot => DataRole::CompositionRoot,
        DataRoleKey::PrimaryAdapter => DataRole::PrimaryAdapter,
        DataRoleKey::Entity => DataRole::entity().map_err(|_| internal_role_error("Entity"))?,
        DataRoleKey::AggregateRoot => {
            DataRole::aggregate_root().map_err(|_| internal_role_error("AggregateRoot"))?
        }
        DataRoleKey::EventPolicy => {
            DataRole::from_str("EventPolicy").map_err(|_| internal_role_error("EventPolicy"))?
        }
    };
    Ok(role)
}

/// Parses a `ContractRole`-keyed config key into a [`ContractRoleKey`].
fn contract_role_key_from_config(key: &str) -> Option<ContractRoleKey> {
    Some(match key {
        "SecondaryPort" => ContractRoleKey::SecondaryPort,
        "SpecificationPort" => ContractRoleKey::SpecificationPort,
        "Repository" => ContractRoleKey::Repository,
        "ApplicationService" => ContractRoleKey::ApplicationService,
        _ => return None,
    })
}

/// Converts a [`ContractRoleKey`] into a placeholder-payload domain `ContractRole`.
fn contract_role_key_to_domain(
    key: &ContractRoleKey,
) -> Result<ContractRole, TestObligationRulesLoadError> {
    let role = match key {
        ContractRoleKey::SecondaryPort => ContractRole::SecondaryPort,
        ContractRoleKey::SpecificationPort => ContractRole::SpecificationPort,
        ContractRoleKey::ApplicationService => ContractRole::ApplicationService,
        ContractRoleKey::Repository => {
            ContractRole::from_str("Repository").map_err(|_| internal_role_error("Repository"))?
        }
    };
    Ok(role)
}

/// Parses a `function_roles` config key into a [`FunctionRoleKey`].
fn function_role_key_from_config(key: &str) -> Option<FunctionRoleKey> {
    Some(match key {
        "UseCaseFunction" => FunctionRoleKey::UseCaseFunction,
        "FreeFunction" => FunctionRoleKey::FreeFunction,
        _ => return None,
    })
}

/// Converts a [`FunctionRoleKey`] into a domain `FunctionRole`.
fn function_role_key_to_domain(key: &FunctionRoleKey) -> FunctionRole {
    match key {
        FunctionRoleKey::UseCaseFunction => FunctionRole::UseCaseFunction,
        FunctionRoleKey::FreeFunction => FunctionRole::FreeFunction,
    }
}

/// Parses a `patterns` config key into a [`PatternKey`].
fn pattern_key_from_config(key: &str) -> Option<PatternKey> {
    Some(match key {
        "typestate" => PatternKey::Typestate,
        _ => return None,
    })
}

/// Converts a [`PatternKey`] into a domain [`TestObligationPatternKind`].
fn pattern_key_to_domain(key: &PatternKey) -> TestObligationPatternKind {
    match key {
        PatternKey::Typestate => TestObligationPatternKind::Typestate,
    }
}

// ---------------------------------------------------------------------------
// Error constructors (private helpers)
// ---------------------------------------------------------------------------

/// Builds an unknown-role error, downgrading an empty key to malformed JSON.
fn unknown_role_error(key: &str) -> TestObligationRulesLoadError {
    match RoleName::try_new(key.to_owned()) {
        Ok(role_name) => TestObligationRulesLoadError::UnknownRoleName { role_name },
        Err(_) => {
            TestObligationRulesLoadError::MalformedJson(diag("config contains an empty role key"))
        }
    }
}

/// Builds an omitted-`obligations` error for `role`.
fn omitted_obligations_error(role: &str) -> TestObligationRulesLoadError {
    match RoleName::try_new(role.to_owned()) {
        Ok(role_name) => TestObligationRulesLoadError::ObligationsFieldOmitted { role_name },
        Err(_) => {
            TestObligationRulesLoadError::MalformedJson(diag("config contains an empty role key"))
        }
    }
}

/// Builds an invalid-rule-value error for `role` with `message`.
fn invalid_rule_error(role: &str, message: &str) -> TestObligationRulesLoadError {
    match RoleName::try_new(role.to_owned()) {
        Ok(role_name) => {
            TestObligationRulesLoadError::InvalidRuleValue { role_name, message: diag(message) }
        }
        Err(_) => TestObligationRulesLoadError::MalformedJson(diag(message)),
    }
}

/// Builds a malformed-JSON error for an unreachable internal role-construction failure.
fn internal_role_error(role: &str) -> TestObligationRulesLoadError {
    TestObligationRulesLoadError::MalformedJson(diag(&format!(
        "internal error: cannot construct role '{role}'"
    )))
}

/// Builds a [`DiagnosticMessage`], substituting a constant for empty input.
fn diag(message: &str) -> DiagnosticMessage {
    let mut text = if message.trim().is_empty() {
        "unspecified rules-config error".to_owned()
    } else {
        message.to_owned()
    };
    loop {
        match DiagnosticMessage::try_new(text) {
            Ok(message) => return message,
            // Unreachable: `text` is non-empty on entry; reset defensively.
            Err(_) => text = "unspecified rules-config error".to_owned(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
    }

    fn rules(kinds: &[&str]) -> RoleObligationRulesDto {
        RoleObligationRulesDto {
            obligations: Some(
                kinds
                    .iter()
                    .map(|k| TestObligationRuleDto {
                        kind: (*k).to_owned(),
                        per: TestObligationPerAxisDto::Entry,
                        min: None,
                        brief_template: None,
                    })
                    .collect(),
            ),
        }
    }

    fn empty_rules() -> RoleObligationRulesDto {
        RoleObligationRulesDto { obligations: Some(vec![]) }
    }

    const DATA_ROLE_KEYS: &[&str] = &[
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

    const CONTRACT_ROLE_KEYS: &[&str] =
        &["SecondaryPort", "SpecificationPort", "Repository", "ApplicationService"];

    const FUNCTION_ROLE_KEYS: &[&str] = &["UseCaseFunction", "FreeFunction"];

    fn section(keys: &[&str]) -> BTreeMap<String, RoleObligationRulesDto> {
        keys.iter().map(|k| ((*k).to_owned(), empty_rules())).collect()
    }

    fn complete_dto() -> TestObligationRulesDocumentDto {
        let mut patterns = BTreeMap::new();
        patterns.insert("typestate".to_owned(), empty_rules());
        TestObligationRulesDocumentDto {
            data_roles: section(DATA_ROLE_KEYS),
            contract_roles: section(CONTRACT_ROLE_KEYS),
            function_roles: section(FUNCTION_ROLE_KEYS),
            patterns,
            trait_impls: section(CONTRACT_ROLE_KEYS),
        }
    }

    #[test]
    fn test_complete_config_decodes() {
        let doc = complete_dto().into_domain().unwrap();
        assert_eq!(doc.data_roles().len(), 17);
        assert_eq!(doc.contract_roles().len(), 4);
        assert_eq!(doc.function_roles().len(), 2);
        assert_eq!(doc.patterns().len(), 1);
        assert_eq!(doc.trait_impls().len(), 4);
    }

    #[test]
    fn test_rules_document_dto_preserves_generation_brief_template() {
        let mut dto = complete_dto();
        let brief_template = "test the declared boundary".to_owned();
        dto.data_roles.insert(
            "ValueObject".to_owned(),
            RoleObligationRulesDto {
                obligations: Some(vec![TestObligationRuleDto {
                    kind: "boundary".to_owned(),
                    per: TestObligationPerAxisDto::Invariant,
                    min: None,
                    brief_template: Some(brief_template.clone()),
                }]),
            },
        );

        let document = dto.into_domain().unwrap();
        let (_, rules) = document
            .data_roles()
            .iter()
            .find(|(role, _)| *role == DataRole::value_object())
            .unwrap();
        let expected = RoleObligationRules::new(vec![TestObligationRule::new(
            TestObligationKind::Boundary,
            TestObligationPerAxis::Invariant,
            None,
            Some(TestObligationBriefTemplate::try_new(brief_template).unwrap()),
        )]);

        assert_eq!(rules, &expected);
    }

    // AC-02 (a): a role variant missing from the config fails with RoleNotCovered.
    #[test]
    fn test_missing_data_role_is_role_not_covered() {
        let mut dto = complete_dto();
        dto.data_roles.remove("PrimaryAdapter");
        match dto.into_domain() {
            Err(TestObligationRulesLoadError::RoleNotCovered { role_name }) => {
                assert_eq!(role_name.as_str(), "PrimaryAdapter");
            }
            other => panic!("expected RoleNotCovered, got {other:?}"),
        }
    }

    // AC-02 (b): omitting `obligations` fails with ObligationsFieldOmitted.
    #[test]
    fn test_omitted_obligations_is_field_omitted() {
        let mut dto = complete_dto();
        dto.data_roles.insert("Dto".to_owned(), RoleObligationRulesDto { obligations: None });
        match dto.into_domain() {
            Err(TestObligationRulesLoadError::ObligationsFieldOmitted { role_name }) => {
                assert_eq!(role_name.as_str(), "Dto");
            }
            other => panic!("expected ObligationsFieldOmitted, got {other:?}"),
        }
    }

    // AC-02 (c): an unknown role key fails with UnknownRoleName.
    #[test]
    fn test_unknown_role_key_is_unknown_role_name() {
        let mut dto = complete_dto();
        dto.data_roles.insert("Frobnicator".to_owned(), empty_rules());
        match dto.into_domain() {
            Err(TestObligationRulesLoadError::UnknownRoleName { role_name }) => {
                assert_eq!(role_name.as_str(), "Frobnicator");
            }
            other => panic!("expected UnknownRoleName, got {other:?}"),
        }
    }

    // AC-16: the trait_impls section must cover every ContractRole variant.
    #[test]
    fn test_missing_trait_impl_role_is_role_not_covered() {
        let mut dto = complete_dto();
        dto.trait_impls.remove("ApplicationService");
        assert!(matches!(
            dto.into_domain(),
            Err(TestObligationRulesLoadError::RoleNotCovered { .. })
        ));
    }

    #[test]
    fn test_unknown_kind_is_invalid_rule_value() {
        let mut dto = complete_dto();
        dto.data_roles.insert("ValueObject".to_owned(), rules(&["frobnicate"]));
        match dto.into_domain() {
            Err(TestObligationRulesLoadError::InvalidRuleValue { role_name, message }) => {
                assert_eq!(role_name.as_str(), "ValueObject");
                assert!(message.as_str().contains("frobnicate"));
            }
            other => panic!("expected InvalidRuleValue, got {other:?}"),
        }
    }

    #[test]
    fn test_per_axis_snake_case_decodes() {
        let dto: TestObligationRuleDto =
            serde_json::from_str(r#"{ "kind": "reaction", "per": "reacts_to" }"#).unwrap();
        assert_eq!(dto.per, TestObligationPerAxisDto::ReactsTo);
        assert_eq!(per_axis_dto_to_domain(&dto.per), TestObligationPerAxis::ReactsTo);
    }

    // CN-10: the rules codec accepts precisely the complete per-axis
    // vocabulary used by the default decision table.
    #[test]
    fn test_per_axis_dto_decodes_all_nine_closed_vocabulary_variants() {
        for (wire, expected) in [
            ("invariant", TestObligationPerAxis::Invariant),
            ("method", TestObligationPerAxis::Method),
            ("handles", TestObligationPerAxis::Handles),
            ("reacts_to", TestObligationPerAxis::ReactsTo),
            ("transition", TestObligationPerAxis::Transition),
            ("trait_method", TestObligationPerAxis::TraitMethod),
            ("entry", TestObligationPerAxis::Entry),
            ("emits", TestObligationPerAxis::Emits),
            ("trait_impl", TestObligationPerAxis::TraitImpl),
        ] {
            let dto: TestObligationPerAxisDto =
                serde_json::from_str(&format!("\"{wire}\"")).unwrap();
            assert_eq!(per_axis_dto_to_domain(&dto), expected);
        }
    }

    #[test]
    fn test_min_zero_is_invalid_rule_value() {
        let mut dto = complete_dto();
        dto.data_roles.insert(
            "UseCase".to_owned(),
            RoleObligationRulesDto {
                obligations: Some(vec![TestObligationRuleDto {
                    kind: "result".to_owned(),
                    per: TestObligationPerAxisDto::Handles,
                    min: Some(0),
                    brief_template: None,
                }]),
            },
        );
        assert!(matches!(
            dto.into_domain(),
            Err(TestObligationRulesLoadError::InvalidRuleValue { .. })
        ));
    }

    // T005 smoke test: the shipped default config decodes with zero totality failures.
    #[test]
    fn test_default_config_loads_successfully() {
        let trusted_root = workspace_root();
        let config_path = trusted_root.join(".harness/config/test-obligation-rules.json");
        let loader = JsonTestObligationRulesLoader::new(config_path, trusted_root);
        let doc = loader.load().expect("default test-obligation-rules.json must load");
        assert_eq!(doc.data_roles().len(), 17);
        assert_eq!(doc.contract_roles().len(), 4);
        assert_eq!(doc.function_roles().len(), 2);
        assert_eq!(doc.patterns().len(), 1);
        assert_eq!(doc.trait_impls().len(), 4);
    }

    // AC-01 / IN-02 / IN-03: the shipped decision table keeps every role
    // explicit, including zero-obligation roles, and preserves the declared
    // kind × per-axis rules through its DTO and domain value objects.
    #[test]
    fn test_default_config_preserves_explicit_rule_shapes_and_empty_lists() {
        let trusted_root = workspace_root();
        let config_path = trusted_root.join(".harness/config/test-obligation-rules.json");
        let source = std::fs::read_to_string(&config_path).unwrap();
        let wire: serde_json::Value = serde_json::from_str(&source).unwrap();

        for section_name in
            ["data_roles", "contract_roles", "function_roles", "patterns", "trait_impls"]
        {
            let section = wire[section_name].as_object().unwrap();
            assert!(
                section.values().all(|entry| entry["obligations"].is_array()),
                "{section_name} must state every obligation list explicitly"
            );
        }

        let doc = JsonTestObligationRulesLoader::new(config_path, trusted_root).load().unwrap();
        let (_, value_object_rules) =
            doc.data_roles().iter().find(|(role, _)| *role == DataRole::value_object()).unwrap();
        let [value_object_rule] = value_object_rules.obligations() else {
            panic!("ValueObject must have its declared boundary rule");
        };
        assert_eq!(value_object_rule.kind(), &TestObligationKind::Boundary);
        assert_eq!(value_object_rule.per_axis(), &TestObligationPerAxis::Invariant);

        let (_, dto_rules) =
            doc.data_roles().iter().find(|(role, _)| *role == DataRole::Dto).unwrap();
        assert!(dto_rules.is_empty_explicitly());

        let (_, trait_impl_rules) = doc
            .trait_impls()
            .iter()
            .find(|(role, _)| *role == ContractRole::ApplicationService)
            .unwrap();
        assert!(trait_impl_rules.is_empty_explicitly());
    }

    #[test]
    fn test_default_config_covers_the_declared_role_universe_and_generation_rules() {
        let trusted_root = workspace_root();
        let config_path = trusted_root.join(".harness/config/test-obligation-rules.json");
        let source = std::fs::read_to_string(&config_path).unwrap();
        let wire: serde_json::Value = serde_json::from_str(&source).unwrap();

        for (section, expected) in [
            ("data_roles", DATA_ROLE_KEYS),
            ("contract_roles", CONTRACT_ROLE_KEYS),
            ("function_roles", FUNCTION_ROLE_KEYS),
            ("trait_impls", CONTRACT_ROLE_KEYS),
        ] {
            let mut actual: Vec<&str> =
                wire[section].as_object().unwrap().keys().map(String::as_str).collect();
            actual.sort_unstable();
            let mut expected = expected.to_vec();
            expected.sort_unstable();
            assert_eq!(actual, expected, "{section} must cover its full role universe");
        }
        assert!(wire["patterns"].get("typestate").is_some());

        assert_eq!(wire["data_roles"]["ValueObject"]["obligations"][0]["kind"], "boundary");
        assert_eq!(wire["data_roles"]["ValueObject"]["obligations"][0]["per"], "invariant");
        assert_eq!(
            wire["trait_impls"]["SecondaryPort"]["obligations"][0]["kind"],
            "contract_conformance"
        );
        assert_eq!(wire["trait_impls"]["SecondaryPort"]["obligations"][0]["per"], "trait_impl");
        assert!(
            wire["trait_impls"]["ApplicationService"]["obligations"].as_array().unwrap().is_empty()
        );

        let document: TestObligationRulesDocumentDto = serde_json::from_str(&source).unwrap();
        let domain = document.into_domain().unwrap();
        assert_eq!(domain.data_roles().len(), DATA_ROLE_KEYS.len());
        assert_eq!(domain.contract_roles().len(), CONTRACT_ROLE_KEYS.len());
        assert_eq!(domain.function_roles().len(), FUNCTION_ROLE_KEYS.len());
        assert_eq!(domain.trait_impls().len(), CONTRACT_ROLE_KEYS.len());
    }

    #[test]
    fn test_loader_rejects_role_incomplete_document_after_reading_config_file() {
        // IN-04 / AC-02: the port must validate totality after reading JSON,
        // rather than relying on callers to invoke DTO conversion themselves.
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".harness/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("test-obligation-rules.json");
        std::fs::write(
            &config_path,
            r#"{
                "data_roles": {},
                "contract_roles": {},
                "function_roles": {},
                "patterns": {},
                "trait_impls": {}
            }"#,
        )
        .unwrap();

        let loader = JsonTestObligationRulesLoader::new(config_path, dir.path().to_path_buf());
        assert!(matches!(loader.load(), Err(TestObligationRulesLoadError::RoleNotCovered { .. })));
    }

    #[test]
    fn test_loader_rejects_omitted_obligations_and_unknown_roles_after_reading_json() {
        let source = std::fs::read_to_string(
            workspace_root().join(".harness/config/test-obligation-rules.json"),
        )
        .unwrap();

        for (replacement, expected_role) in [
            (
                source.replace(
                    "\"Dto\":              { \"obligations\": [] }",
                    "\"Dto\":              {}",
                ),
                "Dto",
            ),
            (
                source.replace(
                    "\"Dto\":              { \"obligations\": [] },",
                    "\"Dto\":              { \"obligations\": [] },\n    \"UnknownRole\":      { \"obligations\": [] },",
                ),
                "UnknownRole",
            ),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let config_dir = dir.path().join(".harness/config");
            std::fs::create_dir_all(&config_dir).unwrap();
            let config_path = config_dir.join("test-obligation-rules.json");
            std::fs::write(&config_path, replacement).unwrap();
            let loader = JsonTestObligationRulesLoader::new(config_path, dir.path().to_path_buf());

            match loader.load() {
                Err(TestObligationRulesLoadError::ObligationsFieldOmitted { role_name }) => {
                    assert_eq!(expected_role, "Dto");
                    assert_eq!(role_name.as_str(), expected_role);
                }
                Err(TestObligationRulesLoadError::UnknownRoleName { role_name }) => {
                    assert_eq!(expected_role, "UnknownRole");
                    assert_eq!(role_name.as_str(), expected_role);
                }
                other => panic!("expected read-time rules validation failure, got {other:?}"),
            }
        }
    }

    #[test]
    fn test_loader_rejects_path_traversal_outside_harness_config() {
        let dir = tempfile::tempdir().unwrap();
        let harness_config = dir.path().join(".harness/config");
        std::fs::create_dir_all(&harness_config).unwrap();
        let source_config = workspace_root().join(".harness/config/test-obligation-rules.json");
        std::fs::copy(source_config, dir.path().join(".harness/outside.json")).unwrap();

        let traversal_config = harness_config.join("../outside.json");
        let loader = JsonTestObligationRulesLoader::new(traversal_config, dir.path().to_path_buf());
        match loader.load() {
            Err(TestObligationRulesLoadError::IoError(message)) => {
                assert!(message.as_str().contains("escapes trusted config root"));
            }
            other => panic!("expected path traversal IoError, got {other:?}"),
        }
    }

    #[test]
    fn test_loader_rejects_path_outside_harness_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".harness/config")).unwrap();
        let source_config = workspace_root().join(".harness/config/test-obligation-rules.json");
        let outside_config = dir.path().join("outside.json");
        std::fs::copy(source_config, &outside_config).unwrap();

        let loader = JsonTestObligationRulesLoader::new(outside_config, dir.path().to_path_buf());
        match loader.load() {
            Err(TestObligationRulesLoadError::IoError(message)) => {
                assert!(message.as_str().contains("escapes trusted config root"));
            }
            other => panic!("expected trusted-root IoError, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_loader_rejects_symlinked_config_path() {
        let dir = tempfile::tempdir().unwrap();
        let harness_config = dir.path().join(".harness/config");
        std::fs::create_dir_all(&harness_config).unwrap();
        let source_config = workspace_root().join(".harness/config/test-obligation-rules.json");
        let real_config = harness_config.join("real.json");
        let symlink_config = harness_config.join("test-obligation-rules.json");
        std::fs::copy(source_config, &real_config).unwrap();
        std::os::unix::fs::symlink(&real_config, &symlink_config).unwrap();

        let loader = JsonTestObligationRulesLoader::new(symlink_config, dir.path().to_path_buf());
        match loader.load() {
            Err(TestObligationRulesLoadError::IoError(message)) => {
                assert!(message.as_str().contains("symlink"));
            }
            other => panic!("expected symlink IoError, got {other:?}"),
        }
    }
}
