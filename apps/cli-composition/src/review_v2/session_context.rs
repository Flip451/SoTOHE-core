//! Validated reviewer identity, prompt, and session-cache wiring for one round.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::{
    TrackId,
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

    Ok(ReviewerSessionContext { track_id, scope, round_type, model, prompt, cache })
}
