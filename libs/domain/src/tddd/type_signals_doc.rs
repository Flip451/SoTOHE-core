//! Freshness-aware evaluation-result document for per-layer TDDD type signals.
//!
//! A document records the declaration and implementation inputs that govern
//! reuse. Any missing implementation identity selects re-extraction.

use std::fmt;

use crate::tddd::catalogue::TypeSignal;
use crate::{ContentHash, Timestamp};

/// Schema version for `<layer>-type-signals.json` documents.
pub const TYPE_SIGNALS_SCHEMA_VERSION: u32 = 3;

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
digest_identity!(
    ImplementationInputHash,
    "Identity of one layer's source contents, lockfile dependency resolution, and toolchain identity."
);

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
    declaration_hash: CatalogueDeclarationHash,
    implementation_input_hash: ImplementationInputHash,
    signals: Vec<TypeSignal>,
}

impl TypeSignalsDocument {
    /// Creates a document at the current schema version.
    #[must_use]
    pub fn new(
        generated_at: Timestamp,
        declaration_hash: CatalogueDeclarationHash,
        implementation_input_hash: ImplementationInputHash,
        signals: Vec<TypeSignal>,
    ) -> Self {
        Self {
            schema_version: TypeSignalsSchemaVersion { value: TYPE_SIGNALS_SCHEMA_VERSION },
            generated_at,
            declaration_hash,
            implementation_input_hash,
            signals,
        }
    }

    /// Creates a document with the decoded schema version.
    #[must_use]
    pub fn with_schema_version(
        schema_version: TypeSignalsSchemaVersion,
        generated_at: Timestamp,
        declaration_hash: CatalogueDeclarationHash,
        implementation_input_hash: ImplementationInputHash,
        signals: Vec<TypeSignal>,
    ) -> Self {
        Self { schema_version, generated_at, declaration_hash, implementation_input_hash, signals }
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
    pub fn declaration_hash(&self) -> &CatalogueDeclarationHash {
        &self.declaration_hash
    }
    #[must_use]
    pub fn implementation_input_hash(&self) -> &ImplementationInputHash {
        &self.implementation_input_hash
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
    /// The declaration changed while the implementation identity still matches.
    ReevaluateWithoutExtraction,
    /// The implementation identity changed or cannot be determined.
    ReextractAndEvaluate,
}

/// Selects the fail-closed reuse path for one layer.
#[must_use]
pub fn decide_type_signals_reuse(
    recorded_declaration_hash: &CatalogueDeclarationHash,
    recorded_implementation_input_hash: &ImplementationInputHash,
    current_declaration_hash: &CatalogueDeclarationHash,
    current_implementation_input_hash: Option<&ImplementationInputHash>,
) -> TypeSignalsReuseDecision {
    let Some(current_implementation_input_hash) = current_implementation_input_hash else {
        return TypeSignalsReuseDecision::ReextractAndEvaluate;
    };
    if current_implementation_input_hash != recorded_implementation_input_hash {
        TypeSignalsReuseDecision::ReextractAndEvaluate
    } else if current_declaration_hash == recorded_declaration_hash {
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
    fn implementation(value: &str) -> ImplementationInputHash {
        ImplementationInputHash::new(digest(value))
    }
    fn timestamp() -> Timestamp {
        Timestamp::new("2026-07-14T00:00:00Z").unwrap()
    }

    #[test]
    fn test_document_retains_only_required_freshness_inputs() {
        let document = TypeSignalsDocument::new(
            timestamp(),
            declaration(A),
            implementation(B),
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
        assert_eq!(document.declaration_hash().as_digest().as_str(), A);
        assert_eq!(document.implementation_input_hash().as_digest().as_str(), B);
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
    fn test_decide_type_signals_reuse_matching_hashes_skips_evaluation() {
        assert_eq!(
            decide_type_signals_reuse(
                &declaration(A),
                &implementation(A),
                &declaration(A),
                Some(&implementation(A))
            ),
            TypeSignalsReuseDecision::SkipEvaluation
        );
    }

    #[test]
    fn test_decide_type_signals_reuse_catalogue_only_change_reevaluates_without_extraction() {
        assert_eq!(
            decide_type_signals_reuse(
                &declaration(A),
                &implementation(A),
                &declaration(B),
                Some(&implementation(A))
            ),
            TypeSignalsReuseDecision::ReevaluateWithoutExtraction
        );
    }

    #[test]
    fn test_decide_type_signals_reuse_full_freshness_decision_table() {
        // Implementation mismatches always re-extract, regardless of whether
        // the declaration matches. A declaration mismatch alone is deliberately
        // cheaper: it re-evaluates the existing rustdoc output.
        assert_eq!(
            decide_type_signals_reuse(
                &declaration(A),
                &implementation(A),
                &declaration(A),
                Some(&implementation(B))
            ),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
        assert_eq!(
            decide_type_signals_reuse(
                &declaration(A),
                &implementation(A),
                &declaration(B),
                Some(&implementation(B))
            ),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
        assert_eq!(
            decide_type_signals_reuse(&declaration(A), &implementation(A), &declaration(A), None),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
    }
}
