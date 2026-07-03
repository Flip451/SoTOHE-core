//! Validation of rendered views (`plan.md`, `registry.md`) against source metadata.

use std::path::Path;

use super::RenderError;
use super::plan::render_plan;
use super::registry::render_registry;
use super::snapshot::{collect_track_snapshots, load_impl_plan_opt};
use crate::track::symlink_guard::reject_symlinks_below;

/// Validates all metadata documents under the project root.
///
/// # Errors
/// Returns `RenderError` if any metadata file cannot be read or decoded.
///
/// # Phase 0 compatibility (ADR 2026-04-19-1242 §D0.0 / §D1.4 / §D6.1)
///
/// Per ADR §D6.1, the plan.md freshness gate fires only "when plan.md is
/// rendered"; if `plan.md` is absent the check is skipped regardless of
/// whether `impl-plan.json` is present. A Phase 0 track (just after
/// `/track:init`) has a freshly created `metadata.json` but no rendered
/// `plan.md` yet — the view is generated after later phases populate
/// `impl-plan.json`. Previously this function unconditionally read `plan.md`
/// and failed with an I/O error for Phase 0 tracks; it now uses
/// `std::fs::metadata()` to distinguish a missing file (NotFound → skip)
/// from a permission error or other I/O failure (propagated), mirroring the
/// presence-conditional pattern used for the optional `registry.md` check.
pub fn validate_track_snapshots(root: &Path) -> Result<(), RenderError> {
    let snapshots = collect_track_snapshots(root)?;
    for snapshot in &snapshots {
        // Only validate v6 (identity-only with branch_strategy_snapshot) tracks.
        // Legacy v2/v3/v4/v5 tracks predate the current renderer and their committed
        // plan.md reflects whatever renderer shipped at their commit time. We intentionally
        // don't touch them; re-validating would create a false OutOfSync for
        // every legacy track without any actionable fix.
        if snapshot.schema_version < 6 {
            continue;
        }
        let plan_path = snapshot.dir.join("plan.md");
        // Phase 0 compat: skip content check when plan.md has not been
        // rendered yet. Per ADR 2026-04-19-1242 §D6.1, the gate fires only
        // "when plan.md is rendered"; if the file is absent, skip regardless
        // of whether impl-plan.json exists.
        //
        // Presence is probed through the shared symlink guard. A missing file
        // is a Phase 0 skip; an existing symlink or non-file is corrupted state.
        if !reject_symlinks_below(&plan_path, &snapshot.dir)? {
            continue;
        }
        if !plan_path.is_file() {
            return Err(RenderError::InvalidTrackMetadata {
                path: plan_path.clone(),
                reason: "plan.md exists but is not a regular file".to_owned(),
            });
        }
        let actual = std::fs::read_to_string(&plan_path)?;
        let impl_plan = load_impl_plan_opt(&snapshot.dir)?;
        let expected = render_plan(&snapshot.track, impl_plan.as_ref());
        if !super::rendered_matches(&actual, &expected) {
            return Err(RenderError::OutOfSync {
                path: plan_path,
                reason: "plan.md does not match metadata.json".to_owned(),
            });
        }
    }

    let registry_path = root.join("track/registry.md");
    // registry.md may be absent if it has been removed from git tracking
    // (e.g., to prevent merge conflicts in parallel track work).
    // In that case, skip the freshness check.
    if reject_symlinks_below(&registry_path, root)? {
        if !registry_path.is_file() {
            return Err(RenderError::InvalidTrackMetadata {
                path: registry_path,
                reason: "registry.md exists but is not a regular file".to_owned(),
            });
        }
        let actual_registry = std::fs::read_to_string(&registry_path)?;
        let expected_registry = render_registry(&snapshots);
        if !super::rendered_matches(&actual_registry, &expected_registry) {
            return Err(RenderError::OutOfSync {
                path: registry_path,
                reason: "registry.md does not match metadata.json".to_owned(),
            });
        }
    }
    Ok(())
}
