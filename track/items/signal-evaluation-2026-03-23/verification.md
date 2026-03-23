---
status: pending
---

# Verification: Spec Signal Evaluation — Stage 1

## Scope Verified

- [ ] spec.md の In Scope / Out of Scope が実装と一致

## Manual Verification Steps

1. `ConfidenceSignal` enum に `#[non_exhaustive]` が付与されていること
2. `SignalBasis` enum の全 variant が source-attribution.md のタグパターンと対応していること。以下の全パターンが正しい信号レベルに評価されること:
   - `[source: <doc> §<section>]` → Document / Blue
   - `[source: <doc>]` (§ なし) → Document / Blue
   - `[source: feedback — ...]` → Feedback / Blue
   - `[source: convention — ...]` → Convention / Blue
   - `[source: discussion]` or `[source: discussion — ...]` → Discussion / Yellow
   - `[source: inference — ...]` → Inference / Yellow
   - source tag なし → MissingSource / Red
3. 信号評価の対象が Scope / Constraints / Acceptance Criteria セクションに限定されていること（Goal やコード例は対象外）
4. `sotp track signals` が spec.md を評価し metadata.json `spec_signals` に格納すること
5. `sotp verify spec-signals` が spec.md body を実評価し `red > 0` でエラーを返すこと
6. `sotp verify spec-states` が `## Domain States` セクション未存在時にエラーを返すこと
6a. `sotp verify spec-states` が `## Domain States` テーブルにデータ行がない場合にエラーを返すこと
7. `cargo make ci` が通ること
8. `cargo make llvm-cov` で新規コードのテストカバレッジが 80% 以上であること

## Deferred

- Multi-source (comma-separated) tag support → JSON SSoT track
- Frontmatter `signals:` drift check → JSON SSoT track

## Result / Open Issues

_実装後に記入_

## verified_at

_検証後に記入_
