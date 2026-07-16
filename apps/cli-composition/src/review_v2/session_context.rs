//! Validated reviewer identity, prompt, and session-cache wiring for one round.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::{
    CommitHash, TrackId,
    review_v2::{MainScopeName, RoundType, ScopeName},
};
use infrastructure::provider_session::FsProviderSessionCacheAdapter;
use usecase::{
    capability_exec::ModelName,
    provider_session::{ProviderSessionCachePort, ReviewerPrompt},
};

use super::{CompositionError, shared};

/// Typed reviewer inputs validated at the composition boundary.
pub(super) struct ReviewerSessionContext {
    pub(super) track_id: TrackId,
    pub(super) scope: ScopeName,
    pub(super) round_type: RoundType,
    pub(super) diff_base: Option<CommitHash>,
    pub(super) model: ModelName,
    pub(super) prompt: ReviewerPrompt,
    pub(super) cache: Arc<dyn ProviderSessionCachePort>,
}

pub(super) fn reviewer_session_context(
    track_id: &str,
    group: &str,
    round_type: &str,
    model: &str,
    base_prompt: String,
    items_dir: &Path,
) -> Result<ReviewerSessionContext, CompositionError> {
    let track_id = TrackId::try_new(track_id)
        .map_err(|error| CompositionError::WiringFailed(error.to_string()))?;
    let scope = if group == "other" {
        ScopeName::Other
    } else {
        ScopeName::Main(
            MainScopeName::new(group)
                .map_err(|error| CompositionError::WiringFailed(error.to_string()))?,
        )
    };
    let round_type = match round_type {
        "fast" => RoundType::Fast,
        "final" => RoundType::Final,
        other => {
            return Err(CompositionError::WiringFailed(format!("invalid round type: {other}")));
        }
    };
    let model = ModelName::try_new(model.to_owned())
        .map_err(|error| CompositionError::WiringFailed(error.to_string()))?;
    let prompt = ReviewerPrompt::try_new(base_prompt)
        .map_err(|error| CompositionError::WiringFailed(error.to_string()))?;
    let repo_root = shared::repo_root_from_items_dir(items_dir)
        .map_err(|error| CompositionError::Infrastructure(error.to_string()))?;
    let cache: Arc<dyn ProviderSessionCachePort> = Arc::new(FsProviderSessionCacheAdapter::new(
        repo_root,
        PathBuf::from("tmp/capability-runtime"),
    ));
    // Session reuse is an optional optimization. If the diff base cannot be resolved, leave
    // the key absent so the reviewer starts fresh rather than risking cross-cycle context reuse.
    let diff_base = resolve_reviewer_diff_base(&track_id, items_dir);

    Ok(ReviewerSessionContext { track_id, scope, round_type, diff_base, model, prompt, cache })
}

fn resolve_reviewer_diff_base(track_id: &TrackId, items_dir: &Path) -> Option<CommitHash> {
    // The review-cycle builder can fall back to a base branch, but that path changes the
    // process CWD while resolving git state. Cache reuse is optional, so use the persisted
    // cycle base only: no state, invalid state, or read failure deliberately means fresh.
    let path = items_dir.join(track_id.to_string()).join(".commit_hash");
    let content = std::fs::read_to_string(path).ok()?;
    CommitHash::try_new(content.trim()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reviewer_diff_base_resolution_failure_disables_resume()
    -> Result<(), Box<dyn std::error::Error>> {
        let track_id = TrackId::try_new("session-context-test")?;

        assert_eq!(resolve_reviewer_diff_base(&track_id, Path::new("missing-review-items")), None);
        Ok(())
    }
}
