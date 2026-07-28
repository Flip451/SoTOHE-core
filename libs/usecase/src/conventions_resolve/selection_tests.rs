//! Tests for [`select_required_conventions`], the pure aggregation step of
//! convention resolution (spec `AC-06`, `AC-08`).
//!
//! A separate file from the module that defines the function only so that
//! neither outgrows the workspace module-size limit. The function itself stays
//! in the parent module because a free function is catalogued under the module
//! that defines it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use super::{
    ConventionCapabilityId, ConventionDocumentPath, ConventionRequirement, ConventionResolution,
    select_required_conventions,
};

fn document(path: &str) -> ConventionDocumentPath {
    ConventionDocumentPath::try_new(PathBuf::from(path)).unwrap()
}

fn capability(id: &str) -> ConventionCapabilityId {
    ConventionCapabilityId::try_new(id).unwrap()
}

fn requirement(path: &str, declared: &[&str]) -> ConventionRequirement {
    ConventionRequirement::new(document(path), declared.iter().map(|id| capability(id)).collect())
}

fn paths(resolution: &ConventionResolution) -> Vec<String> {
    resolution.documents().iter().map(ToString::to_string).collect()
}

#[test]
fn test_select_required_conventions_returns_the_documents_declaring_the_capability() {
    let requirements = [
        requirement("knowledge/conventions/testing.md", &["implementer", "reviewer"]),
        requirement("knowledge/conventions/adr.md", &["reviewer"]),
        requirement("knowledge/conventions/rust/naming.md", &["implementer"]),
        requirement("knowledge/conventions/git-notes.md", &[]),
    ];

    let resolved = select_required_conventions(&requirements, &capability("implementer"));

    assert_eq!(
        paths(&resolved),
        ["knowledge/conventions/rust/naming.md", "knowledge/conventions/testing.md"],
        "a document is selected when it declares the requested capability, whether or not it \
         declares others, and a document declaring only some other capability is not"
    );
}

#[test]
fn test_select_required_conventions_applies_no_normalization_of_its_own() {
    let requirements = [requirement("knowledge/conventions/testing.md", &["implementer"])];

    assert_eq!(
        paths(&select_required_conventions(&requirements, &capability("implementer"))),
        ["knowledge/conventions/testing.md"]
    );
    for near_miss in ["implement", "implementer-lead", "Implementer", " implementer "] {
        assert!(
            select_required_conventions(&requirements, &capability(near_miss)).is_empty(),
            "'{near_miss}' is rejected by the requirement's own comparison, and this function \
             neither loosens that decision nor re-takes it"
        );
    }
}

#[test]
fn test_select_required_conventions_returns_the_canonical_resolution_not_the_scan_order() {
    let one = [
        requirement("knowledge/conventions/testing.md", &["implementer"]),
        requirement("knowledge/conventions/rust/naming.md", &["implementer"]),
        requirement("knowledge/conventions/adr.md", &["implementer"]),
        // The same document scanned twice — two matches, one document.
        requirement("knowledge/conventions/testing.md", &["implementer"]),
    ];
    let other = [
        requirement("knowledge/conventions/adr.md", &["implementer"]),
        requirement("knowledge/conventions/testing.md", &["implementer"]),
        requirement("knowledge/conventions/rust/naming.md", &["implementer"]),
    ];

    let from_one = select_required_conventions(&one, &capability("implementer"));
    let from_other = select_required_conventions(&other, &capability("implementer"));

    assert_eq!(
        paths(&from_one),
        [
            "knowledge/conventions/adr.md",
            "knowledge/conventions/rust/naming.md",
            "knowledge/conventions/testing.md",
        ],
        "the matches are returned through the result's canonical constructor, so they carry its \
         deduplication and ordering rather than the order they were scanned in"
    );
    assert_eq!(from_one, from_other, "two scan orders over the same matches select the same value");
}

#[test]
fn test_select_required_conventions_with_no_matching_document_returns_an_empty_success() {
    let requirements = [
        requirement("knowledge/conventions/testing.md", &["implementer"]),
        requirement("knowledge/conventions/adr.md", &["reviewer"]),
    ];

    let resolved = select_required_conventions(&requirements, &capability("researcher"));

    assert!(resolved.is_empty());
    assert_eq!(
        resolved,
        ConventionResolution::default(),
        "a capability matching zero documents is an ordinary empty result; the signature offers \
         no failure for it to become"
    );
}

#[test]
fn test_select_required_conventions_treats_documents_declaring_nothing_as_ordinary_input() {
    // A document with no front matter and a document whose front matter
    // carries no `required_for` both reach this function as a requirement
    // declaring nothing, so neither is a state it has to recognise.
    let declaring_nothing = [
        requirement("knowledge/conventions/git-notes.md", &[]),
        requirement("knowledge/conventions/adr.md", &[]),
    ];

    let resolved = select_required_conventions(&declaring_nothing, &capability("implementer"));

    assert!(resolved.is_empty());
    assert_eq!(
        select_required_conventions(&[], &capability("implementer")),
        resolved,
        "a scan that found nothing to declare and a scan that found nothing at all select the \
         same empty result"
    );
}

#[test]
fn test_select_required_conventions_is_pure_over_the_already_scanned_input() {
    let requirements = [
        requirement("knowledge/conventions/testing.md", &["implementer"]),
        requirement("knowledge/conventions/adr.md", &["reviewer"]),
    ];
    let before = requirements.clone();

    let first = select_required_conventions(&requirements, &capability("implementer"));
    let second = select_required_conventions(&requirements, &capability("implementer"));

    assert_eq!(first, second, "selecting twice over the same input selects the same result");
    assert_eq!(
        requirements, before,
        "the requirements are borrowed and left as they were: the selection creates, updates and \
         deletes nothing, and its parameters carry no handle through which it could"
    );
}
