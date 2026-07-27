//! Reading one convention document, and naming one when it cannot be read.
//!
//! Kept in a sibling module so the codec, the scan, and the adapter stay inside
//! the parent module's length limit. What lives here is everything about a
//! single document as a document: the bound on how much of one this walk will
//! hold, the read that enforces it, the lift of the path rule that gives a
//! document its repository-relative identity, and the failure that names one.
//! The traversal that decides *which* documents to read is the parent's, and
//! the listing primitives it walks with are `directory_walk`'s.

use std::io::Read;
use std::path::{Path, PathBuf};

use usecase::capability_exec::CapabilityFailureDetail;
use usecase::conventions_resolve::{ConventionDocumentPath, ConventionResolveError};

/// Largest document this walk will read, in bytes.
///
/// A convention document is prose someone wrote, so nothing near this size is
/// one. The bound is here because the alternative is not a smaller read but an
/// uncatchable failure: reading a file whole grows an allocation to whatever
/// the filesystem offers, and a document that is enormous — or sparse, or still
/// being written — would exhaust the process rather than produce a value the
/// caller can handle. Exceeding it is an unreadable document, `AC-07`'s
/// condition this walk decides, so the bound reports through the same variant
/// as any other failed read and names the document it stopped on.
///
/// This is an adapter's self-defence and not a spec condition: `AC-07` says
/// nothing about size, and the number is a limit on what this walk will hold at
/// once rather than a rule about what a convention document may be.
pub(super) const MAX_DOCUMENT_BYTES: u64 = 4 * 1024 * 1024;

/// Reads the document `name` under `parent`, holding at most
/// [`MAX_DOCUMENT_BYTES`] of it.
///
/// `name` is the entry under the handle that listed it and `document` the
/// validated repository-relative identity the failure is reported under; the
/// two are the same file, named the two ways this walk has to name it.
pub(super) fn read_document_at(
    parent: &std::fs::File,
    name: &Path,
    document: &ConventionDocumentPath,
) -> Result<String, ConventionResolveError> {
    // The listing decided this entry was a regular file, but the entry and the
    // open are two moments and the node can be replaced between them. The flags
    // are what carry that decision into the open itself, the way
    // `capability_exec::process::runtime_dir` opens its runtime files:
    // `NOFOLLOW` so a link substituted here fails instead of being followed out
    // of the tree, and `NONBLOCK` so a FIFO returns instead of waiting for a
    // writer. What the flags cannot judge is the node's type, so the opened
    // handle is asked directly below.
    let file = rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(std::fs::File::from)
    .map_err(|error| unreadable(document, std::io::Error::from(error)))?;

    // Asked of the handle rather than of the path, so it describes the file
    // that was actually opened and not whatever the name refers to by now.
    let opened = file.metadata().map_err(|error| unreadable(document, error))?;
    if !opened.is_file() {
        return Err(unreadable(document, "document is no longer a regular file"));
    }

    // One byte past the bound, so that reaching the bound exactly is a document
    // read whole and anything longer is observably longer rather than silently
    // truncated to the limit.
    let mut bytes = Vec::new();
    file.take(MAX_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| unreadable(document, error))?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(unreadable(
            document,
            format!("document is larger than the {MAX_DOCUMENT_BYTES}-byte read bound"),
        ));
    }

    // Decoding after the bound rather than while reading: a document that is
    // both oversized and not UTF-8 is reported as oversized, which is the
    // condition that stopped the read.
    String::from_utf8(bytes).map_err(|error| unreadable(document, error))
}

/// Lifts [`ConventionDocumentPath::try_new`]'s rejection of `candidate`.
///
/// The constructor makes two rejections and this walk stands differently to
/// each. It does not present an escaping path: every candidate is a path the
/// constructor has already accepted joined with one name a directory listing
/// produced, and such a name is never empty, is never `.` or `..`, and carries
/// neither a separator nor a root — so the joined path is inside the convention
/// root whenever the path it extends was. It does present an unrenderable one,
/// because a directory entry may be named with bytes that are not UTF-8 or that
/// hold a line terminator, and joining such a name onto an accepted path yields
/// a path the constructor refuses.
///
/// So this lift has a live producer and is not merely defensive. Its callers
/// are placed accordingly: an entry the walk would not read is filtered before
/// reaching here, so only a document the walk means to present, a link it
/// refuses, or a directory it means to descend into, can fail the scan on its
/// name.
///
/// The lift would be kept even without that producer, since removing it would
/// mean the walk deciding the path rule itself, which is exactly what having
/// one enforcing site exists to prevent.
pub(super) fn document_path(
    candidate: PathBuf,
) -> Result<ConventionDocumentPath, ConventionResolveError> {
    ConventionDocumentPath::try_new(candidate)
        .map_err(|source| ConventionResolveError::DocumentPathRejected { source })
}

/// Builds the unreadable-document failure for `document`.
///
/// Takes the detail as anything printable rather than as an [`std::io::Error`],
/// because not every way a document is unreadable comes from the filesystem:
/// exceeding [`MAX_DOCUMENT_BYTES`] is this walk's own judgement about a read it
/// will not finish, and a link it refuses to follow is unreadable before any
/// read is attempted.
pub(super) fn unreadable(
    document: &ConventionDocumentPath,
    detail: impl std::fmt::Display,
) -> ConventionResolveError {
    ConventionResolveError::DocumentUnreadable {
        document: document.clone(),
        detail: CapabilityFailureDetail::new(detail.to_string()),
    }
}
