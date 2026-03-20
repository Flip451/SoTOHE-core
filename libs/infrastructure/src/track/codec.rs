//! Serde types for metadata.json (TrackDocumentV2) matching Python track_schema.py.

use std::collections::{BTreeMap, HashMap};

use domain::{
    CodeHash, CommitHash, DomainError, EscalationPhase, PlanSection, PlanView, ReviewConcern,
    ReviewConcernStreak, ReviewCycleSummary, ReviewEscalationBlock, ReviewEscalationDecision,
    ReviewEscalationResolution, ReviewEscalationState, ReviewGroupName, ReviewGroupState,
    ReviewRoundResult, ReviewState, ReviewStatus, RoundType, StatusOverride, TaskId, TaskStatus,
    Timestamp, TrackBranch, TrackId, TrackMetadata, TrackTask, ValidationError, Verdict,
};
use serde::{Deserialize, Deserializer};

/// Codec error for metadata.json serialization/deserialization.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("domain validation error: {0}")]
    Domain(#[from] DomainError),

    #[error("invalid field '{field}': {reason}")]
    InvalidField { field: String, reason: String },

    #[error("validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackDocumentV2 {
    pub schema_version: u32,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub tasks: Vec<TrackTaskDocument>,
    pub plan: PlanDocument,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_override: Option<TrackStatusOverrideDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<TrackReviewDocument>,
    /// Unknown fields captured during deserialization and preserved on re-serialization.
    #[serde(flatten)]
    #[serde(default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackTaskDocument {
    pub id: String,
    pub description: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanDocument {
    #[serde(default, deserialize_with = "deserialize_string_vec_relaxed")]
    pub summary: Vec<String>,
    #[serde(default)]
    pub sections: Vec<PlanSectionDocument>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanSectionDocument {
    pub id: String,
    pub title: String,
    #[serde(default, deserialize_with = "deserialize_string_vec_relaxed")]
    pub description: Vec<String>,
    #[serde(default)]
    pub task_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackStatusOverrideDocument {
    pub status: String,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackReviewDocument {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_hash: Option<String>,
    #[serde(default)]
    pub groups: BTreeMap<String, ReviewGroupDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escalation: Option<TrackReviewEscalationDocument>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReviewGroupDocument {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fast: Option<ReviewRoundDocument>,
    #[serde(rename = "final", skip_serializing_if = "Option::is_none")]
    pub final_round: Option<ReviewRoundDocument>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReviewRoundDocument {
    pub round: u32,
    pub verdict: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concerns: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackReviewEscalationDocument {
    pub threshold: u8,
    pub phase: EscalationPhaseDocument,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_cycles: Vec<ReviewCycleDocument>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub concern_streaks: BTreeMap<String, ConcernStreakDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_resolution: Option<ResolutionDocument>,
}

/// Tagged union for `EscalationPhase` ADT.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EscalationPhaseDocument {
    Clear,
    Blocked { concerns: Vec<String>, blocked_at: String },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReviewCycleDocument {
    pub round_type: String,
    pub round: u32,
    pub timestamp: String,
    #[serde(default)]
    pub concerns: Vec<String>,
    #[serde(default)]
    pub groups: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConcernStreakDocument {
    pub consecutive_rounds: u8,
    pub last_round_type: String,
    pub last_round: u32,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolutionDocument {
    pub blocked_concerns: Vec<String>,
    pub workspace_search_ref: String,
    pub reinvention_check_ref: String,
    pub decision: String,
    pub summary: String,
    pub resolved_at: String,
}

/// Metadata not part of the domain aggregate (infrastructure concern).
#[derive(Debug, Clone)]
pub struct DocumentMeta {
    pub schema_version: u32,
    pub created_at: String,
    pub updated_at: String,
    /// Original JSON status string, preserved for values the domain model
    /// cannot compute (e.g., "archived" which is a workflow-level state).
    pub original_status: Option<String>,
    /// Unknown fields captured from the original JSON and preserved on re-serialization.
    pub extra: serde_json::Map<String, serde_json::Value>,
}

fn deserialize_string_vec_relaxed<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Null => Ok(Vec::new()),
        serde_json::Value::String(s) => Ok(vec![s]),
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(|value| match value {
                serde_json::Value::String(s) => Ok(s),
                other => {
                    Err(serde::de::Error::custom(format!("expected string item, got {other}")))
                }
            })
            .collect(),
        other => Err(serde::de::Error::custom(format!("expected string or sequence, got {other}"))),
    }
}

/// Decodes a JSON string into a domain `TrackMetadata` and infrastructure `DocumentMeta`.
///
/// # Errors
/// Returns `CodecError` on JSON parse failure or domain validation failure.
pub fn decode(json: &str) -> Result<(TrackMetadata, DocumentMeta), CodecError> {
    let doc: TrackDocumentV2 = serde_json::from_str(json)?;
    let meta = DocumentMeta {
        schema_version: doc.schema_version,
        created_at: doc.created_at.clone(),
        updated_at: doc.updated_at.clone(),
        original_status: Some(doc.status.clone()),
        extra: doc.extra.clone(),
    };
    let track = track_metadata_from_document(doc)?;
    Ok((track, meta))
}

/// Encodes a domain `TrackMetadata` and infrastructure `DocumentMeta` into a JSON string.
///
/// # Errors
/// Returns `CodecError` on JSON serialization failure.
pub fn encode(track: &TrackMetadata, meta: &DocumentMeta) -> Result<String, CodecError> {
    let doc = document_from_track_metadata(track, meta);
    let mut value = serde_json::to_value(&doc)?;
    if meta.schema_version == 3 && track.branch().is_none() {
        if let serde_json::Value::Object(object) = &mut value {
            object.insert("branch".to_owned(), serde_json::Value::Null);
        }
    }
    let json = serde_json::to_string_pretty(&value)?;
    Ok(json)
}

fn track_metadata_from_document(doc: TrackDocumentV2) -> Result<TrackMetadata, CodecError> {
    let id = TrackId::try_new(&doc.id).map_err(DomainError::from)?;

    let branch = doc
        .branch
        .map(TrackBranch::try_new)
        .transpose()
        .map_err(|e| CodecError::Domain(e.into()))?;

    let tasks: Vec<TrackTask> = doc
        .tasks
        .into_iter()
        .map(|t| {
            let task_id = TaskId::try_new(&t.id).map_err(DomainError::from)?;
            let status = parse_task_status(&t.status, t.commit_hash.as_deref())?;
            TrackTask::with_status(task_id, t.description, status)
                .map_err(|e| CodecError::Domain(e.into()))
        })
        .collect::<Result<Vec<_>, CodecError>>()?;

    let plan = plan_from_document(doc.plan)?;

    let status_override =
        doc.status_override.map(|o| parse_status_override(&o.status, o.reason)).transpose()?;

    let mut track =
        TrackMetadata::with_branch(id, branch, doc.title, tasks, plan, status_override)?;

    // Decode review section (backward compatible: None when absent)
    if let Some(review_doc) = doc.review {
        track.set_review(Some(review_from_document(review_doc)?));
    }

    Ok(track)
}

fn document_from_track_metadata(track: &TrackMetadata, meta: &DocumentMeta) -> TrackDocumentV2 {
    // Preserve "archived" status from the original JSON when the domain model
    // cannot compute it (archived is a workflow-level state, not task-derived).
    let status = match meta.original_status.as_deref() {
        Some("archived") => "archived".to_owned(),
        _ => track.status().to_string(),
    };

    TrackDocumentV2 {
        schema_version: meta.schema_version,
        id: track.id().to_string(),
        branch: track.branch().map(|b| b.to_string()),
        title: track.title().to_string(),
        status,
        created_at: meta.created_at.clone(),
        updated_at: meta.updated_at.clone(),
        tasks: track.tasks().iter().map(task_to_document).collect(),
        plan: plan_to_document(track.plan()),
        status_override: track.status_override().map(override_to_document),
        review: track.review().map(review_to_document),
        extra: meta.extra.clone(),
    }
}

fn parse_task_status(status: &str, commit_hash: Option<&str>) -> Result<TaskStatus, CodecError> {
    match status {
        "todo" => Ok(TaskStatus::Todo),
        "in_progress" => Ok(TaskStatus::InProgress),
        "done" => match commit_hash {
            Some(h) => {
                let hash = CommitHash::try_new(h).map_err(|e| CodecError::Domain(e.into()))?;
                Ok(TaskStatus::DoneTraced { commit_hash: hash })
            }
            None => Ok(TaskStatus::DonePending),
        },
        "skipped" => Ok(TaskStatus::Skipped),
        other => Err(CodecError::InvalidField {
            field: "status".into(),
            reason: format!("unknown task status: {other}"),
        }),
    }
}

fn parse_status_override(status: &str, reason: String) -> Result<StatusOverride, CodecError> {
    match status {
        "blocked" => StatusOverride::blocked(reason).map_err(|e| CodecError::Domain(e.into())),
        "cancelled" => StatusOverride::cancelled(reason).map_err(|e| CodecError::Domain(e.into())),
        other => Err(CodecError::InvalidField {
            field: "status_override.status".into(),
            reason: format!("unknown override status: {other}"),
        }),
    }
}

fn plan_from_document(doc: PlanDocument) -> Result<PlanView, CodecError> {
    let sections = doc
        .sections
        .into_iter()
        .map(|s| {
            let task_ids = s
                .task_ids
                .into_iter()
                .map(|id| TaskId::try_new(id).map_err(|e| CodecError::Domain(e.into())))
                .collect::<Result<Vec<_>, _>>()?;
            PlanSection::new(s.id, s.title, s.description, task_ids)
                .map_err(|e| CodecError::Domain(e.into()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PlanView::new(doc.summary, sections))
}

fn task_to_document(task: &TrackTask) -> TrackTaskDocument {
    let (status, commit_hash) = match task.status() {
        TaskStatus::Todo => ("todo".to_owned(), None),
        TaskStatus::InProgress => ("in_progress".to_owned(), None),
        TaskStatus::DonePending => ("done".to_owned(), None),
        TaskStatus::DoneTraced { commit_hash } => {
            ("done".to_owned(), Some(commit_hash.to_string()))
        }
        TaskStatus::Skipped => ("skipped".to_owned(), None),
    };

    TrackTaskDocument {
        id: task.id().to_string(),
        description: task.description().to_owned(),
        status,
        commit_hash,
    }
}

fn plan_to_document(plan: &PlanView) -> PlanDocument {
    PlanDocument {
        summary: plan.summary().to_vec(),
        sections: plan
            .sections()
            .iter()
            .map(|s| PlanSectionDocument {
                id: s.id().to_owned(),
                title: s.title().to_owned(),
                description: s.description().to_vec(),
                task_ids: s.task_ids().iter().map(|id| id.to_string()).collect(),
            })
            .collect(),
    }
}

fn review_from_document(doc: TrackReviewDocument) -> Result<ReviewState, CodecError> {
    let status = parse_review_status(&doc.status)?;
    let original_group_count = doc.groups.len();
    let groups = doc
        .groups
        .into_iter()
        .map(|(name, group_doc)| {
            let group_name =
                ReviewGroupName::try_new(name).map_err(|e| CodecError::Domain(e.into()))?;
            let fast = group_doc.fast.map(round_result_from_document).transpose()?;
            let final_round = group_doc.final_round.map(round_result_from_document).transpose()?;
            let group_state = match (fast, final_round) {
                (Some(f), Some(fin)) => ReviewGroupState::with_both(f, fin),
                (Some(f), None) => ReviewGroupState::with_fast(f),
                (None, Some(fin)) => ReviewGroupState::with_final_only(fin),
                (None, None) => ReviewGroupState::default(),
            };
            Ok((group_name, group_state))
        })
        .collect::<Result<HashMap<_, _>, CodecError>>()?;
    // Detect collisions after normalization (trimming) — two JSON keys that differ
    // only in whitespace would silently merge, dropping review history.
    if groups.len() != original_group_count {
        return Err(CodecError::Validation(
            "review.groups contains keys that collide after normalization".into(),
        ));
    }

    let escalation = doc.escalation.map(escalation_from_document).transpose()?.unwrap_or_default();

    let code_hash = match doc.code_hash {
        None => CodeHash::NotRecorded,
        Some(s) if s == "PENDING" => CodeHash::Pending,
        Some(s) => CodeHash::computed(s).map_err(|_| CodecError::InvalidField {
            field: "review.code_hash".to_owned(),
            reason: "code hash must not be empty or whitespace-only".to_owned(),
        })?,
    };
    Ok(ReviewState::with_fields(status, code_hash, groups, escalation))
}

fn parse_timestamp(s: String, field: &str) -> Result<Timestamp, CodecError> {
    Timestamp::new(s.clone()).map_err(|_: ValidationError| CodecError::InvalidField {
        field: field.to_owned(),
        reason: format!("invalid ISO 8601 timestamp: {s:?}"),
    })
}

fn round_result_from_document(doc: ReviewRoundDocument) -> Result<ReviewRoundResult, CodecError> {
    let timestamp = parse_timestamp(doc.timestamp, "review.groups.*.*.timestamp")?;
    let verdict = Verdict::parse(&doc.verdict).map_err(|_| CodecError::InvalidField {
        field: "review.groups.*.*.verdict".to_owned(),
        reason: format!("unknown verdict: {:?}", doc.verdict),
    })?;

    // For zero_findings, skip concern parsing entirely (backward compat with legacy/corrupt data).
    // Concerns on a zero_findings round are always stripped regardless of content.
    if verdict.is_zero_findings() {
        return Ok(ReviewRoundResult::new_with_concerns(doc.round, verdict, timestamp, Vec::new()));
    }

    // For other verdicts, parse and validate concerns.
    let concerns = doc
        .concerns
        .into_iter()
        .map(|s| {
            ReviewConcern::try_new(&s)
                .map_err(|_| CodecError::Validation(format!("invalid concern slug: {s:?}")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Only findings_remain with no concerns gets "other" fallback (legacy data).
    let sanitized = if verdict == Verdict::FindingsRemain && concerns.is_empty() {
        let fallback = ReviewConcern::try_new("other")
            .map_err(|_| CodecError::Validation("failed to construct fallback concern".into()))?;
        vec![fallback]
    } else {
        concerns
    };

    Ok(ReviewRoundResult::new_with_concerns(doc.round, verdict, timestamp, sanitized))
}

fn review_to_document(review: &ReviewState) -> TrackReviewDocument {
    // Collect into BTreeMap for deterministic key ordering in serialized JSON.
    let groups: BTreeMap<String, ReviewGroupDocument> = review
        .groups()
        .iter()
        .map(|(name, group)| {
            (
                name.as_ref().to_owned(),
                ReviewGroupDocument {
                    fast: group.fast().map(round_result_to_document),
                    final_round: group.final_round().map(round_result_to_document),
                },
            )
        })
        .collect();

    let escalation = escalation_to_document(review.escalation());

    // Serialize code_hash: Pending → "PENDING", Computed(s) → s, None → None.
    let code_hash = review.code_hash_for_serialization().map(|s| s.to_owned());

    TrackReviewDocument { status: review.status().to_string(), code_hash, groups, escalation }
}

fn round_result_to_document(result: &ReviewRoundResult) -> ReviewRoundDocument {
    let concerns: Vec<String> = result.concerns().iter().map(|c| c.as_ref().to_owned()).collect();
    ReviewRoundDocument {
        round: result.round(),
        verdict: result.verdict().to_string(),
        timestamp: result.timestamp().to_owned(),
        concerns,
    }
}

fn escalation_from_document(
    doc: TrackReviewEscalationDocument,
) -> Result<ReviewEscalationState, CodecError> {
    if doc.threshold == 0 {
        return Err(CodecError::Validation("escalation threshold must be >= 1".into()));
    }
    let phase = escalation_phase_from_document(doc.phase)?;

    let recent_cycles = doc
        .recent_cycles
        .into_iter()
        .map(|c| {
            let round_type = parse_round_type(&c.round_type)?;
            let concerns = c
                .concerns
                .into_iter()
                .map(|s| {
                    ReviewConcern::try_new(s).map_err(|e| CodecError::InvalidField {
                        field: "review.escalation.recent_cycles.*.concerns".into(),
                        reason: e.to_string(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let ts = parse_timestamp(c.timestamp, "review.escalation.recent_cycles.*.timestamp")?;
            let groups = c
                .groups
                .into_iter()
                .map(|s| ReviewGroupName::try_new(s).map_err(|e| CodecError::Domain(e.into())))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ReviewCycleSummary::new(round_type, c.round, ts, concerns, groups))
        })
        .collect::<Result<Vec<_>, CodecError>>()?;

    let concern_streaks = doc
        .concern_streaks
        .into_iter()
        .map(|(key, streak_doc)| {
            let concern = ReviewConcern::try_new(key).map_err(|e| CodecError::InvalidField {
                field: "review.escalation.concern_streaks".into(),
                reason: e.to_string(),
            })?;
            let last_round_type = parse_round_type(&streak_doc.last_round_type)?;
            let last_seen_at = parse_timestamp(
                streak_doc.last_seen_at,
                "review.escalation.concern_streaks.*.last_seen_at",
            )?;
            let streak = ReviewConcernStreak::new(
                streak_doc.consecutive_rounds,
                last_round_type,
                streak_doc.last_round,
                last_seen_at,
            );
            Ok((concern, streak))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>, CodecError>>()?;

    let last_resolution = doc.last_resolution.map(resolution_from_document).transpose()?;

    Ok(ReviewEscalationState::with_fields(
        doc.threshold,
        phase,
        recent_cycles,
        concern_streaks,
        last_resolution,
    ))
}

fn escalation_phase_from_document(
    doc: EscalationPhaseDocument,
) -> Result<EscalationPhase, CodecError> {
    match doc {
        EscalationPhaseDocument::Clear => Ok(EscalationPhase::Clear),
        EscalationPhaseDocument::Blocked { concerns, blocked_at } => {
            let concerns = concerns
                .into_iter()
                .map(|s| {
                    ReviewConcern::try_new(s).map_err(|e| CodecError::InvalidField {
                        field: "review.escalation.phase.concerns".into(),
                        reason: e.to_string(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if concerns.is_empty() {
                return Err(CodecError::Validation(
                    "escalation block must have at least one concern".into(),
                ));
            }
            let blocked_at_ts = parse_timestamp(blocked_at, "review.escalation.phase.blocked_at")?;
            Ok(EscalationPhase::Blocked(ReviewEscalationBlock::new(concerns, blocked_at_ts)))
        }
    }
}

fn resolution_from_document(
    doc: ResolutionDocument,
) -> Result<ReviewEscalationResolution, CodecError> {
    if doc.resolved_at.trim().is_empty() {
        return Err(CodecError::Validation("resolution resolved_at must not be empty".into()));
    }

    let blocked_concerns = doc
        .blocked_concerns
        .into_iter()
        .map(|s| {
            ReviewConcern::try_new(s).map_err(|e| CodecError::InvalidField {
                field: "review.escalation.last_resolution.blocked_concerns".into(),
                reason: e.to_string(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if blocked_concerns.is_empty() {
        return Err(CodecError::Validation("resolution blocked_concerns must not be empty".into()));
    }
    let decision = parse_escalation_decision(&doc.decision)?;
    let resolved_at =
        parse_timestamp(doc.resolved_at, "review.escalation.last_resolution.resolved_at")?;
    ReviewEscalationResolution::new(
        blocked_concerns,
        doc.workspace_search_ref,
        doc.reinvention_check_ref,
        decision,
        doc.summary,
        resolved_at,
    )
    .map_err(|e| CodecError::Validation(e.to_string()))
}

fn escalation_to_document(
    escalation: &ReviewEscalationState,
) -> Option<TrackReviewEscalationDocument> {
    // Skip serializing escalation when it is the default (Clear, threshold=3, no data).
    let is_default = escalation.threshold() == 3
        && matches!(escalation.phase(), EscalationPhase::Clear)
        && escalation.recent_cycles().is_empty()
        && escalation.concern_streaks().is_empty()
        && escalation.last_resolution().is_none();
    if is_default {
        return None;
    }

    let phase = match escalation.phase() {
        EscalationPhase::Clear => EscalationPhaseDocument::Clear,
        EscalationPhase::Blocked(block) => EscalationPhaseDocument::Blocked {
            concerns: block.concerns().iter().map(|c| c.as_ref().to_owned()).collect(),
            blocked_at: block.blocked_at().to_owned(),
        },
    };

    let recent_cycles = escalation
        .recent_cycles()
        .iter()
        .map(|c| ReviewCycleDocument {
            round_type: c.round_type().to_string(),
            round: c.round(),
            timestamp: c.timestamp().to_owned(),
            concerns: c.concerns().iter().map(|rc| rc.as_ref().to_owned()).collect(),
            groups: c.groups().iter().map(|g| g.as_ref().to_owned()).collect(),
        })
        .collect();

    let concern_streaks = escalation
        .concern_streaks()
        .iter()
        .map(|(concern, streak)| {
            (
                concern.as_ref().to_owned(),
                ConcernStreakDocument {
                    consecutive_rounds: streak.consecutive_rounds(),
                    last_round_type: streak.last_round_type().to_string(),
                    last_round: streak.last_round(),
                    last_seen_at: streak.last_seen_at().to_owned(),
                },
            )
        })
        .collect();

    let last_resolution = escalation.last_resolution().map(|r| ResolutionDocument {
        blocked_concerns: r.blocked_concerns().iter().map(|c| c.as_ref().to_owned()).collect(),
        workspace_search_ref: r.workspace_search_ref().to_owned(),
        reinvention_check_ref: r.reinvention_check_ref().to_owned(),
        decision: escalation_decision_to_str(r.decision()).to_owned(),
        summary: r.summary().to_owned(),
        resolved_at: r.resolved_at().to_owned(),
    });

    Some(TrackReviewEscalationDocument {
        threshold: escalation.threshold(),
        phase,
        recent_cycles,
        concern_streaks,
        last_resolution,
    })
}

fn parse_round_type(s: &str) -> Result<RoundType, CodecError> {
    match s {
        "fast" => Ok(RoundType::Fast),
        "final" => Ok(RoundType::Final),
        other => Err(CodecError::InvalidField {
            field: "round_type".into(),
            reason: format!("unknown round type: {other}"),
        }),
    }
}

fn parse_escalation_decision(s: &str) -> Result<ReviewEscalationDecision, CodecError> {
    match s {
        "adopt_workspace_solution" => Ok(ReviewEscalationDecision::AdoptWorkspaceSolution),
        "adopt_external_crate" => Ok(ReviewEscalationDecision::AdoptExternalCrate),
        "continue_self_implementation" => Ok(ReviewEscalationDecision::ContinueSelfImplementation),
        other => Err(CodecError::InvalidField {
            field: "review.escalation.last_resolution.decision".into(),
            reason: format!("unknown escalation decision: {other}"),
        }),
    }
}

fn escalation_decision_to_str(decision: ReviewEscalationDecision) -> &'static str {
    match decision {
        ReviewEscalationDecision::AdoptWorkspaceSolution => "adopt_workspace_solution",
        ReviewEscalationDecision::AdoptExternalCrate => "adopt_external_crate",
        ReviewEscalationDecision::ContinueSelfImplementation => "continue_self_implementation",
    }
}

fn parse_review_status(status: &str) -> Result<ReviewStatus, CodecError> {
    match status {
        "not_started" => Ok(ReviewStatus::NotStarted),
        "invalidated" => Ok(ReviewStatus::Invalidated),
        "fast_passed" => Ok(ReviewStatus::FastPassed),
        "approved" => Ok(ReviewStatus::Approved),
        other => Err(CodecError::InvalidField {
            field: "review.status".into(),
            reason: format!("unknown review status: {other}"),
        }),
    }
}

fn override_to_document(override_: &StatusOverride) -> TrackStatusOverrideDocument {
    TrackStatusOverrideDocument {
        status: override_.kind().to_string(),
        reason: override_.reason().to_owned(),
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use domain::TrackStatus;

    fn sample_json() -> &'static str {
        r#"{
  "schema_version": 2,
  "id": "test-track",
  "title": "Test Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {
      "id": "T1",
      "description": "First task",
      "status": "todo"
    },
    {
      "id": "T2",
      "description": "Second task",
      "status": "done",
      "commit_hash": "abc1234"
    }
  ],
  "plan": {
    "summary": ["Test plan summary"],
    "sections": [
      {
        "id": "S1",
        "title": "Section 1",
        "description": ["Description line"],
        "task_ids": ["T1", "T2"]
      }
    ]
  }
}"#
    }

    #[test]
    fn test_decode_valid_json_returns_track_metadata() {
        let (track, meta) = decode(sample_json()).unwrap();
        assert_eq!(track.id().as_ref(), "test-track");
        assert_eq!(track.title(), "Test Track");
        assert_eq!(track.tasks().len(), 2);
        assert_eq!(meta.schema_version, 2);
        assert_eq!(meta.created_at, "2026-03-11T00:00:00Z");
    }

    #[test]
    fn test_encode_then_decode_round_trip() {
        let (track, meta) = decode(sample_json()).unwrap();
        let json = encode(&track, &meta).unwrap();
        let (track2, meta2) = decode(&json).unwrap();
        assert_eq!(track, track2);
        assert_eq!(meta.schema_version, meta2.schema_version);
    }

    #[test]
    fn test_done_pending_round_trips_without_commit_hash() {
        let json = r#"{
  "schema_version": 2,
  "id": "pending-track",
  "title": "Pending Test",
  "status": "in_progress",
  "created_at": "2026-03-20T00:00:00Z",
  "updated_at": "2026-03-20T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Done without hash", "status": "done"}
  ],
  "plan": {
    "summary": [],
    "sections": [{"id": "S1", "title": "S", "description": [], "task_ids": ["T1"]}]
  }
}"#;
        let (track, meta) = decode(json).unwrap();
        assert!(matches!(track.tasks()[0].status(), TaskStatus::DonePending));

        let re_encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();
        let task = &doc["tasks"][0];
        assert_eq!(task["status"], "done");
        assert!(task.get("commit_hash").is_none() || task["commit_hash"].is_null());

        let (track2, _) = decode(&re_encoded).unwrap();
        assert_eq!(track, track2);
    }

    #[test]
    fn test_decode_with_status_override() {
        let json = r#"{
  "schema_version": 2,
  "id": "blocked-track",
  "title": "Blocked Track",
  "status": "blocked",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Task", "status": "todo"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}
    ]
  },
  "status_override": {"status": "blocked", "reason": "waiting on review"}
}"#;
        let (track, _meta) = decode(json).unwrap();
        assert_eq!(track.status(), TrackStatus::Blocked);
        assert!(track.status_override().is_some());
    }

    #[test]
    fn test_decode_accepts_missing_section_description() {
        let json = r#"{
  "schema_version": 3,
  "id": "compat-track",
  "branch": "track/compat-track",
  "title": "Compat Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {
      "id": "T1",
      "description": "First task",
      "status": "todo"
    }
  ],
  "plan": {
    "summary": [],
    "sections": [
      {
        "id": "S1",
        "title": "Section 1",
        "task_ids": ["T1"]
      }
    ]
  }
        }"#;

        let (track, _) = decode(json).unwrap();
        assert!(track.plan().sections()[0].description().is_empty());
    }

    #[test]
    fn test_decode_accepts_string_summary() {
        let json = r#"{
  "schema_version": 2,
  "id": "string-summary-track",
  "title": "String Summary Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {
      "id": "T1",
      "description": "First task",
      "status": "todo"
    }
  ],
  "plan": {
    "summary": "single summary line",
    "sections": [
      {
        "id": "S1",
        "title": "Section 1",
        "task_ids": ["T1"]
      }
    ]
  }
}"#;

        let (track, _) = decode(json).unwrap();
        assert_eq!(track.plan().summary(), &["single summary line".to_owned()]);
    }

    #[test]
    fn test_decode_invalid_task_status_returns_error() {
        let json = r#"{
  "schema_version": 2,
  "id": "bad-track",
  "title": "Bad Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Task", "status": "unknown_status"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}
    ]
  }
}"#;
        let result = decode(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_invalid_json_returns_error() {
        let result = decode("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_archived_status_preserved_through_round_trip() {
        let json = r#"{
  "schema_version": 2,
  "id": "archived-track",
  "title": "Archived Track",
  "status": "archived",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Done task", "status": "done", "commit_hash": "abc1234"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}
    ]
  }
}"#;
        let (track, meta) = decode(json).unwrap();
        let re_encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();

        // "archived" must be preserved, not rewritten to "done".
        assert_eq!(doc["status"].as_str().unwrap(), "archived");
    }

    #[test]
    fn test_decode_encode_preserves_unknown_fields() {
        let json = r#"{
  "schema_version": 2,
  "id": "test-track",
  "title": "Test Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "custom_field": "preserved_value",
  "tasks": [
    {"id": "T1", "description": "First task", "status": "todo"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section 1", "description": [], "task_ids": ["T1"]}
    ]
  }
}"#;
        let (track, meta) = decode(json).unwrap();
        let re_encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();
        assert_eq!(doc["custom_field"].as_str().unwrap(), "preserved_value");
    }

    #[test]
    fn test_decode_encode_without_extra_fields_round_trips_correctly() {
        let (track, meta) = decode(sample_json()).unwrap();
        let re_encoded = encode(&track, &meta).unwrap();
        let (track2, _) = decode(&re_encoded).unwrap();
        assert_eq!(track, track2);
    }

    #[test]
    fn test_known_fields_are_not_in_extra_map() {
        let json = sample_json();
        let doc: TrackDocumentV2 = serde_json::from_str(json).unwrap();
        // Known fields like "id", "title", "tasks" should NOT appear in the extra map
        assert!(!doc.extra.contains_key("id"));
        assert!(!doc.extra.contains_key("title"));
        assert!(!doc.extra.contains_key("tasks"));
        assert!(!doc.extra.contains_key("schema_version"));
    }

    #[test]
    fn test_encode_v3_branchless_track_preserves_null_branch_field() {
        let json = r#"{
  "schema_version": 3,
  "id": "plan-only-track",
  "branch": null,
  "title": "Plan Only Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Todo task", "status": "todo"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}
    ]
  }
}"#;
        let (track, meta) = decode(json).unwrap();
        let re_encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();

        assert!(doc.get("branch").is_some());
        assert!(doc["branch"].is_null());
    }

    #[test]
    fn test_decode_with_review_section_round_trips() {
        let json = r#"{
  "schema_version": 3,
  "id": "review-track",
  "branch": "track/review-track",
  "title": "Review Track",
  "status": "in_progress",
  "created_at": "2026-03-18T00:00:00Z",
  "updated_at": "2026-03-18T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Task", "status": "in_progress"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}
    ]
  },
  "review": {
    "status": "fast_passed",
    "code_hash": "abc123def",
    "groups": {
      "infra-domain": {
        "fast": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}
      }
    }
  }
}"#;
        let (track, meta) = decode(json).unwrap();
        let review = track.review().unwrap();
        assert_eq!(review.status(), domain::ReviewStatus::FastPassed);
        assert_eq!(review.code_hash(), Some("abc123def"));
        let infra_domain_group = domain::ReviewGroupName::try_new("infra-domain").unwrap();
        assert!(review.groups().get(&infra_domain_group).is_some());
        assert!(review.groups().get(&infra_domain_group).unwrap().fast().is_some());

        // Round-trip
        let re_encoded = encode(&track, &meta).unwrap();
        let (track2, _) = decode(&re_encoded).unwrap();
        assert_eq!(track, track2);
    }

    #[test]
    fn test_decode_without_review_section_backward_compatible() {
        // Existing schema_version 3 without review section should still parse
        let json = r#"{
  "schema_version": 3,
  "id": "compat-track",
  "branch": "track/compat-track",
  "title": "Compat Track",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Task", "status": "todo"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}
    ]
  }
}"#;
        let (track, _) = decode(json).unwrap();
        assert!(track.review().is_none());
    }

    #[test]
    fn test_decode_review_with_fast_and_final_rounds() {
        let json = r#"{
  "schema_version": 3,
  "id": "full-review-track",
  "branch": "track/full-review-track",
  "title": "Full Review Track",
  "status": "in_progress",
  "created_at": "2026-03-18T00:00:00Z",
  "updated_at": "2026-03-18T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Task", "status": "in_progress"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}
    ]
  },
  "review": {
    "status": "approved",
    "code_hash": "def456",
    "groups": {
      "g1": {
        "fast": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"},
        "final": {"round": 2, "verdict": "zero_findings", "timestamp": "2026-03-18T02:00:00Z"}
      }
    }
  }
}"#;
        let (track, _) = decode(json).unwrap();
        let review = track.review().unwrap();
        assert_eq!(review.status(), domain::ReviewStatus::Approved);
        let g1_key = domain::ReviewGroupName::try_new("g1").unwrap();
        let g1 = review.groups().get(&g1_key).unwrap();
        assert_eq!(g1.fast().unwrap().round(), 1);
        assert_eq!(g1.final_round().unwrap().round(), 2);
    }

    #[test]
    fn test_decode_review_final_only_group_round_trips_without_fake_fast() {
        // Ensures that a group with only a final round does NOT synthesize a fake fast entry.
        let json = r#"{
  "schema_version": 3,
  "id": "final-only-track",
  "branch": "track/final-only-track",
  "title": "Final Only Track",
  "status": "in_progress",
  "created_at": "2026-03-18T00:00:00Z",
  "updated_at": "2026-03-18T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Task", "status": "in_progress"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}
    ]
  },
  "review": {
    "status": "fast_passed",
    "code_hash": "abc123def",
    "groups": {
      "g1": {
        "final": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T02:00:00Z"}
      }
    }
  }
}"#;
        let (track, meta) = decode(json).unwrap();
        let review = track.review().unwrap();
        let g1_key = domain::ReviewGroupName::try_new("g1").unwrap();
        let g1 = review.groups().get(&g1_key).unwrap();
        // fast must be None — no synthetic fast round
        assert!(g1.fast().is_none(), "final-only group must not have a synthetic fast round");
        assert!(g1.final_round().is_some());

        // Round-trip: re-encode and decode, verify fast is still absent
        let re_encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();
        let g1_json = &doc["review"]["groups"]["g1"];
        assert!(
            g1_json.get("fast").is_none() || g1_json["fast"].is_null(),
            "re-encoded JSON must not contain a fast entry for final-only group"
        );
    }

    #[test]
    fn test_decode_review_invalid_status_returns_error() {
        let json = r#"{
  "schema_version": 3,
  "id": "bad-review-track",
  "branch": "track/bad-review-track",
  "title": "Bad Review Track",
  "status": "planned",
  "created_at": "2026-03-18T00:00:00Z",
  "updated_at": "2026-03-18T00:00:00Z",
  "tasks": [
    {"id": "T1", "description": "Task", "status": "todo"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}
    ]
  },
  "review": {
    "status": "unknown_status",
    "groups": {}
  }
}"#;
        let result = decode(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_review_groups_serialized_in_deterministic_key_order() {
        // Encode a review with multiple groups inserted in non-alphabetical order.
        // The serialized JSON must list them in alphabetical (BTreeMap) order every time.
        let json_template = r#"{
  "schema_version": 3,
  "id": "order-track",
  "branch": "track/order-track",
  "title": "Order Track",
  "status": "in_progress",
  "created_at": "2026-03-18T00:00:00Z",
  "updated_at": "2026-03-18T00:00:00Z",
  "tasks": [{"id": "T1", "description": "Task", "status": "in_progress"}],
  "plan": {"summary": [], "sections": [{"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}]},
  "review": {
    "status": "fast_passed",
    "code_hash": "abc",
    "groups": {
      "zzz-group": {"fast": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}},
      "aaa-group": {"fast": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}}
    }
  }
}"#;
        let (track, meta) = decode(json_template).unwrap();
        let encoded1 = encode(&track, &meta).unwrap();
        let encoded2 = encode(&track, &meta).unwrap();

        // Encoding must be deterministic
        assert_eq!(encoded1, encoded2);

        // Groups must appear in alphabetical order in the JSON
        let aaa_pos = encoded1.find("\"aaa-group\"").unwrap();
        let zzz_pos = encoded1.find("\"zzz-group\"").unwrap();
        assert!(aaa_pos < zzz_pos, "groups should be in alphabetical order");
    }

    #[test]
    fn test_encode_review_none_omits_review_field() {
        let (track, meta) = decode(sample_json()).unwrap();
        assert!(track.review().is_none());
        let encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert!(doc.get("review").is_none());
    }

    // --- Escalation state tests ---

    fn review_json_with_escalation(escalation_json: &str) -> String {
        format!(
            r#"{{
  "schema_version": 3,
  "id": "escalation-track",
  "branch": "track/escalation-track",
  "title": "Escalation Track",
  "status": "in_progress",
  "created_at": "2026-03-19T00:00:00Z",
  "updated_at": "2026-03-19T00:00:00Z",
  "tasks": [{{"id": "T1", "description": "Task", "status": "in_progress"}}],
  "plan": {{"summary": [], "sections": [{{"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}}]}},
  "review": {{
    "status": "not_started",
    "groups": {{}},
    {escalation_json}
  }}
}}"#
        )
    }

    #[test]
    fn test_escalation_state_round_trip() {
        // Create a metadata with escalation blocked state via JSON
        let json = review_json_with_escalation(
            r#""escalation": {
      "threshold": 3,
      "phase": {"type": "blocked", "concerns": ["shell-parsing"], "blocked_at": "2026-03-19T00:00:00Z"},
      "recent_cycles": [
        {"round_type": "fast", "round": 3, "timestamp": "2026-03-19T00:00:00Z", "concerns": ["shell-parsing"], "groups": ["g1"]}
      ],
      "concern_streaks": {
        "shell-parsing": {"consecutive_rounds": 3, "last_round_type": "fast", "last_round": 3, "last_seen_at": "2026-03-19T00:00:00Z"}
      },
      "last_resolution": null
    }"#,
        );
        let (track, meta) = decode(&json).unwrap();
        let review = track.review().unwrap();
        let escalation = review.escalation();

        assert!(escalation.is_blocked());
        assert_eq!(escalation.threshold(), 3);
        assert_eq!(escalation.recent_cycles().len(), 1);
        assert_eq!(escalation.concern_streaks().len(), 1);
        assert!(escalation.last_resolution().is_none());

        // Round-trip: encode → decode → verify
        let re_encoded = encode(&track, &meta).unwrap();
        let (track2, _) = decode(&re_encoded).unwrap();
        assert_eq!(track.review().unwrap().escalation(), track2.review().unwrap().escalation());
    }

    #[test]
    fn test_escalation_absent_deserializes_to_default() {
        // Parse existing metadata.json format without escalation field
        let json = review_json_with_escalation(r#""code_hash": null"#);
        let (track, _) = decode(&json).unwrap();
        let review = track.review().unwrap();
        let escalation = review.escalation();

        assert!(!escalation.is_blocked());
        assert_eq!(escalation.threshold(), 3);
        assert!(escalation.recent_cycles().is_empty());
        assert!(escalation.concern_streaks().is_empty());
        assert!(escalation.last_resolution().is_none());
    }

    #[test]
    fn test_round_document_with_concerns_round_trips() {
        // Round result with concerns encodes/decodes correctly
        let json = r#"{
  "schema_version": 3,
  "id": "concerns-track",
  "branch": "track/concerns-track",
  "title": "Concerns Track",
  "status": "in_progress",
  "created_at": "2026-03-19T00:00:00Z",
  "updated_at": "2026-03-19T00:00:00Z",
  "tasks": [{"id": "T1", "description": "Task", "status": "in_progress"}],
  "plan": {"summary": [], "sections": [{"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}]},
  "review": {
    "status": "not_started",
    "groups": {
      "g1": {
        "fast": {"round": 1, "verdict": "findings_remain", "timestamp": "2026-03-19T00:00:00Z", "concerns": ["shell-parsing", "domain.review"]}
      }
    }
  }
}"#;
        let (track, meta) = decode(json).unwrap();
        let review = track.review().unwrap();
        let g1_key = domain::ReviewGroupName::try_new("g1").unwrap();
        let fast = review.groups().get(&g1_key).unwrap().fast().unwrap();
        assert_eq!(fast.concerns().len(), 2);
        assert_eq!(fast.concerns()[0].as_ref(), "shell-parsing");
        assert_eq!(fast.concerns()[1].as_ref(), "domain.review");

        // Round-trip
        let re_encoded = encode(&track, &meta).unwrap();
        let (track2, _) = decode(&re_encoded).unwrap();
        let fast2 = track2.review().unwrap().groups().get(&g1_key).unwrap().fast().unwrap().clone();
        assert_eq!(fast2.concerns().len(), 2);
        assert_eq!(fast2.concerns()[0].as_ref(), "shell-parsing");
    }

    #[test]
    fn test_round_document_without_concerns_backward_compatible() {
        // Existing round doc without concerns field still parses (empty vec)
        let json = r#"{
  "schema_version": 3,
  "id": "compat-concerns-track",
  "branch": "track/compat-concerns-track",
  "title": "Compat Concerns Track",
  "status": "in_progress",
  "created_at": "2026-03-19T00:00:00Z",
  "updated_at": "2026-03-19T00:00:00Z",
  "tasks": [{"id": "T1", "description": "Task", "status": "in_progress"}],
  "plan": {"summary": [], "sections": [{"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}]},
  "review": {
    "status": "fast_passed",
    "code_hash": "abc123def",
    "groups": {
      "g1": {
        "fast": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-19T00:00:00Z"}
      }
    }
  }
}"#;
        let (track, _) = decode(json).unwrap();
        let review = track.review().unwrap();
        let g1_key = domain::ReviewGroupName::try_new("g1").unwrap();
        let fast = review.groups().get(&g1_key).unwrap().fast().unwrap();
        // No concerns field in JSON → empty vec
        assert!(fast.concerns().is_empty());
    }

    #[test]
    fn test_escalation_blocked_state_encodes_and_decodes() {
        // Test that a blocked escalation with resolution round-trips correctly
        let json = review_json_with_escalation(
            r#""escalation": {
      "threshold": 3,
      "phase": {"type": "blocked", "concerns": ["domain.review"], "blocked_at": "2026-03-19T00:00:00Z"},
      "last_resolution": {
        "blocked_concerns": ["domain.review"],
        "workspace_search_ref": "search.md",
        "reinvention_check_ref": "reinvention.md",
        "decision": "continue_self_implementation",
        "summary": "No suitable crate found",
        "resolved_at": "2026-03-19T01:00:00Z"
      }
    }"#,
        );
        let (track, meta) = decode(&json).unwrap();
        let escalation = track.review().unwrap().escalation();
        assert!(escalation.is_blocked());
        let resolution = escalation.last_resolution().unwrap();
        assert_eq!(
            resolution.decision(),
            domain::ReviewEscalationDecision::ContinueSelfImplementation
        );
        assert_eq!(resolution.summary(), "No suitable crate found");

        // Round-trip
        let re_encoded = encode(&track, &meta).unwrap();
        let (track2, _) = decode(&re_encoded).unwrap();
        assert_eq!(track.review().unwrap().escalation(), track2.review().unwrap().escalation());
    }

    #[test]
    fn test_default_escalation_omitted_from_serialized_json() {
        // A track with review but default (Clear) escalation should not serialize escalation field
        let json = r#"{
  "schema_version": 3,
  "id": "no-escalation-track",
  "branch": "track/no-escalation-track",
  "title": "No Escalation Track",
  "status": "in_progress",
  "created_at": "2026-03-19T00:00:00Z",
  "updated_at": "2026-03-19T00:00:00Z",
  "tasks": [{"id": "T1", "description": "Task", "status": "in_progress"}],
  "plan": {"summary": [], "sections": [{"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}]},
  "review": {
    "status": "not_started",
    "groups": {}
  }
}"#;
        let (track, meta) = decode(json).unwrap();
        let encoded = encode(&track, &meta).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        // Default escalation must not appear in output
        assert!(
            doc["review"].get("escalation").is_none(),
            "escalation should be omitted when default"
        );
    }

    #[test]
    fn test_escalation_decision_all_variants_round_trip() {
        for decision_str in
            &["adopt_workspace_solution", "adopt_external_crate", "continue_self_implementation"]
        {
            let json = review_json_with_escalation(&format!(
                r#""escalation": {{
      "threshold": 3,
      "phase": {{"type": "blocked", "concerns": ["test-concern"], "blocked_at": "2026-03-19T00:00:00Z"}},
      "last_resolution": {{
        "blocked_concerns": ["test-concern"],
        "workspace_search_ref": "s.md",
        "reinvention_check_ref": "r.md",
        "decision": "{decision_str}",
        "summary": "ok",
        "resolved_at": "2026-03-19T00:00:00Z"
      }}
    }}"#
            ));
            let (track, meta) = decode(&json).unwrap();
            let re_encoded = encode(&track, &meta).unwrap();
            assert!(
                re_encoded.contains(decision_str),
                "decision '{decision_str}' should survive round-trip"
            );
        }
    }

    // --- Finding 2: round_result_from_document verdict/concerns consistency ---

    #[test]
    fn test_round_result_from_document_strips_concerns_on_zero_findings() {
        // Persisted data has zero_findings verdict but non-empty concerns (legacy/corrupt data).
        // The codec must strip concerns rather than reject the document.
        let json = r#"{
  "schema_version": 3,
  "id": "legacy-track",
  "branch": "track/legacy-track",
  "title": "Legacy Track",
  "status": "in_progress",
  "created_at": "2026-03-19T00:00:00Z",
  "updated_at": "2026-03-19T00:00:00Z",
  "tasks": [{"id": "T1", "description": "Task", "status": "in_progress"}],
  "plan": {"summary": [], "sections": [{"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}]},
  "review": {
    "status": "fast_passed",
    "code_hash": "abc",
    "groups": {
      "g1": {
        "fast": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-19T00:00:00Z", "concerns": ["stale-concern"]}
      }
    }
  }
}"#;
        let (track, _) = decode(json).unwrap();
        let g1_key = domain::ReviewGroupName::try_new("g1").unwrap();
        let fast = track.review().unwrap().groups().get(&g1_key).unwrap().fast().unwrap();
        // zero_findings must have empty concerns even if persisted data had non-empty
        assert!(
            fast.concerns().is_empty(),
            "zero_findings verdict must have concerns stripped at load time"
        );
    }

    #[test]
    fn test_round_result_from_document_adds_other_for_findings_remain_without_concerns() {
        // Persisted data has findings_remain verdict but no concerns (legacy data).
        // The codec must add a fallback "other" concern rather than failing.
        let json = r#"{
  "schema_version": 3,
  "id": "legacy-findings-track",
  "branch": "track/legacy-findings-track",
  "title": "Legacy Findings Track",
  "status": "in_progress",
  "created_at": "2026-03-19T00:00:00Z",
  "updated_at": "2026-03-19T00:00:00Z",
  "tasks": [{"id": "T1", "description": "Task", "status": "in_progress"}],
  "plan": {"summary": [], "sections": [{"id": "S1", "title": "Section", "description": [], "task_ids": ["T1"]}]},
  "review": {
    "status": "not_started",
    "groups": {
      "g1": {
        "fast": {"round": 1, "verdict": "findings_remain", "timestamp": "2026-03-19T00:00:00Z"}
      }
    }
  }
}"#;
        let (track, _) = decode(json).unwrap();
        let g1_key = domain::ReviewGroupName::try_new("g1").unwrap();
        let fast = track.review().unwrap().groups().get(&g1_key).unwrap().fast().unwrap();
        // findings_remain without concerns must get a fallback "other" concern
        assert_eq!(fast.concerns().len(), 1, "findings_remain must have at least one concern");
        assert_eq!(fast.concerns()[0].as_ref(), "other");
    }

    // --- Finding 3: resolution_from_document empty field validation ---

    fn resolution_json(
        workspace_search_ref: &str,
        reinvention_check_ref: &str,
        summary: &str,
        resolved_at: &str,
    ) -> String {
        review_json_with_escalation(&format!(
            r#""escalation": {{
      "threshold": 3,
      "phase": {{"type": "blocked", "concerns": ["test-concern"], "blocked_at": "2026-03-19T00:00:00Z"}},
      "last_resolution": {{
        "blocked_concerns": ["test-concern"],
        "workspace_search_ref": "{workspace_search_ref}",
        "reinvention_check_ref": "{reinvention_check_ref}",
        "decision": "continue_self_implementation",
        "summary": "{summary}",
        "resolved_at": "{resolved_at}"
      }}
    }}"#
        ))
    }

    #[test]
    fn test_resolution_from_document_rejects_empty_workspace_search_ref() {
        let json = resolution_json("", "r.md", "summary text", "2026-03-19T00:00:00Z");
        let result = decode(&json);
        assert!(result.is_err(), "empty workspace_search_ref must be rejected");
    }

    #[test]
    fn test_resolution_from_document_rejects_empty_reinvention_check_ref() {
        let json = resolution_json("s.md", "", "summary text", "2026-03-19T00:00:00Z");
        let result = decode(&json);
        assert!(result.is_err(), "empty reinvention_check_ref must be rejected");
    }

    #[test]
    fn test_resolution_from_document_rejects_empty_summary() {
        let json = resolution_json("s.md", "r.md", "", "2026-03-19T00:00:00Z");
        let result = decode(&json);
        assert!(result.is_err(), "empty summary must be rejected");
    }

    #[test]
    fn test_resolution_from_document_rejects_empty_resolved_at() {
        let json = resolution_json("s.md", "r.md", "summary text", "");
        let result = decode(&json);
        assert!(result.is_err(), "empty resolved_at must be rejected");
    }
}
