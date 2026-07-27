//! Handle-relative directory traversal for the convention scan.
//!
//! Kept in a sibling module so the codec, the scan, and the adapter stay inside
//! the parent module's length limit. Every open here is `NOFOLLOW` and, below
//! the root, relative to the handle whose listing produced the name: a name
//! resolved again from the top can have become a link since the listing
//! classified it, and the walk would then leave the convention tree while every
//! path it builds still reads as repository-relative.

use std::path::{Path, PathBuf};

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;

use super::MAX_DIRECTORY_ENTRIES;

/// Lists `directory`, holding at most [`MAX_DIRECTORY_ENTRIES`] entries.
///
/// The entries are kept in whatever order the listing yielded them. This walk
/// deliberately does not order them: ordering convention documents belongs to
/// [`usecase::conventions_resolve::ConventionResolution`], whose constructor
/// sorts and deduplicates so that no consumer has to, and sorting here as well
/// would put a second site in charge of a rule that type already owns.
pub(super) fn bounded_entries(directory: &std::fs::File) -> std::io::Result<Vec<DirectoryEntry>> {
    // Read through a duplicate so the caller's handle keeps its own offset and
    // stays usable for the `*at` opens that follow this listing.
    let listing = rustix::fs::Dir::read_from(directory).map_err(std::io::Error::from)?;

    let mut entries = Vec::new();
    for entry in listing {
        let entry = entry.map_err(std::io::Error::from)?;
        let name = entry.file_name();
        // `.` and `..` are the directory itself and its parent, which this walk
        // reaches on its own terms; following them here would revisit the tree
        // and, for `..`, climb out of it.
        if name == c"." || name == c".." {
            continue;
        }
        if entries.len() >= MAX_DIRECTORY_ENTRIES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("directory holds more than {MAX_DIRECTORY_ENTRIES} entries"),
            ));
        }
        // A listing may decline to classify an entry — `DT_UNKNOWN`, which some
        // filesystems always return — and taking that at face value would set
        // every flag false, so the walk would skip an ordinary document as if it
        // were a socket. The type is then asked of the directory with a
        // no-follow `stat`, still relative to this handle so the answer is about
        // the entry this listing produced.
        let file_type = match entry.file_type() {
            rustix::fs::FileType::Unknown => classify_at(directory, name)?,
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
pub(super) struct DirectoryEntry {
    pub(super) name: PathBuf,
    pub(super) is_dir: bool,
    pub(super) is_file: bool,
    pub(super) is_symlink: bool,
}

/// Opens the convention root itself without following a symlink.
///
/// The one open in this walk taken by pathname rather than relative to a
/// handle, because it is where the walk starts and there is no handle yet. The
/// ancestors above it are checked separately by the symlink guard; from here
/// down every descent goes through [`open_directory_at`].
pub(super) fn open_directory(path: &Path) -> std::io::Result<std::fs::File> {
    rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
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
pub(super) fn open_directory_at(
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

/// Reports whether `error` is the kernel refusing to open a symlink.
///
/// A link substituted between the listing and the open is the same situation as
/// one the listing itself reported: not a document, and skipped. Distinguishing
/// it from a genuine failure keeps a racing consumer from turning an ordinary
/// scan into a failure.
pub(super) fn is_symlink_open_rejection(error: &std::io::Error) -> bool {
    // `ELOOP` is what `O_NOFOLLOW` returns for a symlink; `ENOTDIR` is what an
    // entry that stopped being a directory returns for `O_DIRECTORY`.
    matches!(error.raw_os_error(), Some(libc::ELOOP) | Some(libc::ENOTDIR))
}
