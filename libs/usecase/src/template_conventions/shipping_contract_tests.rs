//! Tests for the convention-shipping contracts (`IN-11`, `AC-18`).
//!
//! Kept in a sibling module so only the production half adds to
//! [`super`]'s length.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::error::Error;
use std::path::PathBuf;

use domain::tddd::catalogue_linter::FreeText;
use domain::tddd::catalogue_v2::NonEmptyVec;

use super::{
    CheckConventionShippingQuery, ConventionShippingCheckError, ConventionShippingVerdict,
};
use crate::conventions_resolve::{ConventionDocumentPath, ConventionDocumentPathError};

fn document(path: &str) -> ConventionDocumentPath {
    ConventionDocumentPath::try_new(PathBuf::from(path)).unwrap()
}

// ---------------------------------------------------------------------------
// CheckConventionShippingQuery
// ---------------------------------------------------------------------------

#[test]
fn test_check_convention_shipping_query_carries_both_tree_roots_verbatim() {
    let query = CheckConventionShippingQuery {
        exported_root: PathBuf::from("/var/tmp/export-42"),
        overlay_dir: PathBuf::from("overlay"),
    };

    assert_eq!(query.exported_root, PathBuf::from("/var/tmp/export-42"));
    assert_eq!(query.overlay_dir, PathBuf::from("overlay"));
}

#[test]
fn test_check_convention_shipping_query_accepts_any_caller_chosen_tree_root() {
    // A tree root carries no invariant this layer can enforce, so building the
    // request has no rejection state: absolute, relative, and bare-name roots
    // are all ordinary requests. The constrained concept in this slice is the
    // document identity inside a tree, not the root.
    for (exported, overlay) in
        [("/srv/exports/tree", "/srv/overlay"), ("./out", "../shared-overlay"), ("tree", "overlay")]
    {
        let query = CheckConventionShippingQuery {
            exported_root: PathBuf::from(exported),
            overlay_dir: PathBuf::from(overlay),
        };

        assert_eq!(query.exported_root, PathBuf::from(exported));
        assert_eq!(query.overlay_dir, PathBuf::from(overlay));
    }
}

#[test]
fn test_check_convention_shipping_query_compares_by_both_roots() {
    let query = CheckConventionShippingQuery {
        exported_root: PathBuf::from("out"),
        overlay_dir: PathBuf::from("overlay"),
    };

    assert_eq!(query.clone(), query);
    assert_ne!(
        query,
        CheckConventionShippingQuery {
            exported_root: PathBuf::from("out"),
            overlay_dir: PathBuf::from("other-overlay"),
        },
        "the overlay side of the comparison is part of the request's identity"
    );
    assert_ne!(
        query,
        CheckConventionShippingQuery {
            exported_root: PathBuf::from("other-out"),
            overlay_dir: PathBuf::from("overlay"),
        },
        "so is the exported side"
    );
}

// ---------------------------------------------------------------------------
// ConventionShippingVerdict
// ---------------------------------------------------------------------------

#[test]
fn test_convention_shipping_verdict_conforming_carries_no_document() {
    let verdict = ConventionShippingVerdict::Conforming;

    assert_eq!(verdict, ConventionShippingVerdict::Conforming);
    assert!(
        !matches!(verdict, ConventionShippingVerdict::UnsuppliedDocumentsShipped { .. }),
        "an export matching the overlay supply is the conforming verdict"
    );
}

#[test]
fn test_convention_shipping_verdict_violation_names_the_offending_documents() {
    let verdict = ConventionShippingVerdict::UnsuppliedDocumentsShipped {
        documents: NonEmptyVec::new(
            document("knowledge/conventions/language-policy.md"),
            vec![document("knowledge/conventions/typed-deserialization.md")],
        ),
    };

    let ConventionShippingVerdict::UnsuppliedDocumentsShipped { documents } = verdict else {
        panic!("expected the violating verdict");
    };
    let named: Vec<String> = documents.as_slice().iter().map(ToString::to_string).collect();
    assert_eq!(
        named,
        [
            "knowledge/conventions/language-policy.md",
            "knowledge/conventions/typed-deserialization.md",
        ],
        "the offending documents travel as typed paths, so a consumer reporting the \
         violation can always name what caused it"
    );
}

#[test]
fn test_convention_shipping_verdict_violation_always_names_at_least_one_document() {
    // The violating variant's payload is a non-empty collection, so "violating
    // but naming no document" has no representation to reach. The constructor
    // rejection below is the same guarantee viewed from the building side.
    assert!(
        NonEmptyVec::<ConventionDocumentPath>::try_new(Vec::new()).is_err(),
        "an empty document list cannot be built into the violating verdict's payload"
    );

    let verdict = ConventionShippingVerdict::UnsuppliedDocumentsShipped {
        documents: NonEmptyVec::try_new(vec![document(
            "knowledge/conventions/nightly-dev-tool.md",
        )])
        .expect("one offending document is a valid payload"),
    };

    let ConventionShippingVerdict::UnsuppliedDocumentsShipped { documents } = verdict else {
        panic!("expected the violating verdict");
    };
    assert_eq!(documents.as_slice().len(), 1);
}

#[test]
fn test_convention_shipping_verdicts_compare_by_the_documents_they_name() {
    let one = ConventionShippingVerdict::UnsuppliedDocumentsShipped {
        documents: NonEmptyVec::new(
            document("knowledge/conventions/language-policy.md"),
            Vec::new(),
        ),
    };
    let same = one.clone();
    let other = ConventionShippingVerdict::UnsuppliedDocumentsShipped {
        documents: NonEmptyVec::new(
            document("knowledge/conventions/typed-deserialization.md"),
            Vec::new(),
        ),
    };

    assert_eq!(one, same);
    assert_ne!(one, other);
    assert_ne!(one, ConventionShippingVerdict::Conforming);
}

// ---------------------------------------------------------------------------
// ConventionShippingCheckError
// ---------------------------------------------------------------------------

#[test]
fn test_convention_root_missing_names_the_tree_that_holds_no_conventions() {
    // An exported tree whose convention directory is absent is the shape the
    // tree takes when the boundary classification is edited away from
    // `overlay`. It is a failure here rather than a vacuous pass.
    let error = ConventionShippingCheckError::ConventionRootMissing {
        tree_root: PathBuf::from("/var/tmp/export-42"),
    };

    assert!(
        error.to_string().contains("/var/tmp/export-42"),
        "the diagnostic names the tree that has no convention root: {error}"
    );
    assert!(error.source().is_none());
}

#[test]
fn test_tree_unreadable_carries_the_unreadable_path_and_its_diagnostic() {
    let error = ConventionShippingCheckError::TreeUnreadable {
        path: PathBuf::from("/var/tmp/export-42/knowledge/conventions"),
        reason: FreeText::new("permission denied (os error 13)".to_owned()),
    };

    let rendered = error.to_string();
    assert!(rendered.contains("/var/tmp/export-42/knowledge/conventions"), "{rendered}");
    assert!(rendered.contains("permission denied (os error 13)"), "{rendered}");
}

#[test]
fn test_document_path_rejected_composes_the_constructor_rejection_unchanged() {
    let rejection = ConventionDocumentPath::try_new(PathBuf::from("knowledge/adr/README.md"))
        .expect_err("a path outside the convention root is rejected by the constructor");

    let error = ConventionShippingCheckError::DocumentPathRejected {
        tree_root: PathBuf::from("overlay"),
        source: rejection,
    };

    let source = error.source().expect("the constructor rejection is exposed as the error source");
    assert!(
        source.to_string().contains("knowledge/adr/README.md"),
        "the rejected path is read from the source, which owns the rule: {source}"
    );
    assert!(
        matches!(
            error,
            ConventionShippingCheckError::DocumentPathRejected {
                source: ConventionDocumentPathError::OutsideConventionRoot { .. },
                ..
            }
        ),
        "the composed rejection reaches the caller as the variant the constructor produced"
    );
}

#[test]
fn test_document_path_rejected_attributes_the_rejection_to_a_tree() {
    // `tree_root` is what the walk adds to the constructor's rejection: without
    // it the same rejected path could not be attributed to the exported tree
    // rather than the overlay.
    for tree_root in ["/var/tmp/export-42", "overlay"] {
        let rejection = ConventionDocumentPath::try_new(PathBuf::from("knowledge/adr/README.md"))
            .expect_err("a path outside the convention root is rejected");
        let error = ConventionShippingCheckError::DocumentPathRejected {
            tree_root: PathBuf::from(tree_root),
            source: rejection,
        };

        assert!(
            error.to_string().contains(tree_root),
            "the diagnostic names the tree the path was inventoried from: {error}"
        );
    }
}

#[test]
fn test_document_path_rejected_does_not_restate_the_rejection_rule() {
    let rejection = ConventionDocumentPath::try_new(PathBuf::from("knowledge/adr/README.md"))
        .expect_err("a path outside the convention root is rejected");
    let error = ConventionShippingCheckError::DocumentPathRejected {
        tree_root: PathBuf::from("overlay"),
        source: rejection,
    };

    let rendered = error.to_string();
    assert!(
        !rendered.contains("knowledge/adr/README.md"),
        "the offending path lives in the source, so this variant does not copy it: {rendered}"
    );
    assert!(
        !rendered.contains("outside"),
        "restating the rule here would put a second wording of it beside the original: {rendered}"
    );
}
