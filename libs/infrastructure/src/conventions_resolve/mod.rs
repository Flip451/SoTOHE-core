//! Filesystem side of convention resolution (spec `IN-05`, `AC-06`, `AC-07`,
//! `AC-08`).
//!
//! This module holds the serde boundary of convention resolution — it turns one
//! document's text into the declarations its front matter carries — and the
//! read-only directory scan that feeds it, together with the adapter that
//! injects that scan as [`ConventionRequirementPort`].
//!
//! Two of `AC-07`'s five conditions are decided here and nowhere else: the
//! decode conditions belong to [`parse_convention_front_matter`] — front matter
//! that is not parseable YAML, and a `required_for` value that is not an array
//! of strings — and the unreadable document belongs to
//! [`scan_convention_requirements`]. The remaining conditions belong elsewhere:
//! the path rule to [`ConventionDocumentPath::try_new`], whose rejection the
//! scan only lifts, and the empty identifier to
//! `ConventionCapabilityId::try_new`, whose rejection
//! [`ConventionFrontMatterDto::into_requirement`] only translates.

#[cfg(test)]
mod front_matter_tests;
#[cfg(test)]
mod scan_tests;

use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};

use usecase::capability_exec::CapabilityFailureDetail;
use usecase::conventions_resolve::{
    ConventionDocumentPath, ConventionRequirement, ConventionRequirementPort,
    ConventionResolveError,
};

use crate::capability_exec::{YAML_LINE_BREAKS, read_front_matter};
use crate::track::symlink_guard::{is_symlink_rejection, reject_symlinks_below};
mod front_matter_dto;

pub use front_matter_dto::{CapabilityIdField, ConventionFrontMatterDto};

/// Decodes one convention document's front matter (`AC-06`, `AC-07`, `AC-08`).
///
/// The front-matter block is located by the crate's existing block reader, the
/// one provider-definition parsing already uses, so the repository has a single
/// notion of where a front-matter block starts and ends — including which line
/// endings delimit it, so a document is read the same way whichever style the
/// consumer's checkout uses.
///
/// A document with no block, a block holding nothing, and a block declaring no
/// `required_for` all decode to [`ConventionFrontMatterDto::default`]: `AC-08`
/// makes those normal empty states, not failures.
///
/// # Errors
///
/// Returns [`ConventionResolveError::FrontMatterUnparseable`] when the block is
/// unclosed, is not parseable YAML, or does not hold a YAML mapping, and
/// [`ConventionResolveError::RequiredForNotStringArray`] when `required_for` is
/// present with any other shape than an array of strings.
pub fn parse_convention_front_matter(
    document: &ConventionDocumentPath,
    content: &str,
) -> Result<ConventionFrontMatterDto, ConventionResolveError> {
    // The block reader admits exactly one rejection, an unclosed block, and its
    // message names the provider definition it was written for, so the
    // condition is re-stated here in this codec's vocabulary.
    let block = read_front_matter(content)
        .map_err(|_| unparseable(document, "front matter block is not closed by a '---' line"))?;
    let Some(block) = block else {
        return Ok(ConventionFrontMatterDto::default());
    };

    // The parser runs first, on every block, so that nothing this codec judges
    // for itself can let malformed front matter through unparsed. An earlier
    // arrangement asked `declares_nothing` before parsing and returned the
    // default on a `true`, which made every disagreement between that test and
    // the parser a silent empty declaration — tab indentation and control
    // characters are two the parser refuses and a lexical test does not.
    let front_matter: serde_yaml::Value =
        serde_yaml::from_str(block).map_err(|error| unparseable(document, &error))?;

    // Only now, and only for the one distinction the parsed value cannot carry.
    // `serde_yaml` gives an empty block and an explicit `null` the same
    // `Value::Null` and one document either way, so a null alone cannot say
    // which it was. A block whose lines are all blank or comments declared
    // nothing, which `AC-08` makes a normal empty state; a block that wrote
    // `null`, `~`, or `Null` wrote a scalar, and a scalar cannot present
    // `required_for`, so it is refused with every other non-mapping below.
    if front_matter.is_null() && declares_nothing(block) {
        return Ok(ConventionFrontMatterDto::default());
    }

    // Rejecting a non-mapping block here, and reading the mapping's entries by
    // key value rather than by field identifier, is what leaves the shape error
    // below attributable to `required_for` alone. An explicit null reaches this
    // check as the scalar it is and is refused with every other non-mapping.
    if !front_matter.is_mapping() {
        return Err(unparseable(document, "front matter does not hold a YAML mapping"));
    }

    serde_yaml::from_value(front_matter).map_err(|error| {
        ConventionResolveError::RequiredForNotStringArray {
            document: document.clone(),
            detail: CapabilityFailureDetail::new(error.to_string()),
        }
    })
}

/// Reports whether `block` carries no YAML node at all.
///
/// A block is empty when every line is blank or a comment, because neither
/// contributes a node: a block holding only those declared nothing, which
/// `AC-08` makes a normal empty state. An explicit `null`, `~`, or `Null` is
/// deliberately not empty by this test — it is a scalar the document wrote, and
/// one that cannot present `required_for`, so it is left to the mapping check.
///
/// Lines are separated on every break form the parser recognises, which is
/// wider than either Rust or a reading of YAML 1.2 would suggest: the `libyaml`
/// scanner behind `serde_yaml` breaks on `\n`, `\r`, NEL (U+0085), LS (U+2028),
/// and PS (U+2029). Splitting on the characters individually handles a CRLF
/// pair by yielding an empty segment between them, which is itself a blank
/// line. [`str::lines`] is deliberately not used: it splits on none of the last
/// three and not on a lone `\r`, so a comment ending in one of them would
/// absorb the declaration on the YAML line after it — verified against the
/// parser rather than inferred from the specification, because the two differ
/// here.
///
/// Only space and tab are stripped before a line is judged, that being the
/// whole of YAML's separation whitespace. [`str::trim`] is deliberately not
/// used: it strips the entire Unicode `White_Space` class, which includes
/// characters YAML does not treat as separation — U+00A0 among them. Stripping
/// one of those would end a line's indentation where YAML does not, so a `#`
/// after it would look like the start of a comment when the parser reads the
/// line as a plain scalar, and this function would report an empty declaration
/// for front matter the parser was about to refuse.
///
/// Both choices are the same rule: every set this test uses is YAML's, spelled
/// out, rather than the nearest Rust convenience. Any test here more permissive
/// than the parser is a route for malformed metadata to be read as declaring
/// nothing, which is the collapse the explicit-null case above already shows,
/// and each Rust abstraction substituted for a YAML rule has been a notch off
/// in a different place.
///
/// Whole lines are examined rather than the block being scanned for a `#`, so a
/// `#` inside a value cannot make a block holding real entries look empty: one
/// line carrying content is enough for the block to have a node, which also
/// keeps a block scalar's literal `#` lines from being read as comments.
fn declares_nothing(block: &str) -> bool {
    block.split(YAML_LINE_BREAKS).all(|line| {
        let line = line.trim_matches([' ', '\t']);
        line.is_empty() || line.starts_with('#')
    })
}

/// Builds the front-matter parse failure for `document`.
fn unparseable(
    document: &ConventionDocumentPath,
    detail: impl std::fmt::Display,
) -> ConventionResolveError {
    ConventionResolveError::FrontMatterUnparseable {
        document: document.clone(),
        detail: CapabilityFailureDetail::new(detail.to_string()),
    }
}

/// Repository-relative directory the scan walks.
///
/// Spelled here as well as in `usecase` because a walk has to open a directory,
/// and a directory to open is not something [`ConventionDocumentPath`] hands
/// back. This is not a second enforcing site of the path rule: every path the
/// walk produces is still built through [`ConventionDocumentPath::try_new`], so
/// a value here that drifted away from the root that constructor enforces would
/// make the walk reject its own documents rather than quietly admit a tree from
/// outside the convention root.
const CONVENTION_ROOT: &str = "knowledge/conventions";

/// Extension of the documents `IN-05`'s `knowledge/conventions/**/*.md` names.
const DOCUMENT_EXTENSION: &str = "md";

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
const MAX_DOCUMENT_BYTES: u64 = 4 * 1024 * 1024;

/// Most entries this walk will hold from one directory listing.
///
/// The listing is materialised before it is walked so that descending into a
/// subdirectory does not require keeping the parent's directory handle open:
/// streaming it instead would hold one handle per level down to
/// [`MAX_DIRECTORY_DEPTH`]. Holding the listing makes its size this walk's
/// problem rather than the filesystem's, and this is where that is answered.
/// The same reasoning and the same reporting route as [`MAX_DOCUMENT_BYTES`]
/// apply, and the bound sits far above any convention tree a person would
/// author.
const MAX_DIRECTORY_ENTRIES: usize = 10_000;

/// Deepest nesting this walk will descend to below the convention root.
///
/// The walk recurses, so depth is stack rather than policy: a tree nested
/// deeply enough would overflow before any per-directory bound noticed, and an
/// overflow is not a failure a caller can catch. The bound is far past any
/// nesting a convention tree would have, and it is reported as the containing
/// directory being unreadable — which, at that depth, is what it is to this
/// walk.
const MAX_DIRECTORY_DEPTH: usize = 64;

/// Scans `project_root`'s convention tree and pairs every document with the
/// capability identifiers its front matter declares (`IN-05`, `AC-06`,
/// `AC-07`).
///
/// The walk is read-only and covers `knowledge/conventions/**/*.md` below
/// `project_root`: it opens directories and reads files and does nothing else,
/// so `AC-06`'s promise that resolution neither creates, updates, deletes, nor
/// indexes a document holds by the operations available here rather than by
/// convention.
///
/// This is the single enforcing site of `AC-07`'s unreadable-document
/// condition. The path condition is [`ConventionDocumentPath::try_new`]'s
/// decision and is only lifted here, and the two rejections that constructor
/// makes reach the lift differently. Its escape rejection is not something this
/// walk can present: every candidate is an already-accepted path joined with
/// one directory-entry name, which carries neither a separator nor a root nor a
/// parent-directory component, so what the walk owes that half is not producing
/// an escaping path in the first place, which the lift then confirms. Its
/// record-renderability rejection this walk does reach, because a directory
/// entry may be named with bytes that are not UTF-8 or that hold a line
/// terminator; such a document is presented as
/// [`ConventionResolveError::DocumentPathRejected`] rather than skipped, since
/// it is a document the walk found and cannot name.
///
/// The rule is asked only of complete document paths, and only of documents the
/// walk actually presents. A neighbour with another extension, a node that is
/// not a regular file, and a directory are all settled before it is applied, so
/// an unrepresentable name costs the consumer their conventions exactly when it
/// belongs to a document and never when it belongs to something the scan was
/// not going to return. A directory named unrepresentably therefore fails the
/// scan if it holds a `*.md` document — the document's own path carries that
/// name — and is walked without complaint if it does not.
///
/// The two decode conditions and the empty identifier arrive already decided by
/// [`parse_convention_front_matter`] and
/// [`ConventionFrontMatterDto::into_requirement`], which this walk calls in
/// turn — the codec returns a decoded view, and turning that view into a
/// [`ConventionRequirement`] is what the scan adds.
///
/// Symlinks are not followed, whether they name a directory or a document.
/// Following one would let the walk leave the convention tree while every path
/// it produced still looked repository-relative, which is precisely the escape
/// the path rule exists to refuse: for a directory the walk itself would leave
/// the tree, and for a document the path would stay inside it while the bytes
/// came from wherever the link pointed, so the path rule would be guaranteeing
/// nothing about what was actually read. Not descending a directory link also
/// bounds the recursion by the real tree's depth.
///
/// That skip is applied to entries a listing reported, so it cannot see the two
/// components the walk reaches by joining rather than by listing: `knowledge`
/// and `knowledge/conventions` themselves. A link at either would be resolved
/// by the listing call before any entry existed to inspect, and the walk would
/// present another tree's files under this one's paths — the same escape,
/// arriving one level above where the walk looks for it. Both are therefore
/// checked here before the root is listed. `project_root` above them is the
/// caller's to vouch for: it is the tree the port was asked about, not
/// something this walk resolved.
///
/// A skipped link is simply not a document this walk presents, the same way a
/// linked subdirectory contributes none. So is anything that is not a regular
/// file — a FIFO, a socket, a device — which is excluded before the read
/// rather than at it, since opening a FIFO named like a document would block
/// this walk for as long as no one wrote to it. That is a decision about which
/// documents the walk offers and not a sixth fail-closed condition: `AC-07`
/// names five, and none of them is "is a symbolic link" or "is not a regular
/// file".
///
/// The documents arrive in whatever order the directory listings produced, and
/// this walk imposes none of its own. Ordering and deduplication are
/// [`usecase::conventions_resolve::ConventionResolution`]'s, which sorts and
/// deduplicates what it is given precisely so that no consumer re-sorts and no
/// two consumers can disagree about the order. Establishing an order here as
/// well would be a second site deciding a rule that type already owns — the
/// same duplication the single-enforcing-site split on this track exists to
/// prevent — so a caller that needs a stable order gets it from that
/// constructor and not from here.
///
/// # Errors
///
/// Returns [`ConventionResolveError::DocumentUnreadable`] when a document or a
/// directory below the convention root cannot be read, including when either
/// exceeds the bound this walk reads it under (`MAX_DOCUMENT_BYTES`,
/// `MAX_DIRECTORY_ENTRIES`), [`ConventionResolveError::DocumentPathRejected`]
/// when a document or a directory the walk would descend into is named in a way
/// [`ConventionDocumentPath::try_new`] refuses, and the failure
/// [`parse_convention_front_matter`] or
/// [`ConventionFrontMatterDto::into_requirement`] decided for the document that
/// carries it.
pub fn scan_convention_requirements(
    project_root: &Path,
) -> Result<Vec<ConventionRequirement>, ConventionResolveError> {
    let convention_root = project_root.join(CONVENTION_ROOT);

    // A convention root that is absent, is not a directory, cannot be listed,
    // or is reached through a link presents no documents, which `AC-08` makes
    // an ordinary empty result. It is also the only path this walk touches that
    // is not itself a document — `ConventionDocumentPath` rejects the root —
    // and so the only path whose failure none of `AC-07`'s five
    // document-shaped conditions can name. Everything below it is fail-closed.
    //
    // The link check runs first and covers `knowledge` as well, because a
    // listing of a linked root would already have produced entries from
    // another tree by the time any of them could be inspected.
    match reject_symlinks_below(&convention_root, project_root) {
        // A link on the way to the root, or at it: the tree reached through it
        // is not the one the caller named, and presenting its documents under
        // repository-relative paths would misreport where they came from.
        Ok(false) => return Ok(Vec::new()),
        Ok(true) => {}
        // A link the guard refused is the same answer as `Ok(false)` for this
        // walk: the tree behind it is not the one the caller named, so it
        // presents no documents. The guard is asked to recognise its own
        // rejection rather than the kind being matched here — it raises
        // `InvalidInput` for a link and the filesystem raises the same kind for
        // other malformed paths, so matching the kind would quietly answer an
        // undecidable root as an ordinary empty one.
        Err(error) if is_symlink_rejection(&error) => return Ok(Vec::new()),
        // An absent component is a repository that keeps no conventions.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        // Anything else means the guard could not decide — `knowledge` being a
        // regular file, a component that cannot be traversed. That is a
        // structural anomaly and fails closed for the same reason an unlistable
        // root does: not knowing what is there is not the same as nothing.
        Err(error) => {
            return Err(ConventionResolveError::ConventionRootUnlistable {
                root: PathBuf::from(CONVENTION_ROOT),
                detail: CapabilityFailureDetail::new(error.to_string()),
            });
        }
    }
    // An absent root is a repository that keeps no conventions, which `AC-08`
    // makes an ordinary empty result. Every other listing failure — a root that
    // is a regular file, or a directory the process may not read — is a
    // structural anomaly and fails closed: "no document declares this
    // capability" and "the documents could not be looked at" mean opposite
    // things to a caller, and returning the first for the second would report a
    // repository whose convention tree is unreadable as one requiring nothing.
    let entries = match bounded_entries(&convention_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(ConventionResolveError::ConventionRootUnlistable {
                root: PathBuf::from(CONVENTION_ROOT),
                detail: CapabilityFailureDetail::new(error.to_string()),
            });
        }
    };

    let mut requirements = Vec::new();
    scan_entries(entries, Path::new(CONVENTION_ROOT), &mut requirements, MAX_DIRECTORY_DEPTH)?;
    Ok(requirements)
}

/// Scans the listing `entries` of the directory at `relative_dir`, descending
/// into real subdirectories and skipping symlinks, and appends one requirement
/// per document.
///
/// `relative_dir` is a raw repository-relative path and deliberately not a
/// [`ConventionDocumentPath`]: a directory is not a document, and requiring it
/// to satisfy the document path rule would fail a scan over a subtree that
/// holds nothing the rule could be about. It is validated as part of a
/// document's complete path, at the document.
///
/// `remaining_depth` is how much further this walk may still descend; it is
/// decremented per level and a subdirectory met at zero is reported unreadable
/// rather than entered.
fn scan_entries(
    entries: Vec<std::fs::DirEntry>,
    relative_dir: &Path,
    requirements: &mut Vec<ConventionRequirement>,
    remaining_depth: usize,
) -> Result<(), ConventionResolveError> {
    for entry in entries {
        // Held as a plain path until the entry is known to be one the walk
        // presents. Validating here instead would let an entry this walk
        // ignores anyway fail the whole scan on its name: a neighbour called
        // `notes\n.txt` is an ordinary directory entry, and the constructor
        // rejects it for a rendering rule that only matters to documents the
        // scan actually returns.
        let candidate = relative_dir.join(entry.file_name());

        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            // An entry that cannot be classified cannot be shown to be one the
            // walk ignores, so it is reported rather than skipped — which means
            // naming it, and so validating its path after all. A name that then
            // fails surfaces as the rejection, which is the more specific of
            // the two failures: an entry that cannot be named is one this
            // report could not have attributed anyway.
            Err(error) => return Err(unreadable(&document_path(candidate)?, &error)),
        };

        // Reported by the listing itself, so this is the link and never what it
        // points at — the one place the walk can tell the two apart, since
        // every read below this point resolves the link silently.
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            // Not validated on the way in. A directory is not a document and is
            // never returned, so its name is only ever a component of some
            // document's path — and a subtree holding no `*.md` candidate has
            // no such path for the rule to be about. Carrying the raw path down
            // and validating the whole of it at the document is what makes an
            // unrepresentable directory fail closed exactly when it has a
            // document under it, and be ignored when it has none.
            //
            // The two failures below are the exception, because reporting one
            // means naming the directory, which validates it after all.
            let Some(nested_depth) = remaining_depth.checked_sub(1) else {
                return Err(unreadable(
                    &document_path(candidate)?,
                    format!("convention tree is nested deeper than {MAX_DIRECTORY_DEPTH} levels"),
                ));
            };
            let nested = match bounded_entries(&entry.path()) {
                Ok(nested) => nested,
                Err(error) => return Err(unreadable(&document_path(candidate)?, &error)),
            };
            scan_entries(nested, &candidate, requirements, nested_depth)?;
            continue;
        }
        // Anything left that is not a regular file is not a document either,
        // and is dropped here rather than at the read: a FIFO named like a
        // document would otherwise block this walk until something wrote to
        // it. This decides which nodes the walk offers; that the decision
        // still holds when the file is opened is `read_document`'s business,
        // since the node can be replaced in between.
        if !file_type.is_file() {
            continue;
        }
        // Read from the unvalidated candidate, which carries the same extension
        // the validated path would: `relative_dir` is already normalised and an
        // entry name is a single component, so the constructor's normalisation
        // has nothing left to drop.
        if candidate.extension() != Some(OsStr::new(DOCUMENT_EXTENSION)) {
            continue;
        }

        // Every candidate the walk does present goes through the constructor,
        // including the ones it built from the root downwards and could argue
        // are safe: arguing that would be the walk deciding the path rule a
        // second time.
        let document = document_path(candidate)?;
        let content = read_document(&entry.path(), &document)?;
        requirements
            .push(parse_convention_front_matter(&document, &content)?.into_requirement(document)?);
    }
    Ok(())
}

/// Lists `directory`, holding at most [`MAX_DIRECTORY_ENTRIES`] entries.
///
/// The entries are kept in whatever order the listing yielded them. This walk
/// deliberately does not order them: ordering convention documents belongs to
/// [`usecase::conventions_resolve::ConventionResolution`], whose constructor
/// sorts and deduplicates so that no consumer has to, and sorting here as well
/// would put a second site in charge of a rule that type already owns.
fn bounded_entries(directory: &Path) -> std::io::Result<Vec<std::fs::DirEntry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        if entries.len() >= MAX_DIRECTORY_ENTRIES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("directory holds more than {MAX_DIRECTORY_ENTRIES} entries"),
            ));
        }
        entries.push(entry?);
    }
    Ok(entries)
}

/// Reads the document at `path`, holding at most [`MAX_DOCUMENT_BYTES`] of it.
///
/// `path` is the location on disk and `document` the validated repository-relative
/// identity the failure is reported under; the two are the same file, named the
/// two ways this walk has to name it.
fn read_document(
    path: &Path,
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
    let file = rustix::fs::open(
        path,
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
/// reaching here, so only a document the walk means to present, or a directory
/// it means to descend into, can fail the scan on its name.
///
/// The lift would be kept even without that producer, since removing it would
/// mean the walk deciding the path rule itself, which is exactly what having
/// one enforcing site exists to prevent.
fn document_path(candidate: PathBuf) -> Result<ConventionDocumentPath, ConventionResolveError> {
    ConventionDocumentPath::try_new(candidate)
        .map_err(|source| ConventionResolveError::DocumentPathRejected { source })
}

/// Builds the unreadable-document failure for `document`.
///
/// Takes the detail as anything printable rather than as an [`std::io::Error`],
/// because not every way a document is unreadable comes from the filesystem:
/// exceeding [`MAX_DOCUMENT_BYTES`] is this walk's own judgement about a read it
/// will not finish.
fn unreadable(
    document: &ConventionDocumentPath,
    detail: impl std::fmt::Display,
) -> ConventionResolveError {
    ConventionResolveError::DocumentUnreadable {
        document: document.clone(),
        detail: CapabilityFailureDetail::new(detail.to_string()),
    }
}

/// Filesystem implementation of [`ConventionRequirementPort`] (`IN-05`,
/// `AC-06`, `AC-07`).
///
/// Stateless: the tree to scan arrives as an argument, so the adapter reads no
/// ambient location and two calls through one shared receiver observe the two
/// roots they were given. It exists as a unit struct rather than as the free
/// function alone because the port is injected as a trait object, which a free
/// function cannot satisfy.
pub struct FsConventionRequirementAdapter;

impl FsConventionRequirementAdapter {
    /// Creates the adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsConventionRequirementAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ConventionRequirementPort for FsConventionRequirementAdapter {
    /// Delegates to [`scan_convention_requirements`] unchanged, so the port's
    /// promise and the scan's behaviour cannot drift apart.
    fn scan_requirements(
        &self,
        project_root: &Path,
    ) -> Result<Vec<ConventionRequirement>, ConventionResolveError> {
        scan_convention_requirements(project_root)
    }
}
