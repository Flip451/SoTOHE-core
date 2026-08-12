//! Serde codec for per-layer TDDD evaluation-result files.

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

/// Decodes a `<layer>-type-signals.json` string.
///
/// # Errors
///
/// Returns an error for malformed JSON, an unsupported schema, invalid hashes,
/// or a non-UTC timestamp.
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
        signals: doc.signals().iter().map(signal_to_dto).collect(),
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
    // An unknown value must fail the decode: mapping it to a default would let
    // an invalid cache document skip the cache-miss/self-healing path.
    let signal = parse_confidence_signal(&dto.signal)
        .ok_or_else(|| TypeSignalsCodecError::InvalidSignal(FreeText::new(dto.signal.clone())))?;
    Ok(TypeSignal::new(
        dto.type_name,
        dto.kind_tag,
        signal,
        dto.found_type,
        dto.found_items,
        dto.missing_items,
        dto.extra_items,
    ))
}

fn signal_to_dto(signal: &TypeSignal) -> TypeSignalDto {
    TypeSignalDto {
        type_name: signal.type_name().to_owned(),
        kind_tag: signal.kind_tag().to_owned(),
        signal: confidence_signal_to_str(signal.signal()).to_owned(),
        found_type: signal.found_type(),
        found_items: signal.found_items().to_vec(),
        missing_items: signal.missing_items().to_vec(),
        extra_items: signal.extra_items().to_vec(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use domain::ConfidenceSignal;
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
                "Example",
                "struct",
                ConfidenceSignal::Blue,
                true,
                vec!["field".to_owned()],
                vec![],
                vec!["unexpected".to_owned()],
            )],
        )
    }

    #[test]
    fn test_encode_decode_roundtrip_preserves_document() {
        let document = sample_doc();
        assert_eq!(decode(&encode(&document).unwrap()).unwrap(), document);
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
        let signal_key = signal.find("\"signal\"").unwrap();
        let type_name = signal.find("\"type_name\"").unwrap();
        assert!(
            extra_items < found_items
                && found_items < found_type
                && found_type < kind_tag
                && kind_tag < signal_key
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
                "Example",
                "struct",
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

        let unsupported_schema = TypeSignalsDocDto { schema_version: 2, ..dto.clone() };
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
