use std::fmt;

use super::error::{FilePathError, FindingError, ReviewHashError, ScopeNameError, VerdictError};

// ── FilePath ──────────────────────────────────────────────────────────

/// A validated repo-relative file path used in review scope classification and hashing.
///
/// Rejects empty strings, absolute paths, and `..` traversal components.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FilePath(String);

impl FilePath {
    /// Creates a validated repo-relative file path.
    ///
    /// # Errors
    /// - `FilePathError::Empty` if empty
    /// - `FilePathError::Absolute` if the path starts with `/` or contains a Windows drive prefix
    /// - `FilePathError::Traversal` if the path contains `..` components (Unix or Windows separators)
    pub fn new(s: impl Into<String>) -> Result<Self, FilePathError> {
        let s = s.into();
        if s.is_empty() {
            return Err(FilePathError::Empty);
        }
        // Reject Unix absolute, Windows drive prefix (C:/ C:\), and UNC/rooted paths (\)
        if s.starts_with('/')
            || s.starts_with('\\')
            || s.get(1..3).is_some_and(|prefix| prefix == ":\\" || prefix == ":/")
        {
            return Err(FilePathError::Absolute(s));
        }
        // Check for '..' traversal using both Unix and Windows separators
        if s.split(&['/', '\\'][..]).any(|seg| seg == "..") {
            return Err(FilePathError::Traversal(s));
        }
        Ok(Self(s))
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

// ── ReviewHashValue / ReviewHash ──────────────────────────────────────

/// A validated review hash string.
///
/// Format: `"rvw1:sha256:<hex>"` where `<hex>` is one or more lowercase hex digits.
/// The inner `String` is private — construction only through `new()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewHashValue(String);

impl ReviewHashValue {
    const PREFIX: &str = "rvw1:sha256:";

    /// Creates a validated review hash value.
    ///
    /// # Errors
    /// Returns `ReviewHashError::InvalidFormat` if the value does not match
    /// `"rvw1:sha256:<hex>"`.
    pub fn new(s: impl Into<String>) -> Result<Self, ReviewHashError> {
        let s = s.into();
        match s.strip_prefix(Self::PREFIX) {
            Some(hex)
                if !hex.is_empty()
                    && hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) =>
            {
                Ok(Self(s))
            }
            _ => Err(ReviewHashError::InvalidFormat(s)),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ReviewHashValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Hash of a review scope's file contents.
///
/// `Computed(ReviewHashValue)` holds a validated `"rvw1:sha256:<hex>"` value.
/// `Empty` indicates no files in the scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewHash {
    Computed(ReviewHashValue),
    Empty,
}

impl ReviewHash {
    /// Creates a `Computed` variant with format validation.
    ///
    /// # Errors
    /// Returns `ReviewHashError::InvalidFormat` if the value is not valid.
    pub fn computed(s: impl Into<String>) -> Result<Self, ReviewHashError> {
        Ok(Self::Computed(ReviewHashValue::new(s)?))
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, ReviewHash::Empty)
    }

    /// Returns the hash string if `Computed`, `None` if `Empty`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ReviewHash::Computed(v) => Some(v.as_str()),
            ReviewHash::Empty => None,
        }
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

// ── NonEmptyFindings ──────────────────────────────────────────────────

/// A non-empty collection of findings.
///
/// Guarantees at least one `Finding` is present. The inner `Vec` is private —
/// construction only through `new()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyFindings(Vec<Finding>);

impl NonEmptyFindings {
    /// Creates a validated non-empty findings collection.
    ///
    /// # Errors
    /// Returns `VerdictError::EmptyFindings` if `findings` is empty.
    pub fn new(findings: Vec<Finding>) -> Result<Self, VerdictError> {
        if findings.is_empty() {
            return Err(VerdictError::EmptyFindings);
        }
        Ok(Self(findings))
    }

    pub fn as_slice(&self) -> &[Finding] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<Finding> {
        self.0
    }
}

// ── Verdict / FastVerdict ─────────────────────────────────────────────

/// Final review verdict for a scope.
///
/// Invariant: `FindingsRemain` always contains at least one finding
/// (enforced by `NonEmptyFindings`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    ZeroFindings,
    FindingsRemain(NonEmptyFindings),
}

impl Verdict {
    /// Constructs a `FindingsRemain` variant with non-empty guarantee.
    ///
    /// # Errors
    /// Returns `VerdictError::EmptyFindings` if `findings` is empty.
    pub fn findings_remain(findings: Vec<Finding>) -> Result<Self, VerdictError> {
        Ok(Self::FindingsRemain(NonEmptyFindings::new(findings)?))
    }
}

/// Fast review verdict (advisory only — not used for approval decisions).
///
/// Same structure as `Verdict` but distinct type to prevent misuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastVerdict {
    ZeroFindings,
    FindingsRemain(NonEmptyFindings),
}

impl FastVerdict {
    /// Constructs a `FindingsRemain` variant with non-empty guarantee.
    ///
    /// # Errors
    /// Returns `VerdictError::EmptyFindings` if `findings` is empty.
    pub fn findings_remain(findings: Vec<Finding>) -> Result<Self, VerdictError> {
        Ok(Self::FindingsRemain(NonEmptyFindings::new(findings)?))
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
