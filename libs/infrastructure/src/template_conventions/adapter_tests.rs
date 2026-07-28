//! Tests for the filesystem convention-inventory adapter.
//!
//! Every assertion here is made through `&dyn ConventionInventoryPort` rather
//! than against the free function, because what is under test is that a caller
//! holding only the port receives the real walk's answers. Fixtures are real
//! tempdir trees for the same reason: a stand-in would demonstrate the test's
//! own arrangement rather than this adapter's.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::sync::Arc;
use std::thread;

use tempfile::TempDir;
use usecase::conventions_resolve::{ConventionDocumentPath, ConventionDocumentPathError};
use usecase::template_conventions::{ConventionInventoryPort, ConventionShippingCheckError};

use super::FsConventionInventoryAdapter;

/// A convention document with front matter, as the overlay ships them.
const OVERLAY_DOCUMENT: &str = "---\nrequired_for:\n  - implementer\n---\n\n# Coding principles\n";

/// Builds a tree from `files`, given as tree-relative path and content pairs.
fn tree_with(files: &[(&str, &str)]) -> TempDir {
    let tree = TempDir::new().unwrap();
    for (relative, content) in files {
        let path = tree.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content.as_bytes()).unwrap();
    }
    tree
}

/// Renders an inventory as paths in name order.
///
/// Sorted here rather than taken as the port produced it: the order a listing
/// yields is not part of what this port promises, so comparing against a sorted
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
fn test_fs_convention_inventory_adapter_lists_the_documents_a_tree_ships_through_the_port() {
    let exported = tree_with(&[
        ("knowledge/conventions/README.md", OVERLAY_DOCUMENT),
        ("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT),
        ("knowledge/conventions/nested/testing.md", OVERLAY_DOCUMENT),
        // Markdown outside the convention root, which a tree holds plenty of.
        ("knowledge/adr/README.md", OVERLAY_DOCUMENT),
    ]);
    let port: &dyn ConventionInventoryPort = &FsConventionInventoryAdapter::new();

    let inventoried = port.list_conventions(exported.path()).unwrap();

    assert_eq!(
        sorted_paths(&inventoried),
        [
            "knowledge/conventions/README.md",
            "knowledge/conventions/coding-principles.md",
            "knowledge/conventions/nested/testing.md",
        ],
        "the port call reads the documents actually on disk, so injecting the adapter is what \
         gives the caller a real inventory rather than a stand-in's"
    );
    // The fixture root is an arbitrary absolute temporary directory, so an
    // identity carrying any part of it would show up in the comparison above.
    for document in &inventoried {
        assert!(
            document.as_path().starts_with("knowledge/conventions"),
            "and every identity it hands back is tree-relative: {document}"
        );
    }
}

#[test]
fn test_fs_convention_inventory_adapter_inventories_both_trees_through_one_injected_value() {
    // The two sides of the shipping comparison, as separate trees at unrelated
    // absolute locations. Both are inventoried through one value held as the
    // trait object the check injects — which is also what the free function
    // alone could not be — so neither side can be walked by rules the other was
    // not.
    let exported = tree_with(&[
        ("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT),
        ("knowledge/conventions/leaked-source.md", OVERLAY_DOCUMENT),
    ]);
    let overlay = tree_with(&[("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT)]);
    let port: Arc<dyn ConventionInventoryPort> = Arc::new(FsConventionInventoryAdapter::new());

    let shipped = port.list_conventions(exported.path()).unwrap();
    let supplied = port.list_conventions(overlay.path()).unwrap();

    let unsupplied: Vec<&ConventionDocumentPath> =
        shipped.iter().filter(|document| !supplied.contains(document)).collect();
    assert_eq!(
        unsupplied.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["knowledge/conventions/leaked-source.md"],
        "the document both trees hold is one and the same value across the two inventories, so \
         subtracting one from the other names exactly the document the export ships beyond what \
         the overlay supplies"
    );
}

#[test]
fn test_fs_convention_inventory_adapter_inventories_each_named_tree_through_one_shared_receiver() {
    let exported = tree_with(&[("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT)]);
    let overlay = tree_with(&[("knowledge/conventions/security.md", OVERLAY_DOCUMENT)]);
    let adapter = FsConventionInventoryAdapter::new();
    let shared: &dyn ConventionInventoryPort = &adapter;

    // Two concurrent inventories of two different roots through one shared
    // receiver: the adapter holds no state and reads no ambient location, which
    // is what makes it safe to wire once into a composition root and ask twice.
    let (first, second) = thread::scope(|scope| {
        let left = scope.spawn(|| shared.list_conventions(exported.path()));
        let right = scope.spawn(|| shared.list_conventions(overlay.path()));
        (
            left.join().expect("the inventory thread does not panic").unwrap(),
            right.join().expect("the inventory thread does not panic").unwrap(),
        )
    });

    assert_eq!(sorted_paths(&first), ["knowledge/conventions/coding-principles.md"]);
    assert_eq!(
        sorted_paths(&second),
        ["knowledge/conventions/security.md"],
        "each call answers about the tree that call named, so the receiver carries nothing from \
         the tree it was last asked about"
    );
}

#[test]
fn test_fs_convention_inventory_adapter_presents_a_document_it_could_not_have_parsed() {
    // Front matter that is not parseable YAML at all, and a `required_for` that
    // is not an array of strings: both are refusals the resolver's scan fails
    // closed on. Reaching an inventory over this tree through the port is what
    // shows this adapter is not that one — a defect in a document's contents
    // cannot turn the shipping question into a parse failure.
    let exported = tree_with(&[
        ("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT),
        ("knowledge/conventions/malformed.md", "---\n\tname: tabbed\n  : :\n---\n\n# Broken\n"),
        ("knowledge/conventions/mistyped.md", "---\nrequired_for: 5\n---\n\n# Mistyped\n"),
    ]);
    let port: &dyn ConventionInventoryPort = &FsConventionInventoryAdapter::new();

    let inventoried = port.list_conventions(exported.path()).unwrap();

    assert_eq!(
        sorted_paths(&inventoried),
        [
            "knowledge/conventions/coding-principles.md",
            "knowledge/conventions/malformed.md",
            "knowledge/conventions/mistyped.md",
        ],
        "a malformed document is a document the tree ships, so the port presents it like any \
         other rather than failing on contents it never opened"
    );
}

#[test]
fn test_fs_convention_inventory_adapter_leaves_the_trees_it_inventories_untouched() {
    // The check runs against an already-produced export, so a port that wrote
    // while listing would alter the artefact under examination.
    let exported = tree_with(&[
        ("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT),
        ("knowledge/conventions/nested/testing.md", OVERLAY_DOCUMENT),
    ]);
    let before = tree_snapshot(exported.path());
    let port: &dyn ConventionInventoryPort = &FsConventionInventoryAdapter::new();

    port.list_conventions(exported.path()).unwrap();

    assert_eq!(
        tree_snapshot(exported.path()),
        before,
        "the inventory a caller obtains through the port is read-only: it neither creates, \
         updates, nor deletes anything in the tree it was asked about"
    );
}

#[test]
fn test_fs_convention_inventory_adapter_surfaces_every_fail_closed_condition_through_the_port() {
    let port: &dyn ConventionInventoryPort = &FsConventionInventoryAdapter::new();

    // A tree holding no convention root at all — the shape an export takes if
    // the boundary classification is edited away from `overlay`. An empty
    // inventory here would let exactly that export answer the shipping question
    // vacuously, so a caller holding only the port must see it fail closed.
    let rootless = tree_with(&[("docs/testing.md", OVERLAY_DOCUMENT)]);

    let result = port.list_conventions(rootless.path());

    let Err(ConventionShippingCheckError::ConventionRootMissing { tree_root }) = result else {
        panic!("a tree that holds no convention root at all fails rather than inventorying nothing")
    };
    assert_eq!(
        tree_root,
        rootless.path(),
        "the condition reaches the port's caller as itself, still naming the tree that holds none"
    );

    // A tree root nobody could open. Folding this in with the condition above
    // would answer that a tree nobody looked at ships nothing, and the shipping
    // check would then pass on a path that does not exist.
    let parent = TempDir::new().unwrap();
    let absent = parent.path().join("no-such-tree");

    let result = port.list_conventions(&absent);

    let Err(ConventionShippingCheckError::TreeUnreadable { path, .. }) = result else {
        panic!("a tree root that could not be opened leaves the tree unread, not rootless")
    };
    assert_eq!(path, absent, "and it names the root that could not be opened");
}

#[cfg(unix)]
#[test]
fn test_fs_convention_inventory_adapter_surfaces_the_rejected_document_path_through_the_port() {
    // The third fail-closed condition, absent from the test above because its
    // fixture is a file *name* rather than a tree shape. A caller holding only
    // the port must still see it, and a sibling test of the walk does not
    // establish that for this implementation.
    use std::os::unix::ffi::OsStrExt;

    let exported = tree_with(&[("knowledge/conventions/coding-principles.md", OVERLAY_DOCUMENT)]);
    std::fs::write(
        exported
            .path()
            .join("knowledge/conventions")
            .join(std::ffi::OsStr::from_bytes(b"line\nbreak.md")),
        b"",
    )
    .unwrap();
    let port: &dyn ConventionInventoryPort = &FsConventionInventoryAdapter::new();

    let result = port.list_conventions(exported.path());

    let Err(ConventionShippingCheckError::DocumentPathRejected { tree_root, source }) = result
    else {
        panic!("a document the walk found and cannot name is one the port cannot present")
    };
    assert!(
        matches!(source, ConventionDocumentPathError::NotRenderableAsRecord { .. }),
        "the rejection that crosses the port is the constructor's own, unchanged: {source}"
    );
    assert_eq!(
        tree_root,
        exported.path(),
        "and it still carries the tree the rejected name was inventoried from, without which the \
         two sides of the comparison could not be told apart"
    );
}
