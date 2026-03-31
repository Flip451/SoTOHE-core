<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
signals: { blue: 25, yellow: 0, red: 0 }
---

# ドキュメント群の knowledge ディレクトリへの集約と整理

## Goal

3箇所に分散したドキュメント群（.claude/docs/, project-docs/, docs/）を既存の knowledge/ ディレクトリに集約し、プロジェクト知識の統一的なアクセスポイントを確立する。ルートレベルの CLAUDE.md 等は Claude Code の自動読み込み対象のため移動しない。
汎用的なドキュメントリンク存在チェック（sotp verify doc-links）を CI に追加し、今後のファイル移動・リネーム時のリンク切れを自動検出する。

## Scope

### In Scope
- .claude/docs/ 配下の全ファイル（DESIGN.md, WORKFLOW.md, research/, designs/, schemas/）を knowledge/ に移行する [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md] [tasks: T006, T007]
- project-docs/conventions/ を knowledge/conventions/ に移行する [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md] [tasks: T004]
- docs/ 配下のファイルを knowledge/external/ と repo root に分散移行する [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md] [tasks: T003, T005]
- 移行に伴う全参照パスの更新（Rust ソース、Python スクリプト、設定ファイル、ドキュメント） [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md] [tasks: T001, T003, T004, T005, T006, T007]
- sotp verify doc-links サブコマンドの実装と CI への組み込み [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md] [tasks: T002, T008]
- DESIGN.md を knowledge/architecture.md にスリム化（Canonical Blocks 削除） [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md] [tasks: T007]
- knowledge/README.md の作成（索引 + 読み順） [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md] [tasks: T007]

### Out of Scope
- 完了済みトラック（track/items/*, track/archive/*）内の歴史的参照の更新 [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md §Consequences]
- .claude/rules/, .claude/commands/, .claude/skills/ ディレクトリの移動（ハーネス設定として .claude/ に残留） [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md §Decision]
- track/ ディレクトリの移動（ワークフローステートマシンとして独立維持） [source: CLAUDE.md §優先参照]
- ルートレベルの CLAUDE.md, DEVELOPER_AI_WORKFLOW.md 等の移動（Claude Code が自動読み込みする） [source: CLAUDE.md §優先参照]

## Constraints
- 各タスクの diff は 500 行以下に抑える [source: feedback — small review surface policy] [tasks: T001, T002, T003, T004, T005, T006, T007, T008]
- TDD ワークフロー必須: 失敗テストを先に書いてから実装 [source: convention — .claude/rules/05-testing.md]
- domain 層はパス定数を持たない（hexagonal architecture 制約） [source: convention — project-docs/conventions/hexagonal-architecture.md]
- 各タスク完了後に cargo make ci が通ること [source: convention — .claude/rules/10-guardrails.md]
- 新規ロジックは Rust で実装する（Rust-first ポリシー） [source: feedback — Rust-first policy]
- 段階的ハードカット方式（シンボリックリンク不使用）。一時的な dual-read 互換性をコードに追加 [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md]

## Domain States

| State | Description |
|-------|-------------|
| scattered | ドキュメントが 5 箇所に分散している現状（移行前） |
| migrating | ファイル移動中、一時的な dual-read 互換性が有効な状態 |
| consolidated | 全ドキュメントが knowledge/ に集約され、旧パスが廃止された状態 |

## Acceptance Criteria
- [ ] .claude/docs/, project-docs/, docs/ ディレクトリが削除されている [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md] [tasks: T008]
- [ ] knowledge/ 配下に architecture.md, WORKFLOW.md, conventions/ (9 convention .md + README.md), research/ (既存 + .claude/docs/research/ 由来 13 ファイル統合), designs/ (3 design + 2 schema = 5 ファイル), external/ (POLICY.md + guides.json) が存在し、各ディレクトリが空でないこと [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md] [tasks: T004, T005, T006, T007]
- [ ] architecture-rules.json が repo root に存在する [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md] [tasks: T003]
- [ ] cargo make ci が全チェック通過する（verify doc-links 含む） [source: convention — .claude/rules/10-guardrails.md] [tasks: T008]
- [ ] sotp verify doc-links が Markdown 内の相対パスリンク存在を検証できる [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md] [tasks: T002]
- [ ] Rust ソース内の旧パス定数（ARCH_RULES_FILE 等）が新パスに更新されている [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md §Decision] [tasks: T003, T004]
- [ ] planning-only 判定（review/mod.rs, git_workflow.rs）が新パスに対応している [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md §Decision] [tasks: T001, T006, T007]
- [ ] knowledge/README.md に統一的な索引と読み順が記載されている [source: knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md] [tasks: T007]

## Related Conventions (Required Reading)
- project-docs/conventions/hexagonal-architecture.md
- project-docs/conventions/source-attribution.md
- project-docs/conventions/adr.md
- .claude/rules/05-testing.md
- .claude/rules/10-guardrails.md

## Signal Summary

### Stage 1: Spec Signals
🔵 25  🟡 0  🔴 0

