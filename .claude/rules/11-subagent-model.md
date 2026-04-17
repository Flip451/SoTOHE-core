# Subagent Model

Default Claude Code subagent model is `claude-sonnet-4-6`.

Override guidance:

- Keep the default for normal review and routine implementation support (Explore / general-purpose etc.).
- **Custom agent files with `model: opus` frontmatter** bypass the default automatically. These are the preferred path when a capability needs Opus guaranteed:
  - `.claude/agents/planner.md` — `subagent_type: "planner"` for `/track:plan` Phase 1.5 / Phase 2
  - `.claude/agents/designer.md` — `subagent_type: "designer"` for TDDD catalogue authoring (typically inline on main session; available as subagent when orchestrator delegates)
  - `.claude/agents/review-fix-lead.md` — `subagent_type: "review-fix-lead"` for `/track:review` scope-owned fix+review loops
  These correspond to the `planner` / `designer` / `reviewer` capabilities in `.harness/config/agent-profiles.json`.
- **Codex-heavy profile**: when `capabilities.planner.provider = codex`, the planner is invoked via `cargo make track-local-plan -- --model {model} --briefing-file ...` wrapper (out-of-process, `claude --bare -p` path does not apply).
- Override to `claude-opus-4-7` on the calling side (Agent tool `model: "opus"`) only when the built-in `subagent_type: "Plan"` / `general-purpose` is used without a custom agent file. Prefer custom agent files for anything recurring.
- Do not downgrade to Haiku for normal track work. `claude-haiku-4-5-20251001` remains allowlisted only as an escape hatch for narrowly scoped, low-risk automation.

When documentation or prompts mention a subagent model, prefer describing the default plus override criteria (or point at the relevant custom agent file) rather than hardcoding Opus as the default.

**Planner model tier rule**: Use the highest available Claude model tier (currently `claude-opus-4-7`) for planning tasks that involve architecture decisions, complex trade-offs, or new crate boundaries. Use the default subagent model (`claude-sonnet-4-6`) only for narrowly scoped, low-risk prompt-only changes. Cheap planning produces expensive review loops downstream.

## Codex Model Tiers

| Tier | Model | 用途 |
|------|-------|------|
| full | `gpt-5.4` | 最終レビュー判断 (`capabilities.reviewer.model`) |
| fast | `gpt-5.4-mini` | reviewer 初回パス (`capabilities.reviewer.fast_model`), 並列サブタスク |
| nano | `gpt-5.4-nano` | API 直接利用のみ。分類・データ抽出・ランキングなど軽量バッチ処理向け（Codex CLI 未対応） |

- `fast_model` は review sequential escalation の初回パスに使用される
- nano は Codex CLI 対応後に `nano_model` フィールド経由で利用可能になる
