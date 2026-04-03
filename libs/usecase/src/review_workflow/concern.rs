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
    if let Some(ref file) = finding.file {
        if file.trim().is_empty() {
            return Err(domain::ReviewError::InvalidConcern(
                "finding file must use `file: null` or a non-empty string".to_owned(),
            ));
        }
    }
    if let Some(ref cat) = finding.category {
        if cat.trim().is_empty() {
            return Err(domain::ReviewError::InvalidConcern(
                "finding category must use `category: null` or a non-empty string".to_owned(),
            ));
        }
        return domain::ReviewConcern::try_new(cat.trim());
    }
    if let Some(ref file) = finding.file {
        let slug = domain::review::file_path_to_concern(file.trim());
        if !slug.trim().is_empty() {
            return domain::ReviewConcern::try_new(slug);
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

#[cfg(test)]
mod tests {
    use super::{finding_to_concern, findings_to_concerns};
    use crate::review_workflow::verdict::ReviewFinding;

    fn finding(message: &str, file: Option<&str>, category: Option<&str>) -> ReviewFinding {
        ReviewFinding {
            message: message.to_owned(),
            severity: Some("P1".to_owned()),
            file: file.map(str::to_owned),
            line: Some(1),
            category: category.map(str::to_owned),
        }
    }

    #[test]
    fn test_finding_to_concern_rejects_blank_category() {
        let result = finding_to_concern(&finding("msg", None, Some(" ")));
        assert!(matches!(result, Err(domain::ReviewError::InvalidConcern(_))));
    }

    #[test]
    fn test_finding_to_concern_rejects_blank_file() {
        let result = finding_to_concern(&finding("msg", Some(" "), None));
        assert!(matches!(result, Err(domain::ReviewError::InvalidConcern(_))));
    }

    #[test]
    fn test_finding_to_concern_rejects_blank_file_even_when_category_is_present() {
        let result = finding_to_concern(&finding("msg", Some(" "), Some("domain.review")));
        assert!(matches!(result, Err(domain::ReviewError::InvalidConcern(_))));
    }

    #[test]
    fn test_finding_to_concern_trims_file_before_slugging() {
        let result = finding_to_concern(&finding("msg", Some(" libs/usecase/src/foo.rs "), None));
        assert!(result.is_ok(), "whitespace-padded file should still derive a concern");
        if let Ok(concern) = result {
            assert_eq!(concern.as_ref(), "usecase.foo");
        }
    }

    #[test]
    fn test_findings_to_concerns_deduplicates_normalized_values() {
        let result = findings_to_concerns(&[
            finding("msg1", None, Some("Domain.Review")),
            finding("msg2", Some("apps/cli/src/review.rs"), None),
        ]);

        assert!(result.is_ok(), "valid findings should produce concerns: {result:?}");
        if let Ok(concerns) = result {
            assert!(concerns.iter().any(|concern| concern.as_ref() == "domain.review"));
            assert!(concerns.iter().any(|concern| concern.as_ref() == "cli.review"));
        }
    }
}
