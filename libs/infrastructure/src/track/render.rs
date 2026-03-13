//! Rendering and sync of track read-only views (`plan.md`, `registry.md`) from metadata.json.

use std::path::{Path, PathBuf};

use domain::{TaskStatus, TrackMetadata};

use super::atomic_write::atomic_write_file;
use super::codec::{self, DocumentMeta};

const TRACK_ITEMS_DIR: &str = "track/items";
const TRACK_ARCHIVE_DIR: &str = "track/archive";

/// Track aggregate plus metadata-only fields required for view rendering.
#[derive(Debug, Clone)]
pub struct TrackSnapshot {
    pub track: TrackMetadata,
    pub meta: DocumentMeta,
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
            continue;
        }

        let (track, meta) = codec::decode(&json).map_err(|source| {
            RenderError::InvalidMetadata { path: metadata_path.clone(), source }
        })?;
        snapshots.push(TrackSnapshot { track, meta });
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
        .map(|task| (task.id().as_str(), task))
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
            if let Some(task) = task_map.get(task_id.as_str()) {
                let marker = match task.status() {
                    TaskStatus::Todo => " ",
                    TaskStatus::InProgress => "~",
                    TaskStatus::Done { .. } => "x",
                    TaskStatus::Skipped => "-",
                };
                let suffix = match task.status() {
                    TaskStatus::Done { commit_hash: Some(hash) } => format!(" {hash}"),
                    _ => String::new(),
                };
                lines.push(format!("- [{marker}] {}{suffix}", task.description()));
            }
        }

        lines.push(String::new());
    }

    lines.join("\n")
}

fn next_command_for_status(status: &str) -> &'static str {
    match status {
        "planned" => "`/track:implement`",
        "in_progress" => "`/track:full-cycle <task>`",
        "blocked" => "`/track:status`",
        "cancelled" | "archived" => "`/track:plan <feature>`",
        _ => "`/track:status`",
    }
}

fn format_date(iso_timestamp: &str) -> &str {
    if iso_timestamp.len() >= 10 { &iso_timestamp[..10] } else { iso_timestamp }
}

/// Renders `registry.md` content from all track snapshots.
#[must_use]
pub fn render_registry(tracks: &[TrackSnapshot]) -> String {
    let active: Vec<_> = tracks
        .iter()
        .filter(|track| {
            matches!(track.status().as_str(), "planned" | "in_progress" | "blocked" | "cancelled")
        })
        .collect();
    let completed: Vec<_> = tracks.iter().filter(|track| track.status() == "done").collect();
    let archived: Vec<_> = tracks.iter().filter(|track| track.status() == "archived").collect();

    let mut lines = vec![
        "# Track Registry".to_owned(),
        String::new(),
        "> This file lists all tracks and their current status.".to_owned(),
        "> Auto-updated by `/track:plan` (on approval) and `/track:commit`.".to_owned(),
        "> `/track:status` uses this file as an entry point to summarize progress.".to_owned(),
        "> Each track is expected to have `spec.md` / `plan.md` / `metadata.json` / `verification.md`.".to_owned(),
        String::new(),
        "## Current Focus".to_owned(),
        String::new(),
    ];

    if let Some(latest) = active.first() {
        let status = latest.status();
        lines.push(format!("- Latest active track: `{}`", latest.track.id()));
        lines.push(format!("- Next recommended command: {}", next_command_for_status(&status)));
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
                next_command_for_status(&status),
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
    lines.push("Use `/track:plan <feature>` to start a new feature or bugfix track.".to_owned());
    lines.push(String::new());

    lines.join("\n")
}

/// Validates all metadata documents under the project root.
///
/// # Errors
/// Returns `RenderError` if any metadata file cannot be read or decoded.
pub fn validate_track_snapshots(root: &Path) -> Result<(), RenderError> {
    let _ = collect_track_snapshots(root)?;
    Ok(())
}

/// Renders `plan.md` and `registry.md` from metadata.json and writes changed files atomically.
///
/// # Errors
/// Returns `RenderError` on file-system or metadata decode failure.
pub fn sync_rendered_views(
    root: &Path,
    track_id: Option<&str>,
) -> Result<Vec<PathBuf>, RenderError> {
    let mut changed = Vec::new();
    let items_root = root.join(TRACK_ITEMS_DIR);

    let track_dirs: Vec<PathBuf> = if let Some(track_id) = track_id {
        vec![items_root.join(track_id)]
    } else if items_root.is_dir() {
        let mut dirs = Vec::new();
        for entry in std::fs::read_dir(&items_root)? {
            let entry = entry?;
            if entry.path().is_dir() {
                dirs.push(entry.path());
            }
        }
        dirs.sort();
        dirs
    } else {
        Vec::new()
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
            continue;
        }
        let (track, _) = codec::decode(&json).map_err(|source| RenderError::InvalidMetadata {
            path: metadata_path.clone(),
            source,
        })?;
        let rendered = render_plan(&track);
        let plan_path = track_dir.join("plan.md");
        let old = std::fs::read_to_string(&plan_path).ok();
        if old.as_deref() != Some(rendered.as_str()) {
            atomic_write_file(&plan_path, rendered.as_bytes())?;
            changed.push(plan_path);
        }
    }

    let snapshots = collect_track_snapshots(root)?;
    let rendered_registry = render_registry(&snapshots);
    let registry_path = root.join("track/registry.md");
    if let Some(parent) = registry_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let old = std::fs::read_to_string(&registry_path).ok();
    if old.as_deref() != Some(rendered_registry.as_str()) {
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
        format!(
            r#"{{
  "schema_version": 3,
  "id": "{id}",
  "branch": "track/{id}",
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
            TrackSnapshot { track: active_track, meta: active_meta },
            TrackSnapshot { track: done_track, meta: done_meta },
            TrackSnapshot { track: archived_track, meta: archived_meta },
        ]);

        assert!(rendered.contains("| track-a | planned | `/track:implement` | 2026-03-13 |"));
        assert!(rendered.contains("| track-b | Done | 2026-03-13 |"));
        assert!(rendered.contains("| track-c | Archived | 2026-03-13 |"));
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

        let changed = sync_rendered_views(dir.path(), None).unwrap();

        assert!(changed.iter().any(|path| path.ends_with("plan.md")));
        assert!(changed.iter().any(|path| path.ends_with("registry.md")));
        assert!(track_dir.join("plan.md").is_file());
        assert!(dir.path().join("track/registry.md").is_file());
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
}
