//! Loads `.harness/config/review-scope.json` into the v2 `ReviewScopeConfig`.
//!
//! v2 ignores `planning_only` and `normalize` fields from the JSON file.
//! Only `groups`, `review_operational`, and `other_track` are consumed.

use std::path::Path;

use domain::TrackId;
use domain::review_v2::{FilePath, ReviewScopeConfig, ScopeConfigError};

/// Errors from loading review-scope.json for v2.
#[derive(Debug, thiserror::Error)]
pub enum ScopeConfigLoadError {
    #[error("I/O error reading {path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("JSON parse error in {path}: {source}")]
    Parse { path: String, source: serde_json::Error },
    #[error("{path}: {detail}")]
    InvalidField { path: String, detail: String },
    #[error("scope config error: {0}")]
    Config(#[from] ScopeConfigError),
}

/// Serde helper for a scope group entry.
/// Top-level review-scope.json structure.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewScopeJsonV2 {
    version: u64,
    groups: std::collections::BTreeMap<String, GroupEntry>,
    #[serde(default)]
    review_operational: Vec<String>,
    #[serde(default)]
    other_track: Vec<String>,
    /// Global default per-scope diff ceiling (lines), used by
    /// `ReviewScopeConfig::diff_ceiling_for_scope` when a scope has no
    /// per-group override. `None` (field absent) means no global default —
    /// scopes without per-group `diff_ceiling_lines` return None
    /// (unconstrained). Introduced by the feature-batch default-inversion
    /// track (D3 / IN-05 / CN-02).
    #[serde(default)]
    default_diff_ceiling_lines: Option<u32>,
    // planning_only and normalize are v1 fields — rejected by deny_unknown_fields
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupEntry {
    patterns: Vec<String>,
    /// Optional workspace-relative path to a scope-specific briefing file.
    ///
    /// When present, the CLI briefing composer appends a reference line so
    /// the reviewer fetches the file via its Read tool (ADR 2026-04-18-1354
    /// §D4). When absent, `#[serde(default)]` resolves it to `None`, which
    /// preserves backward compatibility with review-scope.json files written
    /// before this field was introduced.
    #[serde(default)]
    briefing_file: Option<String>,
    /// Per-group override for the diff ceiling (lines). `None` (field absent)
    /// means inherit the global `default_diff_ceiling_lines`. Used by the
    /// full-cycle orchestrator to compute feature-batch split points (D3 /
    /// IN-04 / IN-05).
    #[serde(default)]
    diff_ceiling_lines: Option<u32>,
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    matches!(
        (bytes.first(), bytes.get(1)),
        (Some(first), Some(b':')) if first.is_ascii_alphabetic()
    )
}

/// Loads `review-scope.json` into a v2 `ReviewScopeConfig`.
///
/// Extracts `groups`, `review_operational`, and `other_track` fields.
/// `planning_only` and `normalize` are ignored (v2 drops these features).
///
/// # Errors
/// Returns `ScopeConfigLoadError` on I/O, parse, or config validation failure.
pub fn load_v2_scope_config(
    review_scope_path: &Path,
    track_id: &TrackId,
    trusted_root: &Path,
) -> Result<ReviewScopeConfig, ScopeConfigLoadError> {
    let path_display = review_scope_path.display().to_string();

    // Defense-in-depth: two complementary path safety checks.
    //
    // Layer 1 (canonicalize): resolves symlinks and checks the *resolved* path is under
    // trusted_root. Catches path escape even through symlink chains.
    //
    // Layer 2 (reject_symlinks_below): rejects symlinks in the *original* path. This is
    // strictly more restrictive — a valid path passes both, but a symlink-based attack
    // fails at layer 2 even if layer 1's resolved path appears safe.
    //
    // TOCTOU note: a race exists between the symlink check (layer 2) and read_to_string
    // below. An attacker could replace the file with a symlink between the two calls.
    // This is an accepted risk for a single-user developer tool where the trusted_root
    // is the local workspace. The two-layer check raises the bar significantly for
    // any file-based attack.
    use crate::track::symlink_guard::reject_symlinks_below;
    let canonical_root = trusted_root.canonicalize().map_err(|source| {
        ScopeConfigLoadError::Io { path: trusted_root.display().to_string(), source }
    })?;
    let canonical_path = review_scope_path
        .canonicalize()
        .map_err(|source| ScopeConfigLoadError::Io { path: path_display.clone(), source })?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(ScopeConfigLoadError::InvalidField {
            path: path_display,
            detail: format!("path escapes trusted root ({})", canonical_root.display()),
        });
    }

    // Layer 2: reject symlinks in the original (non-canonicalized) path.
    reject_symlinks_below(review_scope_path, trusted_root).map_err(|source| {
        if source.kind() == std::io::ErrorKind::InvalidInput {
            ScopeConfigLoadError::InvalidField {
                path: path_display.clone(),
                detail: "symlink detected in review-scope.json path (rejected for security)"
                    .to_owned(),
            }
        } else {
            ScopeConfigLoadError::Io { path: path_display.clone(), source }
        }
    })?;

    let content = std::fs::read_to_string(review_scope_path)
        .map_err(|source| ScopeConfigLoadError::Io { path: path_display.clone(), source })?;

    // Typed deserialization with deny_unknown_fields — rejects typos and v1 fields
    let doc: ReviewScopeJsonV2 = serde_json::from_str(&content)
        .map_err(|source| ScopeConfigLoadError::Parse { path: path_display.clone(), source })?;

    if doc.version != 2 {
        return Err(ScopeConfigLoadError::InvalidField {
            path: path_display,
            detail: format!(
                "review-scope.json version {} is not supported (expected 2)",
                doc.version
            ),
        });
    }

    // Validate briefing_file paths: each configured briefing_file must be a
    // repo-relative, traversal-free, non-symlink path under the trusted root.
    // This protects against an attacker
    // committing `track/review-prompts/policy.md -> /etc/passwd` alongside a
    // review-scope.json change, which would otherwise smuggle workspace-external
    // file reads into the reviewer's Read-tool call (threat model: PR author is
    // the attacker; ADR 2026-04-18-1354 §D4 originally assumed the reviewer
    // sandbox's `read-only` mode would block this but that is not guaranteed).
    // Follows knowledge/conventions/security.md §Symlink rejection.
    for (name, entry) in &doc.groups {
        if let Some(ref briefing) = entry.briefing_file {
            FilePath::new(briefing.as_str()).map_err(|source| {
                ScopeConfigLoadError::InvalidField {
                    path: path_display.clone(),
                    detail: format!(
                        "invalid briefing_file for group '{name}': '{briefing}' ({source})"
                    ),
                }
            })?;
            if has_windows_drive_prefix(briefing) {
                return Err(ScopeConfigLoadError::InvalidField {
                    path: path_display.clone(),
                    detail: format!(
                        "invalid briefing_file for group '{name}': '{briefing}' \
                         (Windows drive prefixes are not repo-relative)"
                    ),
                });
            }
            let briefing_path = trusted_root.join(briefing);
            if !briefing_path.starts_with(trusted_root) {
                return Err(ScopeConfigLoadError::InvalidField {
                    path: path_display.clone(),
                    detail: format!(
                        "briefing_file for group '{name}' escapes trusted root: '{briefing}'"
                    ),
                });
            }
            reject_symlinks_below(&briefing_path, trusted_root).map_err(|source| {
                if source.kind() == std::io::ErrorKind::InvalidInput {
                    ScopeConfigLoadError::InvalidField {
                        path: path_display.clone(),
                        detail: format!(
                            "symlink detected in briefing_file for group '{name}': '{briefing}' \
                             (rejected for security)"
                        ),
                    }
                } else {
                    ScopeConfigLoadError::Io { path: path_display.clone(), source }
                }
            })?;
        }
    }

    #[allow(clippy::type_complexity)] // matches the ReviewScopeConfig::new entries seam.
    let entries: Vec<(String, Vec<String>, Option<String>, Option<u32>)> = doc
        .groups
        .into_iter()
        .map(|(name, entry)| (name, entry.patterns, entry.briefing_file, entry.diff_ceiling_lines))
        .collect();

    Ok(ReviewScopeConfig::new(
        track_id,
        entries,
        doc.review_operational,
        doc.other_track,
        doc.default_diff_ceiling_lines,
    )?)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn write_scope_json(dir: &Path, content: &str) -> std::path::PathBuf {
        let path = dir.join("review-scope.json");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_load_minimal_scope_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "groups": {
                    "domain": { "patterns": ["libs/domain/**"] }
                }
            }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        let config = load_v2_scope_config(&path, &track_id, dir.path()).unwrap();
        assert!(config.contains_scope(&domain::review_v2::ScopeName::Other));
    }

    #[test]
    fn test_load_with_operational_and_other_track() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "groups": {
                    "domain": { "patterns": ["libs/domain/**"] },
                    "cli": { "patterns": ["apps/**"] }
                },
                "review_operational": ["track/items/<track-id>/review.json"],
                "other_track": ["track/items/<other-track>/**"]
            }"#,
        );
        let track_id = TrackId::try_new("my-track").unwrap();
        let config = load_v2_scope_config(&path, &track_id, dir.path()).unwrap();

        // Operational files are excluded
        let review_json =
            domain::review_v2::FilePath::new("track/items/my-track/review.json").unwrap();
        let classified = config.classify(&[review_json]);
        assert!(classified.is_empty(), "operational file should be excluded");
    }

    #[test]
    fn test_planning_only_and_normalize_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "groups": {
                    "domain": { "patterns": ["libs/domain/**"] }
                },
                "planning_only": ["docs/**"],
                "normalize": { "metadata.json": {} }
            }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        // v2 rejects unknown fields (planning_only, normalize are v1)
        let err = load_v2_scope_config(&path, &track_id, dir.path()).unwrap_err();
        assert!(matches!(err, ScopeConfigLoadError::Parse { .. }));
    }

    #[test]
    fn test_missing_groups_returns_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(dir.path(), r#"{ "version": 2 }"#);
        let track_id = TrackId::try_new("test-track").unwrap();
        let err = load_v2_scope_config(&path, &track_id, dir.path()).unwrap_err();
        assert!(matches!(err, ScopeConfigLoadError::Parse { .. }));
    }

    #[test]
    fn test_not_found_returns_io_error() {
        let track_id = TrackId::try_new("test-track").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let err = load_v2_scope_config(Path::new("/nonexistent/path.json"), &track_id, dir.path())
            .unwrap_err();
        assert!(matches!(err, ScopeConfigLoadError::Io { .. }));
    }

    #[test]
    fn test_other_track_excludes_non_current_track() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "groups": {
                    "domain": { "patterns": ["libs/domain/**"] }
                },
                "other_track": ["track/items/<other-track>/**", "track/archive/**"]
            }"#,
        );
        let track_id = TrackId::try_new("my-track").unwrap();
        let config = load_v2_scope_config(&path, &track_id, dir.path()).unwrap();

        // Other track files excluded
        let other_file =
            domain::review_v2::FilePath::new("track/items/other-track/spec.md").unwrap();
        let classified = config.classify(&[other_file]);
        assert!(classified.is_empty(), "other track file should be excluded");

        // Current track files NOT excluded (goes to Other scope)
        let current_file =
            domain::review_v2::FilePath::new("track/items/my-track/spec.md").unwrap();
        let classified = config.classify(&[current_file]);
        assert!(!classified.is_empty(), "current track file should NOT be excluded");
    }

    #[test]
    fn test_multi_scope_classification() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "groups": {
                    "domain": { "patterns": ["libs/domain/**"] },
                    "usecase": { "patterns": ["libs/usecase/**"] },
                    "cli": { "patterns": ["apps/**"] }
                }
            }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        let config = load_v2_scope_config(&path, &track_id, dir.path()).unwrap();

        let files = vec![
            domain::review_v2::FilePath::new("libs/domain/src/lib.rs").unwrap(),
            domain::review_v2::FilePath::new("libs/usecase/src/lib.rs").unwrap(),
            domain::review_v2::FilePath::new("apps/cli/src/main.rs").unwrap(),
            domain::review_v2::FilePath::new("README.md").unwrap(),
        ];
        let classified = config.classify(&files);

        // 3 named scopes + 1 other
        assert_eq!(classified.len(), 4);
        assert!(classified.contains_key(&domain::review_v2::ScopeName::Other));
    }

    #[test]
    fn test_version_not_2_returns_invalid_field_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 1,
                "groups": {
                    "domain": { "patterns": ["libs/domain/**"] }
                }
            }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        let err = load_v2_scope_config(&path, &track_id, dir.path()).unwrap_err();
        assert!(
            matches!(err, ScopeConfigLoadError::InvalidField { .. }),
            "expected InvalidField for version != 2, got: {err}"
        );
    }

    #[test]
    fn test_path_escape_outside_trusted_root_returns_error() {
        // Create two separate temp dirs: one acts as trusted_root, the other holds the
        // scope file. Since the file is under a different canonicalized path, the
        // starts_with check must reject it.
        let root_dir = tempfile::tempdir().unwrap();
        let outside_dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            outside_dir.path(),
            r#"{ "version": 2, "groups": { "domain": { "patterns": ["libs/domain/**"] } } }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        let err = load_v2_scope_config(&path, &track_id, root_dir.path()).unwrap_err();
        assert!(
            matches!(err, ScopeConfigLoadError::InvalidField { .. }),
            "expected InvalidField for path outside trusted root, got: {err}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_symlink_in_path_returns_error() {
        // Create a real file under the trusted root, then a symlink to it.
        // The symlink itself is inside the trusted root, but reject_symlinks_below
        // must refuse it as a defense-in-depth measure.
        let dir = tempfile::tempdir().unwrap();
        let real_file = dir.path().join("real-review-scope.json");
        std::fs::write(
            &real_file,
            r#"{ "version": 2, "groups": { "domain": { "patterns": ["libs/domain/**"] } } }"#,
        )
        .unwrap();
        let symlink_path = dir.path().join("review-scope.json");
        std::os::unix::fs::symlink(&real_file, &symlink_path).unwrap();

        let track_id = TrackId::try_new("test-track").unwrap();
        let err = load_v2_scope_config(&symlink_path, &track_id, dir.path()).unwrap_err();
        assert!(
            matches!(err, ScopeConfigLoadError::InvalidField { .. }),
            "expected InvalidField when symlink is detected, got: {err}"
        );
    }

    // ── T002: GroupEntry.briefing_file serde field ────────────────────

    #[test]
    fn test_load_with_briefing_file_populates_accessor() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "groups": {
                    "impl-plan": {
                        "patterns": ["track/items/**"],
                        "briefing_file": ".harness/custom/review-prompts/impl-plan.md"
                    }
                }
            }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        let config = load_v2_scope_config(&path, &track_id, dir.path()).unwrap();

        let scope = domain::review_v2::ScopeName::Main(
            domain::review_v2::MainScopeName::new("impl-plan").unwrap(),
        );
        assert_eq!(
            config.briefing_file_for_scope(&scope),
            Some(".harness/custom/review-prompts/impl-plan.md")
        );
    }

    #[test]
    fn test_load_without_briefing_file_is_backward_compatible() {
        // A review-scope.json that predates the briefing_file field must continue to
        // load; briefing_file_for_scope returns None because #[serde(default)] fills
        // the missing field with None.
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "groups": {
                    "domain": { "patterns": ["libs/domain/**"] }
                }
            }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        let config = load_v2_scope_config(&path, &track_id, dir.path()).unwrap();

        let scope = domain::review_v2::ScopeName::Main(
            domain::review_v2::MainScopeName::new("domain").unwrap(),
        );
        assert!(config.briefing_file_for_scope(&scope).is_none());
    }

    #[test]
    fn test_typo_in_briefing_file_field_is_rejected() {
        // deny_unknown_fields regression guard: a misspelled field name like
        // `briefng_file` must not silently be ignored.
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "groups": {
                    "impl-plan": {
                        "patterns": ["track/items/**"],
                        "briefng_file": ".harness/custom/review-prompts/impl-plan.md"
                    }
                }
            }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        let err = load_v2_scope_config(&path, &track_id, dir.path()).unwrap_err();
        assert!(
            matches!(err, ScopeConfigLoadError::Parse { .. }),
            "expected Parse error for unknown field, got: {err}"
        );
    }

    #[test]
    fn test_load_rejects_absolute_briefing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "groups": {
                    "impl-plan": {
                        "patterns": ["track/items/**"],
                        "briefing_file": "/etc/passwd"
                    }
                }
            }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        let err = load_v2_scope_config(&path, &track_id, dir.path()).unwrap_err();
        assert!(
            matches!(
                &err,
                ScopeConfigLoadError::InvalidField { detail, .. }
                    if detail.contains("briefing_file") && detail.contains("repo-relative")
            ),
            "expected InvalidField for absolute briefing_file, got: {err}"
        );
    }

    #[test]
    fn test_load_rejects_windows_drive_relative_briefing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "groups": {
                    "impl-plan": {
                        "patterns": ["track/items/**"],
                        "briefing_file": "C:review.md"
                    }
                }
            }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        let err = load_v2_scope_config(&path, &track_id, dir.path()).unwrap_err();
        assert!(
            matches!(
                &err,
                ScopeConfigLoadError::InvalidField { detail, .. }
                    if detail.contains("briefing_file") && detail.contains("Windows drive")
            ),
            "expected InvalidField for drive-relative briefing_file, got: {err}"
        );
    }

    #[test]
    fn test_load_rejects_traversal_briefing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "groups": {
                    "impl-plan": {
                        "patterns": ["track/items/**"],
                        "briefing_file": "../outside.md"
                    }
                }
            }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        let err = load_v2_scope_config(&path, &track_id, dir.path()).unwrap_err();
        assert!(
            matches!(
                &err,
                ScopeConfigLoadError::InvalidField { detail, .. }
                    if detail.contains("briefing_file") && detail.contains("traversal")
            ),
            "expected InvalidField for traversal briefing_file, got: {err}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_load_rejects_symlink_briefing_file() {
        // Attack model: PR author commits review-scope.json with a briefing_file
        // path that is a symlink to a workspace-external secret
        // (e.g. .harness/custom/review-prompts/policy.md -> /etc/passwd). The loader must
        // reject the scope config at load time so the CLI never gets a path that
        // the reviewer's Read tool could follow outside the workspace.
        let dir = tempfile::tempdir().unwrap();
        // Create an in-repo symlink whose target is outside the trusted root.
        let briefing_dir = dir.path().join(".harness/custom/review-prompts");
        std::fs::create_dir_all(&briefing_dir).unwrap();
        let symlink_path = briefing_dir.join("policy.md");
        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("secret.md");
        std::fs::write(&outside_file, "secret").unwrap();
        std::os::unix::fs::symlink(&outside_file, &symlink_path).unwrap();

        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "groups": {
                    "impl-plan": {
                        "patterns": ["track/items/**"],
                        "briefing_file": ".harness/custom/review-prompts/policy.md"
                    }
                }
            }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        let err = load_v2_scope_config(&path, &track_id, dir.path()).unwrap_err();
        assert!(
            matches!(
                &err,
                ScopeConfigLoadError::InvalidField { detail, .. }
                    if detail.contains("briefing_file") && detail.contains("symlink")
            ),
            "expected InvalidField with briefing_file symlink detail, got: {err}"
        );
    }

    // ── feature-batch ceiling fields (D3 / IN-04 / IN-05 / CN-02) ──────

    #[test]
    fn test_load_per_scope_diff_ceiling_overrides_default() {
        // A group with `diff_ceiling_lines` set must surface its override via
        // diff_ceiling_for_scope, regardless of the global default.
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "default_diff_ceiling_lines": 500,
                "groups": {
                    "domain": { "patterns": ["libs/domain/**"], "diff_ceiling_lines": 200 }
                }
            }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        let config = load_v2_scope_config(&path, &track_id, dir.path()).unwrap();
        let domain = domain::review_v2::ScopeName::Main(
            domain::review_v2::MainScopeName::new("domain").unwrap(),
        );
        assert_eq!(config.diff_ceiling_for_scope(&domain), Some(200));
    }

    #[test]
    fn test_load_default_diff_ceiling_applies_to_overrideless_scope() {
        // A group without `diff_ceiling_lines` must inherit the top-level
        // `default_diff_ceiling_lines`.
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "default_diff_ceiling_lines": 500,
                "groups": {
                    "domain": { "patterns": ["libs/domain/**"] }
                }
            }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        let config = load_v2_scope_config(&path, &track_id, dir.path()).unwrap();
        let domain = domain::review_v2::ScopeName::Main(
            domain::review_v2::MainScopeName::new("domain").unwrap(),
        );
        assert_eq!(config.diff_ceiling_for_scope(&domain), Some(500));
    }

    #[test]
    fn test_load_without_ceiling_fields_is_backward_compatible() {
        // A review-scope.json that predates both ceiling fields must load,
        // and diff_ceiling_for_scope returns None for every scope (since both
        // serde defaults resolve to None).
        let dir = tempfile::tempdir().unwrap();
        let path = write_scope_json(
            dir.path(),
            r#"{
                "version": 2,
                "groups": {
                    "domain": { "patterns": ["libs/domain/**"] }
                }
            }"#,
        );
        let track_id = TrackId::try_new("test-track").unwrap();
        let config = load_v2_scope_config(&path, &track_id, dir.path()).unwrap();
        let domain = domain::review_v2::ScopeName::Main(
            domain::review_v2::MainScopeName::new("domain").unwrap(),
        );
        assert!(config.diff_ceiling_for_scope(&domain).is_none());
    }
}
