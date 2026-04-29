use std::fmt;

use super::error::{
    FilePathError, ReviewHashError, ReviewerFindingError, ScopeNameError, VerdictError,
};

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
        if s.eq_ignore_ascii_case("other") {
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

// ── ReviewerFinding ───────────────────────────────────────────────────────────

/// A single reviewer finding with optional location metadata.
///
/// Invariant: `message` is non-empty (enforced by constructor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerFinding {
    message: String,
    severity: Option<String>,
    file: Option<String>,
    line: Option<u64>,
    category: Option<String>,
}

impl ReviewerFinding {
    /// Creates a new finding.
    ///
    /// # Errors
    /// Returns `ReviewerFindingError::EmptyMessage` if `message` is empty or whitespace-only.
    pub fn new(
        message: impl Into<String>,
        severity: Option<String>,
        file: Option<String>,
        line: Option<u64>,
        category: Option<String>,
    ) -> Result<Self, ReviewerFindingError> {
        let message = message.into();
        if message.trim().is_empty() {
            return Err(ReviewerFindingError::EmptyMessage);
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

// ── NonEmptyReviewerFindings ──────────────────────────────────────────────────

/// A non-empty collection of findings.
///
/// Guarantees at least one `ReviewerFinding` is present. The inner `Vec` is private —
/// construction only through `new()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyReviewerFindings(Vec<ReviewerFinding>);

impl NonEmptyReviewerFindings {
    /// Creates a validated non-empty findings collection.
    ///
    /// # Errors
    /// Returns `VerdictError::EmptyFindings` if `findings` is empty.
    pub fn new(findings: Vec<ReviewerFinding>) -> Result<Self, VerdictError> {
        if findings.is_empty() {
            return Err(VerdictError::EmptyFindings);
        }
        Ok(Self(findings))
    }

    pub fn as_slice(&self) -> &[ReviewerFinding] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<ReviewerFinding> {
        self.0
    }
}

// ── Verdict / FastVerdict ─────────────────────────────────────────────

/// Final review verdict for a scope.
///
/// Invariant: `FindingsRemain` always contains at least one finding
/// (enforced by `NonEmptyReviewerFindings`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    ZeroFindings,
    FindingsRemain(NonEmptyReviewerFindings),
}

impl Verdict {
    /// Constructs a `FindingsRemain` variant with non-empty guarantee.
    ///
    /// # Errors
    /// Returns `VerdictError::EmptyFindings` if `findings` is empty.
    pub fn findings_remain(findings: Vec<ReviewerFinding>) -> Result<Self, VerdictError> {
        Ok(Self::FindingsRemain(NonEmptyReviewerFindings::new(findings)?))
    }
}

/// Round type discriminant for CLI `--round-type` arguments.
///
/// Used by `sotp review codex-local` to select fast vs final round semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum RoundType {
    Fast,
    Final,
}

/// Fast review verdict (advisory only — not used for approval decisions).
///
/// Same structure as `Verdict` but distinct type to prevent misuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastVerdict {
    ZeroFindings,
    FindingsRemain(NonEmptyReviewerFindings),
}

impl FastVerdict {
    /// Constructs a `FindingsRemain` variant with non-empty guarantee.
    ///
    /// # Errors
    /// Returns `VerdictError::EmptyFindings` if `findings` is empty.
    pub fn findings_remain(findings: Vec<ReviewerFinding>) -> Result<Self, VerdictError> {
        Ok(Self::FindingsRemain(NonEmptyReviewerFindings::new(findings)?))
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

// ── ScopeRound ────────────────────────────────────────────────────────

/// Persisted state of one review round for a single scope.
///
/// Carries round metadata read from `review.json` by the results command.
/// `hash` mirrors `ReviewHash` (Computed | Empty) so that historical rounds
/// that were stored with an empty hash (e.g. via `write_verdict` on a Skipped
/// scope) can be represented without data loss, parallel with
/// `ReviewReader::read_latest_finals`.
/// No serde derives — domain layer stays persistence-agnostic (CN-04).
#[derive(Debug, Clone)]
pub struct ScopeRound {
    pub round_type: RoundType,
    pub verdict: Verdict,
    pub findings: Vec<ReviewerFinding>,
    pub hash: ReviewHash,
    /// ISO 8601 timestamp recorded when the round was written.
    pub at: String,
}

// ── ReviewApprovalVerdict ─────────────────────────────────────────────

/// Domain verdict expressing the outcome of the approval/bypass check for the
/// entire track's review cycle.
///
/// Enum-first design (per `04-coding-principles.md`): each variant carries
/// exactly the data it needs, eliminating boolean flags and cross-field
/// invariants.
///
/// No serde derives — domain layer stays persistence-agnostic (CN-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewApprovalVerdict {
    /// All scopes are `NotRequired(*)` — no scope requires review.
    ///
    /// `check-approved` exits `[OK]`.
    Approved,

    /// All `Required` scopes are `Required(NotStarted)` and `review.json` is
    /// absent — PR-based workflow bypass; no local review has been recorded.
    ///
    /// `check-approved` emits `[WARN]` then exits successfully.
    ApprovedWithBypass {
        /// Number of `Required(NotStarted)` scopes for the WARN message.
        not_started_count: usize,
    },

    /// One or more scopes still require review and the bypass condition does
    /// not apply.
    ///
    /// `check-approved` emits `[BLOCKED]` and exits with failure.
    Blocked {
        /// Scopes that still require review; non-empty.
        required_scopes: Vec<ScopeName>,
    },
}

// ── Verdict JSON extraction ───────────────────────────────────────────────────

/// Scans text content for a JSON verdict block. Pure function (no file I/O).
///
/// Scans content bottom-up for single-line compact JSON candidates
/// containing `"verdict"` and `"findings"` keys.
#[must_use]
pub fn extract_verdict_json_candidates_compact(content: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    for line in content.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with('{')
            && trimmed.contains("\"verdict\"")
            && trimmed.contains("\"findings\"")
        {
            candidates.push(trimmed.to_owned());
        }
    }
    candidates
}

/// Scans content bottom-up for multi-line pretty-printed JSON candidates
/// containing `"verdict"` and `"findings"` keys.
#[must_use]
pub fn extract_verdict_json_candidates_multiline(content: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let bytes = content.as_bytes();
    let mut end = bytes.len();
    while let Some(close) = content.get(..end).and_then(|s| s.rfind('}')) {
        let mut depth = 0i32;
        let mut start = None;
        for (i, &b) in bytes.get(..=close).iter().flat_map(|s| s.iter().enumerate().rev()) {
            match b {
                b'}' => depth += 1,
                b'{' => {
                    depth -= 1;
                    if depth == 0 {
                        start = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(start) = start {
            if let Some(block) = content.get(start..=close) {
                if block.contains("\"verdict\"") && block.contains("\"findings\"") {
                    candidates.push(block.to_owned());
                }
            }
        }
        end = close;
    }
    candidates
}
