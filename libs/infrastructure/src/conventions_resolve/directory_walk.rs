//! Handle-relative directory traversal for the walks over a convention tree.
//!
//! Kept in a sibling module so the codec, the scan, and the adapter stay inside
//! the parent module's length limit, and visible to the crate because the
//! shipping inventory in [`crate::template_conventions`] walks the same kind of
//! tree under the same refusals. The two walks differ in what they do with a
//! document — one decodes its front matter, the other never opens it — and not
//! in how a directory is reached, so sharing these primitives is what keeps a
//! second traversal from arriving at its own answer about links, about entries
//! a listing declined to classify, and about how much of a tree to enumerate.
//! Only the anchor the walk starts from is
//! opened by pathname; every open below it is `NOFOLLOW` and relative to the
//! handle above it — the one whose listing produced the name, once the walk is
//! inside the tree. A name resolved again from the top can have become a link
//! since it was last looked at, and the walk would then leave the convention
//! tree while every path it builds still reads as repository-relative.

use std::path::{Path, PathBuf};

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;

/// Most entries this walk will hold from one directory listing.
///
/// The listing is materialised before it is walked so that descending into a
/// subdirectory does not require keeping the parent's directory handle open:
/// streaming it instead would hold one handle per level of nesting. Holding the
/// listing makes its size this walk's problem rather than the filesystem's, and
/// this is where that is answered.
///
/// This is not the same question [`MAX_SCAN_ENTRIES`] answers, so having both
/// is not having two limits on one quantity. This one asks how large a single
/// directory the walk lists may be, and its answer names that directory — a
/// thing a person can go and look at. The whole-scan budget asks whether the
/// tree as a whole is a convention tree at all, and its answer can only name
/// the root. A directory of fifteen thousand entries would otherwise be
/// reported as an oversized repository rather than as the one oversized
/// directory it is.
pub(crate) const MAX_DIRECTORY_ENTRIES: usize = 10_000;

/// Most directory entries this walk will examine, across the whole scan.
///
/// The per-directory and per-document bounds are each spent and renewed at
/// every node, so together they bound one listing and one read and nothing
/// about the traversal as a whole: a tree of ten thousand directories each
/// holding ten thousand documents stays inside both while costing a hundred
/// million reads and retaining every requirement decoded along the way. This
/// bound is the one that does not renew.
///
/// Counted in entries a listing produced rather than in documents kept or bytes
/// read, because that is the unit both other costs are downstream of: every
/// document this walk retains was first an entry it counted, and every read it
/// performs was too. A document count would leave a tree of empty directories
/// unbounded, and a byte count is already what `MAX_DOCUMENT_BYTES` spends per
/// document.
///
/// The value is far past any convention tree a person would author — this
/// repository's own holds a few dozen — and the bound exists for the tree that
/// is not one: a `--project-root` pointed at something that is not a convention
/// repository at all. That case is an accident far more often than an attack,
/// and either way the answer is the same, which is why the walk stops at a
/// stated number rather than at exhaustion.
pub(crate) const MAX_SCAN_ENTRIES: usize = 20_000;

/// Why a listing produced no entries.
///
/// The two are kept apart because the caller answers them differently: an
/// exhausted budget is a statement about the tree and is reported against the
/// root, while a failed listing is about the directory being listed and is
/// reported against that directory. Folding both into one [`std::io::Error`]
/// would leave the caller matching on a message to tell them apart.
pub(crate) enum ListingError {
    /// The whole-scan entry budget ran out while this listing was produced.
    BudgetExhausted,
    /// The listing failed, or the directory holds more entries than
    /// [`MAX_DIRECTORY_ENTRIES`] allows one listing to hold.
    Io(std::io::Error),
}

/// Lists `directory`, holding at most [`MAX_DIRECTORY_ENTRIES`] entries and
/// charging every entry it produces to `remaining_entries`.
///
/// The entries are kept in whatever order the listing yielded them. This walk
/// deliberately does not order them: ordering convention documents belongs to
/// [`usecase::conventions_resolve::ConventionResolution`], whose constructor
/// sorts and deduplicates so that no consumer has to, and sorting here as well
/// would put a second site in charge of a rule that type already owns.
///
/// The whole-scan budget is charged here, where entries come into existence,
/// rather than by the caller that walks them. Charging it per directory the
/// walk descends into would bound the descents and not the enumeration: a
/// listing is read, classified, and held in full before the caller looks at any
/// of it, so a chain of large directories would materialise every one of them
/// while costing a single unit each. What the budget is for is the total the
/// walk enumerates, so the total is what it counts.
pub(crate) fn bounded_entries(
    directory: &std::fs::File,
    remaining_entries: &mut usize,
) -> Result<Vec<DirectoryEntry>, ListingError> {
    // Read through a duplicate so the caller's handle keeps its own offset and
    // stays usable for the `*at` opens that follow this listing.
    let listing =
        rustix::fs::Dir::read_from(directory).map_err(|error| ListingError::Io(error.into()))?;

    let mut entries = Vec::new();
    for entry in listing {
        let entry = entry.map_err(|error| ListingError::Io(error.into()))?;
        let name = entry.file_name();
        // `.` and `..` are the directory itself and its parent, which this walk
        // reaches on its own terms; following them here would revisit the tree
        // and, for `..`, climb out of it. They are not charged either, for the
        // same reason: every directory has both, so charging for them would
        // make the budget partly a count of directories rather than of what is
        // in them.
        if name == c"." || name == c".." {
            continue;
        }
        // Charged before the entry is classified or kept, so an entry costs the
        // same whatever it turns out to be. Classifying first would let the one
        // case this budget exists for — an enormous tree of nodes this walk
        // ignores — run a `stat` for every one of them free.
        let Some(left) = remaining_entries.checked_sub(1) else {
            return Err(ListingError::BudgetExhausted);
        };
        *remaining_entries = left;
        if entries.len() >= MAX_DIRECTORY_ENTRIES {
            return Err(ListingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("directory holds more than {MAX_DIRECTORY_ENTRIES} entries"),
            )));
        }
        // A listing may decline to classify an entry — `DT_UNKNOWN`, which some
        // filesystems always return — and taking that at face value would set
        // every flag false, so the walk would skip an ordinary document as if it
        // were a socket. The type is then asked of the directory with a
        // no-follow `stat`, still relative to this handle so the answer is about
        // the entry this listing produced.
        let file_type = match entry.file_type() {
            rustix::fs::FileType::Unknown => {
                classify_at(directory, name).map_err(ListingError::Io)?
            }
            known => known,
        };
        entries.push(DirectoryEntry {
            name: OsString::from_vec(name.to_bytes().to_vec()).into(),
            is_dir: file_type == rustix::fs::FileType::Directory,
            is_file: file_type == rustix::fs::FileType::RegularFile,
            is_symlink: file_type == rustix::fs::FileType::Symlink,
        });
    }
    Ok(entries)
}

/// Asks the filesystem for the type of `name` under `directory`.
///
/// Used only when the listing declined to classify the entry. `NOFOLLOW` so the
/// answer describes the entry itself and not what a link points at, matching
/// what a listing that had classified it would have reported, and relative to
/// the handle so it is the same entry the listing produced rather than whatever
/// the name resolves to from the top by now.
fn classify_at(
    directory: &std::fs::File,
    name: &std::ffi::CStr,
) -> std::io::Result<rustix::fs::FileType> {
    let stat = rustix::fs::statat(directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
        .map_err(std::io::Error::from)?;
    Ok(rustix::fs::FileType::from_raw_mode(stat.st_mode))
}

/// One entry of a directory listing, classified as the listing reported it.
///
/// The classification is kept beside the name rather than re-asked of the path,
/// because asking again would ask about whatever the name refers to by then.
pub(crate) struct DirectoryEntry {
    pub(crate) name: PathBuf,
    pub(crate) is_dir: bool,
    pub(crate) is_file: bool,
    pub(crate) is_symlink: bool,
}

/// Opens the root the walk descends from.
///
/// The one open in this walk taken by pathname rather than relative to a
/// handle, because it is where the walk starts and there is no handle yet.
///
/// Deliberately without `NOFOLLOW`: this is the root the caller vouched for,
/// and a repository checked out under a symlinked path is still that
/// repository. What the walk owes a guarantee about is the components *below*
/// this anchor, and each of those is opened through [`open_directory_at`],
/// relative to the handle above it, so none of them can be swapped for a link
/// between one open and the next.
pub(crate) fn open_trusted_root(path: &Path) -> std::io::Result<std::fs::File> {
    rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(std::fs::File::from)
    .map_err(std::io::Error::from)
}

/// Opens the subdirectory `name` of `parent` without following a symlink.
///
/// Relative to the handle rather than by pathname: a name resolved again from
/// the top can have become a link since the listing classified it, and the
/// listing that followed would then walk a tree outside the convention root
/// while every path this scan builds still reads as repository-relative.
pub(crate) fn open_directory_at(
    parent: &std::fs::File,
    name: &Path,
) -> std::io::Result<std::fs::File> {
    rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(std::fs::File::from)
    .map_err(std::io::Error::from)
}
