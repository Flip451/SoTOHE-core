//! Rendering and sync of track read-only views (`plan.md`, `registry.md`, `spec.md`, `domain-types.md`) from metadata.json / spec.json / domain-types.json.

use std::path::{Path, PathBuf};

use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::tddd::{
    CatalogueLoader, CatalogueLoaderError, ContractMapRenderOptions, ContractMapRenderer,
};
use domain::{ImplPlanDocument, TaskCoverageDocument, TrackId, TrackMetadata, derive_track_status};

use super::atomic_write::atomic_write_file;
use super::codec::{self, DocumentMeta};
use crate::spec;
use crate::tddd::catalogue_document_codec::{CatalogueDocumentCodec, CatalogueDocumentCodecError};
use crate::tddd::contract_map_adapter::FsCatalogueLoader;
use crate::tddd::contract_map_renderer_adapter::ContractMapRendererAdapter;
use crate::tddd::type_signals_codec;
use crate::type_catalogue_render;
use crate::verify::tddd_layers::{LoadTdddLayersError, load_tddd_layers};

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
        "> Each track is expected to have `spec.md` (or `spec.json`) / `plan.md` / `metadata.json`; `observations.md` is optional.".to_owned(),
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
        //   - CatalogueDocumentCodecError::Json (syntax/EOF) warn-and-continue (file may be mid-edit)
        if !is_done_or_archived {
            let arch_rules_path = root.join("architecture-rules.json");
            // `load_tddd_layers` is fail-closed. Missing / symlinked / malformed
            // `architecture-rules.json` are all hard configuration errors —
            // never synthesize a fallback nor silently skip the layer iteration.
            let bindings = load_tddd_layers(&arch_rules_path, root).map_err(|e| match e {
                LoadTdddLayersError::Io { source, .. } => RenderError::Io(source),
                LoadTdddLayersError::Parse(err) => RenderError::Io(std::io::Error::other(format!(
                    "architecture-rules.json: {err}"
                ))),
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
                // T025 / CN-11: v3-native rendering — all catalogues MUST be schema_version 3.
                // Decode directly with CatalogueDocumentCodec. A v2 (or other non-v3) catalogue
                // fails closed (hard error) rather than silently rendering stale v2 content.
                // Only JSON parse failures are treated as warn-and-skip (file may be mid-edit).
                let stem = catalogue_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.strip_suffix("-types.json"))
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        catalogue_path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_owned()
                    });
                match CatalogueDocumentCodec::decode(&catalogue_content, &stem) {
                    Ok(v3_doc) => {
                        // Load `<layer>-type-signals.json` for the Signal column.
                        // Non-fatal miss / stale hash falls back to `—` placeholders.
                        // The authoritative fail-closed path for Missing/Stale lives in
                        // `spec_states::evaluate_layer_catalogue`.
                        let v3_signal_path = track_dir.join(binding.signal_file());
                        // Use `symlink_metadata()` to detect symlinks: `is_file()` follows
                        // symlinks, which would allow a crafted symlink to inject arbitrary
                        // file contents. For the view renderer, symlinks fall back to `—`
                        // (non-fatal miss).
                        let v3_type_signals_opt: Option<Vec<domain::TypeSignal>> = {
                            let is_plain = v3_signal_path
                                .symlink_metadata()
                                .is_ok_and(|m| m.file_type().is_file());
                            if is_plain {
                                std::fs::read_to_string(&v3_signal_path).ok().and_then(|sj| {
                                    type_signals_codec::decode(&sj).ok().and_then(|sd| {
                                        let current = type_signals_codec::declaration_hash(
                                            catalogue_content.as_bytes(),
                                        );
                                        if sd.declaration_hash() == current {
                                            Some(sd.signals().to_vec())
                                        } else {
                                            eprintln!(
                                                "warning: ignoring stale {} for {} \
                                                 (declaration_hash mismatch) — rendered \
                                                 signal column will fall back to `—`",
                                                binding.signal_file(),
                                                track_dir.display()
                                            );
                                            None
                                        }
                                    })
                                })
                            } else {
                                None
                            }
                        };
                        // Load `<layer>-catalogue-spec-signals.json` for the
                        // T020 Cat-Spec column (ADR 2026-04-23-0344 §D2.5).
                        // Opt-in gated; fail-closed on missing / symlinked /
                        // malformed / stale — remediation is documented in
                        // the error message (`sotp track catalogue-spec-signals
                        // <track_id>`). Opt-out layers render the legacy
                        // 5-column view (None).
                        let v3_spec_signals_doc = if binding.catalogue_spec_signal_enabled() {
                            let spec_path = track_dir.join(binding.catalogue_spec_signal_file());
                            Some(
                                type_catalogue_render::load_catalogue_spec_signals_for_view(
                                    &spec_path,
                                    catalogue_content.as_bytes(),
                                )
                                .map_err(|e| {
                                    RenderError::Io(std::io::Error::other(e.to_string()))
                                })?,
                            )
                        } else {
                            None
                        };
                        let rendered = type_catalogue_render::render_type_catalogue_v3(
                            &v3_doc,
                            catalogue_file,
                            v3_type_signals_opt.as_deref(),
                            v3_spec_signals_doc.as_ref(),
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
                    Err(CatalogueDocumentCodecError::Json(ref e))
                        if e.is_syntax() || e.is_eof() =>
                    {
                        // Warn and skip ONLY on true syntax/EOF errors — the file may be
                        // mid-edit (e.g. unsaved buffer or in-flight write). Data errors
                        // (missing required fields, wrong types — `e.is_data()`) indicate a
                        // schema conformance violation and must fail closed per CN-11.
                        eprintln!(
                            "warning: skipping {} render for {} (malformed JSON — syntax error \
                             at line {}, col {})",
                            binding.rendered_file(),
                            track_dir.display(),
                            e.line(),
                            e.column(),
                        );
                    }
                    Err(e) => {
                        // Any other error (UnsupportedSchemaVersion, InvalidEntry,
                        // CrateNameMismatch, or Json data/schema errors) is a hard error.
                        // CN-11: non-v3 or structurally invalid catalogues must not render
                        // silently.
                        return Err(RenderError::Io(std::io::Error::other(format!(
                            "catalogue {} in {}: {e}",
                            catalogue_file,
                            track_dir.display()
                        ))));
                    }
                }
            }
        } // end if !is_done_or_archived

        // Render `contract-map.md` unconditionally (outside the done/archived
        // guard) so the declaration relationship diagram stays fresh even after
        // a track reaches `done`.  The `!is_done_or_archived` block protects
        // frozen views whose content is strictly derived from the phase-2 type
        // design artefacts (`spec.md`, `<layer>-types.md`).  `contract-map.md`
        // is a *rendered graph* derived from all catalogue data and the
        // implementation renderer — it must reflect the final post-implementation
        // state, which may differ from the state captured while the track was
        // still `in_progress`.
        //
        // Failure modes (loader error, empty catalogues, unknown layer)
        // are non-fatal: log to stderr and leave the existing file untouched.
        // The authoritative fail-closed gate lives in
        // `spec_states::evaluate_layer_catalogue` and the merge-gate adapter.
        render_contract_map_view(root, &track_dir, track_id, &mut changed)?;
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
/// Fail-closed for `architecture-rules.json` discovery failures: a missing or
/// malformed `architecture-rules.json` causes `CatalogueLoaderError::LayerDiscoveryFailed`
/// or `CatalogueLoaderError::SymlinkRejected`, both of which are propagated as
/// `RenderError::Io` so that callers detect the configuration error regardless of
/// track status (including done/archived tracks that skip the per-layer catalogue
/// iteration block).
///
/// Non-fatal for catalogue-level failures (`CatalogueNotFound`, `DecodeFailed`,
/// `TopologicalSortFailed`): these indicate per-catalogue errors that should warn
/// but not abort view-sync, because the authoritative fail-closed gate for TDDD
/// correctness lives in `spec_states::evaluate_layer_catalogue` / the merge-gate
/// adapter.
///
/// Returning `Ok(())` means the render either succeeded or was intentionally
/// skipped (no track id, invalid track id, empty layer list, non-fatal catalogue
/// error). Returning `Err(RenderError::Io(_))` means a hard configuration error
/// (`architecture-rules.json` absent or malformed / symlinked).
fn render_contract_map_view(
    root: &Path,
    track_dir: &Path,
    track_id_str: Option<&str>,
    changed: &mut Vec<PathBuf>,
) -> Result<(), RenderError> {
    let Some(track_id_raw) = track_id_str else {
        return Ok(());
    };
    let Ok(track_id) = TrackId::try_new(track_id_raw) else {
        eprintln!(
            "warning: skipping contract-map.md render for {} (invalid track id)",
            track_dir.display()
        );
        return Ok(());
    };

    let items_dir = root.join(TRACK_ITEMS_DIR);
    let rules_path = root.join("architecture-rules.json");
    let loader = FsCatalogueLoader::new(items_dir, rules_path, root.to_path_buf());
    let (layer_order, catalogues) = match loader.load_all(&track_id) {
        Ok(result) => result,
        // Hard errors: architecture-rules.json missing / malformed / symlinked.
        // These are configuration errors that must be visible regardless of track
        // status (a done track with a missing arch-rules file is still broken).
        Err(CatalogueLoaderError::LayerDiscoveryFailed { reason }) => {
            return Err(RenderError::Io(std::io::Error::other(format!(
                "architecture-rules.json error for contract-map render at {}: {reason}",
                track_dir.display()
            ))));
        }
        Err(CatalogueLoaderError::SymlinkRejected { path }) => {
            return Err(RenderError::Io(std::io::Error::other(format!(
                "symlink rejected at {} (contract-map render for {})",
                path.display(),
                track_dir.display()
            ))));
        }
        // Hard error: non-symlink I/O failure reading a catalogue artifact or the
        // architecture-rules.json dependency graph.  Propagate so that callers
        // detect file-system corruption regardless of track status.
        Err(CatalogueLoaderError::IoError { path, reason }) => {
            return Err(RenderError::Io(std::io::Error::other(format!(
                "I/O error at {} (contract-map render for {}): {reason}",
                path.display(),
                track_dir.display()
            ))));
        }
        // Hard error: a cycle in `may_depend_on` is an invalid architecture-rules.json
        // configuration — the contract-map cannot be rendered until the cycle is
        // resolved, so propagate as a hard error rather than silently skipping.
        Err(CatalogueLoaderError::TopologicalSortFailed { reason }) => {
            return Err(RenderError::Io(std::io::Error::other(format!(
                "architecture-rules.json cycle detected (contract-map render for {}): {reason}",
                track_dir.display()
            ))));
        }
        // Non-fatal catalogue-level errors (absent catalogue, decode failure): warn
        // and skip. The authoritative fail-closed gate for TDDD correctness lives in
        // `spec_states::evaluate_layer_catalogue`.
        Err(
            e @ (CatalogueLoaderError::CatalogueNotFound { .. }
            | CatalogueLoaderError::DecodeFailed { .. }),
        ) => {
            eprintln!(
                "warning: skipping contract-map.md render for {} ({})",
                track_dir.display(),
                e
            );
            return Ok(());
        }
    };
    if layer_order.is_empty() {
        // No TDDD-enabled layers on this track — nothing to render.
        return Ok(());
    }

    let style_config_path = root.join(".harness/config/contract-map-style.toml");
    let adapter = ContractMapRendererAdapter::new(style_config_path);
    let docs: Vec<CatalogueDocument> = catalogues.values().cloned().collect();
    let opts = ContractMapRenderOptions::empty();
    let content = match adapter.render(&docs, &layer_order, &opts) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("warning: skipping contract-map.md render for {}: {e}", track_dir.display());
            return Ok(());
        }
    };
    let contract_map_path = track_dir.join("contract-map.md");
    let old = match std::fs::read_to_string(&contract_map_path) {
        Ok(existing) => Some(existing),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            eprintln!(
                "warning: cannot read existing contract-map.md for {}: {e}",
                track_dir.display()
            );
            return Ok(());
        }
    };
    let rendered_str: &str = content.as_ref();
    if old.as_deref().is_none_or(|existing| !rendered_matches(existing, rendered_str)) {
        if let Err(e) = atomic_write_file(&contract_map_path, rendered_str.as_bytes()) {
            eprintln!("warning: cannot write contract-map.md for {}: {e}", track_dir.display());
            return Ok(());
        }
        changed.push(contract_map_path);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "render_tests.rs"]
mod tests;
