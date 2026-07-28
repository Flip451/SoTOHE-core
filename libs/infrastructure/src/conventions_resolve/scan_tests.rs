//! Tests for the convention directory scan and the adapter that injects it.
//!
//! Kept in a sibling module so that only the production half adds to the parent
//! module's length. Every fixture is a real tempdir tree rather than a fake
//! port, because what is under test here is the filesystem behaviour itself.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::thread;

use tempfile::TempDir;
use usecase::conventions_resolve::{
    ConventionCapabilityId, ConventionDocumentPathError, ConventionRequirement,
    ConventionRequirementPort, ConventionResolveError,
};

use super::{FsConventionRequirementAdapter, scan_convention_requirements};

/// Identifiers every summary probes for, so that what a document declares is
/// read back through the same whole-value comparison the resolution uses.
const PROBED: [&str; 4] = ["implementer", "reviewer", "researcher", "consumer-house-style"];

fn capability(id: &str) -> ConventionCapabilityId {
    ConventionCapabilityId::try_new(id).unwrap()
}

/// Builds a project tree from `files`, given as repository-relative path and
/// content pairs.
fn project_with(files: &[(&str, &str)]) -> TempDir {
    let project = TempDir::new().unwrap();
    for (relative, content) in files {
        write_bytes(project.path(), relative, content.as_bytes());
    }
    project
}

fn write_bytes(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// Renders each scanned requirement as `<document> -> <declared identifiers>`
/// and sorts the renderings into name order.
///
/// Sorted by this helper rather than taken as the scan produced it, because
/// what order a scan hands its documents back in is not something this layer
/// promises: `ConventionResolution::from_matches` sorts and deduplicates what
/// it is given, so that is where the order a caller sees is decided. Comparing
/// against an ordered rendering keeps these assertions about *which* documents
/// were scanned and what each declares, which is what this layer does owe.
fn sorted_summaries(requirements: &[ConventionRequirement]) -> Vec<String> {
    let mut summarized: Vec<String> = requirements
        .iter()
        .map(|requirement| {
            let declared: Vec<&str> =
                PROBED.into_iter().filter(|id| requirement.requires(&capability(id))).collect();
            format!("{} -> {}", requirement.document(), declared.join(", "))
        })
        .collect();
    summarized.sort();
    summarized
}

/// Sorts the scanned documents into name order, sorted here for the reason
/// [`sorted_summaries`] is.
fn sorted_documents(requirements: &[ConventionRequirement]) -> Vec<String> {
    let mut listed: Vec<String> =
        requirements.iter().map(|requirement| requirement.document().to_string()).collect();
    listed.sort();
    listed
}

/// Lists every file below `root` with its bytes, in path order.
fn tree_snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path.strip_prefix(root).unwrap().display().to_string();
                found.push((relative, std::fs::read(&path).unwrap()));
            }
        }
    }
    found.sort();
    found
}

const ADR_DOCUMENT: &str = "---\nrequired_for:\n  - implementer\n  - reviewer\n---\n\n# ADR\n";
const NAMING_DOCUMENT: &str = "---\nrequired_for:\n  - implementer\n---\n\n# Naming\n";
const UNDECLARED_DOCUMENT: &str = "# Git notes\n";

#[test]
fn test_scan_convention_requirements_pairs_each_document_with_what_its_front_matter_declares() {
    let project = project_with(&[
        ("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT),
        ("knowledge/conventions/fixture-sub/fixture-beta.md", NAMING_DOCUMENT),
        ("knowledge/conventions/fixture-plain.md", UNDECLARED_DOCUMENT),
    ]);

    let scanned = scan_convention_requirements(project.path()).unwrap();

    // The fixture root is an arbitrary absolute temporary directory, so a
    // document path carrying any part of it would show up in this comparison.
    assert_eq!(
        sorted_summaries(&scanned),
        [
            "knowledge/conventions/fixture-alpha.md -> implementer, reviewer",
            "knowledge/conventions/fixture-plain.md -> ",
            "knowledge/conventions/fixture-sub/fixture-beta.md -> implementer",
        ],
        "each document below the convention tree is scanned once by its repository-relative \
         path, nested ones included, and carries its own declarations rather than the scan's union"
    );
}

#[test]
fn test_scan_convention_requirements_reads_a_document_written_with_crlf_line_endings() {
    // A real file on disk holding the bytes a Windows checkout would hold:
    // every break in it is CRLF, the front-matter delimiters included, so
    // nothing about the fixture is exotic. Written as bytes so no helper can
    // quietly rewrite the breaks between this test and the read.
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    write_bytes(
        project.path(),
        "knowledge/conventions/fixture-crlf-checkout.md",
        b"---\r\nname: crlf\r\nrequired_for:\r\n  - implementer\r\n  - researcher\r\n---\r\n\r\n# CRLF checkout\r\n",
    );

    let scanned = scan_convention_requirements(project.path()).unwrap();

    assert_eq!(
        sorted_summaries(&scanned),
        [
            "knowledge/conventions/fixture-alpha.md -> implementer, reviewer",
            "knowledge/conventions/fixture-crlf-checkout.md -> implementer, researcher",
        ],
        "a CRLF document declares what it says it declares: reading it as carrying no front \
         matter would drop both declarations and hand the caller a document requiring nothing, \
         with no failure anywhere to say so"
    );
}

#[test]
fn test_scan_convention_requirements_reads_only_markdown_documents() {
    let project = project_with(&[
        ("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT),
        ("knowledge/conventions/README.txt", "not a convention document\n"),
        // Well-formed YAML whose `required_for` is not an array of strings: had
        // the walk read it as a document, the scan would have failed closed
        // instead of returning.
        ("knowledge/conventions/index.yaml", "required_for: 5\n"),
        ("knowledge/conventions/notes", "extensionless\n"),
    ]);

    let scanned = scan_convention_requirements(project.path()).unwrap();

    assert_eq!(
        sorted_documents(&scanned),
        ["knowledge/conventions/fixture-alpha.md"],
        "`IN-05` names `knowledge/conventions/**/*.md`, so a neighbouring file with another \
         extension is not a convention document"
    );
}

#[test]
fn test_scan_convention_requirements_scans_only_the_convention_tree_of_the_supplied_root() {
    let project = project_with(&[
        ("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT),
        ("knowledge/adr/README.md", NAMING_DOCUMENT),
        ("docs/testing.md", NAMING_DOCUMENT),
    ]);
    let other = project_with(&[("knowledge/conventions/fixture-other.md", NAMING_DOCUMENT)]);

    let scanned = scan_convention_requirements(project.path()).unwrap();
    let scanned_other = scan_convention_requirements(other.path()).unwrap();

    assert_eq!(
        sorted_documents(&scanned),
        ["knowledge/conventions/fixture-alpha.md"],
        "a markdown file outside `knowledge/conventions/` is not part of the tree scanned"
    );
    assert_eq!(
        sorted_documents(&scanned_other),
        ["knowledge/conventions/fixture-other.md"],
        "the tree scanned is the one under the root the caller named, so the scan reads no \
         ambient location"
    );
}

#[test]
fn test_scan_convention_requirements_without_a_convention_tree_scans_nothing() {
    let project = project_with(&[("docs/testing.md", NAMING_DOCUMENT)]);

    let scanned = scan_convention_requirements(project.path()).unwrap();

    assert!(
        scanned.is_empty(),
        "a project with no convention tree has no documents, which is an ordinary empty result \
         and none of the conditions `AC-07` fails closed on"
    );
}

#[test]
fn test_scan_convention_requirements_with_an_unreadable_document_fails_closed() {
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    // Bytes that are not UTF-8: the document exists and is named like one, and
    // reading its text is what fails.
    write_bytes(project.path(), "knowledge/conventions/fixture-unreadable.md", &[0xff, 0xfe, 0x00]);

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::DocumentUnreadable { document, detail }) = result else {
        panic!("a document that cannot be read is one of `AC-07`'s fail-closed conditions");
    };
    assert_eq!(
        document.to_string(),
        "knowledge/conventions/fixture-unreadable.md",
        "the failure names the document that could not be read"
    );
    assert!(!detail.to_string().is_empty(), "the diagnostic the filesystem gave is carried along");
}

#[test]
fn test_scan_convention_requirements_hands_back_no_documents_when_one_is_unreadable() {
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    write_bytes(project.path(), "knowledge/conventions/fixture-binary.md", &[0xff]);

    // The tree holds a readable document alongside the failing one, and the
    // walk may meet the two in either order. The return type is a
    // `Result<Vec<_>, _>` and not a pair, so whichever it reaches first there is
    // no state in which the caller proceeds on the documents read so far.
    assert!(
        scan_convention_requirements(project.path()).is_err(),
        "an unreadable document fails the whole scan rather than dropping itself from it"
    );
}

#[test]
fn test_scan_convention_requirements_surfaces_a_front_matter_failure_from_its_own_document() {
    let project = project_with(&[
        ("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT),
        ("knowledge/conventions/broken.md", "---\nrequired_for: [unclosed\n---\n"),
    ]);

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::FrontMatterUnparseable { document, .. }) = result else {
        panic!("the codec's decision reaches the caller through the scan");
    };
    assert_eq!(
        document.to_string(),
        "knowledge/conventions/broken.md",
        "the failure is attributed to the document that carries the front matter, not to the \
         document the walk started from"
    );
}

#[test]
fn test_scan_convention_requirements_surfaces_a_required_for_shape_failure() {
    let project = project_with(&[(
        "knowledge/conventions/shape.md",
        "---\nrequired_for: implementer\n---\n",
    )]);

    let result = scan_convention_requirements(project.path());

    assert!(
        matches!(result, Err(ConventionResolveError::RequiredForNotStringArray { .. })),
        "a `required_for` that is not an array of strings fails the scan closed"
    );
}

#[test]
fn test_scan_convention_requirements_turns_the_decoded_view_into_a_requirement() {
    // `into_requirement` is called here rather than inside the codec, so a
    // blank identifier is decided while the scan is building the requirement.
    let project = project_with(&[(
        "knowledge/conventions/blank.md",
        "---\nrequired_for:\n  - \"  \"\n---\n",
    )]);

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::EmptyCapabilityId { document }) = result else {
        panic!("a blank capability id fails the scan closed");
    };
    assert_eq!(document.to_string(), "knowledge/conventions/blank.md");
}

#[test]
fn test_scan_convention_requirements_leaves_the_convention_tree_untouched() {
    let project = project_with(&[
        ("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT),
        ("knowledge/conventions/fixture-sub/fixture-beta.md", NAMING_DOCUMENT),
        ("knowledge/conventions/fixture-readme.md", UNDECLARED_DOCUMENT),
    ]);
    let before = tree_snapshot(project.path());

    scan_convention_requirements(project.path()).unwrap();

    assert_eq!(
        tree_snapshot(project.path()),
        before,
        "`AC-06` makes resolution read-only: no document is created, updated, or deleted, and no \
         index is generated"
    );
}

#[test]
fn test_scan_convention_requirements_produces_only_paths_inside_the_convention_root() {
    // Names picked to look like an escape without being one: a leading pair of
    // dots is an ordinary file name, and a directory listing has no way to
    // present the parent-directory component that would actually leave the
    // tree. The escape the path rule rejects is therefore not something any of
    // these documents can make the walk build. They say nothing about the
    // rule's other rejection, an unrenderable name, which the walk does reach —
    // see
    // `test_scan_convention_requirements_with_an_unrenderable_document_name_fails_closed`.
    let project = project_with(&[
        ("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT),
        ("knowledge/conventions/..fixture-hidden.md", NAMING_DOCUMENT),
        ("knowledge/conventions/fixture-sub/..fixture-deep.md", NAMING_DOCUMENT),
        ("knowledge/conventions/fixture-sub/deeper/fixture-deepest.md", NAMING_DOCUMENT),
    ]);

    let scanned = scan_convention_requirements(project.path()).unwrap();

    assert_eq!(
        sorted_documents(&scanned),
        [
            "knowledge/conventions/..fixture-hidden.md",
            "knowledge/conventions/fixture-alpha.md",
            "knowledge/conventions/fixture-sub/..fixture-deep.md",
            "knowledge/conventions/fixture-sub/deeper/fixture-deepest.md",
        ],
        "every candidate the walk builds is a path already accepted as being inside the root \
         joined with one entry name, so each document it produces is inside that root however \
         deep it sits and whatever it is called"
    );
}

#[cfg(unix)]
#[test]
fn test_scan_convention_requirements_ignores_a_non_document_whose_name_is_not_renderable() {
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    // An ordinary directory entry the walk ignores for its extension, named
    // with a line terminator so that `ConventionDocumentPath::try_new` refuses
    // it. Validating a candidate before deciding it is a document at all would
    // turn this neighbour into a scan-wide failure.
    write_bytes(
        project.path(),
        "knowledge/conventions/notes\n.txt",
        b"not a convention document\n",
    );

    let scanned = scan_convention_requirements(project.path()).unwrap();

    assert_eq!(
        sorted_documents(&scanned),
        ["knowledge/conventions/fixture-alpha.md"],
        "a neighbour that is not a convention document is ignored for its extension whatever it \
         is called, so a file the walk was never going to read cannot cost the consumer every \
         convention they have"
    );
}

#[cfg(unix)]
#[test]
fn test_scan_convention_requirements_with_an_unrenderable_document_name_fails_closed() {
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    // A document the walk does mean to present, whose name holds a line
    // terminator. Unlike the neighbour above this one cannot be ignored: it is
    // a convention document, and the scan has no way to name it in a result.
    write_bytes(project.path(), "knowledge/conventions/two\nlines.md", NAMING_DOCUMENT.as_bytes());

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::DocumentPathRejected { source }) = result else {
        panic!("a document the walk cannot name is one it cannot present");
    };
    let ConventionDocumentPathError::NotRenderableAsRecord { path } = source else {
        panic!("the rejection lifted is the renderability one and not the path escape: {source}");
    };
    assert_eq!(
        path,
        PathBuf::from("knowledge/conventions/two\nlines.md"),
        "and it carries the candidate the walk built, so the failure says which entry it was"
    );
}

#[cfg(unix)]
#[test]
fn test_scan_convention_requirements_with_an_unrenderable_directory_name_fails_closed() {
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    // The directory is not what fails: it is never returned, and the walk does
    // not ask the path rule about it. What fails is `nested.md`, whose own
    // complete path carries the directory's name and so cannot be rendered as a
    // record. Dropping it instead would lose a real convention document
    // silently.
    write_bytes(
        project.path(),
        "knowledge/conventions/two\nlines/nested.md",
        NAMING_DOCUMENT.as_bytes(),
    );

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::DocumentPathRejected { source }) = result else {
        panic!("a document whose path no reader could attribute fails the scan closed");
    };
    let ConventionDocumentPathError::NotRenderableAsRecord { path } = source else {
        panic!("for the renderability rejection rather than the path escape: {source}");
    };
    assert_eq!(
        path,
        PathBuf::from("knowledge/conventions/two\nlines/nested.md"),
        "and the rejected path is the document's complete one, naming the directory as a \
         component rather than as the entry that failed"
    );
}

#[cfg(unix)]
#[test]
fn test_scan_convention_requirements_ignores_an_unrenderable_directory_holding_no_document() {
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    // The same unrepresentable directory name, over a subtree with no `*.md`
    // in it. There is no document path for the rule to be about, so there is
    // nothing to fail on, and a consumer does not lose every convention they
    // have over a stray directory the scan would have read nothing from.
    write_bytes(
        project.path(),
        "knowledge/conventions/two\nlines/README.txt",
        b"not a convention document\n",
    );

    let scanned = scan_convention_requirements(project.path()).unwrap();

    assert_eq!(
        sorted_documents(&scanned),
        ["knowledge/conventions/fixture-alpha.md"],
        "an unrepresentable directory is walked rather than refused, and costs the consumer \
         their conventions only when it actually holds a document"
    );
}

#[cfg(unix)]
#[test]
fn test_scan_convention_requirements_does_not_read_a_linked_document() {
    // Reading through the link would carry `knowledge/conventions/linked.md`, a
    // path the rule accepts, for declarations that came from a file outside the
    // tree entirely. Passing over it instead would hand back a requirement set
    // missing a document that exists, which is the same wrong answer one level
    // down from a linked root: the walk declined to read it, so it cannot
    // report the tree as not having it.
    let project = project_with(&[
        ("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT),
        ("outside/smuggled.md", NAMING_DOCUMENT),
    ]);
    std::os::unix::fs::symlink(
        project.path().join("outside/smuggled.md"),
        project.path().join("knowledge/conventions/linked.md"),
    )
    .unwrap();

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::DocumentUnreadable { document, detail }) = result else {
        panic!(
            "a document the walk refuses to follow is one it cannot read, not one that is absent"
        );
    };
    assert_eq!(document.to_string(), "knowledge/conventions/linked.md");
    assert!(detail.to_string().contains("symbolic link"), "and says why: {detail}");
}

#[cfg(unix)]
#[test]
fn test_scan_convention_requirements_fails_closed_on_a_broken_document_link() {
    // A link is refused for being a link, before anything is read through it,
    // so where it points never comes into it. That the target is missing is not
    // evidence the document is: nothing was looked at.
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    std::os::unix::fs::symlink(
        project.path().join("absent/target.md"),
        project.path().join("knowledge/conventions/dangling.md"),
    )
    .unwrap();

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::DocumentUnreadable { document, .. }) = result else {
        panic!("a link is refused on being a link, whether or not it points at anything");
    };
    assert_eq!(document.to_string(), "knowledge/conventions/dangling.md");
}

#[cfg(unix)]
#[test]
fn test_scan_convention_requirements_does_not_list_a_linked_convention_root() {
    // The root is reached by descending to it, not by being listed as an entry
    // of something, so the refusal that covers entries never sees it. Its own
    // open is what has to refuse: a listing of the link would hand back another
    // tree's entries, which the walk would then name `knowledge/conventions/...`
    // because that is the path it built them from.
    let project = project_with(&[("outside/conventions/smuggled.md", ADR_DOCUMENT)]);
    std::fs::create_dir_all(project.path().join("knowledge")).unwrap();
    std::os::unix::fs::symlink(
        project.path().join("outside/conventions"),
        project.path().join("knowledge/conventions"),
    )
    .unwrap();

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::ConventionRootUnlistable { root, .. }) = result else {
        panic!(
            "a linked convention root neither presents another tree's documents under this tree's \
             paths nor presents none: the documents exist, in the tree the link points at, and \
             refusing to read that tree leaves this one undecided rather than empty"
        );
    };
    assert_eq!(root, PathBuf::from("knowledge/conventions"));
}

#[cfg(unix)]
#[test]
fn test_scan_convention_requirements_does_not_list_a_convention_root_below_a_linked_directory() {
    // The same escape one component higher: the walk descends through
    // `knowledge` too, so a link there redirects the root just as effectively
    // as a link at the root.
    let project = project_with(&[("outside/knowledge/conventions/smuggled.md", ADR_DOCUMENT)]);
    std::os::unix::fs::symlink(
        project.path().join("outside/knowledge"),
        project.path().join("knowledge"),
    )
    .unwrap();

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::ConventionRootUnlistable { root, .. }) = result else {
        panic!(
            "every component the walk descends through is refused on its own open, not only the \
             root itself, and a refusal one component higher is the same undecided tree"
        );
    };
    assert_eq!(root, PathBuf::from("knowledge/conventions"));
}

#[test]
fn test_scan_convention_requirements_with_a_project_root_that_does_not_exist_fails_closed() {
    // `project_root` arrives from the caller — `--project-root` hands it over
    // directly — so a misspelling reaches this walk as an ordinary path. If the
    // failure to open it were folded in with an absent convention root, the
    // scan would answer that a repository nobody opened requires nothing, and
    // dispatch would proceed on it. Absence is only an answer about a
    // repository that was opened.
    let parent = tempfile::tempdir().expect("tempdir must be created");

    let result = scan_convention_requirements(&parent.path().join("no-such-repository"));

    let Err(ConventionResolveError::ConventionRootUnlistable { root, .. }) = result else {
        panic!(
            "a project root that could not be opened leaves the repository unread, not empty: \
             nothing was looked at to declare anything"
        );
    };
    assert_eq!(root, PathBuf::from("knowledge/conventions"));
}

#[cfg(unix)]
#[test]
fn test_scan_convention_requirements_with_a_project_root_link_to_nothing_fails_closed() {
    // The same claim through the anchor's own resolution rather than a plain
    // missing directory: the anchor is opened as the caller handed it over, so
    // it follows a link, and a link to nothing reports the repository absent by
    // the same `NotFound` an absent convention root would.
    let parent = tempfile::tempdir().expect("tempdir must be created");
    let project_root = parent.path().join("repository");
    std::os::unix::fs::symlink(parent.path().join("nowhere"), &project_root)
        .expect("the dangling project-root link must be created");

    let result = scan_convention_requirements(&project_root);

    let Err(ConventionResolveError::ConventionRootUnlistable { root, .. }) = result else {
        panic!("a project root that points at nothing was never opened, so nothing is empty yet");
    };
    assert_eq!(root, PathBuf::from("knowledge/conventions"));
}

#[cfg(unix)]
#[test]
fn test_scan_convention_requirements_does_not_read_a_dangling_ancestor_link_as_an_absent_root() {
    // `knowledge` is a link to nothing, so resolving `knowledge/conventions` as
    // a pathname reports the root absent — and absence is the one condition
    // this scan answers with an empty result. The two are not the same: nothing
    // has said what is behind the link, and this fixture pins that the refusal
    // comes from the open of the component itself rather than from a check made
    // before it. A descent that resolved the path again, however carefully it
    // had checked first, would hand back `NotFound` here.
    let project = tempfile::tempdir().expect("tempdir must be created");
    std::os::unix::fs::symlink(project.path().join("nowhere"), project.path().join("knowledge"))
        .expect("the dangling ancestor link must be created");

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::ConventionRootUnlistable { root, .. }) = result else {
        panic!(
            "a root reached through a link that points at nothing is undecided, not absent: the \
             tree behind the link was never looked at"
        );
    };
    assert_eq!(root, PathBuf::from("knowledge/conventions"));
}

#[cfg(unix)]
#[test]
fn test_scan_convention_requirements_does_not_read_a_node_that_is_not_a_regular_file() {
    // A socket stands in for the node type that matters most here, a FIFO:
    // both are named like a document and are neither a directory nor a link,
    // and opening a FIFO blocks until something writes to it. Excluding them
    // by node type before the read is what keeps a walk of a consumer's tree
    // from stopping on one.
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    let listener =
        std::os::unix::net::UnixListener::bind(project.path().join("knowledge/conventions/odd.md"))
            .unwrap();

    let scanned = scan_convention_requirements(project.path()).unwrap();

    assert_eq!(
        sorted_documents(&scanned),
        ["knowledge/conventions/fixture-alpha.md"],
        "a node that is not a regular file is not a document the walk presents, and reaching it \
         neither fails the scan nor blocks it"
    );
    drop(listener);
}

#[test]
fn test_scan_convention_requirements_with_an_oversized_document_fails_closed() {
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    let oversized = usize::try_from(super::document_read::MAX_DOCUMENT_BYTES).unwrap() + 1;
    write_bytes(project.path(), "knowledge/conventions/huge.md", &vec![b'x'; oversized]);

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::DocumentUnreadable { document, detail }) = result else {
        panic!("a document the walk will not read whole is a document it cannot read");
    };
    assert_eq!(
        document.to_string(),
        "knowledge/conventions/huge.md",
        "the bound reports as the same condition as any other failed read, naming the document \
         it stopped on rather than failing the scan anonymously"
    );
    assert!(
        detail.to_string().contains("read bound"),
        "and says the read was bounded rather than that the filesystem refused: {detail}"
    );
}

#[test]
fn test_scan_convention_requirements_reads_a_document_at_the_read_bound() {
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    let mut at_bound = ADR_DOCUMENT.as_bytes().to_vec();
    at_bound.resize(usize::try_from(super::document_read::MAX_DOCUMENT_BYTES).unwrap(), b'x');
    write_bytes(project.path(), "knowledge/conventions/large.md", &at_bound);

    let scanned = scan_convention_requirements(project.path()).unwrap();

    assert_eq!(
        sorted_summaries(&scanned),
        [
            "knowledge/conventions/fixture-alpha.md -> implementer, reviewer",
            "knowledge/conventions/large.md -> implementer, reviewer",
        ],
        "a document exactly at the bound is read whole and decoded, so the bound rejects only \
         what is past it rather than truncating what reaches it"
    );
}

#[test]
fn test_scan_convention_requirements_with_an_oversized_subdirectory_fails_closed() {
    let project = project_with(&[
        ("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT),
        ("knowledge/conventions/fixture-sub/fixture-beta.md", NAMING_DOCUMENT),
    ]);
    let crowded = project.path().join("knowledge/conventions/fixture-sub");
    for index in 0..=super::directory_walk::MAX_DIRECTORY_ENTRIES {
        std::fs::write(crowded.join(format!("{index}.md")), NAMING_DOCUMENT).unwrap();
    }

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::DocumentUnreadable { document, detail }) = result else {
        panic!("a listing the walk will not hold whole is a directory it cannot read");
    };
    assert_eq!(
        document.to_string(),
        "knowledge/conventions/fixture-sub",
        "the bound names the directory it stopped on, which the walk can do below the root \
         because everything below it is a path `ConventionDocumentPath` accepts"
    );
    assert!(detail.to_string().contains("entries"), "and says what it stopped on: {detail}");
}

#[test]
fn test_scan_convention_requirements_below_the_depth_bound_fails_closed() {
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    let deep = "deep/".repeat(super::MAX_DIRECTORY_DEPTH + 1);
    write_bytes(
        project.path(),
        &format!("knowledge/conventions/{deep}buried.md"),
        NAMING_DOCUMENT.as_bytes(),
    );

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::DocumentUnreadable { document, detail }) = result else {
        panic!("a tree the walk cannot descend without risking the stack is one it cannot read");
    };
    assert!(
        document.to_string().starts_with("knowledge/conventions/deep"),
        "the failure names the directory the walk stopped at: {document}"
    );
    assert!(detail.to_string().contains("nested deeper"), "and says why: {detail}");
}

#[test]
fn test_scan_convention_requirements_within_the_depth_bound_reads_the_document() {
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    // One level shy of the bound, so the deepest directory the walk may enter
    // is entered rather than refused.
    let deep = "deep/".repeat(super::MAX_DIRECTORY_DEPTH - 1);
    write_bytes(
        project.path(),
        &format!("knowledge/conventions/{deep}buried.md"),
        NAMING_DOCUMENT.as_bytes(),
    );

    let scanned = scan_convention_requirements(project.path()).unwrap();

    assert_eq!(
        sorted_documents(&scanned),
        [
            &format!("knowledge/conventions/{deep}buried.md"),
            "knowledge/conventions/fixture-alpha.md"
        ],
        "the bound refuses only what is past it, so nesting up to it is scanned normally"
    );
}

#[cfg(unix)]
#[test]
fn test_scan_convention_requirements_does_not_follow_a_directory_symlink() {
    // Following the link would produce `knowledge/conventions/linked/smuggled.md`
    // for a file that lives outside the tree, which is the escape the path rule
    // refuses. Walking past it would be the other wrong answer: a linked
    // subdirectory can hold conventions, and a scan that succeeds without them
    // lets dispatch proceed under policies nobody read.
    let project = project_with(&[
        ("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT),
        ("outside/smuggled.md", NAMING_DOCUMENT),
    ]);
    std::os::unix::fs::symlink(
        project.path().join("outside"),
        project.path().join("knowledge/conventions/linked"),
    )
    .unwrap();

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::DocumentUnreadable { document, .. }) = result else {
        panic!("a subtree the walk refuses to enter is not a subtree it read and found empty");
    };
    assert_eq!(document.to_string(), "knowledge/conventions/linked");
}

#[test]
fn test_scan_convention_requirements_beyond_the_whole_scan_entry_budget_fails_closed() {
    // The per-directory bound renews at every directory, so a tree can stay
    // inside it at every node and still be arbitrarily large. Spread just past
    // the whole-scan budget over several directories, none of them near the
    // per-directory bound, so what fails here can only be the aggregate one.
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    let per_directory = super::MAX_SCAN_ENTRIES / 4;
    for directory in 0..4 {
        let dir = project.path().join(format!("knowledge/conventions/bulk-{directory}"));
        std::fs::create_dir_all(&dir).expect("the bulk directory must be created");
        for entry in 0..per_directory {
            std::fs::write(dir.join(format!("filler-{entry}.txt")), b"")
                .expect("the filler entry must be written");
        }
    }

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::ConventionRootUnlistable { root, detail }) = result else {
        panic!(
            "a tree larger than this walk will examine is one it did not read, and reporting what \
             it managed to reach would present a partial requirement set as the whole"
        );
    };
    assert_eq!(root, PathBuf::from("knowledge/conventions"));
    assert!(detail.to_string().contains("entries"), "and says why: {detail}");
}

#[test]
fn test_bounded_entries_charges_the_budget_for_every_entry_it_produces() {
    // Where the budget is charged is the whole of what it bounds, and the two
    // candidate points are not equivalent. A listing is read, classified, and
    // held in full before its caller walks any of it, so charging as entries
    // are walked lets a chain of large directories materialise every one of
    // them while spending one unit per directory descended into. Charging as
    // entries are produced is what makes the number the walk advertises the
    // number it enforces.
    //
    // Asserted against the listing rather than through a scan because the two
    // charging points do not differ in what a completed scan returns — a scan
    // that finishes has walked exactly what it produced. What they differ in is
    // how much is held before the same failure, which is visible here and
    // nowhere above.
    let directory = tempfile::tempdir().expect("tempdir must be created");
    for entry in 0..5 {
        std::fs::write(directory.path().join(format!("entry-{entry}.md")), b"")
            .expect("the entry must be written");
    }
    let handle =
        super::directory_walk::open_trusted_root(directory.path()).expect("the tempdir must open");

    let mut remaining = 100;
    let listed = super::directory_walk::bounded_entries(&handle, &mut remaining)
        .unwrap_or_else(|_| panic!("a listing inside the budget must succeed"));

    assert_eq!(listed.len(), 5);
    assert_eq!(remaining, 95, "one unit per entry produced, and none for `.` or `..`");
}

#[test]
fn test_bounded_entries_refuses_a_listing_larger_than_the_budget_left() {
    // The budget runs out part-way through producing this listing, so the
    // refusal comes from the listing itself rather than from whoever would have
    // walked it. Nothing is handed back: a partial listing presented as a whole
    // one is the same wrong answer as an empty tree presented for an unread
    // one.
    let directory = tempfile::tempdir().expect("tempdir must be created");
    for entry in 0..5 {
        std::fs::write(directory.path().join(format!("entry-{entry}.md")), b"")
            .expect("the entry must be written");
    }
    let handle =
        super::directory_walk::open_trusted_root(directory.path()).expect("the tempdir must open");

    let mut remaining = 3;
    let refused = super::directory_walk::bounded_entries(&handle, &mut remaining);

    assert!(
        matches!(refused, Err(super::ListingError::BudgetExhausted)),
        "a listing that cannot be produced inside the budget is refused as the budget running \
         out, not as an I/O failure of the directory"
    );
}

#[test]
fn test_scan_convention_requirements_within_the_whole_scan_entry_budget_reads_the_documents() {
    // The budget refuses only what is past it. A tree comfortably inside it is
    // scanned normally, so the bound cannot be satisfied by a walk that stops
    // early.
    let project = project_with(&[
        ("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT),
        ("knowledge/conventions/fixture-sub/fixture-beta.md", NAMING_DOCUMENT),
    ]);

    let scanned = scan_convention_requirements(project.path()).unwrap();

    assert_eq!(
        sorted_documents(&scanned),
        [
            "knowledge/conventions/fixture-alpha.md",
            "knowledge/conventions/fixture-sub/fixture-beta.md",
        ]
    );
}

#[test]
fn test_fs_convention_requirement_adapter_scans_the_tree_through_the_port() {
    let project = project_with(&[
        ("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT),
        ("knowledge/conventions/fixture-sub/fixture-beta.md", NAMING_DOCUMENT),
    ]);
    let port: &dyn ConventionRequirementPort = &FsConventionRequirementAdapter::new();

    let scanned = port.scan_requirements(project.path()).unwrap();

    assert_eq!(
        sorted_summaries(&scanned),
        [
            "knowledge/conventions/fixture-alpha.md -> implementer, reviewer",
            "knowledge/conventions/fixture-sub/fixture-beta.md -> implementer",
        ],
        "the port call reads the documents actually on disk, so injecting the adapter is what \
         gives the caller a real scan"
    );
}

#[test]
fn test_fs_convention_requirement_adapter_surfaces_every_fail_closed_condition_through_the_port() {
    /// Whether a surfaced failure is the condition its fixture was built to
    /// produce.
    type ConditionCheck = fn(&ConventionResolveError) -> bool;

    // `AC-07` names five conditions. Four of them are things a document can be,
    // and each is exercised below through the port against a real tree holding
    // such a document.
    //
    // The fifth — a resolved path the document path rule refuses — is not in
    // the table because its fixture is a file *name* rather than a document
    // text, and because the rule refuses two different things. A path leaving
    // `knowledge/conventions/` this adapter does not present: the scan behind
    // the port builds every candidate by joining one name taken from a
    // directory listing onto a path `ConventionDocumentPath::try_new` has
    // already accepted, starting at the convention root itself, and a directory
    // entry name is never empty, is never `.` nor `..`, and carries neither a
    // separator nor a root or prefix component, so the joined path lies inside
    // the root whenever the path it extends does. What the implementation owes
    // that half is prevention rather than rejection, which the sibling test
    // named
    // `test_fs_convention_requirement_adapter_prevents_the_path_escape_condition_through_the_port`
    // asserts.
    //
    // A path that does not render as one record the adapter does present, since
    // a directory entry may be named with a line terminator, and that half is
    // covered against a real tree by
    // `test_scan_convention_requirements_with_an_unrenderable_document_name_fails_closed`.
    let document_shaped: [(&str, &str, ConditionCheck); 3] = [
        (
            "knowledge/conventions/unparseable.md",
            "---\nrequired_for: [implementer\n---\n",
            |error| matches!(error, ConventionResolveError::FrontMatterUnparseable { .. }),
        ),
        ("knowledge/conventions/shape.md", "---\nrequired_for: implementer\n---\n", |error| {
            matches!(error, ConventionResolveError::RequiredForNotStringArray { .. })
        }),
        ("knowledge/conventions/blank.md", "---\nrequired_for:\n  - \"  \"\n---\n", |error| {
            matches!(error, ConventionResolveError::EmptyCapabilityId { .. })
        }),
    ];
    let port: &dyn ConventionRequirementPort = &FsConventionRequirementAdapter::new();

    for (relative, content, is_expected_condition) in document_shaped {
        let project = project_with(&[(relative, content)]);

        let Err(surfaced) = port.scan_requirements(project.path()) else {
            panic!("'{relative}' meets one of `AC-07`'s conditions, so the port call fails closed");
        };

        assert!(
            is_expected_condition(&surfaced),
            "the condition reaches the port's caller as itself rather than as a substitute \
             error: {surfaced}"
        );
        assert!(
            surfaced.to_string().contains(relative),
            "and it still names the document that carries it: {surfaced}"
        );
    }

    // The fourth condition, an unreadable document, is a fixture the tree has
    // to hold rather than a document text, so it is built here instead of in
    // the table above.
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    write_bytes(project.path(), "knowledge/conventions/fixture-unreadable.md", &[0xff, 0xfe, 0x00]);

    let result = port.scan_requirements(project.path());

    let Err(ConventionResolveError::DocumentUnreadable { document, .. }) = result else {
        panic!("the adapter surfaces the scan's fail-closed condition as itself");
    };
    assert_eq!(
        document.to_string(),
        "knowledge/conventions/fixture-unreadable.md",
        "the condition reaches the port's caller unchanged rather than as a substitute error"
    );
}

#[test]
fn test_fs_convention_requirement_adapter_prevents_the_path_escape_condition_through_the_port() {
    // The escape half of `AC-07`'s fifth condition — a resolved path leaving
    // `knowledge/conventions/` — cannot be surfaced through this port, because
    // nothing produces it: every candidate path is one name from a directory
    // listing joined onto a path `ConventionDocumentPath::try_new` has already
    // accepted, and such a name is never empty, is never `.` nor `..`, and
    // carries neither a separator nor a root or prefix component. What this
    // implementation owes that half is prevention rather than rejection, and
    // prevention is what this asserts.
    //
    // This is a claim about the escape rejection alone and not about
    // `DocumentPathRejected` as a whole: the constructor's other rejection, a
    // path that does not render as one record, this walk does reach, and
    // `test_scan_convention_requirements_with_an_unrenderable_document_name_fails_closed`
    // covers it. The names planted below are ordinary ones chosen to look like
    // escapes, so none of them meets that other rejection either.
    //
    // The assertion has teeth because both ways of failing it are observable.
    // Had the walk built a candidate outside the root, `try_new` would have
    // rejected it and this call would have answered `DocumentPathRejected`
    // instead of returning any documents at all; had it climbed out of the
    // subtree, the two documents planted outside the convention root below
    // would be in the answer. The tree also puts on the path rule every
    // pressure a real one can — names that read as escapes but are ordinary
    // file names, and nesting several levels deep.
    let project = project_with(&[
        ("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT),
        ("knowledge/conventions/..fixture-hidden.md", NAMING_DOCUMENT),
        ("knowledge/conventions/fixture-sub/..fixture-deep.md", NAMING_DOCUMENT),
        ("knowledge/conventions/fixture-sub/deeper/fixture-deepest.md", NAMING_DOCUMENT),
        ("knowledge/adr/README.md", NAMING_DOCUMENT),
        ("outside/smuggled.md", NAMING_DOCUMENT),
    ]);
    let port: &dyn ConventionRequirementPort = &FsConventionRequirementAdapter::new();

    let scanned = port.scan_requirements(project.path()).unwrap_or_else(|error| {
        panic!("no name this tree holds can make the walk leave the convention root: {error}")
    });

    assert_eq!(
        sorted_documents(&scanned),
        [
            "knowledge/conventions/..fixture-hidden.md",
            "knowledge/conventions/fixture-alpha.md",
            "knowledge/conventions/fixture-sub/..fixture-deep.md",
            "knowledge/conventions/fixture-sub/deeper/fixture-deepest.md",
        ],
        "the port answers with the documents inside the convention root and with nothing from \
         outside it"
    );
    for requirement in &scanned {
        assert!(
            requirement.document().as_path().starts_with("knowledge/conventions"),
            "every path the port hands back stays inside the convention root: {}",
            requirement.document()
        );
    }
}

#[test]
fn test_fs_convention_requirement_adapter_scans_each_supplied_root_through_one_shared_receiver() {
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    let other = project_with(&[("knowledge/conventions/fixture-other.md", NAMING_DOCUMENT)]);
    let adapter = FsConventionRequirementAdapter::new();
    let shared: &dyn ConventionRequirementPort = &adapter;

    // Two concurrent scans of two different roots through one shared receiver:
    // the adapter holds no state and reads no ambient location, which is what
    // makes it safe to wire once into a composition root.
    let (first, second) = thread::scope(|scope| {
        let left = scope.spawn(|| shared.scan_requirements(project.path()));
        let right = scope.spawn(|| shared.scan_requirements(other.path()));
        (
            left.join().expect("the scan thread does not panic").unwrap(),
            right.join().expect("the scan thread does not panic").unwrap(),
        )
    });

    assert_eq!(sorted_documents(&first), ["knowledge/conventions/fixture-alpha.md"]);
    assert_eq!(sorted_documents(&second), ["knowledge/conventions/fixture-other.md"]);
}

#[cfg(unix)]
#[test]
fn test_fs_convention_requirement_adapter_surfaces_the_unrenderable_name_condition_through_the_port()
 {
    // The sixth `AC-07` condition, exercised through the port rather than
    // through the free function. It is absent from the table above because its
    // fixture is a file *name* and not a document text, so it needs a tree the
    // table's shape cannot build — but a caller holding only the port must
    // still see it fail closed, and referring to a sibling test of the scan
    // does not establish that for this implementation.
    let project = project_with(&[("knowledge/conventions/fixture-alpha.md", ADR_DOCUMENT)]);
    write_bytes(project.path(), "knowledge/conventions/two\nlines.md", NAMING_DOCUMENT.as_bytes());
    let port: &dyn ConventionRequirementPort = &FsConventionRequirementAdapter::new();

    let result = port.scan_requirements(project.path());

    let Err(ConventionResolveError::DocumentPathRejected { source }) = result else {
        panic!("a document the scan cannot name is one the port cannot present");
    };
    assert!(
        matches!(source, ConventionDocumentPathError::NotRenderableAsRecord { .. }),
        "the rejection that crosses the port is the renderability one, unchanged: {source}"
    );
}

#[test]
fn test_scan_convention_requirements_with_an_unlistable_root_fails_closed() {
    // `knowledge/conventions` present but not a directory. A caller cannot tell
    // this apart from a repository declaring nothing if the scan answers with an
    // empty result, and the two mean opposite things: one is "no document
    // requires this capability", the other is "the documents could not be looked
    // at". The root is the one path the walk touches that `ConventionDocumentPath`
    // rejects by design, so no document-shaped condition can name it.
    let project = tempfile::tempdir().expect("tempdir must be created");
    std::fs::create_dir_all(project.path().join("knowledge")).expect("knowledge must be created");
    std::fs::write(project.path().join("knowledge/conventions"), b"not a directory")
        .expect("the root stand-in must be written");

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::ConventionRootUnlistable { root, .. }) = result else {
        panic!(
            "a convention root that cannot be listed is a structural anomaly, not an empty result"
        );
    };
    assert_eq!(
        root,
        PathBuf::from("knowledge/conventions"),
        "the failure names the root it could not list, repository-relative"
    );
}

#[test]
fn test_scan_convention_requirements_without_a_convention_root_is_an_empty_result() {
    // The absent root stays an ordinary empty result: a repository that keeps no
    // conventions declares nothing, which `AC-08` makes normal. Only a root that
    // exists and cannot be listed fails closed.
    let project = tempfile::tempdir().expect("tempdir must be created");

    let scanned = scan_convention_requirements(project.path())
        .expect("a repository with no convention tree requires nothing");

    assert!(scanned.is_empty());
}

#[test]
fn test_scan_convention_requirements_with_an_untraversable_ancestor_fails_closed() {
    // `knowledge` is a regular file, so the descent stops one component above
    // the root: the open of `knowledge` fails for not being a directory rather
    // than for being a link, and the root below it is never reached. That is a
    // different arm from the fixture that makes the *root* a file, which gets
    // as far as opening `knowledge` and fails one level lower.
    let project = tempfile::tempdir().expect("tempdir must be created");
    std::fs::write(project.path().join("knowledge"), b"not a directory")
        .expect("the ancestor stand-in must be written");

    let result = scan_convention_requirements(project.path());

    let Err(ConventionResolveError::ConventionRootUnlistable { root, .. }) = result else {
        panic!("an ancestor that cannot be traversed leaves the tree undecidable, not empty");
    };
    assert_eq!(root, PathBuf::from("knowledge/conventions"));
}
