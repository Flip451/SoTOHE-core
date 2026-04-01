<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
signals: { blue: 22, yellow: 2, red: 0 }
---

# TSUMIKI-05/06/07 ヒアリング UX 改善

## Goal

/track:plan skill のヒアリング UX を tsumiki の優れたパターンに基づいて改善する。
構造化質問（AskUserQuestion + multiSelect）、作業規模選定（Full/Focused/Quick）、ヒアリング記録（hearing_history）を導入し、
Phase 3（テスト生成パイプライン）に渡す spec の入力品質を向上させる。

## Scope

### In Scope
- SKILL.md Phase 1 の前に Step 0（モード選択）を挿入: Full/Focused/Quick の 3 モード [source: tsumiki kairo-requirements Stage 2 (Full/Lightweight/Custom), knowledge/research/tsumiki-hearing-deep-dive-2026-04-01.md §6 Priority 2] [tasks: T001]
- Focused/Quick モードでの Phase 1.5（planner review）・Phase 2（Agent Teams）スキップを明示的例外として定義 [source: knowledge/research/2026-04-01-1546-planner-hearing-ux.md §2 TSUMIKI-06 Q2/Q3] [tasks: T001]
- HearingMode enum (Full/Focused/Quick), HearingSignalSnapshot, HearingSignalDelta, HearingRecord を domain 層に追加 [source: knowledge/research/2026-04-01-1546-planner-hearing-ux.md §Canonical Blocks] [tasks: T002]
- spec.json に hearing_history フィールドを追加（append-only、content_hash 計算から除外） [source: knowledge/research/tsumiki-hearing-deep-dive-2026-04-01.md §6 Priority 3, knowledge/research/2026-04-01-1546-planner-hearing-ux.md §2 TSUMIKI-07 Q1/Q4] [tasks: T002]
- render_spec() に Hearing History セクション追加（最新 5 件テーブル） [source: knowledge/research/2026-04-01-1546-planner-hearing-ux.md §2 TSUMIKI-07 Q3] [tasks: T002]
- SKILL.md Step 4a を AskUserQuestion + multiSelect パターンに書き換え（カテゴリ別バッチ、5 項目上限/回） [source: tsumiki kairo-requirements Stage 4 (AskUserQuestion + multiSelect), knowledge/research/tsumiki-hearing-deep-dive-2026-04-01.md §6 Priority 1] [tasks: T003]
- Modify 選択時の個別フォロー AskUserQuestion によるテキスト取得 [source: knowledge/research/2026-04-01-1546-planner-hearing-ux.md §2 TSUMIKI-05 Q3] [tasks: T003]

### Out of Scope
- EARS 記法の導入（CC-SDD-03、Phase 3 候補） [source: knowledge/strategy/TODO-PLAN.md Phase 3]
- シグナル伝播 spec→task（TSUMIKI-08、Phase 3） [source: knowledge/strategy/TODO-PLAN.md 3-13]
- spec.json schema_version の変更（1 のまま後方互換追加） [source: inference — 既存フィールドパターンに準拠]

## Constraints
- TSUMIKI-05/06 は SKILL.md プロンプト改修のみ（Rust コード変更なし） [source: knowledge/strategy/TODO-PLAN.md Phase 2b]
- TSUMIKI-07 は domain + infrastructure + render の 3 層変更が必要 [source: knowledge/research/2026-04-01-1546-planner-hearing-ux.md §1]
- hearing_history は content_hash 計算から除外すること（approval 無効化防止） [source: knowledge/research/2026-04-01-1546-planner-hearing-ux.md §3 TSUMIKI-07]
- hearing_history は append-only（削除・変更メソッドなし） [source: knowledge/research/2026-04-01-1546-planner-hearing-ux.md §2 TSUMIKI-07 Q4]
- 既存の差分ヒアリング spec.json 更新ロジック（source tagging rules）を維持すること [source: knowledge/research/2026-04-01-1546-planner-hearing-ux.md §1 TSUMIKI-05]

## Acceptance Criteria
- [ ] /track:plan 実行時に Full/Focused/Quick モード選択が AskUserQuestion で提示される [source: tsumiki kairo-requirements Stage 2] [tasks: T001]
- [ ] Focused モードで Phase 1.5（planner review）と Phase 2（Agent Teams）がスキップされる [source: knowledge/research/2026-04-01-1546-planner-hearing-ux.md §2 TSUMIKI-06 Q2/Q3] [tasks: T001]
- [ ] spec.json が存在しない場合に Focused/Quick を選択すると Full にフォールバックする [source: knowledge/research/2026-04-01-1546-planner-hearing-ux.md §3 TSUMIKI-06] [tasks: T001]
- [ ] HearingRecord の codec roundtrip テストが通る（encode → decode で同一） [source: convention — .claude/rules/05-testing.md] [tasks: T002]
- [ ] hearing_history 付き spec.json を content_hash 計算しても、hearing_history の追加で hash が変化しない [source: knowledge/research/2026-04-01-1546-planner-hearing-ux.md §3 TSUMIKI-07] [tasks: T002]
- [ ] hearing_history が空の旧 spec.json ファイルが正常にデシリアライズされる（後方互換） [source: inference — schema_version 1 維持の制約] [tasks: T002]
- [ ] render_spec() が hearing_history を最新 5 件のテーブルとして出力する [source: knowledge/research/2026-04-01-1546-planner-hearing-ux.md §2 TSUMIKI-07 Q3] [tasks: T002]
- [ ] 差分ヒアリングの Yellow/Red/Missing 項目が AskUserQuestion + multiSelect で提示される（カテゴリ別バッチ、5 項目上限） [source: tsumiki kairo-requirements Stage 4, knowledge/research/2026-04-01-1546-planner-hearing-ux.md §2 TSUMIKI-05 Q1] [tasks: T003]
- [ ] cargo make ci が全チェック通過する [source: convention — .claude/rules/07-dev-environment.md] [tasks: T004]

## Related Conventions (Required Reading)
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 22  🟡 2  🔴 0

