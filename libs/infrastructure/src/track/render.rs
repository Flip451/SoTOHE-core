//! Rendering and sync of track read-only views (`plan.md`, `registry.md`, `spec.md`, `domain-types.md`) from metadata.json / spec.json / domain-types.json.

use std::path::{Path, PathBuf};

use domain::tddd::{CatalogueLoader, ContractMapRenderOptions, render_contract_map};
use domain::{TaskStatus, TrackId, TrackMetadata};

use super::atomic_write::atomic_write_file;
use super::codec::{self, DocumentMeta};
use crate::spec;
use crate::tddd::contract_map_adapter::FsCatalogueLoader;
use crate::tddd::{catalogue_codec, type_signals_codec};
use crate::type_catalogue_render;
use crate::verify::tddd_layers::{LoadTdddLayersError, load_tddd_layers_from_path};

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

    snapshots.sort_by(|a, b| {
        b.updated_at()
            .cmp(a.updated_at())
            .then_with(|| a.track.id().as_ref().cmp(b.track.id().as_ref()))
    });
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
                        // the declaration codec (post-T007) returns `doc.signals() = None`,
                        // so we have to read the signal file here and call `set_signals`
                        // before rendering.
                        //
                        // Failure modes (missing / malformed / symlinked signal file) are
                        // non-fatal for view rendering — the resulting markdown just
                        // falls back to `—` placeholders, consistent with the pre-T008
                        // transitional state. The authoritative fail-closed path for
                        // Missing/Stale lives in `spec_states::evaluate_layer_catalogue`
                        // (T005), which is the verification gate, not the view renderer.
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
                                    // `spec_states::evaluate_layer_catalogue`
                                    // (T005).
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
                        let rendered =
                            type_catalogue_render::render_type_catalogue(&doc, catalogue_file);
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
            // `spec_states::evaluate_layer_catalogue` (T005) and the
            // merge-gate adapter (T007 follow-up).
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

    // --- T008/T009: render_plan marker tests ---

    #[test]
    fn render_plan_marks_in_progress_task_with_tilde() {
        let json = sample_metadata_json(
            "track-a",
            "in_progress",
            "2026-03-13T01:00:00Z",
            r#"[
    { "id": "T001", "description": "Working task", "status": "in_progress" }
  ]"#,
        );
        let (track, _) = codec::decode(&json).unwrap();
        let rendered = render_plan(&track);
        assert!(
            rendered.contains("- [~] Working task"),
            "expected in_progress marker `[~]` for in_progress task:\n{rendered}"
        );
    }

    #[test]
    fn render_plan_marks_done_task_with_short_commit_hash() {
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
        let rendered = render_plan(&track);
        assert!(
            rendered.contains("- [x] Completed task abc1234"),
            "expected done marker `[x] <desc> <hash>`:\n{rendered}"
        );
    }

    #[test]
    fn render_plan_done_without_commit_hash_omits_literal_none() {
        let json = sample_metadata_json(
            "track-a",
            "done",
            "2026-03-13T01:00:00Z",
            r#"[
    { "id": "T001", "description": "Untraced done", "status": "done" }
  ]"#,
        );
        let (track, _) = codec::decode(&json).unwrap();
        let rendered = render_plan(&track);
        assert!(
            rendered.contains("- [x] Untraced done"),
            "expected done marker `[x] <desc>`:\n{rendered}"
        );
        assert!(
            !rendered.contains("- [x] Untraced done None"),
            "literal 'None' must not be rendered for done without commit_hash:\n{rendered}"
        );
    }

    #[test]
    fn render_plan_marks_skipped_task_with_dash() {
        let json = sample_metadata_json(
            "track-a",
            "done",
            "2026-03-13T01:00:00Z",
            r#"[
    { "id": "T001", "description": "Skipped task", "status": "skipped" }
  ]"#,
        );
        let (track, _) = codec::decode(&json).unwrap();
        let rendered = render_plan(&track);
        assert!(
            rendered.contains("- [-] Skipped task"),
            "expected skipped marker `[-] <desc>`:\n{rendered}"
        );
    }

    #[test]
    fn render_plan_preserves_multi_section_order() {
        // Two sections S1 and S2; S1 must render before S2.
        let json = r#"{
  "schema_version": 3,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Title track-a",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T01:00:00Z",
  "tasks": [
    { "id": "T001", "description": "Task one",   "status": "todo" },
    { "id": "T002", "description": "Task two",   "status": "todo" }
  ],
  "plan": {
    "summary": [],
    "sections": [
      { "id": "S1", "title": "First Section",  "description": [], "task_ids": ["T001"] },
      { "id": "S2", "title": "Second Section", "description": [], "task_ids": ["T002"] }
    ]
  }
}"#;
        let (track, _) = codec::decode(json).unwrap();
        let rendered = render_plan(&track);
        let first_idx = rendered.find("## First Section").expect("S1 header missing");
        let second_idx = rendered.find("## Second Section").expect("S2 header missing");
        assert!(
            first_idx < second_idx,
            "section order not preserved: S1 at {first_idx}, S2 at {second_idx}"
        );
    }

    #[test]
    fn render_plan_places_summary_after_generated_header() {
        let json = r#"{
  "schema_version": 3,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Title track-a",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T01:00:00Z",
  "tasks": [
    { "id": "T001", "description": "Task", "status": "todo" }
  ],
  "plan": {
    "summary": ["Summary line one", "Summary line two"],
    "sections": [
      { "id": "S1", "title": "Section", "description": [], "task_ids": ["T001"] }
    ]
  }
}"#;
        let (track, _) = codec::decode(json).unwrap();
        let rendered = render_plan(&track);
        let header_idx =
            rendered.find("<!-- Generated from metadata.json").expect("generated header missing");
        let summary_idx = rendered.find("Summary line one").expect("summary line missing");
        let section_idx = rendered.find("## Section").expect("section header missing");
        assert!(
            header_idx < summary_idx,
            "summary must follow the generated header: header={header_idx}, summary={summary_idx}"
        );
        assert!(
            summary_idx < section_idx,
            "summary must precede sections: summary={summary_idx}, section={section_idx}"
        );
    }

    #[test]
    fn render_plan_renders_section_description_lines() {
        let json = r#"{
  "schema_version": 3,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Title track-a",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T01:00:00Z",
  "tasks": [
    { "id": "T001", "description": "Task", "status": "todo" }
  ],
  "plan": {
    "summary": [],
    "sections": [
      {
        "id": "S1",
        "title": "Section",
        "description": ["Describe the section goal", "Additional context"],
        "task_ids": ["T001"]
      }
    ]
  }
}"#;
        let (track, _) = codec::decode(json).unwrap();
        let rendered = render_plan(&track);
        assert!(
            rendered.contains("Describe the section goal"),
            "first description line missing:\n{rendered}"
        );
        assert!(
            rendered.contains("Additional context"),
            "second description line missing:\n{rendered}"
        );
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

    // --- T011/T012: registry / snapshot boundary tests ---

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
        // Regression guard for the T007 codec-strip follow-up: after the
        // declaration codec stopped surfacing inline signals, the rendered
        // `<layer>-types.md` lost its signal-column emojis and fell back to
        // `—`. `sync_rendered_views` must read the companion
        // `<layer>-type-signals.json` file and populate `doc.signals()`
        // before rendering so the markdown reflects the evaluated state.
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
        // `spec_states::evaluate_layer_catalogue` (T005); the renderer
        // just avoids misrepresenting the state to a reviewer.
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
}
