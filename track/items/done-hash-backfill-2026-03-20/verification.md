# Verification: done-hash-backfill-2026-03-20

## Scope Verified

### Phase A: TaskStatus Split (WF-40)
- [ ] domain: DonePending/DoneTraced split and BackfillHash transition
- [ ] usecase: resolve_transition accepts &TaskStatus, backfill path works
- [ ] infrastructure: codec round-trip, render markers
- [ ] CLI: PR guard, task counts
- [ ] CLI: transition.rs passes full TaskStatus through execute_by_status → resolve_transition path
- [ ] docs: task-completion-flow.md updated (WF-40 constraint removed)
- [ ] docs: TODO.md WF-40 entry marked as resolved

### Phase B: Track Phase Resolution
- [ ] domain: resolve_phase and resolve_phase_from_record accept TrackStatus enum
- [ ] domain: NextCommand enum with Display impl
- [ ] infrastructure: resolve.rs uses NextCommand, render.rs uses NextCommand for registry
- [ ] infrastructure: resolve_phase uses TrackStatus enum matching internally (production callers: resolve.rs, render.rs)
- [ ] infrastructure: unknown status token at parse boundary returns error (negative test)
- [ ] CLI: /track:resolve output uses NextCommand

### Phase C: Review System ADTs
- [ ] domain: CodeHash::NotRecorded replaces Option<CodeHash>
- [ ] domain: ReviewGroupProgress ADT with FinalOnly for legacy data
- [ ] domain: ReviewEscalationResolution uses NonEmptyString (constructor validates)
- [ ] infrastructure/codec: review_from_document/review_to_document updated for CodeHash, ReviewGroupProgress
- [ ] codec: CodeHash::NotRecorded serializes as field omission (not null)
- [ ] CLI: apps/cli/src/commands/review.rs uses updated ReviewEscalationResolution::new() constructor

### Phase D: Minor Cleanups
- [ ] domain: AutoPhaseError uses AutoPhase enum (not String)
- [ ] domain: StatusOverride struct with StatusOverrideKind + NonEmptyString
- [ ] domain: ReviewGroupName newtype throughout (Vec, HashMap keys, record_round group param, expected_groups param)
- [ ] infrastructure/codec: StatusOverride codec updated for struct form
- [ ] infrastructure/codec: ReviewGroupName codec round-trip
- [ ] CLI: AutoPhaseError callers updated for AutoPhase typed fields
- [ ] CLI: review.rs group/expected_groups params use ReviewGroupName

## Manual Verification Steps

1. `cargo make test` — all tests pass
2. `cargo make ci` — full CI green
3. Phase A: `cargo make track-transition` で DonePending → DoneTraced backfill が成功する
4. Phase A: `cargo make track-transition` で DoneTraced → DoneTraced が拒否される
5. Phase A: `cargo make track-transition` の実コマンドパス（execute_by_status → resolve_transition）で backfill が動作する
6. Phase B: `cargo make track-resolve` で NextCommand enum 経由の出力を確認
7. Phase B: `cargo make track-sync-views` で registry.md の next_command が正常
8. Phase B: unknown status token のパースが runtime error を返す（ユニットテスト）
9. Phase C: 既存 metadata.json（review セクション付き）が正常にデシリアライズされる
10. Phase C: final-only review group を含む metadata.json が FinalOnly として読み込める
11. Phase C: CodeHash::NotRecorded の JSON round-trip でフィールド省略が維持される
12. Phase C: ReviewEscalationResolution に空文字列を渡すと構築時エラーになる
13. Phase D: StatusOverride の empty reason が構築時エラーになる
14. Phase D: AutoPhaseError の from/phase/to フィールドがすべて AutoPhase 型であることをテストで確認
15. Phase D: ReviewGroupName newtype が codec round-trip で正常に動作する
16. Phase D: record_round / record_round_with_pending の group と expected_groups の両方が ReviewGroupName を使用する
17. metadata.json の JSON 出力が既存フォーマットと互換（全 Phase）

## Result

- pending

## Open Issues

- none

## verified_at

- pending
