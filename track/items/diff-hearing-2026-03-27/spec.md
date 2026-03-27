<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
signals: { blue: 8, yellow: 6, red: 0 }
---

# TSUMIKI-03 差分ヒアリング

## Goal

/track:plan skill のヒアリングプロセスに差分ヒアリングを導入し、既存コンテキストから不足・曖昧な情報のみをユーザーに質問する。
毎回ゼロから聞き直す無駄を省き、仕様品質に直結する gap に集中する。

## Scope

### In Scope
- SKILL.md Phase 1 Step 3 の改修 — 既存 spec.json 検出時にシグナル分類を実施 [source: Tsumiki kairo-requirements Step 4, TODO-PLAN 2-5] [tasks: T001]
- SKILL.md Phase 1 Step 4 の改修 — 固定質問リストから差分ヒアリングフローへの置換 [source: Tsumiki kairo-requirements Step 4, TODO-PLAN-v4-draft: plan skill プロンプト変更] [tasks: T002]
- SKILL.md Phase 3 Step 3 の改修 — 差分ヒアリング結果の提示形式追加 [source: inference — Tsumiki の interview-record.md パターンを /track:plan の提示ステップに適用] [tasks: T003]
- 既存 spec.json がない場合は従来の全体ヒアリングにフォールバック [source: inference — 後方互換性の確保] [tasks: T002]

### Out of Scope
- Rust コード（CLI コマンド、domain 型、usecase）の追加・変更 [source: TODO-PLAN-v4-draft: plan skill プロンプト変更]
- spec.json スキーマの変更 [source: inference — 既存の ConfidenceSignal + source tags で差分判定が可能]
- Tsumiki の interview-record.md 相当の専用ファイル生成 [source: inference — S 難易度のスコープ制約、verification.md で代替可能]

## Constraints
- 既存の信号機システム（ConfidenceSignal, SignalBasis, source tags）を差分判定の基盤とすること [source: convention — source-attribution.md, ADR 2026-03-23-1010-three-level-signals]
- Tsumiki のオリジナル概念（既存ドキュメントとの差分のみを聞く）に忠実であること [source: Tsumiki kairo-requirements Step 4, tsumiki-analysis-2026-03-17.md]
- SKILL.md の変更のみで完結すること（難易度 S） [source: TODO-PLAN-v4-draft: plan skill プロンプト変更]

## Acceptance Criteria
- [ ] 既存 spec.json がある track で /track:plan を実行すると、Blue signal の項目はスキップされ Yellow/Red/欠落の項目のみ質問される [source: Tsumiki kairo-requirements Step 4] [tasks: T001, T002]
- [ ] 既存 spec.json がない新規 track で /track:plan を実行すると、従来通りの全体ヒアリングが行われる [source: inference — 後方互換性] [tasks: T002]
- [ ] Phase 3 の提示で、確定済み項目と新規確認項目が明確に区別される [source: inference — Tsumiki の interview-record.md の可視化パターンを応用] [tasks: T003]
- [ ] cargo make ci が全チェック通過する [source: convention — task-completion-flow.md] [tasks: T004]

## Related Conventions (Required Reading)
- project-docs/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 8  🟡 6  🔴 0

