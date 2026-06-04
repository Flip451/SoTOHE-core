//! Value object types for the dry-check domain:
//! `FragmentContentHash`, `RefactorProposal`, and `Rationale`.

use std::fmt;

use thiserror::Error;

// ── FragmentContentHash ───────────────────────────────────────────────────────

/// Validated SHA-256 content hash of a code fragment.
///
/// Format: 64 lowercase hex chars. Part of [`FragmentRef`](crate::dry_check::FragmentRef) — the pair
/// (`FilePath`, `FragmentContentHash`) is the fragment identifier (D8/D9/CN-07).
/// When content changes, `content_hash` changes, so the `FragmentRef` changes,
/// so the `DryCheckPairKey` changes — invalidation is implicit in the identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FragmentContentHash(String);

impl FragmentContentHash {
    /// Construct a [`FragmentContentHash`] from a string.
    ///
    /// # Errors
    ///
    /// Returns [`FragmentContentHashError::InvalidFormat`] when `s` is not a
    /// 64-character lowercase hexadecimal string.
    pub fn new(s: impl Into<String>) -> Result<Self, FragmentContentHashError> {
        let s = s.into();
        if s.len() != 64 || !s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
            return Err(FragmentContentHashError::InvalidFormat(s));
        }
        Ok(Self(s))
    }

    /// Return the underlying hash string (always 64 lowercase hex chars).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FragmentContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Error from [`FragmentContentHash::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FragmentContentHashError {
    /// Input is not a valid 64-char lowercase hex SHA-256 string.
    #[error("fragment content hash must be 64 lowercase hex chars: {0}")]
    InvalidFormat(String),
}

// ── RefactorProposal ──────────────────────────────────────────────────────────

/// Validated non-empty refactor proposal text produced by the dry-checker agent.
///
/// The empty-proposal state is structurally unrepresentable: `DryCheckVerdict::Violation`
/// and `DryCheckFinding::refactor_proposal` both use this type, so an empty
/// proposal cannot exist in a valid violation record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefactorProposal(String);

impl RefactorProposal {
    /// Construct a [`RefactorProposal`].
    ///
    /// # Errors
    ///
    /// Returns [`RefactorProposalError::Empty`] when `s` is empty.
    pub fn new(s: impl Into<String>) -> Result<Self, RefactorProposalError> {
        let s = s.into();
        if s.is_empty() {
            return Err(RefactorProposalError::Empty);
        }
        Ok(Self(s))
    }

    /// Return the underlying proposal text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RefactorProposal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Error from [`RefactorProposal::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RefactorProposalError {
    /// Input string is empty.
    #[error("refactor proposal must not be empty")]
    Empty,
}

// ── Rationale ─────────────────────────────────────────────────────────────────

/// Validated non-empty rationale text (agent judgment reason).
///
/// Required on all records: the D9 schema mandates a non-null judgment reason
/// for every verdict (violation, not-a-violation, accepted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rationale(String);

impl Rationale {
    /// Construct a [`Rationale`].
    ///
    /// # Errors
    ///
    /// Returns [`RationaleError::Empty`] when `s` is empty.
    pub fn new(s: impl Into<String>) -> Result<Self, RationaleError> {
        let s = s.into();
        if s.is_empty() {
            return Err(RationaleError::Empty);
        }
        Ok(Self(s))
    }

    /// Return the underlying rationale text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Rationale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Error from [`Rationale::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RationaleError {
    /// Input string is empty.
    #[error("rationale must not be empty")]
    Empty,
}
