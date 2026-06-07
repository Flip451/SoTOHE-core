//! File-content validators for the latest-track verification pass.
//!
//! Provides placeholder/content-scanning helpers (`placeholder_lines`,
//! `meaningful_non_heading_lines`, `has_task_items`) and the three
//! per-artifact validators (`validate_spec_json_file`, `validate_spec_file`,
//! `validate_plan_file`) called from `latest_track::verify`.

use std::path::Path;

use domain::verify::VerifyFinding;

// ---------------------------------------------------------------------------
// Placeholder / content helpers
// ---------------------------------------------------------------------------

/// Return `(line_number, line)` pairs for placeholder lines outside fenced code blocks.
pub(super) fn placeholder_lines(text: &str) -> Vec<(usize, String)> {
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
        if super::PLACEHOLDER_LINE_RE.as_ref().is_some_and(|re| re.is_match(line)) {
            found.push((line_number, line.to_owned()));
        }
    }
    found
}

/// Return meaningful non-heading, non-blockquote, non-horizontal-rule lines.
pub(super) fn meaningful_non_heading_lines(text: &str) -> Vec<String> {
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
        if super::HORIZONTAL_RULE_RE.as_ref().is_some_and(|re| re.is_match(stripped)) {
            continue;
        }
        meaningful.push(stripped.to_owned());
    }
    meaningful
}

/// Returns `true` when `text` contains at least one task-item line.
pub(super) fn has_task_items(text: &str) -> bool {
    text.lines().any(|line| super::TASK_LINE_RE.as_ref().is_some_and(|re| re.is_match(line)))
}

// ---------------------------------------------------------------------------
// File validators
// ---------------------------------------------------------------------------

/// Validate a `spec.json` artifact: must be readable and decode without error.
pub(super) fn validate_spec_json_file(path: &Path, root: &Path) -> Vec<VerifyFinding> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            return vec![VerifyFinding::error(format!(
                "[ERROR] Cannot read spec.json: {} ({e})",
                super::display_path(path, root)
            ))];
        }
    };
    if text.trim().is_empty() {
        return vec![VerifyFinding::error(format!(
            "[ERROR] Latest track spec.json is empty: {}",
            super::display_path(path, root)
        ))];
    }
    let doc = match crate::spec::codec::decode(&text) {
        Ok(d) => d,
        Err(e) => {
            return vec![VerifyFinding::error(format!(
                "[ERROR] Latest track spec.json is invalid: {} ({e})",
                super::display_path(path, root)
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
    let display = super::display_path(path, root);
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

pub(super) fn validate_spec_file(path: &Path, root: &Path) -> Vec<VerifyFinding> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            return vec![VerifyFinding::error(format!(
                "[ERROR] Cannot read spec.md: {} ({e})",
                super::display_path(path, root)
            ))];
        }
    };
    let mut findings = Vec::new();
    if text.trim().is_empty() {
        return vec![VerifyFinding::error(format!(
            "[ERROR] Latest track spec.md is empty: {}",
            super::display_path(path, root)
        ))];
    }
    let placeholders = placeholder_lines(&text);
    if !placeholders.is_empty() {
        findings.push(VerifyFinding::error(format!(
            "[ERROR] Latest track spec.md still contains placeholders: {}",
            super::display_path(path, root)
        )));
        for (line_number, line) in &placeholders {
            findings.push(VerifyFinding::error(format!("  {line_number}:{line}")));
        }
    }
    if meaningful_non_heading_lines(&text).is_empty() {
        findings.push(VerifyFinding::error(format!(
            "[ERROR] Latest track spec.md lacks substantive content beyond headings: {}",
            super::display_path(path, root)
        )));
    }
    findings
}

pub(super) fn validate_plan_file(path: &Path, root: &Path) -> Vec<VerifyFinding> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            return vec![VerifyFinding::error(format!(
                "[ERROR] Cannot read plan.md: {} ({e})",
                super::display_path(path, root)
            ))];
        }
    };
    let mut findings = Vec::new();
    if text.trim().is_empty() {
        return vec![VerifyFinding::error(format!(
            "[ERROR] Latest track plan.md is empty: {}",
            super::display_path(path, root)
        ))];
    }
    let placeholders = placeholder_lines(&text);
    if !placeholders.is_empty() {
        findings.push(VerifyFinding::error(format!(
            "[ERROR] Latest track plan.md still contains placeholders: {}",
            super::display_path(path, root)
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
            super::display_path(path, root)
        )));
    }
    findings
}
