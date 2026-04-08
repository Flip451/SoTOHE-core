//! Domain types for domain-types.json SSoT.
//!
//! `DomainTypesDocument` is the aggregate root for the catalogue of domain types
//! that a track's specification declares.  `domain-types.json` is the SSoT;
//! `domain-types.md` is a read-only rendered view.

use crate::ConfidenceSignal;
use crate::spec::SpecValidationError;

// ---------------------------------------------------------------------------
// DomainTypeKind enum
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

/// Classifies a domain type by its structural role in the codebase.
///
/// Each variant carries the expected items that the type should expose so that
/// an automated scanner can compute a `DomainTypeSignal` for the entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainTypeKind {
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
    /// A `pub trait` that defines a domain port (hexagonal architecture boundary).
    /// `expected_methods` lists the method names that must appear.
    TraitPort { expected_methods: Vec<String> },
}

impl DomainTypeKind {
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
// DomainTypeEntry
// ---------------------------------------------------------------------------

/// A single entry in the domain-types catalogue.
///
/// Each entry records one named domain type together with its expected structure
/// (`kind`) and whether the entry has been human-approved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainTypeEntry {
    name: String,
    description: String,
    kind: DomainTypeKind,
    approved: bool,
}

impl DomainTypeEntry {
    /// Creates a new `DomainTypeEntry`.
    ///
    /// # Errors
    ///
    /// Returns `SpecValidationError::EmptyDomainStateName` if `name` is empty or
    /// whitespace-only.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        kind: DomainTypeKind,
        approved: bool,
    ) -> Result<Self, SpecValidationError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(SpecValidationError::EmptyDomainStateName);
        }
        Ok(Self { name, description: description.into(), kind, approved })
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
    pub fn kind(&self) -> &DomainTypeKind {
        &self.kind
    }

    /// Returns `true` if this entry has been explicitly approved by a maintainer.
    #[must_use]
    pub fn approved(&self) -> bool {
        self.approved
    }
}

// ---------------------------------------------------------------------------
// DomainTypeSignal
// ---------------------------------------------------------------------------

/// Per-type signal evaluation result produced by comparing a `DomainTypeEntry`
/// against scanned code output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainTypeSignal {
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

impl DomainTypeSignal {
    /// Creates a new `DomainTypeSignal`.
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
// DomainTypesDocument
// ---------------------------------------------------------------------------

/// Aggregate root for the domain-types catalogue (`domain-types.json`).
///
/// The document records the full set of declared domain types together with
/// their optional scan signals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainTypesDocument {
    schema_version: u32,
    entries: Vec<DomainTypeEntry>,
    signals: Option<Vec<DomainTypeSignal>>,
}

impl DomainTypesDocument {
    /// Creates a new `DomainTypesDocument` with no signals.
    #[must_use]
    pub fn new(schema_version: u32, entries: Vec<DomainTypeEntry>) -> Self {
        Self { schema_version, entries, signals: None }
    }

    /// Returns the schema version of this document.
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the domain type entries in this document.
    #[must_use]
    pub fn entries(&self) -> &[DomainTypeEntry] {
        &self.entries
    }

    /// Returns the scan signals, if they have been populated.
    #[must_use]
    pub fn signals(&self) -> Option<&[DomainTypeSignal]> {
        self.signals.as_deref()
    }

    /// Replaces the signals with a new set derived from a code scan.
    pub fn set_signals(&mut self, signals: Vec<DomainTypeSignal>) {
        self.signals = Some(signals);
    }
}

// ---------------------------------------------------------------------------
// Signal evaluation
// ---------------------------------------------------------------------------

use std::collections::HashSet;

use crate::schema::{TypeGraph, TypeKind};

/// Evaluates domain type signals by comparing spec entries against a pre-indexed `TypeGraph`.
///
/// Only types declared as `Typestate` in entries are considered valid transition targets.
///
/// Signal rules: Blue = spec and code fully match. Red = everything else.
#[must_use]
pub fn evaluate_domain_type_signals(
    entries: &[DomainTypeEntry],
    profile: &TypeGraph,
) -> Vec<DomainTypeSignal> {
    // Collect names of typestate-declared types — only these count as valid transition targets.
    let typestate_names: HashSet<&str> = entries
        .iter()
        .filter(|e| matches!(e.kind(), DomainTypeKind::Typestate { .. }))
        .map(|e| e.name())
        .collect();
    entries.iter().map(|entry| evaluate_single(entry, profile, &typestate_names)).collect()
}

fn evaluate_single(
    entry: &DomainTypeEntry,
    profile: &TypeGraph,
    typestate_names: &HashSet<&str>,
) -> DomainTypeSignal {
    let name = entry.name();
    let kind_tag = entry.kind().kind_tag().to_string();

    match entry.kind() {
        DomainTypeKind::Typestate { transitions } => {
            evaluate_typestate(name, &kind_tag, transitions, profile, typestate_names)
        }
        DomainTypeKind::Enum { expected_variants } => {
            evaluate_enum(name, &kind_tag, expected_variants, profile)
        }
        DomainTypeKind::ValueObject => evaluate_value_object(name, &kind_tag, profile),
        DomainTypeKind::ErrorType { expected_variants } => {
            evaluate_error_type(name, &kind_tag, expected_variants, profile)
        }
        DomainTypeKind::TraitPort { expected_methods } => {
            evaluate_trait_port(name, &kind_tag, expected_methods, profile)
        }
    }
}

fn red(name: &str, kind_tag: &str, found_type: bool) -> DomainTypeSignal {
    DomainTypeSignal::new(name, kind_tag, ConfidenceSignal::Red, found_type, vec![], vec![], vec![])
}

fn blue(name: &str, kind_tag: &str) -> DomainTypeSignal {
    DomainTypeSignal::new(name, kind_tag, ConfidenceSignal::Blue, true, vec![], vec![], vec![])
}

fn evaluate_typestate(
    name: &str,
    kind_tag: &str,
    transitions: &TypestateTransitions,
    profile: &TypeGraph,
    _typestate_names: &HashSet<&str>,
) -> DomainTypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        return red(name, kind_tag, false);
    };

    // Use pre-filtered outgoing transitions from TypeGraph (set by build_type_graph).
    // Self-transitions are excluded during construction.
    let code_transitions: HashSet<&str> =
        code_type.outgoing().iter().filter(|t| t.as_str() != name).map(|s| s.as_str()).collect();

    match transitions {
        TypestateTransitions::Terminal => {
            if code_transitions.is_empty() {
                blue(name, kind_tag)
            } else {
                let mut extra: Vec<String> =
                    code_transitions.into_iter().map(|s| s.to_string()).collect();
                extra.sort();
                DomainTypeSignal::new(
                    name,
                    kind_tag,
                    ConfidenceSignal::Red,
                    true,
                    vec![],
                    vec![],
                    extra,
                )
            }
        }
        TypestateTransitions::To(targets) => {
            let declared: HashSet<&str> = targets.iter().map(|s| s.as_str()).collect();

            let mut found = Vec::new();
            let mut missing = Vec::new();
            for target in targets {
                if code_transitions.contains(target.as_str()) {
                    found.push(target.clone());
                } else {
                    missing.push(target.clone());
                }
            }

            // Detect undeclared transitions (code has them, spec doesn't).
            let mut extra: Vec<String> = code_transitions
                .iter()
                .filter(|ct| !declared.contains(**ct))
                .map(|s| s.to_string())
                .collect();
            extra.sort();

            let signal = if missing.is_empty() && extra.is_empty() {
                ConfidenceSignal::Blue
            } else {
                ConfidenceSignal::Red
            };
            DomainTypeSignal::new(name, kind_tag, signal, true, found, missing, extra)
        }
    }
}

fn evaluate_enum(
    name: &str,
    kind_tag: &str,
    expected_variants: &[String],
    profile: &TypeGraph,
) -> DomainTypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        return DomainTypeSignal::new(
            name,
            kind_tag,
            ConfidenceSignal::Red,
            false,
            vec![],
            expected_variants.to_vec(),
            vec![],
        );
    };
    if *code_type.kind() != TypeKind::Enum {
        return DomainTypeSignal::new(
            name,
            kind_tag,
            ConfidenceSignal::Red,
            true,
            vec![],
            expected_variants.to_vec(),
            vec![],
        );
    }

    let code_variants: HashSet<&str> = code_type.members().iter().map(|s| s.as_str()).collect();
    let spec_variants: HashSet<&str> = expected_variants.iter().map(|s| s.as_str()).collect();

    let mut missing: Vec<String> =
        spec_variants.difference(&code_variants).map(|s| s.to_string()).collect();
    let mut extra: Vec<String> =
        code_variants.difference(&spec_variants).map(|s| s.to_string()).collect();
    let mut found: Vec<String> =
        spec_variants.intersection(&code_variants).map(|s| s.to_string()).collect();
    missing.sort();
    extra.sort();
    found.sort();

    let signal = if missing.is_empty() && extra.is_empty() {
        ConfidenceSignal::Blue
    } else {
        ConfidenceSignal::Red
    };

    DomainTypeSignal::new(name, kind_tag, signal, true, found, missing, extra)
}

fn evaluate_value_object(name: &str, kind_tag: &str, profile: &TypeGraph) -> DomainTypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        return red(name, kind_tag, false);
    };
    // ValueObject must be a Struct (not Enum or TypeAlias).
    if *code_type.kind() == TypeKind::Struct {
        blue(name, kind_tag)
    } else {
        red(name, kind_tag, true)
    }
}

fn evaluate_error_type(
    name: &str,
    kind_tag: &str,
    expected_variants: &[String],
    profile: &TypeGraph,
) -> DomainTypeSignal {
    let Some(code_type) = profile.get_type(name) else {
        return DomainTypeSignal::new(
            name,
            kind_tag,
            ConfidenceSignal::Red,
            false,
            vec![],
            expected_variants.to_vec(),
            vec![],
        );
    };
    if *code_type.kind() != TypeKind::Enum {
        return DomainTypeSignal::new(
            name,
            kind_tag,
            ConfidenceSignal::Red,
            true,
            vec![],
            expected_variants.to_vec(),
            vec![],
        );
    }

    // Empty expected_variants with enum confirmation = Blue (existence-only).
    if expected_variants.is_empty() {
        return blue(name, kind_tag);
    }

    let code_variants: HashSet<&str> = code_type.members().iter().map(|s| s.as_str()).collect();

    let mut found = Vec::new();
    let mut missing = Vec::new();
    for v in expected_variants {
        if code_variants.contains(v.as_str()) {
            found.push(v.clone());
        } else {
            missing.push(v.clone());
        }
    }

    let signal = if missing.is_empty() { ConfidenceSignal::Blue } else { ConfidenceSignal::Red };
    DomainTypeSignal::new(name, kind_tag, signal, true, found, missing, vec![])
}

fn evaluate_trait_port(
    name: &str,
    kind_tag: &str,
    expected_methods: &[String],
    profile: &TypeGraph,
) -> DomainTypeSignal {
    let Some(code_trait) = profile.get_trait(name) else {
        return DomainTypeSignal::new(
            name,
            kind_tag,
            ConfidenceSignal::Red,
            false,
            vec![],
            expected_methods.to_vec(),
            vec![],
        );
    };

    let code_methods: HashSet<&str> =
        code_trait.method_names().iter().map(|s| s.as_str()).collect();

    let mut found = Vec::new();
    let mut missing = Vec::new();
    for m in expected_methods {
        if code_methods.contains(m.as_str()) {
            found.push(m.clone());
        } else {
            missing.push(m.clone());
        }
    }

    let signal = if missing.is_empty() { ConfidenceSignal::Blue } else { ConfidenceSignal::Red };
    DomainTypeSignal::new(name, kind_tag, signal, true, found, missing, vec![])
}

// ---------------------------------------------------------------------------
// ConsistencyReport — bidirectional spec ↔ code check
// ---------------------------------------------------------------------------

/// Result of a bidirectional consistency check between domain-types.json (spec)
/// and the crate's public API (code).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsistencyReport {
    /// Forward signals: spec → code evaluation results.
    forward_signals: Vec<DomainTypeSignal>,
    /// Types found in code but not declared in domain-types.json.
    undeclared_types: Vec<String>,
    /// Traits found in code but not declared in domain-types.json.
    undeclared_traits: Vec<String>,
}

impl ConsistencyReport {
    /// Returns the forward (spec → code) signals.
    #[must_use]
    pub fn forward_signals(&self) -> &[DomainTypeSignal] {
        &self.forward_signals
    }

    /// Returns type names found in code but not declared in the spec.
    #[must_use]
    pub fn undeclared_types(&self) -> &[String] {
        &self.undeclared_types
    }

    /// Returns trait names found in code but not declared in the spec.
    #[must_use]
    pub fn undeclared_traits(&self) -> &[String] {
        &self.undeclared_traits
    }
}

/// Performs a bidirectional consistency check between domain-types entries and a TypeGraph.
///
/// - Forward: evaluates each entry against the TypeGraph (existing `evaluate_domain_type_signals`).
/// - Reverse: finds types and traits in the TypeGraph not declared in entries.
#[must_use]
pub fn check_consistency(entries: &[DomainTypeEntry], graph: &TypeGraph) -> ConsistencyReport {
    let forward_signals = evaluate_domain_type_signals(entries, graph);

    let declared_type_names: HashSet<&str> = entries
        .iter()
        .filter(|e| !matches!(e.kind(), DomainTypeKind::TraitPort { .. }))
        .map(|e| e.name())
        .collect();

    let declared_trait_names: HashSet<&str> = entries
        .iter()
        .filter(|e| matches!(e.kind(), DomainTypeKind::TraitPort { .. }))
        .map(|e| e.name())
        .collect();

    let mut undeclared_types: Vec<String> = graph
        .type_names()
        .filter(|name| !declared_type_names.contains(name.as_str()))
        .cloned()
        .collect();
    undeclared_types.sort();

    let mut undeclared_traits: Vec<String> = graph
        .trait_names()
        .filter(|name| !declared_trait_names.contains(name.as_str()))
        .cloned()
        .collect();
    undeclared_traits.sort();

    ConsistencyReport { forward_signals, undeclared_types, undeclared_traits }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn typestate_entry() -> DomainTypeEntry {
        DomainTypeEntry::new(
            "ReviewState",
            "Typestate for review flow",
            DomainTypeKind::Typestate {
                transitions: TypestateTransitions::To(vec!["Approved".into(), "Rejected".into()]),
            },
            true,
        )
        .unwrap()
    }

    #[test]
    fn test_domain_type_entry_with_valid_name_succeeds() {
        let entry = typestate_entry();
        assert_eq!(entry.name(), "ReviewState");
        assert_eq!(entry.description(), "Typestate for review flow");
        assert!(entry.approved());
        assert_eq!(entry.kind().kind_tag(), "typestate");
    }

    #[test]
    fn test_domain_type_entry_with_empty_name_returns_error() {
        let result = DomainTypeEntry::new("", "desc", DomainTypeKind::ValueObject, true);
        assert!(matches!(result, Err(SpecValidationError::EmptyDomainStateName)));
    }

    #[test]
    fn test_domain_type_entry_with_whitespace_name_returns_error() {
        let result = DomainTypeEntry::new("   ", "desc", DomainTypeKind::ValueObject, true);
        assert!(matches!(result, Err(SpecValidationError::EmptyDomainStateName)));
    }

    #[test]
    fn test_domain_type_entry_value_object_kind() {
        let entry = DomainTypeEntry::new(
            "Email",
            "Validated email address",
            DomainTypeKind::ValueObject,
            true,
        )
        .unwrap();
        assert_eq!(entry.kind(), &DomainTypeKind::ValueObject);
        assert_eq!(entry.kind().kind_tag(), "value_object");
    }

    #[test]
    fn test_domain_type_entry_enum_kind_with_variants() {
        let kind =
            DomainTypeKind::Enum { expected_variants: vec!["Active".into(), "Inactive".into()] };
        let entry =
            DomainTypeEntry::new("Status", "Track status enum", kind.clone(), true).unwrap();
        assert_eq!(entry.kind(), &kind);
        assert_eq!(entry.kind().kind_tag(), "enum");
    }

    #[test]
    fn test_domain_type_entry_error_type_kind() {
        let kind = DomainTypeKind::ErrorType {
            expected_variants: vec!["NotFound".into(), "InvalidInput".into()],
        };
        let entry =
            DomainTypeEntry::new("DomainError", "Domain error type", kind.clone(), true).unwrap();
        assert_eq!(entry.kind(), &kind);
        assert_eq!(entry.kind().kind_tag(), "error_type");
    }

    #[test]
    fn test_domain_type_entry_trait_port_kind() {
        let kind = DomainTypeKind::TraitPort {
            expected_methods: vec!["find_by_id".into(), "save".into()],
        };
        let entry =
            DomainTypeEntry::new("UserRepository", "User repo port", kind.clone(), true).unwrap();
        assert_eq!(entry.kind(), &kind);
        assert_eq!(entry.kind().kind_tag(), "trait_port");
    }

    #[test]
    fn test_domain_type_entry_approved_default_true() {
        let entry = DomainTypeEntry::new("Foo", "desc", DomainTypeKind::ValueObject, true).unwrap();
        assert!(entry.approved());
    }

    #[test]
    fn test_domain_type_entry_approved_false_for_ai_added() {
        let entry = DomainTypeEntry::new(
            "AiSuggested",
            "AI-added type",
            DomainTypeKind::ValueObject,
            false,
        )
        .unwrap();
        assert!(!entry.approved());
    }

    #[test]
    fn test_domain_types_document_creation() {
        let entries = vec![typestate_entry()];
        let doc = DomainTypesDocument::new(1, entries.clone());
        assert_eq!(doc.schema_version(), 1);
        assert_eq!(doc.entries(), entries.as_slice());
        assert!(doc.signals().is_none());
    }

    #[test]
    fn test_domain_types_document_set_signals() {
        let mut doc = DomainTypesDocument::new(1, vec![typestate_entry()]);
        let signal = DomainTypeSignal::new(
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
    fn test_domain_type_signal_accessors() {
        let signal = DomainTypeSignal::new(
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

    // --- T003: evaluate_domain_type_signals ---

    use std::collections::{HashMap, HashSet};

    use crate::schema::{TraitNode, TypeGraph, TypeKind, TypeNode};

    /// Build a `TypeGraph` with struct-kinded types only (no members, no return types).
    fn make_profile(type_names: &[&str]) -> TypeGraph {
        let mut types = HashMap::new();
        for name in type_names {
            types.insert(
                name.to_string(),
                TypeNode::new(TypeKind::Struct, vec![], HashSet::new(), HashSet::new()),
            );
        }
        TypeGraph::new(types, HashMap::new())
    }

    /// Build a `TypeGraph` with a single enum type and given variants.
    fn make_profile_with_enum(name: &str, variants: &[&str]) -> TypeGraph {
        let mut types = HashMap::new();
        types.insert(
            name.to_string(),
            TypeNode::new(
                TypeKind::Enum,
                variants.iter().map(|v| v.to_string()).collect(),
                HashSet::new(),
                HashSet::new(),
            ),
        );
        TypeGraph::new(types, HashMap::new())
    }

    /// Build a `TypeGraph` where `from_type` has a method returning `to_type`.
    fn make_profile_with_transition(from_type: &str, to_type: &str) -> TypeGraph {
        let mut types = HashMap::new();
        let return_types: HashSet<String> = [to_type.to_string()].into();
        let outgoing: HashSet<String> = [to_type.to_string()].into();
        let from_node = TypeNode::new(TypeKind::Struct, vec![], return_types, outgoing);
        types.insert(from_type.to_string(), from_node);
        types.insert(
            to_type.to_string(),
            TypeNode::new(TypeKind::Struct, vec![], HashSet::new(), HashSet::new()),
        );
        TypeGraph::new(types, HashMap::new())
    }

    /// Build a `TypeGraph` with a trait and given method names.
    fn make_profile_with_trait(trait_name: &str, methods: &[&str]) -> TypeGraph {
        let mut traits = HashMap::new();
        traits.insert(
            trait_name.to_string(),
            TraitNode::new(methods.iter().map(|m| m.to_string()).collect()),
        );
        TypeGraph::new(HashMap::new(), traits)
    }

    #[test]
    fn test_evaluate_typestate_blue_when_all_transitions_found() {
        let draft = DomainTypeEntry::new(
            "Draft",
            "desc",
            DomainTypeKind::Typestate {
                transitions: TypestateTransitions::To(vec!["Published".into()]),
            },
            true,
        )
        .unwrap();
        let published = DomainTypeEntry::new(
            "Published",
            "desc",
            DomainTypeKind::Typestate { transitions: TypestateTransitions::Terminal },
            true,
        )
        .unwrap();
        let profile = make_profile_with_transition("Draft", "Published");
        let results = evaluate_domain_type_signals(&[draft, published], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_typestate_red_when_type_missing() {
        let entry = DomainTypeEntry::new(
            "Ghost",
            "desc",
            DomainTypeKind::Typestate { transitions: TypestateTransitions::Terminal },
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_domain_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Red);
        assert!(!results.first().unwrap().found_type());
    }

    #[test]
    fn test_evaluate_typestate_red_when_transition_missing() {
        let entry = DomainTypeEntry::new(
            "Draft",
            "desc",
            DomainTypeKind::Typestate {
                transitions: TypestateTransitions::To(vec!["Published".into()]),
            },
            true,
        )
        .unwrap();
        // Type exists but no method returning Published.
        let profile = make_profile(&["Draft"]);
        let results = evaluate_domain_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Red);
        assert_eq!(results.first().unwrap().missing_items(), &["Published"]);
    }

    #[test]
    fn test_evaluate_value_object_blue_when_exists() {
        let entry =
            DomainTypeEntry::new("TrackId", "desc", DomainTypeKind::ValueObject, true).unwrap();
        let profile = make_profile(&["TrackId"]);
        let results = evaluate_domain_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_value_object_red_when_missing() {
        let entry =
            DomainTypeEntry::new("TrackId", "desc", DomainTypeKind::ValueObject, true).unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_domain_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_evaluate_enum_red_when_not_found_in_profile() {
        let entry = DomainTypeEntry::new(
            "Status",
            "desc",
            DomainTypeKind::Enum { expected_variants: vec!["Active".into()] },
            true,
        )
        .unwrap();
        // Profile has no "Status" type.
        let profile = make_profile(&[]);
        let results = evaluate_domain_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_evaluate_enum_blue_when_variants_match() {
        let entry = DomainTypeEntry::new(
            "Status",
            "desc",
            DomainTypeKind::Enum { expected_variants: vec!["Active".into(), "Done".into()] },
            true,
        )
        .unwrap();
        let profile = make_profile_with_enum("Status", &["Active", "Done"]);
        let results = evaluate_domain_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_trait_port_red_when_not_in_profile() {
        let entry = DomainTypeEntry::new(
            "Repo",
            "desc",
            DomainTypeKind::TraitPort { expected_methods: vec!["save".into()] },
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_domain_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_evaluate_trait_port_blue_when_methods_match() {
        let entry = DomainTypeEntry::new(
            "Repo",
            "desc",
            DomainTypeKind::TraitPort { expected_methods: vec!["save".into(), "find".into()] },
            true,
        )
        .unwrap();
        let profile = make_profile_with_trait("Repo", &["save", "find"]);
        let results = evaluate_domain_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_typestate_blue_empty_transitions() {
        // Typestate with Terminal transitions = terminal state.
        let entry = DomainTypeEntry::new(
            "Final",
            "desc",
            DomainTypeKind::Typestate { transitions: TypestateTransitions::Terminal },
            true,
        )
        .unwrap();
        let profile = make_profile(&["Final"]);
        let results = evaluate_domain_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_typestate_uses_outgoing_not_method_return_types() {
        // "Draft" has method_return_types = {"Published", "NonTypestate"},
        // but outgoing = {"Published"} only (NonTypestate was filtered out by build_type_graph).
        // Evaluation must use outgoing — not method_return_types — so "NonTypestate" must not
        // appear in extra_items even though it is in method_return_types.
        let draft_entry = DomainTypeEntry::new(
            "Draft",
            "desc",
            DomainTypeKind::Typestate {
                transitions: TypestateTransitions::To(vec!["Published".into()]),
            },
            true,
        )
        .unwrap();
        let published_entry = DomainTypeEntry::new(
            "Published",
            "desc",
            DomainTypeKind::Typestate { transitions: TypestateTransitions::Terminal },
            true,
        )
        .unwrap();

        // Construct a TypeGraph where method_return_types has a non-typestate extra entry.
        let mut types = HashMap::new();
        let method_return_types: HashSet<String> =
            ["Published".to_string(), "NonTypestate".to_string()].into();
        // outgoing only contains the typestate target — NonTypestate is intentionally absent.
        let outgoing: HashSet<String> = ["Published".to_string()].into();
        let from_node = TypeNode::new(TypeKind::Struct, vec![], method_return_types, outgoing);
        types.insert("Draft".to_string(), from_node);
        types.insert(
            "Published".to_string(),
            TypeNode::new(TypeKind::Struct, vec![], HashSet::new(), HashSet::new()),
        );
        let profile = TypeGraph::new(types, HashMap::new());

        let results = evaluate_domain_type_signals(&[draft_entry, published_entry], &profile);
        let draft_signal = results.first().unwrap();
        // Blue: outgoing matches the declared transition exactly.
        assert_eq!(draft_signal.signal(), ConfidenceSignal::Blue);
        // NonTypestate must not appear in extra_items — evaluation must not read method_return_types.
        assert!(
            draft_signal.extra_items().is_empty(),
            "expected no extra_items, got {:?}",
            draft_signal.extra_items()
        );
    }

    // --- check_consistency tests ---

    #[test]
    fn test_check_consistency_reports_undeclared_types_and_traits() {
        // Entries declare only "TrackId" (value_object).
        let entry =
            DomainTypeEntry::new("TrackId", "desc", DomainTypeKind::ValueObject, true).unwrap();

        // TypeGraph has "TrackId" + "UndeclaredStruct", and trait "UndeclaredTrait".
        let mut types = HashMap::new();
        types.insert(
            "TrackId".to_string(),
            TypeNode::new(TypeKind::Struct, vec![], HashSet::new(), HashSet::new()),
        );
        types.insert(
            "UndeclaredStruct".to_string(),
            TypeNode::new(TypeKind::Struct, vec![], HashSet::new(), HashSet::new()),
        );
        let mut traits = HashMap::new();
        traits.insert("UndeclaredTrait".to_string(), TraitNode::new(vec!["method".into()]));
        let graph = TypeGraph::new(types, traits);

        let report = check_consistency(&[entry], &graph);

        // Forward: TrackId should be Blue.
        assert_eq!(report.forward_signals().len(), 1);
        assert_eq!(report.forward_signals().first().unwrap().signal(), ConfidenceSignal::Blue);

        // Reverse: UndeclaredStruct in types, UndeclaredTrait in traits.
        assert_eq!(report.undeclared_types(), &["UndeclaredStruct"]);
        assert_eq!(report.undeclared_traits(), &["UndeclaredTrait"]);
    }

    #[test]
    fn test_check_consistency_empty_undeclared_when_fully_covered() {
        let entry =
            DomainTypeEntry::new("TrackId", "desc", DomainTypeKind::ValueObject, true).unwrap();
        let trait_entry = DomainTypeEntry::new(
            "Repo",
            "desc",
            DomainTypeKind::TraitPort { expected_methods: vec!["save".into()] },
            true,
        )
        .unwrap();

        let mut types = HashMap::new();
        types.insert(
            "TrackId".to_string(),
            TypeNode::new(TypeKind::Struct, vec![], HashSet::new(), HashSet::new()),
        );
        let mut traits = HashMap::new();
        traits.insert("Repo".to_string(), TraitNode::new(vec!["save".into()]));
        let graph = TypeGraph::new(types, traits);

        let report = check_consistency(&[entry, trait_entry], &graph);
        assert!(report.undeclared_types().is_empty());
        assert!(report.undeclared_traits().is_empty());
    }
}
