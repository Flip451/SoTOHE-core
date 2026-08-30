//! Serde codec for per-layer TDDD evaluation-result files.

use domain::tddd::catalogue_v2::identifiers::CatalogueItemNamespace;
use domain::tddd::signal_evaluator::ThreeWaySignalIdentity;
use domain::tddd::type_signals_doc::{
    BaselineHash, CargoProfileName, CatalogueDeclarationHash, ExpectedRustdocJsonPath,
    ImplementationFingerprint, ResolutionFingerprint, ResolvedCargoTargetDirectory,
    RustdocExecutionIdentity, Sha256Digest, Sha256DigestError, TypeSignalsCacheKey,
    TypeSignalsDocument, TypeSignalsSchemaVersion, TypeSignalsSchemaVersionError,
};
use domain::tddd::{CargoFeatureName, catalogue_v2::CrateName};
use domain::{CommitHash, ContentHash, FreeText, Timestamp, TypeSignal};
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::tddd::catalogue_spec_signals_codec::{
    confidence_signal_to_str, parse_confidence_signal,
};
use crate::tddd::type_signals_evaluator::signal_tags::kind_tag_namespace;

const EXTERNAL_TARGET_IDENTITY_ROOT: &str = "/sotp-external-target";

/// Codec error for per-layer evaluation-result files.
#[derive(Debug, thiserror::Error)]
pub enum TypeSignalsCodecError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error(
        "unsupported schema_version: {0:?}; regenerate type signals with the current sotp build"
    )]
    UnsupportedSchemaVersion(TypeSignalsSchemaVersion),
    #[error("invalid schema_version: {0}")]
    InvalidSchemaVersion(TypeSignalsSchemaVersionError),
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(FreeText),
    #[error("invalid {field} digest: {source}")]
    InvalidDigest { field: FreeText, source: Sha256DigestError },
    #[error("invalid confidence signal value: {0}")]
    InvalidSignal(FreeText),
    #[error("invalid namespace for type-signal kind_tag: {0}")]
    InvalidNamespace(FreeText),
    #[error("invalid rustdoc execution identity: {0}")]
    InvalidExecutionIdentity(FreeText),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TypeSignalsDocDto {
    schema_version: u32,
    generated_at: String,
    declaration_hash: String,
    head_commit: String,
    baseline_hash: String,
    implementation_fingerprint: String,
    resolution_fingerprint: String,
    rustdoc_execution_identity: RustdocExecutionIdentityDto,
    signals: Vec<TypeSignalDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustdocExecutionIdentityDto {
    target_directory: String,
    crate_name: String,
    features: Vec<String>,
    profile: String,
    expected_json_path: String,
}

/// Current-schema reuse-identity fields required to decode a type-signals document.
#[cfg(test)]
pub(crate) fn merge_fixture_reuse_identity(document: &mut serde_json::Value) {
    let Some(object) = document.as_object_mut() else {
        return;
    };
    object.entry("implementation_fingerprint".to_owned()).or_insert_with(json_zero_digest);
    object.entry("resolution_fingerprint".to_owned()).or_insert_with(json_zero_digest);
    object
        .entry("rustdoc_execution_identity".to_owned())
        .or_insert_with(json_fixture_execution_identity);
}

#[cfg(test)]
fn json_zero_digest() -> serde_json::Value {
    serde_json::Value::String("0".repeat(64))
}

#[cfg(test)]
fn json_fixture_execution_identity() -> serde_json::Value {
    serde_json::json!({
        "target_directory": "/tmp/sotohe-fixture-target",
        "crate_name": "domain",
        "features": [],
        "profile": "dev",
        "expected_json_path": "/tmp/sotohe-fixture-target/doc/domain.json",
    })
}

#[cfg(test)]
fn test_execution_identity_dto() -> RustdocExecutionIdentityDto {
    RustdocExecutionIdentityDto {
        target_directory: "/tmp/sotohe-codec-test-target".to_owned(),
        crate_name: "domain".to_owned(),
        features: vec![],
        profile: "dev".to_owned(),
        expected_json_path: "/tmp/sotohe-codec-test-target/domain.json".to_owned(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TypeSignalDto {
    type_name: String,
    /// Catalogue rows carry `type`/`trait`; `free_function` and `unknown` rows
    /// may omit this field or use explicit `null` for a report-only label.
    #[serde(default)]
    namespace: TypeSignalNamespaceField,
    kind_tag: String,
    signal: String,
    found_type: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    found_items: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    missing_items: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    extra_items: Vec<String>,
}

/// Namespace field for one persisted signal.
///
/// `Missing` is an explicit default sentinel because omitted namespaces are
/// valid only for report-label kind tags, while catalogue rows must fail
/// closed. The wire field therefore distinguishes omission from explicit null
/// while the kind-tag validation decides whether either form is permitted.
#[derive(Debug, Clone, Copy, Default)]
enum TypeSignalNamespaceField {
    #[default]
    Missing,
    Catalogue(TypeSignalNamespaceDto),
    Label,
}

impl Serialize for TypeSignalNamespaceField {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Missing => Err(<S::Error as serde::ser::Error>::custom(
                "namespace is required on a type-signal row",
            )),
            Self::Catalogue(namespace) => namespace.serialize(serializer),
            Self::Label => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for TypeSignalNamespaceField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let namespace = Option::<TypeSignalNamespaceDto>::deserialize(deserializer)?;
        Ok(match namespace {
            Some(namespace) => Self::Catalogue(namespace),
            None => Self::Label,
        })
    }
}

/// Serde representation of a catalogue item's namespace.
///
/// The domain namespace intentionally remains serde-free; this infrastructure
/// DTO keeps the wire vocabulary explicit and rejects unknown values during
/// deserialization.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TypeSignalNamespaceDto {
    Type,
    Trait,
}

impl From<CatalogueItemNamespace> for TypeSignalNamespaceDto {
    fn from(namespace: CatalogueItemNamespace) -> Self {
        match namespace {
            CatalogueItemNamespace::Type => Self::Type,
            CatalogueItemNamespace::Trait => Self::Trait,
        }
    }
}

impl From<TypeSignalNamespaceDto> for CatalogueItemNamespace {
    fn from(namespace: TypeSignalNamespaceDto) -> Self {
        match namespace {
            TypeSignalNamespaceDto::Type => Self::Type,
            TypeSignalNamespaceDto::Trait => Self::Trait,
        }
    }
}

/// Decodes a `<layer>-type-signals.json` string.
///
/// # Errors
///
/// Returns an error for malformed JSON, an unsupported schema, invalid hashes,
/// a non-UTC timestamp, or an identity that does not match its kind tag.
pub fn decode(json: &str) -> Result<TypeSignalsDocument, TypeSignalsCodecError> {
    decode_with_identity_root(json, std::path::Path::new("/sotp-workspace"), None)
}

/// Decodes a document for cache comparison using the currently resolved
/// rustdoc identity to rebase a portable external target marker.
///
/// # Errors
///
/// Returns the same errors as [`decode`].
pub(crate) fn decode_with_workspace(
    json: &str,
    workspace_root: &std::path::Path,
) -> Result<TypeSignalsDocument, TypeSignalsCodecError> {
    decode_with_identity_root(json, workspace_root, None)
}

pub(crate) fn decode_with_workspace_for_current(
    json: &str,
    workspace_root: &std::path::Path,
    current_identity: &RustdocExecutionIdentity,
) -> Result<TypeSignalsDocument, TypeSignalsCodecError> {
    decode_with_identity_root(json, workspace_root, Some(current_identity.target_directory()))
}

fn decode_with_identity_root(
    json: &str,
    identity_root: &std::path::Path,
    external_target_directory: Option<&ResolvedCargoTargetDirectory>,
) -> Result<TypeSignalsDocument, TypeSignalsCodecError> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    let schema_version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or(TypeSignalsCodecError::InvalidSchemaVersion(TypeSignalsSchemaVersionError::Zero))?;
    let schema_version = u32::try_from(schema_version).unwrap_or(0);
    let schema_version = TypeSignalsSchemaVersion::try_new(schema_version)
        .map_err(TypeSignalsCodecError::InvalidSchemaVersion)?;
    if schema_version.value() != domain::TYPE_SIGNALS_SCHEMA_VERSION {
        return Err(TypeSignalsCodecError::UnsupportedSchemaVersion(schema_version));
    }
    let dto: TypeSignalsDocDto = serde_json::from_value(value)?;
    let generated_at = Timestamp::new(dto.generated_at.clone()).map_err(|_| {
        TypeSignalsCodecError::InvalidTimestamp(FreeText::new(dto.generated_at.clone()))
    })?;
    if !is_utc_timestamp(&dto.generated_at) {
        return Err(TypeSignalsCodecError::InvalidTimestamp(FreeText::new(dto.generated_at)));
    }
    let cache_key = TypeSignalsCacheKey::new(
        CatalogueDeclarationHash::new(parse_digest("declaration_hash", dto.declaration_hash)?),
        parse_head_commit(dto.head_commit)?,
        BaselineHash::new(parse_digest("baseline_hash", dto.baseline_hash)?),
        ImplementationFingerprint::new(parse_digest(
            "implementation_fingerprint",
            dto.implementation_fingerprint,
        )?),
        ResolutionFingerprint::new(parse_digest(
            "resolution_fingerprint",
            dto.resolution_fingerprint,
        )?),
        execution_identity_from_dto(
            dto.rustdoc_execution_identity,
            identity_root,
            external_target_directory,
        )?,
    );
    Ok(TypeSignalsDocument::with_schema_version(
        schema_version,
        generated_at,
        cache_key,
        dto.signals.into_iter().map(signal_from_dto).collect::<Result<Vec<_>, _>>()?,
    ))
}

/// Encodes a `TypeSignalsDocument` into pretty-printed JSON.
///
/// # Errors
///
/// Returns an error when the document does not use the current schema.
pub fn encode(doc: &TypeSignalsDocument) -> Result<String, TypeSignalsCodecError> {
    encode_with_workspace(doc, None)
}

/// Encodes a document, rewriting workspace-absolute rustdoc identity paths to
/// repo-relative paths so tracked artifacts never contain a work-machine home.
///
/// # Errors
///
/// Returns an error when the document does not use the current schema.
pub(crate) fn encode_with_workspace(
    doc: &TypeSignalsDocument,
    workspace_root: Option<&std::path::Path>,
) -> Result<String, TypeSignalsCodecError> {
    if doc.schema_version().value() != domain::TYPE_SIGNALS_SCHEMA_VERSION {
        return Err(TypeSignalsCodecError::UnsupportedSchemaVersion(doc.schema_version()));
    }
    let dto = TypeSignalsDocDto {
        schema_version: doc.schema_version().value(),
        generated_at: doc.generated_at().as_str().to_owned(),
        declaration_hash: doc.cache_key().declaration_hash().as_digest().as_str().to_owned(),
        head_commit: doc.cache_key().head_commit().as_ref().to_owned(),
        baseline_hash: doc.cache_key().baseline_hash().as_digest().as_str().to_owned(),
        implementation_fingerprint: doc
            .cache_key()
            .implementation_fingerprint()
            .as_digest()
            .as_str()
            .to_owned(),
        resolution_fingerprint: doc
            .cache_key()
            .resolution_fingerprint()
            .as_digest()
            .as_str()
            .to_owned(),
        rustdoc_execution_identity: execution_identity_to_dto(
            doc.cache_key().rustdoc_execution_identity(),
            workspace_root,
        )?,
        signals: doc
            .signals()
            .iter()
            .map(|signal| {
                let dto = signal_to_dto(signal);
                validate_namespace_for_kind_tag(&dto.kind_tag, dto.namespace)?;
                Ok(dto)
            })
            .collect::<Result<Vec<_>, TypeSignalsCodecError>>()?,
    };
    // Serialize through Value so every signal object uses canonical key order
    // before pretty-printing.
    let value = serde_json::to_value(&dto).map_err(TypeSignalsCodecError::Json)?;
    serde_json::to_string_pretty(&value).map_err(TypeSignalsCodecError::Json)
}

fn portable_identity_path(
    stored: String,
    identity_root: &std::path::Path,
    external_target_directory: Option<&ResolvedCargoTargetDirectory>,
) -> std::path::PathBuf {
    let path = std::path::PathBuf::from(stored);
    if let Some(target_directory) = external_target_directory {
        let marker = external_target_identity_path(target_directory.as_path());
        if path == marker {
            return target_directory.as_path().to_path_buf();
        }
        if let Ok(relative) = path.strip_prefix(&marker) {
            return target_directory.as_path().join(relative);
        }
    }
    if path.is_absolute() { path } else { identity_root.join(path) }
}

fn execution_identity_from_dto(
    dto: RustdocExecutionIdentityDto,
    identity_root: &std::path::Path,
    external_target_directory: Option<&ResolvedCargoTargetDirectory>,
) -> Result<RustdocExecutionIdentity, TypeSignalsCodecError> {
    let target_directory = ResolvedCargoTargetDirectory::try_new(portable_identity_path(
        dto.target_directory,
        identity_root,
        external_target_directory,
    ))
    .map_err(|error| {
        TypeSignalsCodecError::InvalidExecutionIdentity(FreeText::new(error.to_string()))
    })?;
    let expected_json_path = ExpectedRustdocJsonPath::try_new(
        portable_identity_path(dto.expected_json_path, identity_root, external_target_directory),
        &target_directory,
    )
    .map_err(|error| {
        TypeSignalsCodecError::InvalidExecutionIdentity(FreeText::new(error.to_string()))
    })?;
    let crate_name = CrateName::new(dto.crate_name).map_err(|error| {
        TypeSignalsCodecError::InvalidExecutionIdentity(FreeText::new(error.to_string()))
    })?;
    let features = dto
        .features
        .into_iter()
        .map(|feature| {
            CargoFeatureName::try_new(feature).map_err(|error| {
                TypeSignalsCodecError::InvalidExecutionIdentity(FreeText::new(error.to_string()))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let profile = CargoProfileName::try_new(dto.profile).map_err(|error| {
        TypeSignalsCodecError::InvalidExecutionIdentity(FreeText::new(error.to_string()))
    })?;
    RustdocExecutionIdentity::new(
        target_directory,
        crate_name,
        features,
        profile,
        expected_json_path,
    )
    .map_err(|error| {
        TypeSignalsCodecError::InvalidExecutionIdentity(FreeText::new(error.to_string()))
    })
}

fn relativize_workspace_path(
    path: &std::path::Path,
    workspace_root: Option<&std::path::Path>,
) -> String {
    workspace_root
        .and_then(|root| path.strip_prefix(root).ok())
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

fn execution_identity_to_dto(
    identity: &RustdocExecutionIdentity,
    workspace_root: Option<&std::path::Path>,
) -> Result<RustdocExecutionIdentityDto, TypeSignalsCodecError> {
    let target_path = identity.target_directory().as_path();
    let (target_directory, external_target) = match workspace_root {
        Some(root) => match target_path.strip_prefix(root) {
            Ok(relative) => (relative.display().to_string(), false),
            Err(_) => (external_target_identity_path(target_path).display().to_string(), true),
        },
        None => (target_path.display().to_string(), false),
    };
    let expected_relative =
        identity.expected_json_path().as_path().strip_prefix(target_path).map_err(|_| {
            TypeSignalsCodecError::InvalidExecutionIdentity(FreeText::new(
                "expected rustdoc JSON path is outside the target directory",
            ))
        })?;
    let expected_json_path = if external_target {
        std::path::Path::new(&target_directory).join(expected_relative).display().to_string()
    } else {
        relativize_workspace_path(identity.expected_json_path().as_path(), workspace_root)
    };
    Ok(RustdocExecutionIdentityDto {
        target_directory,
        crate_name: identity.crate_name().as_str().to_owned(),
        features: identity.features().iter().map(|feature| feature.as_str().to_owned()).collect(),
        profile: identity.profile().as_str().to_owned(),
        expected_json_path,
    })
}

fn external_target_identity_path(path: &std::path::Path) -> std::path::PathBuf {
    let bytes: [u8; 32] = sha2::Sha256::digest(path.to_string_lossy().as_bytes()).into();
    let digest = Sha256Digest::from_content_hash(ContentHash::from_bytes(bytes));
    std::path::Path::new(EXTERNAL_TARGET_IDENTITY_ROOT).join(digest.as_str())
}

/// Computes the SHA-256 digest of declaration file bytes.
#[must_use = "the declaration hash is required to validate type-signal freshness"]
pub fn declaration_hash(declaration_bytes: &[u8]) -> CatalogueDeclarationHash {
    let bytes: [u8; 32] = sha2::Sha256::digest(declaration_bytes).into();
    CatalogueDeclarationHash::new(Sha256Digest::from_content_hash(ContentHash::from_bytes(bytes)))
}

/// Computes the SHA-256 digest of baseline file bytes.
#[must_use = "the baseline hash is required to validate type-signal freshness"]
pub(crate) fn baseline_hash(baseline_bytes: &[u8]) -> BaselineHash {
    let bytes: [u8; 32] = sha2::Sha256::digest(baseline_bytes).into();
    BaselineHash::new(Sha256Digest::from_content_hash(ContentHash::from_bytes(bytes)))
}

fn is_utc_timestamp(raw: &str) -> bool {
    raw.ends_with('Z') || raw.ends_with("+00:00")
}

fn parse_digest(field: &str, value: String) -> Result<Sha256Digest, TypeSignalsCodecError> {
    Sha256Digest::try_new(value).map_err(|source| TypeSignalsCodecError::InvalidDigest {
        field: FreeText::new(field),
        source,
    })
}

fn parse_head_commit(value: String) -> Result<CommitHash, TypeSignalsCodecError> {
    CommitHash::try_new(value.clone()).map_err(|error| {
        TypeSignalsCodecError::InvalidTimestamp(FreeText::new(format!(
            "invalid head_commit '{}': {error}",
            value
        )))
    })
}

fn signal_from_dto(dto: TypeSignalDto) -> Result<TypeSignal, TypeSignalsCodecError> {
    validate_namespace_for_kind_tag(&dto.kind_tag, dto.namespace)?;
    // An unknown value must fail the decode: mapping it to a default would let
    // an invalid cache document skip the cache-miss/self-healing path.
    let signal = parse_confidence_signal(&dto.signal)
        .ok_or_else(|| TypeSignalsCodecError::InvalidSignal(FreeText::new(dto.signal.clone())))?;
    let identity = match dto.namespace {
        TypeSignalNamespaceField::Missing => {
            if kind_tag_namespace(&dto.kind_tag).is_some() {
                return Err(TypeSignalsCodecError::InvalidNamespace(FreeText::new(&dto.kind_tag)));
            }
            ThreeWaySignalIdentity::Label { label: FreeText::new(dto.type_name) }
        }
        TypeSignalNamespaceField::Catalogue(namespace) => ThreeWaySignalIdentity::CatalogueItem {
            item_name: FreeText::new(dto.type_name),
            namespace: namespace.into(),
        },
        TypeSignalNamespaceField::Label => {
            ThreeWaySignalIdentity::Label { label: FreeText::new(dto.type_name) }
        }
    };
    Ok(TypeSignal::new(
        identity,
        dto.kind_tag,
        signal,
        dto.found_type,
        dto.found_items,
        dto.missing_items,
        dto.extra_items,
    ))
}

fn validate_namespace_for_kind_tag(
    kind_tag: &str,
    namespace: TypeSignalNamespaceField,
) -> Result<(), TypeSignalsCodecError> {
    let valid = match (kind_tag_namespace(kind_tag), namespace) {
        (Some(expected), TypeSignalNamespaceField::Catalogue(actual)) => {
            CatalogueItemNamespace::from(actual) == expected
        }
        // `unknown` is emitted for both typed tombstones and report-only
        // labels, so it is the one intentionally ambiguous kind tag.
        (None, TypeSignalNamespaceField::Catalogue(_)) if kind_tag == "unknown" => true,
        (None, TypeSignalNamespaceField::Missing) => true,
        (None, TypeSignalNamespaceField::Label) if kind_tag == "unknown" => true,
        // `free_function` is a report label and must remain namespace-less.
        (None, TypeSignalNamespaceField::Label) => true,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(TypeSignalsCodecError::InvalidNamespace(FreeText::new(kind_tag)))
    }
}

fn signal_to_dto(signal: &TypeSignal) -> TypeSignalDto {
    let (type_name, namespace) = match signal.identity() {
        ThreeWaySignalIdentity::CatalogueItem { item_name, namespace } => (
            item_name.as_str().to_owned(),
            TypeSignalNamespaceField::Catalogue((*namespace).into()),
        ),
        ThreeWaySignalIdentity::Label { label } => {
            (label.as_str().to_owned(), TypeSignalNamespaceField::Label)
        }
    };
    TypeSignalDto {
        type_name,
        namespace,
        kind_tag: signal.kind_tag().to_owned(),
        signal: confidence_signal_to_str(signal.signal()).to_owned(),
        found_type: signal.found_type(),
        found_items: signal.found_items().to_vec(),
        missing_items: signal.missing_items().to_vec(),
        extra_items: signal.extra_items().to_vec(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]
#[path = "type_signals_codec_tests.rs"]
mod tests;
