//! Tests for the convention inventory port (`IN-11`, `AC-18`).
//!
//! Kept in a sibling module so only the production half adds to [`super`]'s
//! length.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;

use domain::tddd::catalogue_linter::FreeText;

use super::{
    ConventionInventoryPort, ConventionShippingCheckError, ConventionShippingVerdict,
    select_unsupplied_conventions,
};
use crate::conventions_resolve::{ConventionDocumentPath, ConventionDocumentPathError};

fn document(path: &str) -> ConventionDocumentPath {
    ConventionDocumentPath::try_new(PathBuf::from(path)).unwrap()
}

/// What a scripted inventory hands back, rebuilt on every call so that the same
/// fake can be inventoried through more than once.
enum ScriptedOutcome {
    /// Every tree inventories to the same documents.
    Uniform(Vec<ConventionDocumentPath>),
    /// Each named tree inventories to its own documents; a tree named nowhere
    /// in the list holds no convention root.
    PerTree(Vec<(PathBuf, Vec<ConventionDocumentPath>)>),
    /// Every tree meets the same condition that prevents an inventory.
    FailedClosed(fn() -> ConventionShippingCheckError),
}

/// Fake adapter standing in for the real filesystem walk.
///
/// The contract is discharged against this rather than a real adapter: nothing
/// below reads a directory, so what is under test is the port's promise to its
/// caller and not any one implementation of it.
struct ScriptedInventory {
    outcome: ScriptedOutcome,
    observed_roots: Mutex<Vec<PathBuf>>,
}

impl ScriptedInventory {
    fn shipping(documents: Vec<ConventionDocumentPath>) -> Self {
        Self {
            outcome: ScriptedOutcome::Uniform(documents),
            observed_roots: Mutex::new(Vec::new()),
        }
    }

    fn per_tree(trees: Vec<(PathBuf, Vec<ConventionDocumentPath>)>) -> Self {
        Self { outcome: ScriptedOutcome::PerTree(trees), observed_roots: Mutex::new(Vec::new()) }
    }

    fn failing(build: fn() -> ConventionShippingCheckError) -> Self {
        Self {
            outcome: ScriptedOutcome::FailedClosed(build),
            observed_roots: Mutex::new(Vec::new()),
        }
    }

    fn observed_roots(&self) -> Vec<PathBuf> {
        self.observed_roots.lock().expect("the scripted inventory is never poisoned").clone()
    }
}

impl ConventionInventoryPort for ScriptedInventory {
    fn list_conventions(
        &self,
        tree_root: &Path,
    ) -> Result<Vec<ConventionDocumentPath>, ConventionShippingCheckError> {
        // Recording the call needs interior mutability precisely because the
        // receiver is shared: the port hands its implementor no exclusive
        // borrow to mutate through.
        self.observed_roots
            .lock()
            .expect("the scripted inventory is never poisoned")
            .push(tree_root.to_path_buf());
        match &self.outcome {
            ScriptedOutcome::Uniform(documents) => Ok(documents.clone()),
            ScriptedOutcome::PerTree(trees) => trees
                .iter()
                .find(|(root, _)| root == tree_root)
                .map(|(_, documents)| documents.clone())
                .ok_or_else(|| ConventionShippingCheckError::ConventionRootMissing {
                    tree_root: tree_root.to_path_buf(),
                }),
            ScriptedOutcome::FailedClosed(build) => Err(build()),
        }
    }
}

#[test]
fn test_list_conventions_inventories_the_tree_the_caller_named() {
    let port = ScriptedInventory::shipping(Vec::new());

    port.list_conventions(Path::new("/var/tmp/export-42")).unwrap();
    port.list_conventions(Path::new("overlay")).unwrap();

    assert_eq!(
        port.observed_roots(),
        [PathBuf::from("/var/tmp/export-42"), PathBuf::from("overlay")],
        "the tree inventoried is the one the caller named, so the port reads no ambient location \
         and the same port answers for the exported tree and for the overlay"
    );
}

#[test]
fn test_list_conventions_identifies_documents_tree_relative_so_two_trees_compare() {
    // The exported tree lives under a throwaway absolute root and the overlay
    // under a repository-relative one, yet both inventory the same document as
    // the same value: the identity is the path inside the tree, never the root
    // it was found under. Absolute identities would carry the two roots into
    // the comparison and make every document look unsupplied.
    let port = ScriptedInventory::per_tree(vec![
        (
            PathBuf::from("/var/tmp/export-42"),
            vec![
                document("knowledge/conventions/coding-principles.md"),
                document("knowledge/conventions/workflow-ceremony-minimization.md"),
            ],
        ),
        (PathBuf::from("overlay"), vec![document("knowledge/conventions/coding-principles.md")]),
    ]);

    let shipped = port.list_conventions(Path::new("/var/tmp/export-42")).unwrap();
    let supplied = port.list_conventions(Path::new("overlay")).unwrap();

    assert_eq!(
        shipped.first().expect("the exported tree ships two documents"),
        supplied.first().expect("the overlay supplies one document"),
        "the document both trees hold is one value, so the two inventories are comparable"
    );
    let verdict = select_unsupplied_conventions(&shipped, &supplied);
    let ConventionShippingVerdict::UnsuppliedDocumentsShipped { documents } = verdict else {
        panic!("the exported tree ships a document the overlay does not supply");
    };
    let named: Vec<String> = documents.as_slice().iter().map(ToString::to_string).collect();
    assert_eq!(
        named,
        ["knowledge/conventions/workflow-ceremony-minimization.md"],
        "only the document with no overlay counterpart is reported, which is the finding the \
         smoke item exists to make against an exported tree"
    );
}

#[test]
fn test_list_conventions_hands_back_the_walk_result_without_canonicalizing_it() {
    // The port's return type is a plain `Vec`, so ordering and deduplication
    // are not among its promises: the walk's result arrives as found, and
    // making it canonical is the comparison's decision. That is the seam — an
    // adapter cannot quietly vary what a violation looks like.
    let port = ScriptedInventory::shipping(vec![
        document("knowledge/conventions/security.md"),
        document("knowledge/conventions/adr-authoring.md"),
        document("knowledge/conventions/security.md"),
    ]);

    let shipped = port.list_conventions(Path::new("/var/tmp/export-42")).unwrap();

    let inventoried: Vec<String> = shipped.iter().map(ToString::to_string).collect();
    assert_eq!(
        inventoried,
        [
            "knowledge/conventions/security.md",
            "knowledge/conventions/adr-authoring.md",
            "knowledge/conventions/security.md",
        ],
        "the inventory is what the walk found, in the order it found it"
    );
    let verdict = select_unsupplied_conventions(&shipped, &[]);
    let ConventionShippingVerdict::UnsuppliedDocumentsShipped { documents } = verdict else {
        panic!("an overlay supplying nothing leaves both shipped documents unsupplied");
    };
    let named: Vec<String> = documents.as_slice().iter().map(ToString::to_string).collect();
    assert_eq!(
        named,
        ["knowledge/conventions/adr-authoring.md", "knowledge/conventions/security.md"],
        "the caller's report is canonical even though the inventory it came from was not"
    );
}

#[test]
fn test_list_conventions_surfaces_every_condition_that_prevents_an_inventory() {
    let conditions: [fn() -> ConventionShippingCheckError; 3] = [
        || ConventionShippingCheckError::ConventionRootMissing {
            tree_root: PathBuf::from("/var/tmp/export-42"),
        },
        || ConventionShippingCheckError::TreeUnreadable {
            path: PathBuf::from("/var/tmp/export-42/knowledge/conventions"),
            reason: FreeText::new("permission denied (os error 13)".to_owned()),
        },
        || ConventionShippingCheckError::DocumentPathRejected {
            tree_root: PathBuf::from("/var/tmp/export-42"),
            source: ConventionDocumentPath::try_new(PathBuf::from("knowledge/adr/README.md"))
                .expect_err("a sibling directory is outside the convention root"),
        },
    ];

    for build in conditions {
        let port = ScriptedInventory::failing(build);

        let result = port.list_conventions(Path::new("/var/tmp/export-42"));

        let Err(surfaced) = result else {
            panic!("a condition preventing an inventory must reach the caller as an error");
        };
        // The return type is `Result<Vec<_>, _>` and not a pair, so an
        // inventory that hit this condition has no way to also hand back the
        // documents it had already walked: the comparison cannot run on a
        // partial list and report a document as unshipped that was merely
        // never reached.
        assert_eq!(
            surfaced.to_string(),
            build().to_string(),
            "the condition reaches the caller as itself, not as a substitute error"
        );
    }
}

#[test]
fn test_list_conventions_cannot_report_a_document_defect_as_a_shipping_finding() {
    // A document whose front matter is malformed YAML is inventoried like any
    // other: this port names paths and never opens a document, so the defect
    // has no way to reach the comparison. Reusing `ConventionRequirementPort`,
    // which decodes `required_for` and fails closed on exactly that defect,
    // would have turned it into a failed shipping check.
    let port = ScriptedInventory::shipping(vec![
        document("knowledge/conventions/malformed-front-matter.md"),
        document("knowledge/conventions/coding-principles.md"),
    ]);

    let shipped = port.list_conventions(Path::new("/var/tmp/export-42")).unwrap();

    assert_eq!(shipped.len(), 2, "a document with unreadable front matter is still shipped");
    // The exhaustive match is the mechanism, not the comment above it: the
    // failure vocabulary of this port has three variants and none of them can
    // name a content-parse condition, so adding one would stop this compiling.
    let condition = ConventionShippingCheckError::ConventionRootMissing {
        tree_root: PathBuf::from("/var/tmp/export-42"),
    };
    match condition {
        ConventionShippingCheckError::ConventionRootMissing { .. }
        | ConventionShippingCheckError::TreeUnreadable { .. }
        | ConventionShippingCheckError::DocumentPathRejected {
            source: ConventionDocumentPathError::OutsideConventionRoot { .. },
            ..
        }
        | ConventionShippingCheckError::DocumentPathRejected {
            source: ConventionDocumentPathError::NotRenderableAsRecord { .. },
            ..
        } => {}
    }
}

#[test]
fn test_list_conventions_reads_through_a_shared_receiver_across_threads() {
    let port = ScriptedInventory::shipping(vec![document("knowledge/conventions/testing.md")]);
    let shared: &dyn ConventionInventoryPort = &port;

    // Two concurrent inventories through one shared reference: the call needs
    // no exclusive borrow and no executor, which is what makes the port both
    // injectable behind `&dyn` and safe to hold in a `Send + Sync` root without
    // an async runtime existing anywhere in this workspace.
    let (first, second) = thread::scope(|scope| {
        let left = scope.spawn(|| shared.list_conventions(Path::new("/var/tmp/export-42")));
        let right = scope.spawn(|| shared.list_conventions(Path::new("overlay")));
        (
            left.join().expect("the inventory thread does not panic").unwrap(),
            right.join().expect("the inventory thread does not panic").unwrap(),
        )
    });

    assert_eq!(first, second, "inventorying twice observes the same documents");
    assert_eq!(port.observed_roots().len(), 2);
}

#[test]
fn test_convention_inventory_port_bound_implies_send_and_sync() {
    fn implied_by_the_port<T: ConventionInventoryPort + ?Sized>() {
        fn shareable<U: Send + Sync + ?Sized>() {}
        // Compiles only because the supertrait bound carries `Send + Sync`
        // through: the check is on the trait, not on any one implementor.
        shareable::<T>();
    }

    implied_by_the_port::<dyn ConventionInventoryPort>();
    implied_by_the_port::<ScriptedInventory>();
}
