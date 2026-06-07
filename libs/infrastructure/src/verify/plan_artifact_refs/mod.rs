//! Verify structured-ref fields in spec.json and catalogue entries, plus
//! task-coverage enforcement and canonical-block suspicion detection.
//!
//! This module validates plan artifact references in a track directory:
//! - `spec.json` ref fields: `adr_refs`, `convention_refs`, `informal_grounds`,
//!   `related_conventions`, and per-catalogue-entry `spec_refs` / `informal_grounds`
//! - File existence for file-based refs (`AdrRef.file`, `ConventionRef.file`, `SpecRef.file`)
//! - `SpecRef.anchor` must resolve to a real element id inside the target `spec.json`
//! - `SpecRef.hash` must match the SHA-256 of the canonical JSON subtree
//! - `AdrAnchor` / `ConventionAnchor` loose non-empty validation (already enforced by newtypes)
//! - `InformalGroundRef` newtype validation (kind variant + non-empty summary)
//! - `task-coverage.json` coverage enforcement (in_scope + acceptance_criteria must have task_refs)
//! - `task-coverage.json` referential integrity (all 4 sections: element ids in spec, task ids in impl-plan)
//! - Canonical-block suspicion detection in `plan.md` (warning only)
//!
//! Per ADR 2026-04-19-1242 §D2.3 / §D3.3.

mod canonical_block;
mod spec_refs;
mod task_coverage;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use domain::SpecRef;
use domain::verify::{VerifyFinding, VerifyOutcome};
use thiserror::Error;

use crate::spec::codec as spec_codec;
use crate::tddd::catalogue_document_codec::CatalogueDocumentCodec;
use crate::track::symlink_guard;

// Re-export items used by tests (via `super::*`) and by external callers.
#[cfg(test)]
pub(crate) use spec_refs::canonical_json;
pub(crate) use spec_refs::{SpecElementMap, build_element_map, canonical_json_sha256};

/// Errors specific to the `plan-artifact-refs` verifier.
///
/// These are surfaced as `VerifyFinding::error` entries in the outcome,
/// not propagated directly to callers.
#[derive(Debug, Error)]
pub enum PlanArtifactRefsError {
    /// JSON parse failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// I/O failure while reading a file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A `SpecRef.anchor` does not resolve to any element in the target spec.
    #[error("unresolved SpecRef anchor '{anchor}' in file '{}'", file.display())]
    UnresolvedSpecRef { file: PathBuf, anchor: String },

    /// `SpecRef.hash` does not match the actual SHA-256 of the element subtree.
    #[error(
        "SpecRef hash mismatch for anchor '{anchor}' in '{}': expected {expected}, actual {actual}",
        file.display()
    )]
    SpecHashMismatch { file: PathBuf, anchor: String, expected: String, actual: String },

    /// An anchor value failed validation (non-empty check).
    #[error("invalid anchor '{anchor}' in file '{}'", file.display())]
    InvalidAnchor { file: PathBuf, anchor: String },

    /// A task-coverage coverage or referential-integrity constraint was violated (T011).
    #[error("coverage violation: {0}")]
    CoverageViolation(String),
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Verify all structured-ref fields in a track directory.
///
/// Reads `spec.json` plus all layer type catalogues
/// (`domain-types.json`, `usecase-types.json`, `infrastructure-types.json`)
/// when present. Validates:
/// - Schema decode of every ref field (malformed refs → error finding)
/// - File existence for file-based refs
/// - `SpecRef.anchor` resolution in the target spec.json
/// - `SpecRef.hash` matching the canonical JSON subtree SHA-256
/// - `AdrAnchor` / `ConventionAnchor` loose validation (non-empty, enforced by newtype)
/// - `InformalGroundRef` newtype validation (kind + non-empty summary)
///
/// Skips silently when `spec.json` is absent (track not yet past Phase 1).
///
/// # Errors
///
/// Returns error-level findings when any ref field fails the checks above.
pub fn verify(track_dir: &Path) -> VerifyOutcome {
    // Absolutize `track_dir` without following symlinks (CWD join, no resolution).
    // All subsequent file paths are built from this absolute, lexical path so that
    // `reject_symlinks_below` can correctly compare ancestor chains.
    let abs_track_dir = super::trusted_root::absolutize(track_dir);

    // Resolve the trusted_root via the shared resolver, which uses git discovery
    // (`SystemGitRepo::discover()`) anchored at the actual repository root and
    // verifies the result is not itself a symlink. This is more robust than
    // manually walking N parent directories (which depends on `track_dir` being
    // exactly 3 levels deep) and catches symlinked intermediate path components
    // that a parent-walk would miss.
    let spec_json_path_for_discovery = abs_track_dir.join("spec.json");
    let trusted_root =
        match super::trusted_root::resolve_trusted_root(&spec_json_path_for_discovery) {
            Ok(r) => r,
            Err(e) => {
                return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "Refusing to verify: trusted_root resolution failed for track '{}': {e}",
                    track_dir.display()
                ))]);
            }
        };

    // Build catalogue-local paths from the absolutized track dir.
    // Repo-relative ref paths are resolved against `trusted_root` directly (inside
    // `resolve_path`), not against a derived parent walk, so both the containment
    // check and the symlink guard use the same root.
    let spec_json_path = abs_track_dir.join("spec.json");

    // -----------------------------------------------------------------------
    // 1. Load and validate spec.json (symlink-guarded)
    //
    // Run the symlink guard BEFORE the existence check so that a symlinked
    // (non-regular) `spec.json` is rejected rather than being treated as
    // absent. `Path::is_file()` follows symlinks and would return `false` for
    // a symlink-to-directory, silently skipping all artifact checks. By
    // calling `reject_symlinks_below` first, any symlink at or above the
    // path returns an error finding, and only a missing (non-existent) file
    // produces `pass()`.
    // -----------------------------------------------------------------------
    match symlink_guard::reject_symlinks_below(&spec_json_path, &trusted_root) {
        Ok(true) => {}
        Ok(false) => {
            // spec.json absent — track not yet past Phase 1; skip silently.
            return VerifyOutcome::pass();
        }
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "spec.json symlink guard: {e}"
            ))]);
        }
    }

    let mut findings: Vec<VerifyFinding> = Vec::new();

    let spec_content = match std::fs::read_to_string(&spec_json_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot read spec.json: {e}"
            ))]);
        }
    };

    // The spec codec already validates all ref newtypes (AdrAnchor, ConventionAnchor,
    // InformalGroundKind, InformalGroundSummary, SpecElementId).
    // Codec errors here surface malformed ref JSON.
    let spec_doc = match spec_codec::decode(&spec_content) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot parse spec.json: {e}"
            ))]);
        }
    };

    // -----------------------------------------------------------------------
    // 2. Build a look-up map from spec element id → raw JSON subtree
    //    for SpecRef.anchor resolution and hash verification.
    //    This is loaded lazily per referenced spec file.
    // -----------------------------------------------------------------------
    let mut spec_element_cache: HashMap<PathBuf, SpecElementMap> = HashMap::new();

    // -----------------------------------------------------------------------
    // 3. Validate file-based refs in spec.json requirement sections
    // -----------------------------------------------------------------------
    let all_requirements: Vec<&domain::SpecRequirement> = spec_doc
        .goal()
        .iter()
        .chain(spec_doc.scope().in_scope().iter())
        .chain(spec_doc.scope().out_of_scope().iter())
        .chain(spec_doc.constraints().iter())
        .chain(spec_doc.acceptance_criteria().iter())
        .collect();

    for req in &all_requirements {
        for adr_ref in req.adr_refs() {
            spec_refs::check_ref_file(
                &adr_ref.file,
                &trusted_root,
                "spec.json adr_ref",
                &mut findings,
            );
        }
        for conv_ref in req.convention_refs() {
            spec_refs::check_ref_file(
                &conv_ref.file,
                &trusted_root,
                "spec.json convention_ref",
                &mut findings,
            );
        }
        // InformalGroundRef: no file path; kind+summary already validated by newtype constructor.
    }

    // Validate top-level related_conventions
    for conv_ref in spec_doc.related_conventions() {
        spec_refs::check_ref_file(
            &conv_ref.file,
            &trusted_root,
            "spec.json related_conventions",
            &mut findings,
        );
    }

    // -----------------------------------------------------------------------
    // 4. Validate SpecRef fields in type catalogue entries
    //    Walk domain-, usecase-, infrastructure-types.json
    // -----------------------------------------------------------------------
    let catalogue_files = ["domain-types.json", "usecase-types.json", "infrastructure-types.json"];

    for catalogue_name in &catalogue_files {
        let catalogue_path = abs_track_dir.join(catalogue_name);

        // Symlink-guard catalogue file before reading.
        match symlink_guard::reject_symlinks_below(&catalogue_path, &trusted_root) {
            Ok(false) => continue, // catalogue absent — layer not active
            Ok(true) => {}
            Err(e) => {
                findings.push(VerifyFinding::error(format!("{catalogue_name} symlink guard: {e}")));
                continue;
            }
        }

        let catalogue_content = match std::fs::read_to_string(&catalogue_path) {
            Ok(c) => c,
            Err(e) => {
                findings.push(VerifyFinding::error(format!("Cannot read {catalogue_name}: {e}")));
                continue;
            }
        };

        // T024: v3-native decode via `CatalogueDocumentCodec::decode`.
        // Non-v3 catalogues surface as an error finding (CN-11 fail-closed).
        let stem = std::path::Path::new(catalogue_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .strip_suffix("-types.json")
            .unwrap_or_else(|| {
                std::path::Path::new(catalogue_name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
            })
            .to_owned();
        let catalogue_doc = match CatalogueDocumentCodec::decode(&catalogue_content, &stem) {
            Ok(d) => d,
            Err(e) => {
                findings
                    .push(VerifyFinding::error(format!("Cannot parse {catalogue_name}: {e:?}")));
                continue;
            }
        };

        // Iterate types → traits → functions (BTreeMap order).
        // Helper: process spec_refs for one entry name + slice.
        let mut process_entry_refs = |entry_name: &str, spec_refs_slice: &[SpecRef]| {
            for spec_ref in spec_refs_slice {
                // Path-traversal guard: resolve the ref path and verify containment.
                let resolved = match spec_refs::resolve_path(&trusted_root, &spec_ref.file) {
                    None => {
                        findings.push(VerifyFinding::error(format!(
                            "{catalogue_name} entry '{entry_name}': spec_ref has invalid path \
                             (absolute or path-traversal): {}",
                            spec_ref.file.display()
                        )));
                        continue;
                    }
                    Some(p) => p,
                };

                // Symlink-guard and existence check for the referenced spec file.
                match symlink_guard::reject_symlinks_below(&resolved, &trusted_root) {
                    Ok(false) => {
                        findings.push(VerifyFinding::error(format!(
                            "{catalogue_name} entry '{entry_name}': spec_ref file not found: {}",
                            spec_ref.file.display()
                        )));
                        continue;
                    }
                    Ok(true) => {}
                    Err(e) => {
                        findings.push(VerifyFinding::error(format!(
                            "{catalogue_name} entry '{entry_name}': spec_ref symlink guard for '{}': {e}",
                            spec_ref.file.display()
                        )));
                        continue;
                    }
                }

                // Load and cache the referenced spec.json element map
                let element_map = match spec_refs::load_spec_element_map(
                    &mut spec_element_cache,
                    &resolved,
                    &trusted_root,
                ) {
                    Ok(m) => m,
                    Err(e) => {
                        findings.push(VerifyFinding::error(format!(
                            "{catalogue_name} entry '{entry_name}': cannot load spec file '{}': {e}",
                            spec_ref.file.display()
                        )));
                        continue;
                    }
                };

                // Anchor resolution: SpecRef.anchor must map to an element in the spec
                let anchor_str = spec_ref.anchor.as_ref();
                match element_map.get(anchor_str) {
                    None => {
                        findings.push(VerifyFinding::error(format!(
                            "{catalogue_name} entry '{entry_name}': unresolved SpecRef anchor \
                             '{anchor_str}' in '{}'",
                            spec_ref.file.display()
                        )));
                    }
                    Some(subtree_json) => {
                        // Hash verification: compare stored hash to actual SHA-256
                        let actual_hash = spec_refs::canonical_json_sha256(subtree_json);
                        let expected_hash = spec_ref.hash.to_hex();
                        if actual_hash != expected_hash {
                            findings.push(VerifyFinding::error(format!(
                                "{catalogue_name} entry '{entry_name}': SpecRef hash mismatch for \
                                 anchor '{anchor_str}' in '{}': expected {expected_hash}, actual \
                                 {actual_hash}",
                                spec_ref.file.display()
                            )));
                        }
                    }
                }
            }
            // InformalGroundRef: kind+summary already validated by codec
        };

        for (type_name, entry) in &catalogue_doc.types {
            process_entry_refs(type_name.as_str(), &entry.spec_refs);
        }
        for (trait_name, entry) in &catalogue_doc.traits {
            process_entry_refs(trait_name.as_str(), &entry.spec_refs);
        }
        for (fn_path, entry) in &catalogue_doc.functions {
            process_entry_refs(&fn_path.to_string(), &entry.spec_refs);
        }
    }

    // -----------------------------------------------------------------------
    // 5. task-coverage.json coverage enforcement + referential integrity
    //    Migrated from spec_coverage::verify (T011).
    //    When task-coverage.json is absent: warn (soft failure for now).
    //    When present: enforce coverage + referential integrity.
    // -----------------------------------------------------------------------
    let task_coverage_path = abs_track_dir.join("task-coverage.json");
    match symlink_guard::reject_symlinks_below(&task_coverage_path, &trusted_root) {
        Ok(false) => {
            // task-coverage.json absent — emit warning, not error.
            // task-coverage.json is optional for existing tracks; coverage
            // enforcement is deferred when the file is absent.
            // impl-plan.json integrity is only enforced when task-coverage.json
            // is present (integrity validates task refs that appear in coverage;
            // no coverage → no refs to validate).
            findings.push(VerifyFinding::warning(
                "task-coverage.json absent — coverage enforcement deferred",
            ));
        }
        Ok(true) => {
            // task-coverage.json present — run coverage + referential integrity.
            task_coverage::verify_task_coverage(
                &abs_track_dir,
                &task_coverage_path,
                &spec_doc,
                &trusted_root,
                &mut findings,
            );
        }
        Err(e) => {
            findings.push(VerifyFinding::error(format!("task-coverage.json symlink guard: {e}")));
        }
    }

    // -----------------------------------------------------------------------
    // 6. Canonical-block suspicion detection in plan.md
    //    Warning-only: long fenced code blocks >10 lines that lack an
    //    "example" marker may be canonical blocks leaking into rendered docs.
    //    observations.md is NOT scanned here — it is a free-form optional
    //    manual observation log and authors may include code-like content
    //    at their discretion.
    // -----------------------------------------------------------------------
    let plan_path = abs_track_dir.join("plan.md");
    match symlink_guard::reject_symlinks_below(&plan_path, &trusted_root) {
        Ok(false) => {} // absent — skip silently
        Ok(true) => {
            canonical_block::scan_canonical_block_suspicion(&plan_path, "plan.md", &mut findings);
        }
        Err(e) => {
            // Symlink-guard failures are security controls: fail at error level,
            // consistent with all other symlink-guard checks in this verifier.
            findings.push(VerifyFinding::error(format!(
                "plan.md symlink guard (canonical-block scan): {e}"
            )));
        }
    }

    if findings.is_empty() { VerifyOutcome::pass() } else { VerifyOutcome::from_findings(findings) }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
#[path = "../plan_artifact_refs_tests.rs"]
mod tests;
