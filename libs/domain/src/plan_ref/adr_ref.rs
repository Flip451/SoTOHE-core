//! `AdrRef` and its anchor newtype.

use std::fmt;
use std::path::PathBuf;

use crate::ValidationError;

/// Validated newtype for an ADR in-document anchor (heading slug or section
/// marker). Loose validation per ADR 2026-04-19-1242 §D2.1: the constructor
/// only enforces non-empty. Strict semantic validation (heading slug vs
/// HTML marker) is deferred to Q15.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AdrAnchor(String);

impl AdrAnchor {
    /// Validate and wrap `value` as an [`AdrAnchor`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyAdrAnchor`] when `value` is empty or
    /// contains only whitespace.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if value.trim().is_empty() { Err(ValidationError::EmptyAdrAnchor) } else { Ok(Self(value)) }
    }
}

impl AsRef<str> for AdrAnchor {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AdrAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Structured reference to a section within an ADR document.
///
/// The containing field name (e.g. `adr_refs` on a spec element) identifies
/// the reference direction. `AdrRef` itself is direction-agnostic.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdrRef {
    pub file: PathBuf,
    pub anchor: AdrAnchor,
}

impl AdrRef {
    pub fn new(file: impl Into<PathBuf>, anchor: AdrAnchor) -> Self {
        Self { file: file.into(), anchor }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn adr_anchor_accepts_non_empty() {
        let anchor = AdrAnchor::try_new("D2.1").unwrap();
        assert_eq!(anchor.as_ref(), "D2.1");
        assert_eq!(anchor.to_string(), "D2.1");
    }

    #[test]
    fn adr_anchor_accepts_heading_slug() {
        let anchor = AdrAnchor::try_new("d2-1-refs").unwrap();
        assert_eq!(anchor.as_ref(), "d2-1-refs");
    }

    #[test]
    fn adr_anchor_rejects_empty() {
        let err = AdrAnchor::try_new("").unwrap_err();
        assert!(matches!(err, ValidationError::EmptyAdrAnchor));
    }

    #[test]
    fn adr_anchor_rejects_whitespace_only() {
        let err = AdrAnchor::try_new("   ").unwrap_err();
        assert!(matches!(err, ValidationError::EmptyAdrAnchor));
    }

    #[test]
    fn adr_ref_constructs_with_path_and_anchor() {
        let anchor = AdrAnchor::try_new("D2.1").unwrap();
        let r = AdrRef::new("knowledge/adr/2026-04-19-1242.md", anchor.clone());
        assert_eq!(r.file, PathBuf::from("knowledge/adr/2026-04-19-1242.md"));
        assert_eq!(r.anchor, anchor);
    }
}
