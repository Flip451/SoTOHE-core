//! Guarded reads of a single track artifact under `<items_dir>/<track-id>/`.
//!
//! Crate-private helper shared by the batch-plan adapters. The invariant it
//! keeps is containment: the artifact must resolve inside the items directory
//! it was asked for, and no symlink below that directory is followed. Where the
//! items directory itself sits is the caller's choice — `--items-dir` is a
//! supported surface of the CLI — so a directory outside the repository the
//! process stands in is read, not refused.
//!
//! "The artifact is not there" stays distinct from "the read failed", which is
//! the distinction the driven ports declare.
//!
//! A refusal says what was wrong and never where the reader looked. Every path
//! this module handles is absolute by the time it fails — the items directory
//! is canonicalised before the artifact is resolved — and the adapters above it
//! render their errors to an operator, so a formatted path here would be a host
//! filesystem detail disclosed at the gate. What each caller needs is already
//! in its own hands: the well-known file name it passed and the track it asked
//! for.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use domain::TrackId;

use crate::sanitized_failure::io_classification;
use crate::track::symlink_guard::{
    is_symlink_rejection, reject_symlinks_below, reject_symlinks_up_to_root,
};

/// Why a track artifact could not be handed back.
///
/// `NotFound` carries nothing: the ports that consume it declare an absent
/// artifact as a state of its own, with no diagnostic to render.
///
/// `Failed` carries a classification and nothing else. Which artifact was being
/// read is not in it: every caller passes the well-known file name in and holds
/// the track id, so it already owns the identity half of the message and only
/// the cause has to travel. Nothing path-bearing can then reach an operator's
/// screen through this type — the absolute location of the items directory is
/// the caller's own configuration, not a diagnostic.
#[derive(Debug)]
pub(crate) enum TrackArtifactReadError {
    /// The artifact is not present for this track.
    NotFound,
    /// The artifact could not be read, named by what was wrong with it.
    Failed(&'static str),
}

/// The artifact resolved onto something outside the items directory.
const ESCAPES_ITEMS_DIRECTORY: &str = "resolves outside the items directory";
/// A directory, a device, a socket or a fifo stands where the artifact belongs.
const NOT_A_REGULAR_FILE: &str = "not a regular file";
/// The artifact is past the size this reader accepts.
const LARGER_THAN_THE_BOUND: &str = "larger than the maximum size allowed";
/// The bytes read are not text.
const NOT_VALID_UTF8: &str = "not valid UTF-8";
/// The items directory was given as an empty path.
const ITEMS_DIRECTORY_EMPTY: &str = "the items directory must not be empty";
/// The items directory was given with a `..` component.
const ITEMS_DIRECTORY_PARENT_COMPONENT: &str = "the items directory must not contain '..'";
/// Some component of the items directory is a symlink.
const ITEMS_DIRECTORY_SYMLINK: &str = "the items directory was rejected as a symlink";
/// The items directory could not be resolved, or is not a directory.
const ITEMS_DIRECTORY_UNRESOLVABLE: &str = "the items directory could not be resolved";

/// Reads `<items_dir>/<track_id>/<file_name>` as UTF-8 text.
///
/// Refuses a symlinked path anywhere below the items directory, an artifact
/// that resolves outside it, and a file larger than `max_bytes`.
pub(crate) fn read_track_artifact(
    items_dir: &Path,
    track_id: &TrackId,
    file_name: &str,
    max_bytes: u64,
) -> Result<String, TrackArtifactReadError> {
    let items_dir = resolve_items_dir(items_dir)?;

    let path = items_dir.join(track_id.as_ref()).join(file_name);

    match reject_symlinks_below(&path, &items_dir) {
        Ok(true) => {}
        Ok(false) => return Err(TrackArtifactReadError::NotFound),
        Err(error) => {
            return Err(TrackArtifactReadError::Failed(io_classification(&error)));
        }
    }

    match path.canonicalize() {
        Ok(resolved) if resolved.starts_with(&items_dir) => {}
        Ok(_) => {
            // Where it landed is not said: naming the escape target would hand
            // out a location outside the directory the operator configured.
            return Err(TrackArtifactReadError::Failed(ESCAPES_ITEMS_DIRECTORY));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(TrackArtifactReadError::NotFound);
        }
        Err(error) => {
            return Err(TrackArtifactReadError::Failed(io_classification(&error)));
        }
    }

    let path_metadata = std::fs::symlink_metadata(&path)
        .map_err(|error| TrackArtifactReadError::Failed(io_classification(&error)))?;
    if !path_metadata.file_type().is_file() {
        return Err(TrackArtifactReadError::Failed(NOT_A_REGULAR_FILE));
    }

    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(TrackArtifactReadError::NotFound);
        }
        Err(error) => {
            return Err(TrackArtifactReadError::Failed(io_classification(&error)));
        }
    };
    let metadata = file
        .metadata()
        .map_err(|error| TrackArtifactReadError::Failed(io_classification(&error)))?;
    if !metadata.is_file() {
        return Err(TrackArtifactReadError::Failed(NOT_A_REGULAR_FILE));
    }
    if metadata.len() > max_bytes {
        return Err(TrackArtifactReadError::Failed(LARGER_THAN_THE_BOUND));
    }

    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| TrackArtifactReadError::Failed(io_classification(&error)))?;
    if bytes.len() as u64 > max_bytes {
        return Err(TrackArtifactReadError::Failed(LARGER_THAN_THE_BOUND));
    }

    String::from_utf8(bytes).map_err(|_| TrackArtifactReadError::Failed(NOT_VALID_UTF8))
}

/// Resolves the items directory the artifact must stay inside.
///
/// A relative path is taken as given, a `..` component is refused before any
/// filesystem access, and every component of the supplied path — the items
/// directory and each of its ancestors — is refused if it is a symlink, because
/// canonicalising first would follow any of them silently. The result is
/// canonical so containment can be decided on real paths rather than on the
/// spelling of the argument.
fn resolve_items_dir(items_dir: &Path) -> Result<PathBuf, TrackArtifactReadError> {
    if items_dir.as_os_str().is_empty() {
        return Err(TrackArtifactReadError::Failed(ITEMS_DIRECTORY_EMPTY));
    }
    if items_dir.components().any(|component| matches!(component, Component::ParentDir)) {
        return Err(TrackArtifactReadError::Failed(ITEMS_DIRECTORY_PARENT_COMPONENT));
    }
    reject_symlinks_up_to_root(items_dir).map_err(|error| {
        // The guard renders the absolute component it rejected, so the refusal
        // is stated rather than carried; a fault while checking is a different
        // answer from a refusal, and the two stay apart.
        TrackArtifactReadError::Failed(if is_symlink_rejection(&error) {
            ITEMS_DIRECTORY_SYMLINK
        } else {
            ITEMS_DIRECTORY_UNRESOLVABLE
        })
    })?;

    let canonical = match items_dir.canonicalize() {
        Ok(canonical) => canonical,
        // Nothing exists at that location, so no artifact of this track does
        // either: the same answer as an empty track directory.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(TrackArtifactReadError::NotFound);
        }
        Err(_) => {
            return Err(TrackArtifactReadError::Failed(ITEMS_DIRECTORY_UNRESOLVABLE));
        }
    };
    if !canonical.is_dir() {
        return Err(TrackArtifactReadError::Failed(ITEMS_DIRECTORY_UNRESOLVABLE));
    }
    Ok(canonical)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{TrackArtifactReadError, read_track_artifact};
    use domain::TrackId;
    use std::path::Path;

    fn items_dir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("track-artifact-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap()
    }

    /// The classification of a refusal, or a panic if the read was not refused.
    fn failure_word(error: &TrackArtifactReadError) -> &'static str {
        match error {
            TrackArtifactReadError::Failed(word) => word,
            TrackArtifactReadError::NotFound => panic!("expected a read failure, got NotFound"),
        }
    }

    /// Asserts a classification says what was wrong without saying where.
    ///
    /// Both the directory as supplied and its canonical form are checked: the
    /// reader resolves the path before it reads, so either spelling could reach
    /// a message, and neither is the operator's business to be told back.
    fn assert_names_no_path(word: &str, supplied_items_dir: &Path) {
        for spelling in [
            supplied_items_dir.to_path_buf(),
            supplied_items_dir.canonicalize().unwrap_or_else(|_| supplied_items_dir.to_path_buf()),
        ] {
            let rendered = spelling.display().to_string();
            assert!(
                !word.contains(&rendered),
                "a classification names no path: {word} contains {rendered}"
            );
        }
        assert!(!word.contains('/'), "a classification names no path: {word}");
    }

    #[test]
    fn test_read_track_artifact_reads_an_items_dir_outside_the_current_repository() {
        // `--items-dir` is a supported surface, so where the directory sits is
        // the caller's choice: what the guard keeps is containment inside it,
        // not membership of the repository this process stands in.
        let dir = tempfile::Builder::new()
            .prefix("track-artifact-outside-")
            .tempdir_in(std::env::temp_dir())
            .unwrap();
        let track_dir = dir.path().join("some-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("artifact.json"), "{\"read\": true}").unwrap();

        let contents = read_track_artifact(
            dir.path(),
            &TrackId::try_new("some-track").unwrap(),
            "artifact.json",
            1024,
        )
        .unwrap();

        assert_eq!(contents, "{\"read\": true}");
    }

    #[cfg(unix)]
    #[test]
    fn test_read_track_artifact_refuses_a_symlinked_ancestor_of_the_items_directory() {
        // The link sits above the items directory rather than at it, and
        // canonicalising the supplied path would follow it just the same, so
        // every component is checked before the path is resolved.
        let base = items_dir();
        let real_items = base.path().join("repo/track/items");
        std::fs::create_dir_all(real_items.join("some-track")).unwrap();
        std::fs::write(real_items.join("some-track/artifact.json"), "{}").unwrap();
        let linked_root = base.path().join("repo-link");
        std::os::unix::fs::symlink(base.path().join("repo"), &linked_root).unwrap();

        let supplied = linked_root.join("track/items");
        let error = read_track_artifact(
            &supplied,
            &TrackId::try_new("some-track").unwrap(),
            "artifact.json",
            1024,
        )
        .unwrap_err();

        let word = failure_word(&error);
        assert_eq!(word, "the items directory was rejected as a symlink");
        assert_names_no_path(word, &supplied);
    }

    #[cfg(unix)]
    #[test]
    fn test_read_track_artifact_refuses_a_symlinked_items_directory() {
        // The symlink is refused on the path as supplied: canonicalising first
        // would follow it and read from wherever it points.
        let real = items_dir();
        let track_dir = real.path().join("some-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("artifact.json"), "{}").unwrap();

        let parent = items_dir();
        let linked = parent.path().join("items-link");
        std::os::unix::fs::symlink(real.path(), &linked).unwrap();

        let error = read_track_artifact(
            &linked,
            &TrackId::try_new("some-track").unwrap(),
            "artifact.json",
            1024,
        )
        .unwrap_err();

        let word = failure_word(&error);
        assert_eq!(word, "the items directory was rejected as a symlink");
        // The guard's own error renders the linked component, and the directory
        // it points at is a location the operator never supplied.
        assert_names_no_path(word, &linked);
        assert_names_no_path(word, real.path());
    }

    #[cfg(unix)]
    #[test]
    fn test_read_track_artifact_refuses_an_artifact_that_leaves_the_items_directory() {
        // The artifact name resolves onto a file outside the items directory:
        // containment is what the guard decides on, so the read is refused
        // rather than following the link.
        let dir = items_dir();
        let outside = tempfile::Builder::new()
            .prefix("track-artifact-target-")
            .tempdir_in(std::env::temp_dir())
            .unwrap();
        let target = outside.path().join("elsewhere.json");
        std::fs::write(&target, "{}").unwrap();

        let track_dir = dir.path().join("some-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::os::unix::fs::symlink(&target, track_dir.join("artifact.json")).unwrap();

        let error = read_track_artifact(
            dir.path(),
            &TrackId::try_new("some-track").unwrap(),
            "artifact.json",
            1024,
        )
        .unwrap_err();

        let word = failure_word(&error);
        // The link is refused below the items directory before containment is
        // decided, so this is the refusal that answers — containment stays as
        // the check behind it.
        assert_eq!(word, "rejected as a symlink");
        // Neither the directory that was searched nor the file the link landed
        // on is named: the escape target is outside what the operator configured.
        assert_names_no_path(word, dir.path());
        assert_names_no_path(word, outside.path());
    }

    #[test]
    fn test_read_track_artifact_rejects_non_regular_files() {
        let dir = items_dir();
        let track_dir = dir.path().join("some-track");
        std::fs::create_dir_all(track_dir.join("artifact.json")).unwrap();

        let error = read_track_artifact(
            dir.path(),
            &TrackId::try_new("some-track").unwrap(),
            "artifact.json",
            1024,
        )
        .unwrap_err();

        let word = failure_word(&error);
        assert_eq!(word, "not a regular file");
        assert_names_no_path(word, dir.path());
    }

    #[cfg(unix)]
    #[test]
    fn test_read_track_artifact_rejects_fifo_without_blocking() {
        let dir = items_dir();
        let track_dir = dir.path().join("some-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let fifo = track_dir.join("artifact.json");
        rustix::fs::mkfifoat(rustix::fs::CWD, &fifo, rustix::fs::Mode::from_raw_mode(0o600))
            .unwrap();

        let error = read_track_artifact(
            dir.path(),
            &TrackId::try_new("some-track").unwrap(),
            "artifact.json",
            1024,
        )
        .unwrap_err();

        let word = failure_word(&error);
        assert_eq!(word, "not a regular file");
        assert_names_no_path(word, dir.path());
    }

    #[test]
    fn test_read_track_artifact_rejects_files_larger_than_the_bound() {
        let dir = items_dir();
        let track_dir = dir.path().join("some-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("artifact.json"), b"12345").unwrap();

        let error = read_track_artifact(
            dir.path(),
            &TrackId::try_new("some-track").unwrap(),
            "artifact.json",
            4,
        )
        .unwrap_err();

        let word = failure_word(&error);
        assert_eq!(word, "larger than the maximum size allowed");
        // Nor is the size the file turned out to have: only that it is past the
        // bound the caller set.
        assert_names_no_path(word, dir.path());
    }

    #[test]
    fn test_a_refusal_names_what_was_wrong_and_never_where_the_reader_looked() {
        // The whole refusal surface at once, so a branch added later that
        // formats a path is caught here rather than at the adapter that renders
        // it. Every one of these is a `Failed`, and every classification is a
        // fixed word.
        let dir = items_dir();
        let track_dir = dir.path().join("some-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("not-utf8.json"), [0xff, 0xfe, 0x00]).unwrap();
        std::fs::write(track_dir.join("too-big.json"), b"12345").unwrap();
        std::fs::create_dir_all(track_dir.join("a-directory.json")).unwrap();

        let track = TrackId::try_new("some-track").unwrap();
        let refusals = [
            read_track_artifact(dir.path(), &track, "not-utf8.json", 1024).unwrap_err(),
            read_track_artifact(dir.path(), &track, "too-big.json", 4).unwrap_err(),
            read_track_artifact(dir.path(), &track, "a-directory.json", 1024).unwrap_err(),
            read_track_artifact(Path::new(""), &track, "artifact.json", 1024).unwrap_err(),
            read_track_artifact(&dir.path().join(".."), &track, "artifact.json", 1024).unwrap_err(),
        ];

        let words: Vec<&'static str> = refusals.iter().map(failure_word).collect();
        assert_eq!(
            words,
            [
                "not valid UTF-8",
                "larger than the maximum size allowed",
                "not a regular file",
                "the items directory must not be empty",
                "the items directory must not contain '..'",
            ]
        );
        for word in words {
            assert_names_no_path(word, dir.path());
        }
    }
}
