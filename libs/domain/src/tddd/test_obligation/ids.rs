//! Validated string newtypes and identity value objects for the test-obligation
//! domain model.
//!
//! Holds the foundational value objects the rules / errors / obligations modules
//! depend on. Two families live here:
//!
//! * Load-time diagnostic newtypes: [`RoleName`] (a role identifier used in
//!   load-time diagnostics) and [`DiagnosticMessage`] (a non-empty human-readable
//!   diagnostic string), both rejecting empty / whitespace-only input (IN-04 /
//!   CN-05).
//! * Obligation / binding identity value objects: [`TestObligationId`],
//!   [`TestObligationAnchorId`], [`TestObligationEdgeId`],
//!   [`TestObligationItemIdentifier`], [`TestObligationBrief`],
//!   [`TestModulePath`], [`TestFunctionName`], and [`WaivedReason`]. These carry
//!   the obligation / edge identity derived from catalogue entry keys plus spec /
//!   ADR anchors, kept as distinct types so identity fields cannot be swapped
//!   (IN-05 / IN-06 / CN-01).

use crate::ValidationError;
use crate::tddd::catalogue_v2::roles::ConstructionError;
use crate::tddd::semantic_verify::CatalogueEntryKey;
use crate::tddd::test_obligation::vocab::TestObligationKind;

/// A validated role name used in test-obligation rules diagnostics.
///
/// Wraps the raw role identifier that appears as a key in
/// `.harness/config/test-obligation-rules.json`. Kept as a distinct newtype so
/// load-time errors ([`crate::tddd::test_obligation::errors::TestObligationRulesLoadError`])
/// carry a role name rather than a bare `String`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoleName(String);

impl RoleName {
    /// Validate and wrap `value` as a [`RoleName`].
    ///
    /// The input is trimmed before the emptiness check, so whitespace-only
    /// strings are treated as empty.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `value` is empty or
    /// whitespace-only.
    pub fn try_new(value: String) -> Result<Self, ValidationError> {
        if value.trim().is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self(value))
    }

    /// Borrow the inner role name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated, non-empty diagnostic message.
///
/// Used by test-obligation errors and verdicts to carry human-readable detail
/// without leaking a bare `String` across port boundaries. Empty /
/// whitespace-only input is rejected so a diagnostic can never be silent.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiagnosticMessage(String);

impl DiagnosticMessage {
    /// Validate and wrap `value` as a [`DiagnosticMessage`].
    ///
    /// The input is trimmed before the emptiness check, so whitespace-only
    /// strings are treated as empty.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `value` is empty or
    /// whitespace-only.
    pub fn try_new(value: String) -> Result<Self, ValidationError> {
        if value.trim().is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self(value))
    }

    /// Borrow the inner diagnostic string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Returns a non-empty fallback detail for error paths that cannot retain their
/// original diagnostic text.
#[must_use]
pub fn unavailable_diagnostic_message() -> DiagnosticMessage {
    DiagnosticMessage("diagnostic detail unavailable".to_owned())
}

/// Validated non-empty item identifier component of a [`TestObligationId`].
///
/// Distinguishes the specific declaration facet (method name, invariant label,
/// etc.) an obligation targets within a catalogue entry (IN-05 / CN-01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationItemIdentifier {
    value: String,
}

impl TestObligationItemIdentifier {
    /// Validate and wrap `value` as a [`TestObligationItemIdentifier`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `value` is empty or
    /// whitespace-only.
    pub fn try_new(value: String) -> Result<Self, ValidationError> {
        if value.trim().is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self { value })
    }

    /// Borrow the inner identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// Stable identity of a single derived test obligation.
///
/// Derived purely from identity inputs — the catalogue entry key, the obligation
/// kind, and the item identifier — so the same obligation keeps the same id
/// across runs regardless of surrounding declaration detail (IN-05 / CN-01 /
/// AC-03, ADR D9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationId {
    entry_key: CatalogueEntryKey,
    obligation_kind: TestObligationKind,
    item_identifier: TestObligationItemIdentifier,
}

impl TestObligationId {
    /// Builds a [`TestObligationId`] from its identity components.
    #[must_use]
    pub fn new(
        entry_key: CatalogueEntryKey,
        obligation_kind: TestObligationKind,
        item_identifier: TestObligationItemIdentifier,
    ) -> Self {
        Self { entry_key, obligation_kind, item_identifier }
    }

    /// Returns the catalogue entry key this obligation targets.
    #[must_use]
    pub fn entry_key(&self) -> &CatalogueEntryKey {
        &self.entry_key
    }

    /// Returns the obligation kind.
    #[must_use]
    pub fn obligation_kind(&self) -> &TestObligationKind {
        &self.obligation_kind
    }

    /// Returns the item identifier component.
    #[must_use]
    pub fn item_identifier(&self) -> &TestObligationItemIdentifier {
        &self.item_identifier
    }
}

/// Identifier of a spec or ADR anchor a test obligation binds to.
///
/// Pairs the anchor's source file path with the element id (e.g. `IN-05`, `D9`).
/// Both components are opaque non-empty strings, validated only for emptiness —
/// path existence and anchor-format checks happen at their respective boundaries
/// (IN-06).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationAnchorId {
    file_path: String,
    element_id: String,
}

impl TestObligationAnchorId {
    /// Validate and wrap `file_path` / `element_id` as a
    /// [`TestObligationAnchorId`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when either component is empty or
    /// whitespace-only.
    pub fn try_new(file_path: String, element_id: String) -> Result<Self, ValidationError> {
        if file_path.trim().is_empty() || element_id.trim().is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self { file_path, element_id })
    }

    /// Borrow the anchor's source file path.
    #[must_use]
    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    /// Borrow the anchor's element id.
    #[must_use]
    pub fn element_id(&self) -> &str {
        &self.element_id
    }
}

/// Stable identity of a single obligation → anchor binding edge.
///
/// Derived from the catalogue entry key plus the [`TestObligationAnchorId`] it
/// resolves against, so a binding edge is addressable independently of the tests
/// bound to it (IN-06 / IN-08 / CN-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationEdgeId {
    entry_key: CatalogueEntryKey,
    anchor_id: TestObligationAnchorId,
}

impl TestObligationEdgeId {
    /// Builds a [`TestObligationEdgeId`] from its identity components.
    #[must_use]
    pub fn new(entry_key: CatalogueEntryKey, anchor_id: TestObligationAnchorId) -> Self {
        Self { entry_key, anchor_id }
    }

    /// Returns the catalogue entry key this edge originates from.
    #[must_use]
    pub fn entry_key(&self) -> &CatalogueEntryKey {
        &self.entry_key
    }

    /// Returns the anchor this edge resolves against.
    #[must_use]
    pub fn anchor_id(&self) -> &TestObligationAnchorId {
        &self.anchor_id
    }
}

/// Non-empty brief text describing what a derived obligation requires.
///
/// Expanded from a rule's brief template for the implementer; rejects empty /
/// whitespace-only input so an obligation is never presented without guidance
/// (IN-05 / CN-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationBrief {
    text: String,
}

impl TestObligationBrief {
    /// Validate and wrap `text` as a [`TestObligationBrief`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `text` is empty or
    /// whitespace-only.
    pub fn try_new(text: String) -> Result<Self, ValidationError> {
        if text.trim().is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self { text })
    }

    /// Borrow the inner brief text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

/// Validated non-empty module path (e.g. `foo::bar`) recorded in a test binding.
///
/// Stored as an opaque non-empty string — path-syntax validation happens where
/// the binding is scanned (IN-06, ADR D9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestModulePath {
    value: String,
}

impl TestModulePath {
    /// Validate and wrap `value` as a [`TestModulePath`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `value` is empty or
    /// whitespace-only.
    pub fn try_new(value: String) -> Result<Self, ValidationError> {
        if value.trim().is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self { value })
    }

    /// Borrow the inner module path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// Validated non-empty Rust test function name recorded in a test binding.
///
/// Stored as an opaque non-empty string — identifier-syntax validation happens
/// where the binding is scanned (IN-06, ADR D9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestFunctionName {
    name: String,
}

impl TestFunctionName {
    /// Validate and wrap `name` as a [`TestFunctionName`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `name` is empty or
    /// whitespace-only.
    pub fn try_new(name: String) -> Result<Self, ValidationError> {
        if name.trim().is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self { name })
    }

    /// Borrow the inner function name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.name
    }
}

/// Non-empty free-text reason recorded when an obligation edge is waived.
///
/// Carried by a test-bindings artifact to justify a deliberately unbound
/// obligation; empty / whitespace-only input is rejected so a waiver can never
/// be silent (IN-06 / AC-11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaivedReason {
    text: String,
}

impl WaivedReason {
    /// Validate and wrap `text` as a [`WaivedReason`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `text` is empty or
    /// whitespace-only.
    pub fn try_new(text: String) -> Result<Self, ValidationError> {
        if text.trim().is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self { text })
    }

    /// Borrow the inner waiver reason.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

/// Validated non-empty set of [`TestObligationEdgeId`] entries.
///
/// Carries the failing / escalation edge identities in
/// [`ObligationEvaluateError`](crate::tddd::test_obligation::errors::ObligationEvaluateError)
/// so a confirmed-failure / human-escalation error can never be raised with an
/// empty edge set. Mirrors the
/// [`NonEmptyTestLocations`](crate::tddd::test_obligation::binding::NonEmptyTestLocations)
/// pattern: [`try_new`](Self::try_new) rejects empty vectors via
/// [`ConstructionError::EmptyCollection`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyEdgeIds((TestObligationEdgeId, Vec<TestObligationEdgeId>));

impl NonEmptyEdgeIds {
    /// Builds a [`NonEmptyEdgeIds`] from a required first element and the
    /// remaining (possibly empty) elements; the invariant holds by construction.
    #[must_use]
    pub fn new(first: TestObligationEdgeId, rest: Vec<TestObligationEdgeId>) -> Self {
        let mut values = Vec::with_capacity(rest.len() + 1);
        values.push(first.clone());
        values.extend(rest);
        Self((first, values))
    }

    /// Validates and wraps `values`, rejecting an empty vector.
    ///
    /// # Errors
    ///
    /// Returns [`ConstructionError::EmptyCollection`] when `values` is empty.
    pub fn try_new(values: Vec<TestObligationEdgeId>) -> Result<Self, ConstructionError> {
        let Some(first) = values.first().cloned() else {
            return Err(ConstructionError::EmptyCollection);
        };
        Ok(Self((first, values)))
    }

    /// Returns the edge ids as a slice, guaranteed non-empty.
    #[must_use]
    pub fn as_slice(&self) -> &[TestObligationEdgeId] {
        self.0.1.as_slice()
    }

    /// Returns the first edge id, always present by invariant.
    #[must_use]
    pub fn first(&self) -> &TestObligationEdgeId {
        &self.0.0
    }

    /// Structural predicate backing the `non_empty` invariant declaration.
    #[must_use]
    pub fn is_non_empty(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_role_name_accepts_non_empty() {
        let name = RoleName::try_new("ValueObject".to_owned()).unwrap();
        assert_eq!(name.as_str(), "ValueObject");
    }

    #[test]
    fn test_role_name_rejects_empty() {
        assert_eq!(RoleName::try_new(String::new()), Err(ValidationError::EmptyString));
    }

    #[test]
    fn test_role_name_rejects_whitespace_only() {
        assert_eq!(RoleName::try_new("   ".to_owned()), Err(ValidationError::EmptyString));
    }

    #[test]
    fn test_diagnostic_message_accepts_non_empty() {
        let msg = DiagnosticMessage::try_new("boom".to_owned()).unwrap();
        assert_eq!(msg.as_str(), "boom");
    }

    #[test]
    fn test_unavailable_diagnostic_message_returns_bound_test_source_message() {
        assert_eq!(unavailable_diagnostic_message().as_str(), "diagnostic detail unavailable");
    }

    #[test]
    fn test_diagnostic_message_rejects_empty() {
        assert_eq!(DiagnosticMessage::try_new(String::new()), Err(ValidationError::EmptyString));
    }

    #[test]
    fn test_diagnostic_message_rejects_whitespace_only() {
        assert_eq!(
            DiagnosticMessage::try_new(" \n\t ".to_owned()),
            Err(ValidationError::EmptyString)
        );
    }

    fn entry_key(raw: &str) -> CatalogueEntryKey {
        CatalogueEntryKey::try_new(raw.to_owned()).unwrap()
    }

    #[test]
    fn test_item_identifier_accepts_non_empty() {
        let id = TestObligationItemIdentifier::try_new("method:find_by_email".to_owned()).unwrap();
        assert_eq!(id.as_str(), "method:find_by_email");
    }

    #[test]
    fn test_item_identifier_rejects_blank() {
        assert_eq!(
            TestObligationItemIdentifier::try_new("  ".to_owned()),
            Err(ValidationError::EmptyString)
        );
    }

    #[test]
    fn test_obligation_id_exposes_identity_components() {
        let item = TestObligationItemIdentifier::try_new("invariant:non_empty".to_owned()).unwrap();
        let id = TestObligationId::new(
            entry_key("domain::User"),
            TestObligationKind::Boundary,
            item.clone(),
        );
        assert_eq!(id.entry_key().as_str(), "domain::User");
        assert_eq!(id.obligation_kind(), &TestObligationKind::Boundary);
        assert_eq!(id.item_identifier(), &item);
    }

    #[test]
    fn test_anchor_id_accepts_non_empty_pair() {
        let anchor = TestObligationAnchorId::try_new(
            "track/items/x/spec.json".to_owned(),
            "IN-05".to_owned(),
        )
        .unwrap();
        assert_eq!(anchor.file_path(), "track/items/x/spec.json");
        assert_eq!(anchor.element_id(), "IN-05");
    }

    #[test]
    fn test_anchor_id_rejects_empty_file_path() {
        assert_eq!(
            TestObligationAnchorId::try_new(String::new(), "IN-05".to_owned()),
            Err(ValidationError::EmptyString)
        );
    }

    #[test]
    fn test_anchor_id_rejects_empty_element_id() {
        assert_eq!(
            TestObligationAnchorId::try_new("spec.json".to_owned(), "   ".to_owned()),
            Err(ValidationError::EmptyString)
        );
    }

    #[test]
    fn test_edge_id_exposes_identity_components() {
        let anchor =
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-06".to_owned()).unwrap();
        let edge = TestObligationEdgeId::new(entry_key("domain::User"), anchor.clone());
        assert_eq!(edge.entry_key().as_str(), "domain::User");
        assert_eq!(edge.anchor_id(), &anchor);
    }

    #[test]
    fn test_obligation_brief_rejects_blank() {
        assert_eq!(
            TestObligationBrief::try_new(" \t ".to_owned()),
            Err(ValidationError::EmptyString)
        );
    }

    #[test]
    fn test_obligation_brief_accepts_non_empty() {
        let brief =
            TestObligationBrief::try_new("cover the empty-input branch".to_owned()).unwrap();
        assert_eq!(brief.as_str(), "cover the empty-input branch");
    }

    #[test]
    fn test_module_path_round_trips() {
        let path = TestModulePath::try_new("domain::user::tests".to_owned()).unwrap();
        assert_eq!(path.as_str(), "domain::user::tests");
    }

    #[test]
    fn test_module_path_rejects_blank() {
        assert_eq!(TestModulePath::try_new(String::new()), Err(ValidationError::EmptyString));
    }

    #[test]
    fn test_function_name_round_trips() {
        let name = TestFunctionName::try_new("test_rejects_empty".to_owned()).unwrap();
        assert_eq!(name.as_str(), "test_rejects_empty");
    }

    #[test]
    fn test_function_name_rejects_blank() {
        assert_eq!(TestFunctionName::try_new("  ".to_owned()), Err(ValidationError::EmptyString));
    }

    #[test]
    fn test_waived_reason_round_trips() {
        let reason = WaivedReason::try_new("covered by integration suite".to_owned()).unwrap();
        assert_eq!(reason.as_str(), "covered by integration suite");
    }

    #[test]
    fn test_waived_reason_rejects_blank() {
        assert_eq!(WaivedReason::try_new(String::new()), Err(ValidationError::EmptyString));
    }

    fn sample_edge_id() -> TestObligationEdgeId {
        let anchor =
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-09".to_owned()).unwrap();
        TestObligationEdgeId::new(entry_key("domain::User"), anchor)
    }

    #[test]
    fn test_non_empty_edge_ids_new_exposes_entries() {
        let edge = sample_edge_id();
        let edges = NonEmptyEdgeIds::new(edge.clone(), vec![edge.clone()]);
        assert!(edges.is_non_empty());
        assert_eq!(edges.as_slice().len(), 2);
        assert_eq!(edges.first(), &edge);
    }

    #[test]
    fn test_non_empty_edge_ids_try_new_accepts_non_empty() {
        let edges = NonEmptyEdgeIds::try_new(vec![sample_edge_id()]).unwrap();
        assert_eq!(edges.as_slice().len(), 1);
    }

    #[test]
    fn test_non_empty_edge_ids_try_new_rejects_empty() {
        assert_eq!(NonEmptyEdgeIds::try_new(vec![]), Err(ConstructionError::EmptyCollection));
    }
}
