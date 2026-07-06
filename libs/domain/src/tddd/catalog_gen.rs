//! Draft-catalogue value objects for the `sotp catalog` generation surface.
//!
//! Holds the domain-level newtypes and closed vocabularies the
//! `catalog add / import / cite / check` commands operate over before a draft
//! catalogue is parsed into a completed typed DTO: validated non-empty text
//! ([`TodoInstruction`], [`DraftHolePath`], [`CatalogEntryName`]), the `$todo`
//! hole value object ([`DraftHole`]), and the closed vocabularies for import
//! actions and entry kinds ([`CatalogImportAction`], [`CatalogEntryKind`]).
//!
//! Design: ADR 2026-07-02-1345 (D2 / D3 / D4 / D6). See spec IN-03, IN-04,
//! IN-07, AC-03, AC-04, AC-05, AC-08.

use crate::ValidationError;

/// Trim `value` and reject empty / whitespace-only input, mirroring the domain
/// "non-empty string" convention (`NonEmptyString`).
///
/// # Errors
///
/// Returns [`ValidationError::EmptyString`] when the trimmed input is empty.
fn parse_non_empty(value: String) -> Result<String, ValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() { Err(ValidationError::EmptyString) } else { Ok(trimmed.to_owned()) }
}

/// A validated non-empty `$todo` fill-in instruction (trimmed, rejects
/// empty / whitespace-only).
///
/// See IN-07, CN-01.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoInstruction(String);

impl TodoInstruction {
    /// Validate and wrap `value` as a [`TodoInstruction`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when the trimmed input is empty.
    pub fn try_new(value: String) -> Result<Self, ValidationError> {
        parse_non_empty(value).map(Self)
    }

    /// Returns the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether the wrapped text is non-empty (the type invariant).
    #[must_use]
    pub fn is_non_empty(&self) -> bool {
        !self.0.is_empty()
    }
}

/// A validated non-empty draft-catalogue hole path (trimmed, rejects
/// empty / whitespace-only).
///
/// See IN-07, AC-05.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftHolePath(String);

impl DraftHolePath {
    /// Validate and wrap `value` as a [`DraftHolePath`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when the trimmed input is empty.
    pub fn try_new(value: String) -> Result<Self, ValidationError> {
        parse_non_empty(value).map(Self)
    }

    /// Returns the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether the wrapped text is non-empty (the type invariant).
    #[must_use]
    pub fn is_non_empty(&self) -> bool {
        !self.0.is_empty()
    }
}

/// A validated non-empty catalogue entry name (trimmed, rejects
/// empty / whitespace-only).
///
/// See IN-03, AC-08.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntryName(String);

impl CatalogEntryName {
    /// Validate and wrap `value` as a [`CatalogEntryName`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when the trimmed input is empty.
    pub fn try_new(value: String) -> Result<Self, ValidationError> {
        parse_non_empty(value).map(Self)
    }

    /// Returns the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether the wrapped text is non-empty (the type invariant).
    #[must_use]
    pub fn is_non_empty(&self) -> bool {
        !self.0.is_empty()
    }
}

/// A single `$todo` hole in a draft catalogue: the path locating it plus the
/// fill-in instruction to display.
///
/// See IN-07, AC-05.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftHole {
    path: DraftHolePath,
    instruction: TodoInstruction,
}

impl DraftHole {
    /// Constructs a [`DraftHole`] from its location `path` and fill-in
    /// `instruction`.
    #[must_use]
    pub fn new(path: DraftHolePath, instruction: TodoInstruction) -> Self {
        Self { path, instruction }
    }

    /// Returns the hole's location path.
    #[must_use]
    pub fn path(&self) -> &DraftHolePath {
        &self.path
    }

    /// Returns the hole's fill-in instruction.
    #[must_use]
    pub fn instruction(&self) -> &TodoInstruction {
        &self.instruction
    }
}

/// The three `sotp catalog import` actions.
///
/// See IN-04, AC-04.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogImportAction {
    /// Take in the existing type unchanged, by reference only.
    Reference,
    /// Take in the current shape as a baseline for later editing.
    Modify,
    /// Take in only the identity needed to record a removal.
    Delete,
}

/// The five catalogue entry kinds.
///
/// See IN-03, AC-03.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogEntryKind {
    /// A `struct` entry.
    Struct,
    /// An `enum` entry.
    Enum,
    /// A `type` alias entry.
    TypeAlias,
    /// A `trait` entry.
    Trait,
    /// A free function entry.
    Function,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_instruction_try_new_accepts_non_empty() {
        let todo = TodoInstruction::try_new("fill in the return type".to_owned()).unwrap();
        assert_eq!(todo.as_str(), "fill in the return type");
        assert!(todo.is_non_empty());
    }

    #[test]
    fn test_todo_instruction_try_new_rejects_empty() {
        assert!(matches!(
            TodoInstruction::try_new(String::new()).unwrap_err(),
            ValidationError::EmptyString
        ));
    }

    #[test]
    fn test_todo_instruction_try_new_rejects_whitespace_only() {
        assert!(matches!(
            TodoInstruction::try_new("   ".to_owned()).unwrap_err(),
            ValidationError::EmptyString
        ));
    }

    #[test]
    fn test_draft_hole_path_try_new_accepts_and_rejects() {
        let path = DraftHolePath::try_new("types.Foo.methods[0]".to_owned()).unwrap();
        assert_eq!(path.as_str(), "types.Foo.methods[0]");
        assert!(path.is_non_empty());
        assert!(DraftHolePath::try_new(String::new()).is_err());
    }

    #[test]
    fn test_catalog_entry_name_try_new_accepts_and_rejects() {
        let name = CatalogEntryName::try_new("CatalogInteractor".to_owned()).unwrap();
        assert_eq!(name.as_str(), "CatalogInteractor");
        assert!(name.is_non_empty());
        assert!(CatalogEntryName::try_new("  ".to_owned()).is_err());
    }

    #[test]
    fn test_draft_hole_new_roundtrips_fields() {
        let path = DraftHolePath::try_new("types.Foo".to_owned()).unwrap();
        let instruction = TodoInstruction::try_new("describe the shape".to_owned()).unwrap();
        let hole = DraftHole::new(path.clone(), instruction.clone());
        assert_eq!(hole.path(), &path);
        assert_eq!(hole.instruction(), &instruction);
    }

    #[test]
    fn test_catalog_import_action_construction() {
        assert_ne!(CatalogImportAction::Reference, CatalogImportAction::Modify);
        assert_eq!(CatalogImportAction::Delete, CatalogImportAction::Delete);
    }

    #[test]
    fn test_catalog_entry_kind_construction() {
        let kinds = [
            CatalogEntryKind::Struct,
            CatalogEntryKind::Enum,
            CatalogEntryKind::TypeAlias,
            CatalogEntryKind::Trait,
            CatalogEntryKind::Function,
        ];
        assert_eq!(kinds.len(), 5);
        assert_ne!(CatalogEntryKind::Struct, CatalogEntryKind::Enum);
    }
}
