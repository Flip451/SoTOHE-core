use std::fmt;

use super::error::{FindingError, ScopeNameError, VerdictError};

// ── FilePath ──────────────────────────────────────────────────────────

/// A repo-relative file path used in review scope classification and hashing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FilePath(String);

impl FilePath {
    /// Creates a new file path.
    ///
    /// No validation — the path is assumed to be repo-relative.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ── ScopeName ─────────────────────────────────────────────────────────

/// A named review scope that is not `other`.
///
/// Validates:
/// - Non-empty
/// - ASCII only
/// - Not the reserved name "other"
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MainScopeName(String);

impl MainScopeName {
    /// Creates a validated scope name.
    ///
    /// # Errors
    /// - `ScopeNameError::Empty` if empty
    /// - `ScopeNameError::NotAscii` if contains non-ASCII characters
    /// - `ScopeNameError::Reserved` if the value is "other"
    pub fn new(s: impl Into<String>) -> Result<Self, ScopeNameError> {
        let s = s.into();
        if s.is_empty() {
            return Err(ScopeNameError::Empty);
        }
        if !s.is_ascii() {
            return Err(ScopeNameError::NotAscii);
        }
        if s == "other" {
            return Err(ScopeNameError::Reserved);
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MainScopeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A review scope identifier.
///
/// Named scopes are `Main(MainScopeName)`; unmatched files go to `Other`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScopeName {
    Main(MainScopeName),
    Other,
}

impl fmt::Display for ScopeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScopeName::Main(name) => write!(f, "{name}"),
            ScopeName::Other => f.write_str("other"),
        }
    }
}

// ── ReviewTarget ──────────────────────────────────────────────────────

/// The set of files to be reviewed within a scope.
#[derive(Debug, Clone)]
pub struct ReviewTarget(Vec<FilePath>);

impl ReviewTarget {
    pub fn new(files: Vec<FilePath>) -> Self {
        Self(files)
    }

    pub fn files(&self) -> &[FilePath] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// ── ReviewHash ────────────────────────────────────────────────────────

/// Hash of a review scope's file contents.
///
/// `Computed(String)` holds a `"rvw1:sha256:<hex>"` value.
/// `Empty` indicates no files in the scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewHash {
    Computed(String),
    Empty,
}

impl ReviewHash {
    pub fn is_empty(&self) -> bool {
        matches!(self, ReviewHash::Empty)
    }
}

// ── Finding ───────────────────────────────────────────────────────────

/// A single reviewer finding with optional location metadata.
///
/// Invariant: `message` is non-empty (enforced by constructor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    message: String,
    severity: Option<String>,
    file: Option<String>,
    line: Option<u64>,
    category: Option<String>,
}

impl Finding {
    /// Creates a new finding.
    ///
    /// # Errors
    /// Returns `FindingError::EmptyMessage` if `message` is empty or whitespace-only.
    pub fn new(
        message: impl Into<String>,
        severity: Option<String>,
        file: Option<String>,
        line: Option<u64>,
        category: Option<String>,
    ) -> Result<Self, FindingError> {
        let message = message.into();
        if message.trim().is_empty() {
            return Err(FindingError::EmptyMessage);
        }
        Ok(Self { message, severity, file, line, category })
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn severity(&self) -> Option<&str> {
        self.severity.as_deref()
    }

    pub fn file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    pub fn line(&self) -> Option<u64> {
        self.line
    }

    pub fn category(&self) -> Option<&str> {
        self.category.as_deref()
    }
}

// ── Verdict / FastVerdict ─────────────────────────────────────────────

/// Final review verdict for a scope.
///
/// Invariant: `FindingsRemain` always contains at least one finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    ZeroFindings,
    FindingsRemain(Vec<Finding>),
}

impl Verdict {
    /// Constructs a `FindingsRemain` variant with non-empty guarantee.
    ///
    /// # Errors
    /// Returns `VerdictError::EmptyFindings` if `findings` is empty.
    pub fn findings_remain(findings: Vec<Finding>) -> Result<Self, VerdictError> {
        if findings.is_empty() {
            return Err(VerdictError::EmptyFindings);
        }
        Ok(Self::FindingsRemain(findings))
    }
}

/// Fast review verdict (advisory only — not used for approval decisions).
///
/// Same structure as `Verdict` but distinct type to prevent misuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastVerdict {
    ZeroFindings,
    FindingsRemain(Vec<Finding>),
}

impl FastVerdict {
    /// Constructs a `FindingsRemain` variant with non-empty guarantee.
    ///
    /// # Errors
    /// Returns `VerdictError::EmptyFindings` if `findings` is empty.
    pub fn findings_remain(findings: Vec<Finding>) -> Result<Self, VerdictError> {
        if findings.is_empty() {
            return Err(VerdictError::EmptyFindings);
        }
        Ok(Self::FindingsRemain(findings))
    }
}

// ── LogInfo ───────────────────────────────────────────────────────────

/// Opaque log payload returned by the reviewer alongside the verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogInfo(String);

impl LogInfo {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ── ReviewOutcome ─────────────────────────────────────────────────────

/// Result of a single scope review.
///
/// `Reviewed` carries the verdict, hash, and log info.
/// `Skipped` means the scope had no files (empty target).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewOutcome<V> {
    Reviewed { verdict: V, log_info: LogInfo, hash: ReviewHash },
    Skipped,
}

// ── ReviewState ───────────────────────────────────────────────────────

/// The review state of a single scope.
///
/// `Required` means the scope needs (re-)review.
/// `NotRequired` means the scope is approved or has no files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewState {
    Required(RequiredReason),
    NotRequired(NotRequiredReason),
}

/// Why a scope requires review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequiredReason {
    /// No final verdict has been recorded for this scope.
    NotStarted,
    /// The latest final verdict is `findings_remain`.
    FindingsRemain,
    /// The latest final verdict is `zero_findings` but the hash no longer matches.
    StaleHash,
}

/// Why a scope does not require review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotRequiredReason {
    /// The scope has no files (empty review target).
    Empty,
    /// The latest final verdict is `zero_findings` and the hash still matches.
    ZeroFindings,
}

impl ReviewState {
    /// Returns `true` if this scope is approved (does not require review).
    pub fn is_approved(&self) -> bool {
        matches!(self, ReviewState::NotRequired(_))
    }
}

impl fmt::Display for ReviewState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReviewState::Required(RequiredReason::NotStarted) => {
                f.write_str("required (not started)")
            }
            ReviewState::Required(RequiredReason::FindingsRemain) => {
                f.write_str("required (findings remain)")
            }
            ReviewState::Required(RequiredReason::StaleHash) => {
                f.write_str("required (stale hash)")
            }
            ReviewState::NotRequired(NotRequiredReason::Empty) => {
                f.write_str("not required (empty)")
            }
            ReviewState::NotRequired(NotRequiredReason::ZeroFindings) => f.write_str("approved"),
        }
    }
}
