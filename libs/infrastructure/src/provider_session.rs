//! Symlink-safe machine-local provider-session cache.

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};
use usecase::capability_exec::{ModelName, ProviderName, ReasoningEffort};
use usecase::git_workflow::DiagnosticText;
use usecase::provider_session::{
    ProviderSessionCacheEntry, ProviderSessionCacheError, ProviderSessionCacheKey,
    ProviderSessionCachePort, ProviderSessionId,
};

use crate::track::symlink_guard::reject_symlinks_below;

const MAX_CACHE_ENTRY_BYTES: u64 = 256 * 1024;

/// Persists symlink-safe machine-local provider sessions in track-local or
/// workspace-local storage.
pub struct FsProviderSessionCacheAdapter {
    repo_root: PathBuf,
    workspace_runtime_dir: PathBuf,
}

impl FsProviderSessionCacheAdapter {
    /// Creates a cache adapter rooted at one repository and its existing runtime path.
    #[must_use]
    pub fn new(repo_root: PathBuf, workspace_runtime_dir: PathBuf) -> Self {
        Self { repo_root, workspace_runtime_dir }
    }

    fn cache_path(
        &self,
        key: &ProviderSessionCacheKey,
    ) -> Result<(PathBuf, String), ProviderSessionCacheError> {
        let root = self.canonical_repo_root()?;
        let (dir, identity) = match key {
            ProviderSessionCacheKey::Review { track_id, scope, round_type, diff_base } => (
                root.join("track/items").join(track_id.to_string()).join(".provider-sessions"),
                format!("review:{track_id}:{scope}:{round_type}:{diff_base}"),
            ),
            ProviderSessionCacheKey::TrackCapability { track_id, capability } => (
                root.join("track/items").join(track_id.to_string()).join(".provider-sessions"),
                format!("track-capability:{track_id}:{}", capability.as_str()),
            ),
            ProviderSessionCacheKey::WorkspaceCapability { capability, target_artifacts } => (
                self.workspace_cache_dir(&root)?,
                format!(
                    "workspace-capability:{}:{}",
                    capability.as_str(),
                    artifact_identity(target_artifacts)
                ),
            ),
        };
        if !dir.starts_with(&root) {
            return Err(identity_boundary("cache directory escaped the repository root"));
        }
        Ok((dir.join(format!("{}.json", hash_text(&identity))), identity))
    }

    fn canonical_repo_root(&self) -> Result<PathBuf, ProviderSessionCacheError> {
        // Reject symlinks on EVERY ancestor before canonicalizing — a
        // symlinked ancestor would otherwise redirect the cache root outside
        // the supplied repository.
        crate::track::symlink_guard::reject_symlinks_up_to_root(&self.repo_root).map_err(
            |error| {
                identity_boundary(format!(
                    "repository root must be a non-symlink directory reachable without \
                     symlinked ancestors: {error}"
                ))
            },
        )?;
        let metadata = fs::symlink_metadata(&self.repo_root).map_err(map_io)?;
        if !metadata.is_dir() {
            return Err(identity_boundary("repository root must be a directory"));
        }
        self.repo_root.canonicalize().map_err(map_io)
    }

    fn workspace_cache_dir(&self, root: &Path) -> Result<PathBuf, ProviderSessionCacheError> {
        if self.workspace_runtime_dir.components().any(|component| {
            matches!(component, std::path::Component::ParentDir | std::path::Component::Prefix(_))
        }) {
            return Err(identity_boundary(
                "workspace runtime path contains an untrusted component",
            ));
        }
        let runtime = if self.workspace_runtime_dir.is_absolute() {
            self.workspace_runtime_dir.clone()
        } else {
            root.join(&self.workspace_runtime_dir)
        };
        if !runtime.starts_with(root) {
            return Err(identity_boundary("workspace runtime path escaped the repository root"));
        }
        Ok(runtime.join("provider-sessions"))
    }

    fn prepare_parent(&self, path: &Path, root: &Path) -> Result<(), ProviderSessionCacheError> {
        let parent = path.parent().ok_or_else(|| {
            identity_boundary("cache entry has no parent directory within the repository")
        })?;
        reject_symlinks_below(parent, root).map_err(map_symlink_or_io)?;
        fs::create_dir_all(parent).map_err(map_io)?;
        reject_symlinks_below(parent, root).map_err(map_symlink_or_io)?;
        Ok(())
    }
}

impl ProviderSessionCachePort for FsProviderSessionCacheAdapter {
    fn load(
        &self,
        key: &ProviderSessionCacheKey,
    ) -> Result<Option<ProviderSessionCacheEntry>, ProviderSessionCacheError> {
        let (path, identity) = self.cache_path(key)?;
        let root = self.canonical_repo_root()?;
        if !reject_symlinks_below(&path, &root).map_err(map_symlink_or_io)? {
            return Ok(None);
        }
        let persisted: PersistedEntry = serde_json::from_slice(&bounded_regular_file(&path)?)
            .map_err(|error| entry_invalid(format!("cache entry JSON decode failed: {error}")))?;
        if persisted.key != identity {
            return Err(entry_invalid("cache entry identity did not match the requested key"));
        }
        persisted.into_entry().map(Some)
    }

    fn save(
        &self,
        key: &ProviderSessionCacheKey,
        entry: &ProviderSessionCacheEntry,
    ) -> Result<(), ProviderSessionCacheError> {
        let (path, identity) = self.cache_path(key)?;
        let root = self.canonical_repo_root()?;
        self.prepare_parent(&path, &root)?;
        reject_symlinks_below(&path, &root).map_err(map_symlink_or_io)?;
        let content = serde_json::to_vec(&PersistedEntry::from_entry(identity, entry))
            .map_err(|error| entry_invalid(format!("cache entry JSON encode failed: {error}")))?;
        // Mirror the load-side bound so the adapter never persists an entry it
        // would refuse to read back.
        if content.len() as u64 > MAX_CACHE_ENTRY_BYTES {
            return Err(entry_invalid("cache entry exceeds the maximum size; refusing to save"));
        }
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .map_err(map_io)?;
        file.write_all(&content).map_err(map_io)?;
        file.sync_all().map_err(map_io)
    }

    fn remove(&self, key: &ProviderSessionCacheKey) -> Result<(), ProviderSessionCacheError> {
        let (path, _) = self.cache_path(key)?;
        let root = self.canonical_repo_root()?;
        if reject_symlinks_below(&path, &root).map_err(map_symlink_or_io)? {
            fs::remove_file(path).map_err(map_io)?;
        }
        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedEntry {
    key: String,
    session_id: String,
    provider: String,
    model: String,
    effort: PersistedEffort,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum PersistedEffort {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl PersistedEntry {
    fn from_entry(key: String, entry: &ProviderSessionCacheEntry) -> Self {
        Self {
            key,
            session_id: entry.session_id().as_str().to_owned(),
            provider: entry.provider().as_str().to_owned(),
            model: entry.model().as_str().to_owned(),
            effort: match entry.effort() {
                ReasoningEffort::Low => PersistedEffort::Low,
                ReasoningEffort::Medium => PersistedEffort::Medium,
                ReasoningEffort::High => PersistedEffort::High,
                ReasoningEffort::XHigh => PersistedEffort::Xhigh,
                ReasoningEffort::Max => PersistedEffort::Max,
            },
        }
    }

    fn into_entry(self) -> Result<ProviderSessionCacheEntry, ProviderSessionCacheError> {
        let session_id = ProviderSessionId::try_new(self.session_id)
            .map_err(|_| entry_invalid("cache entry contained an empty session identifier"))?;
        let provider = ProviderName::try_new(self.provider)
            .map_err(|_| entry_invalid("cache entry contained an empty provider name"))?;
        let model = ModelName::try_new(self.model)
            .map_err(|_| entry_invalid("cache entry contained an empty model name"))?;
        let effort = match self.effort {
            PersistedEffort::Low => ReasoningEffort::Low,
            PersistedEffort::Medium => ReasoningEffort::Medium,
            PersistedEffort::High => ReasoningEffort::High,
            PersistedEffort::Xhigh => ReasoningEffort::XHigh,
            PersistedEffort::Max => ReasoningEffort::Max,
        };
        Ok(ProviderSessionCacheEntry::new(session_id, provider, model, effort))
    }
}

fn bounded_regular_file(path: &Path) -> Result<Vec<u8>, ProviderSessionCacheError> {
    let metadata = fs::symlink_metadata(path).map_err(map_io)?;
    if metadata.file_type().is_symlink() {
        return Err(identity_boundary("cache entry was a symlink"));
    }
    if !metadata.is_file() || metadata.len() > MAX_CACHE_ENTRY_BYTES {
        return Err(entry_invalid("cache entry was not a bounded regular file"));
    }
    // The take-bound caps the allocation even if the file grows between the
    // stat above and this read; reading one extra byte detects that race.
    let mut bytes = Vec::new();
    let file = fs::File::open(path).map_err(map_io)?;
    file.take(MAX_CACHE_ENTRY_BYTES.saturating_add(1)).read_to_end(&mut bytes).map_err(map_io)?;
    if bytes.len() as u64 > MAX_CACHE_ENTRY_BYTES {
        return Err(entry_invalid("cache entry exceeded the maximum size after reading"));
    }
    Ok(bytes)
}

fn artifact_identity(target_artifacts: &usecase::capability_exec::TargetArtifactSet) -> String {
    target_artifacts
        .as_slice()
        .iter()
        .map(|path| path.as_path().as_os_str().as_encoded_bytes())
        .map(encode_hex)
        .collect::<Vec<_>>()
        .join(",")
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn hash_text(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn entry_invalid(detail: impl Into<String>) -> ProviderSessionCacheError {
    ProviderSessionCacheError::EntryInvalid(DiagnosticText::new(detail))
}

fn identity_boundary(detail: impl Into<String>) -> ProviderSessionCacheError {
    ProviderSessionCacheError::IdentityBoundaryViolation(DiagnosticText::new(detail))
}

fn map_io(error: std::io::Error) -> ProviderSessionCacheError {
    ProviderSessionCacheError::StorageUnavailable(DiagnosticText::new(format!(
        "filesystem operation failed: {}",
        error.kind()
    )))
}

fn map_symlink_or_io(error: std::io::Error) -> ProviderSessionCacheError {
    if error.kind() == std::io::ErrorKind::InvalidInput {
        identity_boundary("symlink encountered while accessing cache storage")
    } else {
        map_io(error)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::FsProviderSessionCacheAdapter;
    use domain::review_v2::{MainScopeName, RoundType, ScopeName};
    use domain::{CommitHash, TrackId};
    use usecase::capability_exec::{
        ModelName, ProviderName, ReasoningEffort, TargetArtifactPath, TargetArtifactSet,
    };
    use usecase::dry_write_driver::CapabilityName;
    use usecase::provider_session::{
        ProviderSessionCacheEntry, ProviderSessionCacheError, ProviderSessionCacheKey,
        ProviderSessionCachePort, ProviderSessionId,
    };

    fn workspace_key() -> ProviderSessionCacheKey {
        ProviderSessionCacheKey::WorkspaceCapability {
            capability: CapabilityName::try_new("implementer").unwrap(),
            target_artifacts: TargetArtifactSet::try_new(vec![
                TargetArtifactPath::try_new(PathBuf::from("tmp/briefing.md")).unwrap(),
            ])
            .unwrap(),
        }
    }

    fn track_key_for_target_set(_target_artifacts: TargetArtifactSet) -> ProviderSessionCacheKey {
        ProviderSessionCacheKey::TrackCapability {
            track_id: TrackId::try_new("track-a").unwrap(),
            capability: CapabilityName::try_new("implementer").unwrap(),
        }
    }

    fn entry() -> ProviderSessionCacheEntry {
        ProviderSessionCacheEntry::new(
            ProviderSessionId::try_new("session-1".to_owned()).unwrap(),
            ProviderName::try_new("codex").unwrap(),
            ModelName::try_new("gpt-5").unwrap(),
            ReasoningEffort::High,
        )
    }

    fn entry_with_session_id(session_id: &str) -> ProviderSessionCacheEntry {
        ProviderSessionCacheEntry::new(
            ProviderSessionId::try_new(session_id.to_owned()).unwrap(),
            ProviderName::try_new("codex").unwrap(),
            ModelName::try_new("gpt-5").unwrap(),
            ReasoningEffort::High,
        )
    }

    fn review_key(
        track_id: &str,
        scope: ScopeName,
        round_type: RoundType,
        diff_base: &str,
    ) -> ProviderSessionCacheKey {
        ProviderSessionCacheKey::Review {
            track_id: TrackId::try_new(track_id).unwrap(),
            scope,
            round_type,
            diff_base: CommitHash::try_new(diff_base).unwrap(),
        }
    }

    #[test]
    fn test_provider_session_cache_load_save_remove_round_trip() {
        let directory = tempfile::tempdir().unwrap();
        let cache = FsProviderSessionCacheAdapter::new(
            directory.path().to_path_buf(),
            PathBuf::from("tmp/runtime"),
        );
        let key = workspace_key();
        let expected = entry();

        assert_eq!(cache.load(&key).unwrap(), None);
        cache.save(&key, &expected).unwrap();
        assert_eq!(cache.load(&key).unwrap(), Some(expected));
        cache.remove(&key).unwrap();
        assert_eq!(cache.load(&key).unwrap(), None);
    }

    #[test]
    fn test_provider_session_cache_save_rejects_oversized_entry() {
        let directory = tempfile::tempdir().unwrap();
        let cache = FsProviderSessionCacheAdapter::new(
            directory.path().to_path_buf(),
            PathBuf::from("tmp/runtime"),
        );
        let key = workspace_key();
        let oversized = ProviderSessionCacheEntry::new(
            ProviderSessionId::try_new("s".repeat(512 * 1024)).unwrap(),
            ProviderName::try_new("codex").unwrap(),
            ModelName::try_new("gpt-5-mini").unwrap(),
            ReasoningEffort::Low,
        );

        let error = cache.save(&key, &oversized).unwrap_err();
        assert!(
            matches!(&error, ProviderSessionCacheError::EntryInvalid(detail)
                if detail.as_str().contains("maximum size")),
            "oversized entries must be rejected before writing: {error:?}"
        );
        assert_eq!(cache.load(&key).unwrap(), None, "nothing may be persisted for the key");
    }

    #[test]
    fn test_provider_session_cache_isolates_workspace_artifact_keys() {
        let directory = tempfile::tempdir().unwrap();
        let cache = FsProviderSessionCacheAdapter::new(
            directory.path().to_path_buf(),
            PathBuf::from("tmp/runtime"),
        );
        let first_key = workspace_key();
        let second_key = ProviderSessionCacheKey::WorkspaceCapability {
            capability: CapabilityName::try_new("implementer").unwrap(),
            target_artifacts: TargetArtifactSet::try_new(vec![
                TargetArtifactPath::try_new(PathBuf::from("tmp/other-briefing.md")).unwrap(),
            ])
            .unwrap(),
        };
        let track_key = track_key_for_target_set(
            TargetArtifactSet::try_new(vec![
                TargetArtifactPath::try_new(PathBuf::from("tmp/briefing.md")).unwrap(),
            ])
            .unwrap(),
        );
        let first_entry = entry();
        let second_entry = ProviderSessionCacheEntry::new(
            ProviderSessionId::try_new("session-2".to_owned()).unwrap(),
            ProviderName::try_new("claude").unwrap(),
            ModelName::try_new("sonnet").unwrap(),
            ReasoningEffort::Low,
        );
        let track_entry = ProviderSessionCacheEntry::new(
            ProviderSessionId::try_new("session-3".to_owned()).unwrap(),
            ProviderName::try_new("codex").unwrap(),
            ModelName::try_new("gpt-5-mini").unwrap(),
            ReasoningEffort::Medium,
        );

        cache.save(&first_key, &first_entry).unwrap();
        cache.save(&second_key, &second_entry).unwrap();
        cache.save(&track_key, &track_entry).unwrap();

        assert_eq!(cache.load(&first_key).unwrap(), Some(first_entry));
        assert_eq!(cache.load(&second_key).unwrap(), Some(second_entry));
        assert_eq!(cache.load(&track_key).unwrap(), Some(track_entry));

        let (first_path, first_identity) = cache.cache_path(&first_key).unwrap();
        let (second_path, second_identity) = cache.cache_path(&second_key).unwrap();
        let (track_path, track_identity) = cache.cache_path(&track_key).unwrap();
        assert_ne!(first_path, second_path, "workspace artifact identities need distinct files");
        assert_ne!(first_identity, second_identity);
        assert!(
            first_path.starts_with(directory.path().join("tmp/runtime/provider-sessions")),
            "workspace capability sessions must stay under the workspace transient path"
        );
        assert!(
            track_path.starts_with(directory.path().join("track/items/track-a/.provider-sessions")),
            "track capability sessions must stay inside that track's artifact directory"
        );
        assert_ne!(track_path.parent(), first_path.parent());
        assert_eq!(track_identity, "track-capability:track-a:implementer");
        assert!(first_identity.contains("workspace-capability:implementer:"));
        assert!(first_path.is_file());
        assert!(second_path.is_file());
        assert!(track_path.is_file());
    }

    #[test]
    fn test_provider_session_cache_review_keys_isolate_components_in_track_artifacts() {
        let directory = tempfile::tempdir().unwrap();
        let cache = FsProviderSessionCacheAdapter::new(
            directory.path().to_path_buf(),
            PathBuf::from("tmp/runtime"),
        );
        let infrastructure = ScopeName::Main(MainScopeName::new("infrastructure").unwrap());
        let base_key = review_key("track-a", infrastructure.clone(), RoundType::Fast, "a1b2c3d");
        let different_track =
            review_key("track-b", infrastructure.clone(), RoundType::Fast, "a1b2c3d");
        let different_scope = review_key("track-a", ScopeName::Other, RoundType::Fast, "a1b2c3d");
        let different_round =
            review_key("track-a", infrastructure.clone(), RoundType::Final, "a1b2c3d");
        let different_diff_base = review_key("track-a", infrastructure, RoundType::Fast, "d4e5f6a");
        let base_entry = entry_with_session_id("review-base");
        let track_entry = entry_with_session_id("review-track");
        let scope_entry = entry_with_session_id("review-scope");
        let round_entry = entry_with_session_id("review-round");
        let diff_base_entry = entry_with_session_id("review-diff-base");

        cache.save(&base_key, &base_entry).unwrap();
        cache.save(&different_track, &track_entry).unwrap();
        cache.save(&different_scope, &scope_entry).unwrap();
        cache.save(&different_round, &round_entry).unwrap();
        cache.save(&different_diff_base, &diff_base_entry).unwrap();

        assert_eq!(cache.load(&base_key).unwrap(), Some(base_entry));
        assert_eq!(cache.load(&different_track).unwrap(), Some(track_entry));
        assert_eq!(cache.load(&different_scope).unwrap(), Some(scope_entry));
        assert_eq!(cache.load(&different_round).unwrap(), Some(round_entry));
        assert_eq!(cache.load(&different_diff_base).unwrap(), Some(diff_base_entry));

        let base_path = cache.cache_path(&base_key).unwrap().0;
        let track_path = cache.cache_path(&different_track).unwrap().0;
        let scope_path = cache.cache_path(&different_scope).unwrap().0;
        let round_path = cache.cache_path(&different_round).unwrap().0;
        let diff_base_path = cache.cache_path(&different_diff_base).unwrap().0;
        assert_ne!(base_path, track_path);
        assert_ne!(base_path, scope_path);
        assert_ne!(base_path, round_path);
        assert_ne!(base_path, diff_base_path);
        assert!(
            base_path.starts_with(directory.path().join("track/items/track-a/.provider-sessions"))
        );
        assert!(
            track_path.starts_with(directory.path().join("track/items/track-b/.provider-sessions"))
        );
        assert!(base_path.is_file());
        assert!(track_path.is_file());
        assert!(scope_path.is_file());
        assert!(round_path.is_file());
        assert!(diff_base_path.is_file());
    }

    #[test]
    fn test_provider_session_cache_workspace_capabilities_isolate_same_target_artifacts() {
        let directory = tempfile::tempdir().unwrap();
        let cache = FsProviderSessionCacheAdapter::new(
            directory.path().to_path_buf(),
            PathBuf::from("tmp/runtime"),
        );
        let target_artifacts = TargetArtifactSet::try_new(vec![
            TargetArtifactPath::try_new(PathBuf::from("tmp/briefing.md")).unwrap(),
        ])
        .unwrap();
        let implementer_key = ProviderSessionCacheKey::WorkspaceCapability {
            capability: CapabilityName::try_new("implementer").unwrap(),
            target_artifacts: target_artifacts.clone(),
        };
        let planner_key = ProviderSessionCacheKey::WorkspaceCapability {
            capability: CapabilityName::try_new("impl-planner").unwrap(),
            target_artifacts,
        };
        let implementer_entry = entry_with_session_id("implementer-session");
        let planner_entry = entry_with_session_id("planner-session");

        cache.save(&implementer_key, &implementer_entry).unwrap();
        cache.save(&planner_key, &planner_entry).unwrap();

        assert_eq!(cache.load(&implementer_key).unwrap(), Some(implementer_entry));
        assert_eq!(cache.load(&planner_key).unwrap(), Some(planner_entry));
        let (implementer_path, implementer_identity) = cache.cache_path(&implementer_key).unwrap();
        let (planner_path, planner_identity) = cache.cache_path(&planner_key).unwrap();
        assert_ne!(implementer_path, planner_path);
        assert_ne!(implementer_identity, planner_identity);
        assert!(
            implementer_path.starts_with(directory.path().join("tmp/runtime/provider-sessions"))
        );
        assert!(planner_path.starts_with(directory.path().join("tmp/runtime/provider-sessions")));
        assert!(implementer_path.is_file());
        assert!(planner_path.is_file());
    }

    #[test]
    fn test_provider_session_cache_paths_are_gitignored_machine_local_transients() {
        let directory = tempfile::tempdir().unwrap();
        let cache = FsProviderSessionCacheAdapter::new(
            directory.path().to_path_buf(),
            PathBuf::from("tmp/runtime"),
        );
        let workspace_key = workspace_key();
        let track_key = track_key_for_target_set(
            TargetArtifactSet::try_new(vec![
                TargetArtifactPath::try_new(PathBuf::from("tmp/briefing.md")).unwrap(),
            ])
            .unwrap(),
        );

        cache.save(&workspace_key, &entry()).unwrap();
        cache.save(&track_key, &entry()).unwrap();
        let (workspace_path, _) = cache.cache_path(&workspace_key).unwrap();
        let (track_path, _) = cache.cache_path(&track_key).unwrap();
        let workspace_relative = workspace_path.strip_prefix(directory.path()).unwrap();
        let track_relative = track_path.strip_prefix(directory.path()).unwrap();
        let committed_gitignore = include_str!("../../../.gitignore");

        assert!(
            committed_gitignore.lines().any(|line| line == "tmp/"),
            "the existing runtime directory must stay gitignored"
        );
        assert!(
            committed_gitignore.lines().any(|line| line == "track/items/**/.provider-sessions/"),
            "track-local provider sessions must stay gitignored"
        );
        assert!(
            workspace_relative.starts_with(Path::new("tmp")),
            "workspace sessions must live under the existing runtime directory"
        );
        assert!(
            track_relative.starts_with(Path::new("track/items/track-a/.provider-sessions")),
            "track sessions must live under the gitignored track-local cache directory"
        );
    }

    #[test]
    fn test_provider_session_cache_isolates_review_track_and_workspace_identities() {
        let directory = tempfile::tempdir().unwrap();
        let cache = FsProviderSessionCacheAdapter::new(
            directory.path().to_path_buf(),
            PathBuf::from("tmp/runtime"),
        );
        let target_artifacts = TargetArtifactSet::try_new(vec![
            TargetArtifactPath::try_new(PathBuf::from("tmp/briefing.md")).unwrap(),
        ])
        .unwrap();
        let review_key = ProviderSessionCacheKey::Review {
            track_id: TrackId::try_new("track-a").unwrap(),
            scope: ScopeName::Main(MainScopeName::new("infrastructure").unwrap()),
            round_type: RoundType::Fast,
            diff_base: domain::CommitHash::try_new("a1b2c3d").unwrap(),
        };
        let track_key = track_key_for_target_set(target_artifacts.clone());
        let workspace_key = ProviderSessionCacheKey::WorkspaceCapability {
            capability: CapabilityName::try_new("implementer").unwrap(),
            target_artifacts: target_artifacts.clone(),
        };
        let other_workspace_key = ProviderSessionCacheKey::WorkspaceCapability {
            capability: CapabilityName::try_new("implementer").unwrap(),
            target_artifacts: TargetArtifactSet::try_new(vec![
                TargetArtifactPath::try_new(PathBuf::from("tmp/other-briefing.md")).unwrap(),
            ])
            .unwrap(),
        };
        let other_track_key = ProviderSessionCacheKey::TrackCapability {
            track_id: TrackId::try_new("track-b").unwrap(),
            capability: CapabilityName::try_new("implementer").unwrap(),
        };
        let other_capability_key = ProviderSessionCacheKey::TrackCapability {
            track_id: TrackId::try_new("track-a").unwrap(),
            capability: CapabilityName::try_new("spec-designer").unwrap(),
        };

        cache.save(&review_key, &entry()).unwrap();
        cache.save(&track_key, &entry()).unwrap();
        cache.save(&workspace_key, &entry()).unwrap();
        cache.save(&other_track_key, &entry()).unwrap();
        cache.save(&other_capability_key, &entry()).unwrap();
        let other_workspace_entry = ProviderSessionCacheEntry::new(
            ProviderSessionId::try_new("other-workspace-session".to_owned()).unwrap(),
            ProviderName::try_new("codex").unwrap(),
            ModelName::try_new("gpt-5-mini").unwrap(),
            ReasoningEffort::Low,
        );
        cache.save(&other_workspace_key, &other_workspace_entry).unwrap();

        let (review_path, review_identity) = cache.cache_path(&review_key).unwrap();
        let (track_path, track_identity) = cache.cache_path(&track_key).unwrap();
        let (workspace_path, workspace_identity) = cache.cache_path(&workspace_key).unwrap();
        let (other_workspace_path, other_workspace_identity) =
            cache.cache_path(&other_workspace_key).unwrap();
        let (other_track_path, other_track_identity) = cache.cache_path(&other_track_key).unwrap();
        let (other_capability_path, other_capability_identity) =
            cache.cache_path(&other_capability_key).unwrap();
        assert_ne!(review_path, track_path);
        assert_ne!(review_path, workspace_path);
        assert_ne!(track_path, workspace_path);
        assert_ne!(workspace_path, other_workspace_path);
        assert_ne!(track_path, other_track_path, "distinct track ids need distinct storage");
        assert_ne!(
            track_path, other_capability_path,
            "distinct capabilities need distinct storage within one track"
        );
        assert_ne!(review_identity, track_identity);
        assert_ne!(review_identity, workspace_identity);
        assert_ne!(track_identity, workspace_identity);
        assert_ne!(workspace_identity, other_workspace_identity);
        assert_ne!(track_identity, other_track_identity);
        assert_ne!(track_identity, other_capability_identity);
        assert!(review_identity.starts_with("review:track-a:infrastructure:fast"));
        assert_eq!(track_identity, "track-capability:track-a:implementer");
        assert_eq!(other_track_identity, "track-capability:track-b:implementer");
        assert_eq!(other_capability_identity, "track-capability:track-a:spec-designer");
        assert!(
            other_track_path.starts_with(directory.path().join("track/items/track-b")),
            "each track's entries live under that track's own artifact directory"
        );
        assert!(workspace_identity.starts_with("workspace-capability:implementer:"));
        assert_eq!(cache.load(&other_workspace_key).unwrap(), Some(other_workspace_entry));
        assert_eq!(
            cache.load(&review_key).unwrap(),
            Some(entry()),
            "a review-keyed session is retrievable for the same track, scope, and round type"
        );

        let changed_target_artifacts = TargetArtifactSet::try_new(vec![
            TargetArtifactPath::try_new(PathBuf::from("tmp/other-briefing.md")).unwrap(),
        ])
        .unwrap();
        assert_ne!(target_artifacts, changed_target_artifacts);
        let track_key_for_changed_target_set = track_key_for_target_set(changed_target_artifacts);
        assert_eq!(
            cache.load(&track_key_for_changed_target_set).unwrap(),
            Some(entry()),
            "one track and capability resolve to one cache entry even when target artifacts change"
        );
    }

    #[test]
    fn test_fs_provider_session_cache_adapter_implements_cache_port() {
        fn accepts_cache_port(_: &dyn ProviderSessionCachePort) {}

        let directory = tempfile::tempdir().unwrap();
        let cache = FsProviderSessionCacheAdapter::new(
            directory.path().to_path_buf(),
            PathBuf::from("tmp/runtime"),
        );
        accepts_cache_port(&cache);
    }

    #[test]
    fn test_provider_session_cache_missing_storage_returns_storage_unavailable() {
        let directory = tempfile::tempdir().unwrap();
        let cache = FsProviderSessionCacheAdapter::new(
            directory.path().join("missing-root"),
            PathBuf::from("tmp/runtime"),
        );

        let error = cache.load(&workspace_key()).unwrap_err();

        assert!(matches!(
            error,
            ProviderSessionCacheError::StorageUnavailable(detail)
                if detail.as_str().contains("filesystem operation failed")
        ));
    }

    #[test]
    fn test_provider_session_cache_corrupt_entry_returns_entry_invalid() {
        let directory = tempfile::tempdir().unwrap();
        let cache = FsProviderSessionCacheAdapter::new(
            directory.path().to_path_buf(),
            PathBuf::from("tmp/runtime"),
        );
        let key = workspace_key();
        cache.save(&key, &entry()).unwrap();
        let (path, _) = cache.cache_path(&key).unwrap();
        std::fs::write(path, b"not valid JSON").unwrap();

        let error = cache.load(&key).unwrap_err();

        assert!(matches!(
            error,
            ProviderSessionCacheError::EntryInvalid(detail)
                if detail.as_str().contains("JSON decode failed")
        ));
    }

    #[test]
    fn test_provider_session_cache_workspace_runtime_outside_repository_returns_identity_boundary_violation()
     {
        let repository = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let cache = FsProviderSessionCacheAdapter::new(
            repository.path().to_path_buf(),
            outside.path().to_path_buf(),
        );

        let error = cache.save(&workspace_key(), &entry()).unwrap_err();

        assert!(matches!(
            error,
            ProviderSessionCacheError::IdentityBoundaryViolation(detail)
                if detail.as_str().contains("escaped the repository root")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_provider_session_cache_symlinked_repository_root_returns_identity_boundary_violation() {
        let directory = tempfile::tempdir().unwrap();
        let real_root = directory.path().join("real-root");
        std::fs::create_dir(&real_root).unwrap();
        let symlink_root = directory.path().join("symlink-root");
        std::os::unix::fs::symlink(&real_root, &symlink_root).unwrap();
        let cache = FsProviderSessionCacheAdapter::new(symlink_root, PathBuf::from("tmp/runtime"));

        assert!(matches!(
            cache.canonical_repo_root(),
            Err(ProviderSessionCacheError::IdentityBoundaryViolation(detail))
                if detail.as_str().contains("non-symlink directory")
        ));
    }
}
