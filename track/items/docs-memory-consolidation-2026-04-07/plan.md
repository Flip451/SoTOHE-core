<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# 運用ルールのドキュメント移行 + メモリ棚卸し

Claude Code memory に蓄積された運用ルール（61件）を git 管理ドキュメントに移行し、
memory を 22 件に削減する。全 SoTOHE 利用者が恩恵を受けられるようにする。
ForgeCode 比較調査レポートも research/ に記録する。

## Review Protocol Convention

レビュープロトコル全規約を knowledge/conventions/review-protocol.md に集約。
13件の memory から移行。

- [x] knowledge/conventions/review-protocol.md 新規作成（レビュープロトコル全規約）

## Language Policy Convention

Rust-first ポリシーとファイル命名規約を knowledge/conventions/language-policy.md に集約。

- [x] knowledge/conventions/language-policy.md 新規作成（Rust-first + ファイル命名）

## Workflow Documentation Update

track/workflow.md に PR ワークフロー詳細、generated views 注意、タイムスタンプルールを追記。

- [x] track/workflow.md 追記（PR ワークフロー詳細、generated views、タイムスタンプ）

## Rules Update

.claude/rules/ にガードレール追記（bash redirect、planner gate、model tier、WORKER_ID）。

- [x] .claude/rules/ 追記（bash redirect 制約、planner gate、planner model tier、WORKER_ID）

## ForgeCode Research

ForgeCode 比較分析・MCP 統合戦略・パフォーマンス調査レポートを research/ に記録。

- [x] knowledge/research/ に ForgeCode 比較・MCP 統合戦略レポートを追加
