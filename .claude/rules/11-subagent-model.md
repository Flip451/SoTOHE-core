# Subagent Model

Default Claude Code subagent model is `claude-sonnet-4-6`.

Override guidance:

- Keep the default for normal planning, review, and routine implementation support.
- Override to `claude-opus-4-6` only when the task needs the highest reasoning depth, especially for complex Rust implementation, architecture refactors, or hard debugging.
- Do not downgrade to Haiku for normal track work. `claude-haiku-4-5-20251001` remains allowlisted only as an escape hatch for narrowly scoped, low-risk automation.

When documentation or prompts mention a subagent model, prefer describing the default plus override criteria rather than hardcoding Opus as the default.

## Codex Model Tiers

| Tier | Model | 用途 |
|------|-------|------|
| full | `gpt-5.4` | planner, debugger, 最終レビュー判断 |
| fast | `gpt-5.4-mini` | reviewer 初回パス, researcher (コードベース検索), 並列サブタスク |
| nano | `gpt-5.4-nano` | API 直接利用のみ。分類・データ抽出・ランキングなど軽量バッチ処理向け（Codex CLI 未対応） |

- `fast_model` は review sequential escalation の初回パスに使用される
- nano は Codex CLI 対応後に `nano_model` フィールド経由で利用可能になる
