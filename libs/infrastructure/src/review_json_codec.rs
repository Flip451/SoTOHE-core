//! Codec for review.json (schema_version 1, cycle-based format).
//!
//! Maps between domain types (`ReviewJson`, `ReviewCycle`, etc.) and
//! serde-annotated DTOs for JSON serialization. Domain types have no
//! serde derives per hexagonal architecture convention.

use std::collections::{BTreeMap, BTreeSet};

use domain::{
    CycleError, CycleGroupState, GroupRound, GroupRoundOutcome, GroupRoundVerdict, ReviewConcern,
    ReviewCycle, ReviewGroupName, ReviewJson, RoundType, StoredFinding, Timestamp,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors during review.json codec operations.
#[derive(Debug, thiserror::Error)]
pub enum ReviewJsonCodecError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("domain error: {0}")]
    Cycle(#[from] CycleError),
    #[error("invalid field '{field}': {reason}")]
    InvalidField { field: String, reason: String },
}

// ---------------------------------------------------------------------------
// Serde DTOs
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewJsonDocument {
    schema_version: u32,
    cycles: Vec<CycleDocument>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CycleDocument {
    cycle_id: String,
    started_at: String,
    base_ref: String,
    base_policy_hash: String,
    policy_hash: String,
    approved_head: Option<String>,
    groups: BTreeMap<String, GroupDocument>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupDocument {
    scope: Vec<String>,
    rounds: Vec<RoundDocument>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RoundDocument {
    round_type: String,
    success: String,
    error_message: Option<String>,
    timestamp: String,
    hash: String,
    #[serde(default, skip_serializing_if = "ConcernsField::is_missing")]
    concerns: ConcernsField,
    verdict: Option<VerdictDocument>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum ConcernsField {
    #[default]
    Missing,
    Null,
    Present(Vec<ReviewConcern>),
}

impl ConcernsField {
    fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

impl Serialize for ConcernsField {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Missing | Self::Null => serializer.serialize_none(),
            Self::Present(concerns) => concerns
                .iter()
                .map(|concern| concern.as_ref().to_owned())
                .collect::<Vec<_>>()
                .serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ConcernsField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ConcernsFieldVisitor;

        impl<'de> serde::de::Visitor<'de> for ConcernsFieldVisitor {
            type Value = ConcernsField;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("null or an array of concern strings")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(ConcernsField::Null)
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(ConcernsField::Null)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let concerns = Vec::<String>::deserialize(deserializer)?;
                let mut seen = BTreeSet::new();
                let mut parsed = Vec::with_capacity(concerns.len());
                for concern in concerns {
                    let concern =
                        ReviewConcern::try_new(concern).map_err(serde::de::Error::custom)?;
                    if !seen.insert(concern.clone()) {
                        return Err(serde::de::Error::custom(
                            "round concerns must not contain duplicates",
                        ));
                    }
                    parsed.push(concern);
                }
                Ok(ConcernsField::Present(parsed))
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let concerns =
                    Vec::<String>::deserialize(serde::de::value::SeqAccessDeserializer::new(seq))?;
                let mut seen = BTreeSet::new();
                let mut parsed = Vec::with_capacity(concerns.len());
                for concern in concerns {
                    let concern =
                        ReviewConcern::try_new(concern).map_err(serde::de::Error::custom)?;
                    if !seen.insert(concern.clone()) {
                        return Err(serde::de::Error::custom(
                            "round concerns must not contain duplicates",
                        ));
                    }
                    parsed.push(concern);
                }
                Ok(ConcernsField::Present(parsed))
            }
        }

        deserializer.deserialize_option(ConcernsFieldVisitor)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerdictDocument {
    verdict: String,
    findings: Vec<FindingDocument>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FindingDocument {
    message: String,
    severity: Option<String>,
    file: Option<String>,
    line: Option<u64>,
    #[serde(default)]
    category: Option<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Decodes a review.json string into domain types.
///
/// # Errors
/// Returns `ReviewJsonCodecError` on JSON parse failure, invalid field values,
/// or domain validation failure.
pub fn decode(json: &str) -> Result<ReviewJson, ReviewJsonCodecError> {
    let doc: ReviewJsonDocument = serde_json::from_str(json)?;
    let cycles = doc.cycles.into_iter().map(cycle_from_document).collect::<Result<Vec<_>, _>>()?;
    // Validate append-only cycle ordering (non-decreasing started_at)
    validate_cycle_order(&cycles)?;
    let review_json = ReviewJson::from_parts(doc.schema_version, cycles)?;
    Ok(review_json)
}

/// Encodes domain types into a review.json string.
///
/// # Errors
/// Returns `ReviewJsonCodecError` on JSON serialization or ordering validation failure.
pub fn encode(review: &ReviewJson) -> Result<String, ReviewJsonCodecError> {
    // Validate invariants before serialization to prevent persisting invalid state
    validate_cycle_order(review.cycles())?;
    let other_key =
        ReviewGroupName::try_new("other").map_err(|e| ReviewJsonCodecError::InvalidField {
            field: "groups".into(),
            reason: format!("failed to create 'other' key: {e}"),
        })?;
    for cycle in review.cycles() {
        if !cycle.groups().contains_key(&other_key) {
            return Err(CycleError::MissingOtherGroup.into());
        }
        for group in cycle.groups().values() {
            validate_round_order(group.rounds())?;
            for round in group.rounds() {
                if let Some(findings) = round.outcome().verdict().map(|v| v.findings()) {
                    for finding in findings {
                        validate_stored_finding(finding)?;
                    }
                }
            }
        }
    }
    let doc = document_from_review_json(review)?;
    let json = serde_json::to_string_pretty(&doc)?;
    Ok(json)
}

// ---------------------------------------------------------------------------
// Document → Domain
// ---------------------------------------------------------------------------

fn cycle_from_document(doc: CycleDocument) -> Result<ReviewCycle, ReviewJsonCodecError> {
    let started_at = parse_timestamp(&doc.started_at, "started_at")?;
    let original_count = doc.groups.len();
    let mut groups = BTreeMap::new();
    for (name, group_doc) in doc.groups {
        let group_name =
            ReviewGroupName::try_new(&name).map_err(|e| ReviewJsonCodecError::InvalidField {
                field: "groups".into(),
                reason: format!("invalid group name '{name}': {e}"),
            })?;
        let group_state = group_from_document(group_doc)?;
        groups.insert(group_name, group_state);
    }
    // Detect normalization collisions (e.g., "other" vs " other ")
    if groups.len() != original_count {
        return Err(ReviewJsonCodecError::InvalidField {
            field: "groups".into(),
            reason: "group name normalization caused key collision".into(),
        });
    }
    // Parse approved_head if present.
    let approved_head = doc
        .approved_head
        .map(|sha| {
            domain::ApprovedHead::try_new(sha).map_err(|e| ReviewJsonCodecError::InvalidField {
                field: "approved_head".into(),
                reason: format!("{e}"),
            })
        })
        .transpose()?;

    // Use the validated constructor — review.json from disk is untrusted input.
    let mut cycle = ReviewCycle::new(
        doc.cycle_id,
        started_at,
        doc.base_ref,
        doc.base_policy_hash,
        doc.policy_hash,
        groups,
    )
    .map_err(ReviewJsonCodecError::from)?;

    if let Some(head) = approved_head {
        cycle.set_approved_head(head);
    }
    Ok(cycle)
}

fn group_from_document(doc: GroupDocument) -> Result<CycleGroupState, ReviewJsonCodecError> {
    let rounds = doc.rounds.into_iter().map(round_from_document).collect::<Result<Vec<_>, _>>()?;
    validate_round_order(&rounds)?;
    Ok(CycleGroupState::with_rounds(doc.scope, rounds))
}

fn round_from_document(doc: RoundDocument) -> Result<GroupRound, ReviewJsonCodecError> {
    let round_type = parse_round_type(&doc.round_type)?;
    let timestamp = parse_timestamp(&doc.timestamp, "timestamp")?;
    let concerns = concerns_from_document(doc.concerns)?;

    match doc.success.as_str() {
        "success" => {
            if doc.error_message.is_some() {
                return Err(ReviewJsonCodecError::InvalidField {
                    field: "error_message".into(),
                    reason: "successful round must not have an error_message".into(),
                });
            }
            let verdict_doc = doc.verdict.ok_or_else(|| ReviewJsonCodecError::InvalidField {
                field: "verdict".into(),
                reason: "successful round must have a verdict".into(),
            })?;
            let verdict = verdict_from_document(verdict_doc)?;
            let concerns = normalized_decoded_round_concerns(&verdict, concerns)?;
            let round = GroupRound::success(round_type, timestamp, doc.hash, verdict)?
                .with_concerns(concerns);
            Ok(round)
        }
        "failure" => {
            if doc.verdict.is_some() {
                return Err(ReviewJsonCodecError::InvalidField {
                    field: "verdict".into(),
                    reason: "failed round must not have a verdict".into(),
                });
            }
            let concerns = match concerns {
                ConcernsField::Missing => Vec::new(),
                ConcernsField::Null => {
                    return Err(ReviewJsonCodecError::InvalidField {
                        field: "concerns".into(),
                        reason: "round concerns must not be null".into(),
                    });
                }
                ConcernsField::Present(concerns) if !concerns.is_empty() => {
                    return Err(ReviewJsonCodecError::InvalidField {
                        field: "concerns".into(),
                        reason: "failed round must not persist concerns".into(),
                    });
                }
                ConcernsField::Present(concerns) => concerns,
            };
            let round = GroupRound::failure(round_type, timestamp, doc.hash, doc.error_message)?
                .with_concerns(concerns);
            Ok(round)
        }
        other => Err(ReviewJsonCodecError::InvalidField {
            field: "success".into(),
            reason: format!("expected 'success' or 'failure', got '{other}'"),
        }),
    }
}

fn verdict_from_document(doc: VerdictDocument) -> Result<GroupRoundVerdict, ReviewJsonCodecError> {
    match doc.verdict.as_str() {
        "zero_findings" => {
            if !doc.findings.is_empty() {
                return Err(ReviewJsonCodecError::InvalidField {
                    field: "findings".into(),
                    reason: "zero_findings verdict must have empty findings array".into(),
                });
            }
            Ok(GroupRoundVerdict::ZeroFindings)
        }
        "findings_remain" => {
            let findings = doc
                .findings
                .into_iter()
                .map(finding_from_document)
                .collect::<Result<Vec<_>, _>>()?;
            let verdict = GroupRoundVerdict::findings_remain(findings)?;
            Ok(verdict)
        }
        other => Err(ReviewJsonCodecError::InvalidField {
            field: "verdict".into(),
            reason: format!("expected 'zero_findings' or 'findings_remain', got '{other}'"),
        }),
    }
}

fn finding_from_document(doc: FindingDocument) -> Result<StoredFinding, ReviewJsonCodecError> {
    validate_finding_fields(
        &doc.message,
        doc.severity.as_deref(),
        doc.file.as_deref(),
        doc.line,
        doc.category.as_deref(),
    )?;
    Ok(StoredFinding::new(doc.message, doc.severity, doc.file, doc.line)
        .with_category(doc.category))
}

/// Validates finding field invariants shared by decode and encode paths.
fn validate_stored_finding(finding: &StoredFinding) -> Result<(), ReviewJsonCodecError> {
    validate_finding_fields(
        finding.message(),
        finding.severity(),
        finding.file(),
        finding.line(),
        finding.category(),
    )
}

fn validate_finding_fields(
    message: &str,
    severity: Option<&str>,
    file: Option<&str>,
    line: Option<u64>,
    category: Option<&str>,
) -> Result<(), ReviewJsonCodecError> {
    if message.trim().is_empty() {
        return Err(ReviewJsonCodecError::InvalidField {
            field: "message".into(),
            reason: "finding message must not be empty".into(),
        });
    }
    if severity.is_some_and(|s| s.trim().is_empty()) {
        return Err(ReviewJsonCodecError::InvalidField {
            field: "severity".into(),
            reason: "finding severity must be null or a non-empty string".into(),
        });
    }
    if file.is_some_and(|s| s.trim().is_empty()) {
        return Err(ReviewJsonCodecError::InvalidField {
            field: "file".into(),
            reason: "finding file must be null or a non-empty string".into(),
        });
    }
    if line == Some(0) {
        return Err(ReviewJsonCodecError::InvalidField {
            field: "line".into(),
            reason: "finding line must be null or a 1-based line number".into(),
        });
    }
    if category.is_some_and(|s| s.trim().is_empty()) {
        return Err(ReviewJsonCodecError::InvalidField {
            field: "category".into(),
            reason: "finding category must be null or a non-empty string".into(),
        });
    }
    Ok(())
}

fn concerns_from_document(concerns: ConcernsField) -> Result<ConcernsField, ReviewJsonCodecError> {
    Ok(concerns)
}

fn validate_round_concerns(
    verdict: &GroupRoundVerdict,
    concerns: &ConcernsField,
) -> Result<(), ReviewJsonCodecError> {
    if matches!(concerns, ConcernsField::Null) {
        return Err(ReviewJsonCodecError::InvalidField {
            field: "concerns".into(),
            reason: "round concerns must not be null".into(),
        });
    }
    if verdict.is_zero_findings()
        && matches!(concerns, ConcernsField::Present(concerns) if !concerns.is_empty())
    {
        return Err(ReviewJsonCodecError::InvalidField {
            field: "concerns".into(),
            reason: "zero_findings round must not persist concerns".into(),
        });
    }
    Ok(())
}

fn normalized_decoded_round_concerns(
    verdict: &GroupRoundVerdict,
    concerns: ConcernsField,
) -> Result<Vec<ReviewConcern>, ReviewJsonCodecError> {
    validate_round_concerns(verdict, &concerns)?;

    let GroupRoundVerdict::FindingsRemain(findings) = verdict else {
        return Ok(match concerns {
            ConcernsField::Missing | ConcernsField::Null => Vec::new(),
            ConcernsField::Present(concerns) => concerns,
        });
    };

    let derived_strings: Vec<_> = findings.as_slice().iter().map(stored_finding_concern).collect();
    let concerns = match concerns {
        ConcernsField::Missing => {
            let backfilled = derived_strings
                .iter()
                .cloned()
                .map(|concern| {
                    ReviewConcern::try_new(concern).map_err(|e| {
                        ReviewJsonCodecError::InvalidField {
                            field: "concerns".into(),
                            reason: format!("{e}"),
                        }
                    })
                })
                .collect::<Result<BTreeSet<_>, _>>()?
                .into_iter()
                .collect();
            return Ok(backfilled);
        }
        ConcernsField::Null => {
            return Err(ReviewJsonCodecError::InvalidField {
                field: "concerns".into(),
                reason: "round concerns must not be null".into(),
            });
        }
        ConcernsField::Present(concerns) => concerns,
    };

    let supplied: BTreeSet<_> =
        concerns.iter().map(|concern| concern.as_ref().to_owned()).collect();
    let derived: BTreeSet<_> = derived_strings.into_iter().collect();

    if !derived.is_subset(&supplied) {
        return Err(ReviewJsonCodecError::InvalidField {
            field: "concerns".into(),
            reason: format!(
                "findings_remain round concerns must include all findings-derived concerns (supplied: {:?}, derived: {:?})",
                supplied, derived
            ),
        });
    }

    Ok(concerns)
}

fn stored_finding_concern(finding: &StoredFinding) -> String {
    if let Some(category) = finding.category() {
        category.trim().to_lowercase()
    } else if let Some(file) = finding.file() {
        let slug = domain::review::file_path_to_concern(file.trim()).to_lowercase();
        if slug.trim().is_empty() { "other".to_owned() } else { slug }
    } else {
        "other".to_owned()
    }
}

// ---------------------------------------------------------------------------
// Domain → Document
// ---------------------------------------------------------------------------

fn document_from_review_json(
    review: &ReviewJson,
) -> Result<ReviewJsonDocument, ReviewJsonCodecError> {
    Ok(ReviewJsonDocument {
        schema_version: review.schema_version(),
        cycles: review.cycles().iter().map(cycle_to_document).collect::<Result<Vec<_>, _>>()?,
    })
}

fn cycle_to_document(cycle: &ReviewCycle) -> Result<CycleDocument, ReviewJsonCodecError> {
    let groups = cycle
        .groups()
        .iter()
        .map(|(name, state)| {
            let group = group_to_document(state)?;
            Ok::<(String, GroupDocument), ReviewJsonCodecError>((name.to_string(), group))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(CycleDocument {
        cycle_id: cycle.cycle_id().to_owned(),
        started_at: cycle.started_at().as_str().to_owned(),
        base_ref: cycle.base_ref().to_owned(),
        base_policy_hash: cycle.base_policy_hash().to_owned(),
        policy_hash: cycle.policy_hash().to_owned(),
        approved_head: cycle.approved_head().map(|h| h.as_str().to_owned()),
        groups,
    })
}

fn group_to_document(state: &CycleGroupState) -> Result<GroupDocument, ReviewJsonCodecError> {
    Ok(GroupDocument {
        scope: state.scope().to_vec(),
        rounds: state.rounds().iter().map(round_to_document).collect::<Result<Vec<_>, _>>()?,
    })
}

fn round_to_document(round: &GroupRound) -> Result<RoundDocument, ReviewJsonCodecError> {
    let (success, error_message, verdict) = match round.outcome() {
        GroupRoundOutcome::Success(v) => ("success".to_owned(), None, Some(verdict_to_document(v))),
        GroupRoundOutcome::Failure { error_message } => {
            ("failure".to_owned(), error_message.clone(), None)
        }
    };
    Ok(RoundDocument {
        round_type: round_type_to_string(round.round_type()),
        success,
        error_message,
        timestamp: round.timestamp().as_str().to_owned(),
        hash: round.hash().to_owned(),
        concerns: ConcernsField::Present(normalized_round_concerns(round)?),
        verdict,
    })
}

fn normalized_round_concerns(
    round: &GroupRound,
) -> Result<Vec<ReviewConcern>, ReviewJsonCodecError> {
    match round.outcome() {
        GroupRoundOutcome::Success(GroupRoundVerdict::FindingsRemain(findings)) => {
            let mut concerns = round.concerns().iter().cloned().collect::<BTreeSet<_>>();
            for finding in findings.as_slice() {
                concerns.insert(stored_finding_review_concern(finding)?);
            }
            Ok(concerns.into_iter().collect())
        }
        GroupRoundOutcome::Success(GroupRoundVerdict::ZeroFindings)
        | GroupRoundOutcome::Failure { .. } => Ok(Vec::new()),
    }
}

fn stored_finding_review_concern(
    finding: &StoredFinding,
) -> Result<ReviewConcern, ReviewJsonCodecError> {
    ReviewConcern::try_new(stored_finding_concern(finding)).map_err(|e| {
        ReviewJsonCodecError::InvalidField { field: "concerns".into(), reason: format!("{e}") }
    })
}

fn verdict_to_document(verdict: &GroupRoundVerdict) -> VerdictDocument {
    match verdict {
        GroupRoundVerdict::ZeroFindings => {
            VerdictDocument { verdict: "zero_findings".to_owned(), findings: vec![] }
        }
        GroupRoundVerdict::FindingsRemain(findings) => VerdictDocument {
            verdict: "findings_remain".to_owned(),
            findings: findings.as_slice().iter().map(finding_to_document).collect(),
        },
    }
}

fn finding_to_document(finding: &StoredFinding) -> FindingDocument {
    FindingDocument {
        message: finding.message().to_owned(),
        severity: finding.severity().map(str::to_owned),
        file: finding.file().map(str::to_owned),
        line: finding.line(),
        category: finding.category().map(str::to_owned),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_round_type(s: &str) -> Result<RoundType, ReviewJsonCodecError> {
    match s {
        "fast" => Ok(RoundType::Fast),
        "final" => Ok(RoundType::Final),
        other => Err(ReviewJsonCodecError::InvalidField {
            field: "round_type".into(),
            reason: format!("expected 'fast' or 'final', got '{other}'"),
        }),
    }
}

fn round_type_to_string(rt: RoundType) -> String {
    match rt {
        RoundType::Fast => "fast".to_owned(),
        RoundType::Final => "final".to_owned(),
    }
}

fn parse_timestamp(s: &str, field: &str) -> Result<Timestamp, ReviewJsonCodecError> {
    Timestamp::new(s).map_err(|_| ReviewJsonCodecError::InvalidField {
        field: field.into(),
        reason: format!("invalid timestamp: '{s}'"),
    })
}

/// Validates that cycles are in non-decreasing `started_at` order (append-only).
fn validate_cycle_order(cycles: &[ReviewCycle]) -> Result<(), ReviewJsonCodecError> {
    for pair in cycles.windows(2) {
        let (prev, next) = match (pair.first(), pair.get(1)) {
            (Some(p), Some(n)) => (p, n),
            _ => continue,
        };
        if next.started_at() < prev.started_at() {
            return Err(ReviewJsonCodecError::InvalidField {
                field: "cycles".into(),
                reason: format!(
                    "cycles not in chronological order: {} before {}",
                    prev.started_at(),
                    next.started_at()
                ),
            });
        }
    }
    Ok(())
}

/// Validates that rounds within a group are in non-decreasing timestamp order.
fn validate_round_order(rounds: &[GroupRound]) -> Result<(), ReviewJsonCodecError> {
    for pair in rounds.windows(2) {
        let (prev, next) = match (pair.first(), pair.get(1)) {
            (Some(p), Some(n)) => (p, n),
            _ => continue,
        };
        if next.timestamp() < prev.timestamp() {
            return Err(ReviewJsonCodecError::InvalidField {
                field: "rounds".into(),
                reason: format!(
                    "rounds not in chronological order: {} before {}",
                    prev.timestamp(),
                    next.timestamp()
                ),
            });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn sample_json() -> &'static str {
        r#"{
  "schema_version": 1,
  "cycles": [
    {
      "cycle_id": "2026-03-29T09:47:00Z",
      "started_at": "2026-03-29T09:47:00Z",
      "base_ref": "main",
      "base_policy_hash": "sha256:abc123",
      "policy_hash": "sha256:abc123",
      "groups": {
        "domain": {
          "scope": ["libs/domain/src/lib.rs"],
          "rounds": [
            {
              "round_type": "fast",
              "success": "success",
              "error_message": null,
              "timestamp": "2026-03-29T09:48:23Z",
              "hash": "rvw1:sha256:def456",
              "verdict": {
                "verdict": "zero_findings",
                "findings": []
              }
            }
          ]
        },
        "other": {
          "scope": ["Makefile.toml"],
          "rounds": []
        }
      }
    }
  ]
}"#
    }

    #[test]
    fn test_decode_sample_json() {
        let review = decode(sample_json()).unwrap();
        assert_eq!(review.schema_version(), 1);
        assert_eq!(review.cycles().len(), 1);

        let cycle = review.current_cycle().unwrap();
        assert_eq!(cycle.cycle_id(), "2026-03-29T09:47:00Z");
        assert_eq!(cycle.base_ref(), "main");
        assert_eq!(cycle.policy_hash(), "sha256:abc123");
        assert_eq!(cycle.groups().len(), 2);

        let domain = cycle.group(&ReviewGroupName::try_new("domain").unwrap()).unwrap();
        assert_eq!(domain.scope(), &["libs/domain/src/lib.rs"]);
        assert_eq!(domain.rounds().len(), 1);
        assert!(domain.rounds()[0].is_successful_zero_findings());
        assert_eq!(domain.rounds()[0].hash(), "rvw1:sha256:def456");
    }

    #[test]
    fn test_decode_empty_review() {
        let json = r#"{"schema_version": 1, "cycles": []}"#;
        let review = decode(json).unwrap();
        assert!(review.is_empty());
    }

    #[test]
    fn test_decode_rejects_unsupported_version() {
        let json = r#"{"schema_version": 99, "cycles": []}"#;
        let result = decode(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_failure_round() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "fast",
          "success": "failure",
          "error_message": "timeout after 300s",
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "verdict": null
        }]
      }
    }
  }]
}"#;
        let review = decode(json).unwrap();
        let cycle = review.current_cycle().unwrap();
        let other = cycle.group(&ReviewGroupName::try_new("other").unwrap()).unwrap();
        let round = &other.rounds()[0];
        assert!(!round.is_successful_zero_findings());
        assert_eq!(round.outcome().error_message(), Some("timeout after 300s"));
    }

    #[test]
    fn test_decode_findings_remain() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "final",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "concerns": ["domain.review"],
          "verdict": {
            "verdict": "findings_remain",
            "findings": [
              {"message": "bug found", "severity": "P1", "file": "src/lib.rs", "line": 42, "category": "domain.review"}
            ]
          }
        }]
      }
    }
  }]
}"#;
        let review = decode(json).unwrap();
        let cycle = review.current_cycle().unwrap();
        let other = cycle.group(&ReviewGroupName::try_new("other").unwrap()).unwrap();
        let round = &other.rounds()[0];
        assert!(!round.is_successful_zero_findings());
        assert_eq!(round.outcome().verdict().unwrap().findings().len(), 1);
        assert_eq!(round.outcome().verdict().unwrap().findings()[0].message(), "bug found");
        assert_eq!(round.concerns(), &[ReviewConcern::try_new("domain.review").unwrap()]);
        assert_eq!(
            round.outcome().verdict().unwrap().findings()[0].category(),
            Some("domain.review")
        );
    }

    #[test]
    fn test_decode_accepts_missing_round_concerns_for_backward_compatibility() {
        let review = decode(sample_json()).unwrap();
        let cycle = review.current_cycle().unwrap();
        let domain = cycle.group(&ReviewGroupName::try_new("domain").unwrap()).unwrap();

        assert!(
            domain.rounds()[0].concerns().is_empty(),
            "older review.json rounds without concerns must still decode"
        );
    }

    #[test]
    fn test_decode_backfills_missing_findings_round_concerns_for_backward_compatibility() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "final",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "verdict": {
            "verdict": "findings_remain",
            "findings": [
              {"message": "bug found", "severity": "P1", "file": null, "line": null, "category": "domain.review"}
            ]
          }
        }]
      }
    }
  }]
}"#;

        let review = decode(json).unwrap();
        let round = review
            .current_cycle()
            .unwrap()
            .group(&ReviewGroupName::try_new("other").unwrap())
            .unwrap()
            .rounds()
            .last()
            .unwrap();

        assert_eq!(round.concerns(), &[ReviewConcern::try_new("domain.review").unwrap()]);
    }

    #[test]
    fn test_decode_backfills_missing_findings_round_concerns_from_mixed_case_file() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "final",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "verdict": {
            "verdict": "findings_remain",
            "findings": [
              {"message": "bug found", "severity": "P1", "file": "apps/CLI/src/Foo.rs", "line": 10}
            ]
          }
        }]
      }
    }
  }]
}"#;

        let review = decode(json).unwrap();
        let round = review
            .current_cycle()
            .unwrap()
            .group(&ReviewGroupName::try_new("other").unwrap())
            .unwrap()
            .rounds()
            .last()
            .unwrap();

        assert_eq!(round.concerns(), &[ReviewConcern::try_new("cli.foo").unwrap()]);
    }

    #[test]
    fn test_encode_decode_roundtrip_preserves_round_concerns() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "final",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "concerns": ["cli.review", "domain.review"],
          "verdict": {
            "verdict": "findings_remain",
            "findings": [
              {"message": "bug found", "severity": "P1", "file": null, "line": null, "category": "domain.review"}
            ]
          }
        }]
      }
    }
  }]
}"#;

        let review = decode(json).unwrap();
        let encoded = encode(&review).unwrap();
        let decoded = decode(&encoded).unwrap();
        let round = &decoded
            .current_cycle()
            .unwrap()
            .group(&ReviewGroupName::try_new("other").unwrap())
            .unwrap()
            .rounds()[0];

        assert_eq!(
            round.concerns(),
            &[
                ReviewConcern::try_new("cli.review").unwrap(),
                ReviewConcern::try_new("domain.review").unwrap(),
            ]
        );
    }

    #[test]
    fn test_encode_decode_roundtrip_deduplicates_findings_remain_round_concerns() {
        let mut review = decode(sample_json()).unwrap();
        let cycle = review.current_cycle_mut().unwrap();
        let other = cycle.group_mut(&ReviewGroupName::try_new("other").unwrap()).unwrap();
        let findings = vec![StoredFinding::new("bug found", Some("P1".into()), None, None)];
        let round = GroupRound::success(
            RoundType::Final,
            Timestamp::new("2026-03-29T09:01:00Z").unwrap(),
            "rvw1:sha256:xyz",
            GroupRoundVerdict::findings_remain(findings).unwrap(),
        )
        .unwrap()
        .with_concerns(vec![
            ReviewConcern::try_new("domain.review").unwrap(),
            ReviewConcern::try_new("domain.review").unwrap(),
            ReviewConcern::try_new("cli.review").unwrap(),
        ]);
        other.record_round(round);

        let encoded = encode(&review).unwrap();
        let decoded = decode(&encoded).unwrap();
        let round = decoded
            .current_cycle()
            .unwrap()
            .group(&ReviewGroupName::try_new("other").unwrap())
            .unwrap()
            .rounds()
            .last()
            .unwrap();

        assert_eq!(
            round.concerns(),
            &[
                ReviewConcern::try_new("cli.review").unwrap(),
                ReviewConcern::try_new("domain.review").unwrap(),
                ReviewConcern::try_new("other").unwrap(),
            ]
        );
    }

    #[test]
    fn test_encode_decode_roundtrip_backfills_findings_derived_concerns() {
        let mut review = decode(sample_json()).unwrap();
        let cycle = review.current_cycle_mut().unwrap();
        let other = cycle.group_mut(&ReviewGroupName::try_new("other").unwrap()).unwrap();
        let findings = vec![
            StoredFinding::new("bug found", Some("P1".into()), None, None)
                .with_category(Some("domain.review".into())),
        ];
        let round = GroupRound::success(
            RoundType::Final,
            Timestamp::new("2026-03-29T09:01:00Z").unwrap(),
            "rvw1:sha256:xyz",
            GroupRoundVerdict::findings_remain(findings).unwrap(),
        )
        .unwrap()
        .with_concerns(vec![ReviewConcern::try_new("other").unwrap()]);
        other.record_round(round);

        let encoded = encode(&review).unwrap();
        let decoded = decode(&encoded).unwrap();
        let round = decoded
            .current_cycle()
            .unwrap()
            .group(&ReviewGroupName::try_new("other").unwrap())
            .unwrap()
            .rounds()
            .last()
            .unwrap();

        assert_eq!(
            round.concerns(),
            &[
                ReviewConcern::try_new("domain.review").unwrap(),
                ReviewConcern::try_new("other").unwrap(),
            ]
        );
    }

    #[test]
    fn test_encode_decode_roundtrip_strips_non_findings_round_concerns() {
        let mut review = decode(sample_json()).unwrap();
        let cycle = review.current_cycle_mut().unwrap();

        let zero_round = GroupRound::success(
            RoundType::Final,
            Timestamp::new("2026-03-29T09:49:00Z").unwrap(),
            "rvw1:sha256:zero",
            GroupRoundVerdict::ZeroFindings,
        )
        .unwrap()
        .with_concerns(vec![ReviewConcern::try_new("domain.review").unwrap()]);
        cycle
            .group_mut(&ReviewGroupName::try_new("domain").unwrap())
            .unwrap()
            .record_round(zero_round);

        let failure_round = GroupRound::failure(
            RoundType::Fast,
            Timestamp::new("2026-03-29T09:50:00Z").unwrap(),
            "rvw1:sha256:fail",
            Some("timeout".into()),
        )
        .unwrap()
        .with_concerns(vec![ReviewConcern::try_new("cli.review").unwrap()]);
        cycle
            .group_mut(&ReviewGroupName::try_new("other").unwrap())
            .unwrap()
            .record_round(failure_round);

        let encoded = encode(&review).unwrap();
        let decoded = decode(&encoded).unwrap();
        let cycle = decoded.current_cycle().unwrap();
        let domain = cycle.group(&ReviewGroupName::try_new("domain").unwrap()).unwrap();
        let other = cycle.group(&ReviewGroupName::try_new("other").unwrap()).unwrap();

        assert!(domain.rounds().last().unwrap().concerns().is_empty());
        assert!(other.rounds().last().unwrap().concerns().is_empty());
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let review = decode(sample_json()).unwrap();
        let encoded = encode(&review).unwrap();
        let decoded = decode(&encoded).unwrap();
        assert_eq!(review, decoded);
    }

    #[test]
    fn test_encode_empty_review() {
        let review = ReviewJson::new();
        let json = encode(&review).unwrap();
        let decoded = decode(&json).unwrap();
        assert!(decoded.is_empty());
        assert_eq!(decoded.schema_version(), 1);
    }

    #[test]
    fn test_decode_rejects_invalid_round_type() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "unknown",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "verdict": {"verdict": "zero_findings", "findings": []}
        }]
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(
            matches!(result, Err(ReviewJsonCodecError::InvalidField { field, .. }) if field == "round_type")
        );
    }

    #[test]
    fn test_decode_rejects_invalid_success_value() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "fast",
          "success": "maybe",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "verdict": {"verdict": "zero_findings", "findings": []}
        }]
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(
            matches!(result, Err(ReviewJsonCodecError::InvalidField { field, .. }) if field == "success")
        );
    }

    #[test]
    fn test_decode_success_without_verdict_rejected() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "fast",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "verdict": null
        }]
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(
            matches!(result, Err(ReviewJsonCodecError::InvalidField { field, .. }) if field == "verdict")
        );
    }

    #[test]
    fn test_decode_rejects_failure_with_verdict() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "fast",
          "success": "failure",
          "error_message": "timeout",
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "verdict": {"verdict": "zero_findings", "findings": []}
        }]
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(
            matches!(result, Err(ReviewJsonCodecError::InvalidField { field, .. }) if field == "verdict")
        );
    }

    #[test]
    fn test_decode_rejects_success_with_error_message() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "fast",
          "success": "success",
          "error_message": "should not be here",
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "verdict": {"verdict": "zero_findings", "findings": []}
        }]
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(
            matches!(result, Err(ReviewJsonCodecError::InvalidField { field, .. }) if field == "error_message")
        );
    }

    #[test]
    fn test_decode_rejects_zero_findings_with_findings() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "fast",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "verdict": {
            "verdict": "zero_findings",
            "findings": [{"message": "bug", "severity": null, "file": null, "line": null}]
          }
        }]
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(
            matches!(result, Err(ReviewJsonCodecError::InvalidField { field, .. }) if field == "findings")
        );
    }

    #[test]
    fn test_decode_rejects_finding_with_blank_message() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "fast",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "verdict": {
            "verdict": "findings_remain",
            "findings": [{"message": "  ", "severity": null, "file": null, "line": null}]
          }
        }]
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(
            matches!(result, Err(ReviewJsonCodecError::InvalidField { field, .. }) if field == "message")
        );
    }

    #[test]
    fn test_decode_rejects_finding_with_blank_severity() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "fast",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "verdict": {
            "verdict": "findings_remain",
            "findings": [{"message": "bug", "severity": "", "file": null, "line": null}]
          }
        }]
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(
            matches!(result, Err(ReviewJsonCodecError::InvalidField { field, .. }) if field == "severity")
        );
    }

    #[test]
    fn test_decode_rejects_finding_with_blank_file() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "fast",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "verdict": {
            "verdict": "findings_remain",
            "findings": [{"message": "bug", "severity": null, "file": " ", "line": null}]
          }
        }]
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(
            matches!(result, Err(ReviewJsonCodecError::InvalidField { field, .. }) if field == "file")
        );
    }

    #[test]
    fn test_decode_rejects_finding_with_blank_category() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "fast",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "verdict": {
            "verdict": "findings_remain",
            "findings": [{"message": "bug", "severity": null, "file": null, "line": null, "category": " "}]
          }
        }]
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(
            matches!(result, Err(ReviewJsonCodecError::InvalidField { field, .. }) if field == "category")
        );
    }

    #[test]
    fn test_decode_rejects_findings_round_missing_derived_concern() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "fast",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "concerns": ["other"],
          "verdict": {
            "verdict": "findings_remain",
            "findings": [{"message": "bug", "severity": null, "file": null, "line": null, "category": "domain.review"}]
          }
        }]
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(
            matches!(result, Err(ReviewJsonCodecError::InvalidField { field, .. }) if field == "concerns")
        );
    }

    #[test]
    fn test_decode_rejects_findings_round_with_explicit_empty_concerns() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "fast",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "concerns": [],
          "verdict": {
            "verdict": "findings_remain",
            "findings": [{"message": "bug", "severity": null, "file": null, "line": null, "category": "domain.review"}]
          }
        }]
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(
            matches!(result, Err(ReviewJsonCodecError::InvalidField { field, .. }) if field == "concerns")
        );
    }

    #[test]
    fn test_decode_rejects_findings_round_with_null_concerns() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "fast",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "concerns": null,
          "verdict": {
            "verdict": "findings_remain",
            "findings": [{"message": "bug", "severity": null, "file": null, "line": null, "category": "domain.review"}]
          }
        }]
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(
            matches!(result, Err(ReviewJsonCodecError::InvalidField { field, .. }) if field == "concerns")
        );
    }

    #[test]
    fn test_decode_rejects_finding_with_line_zero() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "other": {
        "scope": [],
        "rounds": [{
          "round_type": "fast",
          "success": "success",
          "error_message": null,
          "timestamp": "2026-03-29T09:01:00Z",
          "hash": "rvw1:sha256:xyz",
          "verdict": {
            "verdict": "findings_remain",
            "findings": [{"message": "bug", "severity": null, "file": null, "line": 0}]
          }
        }]
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(
            matches!(result, Err(ReviewJsonCodecError::InvalidField { field, .. }) if field == "line")
        );
    }

    #[test]
    fn test_decode_rejects_cycle_without_other_group() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-03-29T09:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "groups": {
      "domain": {
        "scope": [],
        "rounds": []
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(matches!(result, Err(ReviewJsonCodecError::Cycle(_))));
    }

    // -- approved_head round-trip tests --

    #[test]
    fn test_approved_head_null_round_trips() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-04-02T00:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "approved_head": null,
    "groups": {
      "other": {
        "scope": [],
        "rounds": []
      }
    }
  }]
}"#;
        let review = decode(json).unwrap();
        let cycle = review.current_cycle().unwrap();
        assert!(cycle.approved_head().is_none());

        let encoded = encode(&review).unwrap();
        let decoded = decode(&encoded).unwrap();
        assert!(decoded.current_cycle().unwrap().approved_head().is_none());
    }

    #[test]
    fn test_approved_head_sha_round_trips() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-04-02T00:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "approved_head": "abcdef0123456789abcdef0123456789abcdef01",
    "groups": {
      "other": {
        "scope": [],
        "rounds": []
      }
    }
  }]
}"#;
        let review = decode(json).unwrap();
        let cycle = review.current_cycle().unwrap();
        assert_eq!(
            cycle.approved_head().unwrap().as_str(),
            "abcdef0123456789abcdef0123456789abcdef01"
        );

        let encoded = encode(&review).unwrap();
        let decoded = decode(&encoded).unwrap();
        assert_eq!(
            decoded.current_cycle().unwrap().approved_head().unwrap().as_str(),
            "abcdef0123456789abcdef0123456789abcdef01"
        );
    }

    #[test]
    fn test_approved_head_invalid_sha_rejected() {
        let json = r#"{
  "schema_version": 1,
  "cycles": [{
    "cycle_id": "c1",
    "started_at": "2026-04-02T00:00:00Z",
    "base_ref": "main",
    "base_policy_hash": "sha256:abc",
    "policy_hash": "sha256:abc",
    "approved_head": "not-a-valid-sha",
    "groups": {
      "other": {
        "scope": [],
        "rounds": []
      }
    }
  }]
}"#;
        let result = decode(json);
        assert!(result.is_err());
    }
}
