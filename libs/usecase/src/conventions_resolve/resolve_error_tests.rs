//! Tests for the fail-closed condition set of [`ConventionResolveError`], for
//! the two rejections it lifts, and for the record-renderability half of the
//! path rejection (spec `AC-07`).
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
fn test_convention_document_path_with_a_line_terminator_returns_not_renderable_error() {
    // Each name is a single file under the convention root, so nothing but the
    // record invariant rejects it. `\n` would make one file appear as two
    // records; a trailing `\r` is stripped by a CRLF-accepting reader, which
    // would collapse the record onto a differently named file.
    for offending in [
        "knowledge/conventions/render-fixture-split\nrecord.md",
        "knowledge/conventions/render-fixture-trailing.md\n",
        "knowledge/conventions/render-fixture-carriage\rreturn.md",
        "knowledge/conventions/render-fixture-crlf.md\r\n",
    ] {
        let supplied = PathBuf::from(offending);

        let result = ConventionDocumentPath::try_new(supplied.clone());

        let Err(ConventionDocumentPathError::NotRenderableAsRecord { path }) = result else {
            panic!(
                "'{}' cannot be one record, so it is not a document path",
                offending.escape_debug()
            );
        };
        assert_eq!(
            path, supplied,
            "the rejection names the offending document, as the escape rejection does"
        );
    }
}

#[cfg(unix)]
#[test]
fn test_convention_document_path_with_a_non_utf8_name_returns_not_renderable_error() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    // Two distinct files under the convention root, differing only in one byte
    // that is invalid UTF-8 in any position.
    let mut first = PathBuf::from("knowledge/conventions");
    first.push(OsStr::from_bytes(b"render-fixture-lone-byte-\x80.md"));
    let mut second = PathBuf::from("knowledge/conventions");
    second.push(OsStr::from_bytes(b"render-fixture-lone-byte-\xfe.md"));

    assert_ne!(first, second, "the two fixtures name different files");
    assert_eq!(
        first.display().to_string(),
        second.display().to_string(),
        "their rendered forms are identical, which is why accepting them would make a record \
         unable to say which file it came from"
    );

    for supplied in [first, second] {
        let result = ConventionDocumentPath::try_new(supplied.clone());

        let Err(ConventionDocumentPathError::NotRenderableAsRecord { path }) = result else {
            panic!("a non-UTF-8 name has no lossless rendered form, so it is not a document path");
        };
        assert_eq!(
            path, supplied,
            "the rejection names the offending document, as the escape rejection does"
        );
        assert!(
            path.to_str().is_none(),
            "the rejected path is carried as the bytes supplied, not as a lossy substitute"
        );
    }
}

#[test]
fn test_convention_resolve_error_declares_every_variant_with_no_wildcard_arm() {
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
        ConventionResolveError::DocumentPathRejected { source: outside_root },
        ConventionResolveError::DocumentUnreadable {
            document: document("knowledge/conventions/unreadable.md"),
            detail: CapabilityFailureDetail::new("permission denied"),
        },
        ConventionResolveError::ConventionRootUnlistable {
            root: PathBuf::from("knowledge/conventions"),
            detail: CapabilityFailureDetail::new("permission denied"),
        },
    ];

    // One arm per variant and no wildcard: a further variant — including one
    // for a normal empty resolution — stops this test compiling. The count is
    // deliberately absent from the name: `AC-07` states its cases are not
    // exhaustive, and one variant here composes two of them, so the number of
    // variants is not the number of conditions.
    let concerned: Vec<PathBuf> = conditions
        .iter()
        .map(|condition| match condition {
            ConventionResolveError::FrontMatterUnparseable { document, .. }
            | ConventionResolveError::RequiredForNotStringArray { document, .. }
            | ConventionResolveError::EmptyCapabilityId { document }
            | ConventionResolveError::DocumentUnreadable { document, .. } => {
                document.as_path().to_path_buf()
            }
            ConventionResolveError::DocumentPathRejected {
                source:
                    ConventionDocumentPathError::OutsideConventionRoot { path }
                    | ConventionDocumentPathError::NotRenderableAsRecord { path },
            } => path.clone(),
            ConventionResolveError::ConventionRootUnlistable { root, .. } => root.clone(),
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
            PathBuf::from("knowledge/conventions"),
        ],
        "every condition carries the document it concerns, the escaped path through the \
         composed constructor rejection rather than a restatement of it"
    );
}

#[test]
fn test_document_path_rejected_lifts_each_constructor_rejection_as_its_source() {
    // Every shape of path the constructor rejects: a sibling directory, a
    // parent-directory escape, an absolute path, the root itself, and — inside
    // the root but unable to be one record — a name holding a line terminator.
    for rejected in [
        "knowledge/adr/README.md",
        "knowledge/conventions/../adr/README.md",
        "/srv/knowledge/conventions/testing.md",
        "knowledge/conventions",
        "knowledge/conventions/lift-fixture-split\nrecord.md",
    ] {
        let supplied = PathBuf::from(rejected);
        let Err(rejection) = ConventionDocumentPath::try_new(supplied.clone()) else {
            panic!("'{}' is not a convention document path", rejected.escape_debug());
        };

        let error = ConventionResolveError::DocumentPathRejected { source: rejection };

        let cause = error.source().expect("the composed path rejection must be the error source");
        let Some(
            ConventionDocumentPathError::OutsideConventionRoot { path }
            | ConventionDocumentPathError::NotRenderableAsRecord { path },
        ) = cause.downcast_ref::<ConventionDocumentPathError>()
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
         `DocumentPathRejected` is only ever built at a named site"
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
