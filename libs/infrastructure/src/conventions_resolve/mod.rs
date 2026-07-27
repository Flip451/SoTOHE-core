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
use std::path::{Path, PathBuf};

use usecase::capability_exec::CapabilityFailureDetail;
use usecase::conventions_resolve::{
    ConventionDocumentPath, ConventionRequirement, ConventionRequirementPort,
    ConventionResolveError,
};

use crate::capability_exec::{YAML_LINE_BREAKS, read_front_matter};
mod directory_walk;
mod document_read;
mod front_matter_dto;

use directory_walk::{
    DirectoryEntry, ListingError, MAX_SCAN_ENTRIES, bounded_entries, open_directory_at,
    open_trusted_root,
};
use document_read::{document_path, read_document_at, unreadable};

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

/// Deepest nesting this walk will descend to below the convention root.
///
/// The walk recurses, so depth is stack rather than policy: a tree nested
/// deeply enough would overflow before any per-directory bound noticed, and an
/// overflow is not a failure a caller can catch. The bound is far past any
/// nesting a convention tree would have, and it is reported as the containing
/// directory being unreadable — which, at that depth, is what it is to this
/// walk.
const MAX_DIRECTORY_DEPTH: usize = 64;

/// Opens [`CONVENTION_ROOT`] below `project_root`, refusing to follow a link at
/// any component of it.
///
/// The root is reached one component at a time, each open relative to the
/// handle above it and `NOFOLLOW`, rather than by resolving the whole path in
/// one call. A pathname open guards only its trailing component, so `knowledge`
/// could be replaced by a link between one resolution and the next and the open
/// would follow it into another tree; a substituted link that dangles comes back
/// `NotFound`, which this walk reads as a repository that keeps no conventions —
/// the one wrong answer it must not give. Holding each handle closes that
/// window, because there is no second resolution left to race.
///
/// This is also why nothing checks these components before the open. A check
/// followed by a fresh resolution is what opens the window in the first place;
/// refusing the link at the moment of the open is the same check with no window
/// to leave.
///
/// The anchor is opened on its own terms and its failures are never absence.
/// `NotFound` reported while descending to `knowledge` or to `conventions` is a
/// repository that keeps no conventions; `NotFound` from the anchor is a
/// repository that was never opened, and the two would otherwise arrive as the
/// same value. `project_root` is taken from the caller — `--project-root` hands
/// it over directly — so a path that is misspelled, or that points at a link to
/// nothing, must not come back as a repository declaring no conventions and let
/// dispatch proceed.
fn open_convention_root(project_root: &Path) -> Result<ConventionRootOpen, ConventionResolveError> {
    let mut directory = open_trusted_root(project_root).map_err(|error| {
        unlistable_root(format!(
            "project root {} could not be opened: {error}",
            project_root.display()
        ))
    })?;
    for component in Path::new(CONVENTION_ROOT).components() {
        let name = Path::new(component.as_os_str());
        directory = match open_directory_at(&directory, name) {
            Ok(descended) => descended,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ConventionRootOpen::Absent);
            }
            Err(error) => {
                return Err(unlistable_root(format!(
                    "{} below the project root could not be opened: {error}",
                    name.display()
                )));
            }
        };
    }
    Ok(ConventionRootOpen::Opened(directory))
}

/// What opening the convention root came to.
///
/// A handle and an absent root are different answers rather than one nullable
/// handle, so that the arm returning the empty result has to be written out and
/// cannot be reached by a failure that merely happens to look like absence.
enum ConventionRootOpen {
    /// The root is open and can be listed.
    Opened(std::fs::File),
    /// The repository keeps no conventions: a component of the convention root
    /// is absent below a project root that opened.
    Absent,
}

/// Builds the failure for a convention root this walk could not read.
///
/// `detail` says which path failed and how, because the variant itself can only
/// name the repository-relative root: the anchor above it and the two
/// components below it all surface here, and a caller told only
/// `knowledge/conventions` could not tell a mistyped project root from an
/// unreadable convention directory.
///
/// The whole-scan entry budget reports through here too, rather than naming the
/// directory the count ran out in. What that failure is about is the tree and
/// not the node the walk happened to reach last, and the root is the only path
/// this walk can name that stands for the tree.
fn unlistable_root(detail: String) -> ConventionResolveError {
    ConventionResolveError::ConventionRootUnlistable {
        root: PathBuf::from(CONVENTION_ROOT),
        detail: CapabilityFailureDetail::new(detail),
    }
}

/// Builds the failure for a tree larger than this walk will enumerate.
///
/// One function rather than the message written at each of the two listings
/// that can raise it, so the two cannot come to say different things about the
/// same bound.
fn budget_exhausted() -> ConventionResolveError {
    unlistable_root(format!("convention tree holds more than {MAX_SCAN_ENTRIES} entries"))
}

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
/// Symlinks are not followed, whether they name a directory or a document, and
/// meeting one fails the scan. Following one would let the walk leave the
/// convention tree while every path it produced still looked repository-
/// relative, which is precisely the escape the path rule exists to refuse: for
/// a directory the walk itself would leave the tree, and for a document the
/// path would stay inside it while the bytes came from wherever the link
/// pointed, so the path rule would be guaranteeing nothing about what was
/// actually read. Not descending a directory link also bounds the recursion by
/// the real tree's depth.
///
/// Refusing rather than passing over is the same distinction this walk draws at
/// the root: a link is a document, or a subtree of them, that the walk declined
/// to read, and a requirement set handed back without it would report
/// conventions as absent when they exist. That refusal is not a sixth `AC-07`
/// condition — it is the first of the five, a document this walk cannot read,
/// decided by the link before any read is attempted.
///
/// That refusal is applied to entries a listing reported, so it cannot see the
/// two
/// components the walk reaches on its way to the root rather than by listing:
/// `knowledge` and `knowledge/conventions` themselves. A link at either would
/// be resolved before any entry existed to inspect, and the walk would present
/// another tree's files under this one's paths — the same escape, arriving one
/// level above where the walk looks for it. The descent this walk starts from
/// refuses both, at the open of each rather than by a check made before them.
/// `project_root` above them is the caller's to vouch for: it is the tree the
/// port was asked about, not something this walk resolved.
///
/// A node that is not a regular file and not a directory — a FIFO, a socket, a
/// device — is passed over rather than refused, and excluded before the read
/// rather than at it, since opening a FIFO named like a document would block
/// this walk for as long as no one wrote to it. That one is a decision about
/// which documents the walk offers and not a fail-closed condition: nothing was
/// declined, because there is nothing there to read. `AC-07` names five
/// conditions and "is not a regular file" is not among them.
///
/// The walk also stops if it examines more than `MAX_SCAN_ENTRIES` entries in
/// total. The per-directory and per-document bounds renew at every node and so
/// bound one listing and one read rather than the traversal; this one is what
/// keeps a tree that is not a convention tree from costing an unbounded number
/// of reads and an unbounded amount of retained requirements.
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
/// Returns [`ConventionResolveError::ConventionRootUnlistable`] when the tree
/// the walk was asked about could not be read at all: `project_root` itself
/// could not be opened, a component of the convention root below it could not
/// be opened for any reason other than being absent — a link among them — or
/// the opened root could not be listed, or the tree held more than
/// `MAX_SCAN_ENTRIES` entries. An absent convention root is not one of these;
/// it is the empty result `AC-08` allows.
///
/// Returns [`ConventionResolveError::DocumentUnreadable`] when a document or a
/// directory below the convention root cannot be read — a link among them, and
/// including when either exceeds the bound this walk reads it under
/// (`MAX_DOCUMENT_BYTES`, `MAX_DIRECTORY_ENTRIES`),
/// [`ConventionResolveError::DocumentPathRejected`]
/// when a document or a directory the walk would descend into is named in a way
/// [`ConventionDocumentPath::try_new`] refuses, and the failure
/// [`parse_convention_front_matter`] or
/// [`ConventionFrontMatterDto::into_requirement`] decided for the document that
/// carries it.
pub fn scan_convention_requirements(
    project_root: &Path,
) -> Result<Vec<ConventionRequirement>, ConventionResolveError> {
    // Only an absent convention root presents no documents, which `AC-08` makes
    // an ordinary empty result. A root that exists but cannot be read — because
    // it is not a directory, cannot be listed, or is reached through a link —
    // fails closed. The root is the only path this walk touches that is not
    // itself a document — `ConventionDocumentPath` rejects it — and so the only
    // path whose failure none of `AC-07`'s five document-shaped conditions can
    // name; `ConventionRootUnlistable` is the variant that names it. Everything
    // below it is fail-closed too.
    //
    // A linked root is not an empty tree: the documents exist, in the tree the
    // link points at, and this walk will not present them because their
    // repository-relative paths would misreport where they came from. A
    // consumer who symlinks their convention directory would otherwise be told
    // they require no conventions, and dispatch would proceed without policies
    // that do exist. Refusing to read a tree is a structural anomaly under
    // `AC-07`; saying it is empty is a wrong answer. The same holds for the
    // ordinary unreadable root — a regular file where the directory should be,
    // or a directory the process may not read: "no document declares this
    // capability" and "the documents could not be looked at" mean opposite
    // things to a caller.
    //
    // Nothing below the root is passed over either. An entry the listing
    // classified as a link, and a node that became one between the listing and
    // the open, both fail the scan: what is behind them was never read, and a
    // walk that continued past them would hand back a requirement set missing
    // whatever they named.
    let root_dir = match open_convention_root(project_root)? {
        ConventionRootOpen::Opened(root_dir) => root_dir,
        ConventionRootOpen::Absent => return Ok(Vec::new()),
    };

    // The root's own listing is charged to the budget like any other, so the
    // count covers everything the walk enumerates rather than everything below
    // the first level.
    let mut remaining_entries = MAX_SCAN_ENTRIES;
    // Every listing failure fails closed, absence included. The root is open by
    // now, so `NotFound` here is no longer evidence that it was never there: a
    // listing can report it for a directory emptied and removed underneath the
    // walk, and that is a tree this scan did not read rather than a tree with
    // nothing in it.
    let entries = match bounded_entries(&root_dir, &mut remaining_entries) {
        Ok(entries) => entries,
        Err(ListingError::BudgetExhausted) => return Err(budget_exhausted()),
        Err(ListingError::Io(error)) => {
            return Err(unlistable_root(format!("{CONVENTION_ROOT} could not be listed: {error}")));
        }
    };

    let mut requirements = Vec::new();
    scan_entries(
        &root_dir,
        entries,
        Path::new(CONVENTION_ROOT),
        &mut requirements,
        MAX_DIRECTORY_DEPTH,
        &mut remaining_entries,
    )?;
    Ok(requirements)
}

/// Scans the listing `entries` of the directory at `relative_dir`, descending
/// into real subdirectories and refusing links, and appends one requirement per
/// document.
///
/// `relative_dir` is a raw repository-relative path and deliberately not a
/// [`ConventionDocumentPath`]: a directory is not a document, and requiring it
/// to satisfy the document path rule would fail a scan over a subtree that
/// holds nothing the rule could be about. It is validated as part of a
/// document's complete path, at the document.
///
/// `remaining_depth` is how much further this walk may still descend; it is
/// decremented per level and a subdirectory met at zero is reported unreadable
/// rather than entered. `remaining_entries` is the whole scan's budget, shared
/// by every level rather than renewed at each, and it is spent by
/// [`bounded_entries`] as entries are produced rather than here as they are
/// walked — the entries in hand have already been paid for.
fn scan_entries(
    parent: &std::fs::File,
    entries: Vec<DirectoryEntry>,
    relative_dir: &Path,
    requirements: &mut Vec<ConventionRequirement>,
    remaining_depth: usize,
    remaining_entries: &mut usize,
) -> Result<(), ConventionResolveError> {
    for entry in entries {
        // Held as a plain path until the entry is known to be one the walk
        // presents. Validating here instead would let an entry this walk
        // ignores anyway fail the whole scan on its name: a neighbour called
        // `notes\n.txt` is an ordinary directory entry, and the constructor
        // rejects it for a rendering rule that only matters to documents the
        // scan actually returns.
        let candidate = relative_dir.join(&entry.name);

        // Reported by the listing itself, so this is the link and never what it
        // points at.
        //
        // A refused link fails the scan rather than being passed over, for the
        // same reason a linked root does: what is behind it was never looked
        // at, and a document the walk declined to read is not a document the
        // tree lacks. Passing over it would hand back a requirement set missing
        // whatever the link named — a convention document kept elsewhere and
        // linked into place, or a whole subtree of them — and a consumer would
        // dispatch under conventions that exist and were not applied.
        //
        // Every link is refused, not only the ones named like a document. What
        // a link points at can only be learned by following it, which is the
        // one thing this walk will not do, so it cannot tell a link to a stray
        // note from a link to a directory full of conventions. Refusing on the
        // evidence available is the same answer the root is given.
        if entry.is_symlink {
            return Err(unreadable(
                &document_path(candidate)?,
                "entry is a symbolic link, which this walk refuses to follow",
            ));
        }
        if entry.is_dir {
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
            // Opened relative to the handle that produced this entry, with
            // `NOFOLLOW`, so the directory that gets listed is the one that was
            // classified. Re-resolving `relative_dir.join(name)` here would let
            // the entry be replaced by a link between the classification and
            // the listing, and the listing would then walk out of the tree
            // while every path this scan builds still reads as repository-
            // relative.
            //
            // A node that became a link, or stopped being a directory, between
            // the listing and this open is refused here rather than passed
            // over. The listing having classified it does not make what is
            // there now readable, and the walk would otherwise report a subtree
            // it never entered as one holding nothing.
            let nested_dir = match open_directory_at(parent, &entry.name) {
                Ok(nested_dir) => nested_dir,
                Err(error) => return Err(unreadable(&document_path(candidate)?, &error)),
            };
            // An exhausted budget is reported against the root rather than
            // against this directory: what ran out is the whole scan's, and
            // naming the directory the count happened to end in would present a
            // statement about the tree as one about a node.
            let nested = match bounded_entries(&nested_dir, remaining_entries) {
                Ok(nested) => nested,
                Err(ListingError::BudgetExhausted) => return Err(budget_exhausted()),
                Err(ListingError::Io(error)) => {
                    return Err(unreadable(&document_path(candidate)?, &error));
                }
            };
            scan_entries(
                &nested_dir,
                nested,
                &candidate,
                requirements,
                nested_depth,
                remaining_entries,
            )?;
            continue;
        }
        // Anything left that is not a regular file is not a document either,
        // and is dropped here rather than at the read: a FIFO named like a
        // document would otherwise block this walk until something wrote to
        // it. This decides which nodes the walk offers; that the decision
        // still holds when the file is opened is `read_document_at`'s business,
        // since the node can be replaced in between.
        if !entry.is_file {
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
        let content = read_document_at(parent, &entry.name, &document)?;
        requirements
            .push(parse_convention_front_matter(&document, &content)?.into_requirement(document)?);
    }
    Ok(())
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
