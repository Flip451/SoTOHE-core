//! Group policy resolution for per-group review scope.
//!
//! Loads `groups` from `track/review-scope.json` (base policy) with optional
//! per-track override from `track/items/<track-id>/review-groups.json`.
//! Derives policy_hash and group partition with mandatory `other` group.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use domain::{ReviewGroupName, TrackId};
use globset::{GlobBuilder, GlobMatcher};
use sha2::Digest;
use usecase::review_workflow::groups::{GroupPartition, GroupPartitionError};
use usecase::review_workflow::scope::RepoRelativePath;

// ---------------------------------------------------------------------------
// Config types (serde)
// ---------------------------------------------------------------------------

/// Group definition in a review scope policy.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewGroupConfig {
    /// Glob patterns for files belonging to this group.
    #[serde(default)]
    pub patterns: Vec<String>,
}

/// Optional per-track override config loaded from `review-groups.json`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewGroupsOverrideConfig {
    /// Named groups that completely replace the base policy groups.
    #[serde(default)]
    pub groups: BTreeMap<String, ReviewGroupConfig>,
}

/// File name for the per-track override.
pub const REVIEW_GROUPS_OVERRIDE_FILE: &str = "review-groups.json";

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors from group policy resolution.
#[derive(Debug, thiserror::Error)]
pub enum GroupPolicyError {
    #[error("I/O error for {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },

    #[error("JSON parse error for {path}: {source}")]
    Parse { path: PathBuf, source: serde_json::Error },

    #[error("invalid group name '{name}': {reason}")]
    InvalidGroupName { name: String, reason: String },

    #[error("reserved group name '{name}' must not be explicitly defined")]
    ReservedGroupName { name: String },

    #[error("group '{group}' has empty patterns list")]
    EmptyGroupPatterns { group: String },

    #[error("invalid glob pattern '{pattern}': {source}")]
    InvalidPattern { pattern: String, source: globset::Error },

    #[error("path '{path}' matches multiple groups: {groups:?}")]
    OverlappingGroups { path: String, groups: Vec<String> },

    #[error("group partition error: {0}")]
    Partition(#[from] GroupPartitionError),
}

// ---------------------------------------------------------------------------
// Override loader
// ---------------------------------------------------------------------------

/// Loads the optional per-track review-groups.json override.
///
/// Returns `Ok(None)` if the file does not exist.
///
/// # Errors
/// Returns `GroupPolicyError` on I/O or parse errors.
pub fn load_review_groups_override(
    items_dir: &Path,
    track_id: &TrackId,
) -> Result<Option<ReviewGroupsOverrideConfig>, GroupPolicyError> {
    let path = items_dir.join(track_id.as_ref()).join(REVIEW_GROUPS_OVERRIDE_FILE);
    // Reject symlinks on all path components below items_dir (fail-closed)
    crate::track::symlink_guard::reject_symlinks_below(&path, items_dir)
        .map_err(|source| GroupPolicyError::Io { path: path.clone(), source })?;
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let config: ReviewGroupsOverrideConfig = serde_json::from_str(&content)
                .map_err(|source| GroupPolicyError::Parse { path, source })?;
            Ok(Some(config))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(GroupPolicyError::Io { path, source }),
    }
}

// ---------------------------------------------------------------------------
// Compiled group matcher
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct CompiledGroup {
    name: ReviewGroupName,
    matchers: Vec<GlobMatcher>,
}

// ---------------------------------------------------------------------------
// ResolvedReviewGroupPolicy
// ---------------------------------------------------------------------------

/// A resolved and compiled group policy ready for partitioning.
#[derive(Debug, Clone)]
pub struct ResolvedReviewGroupPolicy {
    policy_hash: String,
    named_groups: Vec<CompiledGroup>,
}

impl ResolvedReviewGroupPolicy {
    /// Resolves and compiles a group policy from base config and optional override.
    ///
    /// If `override_config` is provided, its `groups` completely replace the base groups.
    /// The mandatory `other` group is always implicitly derived; explicitly defining
    /// `other` in either source is rejected.
    ///
    /// # Errors
    /// Returns `GroupPolicyError` on invalid group names, patterns, or reserved names.
    pub fn resolve(
        base_groups: &BTreeMap<String, ReviewGroupConfig>,
        override_config: Option<&ReviewGroupsOverrideConfig>,
    ) -> Result<Self, GroupPolicyError> {
        let effective = match override_config {
            Some(ov) => &ov.groups,
            None => base_groups,
        };

        // Normalize and validate named groups, building canonical map for hash
        let mut compiled = Vec::new();
        let mut canonical_groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (name, config) in effective {
            // Reject reserved name "other"
            if name.trim().eq_ignore_ascii_case("other") {
                return Err(GroupPolicyError::ReservedGroupName { name: name.clone() });
            }
            // Validate and normalize group name
            let group_name = ReviewGroupName::try_new(name).map_err(|e| {
                GroupPolicyError::InvalidGroupName { name: name.clone(), reason: e.to_string() }
            })?;
            // Detect normalization collisions (e.g., "foo" and " foo ")
            let canonical_key = group_name.to_string();
            if canonical_groups.contains_key(&canonical_key) {
                return Err(GroupPolicyError::InvalidGroupName {
                    name: name.clone(),
                    reason: format!(
                        "group name normalizes to '{canonical_key}' which already exists"
                    ),
                });
            }
            // Reject empty patterns
            if config.patterns.is_empty() {
                return Err(GroupPolicyError::EmptyGroupPatterns { group: name.clone() });
            }
            // Compile glob matchers
            let matchers = config
                .patterns
                .iter()
                .map(|pat| {
                    let glob = GlobBuilder::new(pat).literal_separator(false).build().map_err(
                        |source| GroupPolicyError::InvalidPattern { pattern: pat.clone(), source },
                    )?;
                    Ok::<GlobMatcher, GroupPolicyError>(glob.compile_matcher())
                })
                .collect::<Result<Vec<_>, _>>()?;

            canonical_groups.insert(canonical_key, config.patterns.clone());
            compiled.push(CompiledGroup { name: group_name, matchers });
        }

        // Compute policy hash from normalized canonical groups
        let hash_input = Self::build_canonical_hash_input(&canonical_groups);
        let digest = sha2::Sha256::digest(hash_input.as_bytes());
        let policy_hash = format!("sha256:{digest:x}");

        Ok(Self { policy_hash, named_groups: compiled })
    }

    /// Returns the policy hash for stale detection.
    #[must_use]
    pub fn policy_hash(&self) -> &str {
        &self.policy_hash
    }

    /// Partitions a list of changed paths into review groups.
    ///
    /// Each path is matched against all named groups. If a path matches no named
    /// group, it goes to `other`. If a path matches multiple named groups,
    /// the method fails with `OverlappingGroups`.
    ///
    /// # Errors
    /// Returns `GroupPolicyError::OverlappingGroups` on multi-group match.
    pub fn partition(
        &self,
        paths: &[RepoRelativePath],
    ) -> Result<GroupPartition, GroupPolicyError> {
        let other_key = ReviewGroupName::try_new("other").map_err(|e| {
            GroupPolicyError::InvalidGroupName { name: "other".into(), reason: e.to_string() }
        })?;

        let mut result: BTreeMap<ReviewGroupName, Vec<RepoRelativePath>> = BTreeMap::new();
        // Initialize all named groups + other
        for cg in &self.named_groups {
            result.entry(cg.name.clone()).or_default();
        }
        result.entry(other_key.clone()).or_default();

        for path in paths {
            let s = path.as_str();
            let mut matched_groups = Vec::new();

            for cg in &self.named_groups {
                if cg.matchers.iter().any(|m| m.is_match(s)) {
                    matched_groups.push(cg.name.clone());
                }
            }

            match matched_groups.as_slice() {
                [] => {
                    if let Some(other_vec) = result.get_mut(&other_key) {
                        other_vec.push(path.clone());
                    }
                }
                [single] => {
                    if let Some(group_vec) = result.get_mut(single) {
                        group_vec.push(path.clone());
                    }
                }
                _ => {
                    return Err(GroupPolicyError::OverlappingGroups {
                        path: s.to_owned(),
                        groups: matched_groups.iter().map(|g| g.to_string()).collect(),
                    });
                }
            }
        }

        Ok(GroupPartition::try_new(result)?)
    }

    /// Builds a deterministic, collision-free hash input from normalized group definitions.
    ///
    /// Uses JSON serialization with sorted keys to avoid ambiguity from
    /// delimiter-based concatenation (e.g., `["a,b"]` vs `["a","b"]`).
    /// Input keys are already normalized via `ReviewGroupName::try_new`.
    fn build_canonical_hash_input(groups: &BTreeMap<String, Vec<String>>) -> String {
        let canonical: BTreeMap<&str, Vec<&str>> = groups
            .iter()
            .map(|(name, patterns)| {
                let mut sorted: Vec<&str> = patterns.iter().map(String::as_str).collect();
                sorted.sort();
                (name.as_str(), sorted)
            })
            .collect();
        let envelope = serde_json::json!({
            "version": 1,
            "implicit_other": true,
            "groups": canonical,
        });
        // serde_json with BTreeMap produces sorted keys deterministically
        serde_json::to_string(&envelope).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn path(s: &str) -> RepoRelativePath {
        RepoRelativePath::normalize(s).unwrap()
    }

    fn grn(s: &str) -> ReviewGroupName {
        ReviewGroupName::try_new(s).unwrap()
    }

    fn base_groups() -> BTreeMap<String, ReviewGroupConfig> {
        let mut groups = BTreeMap::new();
        groups
            .insert("domain".into(), ReviewGroupConfig { patterns: vec!["libs/domain/**".into()] });
        groups.insert(
            "infrastructure".into(),
            ReviewGroupConfig { patterns: vec!["libs/infrastructure/**".into()] },
        );
        groups.insert("cli".into(), ReviewGroupConfig { patterns: vec!["apps/**".into()] });
        groups
    }

    // -----------------------------------------------------------------------
    // Parse / Resolve tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_base_groups() {
        let policy = ResolvedReviewGroupPolicy::resolve(&base_groups(), None).unwrap();
        assert!(!policy.policy_hash().is_empty());
    }

    #[test]
    fn test_resolve_empty_groups() {
        let groups = BTreeMap::new();
        let policy = ResolvedReviewGroupPolicy::resolve(&groups, None).unwrap();
        assert!(!policy.policy_hash().is_empty());
    }

    #[test]
    fn test_resolve_rejects_explicit_other() {
        let mut groups = base_groups();
        groups.insert("other".into(), ReviewGroupConfig { patterns: vec!["**".into()] });

        let result = ResolvedReviewGroupPolicy::resolve(&groups, None);
        assert!(
            matches!(result, Err(GroupPolicyError::ReservedGroupName { name }) if name == "other")
        );
    }

    #[test]
    fn test_resolve_rejects_other_case_insensitive() {
        let mut groups = BTreeMap::new();
        groups.insert("Other".into(), ReviewGroupConfig { patterns: vec!["**".into()] });

        let result = ResolvedReviewGroupPolicy::resolve(&groups, None);
        assert!(matches!(result, Err(GroupPolicyError::ReservedGroupName { .. })));
    }

    #[test]
    fn test_resolve_rejects_empty_patterns() {
        let mut groups = BTreeMap::new();
        groups.insert("domain".into(), ReviewGroupConfig { patterns: vec![] });

        let result = ResolvedReviewGroupPolicy::resolve(&groups, None);
        assert!(matches!(result, Err(GroupPolicyError::EmptyGroupPatterns { .. })));
    }

    #[test]
    fn test_resolve_override_replaces_base() {
        let override_config = ReviewGroupsOverrideConfig {
            groups: {
                let mut g = BTreeMap::new();
                g.insert(
                    "cli-only".into(),
                    ReviewGroupConfig { patterns: vec!["apps/cli/**".into()] },
                );
                g
            },
        };

        let policy =
            ResolvedReviewGroupPolicy::resolve(&base_groups(), Some(&override_config)).unwrap();
        // Partition with a domain file → goes to other (since override has only cli-only)
        let partition = policy.partition(&[path("libs/domain/src/lib.rs")]).unwrap();
        assert!(!partition.groups().contains_key(&grn("domain")));
        assert!(partition.groups().contains_key(&grn("cli-only")));
        assert_eq!(partition.groups()[&grn("other")].len(), 1);
    }

    // -----------------------------------------------------------------------
    // Policy hash tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_policy_hash_deterministic() {
        let p1 = ResolvedReviewGroupPolicy::resolve(&base_groups(), None).unwrap();
        let p2 = ResolvedReviewGroupPolicy::resolve(&base_groups(), None).unwrap();
        assert_eq!(p1.policy_hash(), p2.policy_hash());
    }

    #[test]
    fn test_policy_hash_changes_with_different_groups() {
        let p1 = ResolvedReviewGroupPolicy::resolve(&base_groups(), None).unwrap();

        let mut different = base_groups();
        different.insert(
            "usecase".into(),
            ReviewGroupConfig { patterns: vec!["libs/usecase/**".into()] },
        );
        let p2 = ResolvedReviewGroupPolicy::resolve(&different, None).unwrap();

        assert_ne!(p1.policy_hash(), p2.policy_hash());
    }

    #[test]
    fn test_policy_hash_changes_with_different_patterns() {
        let p1 = ResolvedReviewGroupPolicy::resolve(&base_groups(), None).unwrap();

        let mut different = base_groups();
        different.get_mut("domain").unwrap().patterns = vec!["libs/domain/src/**".into()];
        let p2 = ResolvedReviewGroupPolicy::resolve(&different, None).unwrap();

        assert_ne!(p1.policy_hash(), p2.policy_hash());
    }

    // -----------------------------------------------------------------------
    // Partition tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_partition_single_group_match() {
        let policy = ResolvedReviewGroupPolicy::resolve(&base_groups(), None).unwrap();
        let paths = vec![path("libs/domain/src/lib.rs")];
        let partition = policy.partition(&paths).unwrap();

        assert_eq!(partition.groups()[&grn("domain")].len(), 1);
        assert!(partition.groups()[&grn("other")].is_empty());
    }

    #[test]
    fn test_partition_unmatched_goes_to_other() {
        let policy = ResolvedReviewGroupPolicy::resolve(&base_groups(), None).unwrap();
        let paths = vec![path("Cargo.toml"), path("Makefile.toml")];
        let partition = policy.partition(&paths).unwrap();

        assert_eq!(partition.groups()[&grn("other")].len(), 2);
    }

    #[test]
    fn test_partition_other_always_present_even_if_empty() {
        let policy = ResolvedReviewGroupPolicy::resolve(&base_groups(), None).unwrap();
        let paths = vec![path("libs/domain/src/lib.rs")];
        let partition = policy.partition(&paths).unwrap();

        assert!(partition.groups().contains_key(&grn("other")));
        assert!(partition.groups()[&grn("other")].is_empty());
    }

    #[test]
    fn test_partition_multiple_groups() {
        let policy = ResolvedReviewGroupPolicy::resolve(&base_groups(), None).unwrap();
        let paths = vec![
            path("libs/domain/src/lib.rs"),
            path("libs/infrastructure/src/lib.rs"),
            path("apps/cli/src/main.rs"),
            path("Cargo.toml"),
        ];
        let partition = policy.partition(&paths).unwrap();

        assert_eq!(partition.groups()[&grn("domain")].len(), 1);
        assert_eq!(partition.groups()[&grn("infrastructure")].len(), 1);
        assert_eq!(partition.groups()[&grn("cli")].len(), 1);
        assert_eq!(partition.groups()[&grn("other")].len(), 1);
    }

    #[test]
    fn test_partition_overlap_detection() {
        let mut groups = BTreeMap::new();
        groups.insert("a".into(), ReviewGroupConfig { patterns: vec!["libs/**".into()] });
        groups.insert("b".into(), ReviewGroupConfig { patterns: vec!["libs/domain/**".into()] });

        let policy = ResolvedReviewGroupPolicy::resolve(&groups, None).unwrap();
        let paths = vec![path("libs/domain/src/lib.rs")];
        let result = policy.partition(&paths);

        assert!(matches!(result, Err(GroupPolicyError::OverlappingGroups { .. })));
    }

    #[test]
    fn test_partition_no_overlap_different_paths() {
        let mut groups = BTreeMap::new();
        groups.insert("a".into(), ReviewGroupConfig { patterns: vec!["libs/**".into()] });
        groups.insert("b".into(), ReviewGroupConfig { patterns: vec!["apps/**".into()] });

        let policy = ResolvedReviewGroupPolicy::resolve(&groups, None).unwrap();
        let paths = vec![path("libs/domain/src/lib.rs"), path("apps/cli/src/main.rs")];
        let partition = policy.partition(&paths).unwrap();

        assert_eq!(partition.groups()[&grn("a")].len(), 1);
        assert_eq!(partition.groups()[&grn("b")].len(), 1);
    }

    #[test]
    fn test_partition_empty_paths() {
        let policy = ResolvedReviewGroupPolicy::resolve(&base_groups(), None).unwrap();
        let partition = policy.partition(&[]).unwrap();

        // All groups present but empty
        assert!(partition.groups()[&grn("domain")].is_empty());
        assert!(partition.groups()[&grn("other")].is_empty());
    }

    // -----------------------------------------------------------------------
    // Override loading tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_override_absent_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        let result =
            load_review_groups_override(dir.path(), &TrackId::try_new("my-track").unwrap())
                .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_load_override_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("review-groups.json"),
            r#"{"groups": {"cli-only": {"patterns": ["apps/cli/**"]}}}"#,
        )
        .unwrap();

        let result =
            load_review_groups_override(dir.path(), &TrackId::try_new("my-track").unwrap())
                .unwrap();
        let config = result.unwrap();
        assert!(config.groups.contains_key("cli-only"));
    }

    #[test]
    fn test_load_override_invalid_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("review-groups.json"), "not json").unwrap();

        let result =
            load_review_groups_override(dir.path(), &TrackId::try_new("my-track").unwrap());
        assert!(matches!(result, Err(GroupPolicyError::Parse { .. })));
    }
}
