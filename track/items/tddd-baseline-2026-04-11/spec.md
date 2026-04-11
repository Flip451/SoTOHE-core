<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: approved
approved_at: "2026-04-11T07:19:43Z"
version: "1.0.0"
signals: { blue: 26, yellow: 2, red: 0 }
---

# TDDD-02: Baseline reverse signal — 既存型ノイズ排除

## Goal

reverse check で発生する既存型ノイズ (100+ Red) を排除し、TDDD の reverse signal を実用可能にする。
design 時点の TypeGraph スナップショット (baseline) を保存し、baseline との差分比較で新規型・構造変更のみを検出する。

## Scope

### In Scope
- TypeBaseline / TypeBaselineEntry / TraitBaselineEntry 型の定義 (domain 層) [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §1, §7] [tasks: T002]
- Baseline codec (encode/decode) と TypeGraph → TypeBaseline 変換 (infrastructure 層) [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §1, §7] [tasks: T003, T004]
- check_consistency の 4 グループ評価 (A\B, A∩B, B\A, ∁(A∪B)∩C) [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §3] [tasks: T005]
- CLI baseline-capture コマンド (baseline 生成専用) と domain-type-signals の拡張 (baseline 読み込み + 4 グループ評価) [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §4] [tasks: T006]
- 各層に tddd/ モジュール構造の新設 + 既存ファイル (domain_types.rs, domain_types_codec.rs, domain_state_signals.rs) の tddd/ 配下への移動 [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §7] [tasks: T001]
- /track:design コマンドの baseline 生成・コミット推奨の追記 [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §4] [tasks: T007]

### Out of Scope
- MethodDeclaration によるシグネチャ検証 — TDDD-01 の scope [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D2]
- action フィールド (add/modify/reference/delete) — TDDD-03 の scope [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md]
- 多層化 (architecture-rules.json の tddd ブロック) — TDDD-01 の scope [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D1]
- --refresh-baseline フラグ — 廃止済み [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §6]

## Constraints
- baseline は domain-types-baseline.json として track ディレクトリに保存。オブジェクト形式 (型名をキー) [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §1]
- baseline に TypeNode の outgoing と module_path は含めない [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §1]
- baseline 生成 (baseline-capture) と signal 評価 (domain-type-signals) は別コマンドに分離する [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §4]
- domain-type-signals は baseline が存在しない場合エラーとする [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §4]
- baseline-capture は既に baseline が存在する場合スキップする (冪等動作、--force で再生成可) [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §4]
- baseline は /track:design 時に baseline-capture で生成し、design 成果物と一緒にコミットを推奨 [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §4]
- TDDD-03 実装まで、/track:design を使用する track では既存型の削除を含められない [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §3]
- 1 概念 1 ファイルの粒度で配置。既存ファイルを肥大化させない [source: discussion]
- 後方互換性は対応しない [source: discussion]

## Acceptance Criteria
- [ ] 既存 100+ 型が reverse check で Red にならず、スキップされること [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §3 グループ 3] [tasks: T005, T006]
- [ ] baseline 後に追加された未宣言型が Red として検出されること (グループ 4) [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §3 グループ 4] [tasks: T005]
- [ ] baseline と構造が異なる未宣言型が Red として検出されること (グループ 3 構造変更) [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §3 グループ 3] [tasks: T005]
- [ ] baseline にあり現在のコードに存在しない未宣言型が Red として検出されること (グループ 3 削除) [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §3 グループ 3] [tasks: T005]
- [ ] 宣言済み型は forward check で評価され、baseline 比較の対象外であること (グループ 1, 2) [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §3 グループ 1, 2] [tasks: T005]
- [ ] baseline-capture が baseline を正しく生成し、既に存在する場合はスキップすること (冪等動作) [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §4] [tasks: T006]
- [ ] domain-type-signals が baseline 不在時にエラーを返すこと [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §4] [tasks: T006]
- [ ] domain-types-baseline.json がオブジェクト形式で正しく encode/decode されること [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §1] [tasks: T003]
- [ ] cargo make ci が通ること [source: convention — .claude/rules/07-dev-environment.md] [tasks: T001, T002, T003, T004, T005, T006, T007]

## Related Conventions (Required Reading)
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 26  🟡 2  🔴 0

