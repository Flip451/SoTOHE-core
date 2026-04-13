//! Rendering and sync of track read-only views (`plan.md`, `registry.md`, `spec.md`, `domain-types.md`) from metadata.json / spec.json / domain-types.json.

use std::path::{Path, PathBuf};

use domain::{TaskStatus, TrackMetadata};

use super::atomic_write::atomic_write_file;
use super::codec::{self, DocumentMeta};
use crate::spec;
use crate::tddd::catalogue_codec;
use crate::type_catalogue_render;

const TRACK_ITEMS_DIR: &str = "track/items";
const TRACK_ARCHIVE_DIR: &str = "track/archive";
const RESERVED_ID_SEGMENTS: &[&str] = &["git"];
const VALID_TRACK_STATUSES: &[&str] =
    &["planned", "in_progress", "done", "blocked", "cancelled", "archived"];
const REQUIRED_V3_METADATA_FIELDS: &[&str] = &[
    "schema_version",
    "branch",
    "id",
    "title",
    "status",
    "created_at",
    "updated_at",
    "tasks",
    "plan",
];

fn rendered_matches(actual: &str, expected: &str) -> bool {
    actual == expected || actual.trim_end_matches('\n') == expected.trim_end_matches('\n')
}

/// Track aggregate plus metadata-only fields required for view rendering.
#[derive(Debug, Clone)]
pub struct TrackSnapshot {
    pub dir: PathBuf,
    pub track: TrackMetadata,
    pub meta: DocumentMeta,
    pub schema_version: u32,
}

impl TrackSnapshot {
    #[must_use]
    pub fn status(&self) -> String {
        match self.meta.original_status.as_deref() {
            Some("archived") => "archived".to_owned(),
            _ => self.track.status().to_string(),
        }
    }

    #[must_use]
    pub fn updated_at(&self) -> &str {
        &self.meta.updated_at
    }
}

/// Error while collecting or syncing rendered views.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid metadata at {path}: {source}")]
    InvalidMetadata {
        path: PathBuf,
        #[source]
        source: codec::CodecError,
    },

    #[error("rendered view out of sync at {path}: {reason}")]
    OutOfSync { path: PathBuf, reason: String },

    #[error("unsupported schema_version {schema_version} at {path}")]
    UnsupportedSchemaVersion { path: PathBuf, schema_version: u32 },

    #[error("invalid track metadata at {path}: {reason}")]
    InvalidTrackMetadata { path: PathBuf, reason: String },
}

/// Collects all valid track snapshots from active and archive directories.
///
/// # Errors
/// Returns `RenderError` if a metadata file cannot be read or decoded.
pub fn collect_track_snapshots(root: &Path) -> Result<Vec<TrackSnapshot>, RenderError> {
    let mut track_dirs = Vec::new();
    for rel in [TRACK_ITEMS_DIR, TRACK_ARCHIVE_DIR] {
        let base = root.join(rel);
        if !base.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(base)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                track_dirs.push(path);
            }
        }
    }
    track_dirs.sort();

    let mut snapshots = Vec::new();
    for track_dir in track_dirs {
        let metadata_path = track_dir.join("metadata.json");
        if !metadata_path.is_file() {
            continue;
        }

        let json = std::fs::read_to_string(&metadata_path)?;
        let parsed: codec::TrackDocumentV2 =
            serde_json::from_str(&json).map_err(|source| RenderError::InvalidMetadata {
                path: metadata_path.clone(),
                source: codec::CodecError::Json(source),
            })?;
        if !matches!(parsed.schema_version, 2 | 3) {
            return Err(RenderError::UnsupportedSchemaVersion {
                path: metadata_path,
                schema_version: parsed.schema_version,
            });
        }
        validate_track_document(&metadata_path, track_dir.file_name(), &parsed)?;

        let decoded = codec::decode(&json).map_err(|source| RenderError::InvalidMetadata {
            path: metadata_path.clone(),
            source,
        })?;
        let (track, meta) = decoded;
        snapshots.push(TrackSnapshot {
            dir: track_dir,
            track,
            meta,
            schema_version: parsed.schema_version,
        });
    }

    snapshots.sort_by(|a, b| b.updated_at().cmp(a.updated_at()));
    Ok(snapshots)
}

/// Renders `plan.md` content from a track snapshot.
#[must_use]
pub fn render_plan(track: &TrackMetadata) -> String {
    let mut lines = Vec::new();
    lines.push("<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->".to_owned());
    lines.push(format!("# {}", track.title()));
    lines.push(String::new());

    for summary in track.plan().summary() {
        lines.push(summary.clone());
    }
    if !track.plan().summary().is_empty() {
        lines.push(String::new());
    }

    let task_map = track
        .tasks()
        .iter()
        .map(|task| (task.id().as_ref(), task))
        .collect::<std::collections::HashMap<_, _>>();

    for section in track.plan().sections() {
        lines.push(format!("## {}", section.title()));
        lines.push(String::new());

        for desc in section.description() {
            lines.push(desc.clone());
        }
        if !section.description().is_empty() {
            lines.push(String::new());
        }

        for task_id in section.task_ids() {
            if let Some(task) = task_map.get(task_id.as_ref()) {
                let marker = match task.status() {
                    TaskStatus::Todo => " ",
                    TaskStatus::InProgress => "~",
                    TaskStatus::DonePending | TaskStatus::DoneTraced { .. } => "x",
                    TaskStatus::Skipped => "-",
                };
                let suffix = match task.status() {
                    TaskStatus::DoneTraced { commit_hash } => format!(" {commit_hash}"),
                    _ => String::new(),
                };
                lines.push(format!("- [{marker}] {}{suffix}", task.description()));
            }
        }

        lines.push(String::new());
    }

    lines.join("\n")
}

fn next_command_for_track(track: &TrackSnapshot) -> String {
    let raw = domain::track_phase::next_command(&track.track, track.schema_version);
    format!("`{raw}`")
}

fn format_date(iso_timestamp: &str) -> &str {
    if iso_timestamp.len() >= 10 { &iso_timestamp[..10] } else { iso_timestamp }
}

/// Renders `registry.md` content from all track snapshots.
#[must_use]
pub fn render_registry(tracks: &[TrackSnapshot]) -> String {
    let mut active: Vec<_> = tracks
        .iter()
        .filter(|track| {
            matches!(track.status().as_ref(), "planned" | "in_progress" | "blocked" | "cancelled")
        })
        .collect();
    active.sort_by_key(|track| {
        track.schema_version == 3 && track.status() == "planned" && track.track.branch().is_none()
    });
    let completed: Vec<_> = tracks.iter().filter(|track| track.status() == "done").collect();
    let archived: Vec<_> = tracks.iter().filter(|track| track.status() == "archived").collect();

    let mut lines = vec![
        "# Track Registry".to_owned(),
        String::new(),
        "> This file lists all tracks and their current status.".to_owned(),
        "> Auto-updated by `/track:plan`, `/track:plan-only`, `/track:activate`, and `/track:commit`.".to_owned(),
        "> `/track:status` uses this file as an entry point to summarize progress.".to_owned(),
        "> Each track is expected to have `spec.md` / `plan.md` / `metadata.json` / `verification.md`.".to_owned(),
        String::new(),
        "## Current Focus".to_owned(),
        String::new(),
    ];

    if let Some(latest) = active.first() {
        lines.push(format!("- Latest active track: `{}`", latest.track.id()));
        lines.push(format!("- Next recommended command: {}", next_command_for_track(latest)));
        lines.push(format!("- Last updated: `{}`", format_date(latest.updated_at())));
    } else {
        lines.push("- Latest active track: `None yet`".to_owned());
        lines.push("- Next recommended command: `/track:plan <feature>`".to_owned());
        if let Some(latest) = tracks.first() {
            lines.push(format!("- Last updated: `{}`", format_date(latest.updated_at())));
        } else {
            lines.push("- Last updated: `YYYY-MM-DD`".to_owned());
        }
    }
    lines.push(String::new());

    lines.push("## Active Tracks".to_owned());
    lines.push(String::new());
    lines.push("| Track | Status | Next | Updated |".to_owned());
    lines.push("|------|--------|------|---------|".to_owned());
    if active.is_empty() {
        lines.push("| _No active tracks yet_ | - | `/track:plan <feature>` | - |".to_owned());
    } else {
        for track in &active {
            let status = track.status();
            lines.push(format!(
                "| {} | {} | {} | {} |",
                track.track.id(),
                status,
                next_command_for_track(track),
                format_date(track.updated_at())
            ));
        }
    }
    lines.push(String::new());

    lines.push("## Completed Tracks".to_owned());
    lines.push(String::new());
    lines.push("| Track | Result | Updated |".to_owned());
    lines.push("|------|--------|---------|".to_owned());
    if completed.is_empty() {
        lines.push("| _No completed tracks yet_ | - | - |".to_owned());
    } else {
        for track in &completed {
            lines.push(format!(
                "| {} | Done | {} |",
                track.track.id(),
                format_date(track.updated_at())
            ));
        }
    }
    lines.push(String::new());

    lines.push("## Archived Tracks".to_owned());
    lines.push(String::new());
    lines.push("| Track | Result | Archived |".to_owned());
    lines.push("|------|--------|----------|".to_owned());
    if archived.is_empty() {
        lines.push("| _No archived tracks yet_ | - | - |".to_owned());
    } else {
        for track in &archived {
            lines.push(format!(
                "| {} | Archived | {} |",
                track.track.id(),
                format_date(track.updated_at())
            ));
        }
    }
    lines.push(String::new());
    lines.push("---".to_owned());
    lines.push(String::new());
    lines.push(
        "Use `/track:plan <feature>` for the standard lane or `/track:plan-only <feature>` when planning should land before activation.".to_owned(),
    );
    lines.push(String::new());

    lines.join("\n")
}

/// Validates all metadata documents under the project root.
///
/// # Errors
/// Returns `RenderError` if any metadata file cannot be read or decoded.
pub fn validate_track_snapshots(root: &Path) -> Result<(), RenderError> {
    let snapshots = collect_track_snapshots(root)?;
    for snapshot in &snapshots {
        let plan_path = snapshot.dir.join("plan.md");
        let actual = std::fs::read_to_string(&plan_path)?;
        let expected = render_plan(&snapshot.track);
        if !rendered_matches(&actual, &expected) {
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
    if registry_path.is_file() {
        let actual_registry = std::fs::read_to_string(&registry_path)?;
        let expected_registry = render_registry(&snapshots);
        if !rendered_matches(&actual_registry, &expected_registry) {
            return Err(RenderError::OutOfSync {
                path: registry_path,
                reason: "registry.md does not match metadata.json".to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_track_document(
    metadata_path: &Path,
    dir_name: Option<&std::ffi::OsStr>,
    doc: &codec::TrackDocumentV2,
) -> Result<(), RenderError> {
    let Some(dir_name) = dir_name.and_then(std::ffi::OsStr::to_str) else {
        return Err(RenderError::InvalidTrackMetadata {
            path: metadata_path.to_path_buf(),
            reason: "track directory name is not valid UTF-8".to_owned(),
        });
    };

    if doc.id != dir_name {
        return Err(RenderError::InvalidTrackMetadata {
            path: metadata_path.to_path_buf(),
            reason: format!("metadata id '{}' does not match directory '{}'", doc.id, dir_name),
        });
    }

    let segments = doc.id.split('-').collect::<Vec<_>>();
    for reserved in RESERVED_ID_SEGMENTS {
        if segments.iter().any(|segment| segment.eq_ignore_ascii_case(reserved)) {
            return Err(RenderError::InvalidTrackMetadata {
                path: metadata_path.to_path_buf(),
                reason: format!("Track id '{}' contains reserved segment '{}'", doc.id, reserved),
            });
        }
    }

    if !VALID_TRACK_STATUSES.contains(&doc.status.as_ref()) {
        return Err(RenderError::InvalidTrackMetadata {
            path: metadata_path.to_path_buf(),
            reason: format!("Invalid track status '{}'", doc.status),
        });
    }

    let raw_json = std::fs::read_to_string(metadata_path).map_err(RenderError::Io)?;
    let raw_doc: serde_json::Value = serde_json::from_str(&raw_json).map_err(|source| {
        RenderError::InvalidMetadata { path: metadata_path.to_path_buf(), source: source.into() }
    })?;
    if doc.schema_version == 3 {
        let Some(object) = raw_doc.as_object() else {
            return Err(RenderError::InvalidTrackMetadata {
                path: metadata_path.to_path_buf(),
                reason: "metadata.json must be a JSON object".to_owned(),
            });
        };
        if let Some(missing) =
            REQUIRED_V3_METADATA_FIELDS.iter().find(|field| !object.contains_key(**field))
        {
            return Err(RenderError::InvalidTrackMetadata {
                path: metadata_path.to_path_buf(),
                reason: format!("Missing required field '{missing}'"),
            });
        }
    }

    let (track, meta) = codec::decode(&raw_json).map_err(|source| {
        RenderError::InvalidMetadata { path: metadata_path.to_path_buf(), source }
    })?;

    let derived = track.status().to_string();
    if doc.status == "archived" {
        if derived != "done" {
            return Err(RenderError::InvalidTrackMetadata {
                path: metadata_path.to_path_buf(),
                reason: format!(
                    "Status drift: archived track must have all tasks resolved (done/skipped), but derived='{derived}'"
                ),
            });
        }
    } else if doc.status != derived {
        return Err(RenderError::InvalidTrackMetadata {
            path: metadata_path.to_path_buf(),
            reason: format!(
                "Status drift: metadata.status='{}' but derived='{}'",
                doc.status, derived
            ),
        });
    }

    if doc.schema_version == 3
        && doc.branch.is_none()
        && !(doc.status == "planned" && derived == "planned")
    {
        return Err(RenderError::InvalidTrackMetadata {
            path: metadata_path.to_path_buf(),
            reason: "'branch' is required for v3 tracks unless the track is planning-only"
                .to_owned(),
        });
    }

    let _ = meta;
    Ok(())
}

/// Renders `plan.md` and `registry.md` from metadata.json and writes changed files atomically.
///
/// Returned paths may include files ignored by version control (e.g., `track/registry.md`).
/// Callers that stage paths for git operations must apply their own filtering to exclude
/// gitignored entries.
///
/// # Errors
/// Returns `RenderError` on file-system or metadata decode failure.
pub fn sync_rendered_views(
    root: &Path,
    track_id: Option<&str>,
) -> Result<Vec<PathBuf>, RenderError> {
    let mut changed = Vec::new();
    let snapshots = collect_track_snapshots(root)?;
    let rendered_registry = render_registry(&snapshots);
    let registry_path = root.join("track/registry.md");

    // Per-track view rendering: only when a specific `track_id` is requested.
    // Passing `None` means "refresh registry.md only" (useful after archiving
    // or when the caller has no active track context).
    //
    // The previous bulk mode ("render every track under items/ and archive/")
    // has been removed in favour of this scoped design. If a caller wants to
    // refresh multiple tracks, it must iterate and call this function once per
    // explicit track_id. This guarantees that single-track callers (e.g., the
    // final `in_progress → done` transition of an active track) always render
    // the requested track unconditionally, and that archived tracks under
    // `track/archive/` are naturally protected because they are never
    // referenced through `track/items/<id>`.
    let track_dirs: Vec<PathBuf> = match track_id {
        Some(id) => vec![root.join(TRACK_ITEMS_DIR).join(id)],
        None => Vec::new(),
    };

    for track_dir in track_dirs {
        let metadata_path = track_dir.join("metadata.json");
        if !metadata_path.is_file() {
            continue;
        }
        let json = std::fs::read_to_string(&metadata_path)?;
        let parsed: codec::TrackDocumentV2 =
            serde_json::from_str(&json).map_err(|source| RenderError::InvalidMetadata {
                path: metadata_path.clone(),
                source: codec::CodecError::Json(source),
            })?;
        if !matches!(parsed.schema_version, 2 | 3) {
            return Err(RenderError::UnsupportedSchemaVersion {
                path: metadata_path,
                schema_version: parsed.schema_version,
            });
        }

        // `plan.md` must be re-rendered unconditionally on the single-track
        // path because `execute_transition` relies on it to reflect the
        // post-transition task state (including the `in_progress → done`
        // flip that caused the earlier bug). The legacy-protection guard
        // only applies to `spec.md` / `domain-types.md` below, not plan.md.
        //
        // Archived tracks under `track/archive/<id>` are still naturally
        // protected because this function only looks at
        // `track/items/<track_id>` — passing an archived id resolves to a
        // missing metadata file and is silently skipped.
        let is_done_or_archived = matches!(parsed.status.as_str(), "done" | "archived");

        let (track, _) = codec::decode(&json).map_err(|source| RenderError::InvalidMetadata {
            path: metadata_path.clone(),
            source,
        })?;
        let rendered = render_plan(&track);
        let plan_path = track_dir.join("plan.md");
        let old = match std::fs::read_to_string(&plan_path) {
            Ok(content) => Some(content),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(RenderError::Io(e)),
        };
        if old.as_deref().is_none_or(|existing| !rendered_matches(existing, rendered.as_ref())) {
            atomic_write_file(&plan_path, rendered.as_bytes())?;
            changed.push(plan_path);
        }

        // Render spec.md from spec.json if present. Skipped for done/archived
        // tracks to avoid silently overwriting legacy rendered content with a
        // newer renderer that may drop fields an older format preserved —
        // transitions into `done` do NOT touch spec.json, so re-rendering
        // here would only surface renderer-version drift, not new data.
        let spec_json_path = track_dir.join("spec.json");
        if !is_done_or_archived && spec_json_path.is_file() {
            let spec_json_content = std::fs::read_to_string(&spec_json_path)?;
            match spec::codec::decode(&spec_json_content) {
                Ok(spec_doc) => {
                    let rendered_spec = spec::render::render_spec(&spec_doc);
                    let spec_md_path = track_dir.join("spec.md");
                    // Read existing spec.md: propagate real I/O errors, treat NotFound as absent.
                    let old_spec = match std::fs::read_to_string(&spec_md_path) {
                        Ok(content) => Some(content),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                        Err(e) => return Err(RenderError::Io(e)),
                    };
                    if old_spec
                        .as_deref()
                        .is_none_or(|existing| !rendered_matches(existing, &rendered_spec))
                    {
                        atomic_write_file(&spec_md_path, rendered_spec.as_bytes())?;
                        changed.push(spec_md_path);
                    }
                }
                Err(spec::codec::SpecCodecError::Json(_)) => {
                    // Warn and continue only on JSON parse errors — file may be mid-edit.
                    // Schema version and validation errors are hard failures that should surface.
                    eprintln!(
                        "warning: skipping spec.md render for {} (malformed JSON)",
                        track_dir.display()
                    );
                }
                Err(e) => {
                    // Unsupported schema version or domain validation failure — propagate as I/O error
                    // so callers (CI, verify-arch-docs) detect spec.json corruption.
                    return Err(RenderError::Io(std::io::Error::other(format!(
                        "spec.json error at {}: {e}",
                        track_dir.display()
                    ))));
                }
            }
        }

        // Render domain-types.md from domain-types.json if present. Same
        // legacy-protection rationale as spec.md above: transitions into
        // `done` do not modify domain-types.json, so re-rendering here would
        // only surface renderer-version drift. Skip for done/archived.
        let domain_types_json_path = track_dir.join("domain-types.json");
        if !is_done_or_archived && domain_types_json_path.is_file() {
            let domain_types_content = std::fs::read_to_string(&domain_types_json_path)?;
            match catalogue_codec::decode(&domain_types_content) {
                Ok(doc) => {
                    let rendered = type_catalogue_render::render_type_catalogue(&doc);
                    let domain_types_md_path = track_dir.join("domain-types.md");
                    let old_md = match std::fs::read_to_string(&domain_types_md_path) {
                        Ok(content) => Some(content),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                        Err(e) => return Err(RenderError::Io(e)),
                    };
                    if old_md
                        .as_deref()
                        .is_none_or(|existing| !rendered_matches(existing, &rendered))
                    {
                        atomic_write_file(&domain_types_md_path, rendered.as_bytes())?;
                        changed.push(domain_types_md_path);
                    }
                }
                Err(catalogue_codec::TypeCatalogueCodecError::Json(_)) => {
                    // Warn and continue only on JSON parse errors — file may be mid-edit.
                    eprintln!(
                        "warning: skipping domain-types.md render for {} (malformed JSON)",
                        track_dir.display()
                    );
                }
                Err(e) => {
                    return Err(RenderError::Io(std::io::Error::other(format!(
                        "domain-types.json error at {}: {e}",
                        track_dir.display()
                    ))));
                }
            }
        }
    }

    if let Some(parent) = registry_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let old = match std::fs::read_to_string(&registry_path) {
        Ok(content) => Some(content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(RenderError::Io(e)),
    };
    if old.as_deref().is_none_or(|existing| !rendered_matches(existing, rendered_registry.as_ref()))
    {
        atomic_write_file(&registry_path, rendered_registry.as_bytes())?;
        changed.push(registry_path);
    }

    Ok(changed)
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn sample_metadata_json(id: &str, status: &str, updated_at: &str, tasks_json: &str) -> String {
        sample_metadata_json_with_schema_and_branch(
            3,
            id,
            status,
            updated_at,
            tasks_json,
            Some(&format!("track/{id}")),
        )
    }

    fn sample_metadata_json_with_branch(
        id: &str,
        status: &str,
        updated_at: &str,
        tasks_json: &str,
        branch: Option<&str>,
    ) -> String {
        sample_metadata_json_with_schema_and_branch(3, id, status, updated_at, tasks_json, branch)
    }

    fn sample_metadata_json_with_schema_and_branch(
        schema_version: u32,
        id: &str,
        status: &str,
        updated_at: &str,
        tasks_json: &str,
        branch: Option<&str>,
    ) -> String {
        let branch_field = match branch {
            Some(branch) => format!(r#""branch": "{branch}","#),
            None => r#""branch": null,"#.to_owned(),
        };
        format!(
            r#"{{
  "schema_version": {schema_version},
  "id": "{id}",
  {branch_field}
  "title": "Title {id}",
  "status": "{status}",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "{updated_at}",
  "tasks": {tasks_json},
  "plan": {{
    "summary": ["Summary line"],
    "sections": [
      {{
        "id": "S001",
        "title": "Section",
        "description": ["Section desc"],
        "task_ids": ["T001"]
      }}
    ]
  }}
}}
"#
        )
    }

    #[test]
    fn render_plan_matches_expected_layout() {
        let json = sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T01:00:00Z",
            r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
        );
        let (track, _) = codec::decode(&json).unwrap();

        let rendered = render_plan(&track);

        assert!(rendered.contains("<!-- Generated from metadata.json"));
        assert!(rendered.contains("# Title track-a"));
        assert!(rendered.contains("## Section"));
        assert!(rendered.contains("- [ ] First task"));
    }

    #[test]
    fn render_registry_places_active_completed_and_archived() {
        let active_json = sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
        );
        let done_json = sample_metadata_json(
            "track-b",
            "done",
            "2026-03-13T01:00:00Z",
            r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "done",
      "commit_hash": "abc1234"
    }
  ]"#,
        );
        let archived_json = sample_metadata_json(
            "track-c",
            "archived",
            "2026-03-13T00:00:00Z",
            r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
        );

        let (active_track, active_meta) = codec::decode(&active_json).unwrap();
        let (done_track, done_meta) = codec::decode(&done_json).unwrap();
        let (archived_track, archived_meta) = codec::decode(&archived_json).unwrap();
        let rendered = render_registry(&[
            TrackSnapshot {
                dir: PathBuf::from("track/items/track-a"),
                track: active_track,
                meta: active_meta,
                schema_version: 3,
            },
            TrackSnapshot {
                dir: PathBuf::from("track/items/track-b"),
                track: done_track,
                meta: done_meta,
                schema_version: 3,
            },
            TrackSnapshot {
                dir: PathBuf::from("track/archive/track-c"),
                track: archived_track,
                meta: archived_meta,
                schema_version: 3,
            },
        ]);

        assert!(rendered.contains("| track-a | planned | `/track:implement` | 2026-03-13 |"));
        assert!(rendered.contains("| track-b | Done | 2026-03-13 |"));
        assert!(rendered.contains("| track-c | Archived | 2026-03-13 |"));
    }

    #[test]
    fn render_registry_routes_branchless_planning_track_to_activate() {
        let plan_only_json = sample_metadata_json_with_branch(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
            None,
        );
        let (plan_only_track, plan_only_meta) = codec::decode(&plan_only_json).unwrap();
        let rendered = render_registry(&[TrackSnapshot {
            dir: PathBuf::from("track/items/track-a"),
            track: plan_only_track,
            meta: plan_only_meta,
            schema_version: 3,
        }]);

        assert!(rendered.contains("/track:activate track-a"));
        assert!(rendered.contains("/track:plan-only <feature>"));
    }

    #[test]
    fn render_registry_keeps_legacy_v2_branchless_planned_track_on_implement() {
        let legacy_json = sample_metadata_json_with_schema_and_branch(
            2,
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
            None,
        );
        let (legacy_track, legacy_meta) = codec::decode(&legacy_json).unwrap();
        let rendered = render_registry(&[TrackSnapshot {
            dir: PathBuf::from("track/items/track-a"),
            track: legacy_track,
            meta: legacy_meta,
            schema_version: 2,
        }]);

        assert!(rendered.contains("/track:implement"));
        assert!(!rendered.contains("/track:activate track-a"));
    }

    #[test]
    fn render_registry_prefers_materialized_active_track_in_current_focus() {
        let plan_only_json = sample_metadata_json_with_branch(
            "track-plan-only",
            "planned",
            "2026-03-13T03:00:00Z",
            r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
            None,
        );
        let materialized_json = sample_metadata_json(
            "track-materialized",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
        );
        let (plan_only_track, plan_only_meta) = codec::decode(&plan_only_json).unwrap();
        let (materialized_track, materialized_meta) = codec::decode(&materialized_json).unwrap();
        let rendered = render_registry(&[
            TrackSnapshot {
                dir: PathBuf::from("track/items/track-plan-only"),
                track: plan_only_track,
                meta: plan_only_meta,
                schema_version: 3,
            },
            TrackSnapshot {
                dir: PathBuf::from("track/items/track-materialized"),
                track: materialized_track,
                meta: materialized_meta,
                schema_version: 3,
            },
        ]);

        assert!(rendered.contains("- Latest active track: `track-materialized`"));
        assert!(rendered.contains("- Next recommended command: `/track:implement`"));
    }

    #[test]
    fn render_registry_prefers_legacy_v2_planned_track_over_newer_plan_only() {
        let legacy_json = sample_metadata_json_with_schema_and_branch(
            2,
            "track-legacy",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
            None,
        );
        let plan_only_json = sample_metadata_json_with_branch(
            "track-plan-only",
            "planned",
            "2026-03-13T03:00:00Z",
            r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
            None,
        );
        let (legacy_track, legacy_meta) = codec::decode(&legacy_json).unwrap();
        let (plan_only_track, plan_only_meta) = codec::decode(&plan_only_json).unwrap();
        let rendered = render_registry(&[
            TrackSnapshot {
                dir: PathBuf::from("track/items/track-plan-only"),
                track: plan_only_track,
                meta: plan_only_meta,
                schema_version: 3,
            },
            TrackSnapshot {
                dir: PathBuf::from("track/items/track-legacy"),
                track: legacy_track,
                meta: legacy_meta,
                schema_version: 2,
            },
        ]);

        assert!(rendered.contains("- Latest active track: `track-legacy`"));
        assert!(rendered.contains("- Next recommended command: `/track:implement`"));
    }

    #[test]
    fn sync_rendered_views_writes_plan_and_registry() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
            ),
        )
        .unwrap();

        let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

        assert!(changed.iter().any(|path| path.ends_with("plan.md")));
        assert!(changed.iter().any(|path| path.ends_with("registry.md")));
        assert!(track_dir.join("plan.md").is_file());
        assert!(dir.path().join("track/registry.md").is_file());
    }

    #[test]
    fn sync_rendered_views_single_track_rejects_unrelated_invalid_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let good_track = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&good_track).unwrap();
        std::fs::write(
            good_track.join("metadata.json"),
            sample_metadata_json(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
            ),
        )
        .unwrap();

        let bad_track = dir.path().join("track/items/bad-track");
        std::fs::create_dir_all(&bad_track).unwrap();
        std::fs::write(
            bad_track.join("metadata.json"),
            r#"{
  "schema_version": 99,
  "id": "bad-track",
  "title": "Bad Track",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T00:00:00Z",
  "tasks": [],
  "plan": { "summary": [], "sections": [] }
}"#,
        )
        .unwrap();

        let err = sync_rendered_views(dir.path(), Some("track-a")).unwrap_err();
        assert!(matches!(err, RenderError::UnsupportedSchemaVersion { .. }));
        assert!(!good_track.join("plan.md").exists());
        assert!(!dir.path().join("track/registry.md").exists());
    }

    #[test]
    fn validate_track_snapshots_rejects_invalid_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/bad-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), "{").unwrap();

        let err = validate_track_snapshots(dir.path()).unwrap_err();
        assert!(err.to_string().contains("invalid metadata"));
    }

    #[test]
    fn validate_track_snapshots_rejects_unsupported_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/bad-schema");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            r#"{
  "schema_version": 99,
  "id": "bad-schema",
  "title": "Bad Schema",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T00:00:00Z",
  "tasks": [],
  "plan": { "summary": [], "sections": [] }
}"#,
        )
        .unwrap();

        let err = validate_track_snapshots(dir.path()).unwrap_err();
        assert!(err.to_string().contains("unsupported schema_version 99"));
    }

    #[test]
    fn validate_track_snapshots_rejects_out_of_sync_plan() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
            ),
        )
        .unwrap();
        std::fs::write(track_dir.join("plan.md"), "# stale\n").unwrap();
        std::fs::create_dir_all(dir.path().join("track")).unwrap();
        std::fs::write(dir.path().join("track/registry.md"), "# registry\n").unwrap();

        let err = validate_track_snapshots(dir.path()).unwrap_err();
        assert!(err.to_string().contains("plan.md does not match metadata.json"));
    }

    #[test]
    fn validate_track_snapshots_rejects_metadata_id_directory_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json(
                "other-track",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
            ),
        )
        .unwrap();
        std::fs::write(track_dir.join("plan.md"), "# stale\n").unwrap();
        std::fs::create_dir_all(dir.path().join("track")).unwrap();
        std::fs::write(dir.path().join("track/registry.md"), "# registry\n").unwrap();

        let err = validate_track_snapshots(dir.path()).unwrap_err();
        assert!(
            err.to_string()
                .contains("metadata id 'other-track' does not match directory 'track-a'")
        );
    }

    #[test]
    fn validate_track_snapshots_rejects_out_of_sync_registry() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
            ),
        )
        .unwrap();
        let (track, _) =
            codec::decode(&std::fs::read_to_string(track_dir.join("metadata.json")).unwrap())
                .unwrap();
        std::fs::write(track_dir.join("plan.md"), render_plan(&track)).unwrap();
        std::fs::create_dir_all(dir.path().join("track")).unwrap();
        std::fs::write(dir.path().join("track/registry.md"), "# stale registry\n").unwrap();

        let err = validate_track_snapshots(dir.path()).unwrap_err();
        assert!(err.to_string().contains("registry.md does not match metadata.json"));
    }

    #[test]
    fn validate_track_document_accepts_planning_only_v3_without_branch() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
        std::fs::write(
            &metadata_path,
            sample_metadata_json_with_branch(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
                None,
            ),
        )
        .unwrap();

        let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();

        let result = validate_track_document(&metadata_path, track_dir.file_name(), &doc);

        assert!(result.is_ok());
    }

    #[test]
    fn validate_track_document_rejects_non_planning_v3_without_branch() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
        std::fs::write(
            &metadata_path,
            sample_metadata_json_with_branch(
                "track-a",
                "in_progress",
                "2026-03-13T02:00:00Z",
                r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "in_progress"
    }
  ]"#,
                None,
            ),
        )
        .unwrap();

        let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
        let err = validate_track_document(&metadata_path, track_dir.file_name(), &doc).unwrap_err();

        assert!(
            err.to_string()
                .contains("'branch' is required for v3 tracks unless the track is planning-only")
        );
    }

    #[test]
    fn validate_track_document_rejects_v3_track_missing_branch_field() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
        std::fs::write(
            &metadata_path,
            r#"{
  "schema_version": 3,
  "id": "track-a",
  "title": "Track A",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T02:00:00Z",
  "tasks": [
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ],
  "plan": {
    "summary": [],
    "sections": [
      {
        "id": "S1",
        "title": "Build",
        "description": [],
        "task_ids": ["T001"]
      }
    ]
  }
}"#,
        )
        .unwrap();

        let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
        let err = validate_track_document(&metadata_path, track_dir.file_name(), &doc).unwrap_err();

        assert!(err.to_string().contains("Missing required field 'branch'"));
    }

    #[test]
    fn validate_track_document_rejects_v3_track_missing_tasks_field() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
        std::fs::write(
            &metadata_path,
            r#"{
  "schema_version": 3,
  "id": "track-a",
  "branch": null,
  "title": "Track A",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T02:00:00Z",
  "plan": {
    "summary": [],
    "sections": [
      {
        "id": "S1",
        "title": "Build",
        "description": [],
        "task_ids": []
      }
    ]
  }
}"#,
        )
        .unwrap();

        let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
        let err = validate_track_document(&metadata_path, track_dir.file_name(), &doc).unwrap_err();

        assert!(err.to_string().contains("Missing required field 'tasks'"));
    }

    #[test]
    fn validate_track_document_rejects_unreferenced_task() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
        // T002 is declared in tasks but not referenced from any plan section.
        std::fs::write(
            &metadata_path,
            r#"{
  "schema_version": 3,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Track A",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T02:00:00Z",
  "tasks": [
    { "id": "T001", "description": "Referenced task", "status": "todo" },
    { "id": "T002", "description": "Unreferenced task", "status": "todo" }
  ],
  "plan": {
    "summary": [],
    "sections": [
      { "id": "S1", "title": "Build", "description": [], "task_ids": ["T001"] }
    ]
  }
}"#,
        )
        .unwrap();

        let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
        let err = validate_track_document(&metadata_path, track_dir.file_name(), &doc).unwrap_err();

        let message = err.to_string();
        assert!(
            message.contains("T002"),
            "error should reference unreferenced task id T002: {message}"
        );
    }

    #[test]
    fn validate_track_document_rejects_duplicate_task_reference() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
        // T001 is referenced by both S1 and S2 sections.
        std::fs::write(
            &metadata_path,
            r#"{
  "schema_version": 3,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Track A",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T02:00:00Z",
  "tasks": [
    { "id": "T001", "description": "Shared task", "status": "todo" }
  ],
  "plan": {
    "summary": [],
    "sections": [
      { "id": "S1", "title": "First",  "description": [], "task_ids": ["T001"] },
      { "id": "S2", "title": "Second", "description": [], "task_ids": ["T001"] }
    ]
  }
}"#,
        )
        .unwrap();

        let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
        let err = validate_track_document(&metadata_path, track_dir.file_name(), &doc).unwrap_err();

        let message = err.to_string();
        assert!(
            message.contains("T001"),
            "error should reference duplicated task id T001: {message}"
        );
    }

    #[test]
    fn validate_track_document_rejects_status_drift_in_progress_vs_done() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
        // metadata.status is "in_progress" but all tasks are done (derived = "done").
        std::fs::write(
            &metadata_path,
            r#"{
  "schema_version": 3,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Track A",
  "status": "in_progress",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T02:00:00Z",
  "tasks": [
    {
      "id": "T001",
      "description": "Completed task",
      "status": "done",
      "commit_hash": "abc1234"
    }
  ],
  "plan": {
    "summary": [],
    "sections": [
      { "id": "S1", "title": "Build", "description": [], "task_ids": ["T001"] }
    ]
  }
}"#,
        )
        .unwrap();

        let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
        let err = validate_track_document(&metadata_path, track_dir.file_name(), &doc).unwrap_err();

        let message = err.to_string();
        assert!(message.contains("Status drift"), "error should mention status drift: {message}");
    }

    #[test]
    fn validate_track_document_rejects_archived_with_incomplete_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
        // metadata.status is "archived" but one task is still "todo".
        std::fs::write(
            &metadata_path,
            r#"{
  "schema_version": 3,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Track A",
  "status": "archived",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T02:00:00Z",
  "tasks": [
    { "id": "T001", "description": "Unfinished task", "status": "todo" }
  ],
  "plan": {
    "summary": [],
    "sections": [
      { "id": "S1", "title": "Build", "description": [], "task_ids": ["T001"] }
    ]
  }
}"#,
        )
        .unwrap();

        let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
        let err = validate_track_document(&metadata_path, track_dir.file_name(), &doc).unwrap_err();

        let message = err.to_string();
        assert!(
            message.contains("archived track must have all tasks resolved"),
            "error should mention archived+incomplete rejection: {message}"
        );
    }

    #[test]
    fn validate_track_document_accepts_id_with_git_substring_in_segment() {
        // "legit" contains "git" as a substring but is not a whole segment,
        // so reserved-id matching must not reject the track.
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/legit-cleanup-2026-03-11");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
        std::fs::write(
            &metadata_path,
            sample_metadata_json_with_branch(
                "legit-cleanup-2026-03-11",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[
    { "id": "T001", "description": "First task", "status": "todo" }
  ]"#,
                None,
            ),
        )
        .unwrap();

        let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
        let result = validate_track_document(&metadata_path, track_dir.file_name(), &doc);

        assert!(result.is_ok(), "legit-cleanup-* must be accepted, got: {result:?}");
    }

    #[test]
    fn sync_rendered_views_generates_spec_md_from_spec_json() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        // Write valid metadata.json
        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
            ),
        )
        .unwrap();

        // Write a minimal spec.json
        std::fs::write(
            track_dir.join("spec.json"),
            r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature Alpha",
  "scope": { "in_scope": [], "out_of_scope": [] }
}"#,
        )
        .unwrap();

        let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

        // spec.md must be in the changed list
        assert!(
            changed.iter().any(|p| p.ends_with("spec.md")),
            "spec.md should be reported as changed"
        );

        // spec.md must exist and contain the generated header comment and title
        let spec_md = std::fs::read_to_string(track_dir.join("spec.md")).unwrap();
        assert!(spec_md.contains("<!-- Generated from spec.json"));
        assert!(spec_md.contains("Feature Alpha"));
    }

    #[test]
    fn sync_rendered_views_skips_spec_md_when_spec_json_absent() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
            ),
        )
        .unwrap();

        // No spec.json written — legacy mode
        let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

        assert!(
            !changed.iter().any(|p| p.ends_with("spec.md")),
            "spec.md must NOT be in changed list when spec.json is absent"
        );
        assert!(!track_dir.join("spec.md").exists());
    }

    #[test]
    fn sync_rendered_views_does_not_overwrite_spec_md_when_already_up_to_date() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
            ),
        )
        .unwrap();

        let spec_json = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature Beta",
  "scope": { "in_scope": [], "out_of_scope": [] }
}"#;
        std::fs::write(track_dir.join("spec.json"), spec_json).unwrap();

        // First sync — generates spec.md
        sync_rendered_views(dir.path(), Some("track-a")).unwrap();

        // Second sync — spec.md is already up-to-date, must NOT be in changed list
        let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();
        assert!(
            !changed.iter().any(|p| p.ends_with("spec.md")),
            "spec.md must NOT be in changed list when already up-to-date"
        );
    }

    #[test]
    fn sync_rendered_views_continues_on_malformed_spec_json() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
            ),
        )
        .unwrap();

        // Write malformed spec.json (JSON parse error — warn and continue)
        std::fs::write(track_dir.join("spec.json"), "{not valid json}").unwrap();

        // Must succeed (only warn) — plan.md and registry.md are still generated
        let result = sync_rendered_views(dir.path(), Some("track-a"));
        assert!(result.is_ok(), "JSON-parse-error spec.json must not abort sync");

        let changed = result.unwrap();
        assert!(changed.iter().any(|p| p.ends_with("plan.md")));
        assert!(!changed.iter().any(|p| p.ends_with("spec.md")));
    }

    #[test]
    fn sync_rendered_views_propagates_error_on_spec_json_unsupported_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
            ),
        )
        .unwrap();

        // Valid JSON but unsupported schema version — must propagate as an error
        std::fs::write(
            track_dir.join("spec.json"),
            r#"{"schema_version":99,"status":"draft","version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#,
        )
        .unwrap();

        let result = sync_rendered_views(dir.path(), Some("track-a"));
        assert!(result.is_err(), "unsupported spec.json schema version must return an error");
    }

    // ---------------------------------------------------------------------------
    // T011: domain-types.md rendering
    // ---------------------------------------------------------------------------

    const DOMAIN_TYPES_JSON_MINIMAL: &str = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true }
  ]
}"#;

    #[test]
    fn sync_rendered_views_generates_domain_types_md_from_domain_types_json() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
            ),
        )
        .unwrap();
        std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();

        let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

        assert!(
            changed.iter().any(|p| p.ends_with("domain-types.md")),
            "domain-types.md should be reported as changed"
        );

        let md = std::fs::read_to_string(track_dir.join("domain-types.md")).unwrap();
        assert!(md.contains("<!-- Generated from domain-types.json"), "must have generated header");
        assert!(md.contains("TrackId"), "must include declared type name");
    }

    #[test]
    fn sync_rendered_views_skips_domain_types_md_when_domain_types_json_absent() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
            ),
        )
        .unwrap();
        // No domain-types.json

        let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

        assert!(
            !changed.iter().any(|p| p.ends_with("domain-types.md")),
            "domain-types.md must not be generated when domain-types.json is absent"
        );
        assert!(!track_dir.join("domain-types.md").exists());
    }

    #[test]
    fn sync_rendered_views_does_not_overwrite_domain_types_md_when_already_up_to_date() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
            ),
        )
        .unwrap();
        std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();

        // First sync — generates domain-types.md.
        sync_rendered_views(dir.path(), Some("track-a")).unwrap();

        // Second sync — domain-types.md is already up to date, should not appear in changed.
        let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();
        assert!(
            !changed.iter().any(|p| p.ends_with("domain-types.md")),
            "second sync must not report domain-types.md as changed when already up to date"
        );
    }

    #[test]
    fn sync_rendered_views_continues_on_malformed_domain_types_json() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
            ),
        )
        .unwrap();
        // Write malformed domain-types.json (JSON parse error).
        std::fs::write(track_dir.join("domain-types.json"), "{ not valid json }").unwrap();

        let result = sync_rendered_views(dir.path(), Some("track-a"));
        assert!(result.is_ok(), "malformed domain-types.json must not abort sync");

        let changed = result.unwrap();
        assert!(changed.iter().any(|p| p.ends_with("plan.md")));
        assert!(!changed.iter().any(|p| p.ends_with("domain-types.md")));
    }

    #[test]
    fn sync_rendered_views_with_none_refreshes_registry_only() {
        // With `track_id = None` the function now operates in "registry only"
        // mode: it rebuilds track/registry.md from all collected snapshots but
        // does NOT iterate per-track views. Existing plan.md sentinels on
        // other tracks must therefore stay intact, and the bulk mode
        // "render every track under items/ and archive/" is gone.
        let dir = tempfile::tempdir().unwrap();

        // Active track — even with a valid metadata.json, its plan.md must
        // not be generated when `track_id = None` (registry-only mode).
        let active_dir = dir.path().join("track/items/track-active");
        std::fs::create_dir_all(&active_dir).unwrap();
        std::fs::write(
            active_dir.join("metadata.json"),
            sample_metadata_json(
                "track-active",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
            ),
        )
        .unwrap();

        // Done track in items/ — its sentinel plan.md must stay intact.
        let done_dir = dir.path().join("track/items/track-done");
        std::fs::create_dir_all(&done_dir).unwrap();
        std::fs::write(
            done_dir.join("metadata.json"),
            sample_metadata_json(
                "track-done",
                "done",
                "2026-03-10T00:00:00Z",
                r#"[{"id":"T001","description":"Done task","status":"done","commit_hash":"abc1234567890abc1234567890abc1234567890a"}]"#,
            ),
        )
        .unwrap();
        std::fs::write(done_dir.join("plan.md"), "SENTINEL_DONE").unwrap();

        // Archived track in archive/ — registry must still list it, but its
        // plan.md must stay intact.
        let archived_dir = dir.path().join("track/archive/track-archived");
        std::fs::create_dir_all(&archived_dir).unwrap();
        std::fs::write(
            archived_dir.join("metadata.json"),
            sample_metadata_json(
                "track-archived",
                "archived",
                "2026-03-10T00:00:00Z",
                r#"[{"id":"T001","description":"Archived task","status":"done","commit_hash":"def4567890def4567890def4567890def4567890"}]"#,
            ),
        )
        .unwrap();
        std::fs::write(archived_dir.join("plan.md"), "SENTINEL_ARCHIVED").unwrap();

        let changed = sync_rendered_views(dir.path(), None).unwrap();

        // No per-track views should be generated or touched.
        assert!(!active_dir.join("plan.md").exists());
        assert!(!changed.iter().any(|p| p.ends_with("track-active/plan.md")));
        assert_eq!(std::fs::read_to_string(done_dir.join("plan.md")).unwrap(), "SENTINEL_DONE");
        assert!(!changed.iter().any(|p| p.ends_with("track-done/plan.md")));
        assert_eq!(
            std::fs::read_to_string(archived_dir.join("plan.md")).unwrap(),
            "SENTINEL_ARCHIVED"
        );
        assert!(!changed.iter().any(|p| p.ends_with("track-archived/plan.md")));

        // Registry MUST reflect all three tracks (snapshots are collected
        // across items/ and archive/ regardless of rendering mode).
        assert!(changed.iter().any(|p| p.ends_with("registry.md")));
        let registry = std::fs::read_to_string(dir.path().join("track/registry.md")).unwrap();
        assert!(registry.contains("track-active"));
        assert!(registry.contains("track-done"));
        assert!(registry.contains("track-archived"));
    }

    #[test]
    fn sync_rendered_views_single_track_renders_done_track() {
        // Regression: when `track_id=Some(id)` is passed, the caller has
        // explicitly asked to render that track. The done/archived skip is a
        // bulk-sync-only protection and must NOT apply to single-track sync,
        // otherwise the final `in_progress → done` transition of an active
        // track freezes plan.md in its pre-done state (task checkboxes do not
        // flip to `[x]`).
        let dir = tempfile::tempdir().unwrap();

        let done_dir = dir.path().join("track/items/track-done");
        std::fs::create_dir_all(&done_dir).unwrap();
        std::fs::write(
            done_dir.join("metadata.json"),
            sample_metadata_json(
                "track-done",
                "done",
                "2026-03-10T00:00:00Z",
                r#"[{"id":"T001","description":"Done task","status":"done","commit_hash":"abc1234567890abc1234567890abc1234567890a"}]"#,
            ),
        )
        .unwrap();
        // Sentinel content that must be overwritten by the single-track render.
        std::fs::write(done_dir.join("plan.md"), "STALE_SENTINEL_MUST_BE_OVERWRITTEN").unwrap();

        // Single-track path with track_id=Some.
        let changed = sync_rendered_views(dir.path(), Some("track-done")).unwrap();

        // plan.md must be freshly rendered (sentinel overwritten).
        let plan = std::fs::read_to_string(done_dir.join("plan.md")).unwrap();
        assert_ne!(plan, "STALE_SENTINEL_MUST_BE_OVERWRITTEN");
        assert!(plan.contains("Done task"));
        assert!(changed.iter().any(|p| p.ends_with("track-done/plan.md")));
    }

    #[test]
    fn sync_rendered_views_single_track_skips_spec_md_for_done_track() {
        // Regression: single-track rendering must still preserve legacy
        // spec.md content on done/archived tracks to avoid silently
        // overwriting a field an older renderer preserved. Only plan.md is
        // re-rendered unconditionally because it mirrors task state that
        // actually changes during transitions; spec.md reflects spec.json
        // which does NOT change on a task transition.
        let dir = tempfile::tempdir().unwrap();
        let done_dir = dir.path().join("track/items/track-done-spec");
        std::fs::create_dir_all(&done_dir).unwrap();
        std::fs::write(
            done_dir.join("metadata.json"),
            sample_metadata_json(
                "track-done-spec",
                "done",
                "2026-03-10T00:00:00Z",
                r#"[{"id":"T001","description":"Done task","status":"done","commit_hash":"abc1234567890abc1234567890abc1234567890a"}]"#,
            ),
        )
        .unwrap();
        // Minimal spec.json so the render code path is reachable.
        std::fs::write(
            done_dir.join("spec.json"),
            r#"{"schema_version":1,"status":"draft","version":"1.0","title":"Done Feature","scope":{"in_scope":[],"out_of_scope":[]}}"#,
        )
        .unwrap();
        // Sentinel spec.md that must stay intact.
        std::fs::write(done_dir.join("spec.md"), "LEGACY_SPEC_SENTINEL_PRESERVED").unwrap();

        let changed = sync_rendered_views(dir.path(), Some("track-done-spec")).unwrap();

        // spec.md must NOT be re-rendered for a done track.
        let spec = std::fs::read_to_string(done_dir.join("spec.md")).unwrap();
        assert_eq!(spec, "LEGACY_SPEC_SENTINEL_PRESERVED");
        assert!(!changed.iter().any(|p| p.ends_with("spec.md")));

        // plan.md, on the other hand, MUST still have been rendered so
        // the post-transition state is captured.
        assert!(changed.iter().any(|p| p.ends_with("track-done-spec/plan.md")));
    }

    #[test]
    fn sync_rendered_views_single_track_skips_domain_types_md_for_done_track() {
        // Same legacy-protection rationale as the spec.md case above, but
        // for `domain-types.md`.
        let dir = tempfile::tempdir().unwrap();
        let done_dir = dir.path().join("track/items/track-done-domain");
        std::fs::create_dir_all(&done_dir).unwrap();
        std::fs::write(
            done_dir.join("metadata.json"),
            sample_metadata_json(
                "track-done-domain",
                "done",
                "2026-03-10T00:00:00Z",
                r#"[{"id":"T001","description":"Done task","status":"done","commit_hash":"abc1234567890abc1234567890abc1234567890a"}]"#,
            ),
        )
        .unwrap();
        std::fs::write(done_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();
        std::fs::write(done_dir.join("domain-types.md"), "LEGACY_DOMAIN_TYPES_SENTINEL_PRESERVED")
            .unwrap();

        let changed = sync_rendered_views(dir.path(), Some("track-done-domain")).unwrap();

        let domain_types = std::fs::read_to_string(done_dir.join("domain-types.md")).unwrap();
        assert_eq!(domain_types, "LEGACY_DOMAIN_TYPES_SENTINEL_PRESERVED");
        assert!(!changed.iter().any(|p| p.ends_with("domain-types.md")));
        assert!(changed.iter().any(|p| p.ends_with("track-done-domain/plan.md")));
    }
}
