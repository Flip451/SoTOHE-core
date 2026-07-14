//! Serde codec for the per-layer TDDD evaluation-result file
//! (`<layer>-type-signals.json`, schema_version 1).
//!
//! The declaration file (`<layer>-types.json`) stores authored type declarations
//! (decoded by `catalogue_document_codec`); this module handles the generated
//! evaluation-result file introduced by
//! `knowledge/adr/2026-04-18-1400-tddd-ci-gate-and-signals-separation.md` §D1.
//!
//! # Responsibility split
//!
//! - `encode(&TypeSignalsDocument) -> Result<String, _>` emits JSON suitable
//!   for writing to `<layer>-type-signals.json`.
//! - `decode(&str) -> Result<TypeSignalsDocument, _>` parses the same file back
//!   and rejects unknown schema versions / unknown fields / unparseable
//!   timestamps. Unknown `signal` strings are normalised to `Red` (fail-safe
//!   default for unrecognised signal values).
//! - `declaration_hash(bytes: &[u8]) -> CatalogueDeclarationHash` computes the SHA-256 hex
//!   digest of the declaration file bytes *as written to disk* (post-encode).
//!   The algorithm is pinned at schema_version 1 per
//!   ADR §D5 and the `declaration_hash` algorithm documentation on
//!   `TypeSignalsCodecError::UnsupportedSchemaVersion`.
//!
//! No filesystem I/O lives here — callers (CLI writer, CI reader) handle
//! `std::fs` and the `reject_symlinks_below` guard.

use domain::tddd::type_signals_doc::{
    BaselineHash, CatalogueDeclarationHash, EvaluatorContractHash, ImplementationInputHash,
    LiveRustdocSnapshotHash, RustdocExtractionContractHash, Sha256Digest, Sha256DigestError,
    TypeSignalsDocument, TypeSignalsFreshness, TypeSignalsSchemaVersion,
    TypeSignalsSchemaVersionError,
};
use domain::{ConfidenceSignal, ContentHash, FreeText, Timestamp, TypeSignal};
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::tddd::catalogue_spec_signals_codec::{
    confidence_signal_to_str, parse_confidence_signal,
};

/// Codec error for the per-layer evaluation-result file.
///
/// Variants mirror the three failure modes of decoding a
/// `<layer>-type-signals.json` file:
///
/// - `Json`: the payload is not valid JSON or fails DTO deserialization
///   (including `deny_unknown_fields` rejections).
/// - `UnsupportedSchemaVersion`: `schema_version` is not 1. The ADR pins the
///   format at 1; any future incompatible change must bump this version and
///   invalidate all existing `declaration_hash` values.
///
///   Algorithm note for `declaration_hash`: raw SHA-256 of the declaration
///   file bytes as written to disk (post-encode). No whitespace
///   normalisation. Pinned at schema_version 1.
/// - `InvalidTimestamp`: `generated_at` is not a parseable ISO 8601 UTC
///   timestamp.
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

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TypeSignalsDocDto {
    schema_version: u32,
    generated_at: String,
    declaration_hash: String,
    implementation_input_hash: String,
    baseline_hash: String,
    live_rustdoc_snapshot_hash: String,
    evaluator_contract_hash: String,
    rustdoc_extraction_contract_hash: String,
    // `signals` is required — no `#[serde(default)]`. A file that omits the key
    // is malformed/truncated and must fail closed (ADR §D1: signals is a required
    // field of the evaluation-result file shape).
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

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Decodes a `<layer>-type-signals.json` string into a `TypeSignalsDocument`.
///
/// # Errors
///
/// - `Json` when the input is not valid JSON or contains unknown fields.
/// - `UnsupportedSchemaVersion` when `schema_version != 1`.
/// - `InvalidTimestamp` when `generated_at` cannot be parsed as ISO 8601.
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
    // Enforce UTC-only: the on-disk format requires a UTC offset (`Z` or `+00:00`).
    // Non-UTC offsets (e.g. `+09:00`) parse successfully in `Timestamp::new` but violate
    // the ADR §D1 contract, which specifies `generated_at` as an ISO 8601 UTC timestamp.
    if !is_utc_timestamp(dto.generated_at.as_str()) {
        return Err(TypeSignalsCodecError::InvalidTimestamp(FreeText::new(dto.generated_at)));
    }
    let signals = dto.signals.into_iter().map(signal_from_dto).collect();
    let freshness = TypeSignalsFreshness::new(
        CatalogueDeclarationHash::new(parse_digest("declaration_hash", dto.declaration_hash)?),
        ImplementationInputHash::new(parse_digest(
            "implementation_input_hash",
            dto.implementation_input_hash,
        )?),
        BaselineHash::new(parse_digest("baseline_hash", dto.baseline_hash)?),
        LiveRustdocSnapshotHash::new(parse_digest(
            "live_rustdoc_snapshot_hash",
            dto.live_rustdoc_snapshot_hash,
        )?),
        EvaluatorContractHash::new(parse_digest(
            "evaluator_contract_hash",
            dto.evaluator_contract_hash,
        )?),
        RustdocExtractionContractHash::new(parse_digest(
            "rustdoc_extraction_contract_hash",
            dto.rustdoc_extraction_contract_hash,
        )?),
    );
    Ok(TypeSignalsDocument::with_schema_version(schema_version, generated_at, freshness, signals))
}

/// Encodes a `TypeSignalsDocument` into a pretty-printed JSON string.
///
/// The output is deterministic for a given document: serde_json preserves
/// the signal order from the document, and the DTO field order is fixed by
/// the struct layout.
///
/// # Errors
///
/// Returns `TypeSignalsCodecError::UnsupportedSchemaVersion` when the document
/// is not the sole schema version supported by this codec, or
/// `TypeSignalsCodecError::Json` if serialization fails (defensive for future
/// DTO changes).
pub fn encode(doc: &TypeSignalsDocument) -> Result<String, TypeSignalsCodecError> {
    if doc.schema_version().value() != domain::TYPE_SIGNALS_SCHEMA_VERSION {
        return Err(TypeSignalsCodecError::UnsupportedSchemaVersion(doc.schema_version()));
    }
    let dto = TypeSignalsDocDto {
        schema_version: doc.schema_version().value(),
        generated_at: doc.generated_at().as_str().to_owned(),
        declaration_hash: doc.declaration_hash().as_digest().as_str().to_owned(),
        implementation_input_hash: doc
            .freshness()
            .implementation_input_hash()
            .as_digest()
            .as_str()
            .to_owned(),
        baseline_hash: doc.freshness().baseline_hash().as_digest().as_str().to_owned(),
        live_rustdoc_snapshot_hash: doc
            .freshness()
            .live_rustdoc_snapshot_hash()
            .as_digest()
            .as_str()
            .to_owned(),
        evaluator_contract_hash: doc
            .freshness()
            .evaluator_contract_hash()
            .as_digest()
            .as_str()
            .to_owned(),
        rustdoc_extraction_contract_hash: doc
            .freshness()
            .rustdoc_extraction_contract_hash()
            .as_digest()
            .as_str()
            .to_owned(),
        signals: doc.signals().iter().map(signal_to_dto).collect(),
    };
    Ok(serde_json::to_string_pretty(&dto)?)
}

/// Computes the SHA-256 hex digest of the declaration file bytes.
///
/// Algorithm: raw SHA-256 of `declaration_bytes` — no normalisation, no BOM
/// stripping, no whitespace collapse. The algorithm is pinned at
/// schema_version 1. Callers MUST pass the declaration file bytes exactly as
/// written to disk (post-encode) so that `declaration_hash` is stable across
/// reads.
#[must_use = "the declaration hash is required to validate type-signals freshness"]
pub fn declaration_hash(declaration_bytes: &[u8]) -> CatalogueDeclarationHash {
    let bytes: [u8; 32] = sha2::Sha256::digest(declaration_bytes).into();
    CatalogueDeclarationHash::new(Sha256Digest::from_content_hash(ContentHash::from_bytes(bytes)))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns `true` when `raw` carries a UTC offset (`Z` or `+00:00`).
///
/// `Timestamp::new` accepts any RFC 3339 offset, but the on-disk format pins
/// `generated_at` to UTC (ADR §D1). Non-UTC strings that parse successfully
/// are rejected here before reaching `TypeSignalsDocument::with_schema_version`.
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
    // Legacy fallback-to-red contract: unknown signal strings are mapped to Red
    // rather than returning an error. This differs from `catalogue_spec_signals_codec`
    // which is strict. The shared `parse_confidence_signal` returns None for unknown
    // tags; we map that to Red here to preserve the pre-existing lenient behaviour.
    let signal = parse_confidence_signal(dto.signal.as_str()).unwrap_or(ConfidenceSignal::Red);
    TypeSignal::new(
        dto.type_name,
        dto.kind_tag,
        signal,
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn ts(raw: &str) -> Timestamp {
        Timestamp::new(raw).unwrap()
    }

    fn sample_signal_blue(name: &str) -> TypeSignal {
        TypeSignal::new(name, "value_object", ConfidenceSignal::Blue, true, vec![], vec![], vec![])
    }

    fn sample_doc() -> TypeSignalsDocument {
        let digest = Sha256Digest::try_new(DIGEST.to_owned()).unwrap();
        TypeSignalsDocument::new(
            ts("2026-04-18T12:00:00Z"),
            TypeSignalsFreshness::new(
                CatalogueDeclarationHash::new(digest.clone()),
                ImplementationInputHash::new(digest.clone()),
                BaselineHash::new(digest.clone()),
                LiveRustdocSnapshotHash::new(digest.clone()),
                EvaluatorContractHash::new(digest.clone()),
                RustdocExtractionContractHash::new(digest),
            ),
            vec![sample_signal_blue("Foo")],
        )
    }

    fn payload(schema_version: u32, generated_at: &str, signals: serde_json::Value) -> String {
        serde_json::json!({
            "schema_version": schema_version,
            "generated_at": generated_at,
            "declaration_hash": DIGEST,
            "implementation_input_hash": DIGEST,
            "baseline_hash": DIGEST,
            "live_rustdoc_snapshot_hash": DIGEST,
            "evaluator_contract_hash": DIGEST,
            "rustdoc_extraction_contract_hash": DIGEST,
            "signals": signals,
        })
        .to_string()
    }

    // --- encode / decode roundtrip ---

    #[test]
    fn test_encode_decode_roundtrip_preserves_document() {
        let original = sample_doc();
        let json = encode(&original).unwrap();
        let decoded = decode(&json).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_encode_emits_pretty_json_with_expected_fields() {
        let json = encode(&sample_doc()).unwrap();
        assert!(json.contains("\"schema_version\": 2"));
        assert!(json.contains("\"generated_at\": \"2026-04-18T12:00:00Z\""));
        assert!(json.contains(&format!("\"declaration_hash\": \"{DIGEST}\"")));
        assert!(json.contains("\"signal\": \"blue\""));
    }

    #[test]
    fn test_encode_rejects_unsupported_schema_version() {
        let original = sample_doc();
        let document = TypeSignalsDocument::with_schema_version(
            TypeSignalsSchemaVersion::try_new(1).unwrap(),
            original.generated_at().clone(),
            original.freshness().clone(),
            original.signals().to_vec(),
        );

        assert!(matches!(
            encode(&document),
            Err(TypeSignalsCodecError::UnsupportedSchemaVersion(version)) if version.value() == 1
        ));
    }

    #[test]
    fn test_encode_freshness_digest_matches_actual_input_hash() {
        let actual_input = ContentHash::from_bytes([0xbb; 32]);
        let digest = Sha256Digest::from_content_hash(actual_input);
        let document = TypeSignalsDocument::new(
            ts("2026-04-18T12:00:00Z"),
            TypeSignalsFreshness::new(
                CatalogueDeclarationHash::new(digest.clone()),
                ImplementationInputHash::new(digest.clone()),
                BaselineHash::new(digest.clone()),
                LiveRustdocSnapshotHash::new(digest.clone()),
                EvaluatorContractHash::new(digest.clone()),
                RustdocExtractionContractHash::new(digest.clone()),
            ),
            vec![],
        );

        let artifact: serde_json::Value =
            serde_json::from_str(&encode(&document).unwrap()).unwrap();

        assert_eq!(
            digest.as_str(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        );
        assert_eq!(artifact["live_rustdoc_snapshot_hash"], digest.as_str());
    }

    #[test]
    fn test_decode_accepts_minimal_valid_payload() {
        let doc = decode(&payload(2, "2026-04-18T12:00:00Z", serde_json::json!([]))).unwrap();
        assert_eq!(doc.schema_version().value(), 2);
        assert_eq!(doc.declaration_hash().as_digest().as_str(), DIGEST);
        assert!(doc.signals().is_empty());
    }

    #[test]
    fn test_decode_accepts_signals_without_optional_item_lists() {
        let doc = decode(&payload(
            2,
            "2026-04-18T12:00:00Z",
            serde_json::json!([
                {"type_name": "A", "kind_tag": "value_object", "signal": "blue", "found_type": true}
            ]),
        ))
        .unwrap();
        assert_eq!(doc.signals().len(), 1);
        assert_eq!(doc.signals()[0].type_name(), "A");
        assert_eq!(doc.signals()[0].signal(), ConfidenceSignal::Blue);
        assert!(doc.signals()[0].found_items().is_empty());
    }

    #[test]
    fn test_decode_maps_yellow_and_red_signal_strings() {
        let doc = decode(&payload(
            2,
            "2026-04-18T12:00:00Z",
            serde_json::json!([
                {"type_name": "Y", "kind_tag": "enum", "signal": "yellow", "found_type": false},
                {"type_name": "R", "kind_tag": "enum", "signal": "red", "found_type": true}
            ]),
        ))
        .unwrap();
        assert_eq!(doc.signals()[0].signal(), ConfidenceSignal::Yellow);
        assert_eq!(doc.signals()[1].signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_decode_normalises_unknown_signal_strings_to_red() {
        let doc = decode(&payload(
            2,
            "2026-04-18T12:00:00Z",
            serde_json::json!([
                {"type_name": "X", "kind_tag": "enum", "signal": "purple", "found_type": true}
            ]),
        ))
        .unwrap();
        assert_eq!(doc.signals()[0].signal(), ConfidenceSignal::Red);
    }

    // --- error paths ---

    #[test]
    fn test_decode_rejects_invalid_json() {
        let result = decode("not json");
        assert!(matches!(result, Err(TypeSignalsCodecError::Json(_))));
    }

    #[test]
    fn test_decode_rejects_missing_signals_field() {
        // `signals` is a required field; omitting it means the file is malformed.
        let json = r#"{
            "schema_version": 1,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": "h"
        }"#;
        let result = decode(json);
        assert!(matches!(result, Err(TypeSignalsCodecError::Json(_))));
    }

    #[test]
    fn test_decode_rejects_schema_v2_artifact_missing_required_freshness_hash() {
        for field in [
            "implementation_input_hash",
            "baseline_hash",
            "live_rustdoc_snapshot_hash",
            "evaluator_contract_hash",
            "rustdoc_extraction_contract_hash",
        ] {
            let mut value: serde_json::Value =
                serde_json::from_str(&payload(2, "2026-04-18T12:00:00Z", serde_json::json!([])))
                    .unwrap();
            value.as_object_mut().unwrap().remove(field).unwrap();

            assert!(
                matches!(decode(&value.to_string()), Err(TypeSignalsCodecError::Json(_))),
                "schema-v2 artifact missing {field} must fail closed"
            );
        }
    }

    #[test]
    fn test_decode_rejects_schema_v2_artifact_malformed_freshness_digest_values() {
        for (field, value, expected_error) in [
            ("implementation_input_hash", "short".to_owned(), Sha256DigestError::InvalidLength),
            ("live_rustdoc_snapshot_hash", DIGEST.to_uppercase(), Sha256DigestError::InvalidHex),
        ] {
            let mut artifact: serde_json::Value =
                serde_json::from_str(&payload(2, "2026-04-18T12:00:00Z", serde_json::json!([])))
                    .unwrap();
            artifact[field] = serde_json::Value::String(value);

            assert!(
                matches!(
                    decode(&artifact.to_string()),
                    Err(TypeSignalsCodecError::InvalidDigest { field: actual_field, source })
                        if actual_field.as_str() == field && source == expected_error
                ),
                "schema-v2 artifact with malformed {field} must fail as {expected_error:?}"
            );
        }
    }

    #[test]
    fn test_decode_rejects_unknown_top_level_field() {
        let json = r#"{
            "schema_version": 1,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": "h",
            "signals": [],
            "extra_field": "not allowed"
        }"#;
        let result = decode(json);
        assert!(matches!(result, Err(TypeSignalsCodecError::Json(_))));
    }

    #[test]
    fn test_decode_rejects_schema_version_zero() {
        let result = decode(&payload(0, "2026-04-18T12:00:00Z", serde_json::json!([])));
        assert!(matches!(result, Err(TypeSignalsCodecError::InvalidSchemaVersion(_))));
    }

    #[test]
    fn test_decode_rejects_schema_version_two() {
        let result = decode(&payload(1, "2026-04-18T12:00:00Z", serde_json::json!([])));
        assert!(
            matches!(result, Err(TypeSignalsCodecError::UnsupportedSchemaVersion(version)) if version.value() == 1)
        );
    }

    #[test]
    fn test_decode_rejects_invalid_timestamp() {
        let result = decode(&payload(2, "not-a-timestamp", serde_json::json!([])));
        assert!(matches!(result, Err(TypeSignalsCodecError::InvalidTimestamp(_))));
    }

    #[test]
    fn test_decode_rejects_non_utc_timestamp() {
        // +09:00 parses as a valid RFC 3339 timestamp but violates the UTC contract.
        let result = decode(&payload(2, "2026-04-18T12:00:00+09:00", serde_json::json!([])));
        assert!(matches!(result, Err(TypeSignalsCodecError::InvalidTimestamp(_))));
    }

    #[test]
    fn test_decode_accepts_utc_plus00_notation() {
        // +00:00 is a valid UTC representation (equivalent to Z).
        let doc = decode(&payload(2, "2026-04-18T12:00:00+00:00", serde_json::json!([]))).unwrap();
        assert_eq!(doc.schema_version().value(), 2);
    }

    // --- declaration_hash ---

    #[test]
    fn test_declaration_hash_of_empty_bytes_is_known_sha256() {
        let hash = declaration_hash(b"");
        assert_eq!(
            hash.as_digest().as_str(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_declaration_hash_of_known_string_matches_sha256() {
        let hash = declaration_hash(b"abc");
        assert_eq!(
            hash.as_digest().as_str(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_declaration_hash_is_deterministic() {
        let a = declaration_hash(b"hello world");
        let b = declaration_hash(b"hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn test_declaration_hash_differs_on_different_bytes() {
        let a = declaration_hash(b"hello");
        let b = declaration_hash(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn test_declaration_hash_is_64_hex_chars() {
        let hash = declaration_hash(b"any bytes here");
        assert_eq!(hash.as_digest().as_str().len(), 64);
        assert!(
            hash.as_digest()
                .as_str()
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }
}
