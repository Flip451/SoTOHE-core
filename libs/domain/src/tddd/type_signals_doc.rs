//! Freshness-aware evaluation-result document for per-layer TDDD type signals.
//!
//! The document records every input that can affect an evaluation.  Reuse is a
//! conservative optimisation: any unavailable or unequal input selects a path
//! that recomputes rather than risking stale signals.

use std::fmt;

use crate::tddd::catalogue::TypeSignal;
use crate::{ContentHash, Timestamp};

/// Schema version for freshness-aware `<layer>-type-signals.json` documents.
pub const TYPE_SIGNALS_SCHEMA_VERSION: u32 = 2;

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
    "Identity of the resolved implementation build-input closure."
);
digest_identity!(BaselineHash, "Identity of the layer baseline.");
digest_identity!(LiveRustdocSnapshotHash, "Identity of a verified live rustdoc snapshot.");
digest_identity!(EvaluatorContractHash, "Identity of the signal-evaluator contract.");
digest_identity!(RustdocExtractionContractHash, "Identity of the rustdoc-extraction contract.");

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

/// The outcome of directly verifying a live rustdoc snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveRustdocSnapshotStatus {
    /// The snapshot was read, parsed, and its hash matched the recorded value.
    Verified(LiveRustdocSnapshotHash),
    /// No snapshot was available at the resolved target path.
    Missing,
    /// The snapshot could not be read safely.
    ReadFailed,
    /// The snapshot could not be parsed.
    ParseFailed,
    /// The snapshot contents did not match the recorded hash.
    HashMismatch,
}

/// The five current inputs other than the live snapshot itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSignalsCurrentInputs {
    declaration_hash: CatalogueDeclarationHash,
    implementation_input_hash: ImplementationInputHash,
    baseline_hash: BaselineHash,
    evaluator_contract_hash: EvaluatorContractHash,
    rustdoc_extraction_contract_hash: RustdocExtractionContractHash,
}

impl TypeSignalsCurrentInputs {
    #[must_use]
    pub fn new(
        declaration_hash: CatalogueDeclarationHash,
        implementation_input_hash: ImplementationInputHash,
        baseline_hash: BaselineHash,
        evaluator_contract_hash: EvaluatorContractHash,
        rustdoc_extraction_contract_hash: RustdocExtractionContractHash,
    ) -> Self {
        Self {
            declaration_hash,
            implementation_input_hash,
            baseline_hash,
            evaluator_contract_hash,
            rustdoc_extraction_contract_hash,
        }
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
    pub fn baseline_hash(&self) -> &BaselineHash {
        &self.baseline_hash
    }
    #[must_use]
    pub fn evaluator_contract_hash(&self) -> &EvaluatorContractHash {
        &self.evaluator_contract_hash
    }
    #[must_use]
    pub fn rustdoc_extraction_contract_hash(&self) -> &RustdocExtractionContractHash {
        &self.rustdoc_extraction_contract_hash
    }
}

/// All identities recorded when type signals were evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSignalsFreshness {
    declaration_hash: CatalogueDeclarationHash,
    implementation_input_hash: ImplementationInputHash,
    baseline_hash: BaselineHash,
    live_rustdoc_snapshot_hash: LiveRustdocSnapshotHash,
    evaluator_contract_hash: EvaluatorContractHash,
    rustdoc_extraction_contract_hash: RustdocExtractionContractHash,
}

impl TypeSignalsFreshness {
    #[must_use]
    pub fn new(
        declaration_hash: CatalogueDeclarationHash,
        implementation_input_hash: ImplementationInputHash,
        baseline_hash: BaselineHash,
        live_rustdoc_snapshot_hash: LiveRustdocSnapshotHash,
        evaluator_contract_hash: EvaluatorContractHash,
        rustdoc_extraction_contract_hash: RustdocExtractionContractHash,
    ) -> Self {
        Self {
            declaration_hash,
            implementation_input_hash,
            baseline_hash,
            live_rustdoc_snapshot_hash,
            evaluator_contract_hash,
            rustdoc_extraction_contract_hash,
        }
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
    pub fn baseline_hash(&self) -> &BaselineHash {
        &self.baseline_hash
    }
    #[must_use]
    pub fn live_rustdoc_snapshot_hash(&self) -> &LiveRustdocSnapshotHash {
        &self.live_rustdoc_snapshot_hash
    }
    #[must_use]
    pub fn evaluator_contract_hash(&self) -> &EvaluatorContractHash {
        &self.evaluator_contract_hash
    }
    #[must_use]
    pub fn rustdoc_extraction_contract_hash(&self) -> &RustdocExtractionContractHash {
        &self.rustdoc_extraction_contract_hash
    }
}

/// In-memory representation of a freshness-aware `<layer>-type-signals.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSignalsDocument {
    schema_version: TypeSignalsSchemaVersion,
    generated_at: Timestamp,
    freshness: TypeSignalsFreshness,
    signals: Vec<TypeSignal>,
}

impl TypeSignalsDocument {
    /// Creates a document at the current schema version.
    #[must_use]
    pub fn new(
        generated_at: Timestamp,
        freshness: TypeSignalsFreshness,
        signals: Vec<TypeSignal>,
    ) -> Self {
        Self {
            schema_version: TypeSignalsSchemaVersion { value: TYPE_SIGNALS_SCHEMA_VERSION },
            generated_at,
            freshness,
            signals,
        }
    }

    /// Creates a document with the decoded schema version.
    #[must_use]
    pub fn with_schema_version(
        schema_version: TypeSignalsSchemaVersion,
        generated_at: Timestamp,
        freshness: TypeSignalsFreshness,
        signals: Vec<TypeSignal>,
    ) -> Self {
        Self { schema_version, generated_at, freshness, signals }
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
    pub fn freshness(&self) -> &TypeSignalsFreshness {
        &self.freshness
    }
    #[must_use]
    pub fn declaration_hash(&self) -> &CatalogueDeclarationHash {
        self.freshness.declaration_hash()
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
#[derive(Debug, PartialEq, Eq)]
pub enum TypeSignalsReuseDecision {
    /// Every evaluation input and the verified snapshot match; skip evaluation.
    SkipEvaluation,
    /// A semantic input changed but the verified snapshot remains reusable.
    ReevaluateWithSnapshot,
    /// The snapshot cannot be trusted; extract it and then evaluate.
    ReextractAndEvaluate,
}

impl Copy for TypeSignalsReuseDecision {}

impl Clone for TypeSignalsReuseDecision {
    fn clone(&self) -> Self {
        *self
    }
}

/// Selects the fail-closed reuse path for one layer.
#[must_use]
pub fn decide_type_signals_reuse(
    recorded: &TypeSignalsFreshness,
    current: &TypeSignalsCurrentInputs,
    snapshot_status: LiveRustdocSnapshotStatus,
) -> TypeSignalsReuseDecision {
    let snapshot_matches = matches!(
        snapshot_status,
        LiveRustdocSnapshotStatus::Verified(ref hash) if hash == recorded.live_rustdoc_snapshot_hash()
    );
    if current.implementation_input_hash() != recorded.implementation_input_hash()
        || current.rustdoc_extraction_contract_hash() != recorded.rustdoc_extraction_contract_hash()
        || !snapshot_matches
    {
        return TypeSignalsReuseDecision::ReextractAndEvaluate;
    }
    if current.declaration_hash() == recorded.declaration_hash()
        && current.implementation_input_hash() == recorded.implementation_input_hash()
        && current.baseline_hash() == recorded.baseline_hash()
        && current.evaluator_contract_hash() == recorded.evaluator_contract_hash()
    {
        TypeSignalsReuseDecision::SkipEvaluation
    } else {
        TypeSignalsReuseDecision::ReevaluateWithSnapshot
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
    fn freshness() -> TypeSignalsFreshness {
        TypeSignalsFreshness::new(
            CatalogueDeclarationHash::new(digest(A)),
            ImplementationInputHash::new(digest(A)),
            BaselineHash::new(digest(A)),
            LiveRustdocSnapshotHash::new(digest(A)),
            EvaluatorContractHash::new(digest(A)),
            RustdocExtractionContractHash::new(digest(A)),
        )
    }
    fn current() -> TypeSignalsCurrentInputs {
        TypeSignalsCurrentInputs::new(
            CatalogueDeclarationHash::new(digest(A)),
            ImplementationInputHash::new(digest(A)),
            BaselineHash::new(digest(A)),
            EvaluatorContractHash::new(digest(A)),
            RustdocExtractionContractHash::new(digest(A)),
        )
    }
    fn verified() -> LiveRustdocSnapshotStatus {
        LiveRustdocSnapshotStatus::Verified(LiveRustdocSnapshotHash::new(digest(A)))
    }
    fn signal() -> TypeSignal {
        TypeSignal::new(
            "Example",
            "value_object",
            ConfidenceSignal::Blue,
            true,
            vec![],
            vec![],
            vec![],
        )
    }
    fn timestamp() -> Timestamp {
        Timestamp::new("2026-07-14T00:00:00Z").unwrap()
    }

    #[test]
    fn test_sha256_digest_valid_lowercase_hex_is_retained() {
        assert_eq!(digest(A).as_str(), A);
    }

    #[test]
    fn test_sha256_digest_from_content_hash_returns_canonical_lowercase_hex() {
        let digest = Sha256Digest::from_content_hash(ContentHash::from_bytes([0xbb; 32]));

        assert_eq!(digest.as_str(), B);
    }

    #[test]
    fn test_sha256_digest_short_value_returns_invalid_length() {
        assert_eq!(Sha256Digest::try_new("abc".to_owned()), Err(Sha256DigestError::InvalidLength));
    }

    #[test]
    fn test_sha256_digest_uppercase_value_returns_invalid_hex() {
        assert_eq!(Sha256Digest::try_new(A.to_uppercase()), Err(Sha256DigestError::InvalidHex));
    }

    #[test]
    fn test_schema_version_zero_returns_error() {
        assert_eq!(TypeSignalsSchemaVersion::try_new(0), Err(TypeSignalsSchemaVersionError::Zero));
    }

    #[test]
    fn test_freshness_errors_are_copy() {
        fn assert_copy<T: Copy>() {}

        assert_copy::<Sha256DigestError>();
        assert_copy::<TypeSignalsSchemaVersionError>();
    }

    #[test]
    fn test_document_new_pins_current_schema_and_freshness() {
        let document = TypeSignalsDocument::new(timestamp(), freshness(), vec![signal()]);
        assert_eq!(document.schema_version().value(), TYPE_SIGNALS_SCHEMA_VERSION);
        assert_eq!(document.declaration_hash().as_digest().as_str(), A);
        assert_eq!(document.freshness(), &freshness());
        assert_eq!(document.signals(), &[signal()]);
    }

    #[test]
    fn test_load_result_stale_preserves_expected_declaration_hash() {
        let document = TypeSignalsDocument::new(timestamp(), freshness(), vec![]);
        let result =
            TypeSignalsLoadResult::Stale(document, CatalogueDeclarationHash::new(digest(B)));
        assert!(result.is_stale());
        assert!(result.as_current().is_none());
    }

    #[test]
    fn test_decide_type_signals_reuse_matching_five_current_hashes_and_verified_snapshot_skips_evaluation()
     {
        let declaration_hash = CatalogueDeclarationHash::new(digest(A));
        let implementation_input_hash = ImplementationInputHash::new(digest(A));
        let baseline_hash = BaselineHash::new(digest(A));
        let evaluator_contract_hash = EvaluatorContractHash::new(digest(A));
        let rustdoc_extraction_contract_hash = RustdocExtractionContractHash::new(digest(A));
        let snapshot_hash = LiveRustdocSnapshotHash::new(digest(A));
        let recorded = TypeSignalsFreshness::new(
            declaration_hash.clone(),
            implementation_input_hash.clone(),
            baseline_hash.clone(),
            snapshot_hash.clone(),
            evaluator_contract_hash.clone(),
            rustdoc_extraction_contract_hash.clone(),
        );
        let current = TypeSignalsCurrentInputs::new(
            declaration_hash,
            implementation_input_hash,
            baseline_hash,
            evaluator_contract_hash,
            rustdoc_extraction_contract_hash,
        );
        let snapshot_status = LiveRustdocSnapshotStatus::Verified(snapshot_hash);

        assert_eq!(recorded.declaration_hash(), current.declaration_hash());
        assert_eq!(recorded.implementation_input_hash(), current.implementation_input_hash());
        assert_eq!(recorded.baseline_hash(), current.baseline_hash());
        assert_eq!(recorded.evaluator_contract_hash(), current.evaluator_contract_hash());
        assert_eq!(
            recorded.rustdoc_extraction_contract_hash(),
            current.rustdoc_extraction_contract_hash()
        );
        assert!(matches!(
            &snapshot_status,
            LiveRustdocSnapshotStatus::Verified(hash)
                if hash == recorded.live_rustdoc_snapshot_hash()
        ));
        assert_eq!(
            decide_type_signals_reuse(&recorded, &current, snapshot_status.clone()),
            TypeSignalsReuseDecision::SkipEvaluation
        );

        let mut differing_evaluator_contract = current.clone();
        differing_evaluator_contract.evaluator_contract_hash =
            EvaluatorContractHash::new(digest(B));
        assert_eq!(
            decide_type_signals_reuse(&recorded, &differing_evaluator_contract, snapshot_status),
            TypeSignalsReuseDecision::ReevaluateWithSnapshot
        );
    }

    #[test]
    fn test_decide_type_signals_reuse_catalogue_change_reevaluates_with_snapshot() {
        let mut changed = current();
        changed.declaration_hash = CatalogueDeclarationHash::new(digest(B));
        assert_eq!(
            decide_type_signals_reuse(&freshness(), &changed, verified()),
            TypeSignalsReuseDecision::ReevaluateWithSnapshot
        );
    }

    #[test]
    fn test_decide_type_signals_reuse_evaluator_change_reevaluates_with_snapshot() {
        let mut changed = current();
        changed.evaluator_contract_hash = EvaluatorContractHash::new(digest(B));
        assert_eq!(
            decide_type_signals_reuse(&freshness(), &changed, verified()),
            TypeSignalsReuseDecision::ReevaluateWithSnapshot
        );
    }

    #[test]
    fn test_decide_type_signals_reuse_implementation_input_change_reextracts_and_evaluates() {
        let mut changed = current();
        changed.implementation_input_hash = ImplementationInputHash::new(digest(B));
        assert_eq!(
            decide_type_signals_reuse(&freshness(), &changed, verified()),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
    }

    #[test]
    fn test_decide_type_signals_reuse_baseline_change_reevaluates_with_snapshot() {
        let mut changed = current();
        changed.baseline_hash = BaselineHash::new(digest(B));
        assert_eq!(
            decide_type_signals_reuse(&freshness(), &changed, verified()),
            TypeSignalsReuseDecision::ReevaluateWithSnapshot
        );
    }

    #[test]
    fn test_decide_type_signals_reuse_extraction_contract_change_reextracts_and_evaluates() {
        let mut changed = current();
        changed.rustdoc_extraction_contract_hash = RustdocExtractionContractHash::new(digest(B));
        assert_eq!(
            decide_type_signals_reuse(&freshness(), &changed, verified()),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
    }

    #[test]
    fn test_decide_type_signals_reuse_unverified_snapshot_reextracts_and_evaluates() {
        for status in [
            LiveRustdocSnapshotStatus::Missing,
            LiveRustdocSnapshotStatus::ReadFailed,
            LiveRustdocSnapshotStatus::ParseFailed,
            LiveRustdocSnapshotStatus::HashMismatch,
            LiveRustdocSnapshotStatus::Verified(LiveRustdocSnapshotHash::new(digest(B))),
        ] {
            assert_eq!(
                decide_type_signals_reuse(&freshness(), &current(), status),
                TypeSignalsReuseDecision::ReextractAndEvaluate
            );
        }
    }
}
