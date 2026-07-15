//! Cache-key selection and fail-soft persistence for capability sessions.

use std::sync::Arc;

use domain::TrackId;
use usecase::capability_exec::{
    CapabilityDispatchRequest, CapabilityResumeRequest, ModelName, ProviderName, ReasoningEffort,
};
use usecase::provider_session::{
    ProviderSessionCacheEntry, ProviderSessionCacheKey, ProviderSessionCachePort, ProviderSessionId,
};

/// Resolves the cache identity for one capability dispatch without crossing a track boundary.
#[must_use]
pub(crate) fn cache_key(
    request: &CapabilityDispatchRequest,
    track_id: Option<&TrackId>,
) -> Option<ProviderSessionCacheKey> {
    if let Some(track_id) = track_id {
        return Some(ProviderSessionCacheKey::TrackCapability {
            track_id: track_id.clone(),
            capability: request.request.capability.clone(),
        });
    }
    match &request.request.resume {
        CapabilityResumeRequest::Resume(target_artifacts) => {
            Some(ProviderSessionCacheKey::WorkspaceCapability {
                capability: request.request.capability.clone(),
                target_artifacts: target_artifacts.clone(),
            })
        }
        CapabilityResumeRequest::Fresh | CapabilityResumeRequest::ResumeWithoutTarget => None,
    }
}

/// Loads and saves a cache entry only when the selected provider and model still match.
pub(crate) struct CapabilitySession {
    cache: Arc<dyn ProviderSessionCachePort>,
    key: Option<ProviderSessionCacheKey>,
    provider: ProviderName,
    model: ModelName,
    effort: ReasoningEffort,
}

impl CapabilitySession {
    pub(crate) fn new(
        request: &CapabilityDispatchRequest,
        track_id: Option<&TrackId>,
        cache: Arc<dyn ProviderSessionCachePort>,
    ) -> Self {
        Self {
            cache,
            key: cache_key(request, track_id),
            provider: request.profile.provider.clone(),
            model: request.profile.model.clone(),
            effort: request.profile.effort,
        }
    }

    /// Returns a cached ID only for an explicit resume request and matching profile.
    pub(crate) fn resumable_id(&self, request: &CapabilityResumeRequest) -> Option<String> {
        if !matches!(
            request,
            CapabilityResumeRequest::Resume(_) | CapabilityResumeRequest::ResumeWithoutTarget
        ) {
            return None;
        }
        let key = self.key.as_ref()?;
        let entry = self.cache.load(key).ok().flatten()?;
        (entry.provider() == &self.provider && entry.model() == &self.model)
            .then(|| entry.session_id().as_str().to_owned())
    }

    /// Persists a newly observed provider ID. Invalid IDs and cache failures are fail-soft.
    pub(crate) fn save(&self, session_id: Option<String>) {
        let (Some(key), Some(session_id)) = (self.key.as_ref(), session_id) else {
            return;
        };
        let Ok(session_id) = ProviderSessionId::try_new(session_id) else {
            return;
        };
        let entry = ProviderSessionCacheEntry::new(
            session_id,
            self.provider.clone(),
            self.model.clone(),
            self.effort,
        );
        let _ = self.cache.save(key, &entry);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use domain::TrackId;
    use usecase::capability_exec::{
        BriefingText, CapabilityDispatchRequest, CapabilityExecRequest, CapabilityFilePath,
        CapabilityProfile, CapabilityResumeRequest, DisciplineText, ExecutionMode, ModelName,
        ProviderName, ReasoningEffort, TargetArtifactPath, TargetArtifactSet,
    };
    use usecase::dry_write_driver::CapabilityName;
    use usecase::provider_session::{
        ProviderSessionCacheEntry, ProviderSessionCacheError, ProviderSessionCacheKey,
        ProviderSessionCachePort, ProviderSessionId,
    };

    use super::{CapabilitySession, cache_key};

    #[derive(Default)]
    struct FakeCache {
        entries: Mutex<HashMap<ProviderSessionCacheKey, ProviderSessionCacheEntry>>,
    }

    impl ProviderSessionCachePort for FakeCache {
        fn load(
            &self,
            key: &ProviderSessionCacheKey,
        ) -> Result<Option<ProviderSessionCacheEntry>, ProviderSessionCacheError> {
            Ok(self.entries.lock().expect("test cache lock").get(key).cloned())
        }

        fn save(
            &self,
            key: &ProviderSessionCacheKey,
            entry: &ProviderSessionCacheEntry,
        ) -> Result<(), ProviderSessionCacheError> {
            self.entries.lock().expect("test cache lock").insert(key.clone(), entry.clone());
            Ok(())
        }

        fn remove(&self, key: &ProviderSessionCacheKey) -> Result<(), ProviderSessionCacheError> {
            self.entries.lock().expect("test cache lock").remove(key);
            Ok(())
        }
    }

    fn request(resume: CapabilityResumeRequest) -> CapabilityDispatchRequest {
        CapabilityDispatchRequest {
            request: CapabilityExecRequest {
                capability: CapabilityName::try_new("implementer").expect("valid capability"),
                host: ProviderName::try_new("codex").expect("valid host"),
                briefing_file: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))
                    .expect("valid briefing"),
                timeout: None,
                resume,
            },
            profile: CapabilityProfile {
                provider: ProviderName::try_new("codex").expect("valid provider"),
                model: ModelName::try_new("gpt-5").expect("valid model"),
                effort: ReasoningEffort::High,
                execution_mode: ExecutionMode::OrchestratorOutput,
            },
            briefing: BriefingText::try_new("briefing".to_owned()).expect("valid briefing text"),
            discipline: DisciplineText::try_new("discipline".to_owned()).expect("valid discipline"),
        }
    }

    fn targets(path: &str) -> TargetArtifactSet {
        TargetArtifactSet::try_new(vec![
            TargetArtifactPath::try_new(PathBuf::from(path)).expect("valid target"),
        ])
        .expect("non-empty targets")
    }

    #[test]
    fn test_capability_session_track_key_is_selected_for_fresh_and_resume() {
        let track_id = TrackId::try_new("track-a").expect("valid track");
        let fresh = request(CapabilityResumeRequest::Fresh);
        let resume = request(CapabilityResumeRequest::Resume(targets("track/items/a/spec.json")));
        let expected = ProviderSessionCacheKey::TrackCapability {
            track_id,
            capability: CapabilityName::try_new("implementer").expect("valid capability"),
        };

        assert_eq!(cache_key(&fresh, expected_track_id(&expected)), Some(expected.clone()));
        assert_eq!(cache_key(&resume, expected_track_id(&expected)), Some(expected));
    }

    #[test]
    fn test_capability_session_resume_reuses_only_the_same_track_and_capability() {
        let track_a = TrackId::try_new("track-a").expect("valid track");
        let track_b = TrackId::try_new("track-b").expect("valid track");
        let request = request(CapabilityResumeRequest::Resume(targets("track/items/a/spec.json")));
        let cache = Arc::new(FakeCache::default());
        let matching_key = cache_key(&request, Some(&track_a)).expect("track key");
        cache
            .save(
                &matching_key,
                &ProviderSessionCacheEntry::new(
                    ProviderSessionId::try_new("same-track-session".to_owned())
                        .expect("session id"),
                    request.profile.provider.clone(),
                    request.profile.model.clone(),
                    request.profile.effort,
                ),
            )
            .expect("seed matching cache entry");

        let same_track = CapabilitySession::new(&request, Some(&track_a), cache.clone());
        let other_track = CapabilitySession::new(&request, Some(&track_b), cache.clone());
        let mut other_capability_request = request.clone();
        other_capability_request.request.capability =
            CapabilityName::try_new("spec-designer").expect("valid capability");
        let other_capability =
            CapabilitySession::new(&other_capability_request, Some(&track_a), cache.clone());

        assert_eq!(
            same_track.resumable_id(&request.request.resume),
            Some("same-track-session".to_owned())
        );
        assert_eq!(other_track.resumable_id(&request.request.resume), None);
        assert_eq!(other_capability.resumable_id(&other_capability_request.request.resume), None);
        assert_eq!(same_track.resumable_id(&CapabilityResumeRequest::Fresh), None);
    }

    #[test]
    fn test_capability_session_workspace_keys_are_target_scoped_and_targetless_is_unrecorded() {
        let first = request(CapabilityResumeRequest::Resume(targets("track/items/a/spec.json")));
        let second = request(CapabilityResumeRequest::Resume(targets("track/items/a/plan.json")));
        let targetless = request(CapabilityResumeRequest::ResumeWithoutTarget);

        assert_ne!(cache_key(&first, None), cache_key(&second, None));
        assert_eq!(cache_key(&targetless, None), None);
        assert!(matches!(
            cache_key(&first, None),
            Some(ProviderSessionCacheKey::WorkspaceCapability {
                ref capability,
                ref target_artifacts,
            }) if capability.as_str() == "implementer"
                && target_artifacts.as_slice().first().map(|path| path.as_path())
                    == Some(std::path::Path::new("track/items/a/spec.json"))
        ));

        let cache = Arc::new(FakeCache::default());
        let targetless_session = CapabilitySession::new(&targetless, None, cache.clone());
        targetless_session.save(Some("unrecorded-session".to_owned()));
        assert!(cache.entries.lock().expect("test cache lock").is_empty());
    }

    #[test]
    fn test_capability_session_mismatch_starts_fresh_and_success_persists_matching_profile() {
        let request = request(CapabilityResumeRequest::Resume(targets("track/items/a/spec.json")));
        let cache = Arc::new(FakeCache::default());
        let session = CapabilitySession::new(&request, None, cache.clone());
        let key = cache_key(&request, None).expect("workspace key");
        let current_provider = request.profile.provider.clone();
        let current_model = request.profile.model.clone();
        let recorded_provider = ProviderName::try_new("claude").expect("provider");
        let recorded_model = ModelName::try_new("gpt-5").expect("model");
        assert_ne!(current_provider, recorded_provider);
        assert_eq!(current_model, recorded_model);
        cache
            .save(
                &key,
                &ProviderSessionCacheEntry::new(
                    ProviderSessionId::try_new("stale-session".to_owned()).expect("session id"),
                    recorded_provider,
                    recorded_model,
                    ReasoningEffort::High,
                ),
            )
            .expect("seed cache");

        assert_eq!(session.resumable_id(&request.request.resume), None);
        session.save(Some("new-session".to_owned()));
        assert_eq!(
            cache.load(&key).expect("load cache").expect("saved entry").session_id().as_str(),
            "new-session"
        );
    }

    fn expected_track_id(key: &ProviderSessionCacheKey) -> Option<&TrackId> {
        match key {
            ProviderSessionCacheKey::TrackCapability { track_id, .. } => Some(track_id),
            ProviderSessionCacheKey::Review { .. }
            | ProviderSessionCacheKey::WorkspaceCapability { .. } => None,
        }
    }
}
