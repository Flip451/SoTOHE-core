<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Planner capability の Claude 移行 (Phase 1)

default profile の planner を Codex CLI から Claude Code (Opus subagent) に移行する Phase 1。
Rust コード変更なし。config + doc + SKILL 変更のみ。
Phase 2（domain型 + hexagonal 統一 resolver）は別 track で実施。

## Config 変更

agent-profiles.json の default profile planner を claude に変更し、
providers.claude に default_model を追加する。

- [x] agent-profiles.json: default profile planner を codex → claude に変更、providers.claude.default_model 追加

## SKILL / ルールファイル更新

track-plan SKILL.md の Phase 1.5/2 呼び出しパターンと、
rules/02, 08, 11 の planner 参照を Claude 経路に更新する。

- [x] SKILL.md Phase 1.5/2: planner 呼び出しパターンを Claude subagent (Agent tool) に更新
- [x] .claude/rules/02-codex-delegation.md: planner を default Codex capability から除外
- [x] .claude/rules/08-orchestration.md: default specialist profile 更新
- [x] .claude/rules/11-subagent-model.md: planner の Opus 利用を明記

## ワークフロー / 設計ドキュメント更新

track/workflow.md と knowledge/DESIGN.md の Agent Roles を更新する。

- [x] track/workflow.md: planner 参照更新
- [x] knowledge/DESIGN.md: Agent Roles テーブル更新

## リサーチ保存 / Phase 2 TODO

Codex planner の設計レビュー出力を保存し、
Phase 2 の hexagonal 統一 resolver タスクを TODO に追記する。

- [x] Codex planner 設計レビュー出力を knowledge/research/ に保存（Phase 2 参照資料）
- [x] Phase 2（domain型 + 統一 config resolver）の TODO を knowledge/strategy/TODO.md に追記
