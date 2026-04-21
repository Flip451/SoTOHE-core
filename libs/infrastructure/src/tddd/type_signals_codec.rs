//! Serde codec for the per-layer TDDD evaluation-result file
//! (`<layer>-type-signals.json`, schema_version 1).
//!
//! Companion to `catalogue_codec.rs`. The declaration file (`<layer>-types.json`)
//! stores authored type declarations; this module handles the generated
//! evaluation-result file introduced by
//! `knowledge/adr/2026-04-18-1400-tddd-ci-gate-and-signals-separation.md` §D1.
//!
//! # Responsibility split
//!
//! - `encode(&TypeSignalsDocument) -> Result<String, _>` emits JSON suitable
//!   for writing to `<layer>-type-signals.json`.
//! - `decode(&str) -> Result<TypeSignalsDocument, _>` parses the same file back
//!   and rejects unknown schema versions / unknown fields / unparseable
//!   timestamps. Unknown `signal` strings are normalised to `Red` (consistent
//!   with `catalogue_codec`).
//! - `declaration_hash(bytes: &[u8]) -> String` computes the SHA-256 hex
//!   digest of the declaration file bytes *as written to disk* (post-encode).
//!   The algorithm is pinned at schema_version 1 per
//!   ADR §D5 and the `declaration_hash` algorithm documentation on
//!   `TypeSignalsCodecError::UnsupportedSchemaVersion`.
//!
//! No filesystem I/O lives here — callers (CLI writer, CI reader) handle
//! `std::fs` and the `reject_symlinks_below` guard.

use domain::tddd::type_signals_doc::TypeSignalsDocument;
use domain::{ConfidenceSignal, Timestamp, TypeSignal};
use serde::{Deserialize, Serialize};
use sha2::Digest;

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
        "unsupported schema_version: expected 1, got {0}. \
         Re-run `sotp track type-signals` with the current sotp build to \
         regenerate the signal file (declaration_hash algorithm is pinned at \
         schema_version 1: raw SHA-256 of declaration file bytes post-encode)."
    )]
    UnsupportedSchemaVersion(u32),

    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),
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
    if dto.schema_version != 1 {
        return Err(TypeSignalsCodecError::UnsupportedSchemaVersion(dto.schema_version));
    }
    let generated_at = Timestamp::new(dto.generated_at.clone())
        .map_err(|_| TypeSignalsCodecError::InvalidTimestamp(dto.generated_at.clone()))?;
    // Enforce UTC-only: the on-disk format requires a UTC offset (`Z` or `+00:00`).
    // Non-UTC offsets (e.g. `+09:00`) parse successfully in `Timestamp::new` but violate
    // the ADR §D1 contract, which specifies `generated_at` as an ISO 8601 UTC timestamp.
    if !is_utc_timestamp(dto.generated_at.as_str()) {
        return Err(TypeSignalsCodecError::InvalidTimestamp(dto.generated_at));
    }
    let signals = dto.signals.into_iter().map(signal_from_dto).collect();
    Ok(TypeSignalsDocument::with_schema_version(
        dto.schema_version,
        generated_at,
        dto.declaration_hash,
        signals,
    ))
}

/// Encodes a `TypeSignalsDocument` into a pretty-printed JSON string.
///
/// The output is deterministic for a given document: serde_json preserves
/// the signal order from the document, and the DTO field order is fixed by
/// the struct layout.
///
/// # Errors
///
/// Returns `TypeSignalsCodecError::Json` if serialization fails (should not
/// happen for the DTO types used here; this is defensive for future changes).
pub fn encode(doc: &TypeSignalsDocument) -> Result<String, TypeSignalsCodecError> {
    let dto = TypeSignalsDocDto {
        // Always emit schema_version 1, regardless of the in-memory value, so that
        // encode→decode round-trips correctly. Schema_version 1 is the only version
        // this codec can decode; the in-memory field is an informational tag for
        // diagnostics only.
        schema_version: 1,
        generated_at: doc.generated_at().as_str().to_owned(),
        declaration_hash: doc.declaration_hash().to_owned(),
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
#[must_use]
pub fn declaration_hash(declaration_bytes: &[u8]) -> String {
    let digest = sha2::Sha256::digest(declaration_bytes);
    format!("{digest:x}")
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

fn signal_from_dto(dto: TypeSignalDto) -> TypeSignal {
    let signal = match dto.signal.as_str() {
        "blue" => ConfidenceSignal::Blue,
        "yellow" => ConfidenceSignal::Yellow,
        _ => ConfidenceSignal::Red,
    };
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
    let signal_str = match signal.signal() {
        ConfidenceSignal::Blue => "blue",
        ConfidenceSignal::Yellow => "yellow",
        ConfidenceSignal::Red => "red",
        _ => "unknown",
    };
    TypeSignalDto {
        type_name: signal.type_name().to_owned(),
        kind_tag: signal.kind_tag().to_owned(),
        signal: signal_str.to_owned(),
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

    fn ts(raw: &str) -> Timestamp {
        Timestamp::new(raw).unwrap()
    }

    fn sample_signal_blue(name: &str) -> TypeSignal {
        TypeSignal::new(name, "value_object", ConfidenceSignal::Blue, true, vec![], vec![], vec![])
    }

    fn sample_doc() -> TypeSignalsDocument {
        TypeSignalsDocument::new(
            ts("2026-04-18T12:00:00Z"),
            "abc123",
            vec![sample_signal_blue("Foo")],
        )
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
        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains("\"generated_at\": \"2026-04-18T12:00:00Z\""));
        assert!(json.contains("\"declaration_hash\": \"abc123\""));
        assert!(json.contains("\"signal\": \"blue\""));
    }

    #[test]
    fn test_decode_accepts_minimal_valid_payload() {
        let json = r#"{
            "schema_version": 1,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": "h",
            "signals": []
        }"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.schema_version(), 1);
        assert_eq!(doc.declaration_hash(), "h");
        assert!(doc.signals().is_empty());
    }

    #[test]
    fn test_decode_accepts_signals_without_optional_item_lists() {
        let json = r#"{
            "schema_version": 1,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": "h",
            "signals": [
                {"type_name": "A", "kind_tag": "value_object", "signal": "blue", "found_type": true}
            ]
        }"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.signals().len(), 1);
        assert_eq!(doc.signals()[0].type_name(), "A");
        assert_eq!(doc.signals()[0].signal(), ConfidenceSignal::Blue);
        assert!(doc.signals()[0].found_items().is_empty());
    }

    #[test]
    fn test_decode_maps_yellow_and_red_signal_strings() {
        let json = r#"{
            "schema_version": 1,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": "h",
            "signals": [
                {"type_name": "Y", "kind_tag": "enum", "signal": "yellow", "found_type": false},
                {"type_name": "R", "kind_tag": "enum", "signal": "red", "found_type": true}
            ]
        }"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.signals()[0].signal(), ConfidenceSignal::Yellow);
        assert_eq!(doc.signals()[1].signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_decode_normalises_unknown_signal_strings_to_red() {
        let json = r#"{
            "schema_version": 1,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": "h",
            "signals": [
                {"type_name": "X", "kind_tag": "enum", "signal": "purple", "found_type": true}
            ]
        }"#;
        let doc = decode(json).unwrap();
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
        let json = r#"{
            "schema_version": 0,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": "h",
            "signals": []
        }"#;
        let result = decode(json);
        assert!(matches!(result, Err(TypeSignalsCodecError::UnsupportedSchemaVersion(0))));
    }

    #[test]
    fn test_decode_rejects_schema_version_two() {
        let json = r#"{
            "schema_version": 2,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": "h",
            "signals": []
        }"#;
        let result = decode(json);
        assert!(matches!(result, Err(TypeSignalsCodecError::UnsupportedSchemaVersion(2))));
    }

    #[test]
    fn test_decode_rejects_invalid_timestamp() {
        let json = r#"{
            "schema_version": 1,
            "generated_at": "not-a-timestamp",
            "declaration_hash": "h",
            "signals": []
        }"#;
        let result = decode(json);
        assert!(matches!(result, Err(TypeSignalsCodecError::InvalidTimestamp(_))));
    }

    #[test]
    fn test_decode_rejects_non_utc_timestamp() {
        // +09:00 parses as a valid RFC 3339 timestamp but violates the UTC contract.
        let json = r#"{
            "schema_version": 1,
            "generated_at": "2026-04-18T12:00:00+09:00",
            "declaration_hash": "h",
            "signals": []
        }"#;
        let result = decode(json);
        assert!(matches!(result, Err(TypeSignalsCodecError::InvalidTimestamp(_))));
    }

    #[test]
    fn test_decode_accepts_utc_plus00_notation() {
        // +00:00 is a valid UTC representation (equivalent to Z).
        let json = r#"{
            "schema_version": 1,
            "generated_at": "2026-04-18T12:00:00+00:00",
            "declaration_hash": "h",
            "signals": []
        }"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.schema_version(), 1);
    }

    // --- declaration_hash ---

    #[test]
    fn test_declaration_hash_of_empty_bytes_is_known_sha256() {
        let hash = declaration_hash(b"");
        assert_eq!(hash, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn test_declaration_hash_of_known_string_matches_sha256() {
        let hash = declaration_hash(b"abc");
        assert_eq!(hash, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
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
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}
