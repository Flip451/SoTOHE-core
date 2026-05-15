//! Shared DTO types and encode/decode helpers for `spec_refs[]` and
//! `informal_grounds[]` grounding fields in catalogue entry types.
//!
//! These DTOs and helpers are used by the v3 catalogue codec
//! (`catalogue_document_codec`): its type / trait / function entry DTOs all
//! carry these grounding fields, and the `decode` / `encode` submodules use
//! the conversion helpers. They are factored into this module so the same wire
//! format is reused across the codec's entry-DTO submodules.
//!
//! The encode helpers produce `Vec<SpecRefDto>` / `Vec<InformalGroundRefDto>`
//! from domain types; the decode helpers invert the transformation with error
//! context from the containing entry name.

use std::path::PathBuf;

use domain::{
    ContentHash, InformalGroundKind, InformalGroundRef, InformalGroundSummary, SpecElementId,
    SpecRef,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Error helper type
// ---------------------------------------------------------------------------

/// Decode error context for grounding fields.
///
/// Callers convert this to their own error type via the supplied entry name.
#[derive(Debug)]
pub(crate) struct GroundingDecodeError {
    pub(crate) field: &'static str,
    pub(crate) reason: String,
}

// ---------------------------------------------------------------------------
// DTO types
// ---------------------------------------------------------------------------

/// DTO for a single `SpecRef` (SoT Chain ② — catalogue→spec).
///
/// `deny_unknown_fields` ensures stale field names are caught at decode time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpecRefDto {
    /// Relative path to the referenced spec.json file.
    pub(crate) file: String,
    /// Spec element identifier (e.g. `"IN-01"`, `"AC-02"`).
    pub(crate) anchor: String,
    /// 64-character lowercase SHA-256 hex of the canonical JSON subtree.
    pub(crate) hash: String,
}

/// DTO for a single `InformalGroundRef` (unpersisted rationale).
///
/// `deny_unknown_fields` ensures stale field names are caught at decode time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InformalGroundRefDto {
    /// Kind tag: `"discussion"`, `"feedback"`, `"memory"`, or `"user_directive"`.
    pub(crate) kind: String,
    /// Single-line summary of the rationale.
    pub(crate) summary: String,
}

// ---------------------------------------------------------------------------
// Decode helpers
// ---------------------------------------------------------------------------

/// Decode `Vec<SpecRefDto>` → `Vec<SpecRef>`.
///
/// Returns `Err(GroundingDecodeError)` if any DTO fails validation.
/// Enforces the unique-anchor invariant: duplicate `anchor` values are rejected
/// at the codec boundary rather than propagated to downstream verifier paths.
pub(crate) fn spec_refs_from_dtos(
    dtos: &[SpecRefDto],
) -> Result<Vec<SpecRef>, GroundingDecodeError> {
    let mut out = Vec::with_capacity(dtos.len());
    let mut seen_anchors = std::collections::HashSet::new();
    for dto in dtos {
        let anchor = SpecElementId::try_new(&dto.anchor).map_err(|_| GroundingDecodeError {
            field: "spec_refs[].anchor",
            reason: format!(
                "'{}' is not a valid SpecElementId \
                 (expected pattern: <UPPER>{{2,}}-<digits>+)",
                dto.anchor
            ),
        })?;
        if !seen_anchors.insert(anchor.as_ref().to_owned()) {
            return Err(GroundingDecodeError {
                field: "spec_refs[].anchor",
                reason: format!("duplicate anchor '{}' in spec_refs[]", anchor.as_ref()),
            });
        }
        let hash = ContentHash::try_from_hex(&dto.hash).map_err(|_| GroundingDecodeError {
            field: "spec_refs[].hash",
            reason: format!(
                "'{}' is not a valid SHA-256 hex string \
                     (expected 64 lowercase hex characters)",
                dto.hash
            ),
        })?;
        out.push(SpecRef::new(PathBuf::from(&dto.file), anchor, hash));
    }
    Ok(out)
}

/// Decode `Vec<InformalGroundRefDto>` → `Vec<InformalGroundRef>`.
///
/// Returns `Err(GroundingDecodeError)` if any DTO fails validation.
pub(crate) fn informal_grounds_from_dtos(
    dtos: &[InformalGroundRefDto],
) -> Result<Vec<InformalGroundRef>, GroundingDecodeError> {
    let mut out = Vec::with_capacity(dtos.len());
    for dto in dtos {
        let kind =
            informal_ground_kind_from_str(&dto.kind).ok_or_else(|| GroundingDecodeError {
                field: "informal_grounds[].kind",
                reason: format!(
                    "'{}' is not a valid InformalGroundKind \
                 (expected one of: discussion, feedback, memory, user_directive)",
                    dto.kind
                ),
            })?;
        let summary =
            InformalGroundSummary::try_new(&dto.summary).map_err(|_| GroundingDecodeError {
                field: "informal_grounds[].summary",
                reason: format!("'{}' is invalid (must be non-empty and single-line)", dto.summary),
            })?;
        out.push(InformalGroundRef::new(kind, summary));
    }
    Ok(out)
}

/// Parse an `InformalGroundKind` from its string representation.
fn informal_ground_kind_from_str(s: &str) -> Option<InformalGroundKind> {
    match s {
        "discussion" => Some(InformalGroundKind::Discussion),
        "feedback" => Some(InformalGroundKind::Feedback),
        "memory" => Some(InformalGroundKind::Memory),
        "user_directive" => Some(InformalGroundKind::UserDirective),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Encode helpers
// ---------------------------------------------------------------------------

/// Encode `&[SpecRef]` → `Vec<SpecRefDto>`.
pub(crate) fn spec_refs_to_dtos(refs: &[SpecRef]) -> Vec<SpecRefDto> {
    refs.iter()
        .map(|r| SpecRefDto {
            file: r.file.to_string_lossy().into_owned(),
            anchor: r.anchor.as_ref().to_owned(),
            hash: r.hash.to_hex(),
        })
        .collect()
}

/// Encode `&[InformalGroundRef]` → `Vec<InformalGroundRefDto>`.
pub(crate) fn informal_grounds_to_dtos(grounds: &[InformalGroundRef]) -> Vec<InformalGroundRefDto> {
    grounds
        .iter()
        .map(|g| InformalGroundRefDto {
            kind: g.kind.as_str().to_owned(),
            summary: g.summary.as_ref().to_owned(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn make_valid_hash_str() -> String {
        "0".repeat(64)
    }

    #[test]
    fn test_spec_refs_from_dtos_with_valid_input_succeeds() {
        let dtos = vec![SpecRefDto {
            file: "track/items/x/spec.json".to_owned(),
            anchor: "IN-01".to_owned(),
            hash: make_valid_hash_str(),
        }];
        let result = spec_refs_from_dtos(&dtos).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].anchor.as_ref(), "IN-01");
        assert_eq!(result[0].file, PathBuf::from("track/items/x/spec.json"));
    }

    #[test]
    fn test_spec_refs_from_dtos_with_invalid_anchor_returns_error() {
        let dtos = vec![SpecRefDto {
            file: "spec.json".to_owned(),
            anchor: "bad-anchor".to_owned(),
            hash: make_valid_hash_str(),
        }];
        let result = spec_refs_from_dtos(&dtos);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.field, "spec_refs[].anchor");
    }

    #[test]
    fn test_spec_refs_from_dtos_with_invalid_hash_returns_error() {
        let dtos = vec![SpecRefDto {
            file: "spec.json".to_owned(),
            anchor: "IN-01".to_owned(),
            hash: "notahexhash".to_owned(),
        }];
        let result = spec_refs_from_dtos(&dtos);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.field, "spec_refs[].hash");
    }

    #[test]
    fn test_spec_refs_from_dtos_empty_input_returns_empty_vec() {
        let result = spec_refs_from_dtos(&[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_informal_grounds_from_dtos_with_valid_input_succeeds() {
        let dtos = vec![InformalGroundRefDto {
            kind: "discussion".to_owned(),
            summary: "planning session note".to_owned(),
        }];
        let result = informal_grounds_from_dtos(&dtos).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].kind, InformalGroundKind::Discussion);
    }

    #[test]
    fn test_informal_grounds_from_dtos_with_invalid_kind_returns_error() {
        let dtos = vec![InformalGroundRefDto {
            kind: "invalid_kind".to_owned(),
            summary: "some note".to_owned(),
        }];
        let result = informal_grounds_from_dtos(&dtos);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.field, "informal_grounds[].kind");
    }

    #[test]
    fn test_informal_grounds_from_dtos_with_empty_summary_returns_error() {
        let dtos =
            vec![InformalGroundRefDto { kind: "feedback".to_owned(), summary: "".to_owned() }];
        let result = informal_grounds_from_dtos(&dtos);
        assert!(result.is_err());
    }

    #[test]
    fn test_spec_refs_round_trip_encode_decode() {
        let anchor = SpecElementId::try_new("AC-02").unwrap();
        let hash = ContentHash::from_bytes([0xabu8; 32]);
        let spec_ref =
            SpecRef::new(PathBuf::from("track/items/x/spec.json"), anchor.clone(), hash.clone());

        let dtos = spec_refs_to_dtos(std::slice::from_ref(&spec_ref));
        let decoded = spec_refs_from_dtos(&dtos).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0], spec_ref);
    }

    #[test]
    fn test_informal_grounds_round_trip_encode_decode() {
        let summary = InformalGroundSummary::try_new("user directive to defer anchor").unwrap();
        let ground = InformalGroundRef::new(InformalGroundKind::UserDirective, summary);

        let dtos = informal_grounds_to_dtos(std::slice::from_ref(&ground));
        let decoded = informal_grounds_from_dtos(&dtos).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0], ground);
    }
}
