//! Verify that rendered views (plan.md) are up-to-date with metadata.json.
//!
//! Re-renders plan.md from metadata.json and compares with the on-disk file.
//! If they differ, the view is stale and CI should fail.

use std::path::Path;

use domain::verify::{VerifyFinding, VerifyOutcome};

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
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot read {TRACK_ITEMS_DIR}: {e}"
            ))]);
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                findings.push(VerifyFinding::error(format!(
                    "{TRACK_ITEMS_DIR}: cannot read entry: {e}"
                )));
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

        // Skip v2/v3 legacy tracks: they predate the current renderer and
        // their committed plan.md reflects the renderer that shipped at
        // their commit time. We only validate v5 (identity-only, current
        // schema) metadata so that sibling tracks stay untouched by the
        // active track's work.
        //
        // Only skip when schema_version is an explicit integer < 5.
        // When the field is missing, non-numeric, or the file cannot be read
        // or parsed, fall through so that the subsequent render attempt
        // surfaces the real error (fail-closed: do not silently bypass
        // freshness verification for a corrupted v5 file).
        if let Ok(content) = std::fs::read_to_string(&metadata_path) {
            if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(schema_version) =
                    raw.get("schema_version").and_then(serde_json::Value::as_u64)
                {
                    if schema_version < 5 {
                        continue;
                    }
                }
                // schema_version absent or non-numeric: fall through to render
                // (fail-closed — do not silently treat ambiguous files as legacy).
            }
        }

        // plan.md absent: silent SKIP (file existence = phase status).
        // Phase 0/1/2 tracks (pre-implementation) may not yet have plan.md
        // rendered; treat the absence as "not yet rendered" rather than as a
        // freshness failure. This matches `validate_track_snapshots` in
        // `libs/infrastructure/src/track/render.rs:621-624`.
        //
        // Use symlink_metadata to distinguish genuine absence (NotFound →
        // silent skip) from other filesystem states (permission error, etc.
        // → surface as a finding), mirroring the absent-vs-corrupted split
        // in render.rs:621-624. Symlinks are followed via `metadata` to
        // detect dangling symlinks and non-file symlink targets (treated as
        // corrupted track state).
        let sym_meta = match std::fs::symlink_metadata(&plan_path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue, // Genuine absence.
            Err(e) => {
                findings
                    .push(VerifyFinding::error(format!("{track_name}/plan.md: cannot stat: {e}")));
                continue;
            }
        };
        if sym_meta.file_type().is_symlink() {
            match std::fs::metadata(&plan_path) {
                Ok(target) if target.is_file() => {} // Valid symlink to a regular file — proceed.
                Ok(_) => {
                    findings.push(VerifyFinding::error(format!(
                        "{track_name}/plan.md: symlink target is not a regular file (corrupted track state)"
                    )));
                    continue;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    findings.push(VerifyFinding::error(format!(
                        "{track_name}/plan.md: dangling symlink (target missing)"
                    )));
                    continue;
                }
                Err(e) => {
                    findings.push(VerifyFinding::error(format!(
                        "{track_name}/plan.md: cannot follow symlink: {e}"
                    )));
                    continue;
                }
            }
        } else if !sym_meta.is_file() {
            findings.push(VerifyFinding::error(format!(
                "{track_name}/plan.md: not a regular file (corrupted track state)"
            )));
            continue;
        }

        // Read the on-disk plan.md
        let on_disk = match std::fs::read_to_string(&plan_path) {
            Ok(c) => c,
            Err(e) => {
                findings
                    .push(VerifyFinding::error(format!("{track_name}/plan.md: cannot read: {e}")));
                continue;
            }
        };

        // Render plan.md from metadata.json
        let rendered = match render_plan_from_metadata(&metadata_path) {
            Ok(c) => c,
            Err(e) => {
                findings.push(VerifyFinding::error(format!(
                    "{track_name}/plan.md: cannot render from metadata.json: {e}"
                )));
                continue;
            }
        };

        // Normalize trailing whitespace for comparison (files may have trailing newline)
        if on_disk.trim_end() != rendered.trim_end() {
            findings.push(VerifyFinding::error(format!(
                "{track_name}/plan.md: stale — run `cargo make track-sync-views` to update"
            )));
        }
    }

    VerifyOutcome::from_findings(findings)
}

/// Render plan.md content from metadata.json (+ optional impl-plan.json) using the
/// infrastructure render module.
///
/// Only renders v4/v5 (identity-only) metadata. Callers are expected to
/// pre-filter by schema_version; v2/v3 metadata is not supported and will
/// return a decode error.
fn render_plan_from_metadata(metadata_path: &Path) -> Result<String, String> {
    let content = std::fs::read_to_string(metadata_path).map_err(|e| format!("read error: {e}"))?;

    let (track, _meta) =
        crate::track::codec::decode(&content).map_err(|e| format!("decode error: {e}"))?;

    // Load sibling impl-plan.json when present.
    let impl_plan = if let Some(parent) = metadata_path.parent() {
        let impl_plan_path = parent.join("impl-plan.json");
        if impl_plan_path.is_file() {
            let json = std::fs::read_to_string(&impl_plan_path)
                .map_err(|e| format!("impl-plan.json read error: {e}"))?;
            let doc = crate::impl_plan_codec::decode(&json)
                .map_err(|e| format!("impl-plan.json decode error: {e}"))?;
            Some(doc)
        } else {
            None
        }
    } else {
        None
    };

    Ok(crate::track::render::render_plan(&track, impl_plan.as_ref()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn setup_track_with_plan(root: &Path, name: &str, plan_content: &str) {
        let track_dir = root.join(TRACK_ITEMS_DIR).join(name);
        std::fs::create_dir_all(&track_dir).unwrap();

        // v5 identity-only metadata — legacy v2/v3/v4 tracks are skipped by the
        // freshness validator on purpose.
        let metadata = serde_json::json!({
            "schema_version": 5,
            "id": name,
            "branch": format!("track/{name}"),
            "title": "Test Track",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
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
            "schema_version": 5,
            "id": "test-track",
            "branch": "track/test-track",
            "title": "Test Track",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
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
    fn test_view_freshness_skips_when_plan_md_absent() {
        // Phase 0/1/2 tracks may not yet have plan.md rendered. The freshness
        // validator must silently SKIP these tracks, matching the behavior of
        // `validate_track_snapshots` in `libs/infrastructure/src/track/render.rs`.
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join(TRACK_ITEMS_DIR).join("no-plan");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata = serde_json::json!({
            "schema_version": 5,
            "id": "no-plan",
            "branch": "track/no-plan",
            "title": "No Plan",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
        });
        std::fs::write(track_dir.join("metadata.json"), metadata.to_string()).unwrap();
        // No plan.md — should be SKIPPED silently (not a freshness failure).
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "plan.md absent must be a silent skip, not a finding");
    }

    #[test]
    fn test_view_freshness_passes_when_plan_md_absent() {
        // Companion test: minimum Phase 0 fixture (only metadata.json) must
        // PASS. Asserts the explicit positive contract — `verify` returns a
        // pass outcome with no findings — separate from the rename above.
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join(TRACK_ITEMS_DIR).join("phase0-bare");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata = serde_json::json!({
            "schema_version": 5,
            "id": "phase0-bare",
            "branch": "track/phase0-bare",
            "title": "Phase 0 Bare",
            "created_at": "2026-04-27T00:00:00Z",
            "updated_at": "2026-04-27T00:00:00Z",
        });
        std::fs::write(track_dir.join("metadata.json"), metadata.to_string()).unwrap();

        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_view_freshness_skips_v3_legacy_tracks() {
        // v2/v3 legacy tracks must be skipped by the freshness validator so
        // that sibling tracks stay untouched by the active track's work.
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join(TRACK_ITEMS_DIR).join("legacy-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata = serde_json::json!({
            "schema_version": 3,
            "id": "legacy-track",
            "branch": "track/legacy-track",
            "title": "Legacy Track",
            "status": "done",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "tasks": [{"id": "T001", "description": "task", "status": "done"}],
            "plan": {"summary": [], "sections": []}
        });
        std::fs::write(track_dir.join("metadata.json"), metadata.to_string()).unwrap();
        // Intentionally stale plan.md — should be skipped, not flagged.
        std::fs::write(track_dir.join("plan.md"), "# whatever\n").unwrap();

        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "v3 legacy tracks must be skipped silently");
    }

    #[test]
    fn test_view_freshness_does_not_skip_track_with_missing_schema_version() {
        // A metadata.json without a numeric schema_version must NOT be silently
        // skipped as if it were a legacy track. The freshness validator should
        // fall through and surface the error (fail-closed).
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join(TRACK_ITEMS_DIR).join("ambiguous-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        // metadata.json without schema_version field — could be a corrupted v5 file.
        let metadata = serde_json::json!({
            "id": "ambiguous-track",
            "branch": "track/ambiguous-track",
            "title": "Ambiguous Track",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
        });
        std::fs::write(track_dir.join("metadata.json"), metadata.to_string()).unwrap();
        // Write a stale plan.md — should NOT be silently accepted.
        std::fs::write(track_dir.join("plan.md"), "# whatever\n").unwrap();

        let outcome = verify(tmp.path());
        // The track should not be silently skipped; the render attempt will fail
        // because schema_version is required by the codec. Expect either a
        // "cannot render" error or a "stale" finding — either way, not pass.
        assert!(
            outcome.has_errors(),
            "track with missing schema_version must not be silently skipped: {:#?}",
            outcome.findings()
        );
    }
}
