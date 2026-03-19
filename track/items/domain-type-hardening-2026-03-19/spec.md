# Spec: Domain Type Hardening

## Goal

`libs/domain/` 内の stringly-typed フィールドを検証済みドメイン型に置き換え、不正な状態を型システムで構築不能にする。

## Scope

- `ReviewRoundResult::verdict: String` → `Verdict` enum [source: review.rs L421, L664, L789, L829-839]
- `ReviewState::code_hash: Option<String>` + `"PENDING"` sentinel → `CodeHash` enum [source: review.rs L510, L768]
- 5 箇所の `timestamp: String` → `Timestamp` newtype [source: review.rs — ReviewRoundResult.timestamp, ReviewCycleSummary.timestamp, ReviewConcernStreak.last_seen_at, ReviewEscalationBlock.blocked_at, ReviewEscalationResolution.resolved_at]
- `TrackMetadata::title: String`, `TrackTask::description: String` → `NonEmptyString` newtype [source: track.rs]
- `infrastructure` / `usecase` / `cli` 層の型変更対応

## Constraints

- 既存 `metadata.json` ファイルとの serde 後方互換性を維持すること [source: convention — typed-deserialization.md]
- domain 層に serde 依存を追加しないこと（codec は infrastructure 層に留める） [source: architecture-rules.json]
- 既存テスト（1000+ 件）を壊さないこと [source: convention — 05-testing.md]
- TDD で進めること（Red → Green → Refactor） [source: feedback — feedback_tdd_enforcement.md]
- `chrono` は domain 層に入れない。Timestamp の検証は regex なしの手書きパースで行う [source: tech-stack.md — chrono は infrastructure 層のみ]

## Acceptance Criteria

1. `Verdict::ZeroFindings` / `Verdict::FindingsRemain` で review verdict が型安全に表現される
2. `CodeHash::Pending` / `CodeHash::Computed(_)` で PENDING sentinel が型レベルで表現される
3. `Timestamp` newtype で空文字列や明らかに不正な形式のタイムスタンプが構築時に拒否される
4. `NonEmptyString` で空文字列の title/description が構築時に拒否される
5. review.rs 内のすべての verdict 文字列比較（`"zero_findings"` および `validate_verdict_concerns` 内の `"findings_remain"`）が enum match に置換される
6. 既存 metadata.json が変更なく読み込めること（serde 後方互換）
7. `cargo make ci` が通過すること

## Related Conventions (Required Reading)

- `project-docs/conventions/prefer-type-safe-abstractions.md`
- `project-docs/conventions/typed-deserialization.md`
