//! Freshness-aware evaluation-result document for per-layer TDDD type signals.
//!
//! A document records the declaration, HEAD, and baseline inputs that govern
//! reuse. Cache reuse is valid only for a clean worktree at the recorded HEAD.

use std::fmt;
use std::path::{Component, Path, PathBuf};

use sha2::Digest as _;

use crate::tddd::CargoFeatureName;
use crate::tddd::catalogue::TypeSignal;
use crate::tddd::catalogue_v2::{CrateName, RustdocCratePortError};
use crate::{CommitHash, ContentHash, Timestamp};

/// Schema version for `<layer>-type-signals.json` documents.
pub const TYPE_SIGNALS_SCHEMA_VERSION: u32 = 5;

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
    ///
    /// # Errors
    ///
    /// Returns [`Sha256DigestError::InvalidLength`] when `value` is not
    /// exactly 64 characters, or [`Sha256DigestError::InvalidHex`] when it
    /// contains a non-lowercase-hexadecimal character.
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
            /// Creates an identity from a validated SHA-256 digest.
            #[must_use]
            pub fn new(digest: Sha256Digest) -> Self {
                Self { digest }
            }

            /// Returns the underlying validated SHA-256 digest.
            #[must_use]
            pub fn as_digest(&self) -> &Sha256Digest {
                &self.digest
            }
        }
    };
}

digest_identity!(CatalogueDeclarationHash, "Identity of the normalized catalogue declaration.");
digest_identity!(BaselineHash, "Identity of the actual rustdoc baseline bytes.");
digest_identity!(
    ImplementationFingerprint,
    "Identity of the complete rustdoc implementation inputs."
);
digest_identity!(ResolutionFingerprint, "Identity of the complete rustdoc resolution inputs.");
digest_identity!(RustdocJsonHash, "Identity of the exact captured rustdoc JSON bytes.");

/// A resolved absolute Cargo target directory.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedCargoTargetDirectory {
    path: PathBuf,
}

impl ResolvedCargoTargetDirectory {
    /// Validates and stores an absolute Cargo target directory.
    ///
    /// # Errors
    ///
    /// Returns [`RustdocExecutionIdentityError::TargetDirectoryNotAbsolute`]
    /// when `path` is relative.
    pub fn try_new(path: PathBuf) -> Result<Self, RustdocExecutionIdentityError> {
        if path.is_absolute() {
            Ok(Self { path })
        } else {
            Err(RustdocExecutionIdentityError::TargetDirectoryNotAbsolute)
        }
    }

    /// Returns the resolved target directory path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

/// A validated Cargo profile name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CargoProfileName {
    value: String,
}

impl CargoProfileName {
    /// Validates and stores a non-empty profile name.
    ///
    /// # Errors
    ///
    /// Returns [`RustdocExecutionIdentityError::EmptyProfile`] when `value`
    /// is empty or contains only whitespace.
    pub fn try_new(value: String) -> Result<Self, RustdocExecutionIdentityError> {
        if value.trim().is_empty() {
            Err(RustdocExecutionIdentityError::EmptyProfile)
        } else {
            Ok(Self { value })
        }
    }

    /// Returns the profile name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// A rustdoc JSON path confined to a resolved Cargo target directory.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExpectedRustdocJsonPath {
    path: PathBuf,
}

impl ExpectedRustdocJsonPath {
    /// Validates an absolute JSON path against its target directory.
    ///
    /// Parent-directory components are rejected because this domain value
    /// does not perform filesystem canonicalization.
    ///
    /// # Errors
    ///
    /// Returns [`RustdocExecutionIdentityError::ExpectedJsonOutsideTargetDirectory`]
    /// when `path` is relative, contains parent traversal, or is not beneath
    /// `target_directory`.
    pub fn try_new(
        path: PathBuf,
        target_directory: &ResolvedCargoTargetDirectory,
    ) -> Result<Self, RustdocExecutionIdentityError> {
        let contains_parent_traversal =
            path.components().any(|component| component == Component::ParentDir);
        if path.is_absolute()
            && !contains_parent_traversal
            && path.starts_with(target_directory.as_path())
        {
            Ok(Self { path })
        } else {
            Err(RustdocExecutionIdentityError::ExpectedJsonOutsideTargetDirectory)
        }
    }

    /// Returns the expected JSON path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

/// Failure to construct one component of a rustdoc execution identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustdocExecutionIdentityError {
    /// The target directory is not absolute.
    TargetDirectoryNotAbsolute,
    /// The expected JSON path is not beneath the target directory.
    ExpectedJsonOutsideTargetDirectory,
    /// The Cargo profile is empty.
    EmptyProfile,
}

impl fmt::Display for RustdocExecutionIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetDirectoryNotAbsolute => {
                formatter.write_str("Cargo target directory must be absolute")
            }
            Self::ExpectedJsonOutsideTargetDirectory => formatter
                .write_str("expected rustdoc JSON path is outside the Cargo target directory"),
            Self::EmptyProfile => formatter.write_str("Cargo profile must not be empty"),
        }
    }
}

impl std::error::Error for RustdocExecutionIdentityError {}

/// The exact Cargo/rustdoc selection used for one current export.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RustdocExecutionIdentity {
    target_directory: ResolvedCargoTargetDirectory,
    crate_name: CrateName,
    features: Vec<CargoFeatureName>,
    profile: CargoProfileName,
    expected_json_path: ExpectedRustdocJsonPath,
}

impl RustdocExecutionIdentity {
    /// Creates a value-equatable execution selection from validated components.
    ///
    /// The expected JSON path is revalidated against `target_directory` so the
    /// identity cannot combine a target directory with a path validated for a
    /// different target directory.
    ///
    /// # Errors
    ///
    /// Returns [`RustdocExecutionIdentityError::ExpectedJsonOutsideTargetDirectory`]
    /// when `expected_json_path` is not confined to `target_directory`.
    #[must_use = "the execution identity is required for capture and cache operations"]
    pub fn new(
        target_directory: ResolvedCargoTargetDirectory,
        crate_name: CrateName,
        features: Vec<CargoFeatureName>,
        profile: CargoProfileName,
        expected_json_path: ExpectedRustdocJsonPath,
    ) -> Result<Self, RustdocExecutionIdentityError> {
        let expected_json_path = ExpectedRustdocJsonPath::try_new(
            expected_json_path.as_path().to_path_buf(),
            &target_directory,
        )?;
        Ok(Self { target_directory, crate_name, features, profile, expected_json_path })
    }

    /// Returns the resolved Cargo target directory used for the export.
    #[must_use]
    pub fn target_directory(&self) -> &ResolvedCargoTargetDirectory {
        &self.target_directory
    }

    /// Returns the crate selected for the export.
    #[must_use]
    pub fn crate_name(&self) -> &CrateName {
        &self.crate_name
    }

    /// Returns the Cargo features selected for the export.
    #[must_use]
    pub fn features(&self) -> &[CargoFeatureName] {
        &self.features
    }

    /// Returns the Cargo profile selected for the export.
    #[must_use]
    pub fn profile(&self) -> &CargoProfileName {
        &self.profile
    }

    /// Returns the expected rustdoc JSON path for the export.
    #[must_use]
    pub fn expected_json_path(&self) -> &ExpectedRustdocJsonPath {
        &self.expected_json_path
    }
}

/// A typed rustdoc graph paired with the hash of the exact bytes decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedRustdocJson {
    json_hash: RustdocJsonHash,
    crate_data: rustdoc_types::Crate,
}

impl CapturedRustdocJson {
    /// Returns the hash of the exact bytes used to decode the rustdoc graph.
    #[must_use]
    pub fn json_hash(&self) -> &RustdocJsonHash {
        &self.json_hash
    }

    /// Returns the decoded rustdoc graph.
    #[must_use]
    pub fn crate_data(&self) -> &rustdoc_types::Crate {
        &self.crate_data
    }
}

/// An identity-bearing, immutable current rustdoc capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustdocSnapshot {
    execution_identity: RustdocExecutionIdentity,
    captured: CapturedRustdocJson,
}

impl RustdocSnapshot {
    /// Returns the execution identity associated with this capture.
    #[must_use]
    pub fn execution_identity(&self) -> &RustdocExecutionIdentity {
        &self.execution_identity
    }

    /// Returns the hash of the exact bytes used to decode this capture.
    #[must_use]
    pub fn json_hash(&self) -> &RustdocJsonHash {
        self.captured.json_hash()
    }

    /// Returns the decoded rustdoc graph in this capture.
    #[must_use]
    pub fn crate_data(&self) -> &rustdoc_types::Crate {
        self.captured.crate_data()
    }
}

/// Constructs a content-addressed rustdoc value from one immutable byte slice.
///
/// # Errors
///
/// Returns the error produced by `decode` when the bytes are not a valid
/// rustdoc graph.
pub fn construct_captured_rustdoc_json(
    bytes: &[u8],
    decode: fn(&[u8]) -> Result<rustdoc_types::Crate, RustdocCratePortError>,
) -> Result<CapturedRustdocJson, RustdocCratePortError> {
    let crate_data = decode(bytes)?;
    let digest: [u8; 32] = sha2::Sha256::digest(bytes).into();
    Ok(CapturedRustdocJson {
        json_hash: RustdocJsonHash::new(Sha256Digest::from_content_hash(ContentHash::from_bytes(
            digest,
        ))),
        crate_data,
    })
}

/// Constructs one identity-bearing snapshot from the same bytes used to decode it.
///
/// # Errors
///
/// Returns the error produced by `decode` when the bytes are not a valid
/// rustdoc graph.
pub fn construct_rustdoc_snapshot(
    identity: RustdocExecutionIdentity,
    bytes: &[u8],
    decode: fn(&[u8]) -> Result<rustdoc_types::Crate, RustdocCratePortError>,
) -> Result<RustdocSnapshot, RustdocCratePortError> {
    Ok(RustdocSnapshot {
        execution_identity: identity,
        captured: construct_captured_rustdoc_json(bytes, decode)?,
    })
}

/// Complete identity of the inputs that govern a type-signals cache entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeSignalsCacheKey {
    declaration_hash: CatalogueDeclarationHash,
    head_commit: CommitHash,
    baseline_hash: BaselineHash,
    implementation_fingerprint: ImplementationFingerprint,
    resolution_fingerprint: ResolutionFingerprint,
    rustdoc_execution_identity: RustdocExecutionIdentity,
}

impl TypeSignalsCacheKey {
    /// Creates an identity from all authoritative type-signals inputs.
    #[must_use]
    pub fn new(
        declaration_hash: CatalogueDeclarationHash,
        head_commit: CommitHash,
        baseline_hash: BaselineHash,
        implementation_fingerprint: ImplementationFingerprint,
        resolution_fingerprint: ResolutionFingerprint,
        rustdoc_execution_identity: RustdocExecutionIdentity,
    ) -> Self {
        Self {
            declaration_hash,
            head_commit,
            baseline_hash,
            implementation_fingerprint,
            resolution_fingerprint,
            rustdoc_execution_identity,
        }
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

    /// Returns the rustdoc implementation-input fingerprint.
    #[must_use]
    pub fn implementation_fingerprint(&self) -> &ImplementationFingerprint {
        &self.implementation_fingerprint
    }

    /// Returns the rustdoc resolution-input fingerprint.
    #[must_use]
    pub fn resolution_fingerprint(&self) -> &ResolutionFingerprint {
        &self.resolution_fingerprint
    }

    /// Returns the rustdoc execution identity.
    #[must_use]
    pub fn rustdoc_execution_identity(&self) -> &RustdocExecutionIdentity {
        &self.rustdoc_execution_identity
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
    ///
    /// # Errors
    ///
    /// Returns [`TypeSignalsSchemaVersionError::Zero`] when `value` is zero.
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
    /// At least one authoritative identity differs or cannot be reused.
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
    if input.current_key == input.recorded_key {
        TypeSignalsReuseDecision::SkipEvaluation
    } else {
        TypeSignalsReuseDecision::ReextractAndEvaluate
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ConfidenceSignal;
    use crate::tddd::catalogue_linter::FreeText;
    use crate::tddd::signal_evaluator::ThreeWaySignalIdentity;

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

    fn execution_identity() -> RustdocExecutionIdentity {
        execution_identity_with_profile("dev")
    }

    fn execution_identity_with_profile(profile: &str) -> RustdocExecutionIdentity {
        let target =
            ResolvedCargoTargetDirectory::try_new(PathBuf::from("/tmp/sotohe-type-signals-target"))
                .unwrap();
        let expected =
            ExpectedRustdocJsonPath::try_new(target.as_path().join("doc/domain.json"), &target)
                .unwrap();
        RustdocExecutionIdentity::new(
            target,
            CrateName::new("domain").unwrap(),
            vec![],
            CargoProfileName::try_new(profile.to_owned()).unwrap(),
            expected,
        )
        .unwrap()
    }
    fn cache_key(
        declaration_value: &str,
        head_value: char,
        baseline_value: &str,
    ) -> TypeSignalsCacheKey {
        cache_key_with_rustdoc_inputs(
            declaration_value,
            head_value,
            baseline_value,
            ImplementationFingerprint::new(digest(A)),
            ResolutionFingerprint::new(digest(B)),
            execution_identity(),
        )
    }

    fn cache_key_with_rustdoc_inputs(
        declaration_value: &str,
        head_value: char,
        baseline_value: &str,
        implementation_fingerprint: ImplementationFingerprint,
        resolution_fingerprint: ResolutionFingerprint,
        rustdoc_execution_identity: RustdocExecutionIdentity,
    ) -> TypeSignalsCacheKey {
        TypeSignalsCacheKey::new(
            declaration(declaration_value),
            head(head_value),
            baseline(baseline_value),
            implementation_fingerprint,
            resolution_fingerprint,
            rustdoc_execution_identity,
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

    fn empty_rustdoc() -> rustdoc_types::Crate {
        rustdoc_types::Crate {
            root: rustdoc_types::Id(0),
            crate_version: None,
            includes_private: false,
            index: std::collections::HashMap::new(),
            paths: std::collections::HashMap::new(),
            external_crates: std::collections::HashMap::new(),
            format_version: rustdoc_types::FORMAT_VERSION,
            target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
        }
    }

    fn decode_rustdoc(bytes: &[u8]) -> Result<rustdoc_types::Crate, RustdocCratePortError> {
        serde_json::from_slice(bytes).map_err(|error| RustdocCratePortError::ParseFailed {
            crate_name: "test".to_owned(),
            reason: error.to_string(),
        })
    }

    #[test]
    fn test_expected_rustdoc_json_path_rejects_parent_traversal() {
        let target =
            ResolvedCargoTargetDirectory::try_new(PathBuf::from("/tmp/sotohe-type-signals-target"))
                .unwrap();
        let escaped_path = target.as_path().join("../outside/current.json");

        assert_eq!(
            ExpectedRustdocJsonPath::try_new(escaped_path, &target),
            Err(RustdocExecutionIdentityError::ExpectedJsonOutsideTargetDirectory)
        );
    }

    #[test]
    fn test_rustdoc_execution_identity_rejects_expected_path_from_another_target() {
        let target =
            ResolvedCargoTargetDirectory::try_new(PathBuf::from("/tmp/sotohe-target-a")).unwrap();
        let other_target =
            ResolvedCargoTargetDirectory::try_new(PathBuf::from("/tmp/sotohe-target-b")).unwrap();
        let expected = ExpectedRustdocJsonPath::try_new(
            other_target.as_path().join("doc/domain.json"),
            &other_target,
        )
        .unwrap();

        assert_eq!(
            RustdocExecutionIdentity::new(
                target,
                CrateName::new("domain").unwrap(),
                vec![],
                CargoProfileName::try_new("dev".to_owned()).unwrap(),
                expected,
            ),
            Err(RustdocExecutionIdentityError::ExpectedJsonOutsideTargetDirectory)
        );
    }

    #[test]
    fn test_construct_captured_rustdoc_json_hashes_and_decodes_one_byte_snapshot() {
        let crate_data = empty_rustdoc();
        let bytes = serde_json::to_vec(&crate_data).unwrap();
        let captured = construct_captured_rustdoc_json(&bytes, decode_rustdoc).unwrap();
        let expected: [u8; 32] = sha2::Sha256::digest(&bytes).into();

        assert_eq!(captured.crate_data(), &crate_data);
        assert_eq!(
            captured.json_hash().as_digest().as_str(),
            Sha256Digest::from_content_hash(ContentHash::from_bytes(expected)).as_str()
        );
    }

    #[test]
    fn test_construct_rustdoc_snapshot_binds_identity_and_decoded_bytes() {
        let identity = execution_identity();
        let crate_data = empty_rustdoc();
        let bytes = serde_json::to_vec(&crate_data).unwrap();
        let snapshot =
            construct_rustdoc_snapshot(identity.clone(), &bytes, decode_rustdoc).unwrap();

        assert_eq!(snapshot.execution_identity(), &identity);
        assert_eq!(snapshot.crate_data(), &crate_data);
        assert_eq!(snapshot.json_hash(), snapshot.captured.json_hash());
    }

    #[test]
    fn test_document_retains_only_required_freshness_inputs() {
        let document = TypeSignalsDocument::new(
            timestamp(),
            cache_key(A, 'b', A),
            vec![TypeSignal::new(
                ThreeWaySignalIdentity::Label { label: FreeText::new("Example") },
                "value_object".to_owned(),
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
        let recorded_key = cache_key(A, 'a', A);

        assert_eq!(
            decide_type_signals_reuse(&verified_input(recorded_key.clone(), cache_key(B, 'a', A),)),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
        assert_eq!(
            decide_type_signals_reuse(&verified_input(recorded_key.clone(), cache_key(A, 'b', A),)),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
        assert_eq!(
            decide_type_signals_reuse(&verified_input(recorded_key.clone(), cache_key(A, 'a', B))),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
        // Keep every other cache-key component equal to the recorded key: an
        // implementation-input change alone invalidates reuse.
        assert_eq!(
            decide_type_signals_reuse(&verified_input(
                recorded_key.clone(),
                cache_key_with_rustdoc_inputs(
                    A,
                    'a',
                    A,
                    ImplementationFingerprint::new(digest(B)),
                    ResolutionFingerprint::new(digest(B)),
                    execution_identity(),
                ),
            )),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
        // A resolution-input change is independently sufficient to invalidate
        // an otherwise matching cache entry.
        assert_eq!(
            decide_type_signals_reuse(&verified_input(
                recorded_key.clone(),
                cache_key_with_rustdoc_inputs(
                    A,
                    'a',
                    A,
                    ImplementationFingerprint::new(digest(A)),
                    ResolutionFingerprint::new(digest(A)),
                    execution_identity(),
                ),
            )),
            TypeSignalsReuseDecision::ReextractAndEvaluate
        );
        // Changing the rustdoc execution selection alone must also re-extract.
        assert_eq!(
            decide_type_signals_reuse(&verified_input(
                recorded_key,
                cache_key_with_rustdoc_inputs(
                    A,
                    'a',
                    A,
                    ImplementationFingerprint::new(digest(A)),
                    ResolutionFingerprint::new(digest(B)),
                    execution_identity_with_profile("release"),
                ),
            )),
            TypeSignalsReuseDecision::ReextractAndEvaluate
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
