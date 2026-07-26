//! Convention-resolution contracts shared by the read-only `conventions resolve`
//! command and the convention non-shipping check.
//!
//! This module owns the identity of a convention document path and the
//! fail-closed conditions of resolution (spec `IN-05`, `AC-06`, `AC-07`).

use std::path::{Component, Path, PathBuf};

use crate::capability_exec::CapabilityFailureDetail;

/// Repository-relative directory every convention document lives under.
const CONVENTION_ROOT: &str = "knowledge/conventions";

/// Repository-relative path of a convention document.
///
/// The single invariant is that the path names a document strictly inside
/// `knowledge/conventions/`; [`ConventionDocumentPath::try_new`] is the only
/// site that establishes it, so no consumer re-checks it and none can skip it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConventionDocumentPath(PathBuf);

impl ConventionDocumentPath {
    /// Validates and wraps a repository-relative convention document path.
    ///
    /// `.` components are dropped; the remaining path must start with
    /// `knowledge/conventions/` and name something below it.
    ///
    /// # Errors
    ///
    /// Returns [`ConventionDocumentPathError::OutsideConventionRoot`] when
    /// `path` is absolute, carries a parent-directory component, resolves to
    /// the convention root itself, or lies outside that root.
    pub fn try_new(path: PathBuf) -> Result<Self, ConventionDocumentPathError> {
        match normalize_inside_root(&path) {
            Some(normalized) => Ok(Self(normalized)),
            None => Err(ConventionDocumentPathError::OutsideConventionRoot { path }),
        }
    }

    /// Returns the validated repository-relative document path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl std::fmt::Display for ConventionDocumentPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.as_path().display())
    }
}

/// Normalizes `path` and returns it when it names a document strictly inside
/// [`CONVENTION_ROOT`], or `None` when it escapes that root.
fn normalize_inside_root(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    let inside = normalized
        .strip_prefix(CONVENTION_ROOT)
        .is_ok_and(|remainder| !remainder.as_os_str().is_empty());
    inside.then_some(normalized)
}

/// Construction failure of [`ConventionDocumentPath`].
///
/// Extracted from the resolution errors so that every consumer of the
/// constructor admits exactly this rejection and nothing more.
#[derive(Debug, thiserror::Error)]
pub enum ConventionDocumentPathError {
    /// The candidate path does not name a document inside `knowledge/conventions/`.
    #[error("convention document path is outside 'knowledge/conventions/': {}", path.display())]
    OutsideConventionRoot {
        /// Rejected path, as supplied to the constructor.
        path: PathBuf,
    },
}

/// Structural fail-closed conditions of convention resolution (`AC-07`).
///
/// The normal empty states — a document without front matter, without
/// `required_for`, or a capability matching zero documents — are values of the
/// resolution result, not variants here.
#[derive(Debug, thiserror::Error)]
pub enum ConventionResolveError {
    /// A document's front matter could not be parsed as YAML.
    #[error("convention front matter is not parseable YAML in '{document}': {detail}")]
    FrontMatterUnparseable {
        /// Document whose front matter could not be parsed.
        document: ConventionDocumentPath,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
    /// A document's `required_for` value is not an array of strings.
    #[error("'required_for' is not a string array in '{document}': {detail}")]
    RequiredForNotStringArray {
        /// Document whose `required_for` value has the wrong shape.
        document: ConventionDocumentPath,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
    /// A document's `required_for` array holds an empty or blank capability ID.
    #[error("'required_for' holds an empty capability id in '{document}'")]
    EmptyCapabilityId {
        /// Document holding the empty capability ID.
        document: ConventionDocumentPath,
    },
    /// A resolved path escaped the convention root.
    #[error("resolved convention document path escapes the convention root")]
    DocumentPathOutsideRoot {
        /// Constructor rejection this variant composes.
        source: ConventionDocumentPathError,
    },
    /// A document under the convention root could not be read.
    #[error("convention document is unreadable '{document}': {detail}")]
    DocumentUnreadable {
        /// Document that could not be read.
        document: ConventionDocumentPath,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::error::Error as _;
    use std::path::{Path, PathBuf};

    use super::{ConventionDocumentPath, ConventionDocumentPathError, ConventionResolveError};
    use crate::capability_exec::CapabilityFailureDetail;

    fn document(path: &str) -> ConventionDocumentPath {
        ConventionDocumentPath::try_new(PathBuf::from(path)).unwrap()
    }

    #[test]
    fn test_convention_document_path_with_path_inside_root_succeeds() {
        let resolved = ConventionDocumentPath::try_new(PathBuf::from(
            "knowledge/conventions/coding-principles.md",
        ))
        .unwrap();

        assert_eq!(resolved.as_path(), Path::new("knowledge/conventions/coding-principles.md"));
    }

    #[test]
    fn test_convention_document_path_with_nested_path_inside_root_succeeds() {
        let resolved =
            ConventionDocumentPath::try_new(PathBuf::from("knowledge/conventions/rust/testing.md"))
                .unwrap();

        assert_eq!(resolved.as_path(), Path::new("knowledge/conventions/rust/testing.md"));
    }

    #[test]
    fn test_convention_document_path_with_current_dir_prefix_normalizes_and_succeeds() {
        let resolved =
            ConventionDocumentPath::try_new(PathBuf::from("./knowledge/./conventions/testing.md"))
                .unwrap();

        assert_eq!(
            resolved.as_path(),
            Path::new("knowledge/conventions/testing.md"),
            "`.` components must be dropped rather than rejected"
        );
    }

    #[test]
    fn test_convention_document_path_with_parent_dir_escape_returns_outside_convention_root_error()
    {
        let result = ConventionDocumentPath::try_new(PathBuf::from(
            "knowledge/conventions/../adr/README.md",
        ));

        assert!(
            matches!(result, Err(ConventionDocumentPathError::OutsideConventionRoot { .. })),
            "a parent-directory component can leave the convention root, so it is rejected"
        );
    }

    #[test]
    fn test_convention_document_path_with_absolute_path_returns_outside_convention_root_error() {
        let result =
            ConventionDocumentPath::try_new(PathBuf::from("/srv/knowledge/conventions/testing.md"));

        assert!(matches!(result, Err(ConventionDocumentPathError::OutsideConventionRoot { .. })));
    }

    #[test]
    fn test_convention_document_path_with_sibling_directory_returns_outside_convention_root_error()
    {
        let result = ConventionDocumentPath::try_new(PathBuf::from("knowledge/adr/README.md"));

        assert!(matches!(result, Err(ConventionDocumentPathError::OutsideConventionRoot { .. })));
    }

    #[test]
    fn test_convention_document_path_with_root_directory_itself_returns_outside_convention_root_error()
     {
        let result = ConventionDocumentPath::try_new(PathBuf::from("knowledge/conventions"));

        assert!(
            matches!(result, Err(ConventionDocumentPathError::OutsideConventionRoot { .. })),
            "the root directory is not a document inside the root"
        );
    }

    #[test]
    fn test_convention_document_path_error_retains_supplied_path() {
        let result = ConventionDocumentPath::try_new(PathBuf::from("knowledge/adr/README.md"));

        let Err(ConventionDocumentPathError::OutsideConventionRoot { path }) = result else {
            panic!("expected an OutsideConventionRoot rejection");
        };
        assert_eq!(path, PathBuf::from("knowledge/adr/README.md"));
        assert!(path.display().to_string().contains("knowledge/adr/README.md"));
    }

    #[test]
    fn test_convention_document_path_display_renders_repository_relative_path() {
        let resolved = document("knowledge/conventions/testing.md");

        assert_eq!(resolved.to_string(), "knowledge/conventions/testing.md");
    }

    #[test]
    fn test_convention_document_paths_sort_in_stable_lexicographic_order() {
        let mut paths = [
            document("knowledge/conventions/testing.md"),
            document("knowledge/conventions/adr.md"),
            document("knowledge/conventions/rust/naming.md"),
        ];

        paths.sort();

        let ordered: Vec<String> = paths.iter().map(ToString::to_string).collect();
        assert_eq!(
            ordered,
            [
                "knowledge/conventions/adr.md",
                "knowledge/conventions/rust/naming.md",
                "knowledge/conventions/testing.md",
            ]
        );
    }

    #[test]
    fn test_document_path_outside_root_exposes_the_path_error_as_source() {
        let Err(source) = ConventionDocumentPath::try_new(PathBuf::from("knowledge/adr/README.md"))
        else {
            panic!("expected an OutsideConventionRoot rejection");
        };
        let error = ConventionResolveError::DocumentPathOutsideRoot { source };

        let cause = error.source().expect("the composed path rejection must be the error source");
        assert!(matches!(
            cause.downcast_ref::<ConventionDocumentPathError>(),
            Some(ConventionDocumentPathError::OutsideConventionRoot { .. })
        ));
    }

    #[test]
    fn test_convention_resolve_error_declares_exactly_the_five_fail_closed_conditions() {
        let escaping = PathBuf::from("knowledge/adr/README.md");
        let Err(outside_root) = ConventionDocumentPath::try_new(escaping.clone()) else {
            panic!("expected an OutsideConventionRoot rejection");
        };
        let conditions = [
            ConventionResolveError::FrontMatterUnparseable {
                document: document("knowledge/conventions/front-matter.md"),
                detail: CapabilityFailureDetail::new("unexpected token at line 2"),
            },
            ConventionResolveError::RequiredForNotStringArray {
                document: document("knowledge/conventions/required-for.md"),
                detail: CapabilityFailureDetail::new("expected a sequence of strings"),
            },
            ConventionResolveError::EmptyCapabilityId {
                document: document("knowledge/conventions/empty-capability-id.md"),
            },
            ConventionResolveError::DocumentPathOutsideRoot { source: outside_root },
            ConventionResolveError::DocumentUnreadable {
                document: document("knowledge/conventions/unreadable.md"),
                detail: CapabilityFailureDetail::new("permission denied"),
            },
        ];

        // One arm per variant and no wildcard: a sixth condition — including a
        // variant for a normal empty resolution — stops this test compiling.
        let concerned: Vec<PathBuf> = conditions
            .iter()
            .map(|condition| match condition {
                ConventionResolveError::FrontMatterUnparseable { document, .. }
                | ConventionResolveError::RequiredForNotStringArray { document, .. }
                | ConventionResolveError::EmptyCapabilityId { document }
                | ConventionResolveError::DocumentUnreadable { document, .. } => {
                    document.as_path().to_path_buf()
                }
                ConventionResolveError::DocumentPathOutsideRoot {
                    source: ConventionDocumentPathError::OutsideConventionRoot { path },
                } => path.clone(),
            })
            .collect();

        assert_eq!(
            concerned,
            [
                PathBuf::from("knowledge/conventions/front-matter.md"),
                PathBuf::from("knowledge/conventions/required-for.md"),
                PathBuf::from("knowledge/conventions/empty-capability-id.md"),
                escaping,
                PathBuf::from("knowledge/conventions/unreadable.md"),
            ],
            "every condition carries the document it concerns, the escaped path through the \
             composed constructor rejection rather than a restatement of it"
        );
    }

    #[test]
    fn test_convention_resolve_error_display_includes_document_and_detail() {
        let error = ConventionResolveError::FrontMatterUnparseable {
            document: document("knowledge/conventions/testing.md"),
            detail: CapabilityFailureDetail::new("unexpected token at line 2"),
        };

        let rendered = error.to_string();
        assert!(rendered.contains("knowledge/conventions/testing.md"), "{rendered}");
        assert!(rendered.contains("unexpected token at line 2"), "{rendered}");
    }

    #[test]
    fn test_convention_resolve_error_display_for_empty_capability_id_names_the_document() {
        let error = ConventionResolveError::EmptyCapabilityId {
            document: document("knowledge/conventions/adr.md"),
        };

        assert!(error.to_string().contains("knowledge/conventions/adr.md"));
    }
}
