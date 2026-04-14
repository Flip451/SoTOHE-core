//! Verify that tech stack decisions are fully resolved.
//!
//! Rust port of `scripts/verify_tech_stack_ready.py`.

use std::path::{Path, PathBuf};

use domain::verify::{VerifyFinding, VerifyOutcome};
use regex::Regex;

const TECH_STACK_FILE: &str = "track/tech-stack.md";
const TEMPLATE_DEV_MARKER_FILE: &str = ".track-template-dev";
const TRACK_ITEMS_DIR: &str = "track/items";
const TRACK_ARCHIVE_DIR: &str = "track/archive";

/// Check whether the tech stack file has unresolved TODO markers.
///
/// # Errors
///
/// Returns findings when the tech stack file is missing, has unresolved TODOs
/// outside of valid bypass conditions (template-dev mode or planning phase).
pub fn verify(root: &Path) -> VerifyOutcome {
    let tech_stack = root.join(TECH_STACK_FILE);
    if !tech_stack.is_file() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "Missing tech stack file: {TECH_STACK_FILE}"
        ))]);
    }

    let template_dev = is_template_dev_mode(root);
    let tracks_present = has_track_dirs(root);

    if template_dev && !tracks_present {
        return VerifyOutcome::pass();
    }

    let content = match std::fs::read_to_string(&tech_stack) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot read {TECH_STACK_FILE}: {e}"
            ))]);
        }
    };

    let unresolved_re = match Regex::new(r"(?m)^\s*(?:-|\||理由:).*TODO:") {
        Ok(re) => re,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Internal regex error: {e}"
            ))]);
        }
    };
    let unresolved: Vec<&str> =
        content.lines().filter(|line| unresolved_re.is_match(line)).collect();

    if unresolved.is_empty() {
        return VerifyOutcome::pass();
    }

    // Planning-phase bypass: allow TODO when all tracks are still in planning.
    match all_tracks_planned(root) {
        Some(true) => return VerifyOutcome::pass(),
        None => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(
                "Cannot read track metadata; refusing to skip tech stack check".to_owned(),
            )]);
        }
        Some(false) => {}
    }

    let mut findings = vec![VerifyFinding::error(format!(
        "Unresolved tech stack TODOs found in {TECH_STACK_FILE}:"
    ))];
    for line in &unresolved {
        findings.push(VerifyFinding::error(format!("  {line}")));
    }
    VerifyOutcome::from_findings(findings)
}

fn is_template_dev_mode(root: &Path) -> bool {
    if std::env::var("TRACK_TEMPLATE_DEV").ok().is_some_and(|v| v.trim() == "1") {
        return true;
    }
    root.join(TEMPLATE_DEV_MARKER_FILE).is_file()
}

fn has_track_dirs(root: &Path) -> bool {
    !all_track_directories(root).is_empty()
}

fn all_track_directories(root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for base in [TRACK_ITEMS_DIR, TRACK_ARCHIVE_DIR] {
        let base_path = root.join(base);
        if let Ok(entries) = std::fs::read_dir(&base_path) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dirs.push(entry.path());
                }
            }
        }
    }
    dirs.sort();
    dirs
}

fn all_tracks_planned(root: &Path) -> Option<bool> {
    let dirs = all_track_directories(root);
    if dirs.is_empty() {
        return Some(false);
    }

    let archive_root = root.join("track").join("archive");
    let mut found_any = false;
    for track_dir in &dirs {
        // Skip archived tracks.
        if track_dir.starts_with(&archive_root) {
            continue;
        }
        let meta = track_dir.join("metadata.json");
        if !meta.is_file() {
            return None; // fail closed
        }
        let content = std::fs::read_to_string(&meta).ok()?;
        let data: serde_json::Value = serde_json::from_str(&content).ok()?;
        let obj = data.as_object()?;
        let status = obj.get("status")?.as_str()?;
        if status != "planned" {
            return Some(false);
        }
        found_any = true;
    }
    Some(found_any)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn setup_tech_stack(root: &Path, content: &str) {
        let dir = root.join("track");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(root.join(TECH_STACK_FILE), content).unwrap();
    }

    fn setup_track(root: &Path, name: &str, status: &str) {
        let dir = root.join(TRACK_ITEMS_DIR).join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let meta = serde_json::json!({
            "schema_version": 3,
            "id": name,
            "title": "Test",
            "status": status,
            "created_at": "2026-01-01T00:00:00+00:00",
            "updated_at": "2026-01-01T00:00:00+00:00",
            "branch": null,
            "tasks": [],
            "plan": { "summary": [], "sections": [] }
        });
        std::fs::write(dir.join("metadata.json"), meta.to_string()).unwrap();
    }

    #[test]
    fn test_missing_tech_stack_file_returns_error() {
        let tmp = TempDir::new().unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_clean_tech_stack_passes() {
        let tmp = TempDir::new().unwrap();
        setup_tech_stack(tmp.path(), "# Tech Stack\n- **DB**: PostgreSQL\n");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_unresolved_todo_fails_when_tracks_not_planned() {
        let tmp = TempDir::new().unwrap();
        setup_tech_stack(tmp.path(), "- **DB**: TODO: PostgreSQL / SQLite\n");
        setup_track(tmp.path(), "test-track", "in_progress");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_unresolved_todo_bypassed_in_planning_phase() {
        let tmp = TempDir::new().unwrap();
        setup_tech_stack(tmp.path(), "- **DB**: TODO: PostgreSQL / SQLite\n");
        setup_track(tmp.path(), "test-track", "planned");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_template_dev_mode_bypasses_when_no_tracks() {
        let tmp = TempDir::new().unwrap();
        setup_tech_stack(tmp.path(), "- **DB**: TODO: PostgreSQL / SQLite\n");
        std::fs::write(tmp.path().join(TEMPLATE_DEV_MARKER_FILE), "").unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_todo_in_non_structured_line_is_ignored() {
        let tmp = TempDir::new().unwrap();
        setup_tech_stack(
            tmp.path(),
            "# Tech Stack\nThis file mentions TODO: in prose but not in a structured line.\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_table_row_todo_is_detected() {
        let tmp = TempDir::new().unwrap();
        setup_tech_stack(tmp.path(), "| column | TODO: value |\n");
        setup_track(tmp.path(), "test-track", "in_progress");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }
}
