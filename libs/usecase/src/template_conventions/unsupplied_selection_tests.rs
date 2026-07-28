//! Tests for the pure shipping comparison (`IN-11`, `AC-18`).
//!
//! Kept in a sibling module so only the production half adds to [`super`]'s
//! length.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use super::{ConventionShippingVerdict, select_unsupplied_conventions};
use crate::conventions_resolve::ConventionDocumentPath;

fn document(path: &str) -> ConventionDocumentPath {
    ConventionDocumentPath::try_new(PathBuf::from(path)).unwrap()
}

#[test]
fn test_select_unsupplied_conventions_names_the_shipped_documents_the_overlay_does_not_supply() {
    // The overlay supplies the documents a consumer may legitimately receive.
    // The exported tree ships those plus one source convention that was never
    // overlaid — the mutation the smoke item exists to catch — and that
    // document is what the verdict has to name.
    let supplied = [
        document("knowledge/conventions/coding-principles.md"),
        document("knowledge/conventions/testing.md"),
    ];
    let shipped = [
        document("knowledge/conventions/coding-principles.md"),
        document("knowledge/conventions/testing.md"),
        document("knowledge/conventions/workflow-ceremony-minimization.md"),
    ];

    let verdict = select_unsupplied_conventions(&shipped, &supplied);

    let ConventionShippingVerdict::UnsuppliedDocumentsShipped { documents } = verdict else {
        panic!("an export shipping a document the overlay does not supply is a violation");
    };
    let named: Vec<String> = documents.as_slice().iter().map(ToString::to_string).collect();
    assert_eq!(
        named,
        ["knowledge/conventions/workflow-ceremony-minimization.md"],
        "only the unsupplied document is reported: the two the overlay supplies are shipped \
         legitimately"
    );
}

#[test]
fn test_select_unsupplied_conventions_is_conforming_when_the_export_ships_only_supplied_documents()
{
    // This is the state the changed repository has to be in for the smoke item
    // to succeed: every convention document inside the exported tree came from
    // the overlay, so the comparison has nothing to report.
    let supplied = [
        document("knowledge/conventions/coding-principles.md"),
        document("knowledge/conventions/testing.md"),
        document("knowledge/conventions/security.md"),
    ];
    let shipped = [
        document("knowledge/conventions/security.md"),
        document("knowledge/conventions/coding-principles.md"),
        document("knowledge/conventions/testing.md"),
    ];

    assert_eq!(
        select_unsupplied_conventions(&shipped, &supplied),
        ConventionShippingVerdict::Conforming,
        "an export whose every document the overlay supplies ships nothing beyond that supply, \
         whatever order the walk found the documents in"
    );
}

#[test]
fn test_select_unsupplied_conventions_deduplicates_and_stably_orders_the_offending_documents() {
    let supplied = [document("knowledge/conventions/testing.md")];
    // Two inventories of the same offending set, differing only in the order a
    // walk produced them and in one of them reaching the same document twice.
    // Neither the walk order nor the repeat may reach the caller, or a failing
    // smoke run would name the same violation differently from run to run.
    let one_walk = [
        document("knowledge/conventions/security.md"),
        document("knowledge/conventions/testing.md"),
        document("knowledge/conventions/adr-authoring.md"),
        document("knowledge/conventions/rust/naming.md"),
    ];
    let other_walk = [
        document("knowledge/conventions/rust/naming.md"),
        document("knowledge/conventions/adr-authoring.md"),
        document("knowledge/conventions/security.md"),
        document("knowledge/conventions/security.md"),
    ];

    let first = select_unsupplied_conventions(&one_walk, &supplied);
    let second = select_unsupplied_conventions(&other_walk, &supplied);

    let ConventionShippingVerdict::UnsuppliedDocumentsShipped { documents } = first.clone() else {
        panic!("three documents outside the overlay's supply are a violation");
    };
    let named: Vec<String> = documents.as_slice().iter().map(ToString::to_string).collect();
    assert_eq!(
        named,
        [
            "knowledge/conventions/adr-authoring.md",
            "knowledge/conventions/rust/naming.md",
            "knowledge/conventions/security.md",
        ],
        "the offending documents are named once each, in lexicographic path order rather than \
         the order the walk produced them"
    );
    assert_eq!(
        first, second,
        "two walks over the same offending set produce the same verdict value, so a failing \
         smoke run is reproducible and diffable"
    );
}

#[test]
fn test_select_unsupplied_conventions_ignores_documents_the_overlay_supplies_but_the_export_lacks()
{
    // The overlay supplies a document the exported tree does not ship. A
    // symmetric diff would report it, but that is a second finding class the
    // verdict has no variant for and this track states no rule about, so the
    // comparison runs in one direction only.
    let supplied = [
        document("knowledge/conventions/coding-principles.md"),
        document("knowledge/conventions/never-exported.md"),
    ];
    let shipped = [document("knowledge/conventions/coding-principles.md")];

    assert_eq!(
        select_unsupplied_conventions(&shipped, &supplied),
        ConventionShippingVerdict::Conforming,
        "a supplied document missing from the export is outside this comparison's contract"
    );

    // Swapping the arguments asks the opposite question, which is what makes
    // the direction observable rather than an accident of these fixtures.
    let swapped = select_unsupplied_conventions(&supplied, &shipped);
    let ConventionShippingVerdict::UnsuppliedDocumentsShipped { documents } = swapped else {
        panic!("the swapped comparison has one document on its shipped side and not the other");
    };
    let named: Vec<String> = documents.as_slice().iter().map(ToString::to_string).collect();
    assert_eq!(named, ["knowledge/conventions/never-exported.md"]);
}

#[test]
fn test_select_unsupplied_conventions_is_conforming_exactly_when_the_selection_is_empty() {
    let supplied = [document("knowledge/conventions/testing.md")];

    assert_eq!(
        select_unsupplied_conventions(&[], &supplied),
        ConventionShippingVerdict::Conforming,
        "an export shipping no convention document at all selects nothing"
    );
    assert_eq!(
        select_unsupplied_conventions(&supplied, &supplied),
        ConventionShippingVerdict::Conforming,
        "so does an export shipping exactly the supplied documents"
    );

    // One unsupplied document flips the verdict. The violating variant carries
    // a non-empty collection, so the branch returning it is reachable only with
    // a document in hand: "violating but naming nothing" has no representation
    // for the empty selection to fall into, which is why the empty selection
    // has nowhere to go but Conforming.
    let shipped = [
        document("knowledge/conventions/testing.md"),
        document("knowledge/conventions/security.md"),
    ];
    let verdict = select_unsupplied_conventions(&shipped, &supplied);

    let ConventionShippingVerdict::UnsuppliedDocumentsShipped { documents } = verdict else {
        panic!("one document outside the overlay's supply is a violation");
    };
    assert_eq!(documents.as_slice().len(), 1);
    assert_eq!(
        documents.first().expect("a violating verdict always names at least one document"),
        &document("knowledge/conventions/security.md")
    );
}

#[test]
fn test_select_unsupplied_conventions_decides_from_its_two_inventories_alone() {
    // No path below names a file in any tree on disk, and the comparison still
    // answers: the function holds no state, is injected with nothing, and reads
    // no filesystem, so the two inventories it is handed are the whole of its
    // input. That is what lets the shipping rule be exercised without producing
    // an export first.
    let shipped = [
        document("knowledge/conventions/no-such-document.md"),
        document("knowledge/conventions/nested/also-absent.md"),
    ];

    assert_eq!(
        select_unsupplied_conventions(&shipped, &shipped),
        ConventionShippingVerdict::Conforming,
        "documents present on both sides are supplied, existence on disk notwithstanding"
    );

    let verdict = select_unsupplied_conventions(&shipped, &[]);

    let ConventionShippingVerdict::UnsuppliedDocumentsShipped { documents } = verdict else {
        panic!("an overlay supplying nothing leaves every shipped document unsupplied");
    };
    let named: Vec<String> = documents.as_slice().iter().map(ToString::to_string).collect();
    assert_eq!(
        named,
        [
            "knowledge/conventions/nested/also-absent.md",
            "knowledge/conventions/no-such-document.md",
        ],
        "every shipped document is offending when the overlay supplies none of them"
    );
}
