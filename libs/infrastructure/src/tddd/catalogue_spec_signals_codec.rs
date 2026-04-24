//! Serde codec for `<layer>-catalogue-spec-signals.json` (schema_version 1).
//!
//! Companion to `type_signals_codec.rs`. Where `type_signals_codec` handles
//! the TDDD-02 evaluation result file (SoT Chain ③), this module handles the
//! TDDD-03 catalogue-spec signal evaluation result (SoT Chain ②) introduced
//! by ADR `2026-04-23-0344-catalogue-spec-signal-activation.md` §D2.
//!
//! # Responsibility split
//!
//! - `encode(&CatalogueSpecSignalsDocument) -> Result<String, _>` emits JSON
//!   suitable for writing to `<layer>-catalogue-spec-signals.json`. The output
//!   is deterministic: no `generated_at` wall-clock, signal order preserved.
//! - `decode(&str) -> Result<CatalogueSpecSignalsDocument, _>` parses the same
//!   file back. Rejects unknown schema versions, unknown fields
//!   (`deny_unknown_fields` at every nesting), malformed hex hashes, and
//!   unknown `signal` strings.
//!
//! No filesystem I/O lives here — callers (CLI writer, verify reader) handle
//! `std::fs` and the `reject_symlinks_below` guard.

use domain::{
    CATALOGUE_SPEC_SIGNALS_SCHEMA_VERSION, CatalogueSpecSignal, CatalogueSpecSignalsDocument,
    ConfidenceSignal, ContentHash, ValidationError,
};
use serde::{Deserialize, Serialize};

/// Codec error for `<layer>-catalogue-spec-signals.json`.
#[derive(Debug, thiserror::Error)]
pub enum CatalogueSpecSignalsCodecError {
    /// The payload is not valid JSON or fails DTO deserialization (including
    /// `deny_unknown_fields` rejections at any nesting level).
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// `schema_version` is not 1. The ADR pins the format at 1; any future
    /// incompatible change must bump this version through an ADR amendment.
    #[error(
        "unsupported schema_version: expected 1, got {0}. \
         Re-run `sotp track catalogue-spec-signals` with the current sotp build to \
         regenerate the signals file (ADR 2026-04-23-0344 §D2.2 pins schema_version=1)."
    )]
    UnsupportedSchemaVersion(u32),

    /// A field value is invalid (e.g. `catalogue_declaration_hash` is not a
    /// canonical 64-character lowercase hex string, or a signal variant
    /// string is not one of `blue` / `yellow` / `red`).
    #[error("validation error: {0}")]
    Validation(String),
}

impl From<ValidationError> for CatalogueSpecSignalsCodecError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value.to_string())
    }
}

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// Serde DTO mirroring [`CatalogueSpecSignalsDocument`] for JSON round-trip.
///
/// `#[serde(deny_unknown_fields)]` is applied at every nesting level so any
/// unrecognised field — top-level or inside `signals[]` — fails closed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogueSpecSignalsDocumentDto {
    pub schema_version: u32,
    pub catalogue_declaration_hash: String,
    pub(crate) signals: Vec<CatalogueSpecSignalDto>,
}

/// Serde DTO for a single per-entry signal record.
///
/// Visibility is `pub(crate)` — `CatalogueSpecSignalDto` is an internal
/// shape of the codec layer, not part of the catalogue's declared public
/// API. Only the aggregate [`CatalogueSpecSignalsDocumentDto`] is exposed
/// as a catalogue entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CatalogueSpecSignalDto {
    pub(crate) type_name: String,
    pub(crate) signal: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Decodes a `<layer>-catalogue-spec-signals.json` string into a
/// `CatalogueSpecSignalsDocument`.
///
/// # Errors
///
/// - `Json` when the input is not valid JSON or contains unknown fields at
///   any nesting level.
/// - `UnsupportedSchemaVersion` when `schema_version != 1`.
/// - `Validation` when `catalogue_declaration_hash` is not a canonical
///   64-character lowercase hex string, or a `signal` string is not one of
///   `blue` / `yellow` / `red`.
pub fn decode(json: &str) -> Result<CatalogueSpecSignalsDocument, CatalogueSpecSignalsCodecError> {
    let dto: CatalogueSpecSignalsDocumentDto = serde_json::from_str(json)?;
    if dto.schema_version != CATALOGUE_SPEC_SIGNALS_SCHEMA_VERSION {
        return Err(CatalogueSpecSignalsCodecError::UnsupportedSchemaVersion(dto.schema_version));
    }
    let catalogue_declaration_hash = ContentHash::try_from_hex(&dto.catalogue_declaration_hash)?;
    let signals = dto
        .signals
        .into_iter()
        .map(signal_from_dto)
        .collect::<Result<Vec<_>, CatalogueSpecSignalsCodecError>>()?;
    Ok(CatalogueSpecSignalsDocument::new(catalogue_declaration_hash, signals))
}

/// Encodes a `CatalogueSpecSignalsDocument` into a pretty-printed JSON
/// string.
///
/// Deterministic output per ADR §D2.2: no `generated_at`, no wall-clock
/// field. Given identical input, the output is byte-identical across runs.
///
/// # Errors
///
/// Returns `CatalogueSpecSignalsCodecError::Json` if serialization fails.
pub fn encode(
    doc: &CatalogueSpecSignalsDocument,
) -> Result<String, CatalogueSpecSignalsCodecError> {
    let dto = CatalogueSpecSignalsDocumentDto {
        schema_version: CATALOGUE_SPEC_SIGNALS_SCHEMA_VERSION,
        catalogue_declaration_hash: doc.catalogue_declaration_hash.to_hex(),
        signals: doc.signals.iter().map(signal_to_dto).collect(),
    };
    Ok(serde_json::to_string_pretty(&dto)?)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn signal_from_dto(
    dto: CatalogueSpecSignalDto,
) -> Result<CatalogueSpecSignal, CatalogueSpecSignalsCodecError> {
    let signal = match dto.signal.as_str() {
        "blue" => ConfidenceSignal::Blue,
        "yellow" => ConfidenceSignal::Yellow,
        "red" => ConfidenceSignal::Red,
        other => {
            return Err(CatalogueSpecSignalsCodecError::Validation(format!(
                "unknown signal variant '{other}' (expected 'blue', 'yellow', or 'red')"
            )));
        }
    };
    Ok(CatalogueSpecSignal::new(dto.type_name, signal))
}

fn signal_to_dto(signal: &CatalogueSpecSignal) -> CatalogueSpecSignalDto {
    let signal_str = match signal.signal {
        ConfidenceSignal::Blue => "blue",
        ConfidenceSignal::Yellow => "yellow",
        ConfidenceSignal::Red => "red",
        _ => "red", // future-proofing: unknown variants default to the most conservative state
    };
    CatalogueSpecSignalDto { type_name: signal.type_name.clone(), signal: signal_str.to_owned() }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn hex_pattern(byte: u8) -> String {
        let mut s = String::with_capacity(64);
        for _ in 0..32 {
            s.push_str(&format!("{byte:02x}"));
        }
        s
    }

    fn sample_doc() -> CatalogueSpecSignalsDocument {
        CatalogueSpecSignalsDocument::new(
            ContentHash::try_from_hex(hex_pattern(0xab)).unwrap(),
            vec![
                CatalogueSpecSignal::new("Foo", ConfidenceSignal::Blue),
                CatalogueSpecSignal::new("Bar", ConfidenceSignal::Yellow),
                CatalogueSpecSignal::new("Baz", ConfidenceSignal::Red),
            ],
        )
    }

    // --- round-trip ---

    #[test]
    fn encode_decode_roundtrip_preserves_document() {
        let original = sample_doc();
        let json = encode(&original).unwrap();
        let decoded = decode(&json).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn encode_output_is_deterministic_across_runs() {
        let doc = sample_doc();
        let a = encode(&doc).unwrap();
        let b = encode(&doc).unwrap();
        assert_eq!(a, b, "encode() must be deterministic (CN-06 / §D2.2)");
    }

    #[test]
    fn encode_output_lacks_generated_at_field() {
        let json = encode(&sample_doc()).unwrap();
        assert!(
            !json.contains("generated_at"),
            "CatalogueSpecSignalsDocument must not emit generated_at (CN-06)"
        );
    }

    #[test]
    fn encode_emits_all_expected_fields() {
        let json = encode(&sample_doc()).unwrap();
        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains("\"catalogue_declaration_hash\""));
        assert!(json.contains("\"signals\""));
        assert!(json.contains("\"type_name\""));
        assert!(json.contains("\"signal\""));
    }

    // --- schema version ---

    #[test]
    fn decode_rejects_unsupported_schema_version() {
        let json = format!(
            r#"{{
              "schema_version": 2,
              "catalogue_declaration_hash": "{}",
              "signals": []
            }}"#,
            hex_pattern(0x00)
        );
        let err = decode(&json).unwrap_err();
        assert!(matches!(err, CatalogueSpecSignalsCodecError::UnsupportedSchemaVersion(2)));
    }

    // --- deny_unknown_fields ---

    #[test]
    fn decode_rejects_unknown_top_level_field() {
        let json = format!(
            r#"{{
              "schema_version": 1,
              "catalogue_declaration_hash": "{}",
              "signals": [],
              "extra_field": "not allowed"
            }}"#,
            hex_pattern(0x00)
        );
        let err = decode(&json).unwrap_err();
        assert!(matches!(err, CatalogueSpecSignalsCodecError::Json(_)));
    }

    #[test]
    fn decode_rejects_unknown_nested_field_in_signal() {
        let json = format!(
            r#"{{
              "schema_version": 1,
              "catalogue_declaration_hash": "{}",
              "signals": [
                {{"type_name": "Foo", "signal": "blue", "extra": "bad"}}
              ]
            }}"#,
            hex_pattern(0x00)
        );
        let err = decode(&json).unwrap_err();
        assert!(matches!(err, CatalogueSpecSignalsCodecError::Json(_)));
    }

    // --- hash validation ---

    #[test]
    fn decode_rejects_malformed_catalogue_declaration_hash() {
        let json = r#"{
          "schema_version": 1,
          "catalogue_declaration_hash": "not-a-valid-hex",
          "signals": []
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, CatalogueSpecSignalsCodecError::Validation(_)));
    }

    #[test]
    fn decode_rejects_uppercase_hex() {
        let upper = "A".repeat(64);
        let json = format!(
            r#"{{
              "schema_version": 1,
              "catalogue_declaration_hash": "{upper}",
              "signals": []
            }}"#
        );
        let err = decode(&json).unwrap_err();
        assert!(matches!(err, CatalogueSpecSignalsCodecError::Validation(_)));
    }

    // --- signal variant ---

    #[test]
    fn decode_rejects_unknown_signal_variant() {
        let json = format!(
            r#"{{
              "schema_version": 1,
              "catalogue_declaration_hash": "{}",
              "signals": [
                {{"type_name": "Foo", "signal": "pink"}}
              ]
            }}"#,
            hex_pattern(0x00)
        );
        let err = decode(&json).unwrap_err();
        match err {
            CatalogueSpecSignalsCodecError::Validation(msg) => {
                assert!(msg.contains("pink"));
                assert!(msg.contains("blue"));
            }
            other => panic!("expected Validation, got {other:?}"),
        }
    }

    #[test]
    fn decode_accepts_all_three_signal_variants() {
        let json = format!(
            r#"{{
              "schema_version": 1,
              "catalogue_declaration_hash": "{}",
              "signals": [
                {{"type_name": "A", "signal": "blue"}},
                {{"type_name": "B", "signal": "yellow"}},
                {{"type_name": "C", "signal": "red"}}
              ]
            }}"#,
            hex_pattern(0x11)
        );
        let doc = decode(&json).unwrap();
        assert_eq!(doc.signals.len(), 3);
        assert_eq!(doc.signals[0].signal, ConfidenceSignal::Blue);
        assert_eq!(doc.signals[1].signal, ConfidenceSignal::Yellow);
        assert_eq!(doc.signals[2].signal, ConfidenceSignal::Red);
    }

    // --- missing required fields ---

    #[test]
    fn decode_rejects_missing_schema_version() {
        let json = format!(
            r#"{{
              "catalogue_declaration_hash": "{}",
              "signals": []
            }}"#,
            hex_pattern(0x00)
        );
        let err = decode(&json).unwrap_err();
        assert!(matches!(err, CatalogueSpecSignalsCodecError::Json(_)));
    }

    #[test]
    fn decode_rejects_missing_signals_array() {
        let json = format!(
            r#"{{
              "schema_version": 1,
              "catalogue_declaration_hash": "{}"
            }}"#,
            hex_pattern(0x00)
        );
        let err = decode(&json).unwrap_err();
        assert!(matches!(err, CatalogueSpecSignalsCodecError::Json(_)));
    }

    #[test]
    fn encode_preserves_signal_order() {
        let doc = CatalogueSpecSignalsDocument::new(
            ContentHash::try_from_hex(hex_pattern(0x00)).unwrap(),
            vec![
                CatalogueSpecSignal::new("Gamma", ConfidenceSignal::Blue),
                CatalogueSpecSignal::new("Alpha", ConfidenceSignal::Yellow),
                CatalogueSpecSignal::new("Beta", ConfidenceSignal::Red),
            ],
        );
        let json = encode(&doc).unwrap();
        let gamma = json.find("Gamma").expect("Gamma should appear");
        let alpha = json.find("Alpha").expect("Alpha should appear");
        let beta = json.find("Beta").expect("Beta should appear");
        assert!(gamma < alpha);
        assert!(alpha < beta);
    }
}
