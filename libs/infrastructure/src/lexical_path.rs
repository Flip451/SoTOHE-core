//! Lexical path resolution: `.` and `..` folded away without touching the disk.
//!
//! A containment check normally canonicalises both sides, which resolves symlinks
//! and is what a guard wants — but canonicalising requires the path to exist. A
//! path that does not exist still has a location, and a check that gives up on it
//! would let an absent path outside a trusted root pass as an ordinary absence.
//! Folding the components is how that question is answered without a filesystem
//! round-trip.
//!
//! Lexical folding is not a substitute for canonicalisation where the path does
//! exist: `a/link/..` folds to `a` while the filesystem would resolve it through
//! the link's target. Callers canonicalise when they can and fold only when there
//! is nothing on disk to resolve.

use std::path::{Component, Path, PathBuf};

/// Folds `.` and `..` away without consulting the filesystem.
///
/// A leading `..` that cannot be folded (nothing precedes it) is kept, so the
/// result stays a faithful description of where the path points.
pub(crate) fn lexical_normalize(path: &Path) -> PathBuf {
    let mut components: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => match components.last() {
                Some(Component::Normal(_)) => {
                    components.pop();
                }
                _ => components.push(component),
            },
            Component::CurDir => {}
            _ => components.push(component),
        }
    }
    components.iter().collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_folding_removes_the_steps_that_cancel_out() {
        assert_eq!(lexical_normalize(Path::new("/a/./b/../c")), PathBuf::from("/a/c"));
        assert_eq!(lexical_normalize(Path::new("/a/b/../..")), PathBuf::from("/"));
    }

    #[test]
    fn test_a_leading_parent_step_is_kept_rather_than_dropped() {
        // Dropping it would silently turn a path that escapes into one that does
        // not, which is the opposite of what a containment check needs.
        assert_eq!(lexical_normalize(Path::new("../outside")), PathBuf::from("../outside"));
        assert_eq!(lexical_normalize(Path::new("a/../../outside")), PathBuf::from("../outside"));
    }
}
