//! Serde codec for per-layer TDDD evaluation-result files.

use domain::tddd::type_signals_doc::{
    CatalogueDeclarationHash, ImplementationInputHash, Sha256Digest, Sha256DigestError,
    TypeSignalsDocument, TypeSignalsSchemaVersion, TypeSignalsSchemaVersionError,
};
use domain::{ConfidenceSignal, ContentHash, FreeText, Timestamp, TypeSignal};
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TypeSignalsDocDto {
    schema_version: u32,
    generated_at: String,
    declaration_hash: String,
    implementation_input_hash: String,
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
    Ok(TypeSignalsDocument::with_schema_version(
        schema_version,
        generated_at,
        CatalogueDeclarationHash::new(parse_digest("declaration_hash", dto.declaration_hash)?),
        ImplementationInputHash::new(parse_digest(
            "implementation_input_hash",
            dto.implementation_input_hash,
        )?),
        dto.signals.into_iter().map(signal_from_dto).collect(),
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
    serde_json::to_string_pretty(&TypeSignalsDocDto {
        schema_version: doc.schema_version().value(),
        generated_at: doc.generated_at().as_str().to_owned(),
        declaration_hash: doc.declaration_hash().as_digest().as_str().to_owned(),
        implementation_input_hash: doc.implementation_input_hash().as_digest().as_str().to_owned(),
        signals: doc.signals().iter().map(signal_to_dto).collect(),
    })
    .map_err(TypeSignalsCodecError::Json)
}

/// Computes the SHA-256 digest of declaration file bytes.
#[must_use = "the declaration hash is required to validate type-signal freshness"]
pub fn declaration_hash(declaration_bytes: &[u8]) -> CatalogueDeclarationHash {
    let bytes: [u8; 32] = sha2::Sha256::digest(declaration_bytes).into();
    CatalogueDeclarationHash::new(Sha256Digest::from_content_hash(ContentHash::from_bytes(bytes)))
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

fn signal_from_dto(dto: TypeSignalDto) -> TypeSignal {
    TypeSignal::new(
        dto.type_name,
        dto.kind_tag,
        parse_confidence_signal(&dto.signal).unwrap_or(ConfidenceSignal::Red),
        dto.found_type,
        dto.found_items,
        dto.missing_items,
        dto.extra_items,
    )
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
    use super::*;

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn sample_doc() -> TypeSignalsDocument {
        let digest = Sha256Digest::try_new(DIGEST.to_owned()).unwrap();
        TypeSignalsDocument::new(
            Timestamp::new("2026-04-18T12:00:00Z").unwrap(),
            CatalogueDeclarationHash::new(digest.clone()),
            ImplementationInputHash::new(digest),
            vec![],
        )
    }

    #[test]
    fn test_encode_decode_roundtrip_preserves_document() {
        let document = sample_doc();
        assert_eq!(decode(&encode(&document).unwrap()).unwrap(), document);
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
            implementation_input_hash: DIGEST.to_owned(),
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
    fn test_decode_requires_implementation_input_hash() {
        let mut payload = serde_json::to_value(&TypeSignalsDocDto {
            schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
            generated_at: "2026-04-18T12:00:00Z".to_owned(),
            declaration_hash: DIGEST.to_owned(),
            implementation_input_hash: DIGEST.to_owned(),
            signals: vec![],
        })
        .unwrap();
        payload.as_object_mut().unwrap().remove("implementation_input_hash");
        assert!(matches!(decode(&payload.to_string()), Err(TypeSignalsCodecError::Json(_))));
    }

    #[test]
    fn test_decode_requires_declaration_hash() {
        let mut payload = serde_json::to_value(&TypeSignalsDocDto {
            schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
            generated_at: "2026-04-18T12:00:00Z".to_owned(),
            declaration_hash: DIGEST.to_owned(),
            implementation_input_hash: DIGEST.to_owned(),
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
            implementation_input_hash: DIGEST.to_owned(),
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

        let invalid_digest = TypeSignalsDocDto { implementation_input_hash: "g".repeat(64), ..dto };
        assert!(matches!(
            decode(&serde_json::to_string(&invalid_digest).unwrap()),
            Err(TypeSignalsCodecError::InvalidDigest { source: Sha256DigestError::InvalidHex, .. })
        ));
    }
}
