# Spec: CLAUDE.md 50行以下圧縮 + workspace-tree コマンド化

## Goal

CLAUDE.md は毎会話のコンテキストに自動注入されるため、トークン消費を最小化する。
250行 → 50行以下に圧縮し、詳細は `.claude/rules/` に移動する。
Workspace Map は CLAUDE.md から削除し、`cargo make workspace-tree` / `workspace-tree-full`
コマンドで `architecture-rules.json` から動的生成する方式に置換する。

## Scope

- CLAUDE.md の詳細セクションを `.claude/rules/` に分散（08, 09, 10）
- CLAUDE.md をポインタファイル化（SSoT リスト + 最小限のルール）
- Workspace Map を CLAUDE.md から削除
- `architecture_rules.py` に `workspace-tree`（crate のみ）/ `workspace-tree-full`（crate + extra_dirs）サブコマンドを追加
- `architecture-rules.json` に `extra_dirs` フィールドを追加（非 crate ディレクトリ用）
- `verify_claude_workspace_map()` を削除し、全消費者を更新:
  - `architecture_rules.py`, `verify_architecture_docs.py`（本体）
  - `test_architecture_rules.py`, `test_verify_scripts.py`, `test_make_wrappers.py`（テスト）
  - `Makefile.toml`（タスク定義）
  - `verify_orchestra_guardrails.py`（ホワイトリスト）
  - `.claude/settings.json`（許可リスト）
- 07-dev-environment.md に未ドキュメントの cargo make タスクを追記（既に編集済み）

## Constraints

- `.claude/rules/` は自動注入されるため、移動しても AI の行動は変わらない
- 既存 rules ファイルの番号体系（01〜07）を維持、新規は 08〜10
- `architecture-rules.json` が workspace 構造の唯一の SSoT
- `extra_dirs` は additive な変更とし、既存の `workspace_members()` のセマンティクスを変えない
- 圧縮後の CLAUDE.md に `project-docs/conventions/` の言及を残す（`verify_architecture_docs.py` のチェック対応）

## Acceptance Criteria

- CLAUDE.md が50行以下
- `cargo make ci` が全パス
- `cargo make workspace-tree` が crate のみの説明付き tree を出力
- `cargo make workspace-tree-full` が crate + extra_dirs の説明付き tree を出力
- 移動元の情報が `.claude/rules/` で参照可能
- `verify-claude-workspace-map` の全参照が削除済み
