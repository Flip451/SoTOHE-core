//! Guarded reads of a single track artifact under `<items_dir>/<track-id>/`.
//!
//! Crate-private helper shared by the batch-plan adapters. It composes the
//! existing repository-root resolution and symlink guards rather than adding a
//! second path policy, and keeps "the artifact is not there" distinct from
//! "the read failed", which is the distinction the driven ports declare.

use std::io::Read;
use std::path::{Path, PathBuf};

use domain::TrackId;

use crate::resolve_items_dir_under_current_repo;
use crate::track::symlink_guard::reject_symlinks_below;

/// Why a track artifact could not be handed back.
///
/// `NotFound` carries nothing: the ports that consume it declare an absent
/// artifact as a state of its own, with no diagnostic to render.
pub(crate) enum TrackArtifactReadError {
    /// The artifact is not present for this track.
    NotFound,
    /// The artifact could not be read.
    Failed(String),
}

/// Reads `<items_dir>/<track_id>/<file_name>` as UTF-8 text.
///
/// Refuses a symlinked path anywhere below the items directory, an items
/// directory outside the current repository, and a file larger than
/// `max_bytes`.
pub(crate) fn read_track_artifact(
    items_dir: &Path,
    track_id: &TrackId,
    file_name: &str,
    max_bytes: u64,
) -> Result<String, TrackArtifactReadError> {
    let items_dir = resolve_items_dir_under_current_repo(items_dir).map_err(|error| {
        TrackArtifactReadError::Failed(format!(
            "items_dir rejected before reading {file_name}: {error}"
        ))
    })?;
    reject_symlinked_items_dir(&items_dir)?;

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

fn reject_symlinked_items_dir(items_dir: &PathBuf) -> Result<(), TrackArtifactReadError> {
    match std::fs::symlink_metadata(items_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(TrackArtifactReadError::Failed(
            format!("symlink check failed for {}: refused symlink", items_dir.display()),
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(TrackArtifactReadError::Failed(format!(
            "metadata error reading {}: {error}",
            items_dir.display()
        ))),
    }
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
