# Verification: knowledge-consolidation-2026-03-30

## Scope Verified

- [ ] .claude/docs/ 配下の全ファイルが knowledge/ に移行済み
- [ ] project-docs/conventions/ が knowledge/conventions/ に移行済み
- [ ] docs/ 配下が knowledge/external/ + repo root に移行済み
- [ ] 旧ディレクトリ（.claude/docs/, project-docs/, docs/）が削除済み
- [ ] knowledge/README.md が索引として機能している
- [ ] knowledge/architecture.md が Canonical Blocks なしでスリム化されている

## Manual Verification Steps

1. `cargo make ci` が全チェック通過することを確認
2. `bin/sotp verify doc-links` が Markdown 相対パスリンクの存在を検証できることを確認
3. `architecture-rules.json` が repo root に存在し、`cargo make check-layers` が通過することを確認
4. `knowledge/conventions/README.md` のインデックスが正しく生成されることを確認
5. planning-only 判定が新パス（knowledge/, architecture-rules.json）で正しく動作することを確認
6. 完了済みトラック内の歴史的参照が変更されていないことを確認
7. 旧パスへの参照が残っていないことを確認。全コマンドに `--hidden -g '!.git/**'` を付与して隠しファイル（`.claude/` 等）も検索対象にする。以下の `rg` コマンドが全て結果 0 件であること:
   - `rg -n --hidden -g '!.git/**' -g '!track/items/**' -g '!track/archive/**' -g '!knowledge/adr/**' -g '!tmp/**' -g '!target*/**' -g '!vendor/**' "docs/architecture-rules\.json"`
   - `rg -n --hidden -g '!.git/**' -g '!track/items/**' -g '!track/archive/**' -g '!knowledge/adr/**' -g '!tmp/**' -g '!target*/**' -g '!vendor/**' "project-docs/conventions/"`
   - `rg -n --hidden -g '!.git/**' -g '!track/items/**' -g '!track/archive/**' -g '!knowledge/adr/**' -g '!tmp/**' -g '!target*/**' -g '!vendor/**' "\.claude/docs/"`
   - `rg -n --hidden -g '!.git/**' -g '!track/items/**' -g '!track/archive/**' -g '!knowledge/adr/**' -g '!tmp/**' -g '!target*/**' -g '!vendor/**' "docs/EXTERNAL_GUIDES|docs/external-guides"`
   - `rg -n --hidden --pcre2 -g '!.git/**' -g '!track/items/**' -g '!track/archive/**' -g '!knowledge/adr/**' -g '!tmp/**' -g '!target*/**' -g '!vendor/**' '(?<![a-zA-Z0-9_-])docs/'` (一般的な docs/ 参照。project-docs/ は lookbehind で除外)
   - 除外理由: track/items/, track/archive/ は歴史的記録、knowledge/adr/ は ADR（過去の決定記録として旧パスを保持可能）、tmp/, target*/, vendor/, .git/ は非ソース
8. 移行先ファイルの存在確認:
   - `knowledge/designs/` に以下のファイルが含まれていること:
     - `.claude/docs/designs/` 由来: auto-mode-agent-briefings.md, auto-mode-escalation-ui.md, auto-mode-integration.md
     - `.claude/docs/schemas/` 由来: auto-mode-config-schema.md, auto-state-schema.md
   - `knowledge/external/` に以下のファイルが含まれていること:
     - `POLICY.md`（docs/EXTERNAL_GUIDES.md 由来）
     - `guides.json`（docs/external-guides.json 由来）
   - `knowledge/WORKFLOW.md` が存在すること（.claude/docs/WORKFLOW.md 由来）
   - `knowledge/research/` に `.claude/docs/research/` 由来のファイルが全て含まれていること（README.md, .gitignore, version-baseline-template.md, version-baseline-2026-03-11.md, cc-sdd-analysis-2026-03-17.md, gemini-gap-analysis-2026-03-18.md, harness-engineering-best-practices-2026-03-09.md, harness-issues-analysis.md, planner-branch-strategy-2026-03-12.md, planner-pr-review-cycle-2026-03-12.md, reinvention-check-workflow.md, symphony-analysis-2026-03-17.md, tsumiki-analysis-2026-03-17.md — 計 13 ファイル。既存の knowledge/research/ にあるファイルとマージ）
   - `knowledge/architecture.md` が存在すること（.claude/docs/DESIGN.md のスリム版）
   - `knowledge/README.md` が存在し、全サブディレクトリへのリンクが含まれていること

## Result / Open Issues

- (実施後に記録)

## verified_at

- (実施後に記録)
