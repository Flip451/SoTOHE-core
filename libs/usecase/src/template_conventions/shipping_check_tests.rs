//! Tests for the shipping check's primary port and the interactor that
//! realises it (`IN-11`, `AC-18`).
//!
//! Kept in a sibling module so only the production half adds to [`super`]'s
//! length.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use domain::tddd::catalogue_linter::FreeText;

use super::{
    CheckConventionShippingQuery, ConventionInventoryPort, ConventionShippingCheckError,
    ConventionShippingCheckInteractor, ConventionShippingCheckService, ConventionShippingVerdict,
    select_unsupplied_conventions,
};
use crate::conventions_resolve::ConventionDocumentPath;

fn documents(paths: &[&str]) -> Vec<ConventionDocumentPath> {
    paths.iter().map(|path| ConventionDocumentPath::try_new(PathBuf::from(path)).unwrap()).collect()
}

fn named(documents: &[ConventionDocumentPath]) -> Vec<String> {
    documents.iter().map(ToString::to_string).collect()
}

/// Builds the fake from the documents each named tree ships.
fn inventorying(trees: &[(&str, &[&str])]) -> ScriptedInventory {
    ScriptedInventory {
        listings: trees
            .iter()
            .map(|(root, paths)| (PathBuf::from(root), documents(paths)))
            .collect(),
        unlistable: None,
        observed_roots: Mutex::new(Vec::new()),
    }
}

/// Builds the request from the exported tree root and the overlay directory it
/// is checked against.
fn checking(exported_root: &str, overlay_dir: &str) -> CheckConventionShippingQuery {
    CheckConventionShippingQuery {
        exported_root: PathBuf::from(exported_root),
        overlay_dir: PathBuf::from(overlay_dir),
    }
}

/// Fake inventory the check is driven through; no directory is read anywhere
/// below, so what is under test is the wiring and not a filesystem walk.
///
/// It answers per tree root, so the two sides of one check can be given
/// different documents, and it records every root it was asked about, so a test
/// can read back which walks went through the one value it injected.
struct ScriptedInventory {
    listings: Vec<(PathBuf, Vec<ConventionDocumentPath>)>,
    unlistable: Option<(PathBuf, fn() -> ConventionShippingCheckError)>,
    observed_roots: Mutex<Vec<PathBuf>>,
}

impl ScriptedInventory {
    fn unlistable_at(mut self, root: &str, build: fn() -> ConventionShippingCheckError) -> Self {
        self.unlistable = Some((PathBuf::from(root), build));
        self
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
        self.observed_roots
            .lock()
            .expect("the scripted inventory is never poisoned")
            .push(tree_root.to_path_buf());
        if let Some((root, build)) = &self.unlistable
            && root == tree_root
        {
            return Err(build());
        }
        self.listings
            .iter()
            .find(|(root, _)| root == tree_root)
            .map(|(_, documents)| documents.clone())
            .ok_or_else(|| ConventionShippingCheckError::ConventionRootMissing {
                tree_root: tree_root.to_path_buf(),
            })
    }
}

// ---------------------------------------------------------------------------
// ConventionShippingCheckService
// ---------------------------------------------------------------------------

#[test]
fn test_check_hands_a_shipping_violation_back_as_a_verdict_and_not_as_a_failure() {
    // The exported tree ships a source convention the overlay never supplied,
    // which is the mutation this check exists to catch against an exported
    // tree.
    let inventory = Arc::new(inventorying(&[
        (
            "/var/tmp/export-42",
            &[
                "knowledge/conventions/coding-principles.md",
                "knowledge/conventions/workflow-ceremony-minimization.md",
            ],
        ),
        ("template/overlay", &["knowledge/conventions/coding-principles.md"]),
    ]));
    // Held as the declared port, so what answers below is the operation the
    // trait declares rather than an inherent method of one type.
    let service: Arc<dyn ConventionShippingCheckService> =
        Arc::new(ConventionShippingCheckInteractor::new(
            Arc::clone(&inventory) as Arc<dyn ConventionInventoryPort>
        ));

    let result = service.check(checking("/var/tmp/export-42", "template/overlay"));

    let Ok(ConventionShippingVerdict::UnsuppliedDocumentsShipped { documents }) = result else {
        panic!("a detected violation is the check's normal finding, not a failed check");
    };
    assert_eq!(
        named(documents.as_slice()),
        ["knowledge/conventions/workflow-ceremony-minimization.md"],
        "the exported tree the request named is the subject the question was put about, and the \
         answer settles it by naming the document that ships without an overlay counterpart"
    );
}

#[test]
fn test_check_reports_a_tree_it_could_not_inventory_as_a_failure_and_not_as_a_verdict() {
    // Only the overlay has a listing. The exported root therefore holds no
    // convention root at all, which is the shape a tree takes when the boundary
    // classification is edited away from `overlay` — not an export that
    // legitimately ships nothing.
    let inventory = Arc::new(inventorying(&[(
        "template/overlay",
        &["knowledge/conventions/coding-principles.md"],
    )]));
    let service: Arc<dyn ConventionShippingCheckService> =
        Arc::new(ConventionShippingCheckInteractor::new(
            Arc::clone(&inventory) as Arc<dyn ConventionInventoryPort>
        ));

    let result = service.check(checking("/var/tmp/export-42", "template/overlay"));

    let Err(ConventionShippingCheckError::ConventionRootMissing { tree_root }) = result else {
        panic!("a tree that could not be inventoried has no verdict to report");
    };
    assert_eq!(
        tree_root,
        PathBuf::from("/var/tmp/export-42"),
        "the unanswerable state names the tree it concerns and arrives on the failure arm, so it \
         cannot be read as the `Conforming` verdict a vacuously empty export would produce"
    );
}

#[test]
fn test_check_is_reachable_from_more_than_one_holder_of_one_shared_port() {
    let inventory = Arc::new(inventorying(&[
        ("/var/tmp/export-42", &["knowledge/conventions/testing.md"]),
        ("template/overlay", &[]),
    ]));
    let service: Arc<dyn ConventionShippingCheckService> =
        Arc::new(ConventionShippingCheckInteractor::new(
            Arc::clone(&inventory) as Arc<dyn ConventionInventoryPort>
        ));
    let smoke = Arc::clone(&service);
    let cli = Arc::clone(&service);

    // Two independent holders check through the same value at the same time:
    // the receiver is shared and the supertrait bound carries `Send + Sync`,
    // which is what lets a smoke run and a command hold one port rather than
    // each owning a check of its own.
    let (from_smoke, from_cli) = thread::scope(|scope| {
        let left =
            scope.spawn(move || smoke.check(checking("/var/tmp/export-42", "template/overlay")));
        let right =
            scope.spawn(move || cli.check(checking("/var/tmp/export-42", "template/overlay")));
        (
            left.join().expect("the checking thread does not panic").unwrap(),
            right.join().expect("the checking thread does not panic").unwrap(),
        )
    });

    fn shareable<T: Send + Sync + ?Sized>() {}
    // Compiles only because the supertrait bound carries `Send + Sync` through:
    // the check is on the trait, not on any one implementor.
    shareable::<dyn ConventionShippingCheckService>();
    assert_eq!(from_smoke, from_cli, "checking twice answers the same verdict");
}

// ---------------------------------------------------------------------------
// ConventionShippingCheckInteractor
// ---------------------------------------------------------------------------

#[test]
fn test_interactor_inventories_both_trees_through_the_one_port_it_was_given() {
    // Keep the observer local: the assertion below is evidence about this
    // fake's calls, rather than a claim about behavior hidden elsewhere in the
    // test module.
    struct RecordingInventory {
        exported_listing: Vec<ConventionDocumentPath>,
        overlay_listing: Vec<ConventionDocumentPath>,
        observed_roots: Mutex<Vec<PathBuf>>,
    }

    impl ConventionInventoryPort for RecordingInventory {
        fn list_conventions(
            &self,
            tree_root: &Path,
        ) -> Result<Vec<ConventionDocumentPath>, ConventionShippingCheckError> {
            self.observed_roots
                .lock()
                .expect("the recording inventory is never poisoned")
                .push(tree_root.to_path_buf());
            match tree_root {
                root if root == Path::new("/var/tmp/export-42") => {
                    Ok(self.exported_listing.clone())
                }
                root if root == Path::new("template/overlay") => Ok(self.overlay_listing.clone()),
                root => Err(ConventionShippingCheckError::ConventionRootMissing {
                    tree_root: root.to_path_buf(),
                }),
            }
        }
    }

    // One inventory value, configured here with a different listing for each
    // of the two roots.
    let inventory = Arc::new(RecordingInventory {
        exported_listing: documents(&[
            "knowledge/conventions/coding-principles.md",
            "knowledge/conventions/workflow-ceremony-minimization.md",
        ]),
        overlay_listing: documents(&["knowledge/conventions/coding-principles.md"]),
        observed_roots: Mutex::new(Vec::new()),
    });
    // Handed over exactly once: the constructor takes one port and the type
    // offers no route through which a second walker could be acquired.
    let interactor = ConventionShippingCheckInteractor::new(
        Arc::clone(&inventory) as Arc<dyn ConventionInventoryPort>
    );

    let verdict = interactor.check(checking("/var/tmp/export-42", "template/overlay")).unwrap();

    assert_eq!(
        inventory.observed_roots.lock().expect("the recording inventory is never poisoned").clone(),
        [PathBuf::from("/var/tmp/export-42"), PathBuf::from("template/overlay")],
        "both trees were walked by the single value handed in above, so neither side of the \
         comparison was produced by a walker of its own that could apply different rules"
    );
    let ConventionShippingVerdict::UnsuppliedDocumentsShipped { documents } = verdict else {
        panic!("the exported tree ships a document the overlay does not supply");
    };
    assert_eq!(
        named(documents.as_slice()),
        ["knowledge/conventions/workflow-ceremony-minimization.md"],
        "the answer is the difference between those two walks, so the record above is of the \
         walks this verdict was actually built from"
    );
}

#[test]
fn test_interactor_answers_with_what_the_comparison_selects_over_the_two_inventories() {
    // The exported listing arrives as a walk would produce it: unordered, with
    // one document reached twice.
    let shipped = documents(&[
        "knowledge/conventions/security.md",
        "knowledge/conventions/coding-principles.md",
        "knowledge/conventions/security.md",
        "knowledge/conventions/adr-authoring.md",
    ]);
    let supplied = documents(&["knowledge/conventions/coding-principles.md"]);
    let inventory = Arc::new(ScriptedInventory {
        listings: vec![
            (PathBuf::from("/var/tmp/export-42"), shipped.clone()),
            (PathBuf::from("template/overlay"), supplied.clone()),
        ],
        unlistable: None,
        observed_roots: Mutex::new(Vec::new()),
    });
    let interactor = ConventionShippingCheckInteractor::new(
        Arc::clone(&inventory) as Arc<dyn ConventionInventoryPort>
    );

    let verdict = interactor.check(checking("/var/tmp/export-42", "template/overlay")).unwrap();

    // Computed here from the same two listings, independently of the
    // interactor. Equality is the whole of its promise: the two inventories
    // reach the comparison as the port produced them, and the comparison's
    // answer reaches the caller with nothing inserted, dropped or reordered on
    // the way.
    assert_eq!(verdict, select_unsupplied_conventions(&shipped, &supplied));
    let ConventionShippingVerdict::UnsuppliedDocumentsShipped { documents } = verdict else {
        panic!("two shipped documents have no overlay counterpart");
    };
    assert_eq!(
        named(documents.as_slice()),
        ["knowledge/conventions/adr-authoring.md", "knowledge/conventions/security.md"],
        "the repeat and the walk order are resolved by the comparison, which the interactor \
         neither performs itself nor post-processes"
    );
}

#[test]
fn test_interactor_reports_conforming_when_the_exported_tree_ships_only_supplied_documents() {
    // This is the state the changed repository has to be in for the shipping
    // check to succeed against its exported tree: every convention document
    // inside the export came from the overlay. The two listings differ in
    // order, because two walks of two different trees have no reason to agree
    // on one.
    let inventory = Arc::new(inventorying(&[
        (
            "/var/tmp/export-42",
            &[
                "knowledge/conventions/testing.md",
                "knowledge/conventions/coding-principles.md",
                "knowledge/conventions/security.md",
            ],
        ),
        (
            "template/overlay",
            &[
                "knowledge/conventions/coding-principles.md",
                "knowledge/conventions/security.md",
                "knowledge/conventions/testing.md",
            ],
        ),
    ]));
    let interactor = ConventionShippingCheckInteractor::new(
        Arc::clone(&inventory) as Arc<dyn ConventionInventoryPort>
    );

    let verdict = interactor.check(checking("/var/tmp/export-42", "template/overlay")).unwrap();

    assert_eq!(
        verdict,
        ConventionShippingVerdict::Conforming,
        "the whole check ran over an exported tree that ships nothing beyond the overlay's supply \
         and reported no finding, whatever order either walk produced"
    );
    assert_eq!(
        inventory.observed_roots(),
        [PathBuf::from("/var/tmp/export-42"), PathBuf::from("template/overlay")],
        "that outcome came from inventorying both trees, not from skipping a side"
    );
}

#[test]
fn test_interactor_surfaces_a_tree_it_could_not_inventory_as_that_failure() {
    for unlistable in ["/var/tmp/export-42", "template/overlay"] {
        // Whichever side cannot be inventoried, the exported tree ships two
        // documents the overlay does not supply. Reading a failure as an empty
        // listing would answer `Conforming` when the exported side failed and a
        // violation when the overlay side did — both from a check that never
        // compared anything.
        let inventory = Arc::new(
            inventorying(&[
                (
                    "/var/tmp/export-42",
                    &["knowledge/conventions/security.md", "knowledge/conventions/testing.md"],
                ),
                ("template/overlay", &[]),
            ])
            .unlistable_at(unlistable, || {
                ConventionShippingCheckError::TreeUnreadable {
                    path: PathBuf::from("knowledge/conventions"),
                    reason: FreeText::new("permission denied (os error 13)".to_owned()),
                }
            }),
        );
        let interactor = ConventionShippingCheckInteractor::new(
            Arc::clone(&inventory) as Arc<dyn ConventionInventoryPort>
        );

        let result = interactor.check(checking("/var/tmp/export-42", "template/overlay"));

        let Err(ConventionShippingCheckError::TreeUnreadable { path, reason }) = result else {
            panic!("a tree that could not be inventoried must not be answered with a verdict");
        };
        assert_eq!(
            (path, reason.as_str().to_owned()),
            (PathBuf::from("knowledge/conventions"), "permission denied (os error 13)".to_owned()),
            "the port's condition reaches the caller as itself: the interactor substitutes no \
             failure of its own and compares nothing the inventory never handed it"
        );
    }
}

#[test]
fn test_interactor_carries_nothing_from_one_check_into_the_next() {
    let inventory = Arc::new(inventorying(&[
        (
            "/var/tmp/export-42",
            &["knowledge/conventions/coding-principles.md", "knowledge/conventions/security.md"],
        ),
        ("template/overlay", &["knowledge/conventions/coding-principles.md"]),
        ("/var/tmp/export-7", &["knowledge/conventions/coding-principles.md"]),
    ]));
    let interactor = ConventionShippingCheckInteractor::new(
        Arc::clone(&inventory) as Arc<dyn ConventionInventoryPort>
    );

    let first = interactor.check(checking("/var/tmp/export-42", "template/overlay")).unwrap();
    let intervening = interactor.check(checking("/var/tmp/export-7", "template/overlay")).unwrap();
    let again = interactor.check(checking("/var/tmp/export-42", "template/overlay")).unwrap();

    assert_eq!(intervening, ConventionShippingVerdict::Conforming);
    assert_eq!(
        first, again,
        "the injected inventory is the only thing the interactor holds, so an intervening check \
         of another tree leaves nothing behind to change this answer"
    );
    assert_eq!(
        inventory.observed_roots().len(),
        6,
        "every check inventories both of its own trees rather than reusing an earlier listing"
    );
}
