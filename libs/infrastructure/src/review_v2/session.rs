//! Shared fail-soft reviewer session-cache handling.

use std::sync::Arc;

use domain::review_v2::{RoundType, ScopeName};
use domain::{CommitHash, TrackId};
use usecase::capability_exec::{ModelName, ProviderName, ReasoningEffort};
use usecase::provider_session::{
    ProviderSessionCacheEntry, ProviderSessionCacheKey, ProviderSessionCachePort, ProviderSessionId,
};

/// Cache identity and resolved profile for one reviewer round.
pub(super) struct ReviewerSession {
    cache: Arc<dyn ProviderSessionCachePort>,
    key: Option<ProviderSessionCacheKey>,
    provider: &'static str,
    model: ModelName,
    effort: ReasoningEffort,
}

impl ReviewerSession {
    #[allow(clippy::too_many_arguments)] // cache identity and resolved profile are constructed together
    pub(super) fn new(
        track_id: TrackId,
        scope: ScopeName,
        round_type: RoundType,
        diff_base: Option<CommitHash>,
        provider: &'static str,
        model: ModelName,
        effort: ReasoningEffort,
        cache: Arc<dyn ProviderSessionCachePort>,
    ) -> Self {
        Self {
            cache,
            key: diff_base.map(|diff_base| ProviderSessionCacheKey::Review {
                track_id,
                scope,
                round_type,
                diff_base,
            }),
            provider,
            model,
            effort,
        }
    }

    /// Loads only an entry created by the currently resolved provider and model.
    /// Cache I/O and diff-base resolution are intentionally fail-soft: a reviewer round must
    /// still start fresh.
    pub(super) fn resumable_id(&self) -> Option<String> {
        let key = self.key.as_ref()?;
        let entry = self.cache.load(key).ok().flatten()?;
        (entry.provider().as_str() == self.provider
            && entry.model() == &self.model
            && entry.effort() == self.effort)
            .then(|| entry.session_id().as_str().to_owned())
    }

    pub(super) fn effort(&self) -> ReasoningEffort {
        self.effort
    }

    /// Persists a successfully observed provider session. Missing or invalid IDs are ignored.
    pub(super) fn save(&self, session_id: Option<String>) {
        let Some(session_id) = session_id else {
            return;
        };
        let Ok(session_id) = ProviderSessionId::try_new(session_id) else {
            return;
        };
        let Ok(provider) = ProviderName::try_new(self.provider.to_owned()) else {
            return;
        };
        let entry =
            ProviderSessionCacheEntry::new(session_id, provider, self.model.clone(), self.effort);
        if let Some(key) = &self.key {
            let _ = self.cache.save(key, &entry);
        }
    }
}

pub(super) fn effort_value(effort: ReasoningEffort) -> &'static str {
    match effort {
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::XHigh => "xhigh",
        ReasoningEffort::Max => "max",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use domain::review_v2::MainScopeName;
    use usecase::provider_session::ProviderSessionCacheError;

    struct FakeCache {
        entry: Option<ProviderSessionCacheEntry>,
        expected_key: Option<ProviderSessionCacheKey>,
        fail_load: bool,
        saved: Mutex<Vec<ProviderSessionCacheEntry>>,
    }

    impl ProviderSessionCachePort for FakeCache {
        fn load(
            &self,
            key: &ProviderSessionCacheKey,
        ) -> Result<Option<ProviderSessionCacheEntry>, ProviderSessionCacheError> {
            if self.fail_load {
                return Err(ProviderSessionCacheError::EntryInvalid(
                    usecase::git_workflow::DiagnosticText::new("invalid"),
                ));
            }
            Ok((self.expected_key.as_ref() == Some(key)).then(|| self.entry.clone()).flatten())
        }

        fn save(
            &self,
            _: &ProviderSessionCacheKey,
            entry: &ProviderSessionCacheEntry,
        ) -> Result<(), ProviderSessionCacheError> {
            self.saved.lock().unwrap().push(entry.clone());
            Ok(())
        }

        fn remove(&self, _: &ProviderSessionCacheKey) -> Result<(), ProviderSessionCacheError> {
            Ok(())
        }
    }

    fn diff_base(value: &str) -> CommitHash {
        CommitHash::try_new(value).unwrap()
    }

    fn key(round_type: RoundType) -> ProviderSessionCacheKey {
        ProviderSessionCacheKey::Review {
            track_id: TrackId::try_new("session-test").unwrap(),
            scope: ScopeName::Main(MainScopeName::new("infrastructure").unwrap()),
            round_type,
            diff_base: diff_base("a1b2c3d"),
        }
    }

    fn entry(provider: &str, model: &str) -> ProviderSessionCacheEntry {
        entry_with_effort(provider, model, ReasoningEffort::High)
    }

    fn entry_with_effort(
        provider: &str,
        model: &str,
        effort: ReasoningEffort,
    ) -> ProviderSessionCacheEntry {
        ProviderSessionCacheEntry::new(
            ProviderSessionId::try_new("prior-session".to_owned()).unwrap(),
            ProviderName::try_new(provider.to_owned()).unwrap(),
            ModelName::try_new(model.to_owned()).unwrap(),
            effort,
        )
    }

    fn session(cache: Arc<FakeCache>, round_type: RoundType) -> ReviewerSession {
        ReviewerSession::new(
            TrackId::try_new("session-test").unwrap(),
            ScopeName::Main(MainScopeName::new("infrastructure").unwrap()),
            round_type,
            Some(diff_base("a1b2c3d")),
            "claude",
            ModelName::try_new("model-current".to_owned()).unwrap(),
            ReasoningEffort::High,
            cache,
        )
    }

    #[test]
    fn test_reviewer_session_matching_entry_resumes_and_persists_success() {
        let cache = Arc::new(FakeCache {
            entry: Some(entry("claude", "model-current")),
            expected_key: Some(key(RoundType::Fast)),
            fail_load: false,
            saved: Mutex::new(vec![]),
        });
        let session = session(cache.clone(), RoundType::Fast);
        assert_eq!(session.resumable_id().as_deref(), Some("prior-session"));
        session.save(Some("new-session".to_owned()));
        assert_eq!(
            cache.saved.lock().unwrap().first().unwrap().session_id().as_str(),
            "new-session"
        );
    }

    #[test]
    fn test_reviewer_session_mismatch_or_load_error_starts_fresh() {
        let mismatch = Arc::new(FakeCache {
            entry: Some(entry("codex", "model-current")),
            expected_key: Some(key(RoundType::Fast)),
            fail_load: false,
            saved: Mutex::new(vec![]),
        });
        assert_eq!(session(mismatch, RoundType::Fast).resumable_id(), None);
        let effort_mismatch = Arc::new(FakeCache {
            entry: Some(entry_with_effort("claude", "model-current", ReasoningEffort::Low)),
            expected_key: Some(key(RoundType::Fast)),
            fail_load: false,
            saved: Mutex::new(vec![]),
        });
        assert_eq!(session(effort_mismatch, RoundType::Fast).resumable_id(), None);
        let failure = Arc::new(FakeCache {
            entry: None,
            expected_key: Some(key(RoundType::Fast)),
            fail_load: true,
            saved: Mutex::new(vec![]),
        });
        assert_eq!(session(failure, RoundType::Fast).resumable_id(), None);
    }

    #[test]
    fn test_reviewer_session_final_never_loads_fast_entry() {
        let cache = Arc::new(FakeCache {
            entry: Some(entry("claude", "model-current")),
            expected_key: Some(key(RoundType::Fast)),
            fail_load: false,
            saved: Mutex::new(vec![]),
        });
        assert_eq!(session(cache, RoundType::Final).resumable_id(), None);
    }

    #[test]
    fn test_reviewer_session_different_diff_base_starts_fresh() {
        let cache = Arc::new(FakeCache {
            entry: Some(entry("claude", "model-current")),
            expected_key: Some(key(RoundType::Fast)),
            fail_load: false,
            saved: Mutex::new(vec![]),
        });
        let session = ReviewerSession::new(
            TrackId::try_new("session-test").unwrap(),
            ScopeName::Main(MainScopeName::new("infrastructure").unwrap()),
            RoundType::Fast,
            Some(diff_base("d4e5f6a")),
            "claude",
            ModelName::try_new("model-current".to_owned()).unwrap(),
            ReasoningEffort::High,
            cache,
        );

        assert_eq!(session.resumable_id(), None);
    }

    #[test]
    fn test_reviewer_session_missing_diff_base_starts_fresh() {
        let cache = Arc::new(FakeCache {
            entry: Some(entry("claude", "model-current")),
            expected_key: Some(key(RoundType::Fast)),
            fail_load: false,
            saved: Mutex::new(vec![]),
        });
        let session = ReviewerSession::new(
            TrackId::try_new("session-test").unwrap(),
            ScopeName::Main(MainScopeName::new("infrastructure").unwrap()),
            RoundType::Fast,
            None,
            "claude",
            ModelName::try_new("model-current".to_owned()).unwrap(),
            ReasoningEffort::High,
            cache.clone(),
        );

        assert_eq!(session.resumable_id(), None);
        session.save(Some("new-session".to_owned()));
        assert!(cache.saved.lock().unwrap().is_empty());
    }
}
