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

fn yellow(name: &str, kind_tag: &str) -> DomainTypeSignal {
    DomainTypeSignal::new(name, kind_tag, ConfidenceSignal::Yellow, false, vec![], vec![], vec![])
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
        return yellow(name, kind_tag);
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
        return yellow(name, kind_tag);
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
        return yellow(name, kind_tag);
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
        return yellow(name, kind_tag);
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
        return yellow(name, kind_tag);
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
// undeclared_to_signals — reverse check Red signal conversion
// ---------------------------------------------------------------------------

/// Converts undeclared type and trait names into Red `DomainTypeSignal`s.
///
/// - Undeclared types get `kind_tag = "undeclared_type"`
/// - Undeclared traits get `kind_tag = "undeclared_trait"`
/// - All signals are `ConfidenceSignal::Red` with `found_type = true`
///   (they exist in code but not in domain-types.json).
///
/// # Errors
///
/// This function is infallible.
#[must_use]
pub fn undeclared_to_signals(
    undeclared_types: &[String],
    undeclared_traits: &[String],
) -> Vec<DomainTypeSignal> {
    let mut signals = Vec::with_capacity(undeclared_types.len() + undeclared_traits.len());

    for name in undeclared_types {
        signals.push(DomainTypeSignal::new(
            name.clone(),
            "undeclared_type",
            ConfidenceSignal::Red,
            true,
            vec![],
            vec![],
            vec![],
        ));
    }

    for name in undeclared_traits {
        signals.push(DomainTypeSignal::new(
            name.clone(),
            "undeclared_trait",
            ConfidenceSignal::Red,
            true,
            vec![],
            vec![],
            vec![],
        ));
    }

    signals
}

// ---------------------------------------------------------------------------
// ConsistencyReport — bidirectional spec ↔ code check
// ---------------------------------------------------------------------------

/// Result of a bidirectional consistency check between domain-types.json (spec)
/// and the crate's public API (code), with baseline-aware filtering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsistencyReport {
    /// Forward signals: spec → code evaluation results (groups 1 + 2).
    forward_signals: Vec<DomainTypeSignal>,
    /// Types found in code but not in declarations or baseline (group 4).
    undeclared_types: Vec<String>,
    /// Traits found in code but not in declarations or baseline (group 4).
    undeclared_traits: Vec<String>,
    /// Count of baseline types/traits skipped because structure is unchanged (group 3).
    skipped_count: usize,
    /// Red signals from baseline comparison: structural changes or deletions (group 3).
    baseline_red_types: Vec<String>,
    /// Red signals from baseline comparison for traits (group 3).
    baseline_red_traits: Vec<String>,
}

impl ConsistencyReport {
    /// Returns the forward (spec → code) signals.
    #[must_use]
    pub fn forward_signals(&self) -> &[DomainTypeSignal] {
        &self.forward_signals
    }

    /// Returns type names found in code but not in declarations or baseline (group 4).
    #[must_use]
    pub fn undeclared_types(&self) -> &[String] {
        &self.undeclared_types
    }

    /// Returns trait names found in code but not in declarations or baseline (group 4).
    #[must_use]
    pub fn undeclared_traits(&self) -> &[String] {
        &self.undeclared_traits
    }

    /// Returns the count of baseline entries skipped (structure unchanged, group 3).
    #[must_use]
    pub fn skipped_count(&self) -> usize {
        self.skipped_count
    }

    /// Returns type names from baseline with structural changes or deletions (group 3 Red).
    #[must_use]
    pub fn baseline_red_types(&self) -> &[String] {
        &self.baseline_red_types
    }

    /// Returns trait names from baseline with structural changes or deletions (group 3 Red).
    #[must_use]
    pub fn baseline_red_traits(&self) -> &[String] {
        &self.baseline_red_traits
    }
}

/// Performs a baseline-aware bidirectional consistency check.
///
/// Uses the 4-group evaluation from ADR TDDD-02 §3:
/// - **Group 1 (A\B)**: declared, not in baseline → forward check
/// - **Group 2 (A∩B)**: declared and in baseline → forward check
/// - **Group 3 (B\A)**: baseline, not declared → skip if unchanged, Red if changed/deleted
/// - **Group 4 (∁(A∪B)∩C)**: not declared, not in baseline, in code → Red
///
/// Groups 1+2 are handled by `evaluate_domain_type_signals` (forward check).
/// Groups 3+4 replace the old undeclared-types reverse check.
#[must_use]
pub fn check_consistency(
    entries: &[DomainTypeEntry],
    graph: &TypeGraph,
    baseline: &crate::TypeBaseline,
) -> ConsistencyReport {
    // Forward check (groups 1 + 2): evaluate declared entries against code.
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

    let mut skipped_count: usize = 0;
    let mut baseline_red_types: Vec<String> = Vec::new();
    let mut baseline_red_traits: Vec<String> = Vec::new();

    // Group 3 — types: B\A (in baseline, not declared)
    for (name, baseline_entry) in baseline.types() {
        if declared_type_names.contains(name.as_str()) {
            continue; // Group 2: declared → handled by forward check
        }
        match graph.get_type(name) {
            Some(code_node) => {
                // Compare baseline entry against current code structure.
                let current = crate::TypeBaselineEntry::new(
                    code_node.kind().clone(),
                    code_node.members().to_vec(),
                    code_node.method_return_types().iter().cloned().collect(),
                );
                if baseline_entry.structurally_equal(&current) {
                    skipped_count += 1; // Unchanged → skip
                } else {
                    baseline_red_types.push(name.clone()); // Structural change → Red
                }
            }
            None => {
                baseline_red_types.push(name.clone()); // Deleted → Red
            }
        }
    }

    // Group 3 — traits: B\A (in baseline, not declared)
    for (name, baseline_entry) in baseline.traits() {
        if declared_trait_names.contains(name.as_str()) {
            continue; // Group 2: declared → handled by forward check
        }
        match graph.get_trait(name) {
            Some(code_node) => {
                let current = crate::TraitBaselineEntry::new(code_node.method_names().to_vec());
                if baseline_entry.structurally_equal(&current) {
                    skipped_count += 1;
                } else {
                    baseline_red_traits.push(name.clone());
                }
            }
            None => {
                baseline_red_traits.push(name.clone());
            }
        }
    }

    baseline_red_types.sort();
    baseline_red_traits.sort();

    // Group 4 — ∁(A∪B)∩C: in code, not declared, not in baseline → Red
    let mut undeclared_types: Vec<String> = graph
        .type_names()
        .filter(|name| !declared_type_names.contains(name.as_str()) && !baseline.has_type(name))
        .cloned()
        .collect();
    undeclared_types.sort();

    let mut undeclared_traits: Vec<String> = graph
        .trait_names()
        .filter(|name| !declared_trait_names.contains(name.as_str()) && !baseline.has_trait(name))
        .cloned()
        .collect();
    undeclared_traits.sort();

    ConsistencyReport {
        forward_signals,
        undeclared_types,
        undeclared_traits,
        skipped_count,
        baseline_red_types,
        baseline_red_traits,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
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
    fn test_evaluate_typestate_yellow_when_type_not_implemented() {
        let entry = DomainTypeEntry::new(
            "Ghost",
            "desc",
            DomainTypeKind::Typestate { transitions: TypestateTransitions::Terminal },
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_domain_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
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
    fn test_evaluate_value_object_yellow_when_not_implemented() {
        let entry =
            DomainTypeEntry::new("TrackId", "desc", DomainTypeKind::ValueObject, true).unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_domain_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
        assert!(!results.first().unwrap().found_type());
    }

    #[test]
    fn test_evaluate_enum_yellow_when_not_implemented() {
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
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
        assert!(!results.first().unwrap().found_type());
    }

    #[test]
    fn test_evaluate_error_type_yellow_when_not_implemented() {
        let entry = DomainTypeEntry::new(
            "DomainError",
            "desc",
            DomainTypeKind::ErrorType { expected_variants: vec!["NotFound".into()] },
            true,
        )
        .unwrap();
        // Profile has no "DomainError" type — declared in spec, not yet implemented.
        let profile = make_profile(&[]);
        let results = evaluate_domain_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
        assert!(!results.first().unwrap().found_type());
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
    fn test_evaluate_trait_port_yellow_when_not_implemented() {
        let entry = DomainTypeEntry::new(
            "Repo",
            "desc",
            DomainTypeKind::TraitPort { expected_methods: vec!["save".into()] },
            true,
        )
        .unwrap();
        let profile = make_profile(&[]);
        let results = evaluate_domain_type_signals(&[entry], &profile);
        assert_eq!(results.first().unwrap().signal(), ConfidenceSignal::Yellow);
        assert!(!results.first().unwrap().found_type());
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

    // --- undeclared_to_signals tests ---

    #[test]
    fn test_undeclared_to_signals_converts_types_to_red() {
        let undeclared = vec!["Foo".to_string(), "Bar".to_string()];
        let signals = undeclared_to_signals(&undeclared, &[]);

        assert_eq!(signals.len(), 2);
        assert_eq!(signals[0].type_name(), "Foo");
        assert_eq!(signals[0].kind_tag(), "undeclared_type");
        assert_eq!(signals[0].signal(), ConfidenceSignal::Red);
        assert!(signals[0].found_type());
        assert!(signals[0].missing_items().is_empty());
        assert!(signals[0].extra_items().is_empty());

        assert_eq!(signals[1].type_name(), "Bar");
        assert_eq!(signals[1].kind_tag(), "undeclared_type");
        assert_eq!(signals[1].signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_undeclared_to_signals_converts_traits_to_red() {
        let undeclared_traits = vec!["MyTrait".to_string()];
        let signals = undeclared_to_signals(&[], &undeclared_traits);

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].type_name(), "MyTrait");
        assert_eq!(signals[0].kind_tag(), "undeclared_trait");
        assert_eq!(signals[0].signal(), ConfidenceSignal::Red);
        assert!(signals[0].found_type());
    }

    #[test]
    fn test_undeclared_to_signals_empty_inputs_returns_empty() {
        let signals = undeclared_to_signals(&[], &[]);
        assert!(signals.is_empty());
    }

    #[test]
    fn test_undeclared_to_signals_mixed_types_and_traits() {
        let types = vec!["Foo".to_string()];
        let traits = vec!["Bar".to_string()];
        let signals = undeclared_to_signals(&types, &traits);

        assert_eq!(signals.len(), 2);
        assert_eq!(signals[0].kind_tag(), "undeclared_type");
        assert_eq!(signals[1].kind_tag(), "undeclared_trait");
    }

    // --- check_consistency tests (4-group baseline-aware) ---

    use crate::Timestamp;
    use crate::tddd::baseline::{TraitBaselineEntry, TypeBaseline, TypeBaselineEntry};

    fn empty_baseline() -> TypeBaseline {
        TypeBaseline::new(
            1,
            Timestamp::new("2026-04-11T00:00:00Z").unwrap(),
            HashMap::new(),
            HashMap::new(),
        )
    }

    fn baseline_with_types(entries: Vec<(&str, TypeBaselineEntry)>) -> TypeBaseline {
        let types = entries.into_iter().map(|(n, e)| (n.to_string(), e)).collect();
        TypeBaseline::new(1, Timestamp::new("2026-04-11T00:00:00Z").unwrap(), types, HashMap::new())
    }

    fn baseline_with_traits(entries: Vec<(&str, TraitBaselineEntry)>) -> TypeBaseline {
        let traits = entries.into_iter().map(|(n, e)| (n.to_string(), e)).collect();
        TypeBaseline::new(
            1,
            Timestamp::new("2026-04-11T00:00:00Z").unwrap(),
            HashMap::new(),
            traits,
        )
    }

    #[test]
    fn test_group4_undeclared_new_type_is_red() {
        // Type in code, not declared, not in baseline → group 4 Red
        let mut types = HashMap::new();
        types.insert(
            "NewType".to_string(),
            TypeNode::new(TypeKind::Struct, vec![], HashSet::new(), HashSet::new()),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let report = check_consistency(&[], &graph, &empty_baseline());
        assert_eq!(report.undeclared_types(), &["NewType"]);
        assert_eq!(report.skipped_count(), 0);
    }

    #[test]
    fn test_group3_baseline_unchanged_type_is_skipped() {
        // Type in baseline and code, not declared, structure unchanged → skip
        let bl = baseline_with_types(vec![(
            "ExistingType",
            TypeBaselineEntry::new(TypeKind::Struct, vec!["field".into()], vec![]),
        )]);

        let mut types = HashMap::new();
        types.insert(
            "ExistingType".to_string(),
            TypeNode::new(TypeKind::Struct, vec!["field".into()], HashSet::new(), HashSet::new()),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let report = check_consistency(&[], &graph, &bl);
        assert_eq!(report.skipped_count(), 1);
        assert!(report.undeclared_types().is_empty());
        assert!(report.baseline_red_types().is_empty());
    }

    #[test]
    fn test_group3_baseline_changed_type_is_red() {
        // Type in baseline and code, not declared, structure changed → Red
        let bl = baseline_with_types(vec![(
            "ChangedType",
            TypeBaselineEntry::new(TypeKind::Enum, vec!["A".into()], vec![]),
        )]);

        let mut types = HashMap::new();
        types.insert(
            "ChangedType".to_string(),
            TypeNode::new(
                TypeKind::Enum,
                vec!["A".into(), "B".into()], // new variant added
                HashSet::new(),
                HashSet::new(),
            ),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let report = check_consistency(&[], &graph, &bl);
        assert_eq!(report.baseline_red_types(), &["ChangedType"]);
        assert_eq!(report.skipped_count(), 0);
    }

    #[test]
    fn test_group3_baseline_deleted_type_is_red() {
        // Type in baseline but not in code, not declared → Red (deletion)
        let bl = baseline_with_types(vec![(
            "DeletedType",
            TypeBaselineEntry::new(TypeKind::Struct, vec![], vec![]),
        )]);

        let graph = TypeGraph::new(HashMap::new(), HashMap::new());

        let report = check_consistency(&[], &graph, &bl);
        assert_eq!(report.baseline_red_types(), &["DeletedType"]);
        assert_eq!(report.skipped_count(), 0);
    }

    #[test]
    fn test_group2_declared_baseline_type_uses_forward_check() {
        // Type in both baseline and declarations → forward check (group 2)
        let bl = baseline_with_types(vec![(
            "TrackId",
            TypeBaselineEntry::new(TypeKind::Struct, vec!["0".into()], vec![]),
        )]);

        let entry =
            DomainTypeEntry::new("TrackId", "desc", DomainTypeKind::ValueObject, true).unwrap();

        let mut types = HashMap::new();
        types.insert(
            "TrackId".to_string(),
            TypeNode::new(TypeKind::Struct, vec!["0".into()], HashSet::new(), HashSet::new()),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let report = check_consistency(&[entry], &graph, &bl);
        assert_eq!(report.forward_signals().len(), 1);
        assert_eq!(report.forward_signals()[0].signal(), ConfidenceSignal::Blue);
        // Not counted as skipped (it's declared → forward check handles it)
        assert_eq!(report.skipped_count(), 0);
        assert!(report.baseline_red_types().is_empty());
    }

    #[test]
    fn test_group1_new_declared_type_uses_forward_check() {
        // Declared but not in baseline → group 1, forward check
        let entry =
            DomainTypeEntry::new("NewType", "desc", DomainTypeKind::ValueObject, true).unwrap();

        let mut types = HashMap::new();
        types.insert(
            "NewType".to_string(),
            TypeNode::new(TypeKind::Struct, vec![], HashSet::new(), HashSet::new()),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let report = check_consistency(&[entry], &graph, &empty_baseline());
        assert_eq!(report.forward_signals().len(), 1);
        assert_eq!(report.forward_signals()[0].signal(), ConfidenceSignal::Blue);
        assert!(report.undeclared_types().is_empty());
    }

    #[test]
    fn test_group3_baseline_unchanged_trait_is_skipped() {
        let bl = baseline_with_traits(vec![(
            "MyTrait",
            TraitBaselineEntry::new(vec!["method_a".into()]),
        )]);

        let mut traits = HashMap::new();
        traits.insert("MyTrait".to_string(), TraitNode::new(vec!["method_a".into()]));
        let graph = TypeGraph::new(HashMap::new(), traits);

        let report = check_consistency(&[], &graph, &bl);
        assert_eq!(report.skipped_count(), 1);
        assert!(report.baseline_red_traits().is_empty());
    }

    #[test]
    fn test_group3_baseline_changed_trait_is_red() {
        let bl = baseline_with_traits(vec![(
            "MyTrait",
            TraitBaselineEntry::new(vec!["method_a".into()]),
        )]);

        let mut traits = HashMap::new();
        traits.insert(
            "MyTrait".to_string(),
            TraitNode::new(vec!["method_a".into(), "method_b".into()]),
        );
        let graph = TypeGraph::new(HashMap::new(), traits);

        let report = check_consistency(&[], &graph, &bl);
        assert_eq!(report.baseline_red_traits(), &["MyTrait"]);
        assert_eq!(report.skipped_count(), 0);
    }

    #[test]
    fn test_mixed_groups_comprehensive() {
        // Set up a scenario with all 4 groups:
        // - "DeclaredNew" (group 1): declared, not in baseline
        // - "DeclaredExisting" (group 2): declared, in baseline
        // - "UnchangedExisting" (group 3 skip): in baseline, unchanged
        // - "ChangedExisting" (group 3 red): in baseline, changed
        // - "BrandNew" (group 4): not declared, not in baseline
        let bl = baseline_with_types(vec![
            ("DeclaredExisting", TypeBaselineEntry::new(TypeKind::Struct, vec![], vec![])),
            (
                "UnchangedExisting",
                TypeBaselineEntry::new(TypeKind::Struct, vec!["x".into()], vec![]),
            ),
            ("ChangedExisting", TypeBaselineEntry::new(TypeKind::Enum, vec!["A".into()], vec![])),
        ]);

        let entries = vec![
            DomainTypeEntry::new("DeclaredNew", "d", DomainTypeKind::ValueObject, true).unwrap(),
            DomainTypeEntry::new("DeclaredExisting", "d", DomainTypeKind::ValueObject, true)
                .unwrap(),
        ];

        let mut types = HashMap::new();
        for name in
            &["DeclaredNew", "DeclaredExisting", "UnchangedExisting", "ChangedExisting", "BrandNew"]
        {
            let (kind, members) = if *name == "ChangedExisting" {
                (TypeKind::Enum, vec!["A".into(), "B".into()]) // changed
            } else if *name == "UnchangedExisting" {
                (TypeKind::Struct, vec!["x".into()])
            } else {
                (TypeKind::Struct, vec![])
            };
            types.insert(
                name.to_string(),
                TypeNode::new(kind, members, HashSet::new(), HashSet::new()),
            );
        }
        let graph = TypeGraph::new(types, HashMap::new());

        let report = check_consistency(&entries, &graph, &bl);

        // Groups 1+2: 2 forward signals
        assert_eq!(report.forward_signals().len(), 2);
        // Group 3 skip: UnchangedExisting
        assert_eq!(report.skipped_count(), 1);
        // Group 3 red: ChangedExisting
        assert_eq!(report.baseline_red_types(), &["ChangedExisting"]);
        // Group 4: BrandNew
        assert_eq!(report.undeclared_types(), &["BrandNew"]);
    }
}
