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
use crate::spec::SpecValidationError;

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
    /// A `pub trait` that defines a hexagonal port boundary.
    /// `expected_methods` lists the method names that must appear.
    TraitPort { expected_methods: Vec<String> },
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
            Self::TraitPort { .. } => "trait_port",
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
/// Layer-neutral naming (T001, formerly `DomainTypeEntry`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeCatalogueEntry {
    name: String,
    description: String,
    kind: TypeDefinitionKind,
    action: TypeAction,
    approved: bool,
}

impl TypeCatalogueEntry {
    /// Creates a new `TypeCatalogueEntry`.
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
        Ok(Self { name, description: description.into(), kind, action, approved })
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
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
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
    fn test_type_catalogue_entry_trait_port_kind() {
        let kind = TypeDefinitionKind::TraitPort {
            expected_methods: vec!["find_by_id".into(), "save".into()],
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
        assert_eq!(entry.kind().kind_tag(), "trait_port");
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
}
