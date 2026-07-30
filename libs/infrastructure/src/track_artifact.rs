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

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use domain::TrackId;

use crate::track::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};

/// Why a track artifact could not be handed back.
///
/// `NotFound` carries nothing: the ports that consume it declare an absent
/// artifact as a state of its own, with no diagnostic to render.
#[derive(Debug)]
pub(crate) enum TrackArtifactReadError {
    /// The artifact is not present for this track.
    NotFound,
    /// The artifact could not be read.
    Failed(String),
}

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
    let items_dir = resolve_items_dir(items_dir, file_name)?;

    let path = items_dir.join(track_id.as_ref()).join(file_name);

    match reject_symlinks_below(&path, &items_dir) {
        Ok(true) => {}
        Ok(false) => return Err(TrackArtifactReadError::NotFound),
        Err(error) => {
            return Err(TrackArtifactReadError::Failed(format!(
                "symlink check failed for {}: {error}",
                path.display()
            )));
        }
    }

    match path.canonicalize() {
        Ok(resolved) if resolved.starts_with(&items_dir) => {}
        Ok(resolved) => {
            return Err(TrackArtifactReadError::Failed(format!(
                "{file_name} resolves outside the items directory {}: {}",
                items_dir.display(),
                resolved.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(TrackArtifactReadError::NotFound);
        }
        Err(error) => {
            return Err(TrackArtifactReadError::Failed(format!(
                "cannot resolve {}: {error}",
                path.display()
            )));
        }
    }

    let path_metadata = std::fs::symlink_metadata(&path).map_err(|error| {
        TrackArtifactReadError::Failed(format!(
            "metadata error reading {}: {error}",
            path.display()
        ))
    })?;
    if !path_metadata.file_type().is_file() {
        return Err(TrackArtifactReadError::Failed(format!(
            "artifact {} is not a regular file",
            path.display()
        )));
    }

    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(TrackArtifactReadError::NotFound);
        }
        Err(error) => {
            return Err(TrackArtifactReadError::Failed(format!(
                "I/O error opening {}: {error}",
                path.display()
            )));
        }
    };
    let metadata = file.metadata().map_err(|error| {
        TrackArtifactReadError::Failed(format!(
            "metadata error reading {}: {error}",
            path.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(TrackArtifactReadError::Failed(format!(
            "artifact {} is not a regular file",
            path.display()
        )));
    }
    if metadata.len() > max_bytes {
        return Err(TrackArtifactReadError::Failed(format!(
            "{file_name} exceeds maximum size of {max_bytes} bytes: {} bytes",
            metadata.len()
        )));
    }

    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1)).read_to_end(&mut bytes).map_err(|error| {
        TrackArtifactReadError::Failed(format!("I/O error reading {}: {error}", path.display()))
    })?;
    if bytes.len() as u64 > max_bytes {
        return Err(TrackArtifactReadError::Failed(format!(
            "{file_name} exceeds maximum size of {max_bytes} bytes while reading",
        )));
    }

    String::from_utf8(bytes).map_err(|error| {
        TrackArtifactReadError::Failed(format!("UTF-8 error in {}: {error}", path.display()))
    })
}

/// Resolves the items directory the artifact must stay inside.
///
/// A relative path is taken as given, a `..` component is refused before any
/// filesystem access, and every component of the supplied path — the items
/// directory and each of its ancestors — is refused if it is a symlink, because
/// canonicalising first would follow any of them silently. The result is
/// canonical so containment can be decided on real paths rather than on the
/// spelling of the argument.
fn resolve_items_dir(items_dir: &Path, file_name: &str) -> Result<PathBuf, TrackArtifactReadError> {
    if items_dir.as_os_str().is_empty() {
        return Err(TrackArtifactReadError::Failed(format!(
            "items_dir must not be empty when reading {file_name}"
        )));
    }
    if items_dir.components().any(|component| matches!(component, Component::ParentDir)) {
        return Err(TrackArtifactReadError::Failed(format!(
            "items_dir must not contain '..' when reading {file_name}: {}",
            items_dir.display()
        )));
    }
    reject_symlinks_up_to_root(items_dir).map_err(|error| {
        TrackArtifactReadError::Failed(format!(
            "symlink check failed for items_dir {} before reading {file_name}: {error}",
            items_dir.display()
        ))
    })?;

    let canonical = match items_dir.canonicalize() {
        Ok(canonical) => canonical,
        // Nothing exists at that location, so no artifact of this track does
        // either: the same answer as an empty track directory.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(TrackArtifactReadError::NotFound);
        }
        Err(error) => {
            return Err(TrackArtifactReadError::Failed(format!(
                "cannot resolve items_dir {} before reading {file_name}: {error}",
                items_dir.display()
            )));
        }
    };
    if !canonical.is_dir() {
        return Err(TrackArtifactReadError::Failed(format!(
            "items_dir is not a directory: {}",
            items_dir.display()
        )));
    }
    Ok(canonical)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{TrackArtifactReadError, read_track_artifact};
    use domain::TrackId;

    fn items_dir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("track-artifact-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap()
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

        let error = read_track_artifact(
            &linked_root.join("track/items"),
            &TrackId::try_new("some-track").unwrap(),
            "artifact.json",
            1024,
        )
        .unwrap_err();

        assert!(matches!(error, TrackArtifactReadError::Failed(_)), "unexpected: {error:?}");
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

        assert!(matches!(error, TrackArtifactReadError::Failed(_)), "unexpected: {error:?}");
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

        assert!(matches!(error, TrackArtifactReadError::Failed(_)));
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

        assert!(matches!(error, TrackArtifactReadError::Failed(_)));
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

        assert!(matches!(error, TrackArtifactReadError::Failed(_)));
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

        assert!(matches!(error, TrackArtifactReadError::Failed(_)));
    }
}
