//! Tests for the convention-shipping tree inventory.
//!
//! Kept in a sibling module so that only the production half adds to the parent
//! module's length. Every fixture is a real tempdir tree rather than a fake
//! port, because what is under test here is the filesystem behaviour itself.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use usecase::conventions_resolve::{ConventionDocumentPath, ConventionDocumentPathError};
use usecase::template_conventions::ConventionShippingCheckError;

use super::list_convention_documents;

/// A convention document with front matter, as the overlay ships them.
const OVERLAY_DOCUMENT: &str = "---\nrequired_for:\n  - implementer\n---\n\n# Coding principles\n";

/// Builds a tree from `files`, given as tree-relative path and content pairs.
fn tree_with(files: &[(&str, &str)]) -> TempDir {
    let tree = TempDir::new().unwrap();
    for (relative, content) in files {
        write_bytes(tree.path(), relative, content.as_bytes());
    }
    tree
}

fn write_bytes(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// Renders the inventory as paths in name order.
///
/// Sorted here rather than taken as the walk produced it, because the order a
/// listing yields is not something this walk promises: `select_unsupplied_
/// conventions` is what commits to an order. Comparing against a sorted
/// rendering keeps these assertions about *which* documents were inventoried.
fn sorted_paths(documents: &[ConventionDocumentPath]) -> Vec<String> {
    let mut listed: Vec<String> = documents.iter().map(ToString::to_string).collect();
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

#[test]
fn test_list_convention_documents_names_every_document_an_exported_tree_ships() {
    // The shape an export produces: the overlay's documents under
    // `knowledge/conventions/`, one of them nested.
    let exported = tree_with(&[
        ("knowledge/conventions/README.md", OVERLAY_DOCUMENT),
        ("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT),
        ("knowledge/conventions/nested/testing.md", OVERLAY_DOCUMENT),
    ]);

    let inventoried = list_convention_documents(exported.path()).unwrap();

    // The fixture root is an arbitrary absolute temporary directory, so an
    // identity carrying any part of it would show up in this comparison.
    assert_eq!(
        sorted_paths(&inventoried),
        [
            "knowledge/conventions/README.md",
            "knowledge/conventions/coding-principles.md",
            "knowledge/conventions/nested/testing.md",
        ],
        "every document the tree ships is inventoried once, nested ones included, by a path \
         relative to the tree rather than to the filesystem"
    );
}

#[test]
fn test_list_convention_documents_yields_identities_two_trees_can_be_compared_by() {
    // The two sides of the shipping comparison, as separate trees at unrelated
    // absolute locations. The check subtracts one inventory from the other, so
    // if a root leaked into the identities nothing would ever compare equal and
    // every shipped document would read as unsupplied.
    let exported = tree_with(&[
        ("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT),
        ("knowledge/conventions/leaked-source.md", OVERLAY_DOCUMENT),
    ]);
    let overlay = tree_with(&[("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT)]);

    let shipped = list_convention_documents(exported.path()).unwrap();
    let supplied = list_convention_documents(overlay.path()).unwrap();

    let unsupplied: Vec<&ConventionDocumentPath> =
        shipped.iter().filter(|document| !supplied.contains(document)).collect();
    assert_eq!(
        unsupplied.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["knowledge/conventions/leaked-source.md"],
        "the document both trees hold is one and the same value across the two inventories, so \
         subtracting one from the other names exactly the document the overlay does not supply"
    );
}

#[test]
fn test_list_convention_documents_inventories_the_tree_it_was_named_and_no_ambient_location() {
    let named = tree_with(&[("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT)]);
    let other = tree_with(&[("knowledge/conventions/security.md", OVERLAY_DOCUMENT)]);
    write_bytes(named.path(), "knowledge/adr/README.md", OVERLAY_DOCUMENT.as_bytes());
    write_bytes(named.path(), "docs/testing.md", OVERLAY_DOCUMENT.as_bytes());

    let inventoried = list_convention_documents(named.path()).unwrap();
    let inventoried_other = list_convention_documents(other.path()).unwrap();

    assert_eq!(
        sorted_paths(&inventoried),
        ["knowledge/conventions/coding-principles.md"],
        "a markdown file outside `knowledge/conventions/` is not a convention document the tree \
         ships"
    );
    assert_eq!(
        sorted_paths(&inventoried_other),
        ["knowledge/conventions/security.md"],
        "the tree inventoried is the one the caller named, so one port answers for the exported \
         tree and for the overlay without reading any ambient location"
    );
}

#[test]
fn test_list_convention_documents_inventories_only_markdown_documents() {
    let exported = tree_with(&[
        ("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT),
        ("knowledge/conventions/README.txt", "not a convention document\n"),
        ("knowledge/conventions/index.yaml", "required_for: 5\n"),
        ("knowledge/conventions/notes", "extensionless\n"),
    ]);

    let inventoried = list_convention_documents(exported.path()).unwrap();

    assert_eq!(
        sorted_paths(&inventoried),
        ["knowledge/conventions/coding-principles.md"],
        "a neighbouring file with another extension is not a convention document, so shipping one \
         is not a shipping violation"
    );
}

#[test]
fn test_list_convention_documents_inventories_a_document_it_could_not_have_parsed() {
    // Front matter that is not parseable YAML at all, plus a `required_for`
    // that is not an array of strings: both are refusals the resolver's scan
    // fails closed on. Reaching a verdict over this tree is what shows the
    // inventory never opened either file — a walk that decoded them would have
    // failed here instead of naming them.
    let exported = tree_with(&[
        ("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT),
        ("knowledge/conventions/malformed.md", "---\n\tname: tabbed\n  : :\n---\n\n# Broken\n"),
        ("knowledge/conventions/mistyped.md", "---\nrequired_for: 5\n---\n\n# Mistyped\n"),
        ("knowledge/conventions/unclosed.md", "---\nrequired_for:\n  - implementer\n"),
    ]);

    let inventoried = list_convention_documents(exported.path()).unwrap();

    assert_eq!(
        sorted_paths(&inventoried),
        [
            "knowledge/conventions/coding-principles.md",
            "knowledge/conventions/malformed.md",
            "knowledge/conventions/mistyped.md",
            "knowledge/conventions/unclosed.md",
        ],
        "a malformed document is a document the tree ships, so it is inventoried like any other: \
         no defect in a document's contents can turn the shipping question into a parse failure"
    );
}

#[test]
fn test_list_convention_documents_leaves_the_tree_untouched() {
    let exported = tree_with(&[
        ("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT),
        ("knowledge/conventions/nested/testing.md", OVERLAY_DOCUMENT),
    ]);
    let before = tree_snapshot(exported.path());

    list_convention_documents(exported.path()).unwrap();

    assert_eq!(
        tree_snapshot(exported.path()),
        before,
        "the inventory is read-only: it neither creates, updates, nor deletes anything in the \
         tree it was asked about"
    );
}

#[test]
fn test_list_convention_documents_without_a_convention_root_fails_closed() {
    // The shape an exported tree takes if the boundary classification is edited
    // away from `overlay`, which is the mutation this check exists to catch. An
    // empty inventory here would make that export pass vacuously.
    let exported = tree_with(&[("docs/testing.md", OVERLAY_DOCUMENT)]);

    let result = list_convention_documents(exported.path());

    let Err(ConventionShippingCheckError::ConventionRootMissing { tree_root }) = result else {
        panic!("a tree that holds no convention root at all fails rather than inventorying nothing")
    };
    assert_eq!(tree_root, exported.path(), "and names the tree that holds none");
}

#[cfg(unix)]
#[test]
fn test_list_convention_documents_does_not_report_a_linked_convention_root_as_missing() {
    // The root is reached by descending to it, so the refusal that covers
    // entries never sees it; its own open is what has to refuse. Reporting it as
    // missing would be worse here than in the resolver's scan, because a missing
    // root is this check's fail-closed condition and a link is a tree that was
    // never looked at — two different answers that must not collapse into one.
    let exported = tree_with(&[("outside/conventions/smuggled.md", OVERLAY_DOCUMENT)]);
    std::fs::create_dir_all(exported.path().join("knowledge")).unwrap();
    std::os::unix::fs::symlink(
        exported.path().join("outside/conventions"),
        exported.path().join("knowledge/conventions"),
    )
    .unwrap();

    let result = list_convention_documents(exported.path());

    let Err(ConventionShippingCheckError::TreeUnreadable { path, .. }) = result else {
        panic!(
            "a linked convention root neither presents another tree's documents under this tree's \
             paths nor is a tree that has no root: it is a tree this walk did not read"
        )
    };
    assert_eq!(path, exported.path().join("knowledge/conventions"));
}

#[cfg(unix)]
#[test]
fn test_list_convention_documents_does_not_read_a_dangling_ancestor_link_as_a_missing_root() {
    // `knowledge` is a link to nothing, so resolving `knowledge/conventions` as
    // a pathname would report the root absent — the one condition this walk
    // answers with `ConventionRootMissing`. This fixture pins that the refusal
    // comes from the open of the component itself under `NOFOLLOW`, which fails
    // on the link rather than on its missing target.
    let exported = TempDir::new().unwrap();
    std::os::unix::fs::symlink(exported.path().join("nowhere"), exported.path().join("knowledge"))
        .unwrap();

    let result = list_convention_documents(exported.path());

    let Err(ConventionShippingCheckError::TreeUnreadable { path, .. }) = result else {
        panic!(
            "a root reached through a link that points at nothing is unread, not absent: the tree \
             behind the link was never looked at"
        )
    };
    assert_eq!(path, exported.path().join("knowledge"), "and names the component it stopped at");
}

#[test]
fn test_list_convention_documents_with_a_tree_root_that_does_not_exist_fails_closed() {
    // The tree root arrives from the caller, so a misspelling reaches this walk
    // as an ordinary path. Folding its failure in with an absent convention root
    // would answer that a tree nobody opened ships nothing, and the shipping
    // check would pass on a path that does not exist.
    let parent = TempDir::new().unwrap();

    let result = list_convention_documents(&parent.path().join("no-such-tree"));

    let Err(ConventionShippingCheckError::TreeUnreadable { path, .. }) = result else {
        panic!("a tree root that could not be opened leaves the tree unread, not rootless")
    };
    assert_eq!(path, parent.path().join("no-such-tree"));
}

#[cfg(unix)]
#[test]
fn test_list_convention_documents_refuses_a_linked_document() {
    // Passing over the link would report a document the walk declined to read as
    // one the tree does not ship, and "does not ship" is this check's passing
    // answer — so a source convention linked into an export would be inventoried
    // away rather than found.
    let exported = tree_with(&[
        ("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT),
        ("outside/smuggled.md", OVERLAY_DOCUMENT),
    ]);
    std::os::unix::fs::symlink(
        exported.path().join("outside/smuggled.md"),
        exported.path().join("knowledge/conventions/linked.md"),
    )
    .unwrap();

    let result = list_convention_documents(exported.path());

    let Err(ConventionShippingCheckError::TreeUnreadable { path, reason }) = result else {
        panic!(
            "a document the walk refuses to follow is one it cannot read, not one that is absent"
        )
    };
    assert_eq!(path, exported.path().join("knowledge/conventions/linked.md"));
    assert!(reason.to_string().contains("symbolic link"), "and says why: {reason}");
}

#[cfg(unix)]
#[test]
fn test_list_convention_documents_refuses_a_linked_subdirectory() {
    // Following it would produce `knowledge/conventions/linked/smuggled.md` for
    // a file outside the tree, an identity the overlay could never supply and so
    // a violation the export did not commit. Walking past it would be the other
    // wrong answer: a linked subdirectory can hold shipped documents.
    let exported = tree_with(&[
        ("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT),
        ("outside/smuggled.md", OVERLAY_DOCUMENT),
    ]);
    std::os::unix::fs::symlink(
        exported.path().join("outside"),
        exported.path().join("knowledge/conventions/linked"),
    )
    .unwrap();

    let result = list_convention_documents(exported.path());

    let Err(ConventionShippingCheckError::TreeUnreadable { path, .. }) = result else {
        panic!("a subtree the walk refuses to enter is not a subtree it read and found empty")
    };
    assert_eq!(path, exported.path().join("knowledge/conventions/linked"));
}

#[cfg(unix)]
#[test]
fn test_list_convention_documents_passes_over_a_node_that_is_not_a_regular_file() {
    // A socket stands in for the node type that matters most here, a FIFO: both
    // are named like a document and are neither a directory nor a link, and
    // opening a FIFO blocks until something writes to it. Excluding them by node
    // type is what keeps a walk of a consumer's tree from stopping on one.
    let exported = tree_with(&[("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT)]);
    let listener = std::os::unix::net::UnixListener::bind(
        exported.path().join("knowledge/conventions/odd.md"),
    )
    .unwrap();

    let inventoried = list_convention_documents(exported.path()).unwrap();

    assert_eq!(
        sorted_paths(&inventoried),
        ["knowledge/conventions/coding-principles.md"],
        "a node that is not a regular file is not a document the tree ships, and reaching one \
         neither fails the inventory nor blocks it"
    );
    drop(listener);
}

#[test]
fn test_list_convention_documents_beyond_the_entry_ceiling_fails_closed() {
    // The per-directory bound renews at every directory, so a tree can stay
    // inside it at every node and still be arbitrarily large. Spread just past
    // the whole-walk ceiling over several directories, none of them near the
    // per-directory bound, so what fails here can only be the aggregate one.
    let exported = tree_with(&[("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT)]);
    let per_directory = super::MAX_SCAN_ENTRIES / 4;
    for directory in 0..4 {
        let dir = exported.path().join(format!("knowledge/conventions/bulk-{directory}"));
        std::fs::create_dir_all(&dir).unwrap();
        for entry in 0..per_directory {
            std::fs::write(dir.join(format!("filler-{entry}.txt")), b"").unwrap();
        }
    }

    let result = list_convention_documents(exported.path());

    let Err(ConventionShippingCheckError::TreeUnreadable { path, reason }) = result else {
        panic!(
            "a tree larger than this walk will examine is one it did not read, and reporting what \
             it managed to reach would present a partial inventory as the whole"
        )
    };
    assert_eq!(
        path,
        exported.path().join("knowledge/conventions"),
        "the ceiling is a statement about the tree, so it names the root rather than the \
         directory the count happened to end in"
    );
    assert!(reason.to_string().contains("entries"), "and says why: {reason}");
}

#[test]
fn test_list_convention_documents_with_an_oversized_directory_fails_closed() {
    // The bound the ceiling above does not cover: one listing this walk would
    // have to hold whole. It names the directory, which a statement about a
    // single node can do.
    let exported = tree_with(&[("knowledge/conventions/nested/testing.md", OVERLAY_DOCUMENT)]);
    let crowded = exported.path().join("knowledge/conventions/nested");
    for index in 0..=crate::conventions_resolve::directory_walk::MAX_DIRECTORY_ENTRIES {
        std::fs::write(crowded.join(format!("{index}.md")), b"").unwrap();
    }

    let result = list_convention_documents(exported.path());

    let Err(ConventionShippingCheckError::TreeUnreadable { path, reason }) = result else {
        panic!("a listing the walk will not hold whole is a directory it cannot read")
    };
    assert_eq!(path, exported.path().join("knowledge/conventions/nested"));
    assert!(reason.to_string().contains("entries"), "and says what it stopped on: {reason}");
}

#[test]
fn test_list_convention_documents_below_the_depth_bound_fails_closed() {
    let exported = tree_with(&[("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT)]);
    let deep = "deep/".repeat(super::MAX_DIRECTORY_DEPTH + 1);
    write_bytes(
        exported.path(),
        &format!("knowledge/conventions/{deep}buried.md"),
        OVERLAY_DOCUMENT.as_bytes(),
    );

    let result = list_convention_documents(exported.path());

    let Err(ConventionShippingCheckError::TreeUnreadable { path, reason }) = result else {
        panic!("a tree the walk cannot descend without risking the stack is one it cannot read")
    };
    assert!(
        path.starts_with(exported.path().join("knowledge/conventions/deep")),
        "the failure names the directory the walk stopped at: {}",
        path.display()
    );
    assert!(reason.to_string().contains("nested deeper"), "and says why: {reason}");
}

#[test]
fn test_list_convention_documents_within_the_depth_bound_inventories_the_document() {
    let exported = tree_with(&[("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT)]);
    // One level shy of the bound, so the deepest directory the walk may enter is
    // entered rather than refused.
    let deep = "deep/".repeat(super::MAX_DIRECTORY_DEPTH - 1);
    write_bytes(
        exported.path(),
        &format!("knowledge/conventions/{deep}buried.md"),
        OVERLAY_DOCUMENT.as_bytes(),
    );

    let inventoried = list_convention_documents(exported.path()).unwrap();

    assert_eq!(
        sorted_paths(&inventoried),
        [
            "knowledge/conventions/coding-principles.md",
            &format!("knowledge/conventions/{deep}buried.md"),
        ],
        "the bound refuses only what is past it, so nesting up to it is inventoried normally"
    );
}

#[cfg(unix)]
#[test]
fn test_list_convention_documents_attributes_a_rejected_document_path_to_its_own_tree() {
    // The one thing this walk adds to the path rule. `ConventionDocumentPath::
    // try_new` decides that a name holding a line terminator is not renderable
    // as a record, and it is never told which tree the name came from; both
    // trees below hold the identically-named document, so the constructor's
    // rejection is the same on each side and only the walk can say which
    // inventory produced it.
    use std::os::unix::ffi::OsStrExt;

    let unrenderable = std::ffi::OsStr::from_bytes(b"line\nbreak.md");
    let exported = tree_with(&[("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT)]);
    let overlay = tree_with(&[("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT)]);
    for tree in [&exported, &overlay] {
        std::fs::write(tree.path().join("knowledge/conventions").join(unrenderable), b"").unwrap();
    }

    let from_export = list_convention_documents(exported.path());
    let from_overlay = list_convention_documents(overlay.path());

    let attributed = |result| match result {
        Err(ConventionShippingCheckError::DocumentPathRejected { tree_root, source }) => {
            assert!(
                matches!(source, ConventionDocumentPathError::NotRenderableAsRecord { .. }),
                "the rejection is the constructor's own and is lifted rather than restated"
            );
            tree_root
        }
        _ => panic!("a document the walk found and cannot name fails the inventory"),
    };

    assert_eq!(attributed(from_export), PathBuf::from(exported.path()));
    assert_eq!(
        attributed(from_overlay),
        PathBuf::from(overlay.path()),
        "the same rejected name in the other tree is attributed to that tree, which is the whole \
         of what this walk adds: the constructor could not have told the two apart"
    );
}

#[cfg(unix)]
#[test]
fn test_list_convention_documents_ignores_an_unrenderable_name_that_is_not_a_document() {
    // The rule is asked only of documents the walk presents, so an
    // unrepresentable name costs the caller an inventory exactly when it belongs
    // to a shipped document and never when it belongs to something the walk was
    // not going to return.
    use std::os::unix::ffi::OsStrExt;

    let exported = tree_with(&[("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT)]);
    std::fs::write(
        exported.path().join("knowledge/conventions").join(std::ffi::OsStr::from_bytes(b"no\nte")),
        b"",
    )
    .unwrap();

    let inventoried = list_convention_documents(exported.path()).unwrap();

    assert_eq!(
        sorted_paths(&inventoried),
        ["knowledge/conventions/coding-principles.md"],
        "a neighbour with no document extension is not a convention document, so its name is \
         never put to the rule"
    );
}
