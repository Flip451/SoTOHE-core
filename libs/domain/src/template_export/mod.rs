//! Domain model for the template extraction boundary manifest.
//!
//! The boundary manifest is the single machine-readable source of truth that
//! classifies every workspace path as `include` / `exclude` / `overlay`
//! (spec IN-02, CN-02). Export is driven purely by these classifications; it
//! never parses or rewrites file contents (spec CN-03). The manifest is
//! fail-closed: construction rejects duplicate patterns and an empty manifest
//! so unclassified or conflicting paths surface before export runs
//! (spec IN-12). See ADR `2026-07-06-1717-template-extraction-boundary.md` D4.

use std::path::{Component, Path};

use thiserror::Error;

use crate::FreeText;

/// A workspace-relative path pattern used as a manifest key.
///
/// The wrapped value is canonicalized and validated on construction to be
/// workspace-relative (spec IN-02, AC-02, CN-02): non-empty, not absolute, and
/// free of any parent-directory (`..`) component that could escape the
/// workspace. Canonicalization removes current-directory markers and trailing
/// separators, so semantically identical spellings (`vendor`, `vendor/`,
/// `./vendor`) construct equal patterns.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TemplatePathPattern(String);

impl TemplatePathPattern {
    /// Constructs a `TemplatePathPattern`, canonicalizing current-directory
    /// spellings and trailing separators while validating the
    /// workspace-relative invariant (spec IN-02, AC-02).
    ///
    /// # Errors
    ///
    /// Returns [`TemplatePathPatternError::InvalidPattern`] when `value` is not
    /// workspace-relative (empty, absolute, or containing a `..` component).
    pub fn try_new(value: String) -> Result<Self, TemplatePathPatternError> {
        match Self::normalize_workspace_relative(&value) {
            Some(normalized) => Ok(Self(normalized)),
            None => Err(TemplatePathPatternError::InvalidPattern {
                reason: FreeText::new(format!(
                    "pattern must be a non-empty workspace-relative path: {value:?}",
                )),
            }),
        }
    }

    /// Returns the underlying pattern string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether the pattern is workspace-relative.
    ///
    /// Invariant predicate for `is_workspace_relative`: a pattern is
    /// workspace-relative when its normalized representation is non-empty and
    /// every path component is a normal segment or the current-directory marker
    /// (no root, prefix, or parent-directory component).
    #[must_use]
    pub fn is_workspace_relative(&self) -> bool {
        Self::normalize_workspace_relative(&self.0).as_deref() == Some(self.0.as_str())
    }

    fn normalize_workspace_relative(value: &str) -> Option<String> {
        let mut normalized = String::new();
        let mut saw_current_dir = false;

        for component in Path::new(value).components() {
            match component {
                Component::Normal(segment) => {
                    let segment = segment.to_str()?;
                    if !normalized.is_empty() {
                        normalized.push('/');
                    }
                    normalized.push_str(segment);
                }
                Component::CurDir => {
                    saw_current_dir = true;
                }
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return None;
                }
            }
        }

        if normalized.is_empty() {
            saw_current_dir.then(|| ".".to_owned())
        } else {
            Some(normalized)
        }
    }
}

/// Error produced by [`TemplatePathPattern::try_new`] (spec IN-02, IN-12, AC-02).
#[derive(Debug, Error)]
pub enum TemplatePathPatternError {
    /// The supplied value is not a workspace-relative path.
    #[error("invalid template path pattern: {reason}")]
    InvalidPattern {
        /// Human-readable diagnostic describing why the pattern was rejected.
        reason: FreeText,
    },
}

/// Boundary classification for a manifest row (spec IN-01, IN-02, AC-02, CN-03).
///
/// Each classification maps a workspace path to one export behaviour:
/// `Include` copies the path as-is, `Exclude` drops it from the template, and
/// `Overlay` replaces it with the overlay directory's template version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplatePathClassification {
    /// Include the path in the exported template unchanged.
    Include,
    /// Exclude the path from the exported template.
    Exclude,
    /// Overwrite the path with the overlay directory's template version.
    Overlay,
}

/// A single manifest row pairing a pattern with its classification
/// (spec IN-02, AC-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplatePathEntry {
    pattern: TemplatePathPattern,
    classification: TemplatePathClassification,
}

impl TemplatePathEntry {
    /// Constructs a `TemplatePathEntry` from a pattern and its classification.
    #[must_use]
    pub fn new(pattern: TemplatePathPattern, classification: TemplatePathClassification) -> Self {
        Self { pattern, classification }
    }

    /// Returns the row's path pattern.
    #[must_use]
    pub fn pattern(&self) -> &TemplatePathPattern {
        &self.pattern
    }

    /// Returns the row's boundary classification.
    #[must_use]
    pub fn classification(&self) -> TemplatePathClassification {
        self.classification
    }
}

/// The boundary SSoT: the full set of classified paths (spec IN-02, IN-12,
/// AC-02, CN-02).
///
/// Construction is fail-closed (spec IN-12): it rejects an empty manifest and
/// any duplicate pattern so conflicting classifications cannot reach export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateBoundaryManifest {
    entries: Vec<TemplatePathEntry>,
}

impl TemplateBoundaryManifest {
    /// Constructs a `TemplateBoundaryManifest`, enforcing the non-empty,
    /// unique-pattern, and prefix-free invariants (spec IN-02, IN-12, AC-02).
    ///
    /// # Errors
    ///
    /// - [`TemplateBoundaryManifestError::EmptyManifest`] when `entries` is empty.
    /// - [`TemplateBoundaryManifestError::DuplicatePattern`] when two entries
    ///   share the same pattern.
    /// - [`TemplateBoundaryManifestError::NestedPattern`] when one entry's pattern
    ///   is a path-ancestor of another's, which would let an `include` subtree
    ///   copy leak a differently-classified descendant (spec IN-12).
    pub fn try_new(entries: Vec<TemplatePathEntry>) -> Result<Self, TemplateBoundaryManifestError> {
        if entries.is_empty() {
            return Err(TemplateBoundaryManifestError::EmptyManifest);
        }

        // Fail-closed on conflicting classifications: reject the first pattern
        // that appears in more than one entry (spec IN-12).
        for (index, entry) in entries.iter().enumerate() {
            if entries.iter().take(index).any(|prior| prior.pattern() == entry.pattern()) {
                return Err(TemplateBoundaryManifestError::DuplicatePattern {
                    pattern: entry.pattern().clone(),
                });
            }
        }

        // Fail-closed on nested classifications: a pattern that is a path-ancestor
        // of another would make the ancestor's subtree copy leak the descendant's
        // separately-classified path (spec IN-12). Reject the first such pair.
        for (index, entry) in entries.iter().enumerate() {
            for prior in entries.iter().take(index) {
                if is_path_ancestor(prior.pattern(), entry.pattern()) {
                    return Err(TemplateBoundaryManifestError::NestedPattern {
                        ancestor: prior.pattern().clone(),
                        descendant: entry.pattern().clone(),
                    });
                }
                if is_path_ancestor(entry.pattern(), prior.pattern()) {
                    return Err(TemplateBoundaryManifestError::NestedPattern {
                        ancestor: entry.pattern().clone(),
                        descendant: prior.pattern().clone(),
                    });
                }
            }
        }

        Ok(Self { entries })
    }

    /// Returns the manifest rows in insertion order.
    #[must_use]
    pub fn entries(&self) -> &[TemplatePathEntry] {
        &self.entries
    }

    /// Looks up the classification for `path` (spec IN-02, AC-02, CN-03).
    ///
    /// Returns `None` when the path is not present in the manifest; callers
    /// treat an unclassified path as a fail-closed condition (spec IN-12).
    #[must_use]
    pub fn classify(&self, path: &TemplatePathPattern) -> Option<TemplatePathClassification> {
        self.entries
            .iter()
            .find(|entry| entry.pattern() == path)
            .map(TemplatePathEntry::classification)
    }

    /// Returns whether every entry has a distinct pattern.
    ///
    /// Invariant predicate for `patterns_are_unique`; construction guarantees
    /// this holds, but it is exposed for downstream re-validation.
    #[must_use]
    pub fn patterns_are_unique(&self) -> bool {
        self.entries.iter().enumerate().all(|(index, entry)| {
            !self.entries.iter().take(index).any(|prior| prior.pattern() == entry.pattern())
        })
    }

    /// Returns whether no entry's pattern is a path-ancestor of another's.
    ///
    /// Invariant predicate for `patterns_are_prefix_free`; construction guarantees
    /// this holds, but it is exposed for downstream re-validation. Prefix-freeness
    /// keeps every classification's subtree disjoint, so an `include` copy can
    /// never leak a differently-classified descendant (spec IN-12).
    #[must_use]
    pub fn patterns_are_prefix_free(&self) -> bool {
        self.entries.iter().enumerate().all(|(index, entry)| {
            self.entries.iter().take(index).all(|prior| {
                !is_path_ancestor(prior.pattern(), entry.pattern())
                    && !is_path_ancestor(entry.pattern(), prior.pattern())
            })
        })
    }
}

/// Returns whether `ancestor` is a strict path-ancestor of `descendant`,
/// comparing canonical path segments rather than raw string prefixes.
///
/// `a` is an ancestor of `a/b`, but `ab` is not an ancestor of `a/b` (segment
/// boundaries are respected). The workspace-root pattern `.` is an ancestor of
/// every other pattern and of nothing but itself. Equal patterns are never
/// ancestors of each other (the relation is strict).
fn is_path_ancestor(ancestor: &TemplatePathPattern, descendant: &TemplatePathPattern) -> bool {
    if ancestor == descendant {
        return false;
    }
    // The workspace root contains every other path; nothing contains the root.
    if ancestor.as_str() == "." {
        return true;
    }
    if descendant.as_str() == "." {
        return false;
    }

    let mut descendant_segments = descendant.as_str().split('/');
    for ancestor_segment in ancestor.as_str().split('/') {
        match descendant_segments.next() {
            Some(descendant_segment) if descendant_segment == ancestor_segment => {}
            _ => return false,
        }
    }
    // Strict containment: the descendant must have at least one further segment.
    descendant_segments.next().is_some()
}

/// Error produced by [`TemplateBoundaryManifest::try_new`]
/// (spec IN-02, IN-12, AC-02).
#[derive(Debug, Error)]
pub enum TemplateBoundaryManifestError {
    /// Two manifest entries share the same pattern.
    #[error("duplicate template path pattern: {pattern:?}")]
    DuplicatePattern {
        /// The pattern that appeared more than once.
        pattern: TemplatePathPattern,
    },
    /// One entry's pattern is a path-ancestor of another's, so an `include`
    /// subtree copy would leak a differently-classified descendant (spec IN-12).
    #[error("nested template path patterns: {ancestor:?} contains {descendant:?}")]
    NestedPattern {
        /// The ancestor pattern.
        ancestor: TemplatePathPattern,
        /// The descendant pattern nested beneath `ancestor`.
        descendant: TemplatePathPattern,
    },
    /// The manifest contains no entries.
    #[error("template boundary manifest must not be empty")]
    EmptyManifest,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn pattern(value: &str) -> TemplatePathPattern {
        TemplatePathPattern::try_new(value.to_owned()).unwrap()
    }

    fn entry(value: &str, classification: TemplatePathClassification) -> TemplatePathEntry {
        TemplatePathEntry::new(pattern(value), classification)
    }

    // --- TemplatePathPattern ---

    #[rstest]
    #[case::simple("libs/domain/src")]
    #[case::single_segment("Cargo.toml")]
    #[case::cur_dir_prefixed("./libs/usecase")]
    fn pattern_accepts_workspace_relative(#[case] value: &str) {
        let result = TemplatePathPattern::try_new(value.to_owned());
        assert!(result.is_ok(), "expected {value:?} to be accepted");
        assert!(result.unwrap().is_workspace_relative());
    }

    #[rstest]
    #[case::cur_dir_prefixed("./libs/usecase", "libs/usecase")]
    #[case::embedded_cur_dir("libs/./domain", "libs/domain")]
    #[case::trailing_separator("vendor/", "vendor")]
    #[case::workspace_root(".", ".")]
    fn pattern_normalizes_semantically_identical_paths(
        #[case] value: &str,
        #[case] expected: &str,
    ) {
        let result = TemplatePathPattern::try_new(value.to_owned()).unwrap();
        assert_eq!(result.as_str(), expected);
    }

    #[rstest]
    #[case::empty("")]
    #[case::absolute("/etc/passwd")]
    #[case::parent_escape("../outside")]
    #[case::embedded_parent("libs/../../secret")]
    fn pattern_rejects_non_workspace_relative(#[case] value: &str) {
        let result = TemplatePathPattern::try_new(value.to_owned());
        assert!(
            matches!(result, Err(TemplatePathPatternError::InvalidPattern { .. })),
            "expected {value:?} to be rejected"
        );
    }

    #[test]
    fn pattern_as_str_returns_inner() {
        assert_eq!(pattern("apps/cli").as_str(), "apps/cli");
    }

    #[test]
    fn pattern_ordering_is_lexicographic() {
        assert!(pattern("apps/cli") < pattern("libs/domain"));
    }

    #[test]
    fn pattern_equality_uses_path_identity() {
        assert_eq!(pattern("vendor"), pattern("vendor/"));
    }

    // --- TemplatePathEntry ---

    #[test]
    fn entry_exposes_pattern_and_classification() {
        let row = entry("vendor/", TemplatePathClassification::Exclude);
        assert_eq!(row.pattern().as_str(), "vendor");
        assert_eq!(row.classification(), TemplatePathClassification::Exclude);
    }

    // --- TemplateBoundaryManifest ---

    #[test]
    fn manifest_rejects_empty_entries() {
        let result = TemplateBoundaryManifest::try_new(vec![]);
        assert!(matches!(result, Err(TemplateBoundaryManifestError::EmptyManifest)));
    }

    #[test]
    fn manifest_rejects_duplicate_pattern() {
        let entries = vec![
            entry("libs/domain", TemplatePathClassification::Include),
            entry("libs/domain", TemplatePathClassification::Exclude),
        ];
        let err = TemplateBoundaryManifest::try_new(entries).unwrap_err();
        assert!(
            matches!(
                err,
                TemplateBoundaryManifestError::DuplicatePattern { ref pattern }
                    if pattern.as_str() == "libs/domain"
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn manifest_rejects_duplicate_pattern_after_normalization() {
        let entries = vec![
            entry("libs/domain", TemplatePathClassification::Include),
            entry("./libs/domain", TemplatePathClassification::Exclude),
        ];
        let err = TemplateBoundaryManifest::try_new(entries).unwrap_err();
        assert!(
            matches!(
                err,
                TemplateBoundaryManifestError::DuplicatePattern { ref pattern }
                    if pattern.as_str() == "libs/domain"
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn manifest_rejects_duplicate_pattern_with_trailing_separator() {
        let entries = vec![
            entry("vendor", TemplatePathClassification::Include),
            entry("vendor/", TemplatePathClassification::Exclude),
        ];
        let err = TemplateBoundaryManifest::try_new(entries).unwrap_err();
        assert!(
            matches!(
                err,
                TemplateBoundaryManifestError::DuplicatePattern { ref pattern }
                    if pattern.as_str() == "vendor"
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn manifest_accepts_unique_entries() {
        let entries = vec![
            entry("libs/domain", TemplatePathClassification::Include),
            entry("Makefile.toml", TemplatePathClassification::Overlay),
            entry("vendor/", TemplatePathClassification::Exclude),
        ];
        let manifest = TemplateBoundaryManifest::try_new(entries).unwrap();
        assert_eq!(manifest.entries().len(), 3);
        assert!(manifest.patterns_are_unique());
        assert!(manifest.patterns_are_prefix_free());
    }

    #[test]
    fn manifest_rejects_nested_ancestor_descendant_pair() {
        // `config` include + `config/secrets` exclude: the include subtree copy
        // would leak the excluded child, so construction must fail closed.
        let entries = vec![
            entry("config", TemplatePathClassification::Include),
            entry("config/secrets", TemplatePathClassification::Exclude),
        ];
        let err = TemplateBoundaryManifest::try_new(entries).unwrap_err();
        assert!(
            matches!(
                err,
                TemplateBoundaryManifestError::NestedPattern { ref ancestor, ref descendant }
                    if ancestor.as_str() == "config" && descendant.as_str() == "config/secrets"
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn manifest_rejects_nested_pair_regardless_of_order() {
        // Descendant listed before ancestor still reports ancestor/descendant
        // in canonical (containment) order.
        let entries = vec![
            entry("config/secrets", TemplatePathClassification::Exclude),
            entry("config", TemplatePathClassification::Include),
        ];
        let err = TemplateBoundaryManifest::try_new(entries).unwrap_err();
        assert!(
            matches!(
                err,
                TemplateBoundaryManifestError::NestedPattern { ref ancestor, ref descendant }
                    if ancestor.as_str() == "config" && descendant.as_str() == "config/secrets"
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn manifest_accepts_sibling_prefix_that_is_not_a_path_ancestor() {
        // `ab` is a string prefix of `a/b` but NOT a path-ancestor: segment
        // boundaries are respected, so this pair is accepted.
        let entries = vec![
            entry("ab", TemplatePathClassification::Include),
            entry("a/b", TemplatePathClassification::Exclude),
        ];
        let manifest = TemplateBoundaryManifest::try_new(entries).unwrap();
        assert_eq!(manifest.entries().len(), 2);
        assert!(manifest.patterns_are_prefix_free());
    }

    #[test]
    fn manifest_rejects_workspace_root_combined_with_any_entry() {
        // The workspace-root pattern `.` is an ancestor of everything, so it may
        // never coexist with another entry.
        let entries = vec![
            entry(".", TemplatePathClassification::Include),
            entry("libs/domain", TemplatePathClassification::Overlay),
        ];
        let err = TemplateBoundaryManifest::try_new(entries).unwrap_err();
        assert!(
            matches!(
                err,
                TemplateBoundaryManifestError::NestedPattern { ref ancestor, ref descendant }
                    if ancestor.as_str() == "." && descendant.as_str() == "libs/domain"
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn manifest_classify_returns_matching_classification() {
        let entries = vec![
            entry("libs/domain", TemplatePathClassification::Include),
            entry("Makefile.toml", TemplatePathClassification::Overlay),
        ];
        let manifest = TemplateBoundaryManifest::try_new(entries).unwrap();
        assert_eq!(
            manifest.classify(&pattern("Makefile.toml")),
            Some(TemplatePathClassification::Overlay)
        );
    }

    #[test]
    fn manifest_classify_returns_none_for_unclassified_path() {
        let entries = vec![entry("libs/domain", TemplatePathClassification::Include)];
        let manifest = TemplateBoundaryManifest::try_new(entries).unwrap();
        assert_eq!(manifest.classify(&pattern("unknown/path")), None);
    }
}
