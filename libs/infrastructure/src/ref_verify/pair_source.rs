//! Helper functions for enumerating Chain-1 and Chain-2 reference pairs.
//!
//! These free functions are called by [`super::RefVerifyPairSourceAdapter`] and are
//! separated here to keep the parent module within the 700-line production-code limit.

pub(super) use super::pair_source_json::{extract_json_object_parsed, render_prompt_template};

use crate::adr_decision::parse_adr_frontmatter;
use crate::verify::plan_artifact_refs::{build_element_map, canonical_json, canonical_json_sha256};
use crate::verify::tddd_layers::{TdddLayerBinding, parse_tddd_layers};
use domain::tddd::LayerId;
use domain::{AdrDecisionCommon, AdrDecisionEntry, ContentHash};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use usecase::ref_verify::{RefVerifyCacheScope, RefVerifyError, RefVerifyPair};

// ---------------------------------------------------------------------------
// Shared hash helper
// ---------------------------------------------------------------------------

/// Compute a `ContentHash` by SHA-256-hashing `text`.
pub(super) fn hash_text(text: &str) -> Result<ContentHash, RefVerifyError> {
    let hex = canonical_json_sha256(text);
    ContentHash::try_from_hex(hex)
        .map_err(|e| RefVerifyError::VerifierPort { message: format!("invalid content hash: {e}") })
}

/// Compute a SHA-256 Git blob-object hash for `text`.
///
/// `ContentHash` is a 32-byte value object, so Chain-1 stores the SHA-256 object-format
/// identity of the Git blob preimage: `blob <len>\0<bytes>`.
pub(super) fn hash_git_blob_text(text: &str) -> ContentHash {
    use sha2::Digest as _;

    let bytes = text.as_bytes();
    let mut hasher = sha2::Sha256::new();
    hasher.update(format!("blob {}\0", bytes.len()).as_bytes());
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    ContentHash::from_bytes(out)
}

// Path resolution / guarded reads live in `super::guarded_io`.
use super::guarded_io::{lexically_normalize, read_guarded_text, resolve_and_guard_path};

// ---------------------------------------------------------------------------
// Chain-1: spec → ADR
// ---------------------------------------------------------------------------

/// Enumerate Chain-1 pairs: for each spec requirement's adr_refs,
/// produce `(spec element text, ADR decision text)` pairs.
pub(super) fn enumerate_chain1_pairs(
    track_dir: &Path,
    project_root: &Path,
) -> Result<Vec<RefVerifyPair>, RefVerifyError> {
    let spec_path = track_dir.join("spec.json");

    let spec_text =
        read_guarded_text(&spec_path, project_root).map_err(|e| RefVerifyError::VerifierPort {
            message: format!("cannot read spec.json at '{}': {e}", spec_path.display()),
        })?;
    let spec_doc = crate::spec::codec::decode(&spec_text).map_err(|e| {
        RefVerifyError::VerifierPort { message: format!("spec.json decode error: {e}") }
    })?;

    // Build spec element map from raw JSON to compute canonical per-element hashes
    // (ADR D4: Chain-1 claim_hash = per-element SHA-256 of canonical JSON subtree).
    let spec_raw: serde_json::Value = serde_json::from_str(&spec_text).map_err(|e| {
        RefVerifyError::VerifierPort { message: format!("spec.json raw JSON parse error: {e}") }
    })?;
    let spec_element_map = build_element_map(&spec_raw);

    let mut pairs: Vec<RefVerifyPair> = Vec::new();

    // Collect all requirements from all sections, tagged with their section
    // kind. The section kind is part of the claim text so the verifier can
    // interpret out_of_scope elements as exclusion declarations ("we will NOT
    // do this") instead of misreading them as requirements.
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
        let id = req.id().as_ref();
        let claim_text = format!("[{section} {id}] {}", req.text());

        // ADR D4: claim_hash = SHA-256 of the canonical JSON subtree for this spec element.
        let canonical_element_json =
            spec_element_map.get(id).ok_or_else(|| RefVerifyError::VerifierPort {
                message: format!(
                    "spec element '{id}' not found in element map (internal consistency error)"
                ),
            })?;
        let claim_hash = hash_text(canonical_element_json)?;

        for adr_ref in req.adr_refs() {
            // Validate the reference is repo-relative before the guarded read.
            let adr_path = resolve_and_guard_path(
                project_root,
                &adr_ref.file,
                &format!("Chain-1 adr_ref '{}'", adr_ref.file.display()),
            )?;
            let (evidence_text, adr_raw) =
                read_adr_anchor_text(&adr_path, project_root, adr_ref.anchor.as_ref())
                    .map_err(|e| RefVerifyError::VerifierPort { message: e })?;

            // ADR D3/D4: the Chain-1 evidence key is the ADR file's Git blob-object identity.
            let evidence_hash = hash_git_blob_text(&adr_raw);
            pairs.push(RefVerifyPair {
                claim: claim_text.clone(),
                evidence: evidence_text,
                claim_hash: claim_hash.clone(),
                evidence_hash,
                cache_scope: RefVerifyCacheScope::SpecAdr,
                known_bad: false,
            });
        }
    }

    Ok(pairs)
}

/// Read a referenced ADR decision from YAML front-matter, and also return the
/// raw file content for use as the evidence hash source.
///
/// Returns `(decision_evidence_text, raw_file_text)`.
///
/// The reference is valid only when `anchor` appears in the typed
/// `decisions[].id` list from the ADR front-matter.  A matching markdown section
/// is included as extra verifier context when present, but headings are not the
/// source of reference validity.
///
/// When no front-matter decision matches, the function fails closed and returns an error describing
/// the unresolved anchor — returning the whole ADR would silently mask a broken
/// reference in spec.json and allow a structurally invalid link to produce a cached
/// semantic verdict.
///
/// The raw file text is always the complete file content — it is the basis for the
/// evidence_hash (ADR D3/D4: Chain-1 cache key uses the git blob hash of the ADR file).
pub(super) fn read_adr_anchor_text(
    adr_path: &Path,
    trusted_root: &Path,
    anchor: &str,
) -> Result<(String, String), String> {
    let raw_text = read_guarded_text(adr_path, trusted_root)
        .map_err(|e| format!("cannot read ADR '{}': {e}", adr_path.display()))?;

    let frontmatter = parse_adr_frontmatter(&raw_text)
        .map_err(|e| format!("cannot parse ADR front-matter for '{}': {e}", adr_path.display()))?;

    let decision = frontmatter.decisions().iter().find(|decision| decision_id(decision) == anchor);
    let decision = decision.ok_or_else(|| {
        format!("anchor '{}' not found in decisions[].id of ADR '{}'", anchor, adr_path.display())
    })?;

    let mut evidence_text = render_adr_decision_entry(decision);
    if let Some(section_text) = extract_markdown_decision_section(&raw_text, anchor) {
        evidence_text.push_str("\n\n");
        evidence_text.push_str(&section_text);
    }

    Ok((evidence_text, raw_text))
}

fn decision_common(decision: &AdrDecisionEntry) -> &AdrDecisionCommon {
    match decision {
        AdrDecisionEntry::ProposedDecision(decision) => &decision.common,
        AdrDecisionEntry::AcceptedDecision(decision) => &decision.common,
        AdrDecisionEntry::ImplementedDecision(decision) => &decision.common,
        AdrDecisionEntry::SupersededDecision(decision) => &decision.common,
        AdrDecisionEntry::DeprecatedDecision(decision) => &decision.common,
    }
}

fn decision_id(decision: &AdrDecisionEntry) -> &str {
    decision_common(decision).id()
}

fn decision_status(decision: &AdrDecisionEntry) -> &'static str {
    match decision {
        AdrDecisionEntry::ProposedDecision(_) => "proposed",
        AdrDecisionEntry::AcceptedDecision(_) => "accepted",
        AdrDecisionEntry::ImplementedDecision(_) => "implemented",
        AdrDecisionEntry::SupersededDecision(_) => "superseded",
        AdrDecisionEntry::DeprecatedDecision(_) => "deprecated",
    }
}

fn render_adr_decision_entry(decision: &AdrDecisionEntry) -> String {
    let common = decision_common(decision);
    let mut lines = vec![
        format!("ADR decision [{}]", common.id()),
        format!("status: {}", decision_status(decision)),
    ];

    if let Some(user_decision_ref) = common.user_decision_ref() {
        lines.push(format!("user_decision_ref: {user_decision_ref}"));
    }
    if let Some(review_finding_ref) = common.review_finding_ref() {
        lines.push(format!("review_finding_ref: {review_finding_ref}"));
    }
    if let Some(candidate_selection) = common.candidate_selection() {
        lines.push(format!("candidate_selection: {candidate_selection}"));
    }
    if common.grandfathered() {
        lines.push("grandfathered: true".to_owned());
    }
    match decision {
        AdrDecisionEntry::ImplementedDecision(decision) => {
            lines.push(format!("implemented_in: {}", decision.implemented_in()));
        }
        AdrDecisionEntry::SupersededDecision(decision) => {
            lines.push(format!("superseded_by: {}", decision.superseded_by()));
        }
        AdrDecisionEntry::ProposedDecision(_)
        | AdrDecisionEntry::AcceptedDecision(_)
        | AdrDecisionEntry::DeprecatedDecision(_) => {}
    }

    lines.join("\n")
}

fn extract_markdown_decision_section(raw_text: &str, anchor: &str) -> Option<String> {
    for heading_prefix in &["### ", "## "] {
        let heading_hashes = heading_prefix.trim_end_matches(' ');
        // A section ends at any heading whose level is <= the current section's level
        // (i.e., a parent or a sibling heading).  For `###` (level 3), we stop at
        // `#`, `##`, or `###` — so the threshold is `heading_hashes.len()`, not `- 1`.
        let section_level = heading_hashes.len();

        let mut in_section = false;
        let mut section_lines: Vec<&str> = Vec::new();

        for line in raw_text.lines() {
            if line.starts_with(heading_prefix) {
                if in_section {
                    // Sibling heading at the same level — end of this section.
                    break;
                }
                // Check if this heading matches the anchor.
                let heading_content = line.trim_start_matches('#').trim();
                // Anchor matches if the heading content is exactly the anchor string
                // or the anchor string immediately followed by `:` or a space.
                // We deliberately do NOT use `contains` to avoid "D1" matching "D10".
                if heading_content == anchor
                    || heading_content.starts_with(&format!("{anchor}:"))
                    || heading_content.starts_with(&format!("{anchor} "))
                {
                    in_section = true;
                    section_lines.push(line);
                }
            } else if in_section {
                // End the section when a heading at a higher or equal level is encountered.
                let leading_hashes = line.chars().take_while(|c| *c == '#').count();
                if leading_hashes > 0 && leading_hashes <= section_level {
                    break;
                }
                section_lines.push(line);
            }
        }

        if !section_lines.is_empty() {
            return Some(section_lines.join("\n"));
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Chain-2: catalogue → spec
// ---------------------------------------------------------------------------

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

    let catalogue_exists =
        catalogue_path.try_exists().map_err(|e| RefVerifyError::VerifierPort {
            message: format!("cannot inspect catalogue '{}': {e}", catalogue_path.display()),
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

pub(super) fn validate_template_path(
    template_path: &Path,
    project_root: &Path,
) -> Result<PathBuf, String> {
    let project_root = project_root.canonicalize().map_err(|e| {
        format!("cannot canonicalize project root '{}': {e}", project_root.display())
    })?;
    if template_path.is_absolute() {
        return Err(format!(
            "template path must be relative, got absolute: '{}'",
            template_path.display()
        ));
    }
    if template_path.components().any(|c| c == Component::ParentDir) {
        return Err(format!("template path must not contain '..': '{}'", template_path.display()));
    }
    if !template_path.components().any(|c| matches!(c, Component::Normal(_))) {
        return Err(format!("template path is empty or '.': '{}'", template_path.display()));
    }

    let resolved = lexically_normalize(&project_root.join(template_path));
    if !resolved.starts_with(&project_root) {
        return Err(format!("template path escapes project root: '{}'", template_path.display()));
    }

    Ok(resolved)
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
    // This mirrors the scope resolver's `load_bindings` which also returns an empty set
    // when architecture-rules.json is absent, allowing pre-Phase-0 repos to run All scope.
    let rules_path = project_root.join("architecture-rules.json");
    let rules_exist = rules_path.try_exists().map_err(|e| RefVerifyError::VerifierPort {
        message: format!(
            "cannot inspect architecture-rules.json at '{}': {e}",
            rules_path.display()
        ),
    })?;
    if !rules_exist {
        return Ok(pairs);
    }

    let bindings = load_tddd_layer_bindings(project_root)?;

    // First pass: determine whether we are in the all-present or all-absent state.
    // Fail closed if we observe a partial set (some present, some absent).
    let mut present_count = 0usize;
    let mut absent_count = 0usize;
    for binding in &bindings {
        let catalogue_path = track_dir.join(binding.catalogue_file());
        let exists = catalogue_path.try_exists().map_err(|e| RefVerifyError::VerifierPort {
            message: format!("cannot inspect catalogue '{}': {e}", catalogue_path.display()),
        })?;
        if exists {
            present_count += 1;
        } else {
            absent_count += 1;
        }
    }

    if present_count > 0 && absent_count > 0 {
        // Partial catalogue set — fail closed to avoid silent under-verification.
        let missing: Vec<String> = bindings
            .iter()
            .filter(|b| !track_dir.join(b.catalogue_file()).exists())
            .map(|b| b.catalogue_file().to_owned())
            .collect();
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

// ---------------------------------------------------------------------------
// Known-bad probes
// ---------------------------------------------------------------------------

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
    match rules_path.try_exists() {
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

/// Calculate how many known-bad probes to inject given the total pair count and
/// injection rate percentage.
pub(super) fn calculate_probe_count(pair_count: usize, injection_rate_percent: u8) -> usize {
    if pair_count == 0 {
        return 0;
    }
    (pair_count * injection_rate_percent as usize).div_ceil(100)
}

/// Create a known-bad monitor probe.
///
/// The probe uses a deliberately incorrect claim/evidence combination so that a
/// well-functioning verifier should return `Fail` for it.
///
/// `cache_scope` must match the scope of the run so the probe is dispatched through the
/// same chain capability (Chain1 for `SpecAdr`, Chain2 for `CatalogueSpec`) as the real
/// pairs. Hard-coding `SpecAdr` during a Chain2 run would leave Chain2 calibration unchecked.
pub(super) fn make_known_bad_probe(
    index: usize,
    cache_scope: RefVerifyCacheScope,
) -> Result<RefVerifyPair, RefVerifyError> {
    let claim = format!("known-bad-probe-{index}: The system must implement feature X");
    let evidence = format!(
        "known-bad-probe-{index}: The ADR decision states the opposite of X, invalidating the claim"
    );
    let claim_hash = hash_text(&claim)?;
    let evidence_hash = hash_text(&evidence)?;
    Ok(RefVerifyPair { claim, evidence, claim_hash, evidence_hash, cache_scope, known_bad: true })
}
