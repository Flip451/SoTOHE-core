//! Catalogue linter — S3 linter framework foundation types.
//!
//! Defines the rule vocabulary, violation value object, and `CatalogueLinter`
//! secondary-port trait for the S3 catalogue linter described in ADR
//! `knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md`
//! §S3 / IN-05 / AC-05.
//!
//! ## Design overview
//!
//! - `CatalogueLinterRuleKind` — enum of rule categories
//! - `CatalogueLinterRule` — value object representing one rule; constructed
//!   via `try_new`, which rejects ill-formed combinations
//! - `CatalogueLinterRuleError` — error type for `try_new` rejections
//! - `CatalogueLintViolation` — value object produced when a rule fires
//! - `CatalogueLinterError` — error type for linter execution failures
//! - `CatalogueLinter` — secondary-port trait; implementation lives in the
//!   infrastructure layer (T005 / T006)
//!
//! No `serde` derives are attached here — ADR
//! `knowledge/adr/2026-04-14-1531-…` forbids serde inside `libs/domain`;
//! codec / serde support lives in the infrastructure codec (T005).

use crate::tddd::catalogue::TypeCatalogueDocument;

// ---------------------------------------------------------------------------
// CatalogueLinterRuleKind — rule category enum
// ---------------------------------------------------------------------------

/// Classifies what invariant a catalogue linter rule asserts.
///
/// Per IN-05 / AC-05 of the S3 specification:
///
/// - `FieldEmpty` and `FieldNonEmpty` apply to struct-based kind entries and
///   check whether a specified field (`target_field`) must be empty or
///   non-empty for a given `target_kind`.
/// - `KindLayerConstraint` restricts which layers a given `target_kind` may
///   appear in. The caller (interactor) injects the current `layer_id` so
///   that the linter can compare it against `permitted_layers`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogueLinterRuleKind {
    /// Rule asserts that the named field must be empty for entries of the
    /// target kind.
    FieldEmpty,
    /// Rule asserts that the named field must be non-empty for entries of
    /// the target kind.
    FieldNonEmpty,
    /// Rule constrains which layers entries of the target kind may appear in.
    KindLayerConstraint,
}

// ---------------------------------------------------------------------------
// CatalogueLinterRuleError — error type for try_new rejections
// ---------------------------------------------------------------------------

/// Errors returned by [`CatalogueLinterRule::try_new`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CatalogueLinterRuleError {
    /// `target_kind` is an empty string.
    #[error("target_kind must not be empty")]
    EmptyTargetKind,
    /// `target_field` is empty for a `FieldEmpty` or `FieldNonEmpty` rule.
    #[error("target_field must not be empty for FieldEmpty/FieldNonEmpty rules")]
    EmptyTargetField,
    /// `permitted_layers` is empty for a `KindLayerConstraint` rule.
    #[error("permitted_layers must not be empty for KindLayerConstraint rules")]
    EmptyPermittedLayers,
}

// ---------------------------------------------------------------------------
// CatalogueLinterRule — value object
// ---------------------------------------------------------------------------

/// A single catalogue linter rule.
///
/// Constructed via [`CatalogueLinterRule::try_new`], which rejects
/// ill-formed combinations:
///
/// - `target_kind` must not be empty (applies to all rule kinds)
/// - For `FieldEmpty` / `FieldNonEmpty`: `target_field` must be `Some("…")`
///   with a non-empty string
/// - For `KindLayerConstraint`: `permitted_layers` must be non-empty
///
/// All fields are private; read access is via accessor methods following the
/// existing domain layer style (`catalogue.rs`, `catalogue_ports.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueLinterRule {
    rule_kind: CatalogueLinterRuleKind,
    /// Kind tag string, e.g. `"value_object"`, `"domain_service"`.
    target_kind: String,
    /// Field name, e.g. `"expected_methods"`. Only set for `FieldEmpty` /
    /// `FieldNonEmpty`; always `None` for `KindLayerConstraint`.
    target_field: Option<String>,
    /// Layer ids where the target kind is permitted. Only populated for
    /// `KindLayerConstraint`; always empty for `FieldEmpty` / `FieldNonEmpty`.
    permitted_layers: Vec<String>,
}

impl CatalogueLinterRule {
    /// Creates a new `CatalogueLinterRule`.
    ///
    /// # Errors
    ///
    /// Returns `CatalogueLinterRuleError::EmptyTargetKind` if `target_kind`
    /// is empty.
    /// Returns `CatalogueLinterRuleError::EmptyTargetField` if `rule_kind` is
    /// `FieldEmpty` or `FieldNonEmpty` and `target_field` is `None` or empty.
    /// Returns `CatalogueLinterRuleError::EmptyPermittedLayers` if `rule_kind`
    /// is `KindLayerConstraint` and `permitted_layers` is empty.
    pub fn try_new(
        rule_kind: CatalogueLinterRuleKind,
        target_kind: impl Into<String>,
        target_field: Option<String>,
        permitted_layers: Vec<String>,
    ) -> Result<Self, CatalogueLinterRuleError> {
        let target_kind = target_kind.into();
        if target_kind.is_empty() {
            return Err(CatalogueLinterRuleError::EmptyTargetKind);
        }
        match rule_kind {
            CatalogueLinterRuleKind::FieldEmpty | CatalogueLinterRuleKind::FieldNonEmpty => {
                match &target_field {
                    None => return Err(CatalogueLinterRuleError::EmptyTargetField),
                    Some(f) if f.is_empty() => {
                        return Err(CatalogueLinterRuleError::EmptyTargetField);
                    }
                    _ => {}
                }
            }
            CatalogueLinterRuleKind::KindLayerConstraint => {
                if permitted_layers.is_empty() {
                    return Err(CatalogueLinterRuleError::EmptyPermittedLayers);
                }
            }
        }
        Ok(Self { rule_kind, target_kind, target_field, permitted_layers })
    }

    /// Returns the rule kind.
    #[must_use]
    pub fn rule_kind(&self) -> &CatalogueLinterRuleKind {
        &self.rule_kind
    }

    /// Returns the target kind tag (e.g. `"value_object"`).
    #[must_use]
    pub fn target_kind(&self) -> &str {
        &self.target_kind
    }

    /// Returns the target field name, or `None` for `KindLayerConstraint` rules.
    #[must_use]
    pub fn target_field(&self) -> Option<&str> {
        self.target_field.as_deref()
    }

    /// Returns the permitted layer ids for `KindLayerConstraint` rules.
    /// Always empty for `FieldEmpty` / `FieldNonEmpty` rules.
    #[must_use]
    pub fn permitted_layers(&self) -> &[String] {
        &self.permitted_layers
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
    rule_kind: CatalogueLinterRuleKind,
    entry_name: String,
    message: String,
}

impl CatalogueLintViolation {
    /// Creates a new `CatalogueLintViolation`.
    ///
    /// All three parameters are required; no validation is performed because
    /// violations are constructed only by a trusted linter implementation.
    #[must_use]
    pub fn new(
        rule_kind: CatalogueLinterRuleKind,
        entry_name: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self { rule_kind, entry_name: entry_name.into(), message: message.into() }
    }

    /// Returns the kind of rule that generated this violation.
    #[must_use]
    pub fn rule_kind(&self) -> &CatalogueLinterRuleKind {
        &self.rule_kind
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
// CatalogueLinterError — error type for linter execution failures
// ---------------------------------------------------------------------------

/// Errors returned by [`CatalogueLinter::run`].
#[derive(Debug, thiserror::Error)]
pub enum CatalogueLinterError {
    /// The linter rule configuration is invalid and prevents execution.
    #[error("invalid linter rule configuration: {0}")]
    InvalidRuleConfig(String),
}

// ---------------------------------------------------------------------------
// CatalogueLinter — secondary-port trait
// ---------------------------------------------------------------------------

/// Secondary port for running catalogue lint rules against a layer's type
/// catalogue.
///
/// `layer_id` is injected by the caller (interactor) rather than embedded in
/// the rules, so that the same rule set can be reused across layers and the
/// `KindLayerConstraint` primitive only checks the layer in which the linter
/// is invoked.
///
/// Implementations live in the infrastructure layer (T005 / T006).
pub trait CatalogueLinter: Send + Sync {
    /// Run `rules` against `catalogue` for the given `layer_id`.
    ///
    /// Returns the full list of violations found. An empty `Vec` means no
    /// rules fired.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogueLinterError::InvalidRuleConfig`] if the provided
    /// rule configuration is internally inconsistent and prevents execution.
    fn run(
        &self,
        rules: &[CatalogueLinterRule],
        catalogue: &TypeCatalogueDocument,
        layer_id: &str,
    ) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // CatalogueLinterRuleKind — exhaustive variant existence test
    // ------------------------------------------------------------------

    #[test]
    fn test_catalogue_linter_rule_kind_variants_exist() {
        // Construct each variant to verify all three exist and are distinct.
        let field_empty = CatalogueLinterRuleKind::FieldEmpty;
        let field_non_empty = CatalogueLinterRuleKind::FieldNonEmpty;
        let kind_layer = CatalogueLinterRuleKind::KindLayerConstraint;
        assert_ne!(field_empty, field_non_empty);
        assert_ne!(field_non_empty, kind_layer);
        assert_ne!(field_empty, kind_layer);
    }

    // ------------------------------------------------------------------
    // CatalogueLinterRuleError — exhaustive variant existence test
    // ------------------------------------------------------------------

    #[test]
    fn test_catalogue_linter_rule_error_variants_exist() {
        let e1 = CatalogueLinterRuleError::EmptyTargetKind;
        let e2 = CatalogueLinterRuleError::EmptyTargetField;
        let e3 = CatalogueLinterRuleError::EmptyPermittedLayers;
        assert_ne!(e1, e2);
        assert_ne!(e2, e3);
        assert_ne!(e1, e3);
    }

    // ------------------------------------------------------------------
    // CatalogueLinterRule::try_new — happy path for each rule_kind
    // ------------------------------------------------------------------

    #[test]
    fn test_linter_rule_try_new_field_empty_succeeds_with_valid_inputs() {
        let rule = CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::FieldEmpty,
            "value_object",
            Some("expected_methods".to_owned()),
            vec![],
        )
        .unwrap();
        assert_eq!(rule.rule_kind(), &CatalogueLinterRuleKind::FieldEmpty);
        assert_eq!(rule.target_kind(), "value_object");
        assert_eq!(rule.target_field(), Some("expected_methods"));
        assert!(rule.permitted_layers().is_empty());
    }

    #[test]
    fn test_linter_rule_try_new_field_non_empty_succeeds_with_valid_inputs() {
        let rule = CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::FieldNonEmpty,
            "secondary_port",
            Some("expected_methods".to_owned()),
            vec![],
        )
        .unwrap();
        assert_eq!(rule.rule_kind(), &CatalogueLinterRuleKind::FieldNonEmpty);
        assert_eq!(rule.target_kind(), "secondary_port");
        assert_eq!(rule.target_field(), Some("expected_methods"));
        assert!(rule.permitted_layers().is_empty());
    }

    #[test]
    fn test_linter_rule_try_new_kind_layer_constraint_succeeds_with_valid_inputs() {
        let rule = CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::KindLayerConstraint,
            "domain_service",
            None,
            vec!["domain".to_owned(), "usecase".to_owned()],
        )
        .unwrap();
        assert_eq!(rule.rule_kind(), &CatalogueLinterRuleKind::KindLayerConstraint);
        assert_eq!(rule.target_kind(), "domain_service");
        assert!(rule.target_field().is_none());
        assert_eq!(rule.permitted_layers(), &["domain", "usecase"]);
    }

    // ------------------------------------------------------------------
    // CatalogueLinterRule::try_new — rejection cases
    // ------------------------------------------------------------------

    #[test]
    fn test_linter_rule_try_new_rejects_empty_target_kind_for_field_empty() {
        let result = CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::FieldEmpty,
            "",
            Some("expected_methods".to_owned()),
            vec![],
        );
        assert_eq!(result, Err(CatalogueLinterRuleError::EmptyTargetKind));
    }

    #[test]
    fn test_linter_rule_try_new_rejects_empty_target_kind_for_field_non_empty() {
        let result = CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::FieldNonEmpty,
            "",
            Some("expected_methods".to_owned()),
            vec![],
        );
        assert_eq!(result, Err(CatalogueLinterRuleError::EmptyTargetKind));
    }

    #[test]
    fn test_linter_rule_try_new_rejects_empty_target_kind_for_kind_layer_constraint() {
        let result = CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::KindLayerConstraint,
            "",
            None,
            vec!["domain".to_owned()],
        );
        assert_eq!(result, Err(CatalogueLinterRuleError::EmptyTargetKind));
    }

    #[test]
    fn test_linter_rule_try_new_field_empty_rejects_none_target_field() {
        let result = CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::FieldEmpty,
            "value_object",
            None,
            vec![],
        );
        assert_eq!(result, Err(CatalogueLinterRuleError::EmptyTargetField));
    }

    #[test]
    fn test_linter_rule_try_new_field_empty_rejects_empty_string_target_field() {
        let result = CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::FieldEmpty,
            "value_object",
            Some(String::new()),
            vec![],
        );
        assert_eq!(result, Err(CatalogueLinterRuleError::EmptyTargetField));
    }

    #[test]
    fn test_linter_rule_try_new_field_non_empty_rejects_none_target_field() {
        let result = CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::FieldNonEmpty,
            "secondary_port",
            None,
            vec![],
        );
        assert_eq!(result, Err(CatalogueLinterRuleError::EmptyTargetField));
    }

    #[test]
    fn test_linter_rule_try_new_field_non_empty_rejects_empty_string_target_field() {
        let result = CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::FieldNonEmpty,
            "secondary_port",
            Some(String::new()),
            vec![],
        );
        assert_eq!(result, Err(CatalogueLinterRuleError::EmptyTargetField));
    }

    #[test]
    fn test_linter_rule_try_new_kind_layer_constraint_rejects_empty_permitted_layers() {
        let result = CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::KindLayerConstraint,
            "domain_service",
            None,
            vec![],
        );
        assert_eq!(result, Err(CatalogueLinterRuleError::EmptyPermittedLayers));
    }

    // ------------------------------------------------------------------
    // CatalogueLintViolation — constructor + accessor methods
    // ------------------------------------------------------------------

    #[test]
    fn test_catalogue_lint_violation_constructor_and_accessors() {
        let violation = CatalogueLintViolation::new(
            CatalogueLinterRuleKind::FieldEmpty,
            "MyValueObject",
            "expected_methods must be empty for value_object entries",
        );
        assert_eq!(violation.rule_kind(), &CatalogueLinterRuleKind::FieldEmpty);
        assert_eq!(violation.entry_name(), "MyValueObject");
        assert_eq!(violation.message(), "expected_methods must be empty for value_object entries");
    }

    #[test]
    fn test_catalogue_lint_violation_with_kind_layer_constraint() {
        let violation = CatalogueLintViolation::new(
            CatalogueLinterRuleKind::KindLayerConstraint,
            "PaymentService",
            "domain_service is not permitted in the infrastructure layer",
        );
        assert_eq!(violation.rule_kind(), &CatalogueLinterRuleKind::KindLayerConstraint);
        assert_eq!(violation.entry_name(), "PaymentService");
        assert!(violation.message().contains("infrastructure"));
    }

    // ------------------------------------------------------------------
    // CatalogueLinterError::InvalidRuleConfig — constructor test
    // ------------------------------------------------------------------

    #[test]
    fn test_catalogue_linter_error_invalid_rule_config_stores_message() {
        let err = CatalogueLinterError::InvalidRuleConfig("contradictory rule set".to_owned());
        assert!(err.to_string().contains("contradictory rule set"));
    }
}
