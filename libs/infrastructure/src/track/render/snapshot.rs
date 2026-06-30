//! Track snapshot collection: reading and decoding metadata.json files from
//! `track/items/` and `track/archive/` directories.

use std::path::{Path, PathBuf};

use domain::{TrackMetadata, derive_track_status};

use super::super::codec::{self, DocumentMeta};
use super::{TRACK_ARCHIVE_DIR, TRACK_ITEMS_DIR, VALID_TRACK_STATUSES};
use crate::impl_plan_codec;

use super::RenderError;

/// Minimal DTO for peeking at a metadata.json's schema_version + identity
/// before dispatching to a version-specific decoder. This is intentionally
/// loose (no `deny_unknown_fields`) so that legacy v2/v3/v4 metadata — which
/// still carries removed fields like `status`, `tasks`, `plan` — can be
/// identified and routed through the legacy path. The strict v5 DTO
/// (`codec::TrackDocumentV2`) is only applied in the v5 branch via
/// `codec::decode`.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct TrackSchemaPeek {
    pub schema_version: u32,
    pub id: String,
    #[serde(default)]
    pub branch: Option<String>,
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

/// Loads `impl-plan.json` from a track directory, returning `None` when the file
/// does not exist. Propagates I/O and decode errors as `RenderError::Io`.
pub(super) fn load_impl_plan_opt(
    track_dir: &Path,
) -> Result<Option<domain::ImplPlanDocument>, RenderError> {
    let path = track_dir.join("impl-plan.json");
    if !path.is_file() {
        return Ok(None);
    }
    let json = std::fs::read_to_string(&path)?;
    impl_plan_codec::decode(&json).map(Some).map_err(|e| {
        RenderError::Io(std::io::Error::other(format!(
            "impl-plan.json decode error at {}: {e}",
            path.display()
        )))
    })
}

/// Loads `task-coverage.json` from a track directory, returning `None` when the
/// file does not exist. Propagates I/O and decode errors as `RenderError::Io`.
pub(super) fn load_task_coverage_opt(
    track_dir: &Path,
) -> Result<Option<domain::TaskCoverageDocument>, RenderError> {
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

/// Decodes a legacy v2/v3/v4 metadata JSON value into a `(TrackMetadata, DocumentMeta)` pair.
///
/// Legacy format includes a `status` field and (in v2/v3) `tasks`/`plan` fields that are
/// no longer part of the v5 schema. This function extracts the identity fields (id, branch,
/// title) and, for v4 tracks, the `status_override` sub-field (introduced in v4).
///
/// # Errors
///
/// Returns `CodecError::InvalidField` when required fields are missing or malformed.
pub(crate) fn decode_legacy_metadata(
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

    // TODO(T005): decode branch_strategy_snapshot from legacy metadata once schema_version is bumped.
    // Bootstrap placeholder: legacy tracks have no snapshot; use main defaults.
    let main_branch = domain::NonEmptyString::try_new("main")
        .map_err(|e| codec::CodecError::Domain(domain::DomainError::Validation(e)))?;
    let legacy_snapshot = domain::branch_strategy::BranchStrategySnapshot::new(
        main_branch.clone(),
        main_branch,
        domain::branch_strategy::MergeMethod::Squash,
    );
    let track = TrackMetadata::with_branch(id, branch, title_str, status_override, legacy_snapshot)
        .map_err(codec::CodecError::Domain)?;
    let meta = DocumentMeta { schema_version, created_at, updated_at };

    let _ = metadata_path; // used for error context by callers
    Ok((track, meta))
}

// `REQUIRED_V3_METADATA_FIELDS` is only used in `validate_track_document`.
const REQUIRED_V3_METADATA_FIELDS: &[&str] =
    &["schema_version", "branch", "id", "title", "status", "created_at", "updated_at"];
// `tasks` and `plan` fields moved to impl-plan.json; removed from required list.

pub(crate) fn validate_track_document(
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

    let reserved_id_segments: &[&str] = &["git"];
    let segments = doc.id.split('-').collect::<Vec<_>>();
    for reserved in reserved_id_segments {
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
