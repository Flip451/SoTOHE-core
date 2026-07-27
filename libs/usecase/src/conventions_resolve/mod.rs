//! Convention-resolution contracts shared by the read-only `conventions resolve`
//! command and the convention non-shipping check.
//!
//! This module owns the identity of a convention document path, the capability
//! identifier both sides of the match are compared as, the request that names
//! what to resolve, the scanned-document and result values, the fail-closed
//! conditions of resolution, the port that scans documents, the pure selection
//! among them, and the primary port both drivers resolve through (spec
//! `IN-05`, `IN-06`, `AC-06`, `AC-07`, `AC-08`, `AC-09`).

mod capability_id;
mod requirement_port;
#[cfg(test)]
mod resolve_error_tests;
mod resolve_service;
#[cfg(test)]
mod selection_tests;

use std::path::{Component, Path, PathBuf};

pub use capability_id::{ConventionCapabilityId, ConventionCapabilityIdError};
pub use requirement_port::ConventionRequirementPort;
pub use resolve_service::{ConventionResolveInteractor, ConventionResolveService};

use crate::capability_exec::CapabilityFailureDetail;

/// Repository-relative directory every convention document lives under.
const CONVENTION_ROOT: &str = "knowledge/conventions";

/// Repository-relative path of a convention document.
///
/// Two invariants, both established only by
/// [`ConventionDocumentPath::try_new`], so no consumer re-checks either and
/// none can skip them: the path names a document strictly inside
/// `knowledge/conventions/`, and its rendered text is one line that still
/// identifies the file it came from.
///
/// The second invariant belongs here and not at the renderer because
/// `IN-05`/`AC-06` promise a canonical result of one repository-relative path
/// per line, and whether a candidate can be one such record is part of the
/// question this type already answers. Deciding it at a rendering site would
/// make that site a second definition of a valid convention document path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConventionDocumentPath(PathBuf);

impl ConventionDocumentPath {
    /// Validates and wraps a repository-relative convention document path.
    ///
    /// `.` components are dropped; the remaining path must start with
    /// `knowledge/conventions/` and name something below it, and it must
    /// render as a single text record.
    ///
    /// # Errors
    ///
    /// Returns [`ConventionDocumentPathError::OutsideConventionRoot`] when
    /// `path` is absolute, carries a parent-directory component, resolves to
    /// the convention root itself, or lies outside that root — that rule is
    /// applied first, so a path failing both is reported as escaping the root.
    /// Returns [`ConventionDocumentPathError::NotRenderableAsRecord`] when a
    /// path inside the root does not render as one identifying line.
    pub fn try_new(path: PathBuf) -> Result<Self, ConventionDocumentPathError> {
        let Some(normalized) = normalize_inside_root(&path) else {
            return Err(ConventionDocumentPathError::OutsideConventionRoot { path });
        };
        if !renders_as_one_record(&normalized) {
            return Err(ConventionDocumentPathError::NotRenderableAsRecord { path });
        }
        Ok(Self(normalized))
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

/// Reports whether `path` renders as one line of text that still identifies
/// the file it names.
///
/// Both halves it rejects are failures of identification rather than of
/// appearance, which is why neither is answerable by escaping or replacing the
/// offending bytes at render time — the escaped text would name a file that
/// does not exist.
///
/// A path that is not valid UTF-8 has a rendered form only through a lossy
/// conversion, so two distinct paths can render as the same replacement text
/// and the reader cannot tell which file a record came from. A path holding
/// `\n` splits into two apparent records under any line-oriented reader, and
/// `\r` is rejected with it because a reader that accepts CRLF strips a
/// trailing one, collapsing the record onto a different path. Nothing narrower
/// than "valid UTF-8 and no line terminator" delivers the guarantee wanted:
/// that the rendered line maps back to exactly one file.
fn renders_as_one_record(path: &Path) -> bool {
    path.to_str().is_some_and(|text| !text.contains(['\n', '\r']))
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
    /// The candidate path does not render as one line of text identifying it.
    #[error(
        "convention document path is not renderable as a single record (it must be valid UTF-8 \
         and hold no line terminator): {path:?}"
    )]
    NotRenderableAsRecord {
        /// Rejected path, as supplied to the constructor. Rendered
        /// debug-escaped rather than through `Display`, because a `Display` of
        /// this path is precisely what the variant rejects: it would either
        /// break the diagnostic across lines or lose which file was rejected.
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
    /// A resolved path did not satisfy the convention document path rule.
    ///
    /// Both the name and the message state the category rather than the rule
    /// that fired, because [`ConventionDocumentPathError`] has more than one
    /// shape: it composes the escape rejection and the record-renderability
    /// rejection alike. The earlier name `DocumentPathOutsideRoot` would tell a
    /// reader who hit a filename holding a line terminator to look for a path
    /// escape that never happened.
    ///
    /// Naming the category is also what keeps the rule out of this enum:
    /// [`ConventionDocumentPathError`] owns every rejection reason and carries
    /// the offending path, so restating one of them here would put a second
    /// wording of the same rule beside the original. The specific reason is
    /// read from the source.
    #[error("the scan resolved a path that is not a convention document path")]
    DocumentPathRejected {
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
    /// The convention root exists but its contents could not be listed.
    ///
    /// Distinct from [`Self::DocumentUnreadable`] because the root is not a
    /// document: [`ConventionDocumentPath`] rejects it by design, so no
    /// document-shaped variant can name it. Distinct from a normal empty result
    /// because "no document declares this capability" and "the documents could
    /// not be looked at" mean opposite things to a caller, and collapsing them
    /// would report a repository whose convention tree is unreadable as one
    /// that simply requires nothing.
    #[error("convention root is unlistable '{root}': {detail}")]
    ConventionRootUnlistable {
        /// Repository-relative convention root that could not be listed.
        root: PathBuf,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
}

/// Read-only request naming the capability whose required conventions are
/// wanted and the project root to scan (`IN-05`, `IN-06`, `AC-09`).
///
/// The capability is a [`ConventionCapabilityId`] rather than a string because
/// non-blankness is a real invariant of the value, and rather than the
/// `CapabilityName` lookup key because the query side of an exact-match
/// comparison has to preserve its value as literally as the document side
/// does. Nothing further is asserted about it, so an identifier registered in
/// neither `.harness/capabilities/` nor `agent-profiles.json` is an ordinary
/// request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveConventionsQuery {
    /// Capability whose required convention documents are wanted.
    pub capability: ConventionCapabilityId,
    /// Root of the project whose `knowledge/conventions/` tree is scanned.
    pub project_root: PathBuf,
}

/// One scanned convention document paired with the capability identifiers its
/// front matter declares (`IN-05`, `AC-06`, `AC-08`).
///
/// An empty declaration list is an ordinary value of this same shape: a
/// document without front matter and a document whose front matter carries no
/// `required_for` are both requirements that require nothing, so no consumer
/// has to re-join those cases and neither can drift into meaning a failure.
///
/// The declared identifiers are [`ConventionCapabilityId`], the matching-term
/// type, and so is the argument of [`ConventionRequirement::requires`], so both
/// sides of the comparison preserve their value verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionRequirement {
    document: ConventionDocumentPath,
    required_for: Vec<ConventionCapabilityId>,
}

impl ConventionRequirement {
    /// Pairs a scanned document with the capability identifiers it declares.
    #[must_use]
    pub fn new(
        document: ConventionDocumentPath,
        required_for: Vec<ConventionCapabilityId>,
    ) -> Self {
        Self { document, required_for }
    }

    /// Borrows the document this requirement was scanned from.
    #[must_use]
    pub fn document(&self) -> &ConventionDocumentPath {
        &self.document
    }

    /// Reports whether the document declares `capability`.
    ///
    /// The comparison is whole-value equality of the identifier with nothing
    /// normalized in between, so a prefix, a suffix, a case-differing
    /// spelling, or a padded spelling does not match.
    #[must_use]
    pub fn requires(&self, capability: &ConventionCapabilityId) -> bool {
        self.required_for.contains(capability)
    }
}

/// Canonical result of resolving the conventions required by one capability
/// (`IN-05`, `AC-06`, `AC-08`).
///
/// The type owns the deduplication and stable-ordering guarantee, so no
/// consumer re-sorts the documents and none can disagree about their order.
/// Being a success value by construction, the zero-match case is this value
/// with no documents — [`ConventionResolution::default`] — and is unreachable
/// through [`ConventionResolveError`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConventionResolution {
    documents: Vec<ConventionDocumentPath>,
}

impl ConventionResolution {
    /// Builds the canonical resolution from the documents that matched.
    ///
    /// Duplicates are removed and the documents are placed in lexicographic
    /// path order, so the result does not depend on the order the scan
    /// produced its matches in.
    #[must_use]
    pub fn from_matches(mut matches: Vec<ConventionDocumentPath>) -> Self {
        matches.sort();
        matches.dedup();
        Self { documents: matches }
    }

    /// Borrows the matched documents, deduplicated and in stable order.
    #[must_use]
    pub fn documents(&self) -> &[ConventionDocumentPath] {
        &self.documents
    }

    /// Reports whether the resolution matched no document.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }
}

/// Selects the scanned documents that declare `capability` (`AC-06`, `AC-08`).
///
/// A free function rather than an interactor method because it holds no state
/// and needs no injected dependency, which is also what makes the selection
/// exercisable without any filesystem input: everything it decides on is
/// already in `requirements`.
///
/// Whether a document declares the capability is
/// [`ConventionRequirement::requires`]' decision, taken unchanged — nothing is
/// normalized on either side here. The matches are handed to
/// [`ConventionResolution::from_matches`], so the deduplication and ordering
/// the result guarantees are the ones this function returns, and the scan's
/// order does not reach the caller.
///
/// The return type is not a `Result`: matching no document is the ordinary
/// empty resolution (`AC-08`), not a failure.
///
/// Defined here rather than in a sibling module because a free function is
/// catalogued under the module that defines it, and a `pub use` re-export does
/// not fold it back; its tests live in the `selection_tests` sibling so that
/// only the small production half adds to this module's length.
#[must_use]
pub fn select_required_conventions(
    requirements: &[ConventionRequirement],
    capability: &ConventionCapabilityId,
) -> ConventionResolution {
    ConventionResolution::from_matches(
        requirements
            .iter()
            .filter(|requirement| requirement.requires(capability))
            .map(|requirement| requirement.document().clone())
            .collect(),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        ConventionCapabilityId, ConventionDocumentPath, ConventionDocumentPathError,
        ConventionRequirement, ConventionResolution, ResolveConventionsQuery,
    };

    fn document(path: &str) -> ConventionDocumentPath {
        ConventionDocumentPath::try_new(PathBuf::from(path)).unwrap()
    }

    fn capability(id: &str) -> ConventionCapabilityId {
        ConventionCapabilityId::try_new(id).unwrap()
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
    fn test_convention_requirement_pairs_the_scanned_document_with_the_capabilities_it_declares() {
        let requirement = ConventionRequirement::new(
            document("knowledge/conventions/testing.md"),
            vec![capability("implementer"), capability("reviewer")],
        );

        assert_eq!(requirement.document(), &document("knowledge/conventions/testing.md"));
        assert!(requirement.requires(&capability("implementer")));
        assert!(requirement.requires(&capability("reviewer")));
        assert!(
            !requirement.requires(&capability("researcher")),
            "a capability the document does not declare is not required by it"
        );
    }

    #[test]
    fn test_convention_requirement_requires_only_on_whole_value_capability_id_match() {
        let requirement = ConventionRequirement::new(
            document("knowledge/conventions/testing.md"),
            vec![capability("implementer")],
        );

        assert!(requirement.requires(&capability("implementer")));
        // The padded spellings are the reason the matching term is not the
        // trimming lookup key: under that key they would all normalize to the
        // declared identifier and match it.
        for near_miss in [
            "implement",
            "implementer-lead",
            "Implementer",
            "impl",
            " implementer",
            "implementer ",
            " implementer ",
            "implementer\t",
        ] {
            assert!(
                !requirement.requires(&capability(near_miss)),
                "'{near_miss}' is not the declared identifier, so it must not match"
            );
        }
    }

    #[test]
    fn test_convention_requirement_without_declared_capabilities_requires_nothing() {
        // A document with no front matter and a document whose front matter
        // carries no `required_for` both arrive here as an empty declaration
        // list: an ordinary value, with no failure state to reach.
        let requirement =
            ConventionRequirement::new(document("knowledge/conventions/git-notes.md"), Vec::new());

        assert_eq!(requirement.document(), &document("knowledge/conventions/git-notes.md"));
        assert!(!requirement.requires(&capability("implementer")));
        assert!(!requirement.requires(&capability("reviewer")));
    }

    #[test]
    fn test_convention_resolution_from_matches_deduplicates_repeated_documents() {
        let resolution = ConventionResolution::from_matches(vec![
            document("knowledge/conventions/testing.md"),
            document("knowledge/conventions/adr.md"),
            document("knowledge/conventions/testing.md"),
        ]);

        let paths: Vec<String> = resolution.documents().iter().map(ToString::to_string).collect();
        assert_eq!(
            paths,
            ["knowledge/conventions/adr.md", "knowledge/conventions/testing.md"],
            "a document matched twice by the scan appears once in the result"
        );
    }

    #[test]
    fn test_convention_resolution_from_matches_orders_documents_independently_of_scan_order() {
        let one = ConventionResolution::from_matches(vec![
            document("knowledge/conventions/testing.md"),
            document("knowledge/conventions/rust/naming.md"),
            document("knowledge/conventions/adr.md"),
        ]);
        let other = ConventionResolution::from_matches(vec![
            document("knowledge/conventions/adr.md"),
            document("knowledge/conventions/testing.md"),
            document("knowledge/conventions/rust/naming.md"),
        ]);

        let paths: Vec<String> = one.documents().iter().map(ToString::to_string).collect();
        assert_eq!(
            paths,
            [
                "knowledge/conventions/adr.md",
                "knowledge/conventions/rust/naming.md",
                "knowledge/conventions/testing.md",
            ],
            "the result is in stable repository-relative path order"
        );
        assert_eq!(one, other, "two scan orders over the same matches resolve to the same value");
    }

    #[test]
    fn test_convention_resolution_with_zero_matches_is_an_empty_success_value() {
        let resolution = ConventionResolution::from_matches(Vec::new());

        assert!(resolution.is_empty());
        assert!(resolution.documents().is_empty());
        assert_eq!(
            resolution,
            ConventionResolution::default(),
            "matching no document yields the ordinary empty result, not a failure"
        );
    }

    #[test]
    fn test_resolve_conventions_query_carries_the_requested_capability_and_project_root() {
        let query = ResolveConventionsQuery {
            capability: capability("implementer"),
            project_root: PathBuf::from("/srv/consumer-project"),
        };

        assert_eq!(query.capability.as_str(), "implementer");
        assert_eq!(query.project_root, PathBuf::from("/srv/consumer-project"));
    }

    #[test]
    fn test_resolve_conventions_query_capability_rejects_only_empty_or_blank_identifiers() {
        for blank in ["", "   ", "\t"] {
            assert!(
                ConventionCapabilityId::try_new(blank).is_err(),
                "an empty capability id is the one rejection this value carries"
            );
        }

        for accepted in ["implementer", "team::custom-lint", "SHOUTING_ID", "x", " padded "] {
            let query = ResolveConventionsQuery {
                capability: ConventionCapabilityId::try_new(accepted)
                    .expect("a non-blank capability id is accepted as-is"),
                project_root: PathBuf::from("."),
            };
            assert_eq!(
                query.capability.as_str(),
                accepted,
                "the query carries the requested identifier verbatim, so the match it feeds \
                 compares the value the caller actually named"
            );
        }
    }

    #[test]
    fn test_resolve_conventions_query_accepts_a_capability_id_registered_in_no_registry() {
        // `consumer-house-style` names no entry in `.harness/capabilities/` and
        // none in `agent-profiles.json`. The query consults neither, so there is
        // no state in which building the request fails for want of a
        // registration; the identifier is carried through unchanged.
        let unregistered = ConventionCapabilityId::try_new("consumer-house-style")
            .expect("an identifier registered nowhere is still a valid capability id");

        let query = ResolveConventionsQuery {
            capability: unregistered.clone(),
            project_root: PathBuf::from("/srv/consumer-project"),
        };

        assert_eq!(query.capability, unregistered);
        assert_eq!(query.capability.as_str(), "consumer-house-style");
    }
}
