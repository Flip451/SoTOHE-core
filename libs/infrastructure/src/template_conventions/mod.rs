//! Filesystem side of the convention-shipping check (spec `IN-11`, `AC-18`).
//!
//! What lives here is the tree inventory the check compares two of: a read-only
//! walk that hands back one identity per convention document a tree ships.
//!
//! Two of the check's three structural fail-closed conditions are decided here
//! and nowhere else. [`ConventionShippingCheckError::ConventionRootMissing`] is
//! this walk's answer for a tree that holds no convention root at all, which is
//! a failure rather than an empty inventory precisely because a vacuously
//! passing export is the mutation the check exists to catch; and
//! [`ConventionShippingCheckError::TreeUnreadable`] is its answer for every path
//! it could not read or list. The third,
//! [`ConventionShippingCheckError::DocumentPathRejected`], it only lifts:
//! [`ConventionDocumentPath::try_new`] decides the path rule, and what this walk
//! adds is the `tree_root` that constructor is never told, since the same
//! document path can be rejected while inventorying either tree.
//!
//! It reads no file contents and parses no front matter. That is what keeps a
//! malformed document from turning a shipping question into a parse failure, and
//! it is why this is a walk of its own rather than a reuse of
//! [`scan_convention_requirements`](crate::conventions_resolve::scan_convention_requirements),
//! whose whole purpose is the `required_for` decode. What the two do share is
//! how a directory is reached — the `directory_walk` primitives — so neither arrives at its
//! own answer about links or about how much of a tree to enumerate.

#[cfg(test)]
mod inventory_tests;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use domain::tddd::catalogue_linter::FreeText;
use usecase::conventions_resolve::ConventionDocumentPath;
use usecase::template_conventions::ConventionShippingCheckError;

use crate::conventions_resolve::directory_walk::{
    DirectoryEntry, ListingError, MAX_SCAN_ENTRIES, bounded_entries, open_directory_at,
    open_trusted_root,
};

/// Tree-relative directory this inventory walks.
///
/// Spelled here as well as in `usecase` because a walk has to open a directory,
/// and a directory to open is not something [`ConventionDocumentPath`] hands
/// back. This is not a second enforcing site of the path rule: every path this
/// walk produces is still built through [`ConventionDocumentPath::try_new`], so
/// a value here that drifted away from the root that constructor enforces would
/// make the walk reject its own documents rather than quietly inventory a tree
/// from outside the convention root.
const CONVENTION_ROOT: &str = "knowledge/conventions";

/// Extension of the documents a convention tree ships.
const DOCUMENT_EXTENSION: &str = "md";

/// Deepest nesting this walk will descend to below the convention root.
///
/// The walk recurses, so depth is stack rather than policy: a tree nested deeply
/// enough would overflow before the entry ceiling noticed, and an overflow is
/// not a failure a caller can catch. The bound is far past any nesting a
/// convention tree would have, and it is reported as the containing directory
/// being unreadable — which, at that depth, is what it is to this walk.
const MAX_DIRECTORY_DEPTH: usize = 64;

/// Inventories the convention documents the tree at `tree_root` ships (`IN-11`,
/// `AC-18`).
///
/// Documents are identified tree-relative, so an inventory of the exported tree
/// and one of the overlay tree yield directly comparable identities rather than
/// paths a caller must normalise before comparing. That is the whole of what
/// makes two inventories comparable at all: an absolute path would carry each
/// tree's root into the identity, and every document would then differ from its
/// counterpart.
///
/// The documents arrive in whatever order the directory listings produced, and
/// this walk imposes none of its own. Deduplication and ordering belong to
/// [`select_unsupplied_conventions`](usecase::template_conventions::select_unsupplied_conventions),
/// which commits to them so a failing run names the same documents in the same
/// order every time; establishing an order here as well would put a second site
/// in charge of a rule that function already owns.
///
/// Nothing here opens a document. A tree is enumerated and its entries are
/// classified, so a document whose front matter is malformed is inventoried like
/// any other — the shipping question is about which documents a tree ships, and
/// a document defect must not be able to enter the answer as a failure to ship.
///
/// Symlinks are refused rather than passed over, whether they name a directory
/// or a document, for the reason the sibling scan refuses them: following one
/// would let the walk leave the tree while every path it produced still read as
/// tree-relative, and passing over one would report a document the walk declined
/// to look at as one the tree does not ship. Under this check the second is the
/// more dangerous of the two, because "not shipped" is the passing answer.
///
/// A node that is neither a regular file nor a directory — a FIFO, a socket, a
/// device — is passed over, and by node type before any open, since opening a
/// FIFO named like a document would block this walk for as long as no one wrote
/// to it. Nothing is declined by that: there is no document there to ship.
///
/// The walk also stops if it examines more than `MAX_SCAN_ENTRIES` entries in
/// total. Counting entries rather than bytes is what bounds this walk's cost,
/// since it performs no reads to bound: the per-directory bound renews at every
/// node, so a tree pointed at something that is not a convention tree at all
/// would otherwise be enumerated without limit.
///
/// # Errors
///
/// Returns [`ConventionShippingCheckError::ConventionRootMissing`] when a
/// component of the convention root is absent below a tree root that opened —
/// and only then, so a root that exists but could not be reached is never
/// reported as one that is not there.
///
/// Returns [`ConventionShippingCheckError::TreeUnreadable`] when `tree_root`
/// itself could not be opened, when a component of the convention root below it
/// could not be opened for any reason other than being absent — a link among
/// them — when a directory could not be listed, when the walk meets a symlink or
/// nesting past `MAX_DIRECTORY_DEPTH`, and when the tree holds more entries
/// than the walk will examine.
///
/// Returns [`ConventionShippingCheckError::DocumentPathRejected`] when a
/// document the walk found is named in a way [`ConventionDocumentPath::try_new`]
/// refuses.
pub fn list_convention_documents(
    tree_root: &Path,
) -> Result<Vec<ConventionDocumentPath>, ConventionShippingCheckError> {
    let mut inventory = Inventory::new(tree_root);
    let root_dir = inventory.open_convention_root()?;
    // The root's own listing is charged to the ceiling like any other, so the
    // count covers everything the walk enumerates rather than everything below
    // the first level.
    let entries = inventory.list(&root_dir, Path::new(CONVENTION_ROOT))?;
    inventory.walk(&root_dir, entries, Path::new(CONVENTION_ROOT), MAX_DIRECTORY_DEPTH)?;
    Ok(inventory.documents)
}

/// The state one inventory carries across the levels of a tree.
///
/// `tree_root` is held rather than passed down because both failures this walk
/// owns have to name it: an unreadable path is reported under the tree it was
/// reached in, and a rejected document path carries the root that says which of
/// the two trees rejected it.
struct Inventory<'tree> {
    tree_root: &'tree Path,
    documents: Vec<ConventionDocumentPath>,
    remaining_entries: usize,
}

impl<'tree> Inventory<'tree> {
    fn new(tree_root: &'tree Path) -> Self {
        Self { tree_root, documents: Vec::new(), remaining_entries: MAX_SCAN_ENTRIES }
    }

    /// Opens [`CONVENTION_ROOT`] below the tree root, refusing to follow a link
    /// at any component of it.
    ///
    /// The root is reached one component at a time, each open relative to the
    /// handle above it and `NOFOLLOW`, rather than by resolving the whole path
    /// in one call. A pathname open guards only its trailing component, so
    /// `knowledge` could be replaced by a link between one resolution and the
    /// next and the open would follow it into another tree. Holding each handle
    /// closes that window, because there is no second resolution left to race.
    ///
    /// Absence is separated from every other failure here, and that separation
    /// is the point: `NotFound` while descending is a tree that ships no
    /// conventions, which this check refuses as
    /// [`ConventionShippingCheckError::ConventionRootMissing`] rather than
    /// treating as an inventory of nothing. Everything else — a link, a
    /// permission refusal, a regular file where the directory should be — is a
    /// tree that could not be read, and must not be allowed to arrive as the
    /// absence it can resemble. A dangling link is exactly that case: with
    /// `NOFOLLOW` its open fails on the link itself, never on the missing
    /// target, so it stays unreadable rather than becoming a missing root.
    ///
    /// The tree root above those components is opened on the caller's terms and
    /// its own failure is never absence: a tree root that could not be opened is
    /// a tree nobody looked at, and calling that "ships no conventions" would
    /// pass the shipping check on a path that does not exist.
    fn open_convention_root(&self) -> Result<std::fs::File, ConventionShippingCheckError> {
        let mut directory = open_trusted_root(self.tree_root).map_err(|error| {
            self.unreadable(self.tree_root.to_path_buf(), format!("tree root: {error}"))
        })?;
        let mut reached = PathBuf::new();
        for component in Path::new(CONVENTION_ROOT).components() {
            let name = Path::new(component.as_os_str());
            reached.push(name);
            directory = match open_directory_at(&directory, name) {
                Ok(descended) => descended,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(ConventionShippingCheckError::ConventionRootMissing {
                        tree_root: self.tree_root.to_path_buf(),
                    });
                }
                Err(error) => return Err(self.unreadable_at(&reached, error)),
            };
        }
        Ok(directory)
    }

    /// Lists `directory`, charging every entry to this inventory's ceiling.
    fn list(
        &mut self,
        directory: &std::fs::File,
        relative_dir: &Path,
    ) -> Result<Vec<DirectoryEntry>, ConventionShippingCheckError> {
        match bounded_entries(directory, &mut self.remaining_entries) {
            Ok(entries) => Ok(entries),
            // An exhausted ceiling is reported against the convention root
            // rather than against the directory the count happened to end in:
            // what ran out is the whole walk's, and naming that directory would
            // present a statement about the tree as one about a node.
            Err(ListingError::BudgetExhausted) => Err(self.unreadable_at(
                Path::new(CONVENTION_ROOT),
                format!("tree holds more than {MAX_SCAN_ENTRIES} entries"),
            )),
            Err(ListingError::Io(error)) => Err(self.unreadable_at(relative_dir, error)),
        }
    }

    /// Walks the listing `entries` of the directory at `relative_dir`,
    /// descending into real subdirectories and refusing links, and appends one
    /// identity per document.
    ///
    /// `relative_dir` is a raw tree-relative path and deliberately not a
    /// [`ConventionDocumentPath`]: a directory is not a document, and requiring
    /// it to satisfy the document path rule would fail an inventory over a
    /// subtree that holds nothing the rule could be about. It is validated as
    /// part of a document's complete path, at the document.
    fn walk(
        &mut self,
        parent: &std::fs::File,
        entries: Vec<DirectoryEntry>,
        relative_dir: &Path,
        remaining_depth: usize,
    ) -> Result<(), ConventionShippingCheckError> {
        for entry in entries {
            // Held as a plain path until the entry is known to be one this walk
            // presents. Validating here instead would let an entry the walk
            // ignores anyway fail the whole inventory on its name.
            let candidate = relative_dir.join(&entry.name);

            // Reported by the listing itself, so this is the link and never what
            // it points at. Every link is refused, not only the ones named like
            // a document: what a link points at can only be learned by following
            // it, which is the one thing this walk will not do.
            if entry.is_symlink {
                return Err(self.unreadable_at(
                    &candidate,
                    "entry is a symbolic link, which this walk refuses to follow",
                ));
            }
            if entry.is_dir {
                self.descend(parent, &entry, &candidate, remaining_depth)?;
                continue;
            }
            // Anything left that is not a regular file is not a document either.
            if !entry.is_file {
                continue;
            }
            // Read from the unvalidated candidate, which carries the same
            // extension the validated path would: `relative_dir` is already
            // normalised and an entry name is a single component, so the
            // constructor's normalisation has nothing left to drop.
            if candidate.extension() != Some(OsStr::new(DOCUMENT_EXTENSION)) {
                continue;
            }
            // Every candidate the walk does present goes through the
            // constructor, including the ones it built from the root downwards
            // and could argue are safe: arguing that would be this walk deciding
            // the path rule a second time.
            let document = ConventionDocumentPath::try_new(candidate).map_err(|source| {
                ConventionShippingCheckError::DocumentPathRejected {
                    tree_root: self.tree_root.to_path_buf(),
                    source,
                }
            })?;
            self.documents.push(document);
        }
        Ok(())
    }

    /// Enters the subdirectory `entry` of `parent` and walks it.
    ///
    /// The open is relative to the handle that produced the entry, with
    /// `NOFOLLOW`, so the directory listed is the one that was classified.
    /// Re-resolving the path here would let the entry be replaced by a link
    /// between the classification and the listing, and the listing would then
    /// walk out of the tree while every path this inventory builds still read as
    /// tree-relative. A node that became a link, or stopped being a directory,
    /// in that window is refused here rather than passed over.
    fn descend(
        &mut self,
        parent: &std::fs::File,
        entry: &DirectoryEntry,
        candidate: &Path,
        remaining_depth: usize,
    ) -> Result<(), ConventionShippingCheckError> {
        let Some(nested_depth) = remaining_depth.checked_sub(1) else {
            return Err(self.unreadable_at(
                candidate,
                format!("tree is nested deeper than {MAX_DIRECTORY_DEPTH} levels"),
            ));
        };
        let nested_dir = match open_directory_at(parent, &entry.name) {
            Ok(nested_dir) => nested_dir,
            Err(error) => return Err(self.unreadable_at(candidate, error)),
        };
        let nested = self.list(&nested_dir, candidate)?;
        self.walk(&nested_dir, nested, candidate, nested_depth)
    }

    /// Builds the unreadable failure for the tree-relative `relative` path.
    ///
    /// The path is reported joined onto the tree root rather than tree-relative,
    /// because this check inventories two trees and the same tree-relative path
    /// exists in both: a caller told only `knowledge/conventions` could not tell
    /// which of them it could not read.
    fn unreadable_at(
        &self,
        relative: &Path,
        reason: impl std::fmt::Display,
    ) -> ConventionShippingCheckError {
        self.unreadable(self.tree_root.join(relative), reason)
    }

    /// Builds the unreadable failure for an already-complete `path`.
    ///
    /// Takes the reason as anything printable rather than as an
    /// [`std::io::Error`], because not every way a path is unreadable comes from
    /// the filesystem: exceeding the entry ceiling and refusing a link are both
    /// this walk's own judgements, made before or instead of a failed call.
    fn unreadable(
        &self,
        path: PathBuf,
        reason: impl std::fmt::Display,
    ) -> ConventionShippingCheckError {
        ConventionShippingCheckError::TreeUnreadable {
            path,
            reason: FreeText::new(reason.to_string()),
        }
    }
}
