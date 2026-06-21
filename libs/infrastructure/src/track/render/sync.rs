//! Synchronization of rendered views (`plan.md`, `spec.md`, `*-types.md`, `registry.md`)
//! from track metadata and type catalogue sources.

use std::path::{Path, PathBuf};

use super::super::atomic_write::atomic_write_file;
use super::super::codec;
use super::RenderError;
use super::TRACK_ITEMS_DIR;
use super::contract_map::render_contract_map_view;
use super::plan::render_plan;
use super::registry::render_registry;
use super::snapshot::{
    TrackSchemaPeek, collect_track_snapshots, decode_legacy_metadata, load_impl_plan_opt,
    load_task_coverage_opt,
};
use crate::git_cli::{GitRepository, SystemGitRepo};
use crate::spec;
use crate::tddd::catalogue_document_codec::{CatalogueDocumentCodec, CatalogueDocumentCodecError};
use crate::tddd::type_signals_codec;
use crate::type_catalogue_render;
use crate::verify::tddd_layers::{LoadTdddLayersError, load_tddd_layers};

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
        // Branch-based guard (IN-04 / CN-01 / CN-03): only render spec.md and
        // <layer>-types.md when the current git branch matches the track's configured
        // branch (`track/<id>`). This replaces the former `is_done_or_archived`
        // status guard (done|archived skip) — the protection criterion moves from
        // "status=done|archived" to "branch does not match current git branch".
        //
        // plan.md is rendered unconditionally (outside the guard), because
        // task-state transitions must always reflect the latest impl-plan.json.
        // The branch guard applies only to spec.md and <layer>-types.md.
        //
        // Fail-closed (CN-01): if git discovery fails or the branch cannot be read,
        // return `RenderError::InvalidTrackMetadata` rather than skipping silently
        // when a protected render input (`spec.json` or `*-types.json`) is present.
        // This error is deferred until after plan.md so task state is still
        // reflected before protected renders are skipped.
        let branch_guard_result: Result<bool, String> = {
            // Derive the expected branch from metadata (track/<id>).
            let expected_branch = format!("track/{}", parsed.id);
            // Discover the current git branch from the workspace root (root) via
            // the GitRepository port (IN-06 / AC-15). Filters out detached HEAD
            // ("HEAD") and empty strings to preserve the original guard semantics.
            let current_branch_opt: Option<String> = SystemGitRepo::discover_from(root)
                .ok()
                .and_then(|r| r.current_branch().ok())
                .flatten()
                .filter(|b| !b.is_empty() && b != "HEAD");
            match current_branch_opt {
                Some(ref branch) => Ok(branch == &expected_branch),
                None => Err(format!(
                    "cannot determine current git branch for branch-based \
                     guard on track '{}' (fail-closed, CN-01)",
                    parsed.id
                )),
            }
        };
        let impl_plan = load_impl_plan_opt(&track_dir)?;
        let rendered = render_plan(&track, impl_plan.as_ref());
        let plan_path = track_dir.join("plan.md");
        let old = match std::fs::read_to_string(&plan_path) {
            Ok(content) => Some(content),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(RenderError::Io(e)),
        };
        if old
            .as_deref()
            .is_none_or(|existing| !super::rendered_matches(existing, rendered.as_ref()))
        {
            atomic_write_file(&plan_path, rendered.as_bytes())?;
            changed.push(plan_path);
        }

        // Render spec.md from spec.json if present. Skipped when the current git
        // branch does not match the track's configured branch (`track/<id>`):
        // this protects non-current-branch tracks from being overwritten while
        // still allowing full re-renders for the active track regardless of status.
        //
        let spec_json_path = track_dir.join("spec.json");
        let branch_matches_track_for_spec: bool = if spec_json_path.is_file() {
            match &branch_guard_result {
                Ok(v) => *v,
                Err(reason) => {
                    return Err(RenderError::InvalidTrackMetadata {
                        path: metadata_path.clone(),
                        reason: reason.clone(),
                    });
                }
            }
        } else {
            // No spec.json — nothing to protect here; types rendering handles its
            // own protected-input check below.
            false
        };
        if branch_matches_track_for_spec && spec_json_path.is_file() {
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
                        .is_none_or(|existing| !super::rendered_matches(existing, &rendered_spec))
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
        // existing `parse_tddd_layers` resolver. Only renders when the current
        // git branch matches the track's configured branch (branch_matches_track).
        // This replaces the former `is_done_or_archived` guard with a branch-based
        // validation (IN-04 / CN-01 / CN-03 / CN-04).
        let load_type_bindings = || {
            let arch_rules_path = root.join("architecture-rules.json");
            // `load_tddd_layers` is fail-closed. Missing / symlinked / malformed
            // `architecture-rules.json` are all hard configuration errors —
            // never synthesize a fallback nor silently skip the layer iteration.
            load_tddd_layers(&arch_rules_path, root).map_err(|e| match e {
                LoadTdddLayersError::Io { source, .. } => RenderError::Io(source),
                LoadTdddLayersError::Parse(err) => RenderError::Io(std::io::Error::other(format!(
                    "architecture-rules.json: {err}"
                ))),
            })
        };
        let catalogue_path_exists = |path: &Path| -> Result<bool, RenderError> {
            match path.symlink_metadata() {
                Ok(metadata) => {
                    let file_type = metadata.file_type();
                    Ok(file_type.is_file() || file_type.is_symlink())
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(e) => Err(RenderError::Io(e)),
            }
        };
        let has_default_type_catalogue_input = || -> Result<bool, RenderError> {
            for entry in std::fs::read_dir(&track_dir)? {
                let entry = entry?;
                let file_name = entry.file_name();
                let Some(file_name) = file_name.to_str() else {
                    continue;
                };
                if !file_name.ends_with("-types.json") {
                    continue;
                }
                if catalogue_path_exists(&entry.path())? {
                    return Ok(true);
                }
            }
            Ok(false)
        };
        let has_type_catalogue_input = || -> Result<bool, RenderError> {
            match load_type_bindings() {
                Ok(bindings) => {
                    for binding in &bindings {
                        if catalogue_path_exists(&track_dir.join(binding.catalogue_file()))? {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                }
                Err(_) => has_default_type_catalogue_input(),
            }
        };

        match &branch_guard_result {
            Ok(true) => {
                // Branch matches — render configured type catalogues.
                let bindings = load_type_bindings()?;

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
                            catalogue_path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_owned()
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
                            // the error message (`sotp signal calc-catalog-spec
                            // <track_id>`). Opt-out layers render the legacy
                            // 5-column view (None).
                            let v3_spec_signals_doc = if binding.catalogue_spec_signal_enabled() {
                                let spec_path =
                                    track_dir.join(binding.catalogue_spec_signal_file());
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
                            if old_md.as_deref().is_none_or(|existing| {
                                !super::rendered_matches(existing, &rendered)
                            }) {
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
            }
            Ok(false) => {
                // Branch mismatch protects type views by skipping their render.
            }
            Err(reason) => {
                if has_type_catalogue_input()? {
                    return Err(RenderError::InvalidTrackMetadata {
                        path: metadata_path.clone(),
                        reason: reason.clone(),
                    });
                }
            }
        }

        // Render `contract-map.md` unconditionally (outside the branch guard)
        // so the declaration relationship diagram stays fresh regardless of
        // track status or current branch.  The branch guard above protects
        // spec.md and <layer>-types.md (frozen views derived from phase-2 type
        // design artefacts).  `contract-map.md` is a *rendered graph* derived
        // from all catalogue data and the implementation renderer — it must
        // reflect the final post-implementation state, which may differ from the
        // state captured while the track was still `in_progress`.
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
    if old
        .as_deref()
        .is_none_or(|existing| !super::rendered_matches(existing, rendered_registry.as_ref()))
    {
        atomic_write_file(&registry_path, rendered_registry.as_bytes())?;
        changed.push(registry_path);
    }

    Ok(changed)
}
