//! Verify that rendered views (plan.md) are up-to-date with metadata.json.
//!
//! Re-renders plan.md from metadata.json and compares with the on-disk file.
//! If they differ, the view is stale and CI should fail.

use std::path::Path;

use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::track::symlink_guard::reject_symlinks_below;

const TRACK_ITEMS_DIR: &str = "track/items";

/// Check that all active track plan.md files match their metadata.json renderings.
///
/// # Errors
///
/// Returns findings when plan.md content differs from the expected rendering.
pub fn verify(root: &Path) -> VerifyOutcome {
    let items_dir = root.join(TRACK_ITEMS_DIR);
    let items_meta = match items_dir.symlink_metadata() {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return VerifyOutcome::pass(),
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot stat {TRACK_ITEMS_DIR}: {e}"
            ))]);
        }
    };
    if items_meta.file_type().is_symlink() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{TRACK_ITEMS_DIR}: symlinked track root is unsafe"
        ))]);
    }
    if !items_meta.is_dir() {
        return VerifyOutcome::pass();
    }

    let entries = match std::fs::read_dir(&items_dir) {
        Ok(e) => e,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot read {TRACK_ITEMS_DIR}: {e}"
            ))]);
        }
    };

    let mut findings = Vec::new();

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
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(e) => {
                findings.push(VerifyFinding::error(format!(
                    "{TRACK_ITEMS_DIR}: cannot inspect entry: {e}"
                )));
                continue;
            }
        };
        if file_type.is_symlink() {
            findings.push(VerifyFinding::error(format!(
                "{}: symlinked track directory is unsafe",
                track_dir.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
            )));
            continue;
        }
        if !file_type.is_dir() {
            continue;
        }

        let metadata_path = track_dir.join("metadata.json");
        let plan_path = track_dir.join("plan.md");

        let track_name =
            track_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

        let metadata_present =
            match guarded_file_present(&metadata_path, &track_dir, &track_name, "metadata.json") {
                Ok(present) => present,
                Err(finding) => {
                    findings.push(finding);
                    continue;
                }
            };
        if !metadata_present {
            continue;
        }

        // Skip v2/v3/v4/v5 legacy tracks: they predate the current renderer and
        // their committed plan.md reflects the renderer that shipped at
        // their commit time. We only validate v6 (identity-only with
        // branch_strategy_snapshot, current schema) metadata so that sibling
        // tracks stay untouched by the active track's work.
        //
        // Only skip when schema_version is an explicit integer < 6.
        // When the field is missing, non-numeric, or the file cannot be read
        // or parsed, fall through so that the subsequent render attempt
        // surfaces the real error (fail-closed: do not silently bypass
        // freshness verification for a corrupted v6 file).
        if let Ok(content) = std::fs::read_to_string(&metadata_path) {
            if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(schema_version) =
                    raw.get("schema_version").and_then(serde_json::Value::as_u64)
                {
                    if schema_version < 6 {
                        continue;
                    }
                }
                // schema_version absent or non-numeric: fall through to render
                // (fail-closed — do not silently treat ambiguous files as legacy).
            }
        }

        // plan.md absent: silent SKIP (file existence = phase status).
        // Phase 0/1/2 tracks may not yet have plan.md rendered; treat absence
        // as "not yet rendered", but reject symlinks and non-file entries.
        let plan_present =
            match guarded_file_present(&plan_path, &track_dir, &track_name, "plan.md") {
                Ok(present) => present,
                Err(finding) => {
                    findings.push(finding);
                    continue;
                }
            };
        if !plan_present {
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
    let parent = metadata_path.parent().ok_or_else(|| "metadata.json has no parent".to_owned())?;
    match reject_symlinks_below(metadata_path, parent) {
        Ok(true) => {}
        Ok(false) => return Err("metadata.json is missing".to_owned()),
        Err(e) => return Err(format!("metadata.json symlink guard: {e}")),
    }
    if !metadata_path.is_file() {
        return Err("metadata.json exists but is not a regular file".to_owned());
    }
    let content = std::fs::read_to_string(metadata_path).map_err(|e| format!("read error: {e}"))?;

    let (track, _meta) =
        crate::track::codec::decode(&content).map_err(|e| format!("decode error: {e}"))?;

    // Load sibling impl-plan.json when present.
    let impl_plan = {
        let impl_plan_path = parent.join("impl-plan.json");
        match reject_symlinks_below(&impl_plan_path, parent) {
            Ok(false) => None,
            Ok(true) => {
                if !impl_plan_path.is_file() {
                    return Err("impl-plan.json exists but is not a regular file".to_owned());
                }
                let json = std::fs::read_to_string(&impl_plan_path)
                    .map_err(|e| format!("impl-plan.json read error: {e}"))?;
                let doc = crate::impl_plan_codec::decode(&json)
                    .map_err(|e| format!("impl-plan.json decode error: {e}"))?;
                Some(doc)
            }
            Err(e) => return Err(format!("impl-plan.json symlink guard: {e}")),
        }
    };

    Ok(crate::track::render::render_plan(&track, impl_plan.as_ref()))
}

fn guarded_file_present(
    path: &Path,
    trusted_root: &Path,
    track_name: &str,
    filename: &str,
) -> Result<bool, VerifyFinding> {
    match reject_symlinks_below(path, trusted_root) {
        Ok(false) => Ok(false),
        Ok(true) => {
            if path.is_file() {
                Ok(true)
            } else {
                Err(VerifyFinding::error(format!(
                    "{track_name}/{filename}: not a regular file (corrupted track state)"
                )))
            }
        }
        Err(e) => Err(VerifyFinding::error(format!(
            "{track_name}/{filename}: symlink guard rejected path: {e}"
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn setup_track_with_plan(root: &Path, name: &str, plan_content: &str) {
        let track_dir = root.join(TRACK_ITEMS_DIR).join(name);
        std::fs::create_dir_all(&track_dir).unwrap();

        // v6 identity-only metadata — legacy v2/v3/v4/v5 tracks are skipped by the
        // freshness validator on purpose.
        let metadata = serde_json::json!({
            "schema_version": 6,
            "id": name,
            "branch": format!("track/{name}"),
            "title": "Test Track",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "branch_strategy_snapshot": {
                "base_branch": "main",
                "merge_target": "main",
                "merge_method": "squash"
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
            "schema_version": 6,
            "id": "test-track",
            "branch": "track/test-track",
            "title": "Test Track",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "branch_strategy_snapshot": {
                "base_branch": "main",
                "merge_target": "main",
                "merge_method": "squash"
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
    fn test_view_freshness_skips_when_plan_md_absent() {
        // Phase 0/1/2 tracks may not yet have plan.md rendered. The freshness
        // validator must silently SKIP these tracks, matching the behavior of
        // `validate_track_snapshots` in `libs/infrastructure/src/track/render.rs`.
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join(TRACK_ITEMS_DIR).join("no-plan");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata = serde_json::json!({
            "schema_version": 6,
            "id": "no-plan",
            "branch": "track/no-plan",
            "title": "No Plan",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "branch_strategy_snapshot": {
                "base_branch": "main",
                "merge_target": "main",
                "merge_method": "squash"
            }
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
            "schema_version": 6,
            "id": "phase0-bare",
            "branch": "track/phase0-bare",
            "title": "Phase 0 Bare",
            "created_at": "2026-04-27T00:00:00Z",
            "updated_at": "2026-04-27T00:00:00Z",
            "branch_strategy_snapshot": {
                "base_branch": "main",
                "merge_target": "main",
                "merge_method": "squash"
            }
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

    #[test]
    #[cfg(unix)]
    fn test_view_freshness_rejects_symlinked_metadata_file() {
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join(TRACK_ITEMS_DIR).join("metadata-link");
        std::fs::create_dir_all(&track_dir).unwrap();
        let outside = TempDir::new().unwrap();
        let target = outside.path().join("metadata.json");
        std::fs::write(
            &target,
            serde_json::json!({
                "schema_version": 6,
                "id": "metadata-link",
                "branch": "track/metadata-link",
                "title": "Metadata Link",
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z",
                "branch_strategy_snapshot": {
                    "base_branch": "main",
                    "merge_target": "main",
                    "merge_method": "squash"
                }
            })
            .to_string(),
        )
        .unwrap();
        std::os::unix::fs::symlink(&target, track_dir.join("metadata.json")).unwrap();

        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "symlinked metadata.json must fail closed");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("symlink")),
            "finding must mention symlink: {outcome:?}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_view_freshness_rejects_dangling_symlinked_metadata_file() {
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join(TRACK_ITEMS_DIR).join("dangling-metadata-link");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::os::unix::fs::symlink(
            track_dir.join("missing-metadata.json"),
            track_dir.join("metadata.json"),
        )
        .unwrap();

        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "dangling metadata.json symlink must fail closed");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("symlink")),
            "finding must mention symlink: {outcome:?}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_view_freshness_rejects_symlinked_plan_file() {
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join(TRACK_ITEMS_DIR).join("plan-link");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata = serde_json::json!({
            "schema_version": 6,
            "id": "plan-link",
            "branch": "track/plan-link",
            "title": "Plan Link",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "branch_strategy_snapshot": {
                "base_branch": "main",
                "merge_target": "main",
                "merge_method": "squash"
            }
        });
        std::fs::write(track_dir.join("metadata.json"), metadata.to_string()).unwrap();
        let outside = TempDir::new().unwrap();
        let target = outside.path().join("plan.md");
        std::fs::write(&target, "# Plan\n").unwrap();
        std::os::unix::fs::symlink(&target, track_dir.join("plan.md")).unwrap();

        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "symlinked plan.md must fail closed");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("symlink")),
            "finding must mention symlink: {outcome:?}"
        );
    }
}
