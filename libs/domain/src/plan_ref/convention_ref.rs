//! `ConventionRef` and its anchor newtype.

use std::fmt;
use std::path::PathBuf;

use crate::ValidationError;

/// Validated newtype for a convention in-document anchor. Symmetric to
/// [`super::AdrAnchor`]: loose validation (non-empty) only; strict semantic
/// validation is deferred to ADR 2026-04-19-1242 §Q15.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConventionAnchor(String);

impl ConventionAnchor {
    /// Validate and wrap `value` as a [`ConventionAnchor`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyConventionAnchor`] when `value` is
    /// empty or contains only whitespace.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            Err(ValidationError::EmptyConventionAnchor)
        } else {
            Ok(Self(value))
        }
    }
}

impl AsRef<str> for ConventionAnchor {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConventionAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Structured reference to a section within a convention document.
///
/// Used both in per-element `convention_refs[]` on spec.json and in the
/// top-level `related_conventions[]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConventionRef {
    pub file: PathBuf,
    pub anchor: ConventionAnchor,
}

impl ConventionRef {
    pub fn new(file: impl Into<PathBuf>, anchor: ConventionAnchor) -> Self {
        Self { file: file.into(), anchor }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn convention_anchor_accepts_non_empty() {
        let a = ConventionAnchor::try_new("newtype-pattern").unwrap();
        assert_eq!(a.as_ref(), "newtype-pattern");
    }

    #[test]
    fn convention_anchor_rejects_empty() {
        let err = ConventionAnchor::try_new("").unwrap_err();
        assert!(matches!(err, ValidationError::EmptyConventionAnchor));
    }

    #[test]
    fn convention_anchor_rejects_whitespace_only() {
        let err = ConventionAnchor::try_new("\t\n").unwrap_err();
        assert!(matches!(err, ValidationError::EmptyConventionAnchor));
    }

    #[test]
    fn convention_ref_constructs_with_path_and_anchor() {
        let a = ConventionAnchor::try_new("newtype").unwrap();
        let r = ConventionRef::new(".claude/rules/04-coding-principles.md", a.clone());
        assert_eq!(r.file, PathBuf::from(".claude/rules/04-coding-principles.md"));
        assert_eq!(r.anchor, a);
    }
}
