//! Chain-2 (catalogue → spec) pair enumeration helpers.
//!
//! These free functions are called by [`super::RefVerifyPairSourceAdapter`] and are
//! separated from `pair_source.rs` (Chain-1 + shared helpers) to keep each module
//! within the 700-line production-code limit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use domain::ContentHash;
use domain::tddd::LayerId;
use usecase::ref_verify::{RefVerifyCacheScope, RefVerifyError, RefVerifyPair};

use crate::verify::plan_artifact_refs::{build_element_map, canonical_json};
use crate::verify::tddd_layers::{TdddLayerBinding, parse_tddd_layers};

use crate::track::symlink_guard::reject_symlinks_below;

use super::guarded_io::{read_guarded_text, resolve_and_guard_path};
use super::pair_source::hash_text;

/// Enumerate Chain-2 pairs for a single layer.
pub(super) fn enumerate_chain2_pairs_for_layer(
    track_dir: &Path,
    project_root: &Path,
    layer: LayerId,
) -> Result<Vec<RefVerifyPair>, RefVerifyError> {
    let catalogue_file = catalogue_file_for_layer(project_root, &layer)?;
    enumerate_chain2_pairs_for_catalogue(track_dir, project_root, layer, &catalogue_file)
}

fn enumerate_chain2_pairs_for_catalogue(
    track_dir: &Path,
    project_root: &Path,
    layer: LayerId,
    catalogue_file: &str,
) -> Result<Vec<RefVerifyPair>, RefVerifyError> {
    let layer_str = layer.as_ref();
    let catalogue_path = track_dir.join(catalogue_file);

    let catalogue_exists = reject_symlinks_below(&catalogue_path, project_root).map_err(|e| {
        RefVerifyError::VerifierPort {
            message: format!("cannot inspect catalogue '{}': {e}", catalogue_path.display()),
        }
    })?;
    if !catalogue_exists {
        return Err(RefVerifyError::VerifierPort {
            message: format!(
                "catalogue file for layer '{layer_str}' not found at '{}'; \
                 cannot verify Chain-2 pairs for a declared TDDD layer",
                catalogue_path.display()
            ),
        });
    }
    let catalogue_text = read_guarded_text(&catalogue_path, project_root).map_err(|e| {
        RefVerifyError::VerifierPort {
            message: format!("cannot read catalogue '{}': {e}", catalogue_path.display()),
        }
    })?;
    let crate_name_guess = layer_str;
    let catalogue = crate::tddd::catalogue_document_codec::CatalogueDocumentCodec::decode(
        &catalogue_text,
        crate_name_guess,
    )
    .map_err(|e| RefVerifyError::VerifierPort {
        message: format!("catalogue decode error for layer '{layer_str}': {e:?}"),
    })?;

    // Parse catalogue as raw JSON for canonical per-entry hashes
    // (ADR D4: Chain-2 claim_hash = SHA-256 of canonical JSON subtree of the catalogue entry).
    let catalogue_raw: serde_json::Value =
        serde_json::from_str(&catalogue_text).map_err(|e| RefVerifyError::VerifierPort {
            message: format!("catalogue '{layer_str}' raw JSON parse error: {e}"),
        })?;

    let mut pairs: Vec<RefVerifyPair> = Vec::new();
    let cache_scope = RefVerifyCacheScope::CatalogueSpec { layer: layer.clone() };
    let mut spec_cache: HashMap<PathBuf, Chain2SpecEvidence> = HashMap::new();

    for entry in usecase::catalogue_traversal::iter_catalogue_entries(&catalogue) {
        // Determine the JSON section key for this entry (types, traits, or functions).
        let section = entry.section_key.split(':').next().unwrap_or("types");
        let entry_key = &entry.key;

        // ADR D4: claim_hash = SHA-256 of the canonical JSON subtree of the catalogue entry.
        let entry_value = catalogue_raw
            .get(section)
            .and_then(|s| s.get(entry_key.as_str()))
            .ok_or_else(|| RefVerifyError::VerifierPort {
                message: format!(
                    "catalogue entry '{entry_key}' not found in section '{section}' of raw JSON \
                     (internal consistency error for layer '{layer_str}')"
                ),
            })?;
        let canonical_entry_json = canonical_json(entry_value);
        let claim_hash = hash_text(&canonical_entry_json)?;
        let claim_text = format!(
            "[{}] {}\n\nCatalogue entry canonical JSON:\n{}",
            entry.section_key, entry_key, canonical_entry_json
        );

        for spec_ref in entry.spec_refs {
            let anchor = spec_ref.anchor.as_ref();
            let (evidence_text, evidence_hash) = load_chain2_spec_evidence(
                &mut spec_cache,
                project_root,
                &spec_ref.file,
                anchor,
                entry_key,
            )?;

            pairs.push(RefVerifyPair {
                claim: claim_text.clone(),
                evidence: evidence_text,
                claim_hash: claim_hash.clone(),
                evidence_hash,
                cache_scope: cache_scope.clone(),
                known_bad: false,
            });
        }
    }

    Ok(pairs)
}

struct Chain2SpecEvidence {
    spec_doc: domain::SpecDocument,
    element_map: HashMap<String, String>,
}

fn load_chain2_spec_evidence(
    spec_cache: &mut HashMap<PathBuf, Chain2SpecEvidence>,
    project_root: &Path,
    spec_file: &Path,
    anchor: &str,
    entry_key: &str,
) -> Result<(String, ContentHash), RefVerifyError> {
    let context = format!("Chain-2 spec_ref '{}'", spec_file.display());
    let spec_path = resolve_and_guard_path(project_root, spec_file, &context)?;
    if !spec_cache.contains_key(&spec_path) {
        spec_cache.insert(spec_path.clone(), load_chain2_spec_file(&spec_path, project_root)?);
    }
    let loaded = spec_cache.get(&spec_path).ok_or_else(|| RefVerifyError::VerifierPort {
        message: format!("internal spec cache error for '{}'", spec_path.display()),
    })?;

    let evidence_text = find_spec_element_text(&loaded.spec_doc, anchor).ok_or_else(|| {
        RefVerifyError::VerifierPort {
            message: format!(
                "spec element '{anchor}' referenced by catalogue entry '{entry_key}' not found in '{}'",
                spec_file.display()
            ),
        }
    })?;

    // ADR D4: evidence_hash = SHA-256 of the canonical JSON subtree of the referenced spec element.
    let canonical_spec_json =
        loaded.element_map.get(anchor).ok_or_else(|| RefVerifyError::VerifierPort {
            message: format!(
                "spec element '{anchor}' not found in element map for '{}' (internal consistency \
                 error): spec file and decoded document should agree on element ids",
                spec_file.display()
            ),
        })?;
    Ok((evidence_text, hash_text(canonical_spec_json)?))
}

fn load_chain2_spec_file(
    spec_path: &Path,
    project_root: &Path,
) -> Result<Chain2SpecEvidence, RefVerifyError> {
    let spec_text =
        read_guarded_text(spec_path, project_root).map_err(|e| RefVerifyError::VerifierPort {
            message: format!("cannot read Chain-2 spec ref '{}': {e}", spec_path.display()),
        })?;
    let spec_doc =
        crate::spec::codec::decode(&spec_text).map_err(|e| RefVerifyError::VerifierPort {
            message: format!("Chain-2 spec ref decode error for '{}': {e}", spec_path.display()),
        })?;

    let spec_raw: serde_json::Value =
        serde_json::from_str(&spec_text).map_err(|e| RefVerifyError::VerifierPort {
            message: format!(
                "Chain-2 spec ref raw JSON parse error for '{}': {e}",
                spec_path.display()
            ),
        })?;
    let element_map = build_element_map(&spec_raw);

    Ok(Chain2SpecEvidence { spec_doc, element_map })
}

/// Find the text of a spec element by its id string.
///
/// The element's section kind is included in the rendered text so the
/// Chain-2 verifier can interpret out_of_scope elements as exclusion
/// declarations instead of behavioral requirements.
pub(super) fn find_spec_element_text(
    spec_doc: &domain::SpecDocument,
    element_id: &str,
) -> Option<String> {
    let all_reqs: Vec<(&str, &domain::SpecRequirement)> = spec_doc
        .goal()
        .iter()
        .map(|r| ("goal", r))
        .chain(spec_doc.scope().in_scope().iter().map(|r| ("in_scope", r)))
        .chain(spec_doc.scope().out_of_scope().iter().map(|r| ("out_of_scope", r)))
        .chain(spec_doc.constraints().iter().map(|r| ("constraint", r)))
        .chain(spec_doc.acceptance_criteria().iter().map(|r| ("acceptance_criterion", r)))
        .collect();

    for (section, req) in all_reqs {
        if req.id().as_ref() == element_id {
            return Some(format!("[{section} {}] {}", req.id().as_ref(), req.text()));
        }
    }
    None
}

fn load_tddd_layer_bindings(project_root: &Path) -> Result<Vec<TdddLayerBinding>, RefVerifyError> {
    let rules_path = project_root.join("architecture-rules.json");
    let rules_text =
        read_guarded_text(&rules_path, project_root).map_err(|e| RefVerifyError::VerifierPort {
            message: format!(
                "cannot read architecture-rules.json at '{}': {e}",
                rules_path.display()
            ),
        })?;
    let bindings = parse_tddd_layers(&rules_text).map_err(|e| RefVerifyError::VerifierPort {
        message: format!("architecture-rules.json parse error at '{}': {e}", rules_path.display()),
    })?;
    Ok(bindings)
}

fn catalogue_file_for_layer(
    project_root: &Path,
    layer: &LayerId,
) -> Result<String, RefVerifyError> {
    let layer_str = layer.as_ref();
    let bindings = load_tddd_layer_bindings(project_root)?;
    bindings
        .into_iter()
        .find(|binding| binding.layer_id() == layer_str)
        .map(|binding| binding.catalogue_file().to_owned())
        .ok_or_else(|| RefVerifyError::VerifierPort {
            message: format!(
                "layer '{layer_str}' not found or not tddd.enabled in architecture-rules.json"
            ),
        })
}

/// Enumerate Chain-2 pairs for all TDDD-enabled layers declared in architecture-rules.json.
///
/// The function handles two legal states for the catalogue set:
///
/// - **All absent** (pre-Phase-2 run): zero pairs are returned without error. This is the
///   legal pre-Phase-2 path where `spec.json` exists but no type catalogue has been authored yet.
/// - **All present**: pairs are enumerated for every layer.
///
/// A partial catalogue set (some present, some absent) is fail-closed — the scope resolver
/// rejects it before this function is called, and this function re-checks defensively so that
/// a catalogue disappearing between resolution and loading also triggers an error rather than
/// silently under-verifying.
pub(super) fn enumerate_chain2_all_layers(
    track_dir: &Path,
    project_root: &Path,
) -> Result<Vec<RefVerifyPair>, RefVerifyError> {
    let mut pairs: Vec<RefVerifyPair> = Vec::new();
    let _ = super::guarded_io::guarded_track_dir_entry_names(track_dir, project_root)?;

    // No architecture-rules.json → no TDDD layers declared → zero Chain-2 pairs.
    // This mirrors the scope resolver's `load_bindings_or_empty` which also returns an
    // empty set when architecture-rules.json is absent, allowing pre-Phase-0 repos to
    // run All scope.
    let rules_path = project_root.join("architecture-rules.json");
    let rules_exist = reject_symlinks_below(&rules_path, project_root).map_err(|e| {
        RefVerifyError::VerifierPort {
            message: format!(
                "cannot inspect architecture-rules.json at '{}': {e}",
                rules_path.display()
            ),
        }
    })?;
    if !rules_exist {
        return Ok(pairs);
    }

    let bindings = load_tddd_layer_bindings(project_root)?;

    // First pass: determine whether we are in the all-present or all-absent state.
    // Fail closed if we observe a partial set (some present, some absent).
    // Use `reject_symlinks_below` (symlink-aware) for all presence checks so that
    // dangling catalogue symlinks are treated as errors (fail-closed) rather than
    // as absent files, consistent with the symlink-hardening contract in the briefing.
    let mut present_count = 0usize;
    let mut absent_count = 0usize;
    for binding in &bindings {
        let catalogue_path = track_dir.join(binding.catalogue_file());
        let exists = reject_symlinks_below(&catalogue_path, project_root).map_err(|e| {
            RefVerifyError::VerifierPort {
                message: format!("cannot inspect catalogue '{}': {e}", catalogue_path.display()),
            }
        })?;
        if exists {
            present_count += 1;
        } else {
            absent_count += 1;
        }
    }

    if present_count > 0 && absent_count > 0 {
        // Partial catalogue set — fail closed to avoid silent under-verification.
        // Rebuild the missing list using `reject_symlinks_below` for consistency
        // with the presence checks above (dangling symlinks → error, not absent).
        let mut missing: Vec<String> = Vec::new();
        for binding in &bindings {
            let catalogue_path = track_dir.join(binding.catalogue_file());
            let present = reject_symlinks_below(&catalogue_path, project_root).map_err(|e| {
                RefVerifyError::VerifierPort {
                    message: format!(
                        "cannot inspect catalogue '{}': {e}",
                        catalogue_path.display()
                    ),
                }
            })?;
            if !present {
                missing.push(binding.catalogue_file().to_owned());
            }
        }
        return Err(RefVerifyError::VerifierPort {
            message: format!(
                "partial TDDD catalogue set for All-scope run — missing: {}",
                missing.join(", ")
            ),
        });
    }

    if absent_count == bindings.len() && !bindings.is_empty() {
        // All catalogues absent: pre-Phase-2 path, contribute zero Chain-2 pairs.
        return Ok(pairs);
    }

    // Second pass: all catalogues are present — enumerate pairs for each layer.
    for binding in bindings {
        let layer_str = binding.layer_id();
        let layer =
            LayerId::try_new(layer_str.to_owned()).map_err(|e| RefVerifyError::VerifierPort {
                message: format!("invalid layer id '{layer_str}' in architecture-rules.json: {e}"),
            })?;
        pairs.extend(enumerate_chain2_pairs_for_catalogue(
            track_dir,
            project_root,
            layer,
            binding.catalogue_file(),
        )?);
    }

    Ok(pairs)
}

/// Return a `RefVerifyCacheScope::CatalogueSpec` for the first TDDD-enabled layer declared in
/// `architecture-rules.json`, or `None` if no TDDD layers exist or the rules file is absent.
///
/// Returns `Err` if the rules file exists but cannot be read, parsed, or yields an invalid
/// layer id — callers must propagate the error rather than silently falling back to `SpecAdr`,
/// which would route Chain-2 known-bad probes through Chain-1 and leave Chain-2 uncalibrated.
///
/// Used by the probe injector to route known-bad probes through Chain2 during All-scope runs.
pub(super) fn first_tddd_layer_scope(
    project_root: &Path,
) -> Result<Option<RefVerifyCacheScope>, RefVerifyError> {
    let rules_path = project_root.join("architecture-rules.json");
    match reject_symlinks_below(&rules_path, project_root) {
        Ok(false) => return Ok(None),
        Ok(true) => {}
        Err(e) => {
            return Err(RefVerifyError::VerifierPort {
                message: format!(
                    "cannot check existence of architecture-rules.json at '{}': {e}",
                    rules_path.display()
                ),
            });
        }
    }
    let bindings = load_tddd_layer_bindings(project_root)?;
    let Some(binding) = bindings.into_iter().next() else {
        return Ok(None);
    };
    let layer = LayerId::try_new(binding.layer_id().to_owned()).map_err(|e| {
        RefVerifyError::VerifierPort {
            message: format!(
                "invalid layer id '{}' in architecture-rules.json: {e}",
                binding.layer_id()
            ),
        }
    })?;
    Ok(Some(RefVerifyCacheScope::CatalogueSpec { layer }))
}
