<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
signals: { blue: 20, yellow: 14, red: 0 }
---

# SPEC-05 Domain States 信号機 Stage 2 — per-state signal + 遷移関数検証

## Goal

spec.json の Domain States を domain コードと照合し、per-state の信号 (Blue/Yellow/Red) を自動評価する。
型の存在だけでなく、宣言された状態遷移を表す関数の存在も検証し、主観を排除した客観的な品質指標を提供する。

## Scope

### In Scope
- DomainStateEntry に transitions_to フィールド追加 (省略=未定義, []=終端) [source: discussion]
- Per-state 信号評価: 型存在 × 遷移関数存在 → Blue/Yellow/Red [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- syn AST スキャナー: domain コードから型名 + 遷移関数を自動検出 [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- Result/Option 型のアンラップで遷移先判定 [source: discussion]
- transitions_to 参照先の domain_states 内存在チェック [source: discussion]
- spec.json に domain_state_signals フィールド追加 [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- sotp track domain-state-signals CLI コマンド [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- sotp verify spec-states に red==0 gate + Stage 1 前提条件追加 [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- render_spec() の Domain States テーブルに Signal + Transitions 列追加 [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- plan.md に Stage 1 + Stage 2 信号サマリー表示 [source: TODO-PLAN-2026-03-22 §Phase2 2-2]

### Out of Scope
- 未宣言遷移の検出 (code→spec 逆方向チェック) [source: inference — Phase 3 spec ↔ code 整合性で対応]
- モジュールスコープによる同名型の曖昧性解消 [source: inference — Phase 3 BRIDGE-01 で対応]
- TrackMetadata への spec_signals 型化 [source: inference — Stage 2 完了後に別途検討]

## Constraints
- domain 層は I/O を含まない (hexagonal purity) [source: convention — hexagonal-architecture.md]
- syn AST パースは infrastructure 層に配置 [source: convention — hexagonal-architecture.md]
- Stage 1 (spec signals red==0) が前提条件 [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- 終端状態 (transitions_to: []) は型存在のみで Blue 判定可 [source: discussion]
- transitions_to 省略は未定義扱い (最大でも Yellow) [source: discussion]
- 遷移関数の検出は fn の引数型/self型に state A、戻り値の Result/Option 内部型に state B を含む場合に A→B 遷移として認識 [source: discussion]
- spec.json (仕様 SSoT) の schema_version 1 に後方互換フィールドとして追加。metadata.json (トラック SSoT, schema_version 3) とは別体系 [source: discussion]

## Domain States

| State | Description | Signal | Transitions |
|-------|-------------|--------|-------------|
| DomainStateEntry | spec.json の各状態エントリ。name + description + transitions_to を保持 | 🔵 | ∅ (terminal) |
| DomainStateSignal | per-state の信号判定結果: state_name + signal (Blue/Yellow/Red) + found_type (bool) + found_transitions (Vec) + missing_transitions (Vec) | 🔵 | ∅ (terminal) |
| CodeScanResult | syn スキャン結果: 検出された型名 Set + 遷移関数マップ (from_state → to_states) | 🔵 | ∅ (terminal) |

## Acceptance Criteria
- [ ] DomainStateEntry に transitions_to フィールドが追加されている [source: discussion]
- [ ] transitions_to が省略/空配列/値ありの3パターンで正しく区別される [source: discussion]
- [ ] transitions_to の参照先が domain_states に存在しない場合にバリデーションエラーが発生する [source: discussion]
- [ ] syn AST スキャンで enum variant / struct 名が正しく検出される [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- [ ] syn AST スキャンで遷移関数が正しく検出される (self型→Result内部型) [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- [ ] Result/Option 内部型のアンラップが正しく動作する [source: discussion]
- [ ] Blue 判定: 型存在 AND (終端 OR 全宣言遷移関数存在) [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- [ ] Yellow 判定: 型存在だが遷移未発見、または transitions_to 未宣言 [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- [ ] Red 判定: 型未存在、またはプレースホルダー [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- [ ] sotp track domain-state-signals が spec.json の domain_state_signals を更新する [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- [ ] sotp verify spec-states が red==0 gate を適用する [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- [ ] sotp verify spec-states が Stage 1 前提条件を検証する [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- [ ] rendered spec.md の Domain States テーブルに Signal と Transitions 列が表示される [source: TODO-PLAN-2026-03-22 §Phase2 2-2]
- [ ] cargo make ci が全テスト通過する [source: convention — hexagonal-architecture.md]

## Related Conventions (Required Reading)
- project-docs/conventions/hexagonal-architecture.md
- project-docs/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 20  🟡 14  🔴 0

### Stage 2: Domain State Signals
🔵 3  🟡 0  🔴 0

