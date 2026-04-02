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
// Operational file exclusion
// ---------------------------------------------------------------------------

/// Result of loading `review-scope.json` in one pass.
#[derive(Debug)]
pub struct ReviewScopeConfig {
    /// Named group definitions (`groups` field).
    pub groups: BTreeMap<String, ReviewGroupConfig>,
    /// Compiled operational exclusion matchers (`review_operational` field,
    /// with `<track-id>` expanded).
    pub operational_matchers: Vec<GlobMatcher>,
}

/// Loads both `groups` and `review_operational` from `review-scope.json` in a
/// single file read, avoiding TOCTOU between the two data sets.
///
/// `<track-id>` placeholders in `review_operational` patterns are expanded
/// with the given track ID.
///
/// # Errors
///
/// Returns a string error on I/O, JSON parse, or glob compilation failure.
pub fn load_review_scope_config(
    review_scope_path: &Path,
    track_id: &TrackId,
) -> Result<ReviewScopeConfig, String> {
    let content = std::fs::read_to_string(review_scope_path)
        .map_err(|e| format!("read {}: {e}", review_scope_path.display()))?;
    let doc: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("parse {}: {e}", review_scope_path.display()))?;

    // Fail-closed: reject non-object top-level values.
    if !doc.is_object() {
        return Err(format!(
            "{}: top-level value must be a JSON object",
            review_scope_path.display()
        ));
    }

    // Parse groups (fail-closed: missing groups field is an error)
    let groups = match doc.get("groups") {
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| format!("parse groups in {}: {e}", review_scope_path.display()))?,
        None => {
            return Err(format!(
                "{}: missing required 'groups' field",
                review_scope_path.display()
            ));
        }
    };

    // Parse and compile operational matchers
    let operational_matchers = compile_operational_matchers(&doc, track_id)?;

    Ok(ReviewScopeConfig { groups, operational_matchers })
}

/// Compiles operational matchers from a pre-parsed JSON document.
fn compile_operational_matchers(
    doc: &serde_json::Value,
    track_id: &TrackId,
) -> Result<Vec<GlobMatcher>, String> {
    let arr = match doc.get("review_operational") {
        Some(v) => v.as_array().ok_or_else(|| "review_operational must be an array".to_owned())?,
        None => return Ok(Vec::new()),
    };
    let mut matchers = Vec::with_capacity(arr.len());
    for pat_val in arr {
        let raw = pat_val
            .as_str()
            .ok_or_else(|| "review_operational entries must be strings".to_owned())?;
        let expanded = raw.replace("<track-id>", track_id.as_ref());
        let glob = GlobBuilder::new(&expanded)
            .literal_separator(true)
            .build()
            .map_err(|e| format!("bad operational glob '{expanded}': {e}"))?;
        matchers.push(glob.compile_matcher());
    }
    Ok(matchers)
}

/// Filters out paths that match any operational glob matcher.
///
/// Returns a new vec with only non-operational paths.
#[must_use]
pub fn filter_operational(
    paths: &[RepoRelativePath],
    matchers: &[GlobMatcher],
) -> Vec<RepoRelativePath> {
    if matchers.is_empty() {
        return paths.to_vec();
    }
    paths.iter().filter(|p| !matchers.iter().any(|m| m.is_match(p.as_str()))).cloned().collect()
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

    // -----------------------------------------------------------------------
    // Operational exclusion tests (T001/T002)
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_operational_matchers_expands_track_id() {
        let dir = tempfile::tempdir().unwrap();
        let scope_file = dir.path().join("review-scope.json");
        std::fs::write(
            &scope_file,
            r#"{"groups": {}, "review_operational": ["track/items/<track-id>/review.json"]}"#,
        )
        .unwrap();

        let track_id = TrackId::try_new("my-track-2026-04-01").unwrap();
        let matchers =
            load_review_scope_config(&scope_file, &track_id).unwrap().operational_matchers;

        assert_eq!(matchers.len(), 1);
        assert!(matchers[0].is_match("track/items/my-track-2026-04-01/review.json"));
        assert!(!matchers[0].is_match("track/items/other-track/review.json"));
    }

    #[test]
    fn test_load_operational_matchers_missing_field_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let scope_file = dir.path().join("review-scope.json");
        std::fs::write(&scope_file, r#"{"groups": {}}"#).unwrap();

        let track_id = TrackId::try_new("my-track").unwrap();
        let matchers =
            load_review_scope_config(&scope_file, &track_id).unwrap().operational_matchers;

        assert!(matchers.is_empty());
    }

    #[test]
    fn test_filter_operational_removes_matching_paths() {
        let dir = tempfile::tempdir().unwrap();
        let scope_file = dir.path().join("review-scope.json");
        std::fs::write(
            &scope_file,
            r#"{"groups": {}, "review_operational": ["track/items/<track-id>/review.json"]}"#,
        )
        .unwrap();

        let track_id = TrackId::try_new("my-track").unwrap();
        let matchers =
            load_review_scope_config(&scope_file, &track_id).unwrap().operational_matchers;

        let paths = vec![
            path("libs/domain/src/lib.rs"),
            path("track/items/my-track/review.json"),
            path("track/items/my-track/metadata.json"),
        ];
        let filtered = filter_operational(&paths, &matchers);

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|p| p.as_str() == "libs/domain/src/lib.rs"));
        assert!(filtered.iter().any(|p| p.as_str() == "track/items/my-track/metadata.json"));
        assert!(!filtered.iter().any(|p| p.as_str().contains("review.json")));
    }

    #[test]
    fn test_filter_operational_no_matchers_returns_all() {
        let paths = vec![path("a.rs"), path("b.rs")];
        let filtered = filter_operational(&paths, &[]);

        assert_eq!(filtered.len(), 2);
    }

    // Negative-path tests for load_review_scope_config

    #[test]
    fn test_load_review_scope_config_non_array_operational_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let scope_file = dir.path().join("review-scope.json");
        std::fs::write(&scope_file, r#"{"groups": {}, "review_operational": "not-an-array"}"#)
            .unwrap();

        let track_id = TrackId::try_new("my-track").unwrap();
        let result = load_review_scope_config(&scope_file, &track_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be an array"));
    }

    #[test]
    fn test_load_review_scope_config_non_string_entry_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let scope_file = dir.path().join("review-scope.json");
        std::fs::write(&scope_file, r#"{"groups": {}, "review_operational": [123]}"#).unwrap();

        let track_id = TrackId::try_new("my-track").unwrap();
        let result = load_review_scope_config(&scope_file, &track_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be strings"));
    }

    #[test]
    fn test_load_review_scope_config_invalid_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let scope_file = dir.path().join("review-scope.json");
        std::fs::write(&scope_file, "not json").unwrap();

        let track_id = TrackId::try_new("my-track").unwrap();
        let result = load_review_scope_config(&scope_file, &track_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_review_scope_config_non_object_top_level_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let scope_file = dir.path().join("review-scope.json");
        std::fs::write(&scope_file, "[]").unwrap();

        let track_id = TrackId::try_new("my-track").unwrap();
        let result = load_review_scope_config(&scope_file, &track_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be a JSON object"));
    }

    #[test]
    fn test_load_review_scope_config_missing_groups_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let scope_file = dir.path().join("review-scope.json");
        std::fs::write(&scope_file, r#"{"review_operational": []}"#).unwrap();

        let track_id = TrackId::try_new("my-track").unwrap();
        let result = load_review_scope_config(&scope_file, &track_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing required 'groups'"));
    }
}
