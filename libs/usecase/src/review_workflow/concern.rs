//! Concern extraction from review findings.

use super::verdict::ReviewFinding;

/// Normalizes a `ReviewFinding` into a `ReviewConcern`.
///
/// Fallback order: category → file path (via `domain::review::file_path_to_concern`) → "other".
///
/// # Errors
///
/// Returns `domain::ReviewError::InvalidConcern` if the derived concern slug is empty
/// (which should not occur in practice given the "other" fallback).
pub fn finding_to_concern(
    finding: &ReviewFinding,
) -> Result<domain::ReviewConcern, domain::ReviewError> {
    if let Some(ref cat) = finding.category {
        if !cat.trim().is_empty() {
            return domain::ReviewConcern::try_new(cat.as_str());
        }
    }
    if let Some(ref file) = finding.file {
        if !file.trim().is_empty() {
            let slug = domain::review::file_path_to_concern(file);
            if !slug.trim().is_empty() {
                return domain::ReviewConcern::try_new(slug);
            }
        }
    }
    domain::ReviewConcern::try_new("other")
}

/// Extracts and normalizes concerns from findings, deduplicating and sorting.
///
/// # Errors
///
/// Returns `domain::ReviewError::InvalidConcern` if a derived concern slug is empty.
pub fn findings_to_concerns(
    findings: &[ReviewFinding],
) -> Result<Vec<domain::ReviewConcern>, domain::ReviewError> {
    let mut set = std::collections::BTreeSet::new();
    for f in findings {
        set.insert(finding_to_concern(f)?);
    }
    Ok(set.into_iter().collect())
}
