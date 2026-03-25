//! Scope filtering for reviewer findings.
//!
//! Classifies findings by whether they fall within the diff scope of a review,
//! and applies filtering so that out-of-scope findings do not block commits.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::review_workflow::{ReviewFinalPayload, ReviewFinding, ReviewPayloadVerdict};

// ---------------------------------------------------------------------------
// Core path type
// ---------------------------------------------------------------------------

/// A normalized, repo-relative file path (forward-slash separated, no leading `./`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoRelativePath(String);

impl RepoRelativePath {
    /// Normalizes a raw file path string into a [`RepoRelativePath`].
    ///
    /// Rules applied (in order):
    /// 1. Replace `\` with `/`.
    /// 2. Strip a leading `./`.
    /// 3. Reject absolute paths (starting with `/`).
    /// 4. Reject Windows drive-letter paths (e.g., `C:/...`).
    /// 5. Reject parent traversal (`../` prefix, mid-path `/../`, trailing `/..`).
    /// 6. Strip trailing `/`.
    /// 7. Reject empty result.
    ///
    /// Bare filenames (e.g., `Cargo.toml`) are accepted — they represent valid
    /// root-level repo files. Ambiguity is handled at classification time, not here.
    ///
    /// Returns `None` when the path cannot be reliably normalized.
    #[must_use]
    pub fn normalize(raw: &str) -> Option<Self> {
        // Step 0: trim surrounding whitespace and backtick wrapping.
        let raw = raw.trim();
        let raw = raw.strip_prefix('`').and_then(|s| s.strip_suffix('`')).unwrap_or(raw);

        // NOTE: We do NOT strip `:digits` or `(...)` suffixes. The --output-schema
        // constrains the reviewer to use separate `file` and `line` fields. Stripping
        // decorations collides with valid filenames (e.g., `docs/rfc:2026`).
        // Decorated paths will fail to match DiffScope and be handled by fail-closed rules.

        // Step 1: replace backslashes with forward slashes.
        let mut normalized = raw.replace('\\', "/");

        // Step 2: reject absolute paths (before any segment manipulation).
        if normalized.starts_with('/') {
            return None;
        }

        // Step 3: canonicalize segments — collapse //, remove `.`, strip trailing `/`.
        // Must happen BEFORE drive-letter/traversal checks so that `./C:/...` is
        // canonicalized to `C:/...` before the drive-letter check catches it.
        let parts: Vec<&str> =
            normalized.split('/').filter(|seg| !seg.is_empty() && *seg != ".").collect();
        normalized = parts.join("/");

        // Step 4: reject Windows drive-letter paths (e.g., "C:/repo/file.rs").
        if normalized.len() >= 2
            && normalized.as_bytes().first().is_some_and(|b| b.is_ascii_alphabetic())
            && normalized.as_bytes().get(1) == Some(&b':')
        {
            return None;
        }

        // Step 5: reject parent traversal (prefix, mid-path, or trailing).
        if normalized.starts_with("../")
            || normalized == ".."
            || normalized.contains("/../")
            || normalized.ends_with("/..")
        {
            return None;
        }

        // Step 6: reject empty.
        if normalized.is_empty() {
            return None;
        }

        Some(Self(normalized.to_owned()))
    }

    /// Returns `true` if this path has no directory separator (a root-level file).
    #[must_use]
    pub fn is_bare_filename(&self) -> bool {
        !self.0.contains('/')
    }

    /// Returns `true` if this path contains decoration characters (`:`, ` (`) that
    /// suggest the reviewer embedded line/column info or annotations into the file field.
    /// These paths should not be treated as definitive OutOfScope — they are ambiguous.
    #[must_use]
    pub fn looks_decorated(&self) -> bool {
        self.0.contains(':') || self.0.contains(" (")
    }

    /// Returns the inner path string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// DiffScope
// ---------------------------------------------------------------------------

/// The set of repo-relative file paths that changed in a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffScope {
    files: BTreeSet<RepoRelativePath>,
}

impl DiffScope {
    /// Creates a [`DiffScope`] from an iterator of [`RepoRelativePath`]s.
    pub fn new(files: impl IntoIterator<Item = RepoRelativePath>) -> Self {
        Self { files: files.into_iter().collect() }
    }

    /// Returns `true` if `path` is present in this scope.
    #[must_use]
    pub fn contains(&self, path: &RepoRelativePath) -> bool {
        self.files.contains(path)
    }
}

// ---------------------------------------------------------------------------
// Finding scope classification
// ---------------------------------------------------------------------------

/// How a single finding's file path relates to the diff scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingScopeClass {
    /// The finding is within (or tied to) the diff scope.
    InScope,
    /// The finding's path was normalized and is NOT in the diff scope.
    OutOfScope,
    /// The finding's path could not be normalized — scope is indeterminate.
    UnknownPath,
}

/// Classifies a single finding's `file` field against `scope`.
///
/// - `None` file → [`FindingScopeClass::InScope`] (can't determine otherwise).
/// - Normalizable path in scope → [`FindingScopeClass::InScope`].
/// - Normalizable path not in scope → [`FindingScopeClass::OutOfScope`].
/// - Bare filename not in scope → [`FindingScopeClass::UnknownPath`] (ambiguous —
///   could refer to a nested file; fail-closed).
/// - Non-normalizable path → [`FindingScopeClass::UnknownPath`].
#[must_use]
pub fn classify_finding_scope(finding_file: Option<&str>, scope: &DiffScope) -> FindingScopeClass {
    let Some(raw) = finding_file else {
        return FindingScopeClass::InScope;
    };

    match RepoRelativePath::normalize(raw) {
        None => FindingScopeClass::UnknownPath,
        Some(path) => {
            if scope.contains(&path) {
                FindingScopeClass::InScope
            } else if path.is_bare_filename() || path.looks_decorated() {
                // Bare filenames and decorated paths (e.g., `foo.rs:42`) not in scope
                // are ambiguous. Fail-closed: treat as unknown.
                FindingScopeClass::UnknownPath
            } else {
                FindingScopeClass::OutOfScope
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Partition helpers
// ---------------------------------------------------------------------------

/// The result of partitioning findings into in-scope and out-of-scope buckets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeFilterResult {
    /// Findings that are in scope (or whose scope is indeterminate).
    pub in_scope: Vec<ReviewFinding>,
    /// Findings that are definitively out of scope.
    pub out_of_scope: Vec<ReviewFinding>,
    /// Number of findings whose path could not be normalized (counted in `in_scope`).
    pub unknown_path_count: usize,
}

/// Partitions `findings` into in-scope and out-of-scope buckets.
///
/// [`FindingScopeClass::UnknownPath`] findings are placed in `in_scope` (fail-safe)
/// and increment `unknown_path_count`.
#[must_use]
pub fn partition_findings_by_scope(
    findings: Vec<ReviewFinding>,
    scope: &DiffScope,
) -> ScopeFilterResult {
    let mut in_scope = Vec::new();
    let mut out_of_scope = Vec::new();
    let mut unknown_path_count = 0usize;

    for finding in findings {
        match classify_finding_scope(finding.file.as_deref(), scope) {
            FindingScopeClass::InScope => in_scope.push(finding),
            FindingScopeClass::OutOfScope => out_of_scope.push(finding),
            FindingScopeClass::UnknownPath => {
                unknown_path_count += 1;
                in_scope.push(finding);
            }
        }
    }

    ScopeFilterResult { in_scope, out_of_scope, unknown_path_count }
}

// ---------------------------------------------------------------------------
// Filtered payload
// ---------------------------------------------------------------------------

/// A [`ReviewFinalPayload`] after scope filtering, together with metadata about
/// what was removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeFilteredPayload {
    /// The payload with out-of-scope findings removed and verdict adjusted.
    pub adjusted_payload: ReviewFinalPayload,
    /// Findings that were removed because they are out of scope.
    pub out_of_scope: Vec<ReviewFinding>,
    /// Number of findings whose path could not be normalized (kept in `adjusted_payload`).
    pub unknown_path_count: usize,
}

/// Applies scope filtering to a [`ReviewFinalPayload`].
///
/// If the payload's verdict is already [`ReviewPayloadVerdict::ZeroFindings`], it
/// is returned unchanged (no filtering necessary).
///
/// Otherwise:
/// - Findings are partitioned by scope.
/// - If all findings were out-of-scope, the adjusted verdict becomes
///   [`ReviewPayloadVerdict::ZeroFindings`] with an empty findings list.
/// - If some findings remain in-scope, the verdict stays
///   [`ReviewPayloadVerdict::FindingsRemain`] with only the in-scope findings.
#[must_use]
pub fn apply_scope_filter(payload: ReviewFinalPayload, scope: &DiffScope) -> ScopeFilteredPayload {
    if payload.verdict == ReviewPayloadVerdict::ZeroFindings {
        return ScopeFilteredPayload {
            adjusted_payload: payload,
            out_of_scope: Vec::new(),
            unknown_path_count: 0,
        };
    }

    let ScopeFilterResult { in_scope, out_of_scope, unknown_path_count } =
        partition_findings_by_scope(payload.findings, scope);

    let adjusted_payload = if in_scope.is_empty() {
        ReviewFinalPayload { verdict: ReviewPayloadVerdict::ZeroFindings, findings: Vec::new() }
    } else {
        ReviewFinalPayload { verdict: ReviewPayloadVerdict::FindingsRemain, findings: in_scope }
    };

    ScopeFilteredPayload { adjusted_payload, out_of_scope, unknown_path_count }
}

// ---------------------------------------------------------------------------
// DiffScopeProvider port
// ---------------------------------------------------------------------------

/// Errors returned by [`DiffScopeProvider`].
#[derive(Debug, Error)]
pub enum DiffScopeProviderError {
    /// The requested base ref could not be resolved.
    #[error("base ref '{base_ref}' could not be resolved")]
    UnknownBaseRef {
        /// The base ref that was not found.
        base_ref: String,
    },
    /// Any other error while computing the diff scope.
    #[error("failed to compute diff scope: {0}")]
    Other(String),
}

/// Port for computing the set of files changed relative to a base ref.
pub trait DiffScopeProvider {
    /// Returns the set of repo-relative paths changed since `base_ref`.
    ///
    /// # Errors
    /// Returns [`DiffScopeProviderError::UnknownBaseRef`] if `base_ref` cannot be
    /// resolved, or [`DiffScopeProviderError::Other`] for any other failure.
    fn changed_files(&self, base_ref: &str) -> Result<DiffScope, DiffScopeProviderError>;
}

// ---------------------------------------------------------------------------
// Path decoration strippers (private helpers for normalize)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tests (written first — Red phase)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // Helper: build a DiffScope from string slices.
    fn scope_from_strs(paths: &[&str]) -> DiffScope {
        DiffScope::new(paths.iter().filter_map(|p| RepoRelativePath::normalize(p)))
    }

    // Helper: build a finding with a given file path.
    fn finding_with_file(file: Option<&str>) -> ReviewFinding {
        ReviewFinding {
            message: "test finding".to_owned(),
            severity: None,
            file: file.map(str::to_owned),
            line: None,
            category: None,
        }
    }

    // -----------------------------------------------------------------------
    // RepoRelativePath::normalize
    // -----------------------------------------------------------------------

    #[test]
    fn test_repo_relative_path_normalize_strips_dot_slash() {
        let result = RepoRelativePath::normalize("./libs/domain/src/foo.rs");
        assert_eq!(result.unwrap().as_str(), "libs/domain/src/foo.rs");
    }

    #[test]
    fn test_repo_relative_path_normalize_converts_backslash() {
        let result = RepoRelativePath::normalize("libs\\domain\\src\\foo.rs");
        assert_eq!(result.unwrap().as_str(), "libs/domain/src/foo.rs");
    }

    #[test]
    fn test_repo_relative_path_normalize_bare_filename_accepted() {
        // Root-level files like Cargo.toml are valid repo-relative paths.
        let path = RepoRelativePath::normalize("Cargo.toml").unwrap();
        assert_eq!(path.as_str(), "Cargo.toml");
        assert!(path.is_bare_filename());
    }

    #[test]
    fn test_repo_relative_path_normalize_mid_path_parent_traversal_returns_none() {
        assert!(RepoRelativePath::normalize("libs/domain/../src/foo.rs").is_none());
        assert!(RepoRelativePath::normalize("libs/domain/..").is_none());
    }

    #[test]
    fn test_repo_relative_path_normalize_absolute_returns_none() {
        let result = RepoRelativePath::normalize("/home/user/project/libs/domain/src/foo.rs");
        assert!(result.is_none());
    }

    #[test]
    fn test_repo_relative_path_normalize_valid_relative() {
        let result = RepoRelativePath::normalize("libs/domain/src/foo.rs");
        assert_eq!(result.unwrap().as_str(), "libs/domain/src/foo.rs");
    }

    #[test]
    fn test_repo_relative_path_normalize_windows_drive_letter_returns_none() {
        assert!(RepoRelativePath::normalize("C:\\repo\\file.rs").is_none());
        assert!(RepoRelativePath::normalize("D:/repo/file.rs").is_none());
    }

    #[test]
    fn test_repo_relative_path_normalize_parent_traversal_returns_none() {
        assert!(RepoRelativePath::normalize("../file.rs").is_none());
        assert!(RepoRelativePath::normalize("..").is_none());
    }

    #[test]
    fn test_repo_relative_path_normalize_repeated_dot_slash_stripped() {
        let result = RepoRelativePath::normalize("././Cargo.toml");
        assert_eq!(result.unwrap().as_str(), "Cargo.toml");
    }

    #[test]
    fn test_repo_relative_path_normalize_double_slash_collapsed() {
        let result = RepoRelativePath::normalize("libs//domain//src/foo.rs");
        assert_eq!(result.unwrap().as_str(), "libs/domain/src/foo.rs");
    }

    #[test]
    fn test_repo_relative_path_normalize_trailing_slash_stripped() {
        let result = RepoRelativePath::normalize("libs/domain/src/");
        assert_eq!(result.unwrap().as_str(), "libs/domain/src");
    }

    #[test]
    fn test_repo_relative_path_normalize_empty_returns_none() {
        assert!(RepoRelativePath::normalize("").is_none());
    }

    #[test]
    fn test_repo_relative_path_normalize_trims_whitespace() {
        let result = RepoRelativePath::normalize("  libs/domain/src/foo.rs\n");
        assert_eq!(result.unwrap().as_str(), "libs/domain/src/foo.rs");
    }

    #[test]
    fn test_repo_relative_path_normalize_internal_dot_segment() {
        let result = RepoRelativePath::normalize("src/./lib.rs");
        assert_eq!(result.unwrap().as_str(), "src/lib.rs");
    }

    #[test]
    fn test_repo_relative_path_normalize_dot_slash_drive_letter_rejected() {
        assert!(RepoRelativePath::normalize("./C:/repo/file.rs").is_none());
    }

    #[test]
    fn test_repo_relative_path_normalize_backtick_wrapping() {
        let result = RepoRelativePath::normalize("`libs/domain/src/foo.rs`");
        assert_eq!(result.unwrap().as_str(), "libs/domain/src/foo.rs");
    }

    #[test]
    fn test_repo_relative_path_normalize_decorated_path_not_stripped() {
        // Decorated paths are NOT stripped — they contain valid characters
        // and the --output-schema constrains file/line separation.
        // These will fail to match DiffScope and be handled by fail-closed rules.
        let colon = RepoRelativePath::normalize("libs/domain/src/foo.rs:42");
        assert_eq!(colon.unwrap().as_str(), "libs/domain/src/foo.rs:42");

        let paren = RepoRelativePath::normalize("libs/domain/src/foo.rs (line 42)");
        assert_eq!(paren.unwrap().as_str(), "libs/domain/src/foo.rs (line 42)");
    }

    // -----------------------------------------------------------------------
    // classify_finding_scope
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_finding_scope_none_file_is_in_scope() {
        let scope = scope_from_strs(&["libs/domain/src/foo.rs"]);
        assert_eq!(classify_finding_scope(None, &scope), FindingScopeClass::InScope);
    }

    #[test]
    fn test_classify_finding_scope_in_scope() {
        let scope = scope_from_strs(&["libs/domain/src/foo.rs"]);
        assert_eq!(
            classify_finding_scope(Some("libs/domain/src/foo.rs"), &scope),
            FindingScopeClass::InScope
        );
    }

    #[test]
    fn test_classify_finding_scope_out_of_scope() {
        let scope = scope_from_strs(&["libs/domain/src/foo.rs"]);
        assert_eq!(
            classify_finding_scope(Some("libs/domain/src/bar.rs"), &scope),
            FindingScopeClass::OutOfScope
        );
    }

    #[test]
    fn test_classify_finding_scope_unnormalizable_is_unknown_path() {
        let scope = scope_from_strs(&["libs/domain/src/foo.rs"]);
        // Absolute path — cannot be normalized.
        assert_eq!(
            classify_finding_scope(Some("/absolute/path.rs"), &scope),
            FindingScopeClass::UnknownPath
        );
    }

    #[test]
    fn test_classify_finding_scope_bare_filename_not_in_scope_is_unknown() {
        let scope = scope_from_strs(&["libs/domain/src/foo.rs"]);
        // Bare filename not in scope → ambiguous → UnknownPath (fail-closed).
        assert_eq!(
            classify_finding_scope(Some("state.rs"), &scope),
            FindingScopeClass::UnknownPath
        );
    }

    #[test]
    fn test_classify_finding_scope_bare_filename_in_scope_is_in_scope() {
        let scope = scope_from_strs(&["Cargo.toml"]);
        // Bare filename that IS in scope → InScope.
        assert_eq!(classify_finding_scope(Some("Cargo.toml"), &scope), FindingScopeClass::InScope);
    }

    #[test]
    fn test_classify_finding_scope_decorated_path_is_unknown() {
        let scope = scope_from_strs(&["libs/usecase/src/review_workflow/scope.rs"]);
        // Decorated with :line → not exact match → UnknownPath (fail-closed).
        assert_eq!(
            classify_finding_scope(Some("libs/usecase/src/review_workflow/scope.rs:42"), &scope),
            FindingScopeClass::UnknownPath
        );
    }

    #[test]
    fn test_classify_finding_scope_paren_decorated_is_unknown() {
        let scope = scope_from_strs(&["libs/domain/src/foo.rs"]);
        assert_eq!(
            classify_finding_scope(Some("libs/domain/src/foo.rs (line 42)"), &scope),
            FindingScopeClass::UnknownPath
        );
    }

    // -----------------------------------------------------------------------
    // partition_findings_by_scope
    // -----------------------------------------------------------------------

    #[test]
    fn test_partition_findings_by_scope_mixed() {
        let scope = scope_from_strs(&["libs/domain/src/foo.rs"]);

        let findings = vec![
            finding_with_file(Some("libs/domain/src/foo.rs")), // in scope
            finding_with_file(Some("libs/domain/src/bar.rs")), // out of scope
            finding_with_file(Some("state.rs")),               // unknown path
            finding_with_file(None),                           // no file -> in scope
        ];

        let result = partition_findings_by_scope(findings, &scope);

        assert_eq!(result.in_scope.len(), 3);
        assert_eq!(result.out_of_scope.len(), 1);
        assert_eq!(result.unknown_path_count, 1);
    }

    // -----------------------------------------------------------------------
    // apply_scope_filter
    // -----------------------------------------------------------------------

    #[test]
    fn test_apply_scope_filter_all_out_of_scope_becomes_zero_findings() {
        let scope = scope_from_strs(&["libs/domain/src/foo.rs"]);

        let payload = ReviewFinalPayload {
            verdict: ReviewPayloadVerdict::FindingsRemain,
            findings: vec![
                finding_with_file(Some("libs/other/src/bar.rs")), // out of scope
            ],
        };

        let filtered = apply_scope_filter(payload, &scope);

        assert_eq!(filtered.adjusted_payload.verdict, ReviewPayloadVerdict::ZeroFindings);
        assert!(filtered.adjusted_payload.findings.is_empty());
        assert_eq!(filtered.out_of_scope.len(), 1);
        assert_eq!(filtered.unknown_path_count, 0);
    }

    #[test]
    fn test_apply_scope_filter_zero_findings_passthrough() {
        let scope = scope_from_strs(&["libs/domain/src/foo.rs"]);

        let payload =
            ReviewFinalPayload { verdict: ReviewPayloadVerdict::ZeroFindings, findings: vec![] };

        let filtered = apply_scope_filter(payload.clone(), &scope);

        assert_eq!(filtered.adjusted_payload, payload);
        assert!(filtered.out_of_scope.is_empty());
        assert_eq!(filtered.unknown_path_count, 0);
    }
}
