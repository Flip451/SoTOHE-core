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
//!
//! ## T008 partial — V1 type migration (MethodDeclaration / ParamDeclaration)
//!
//! The V1 `MethodDeclaration` and `ParamDeclaration` (plain `String` fields) were
//! deleted and replaced by `catalogue_v2::methods` equivalents (newtype fields:
//! `MethodName`, `ParamName`, `TypeRef`, `SelfReceiver`).  All workspace call sites
//! were migrated to the V2 newtype API in the same change; the workspace compiles
//! cleanly with no String-argument callers remaining.  The `pub use` below
//! re-exports the V2 types at the old `catalogue::*` path so that import paths stay
//! stable across the migration (not a backward-compatibility shim).

use std::collections::HashSet;

use crate::ConfidenceSignal;
use crate::plan_ref::{InformalGroundRef, SpecRef};
use crate::spec::SpecValidationError;
// Re-exports that preserve the `catalogue::MethodDeclaration` / `catalogue::ParamDeclaration`
// module paths after V1 types were deleted and replaced by V2 (catalogue_v2::methods).
// All call sites have been migrated to the V2 newtype API (MethodName / ParamName / TypeRef /
// SelfReceiver); the `pub use` keeps import paths stable across the migration.
// These types are NOT source-compatible with the old V1 constructor signatures.
pub use crate::tddd::catalogue_v2::methods::{MethodDeclaration, ParamDeclaration};

// ---------------------------------------------------------------------------
// Identifier validation helpers (module-private)
// ---------------------------------------------------------------------------

/// Returns `true` if `s` is a syntactically valid Rust enum-variant identifier.
///
/// Rules applied:
/// - Matches `[a-zA-Z_][a-zA-Z0-9_]*` (ASCII-only subset of the Rust identifier grammar).
/// - Rejects the bare wildcard `"_"` which is a placeholder in Rust and cannot serve as a
///   meaningful enum-variant name.
///
/// Keyword names are accepted because rustdoc strips the `r#` prefix from raw identifiers
/// (e.g. `r#type` is exported as `"type"`), so rejecting keywords would create false
/// contract mismatches against valid Rust enums that use raw identifiers as variant names.
///
/// Rust also permits XID_Continue Unicode characters in identifiers, but catalogue entries
/// always use ASCII-only L1 names so ASCII-only checking is the correct invariant here.
fn is_valid_rust_identifier(s: &str) -> bool {
    if s == "_" {
        return false;
    }
    let mut chars = s.chars();
    match chars.next() {
        None => false,
        Some(first) => {
            (first.is_ascii_alphabetic() || first == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
    }
}

// ---------------------------------------------------------------------------
// EnumVariantDeclaration — enum variant with optional payload types
// ---------------------------------------------------------------------------

/// An enum variant declaration capturing the variant name and its payload
/// type strings at L1 resolution.
///
/// `payload_types` holds the complete type strings (generic arguments included)
/// for each field in the variant payload:
/// - Tuple variant `Foo(Bar, Baz)` → `payload_types: ["Bar", "Baz"]`
/// - Struct variant `Foo { x: Bar }` → `payload_types: ["Bar"]`
/// - Unit variant `Foo` → `payload_types: []`
///
/// Type strings use last-segment short names; module paths containing `::`
/// are rejected by codec validation (L1 invariant, CN-03).
///
/// # Field visibility and invariant contract
///
/// `name` and `payload_types` are declared `pub` so that they appear in the
/// rustdoc-derived schema export, allowing the TDDD L2 member-visibility check
/// to verify that these fields match the catalogue declaration
/// (`expected_members: ["name", "payload_types"]`). Without `pub` visibility,
/// the fields are invisible to rustdoc and the L2 check reports a yellow signal.
///
/// **Accepted encapsulation tradeoff**: direct field assignment (e.g.
/// `evd.name = "invalid::name".to_string()`) bypasses the validation enforced
/// by [`try_new`](EnumVariantDeclaration::try_new). This is the same category of
/// known bypass as the infallible [`new`](EnumVariantDeclaration::new) constructor
/// documented below. All application and codec paths use `try_new` or `new` for
/// construction; callers that require validated invariants must go through one of
/// those constructors and must not mutate fields directly afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariantDeclaration {
    pub name: String,
    pub payload_types: Vec<String>,
}

impl EnumVariantDeclaration {
    /// Creates a new `EnumVariantDeclaration`, validating that `name` is a
    /// syntactically valid Rust identifier and that each entry in `payload_types`
    /// follows the L1 short-name convention (no `::` module path separators).
    ///
    /// A valid variant name matches `[a-zA-Z_][a-zA-Z0-9_]*` — no spaces,
    /// no path separators (`::`) and no other punctuation that cannot appear
    /// in a real Rust enum variant.
    ///
    /// Prefer this constructor in application / test code to enforce the domain
    /// invariants that a variant name must identify a real Rust variant and that
    /// type strings must use L1 last-segment short names (CN-03).
    ///
    /// # Errors
    ///
    /// Returns `SpecValidationError::EmptyVariantName` if `name` is empty or
    /// contains only whitespace.
    ///
    /// Returns `SpecValidationError::InvalidVariantName` if `name` is non-empty
    /// but fails the Rust-identifier character rules.
    ///
    /// Returns `SpecValidationError::InvalidPayloadType` if any `payload_types` entry
    /// is empty, contains only whitespace, or contains `::` (module path separator,
    /// violates CN-03 L1 invariant).
    pub fn try_new(
        name: impl Into<String>,
        payload_types: Vec<String>,
    ) -> Result<Self, SpecValidationError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(SpecValidationError::EmptyVariantName);
        }
        if !is_valid_rust_identifier(&name) {
            return Err(SpecValidationError::InvalidVariantName(name));
        }
        for pt in &payload_types {
            // Reject empty, whitespace-only, strings with leading/trailing whitespace, and
            // strings with '::' module path separators (CN-03 L1 invariant).
            if pt.trim().is_empty() || pt.as_str() != pt.trim() || pt.contains("::") {
                return Err(SpecValidationError::InvalidPayloadType(pt.clone()));
            }
        }
        Ok(Self { name, payload_types })
    }

    /// Creates a new `EnumVariantDeclaration` without name validation.
    ///
    /// **Prefer `try_new` in new call sites.** This infallible variant exists
    /// for codec and render paths that have already validated the name upstream
    /// (e.g. `catalogue_codec` rejects empty names at the JSON boundary).
    /// Passing an empty or whitespace-only name produces a value that violates
    /// the domain invariant.
    #[must_use]
    pub fn new(name: impl Into<String>, payload_types: Vec<String>) -> Self {
        Self { name: name.into(), payload_types }
    }

    /// Returns the variant name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the payload type strings (empty for unit variants).
    #[must_use]
    pub fn payload_types(&self) -> &[String] {
        &self.payload_types
    }
}

// ---------------------------------------------------------------------------
// MemberDeclaration — composite type member (enum variant or struct field)
// ---------------------------------------------------------------------------

/// A member of a composite type: either an enum variant (name + payload types)
/// or a struct field (name + type string).
///
/// **Enum-first design** (see `.claude/rules/04-coding-principles.md` § Enum-first):
/// the two states carry structurally distinct data — a variant has a name and
/// payload types while a field has a name and a type string. A
/// `struct { name, ty: Option<String> }` shape would allow the illegal
/// `Field { ty: None }` state; the enum shape prevents it at compile time.
///
/// Type strings (on `Field` and in `Variant.payload_types`) follow the same L1
/// convention as `MethodDeclaration`: last-segment short names, generics preserved
/// verbatim. Module paths containing `::` are rejected by codec validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberDeclaration {
    /// An enum variant: name + payload type strings at L1 resolution.
    /// Unit variants carry an empty `payload_types` vec.
    Variant(EnumVariantDeclaration),
    /// A struct field with its type string.
    Field { name: String, ty: String },
}

impl MemberDeclaration {
    /// Creates a new enum-variant member with the given payload types.
    #[must_use]
    pub fn variant(name: impl Into<String>, payload_types: Vec<String>) -> Self {
        Self::Variant(EnumVariantDeclaration::new(name, payload_types))
    }

    /// Creates a new unit enum-variant member (no payload).
    #[must_use]
    pub fn unit_variant(name: impl Into<String>) -> Self {
        Self::Variant(EnumVariantDeclaration::new(name, Vec::new()))
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
            Self::Variant(evd) => evd.name(),
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
/// Layer-neutral naming: this used to be `DomainTypeKind` when the catalogue
/// lived only in the domain layer. The rename reflects that TDDD now applies
/// to usecase and future layers via `architecture-rules.json` layer blocks
/// (ADR 0002 §D1).
///
/// M1 / S1 (ADR 2026-04-28-0135): Every struct-based kind (Typestate,
/// ValueObject, UseCase, Interactor, Dto, Command, Query, Factory,
/// SecondaryAdapter) carries both `expected_members` and `expected_methods`
/// uniformly — the schema represents capability, semantic restrictions
/// (e.g. value_object having no behavioral methods) are enforced separately
/// by the catalogue linter (S3). The new `DomainService` variant covers
/// behavioral structs that own field state and behavioral methods but do
/// not implement an `ApplicationService` trait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDefinitionKind {
    /// A type whose instances carry state-machine phase information.
    ///
    /// `expected_members` lists the struct fields the typestate is expected
    /// to expose (L1 resolution). `expected_methods` lists the L1 method
    /// signatures expected on this typestate (e.g. transition functions).
    Typestate {
        transitions: TypestateTransitions,
        expected_members: Vec<MemberDeclaration>,
        expected_methods: Vec<MethodDeclaration>,
    },
    /// A `pub enum` with a fixed set of variants.
    /// `expected_variants` lists the variant declarations that must appear.
    Enum { expected_variants: Vec<EnumVariantDeclaration> },
    /// A newtype or small struct used solely as a validated value.
    ///
    /// `expected_members` lists the struct fields. `expected_methods` is
    /// schema-uniform (M1) but value_object semantics restrict behavioral
    /// methods — this is enforced by the catalogue linter (S3 field-empty
    /// rule), not by the schema.
    ValueObject {
        expected_members: Vec<MemberDeclaration>,
        expected_methods: Vec<MethodDeclaration>,
    },
    /// An `enum` used exclusively as an error type.
    /// `expected_variants` lists the variant declarations that must appear.
    ErrorType { expected_variants: Vec<EnumVariantDeclaration> },
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
    /// A struct-only use case type.
    ///
    /// `expected_members` lists the struct fields. `expected_methods` lists
    /// the L1 method signatures expected on this use case struct.
    UseCase { expected_members: Vec<MemberDeclaration>, expected_methods: Vec<MethodDeclaration> },
    /// An `ApplicationService` implementation struct.
    ///
    /// `expected_members` lists the struct fields. `expected_methods` lists
    /// the L1 method signatures expected on the interactor itself (in
    /// addition to `ApplicationService` trait methods, which are checked
    /// via `declares_application_service`).
    ///
    /// `declares_application_service` lists the `ApplicationService` trait
    /// names that this interactor is expected to implement. Domain layer
    /// allows empty Vec; codec enforces at-least-one constraint.
    Interactor {
        expected_members: Vec<MemberDeclaration>,
        declares_application_service: Vec<String>,
        expected_methods: Vec<MethodDeclaration>,
    },
    /// A pure data-transfer object struct.
    ///
    /// `expected_members` lists the struct fields. `expected_methods` is
    /// schema-uniform (M1) but DTO semantics restrict behavioral methods
    /// to encoding/decoding helpers — restriction enforced by the catalogue
    /// linter (S3), not by the schema.
    Dto { expected_members: Vec<MemberDeclaration>, expected_methods: Vec<MethodDeclaration> },
    /// A CQRS command object struct.
    ///
    /// `expected_members` lists the struct fields. `expected_methods`
    /// follows the schema-uniform M1 model.
    Command { expected_members: Vec<MemberDeclaration>, expected_methods: Vec<MethodDeclaration> },
    /// A CQRS query object struct.
    ///
    /// `expected_members` lists the struct fields. `expected_methods`
    /// follows the schema-uniform M1 model.
    Query { expected_members: Vec<MemberDeclaration>, expected_methods: Vec<MethodDeclaration> },
    /// An aggregate or entity factory struct.
    ///
    /// `expected_members` lists the struct fields. `expected_methods` lists
    /// the L1 method signatures expected on this factory (typically
    /// reconstitution / construction methods).
    Factory { expected_members: Vec<MemberDeclaration>, expected_methods: Vec<MethodDeclaration> },
    /// A struct that implements one or more hexagonal secondary (driven) port
    /// traits.
    ///
    /// `implements` lists each trait the adapter is expected to implement.
    /// An empty `implements` vec means existence-check only (the struct must
    /// exist, no impl constraint is checked).
    ///
    /// `expected_members` lists the struct fields. `expected_methods` lists
    /// L1 method signatures defined directly on the adapter struct (e.g.
    /// constructor / helper methods); trait method signatures live under
    /// `implements[].expected_methods`. Renderer `methods_of()` will merge
    /// the two sources once T003 is implemented (S2 planned behaviour).
    ///
    /// See ADR `knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md` §D1.
    SecondaryAdapter {
        implements: Vec<TraitImplDecl>,
        expected_members: Vec<MemberDeclaration>,
        expected_methods: Vec<MethodDeclaration>,
    },
    /// A behavioral struct that owns field state and behavioral methods,
    /// but does not implement an `ApplicationService` trait — the canonical
    /// home for DDD-style domain services and similar.
    ///
    /// Layer constraint (S1): `domain_service` is allowed in the `domain`
    /// layer (✓), allowed in the `usecase` layer with explicit grounds (△),
    /// and forbidden in the `infrastructure` layer (✗). Enforcement lives
    /// in the catalogue linter (S3 kind-layer constraint), not in this
    /// schema definition.
    ///
    /// Differs from `Interactor`: `domain_service` does not declare an
    /// `ApplicationService` trait implementation. If the type implements
    /// `ApplicationService` choose `Interactor`; if it implements a
    /// `SecondaryPort` choose `SecondaryAdapter`; otherwise this kind.
    DomainService {
        expected_members: Vec<MemberDeclaration>,
        expected_methods: Vec<MethodDeclaration>,
    },
    /// A free function (non-method top-level or sub-module pub fn).
    ///
    /// `module_path` is `None` for top-level pub fn declarations, or
    /// `Some("sub::module")` for functions nested in a submodule.
    ///
    /// `expected_params` holds the parameter declarations at L1 resolution.
    /// `expected_returns` holds the return-type identifiers (last-segment short
    /// names, generics preserved verbatim). Multiple entries allow representing
    /// `Result<T, E>` style generics.
    /// `expected_is_async` indicates whether the function is declared `async fn`.
    FreeFunction {
        module_path: Option<String>,
        expected_params: Vec<ParamDeclaration>,
        expected_returns: Vec<String>,
        expected_is_async: bool,
    },
}

impl TypeDefinitionKind {
    /// Returns the canonical lowercase string tag for this kind.
    #[must_use]
    pub fn kind_tag(&self) -> &'static str {
        match self {
            Self::Typestate { .. } => "typestate",
            Self::Enum { .. } => "enum",
            Self::ValueObject { .. } => "value_object",
            Self::ErrorType { .. } => "error_type",
            Self::SecondaryPort { .. } => "secondary_port",
            Self::ApplicationService { .. } => "application_service",
            Self::UseCase { .. } => "use_case",
            Self::Interactor { .. } => "interactor",
            Self::Dto { .. } => "dto",
            Self::Command { .. } => "command",
            Self::Query { .. } => "query",
            Self::Factory { .. } => "factory",
            Self::SecondaryAdapter { .. } => "secondary_adapter",
            Self::DomainService { .. } => "domain_service",
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

    /// Returns the confidence signal as a lowercase string (`"blue"`, `"yellow"`, or `"red"`).
    ///
    /// Callers that only need to branch on the signal level (e.g. CLI display
    /// code under CN-01) should prefer this over `signal()` so they do not need
    /// to import `domain::ConfidenceSignal` directly.
    #[must_use]
    pub fn signal_as_str(&self) -> &'static str {
        match self.signal {
            ConfidenceSignal::Blue => "blue",
            ConfidenceSignal::Yellow => "yellow",
            ConfidenceSignal::Red => "red",
        }
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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use crate::tddd::catalogue_v2::identifiers::{MethodName, ParamName, TypeRef};
    use crate::tddd::catalogue_v2::roles::SelfReceiver;

    /// Build a [`ParamDeclaration`] from plain `&str` values (test helper).
    fn mk_param(name: &str, ty: &str) -> ParamDeclaration {
        ParamDeclaration::new(
            ParamName::new(name).expect("test param name"),
            TypeRef::new(ty).expect("test param ty"),
        )
    }

    /// Build a [`MethodDeclaration`] from plain `&str` values (test helper).
    fn mk_method(
        name: &str,
        receiver: Option<SelfReceiver>,
        params: Vec<ParamDeclaration>,
        returns: &str,
    ) -> MethodDeclaration {
        MethodDeclaration::new(
            MethodName::new(name).expect("test method name"),
            receiver,
            params,
            TypeRef::new(returns).expect("test return type"),
            false,
            None,
        )
    }

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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(
            entry.kind(),
            &TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new()
            }
        );
        assert_eq!(entry.kind().kind_tag(), "value_object");
    }

    // --- EnumVariantDeclaration ---

    #[test]
    fn test_enum_variant_declaration_try_new_with_valid_name_succeeds() {
        let evd = EnumVariantDeclaration::try_new("Active", vec![]).unwrap();
        assert_eq!(evd.name(), "Active");
        assert!(evd.payload_types().is_empty());
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_payload_types_succeeds() {
        let evd =
            EnumVariantDeclaration::try_new("Wrap", vec!["String".into(), "u32".into()]).unwrap();
        assert_eq!(evd.name(), "Wrap");
        assert_eq!(evd.payload_types(), &["String", "u32"]);
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_empty_name_returns_error() {
        let result = EnumVariantDeclaration::try_new("", vec![]);
        assert!(matches!(result, Err(SpecValidationError::EmptyVariantName)));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_whitespace_name_returns_error() {
        let result = EnumVariantDeclaration::try_new("   ", vec![]);
        assert!(matches!(result, Err(SpecValidationError::EmptyVariantName)));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_space_in_name_returns_invalid_error() {
        let result = EnumVariantDeclaration::try_new("Foo Bar", vec![]);
        assert!(matches!(result, Err(SpecValidationError::InvalidVariantName(_))));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_path_separator_returns_invalid_error() {
        let result = EnumVariantDeclaration::try_new("Foo::Bar", vec![]);
        assert!(matches!(result, Err(SpecValidationError::InvalidVariantName(_))));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_underscore_prefix_succeeds() {
        let evd = EnumVariantDeclaration::try_new("_Private", vec![]).unwrap();
        assert_eq!(evd.name(), "_Private");
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_bare_underscore_returns_invalid_error() {
        let result = EnumVariantDeclaration::try_new("_", vec![]);
        assert!(matches!(result, Err(SpecValidationError::InvalidVariantName(_))));
    }

    /// rustdoc strips `r#` from raw-identifier variant names (e.g. `r#type` → `"type"`),
    /// so keyword strings must be accepted as valid variant names. Otherwise valid
    /// Rust enums using raw identifiers would create false contract mismatches.
    #[test]
    fn test_enum_variant_declaration_try_new_with_rust_keyword_succeeds() {
        for kw in ["fn", "type", "union", "match", "where"] {
            let evd = EnumVariantDeclaration::try_new(kw, vec![]).unwrap_or_else(|e| {
                panic!("keyword '{kw}' must be accepted (raw-identifier): {e}")
            });
            assert_eq!(evd.name(), kw);
        }
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_module_path_payload_type_returns_error() {
        let result = EnumVariantDeclaration::try_new("Wrap", vec!["domain::UserId".into()]);
        assert!(matches!(result, Err(SpecValidationError::InvalidPayloadType(_))));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_valid_payload_type_succeeds() {
        let evd = EnumVariantDeclaration::try_new("Wrap", vec!["UserId".into()]).unwrap();
        assert_eq!(evd.payload_types(), &["UserId"]);
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_empty_payload_type_returns_error() {
        let result = EnumVariantDeclaration::try_new("Wrap", vec!["".into()]);
        assert!(matches!(result, Err(SpecValidationError::InvalidPayloadType(_))));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_whitespace_payload_type_returns_error() {
        let result = EnumVariantDeclaration::try_new("Wrap", vec!["  ".into()]);
        assert!(matches!(result, Err(SpecValidationError::InvalidPayloadType(_))));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_leading_whitespace_payload_type_returns_error() {
        let result = EnumVariantDeclaration::try_new("Wrap", vec![" UserId".into()]);
        assert!(matches!(result, Err(SpecValidationError::InvalidPayloadType(_))));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_trailing_whitespace_payload_type_returns_error() {
        let result = EnumVariantDeclaration::try_new("Wrap", vec!["UserId ".into()]);
        assert!(matches!(result, Err(SpecValidationError::InvalidPayloadType(_))));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_generic_payload_type_succeeds() {
        // Generic payload types like "Vec<String>" or "Result<User, DomainError>" are valid.
        let evd = EnumVariantDeclaration::try_new("Wrap", vec!["Result<User, DomainError>".into()])
            .unwrap();
        assert_eq!(evd.payload_types(), &["Result<User, DomainError>"]);
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_alphanumeric_name_succeeds() {
        let evd = EnumVariantDeclaration::try_new("State1", vec![]).unwrap();
        assert_eq!(evd.name(), "State1");
    }

    #[test]
    fn test_member_declaration_variant_constructor_with_payload_types() {
        let m = MemberDeclaration::variant("Wrap", vec!["i64".into()]);
        assert_eq!(m.name(), "Wrap");
        assert!(m.ty().is_none());
        if let MemberDeclaration::Variant(evd) = &m {
            assert_eq!(evd.payload_types(), &["i64"]);
        } else {
            panic!("expected Variant");
        }
    }

    #[test]
    fn test_type_catalogue_entry_enum_kind_with_variants() {
        let kind = TypeDefinitionKind::Enum {
            expected_variants: vec![
                EnumVariantDeclaration::new("Active", vec![]),
                EnumVariantDeclaration::new("Inactive", vec![]),
            ],
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
            expected_variants: vec![
                EnumVariantDeclaration::new("NotFound", vec![]),
                EnumVariantDeclaration::new("InvalidInput", vec![]),
            ],
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
                mk_method(
                    "find_by_id",
                    Some(SelfReceiver::SharedRef),
                    vec![mk_param("id", "UserId")],
                    "Option<User>",
                ),
                mk_method(
                    "save",
                    Some(SelfReceiver::SharedRef),
                    vec![mk_param("user", "User")],
                    "Result<(), DomainError>",
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
            expected_methods: vec![mk_method(
                "execute",
                Some(SelfReceiver::SharedRef),
                vec![mk_param("cmd", "CreateCommand")],
                "Result<(), AppError>",
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
            TypeDefinitionKind::UseCase {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(
            entry.kind(),
            &TypeDefinitionKind::UseCase {
                expected_members: Vec::new(),
                expected_methods: Vec::new()
            }
        );
        assert_eq!(entry.kind().kind_tag(), "use_case");
    }

    #[test]
    fn test_type_catalogue_entry_interactor_kind() {
        let entry = TypeCatalogueEntry::new(
            "SaveTrackInteractor",
            "Interactor implementing SaveTrackUseCase",
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(
            entry.kind(),
            &TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: Vec::new(),
                expected_methods: Vec::new(),
            }
        );
        assert_eq!(entry.kind().kind_tag(), "interactor");
    }

    #[test]
    fn test_type_catalogue_entry_dto_kind() {
        let entry = TypeCatalogueEntry::new(
            "TrackDto",
            "Data transfer object for a track",
            TypeDefinitionKind::Dto { expected_members: Vec::new(), expected_methods: Vec::new() },
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(
            entry.kind(),
            &TypeDefinitionKind::Dto { expected_members: Vec::new(), expected_methods: Vec::new() }
        );
        assert_eq!(entry.kind().kind_tag(), "dto");
    }

    #[test]
    fn test_type_catalogue_entry_command_kind() {
        let entry = TypeCatalogueEntry::new(
            "CreateTrackCommand",
            "CQRS command to create a track",
            TypeDefinitionKind::Command {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(
            entry.kind(),
            &TypeDefinitionKind::Command {
                expected_members: Vec::new(),
                expected_methods: Vec::new()
            }
        );
        assert_eq!(entry.kind().kind_tag(), "command");
    }

    #[test]
    fn test_type_catalogue_entry_query_kind() {
        let entry = TypeCatalogueEntry::new(
            "FindTrackQuery",
            "CQRS query to find a track",
            TypeDefinitionKind::Query {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(
            entry.kind(),
            &TypeDefinitionKind::Query {
                expected_members: Vec::new(),
                expected_methods: Vec::new()
            }
        );
        assert_eq!(entry.kind().kind_tag(), "query");
    }

    #[test]
    fn test_type_catalogue_entry_factory_kind() {
        let entry = TypeCatalogueEntry::new(
            "TrackFactory",
            "Factory for creating Track aggregates",
            TypeDefinitionKind::Factory {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        assert_eq!(
            entry.kind(),
            &TypeDefinitionKind::Factory {
                expected_members: Vec::new(),
                expected_methods: Vec::new()
            }
        );
        assert_eq!(entry.kind().kind_tag(), "factory");
    }

    #[test]
    fn test_all_fourteen_kind_tags_are_unique() {
        let tags = [
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::Terminal,
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            }
            .kind_tag(),
            TypeDefinitionKind::Enum { expected_variants: Vec::new() }.kind_tag(),
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            }
            .kind_tag(),
            TypeDefinitionKind::ErrorType { expected_variants: Vec::new() }.kind_tag(),
            TypeDefinitionKind::SecondaryPort { expected_methods: vec![] }.kind_tag(),
            TypeDefinitionKind::ApplicationService { expected_methods: vec![] }.kind_tag(),
            TypeDefinitionKind::UseCase {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            }
            .kind_tag(),
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: Vec::new(),
                expected_methods: Vec::new(),
            }
            .kind_tag(),
            TypeDefinitionKind::Dto { expected_members: Vec::new(), expected_methods: Vec::new() }
                .kind_tag(),
            TypeDefinitionKind::Command {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            }
            .kind_tag(),
            TypeDefinitionKind::Query {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            }
            .kind_tag(),
            TypeDefinitionKind::Factory {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            }
            .kind_tag(),
            TypeDefinitionKind::SecondaryAdapter {
                implements: vec![],
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            }
            .kind_tag(),
            TypeDefinitionKind::DomainService {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            }
            .kind_tag(),
            TypeDefinitionKind::FreeFunction {
                module_path: None,
                expected_params: Vec::new(),
                expected_returns: Vec::new(),
                expected_is_async: false,
            }
            .kind_tag(),
        ];
        let unique: std::collections::HashSet<&str> = tags.iter().copied().collect();
        assert_eq!(unique.len(), 15, "all 15 kind tags must be distinct");
    }

    // --- M1 / S1: expected_methods uniformity + DomainService variant ---

    #[test]
    fn test_struct_kinds_carry_expected_methods_field() {
        // Build one method declaration to confirm every struct-based kind
        // (M1) accepts a non-empty `expected_methods` list at the type level.
        let method = mk_method("len", Some(SelfReceiver::SharedRef), vec![], "usize");
        let methods = vec![method];

        let typestate = TypeDefinitionKind::Typestate {
            transitions: TypestateTransitions::Terminal,
            expected_members: Vec::new(),
            expected_methods: methods.clone(),
        };
        let value_object = TypeDefinitionKind::ValueObject {
            expected_members: Vec::new(),
            expected_methods: methods.clone(),
        };
        let use_case = TypeDefinitionKind::UseCase {
            expected_members: Vec::new(),
            expected_methods: methods.clone(),
        };
        let interactor = TypeDefinitionKind::Interactor {
            expected_members: Vec::new(),
            declares_application_service: Vec::new(),
            expected_methods: methods.clone(),
        };
        let dto = TypeDefinitionKind::Dto {
            expected_members: Vec::new(),
            expected_methods: methods.clone(),
        };
        let command = TypeDefinitionKind::Command {
            expected_members: Vec::new(),
            expected_methods: methods.clone(),
        };
        let query = TypeDefinitionKind::Query {
            expected_members: Vec::new(),
            expected_methods: methods.clone(),
        };
        let factory = TypeDefinitionKind::Factory {
            expected_members: Vec::new(),
            expected_methods: methods.clone(),
        };
        let secondary_adapter = TypeDefinitionKind::SecondaryAdapter {
            implements: Vec::new(),
            expected_members: Vec::new(),
            expected_methods: methods.clone(),
        };

        // PartialEq comparison verifies the field exists with the expected
        // payload — if the field were missing the construct would not even
        // compile (E0063), which is itself part of the M1 invariant.
        assert_eq!(typestate, typestate.clone());
        assert_eq!(value_object, value_object.clone());
        assert_eq!(use_case, use_case.clone());
        assert_eq!(interactor, interactor.clone());
        assert_eq!(dto, dto.clone());
        assert_eq!(command, command.clone());
        assert_eq!(query, query.clone());
        assert_eq!(factory, factory.clone());
        assert_eq!(secondary_adapter, secondary_adapter.clone());
    }

    #[test]
    fn test_domain_service_variant_carries_members_and_methods() {
        let member = MemberDeclaration::Field {
            name: "audit_log".to_owned(),
            ty: "Vec<AuditEntry>".to_owned(),
        };
        let method = mk_method(
            "transfer",
            Some(SelfReceiver::ExclusiveRef),
            vec![mk_param("from", "AccountId"), mk_param("to", "AccountId")],
            "Result<(), DomainError>",
        );

        let kind = TypeDefinitionKind::DomainService {
            expected_members: vec![member.clone()],
            expected_methods: vec![method.clone()],
        };

        assert_eq!(kind.kind_tag(), "domain_service");
        match kind {
            TypeDefinitionKind::DomainService { expected_members, expected_methods } => {
                assert_eq!(expected_members.len(), 1);
                assert_eq!(expected_methods.len(), 1);
                assert_eq!(expected_methods[0].name.as_str(), "transfer");
            }
            _ => panic!("expected DomainService variant"),
        }
    }

    #[test]
    fn test_type_catalogue_entry_approved_default_true() {
        let entry = TypeCatalogueEntry::new(
            "Foo",
            "desc",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let enum_entry = TypeCatalogueEntry::new(
            "Status",
            "Status enum",
            TypeDefinitionKind::Enum {
                expected_variants: vec![EnumVariantDeclaration::new("Active", vec![])],
            },
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::Terminal,
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let ts2 = TypeCatalogueEntry::new(
            "StateB",
            "Second typestate",
            TypeDefinitionKind::Typestate {
                transitions: TypestateTransitions::To(vec!["StateA".into()]),
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
        let methods = vec![mk_method("find", Some(SelfReceiver::SharedRef), vec![], "()")];
        let decl = TraitImplDecl::new("ReviewReader", methods.clone());
        assert_eq!(decl.trait_name(), "ReviewReader");
        assert_eq!(decl.expected_methods().len(), 1);
        assert_eq!(decl.expected_methods()[0].name.as_str(), "find");
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
        let kind = TypeDefinitionKind::SecondaryAdapter {
            implements: vec![],
            expected_members: Vec::new(),
            expected_methods: Vec::new(),
        };
        assert_eq!(kind.kind_tag(), "secondary_adapter");
    }

    #[test]
    fn test_secondary_adapter_with_implements() {
        let decl = TraitImplDecl::new("ReviewReader", vec![]);
        let kind = TypeDefinitionKind::SecondaryAdapter {
            implements: vec![decl],
            expected_members: Vec::new(),
            expected_methods: Vec::new(),
        };
        if let TypeDefinitionKind::SecondaryAdapter { implements, .. } = &kind {
            assert_eq!(implements.len(), 1);
            assert_eq!(implements[0].trait_name(), "ReviewReader");
        } else {
            panic!("expected SecondaryAdapter");
        }
    }
}
