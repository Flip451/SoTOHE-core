//! Branch strategy domain types.

use crate::{RefValidationError, TrackBranch, TrackId, TrackMetadata, validate_branch_ref};

/// Validated name of the base branch permitted as a merge source.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BaseBranchName {
    value: String,
}

impl BaseBranchName {
    /// Validates and stores a git branch reference.
    ///
    /// # Errors
    ///
    /// Returns [`RefValidationError`] when `value` is not a safe branch reference.
    pub fn try_new(value: String) -> Result<Self, RefValidationError> {
        validate_branch_ref(&value)?;
        validate_git_branch_name(&value)?;
        Ok(Self { value })
    }

    /// Returns the validated base-branch reference.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// Applies the remaining pure Git branch-name restrictions not needed by
/// `validate_branch_ref`'s `origin/{branch}:path` interpolation use case.
fn validate_git_branch_name(branch: &str) -> Result<(), RefValidationError> {
    if branch.starts_with('-') {
        return Err(RefValidationError::DisallowedCharacter("leading '-'".to_owned()));
    }
    if branch == "@" {
        return Err(RefValidationError::DisallowedCharacter("@".to_owned()));
    }
    if branch.starts_with('/') || branch.ends_with('/') || branch.contains("//") {
        return Err(RefValidationError::DisallowedCharacter("/".to_owned()));
    }
    if branch.ends_with('.') {
        return Err(RefValidationError::DisallowedCharacter("trailing '.'".to_owned()));
    }
    if branch.split('/').any(|component| component.ends_with(".lock")) {
        return Err(RefValidationError::DisallowedCharacter(".lock".to_owned()));
    }
    if branch.split('/').any(|component| component.starts_with('.')) {
        return Err(RefValidationError::DisallowedCharacter("leading '.'".to_owned()));
    }
    if let Some(invalid) = branch.chars().find(|ch| matches!(ch, '?' | '*' | '[' | '\\')) {
        return Err(RefValidationError::DisallowedCharacter(invalid.to_string()));
    }

    Ok(())
}

/// Direction information for a base-to-track merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseMergeDirection {
    track_id: TrackId,
    active_track: TrackBranch,
    source: BaseBranchName,
}

impl BaseMergeDirection {
    /// Returns the active track identifier.
    #[must_use]
    pub fn track_id(&self) -> &TrackId {
        &self.track_id
    }

    /// Returns the active track branch.
    #[must_use]
    pub fn active_track(&self) -> &TrackBranch {
        &self.active_track
    }

    /// Returns the validated base-branch merge source.
    #[must_use]
    pub fn source(&self) -> &BaseBranchName {
        &self.source
    }
}

/// Errors deriving a base-to-track merge direction.
#[derive(Debug, thiserror::Error)]
pub enum BaseMergeDirectionError {
    /// The track has no materialized branch.
    #[error("base merge requires an active track branch")]
    InactiveTrack,
    /// The snapshot base branch is not a safe git reference.
    #[error("invalid base branch name: {0}")]
    InvalidBaseName(#[from] RefValidationError),
}

/// Derives the only permitted merge direction from track metadata.
///
/// # Errors
///
/// Returns [`BaseMergeDirectionError::InactiveTrack`] when the track has no branch, or
/// [`BaseMergeDirectionError::InvalidBaseName`] when its snapshot base branch is invalid.
pub fn derive_base_merge_direction(
    track: &TrackMetadata,
) -> Result<BaseMergeDirection, BaseMergeDirectionError> {
    let active_track = track.branch().cloned().ok_or(BaseMergeDirectionError::InactiveTrack)?;
    let source =
        BaseBranchName::try_new(track.branch_strategy_snapshot().base_branch().to_owned())?;
    Ok(BaseMergeDirection { track_id: track.id().clone(), active_track, source })
}

/// The merge method used when integrating a track branch into the merge target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMethod {
    Squash,
    Merge,
    Rebase,
}

/// Immutable snapshot of branch strategy configuration captured at track init time.
///
/// Carries base_branch, merge_target, and merge_method. Created at `/track:init` time
/// and stored in `metadata.json#branch_strategy_snapshot` so that global config
/// changes do not affect in-flight tracks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchStrategySnapshot {
    base_branch: crate::NonEmptyString,
    merge_target: crate::NonEmptyString,
    merge_method: MergeMethod,
}

impl BranchStrategySnapshot {
    /// Creates a new snapshot with the given base branch, merge target, and merge method.
    pub fn new(
        base_branch: crate::NonEmptyString,
        merge_target: crate::NonEmptyString,
        merge_method: MergeMethod,
    ) -> Self {
        Self { base_branch, merge_target, merge_method }
    }

    /// Returns the base branch name (branch from which track branches are created).
    #[must_use]
    pub fn base_branch(&self) -> &str {
        self.base_branch.as_ref()
    }

    /// Returns the merge target branch name (branch into which track branches are merged).
    #[must_use]
    pub fn merge_target(&self) -> &str {
        self.merge_target.as_ref()
    }

    /// Returns the merge method.
    #[must_use]
    pub fn merge_method(&self) -> MergeMethod {
        self.merge_method
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::StatusOverride;

    fn make_snapshot(base: &str, target: &str, method: MergeMethod) -> BranchStrategySnapshot {
        BranchStrategySnapshot::new(
            crate::NonEmptyString::try_new(base).unwrap(),
            crate::NonEmptyString::try_new(target).unwrap(),
            method,
        )
    }

    #[test]
    fn branch_strategy_snapshot_accessors_return_stored_values() {
        let snap = make_snapshot("main", "main", MergeMethod::Squash);
        assert_eq!(snap.base_branch(), "main");
        assert_eq!(snap.merge_target(), "main");
        assert_eq!(snap.merge_method(), MergeMethod::Squash);
    }

    #[test]
    fn branch_strategy_snapshot_develop_variant() {
        let snap = make_snapshot("develop", "develop", MergeMethod::Merge);
        assert_eq!(snap.base_branch(), "develop");
        assert_eq!(snap.merge_target(), "develop");
        assert_eq!(snap.merge_method(), MergeMethod::Merge);
    }

    #[test]
    fn merge_method_rebase_stored_correctly() {
        let snap = make_snapshot("main", "main", MergeMethod::Rebase);
        assert_eq!(snap.merge_method(), MergeMethod::Rebase);
    }

    #[test]
    fn branch_strategy_snapshot_equality() {
        let a = make_snapshot("main", "main", MergeMethod::Squash);
        let b = make_snapshot("main", "main", MergeMethod::Squash);
        assert_eq!(a, b);
    }

    #[test]
    fn branch_strategy_snapshot_inequality_on_method() {
        let a = make_snapshot("main", "main", MergeMethod::Squash);
        let b = make_snapshot("main", "main", MergeMethod::Merge);
        assert_ne!(a, b);
    }

    #[test]
    fn test_derive_base_merge_direction_active_track_uses_snapshot_base_branch() {
        let track = TrackMetadata::with_branch(
            TrackId::try_new("merge-track").unwrap(),
            Some(TrackBranch::try_new("track/merge-track").unwrap()),
            "Merge track",
            Option::<StatusOverride>::None,
            make_snapshot("develop", "develop", MergeMethod::Merge),
        )
        .unwrap();

        let direction = derive_base_merge_direction(&track).unwrap();

        assert_eq!(direction.track_id().as_ref(), "merge-track");
        assert_eq!(direction.active_track().as_ref(), "track/merge-track");
        assert_eq!(direction.source().as_str(), "develop");
    }

    #[test]
    fn test_derive_base_merge_direction_inactive_track_returns_error() {
        let track = TrackMetadata::new(
            TrackId::try_new("planned-track").unwrap(),
            "Planned track",
            None,
            make_snapshot("develop", "develop", MergeMethod::Merge),
        )
        .unwrap();

        assert!(matches!(
            derive_base_merge_direction(&track),
            Err(BaseMergeDirectionError::InactiveTrack)
        ));
    }

    #[test]
    fn test_derive_base_merge_direction_invalid_snapshot_base_returns_error() {
        let track = TrackMetadata::with_branch(
            TrackId::try_new("merge-track").unwrap(),
            Some(TrackBranch::try_new("track/merge-track").unwrap()),
            "Merge track",
            None,
            make_snapshot("develop..unsafe", "develop", MergeMethod::Merge),
        )
        .unwrap();

        assert!(matches!(
            derive_base_merge_direction(&track),
            Err(BaseMergeDirectionError::InvalidBaseName(RefValidationError::DisallowedCharacter(
                _
            )))
        ));
    }

    #[test]
    fn test_derive_base_merge_direction_option_like_snapshot_base_returns_error() {
        let track = TrackMetadata::with_branch(
            TrackId::try_new("merge-track").unwrap(),
            Some(TrackBranch::try_new("track/merge-track").unwrap()),
            "Merge track",
            None,
            make_snapshot("--abort", "develop", MergeMethod::Merge),
        )
        .unwrap();

        assert!(matches!(
            derive_base_merge_direction(&track),
            Err(BaseMergeDirectionError::InvalidBaseName(RefValidationError::DisallowedCharacter(
                reason
            ))) if reason == "leading '-'"
        ));
    }

    #[test]
    fn test_base_branch_name_rejects_git_invalid_branch_syntax() {
        for invalid_name in [
            ".hidden",
            "feature/.hidden",
            "feature//branch",
            "feature/",
            "feature.",
            "feature.lock",
            "feature.lock/topic",
            "feature?query",
            "feature*glob",
            "feature[range",
            "feature\\path",
            "@",
        ] {
            assert!(
                BaseBranchName::try_new(invalid_name.to_owned()).is_err(),
                "{invalid_name:?} must not be a valid base branch"
            );
        }
    }
}
