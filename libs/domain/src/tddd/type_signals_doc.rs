//! Freshness-aware evaluation-result document for per-layer TDDD type signals.
//!
//! A document records the declaration, HEAD, and baseline inputs that govern
//! reuse. Cache reuse is valid only for a clean worktree at the recorded HEAD.

use std::fmt;

use crate::tddd::catalogue::TypeSignal;
use crate::{CommitHash, ContentHash, Timestamp};

/// Schema version for `<layer>-type-signals.json` documents.
pub const TYPE_SIGNALS_SCHEMA_VERSION: u32 = 4;

/// A validated lowercase SHA-256 hexadecimal digest.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Sha256Digest {
    value: String,
}

/// Error returned when a SHA-256 digest is malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sha256DigestError {
    /// The digest does not contain exactly 64 characters.
    InvalidLength,
    /// The digest contains a non-lowercase-hexadecimal character.
    InvalidHex,
}

impl fmt::Display for Sha256DigestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength => formatter.write_str("SHA-256 digest must contain 64 characters"),
            Self::InvalidHex => formatter.write_str("SHA-256 digest must be lowercase hexadecimal"),
        }
    }
}

impl std::error::Error for Sha256DigestError {}

impl Sha256Digest {
    /// Converts a computed content hash into its canonical digest representation.
    #[must_use]
    pub fn from_content_hash(content_hash: ContentHash) -> Self {
        Self { value: content_hash.to_hex() }
    }

    /// Validates and stores a lowercase SHA-256 hexadecimal digest.
    pub fn try_new(value: String) -> Result<Self, Sha256DigestError> {
        if value.len() != 64 {
            return Err(Sha256DigestError::InvalidLength);
        }
        if !value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
            return Err(Sha256DigestError::InvalidHex);
        }
        Ok(Self { value })
    }

    /// Returns the validated digest text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

macro_rules! digest_identity {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name {
            digest: Sha256Digest,
        }

        impl $name {
            #[must_use]
            pub fn new(digest: Sha256Digest) -> Self {
                Self { digest }
            }

            #[must_use]
            pub fn as_digest(&self) -> &Sha256Digest {
                &self.digest
            }
        }
    };
}

digest_identity!(CatalogueDeclarationHash, "Identity of the normalized catalogue declaration.");
digest_identity!(BaselineHash, "Identity of the actual rustdoc baseline bytes.");

/// Complete identity of the inputs that govern a type-signals cache entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeSignalsCacheKey {
    declaration_hash: CatalogueDeclarationHash,
    head_commit: CommitHash,
    baseline_hash: BaselineHash,
}

impl TypeSignalsCacheKey {
    /// Creates an identity from all authoritative type-signals inputs.
    #[must_use]
    pub fn new(
        declaration_hash: CatalogueDeclarationHash,
        head_commit: CommitHash,
        baseline_hash: BaselineHash,
    ) -> Self {
        Self { declaration_hash, head_commit, baseline_hash }
    }

    /// Returns the catalogue declaration identity.
    #[must_use]
    pub fn declaration_hash(&self) -> &CatalogueDeclarationHash {
        &self.declaration_hash
    }

    /// Returns the recorded repository HEAD identity.
    #[must_use]
    pub fn head_commit(&self) -> &CommitHash {
        &self.head_commit
    }

    /// Returns the baseline identity.
    #[must_use]
    pub fn baseline_hash(&self) -> &BaselineHash {
        &self.baseline_hash
    }
}

/// A non-zero persisted type-signals schema version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeSignalsSchemaVersion {
    value: u32,
}

/// Error returned for an invalid type-signals schema version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeSignalsSchemaVersionError {
    /// Zero is not a schema version.
    Zero,
}

impl fmt::Display for TypeSignalsSchemaVersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("type-signals schema version must be non-zero")
    }
}

impl std::error::Error for TypeSignalsSchemaVersionError {}

impl TypeSignalsSchemaVersion {
    /// Validates and stores a non-zero schema version.
    pub fn try_new(value: u32) -> Result<Self, TypeSignalsSchemaVersionError> {
        if value == 0 {
            return Err(TypeSignalsSchemaVersionError::Zero);
        }
        Ok(Self { value })
    }

    /// Returns the version number.
    #[must_use]
    pub fn value(self) -> u32 {
        self.value
    }
}

/// In-memory representation of a freshness-aware `<layer>-type-signals.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSignalsDocument {
    schema_version: TypeSignalsSchemaVersion,
    generated_at: Timestamp,
    cache_key: TypeSignalsCacheKey,
    signals: Vec<TypeSignal>,
}

impl TypeSignalsDocument {
    /// Creates a document at the current schema version.
    #[must_use]
    pub fn new(
        generated_at: Timestamp,
        cache_key: TypeSignalsCacheKey,
        signals: Vec<TypeSignal>,
    ) -> Self {
        Self {
            schema_version: TypeSignalsSchemaVersion { value: TYPE_SIGNALS_SCHEMA_VERSION },
            generated_at,
            cache_key,
            signals,
        }
    }

    /// Creates a document with the decoded schema version.
    #[must_use]
    pub fn with_schema_version(
        schema_version: TypeSignalsSchemaVersion,
        generated_at: Timestamp,
        cache_key: TypeSignalsCacheKey,
        signals: Vec<TypeSignal>,
    ) -> Self {
        Self { schema_version, generated_at, cache_key, signals }
    }

    #[must_use]
    pub fn schema_version(&self) -> TypeSignalsSchemaVersion {
        self.schema_version
    }
    #[must_use]
    pub fn generated_at(&self) -> &Timestamp {
        &self.generated_at
    }
    #[must_use]
    pub fn cache_key(&self) -> &TypeSignalsCacheKey {
        &self.cache_key
    }
    #[must_use]
    pub fn signals(&self) -> &[TypeSignal] {
        &self.signals
    }
}

/// Result of loading a persisted type-signals document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeSignalsLoadResult {
    /// The persisted document matches the current catalogue declaration.
    Current(TypeSignalsDocument),
    /// The document exists but has an older declaration identity.
    Stale(TypeSignalsDocument, CatalogueDeclarationHash),
    /// No persisted document exists.
    Missing,
}

impl TypeSignalsLoadResult {
    #[must_use]
    pub fn as_current(&self) -> Option<&TypeSignalsDocument> {
        match self {
            Self::Current(document) => Some(document),
            _ => None,
        }
    }
    #[must_use]
    pub fn is_current(&self) -> bool {
        matches!(self, Self::Current(_))
    }
    #[must_use]
    pub fn is_stale(&self) -> bool {
        matches!(self, Self::Stale(_, _))
    }
    #[must_use]
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

/// The only safe outcomes for an attempted type-signals reuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeSignalsReuseDecision {
    /// Both identities match, so no evaluation is needed.
    SkipEvaluation,
    /// The declaration changed while the HEAD identity still matches.
    ReevaluateWithoutExtraction,
    /// The HEAD identity changed or cannot be determined.
    ReextractAndEvaluate,
}

/// Observation of the worktree used to validate a reuse candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeSignalsWorktreeStatus {
    /// No worktree changes were observed.
    Clean,
    /// Worktree changes were observed.
    Dirty,
}

/// Observation of the cache authority used to validate a reuse candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeSignalsAuthorityStatus {
    /// The cache authority was readable and valid.
    Readable,
    /// The cache authority was unavailable or invalid.
    Unreadable,
}

/// Opaque evidence supplied by the infrastructure evaluator before reuse is decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSignalsReuseInput {
    /// The cache document's recorded input identity.
    recorded_key: TypeSignalsCacheKey,
    /// The currently observed input identity.
    current_key: TypeSignalsCacheKey,
}

impl TypeSignalsReuseInput {
    /// Constructs reuse evidence only after the validation observations pass.
    #[must_use]
    pub fn verify(
        recorded_key: TypeSignalsCacheKey,
        current_key: TypeSignalsCacheKey,
        worktree_status: TypeSignalsWorktreeStatus,
        authority_status: TypeSignalsAuthorityStatus,
    ) -> Option<Self> {
        if worktree_status != TypeSignalsWorktreeStatus::Clean
            || authority_status != TypeSignalsAuthorityStatus::Readable
        {
            return None;
        }
        Some(Self { recorded_key, current_key })
    }
}

/// Selects the fail-closed reuse path for one layer.
#[must_use]
pub fn decide_type_signals_reuse(input: &TypeSignalsReuseInput) -> TypeSignalsReuseDecision {
    if input.current_key.head_commit() != input.recorded_key.head_commit() {
        TypeSignalsReuseDecision::ReextractAndEvaluate
    } else if input.current_key == input.recorded_key {
        TypeSignalsReuseDecision::SkipEvaluation
    } else {
        TypeSignalsReuseDecision::ReevaluateWithoutExtraction
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ConfidenceSignal;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn digest(value: &str) -> Sha256Digest {
        Sha256Digest::try_new(value.to_owned()).unwrap()
    }
    fn declaration(value: &str) -> CatalogueDeclarationHash {
        CatalogueDeclarationHash::new(digest(value))
    }
    fn head(value: char) -> CommitHash {
        CommitHash::try_new(value.to_string().repeat(40)).unwrap()
    }
    fn baseline(value: &str) -> BaselineHash {
        BaselineHash::new(digest(value))
    }
    fn cache_key(
        declaration_value: &str,
        head_value: char,
        baseline_value: &str,
    ) -> TypeSignalsCacheKey {
        TypeSignalsCacheKey::new(
            declaration(declaration_value),
            head(head_value),
            baseline(baseline_value),
        )
    }

    fn verified_input(
        recorded_key: TypeSignalsCacheKey,
        current_key: TypeSignalsCacheKey,
    ) -> TypeSignalsReuseInput {
        TypeSignalsReuseInput::verify(
            recorded_key,
            current_key,
            TypeSignalsWorktreeStatus::Clean,
            TypeSignalsAuthorityStatus::Readable,
        )
        .unwrap()
    }

    fn timestamp() -> Timestamp {
        Timestamp::new("2026-07-14T00:00:00Z").unwrap()
    }

    #[test]
    fn test_document_retains_only_required_freshness_inputs() {
        let document = TypeSignalsDocument::new(
            timestamp(),
            cache_key(A, 'b', A),
            vec![TypeSignal::new(
                "Example",
                "value_object",
                ConfidenceSignal::Blue,
                true,
                vec![],
                vec![],
                vec![],
            )],
        );

        assert_eq!(document.schema_version().value(), TYPE_SIGNALS_SCHEMA_VERSION);
        assert_eq!(document.cache_key().declaration_hash().as_digest().as_str(), A);
        assert_eq!(document.cache_key().head_commit().as_ref(), &B[..40]);
        assert_eq!(document.cache_key().baseline_hash().as_digest().as_str(), A);
    }

    #[test]
    fn test_sha256_digest_try_new_rejects_invalid_length_and_hex() {
        assert_eq!(Sha256Digest::try_new("a".repeat(63)), Err(Sha256DigestError::InvalidLength));
        assert_eq!(Sha256Digest::try_new("g".repeat(64)), Err(Sha256DigestError::InvalidHex));
    }

    #[test]
    fn test_type_signals_schema_version_try_new_rejects_zero() {
        assert_eq!(TypeSignalsSchemaVersion::try_new(0), Err(TypeSignalsSchemaVersionError::Zero));
    }

    #[test]
    fn test_decide_type_signals_reuse_matching_declaration_head_and_baseline_skips_evaluation() {
        assert_eq!(
            decide_type_signals_reuse(&verified_input(cache_key(A, 'a', A), cache_key(A, 'a', A),)),
            TypeSignalsReuseDecision::SkipEvaluation
        );
    }

    #[test]
    fn test_decide_type_signals_reuse_each_isolated_cache_identity_mismatch_reevaluates() {
        assert_eq!(
            decide_type_signals_reuse(&verified_input(cache_key(A, 'a', A), cache_key(B, 'a', A),)),
            TypeSignalsReuseDecision::ReevaluateWithoutExtraction
        );
        assert_eq!(
            decide_type_signals_reuse(&verified_input(cache_key(A, 'a', A), cache_key(A, 'b', A),)),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
        assert_eq!(
            decide_type_signals_reuse(&verified_input(cache_key(A, 'a', A), cache_key(A, 'a', B),)),
            TypeSignalsReuseDecision::ReevaluateWithoutExtraction
        );
    }

    #[test]
    fn test_type_signals_reuse_input_verify_rejects_unverified_evidence() {
        assert_eq!(
            TypeSignalsReuseInput::verify(
                cache_key(A, 'a', A),
                cache_key(A, 'a', A),
                TypeSignalsWorktreeStatus::Dirty,
                TypeSignalsAuthorityStatus::Readable,
            ),
            None
        );
        assert_eq!(
            TypeSignalsReuseInput::verify(
                cache_key(A, 'a', A),
                cache_key(A, 'a', A),
                TypeSignalsWorktreeStatus::Clean,
                TypeSignalsAuthorityStatus::Unreadable,
            ),
            None
        );
    }

    #[test]
    fn test_decide_type_signals_reuse_unverified_observations_reevaluate() {
        let decide_after_verification = |worktree_status, authority_status| {
            TypeSignalsReuseInput::verify(
                cache_key(A, 'a', A),
                cache_key(A, 'a', A),
                worktree_status,
                authority_status,
            )
            .as_ref()
            .map_or(TypeSignalsReuseDecision::ReextractAndEvaluate, decide_type_signals_reuse)
        };

        assert_eq!(
            decide_after_verification(
                TypeSignalsWorktreeStatus::Dirty,
                TypeSignalsAuthorityStatus::Readable
            ),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
        assert_eq!(
            decide_after_verification(
                TypeSignalsWorktreeStatus::Clean,
                TypeSignalsAuthorityStatus::Unreadable
            ),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
    }
}
