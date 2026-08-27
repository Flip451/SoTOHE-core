//! Serde codec for per-layer TDDD evaluation-result files.

use domain::tddd::catalogue_v2::identifiers::CatalogueItemNamespace;
use domain::tddd::signal_evaluator::ThreeWaySignalIdentity;
use domain::tddd::type_signals_doc::{
    BaselineHash, CatalogueDeclarationHash, Sha256Digest, Sha256DigestError, TypeSignalsCacheKey,
    TypeSignalsDocument, TypeSignalsSchemaVersion, TypeSignalsSchemaVersionError,
};
use domain::{CommitHash, ContentHash, FreeText, Timestamp, TypeSignal};
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::tddd::catalogue_spec_signals_codec::{
    confidence_signal_to_str, parse_confidence_signal,
};
use crate::tddd::type_signals_evaluator::signal_tags::kind_tag_namespace;

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TypeSignalsDocDto {
    schema_version: u32,
    generated_at: String,
    declaration_hash: String,
    head_commit: String,
    baseline_hash: String,
    signals: Vec<TypeSignalDto>,
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
    let dto: TypeSignalsDocDto = serde_json::from_str(json)?;
    let schema_version = TypeSignalsSchemaVersion::try_new(dto.schema_version)
        .map_err(TypeSignalsCodecError::InvalidSchemaVersion)?;
    if schema_version.value() != domain::TYPE_SIGNALS_SCHEMA_VERSION {
        return Err(TypeSignalsCodecError::UnsupportedSchemaVersion(schema_version));
    }
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
    if doc.schema_version().value() != domain::TYPE_SIGNALS_SCHEMA_VERSION {
        return Err(TypeSignalsCodecError::UnsupportedSchemaVersion(doc.schema_version()));
    }
    let dto = TypeSignalsDocDto {
        schema_version: doc.schema_version().value(),
        generated_at: doc.generated_at().as_str().to_owned(),
        declaration_hash: doc.cache_key().declaration_hash().as_digest().as_str().to_owned(),
        head_commit: doc.cache_key().head_commit().as_ref().to_owned(),
        baseline_hash: doc.cache_key().baseline_hash().as_digest().as_str().to_owned(),
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
mod tests {
    use domain::ConfidenceSignal;
    use domain::tddd::catalogue_v2::identifiers::CatalogueItemNamespace;
    use domain::tddd::type_signals_doc::{
        TypeSignalsAuthorityStatus, TypeSignalsReuseDecision, TypeSignalsReuseInput,
        TypeSignalsWorktreeStatus, decide_type_signals_reuse,
    };

    use super::*;

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn sample_doc() -> TypeSignalsDocument {
        let digest = Sha256Digest::try_new(DIGEST.to_owned()).unwrap();
        TypeSignalsDocument::new(
            Timestamp::new("2026-04-18T12:00:00Z").unwrap(),
            TypeSignalsCacheKey::new(
                CatalogueDeclarationHash::new(digest.clone()),
                CommitHash::try_new("a".repeat(40)).unwrap(),
                BaselineHash::new(digest),
            ),
            vec![TypeSignal::new(
                ThreeWaySignalIdentity::Label { label: FreeText::new("Example") },
                "free_function".to_owned(),
                ConfidenceSignal::Blue,
                true,
                vec!["field".to_owned()],
                vec![],
                vec!["unexpected".to_owned()],
            )],
        )
    }

    fn sample_catalogue_item_doc() -> TypeSignalsDocument {
        let digest = Sha256Digest::try_new(DIGEST.to_owned()).unwrap();
        TypeSignalsDocument::new(
            Timestamp::new("2026-04-18T12:00:00Z").unwrap(),
            TypeSignalsCacheKey::new(
                CatalogueDeclarationHash::new(digest.clone()),
                CommitHash::try_new("a".repeat(40)).unwrap(),
                BaselineHash::new(digest),
            ),
            vec![
                TypeSignal::new(
                    ThreeWaySignalIdentity::CatalogueItem {
                        item_name: FreeText::new("Shared"),
                        namespace: CatalogueItemNamespace::Type,
                    },
                    "value_object".to_owned(),
                    ConfidenceSignal::Blue,
                    true,
                    vec![],
                    vec![],
                    vec![],
                ),
                TypeSignal::new(
                    ThreeWaySignalIdentity::CatalogueItem {
                        item_name: FreeText::new("Shared"),
                        namespace: CatalogueItemNamespace::Trait,
                    },
                    "secondary_port".to_owned(),
                    ConfidenceSignal::Yellow,
                    false,
                    vec![],
                    vec!["method".to_owned()],
                    vec![],
                ),
            ],
        )
    }

    #[test]
    fn test_encode_decode_roundtrip_preserves_document() {
        let document = sample_doc();
        assert_eq!(decode(&encode(&document).unwrap()).unwrap(), document);
    }

    #[test]
    fn test_encode_decode_roundtrip_preserves_catalogue_item_namespaces() {
        let document = sample_catalogue_item_doc();

        let encoded = encode(&document).unwrap();
        assert!(encoded.contains("\"namespace\": \"type\""));
        assert!(encoded.contains("\"namespace\": \"trait\""));

        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, document);
        let namespaces = decoded
            .signals()
            .iter()
            .map(|signal| signal.identity().namespace())
            .collect::<Vec<_>>();
        assert_eq!(
            namespaces,
            vec![Some(CatalogueItemNamespace::Type), Some(CatalogueItemNamespace::Trait)]
        );
        let signals = decoded.signals().iter().map(TypeSignal::signal).collect::<Vec<_>>();
        assert_eq!(signals, vec![ConfidenceSignal::Blue, ConfidenceSignal::Yellow]);
    }

    #[test]
    fn test_encode_rejects_catalogue_kind_without_namespace() {
        let digest = Sha256Digest::try_new(DIGEST.to_owned()).unwrap();
        let document = TypeSignalsDocument::new(
            Timestamp::new("2026-04-18T12:00:00Z").unwrap(),
            TypeSignalsCacheKey::new(
                CatalogueDeclarationHash::new(digest.clone()),
                CommitHash::try_new("a".repeat(40)).unwrap(),
                BaselineHash::new(digest),
            ),
            vec![TypeSignal::new(
                ThreeWaySignalIdentity::Label { label: FreeText::new("Shared") },
                "value_object".to_owned(),
                ConfidenceSignal::Blue,
                true,
                vec![],
                vec![],
                vec![],
            )],
        );

        assert!(matches!(encode(&document), Err(TypeSignalsCodecError::InvalidNamespace(_))));
    }

    #[test]
    fn test_encode_canonicalizes_json_keys_and_is_byte_stable() {
        let document = sample_doc();

        let first = encode(&document).unwrap();
        let second = encode(&document).unwrap();

        assert_eq!(first, second, "type-signal encoding must not churn JSON bytes");
        assert!(
            first.starts_with("{\n  \"baseline_hash\":"),
            "type-signal keys must be canonicalized: {first}"
        );
        let signal = &first[first.find("\"extra_items\"").unwrap()..];
        let extra_items = signal.find("\"extra_items\"").unwrap();
        let found_items = signal.find("\"found_items\"").unwrap();
        let found_type = signal.find("\"found_type\"").unwrap();
        let kind_tag = signal.find("\"kind_tag\"").unwrap();
        let namespace = signal.find("\"namespace\"").unwrap();
        let signal_key = signal.find("\"signal\"").unwrap();
        let type_name = signal.find("\"type_name\"").unwrap();
        assert!(
            extra_items < found_items
                && found_items < found_type
                && found_type < kind_tag
                && kind_tag < namespace
                && kind_tag < signal_key
                && namespace < signal_key
                && signal_key < type_name,
            "type-signal objects must be recursively canonicalized: {signal}"
        );
    }

    #[test]
    fn test_declaration_hash_changes_when_declaration_bytes_change() {
        assert_eq!(
            declaration_hash(b"type A = u8;").as_digest().as_str(),
            "bdc16928bf7d4bbd73c69b65d1d3e4e644225f9c3322d589f1a63b5f37af0592"
        );
        assert_ne!(declaration_hash(b"type A = u8;"), declaration_hash(b"type A = u16;"));
    }

    #[test]
    fn test_decode_rejects_unknown_fields() {
        let mut payload = serde_json::to_value(&TypeSignalsDocDto {
            schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
            generated_at: "2026-04-18T12:00:00Z".to_owned(),
            declaration_hash: DIGEST.to_owned(),
            head_commit: "a".repeat(40),
            baseline_hash: DIGEST.to_owned(),
            signals: vec![],
        })
        .unwrap();
        payload
            .as_object_mut()
            .unwrap()
            .insert("unknown_field".to_owned(), serde_json::Value::String(DIGEST.to_owned()));
        assert!(matches!(decode(&payload.to_string()), Err(TypeSignalsCodecError::Json(_))));
    }

    #[test]
    fn test_decode_requires_head_commit() {
        let mut payload = serde_json::to_value(&TypeSignalsDocDto {
            schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
            generated_at: "2026-04-18T12:00:00Z".to_owned(),
            declaration_hash: DIGEST.to_owned(),
            head_commit: "a".repeat(40),
            baseline_hash: DIGEST.to_owned(),
            signals: vec![],
        })
        .unwrap();
        payload.as_object_mut().unwrap().remove("head_commit");
        assert!(matches!(decode(&payload.to_string()), Err(TypeSignalsCodecError::Json(_))));
    }

    #[test]
    fn test_decode_requires_baseline_hash() {
        let mut payload = serde_json::to_value(&TypeSignalsDocDto {
            schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
            generated_at: "2026-04-18T12:00:00Z".to_owned(),
            declaration_hash: DIGEST.to_owned(),
            head_commit: "a".repeat(40),
            baseline_hash: DIGEST.to_owned(),
            signals: vec![],
        })
        .unwrap();
        payload.as_object_mut().unwrap().remove("baseline_hash");
        assert!(matches!(decode(&payload.to_string()), Err(TypeSignalsCodecError::Json(_))));
    }

    #[test]
    fn test_decode_rejects_unknown_confidence_signal_value() {
        let mut payload = serde_json::to_value(&TypeSignalsDocDto {
            schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
            generated_at: "2026-04-18T12:00:00Z".to_owned(),
            declaration_hash: DIGEST.to_owned(),
            head_commit: "a".repeat(40),
            baseline_hash: DIGEST.to_owned(),
            signals: vec![signal_to_dto(&TypeSignal::new(
                ThreeWaySignalIdentity::Label { label: FreeText::new("Example") },
                "free_function".to_owned(),
                ConfidenceSignal::Blue,
                true,
                vec![],
                vec![],
                vec![],
            ))],
        })
        .unwrap();
        *payload.pointer_mut("/signals/0/signal").unwrap() =
            serde_json::Value::String("bogus".to_owned());
        assert!(
            matches!(decode(&payload.to_string()), Err(TypeSignalsCodecError::InvalidSignal(_))),
            "an unknown signal value must fail the decode instead of defaulting"
        );
    }

    #[test]
    fn test_decode_rejects_unknown_catalogue_item_namespace() {
        let mut payload = serde_json::to_value(&TypeSignalsDocDto {
            schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
            generated_at: "2026-04-18T12:00:00Z".to_owned(),
            declaration_hash: DIGEST.to_owned(),
            head_commit: "a".repeat(40),
            baseline_hash: DIGEST.to_owned(),
            signals: vec![signal_to_dto(&TypeSignal::new(
                ThreeWaySignalIdentity::CatalogueItem {
                    item_name: FreeText::new("Shared"),
                    namespace: CatalogueItemNamespace::Type,
                },
                "value_object".to_owned(),
                ConfidenceSignal::Blue,
                true,
                vec![],
                vec![],
                vec![],
            ))],
        })
        .unwrap();
        *payload.pointer_mut("/signals/0/namespace").unwrap() =
            serde_json::Value::String("module".to_owned());

        assert!(matches!(decode(&payload.to_string()), Err(TypeSignalsCodecError::Json(_))));
    }

    #[test]
    fn test_decode_rejects_v5_signal_without_explicit_identity() {
        let payload = serde_json::json!({
            "schema_version": domain::TYPE_SIGNALS_SCHEMA_VERSION,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": DIGEST,
            "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "baseline_hash": DIGEST,
            "signals": [{
                "type_name": "Shared",
                "kind_tag": "value_object",
                "signal": "blue",
                "found_type": true
            }]
        });

        let result = decode(&payload.to_string());
        assert!(
            matches!(result, Err(TypeSignalsCodecError::InvalidNamespace(_))),
            "v5 must not infer a label from an omitted catalogue identity discriminator: {result:?}"
        );
    }

    #[test]
    fn test_decode_accepts_missing_namespace_for_function_label() {
        let payload = serde_json::json!({
            "schema_version": domain::TYPE_SIGNALS_SCHEMA_VERSION,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": DIGEST,
            "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "baseline_hash": DIGEST,
            "signals": [{
                "type_name": "compute",
                "kind_tag": "free_function",
                "signal": "blue",
                "found_type": true
            }]
        });

        let document =
            decode(&payload.to_string()).expect("an omitted namespace identifies a label row");
        assert!(matches!(
            document.signals()[0].identity(),
            ThreeWaySignalIdentity::Label { label } if label.as_str() == "compute"
        ));
    }

    #[test]
    fn test_decode_accepts_missing_namespace_for_unknown_label() {
        let payload = serde_json::json!({
            "schema_version": domain::TYPE_SIGNALS_SCHEMA_VERSION,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": DIGEST,
            "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "baseline_hash": DIGEST,
            "signals": [{
                "type_name": "report::label",
                "kind_tag": "unknown",
                "signal": "red",
                "found_type": true
            }]
        });

        let document =
            decode(&payload.to_string()).expect("an omitted namespace identifies a label row");
        assert!(matches!(
            document.signals()[0].identity(),
            ThreeWaySignalIdentity::Label { label } if label.as_str() == "report::label"
        ));
    }

    #[test]
    fn test_decode_accepts_explicit_null_label_identity() {
        let payload = serde_json::json!({
            "schema_version": domain::TYPE_SIGNALS_SCHEMA_VERSION,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": DIGEST,
            "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "baseline_hash": DIGEST,
            "signals": [{
                "type_name": "Shared",
                "namespace": null,
                "kind_tag": "free_function",
                "signal": "blue",
                "found_type": true
            }]
        });

        let document = decode(&payload.to_string()).expect("explicit null must identify a label");
        assert!(matches!(
            document.signals()[0].identity(),
            ThreeWaySignalIdentity::Label { label } if label.as_str() == "Shared"
        ));
    }

    #[test]
    fn test_decode_rejects_null_namespace_for_catalogue_kind_tag() {
        let payload = serde_json::json!({
            "schema_version": domain::TYPE_SIGNALS_SCHEMA_VERSION,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": DIGEST,
            "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "baseline_hash": DIGEST,
            "signals": [{
                "type_name": "Shared",
                "namespace": null,
                "kind_tag": "value_object",
                "signal": "blue",
                "found_type": true
            }]
        });

        assert!(matches!(
            decode(&payload.to_string()),
            Err(TypeSignalsCodecError::InvalidNamespace(_))
        ));
    }

    #[test]
    fn test_decode_rejects_catalogue_namespace_for_function_label() {
        let payload = serde_json::json!({
            "schema_version": domain::TYPE_SIGNALS_SCHEMA_VERSION,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": DIGEST,
            "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "baseline_hash": DIGEST,
            "signals": [{
                "type_name": "compute",
                "namespace": "type",
                "kind_tag": "free_function",
                "signal": "blue",
                "found_type": true
            }]
        });

        assert!(matches!(
            decode(&payload.to_string()),
            Err(TypeSignalsCodecError::InvalidNamespace(_))
        ));
    }

    #[test]
    fn test_decode_requires_declaration_hash() {
        let mut payload = serde_json::to_value(&TypeSignalsDocDto {
            schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
            generated_at: "2026-04-18T12:00:00Z".to_owned(),
            declaration_hash: DIGEST.to_owned(),
            head_commit: "a".repeat(40),
            baseline_hash: DIGEST.to_owned(),
            signals: vec![],
        })
        .unwrap();
        payload.as_object_mut().unwrap().remove("declaration_hash");
        assert!(matches!(decode(&payload.to_string()), Err(TypeSignalsCodecError::Json(_))));
    }

    #[test]
    fn test_decode_rejects_each_typed_codec_error_variant() {
        let dto = TypeSignalsDocDto {
            schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
            generated_at: "2026-04-18T12:00:00Z".to_owned(),
            declaration_hash: DIGEST.to_owned(),
            head_commit: "a".repeat(40),
            baseline_hash: DIGEST.to_owned(),
            signals: vec![],
        };

        let zero_schema = TypeSignalsDocDto { schema_version: 0, ..dto.clone() };
        assert!(matches!(
            decode(&serde_json::to_string(&zero_schema).unwrap()),
            Err(TypeSignalsCodecError::InvalidSchemaVersion(TypeSignalsSchemaVersionError::Zero))
        ));

        // v4 predates the namespace-bearing TypeSignal identity and must be
        // rejected so the evaluator treats legacy rows as a cache miss.
        let unsupported_schema = TypeSignalsDocDto { schema_version: 4, ..dto.clone() };
        assert!(matches!(
            decode(&serde_json::to_string(&unsupported_schema).unwrap()),
            Err(TypeSignalsCodecError::UnsupportedSchemaVersion(_))
        ));

        let invalid_timestamp = TypeSignalsDocDto {
            generated_at: "2026-04-18T12:00:00+09:00".to_owned(),
            ..dto.clone()
        };
        assert!(matches!(
            decode(&serde_json::to_string(&invalid_timestamp).unwrap()),
            Err(TypeSignalsCodecError::InvalidTimestamp(_))
        ));

        let invalid_head = TypeSignalsDocDto { head_commit: "g".repeat(40), ..dto };
        assert!(matches!(
            decode(&serde_json::to_string(&invalid_head).unwrap()),
            Err(TypeSignalsCodecError::InvalidTimestamp(_))
        ));
    }

    #[test]
    fn test_baseline_hash_changes_when_baseline_bytes_change() {
        assert_eq!(
            baseline_hash(b"baseline A").as_digest().as_str(),
            "7061fe86b948cf084b16235a204ce4a357f6b38f637f28edad27213428fda3d6"
        );
        assert_ne!(baseline_hash(b"baseline A"), baseline_hash(b"baseline B"));
    }

    #[test]
    fn test_baseline_hash_cache_key_tracks_baseline_bytes_and_mismatch_requires_recomparison() {
        let digest = Sha256Digest::try_new(DIGEST.to_owned()).unwrap();
        let declaration = CatalogueDeclarationHash::new(digest.clone());
        let head_commit = CommitHash::try_new("a".repeat(40)).unwrap();
        let recorded_baseline = baseline_hash(b"baseline A");
        let current_baseline = baseline_hash(b"baseline B");
        let recorded = TypeSignalsCacheKey::new(
            declaration.clone(),
            head_commit.clone(),
            recorded_baseline.clone(),
        );
        let current = TypeSignalsCacheKey::new(declaration, head_commit, current_baseline);

        assert_eq!(recorded.baseline_hash(), &baseline_hash(b"baseline A"));
        let input = TypeSignalsReuseInput::verify(
            recorded,
            current,
            TypeSignalsWorktreeStatus::Clean,
            TypeSignalsAuthorityStatus::Readable,
        )
        .unwrap();
        assert_eq!(
            decide_type_signals_reuse(&input),
            TypeSignalsReuseDecision::ReevaluateWithoutExtraction,
            "a changed rustdoc baseline digest must invalidate reuse"
        );
    }
}
