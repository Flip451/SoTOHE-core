//! Verification result types for `sotp verify` subcommands.

use std::fmt;

/// Severity level for a verification finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational note — does not fail verification.
    Info,
    /// Warning — noteworthy but does not fail verification.
    Warning,
    /// Error — causes verification to fail.
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A single verification finding.
#[derive(Debug, Clone)]
pub struct VerifyFinding {
    severity: Severity,
    message: String,
}

impl VerifyFinding {
    /// Creates a new finding.
    pub fn new(severity: Severity, message: impl Into<String>) -> Self {
        Self { severity, message: message.into() }
    }

    /// Creates an error-level finding.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(Severity::Error, message)
    }

    /// Creates a warning-level finding.
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, message)
    }

    /// Returns the severity level.
    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// Returns the message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for VerifyFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.severity, self.message)
    }
}

/// Outcome of a verification check.
#[derive(Debug, Clone)]
pub struct VerifyOutcome {
    findings: Vec<VerifyFinding>,
}

impl VerifyOutcome {
    /// Creates an outcome with no findings (pass).
    pub fn pass() -> Self {
        Self { findings: Vec::new() }
    }

    /// Creates an outcome from a list of findings.
    pub fn from_findings(findings: Vec<VerifyFinding>) -> Self {
        Self { findings }
    }

    /// Returns `true` if the outcome has no error-level findings.
    pub fn is_ok(&self) -> bool {
        !self.findings.iter().any(|f| f.severity() == Severity::Error)
    }

    /// Returns `true` if the outcome has at least one error-level finding.
    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.severity() == Severity::Error)
    }

    /// Returns all findings.
    pub fn findings(&self) -> &[VerifyFinding] {
        &self.findings
    }

    /// Adds a finding to the outcome.
    pub fn add(&mut self, finding: VerifyFinding) {
        self.findings.push(finding);
    }

    /// Merges another outcome into this one.
    pub fn merge(&mut self, other: Self) {
        self.findings.extend(other.findings);
    }

    /// Returns the number of error-level findings.
    pub fn error_count(&self) -> usize {
        self.findings.iter().filter(|f| f.severity() == Severity::Error).count()
    }
}

impl fmt::Display for VerifyOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.findings.is_empty() {
            write!(f, "All checks passed.")
        } else {
            for finding in &self.findings {
                writeln!(f, "{finding}")?;
            }
            let errors = self.error_count();
            if errors > 0 {
                write!(f, "{errors} error(s) found.")?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_outcome_is_ok() {
        let outcome = VerifyOutcome::pass();
        assert!(outcome.is_ok());
        assert!(!outcome.has_errors());
        assert_eq!(outcome.error_count(), 0);
    }

    #[test]
    fn test_outcome_with_error_is_not_ok() {
        let outcome =
            VerifyOutcome::from_findings(vec![VerifyFinding::error("something is wrong")]);
        assert!(!outcome.is_ok());
        assert!(outcome.has_errors());
        assert_eq!(outcome.error_count(), 1);
    }

    #[test]
    fn test_outcome_with_only_warnings_is_ok() {
        let outcome =
            VerifyOutcome::from_findings(vec![VerifyFinding::warning("might be an issue")]);
        assert!(outcome.is_ok());
        assert!(!outcome.has_errors());
    }

    #[test]
    fn test_merge_combines_findings() {
        let mut a = VerifyOutcome::from_findings(vec![VerifyFinding::error("err1")]);
        let b = VerifyOutcome::from_findings(vec![VerifyFinding::warning("warn1")]);
        a.merge(b);
        assert_eq!(a.findings().len(), 2);
        assert_eq!(a.error_count(), 1);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }

    #[test]
    fn test_finding_display() {
        let f = VerifyFinding::error("bad config");
        assert_eq!(f.to_string(), "[error] bad config");
    }

    #[test]
    fn test_outcome_display_pass() {
        let outcome = VerifyOutcome::pass();
        assert_eq!(outcome.to_string(), "All checks passed.");
    }
}
