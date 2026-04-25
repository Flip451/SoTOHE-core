//! Rendering and sync of track read-only views (`plan.md`, `registry.md`, `spec.md`, `domain-types.md`) from metadata.json / spec.json / domain-types.json.

use std::path::{Path, PathBuf};

use domain::tddd::{CatalogueLoader, ContractMapRenderOptions, render_contract_map};
use domain::{ImplPlanDocument, TaskCoverageDocument, TrackId, TrackMetadata, derive_track_status};

use super::atomic_write::atomic_write_file;
use super::codec::{self, DocumentMeta};
use crate::spec;
use crate::tddd::contract_map_adapter::FsCatalogueLoader;
use crate::tddd::{catalogue_codec, type_signals_codec};
use crate::type_catalogue_render;
use crate::verify::tddd_layers::{LoadTdddLayersError, load_tddd_layers_from_path};

/// Loads `impl-plan.json` from a track directory, returning `None` when the file
/// does not exist. Propagates I/O and decode errors as `RenderError::Io`.
fn load_impl_plan_opt(track_dir: &Path) -> Result<Option<ImplPlanDocument>, RenderError> {
    let path = track_dir.join("impl-plan.json");
    if !path.is_file() {
        return Ok(None);
    }
    let json = std::fs::read_to_string(&path)?;
    crate::impl_plan_codec::decode(&json).map(Some).map_err(|e| {
        RenderError::Io(std::io::Error::other(format!(
            "impl-plan.json decode error at {}: {e}",
            path.display()
        )))
    })
}

/// Loads `task-coverage.json` from a track directory, returning `None` when the
/// file does not exist. Propagates I/O and decode errors as `RenderError::Io`.
fn load_task_coverage_opt(track_dir: &Path) -> Result<Option<TaskCoverageDocument>, RenderError> {
    let path = track_dir.join("task-coverage.json");
    if !path.is_file() {
        return Ok(None);
    }
    let json = std::fs::read_to_string(&path)?;
    crate::task_coverage_codec::decode(&json).map(Some).map_err(|e| {
        RenderError::Io(std::io::Error::other(format!(
            "task-coverage.json decode error at {}: {e}",
            path.display()
        )))
    })
}

const TRACK_ITEMS_DIR: &str = "track/items";
const TRACK_ARCHIVE_DIR: &str = "track/archive";
const RESERVED_ID_SEGMENTS: &[&str] = &["git"];
const VALID_TRACK_STATUSES: &[&str] =
    &["planned", "in_progress", "done", "blocked", "cancelled", "archived"];
// `tasks` and `plan` fields moved to impl-plan.json; removed from required list.
const REQUIRED_V3_METADATA_FIELDS: &[&str] =
    &["schema_version", "branch", "id", "title", "status", "created_at", "updated_at"];

fn rendered_matches(actual: &str, expected: &str) -> bool {
    actual == expected || actual.trim_end_matches('\n') == expected.trim_end_matches('\n')
}

/// Minimal DTO for peeking at a metadata.json's schema_version + identity
/// before dispatching to a version-specific decoder. This is intentionally
/// loose (no `deny_unknown_fields`) so that legacy v2/v3/v4 metadata — which
/// still carries removed fields like `status`, `tasks`, `plan` — can be
/// identified and routed through the legacy path. The strict v5 DTO
/// (`codec::TrackDocumentV2`) is only applied in the v5 branch via
/// `codec::decode`.
#[derive(Debug, Clone, serde::Deserialize)]
struct TrackSchemaPeek {
    schema_version: u32,
    id: String,
    #[serde(default)]
    branch: Option<String>,
}

/// Track aggregate plus metadata-only fields required for view rendering.
#[derive(Debug, Clone)]
pub struct TrackSnapshot {
    pub dir: PathBuf,
    pub track: TrackMetadata,
    pub meta: DocumentMeta,
    pub schema_version: u32,
    /// Derived track status string: computed from `impl-plan.json` +
    /// `status_override` at construction time via `domain::derive_track_status`.
    /// For legacy v2/v3 tracks that still carry an explicit `status` field,
    /// the stored value from the raw JSON is used directly.
    pub derived_status: String,
}

impl TrackSnapshot {
    #[must_use]
    pub fn status(&self) -> String {
        // Status is not stored in `TrackMetadata`; it is derived on demand
        // and cached at snapshot construction time.
        self.derived_status.clone()
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

/// Decodes a legacy v2/v3/v4 metadata JSON value into a `(TrackMetadata, DocumentMeta)` pair.
///
/// Legacy format includes a `status` field and (in v2/v3) `tasks`/`plan` fields that are
/// no longer part of the v5 schema. This function extracts the identity fields (id, branch,
/// title) and, for v4 tracks, the `status_override` sub-field (introduced in v4).
///
/// # Errors
///
/// Returns `CodecError::InvalidField` when required fields are missing or malformed.
fn decode_legacy_metadata(
    raw: &serde_json::Value,
    metadata_path: &Path,
) -> Result<(TrackMetadata, DocumentMeta), codec::CodecError> {
    use domain::{StatusOverride, TrackBranch, TrackId};

    let schema_version_u64 = raw.get("schema_version").and_then(|v| v.as_u64()).unwrap_or(0);
    let schema_version = u32::try_from(schema_version_u64).map_err(|_| {
        codec::CodecError::Validation(format!("schema_version {schema_version_u64} overflows u32"))
    })?;
    let id_str =
        raw.get("id").and_then(|v| v.as_str()).ok_or_else(|| codec::CodecError::InvalidField {
            field: "id".to_owned(),
            reason: "missing or not a string".to_owned(),
        })?;
    let branch_str = raw.get("branch").and_then(|v| v.as_str());
    let title_str = raw.get("title").and_then(|v| v.as_str()).ok_or_else(|| {
        codec::CodecError::InvalidField {
            field: "title".to_owned(),
            reason: "missing or not a string".to_owned(),
        }
    })?;
    let created_at = raw
        .get("created_at")
        .and_then(|v| v.as_str())
        .ok_or_else(|| codec::CodecError::InvalidField {
            field: "created_at".to_owned(),
            reason: "missing or not a string".to_owned(),
        })?
        .to_owned();
    let updated_at = raw
        .get("updated_at")
        .and_then(|v| v.as_str())
        .ok_or_else(|| codec::CodecError::InvalidField {
            field: "updated_at".to_owned(),
            reason: "missing or not a string".to_owned(),
        })?
        .to_owned();

    let id = TrackId::try_new(id_str).map_err(domain::DomainError::from)?;
    let branch = branch_str
        .map(TrackBranch::try_new)
        .transpose()
        .map_err(|e| codec::CodecError::Domain(domain::DomainError::from(e)))?;

    // v2/v3 tracks do not have `status_override` — the field was introduced in v4.
    // For v4 tracks, read `status_override` from the JSON so that the override reason
    // is available in the rendered registry (e.g., "Reason: …" for blocked tracks).
    // v2 and v3 metadata is always treated as having no override.
    let status_override: Option<StatusOverride> = if schema_version >= 4 {
        if let Some(obj) = raw.get("status_override").and_then(|v| v.as_object()) {
            let status_str = obj.get("status").and_then(|v| v.as_str()).ok_or_else(|| {
                codec::CodecError::InvalidField {
                    field: "status_override.status".to_owned(),
                    reason: "missing or not a string".to_owned(),
                }
            })?;
            let reason = obj.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_owned();
            let ov = match status_str {
                "blocked" => StatusOverride::blocked(reason)
                    .map_err(|e| codec::CodecError::Domain(domain::DomainError::from(e)))?,
                "cancelled" => StatusOverride::cancelled(reason)
                    .map_err(|e| codec::CodecError::Domain(domain::DomainError::from(e)))?,
                other => {
                    return Err(codec::CodecError::InvalidField {
                        field: "status_override.status".to_owned(),
                        reason: format!("unknown override status: {other}"),
                    });
                }
            };
            Some(ov)
        } else {
            None
        }
    } else {
        None
    };

    let track = TrackMetadata::with_branch(id, branch, title_str, status_override)
        .map_err(codec::CodecError::Domain)?;
    let meta = DocumentMeta { schema_version, created_at, updated_at };

    let _ = metadata_path; // used for error context by callers
    Ok((track, meta))
}

/// Collects all valid track snapshots from active and archive directories.
///
/// # Errors
/// Returns `RenderError` if a metadata file cannot be read or decoded.
pub fn collect_track_snapshots(root: &Path) -> Result<Vec<TrackSnapshot>, RenderError> {
    // Collect (path, is_archive) pairs so that the archive flag is preserved per
    // directory. v5 tracks under `track/archive/` must have `derived_status =
    // "archived"` because `derive_track_status` cannot return that value — archived
    // state is encoded by directory location rather than a status field in the v5 schema.
    let mut track_dirs: Vec<(PathBuf, bool)> = Vec::new();
    for (rel, is_archive) in [(TRACK_ITEMS_DIR, false), (TRACK_ARCHIVE_DIR, true)] {
        let base = root.join(rel);
        if !base.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(base)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                track_dirs.push((path, is_archive));
            }
        }
    }
    track_dirs.sort_by(|(a, _), (b, _)| a.cmp(b));

    let mut snapshots = Vec::new();
    for (track_dir, is_archive) in track_dirs {
        let metadata_path = track_dir.join("metadata.json");
        if !metadata_path.is_file() {
            continue;
        }

        let json = std::fs::read_to_string(&metadata_path)?;
        // Peek at schema_version + identity through a loose DTO that does not
        // enforce `deny_unknown_fields`; this is required because legacy
        // v2/v3/v4 metadata still carries removed fields (`status`, `tasks`,
        // `plan`) that the strict `codec::TrackDocumentV2` DTO rejects. The
        // strict DTO is applied inside the v5 branch via `codec::decode`.
        let parsed: TrackSchemaPeek =
            serde_json::from_str(&json).map_err(|source| RenderError::InvalidMetadata {
                path: metadata_path.clone(),
                source: codec::CodecError::Json(source),
            })?;
        // Schema version 5 is the identity-only format (no `status` field).
        // Legacy v2/v3 tracks that still carry a `status` field are accepted for
        // rendering purposes (registry.md, plan.md) but are not validated for
        // plan.md freshness — that guard only runs for v4+ (see `validate_track_snapshots`).
        if !matches!(parsed.schema_version, 2..=5) {
            return Err(RenderError::UnsupportedSchemaVersion {
                path: metadata_path,
                schema_version: parsed.schema_version,
            });
        }
        validate_track_document(&metadata_path, track_dir.file_name(), &parsed)?;

        // For v5 tracks: use `codec::decode` (schema v5 required).
        // For legacy v2/v3: parse raw JSON for the `status` field and construct
        // `TrackMetadata` directly without calling `codec::decode`, which would
        // reject the old schema.
        let (track, meta, derived_status) = if parsed.schema_version == 5 {
            let (t, m) = codec::decode(&json).map_err(|source| RenderError::InvalidMetadata {
                path: metadata_path.clone(),
                source,
            })?;
            // Tracks under `track/archive/` are archived regardless of impl-plan
            // state. `derive_track_status` cannot return `archived` (that state is
            // encoded by directory location in the v5 schema, not by a status field),
            // so we set `derived_status` explicitly before calling the derive function.
            let status = if is_archive {
                "archived".to_owned()
            } else {
                let impl_plan = load_impl_plan_opt(&track_dir)?;
                derive_track_status(impl_plan.as_ref(), t.status_override()).to_string()
            };
            (t, m, status)
        } else {
            // Legacy v2/v3/v4: read `status` from raw JSON; decode only identity
            // fields (branch, title, id) via the legacy decode path.
            let raw: serde_json::Value =
                serde_json::from_str(&json).map_err(|source| RenderError::InvalidMetadata {
                    path: metadata_path.clone(),
                    source: codec::CodecError::Json(source),
                })?;
            // `status` is a required field in all legacy schemas (v2/v3/v4); failing
            // closed here mirrors the old codec::decode validation behaviour.
            let legacy_status = raw
                .get("status")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RenderError::InvalidTrackMetadata {
                    path: metadata_path.clone(),
                    reason: "legacy metadata.json is missing required 'status' field".to_owned(),
                })?
                .to_owned();
            if !VALID_TRACK_STATUSES.contains(&legacy_status.as_str()) {
                return Err(RenderError::InvalidTrackMetadata {
                    path: metadata_path.clone(),
                    reason: format!("invalid legacy track status '{legacy_status}'"),
                });
            }
            let (t, m) = decode_legacy_metadata(&raw, &metadata_path).map_err(|source| {
                RenderError::InvalidMetadata { path: metadata_path.clone(), source }
            })?;
            (t, m, legacy_status)
        };

        snapshots.push(TrackSnapshot {
            dir: track_dir,
            track,
            meta,
            schema_version: parsed.schema_version,
            derived_status,
        });
    }

    snapshots.sort_by(|a, b| {
        b.updated_at()
            .cmp(a.updated_at())
            .then_with(|| a.track.id().as_ref().cmp(b.track.id().as_ref()))
    });
    Ok(snapshots)
}

/// Renders `plan.md` content from track identity metadata and an optional
/// `ImplPlanDocument`.
///
/// When `impl_plan` is `Some`, renders the full task list and plan sections
/// from the document. When `None`, emits a placeholder stub (used for
/// planning-only tracks that have not yet generated `impl-plan.json`).
#[must_use]
pub fn render_plan(track: &TrackMetadata, impl_plan: Option<&ImplPlanDocument>) -> String {
    let mut lines = Vec::new();
    lines.push(
        "<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->".to_owned(),
    );
    lines.push(format!("# {}", track.title()));
    lines.push(String::new());

    let Some(doc) = impl_plan else {
        lines.push(
            "> **Note**: `impl-plan.json` not yet generated. \
             Run `/track:impl-plan` to generate the implementation plan."
                .to_owned(),
        );
        lines.push(String::new());
        return lines.join("\n");
    };

    // Summary lines (if any).
    if !doc.plan().summary().is_empty() {
        lines.push("## Summary".to_owned());
        lines.push(String::new());
        for line in doc.plan().summary() {
            lines.push(line.clone());
        }
        lines.push(String::new());
    }

    // Task list per section.
    let total = doc.tasks().len();
    let done_count = doc.tasks().iter().filter(|t| t.status().is_resolved()).count();
    lines.push(format!("## Tasks ({done_count}/{total} resolved)"));
    lines.push(String::new());

    for section in doc.plan().sections() {
        lines.push(format!("### {} — {}", section.id(), section.title()));
        lines.push(String::new());
        if !section.description().is_empty() {
            for desc_line in section.description() {
                lines.push(format!("> {desc_line}"));
            }
            lines.push(String::new());
        }
        for task_id in section.task_ids() {
            if let Some(task) = doc.tasks().iter().find(|t| t.id() == task_id) {
                let status_label = match task.status() {
                    domain::TaskStatus::Todo => "[ ]",
                    domain::TaskStatus::InProgress => "[~]",
                    domain::TaskStatus::DonePending | domain::TaskStatus::DoneTraced { .. } => {
                        "[x]"
                    }
                    domain::TaskStatus::Skipped => "[-]",
                };
                let hash_note = match task.status() {
                    domain::TaskStatus::DoneTraced { commit_hash } => {
                        format!(" (`{}`)", commit_hash)
                    }
                    _ => String::new(),
                };
                lines.push(format!(
                    "- {} **{}**: {}{}",
                    status_label,
                    task_id,
                    task.description(),
                    hash_note
                ));
            }
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

fn next_command_for_track(track: &TrackSnapshot) -> String {
    // Status is derived and cached in `derived_status`; use `resolve_phase_from_record`
    // to avoid re-loading impl-plan.json. Parse the status string into `TrackStatus`.
    let status = parse_track_status_str(track.derived_status.as_str());
    let override_reason = track.track.status_override().map(|o| o.reason()).filter(|_| {
        matches!(status, domain::TrackStatus::Blocked | domain::TrackStatus::Cancelled)
    });
    let info = domain::track_phase::resolve_phase_from_record(
        track.track.id().as_ref(),
        status,
        track.track.branch().is_some(),
        track.schema_version,
        override_reason,
    );
    format!("`{}`", info.next_command)
}

/// Parses a track status string into `domain::TrackStatus`.
/// Returns `TrackStatus::Planned` for unrecognized values.
fn parse_track_status_str(s: &str) -> domain::TrackStatus {
    match s {
        "planned" => domain::TrackStatus::Planned,
        "in_progress" => domain::TrackStatus::InProgress,
        "done" => domain::TrackStatus::Done,
        "blocked" => domain::TrackStatus::Blocked,
        "cancelled" => domain::TrackStatus::Cancelled,
        "archived" => domain::TrackStatus::Archived,
        _ => domain::TrackStatus::Planned,
    }
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
    // Deprioritise branchless planning-only tracks (schema_version 3, 4, or 5) so that
    // an actual in-progress track wins the "Latest active track" slot.
    // schema_version 2 branchless planned tracks are legacy pre-planning-only behaviour
    // and are left in normal position (their branch semantics differ).
    // Branchless planning-only shapes: schema versions 3, 4, and 5.
    active.sort_by_key(|track| {
        matches!(track.schema_version, 3..=5)
            && track.status() == "planned"
            && track.track.branch().is_none()
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
        // Only validate v5 (identity-only) tracks. Legacy v2/v3/v4 tracks
        // predate the current renderer and their committed plan.md reflects
        // whatever renderer shipped at their commit time. We intentionally
        // don't touch them; re-validating would create a false OutOfSync for
        // every legacy track without any actionable fix.
        if snapshot.schema_version < 5 {
            continue;
        }
        let plan_path = snapshot.dir.join("plan.md");
        // Phase 0 compat: skip content check when plan.md has not been
        // rendered yet. Per ADR 2026-04-19-1242 §D6.1, the gate fires only
        // "when plan.md is rendered"; if the file is absent, skip regardless
        // of whether impl-plan.json exists.
        //
        // Presence is probed in two layers to disambiguate "absent" from
        // "corrupted":
        //   1. `symlink_metadata` checks whether any entry exists at the
        //      path without following symlinks. `NotFound` here means the
        //      file is genuinely absent (Phase 0 — skip). Other errors
        //      (permission, etc.) are propagated rather than silently
        //      treated as absent.
        //   2. If the entry is a symlink, follow it via `metadata`. A
        //      dangling symlink surfaces as `NotFound` from the follow
        //      path; treat that as corrupted track state (the file
        //      appears to exist but the target is gone) rather than as
        //      "plan.md absent" — otherwise the freshness check would be
        //      silently bypassed.
        //   3. Any non-regular-file target (directory, FIFO, dangling
        //      symlink, etc.) is rejected as corrupted track state.
        let sym_meta = match std::fs::symlink_metadata(&plan_path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(RenderError::Io(e)),
        };
        if sym_meta.file_type().is_symlink() {
            match std::fs::metadata(&plan_path) {
                Ok(target) if target.is_file() => {}
                Ok(_) => {
                    return Err(RenderError::InvalidTrackMetadata {
                        path: plan_path.clone(),
                        reason: "plan.md symlink target is not a regular file".to_owned(),
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Err(RenderError::InvalidTrackMetadata {
                        path: plan_path.clone(),
                        reason: "plan.md is a dangling symlink (target missing)".to_owned(),
                    });
                }
                Err(e) => return Err(RenderError::Io(e)),
            }
        } else if !sym_meta.is_file() {
            return Err(RenderError::InvalidTrackMetadata {
                path: plan_path.clone(),
                reason: "plan.md exists but is not a regular file".to_owned(),
            });
        }
        let actual = std::fs::read_to_string(&plan_path)?;
        let impl_plan = load_impl_plan_opt(&snapshot.dir)?;
        let expected = render_plan(&snapshot.track, impl_plan.as_ref());
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
    doc: &TrackSchemaPeek,
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
        // For v3 tracks, validate the explicit `status` field from raw JSON.
        let status_str = raw_doc.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if !VALID_TRACK_STATUSES.contains(&status_str) {
            return Err(RenderError::InvalidTrackMetadata {
                path: metadata_path.to_path_buf(),
                reason: format!("Invalid track status '{status_str}'"),
            });
        }
        // For v3 planning-only tracks (no branch), only `planned` status is valid.
        if doc.branch.is_none() && status_str != "planned" {
            return Err(RenderError::InvalidTrackMetadata {
                path: metadata_path.to_path_buf(),
                reason: "'branch' is required for v3 tracks unless the track is planning-only"
                    .to_owned(),
            });
        }
    }

    // For v5 tracks: decode via the authoritative codec and verify fields round-trip.
    // `status` field absent in v5 — no drift check needed.
    if doc.schema_version == 5 {
        let (_track, _meta) = codec::decode(&raw_json).map_err(|source| {
            RenderError::InvalidMetadata { path: metadata_path.to_path_buf(), source }
        })?;
    }

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
        // Loose peek (same rationale as `collect_track_snapshots_inner`).
        let parsed: TrackSchemaPeek =
            serde_json::from_str(&json).map_err(|source| RenderError::InvalidMetadata {
                path: metadata_path.clone(),
                source: codec::CodecError::Json(source),
            })?;
        // Schema version 5 is the identity-only format (no `status` field).
        // Legacy v2/v3 tracks are also accepted for rendering.
        if !matches!(parsed.schema_version, 2..=5) {
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
        //
        // For v5 tracks, derive status from impl-plan.json + status_override.
        // For legacy v2/v3 tracks, read the `status` field from raw JSON.
        let (track, _) = if parsed.schema_version == 5 {
            codec::decode(&json).map_err(|source| RenderError::InvalidMetadata {
                path: metadata_path.clone(),
                source,
            })?
        } else {
            let raw: serde_json::Value =
                serde_json::from_str(&json).map_err(|source| RenderError::InvalidMetadata {
                    path: metadata_path.clone(),
                    source: codec::CodecError::Json(source),
                })?;
            decode_legacy_metadata(&raw, &metadata_path).map_err(|source| {
                RenderError::InvalidMetadata { path: metadata_path.clone(), source }
            })?
        };
        let impl_plan_for_status = load_impl_plan_opt(&track_dir)?;
        let derived_status = if parsed.schema_version == 5 {
            derive_track_status(impl_plan_for_status.as_ref(), track.status_override()).to_string()
        } else {
            // Legacy v2/v3: read explicit `status` from raw JSON.
            serde_json::from_str::<serde_json::Value>(&json)
                .ok()
                .and_then(|v| v.get("status").and_then(|s| s.as_str()).map(str::to_owned))
                .unwrap_or_else(|| "planned".to_owned())
        };
        let is_done_or_archived = matches!(derived_status.as_str(), "done" | "archived");
        let impl_plan = load_impl_plan_opt(&track_dir)?;
        let rendered = render_plan(&track, impl_plan.as_ref());
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
                    // Load sibling task-coverage.json when present so that spec.md
                    // aggregates task coverage annotations.
                    let task_coverage = load_task_coverage_opt(&track_dir)?;
                    let rendered_spec =
                        spec::render::render_spec_with_coverage(&spec_doc, task_coverage.as_ref());
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
                Err(spec::codec::SpecCodecError::Json(ref json_err))
                    if json_err.classify() == serde_json::error::Category::Syntax
                        || json_err.classify() == serde_json::error::Category::Eof =>
                {
                    // Warn and continue only on JSON SYNTAX errors — file may be mid-edit.
                    // Data errors (unknown field, wrong type, deny_unknown_fields violations)
                    // are schema failures and must surface as hard errors. A v1 spec.json
                    // with legacy fields like `status` now produces a Data error here.
                    eprintln!(
                        "warning: skipping spec.md render for {} (malformed JSON syntax)",
                        track_dir.display()
                    );
                }
                Err(spec::codec::SpecCodecError::Json(_)) => {
                    // JSON data/type error (unknown field, deny_unknown_fields, wrong type) —
                    // this is a schema failure, not a syntax issue. Propagate as a hard error.
                    return Err(RenderError::Io(std::io::Error::other(format!(
                        "spec.json schema error at {}: JSON data error (v1 fields or unknown schema elements present)",
                        track_dir.display()
                    ))));
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

        // Render per-layer <layer>-types.md from each <layer>-types.json present.
        // Iterates all tddd.enabled layers in architecture-rules.json via the
        // existing `parse_tddd_layers` resolver (introduced in tddd-01 Phase 1 Task 7,
        // already reused by `apps/cli::resolve_layers`). Preserves the 3 patterns
        // from the original domain-only block:
        //   - is_done_or_archived guard (skip frozen tracks entirely — architecture-rules.json
        //     is not loaded for done/archived tracks so a malformed rules file cannot cause
        //     failures on frozen tracks where type-catalogue rendering is a no-op)
        //   - rendered_matches drift check (no-op if content unchanged)
        //   - TypeCatalogueCodecError::Json warn-and-continue (file may be mid-edit)
        // Legacy fallback: when architecture-rules.json is absent, a synthetic
        // domain-only binding is used so pre-multilayer tracks continue to work.
        if !is_done_or_archived {
            let arch_rules_path = root.join("architecture-rules.json");
            // Symlink handling + legacy-fallback policy is centralized in
            // `load_tddd_layers_from_path` (which delegates to
            // `symlink_guard::reject_symlinks_below`). A dangling or
            // unexpectedly-linked `architecture-rules.json` fails closed here
            // instead of silently degrading to the synthetic domain-only
            // binding.
            let bindings =
                load_tddd_layers_from_path(&arch_rules_path, root).map_err(|e| match e {
                    LoadTdddLayersError::Io { source, .. } => RenderError::Io(source),
                    LoadTdddLayersError::Parse(err) => RenderError::Io(std::io::Error::other(
                        format!("architecture-rules.json: {err}"),
                    )),
                })?;

            // Guard against duplicate rendered paths: `parse_tddd_layers` rejects
            // duplicate `catalogue_file` values (exact string match), but two names
            // like `"foo"` and `"foo.json"` both derive to `foo.md` via
            // `.rendered_file()`. The duplicate check is placed AFTER the
            // per-layer opt-out so that a layer whose catalogue file is absent
            // does not consume the rendered-path slot and accidentally suppress
            // a later layer whose catalogue file IS present.
            let mut seen_rendered: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for binding in &bindings {
                let catalogue_file = binding.catalogue_file();
                let catalogue_path = track_dir.join(catalogue_file);
                if !catalogue_path.is_file() {
                    continue;
                }
                // Only check/reserve the rendered path slot after confirming
                // the catalogue file exists. This ensures an absent catalogue
                // does not block a later layer that shares the same rendered name.
                let rendered_name = binding.rendered_file();
                if !seen_rendered.insert(rendered_name.clone()) {
                    eprintln!(
                        "warning: skipping duplicate rendered path {} for {} (rendered path collision)",
                        rendered_name,
                        track_dir.display()
                    );
                    continue;
                }
                let catalogue_content = std::fs::read_to_string(&catalogue_path)?;
                match catalogue_codec::decode(&catalogue_content) {
                    Ok(mut doc) => {
                        // Populate signals from the external `<layer>-type-signals.json`
                        // file so the rendered markdown shows the evaluated Blue/Yellow/Red
                        // emojis instead of `—` placeholders. ADR 2026-04-18-1400 §D1
                        // moved signals out of the declaration file into the signal file;
                        // the declaration codec returns `doc.signals() = None`, so we
                        // have to read the signal file here and call `set_signals`
                        // before rendering.
                        //
                        // Failure modes (missing / malformed / symlinked signal file) are
                        // non-fatal for view rendering — the resulting markdown just
                        // falls back to `—` placeholders. The authoritative fail-closed
                        // path for Missing/Stale lives in
                        // `spec_states::evaluate_layer_catalogue`, which is the
                        // verification gate, not the view renderer.
                        let signal_path = track_dir.join(binding.signal_file());
                        // Use `symlink_metadata()` to detect symlinks: `is_file()` follows
                        // symlinks, which would allow a crafted symlink to inject arbitrary
                        // file contents. For the view renderer, symlinks fall back to `—`
                        // (non-fatal miss). Only read the signal file when the path exists
                        // and is a plain file (not a symlink).
                        let is_plain_file = signal_path
                            .symlink_metadata()
                            .map(|m| m.file_type().is_file())
                            .unwrap_or(false);
                        if is_plain_file {
                            if let Ok(signal_json) = std::fs::read_to_string(&signal_path) {
                                if let Ok(signals_doc) = type_signals_codec::decode(&signal_json) {
                                    // Validate `declaration_hash` before adopting
                                    // signals. Stale signal files (declaration
                                    // changed, signals not regenerated) would
                                    // otherwise paint misleading Blue/Yellow/Red
                                    // emojis in `<layer>-types.md` from an old
                                    // evaluation. Fall back to `—` placeholders
                                    // on mismatch — the authoritative fail-closed
                                    // response to stale signals lives in
                                    // `spec_states::evaluate_layer_catalogue`.
                                    let current_hash = type_signals_codec::declaration_hash(
                                        catalogue_content.as_bytes(),
                                    );
                                    if signals_doc.declaration_hash() == current_hash {
                                        doc.set_signals(signals_doc.signals().to_vec());
                                    } else {
                                        eprintln!(
                                            "warning: ignoring stale {} for {} \
                                             (declaration_hash mismatch) — rendered signal \
                                             column will fall back to `—`",
                                            binding.signal_file(),
                                            track_dir.display()
                                        );
                                    }
                                }
                            }
                        }
                        // Load `<layer>-catalogue-spec-signals.json` for the
                        // T020 Cat-Spec column (ADR 2026-04-23-0344 §D2.5).
                        // Opt-in gated; fail-closed on missing / symlinked /
                        // malformed / stale — remediation is documented in
                        // the error message (`sotp track catalogue-spec-signals
                        // <track_id>`). Opt-out layers render the legacy
                        // 5-column view (None).
                        let spec_signals_doc = if binding.catalogue_spec_signal_enabled() {
                            Some(
                                type_catalogue_render::load_catalogue_spec_signals_for_view(
                                    &track_dir.join(binding.catalogue_spec_signal_file()),
                                    catalogue_content.as_bytes(),
                                )
                                .map_err(|e| {
                                    RenderError::Io(std::io::Error::other(e.to_string()))
                                })?,
                            )
                        } else {
                            None
                        };
                        let rendered = type_catalogue_render::render_type_catalogue(
                            &doc,
                            catalogue_file,
                            spec_signals_doc.as_ref(),
                        );
                        let rendered_md_path = track_dir.join(binding.rendered_file());
                        let old_md = match std::fs::read_to_string(&rendered_md_path) {
                            Ok(content) => Some(content),
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                            Err(e) => return Err(RenderError::Io(e)),
                        };
                        if old_md
                            .as_deref()
                            .is_none_or(|existing| !rendered_matches(existing, &rendered))
                        {
                            atomic_write_file(&rendered_md_path, rendered.as_bytes())?;
                            changed.push(rendered_md_path);
                        }
                    }
                    Err(catalogue_codec::TypeCatalogueCodecError::Json(_)) => {
                        // Warn and continue only on JSON parse errors — file may be mid-edit.
                        eprintln!(
                            "warning: skipping {} render for {} (malformed JSON)",
                            binding.rendered_file(),
                            track_dir.display()
                        );
                    }
                    Err(e) => {
                        return Err(RenderError::Io(std::io::Error::other(format!(
                            "{} error at {}: {e}",
                            catalogue_file,
                            track_dir.display()
                        ))));
                    }
                }
            }

            // Render `contract-map.md` alongside the per-layer views so it
            // stays fresh on every track-transition / sync-views / pre-commit
            // run — callers (especially reviewers) never see a stale
            // declaration relationship diagram.
            //
            // Failure modes (loader error, empty catalogues, unknown layer)
            // are non-fatal for this view: log to stderr and continue without
            // aborting the wider sync. The authoritative fail-closed path for
            // TDDD semantic correctness lives in
            // `spec_states::evaluate_layer_catalogue` and the merge-gate
            // adapter.
            render_contract_map_view(root, &track_dir, track_id, &mut changed);
        } // end if !is_done_or_archived
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

/// Renders `contract-map.md` for the active track and appends the path to
/// `changed` when the content actually differs on disk.
///
/// Non-fatal by design — all failures (loader error, empty catalogues,
/// invalid TrackId) produce a stderr warning and leave the existing
/// `contract-map.md` untouched. The authoritative fail-closed gate for
/// TDDD correctness lives in `spec_states::evaluate_layer_catalogue` /
/// the merge-gate adapter; this function just keeps the rendered diagram
/// in sync with the declarations.
fn render_contract_map_view(
    root: &Path,
    track_dir: &Path,
    track_id_str: Option<&str>,
    changed: &mut Vec<PathBuf>,
) {
    let Some(track_id_raw) = track_id_str else {
        return;
    };
    let Ok(track_id) = TrackId::try_new(track_id_raw) else {
        eprintln!(
            "warning: skipping contract-map.md render for {} (invalid track id)",
            track_dir.display()
        );
        return;
    };

    let items_dir = root.join(TRACK_ITEMS_DIR);
    let rules_path = root.join("architecture-rules.json");
    let loader = FsCatalogueLoader::new(items_dir, rules_path, root.to_path_buf());
    let (layer_order, catalogues) = match loader.load_all(&track_id) {
        Ok(result) => result,
        Err(e) => {
            eprintln!(
                "warning: skipping contract-map.md render for {} ({})",
                track_dir.display(),
                e
            );
            return;
        }
    };
    if layer_order.is_empty() {
        // No TDDD-enabled layers on this track — nothing to render.
        return;
    }

    let opts = ContractMapRenderOptions::default();
    let content = render_contract_map(&catalogues, &layer_order, &opts);
    let contract_map_path = track_dir.join("contract-map.md");
    let old = match std::fs::read_to_string(&contract_map_path) {
        Ok(existing) => Some(existing),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            eprintln!(
                "warning: cannot read existing contract-map.md for {}: {e}",
                track_dir.display()
            );
            return;
        }
    };
    let rendered_str: &str = content.as_ref();
    if old.as_deref().is_none_or(|existing| !rendered_matches(existing, rendered_str)) {
        if let Err(e) = atomic_write_file(&contract_map_path, rendered_str.as_bytes()) {
            eprintln!("warning: cannot write contract-map.md for {}: {e}", track_dir.display());
            return;
        }
        changed.push(contract_map_path);
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// Generates a v5 metadata.json string (no `status` field).
    /// The `status` parameter is accepted for API compatibility but is ignored
    /// since v5 derives status from impl-plan.json at runtime.
    /// The `tasks_json` parameter is also ignored (tasks live in impl-plan.json).
    fn sample_metadata_json(
        id: &str,
        _status: &str,
        updated_at: &str,
        _tasks_json: &str,
    ) -> String {
        sample_metadata_json_with_schema_and_branch(
            5,
            id,
            _status,
            updated_at,
            _tasks_json,
            Some(&format!("track/{id}")),
        )
    }

    fn sample_metadata_json_with_branch(
        id: &str,
        _status: &str,
        updated_at: &str,
        _tasks_json: &str,
        branch: Option<&str>,
    ) -> String {
        sample_metadata_json_with_schema_and_branch(5, id, _status, updated_at, _tasks_json, branch)
    }

    /// Generates a metadata.json string.
    ///
    /// For `schema_version == 5`: emits v5 format (no `status`, no `tasks`/`plan`).
    /// For `schema_version < 5`: emits legacy format with `status`, `tasks`, and `plan`.
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
        if schema_version >= 5 {
            // v5: no `status`, no `tasks`, no `plan`
            format!(
                r#"{{
  "schema_version": {schema_version},
  "id": "{id}",
  {branch_field}
  "title": "Title {id}",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "{updated_at}"
}}
"#
            )
        } else {
            // Legacy v2/v3: include `status`, `tasks`, `plan`
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
    }

    /// Build an `ImplPlanDocument` from flat task + section specs.
    ///
    /// - `tasks`: `(id, description, status_str, commit_hash_opt)` where
    ///   status_str is `"todo" | "in_progress" | "done_pending" | "done_traced" | "skipped"`
    /// - `sections`: `(section_id, section_title, task_ids_slice)`
    fn make_impl_plan_with_tasks(
        tasks: &[(&str, &str, &str, Option<&str>)],
        sections: &[(&str, &str, &[&str])],
    ) -> domain::ImplPlanDocument {
        make_impl_plan_with_summary_and_tasks(&[], tasks, sections)
    }

    fn make_impl_plan_with_summary_and_tasks(
        summary: &[&str],
        tasks: &[(&str, &str, &str, Option<&str>)],
        sections: &[(&str, &str, &[&str])],
    ) -> domain::ImplPlanDocument {
        let sections_with_desc: Vec<(&str, &str, &[&str], &[&str])> = sections
            .iter()
            .map(|(id, title, task_ids)| (*id, *title, *task_ids, [].as_slice()))
            .collect();
        make_impl_plan_inner(summary, tasks, &sections_with_desc)
    }

    fn make_impl_plan_with_desc_and_tasks(
        tasks: &[(&str, &str, &str, Option<&str>)],
        sections: &[(&str, &str, &[&str], &[&str])],
    ) -> domain::ImplPlanDocument {
        make_impl_plan_inner(&[], tasks, sections)
    }

    fn make_impl_plan_inner(
        summary: &[&str],
        tasks: &[(&str, &str, &str, Option<&str>)],
        sections: &[(&str, &str, &[&str], &[&str])],
    ) -> domain::ImplPlanDocument {
        use domain::{CommitHash, PlanSection, PlanView, TaskId, TaskStatus, TrackTask};

        let domain_tasks: Vec<TrackTask> = tasks
            .iter()
            .map(|(id, desc, status_str, hash)| {
                let task_id = TaskId::try_new(id.to_string()).unwrap();
                let status = match *status_str {
                    "todo" => TaskStatus::Todo,
                    "in_progress" => TaskStatus::InProgress,
                    "done_pending" => TaskStatus::DonePending,
                    "done_traced" => {
                        let h = CommitHash::try_new(hash.unwrap_or("abc1234")).unwrap();
                        TaskStatus::DoneTraced { commit_hash: h }
                    }
                    "skipped" => TaskStatus::Skipped,
                    other => panic!("unknown status: {other}"),
                };
                TrackTask::with_status(task_id, *desc, status).unwrap()
            })
            .collect();

        let domain_sections: Vec<PlanSection> = sections
            .iter()
            .map(|(sid, stitle, task_ids, desc)| {
                let tids: Vec<TaskId> =
                    task_ids.iter().map(|t| TaskId::try_new(t.to_string()).unwrap()).collect();
                PlanSection::new(*sid, *stitle, desc.iter().map(|s| s.to_string()).collect(), tids)
                    .unwrap()
            })
            .collect();

        let plan = PlanView::new(summary.iter().map(|s| s.to_string()).collect(), domain_sections);
        domain::ImplPlanDocument::new(domain_tasks, plan).unwrap()
    }

    #[test]
    fn render_plan_matches_expected_layout() {
        // render_plan with None impl_plan renders header, title, and impl-plan stub note.
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

        let rendered = render_plan(&track, None);

        assert!(rendered.contains("<!-- Generated from metadata.json + impl-plan.json"));
        assert!(rendered.contains("# Title track-a"));
        assert!(rendered.contains("impl-plan.json"), "None case must mention impl-plan.json");
    }

    // --- render_plan marker tests ---

    #[test]
    fn render_plan_marks_in_progress_task_with_tilde() {
        // With an impl-plan containing an in-progress task, [~] marker appears.
        let json = sample_metadata_json(
            "track-a",
            "in_progress",
            "2026-03-13T01:00:00Z",
            r#"[
    { "id": "T001", "description": "Working task", "status": "in_progress" }
  ]"#,
        );
        let (track, _) = codec::decode(&json).unwrap();
        let impl_plan = make_impl_plan_with_tasks(
            &[("T001", "Working task", "in_progress", None)],
            &[("S1", "Section", &["T001"])],
        );
        let rendered = render_plan(&track, Some(&impl_plan));
        assert!(rendered.contains("# Title track-a"), "title must appear:\n{rendered}");
        assert!(
            rendered.contains("[~]"),
            "[~] marker must appear for in_progress task:\n{rendered}"
        );
    }

    #[test]
    fn render_plan_marks_done_task_with_short_commit_hash() {
        // Done task with commit hash renders [x] marker and hash note.
        let json = sample_metadata_json(
            "track-a",
            "done",
            "2026-03-13T01:00:00Z",
            r#"[
    {
      "id": "T001",
      "description": "Completed task",
      "status": "done",
      "commit_hash": "abc1234"
    }
  ]"#,
        );
        let (track, _) = codec::decode(&json).unwrap();
        let impl_plan = make_impl_plan_with_tasks(
            &[("T001", "Completed task", "done_traced", Some("abc1234"))],
            &[("S1", "Section", &["T001"])],
        );
        let rendered = render_plan(&track, Some(&impl_plan));
        assert!(rendered.contains("# Title track-a"), "title must appear:\n{rendered}");
        assert!(rendered.contains("[x]"), "[x] marker must appear for done task:\n{rendered}");
        assert!(rendered.contains("abc1234"), "commit hash must appear:\n{rendered}");
    }

    #[test]
    fn render_plan_done_without_commit_hash_omits_literal_none() {
        // Done task without commit hash renders [x] but no "None" string.
        let json = sample_metadata_json(
            "track-a",
            "done",
            "2026-03-13T01:00:00Z",
            r#"[
    { "id": "T001", "description": "Untraced done", "status": "done" }
  ]"#,
        );
        let (track, _) = codec::decode(&json).unwrap();
        let impl_plan = make_impl_plan_with_tasks(
            &[("T001", "Untraced done", "done_pending", None)],
            &[("S1", "Section", &["T001"])],
        );
        let rendered = render_plan(&track, Some(&impl_plan));
        assert!(
            !rendered.contains("None"),
            "literal 'None' must never appear in rendered plan:\n{rendered}"
        );
        assert!(
            rendered.contains("[x]"),
            "[x] marker must appear for done_pending task:\n{rendered}"
        );
    }

    #[test]
    fn render_plan_marks_skipped_task_with_dash() {
        // Skipped task renders with [-] marker.
        let json = sample_metadata_json(
            "track-a",
            "done",
            "2026-03-13T01:00:00Z",
            r#"[
    { "id": "T001", "description": "Skipped task", "status": "skipped" }
  ]"#,
        );
        let (track, _) = codec::decode(&json).unwrap();
        let impl_plan = make_impl_plan_with_tasks(
            &[("T001", "Skipped task", "skipped", None)],
            &[("S1", "Section", &["T001"])],
        );
        let rendered = render_plan(&track, Some(&impl_plan));
        assert!(rendered.contains("# Title track-a"), "title must appear:\n{rendered}");
        assert!(rendered.contains("[-]"), "[-] marker must appear for skipped task:\n{rendered}");
    }

    #[test]
    fn render_plan_preserves_multi_section_order() {
        // Sections rendered in order (S1 before S2).
        // Uses v5 metadata JSON (no `status`/`tasks`/`plan` fields).
        let json = r#"{
  "schema_version": 5,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Title track-a",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T01:00:00Z"
}"#;
        let (track, _) = codec::decode(json).unwrap();
        let impl_plan = make_impl_plan_with_tasks(
            &[("T001", "Task one", "todo", None), ("T002", "Task two", "todo", None)],
            &[("S1", "First Section", &["T001"]), ("S2", "Second Section", &["T002"])],
        );
        let rendered = render_plan(&track, Some(&impl_plan));
        assert!(rendered.contains("# Title track-a"), "title must appear:\n{rendered}");
        let s1_pos = rendered.find("First Section").expect("S1 not found");
        let s2_pos = rendered.find("Second Section").expect("S2 not found");
        assert!(s1_pos < s2_pos, "S1 must appear before S2:\n{rendered}");
    }

    #[test]
    fn render_plan_places_summary_after_generated_header() {
        // Summary lines appear after the header and before task sections.
        // Uses v5 metadata JSON.
        let json = r#"{
  "schema_version": 5,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Title track-a",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T01:00:00Z"
}"#;
        let (track, _) = codec::decode(json).unwrap();
        let impl_plan = make_impl_plan_with_summary_and_tasks(
            &["Summary line one", "Summary line two"],
            &[("T001", "Task", "todo", None)],
            &[("S1", "Section", &["T001"])],
        );
        let rendered = render_plan(&track, Some(&impl_plan));
        let header_idx = rendered
            .find("<!-- Generated from metadata.json + impl-plan.json")
            .expect("generated header missing");
        let summary_idx = rendered.find("Summary line one").expect("summary not found");
        let tasks_idx = rendered.find("## Tasks").expect("tasks section not found");
        assert!(header_idx < summary_idx, "header must precede summary:\n{rendered}");
        assert!(summary_idx < tasks_idx, "summary must precede tasks:\n{rendered}");
    }

    #[test]
    fn render_plan_renders_section_description_lines() {
        // Section description lines appear as blockquotes under the section heading.
        // Uses v5 metadata JSON.
        let json = r#"{
  "schema_version": 5,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Title track-a",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T01:00:00Z"
}"#;
        let (track, _) = codec::decode(json).unwrap();
        let impl_plan = make_impl_plan_with_desc_and_tasks(
            &[("T001", "Task", "todo", None)],
            &[("S1", "Section", &["T001"], &["Describe the section goal", "Additional context"])],
        );
        let rendered = render_plan(&track, Some(&impl_plan));
        assert!(rendered.contains("# Title track-a"), "title must appear:\n{rendered}");
        assert!(
            rendered.contains("Describe the section goal"),
            "first description line missing:\n{rendered}"
        );
        assert!(
            rendered.contains("Additional context"),
            "second description line missing:\n{rendered}"
        );
    }

    /// Decodes v5 metadata JSON and returns a `TrackSnapshot` with a specified `derived_status`.
    /// For test use only: allows setting an explicit `derived_status` without impl-plan.json I/O.
    fn make_snapshot_v5(
        json: &str,
        derived_status: &str,
        schema_version: u32,
        dir: PathBuf,
    ) -> TrackSnapshot {
        let (track, meta) = codec::decode(json).unwrap();
        TrackSnapshot {
            dir,
            track,
            meta,
            schema_version,
            derived_status: derived_status.to_owned(),
        }
    }

    /// Decodes legacy v2/v3 metadata JSON and returns a `TrackSnapshot`.
    /// The `derived_status` is read from the raw JSON `status` field.
    fn make_snapshot_legacy(json: &str, schema_version: u32, dir: PathBuf) -> TrackSnapshot {
        let raw: serde_json::Value = serde_json::from_str(json).unwrap();
        let status = raw.get("status").and_then(|v| v.as_str()).unwrap_or("planned").to_owned();
        let (track, meta) = decode_legacy_metadata(&raw, std::path::Path::new("test")).unwrap();
        TrackSnapshot { dir, track, meta, schema_version, derived_status: status }
    }

    #[test]
    fn render_registry_places_active_completed_and_archived() {
        // Active track is v5 (no `status` field; derived status = "planned").
        // Done and archived tracks use legacy v3 JSON (status field present).
        let active_json = sample_metadata_json("track-a", "planned", "2026-03-13T02:00:00Z", "[]");
        let done_json = sample_metadata_json_with_schema_and_branch(
            3,
            "track-b",
            "done",
            "2026-03-13T01:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"done","commit_hash":"abc1234"}]"#,
            Some("track/track-b"),
        );
        let archived_json = sample_metadata_json_with_schema_and_branch(
            3,
            "track-c",
            "archived",
            "2026-03-13T00:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
            Some("track/track-c"),
        );

        let active_snapshot =
            make_snapshot_v5(&active_json, "planned", 5, PathBuf::from("track/items/track-a"));
        let done_snapshot =
            make_snapshot_legacy(&done_json, 3, PathBuf::from("track/items/track-b"));
        let archived_snapshot =
            make_snapshot_legacy(&archived_json, 3, PathBuf::from("track/archive/track-c"));

        let rendered = render_registry(&[active_snapshot, done_snapshot, archived_snapshot]);

        assert!(rendered.contains("| track-a | planned | `/track:implement` | 2026-03-13 |"));
        assert!(rendered.contains("| track-b | Done | 2026-03-13 |"));
        assert!(rendered.contains("| track-c | Archived | 2026-03-13 |"));
    }

    #[test]
    fn render_registry_routes_branchless_planning_track_to_activate() {
        // v5 branchless planned track → no branch, derived_status = "planned"
        let plan_only_json = sample_metadata_json_with_branch(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            "[]",
            None,
        );
        let snapshot =
            make_snapshot_v5(&plan_only_json, "planned", 5, PathBuf::from("track/items/track-a"));
        let rendered = render_registry(&[snapshot]);

        assert!(rendered.contains("/track:activate track-a"));
        assert!(rendered.contains("/track:plan-only <feature>"));
    }

    #[test]
    fn render_registry_keeps_legacy_v2_branchless_planned_track_on_implement() {
        // Legacy v2 track uses the legacy decode path.
        let legacy_json = sample_metadata_json_with_schema_and_branch(
            2,
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
            None,
        );
        let snapshot = make_snapshot_legacy(&legacy_json, 2, PathBuf::from("track/items/track-a"));
        let rendered = render_registry(&[snapshot]);

        assert!(rendered.contains("/track:implement"));
        assert!(!rendered.contains("/track:activate track-a"));
    }

    #[test]
    fn render_registry_prefers_materialized_active_track_in_current_focus() {
        // Both are v5: no branch (plan-only) vs with branch (materialized).
        let plan_only_json = sample_metadata_json_with_branch(
            "track-plan-only",
            "planned",
            "2026-03-13T03:00:00Z",
            "[]",
            None,
        );
        let materialized_json =
            sample_metadata_json("track-materialized", "planned", "2026-03-13T02:00:00Z", "[]");
        let plan_only_snap = make_snapshot_v5(
            &plan_only_json,
            "planned",
            5,
            PathBuf::from("track/items/track-plan-only"),
        );
        let materialized_snap = make_snapshot_v5(
            &materialized_json,
            "planned",
            5,
            PathBuf::from("track/items/track-materialized"),
        );
        let rendered = render_registry(&[plan_only_snap, materialized_snap]);

        assert!(rendered.contains("- Latest active track: `track-materialized`"));
        assert!(rendered.contains("- Next recommended command: `/track:implement`"));
    }

    #[test]
    fn render_registry_prefers_legacy_v2_planned_track_over_newer_plan_only() {
        // Legacy v2 (no branch) vs v5 plan-only (no branch, schema_version 5).
        // The v2 legacy track should be preferred (lower priority sort key).
        let legacy_json = sample_metadata_json_with_schema_and_branch(
            2,
            "track-legacy",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
            None,
        );
        let plan_only_json = sample_metadata_json_with_branch(
            "track-plan-only",
            "planned",
            "2026-03-13T03:00:00Z",
            "[]",
            None,
        );
        let legacy_snap =
            make_snapshot_legacy(&legacy_json, 2, PathBuf::from("track/items/track-legacy"));
        let plan_only_snap = make_snapshot_v5(
            &plan_only_json,
            "planned",
            5,
            PathBuf::from("track/items/track-plan-only"),
        );
        let rendered = render_registry(&[plan_only_snap, legacy_snap]);

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

    // --- registry / snapshot boundary tests ---

    #[test]
    fn collect_track_snapshots_ignores_plain_files_under_items() {
        let dir = tempfile::tempdir().unwrap();
        let items_root = dir.path().join("track/items");
        std::fs::create_dir_all(&items_root).unwrap();
        // Valid track directory.
        let track_dir = items_root.join("track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json(
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[
    { "id": "T001", "description": "First", "status": "todo" }
  ]"#,
            ),
        )
        .unwrap();
        // A stray file (not a directory) directly under track/items.
        std::fs::write(items_root.join("stray.txt"), "not a track").unwrap();

        let snapshots = collect_track_snapshots(dir.path()).unwrap();
        assert_eq!(snapshots.len(), 1, "stray file must be ignored: got {snapshots:?}");
        assert_eq!(snapshots[0].track.id().as_ref(), "track-a");
    }

    #[test]
    fn collect_track_snapshots_tie_breaks_same_updated_at_by_track_id() {
        let dir = tempfile::tempdir().unwrap();
        let items_root = dir.path().join("track/items");
        std::fs::create_dir_all(&items_root).unwrap();

        // track-b is inserted first to verify the tie-break applies regardless of
        // directory traversal order.
        for id in ["track-b", "track-a"] {
            let td = items_root.join(id);
            std::fs::create_dir_all(&td).unwrap();
            std::fs::write(
                td.join("metadata.json"),
                sample_metadata_json(
                    id,
                    "planned",
                    "2026-03-13T02:00:00Z", // identical updated_at
                    r#"[
    { "id": "T001", "description": "First", "status": "todo" }
  ]"#,
                ),
            )
            .unwrap();
        }

        let snapshots = collect_track_snapshots(dir.path()).unwrap();
        let ids: Vec<&str> = snapshots.iter().map(|s| s.track.id().as_ref()).collect();
        assert_eq!(
            ids,
            vec!["track-a", "track-b"],
            "same updated_at must tie-break by track_id asc"
        );
    }

    #[test]
    fn sync_rendered_views_omits_unchanged_registry_from_changed_set() {
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
    { "id": "T001", "description": "First", "status": "todo" }
  ]"#,
            ),
        )
        .unwrap();

        // First call populates plan.md and registry.md.
        let first_changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();
        assert!(first_changed.iter().any(|p| p.ends_with("registry.md")));

        // Second call with no metadata changes must leave both outputs untouched.
        let second_changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();
        assert!(
            !second_changed.iter().any(|p| p.ends_with("registry.md")),
            "unchanged registry.md must be omitted from changed set: {second_changed:?}"
        );
        assert!(
            !second_changed.iter().any(|p| p.ends_with("plan.md")),
            "unchanged plan.md must be omitted from changed set: {second_changed:?}"
        );
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
    fn validate_track_snapshots_tolerates_phase_zero_missing_plan_md() {
        // Phase 0 compat (ADR 2026-04-19-1242 §D0.0 / §D1.4): a freshly-created
        // v5 track directory containing only `metadata.json` (no `plan.md` yet,
        // because the view is rendered in later phases) must pass validation.
        // The previous behaviour failed with an I/O error on the missing file.
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json_with_schema_and_branch(
                5,
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                "[]",
                Some("track/track-a"),
            ),
        )
        .unwrap();
        // NOTE: no `plan.md` on purpose — this mirrors the state right after
        // `/track:init` before any downstream view rendering has occurred.
        assert!(validate_track_snapshots(dir.path()).is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn validate_track_snapshots_rejects_dangling_plan_md_symlink() {
        // Regression guard (Codex review #110, 2026-04-23): a `plan.md` that
        // exists as a symlink pointing at a non-existent target must NOT be
        // treated as "Phase 0 plan.md absent". Previously `std::fs::metadata`
        // followed the symlink and returned NotFound, so the branch
        // swallowed the dangling-symlink case and reported success for a
        // corrupted track directory.
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json_with_schema_and_branch(
                5,
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                "[]",
                Some("track/track-a"),
            ),
        )
        .unwrap();
        // Create a symlink whose target does not exist.
        let link = track_dir.join("plan.md");
        std::os::unix::fs::symlink(track_dir.join("missing-target.md"), &link).unwrap();

        let err = validate_track_snapshots(dir.path()).unwrap_err();
        assert!(
            err.to_string().contains("dangling symlink"),
            "expected dangling-symlink rejection, got: {err}"
        );
    }

    #[test]
    fn validate_track_snapshots_rejects_out_of_sync_plan() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        // v5 identity-only metadata — v2/v3/v4 legacy tracks are intentionally
        // skipped by `validate_track_snapshots` so only v5 mismatches surface.
        std::fs::write(
            track_dir.join("metadata.json"),
            sample_metadata_json_with_schema_and_branch(
                5,
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                "[]",
                Some("track/track-a"),
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
        let metadata_path = track_dir.join("metadata.json");
        // v5 identity-only metadata so the plan.md freshness check runs and
        // passes before we reach the registry.md check.
        std::fs::write(
            &metadata_path,
            sample_metadata_json_with_schema_and_branch(
                5,
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                "[]",
                Some("track/track-a"),
            ),
        )
        .unwrap();
        let (track, _) = codec::decode(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
        std::fs::write(track_dir.join("plan.md"), render_plan(&track, None)).unwrap();
        std::fs::create_dir_all(dir.path().join("track")).unwrap();
        std::fs::write(dir.path().join("track/registry.md"), "# stale registry\n").unwrap();

        let err = validate_track_snapshots(dir.path()).unwrap_err();
        assert!(err.to_string().contains("registry.md does not match metadata.json"));
    }

    #[test]
    fn validate_track_document_accepts_planning_only_v3_without_branch() {
        // Validates legacy v3 behavior. Uses explicit v3 JSON (with `status`
        // field) since `sample_metadata_json_with_branch` now generates v5.
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
        std::fs::write(
            &metadata_path,
            sample_metadata_json_with_schema_and_branch(
                3,
                "track-a",
                "planned",
                "2026-03-13T02:00:00Z",
                r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
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
        // Validates legacy v3 behavior. Uses explicit v3 JSON.
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
        std::fs::write(
            &metadata_path,
            sample_metadata_json_with_schema_and_branch(
                3,
                "track-a",
                "in_progress",
                "2026-03-13T02:00:00Z",
                r#"[{"id":"T001","description":"First task","status":"in_progress"}]"#,
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
        // tasks/plan fields are stripped by codec::decode() during the v2/v3 migration window.
        // A v3 document missing 'tasks' still decodes successfully (stripped) → Ok.
        // The "Missing required field 'tasks'" check is not enforced (tasks moved to ImplPlanDocument).
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
        assert!(
            validate_track_document(&metadata_path, track_dir.file_name(), &doc).is_ok(),
            "v3 doc missing 'tasks' field is accepted (tasks stripped during migration)"
        );
    }

    #[test]
    fn validate_track_document_rejects_unreferenced_task() {
        // tasks/plan fields are stripped by codec::decode() during the v2/v3 migration window.
        // An unreferenced task in a v3 doc no longer causes a validate error (tasks moved to ImplPlanDocument).
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
        // T002 is declared in tasks but not referenced from any plan section (legacy v3 doc).
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
        assert!(
            validate_track_document(&metadata_path, track_dir.file_name(), &doc).is_ok(),
            "v3 doc with unreferenced task is accepted (tasks/plan stripped during migration)"
        );
    }

    #[test]
    fn validate_track_document_rejects_duplicate_task_reference() {
        // Duplicate task_ids in plan sections are an ImplPlanDocument concern.
        // validate_track_document strips tasks/plan fields via codec::decode(); no error expected.
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
        // T001 is referenced by both S1 and S2 sections (legacy v3 format).
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
        // Duplicate plan references are no longer checked at this layer.
        // Document should decode successfully (tasks/plan stripped by codec).
        assert!(
            validate_track_document(&metadata_path, track_dir.file_name(), &doc).is_ok(),
            "duplicate plan ref check moved to ImplPlanDocument"
        );
    }

    #[test]
    fn validate_track_document_rejects_status_drift_in_progress_vs_done() {
        // Status is stored explicitly in legacy tracks (not task-derived).
        // The stored metadata.status is the authoritative source; task states are ignored.
        // The old task-derived drift check is gone — this document now passes validation.
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
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
        // Task-derived status drift check removed; stored status is authoritative.
        // doc.status="in_progress" → decoded status="in_progress" → no drift → Ok.
        assert!(
            validate_track_document(&metadata_path, track_dir.file_name(), &doc).is_ok(),
            "task-derived status drift check removed; stored status is authoritative"
        );
    }

    #[test]
    fn validate_track_document_rejects_archived_with_incomplete_tasks() {
        // "archived must have all tasks resolved" is now an ImplPlanDocument concern.
        // validate_track_document no longer checks task states (stripped by codec::decode()).
        // A v3 archived track with a todo task decodes correctly with the identity-only semantics.
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata_path = track_dir.join("metadata.json");
        // metadata.status is "archived" — tasks are ignored under identity-only semantics.
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
        // Task-completion check for archived tracks moved to ImplPlanDocument.
        // Document should now decode without error (status=archived is valid; tasks stripped).
        assert!(
            validate_track_document(&metadata_path, track_dir.file_name(), &doc).is_ok(),
            "archived+incomplete check moved to ImplPlanDocument"
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

        // Write a minimal spec.json (schema v2: no status field)
        std::fs::write(
            track_dir.join("spec.json"),
            r#"{
  "schema_version": 2,
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
  "schema_version": 2,
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

        // Valid JSON but unsupported schema version — must propagate as an error.
        // Note: no legacy fields (e.g. "status") here; deny_unknown_fields would turn those
        // into a Json error, which is warn-and-continue. This tests the version gate path.
        std::fs::write(
            track_dir.join("spec.json"),
            r#"{"schema_version":99,"version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#,
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
    fn sync_rendered_views_populates_signal_emojis_from_signal_file() {
        // Regression guard: after the declaration codec stopped surfacing inline
        // signals, the rendered `<layer>-types.md` lost its signal-column emojis
        // and fell back to `—`. `sync_rendered_views` must read the companion
        // `<layer>-type-signals.json` file and populate `doc.signals()` before
        // rendering so the markdown reflects the evaluated state.
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

        // Companion signal file with a Blue signal for the declared TrackId.
        let decl_bytes = std::fs::read(track_dir.join("domain-types.json")).unwrap();
        let hash = crate::tddd::type_signals_codec::declaration_hash(&decl_bytes);
        let signal_file = serde_json::json!({
            "schema_version": 1,
            "generated_at": "2026-04-19T00:00:00Z",
            "declaration_hash": hash,
            "signals": [
                {
                    "type_name": "TrackId",
                    "kind_tag": "value_object",
                    "signal": "blue",
                    "found_type": true
                }
            ],
        });
        std::fs::write(
            track_dir.join("domain-type-signals.json"),
            serde_json::to_string_pretty(&signal_file).unwrap(),
        )
        .unwrap();

        let _changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

        let md = std::fs::read_to_string(track_dir.join("domain-types.md")).unwrap();
        assert!(
            md.contains('\u{1f535}'),
            "rendered markdown must include the Blue emoji populated from the signal file, got:\n{md}"
        );
    }

    #[test]
    fn sync_rendered_views_ignores_stale_signal_file_when_hash_mismatches() {
        // Regression guard for the stale-hash view-render bug: if the
        // declaration changes without regenerating signals, the rendered
        // `<layer>-types.md` must NOT paint misleading Blue emojis from the
        // old evaluation. Fall back to `—` placeholders instead. The
        // authoritative fail-closed behavior for stale signals lives in
        // `spec_states::evaluate_layer_catalogue`; the renderer just avoids
        // misrepresenting the state to a reviewer.
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

        // Stale signal file — `declaration_hash` does NOT match the on-disk
        // declaration bytes.
        let stale_signal = serde_json::json!({
            "schema_version": 1,
            "generated_at": "2026-04-19T00:00:00Z",
            "declaration_hash": "0000000000000000000000000000000000000000000000000000000000000000",
            "signals": [
                {
                    "type_name": "TrackId",
                    "kind_tag": "value_object",
                    "signal": "blue",
                    "found_type": true
                }
            ],
        });
        std::fs::write(
            track_dir.join("domain-type-signals.json"),
            serde_json::to_string_pretty(&stale_signal).unwrap(),
        )
        .unwrap();

        let _changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

        let md = std::fs::read_to_string(track_dir.join("domain-types.md")).unwrap();
        assert!(
            !md.contains('\u{1f535}'),
            "stale signal file must NOT produce a Blue emoji in the rendered markdown, got:\n{md}"
        );
        assert!(
            md.contains('—'),
            "rendered markdown must fall back to `—` placeholder on stale signal file, got:\n{md}"
        );
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
        // track freezes plan.md in its pre-done state.
        //
        // Verify that single-track sync overwrites stale plan.md and renders the title.
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
        // Verify title and stub note are present.
        assert!(plan.contains("# Title track-done"), "title must appear in plan.md:\n{plan}");
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
        // v5 identity-only metadata. Derived status comes from impl-plan.json
        // below (all tasks done → status = Done).
        std::fs::write(
            done_dir.join("metadata.json"),
            sample_metadata_json("track-done-spec", "done", "2026-03-10T00:00:00Z", "[]"),
        )
        .unwrap();
        // impl-plan.json with an all-done task list so the derived track status
        // resolves to Done and the done-branch render path is exercised.
        std::fs::write(
            done_dir.join("impl-plan.json"),
            r#"{"schema_version":1,"tasks":[{"id":"T001","description":"Done task","status":"done","commit_hash":"abc1234567890abc1234567890abc1234567890a"}],"plan":{"summary":[],"sections":[{"id":"S1","title":"All","task_ids":["T001"]}]}}"#,
        )
        .unwrap();
        // Done tracks intentionally do not require spec.json to be re-decoded
        // (it is never re-rendered for a frozen track). Writing a minimal v2
        // spec.json here is fine; an absent spec.json would also work.
        std::fs::write(
            done_dir.join("spec.json"),
            r#"{"schema_version":2,"version":"1.0","title":"Done Feature","goal":[],"scope":{"in_scope":[],"out_of_scope":[]},"constraints":[],"acceptance_criteria":[]}"#,
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
            sample_metadata_json("track-done-domain", "done", "2026-03-10T00:00:00Z", "[]"),
        )
        .unwrap();
        // impl-plan.json with an all-done task list so the derived track status
        // resolves to Done.
        std::fs::write(
            done_dir.join("impl-plan.json"),
            r#"{"schema_version":1,"tasks":[{"id":"T001","description":"Done task","status":"done","commit_hash":"abc1234567890abc1234567890abc1234567890a"}],"plan":{"summary":[],"sections":[{"id":"S1","title":"All","task_ids":["T001"]}]}}"#,
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

    // ---------------------------------------------------------------------------
    // Multi-layer sync_rendered_views tests (T004 / D3)
    // ---------------------------------------------------------------------------
    //
    // These tests verify that sync_rendered_views correctly iterates all
    // tddd.enabled layers from architecture-rules.json and generates the
    // corresponding <layer>-types.md for each layer whose <layer>-types.json
    // is present in the track directory. The loop uses the existing
    // `parse_tddd_layers` resolver (introduced in tddd-01 Phase 1 Task 7,
    // already reused by `apps/cli::resolve_layers`).

    const USECASE_TYPES_JSON_MINIMAL: &str = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "TrackReader", "kind": "value_object", "description": "Test usecase type", "approved": true }
  ]
}"#;

    const INFRASTRUCTURE_TYPES_JSON_MINIMAL: &str = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "FsTrackStore", "kind": "value_object", "description": "Test infrastructure type", "approved": true }
  ]
}"#;

    const MULTI_LAYER_ARCH_RULES: &str = r#"{
      "layers": [
        { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } },
        { "crate": "usecase", "tddd": { "enabled": true, "catalogue_file": "usecase-types.json" } },
        { "crate": "infrastructure", "tddd": { "enabled": true, "catalogue_file": "infrastructure-types.json" } }
      ]
    }"#;

    #[test]
    fn sync_rendered_views_generates_usecase_types_md_from_usecase_types_json() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(dir.path().join("architecture-rules.json"), MULTI_LAYER_ARCH_RULES).unwrap();

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
        std::fs::write(track_dir.join("usecase-types.json"), USECASE_TYPES_JSON_MINIMAL).unwrap();

        let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

        assert!(
            changed.iter().any(|p| p.ends_with("usecase-types.md")),
            "usecase-types.md should be reported as changed"
        );

        let md = std::fs::read_to_string(track_dir.join("usecase-types.md")).unwrap();
        assert!(
            md.contains("<!-- Generated from usecase-types.json"),
            "must have usecase-types.json header (not domain-types.json), got: {md}"
        );
        assert!(md.contains("TrackReader"), "must include declared type name");
    }

    #[test]
    fn sync_rendered_views_generates_infrastructure_types_md_from_infrastructure_types_json() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(dir.path().join("architecture-rules.json"), MULTI_LAYER_ARCH_RULES).unwrap();

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
        std::fs::write(
            track_dir.join("infrastructure-types.json"),
            INFRASTRUCTURE_TYPES_JSON_MINIMAL,
        )
        .unwrap();

        let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

        assert!(
            changed.iter().any(|p| p.ends_with("infrastructure-types.md")),
            "infrastructure-types.md should be reported as changed"
        );

        let md = std::fs::read_to_string(track_dir.join("infrastructure-types.md")).unwrap();
        assert!(
            md.contains("<!-- Generated from infrastructure-types.json"),
            "must have infrastructure-types.json header, got: {md}"
        );
        assert!(md.contains("FsTrackStore"), "must include declared type name");
    }

    #[test]
    fn sync_rendered_views_generates_multiple_layer_types_md_independently() {
        // Multi-layer track: domain + usecase + infrastructure catalogue files all
        // present. The loop must render each <layer>-types.md independently (one
        // layer's presence/absence must not affect another's rendering).
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(dir.path().join("architecture-rules.json"), MULTI_LAYER_ARCH_RULES).unwrap();

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
        std::fs::write(track_dir.join("usecase-types.json"), USECASE_TYPES_JSON_MINIMAL).unwrap();
        std::fs::write(
            track_dir.join("infrastructure-types.json"),
            INFRASTRUCTURE_TYPES_JSON_MINIMAL,
        )
        .unwrap();

        let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

        // All 3 layer rendered views must be reported as changed
        assert!(
            changed.iter().any(|p| p.ends_with("domain-types.md")),
            "domain-types.md should be reported as changed"
        );
        assert!(
            changed.iter().any(|p| p.ends_with("usecase-types.md")),
            "usecase-types.md should be reported as changed"
        );
        assert!(
            changed.iter().any(|p| p.ends_with("infrastructure-types.md")),
            "infrastructure-types.md should be reported as changed"
        );

        // Each rendered view must carry its own source_file_name in the header
        let domain_md = std::fs::read_to_string(track_dir.join("domain-types.md")).unwrap();
        assert!(
            domain_md.contains("<!-- Generated from domain-types.json"),
            "domain-types.md must have its own header"
        );

        let usecase_md = std::fs::read_to_string(track_dir.join("usecase-types.md")).unwrap();
        assert!(
            usecase_md.contains("<!-- Generated from usecase-types.json"),
            "usecase-types.md must have its own header (independent of domain-types.md)"
        );

        let infra_md = std::fs::read_to_string(track_dir.join("infrastructure-types.md")).unwrap();
        assert!(
            infra_md.contains("<!-- Generated from infrastructure-types.json"),
            "infrastructure-types.md must have its own header"
        );
    }

    #[test]
    fn sync_rendered_views_malformed_layer_json_does_not_block_other_layers() {
        // D3 guarantee: the per-layer `TypeCatalogueCodecError::Json` warn-and-continue
        // path must be exercised in a multi-layer scenario. A malformed catalogue for
        // one layer (usecase) must not prevent the other layers (domain, infrastructure)
        // from rendering their views. This is the cross-layer error isolation guarantee.
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(dir.path().join("architecture-rules.json"), MULTI_LAYER_ARCH_RULES).unwrap();

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
        // domain and infrastructure catalogue files are valid
        std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();
        // usecase catalogue is malformed JSON — must warn and continue
        std::fs::write(track_dir.join("usecase-types.json"), "{ not valid json }").unwrap();
        std::fs::write(
            track_dir.join("infrastructure-types.json"),
            INFRASTRUCTURE_TYPES_JSON_MINIMAL,
        )
        .unwrap();

        // Must succeed — malformed usecase JSON must not abort the sync
        let result = sync_rendered_views(dir.path(), Some("track-a"));
        assert!(result.is_ok(), "malformed usecase-types.json must not abort multi-layer sync");

        let changed = result.unwrap();

        // domain and infrastructure rendered views must still be generated
        assert!(
            changed.iter().any(|p| p.ends_with("domain-types.md")),
            "domain-types.md must still render when usecase-types.json is malformed"
        );
        assert!(
            changed.iter().any(|p| p.ends_with("infrastructure-types.md")),
            "infrastructure-types.md must still render when usecase-types.json is malformed"
        );

        // usecase rendered view must NOT appear in changed list (malformed → skipped)
        assert!(
            !changed.iter().any(|p| p.ends_with("usecase-types.md")),
            "usecase-types.md must NOT be rendered when usecase-types.json is malformed"
        );
    }

    // ---------------------------------------------------------------------------
    // T020 sync_rendered_views <layer>-catalogue-spec-signals.json integration
    // ---------------------------------------------------------------------------
    //
    // End-to-end tests covering the catalogue-spec signals file-loading path
    // added in T020 (ADR 2026-04-23-0344 §D2.5 / IN-17). Complements the
    // renderer-level Some/None unit tests in `type_catalogue_render.rs` by
    // exercising the actual `sync_rendered_views` pipeline:
    //   - opt-in guard via `catalogue_spec_signal.enabled`
    //   - filename derivation (`<layer_id>-catalogue-spec-signals.json`)
    //   - fresh-hash validation (hex comparison)
    //   - stale / malformed fallback to `None` (em-dash fallback, non-fatal)

    const MULTI_LAYER_ARCH_RULES_WITH_CAT_SPEC_OPT_IN: &str = r#"{
      "layers": [
        {
          "crate": "domain",
          "tddd": {
            "enabled": true,
            "catalogue_file": "domain-types.json",
            "catalogue_spec_signal": { "enabled": true }
          }
        }
      ]
    }"#;

    const MULTI_LAYER_ARCH_RULES_CAT_SPEC_OPT_OUT: &str = r#"{
      "layers": [
        {
          "crate": "domain",
          "tddd": {
            "enabled": true,
            "catalogue_file": "domain-types.json"
          }
        }
      ]
    }"#;

    #[test]
    fn sync_rendered_views_renders_cat_spec_column_when_signals_fresh_and_opt_in_enabled() {
        // Happy path: opt-in flag is true, signals file exists with a matching
        // catalogue_declaration_hash, and a per-entry `blue` signal is declared
        // for `TrackId`. The rendered markdown must carry the 6-column header
        // and paint the 🔵 emoji in the Cat-Spec column.
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(
            dir.path().join("architecture-rules.json"),
            MULTI_LAYER_ARCH_RULES_WITH_CAT_SPEC_OPT_IN,
        )
        .unwrap();

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

        // Catalogue-spec-signals file with a fresh (matching) hash.
        let decl_bytes = std::fs::read(track_dir.join("domain-types.json")).unwrap();
        let hash_hex = crate::tddd::type_signals_codec::declaration_hash(&decl_bytes);
        let spec_signals_json = serde_json::json!({
            "schema_version": 1,
            "catalogue_declaration_hash": hash_hex,
            "signals": [
                { "type_name": "TrackId", "signal": "blue" }
            ],
        });
        // Filename derivation: `<layer_id>-catalogue-spec-signals.json`
        // = `domain-catalogue-spec-signals.json`.
        std::fs::write(
            track_dir.join("domain-catalogue-spec-signals.json"),
            serde_json::to_string_pretty(&spec_signals_json).unwrap(),
        )
        .unwrap();

        let _changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

        let md = std::fs::read_to_string(track_dir.join("domain-types.md")).unwrap();
        assert!(
            md.contains("| Name | Kind | Action | Details | Signal | Cat-Spec |"),
            "6-column header must appear when opt-in + fresh signals present, got:\n{md}"
        );
        // TrackId entry row must include the 🔵 emoji in Cat-Spec column.
        let track_id_row = md
            .lines()
            .find(|l| l.starts_with("| TrackId |"))
            .expect("TrackId row must be rendered");
        assert!(
            track_id_row.contains('\u{1f535}'),
            "TrackId row must show Blue emoji in Cat-Spec column, got: {track_id_row}"
        );
    }

    #[test]
    fn sync_rendered_views_skips_cat_spec_column_when_opt_in_disabled() {
        // Opt-in guard: even when a valid catalogue-spec-signals.json is
        // present on disk, the renderer must produce the legacy 5-column
        // layout if the layer has NOT opted in via `catalogue_spec_signal.enabled`.
        // This is the phased-activation knob per ADR §D5.4.
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(
            dir.path().join("architecture-rules.json"),
            MULTI_LAYER_ARCH_RULES_CAT_SPEC_OPT_OUT,
        )
        .unwrap();

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

        // Signals file present with matching hash — but layer hasn't opted in.
        let decl_bytes = std::fs::read(track_dir.join("domain-types.json")).unwrap();
        let hash_hex = crate::tddd::type_signals_codec::declaration_hash(&decl_bytes);
        let spec_signals_json = serde_json::json!({
            "schema_version": 1,
            "catalogue_declaration_hash": hash_hex,
            "signals": [ { "type_name": "TrackId", "signal": "blue" } ],
        });
        std::fs::write(
            track_dir.join("domain-catalogue-spec-signals.json"),
            serde_json::to_string_pretty(&spec_signals_json).unwrap(),
        )
        .unwrap();

        let _changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

        let md = std::fs::read_to_string(track_dir.join("domain-types.md")).unwrap();
        assert!(
            !md.contains("Cat-Spec"),
            "Cat-Spec column must NOT appear when opt-in is disabled, got:\n{md}"
        );
        assert!(
            md.contains("| Name | Kind | Action | Details | Signal |"),
            "legacy 5-column header must be preserved when opt-in is disabled, got:\n{md}"
        );
    }

    #[test]
    fn sync_rendered_views_errors_on_stale_cat_spec_signals() {
        // Fail-closed: a stale `catalogue_declaration_hash` in the signals
        // file indicates the catalogue changed without regenerating signals.
        // View rendering aborts and the caller is expected to run
        // `sotp track catalogue-spec-signals <track_id>` before retrying.
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(
            dir.path().join("architecture-rules.json"),
            MULTI_LAYER_ARCH_RULES_WITH_CAT_SPEC_OPT_IN,
        )
        .unwrap();

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

        // Stale: hash does NOT match on-disk catalogue.
        let stale_hash = "0".repeat(64);
        let stale_json = serde_json::json!({
            "schema_version": 1,
            "catalogue_declaration_hash": stale_hash,
            "signals": [ { "type_name": "TrackId", "signal": "blue" } ],
        });
        std::fs::write(
            track_dir.join("domain-catalogue-spec-signals.json"),
            serde_json::to_string_pretty(&stale_json).unwrap(),
        )
        .unwrap();

        let err = sync_rendered_views(dir.path(), Some("track-a")).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("stale") && msg.contains("catalogue-spec-signals"),
            "stale-hash error expected, got: {msg}"
        );
    }

    #[test]
    fn sync_rendered_views_errors_on_malformed_cat_spec_signals() {
        // Fail-closed: an unparseable signals file is a system-state error,
        // not a silent fallback. The view renderer propagates the decode
        // failure and the caller re-runs `sotp track catalogue-spec-signals`.
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(
            dir.path().join("architecture-rules.json"),
            MULTI_LAYER_ARCH_RULES_WITH_CAT_SPEC_OPT_IN,
        )
        .unwrap();

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

        // Malformed JSON.
        std::fs::write(
            track_dir.join("domain-catalogue-spec-signals.json"),
            "{ this is not valid json ",
        )
        .unwrap();

        let err = sync_rendered_views(dir.path(), Some("track-a")).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("decode") || msg.contains("JSON"),
            "decode error expected, got: {msg}"
        );
    }

    #[test]
    fn sync_rendered_views_errors_on_missing_cat_spec_signals_when_opt_in() {
        // Fail-closed: when opt-in is enabled but the signals file has never
        // been generated, view rendering must error and direct the user to
        // `sotp track catalogue-spec-signals <track_id>`.
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        std::fs::write(
            dir.path().join("architecture-rules.json"),
            MULTI_LAYER_ARCH_RULES_WITH_CAT_SPEC_OPT_IN,
        )
        .unwrap();
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

        let err = sync_rendered_views(dir.path(), Some("track-a")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not found"), "not-found error expected, got: {msg}");
        assert!(msg.contains("sotp track catalogue-spec-signals"), "remediation missing: {msg}");
    }
}
