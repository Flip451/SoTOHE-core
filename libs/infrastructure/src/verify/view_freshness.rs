//! Verify that rendered views (plan.md) are up-to-date with metadata.json.
//!
//! Re-renders plan.md from metadata.json and compares with the on-disk file.
//! If they differ, the view is stale and CI should fail.

use std::path::Path;

use domain::verify::{Finding, VerifyOutcome};

const TRACK_ITEMS_DIR: &str = "track/items";

/// Check that all active track plan.md files match their metadata.json renderings.
///
/// # Errors
///
/// Returns findings when plan.md content differs from the expected rendering.
pub fn verify(root: &Path) -> VerifyOutcome {
    let items_dir = root.join(TRACK_ITEMS_DIR);
    if !items_dir.is_dir() {
        return VerifyOutcome::pass();
    }

    let mut findings = Vec::new();

    let entries = match std::fs::read_dir(&items_dir) {
        Ok(e) => e,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "Cannot read {TRACK_ITEMS_DIR}: {e}"
            ))]);
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                findings.push(Finding::error(format!("{TRACK_ITEMS_DIR}: cannot read entry: {e}")));
                continue;
            }
        };
        let track_dir = entry.path();
        if !track_dir.is_dir() {
            continue;
        }

        let metadata_path = track_dir.join("metadata.json");
        let plan_path = track_dir.join("plan.md");

        if !metadata_path.is_file() {
            continue;
        }

        let track_name =
            track_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

        // plan.md must exist if metadata.json exists (it's a generated view)
        if !plan_path.is_file() {
            findings.push(Finding::error(format!(
                "{track_name}/plan.md: missing — run `cargo make track-sync-views` to generate"
            )));
            continue;
        }

        // Read the on-disk plan.md
        let on_disk = match std::fs::read_to_string(&plan_path) {
            Ok(c) => c,
            Err(e) => {
                findings.push(Finding::error(format!("{track_name}/plan.md: cannot read: {e}")));
                continue;
            }
        };

        // Render plan.md from metadata.json
        let rendered = match render_plan_from_metadata(&metadata_path) {
            Ok(c) => c,
            Err(e) => {
                findings.push(Finding::error(format!(
                    "{track_name}/plan.md: cannot render from metadata.json: {e}"
                )));
                continue;
            }
        };

        // Normalize trailing whitespace for comparison (files may have trailing newline)
        if on_disk.trim_end() != rendered.trim_end() {
            findings.push(Finding::error(format!(
                "{track_name}/plan.md: stale — run `cargo make track-sync-views` to update"
            )));
        }
    }

    VerifyOutcome::from_findings(findings)
}

/// Render plan.md content from metadata.json using the infrastructure render module.
fn render_plan_from_metadata(metadata_path: &Path) -> Result<String, String> {
    let content = std::fs::read_to_string(metadata_path).map_err(|e| format!("read error: {e}"))?;

    let (track, _meta) =
        crate::track::codec::decode(&content).map_err(|e| format!("decode error: {e}"))?;

    Ok(crate::track::render::render_plan(&track))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn setup_track_with_plan(root: &Path, name: &str, plan_content: &str) {
        let track_dir = root.join(TRACK_ITEMS_DIR).join(name);
        std::fs::create_dir_all(&track_dir).unwrap();

        let metadata = serde_json::json!({
            "schema_version": 3,
            "id": name,
            "branch": format!("track/{name}"),
            "title": "Test Track",
            "status": "in_progress",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "tasks": [
                {"id": "T001", "description": "Test task", "status": "todo"}
            ],
            "plan": {
                "summary": ["Test summary"],
                "sections": [
                    {"id": "S1", "title": "Section 1", "task_ids": ["T001"], "description": ["Do the thing"]}
                ]
            }
        });
        std::fs::write(track_dir.join("metadata.json"), metadata.to_string()).unwrap();
        std::fs::write(track_dir.join("plan.md"), plan_content).unwrap();
    }

    #[test]
    fn test_view_freshness_passes_when_up_to_date() {
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join(TRACK_ITEMS_DIR).join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        let metadata = serde_json::json!({
            "schema_version": 3,
            "id": "test-track",
            "branch": "track/test-track",
            "title": "Test Track",
            "status": "in_progress",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "tasks": [
                {"id": "T001", "description": "Test task", "status": "todo"}
            ],
            "plan": {
                "summary": ["Test summary"],
                "sections": [
                    {"id": "S1", "title": "Section 1", "task_ids": ["T001"], "description": ["Do the thing"]}
                ]
            }
        });
        std::fs::write(track_dir.join("metadata.json"), metadata.to_string()).unwrap();

        // Render the expected plan.md
        let rendered = render_plan_from_metadata(&track_dir.join("metadata.json")).unwrap();
        std::fs::write(track_dir.join("plan.md"), &rendered).unwrap();

        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_view_freshness_fails_when_stale() {
        let tmp = TempDir::new().unwrap();
        setup_track_with_plan(tmp.path(), "stale-track", "# Stale content\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(outcome.findings()[0].to_string().contains("stale"));
    }

    #[test]
    fn test_view_freshness_passes_with_no_tracks() {
        let tmp = TempDir::new().unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_view_freshness_errors_when_plan_md_missing() {
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join(TRACK_ITEMS_DIR).join("no-plan");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata = serde_json::json!({
            "schema_version": 3,
            "id": "no-plan",
            "branch": "track/no-plan",
            "title": "No Plan",
            "status": "planned",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "tasks": [],
            "plan": {"summary": [], "sections": []}
        });
        std::fs::write(track_dir.join("metadata.json"), metadata.to_string()).unwrap();
        // No plan.md — should report as missing (fail-closed)
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(outcome.findings()[0].to_string().contains("missing"));
    }
}
