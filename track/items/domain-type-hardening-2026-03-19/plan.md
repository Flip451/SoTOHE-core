<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Domain Type Hardening: Replace stringly-typed fields with validated domain types

Replace stringly-typed domain fields with validated types: Verdict enum, CodeHash enum, Timestamp newtype, NonEmptyString newtype.
Follows prefer-type-safe-abstractions convention to make illegal states unrepresentable.
T001-T004 are independent domain-layer changes; T005-T007 propagate changes through infrastructure/usecase/cli layers.

## Verdict enum (P1)

Define enum Verdict { ZeroFindings, FindingsRemain } in review.rs
Replace ReviewRoundResult::verdict: String with Verdict
Replace all string comparisons (r.verdict == "zero_findings" and "findings_remain" in validate_verdict_concerns) with enum matching
Add Display impl mapping to snake_case strings for serialization compatibility

- [x] Introduce Verdict enum replacing ReviewRoundResult::verdict String field 272c0c4

## CodeHash enum (P2)

Define enum CodeHash { Computed(String), Pending } in review.rs
Replace ReviewState::code_hash: Option<String> with Option<CodeHash>
Remove "PENDING" magic string from record_round_with_pending and related logic
CodeHash::Computed validates non-empty string

- [x] Introduce CodeHash enum replacing ReviewState::code_hash Option<String> with PENDING sentinel 272c0c4

## Timestamp newtype (P3)

Define Timestamp newtype wrapping chrono::DateTime<Utc> in ids.rs (stores parsed DateTime + original RFC 3339 string)
Validate via str::parse::<DateTime<Utc>>() (relaxed RFC 3339: accepts space separator, rejects invalid dates/times)
Replace 5 timestamp String fields: ReviewRoundResult.timestamp, ReviewCycleSummary.timestamp, ReviewConcernStreak.last_seen_at, ReviewEscalationBlock.blocked_at, ReviewEscalationResolution.resolved_at

- [x] Introduce Timestamp newtype replacing bare String timestamps in review types 272c0c4

## NonEmptyString newtype (P4)

Define NonEmptyString(String) newtype with trimmed non-empty validation
Replace TrackMetadata::title and TrackTask::description with NonEmptyString
Remove inline non-empty checks from TrackMetadata::with_branch and TrackTask::with_status constructors (actual validation sites)

- [x] Introduce NonEmptyString newtype for TrackMetadata::title and TrackTask::description 272c0c4

## Layer propagation

Update infrastructure codec (TrackDocumentV2 serde) to map between JSON strings and new domain types
Maintain backward compatibility with existing metadata.json files
Update usecase layer type references
Update cli layer command handlers

- [x] Update infrastructure codec layer for new domain types (serde backward compatibility) 272c0c4
- [x] Update usecase layer for new domain types 272c0c4
- [x] Update cli layer for new domain types 272c0c4
