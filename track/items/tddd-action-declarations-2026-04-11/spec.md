<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: approved
approved_at: "2026-04-11T12:43:44Z"
version: "1.0.0"
signals: { blue: 27, yellow: 0, red: 0 }
---

# TDDD-03: Type action declarations — add/modify/reference/delete

## Goal

domain-types.json の各エントリに optional な action フィールドを追加し、型操作の意図 (追加/変更/参照/削除) を明示的に記録する。
TDDD-02 の制約「既存型の削除と TDDD の併用不可」を解消し、action: delete で意図的な型削除を TDDD 内で宣言可能にする。
kind migration (struct→trait 等) を同名 delete+add ペアで表現可能にする。

## Scope

### In Scope
- TypeAction enum (Add/Modify/Reference/Delete) の定義と DomainTypeEntry への action フィールド追加 (domain 層) [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md] [tasks: T001]
- Delete action の forward check 反転: 型不在→Blue、型存在→Yellow (domain 層) [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §action の値と評価ロジック] [tasks: T002]
- action と baseline の矛盾検出 (contradiction warnings) + delete baseline 検証 (error) (domain 層) [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §action の値と評価ロジック, knowledge/adr/2026-04-11-0003-type-action-declarations.md §実装時の注意] [tasks: T003, T004]
- Codec DTO 拡張: action フィールドの serde 対応 (default=add, skip_serializing_if=add) (infrastructure 層) [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §Decision, knowledge/research/2026-04-11-1203-planner-tddd-03-type-action.md §Key Design Decisions] [tasks: T006]
- Duplicate name 検証緩和: 同名エントリは delete+add ペア (2件) のみ許可、3件以上は常にエラー (infrastructure 層) [source: feedback — kind migration 議論で delete+add ペア方式に合意] [tasks: T006]
- domain_types_render に Action 列追加 (infrastructure 層) [source: knowledge/research/2026-04-11-1203-planner-tddd-03-type-action.md] [tasks: T007]
- CLI verify.rs / signals.rs に contradictions と delete_errors のハンドリング追加 [source: knowledge/research/2026-04-11-1203-planner-tddd-03-type-action.md] [tasks: T008, T009]
- /track:design コマンドに action 選択ガイダンスと JSON スキーマ例を追加 [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md] [tasks: T010]

### Out of Scope
- TDDD-01 (多層化 + シグネチャ検証) — 別 ADR・別 track [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md]
- DomainTypeKind → TypeDefinitionKind リネーム — TDDD-01 (Step 3) の scope [source: knowledge/strategy/tddd-implementation-plan.md §Step 3]
- action: rename (旧名→新名ペア宣言) — ADR Reassess When に記載、現時点では不要 [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §Reassess When]

## Constraints
- action フィールドは optional で、省略時は Add をデフォルトとする (serde default による parse-time のデフォルト値。完了済み track の domain-types.json との後方互換性を保証するものではない — ADR §Consequences 参照) [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §Decision, knowledge/adr/2026-04-11-0003-type-action-declarations.md §Consequences]
- action: delete 宣言時は当該型が baseline に存在することを検証し、存在しない場合はエラーとする [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §実装時の注意]
- 同名エントリは delete+add ペア (正確に 2 件) のみ許可。3 件以上や delete+delete, add+add は常にエラー [source: feedback — kind migration 議論で合意]
- contradiction は warning として報告する (CI ブロックしない)。delete_errors は error として報告する (CI ブロック) [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md, knowledge/research/2026-04-11-1203-planner-tddd-03-type-action.md]
- 後方互換性は対応しない (ADR 方針) [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §Consequences]
- 1 概念 1 ファイルの粒度で配置。TypeAction は catalogue.rs 内に定義 (enum-first、遷移なし) [source: convention — .claude/rules/04-coding-principles.md]

## Acceptance Criteria
- [ ] action 省略時に DomainTypeEntry.action() が TypeAction::Add を返すこと [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md] [tasks: T001, T006]
- [ ] action: delete の型が forward check で不在→Blue、存在→Yellow となること [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §action の値と評価ロジック] [tasks: T002]
- [ ] action: delete の型が codec round-trip を経て TypeAction::Delete として復元され、forward check が正しく評価されること (end-to-end) [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §action の値と評価ロジック] [tasks: T001, T002, T006]
- [ ] action: delete + TraitPort の型が forward check で不在→Blue、存在→Yellow となること [source: knowledge/research/2026-04-11-1203-planner-tddd-03-type-action.md EC-2] [tasks: T002]
- [ ] action: modify の型が codec round-trip を経て TypeAction::Modify として復元され、C に存在し宣言と一致する場合 Blue となること [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §action の値と評価ロジック] [tasks: T001, T002, T006]
- [ ] action: reference の型が codec round-trip を経て TypeAction::Reference として復元され、C に存在し宣言と一致する場合 Blue となること [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §action の値と評価ロジック] [tasks: T001, T002, T006]
- [ ] action: add + baseline にある型で AddButAlreadyInBaseline contradiction が検出されること [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §action の値と評価ロジック] [tasks: T004]
- [ ] action: modify + baseline にない型で ModifyButNotInBaseline contradiction が検出されること [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §action の値と評価ロジック] [tasks: T004]
- [ ] action: reference + baseline にない型で ReferenceButNotInBaseline contradiction が検出されること [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §action の値と評価ロジック] [tasks: T004]
- [ ] action: reference + 宣言と実装が不一致な型で ReferenceButNotBlue contradiction が検出されること [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §action の値と評価ロジック] [tasks: T004]
- [ ] action: delete + baseline にない型で delete_errors が報告されること [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §実装時の注意] [tasks: T004]
- [ ] 同名 delete+add ペア (kind migration) が codec で受け入れられ、正しく評価されること [source: feedback — kind migration 議論で合意] [tasks: T006]
- [ ] 同名 3 件以上 / delete+delete / add+add が codec でエラーとなること [source: feedback — kind migration 議論で合意] [tasks: T006]
- [ ] action: add の encode 時に action フィールドが JSON から省略されること (skip_serializing_if) [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md §Decision, knowledge/research/2026-04-11-1203-planner-tddd-03-type-action.md §Key Design Decisions] [tasks: T006]
- [ ] domain-types.md に Action 列が表示されること [source: knowledge/research/2026-04-11-1203-planner-tddd-03-type-action.md] [tasks: T007]
- [ ] cargo make ci が通ること [source: convention — .claude/rules/07-dev-environment.md] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010]

## Related Conventions (Required Reading)
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 27  🟡 0  🔴 0

