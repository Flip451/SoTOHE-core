//! Tests for the fail-closed condition set of [`ConventionResolveError`] and
//! for the two rejections it lifts (spec `AC-07`).
//!
//! A separate file from the module that declares the type only so that neither
//! outgrows the workspace module-size limit.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::error::Error as _;
use std::marker::PhantomData;
use std::path::PathBuf;

use super::{
    ConventionCapabilityId, ConventionCapabilityIdError, ConventionDocumentPath,
    ConventionDocumentPathError, ConventionResolveError,
};
use crate::capability_exec::CapabilityFailureDetail;

fn document(path: &str) -> ConventionDocumentPath {
    ConventionDocumentPath::try_new(PathBuf::from(path)).unwrap()
}

/// Answers `false` for every type, and is shadowed by the inherent constants
/// below whenever their bound holds.
///
/// Observing the *absence* of a `From` impl needs this two-step resolution:
/// an inherent associated constant whose bound does not hold falls back to the
/// blanket trait one, so the value reports whether the conversion exists.
trait NoFromImpl {
    const LIFTS_IMPLICITLY: bool = false;
}

impl<T: ?Sized> NoFromImpl for T {}

/// Probe for `T: From<ConventionDocumentPathError>`.
#[allow(dead_code)]
struct LiftsPathError<T>(PhantomData<T>);

#[allow(dead_code)]
impl<T: From<ConventionDocumentPathError>> LiftsPathError<T> {
    const LIFTS_IMPLICITLY: bool = true;
}

/// Probe for `T: From<ConventionCapabilityIdError>`.
#[allow(dead_code)]
struct LiftsCapabilityIdError<T>(PhantomData<T>);

#[allow(dead_code)]
impl<T: From<ConventionCapabilityIdError>> LiftsCapabilityIdError<T> {
    const LIFTS_IMPLICITLY: bool = true;
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
fn test_document_path_outside_root_lifts_each_constructor_rejection_as_its_source() {
    // Every shape of path that does not name a document inside the convention
    // root: a sibling directory, a parent-directory escape, an absolute path,
    // and the root itself.
    for escaping in [
        "knowledge/adr/README.md",
        "knowledge/conventions/../adr/README.md",
        "/srv/knowledge/conventions/testing.md",
        "knowledge/conventions",
    ] {
        let supplied = PathBuf::from(escaping);
        let Err(rejection) = ConventionDocumentPath::try_new(supplied.clone()) else {
            panic!("'{escaping}' does not name a document inside the convention root");
        };

        let error = ConventionResolveError::DocumentPathOutsideRoot { source: rejection };

        let cause = error.source().expect("the composed path rejection must be the error source");
        let Some(ConventionDocumentPathError::OutsideConventionRoot { path }) =
            cause.downcast_ref::<ConventionDocumentPathError>()
        else {
            panic!("the source must be the constructor rejection itself, carried unchanged");
        };
        assert_eq!(
            path, &supplied,
            "the rejected path travels inside the composed rejection, so this variant restates \
             neither the path nor the rule that rejected it"
        );
    }
}

#[test]
fn test_empty_capability_id_supplies_the_document_its_rejection_cannot_carry() {
    let Err(rejection) = ConventionCapabilityId::try_new("   ") else {
        panic!("expected a blank identifier to be rejected");
    };
    assert!(
        !rejection.to_string().contains("knowledge/conventions/"),
        "the rejection names no document, so composing it alone would lose which document \
         spelled the blank identifier"
    );

    let error = ConventionResolveError::EmptyCapabilityId {
        document: document("knowledge/conventions/adr.md"),
    };

    assert!(
        error.source().is_none(),
        "this variant composes nothing: it carries the document instead, which is why it could \
         not have been a conversion from the rejection"
    );
    assert!(error.to_string().contains("knowledge/conventions/adr.md"));
}

#[test]
fn test_convention_resolve_error_declares_no_from_impl_for_either_lifted_rejection() {
    let lifts_path_error = <LiftsPathError<ConventionResolveError>>::LIFTS_IMPLICITLY;
    let lifts_capability_id_error =
        <LiftsCapabilityIdError<ConventionResolveError>>::LIFTS_IMPLICITLY;

    // Both probes report `true` where the conversion does exist, so `false`
    // below is the absence of an impl rather than a probe that never fires.
    let control_path_error = <LiftsPathError<ConventionDocumentPathError>>::LIFTS_IMPLICITLY;
    let control_capability_id_error =
        <LiftsCapabilityIdError<ConventionCapabilityIdError>>::LIFTS_IMPLICITLY;
    assert!(control_path_error);
    assert!(control_capability_id_error);

    assert!(
        !lifts_path_error,
        "no `From<ConventionDocumentPathError>`: a caller cannot raise this error with `?`, so \
         `DocumentPathOutsideRoot` is only ever built at a named site"
    );
    assert!(
        !lifts_capability_id_error,
        "no `From<ConventionCapabilityIdError>`: `EmptyCapabilityId` needs the document, which \
         a conversion has no way to supply"
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
