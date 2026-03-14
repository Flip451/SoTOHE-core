//! Pure workflow rules for guarded git operations.
//!
//! `tmp/track-commit/*` is the primary scratch contract. Legacy `.takt/pending-*`
//! entries remain here only until the remaining takt wrappers are removed.

use std::path::PathBuf;

pub const TRANSIENT_AUTOMATION_FILES: &[&str] = &[
    ".takt/pending-add-paths.txt",
    ".takt/pending-note.md",
    ".takt/pending-commit-message.txt",
    "tmp/track-commit/add-paths.txt",
    "tmp/track-commit/commit-message.txt",
    "tmp/track-commit/note.md",
    "tmp/track-commit/track-dir.txt",
];

pub const TRANSIENT_AUTOMATION_DIRS: &[&str] = &[".takt/handoffs", "tmp"];

const GLOB_MAGIC_CHARS: &[char] = &['*', '?', '[', ']'];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitTrackBranch {
    pub display_path: String,
    pub expected_branch: Option<String>,
    pub status: Option<String>,
    pub schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackBranchClaim {
    pub track_name: String,
    pub branch: Option<String>,
    pub status: Option<String>,
    pub schema_version: u32,
}

pub fn validate_stage_path_entries<'a, I>(entries: I) -> Result<Vec<String>, String>
where
    I: IntoIterator<Item = &'a str>,
{
    let transient_paths: Vec<PathBuf> =
        TRANSIENT_AUTOMATION_FILES.iter().map(PathBuf::from).collect();
    let transient_dirs: Vec<PathBuf> =
        TRANSIENT_AUTOMATION_DIRS.iter().map(PathBuf::from).collect();
    let mut stage_paths = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    for raw_line in entries {
        let entry = raw_line.trim();
        if entry.is_empty() || entry.starts_with('#') || !seen.insert(entry.to_owned()) {
            continue;
        }

        let entry_path = PathBuf::from(entry);
        if entry_path.is_absolute() {
            return Err(format!("Stage path list must use repo-relative paths: {entry}"));
        }
        if entry_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(format!("Stage path list cannot escape the repo root: {entry}"));
        }
        if matches!(entry, "." | "./") {
            return Err(format!("Stage path list cannot use whole-worktree pathspecs: {entry}"));
        }
        if entry.starts_with(':') {
            return Err(format!(
                "Stage path list cannot use git pathspec magic or shorthand: {entry}"
            ));
        }
        if entry.chars().any(|ch| GLOB_MAGIC_CHARS.contains(&ch)) {
            return Err(format!("Stage path list cannot use glob patterns: {entry}"));
        }
        if transient_paths
            .iter()
            .any(|transient| entry_path == *transient || transient.starts_with(&entry_path))
        {
            return Err(format!(
                "Stage path list cannot include transient automation files or their parent directories: {entry}"
            ));
        }
        if transient_dirs.iter().any(|transient_dir| {
            entry_path == *transient_dir
                || entry_path.starts_with(transient_dir)
                || transient_dir.starts_with(&entry_path)
        }) {
            return Err(format!(
                "Stage path list cannot include transient automation directories or their contents: {entry}"
            ));
        }

        stage_paths.push(entry.to_owned());
    }

    if stage_paths.is_empty() {
        return Err("Stage path list file has no usable entries".to_owned());
    }

    Ok(stage_paths)
}

pub fn verify_explicit_track_branch(
    current_branch: Option<&str>,
    explicit_track: &ExplicitTrackBranch,
) -> Result<(), String> {
    if explicit_track.expected_branch.is_none()
        && explicit_track.schema_version == 3
        && explicit_track.status.as_deref() == Some("planned")
    {
        return match current_branch {
            None => Err(
                "Cannot determine current git branch — planning-only commits require a non-track branch with an explicit selector"
                    .to_owned(),
            ),
            Some("HEAD") => Err(
                "Detached HEAD — planning-only commits require a non-track branch with an explicit selector"
                    .to_owned(),
            ),
            Some(branch) if branch.starts_with("track/") => Err(format!(
                "Current branch '{branch}' is a track branch; planning-only commits require a non-track branch with an explicit selector"
            )),
            Some(_) => Ok(()),
        };
    }

    let Some(expected_branch) = explicit_track.expected_branch.as_deref() else {
        return Ok(());
    };

    match current_branch {
        None => Err(format!("Cannot determine current git branch — expected '{expected_branch}'")),
        Some("HEAD") => {
            Err(format!("Detached HEAD — expected branch '{expected_branch}', cannot verify"))
        }
        Some(branch) if branch != expected_branch => {
            Err(format!("Current branch '{branch}' does not match expected '{expected_branch}'"))
        }
        Some(_) => Ok(()),
    }
}

pub fn validate_planning_only_commit_paths(
    explicit_track: &ExplicitTrackBranch,
    staged_paths: &[String],
) -> Result<(), String> {
    if explicit_track.schema_version != 3
        || explicit_track.expected_branch.is_some()
        || explicit_track.status.as_deref() != Some("planned")
    {
        return Ok(());
    }

    let track_prefix = format!("{}/", explicit_track.display_path);
    for path in staged_paths {
        if path == &explicit_track.display_path
            || path.starts_with(&track_prefix)
            || matches!(
                path.as_str(),
                "track/registry.md" | "track/tech-stack.md" | ".claude/docs/DESIGN.md"
            )
        {
            continue;
        }

        return Err(format!(
            "planning-only commit for '{}' may not stage '{}'; run /track:activate <track-id> before committing implementation files",
            explicit_track.display_path, path
        ));
    }

    Ok(())
}

pub fn verify_auto_detected_branch(
    current_branch: Option<&str>,
    claims: &[TrackBranchClaim],
) -> Result<(), String> {
    let branch = match current_branch {
        Some(branch) => branch,
        None => return Err("cannot determine current git branch".to_owned()),
    };
    if branch == "HEAD" {
        return Err("detached HEAD — cannot verify track branch".to_owned());
    }
    if !branch.starts_with("track/") {
        return Ok(());
    }

    let matches = claims
        .iter()
        .filter(|claim| {
            claim.branch.as_deref() == Some(branch) && claim.status.as_deref() != Some("archived")
        })
        .collect::<Vec<_>>();

    if matches.is_empty() {
        let archived_match = claims.iter().any(|claim| {
            claim.branch.as_deref() == Some(branch) && claim.status.as_deref() == Some("archived")
        });
        if archived_match {
            return Ok(());
        }

        let slug = branch.trim_start_matches("track/");
        let fallback_match = claims.iter().any(|claim| {
            claim.track_name == slug
                && claim.branch.is_none()
                && claim.schema_version != 3
                && claim.status.as_deref() != Some("archived")
        });
        if fallback_match {
            return Ok(());
        }

        return Err(format!(
            "on branch '{branch}' but no track claims this branch in metadata.json"
        ));
    }

    if matches.len() > 1 {
        let names =
            matches.iter().map(|claim| claim.track_name.clone()).collect::<Vec<_>>().join(", ");
        return Err(format!("multiple tracks claim branch '{branch}': {names}"));
    }

    match matches.first() {
        Some(claim) => verify_explicit_track_branch(
            Some(branch),
            &ExplicitTrackBranch {
                display_path: claim.track_name.clone(),
                expected_branch: claim.branch.clone(),
                status: claim.status.clone(),
                schema_version: claim.schema_version,
            },
        ),
        None => Err("internal error: expected exactly one branch match".to_owned()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{
        ExplicitTrackBranch, TrackBranchClaim, validate_planning_only_commit_paths,
        validate_stage_path_entries, verify_auto_detected_branch, verify_explicit_track_branch,
    };

    #[test]
    fn validate_stage_path_entries_accepts_unique_repo_relative_paths() {
        let paths =
            validate_stage_path_entries(["src/lib.rs", "# comment", "src/lib.rs", "README.md"])
                .unwrap();

        assert_eq!(paths, vec!["src/lib.rs".to_owned(), "README.md".to_owned()]);
    }

    #[test]
    fn validate_stage_path_entries_rejects_transient_parent_directory() {
        let err = validate_stage_path_entries(["tmp/track-commit"]).unwrap_err();

        assert!(err.contains("transient automation"));
    }

    #[test]
    fn verify_explicit_track_branch_rejects_mismatch() {
        let err = verify_explicit_track_branch(
            Some("track/other"),
            &ExplicitTrackBranch {
                display_path: "track/items/example".to_owned(),
                expected_branch: Some("track/example".to_owned()),
                status: Some("in_progress".to_owned()),
                schema_version: 3,
            },
        )
        .unwrap_err();

        assert!(err.contains("does not match expected"));
    }

    #[test]
    fn verify_explicit_track_branch_rejects_planning_only_selector_on_track_branch() {
        let err = verify_explicit_track_branch(
            Some("track/other"),
            &ExplicitTrackBranch {
                display_path: "track/items/example".to_owned(),
                expected_branch: None,
                status: Some("planned".to_owned()),
                schema_version: 3,
            },
        )
        .unwrap_err();

        assert!(err.contains("non-track branch"));
    }

    #[test]
    fn verify_explicit_track_branch_rejects_planning_only_selector_on_detached_head() {
        let err = verify_explicit_track_branch(
            Some("HEAD"),
            &ExplicitTrackBranch {
                display_path: "track/items/example".to_owned(),
                expected_branch: None,
                status: Some("planned".to_owned()),
                schema_version: 3,
            },
        )
        .unwrap_err();

        assert!(err.contains("Detached HEAD"));
    }

    #[test]
    fn verify_auto_detected_branch_accepts_legacy_null_branch_fallback() {
        let claims = vec![TrackBranchClaim {
            track_name: "example".to_owned(),
            branch: None,
            status: Some("in_progress".to_owned()),
            schema_version: 2,
        }];

        assert!(verify_auto_detected_branch(Some("track/example"), &claims).is_ok());
    }

    #[test]
    fn verify_auto_detected_branch_rejects_archived_null_branch_fallback() {
        let claims = vec![TrackBranchClaim {
            track_name: "example".to_owned(),
            branch: None,
            status: Some("archived".to_owned()),
            schema_version: 2,
        }];

        let err = verify_auto_detected_branch(Some("track/example"), &claims).unwrap_err();

        assert!(err.contains("no track claims this branch"));
    }

    #[test]
    fn verify_auto_detected_branch_rejects_planned_null_branch_fallback() {
        let claims = vec![TrackBranchClaim {
            track_name: "example".to_owned(),
            branch: None,
            status: Some("planned".to_owned()),
            schema_version: 3,
        }];

        let err = verify_auto_detected_branch(Some("track/example"), &claims).unwrap_err();

        assert!(err.contains("no track claims this branch"));
    }

    #[test]
    fn verify_auto_detected_branch_accepts_legacy_planned_null_branch_fallback() {
        let claims = vec![TrackBranchClaim {
            track_name: "example".to_owned(),
            branch: None,
            status: Some("planned".to_owned()),
            schema_version: 2,
        }];

        assert!(verify_auto_detected_branch(Some("track/example"), &claims).is_ok());
    }

    #[test]
    fn verify_auto_detected_branch_rejects_duplicate_claims() {
        let claims = vec![
            TrackBranchClaim {
                track_name: "one".to_owned(),
                branch: Some("track/example".to_owned()),
                status: Some("in_progress".to_owned()),
                schema_version: 3,
            },
            TrackBranchClaim {
                track_name: "two".to_owned(),
                branch: Some("track/example".to_owned()),
                status: Some("in_progress".to_owned()),
                schema_version: 3,
            },
        ];

        let err = verify_auto_detected_branch(Some("track/example"), &claims).unwrap_err();

        assert!(err.contains("multiple tracks claim branch"));
    }

    #[test]
    fn validate_planning_only_commit_paths_rejects_non_artifact_files() {
        let err = validate_planning_only_commit_paths(
            &ExplicitTrackBranch {
                display_path: "track/items/example".to_owned(),
                expected_branch: None,
                status: Some("planned".to_owned()),
                schema_version: 3,
            },
            &["src/lib.rs".to_owned()],
        )
        .unwrap_err();

        assert!(err.contains("run /track:activate"));
    }

    #[test]
    fn validate_planning_only_commit_paths_allows_planning_artifacts() {
        let result = validate_planning_only_commit_paths(
            &ExplicitTrackBranch {
                display_path: "track/items/example".to_owned(),
                expected_branch: None,
                status: Some("planned".to_owned()),
                schema_version: 3,
            },
            &[
                "track/items/example/spec.md".to_owned(),
                "track/registry.md".to_owned(),
                "track/tech-stack.md".to_owned(),
                ".claude/docs/DESIGN.md".to_owned(),
            ],
        );

        assert!(result.is_ok());
    }

    #[test]
    fn validate_planning_only_commit_paths_ignores_legacy_v2_branchless_track() {
        let result = validate_planning_only_commit_paths(
            &ExplicitTrackBranch {
                display_path: "track/items/example".to_owned(),
                expected_branch: None,
                status: Some("planned".to_owned()),
                schema_version: 2,
            },
            &["src/lib.rs".to_owned()],
        );

        assert!(result.is_ok());
    }
}
