//! Type catalogue — declared type entries for the per-track TDDD catalogue.
//!
//! This module owns the **type definitions** only: `TypeAction`,
//! `TypestateTransitions`, `TypeDefinitionKind`, `TypeCatalogueEntry`,
//! `TypeSignal`, and the aggregate root `TypeCatalogueDocument`.
//!
//! Signal evaluation (`evaluate_type_signals` and per-kind evaluators) lives in
//! `super::signals`, and bidirectional consistency + Stage 2 signal-gate
//! checking (`check_consistency`, `check_type_signals`, `ConsistencyReport`)
//! lives in `super::consistency`. The three modules collaborate via
//! `pub`/`pub(crate)` items — consumers should import via the crate-root
//! re-exports in `libs/domain/src/lib.rs` (e.g. `use domain::TypeCatalogueEntry`)
//! rather than from these submodules directly.
//!
//! Historical note (T001): this file used to hold all three responsibilities in
//! a single 2088-line module under the name `DomainType*`. The split and the
//! rename were performed together in the TDDD-01 track (see ADR
//! `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` D3 and DM-06 in
//! `knowledge/strategy/TODO.md`).

use std::collections::HashSet;

use crate::ConfidenceSignal;
use crate::plan_ref::{InformalGroundRef, SpecRef};
use crate::spec::SpecValidationError;

// ---------------------------------------------------------------------------
// ParamDeclaration — single parameter in a method signature
// ---------------------------------------------------------------------------

/// A single parameter in a method signature, captured at L1 resolution.
///
/// L1 resolution means: the type string uses last-segment short names and
/// preserves the generic structure verbatim (e.g. `"Result<Option<User>, DomainError>"`).
/// Module paths (`domain::user::UserId`) are NOT included — codec validation
/// rejects `ty` strings containing `::`.
///
/// # Examples
///
/// ```text
/// // fn find_by_id(&self, id: UserId) -> Result<User, DomainError>
/// // ...
/// // params[0]: { name: "id", ty: "UserId" }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamDeclaration {
    name: String,
    ty: String,
}

impl ParamDeclaration {
    /// Creates a new `ParamDeclaration`.
    #[must_use]
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self { name: name.into(), ty: ty.into() }
    }

    /// Returns the parameter binding name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the parameter type string (L1 short names, generics preserved).
    #[must_use]
    pub fn ty(&self) -> &str {
        &self.ty
    }
}

// ---------------------------------------------------------------------------
// MethodDeclaration — structured method signature at L1 resolution
// ---------------------------------------------------------------------------

/// A structured method signature at L1 resolution.
///
/// Shared across three contexts:
/// - Catalogue declaration: `TypeDefinitionKind::SecondaryPort { expected_methods }`
///   or `TypeDefinitionKind::ApplicationService { expected_methods }` (populated in T006)
/// - `TypeGraph`: `TypeNode::methods` / `TraitNode::methods` (the "code reality"
///   extracted from rustdoc JSON)
/// - Baseline: `TypeBaselineEntry::methods` / `TraitBaselineEntry::methods`
///   (captured snapshot at `/track:design` time)
///
/// Type strings (`ParamDeclaration::ty`, `returns`) use last-segment short
/// names and preserve generics verbatim (e.g. `"Result<Option<User>, DomainError>"`,
/// not `"Result"` or `"domain::user::Result"`). Codec validation rejects
/// strings containing `::`.
///
/// See ADR `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` §D2
/// for the L1 JSON schema and forward-check rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodDeclaration {
    name: String,
    /// Self-receiver form: `"&self"` / `"&mut self"` / `"self"` / `None`
    /// (associated function without a self parameter).
    receiver: Option<String>,
    params: Vec<ParamDeclaration>,
    /// Return type string (`"()"` when the return type is the unit type).
    returns: String,
    /// Whether the method is declared `async fn`.
    is_async: bool,
}

impl MethodDeclaration {
    /// Creates a new `MethodDeclaration`.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        receiver: Option<String>,
        params: Vec<ParamDeclaration>,
        returns: impl Into<String>,
        is_async: bool,
    ) -> Self {
        Self { name: name.into(), receiver, params, returns: returns.into(), is_async }
    }

    /// Returns the method name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the self-receiver form (`"&self"` / `"&mut self"` / `"self"`),
    /// or `None` for associated functions.
    #[must_use]
    pub fn receiver(&self) -> Option<&str> {
        self.receiver.as_deref()
    }

    /// Returns the ordered parameter list (excluding the self receiver).
    #[must_use]
    pub fn params(&self) -> &[ParamDeclaration] {
        &self.params
    }

    /// Returns the return type string.
    #[must_use]
    pub fn returns(&self) -> &str {
        &self.returns
    }

    /// Returns `true` if the method is declared `async fn`.
    #[must_use]
    pub fn is_async(&self) -> bool {
        self.is_async
    }

    /// Reconstructs a human-readable signature string from the structured
    /// fields for rendering / debugging.
    ///
    /// Layout:
    ///
    /// ```text
    /// [async ]fn name(receiver[, param1: ty1, param2: ty2]) -> returns
    /// ```
    ///
    /// The unit return type is rendered as `"()"`.
    #[must_use]
    pub fn signature_string(&self) -> String {
        let async_prefix = if self.is_async { "async " } else { "" };
        let receiver = self.receiver.as_deref().unwrap_or("");
        let params_str = self
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.ty))
            .collect::<Vec<_>>()
            .join(", ");
        let args = match (receiver.is_empty(), params_str.is_empty()) {
            (true, true) => String::new(),
            (true, false) => params_str,
            (false, true) => receiver.to_string(),
            (false, false) => format!("{receiver}, {params_str}"),
        };
        format!("{async_prefix}fn {}({}) -> {}", self.name, args, self.returns)
    }
}

// ---------------------------------------------------------------------------
// MemberDeclaration — composite type member (enum variant or struct field)
// ---------------------------------------------------------------------------

/// A member of a composite type: either an enum variant (name only) or a
/// struct field (name + type string).
///
/// **Enum-first design** (see `.claude/rules/04-coding-principles.md` § Enum-first):
/// the two states carry structurally distinct data — a variant has only a name
/// while a field has a name and a type string. A `struct { name, ty: Option<String> }`
/// shape would allow the illegal `Field { ty: None }` state; the enum shape
/// prevents it at compile time.
///
/// Type strings (on `Field`) follow the same L1 convention as
/// `MethodDeclaration`: last-segment short names, generics preserved verbatim.
/// Module paths containing `::` are rejected by codec validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberDeclaration {
    /// An enum variant: only a name is tracked at L1 (payload types are
    /// out of scope until L2).
    Variant(String),
    /// A struct field with its type string.
    Field { name: String, ty: String },
}

impl MemberDeclaration {
    /// Creates a new enum-variant member.
    #[must_use]
    pub fn variant(name: impl Into<String>) -> Self {
        Self::Variant(name.into())
    }

    /// Creates a new struct-field member.
    #[must_use]
    pub fn field(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self::Field { name: name.into(), ty: ty.into() }
    }

    /// Returns the member name regardless of kind.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Variant(name) => name,
            Self::Field { name, .. } => name,
        }
    }

    /// Returns the field type, or `None` for enum variants.
    #[must_use]
    pub fn ty(&self) -> Option<&str> {
        match self {
            Self::Variant(_) => None,
            Self::Field { ty, .. } => Some(ty),
        }
    }
}

// ---------------------------------------------------------------------------
// TraitImplDecl — trait implementation declaration for SecondaryAdapter
// ---------------------------------------------------------------------------

/// A single trait implementation declaration within a `SecondaryAdapter` entry.
///
/// Holds the trait name the adapter implements and an optional set of
/// expected method signatures (L1 resolution). When `expected_methods` is
/// empty the evaluator checks only for impl existence.
///
/// Type strings follow the same L1 convention as `MethodDeclaration`:
/// last-segment short names, generics preserved verbatim. Module paths
/// containing `::` are rejected by codec validation.
///
/// See ADR `knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md` §D2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitImplDecl {
    trait_name: String,
    expected_methods: Vec<MethodDeclaration>,
}

impl TraitImplDecl {
    /// Creates a new `TraitImplDecl`.
    #[must_use]
    pub fn new(trait_name: impl Into<String>, expected_methods: Vec<MethodDeclaration>) -> Self {
        Self { trait_name: trait_name.into(), expected_methods }
    }

    /// Returns the trait name (L1 last-segment short name).
    #[must_use]
    pub fn trait_name(&self) -> &str {
        &self.trait_name
    }

    /// Returns the expected method signatures (empty = existence check only).
    #[must_use]
    pub fn expected_methods(&self) -> &[MethodDeclaration] {
        &self.expected_methods
    }
}

// ---------------------------------------------------------------------------
// TypeAction enum
// ---------------------------------------------------------------------------

/// Declares the intended operation for a type catalogue entry.
///
/// Used in the per-layer catalogue file (e.g. `domain-types.json`) to record
/// developer intent about how a type should be evaluated relative to the
/// baseline.
///
/// - `Add` (default): type is being newly added
/// - `Modify`: type is being modified from its baseline structure
/// - `Reference`: type is declared as-is for documentation purposes
/// - `Delete`: type is being intentionally deleted (inverts forward check)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TypeAction {
    /// Type is being newly added. Default when field is omitted.
    #[default]
    Add,
    /// Type is being modified from its baseline structure.
    Modify,
    /// Type is declared as-is for documentation purposes (reference only).
    Reference,
    /// Type is being intentionally deleted (inverts forward check).
    Delete,
}

impl TypeAction {
    /// Returns the canonical lowercase string tag for this action.
    #[must_use]
    pub fn action_tag(&self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Modify => "modify",
            Self::Reference => "reference",
            Self::Delete => "delete",
        }
    }

    /// Returns `true` if this is the default action (`Add`).
    #[must_use]
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Add)
    }
}

// ---------------------------------------------------------------------------
// TypeDefinitionKind enum + TypestateTransitions
// ---------------------------------------------------------------------------

/// Declared transitions for a typestate type.
///
/// Makes the terminal vs non-terminal distinction explicit at the type level.
/// An empty `Vec<String>` is structurally impossible in `To` — use `Terminal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypestateTransitions {
    /// This typestate has no outgoing transitions (end state).
    Terminal,
    /// This typestate transitions to the named target states.
    To(Vec<String>),
}

/// Classifies a type-catalogue entry by its structural role in the codebase.
///
/// Each variant carries the expected items that the type should expose so that
/// an automated scanner can compute a `TypeSignal` for the entry.
///
/// Layer-neutral naming (T001): this used to be `DomainTypeKind` when the
/// catalogue lived only in the domain layer. The rename reflects that TDDD now
/// applies to usecase and future layers via `architecture-rules.json` layer
/// blocks (ADR 0002 §D1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDefinitionKind {
    /// A type whose instances carry state-machine phase information.
    Typestate { transitions: TypestateTransitions },
    /// A `pub enum` with a fixed set of variants.
    /// `expected_variants` lists the names that must appear.
    Enum { expected_variants: Vec<String> },
    /// A newtype or small struct used solely as a validated value.
    /// No extra metadata is needed.
    ValueObject,
    /// An `enum` used exclusively as an error type.
    /// `expected_variants` lists the variants that must appear.
    ErrorType { expected_variants: Vec<String> },
    /// A `pub trait` that defines a hexagonal secondary (driven/infrastructure) port boundary.
    ///
    /// `expected_methods` holds the full L1 method signatures that the trait
    /// should expose. The forward check compares each declared method against
    /// the code by name → receiver → params count → params types → return
    /// type → `is_async` (ADR 0002 §D2). A reverse check flags any trait
    /// method found in code that the catalogue does not declare.
    ///
    /// Use `ApplicationService` for primary (driving) port boundaries instead.
    SecondaryPort { expected_methods: Vec<MethodDeclaration> },
    /// A `pub trait` that defines a hexagonal primary (driving/application) port boundary.
    ///
    /// Semantically paired with `SecondaryPort`: where `SecondaryPort` describes
    /// driven/infrastructure ports (repositories, adapters), `ApplicationService`
    /// describes the driving side — the use-case interface exposed to the outer world.
    ///
    /// Forward and reverse checks are identical to `SecondaryPort` (same L1 axes).
    ApplicationService { expected_methods: Vec<MethodDeclaration> },
    /// A struct-only use case type; evaluated by existence check only.
    UseCase,
    /// An `ApplicationService` implementation struct; evaluated by existence check only.
    ///
    /// `declares_application_service` records the L1 short name of the
    /// `ApplicationService` (primary port) the interactor implements. When
    /// `Some(name)` and `name` resolves in the per-track catalogue's
    /// `ApplicationService` index, the Contract Map renderer emits a
    /// `Interactor -.impl.-> ApplicationService` edge (ADR
    /// `2026-04-17-1528-tddd-contract-map.md` §D4 (2) extension).
    /// `None` keeps the historical existence-only behaviour.
    Interactor { declares_application_service: Option<String> },
    /// A pure data-transfer object struct; evaluated by existence check only.
    Dto,
    /// A CQRS command object struct; evaluated by existence check only.
    Command,
    /// A CQRS query object struct; evaluated by existence check only.
    Query,
    /// An aggregate or entity factory struct; evaluated by existence check only.
    Factory,
    /// A struct that implements one or more hexagonal secondary (driven) port
    /// traits.
    ///
    /// `implements` lists each trait the adapter is expected to implement.
    /// An empty `implements` vec means existence-check only (the struct must
    /// exist, no impl constraint is checked).
    ///
    /// See ADR `knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md` §D1.
    SecondaryAdapter { implements: Vec<TraitImplDecl> },
    /// A free function (not a type) declared in the catalogue so the renderer
    /// can attach edges to its parameter and return types.
    ///
    /// `expected_params` and `expected_returns` are declared at L1 resolution
    /// (last-segment short names, generics preserved). Phase 2 forward check
    /// is existence-only (same evaluator path as `ValueObject`); a richer
    /// signature check is deferred to a later phase that adds free-function
    /// support to `TypeGraph` extraction.
    ///
    /// See ADR `knowledge/adr/2026-04-17-1528-tddd-contract-map.md`
    /// § Known Limitations §L4 and `2026-04-13-1813-tddd-taxonomy-expansion`
    /// §D1 (taxonomy extension).
    FreeFunction { expected_params: Vec<ParamDeclaration>, expected_returns: Vec<String> },
}

impl TypeDefinitionKind {
    /// Returns the canonical lowercase string tag for this kind.
    #[must_use]
    pub fn kind_tag(&self) -> &'static str {
        match self {
            Self::Typestate { .. } => "typestate",
            Self::Enum { .. } => "enum",
            Self::ValueObject => "value_object",
            Self::ErrorType { .. } => "error_type",
            Self::SecondaryPort { .. } => "secondary_port",
            Self::ApplicationService { .. } => "application_service",
            Self::UseCase => "use_case",
            Self::Interactor { .. } => "interactor",
            Self::Dto => "dto",
            Self::Command => "command",
            Self::Query => "query",
            Self::Factory => "factory",
            Self::SecondaryAdapter { .. } => "secondary_adapter",
            Self::FreeFunction { .. } => "free_function",
        }
    }
}

// ---------------------------------------------------------------------------
// TypeCatalogueEntry
// ---------------------------------------------------------------------------

/// A single entry in the type catalogue (`<catalogue_file>.json`).
///
/// Each entry records one named type together with its expected structure
/// (`kind`), intended operation (`action`), and whether the entry has been
/// human-approved.
///
/// `spec_refs` holds structured references to spec.json elements (SoT Chain ②).
/// `informal_grounds` holds unpersisted ground citations; non-empty → 🟡 advisory
/// signal per ADR 2026-04-19-1242 §D1.3 / §D3.2.
///
/// Layer-neutral naming (T001, formerly `DomainTypeEntry`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeCatalogueEntry {
    name: String,
    description: String,
    kind: TypeDefinitionKind,
    action: TypeAction,
    approved: bool,
    /// Structured references to spec.json elements (SoT Chain ②).
    /// Empty by default; populated when the entry is traced to a spec requirement.
    spec_refs: Vec<SpecRef>,
    /// Unpersisted ground citations.
    /// Non-empty → 🟡 advisory signal (ADR 2026-04-19-1242 §D3.2).
    informal_grounds: Vec<InformalGroundRef>,
}

impl TypeCatalogueEntry {
    /// Creates a new `TypeCatalogueEntry` with empty `spec_refs` and `informal_grounds`.
    ///
    /// # Errors
    ///
    /// Returns `SpecValidationError::EmptyDomainStateName` if `name` is empty or
    /// whitespace-only. (The error variant keeps its historical name for
    /// compatibility with existing call sites in the spec validator.)
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        kind: TypeDefinitionKind,
        action: TypeAction,
        approved: bool,
    ) -> Result<Self, SpecValidationError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(SpecValidationError::EmptyDomainStateName);
        }
        Ok(Self {
            name,
            description: description.into(),
            kind,
            action,
            approved,
            spec_refs: Vec::new(),
            informal_grounds: Vec::new(),
        })
    }

    /// Creates a new `TypeCatalogueEntry` with explicit `spec_refs` and `informal_grounds`.
    ///
    /// # Errors
    ///
    /// Returns `SpecValidationError::EmptyDomainStateName` if `name` is empty or
    /// whitespace-only.
    /// Returns `SpecValidationError::DuplicateElementId` if two `SpecRef` entries in
    /// `spec_refs` share the same `anchor` (`SpecElementId`). Element IDs must be unique
    /// within a single catalogue entry to avoid ambiguous SoT Chain ② citations.
    pub fn with_refs(
        name: impl Into<String>,
        description: impl Into<String>,
        kind: TypeDefinitionKind,
        action: TypeAction,
        approved: bool,
        spec_refs: Vec<SpecRef>,
        informal_grounds: Vec<InformalGroundRef>,
    ) -> Result<Self, SpecValidationError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(SpecValidationError::EmptyDomainStateName);
        }
        // Validate that no two spec_refs share the same anchor (SpecElementId).
        let mut seen_anchors: HashSet<&str> = HashSet::new();
        for sr in &spec_refs {
            if !seen_anchors.insert(sr.anchor.as_ref()) {
                return Err(SpecValidationError::DuplicateElementId(sr.anchor.as_ref().to_owned()));
            }
        }
        Ok(Self {
            name,
            description: description.into(),
            kind,
            action,
            approved,
            spec_refs,
            informal_grounds,
        })
    }

    /// Returns the type name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the human-readable description.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns the structural classification of this type.
    #[must_use]
    pub fn kind(&self) -> &TypeDefinitionKind {
        &self.kind
    }

    /// Returns the intended operation for this entry.
    #[must_use]
    pub fn action(&self) -> TypeAction {
        self.action
    }

    /// Returns `true` if this entry has been explicitly approved by a maintainer.
    #[must_use]
    pub fn approved(&self) -> bool {
        self.approved
    }

    /// Returns the spec.json element references (SoT Chain ②).
    #[must_use]
    pub fn spec_refs(&self) -> &[SpecRef] {
        &self.spec_refs
    }

    /// Returns the unpersisted ground citations.
    /// Non-empty → 🟡 advisory signal.
    #[must_use]
    pub fn informal_grounds(&self) -> &[InformalGroundRef] {
        &self.informal_grounds
    }

    /// Returns `true` if any informal grounds are present (🟡 advisory signal trigger).
    #[must_use]
    pub fn has_informal_grounds(&self) -> bool {
        !self.informal_grounds.is_empty()
    }
}

// ---------------------------------------------------------------------------
// TypeSignal
// ---------------------------------------------------------------------------

/// Per-type signal evaluation result produced by comparing a
/// `TypeCatalogueEntry` against scanned code output.
///
/// Layer-neutral naming (T001, formerly `DomainTypeSignal`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSignal {
    type_name: String,
    /// Canonical kind tag (e.g. `"typestate"`, `"enum"`, `"value_object"`, …).
    kind_tag: String,
    signal: ConfidenceSignal,
    /// Whether the type was found in the scanned code.
    found_type: bool,
    /// Items (variants / methods / transitions) found in the scanned code.
    found_items: Vec<String>,
    /// Expected items that were not found.
    missing_items: Vec<String>,
    /// Items found in code that were not listed in the entry.
    extra_items: Vec<String>,
}

impl TypeSignal {
    /// Creates a new `TypeSignal`.
    #[must_use]
    pub fn new(
        type_name: impl Into<String>,
        kind_tag: impl Into<String>,
        signal: ConfidenceSignal,
        found_type: bool,
        found_items: Vec<String>,
        missing_items: Vec<String>,
        extra_items: Vec<String>,
    ) -> Self {
        Self {
            type_name: type_name.into(),
            kind_tag: kind_tag.into(),
            signal,
            found_type,
            found_items,
            missing_items,
            extra_items,
        }
    }

    /// Returns the type name.
    #[must_use]
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns the canonical kind tag string.
    #[must_use]
    pub fn kind_tag(&self) -> &str {
        &self.kind_tag
    }

    /// Returns the confidence signal computed from the scan result.
    #[must_use]
    pub fn signal(&self) -> ConfidenceSignal {
        self.signal
    }

    /// Returns `true` if the type was found during the code scan.
    #[must_use]
    pub fn found_type(&self) -> bool {
        self.found_type
    }

    /// Returns the list of items that were found in the scanned code.
    #[must_use]
    pub fn found_items(&self) -> &[String] {
        &self.found_items
    }

    /// Returns the list of expected items not found in the scanned code.
    #[must_use]
    pub fn missing_items(&self) -> &[String] {
        &self.missing_items
    }

    /// Returns the list of items found in code but not declared in the entry.
    #[must_use]
    pub fn extra_items(&self) -> &[String] {
        &self.extra_items
    }
}

// ---------------------------------------------------------------------------
// TypeCatalogueDocument
// ---------------------------------------------------------------------------

/// Aggregate root for a layer's type catalogue (e.g. `domain-types.json`).
///
/// The document records the full set of declared types together with their
/// optional scan signals.
///
/// Layer-neutral naming (T001, formerly `DomainTypesDocument`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeCatalogueDocument {
    schema_version: u32,
    entries: Vec<TypeCatalogueEntry>,
    signals: Option<Vec<TypeSignal>>,
}

impl TypeCatalogueDocument {
    /// Creates a new `TypeCatalogueDocument` with no signals.
    #[must_use]
    pub fn new(schema_version: u32, entries: Vec<TypeCatalogueEntry>) -> Self {
        Self { schema_version, entries, signals: None }
    }

    /// Returns the schema version of this document.
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the type catalogue entries in this document.
    #[must_use]
    pub fn entries(&self) -> &[TypeCatalogueEntry] {
        &self.entries
    }

    /// Returns the scan signals, if they have been populated.
    #[must_use]
    pub fn signals(&self) -> Option<&[TypeSignal]> {
        self.signals.as_deref()
    }

    /// Replaces the signals with a new set derived from a code scan.
    pub fn set_signals(&mut self, signals: Vec<TypeSignal>) {
        self.signals = Some(signals);
    }

    /// Returns the names of entries classified as `Typestate`.
    ///
    /// Used by `build_type_graph` to filter outgoing transitions.
    #[must_use]
    pub fn typestate_names(&self) -> HashSet<String> {
        self.entries
            .iter()
            .filter(|e| matches!(e.kind(), TypeDefinitionKind::Typestate { .. }))
            .map(|e| e.name().to_string())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests — type definitions
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    // --- TypeAction ---

    #[test]
    fn test_type_action_default_is_add() {
        assert_eq!(TypeAction::default(), TypeAction::Add);
    }

    #[test]
    fn test_type_action_is_default_returns_true_for_add() {
        assert!(TypeAction::Add.is_default());
    }

    #[test]
    fn test_type_action_is_default_returns_false_for_non_add() {
        assert!(!TypeAction::Delete.is_default());
        assert!(!TypeAction::Modify.is_default());
        assert!(!TypeAction::Reference.is_default());
    }

    #[test]
    fn test_type_action_tag_returns_canonical_string() {
        assert_eq!(TypeAction::Add.action_tag(), "add");
        assert_eq!(TypeAction::Modify.action_tag(), "modify");
        assert_eq!(TypeAction::Reference.action_tag(), "reference");
        assert_eq!(TypeAction::Delete.action_tag(), "delete");
    }

    // --- TypeCatalogueEntry action field ---

    #[test]
    fn test_type_catalogue_entry_action_defaults_to_add() {
        let entry = TypeCatalogueEntry::new(
            "Foo",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(entry.action(), TypeAction::Add);
    }

    #[test]
    fn test_type_catalogue_entry_stores_delete_action() {
        let entry = TypeCatalogueEntry::new(
            "OldType",
            "Intentionally deleted",
            TypeDefinitionKind::ValueObject,
            TypeAction::Delete,
            true,
        )
        .unwrap();
        assert_eq!(entry.action(), TypeAction::Delete);
    }

    #[test]
    fn test_type_catalogue_entry_stores_modify_action() {
        let entry = TypeCatalogueEntry::new(
            "ChangedType",
            "Modified existing type",
            TypeDefinitionKind::ValueObject,
            TypeAction::Modify,
            true,
        )
        .unwrap();
        assert_eq!(entry.action(), TypeAction::Modify);
    }

    #[test]
    fn test_type_catalogue_entry_stores_reference_action() {
        let entry = TypeCatalogueEntry::new(
            "RefType",
            "Referenced for docs",
            TypeDefinitionKind::ValueObject,
            TypeAction::Reference,
            true,
        )
        .unwrap();
        assert_eq!(entry.action(), TypeAction::Reference);
    }

    // --- TypeCatalogueEntry constructor / accessors ---

    fn typestate_entry() -> TypeCatalogueEntry {
        TypeCatalogueEntry::new(
            "ReviewState",
            "Typestate for review flow",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::To(vec!["Approved".into(), "Rejected".into()]),
            },
            TypeAction::Add,
            true,
        )
        .unwrap()
    }

    #[test]
    fn test_type_catalogue_entry_with_valid_name_succeeds() {
        let entry = typestate_entry();
        assert_eq!(entry.name(), "ReviewState");
        assert_eq!(entry.description(), "Typestate for review flow");
        assert!(entry.approved());
        assert_eq!(entry.kind().kind_tag(), "typestate");
    }

    #[test]
    fn test_type_catalogue_entry_with_empty_name_returns_error() {
        let result = TypeCatalogueEntry::new(
            "",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        );
        assert!(matches!(result, Err(SpecValidationError::EmptyDomainStateName)));
    }

    #[test]
    fn test_type_catalogue_entry_with_whitespace_name_returns_error() {
        let result = TypeCatalogueEntry::new(
            "   ",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        );
        assert!(matches!(result, Err(SpecValidationError::EmptyDomainStateName)));
    }

    #[test]
    fn test_type_catalogue_entry_value_object_kind() {
        let entry = TypeCatalogueEntry::new(
            "Email",
            "Validated email address",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(entry.kind(), &TypeDefinitionKind::ValueObject);
        assert_eq!(entry.kind().kind_tag(), "value_object");
    }

    #[test]
    fn test_type_catalogue_entry_enum_kind_with_variants() {
        let kind = TypeDefinitionKind::Enum {
            expected_variants: vec!["Active".into(), "Inactive".into()],
        };
        let entry = TypeCatalogueEntry::new(
            "Status",
            "Track status enum",
            kind.clone(),
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(entry.kind(), &kind);
        assert_eq!(entry.kind().kind_tag(), "enum");
    }

    #[test]
    fn test_type_catalogue_entry_error_type_kind() {
        let kind = TypeDefinitionKind::ErrorType {
            expected_variants: vec!["NotFound".into(), "InvalidInput".into()],
        };
        let entry = TypeCatalogueEntry::new(
            "DomainError",
            "Domain error type",
            kind.clone(),
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(entry.kind(), &kind);
        assert_eq!(entry.kind().kind_tag(), "error_type");
    }

    #[test]
    fn test_type_catalogue_entry_secondary_port_kind() {
        let kind = TypeDefinitionKind::SecondaryPort {
            expected_methods: vec![
                MethodDeclaration::new(
                    "find_by_id",
                    Some("&self".into()),
                    vec![ParamDeclaration::new("id", "UserId")],
                    "Option<User>",
                    false,
                ),
                MethodDeclaration::new(
                    "save",
                    Some("&self".into()),
                    vec![ParamDeclaration::new("user", "User")],
                    "Result<(), DomainError>",
                    false,
                ),
            ],
        };
        let entry = TypeCatalogueEntry::new(
            "UserRepository",
            "User repo port",
            kind.clone(),
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(entry.kind(), &kind);
        assert_eq!(entry.kind().kind_tag(), "secondary_port");
    }

    #[test]
    fn test_type_catalogue_entry_application_service_kind() {
        let kind = TypeDefinitionKind::ApplicationService {
            expected_methods: vec![MethodDeclaration::new(
                "execute",
                Some("&self".into()),
                vec![ParamDeclaration::new("cmd", "CreateCommand")],
                "Result<(), AppError>",
                false,
            )],
        };
        let entry = TypeCatalogueEntry::new(
            "CreateUseCase",
            "Primary port for create operation",
            kind.clone(),
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(entry.kind(), &kind);
        assert_eq!(entry.kind().kind_tag(), "application_service");
    }

    #[test]
    fn test_type_catalogue_entry_use_case_kind() {
        let entry = TypeCatalogueEntry::new(
            "SaveTrackUseCase",
            "Use case for saving a track",
            TypeDefinitionKind::UseCase,
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(entry.kind(), &TypeDefinitionKind::UseCase);
        assert_eq!(entry.kind().kind_tag(), "use_case");
    }

    #[test]
    fn test_type_catalogue_entry_interactor_kind() {
        let entry = TypeCatalogueEntry::new(
            "SaveTrackInteractor",
            "Interactor implementing SaveTrackUseCase",
            TypeDefinitionKind::Interactor { declares_application_service: None },
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(
            entry.kind(),
            &TypeDefinitionKind::Interactor { declares_application_service: None }
        );
        assert_eq!(entry.kind().kind_tag(), "interactor");
    }

    #[test]
    fn test_type_catalogue_entry_dto_kind() {
        let entry = TypeCatalogueEntry::new(
            "TrackDto",
            "Data transfer object for a track",
            TypeDefinitionKind::Dto,
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(entry.kind(), &TypeDefinitionKind::Dto);
        assert_eq!(entry.kind().kind_tag(), "dto");
    }

    #[test]
    fn test_type_catalogue_entry_command_kind() {
        let entry = TypeCatalogueEntry::new(
            "CreateTrackCommand",
            "CQRS command to create a track",
            TypeDefinitionKind::Command,
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(entry.kind(), &TypeDefinitionKind::Command);
        assert_eq!(entry.kind().kind_tag(), "command");
    }

    #[test]
    fn test_type_catalogue_entry_query_kind() {
        let entry = TypeCatalogueEntry::new(
            "FindTrackQuery",
            "CQRS query to find a track",
            TypeDefinitionKind::Query,
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(entry.kind(), &TypeDefinitionKind::Query);
        assert_eq!(entry.kind().kind_tag(), "query");
    }

    #[test]
    fn test_type_catalogue_entry_factory_kind() {
        let entry = TypeCatalogueEntry::new(
            "TrackFactory",
            "Factory for creating Track aggregates",
            TypeDefinitionKind::Factory,
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(entry.kind(), &TypeDefinitionKind::Factory);
        assert_eq!(entry.kind().kind_tag(), "factory");
    }

    #[test]
    fn test_all_fourteen_kind_tags_are_unique() {
        let tags = [
            TypeDefinitionKind::Typestate { transitions: TypestateTransitions::Terminal }
                .kind_tag(),
            TypeDefinitionKind::Enum { expected_variants: vec![] }.kind_tag(),
            TypeDefinitionKind::ValueObject.kind_tag(),
            TypeDefinitionKind::ErrorType { expected_variants: vec![] }.kind_tag(),
            TypeDefinitionKind::SecondaryPort { expected_methods: vec![] }.kind_tag(),
            TypeDefinitionKind::ApplicationService { expected_methods: vec![] }.kind_tag(),
            TypeDefinitionKind::UseCase.kind_tag(),
            TypeDefinitionKind::Interactor { declares_application_service: None }.kind_tag(),
            TypeDefinitionKind::Dto.kind_tag(),
            TypeDefinitionKind::Command.kind_tag(),
            TypeDefinitionKind::Query.kind_tag(),
            TypeDefinitionKind::Factory.kind_tag(),
            TypeDefinitionKind::SecondaryAdapter { implements: vec![] }.kind_tag(),
            TypeDefinitionKind::FreeFunction { expected_params: vec![], expected_returns: vec![] }
                .kind_tag(),
        ];
        let unique: std::collections::HashSet<&str> = tags.iter().copied().collect();
        assert_eq!(unique.len(), 14, "all 14 kind tags must be distinct");
    }

    #[test]
    fn test_type_catalogue_entry_approved_default_true() {
        let entry = TypeCatalogueEntry::new(
            "Foo",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert!(entry.approved());
    }

    #[test]
    fn test_type_catalogue_entry_approved_false_for_ai_added() {
        let entry = TypeCatalogueEntry::new(
            "AiSuggested",
            "AI-added type",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            false,
        )
        .unwrap();
        assert!(!entry.approved());
    }

    // --- TypeCatalogueDocument ---

    #[test]
    fn test_type_catalogue_document_creation() {
        let entries = vec![typestate_entry()];
        let doc = TypeCatalogueDocument::new(1, entries.clone());
        assert_eq!(doc.schema_version(), 1);
        assert_eq!(doc.entries(), entries.as_slice());
        assert!(doc.signals().is_none());
    }

    #[test]
    fn test_type_catalogue_document_set_signals() {
        let mut doc = TypeCatalogueDocument::new(1, vec![typestate_entry()]);
        let signal = TypeSignal::new(
            "ReviewState",
            "typestate",
            ConfidenceSignal::Blue,
            true,
            vec!["Approved".into(), "Rejected".into()],
            vec![],
            vec![],
        );
        doc.set_signals(vec![signal.clone()]);
        let stored = doc.signals().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored.first().unwrap(), &signal);
    }

    #[test]
    fn test_typestate_names_returns_only_typestate_entries() {
        let typestate = typestate_entry();
        let value_obj = TypeCatalogueEntry::new(
            "Email",
            "Validated email",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap();
        let enum_entry = TypeCatalogueEntry::new(
            "Status",
            "Status enum",
            TypeDefinitionKind::Enum { expected_variants: vec!["Active".into()] },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let doc = TypeCatalogueDocument::new(1, vec![typestate, value_obj, enum_entry]);
        let names = doc.typestate_names();
        assert_eq!(names.len(), 1);
        assert!(names.contains("ReviewState"));
        assert!(!names.contains("Email"));
        assert!(!names.contains("Status"));
    }

    #[test]
    fn test_typestate_names_with_no_typestate_entries_returns_empty_set() {
        let value_obj = TypeCatalogueEntry::new(
            "Email",
            "Validated email",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap();
        let doc = TypeCatalogueDocument::new(1, vec![value_obj]);
        assert!(doc.typestate_names().is_empty());
    }

    #[test]
    fn test_typestate_names_with_multiple_typestate_entries_returns_all() {
        let ts1 = TypeCatalogueEntry::new(
            "StateA",
            "First typestate",
            TypeDefinitionKind::Typestate { transitions: TypestateTransitions::Terminal },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let ts2 = TypeCatalogueEntry::new(
            "StateB",
            "Second typestate",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::To(vec!["StateA".into()]),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let doc = TypeCatalogueDocument::new(1, vec![ts1, ts2]);
        let names = doc.typestate_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains("StateA"));
        assert!(names.contains("StateB"));
    }

    // --- spec_refs and informal_grounds fields ---

    #[test]
    fn test_type_catalogue_entry_new_has_empty_spec_refs_by_default() {
        let entry = TypeCatalogueEntry::new(
            "Foo",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert!(entry.spec_refs().is_empty());
    }

    #[test]
    fn test_type_catalogue_entry_new_has_empty_informal_grounds_by_default() {
        let entry = TypeCatalogueEntry::new(
            "Foo",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert!(entry.informal_grounds().is_empty());
        assert!(!entry.has_informal_grounds());
    }

    #[test]
    fn test_type_catalogue_entry_with_refs_stores_spec_refs() {
        use crate::plan_ref::{ContentHash, SpecElementId, SpecRef};

        let anchor = SpecElementId::try_new("IN-01").unwrap();
        let hash = ContentHash::from_bytes([0u8; 32]);
        let spec_ref = SpecRef::new("track/items/x/spec.json", anchor, hash);
        let entry = TypeCatalogueEntry::with_refs(
            "Bar",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Modify,
            true,
            vec![spec_ref.clone()],
            vec![],
        )
        .unwrap();
        assert_eq!(entry.spec_refs().len(), 1);
        assert_eq!(entry.spec_refs()[0], spec_ref);
    }

    #[test]
    fn test_type_catalogue_entry_with_refs_stores_informal_grounds() {
        use crate::plan_ref::{InformalGroundKind, InformalGroundRef, InformalGroundSummary};

        let summary = InformalGroundSummary::try_new("per user directive").unwrap();
        let ground = InformalGroundRef::new(InformalGroundKind::UserDirective, summary);
        let entry = TypeCatalogueEntry::with_refs(
            "Baz",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            false,
            vec![],
            vec![ground.clone()],
        )
        .unwrap();
        assert_eq!(entry.informal_grounds().len(), 1);
        assert_eq!(entry.informal_grounds()[0], ground);
        assert!(entry.has_informal_grounds());
    }

    #[test]
    fn test_type_catalogue_entry_with_refs_empty_name_returns_error() {
        let result = TypeCatalogueEntry::with_refs(
            "",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
            vec![],
            vec![],
        );
        assert!(matches!(result, Err(SpecValidationError::EmptyDomainStateName)));
    }

    #[test]
    fn test_type_catalogue_entry_with_refs_duplicate_spec_element_id_returns_error() {
        use crate::plan_ref::{ContentHash, SpecElementId, SpecRef};

        let anchor = SpecElementId::try_new("IN-01").unwrap();
        let hash = ContentHash::from_bytes([0u8; 32]);
        let ref1 = SpecRef::new("track/items/x/spec.json", anchor.clone(), hash.clone());
        let ref2 = SpecRef::new("track/items/y/spec.json", anchor.clone(), hash.clone()); // same anchor
        let result = TypeCatalogueEntry::with_refs(
            "Foo",
            "desc",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
            vec![ref1, ref2],
            vec![],
        );
        assert!(
            matches!(result, Err(SpecValidationError::DuplicateElementId(ref id)) if id == "IN-01"),
            "duplicate SpecElementId anchor must be rejected"
        );
    }

    // --- TypeSignal ---

    #[test]
    fn test_type_signal_accessors() {
        let signal = TypeSignal::new(
            "TrackStatus",
            "enum",
            ConfidenceSignal::Yellow,
            true,
            vec!["Active".into()],
            vec!["Done".into()],
            vec!["Legacy".into()],
        );
        assert_eq!(signal.type_name(), "TrackStatus");
        assert_eq!(signal.kind_tag(), "enum");
        assert_eq!(signal.signal(), ConfidenceSignal::Yellow);
        assert!(signal.found_type());
        assert_eq!(signal.found_items(), &["Active"]);
        assert_eq!(signal.missing_items(), &["Done"]);
        assert_eq!(signal.extra_items(), &["Legacy"]);
    }

    // --- TraitImplDecl ---

    #[test]
    fn test_trait_impl_decl_accessors() {
        let methods =
            vec![MethodDeclaration::new("find", Some("&self".into()), vec![], "()", false)];
        let decl = TraitImplDecl::new("ReviewReader", methods.clone());
        assert_eq!(decl.trait_name(), "ReviewReader");
        assert_eq!(decl.expected_methods().len(), 1);
        assert_eq!(decl.expected_methods()[0].name(), "find");
    }

    #[test]
    fn test_trait_impl_decl_empty_methods() {
        let decl = TraitImplDecl::new("TrackWriter", vec![]);
        assert_eq!(decl.trait_name(), "TrackWriter");
        assert!(decl.expected_methods().is_empty());
    }

    // --- SecondaryAdapter variant ---

    #[test]
    fn test_secondary_adapter_kind_tag() {
        let kind = TypeDefinitionKind::SecondaryAdapter { implements: vec![] };
        assert_eq!(kind.kind_tag(), "secondary_adapter");
    }

    #[test]
    fn test_secondary_adapter_with_implements() {
        let decl = TraitImplDecl::new("ReviewReader", vec![]);
        let kind = TypeDefinitionKind::SecondaryAdapter { implements: vec![decl] };
        if let TypeDefinitionKind::SecondaryAdapter { implements } = &kind {
            assert_eq!(implements.len(), 1);
            assert_eq!(implements[0].trait_name(), "ReviewReader");
        } else {
            panic!("expected SecondaryAdapter");
        }
    }
}
