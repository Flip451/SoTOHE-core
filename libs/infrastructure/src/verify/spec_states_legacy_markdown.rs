//! Legacy `spec.md` (markdown) verification path extracted from `spec_states`.
//!
//! Hosts the legacy markdown-table-based `## Domain States` validation chain plus
//! the trusted-root guard for legacy `spec.md` reads. Kept as a sibling module
//! so `spec_states.rs` stays under the 700-line module-size cap.

use std::path::{Path, PathBuf};

use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::track::symlink_guard;

use super::spec_states::visible_markdown_body_lines;

/// Result of running the markdown-side `## Domain States` validation.
pub(crate) enum DomainStatesMarkdownCheck {
    MissingSection,
    EmptyTable,
    MissingSeparator,
    SeparatorWithoutHeader,
    NoDataRows,
    Valid,
}

/// Trusted-root + symlink guard for the legacy `spec.md` path before reading.
pub(crate) fn guard_legacy_spec_markdown_path(
    spec_path: &Path,
    trusted_root: Option<&Path>,
) -> Result<PathBuf, VerifyOutcome> {
    let absolute_spec =
        super::path_safety::lexical_normalize(&super::trusted_root::absolutize(spec_path));
    let root = match trusted_root {
        Some(root) => {
            let normalized_root =
                super::path_safety::lexical_normalize(&super::trusted_root::absolutize(root));
            super::trusted_root::ensure_not_symlink_root(normalized_root).map_err(|e| {
                VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "{}: trusted_root rejected before reading legacy spec.md: {e}",
                    spec_path.display()
                ))])
            })?
        }
        None => super::trusted_root::resolve_trusted_root(&absolute_spec).map_err(|e| {
            VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: trusted_root resolution failed before reading legacy spec.md: {e}",
                spec_path.display()
            ))])
        })?,
    };
    let root = super::path_safety::lexical_normalize(&root);

    if !absolute_spec.starts_with(&root) {
        return Err(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{}: legacy spec.md path resolves outside trusted_root '{}'",
            spec_path.display(),
            root.display()
        ))]));
    }

    match symlink_guard::reject_symlinks_below(&absolute_spec, &root) {
        Ok(true) => Ok(absolute_spec),
        Ok(false) => Err(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "cannot read {}: file not found",
            spec_path.display()
        ))])),
        Err(e) => Err(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{}: symlink guard: {e}",
            spec_path.display()
        ))])),
    }
}

/// Maps the markdown-validation result to a [`VerifyOutcome`].
pub(crate) fn verify_domain_states_markdown(spec_path: &Path, content: &str) -> VerifyOutcome {
    match check_domain_states_markdown(content) {
        DomainStatesMarkdownCheck::Valid => VerifyOutcome::pass(),
        DomainStatesMarkdownCheck::MissingSection => {
            VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: missing '## Domain States' section",
                spec_path.display()
            ))])
        }
        DomainStatesMarkdownCheck::EmptyTable => {
            VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: '## Domain States' section has no markdown table",
                spec_path.display()
            ))])
        }
        DomainStatesMarkdownCheck::MissingSeparator => {
            VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: '## Domain States' table has no separator row (header-only table)",
                spec_path.display()
            ))])
        }
        DomainStatesMarkdownCheck::SeparatorWithoutHeader => {
            VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: '## Domain States' table has no header row before separator",
                spec_path.display()
            ))])
        }
        DomainStatesMarkdownCheck::NoDataRows => {
            VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: '## Domain States' table has no data rows (header + separator only)",
                spec_path.display()
            ))])
        }
    }
}

pub(crate) fn check_domain_states_markdown(content: &str) -> DomainStatesMarkdownCheck {
    let body_lines = visible_markdown_body_lines(content);
    let Some(section_idx) =
        body_lines.iter().position(|line| line.trim_end() == "## Domain States")
    else {
        return DomainStatesMarkdownCheck::MissingSection;
    };

    let table_lines: Vec<&str> = body_lines
        .iter()
        .skip(section_idx + 1)
        .take_while(|line| !is_markdown_section_boundary(line))
        .copied()
        .filter(|line| line.trim_start().starts_with('|'))
        .collect();

    if table_lines.is_empty() {
        return DomainStatesMarkdownCheck::EmptyTable;
    }

    let sep_idx = table_lines.iter().position(|l| is_table_separator(l));
    let Some(sep_pos) = sep_idx else {
        return DomainStatesMarkdownCheck::MissingSeparator;
    };
    if sep_pos == 0 {
        return DomainStatesMarkdownCheck::SeparatorWithoutHeader;
    }

    let has_data_row = table_lines.iter().skip(sep_pos + 1).any(|line| !is_table_separator(line));
    if has_data_row {
        DomainStatesMarkdownCheck::Valid
    } else {
        DomainStatesMarkdownCheck::NoDataRows
    }
}

fn is_markdown_section_boundary(line: &str) -> bool {
    let trimmed = line.trim_end();
    trimmed.starts_with("## ") || trimmed == "##" || trimmed.starts_with("# ") || trimmed == "#"
}

/// Returns `true` when `line` is a markdown table separator row.
fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return false;
    }
    if !trimmed.contains('-') {
        return false;
    }
    trimmed.chars().all(|c| c == '|' || c == '-' || c == ':' || c == ' ')
}
