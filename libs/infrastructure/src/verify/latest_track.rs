//! Verify that the latest track has complete, non-placeholder artifacts.
//!
//! Rust port of `scripts/verify_latest_track_files.py`.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use domain::verify::{VerifyFinding, VerifyOutcome};
use domain::{StatusOverride, derive_track_status};
use regex::Regex;

use crate::track::codec;

const TRACK_ITEMS_DIR: &str = "track/items";
const TRACK_ARCHIVE_DIR: &str = "track/archive";

/// Type alias to reduce repetition in metadata-loading signatures.
type TrackMeta = (i64, String, Option<String>, u32);

/// Type alias for file-validator function pointers used in the verify loop.
type FileValidator = fn(&Path, &Path) -> Vec<VerifyFinding>;

static PLACEHOLDER_LINE_RE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"(?i)TODO:|TEMPLATE STUB").ok());

static TASK_LINE_RE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^\s*(?:[-*]|\d+\.)\s+\[[^\]]\]\s+.+").ok());

static LIST_MARKER_RE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^\s*(?:[-*]|\d+\.)\s+").ok());

static HORIZONTAL_RULE_RE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^[-*_]{3,}$").ok());

/// Scaffold keyword set for verification.md placeholders.
/// Values are lowercase normalized strings matched after stripping list markers and trailing `:`.
static VERIFICATION_SCAFFOLD_LINES: LazyLock<std::collections::HashSet<&'static str>> =
    LazyLock::new(|| {
        [
            "scope verified",
            "manual verification steps",
            "result / open issues",
            "verified_at",
            "検証範囲",
            "手動検証手順",
            "結果 / 未解決事項",
            "検証日",
        ]
        .into_iter()
        .collect()
    });

/// Run the latest-track file verification.
///
/// Finds the "latest" non-archived track and validates its `spec.md`,
/// `plan.md`, and `verification.md` files for completeness.
///
/// # Errors
///
/// Returns error findings when any track's `metadata.json` is malformed,
/// or when the latest track's markdown files are empty, contain placeholders,
/// or lack substantive content.
pub fn verify(root: &Path) -> VerifyOutcome {
    match latest_track_dir(root) {
        Err(findings) => VerifyOutcome::from_findings(findings),
        Ok(None) => VerifyOutcome::pass(),
        Ok(Some(track_dir)) => {
            let mut outcome = VerifyOutcome::pass();

            // spec.md is optional when spec.json is present (spec.md is a
            // generated read-only view in that case). Validate whichever
            // artifact exists; require at least one.
            let spec_json_path = track_dir.join("spec.json");
            let spec_md_path = track_dir.join("spec.md");
            if spec_json_path.is_file() {
                // spec.json exists — validate it; spec.md is optional.
                for finding in validate_spec_json_file(&spec_json_path, root) {
                    outcome.add(finding);
                }
            } else if spec_md_path.is_file() {
                for finding in validate_spec_file(&spec_md_path, root) {
                    outcome.add(finding);
                }
            } else {
                outcome.add(VerifyFinding::error(format!(
                    "[ERROR] Latest track is missing spec.md (and no spec.json found): {}",
                    display_path(&spec_md_path, root)
                )));
            }

            let other_files: [(&str, FileValidator); 2] =
                [("plan.md", validate_plan_file), ("verification.md", validate_verification_file)];
            for (filename, validator) in &other_files {
                let path = track_dir.join(filename);
                if !path.is_file() {
                    outcome.add(VerifyFinding::error(format!(
                        "[ERROR] Latest track is missing {filename}: {}",
                        display_path(&path, root)
                    )));
                } else {
                    for finding in validator(&path, root) {
                        outcome.add(finding);
                    }
                }
            }
            outcome
        }
    }
}

/// Collect all track directories from `track/items/` and `track/archive/`, sorted by name.
fn all_track_directories(root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for base in [TRACK_ITEMS_DIR, TRACK_ARCHIVE_DIR] {
        let base_path = root.join(base);
        if let Ok(entries) = std::fs::read_dir(&base_path) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dirs.push(entry.path());
                }
            }
        }
    }
    dirs.sort();
    dirs
}

/// Find the "latest" track directory that should be verified.
///
/// Returns `Ok(None)` when no tracks exist.
/// Returns `Err(findings)` when any `metadata.json` is malformed.
fn latest_track_dir(root: &Path) -> Result<Option<PathBuf>, Vec<VerifyFinding>> {
    let dirs = all_track_directories(root);
    if dirs.is_empty() {
        return Ok(None);
    }

    let archive_root = root.join(TRACK_ARCHIVE_DIR);

    let mut latest_dir: Option<PathBuf> = None;
    // Rank tuple: (priority, updated_at_secs, dir_name)
    let mut latest_rank: (u32, i64, String) = (0, i64::MIN, String::new());
    let mut errors: Vec<VerifyFinding> = Vec::new();

    for dir_path in dirs {
        // Skip tracks under track/archive/ regardless of metadata content.
        if dir_path.starts_with(&archive_root) {
            continue;
        }

        match load_track_metadata(&dir_path, root) {
            Err(mut track_errors) => {
                errors.append(&mut track_errors);
                continue;
            }
            Ok(None) => continue, // archived status, skip
            Ok(Some((updated_at_secs, status, branch, schema_version))) => {
                let priority = selection_priority(&status, branch.as_deref(), schema_version);
                let dir_name =
                    dir_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_owned();
                let rank = (priority, updated_at_secs, dir_name);
                if rank > latest_rank {
                    latest_rank = rank;
                    latest_dir = Some(dir_path);
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(latest_dir)
}

/// Load and validate track metadata.
///
/// Returns `Ok(None)` if the track should be skipped (archived status).
/// Returns `Err(findings)` for malformed metadata.
///
/// On success returns `(updated_at_unix_secs, status, branch, schema_version)`.
fn load_track_metadata(
    track_dir: &Path,
    root: &Path,
) -> Result<Option<TrackMeta>, Vec<VerifyFinding>> {
    let metadata_file = track_dir.join("metadata.json");
    if !metadata_file.is_file() {
        return Err(vec![VerifyFinding::error(format!(
            "[ERROR] Cannot determine latest track because metadata.json is missing: {}",
            display_path(&metadata_file, root)
        ))]);
    }

    let content = match std::fs::read_to_string(&metadata_file) {
        Ok(c) => c,
        Err(e) => {
            return Err(vec![VerifyFinding::error(format!(
                "[ERROR] Cannot determine latest track because metadata.json is invalid: {} ({e})",
                display_path(&metadata_file, root)
            ))]);
        }
    };

    let data: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            return Err(vec![VerifyFinding::error(format!(
                "[ERROR] Cannot determine latest track because metadata.json is invalid: {} ({e})",
                display_path(&metadata_file, root)
            ))]);
        }
    };

    let obj = match data.as_object() {
        Some(o) => o,
        None => {
            return Err(vec![VerifyFinding::error(format!(
                "[ERROR] Cannot determine latest track because metadata.json is invalid: {} (metadata.json must be a JSON object)",
                display_path(&metadata_file, root)
            ))]);
        }
    };

    // Determine schema_version from the parsed JSON.
    // A missing or non-numeric `schema_version` is an error — do NOT silently
    // default to a legacy value (which would skip the track) or to `5` (which
    // would let a malformed file pass as a valid v5 track). Require an explicit
    // integer value.
    let schema_version: u32 = match obj.get("schema_version").and_then(|v| v.as_u64()) {
        Some(v) => v as u32,
        None => {
            return Err(vec![VerifyFinding::error(format!(
                "[ERROR] Cannot determine latest track because schema_version is missing or invalid: {}",
                display_path(&metadata_file, root)
            ))]);
        }
    };

    // Skip legacy (v2/v3/v4) tracks structurally — they predate the identity-only
    // schema and carry a `status` field that is no longer supported. Only v5+
    // tracks participate in latest-track selection and verification.
    if schema_version < 5 {
        return Ok(None);
    }

    // Full schema validation via the authoritative v5 codec. Any structural
    // inconsistency (missing required fields, malformed branch, invalid
    // status_override syntax, etc.) is surfaced as an error here rather than
    // being discovered later or silently ignored.
    let (track, doc_meta) = codec::decode(&content).map_err(|e| {
        vec![VerifyFinding::error(format!(
            "[ERROR] Cannot determine latest track because metadata.json fails v5 schema validation: {} ({e})",
            display_path(&metadata_file, root)
        ))]
    })?;

    // Load impl-plan.json (if present) and enforce the activation invariant
    // `is_activated() -> impl-plan.json present` at the domain layer. A
    // branch-materialized track with a missing impl-plan.json is corrupt and
    // must not be treated as a healthy "planned" track.
    let impl_plan = load_impl_plan_from_dir(track_dir).map_err(|e| {
        vec![VerifyFinding::error(format!(
            "[ERROR] Cannot determine latest track because impl-plan.json is invalid: {} ({e})",
            display_path(&metadata_file, root)
        ))]
    })?;
    domain::check_impl_plan_presence(&track, impl_plan.as_ref()).map_err(|e| {
        vec![VerifyFinding::error(format!(
            "[ERROR] Cannot determine latest track because activation invariant is violated: {} ({e})",
            display_path(&metadata_file, root)
        ))]
    })?;

    // Derive status from impl-plan.json + status_override (v5 has no status field in JSON).
    // Surface errors so that a broken track cannot silently be treated as a healthy "planned" track.
    let status = derive_status_from_v5(impl_plan.as_ref(), obj).map_err(|e| {
        vec![VerifyFinding::error(format!(
            "[ERROR] Cannot determine latest track because status derivation failed: {} ({e})",
            display_path(&metadata_file, root)
        ))]
    })?;

    // Skip archived tracks early.
    if status == "archived" {
        return Ok(None);
    }

    // Use updated_at from the decoded document meta (authoritative — validated by codec).
    let updated_at_secs = match parse_updated_at(&doc_meta.updated_at) {
        Ok(secs) => secs,
        Err(e) => {
            return Err(vec![VerifyFinding::error(format!(
                "[ERROR] Cannot determine latest track because updated_at is invalid: {} ({e})",
                display_path(&metadata_file, root)
            ))]);
        }
    };

    let branch = obj.get("branch").and_then(|v| v.as_str()).map(|s| s.to_owned());

    Ok(Some((updated_at_secs, status, branch, schema_version)))
}

/// Parse an ISO 8601 timestamp and return Unix seconds.
///
/// Handles `Z` suffix and date-only strings.
///
/// # Errors
///
/// Returns an error string when the timestamp cannot be parsed.
fn parse_updated_at(raw: &str) -> Result<i64, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("updated_at must be a non-empty string".to_owned());
    }

    // Normalize Z suffix.
    let normalized = if let Some(stripped) = value.strip_suffix('Z') {
        format!("{stripped}+00:00")
    } else {
        value.to_owned()
    };

    // Try RFC 3339 first (T-separated with offset).
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&normalized) {
        return Ok(dt.timestamp());
    }

    // Try space-separated with offset (Python fromisoformat accepts this).
    // Normalize space to T for RFC 3339 parsing.
    let t_normalized = normalized.replacen(' ', "T", 1);
    if t_normalized != normalized {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&t_normalized) {
            return Ok(dt.timestamp());
        }
    }

    // Try datetime with offset using chrono fixed-offset formats.
    let offset_formats = [
        "%Y-%m-%dT%H:%M:%S%:z",
        "%Y-%m-%dT%H:%M:%S%.f%:z",
        "%Y-%m-%d %H:%M:%S%:z",
        "%Y-%m-%d %H:%M:%S%.f%:z",
    ];
    for fmt in &offset_formats {
        if let Ok(dt) = chrono::DateTime::parse_from_str(&normalized, fmt) {
            return Ok(dt.timestamp());
        }
    }

    // Try date-only parse (YYYY-MM-DD).
    if let Ok(date) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        use chrono::TimeZone as _;
        let naive_dt =
            date.and_hms_opt(0, 0, 0).ok_or_else(|| "invalid time components".to_owned())?;
        return Ok(chrono::Utc.from_utc_datetime(&naive_dt).timestamp());
    }

    // Try datetime without timezone offset (T-separated and space-separated,
    // with optional fractional seconds — matches Python's fromisoformat).
    let naive_formats =
        ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M:%S%.f"];
    for fmt in &naive_formats {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&normalized, fmt) {
            use chrono::TimeZone as _;
            return Ok(chrono::Utc.from_utc_datetime(&dt).timestamp());
        }
    }

    Err(format!("cannot parse timestamp: '{value}'"))
}

/// Derive the track status string for a v5 metadata document.
///
/// Loads `impl-plan.json` (if present) from the same directory, parses
/// `status_override` from the raw JSON, and delegates to
/// `domain::derive_track_status` to compute the effective status.
///
/// Returns the status as a lowercase string compatible with the status
/// strings used throughout this verifier.
///
/// # Errors
///
/// Returns an error string when:
/// - `impl-plan.json` exists but cannot be read (I/O error).
/// - `impl-plan.json` exists but cannot be decoded (corrupt / invalid JSON).
/// - `status_override` is present but has an unrecognised `status` value.
///
/// Absent `impl-plan.json` (file does not exist) is not an error — it means
/// the track is in the planning phase (`Planned` status). Activation-invariant
/// enforcement (a branch-materialized track must carry an impl-plan.json) is
/// the caller's responsibility — this helper only derives status from the
/// already-loaded optional impl-plan plus the raw `status_override` JSON.
fn derive_status_from_v5(
    impl_plan: Option<&domain::ImplPlanDocument>,
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<String, String> {
    // Parse status_override from raw JSON (same shape as codec.rs).
    // If the key is present but the value is not a JSON object (e.g. a string or
    // null), treat that as malformed metadata and surface an error rather than
    // silently ignoring the override.
    let status_override: Option<StatusOverride> = match obj.get("status_override") {
        None => None,
        Some(v) if v.is_null() => None,
        Some(v) => {
            let override_obj = v
                .as_object()
                .ok_or_else(|| "status_override is present but not a JSON object".to_owned())?;
            let status_str = override_obj
                .get("status")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "status_override.status is missing or not a string".to_owned())?;
            let reason =
                override_obj.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_owned();
            match status_str {
                "blocked" => Some(
                    StatusOverride::blocked(reason)
                        .map_err(|e| format!("invalid blocked override: {e}"))?,
                ),
                "cancelled" => Some(
                    StatusOverride::cancelled(reason)
                        .map_err(|e| format!("invalid cancelled override: {e}"))?,
                ),
                other => {
                    return Err(format!("unrecognised status_override.status: '{other}'"));
                }
            }
        }
    };

    Ok(derive_track_status(impl_plan, status_override.as_ref()).to_string())
}

/// Load and decode `impl-plan.json` from a track directory.
///
/// Absent file → `Ok(None)` (planning-only track).
/// Present but unreadable or corrupt → `Err` (fail-closed).
///
/// # Errors
///
/// Returns a descriptive error string when the file exists but cannot be read
/// or decoded.
fn load_impl_plan_from_dir(track_dir: &Path) -> Result<Option<domain::ImplPlanDocument>, String> {
    let path = track_dir.join("impl-plan.json");
    if !path.exists() {
        return Ok(None);
    }
    let json =
        std::fs::read_to_string(&path).map_err(|e| format!("cannot read impl-plan.json: {e}"))?;
    let doc = crate::impl_plan_codec::decode(&json)
        .map_err(|e| format!("cannot decode impl-plan.json: {e}"))?;
    Ok(Some(doc))
}

/// Compute track selection priority.
///
/// Returns:
/// - `2` when the track has a branch and is not done, or is branchless but
///   actively in-progress (status is active and not `planned`).
/// - `1` when the track is branchless v3/v4 and `planned` (planning-only).
///   These are deprioritised so that a branchful active track always wins.
/// - `0` otherwise (done, archived, or unrecognized).
fn selection_priority(status: &str, branch: Option<&str>, schema_version: u32) -> u32 {
    let branch_name = branch.map(|b| b.trim()).unwrap_or("");
    let has_branch = !branch_name.is_empty();
    let is_active = status != "done" && status != "archived";

    if has_branch && is_active {
        return 2;
    }
    // Branchless track with active status that is NOT a planning-only placeholder.
    // v3/v4/v5 tracks in "planned" state are planning-only (priority 1).
    // All other branchless active tracks get priority 2.
    if !has_branch && is_active && !(matches!(schema_version, 3..=5) && status == "planned") {
        return 2;
    }
    if !has_branch && status == "planned" {
        return 1;
    }
    0
}

// ---------------------------------------------------------------------------
// Placeholder / content helpers
// ---------------------------------------------------------------------------

/// Return `(line_number, line)` pairs for placeholder lines outside fenced code blocks.
fn placeholder_lines(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut in_fence = false;
    for (line_number, line) in text.lines().enumerate().map(|(i, l)| (i + 1, l)) {
        let stripped = line.trim();
        if stripped.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if PLACEHOLDER_LINE_RE.as_ref().is_some_and(|re| re.is_match(line)) {
            found.push((line_number, line.to_owned()));
        }
    }
    found
}

/// Return meaningful non-heading, non-blockquote, non-horizontal-rule lines.
fn meaningful_non_heading_lines(text: &str) -> Vec<String> {
    let mut meaningful = Vec::new();
    for line in text.lines() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        if stripped.starts_with('#') {
            continue;
        }
        if stripped.starts_with('>') {
            continue;
        }
        if stripped.starts_with("```") {
            continue;
        }
        if HORIZONTAL_RULE_RE.as_ref().is_some_and(|re| re.is_match(stripped)) {
            continue;
        }
        meaningful.push(stripped.to_owned());
    }
    meaningful
}

/// Returns `true` when `text` contains at least one task-item line.
fn has_task_items(text: &str) -> bool {
    text.lines().any(|line| TASK_LINE_RE.as_ref().is_some_and(|re| re.is_match(line)))
}

/// Normalize a line for scaffold keyword matching.
///
/// Strips leading/trailing whitespace, removes list markers, strips trailing `:`,
/// and lowercases the result.
fn normalize_scaffold_line(line: &str) -> String {
    let stripped = line.trim();
    let without_marker = LIST_MARKER_RE
        .as_ref()
        .map(|re| re.replace(stripped, "").into_owned())
        .unwrap_or_else(|| stripped.to_owned());
    without_marker.trim_end_matches(':').trim().to_lowercase()
}

/// Return `(line_number, line)` pairs for scaffold placeholder lines.
fn scaffold_placeholder_lines(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (line_number, line) in text.lines().enumerate().map(|(i, l)| (i + 1, l)) {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        if stripped.starts_with('#') {
            continue;
        }
        let normalized = normalize_scaffold_line(line);
        if VERIFICATION_SCAFFOLD_LINES.contains(normalized.as_str()) {
            found.push((line_number, line.to_owned()));
        }
    }
    found
}

// ---------------------------------------------------------------------------
// File validators
// ---------------------------------------------------------------------------

/// Validate a `spec.json` artifact: must be readable and decode without error.
fn validate_spec_json_file(path: &Path, root: &Path) -> Vec<VerifyFinding> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            return vec![VerifyFinding::error(format!(
                "[ERROR] Cannot read spec.json: {} ({e})",
                display_path(path, root)
            ))];
        }
    };
    if text.trim().is_empty() {
        return vec![VerifyFinding::error(format!(
            "[ERROR] Latest track spec.json is empty: {}",
            display_path(path, root)
        ))];
    }
    let doc = match crate::spec::codec::decode(&text) {
        Ok(d) => d,
        Err(e) => {
            return vec![VerifyFinding::error(format!(
                "[ERROR] Latest track spec.json is invalid: {} ({e})",
                display_path(path, root)
            ))];
        }
    };

    // Collect ALL text-bearing strings from the document for placeholder scanning.
    // `all_texts` holds borrowed slices; `owned_file_paths` holds String copies
    // of PathBuf file paths (whose Cow temporaries do not live long enough to
    // borrow into all_texts directly).
    let mut all_texts: Vec<&str> = vec![doc.title(), doc.version()];
    let mut owned_file_paths: Vec<String> = Vec::new();

    // goal is now Vec<SpecRequirement>; scan id, text, and all typed ref strings.
    for req in doc.goal() {
        all_texts.push(req.id().as_ref());
        all_texts.push(req.text());
        for adr_ref in req.adr_refs() {
            owned_file_paths.push(adr_ref.file.to_string_lossy().into_owned());
            all_texts.push(adr_ref.anchor.as_ref());
        }
        for conv_ref in req.convention_refs() {
            owned_file_paths.push(conv_ref.file.to_string_lossy().into_owned());
            all_texts.push(conv_ref.anchor.as_ref());
        }
        for informal in req.informal_grounds() {
            all_texts.push(informal.summary.as_ref());
        }
    }
    let all_reqs = doc
        .scope()
        .in_scope()
        .iter()
        .chain(doc.scope().out_of_scope().iter())
        .chain(doc.constraints().iter())
        .chain(doc.acceptance_criteria().iter());
    for req in all_reqs {
        // Scan id alongside text and typed refs.
        all_texts.push(req.id().as_ref());
        all_texts.push(req.text());
        // Typed refs: scan both file path and anchor for placeholders.
        for adr_ref in req.adr_refs() {
            owned_file_paths.push(adr_ref.file.to_string_lossy().into_owned());
            all_texts.push(adr_ref.anchor.as_ref());
        }
        for conv_ref in req.convention_refs() {
            owned_file_paths.push(conv_ref.file.to_string_lossy().into_owned());
            all_texts.push(conv_ref.anchor.as_ref());
        }
        for informal in req.informal_grounds() {
            all_texts.push(informal.summary.as_ref());
        }
    }
    for section in doc.additional_sections() {
        all_texts.push(section.title());
        for line in section.content() {
            all_texts.push(line.as_str());
        }
    }
    // related_conventions is now Vec<ConventionRef>; scan both file path and anchor.
    for conv in doc.related_conventions() {
        owned_file_paths.push(conv.file.to_string_lossy().into_owned());
        all_texts.push(conv.anchor.as_ref());
    }
    // Append owned file path strings so they are scanned alongside the borrowed slices.
    let file_path_refs: Vec<&str> = owned_file_paths.iter().map(String::as_str).collect();
    let all_texts: Vec<&str> = all_texts.into_iter().chain(file_path_refs).collect();

    let mut findings = Vec::new();
    let placeholder_patterns = ["TODO:", "TEMPLATE STUB", "TBD"];
    let display = display_path(path, root);
    for text in &all_texts {
        let upper = text.to_uppercase();
        for pattern in &placeholder_patterns {
            if upper.contains(pattern) {
                findings.push(VerifyFinding::error(format!(
                    "[ERROR] Latest track spec.json contains placeholder '{pattern}': {display}"
                )));
                // One finding per placeholder pattern per document is enough
                break;
            }
        }
        if !findings.is_empty() {
            break;
        }
    }
    findings
}

fn validate_spec_file(path: &Path, root: &Path) -> Vec<VerifyFinding> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            return vec![VerifyFinding::error(format!(
                "[ERROR] Cannot read spec.md: {} ({e})",
                display_path(path, root)
            ))];
        }
    };
    let mut findings = Vec::new();
    if text.trim().is_empty() {
        return vec![VerifyFinding::error(format!(
            "[ERROR] Latest track spec.md is empty: {}",
            display_path(path, root)
        ))];
    }
    let placeholders = placeholder_lines(&text);
    if !placeholders.is_empty() {
        findings.push(VerifyFinding::error(format!(
            "[ERROR] Latest track spec.md still contains placeholders: {}",
            display_path(path, root)
        )));
        for (line_number, line) in &placeholders {
            findings.push(VerifyFinding::error(format!("  {line_number}:{line}")));
        }
    }
    if meaningful_non_heading_lines(&text).is_empty() {
        findings.push(VerifyFinding::error(format!(
            "[ERROR] Latest track spec.md lacks substantive content beyond headings: {}",
            display_path(path, root)
        )));
    }
    findings
}

fn validate_plan_file(path: &Path, root: &Path) -> Vec<VerifyFinding> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            return vec![VerifyFinding::error(format!(
                "[ERROR] Cannot read plan.md: {} ({e})",
                display_path(path, root)
            ))];
        }
    };
    let mut findings = Vec::new();
    if text.trim().is_empty() {
        return vec![VerifyFinding::error(format!(
            "[ERROR] Latest track plan.md is empty: {}",
            display_path(path, root)
        ))];
    }
    let placeholders = placeholder_lines(&text);
    if !placeholders.is_empty() {
        findings.push(VerifyFinding::error(format!(
            "[ERROR] Latest track plan.md still contains placeholders: {}",
            display_path(path, root)
        )));
        for (line_number, line) in &placeholders {
            findings.push(VerifyFinding::error(format!("  {line_number}:{line}")));
        }
    }
    // Skip the task-items check when:
    //   (a) impl-plan.json is absent AND the plan.md carries both the machine-generated
    //       header and the stub Note — this is the transition stub emitted by
    //       `render_plan(_, None)`.  Requiring both markers makes the bypass much harder
    //       to trigger accidentally via a hand-written plan.  Requiring the file to be
    //       absent prevents a copied stub header from bypassing the check when a real
    //       impl-plan.json already exists.
    //   (b) impl-plan.json is present, has zero tasks, and the plan.md does NOT carry the
    //       stub Note — render_plan(Some(empty_doc)) produces the "Tasks (0/0 resolved)"
    //       header but no task-item lines, which is correct.  If the stub Note is still
    //       present, the plan.md is stale (view_freshness will catch it, and we treat
    //       it as an error here too rather than silently skipping the check).
    let impl_plan_path = path.parent().map(|d| d.join("impl-plan.json"));
    let impl_plan_absent = !impl_plan_path.as_ref().is_some_and(|p| p.is_file());
    let has_stub_note = text.contains("> **Note**: `impl-plan.json` not yet generated.");
    let impl_plan_empty_and_fresh = impl_plan_path.as_ref().is_some_and(|p| {
        !has_stub_note
            && p.is_file()
            && std::fs::read_to_string(p).ok().and_then(|json| {
                crate::impl_plan_codec::decode(&json).ok().map(|doc| doc.tasks().is_empty())
            }) == Some(true)
    });
    let is_t005_stub = impl_plan_absent
        && text.contains(
            "<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->",
        )
        && has_stub_note;
    if !is_t005_stub && !impl_plan_empty_and_fresh && !has_task_items(&text) {
        findings.push(VerifyFinding::error(format!(
            "[ERROR] Latest track plan.md does not contain any task items: {}",
            display_path(path, root)
        )));
    }
    findings
}

fn validate_verification_file(path: &Path, root: &Path) -> Vec<VerifyFinding> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            return vec![VerifyFinding::error(format!(
                "[ERROR] Cannot read verification.md: {} ({e})",
                display_path(path, root)
            ))];
        }
    };
    let mut findings = Vec::new();
    if text.trim().is_empty() {
        return vec![VerifyFinding::error(format!(
            "[ERROR] Latest track verification.md is empty: {}",
            display_path(path, root)
        ))];
    }
    let placeholders = placeholder_lines(&text);
    if !placeholders.is_empty() {
        findings.push(VerifyFinding::error(format!(
            "[ERROR] Latest track verification.md still contains placeholders: {}",
            display_path(path, root)
        )));
        for (line_number, line) in &placeholders {
            findings.push(VerifyFinding::error(format!("  {line_number}:{line}")));
        }
    }
    if meaningful_non_heading_lines(&text).is_empty() {
        findings.push(VerifyFinding::error(format!(
            "[ERROR] Latest track verification.md lacks substantive content beyond headings: {}",
            display_path(path, root)
        )));
    }
    let scaffold_lines = scaffold_placeholder_lines(&text);
    if !scaffold_lines.is_empty() {
        findings.push(VerifyFinding::error(format!(
            "[ERROR] Latest track verification.md still contains scaffold placeholders: {}",
            display_path(path, root)
        )));
        for (line_number, line) in &scaffold_lines {
            findings.push(VerifyFinding::error(format!("  {line_number}:{line}")));
        }
    }
    findings
}

// ---------------------------------------------------------------------------
// Path helper
// ---------------------------------------------------------------------------

fn display_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    // ---- helpers ----

    fn write_file(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    /// Build a minimal valid v5 identity-only metadata JSON for a track.
    ///
    /// v5 has no `status`, `tasks`, or `plan` fields. Status is derived from
    /// `impl-plan.json` + `status_override` at runtime.
    fn make_metadata_v5(id: &str, branch_json: &str, status_override_json: &str) -> String {
        format!(
            r#"{{"schema_version":5,"id":"{id}","title":"Track {id}","created_at":"2026-01-01T00:00:00+00:00","updated_at":"2026-01-15T00:00:00+00:00","branch":{branch_json}{status_override_json}}}"#
        )
    }

    /// Minimal valid `impl-plan.json` content for test fixtures that need an
    /// activated (branched) track without caring about the plan contents.
    const MINIMAL_IMPL_PLAN_JSON: &str =
        r#"{"schema_version":1,"plan":{"summary":[],"sections":[]}}"#;

    fn setup_track(root: &Path, id: &str, branch: Option<&str>) {
        let dir = root.join(TRACK_ITEMS_DIR).join(id);
        fs::create_dir_all(&dir).unwrap();
        let branch_json = match branch {
            Some(b) => format!(r#""{b}""#),
            None => "null".to_owned(),
        };
        let meta = make_metadata_v5(id, &branch_json, "");
        fs::write(dir.join("metadata.json"), meta).unwrap();
    }

    fn setup_track_planned(root: &Path, id: &str) {
        // v5 planning-only: no branch, no impl-plan.json → status derives to "planned".
        setup_track(root, id, None);
    }

    fn setup_track_with_branch(root: &Path, id: &str) {
        let branch = format!("track/{id}");
        setup_track(root, id, Some(&branch));
        // Activated tracks (branch materialized) must carry impl-plan.json per the
        // activation invariant `is_activated() → impl-plan.json present`. Write a
        // minimal valid document so that `check_impl_plan_presence` in
        // `load_track_metadata` succeeds and tests can focus on artifact validation.
        let dir = root.join(TRACK_ITEMS_DIR).join(id);
        fs::write(dir.join("impl-plan.json"), MINIMAL_IMPL_PLAN_JSON).unwrap();
    }

    fn setup_complete_track(root: &Path, id: &str, branch: Option<&str>) {
        setup_track(root, id, branch);
        // Activated (branched) tracks must carry impl-plan.json. Write a minimal valid
        // document so `check_impl_plan_presence` succeeds and the verifier can proceed
        // to check the actual markdown artifacts that this helper is testing.
        if branch.is_some() {
            let dir = root.join(TRACK_ITEMS_DIR).join(id);
            fs::write(dir.join("impl-plan.json"), MINIMAL_IMPL_PLAN_JSON).unwrap();
        }
        write_file(
            root,
            &format!("{TRACK_ITEMS_DIR}/{id}/spec.md"),
            "# Spec\n\nThis is a complete specification with real content.\n",
        );
        write_file(
            root,
            &format!("{TRACK_ITEMS_DIR}/{id}/plan.md"),
            "# Plan\n\n- [ ] Task one\n- [x] Task two done\n",
        );
        write_file(
            root,
            &format!("{TRACK_ITEMS_DIR}/{id}/verification.md"),
            "# Verification\n\nAll checks passed. The implementation has been verified.\n",
        );
    }

    // ---- test cases ----

    #[test]
    fn test_no_tracks_passes() {
        let tmp = TempDir::new().unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "empty repo should pass: {outcome:?}");
    }

    #[test]
    fn test_complete_v5_track_passes() {
        let tmp = TempDir::new().unwrap();
        // v5 track with branch (in-progress derived from impl-plan) and all artifacts.
        setup_complete_track(tmp.path(), "my-feature", Some("track/my-feature"));
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "complete v5 track should pass: {:#?}", outcome.findings());
    }

    #[test]
    fn test_legacy_v3_track_is_skipped() {
        // v3 metadata must be skipped by latest_track.rs. With only a v3 track
        // in the repo, no track is selected and verify() returns pass.
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(TRACK_ITEMS_DIR).join("legacy-feat");
        fs::create_dir_all(&dir).unwrap();
        let meta = r#"{"schema_version":3,"id":"legacy-feat","title":"Legacy","status":"in_progress","created_at":"2026-01-01T00:00:00+00:00","updated_at":"2026-01-15T00:00:00+00:00","branch":"track/legacy-feat","tasks":[{"id":"t1","description":"Task","status":"todo"}],"plan":{"summary":[],"sections":[{"id":"s1","title":"S","description":[],"task_ids":["t1"]}]}}"#;
        fs::write(dir.join("metadata.json"), meta).unwrap();

        let outcome = verify(tmp.path());
        assert!(
            outcome.is_ok(),
            "v3 tracks must be skipped; no v5 track → pass: {:#?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_missing_spec_fails() {
        let tmp = TempDir::new().unwrap();
        setup_track_with_branch(tmp.path(), "feat-a");
        // plan.md and verification.md present, spec.md absent
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-a/plan.md"),
            "# Plan\n\n- [ ] Task one\n",
        );
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-a/verification.md"),
            "# Verification\n\nCompleted all items.\n",
        );

        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "missing spec.md should fail");
        let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("spec.md")),
            "error should mention spec.md, got: {msgs:?}"
        );
    }

    #[test]
    fn test_placeholder_in_spec_fails() {
        let tmp = TempDir::new().unwrap();
        setup_track_with_branch(tmp.path(), "feat-b");
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-b/spec.md"),
            "# Spec\n\nTODO: fill in details\n",
        );
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-b/plan.md"),
            "# Plan\n\n- [ ] Task one\n",
        );
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-b/verification.md"),
            "# Verification\n\nAll verified.\n",
        );

        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "placeholder in spec should fail");
        let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("placeholder")),
            "error should mention placeholder, got: {msgs:?}"
        );
    }

    #[test]
    fn test_placeholder_in_fenced_block_ignored() {
        let tmp = TempDir::new().unwrap();
        setup_track_with_branch(tmp.path(), "feat-c");
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-c/spec.md"),
            "# Spec\n\nReal content here.\n\n```\nTODO: this is inside a code block\n```\n",
        );
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-c/plan.md"),
            "# Plan\n\n- [ ] Task one\n",
        );
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-c/verification.md"),
            "# Verification\n\nAll verified.\n",
        );

        let outcome = verify(tmp.path());
        assert!(
            outcome.is_ok(),
            "TODO inside fenced block should be ignored: {:#?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_selection_priority_v5_active_branch_highest() {
        // v5 + branch + not-done => priority 2
        assert_eq!(selection_priority("in_progress", Some("track/feat"), 5), 2);
        // v5 + planned + no branch => priority 1
        assert_eq!(selection_priority("planned", None, 5), 1);
        // v5 + done + branch => priority 0
        assert_eq!(selection_priority("done", Some("track/feat"), 5), 0);
        // Active branch beats branchless planned
        assert!(
            selection_priority("in_progress", Some("track/feat"), 5)
                > selection_priority("planned", None, 5)
        );
    }

    #[test]
    fn test_scaffold_placeholder_detected() {
        let tmp = TempDir::new().unwrap();
        setup_track_with_branch(tmp.path(), "feat-d");
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-d/spec.md"),
            "# Spec\n\nReal content.\n",
        );
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-d/plan.md"),
            "# Plan\n\n- [ ] Task one\n",
        );
        // verification.md with scaffold placeholder lines
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-d/verification.md"),
            "# Verification\n\nScope Verified:\nManual Verification Steps:\nResult / Open Issues:\nverified_at:\n",
        );

        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "scaffold placeholders should fail");
        let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("scaffold")),
            "error should mention scaffold, got: {msgs:?}"
        );
    }

    #[test]
    fn test_v5_branchless_planned_valid() {
        let tmp = TempDir::new().unwrap();
        // v5 planning-only: no branch, no impl-plan.json → status derives to "planned".
        setup_track_planned(tmp.path(), "planning-track");
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/planning-track/spec.md"),
            "# Spec\n\nPlanning specification with real content.\n",
        );
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/planning-track/plan.md"),
            "# Plan\n\n- [ ] Task one\n",
        );
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/planning-track/verification.md"),
            "# Verification\n\nNot yet started.\n",
        );

        let outcome = verify(tmp.path());
        assert!(
            outcome.is_ok(),
            "v5 branchless planned track should pass: {:#?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_archived_track_in_archive_dir_skipped() {
        let tmp = TempDir::new().unwrap();
        // Track under track/archive/ is skipped by path, no markdown files needed.
        let archive_dir = tmp.path().join(TRACK_ARCHIVE_DIR).join("old-feat");
        fs::create_dir_all(&archive_dir).unwrap();
        // Even v5 metadata in the archive directory is skipped by path.
        let meta = make_metadata_v5("old-feat", r#""track/old-feat""#, "");
        fs::write(archive_dir.join("metadata.json"), meta).unwrap();

        let outcome = verify(tmp.path());
        assert!(
            outcome.is_ok(),
            "archived track under archive dir should be skipped: {:#?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_corrupt_impl_plan_surfaces_error() {
        // A present but corrupt impl-plan.json must NOT silently be treated as
        // absent. The verifier should surface an error so that a broken track
        // is not silently selected as the latest track.
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(TRACK_ITEMS_DIR).join("corrupt-track");
        fs::create_dir_all(&dir).unwrap();
        let meta = make_metadata_v5("corrupt-track", r#""track/corrupt-track""#, "");
        fs::write(dir.join("metadata.json"), meta).unwrap();
        // Write invalid JSON to impl-plan.json.
        fs::write(dir.join("impl-plan.json"), "NOT VALID JSON").unwrap();

        let outcome = verify(tmp.path());
        assert!(
            outcome.has_errors(),
            "corrupt impl-plan.json must surface an error: {:#?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_activated_track_missing_impl_plan_surfaces_error() {
        // An activated (branch-materialized) v5 track without impl-plan.json violates
        // the activation invariant `is_activated() → impl-plan.json present` and must
        // surface an error. A broken activated track must not be silently selected and
        // treated as a healthy "planned" track.
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(TRACK_ITEMS_DIR).join("activated-no-plan");
        fs::create_dir_all(&dir).unwrap();
        // Branch is set → activated; no impl-plan.json written → invariant violation.
        let meta = make_metadata_v5("activated-no-plan", r#""track/activated-no-plan""#, "");
        fs::write(dir.join("metadata.json"), meta).unwrap();
        // Intentionally omit impl-plan.json.

        let outcome = verify(tmp.path());
        assert!(
            outcome.has_errors(),
            "activated track missing impl-plan.json must surface an error: {:#?}",
            outcome.findings()
        );
        let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("activation invariant")),
            "error should mention activation invariant, got: {msgs:?}"
        );
    }

    #[test]
    fn test_missing_schema_version_is_not_silently_skipped() {
        // A metadata.json without `schema_version` must NOT be treated as a
        // legacy v2/v3 track and silently skipped. It should fall through to
        // v5 processing so that errors are surfaced (fail-closed).
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(TRACK_ITEMS_DIR).join("no-version-track");
        fs::create_dir_all(&dir).unwrap();
        // metadata.json without schema_version — must not be silently skipped.
        let meta = r#"{"id":"no-version-track","branch":"track/no-version-track","title":"No Version","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-15T00:00:00Z"}"#;
        fs::write(dir.join("metadata.json"), meta).unwrap();
        // No impl-plan.json — if the track were processed as v5, status derives
        // to "planned" and the verifier proceeds to check artifacts. Without the
        // required spec.md / plan.md the outcome must be an error (not a silent pass).
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/no-version-track/plan.md"),
            "# Plan\n\n- [ ] Task one\n",
        );
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/no-version-track/verification.md"),
            "# Verification\n\nAll done.\n",
        );
        // No spec.md — should produce a "missing spec" error (not pass silently).
        let outcome = verify(tmp.path());
        assert!(
            outcome.has_errors(),
            "track with missing schema_version must not be silently skipped: {:#?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_parse_updated_at_z_suffix() {
        let secs_z = parse_updated_at("2026-01-15T00:00:00Z").unwrap();
        let secs_offset = parse_updated_at("2026-01-15T00:00:00+00:00").unwrap();
        assert_eq!(secs_z, secs_offset);
    }

    // ---- spec.json artifact tests ----

    const VALID_SPEC_JSON: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature",
  "scope": { "in_scope": [], "out_of_scope": [] }
}"#;

    /// Helper: set up a track with spec.json instead of spec.md.
    fn setup_complete_track_with_spec_json(root: &Path, id: &str) {
        setup_track_with_branch(root, id);
        write_file(root, &format!("{TRACK_ITEMS_DIR}/{id}/spec.json"), VALID_SPEC_JSON);
        write_file(root, &format!("{TRACK_ITEMS_DIR}/{id}/plan.md"), "# Plan\n\n- [ ] Task one\n");
        write_file(
            root,
            &format!("{TRACK_ITEMS_DIR}/{id}/verification.md"),
            "# Verification\n\nAll checks passed. The implementation has been verified.\n",
        );
    }

    #[test]
    fn test_spec_json_instead_of_spec_md_passes() {
        let tmp = TempDir::new().unwrap();
        setup_complete_track_with_spec_json(tmp.path(), "feat-json");
        let outcome = verify(tmp.path());
        assert!(
            outcome.is_ok(),
            "track with valid spec.json and no spec.md should pass: {:#?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_spec_json_and_spec_md_both_present_uses_spec_json() {
        let tmp = TempDir::new().unwrap();
        setup_complete_track_with_spec_json(tmp.path(), "feat-both");
        // Also write a spec.md with placeholder content that would fail markdown checks
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-both/spec.md"),
            "TODO: placeholder only\n",
        );
        let outcome = verify(tmp.path());
        // spec.json is preferred; valid spec.json should pass regardless of spec.md content
        assert!(
            outcome.is_ok(),
            "spec.json takes priority over spec.md: {:#?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_invalid_spec_json_fails() {
        let tmp = TempDir::new().unwrap();
        setup_track_with_branch(tmp.path(), "feat-bad-json");
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-bad-json/spec.json"),
            "not valid json",
        );
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-bad-json/plan.md"),
            "# Plan\n\n- [ ] Task one\n",
        );
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-bad-json/verification.md"),
            "# Verification\n\nAll checks passed.\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "invalid spec.json should fail");
        let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("spec.json")),
            "error should mention spec.json, got: {msgs:?}"
        );
    }

    #[test]
    fn test_missing_spec_md_and_spec_json_fails() {
        let tmp = TempDir::new().unwrap();
        setup_track_with_branch(tmp.path(), "feat-no-spec");
        // Neither spec.md nor spec.json present
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-no-spec/plan.md"),
            "# Plan\n\n- [ ] Task one\n",
        );
        write_file(
            tmp.path(),
            &format!("{TRACK_ITEMS_DIR}/feat-no-spec/verification.md"),
            "# Verification\n\nAll checks passed.\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "missing both spec.md and spec.json should fail");
        let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("spec.md")),
            "error should mention spec.md, got: {msgs:?}"
        );
    }
}
