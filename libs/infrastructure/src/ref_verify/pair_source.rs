//! Helper functions for enumerating Chain-1 and Chain-2 reference pairs.
//!
//! These free functions are called by [`super::RefVerifyPairSourceAdapter`] and are
//! separated here to keep the parent module within the 700-line production-code limit.
//! Chain-1 (spec → ADR) enumeration and shared helpers live here; Chain-2
//! (catalogue → spec) enumeration lives in `pair_source_chain2.rs`.

pub(super) use super::pair_source_json::{extract_json_object_parsed, render_prompt_template};

use crate::adr_decision::parse_adr_frontmatter;
use crate::verify::plan_artifact_refs::{build_element_map, canonical_json_sha256};
use domain::{AdrDecisionCommon, AdrDecisionEntry, ContentHash};
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
    use crate::track::symlink_guard::reject_symlinks_below;

    // Guard the track directory via the full symlink-rejecting path so that
    // symlinked intermediate components (e.g. track/items or the track_id dir
    // itself) are also rejected. A missing track directory means the track ID
    // is invalid — distinct from Phase 0 (directory exists, spec.json absent).
    match reject_symlinks_below(track_dir, project_root) {
        Ok(true) => {
            // Confirm the path is actually a directory, not a regular file.
            // `reject_symlinks_below` only proves existence + non-symlink; a
            // regular file at the track path would otherwise pass this guard and
            // silently produce zero pairs via the spec-absent early-return below.
            if !track_dir.is_dir() {
                return Err(RefVerifyError::VerifierPort {
                    message: format!(
                        "track path '{}' exists but is not a directory — invalid track ID",
                        track_dir.display()
                    ),
                });
            }
        }
        Ok(false) => {
            return Err(RefVerifyError::VerifierPort {
                message: format!(
                    "track directory '{}' not found — invalid track ID or missing directory",
                    track_dir.display()
                ),
            });
        }
        Err(e) => {
            return Err(RefVerifyError::VerifierPort {
                message: format!("cannot inspect track directory '{}': {e}", track_dir.display()),
            });
        }
    }

    let spec_path = track_dir.join("spec.json");

    // Consistent absence (pre-Phase-1): spec.json does not exist yet, so
    // Chain-1 contributes zero pairs. The scope resolver has already rejected
    // the inconsistent case (catalogue present + spec absent, IN-06), so this
    // early-return is only reachable in the Phase 0 state. Present-but-broken
    // spec.json still fails closed via the read/decode errors below (IN-07).
    //
    // Use reject_symlinks_below (which inspects parent components as well as the
    // leaf) instead of try_exists / symlink_metadata so that a symlinked path at
    // any level is treated as an error rather than as absent.
    let spec_exists = match reject_symlinks_below(&spec_path, project_root) {
        Ok(exists) => exists,
        Err(e) => {
            return Err(RefVerifyError::VerifierPort {
                message: format!("cannot inspect spec.json at '{}': {e}", spec_path.display()),
            });
        }
    };
    if !spec_exists {
        println!("[SKIP] spec.json not found — Chain-1 has zero pairs");
        return Ok(Vec::new());
    }

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
        lines.push(format!("user_decision_ref: {}", user_decision_ref.as_str()));
    }
    if let Some(review_finding_ref) = common.review_finding_ref() {
        lines.push(format!("review_finding_ref: {}", review_finding_ref.as_str()));
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

// ---------------------------------------------------------------------------
// Known-bad probes
// ---------------------------------------------------------------------------

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
