# Agent Definitions

`.claude/agents/` には、Claude Code Orchestra で subagent 経由で呼び出される custom agent 定義を置きます。各 agent は `model: opus` frontmatter で Opus を固定し、`CLAUDE_CODE_SUBAGENT_MODEL` env (default: Sonnet) による意図しない Sonnet フォールバックを防ぎます。

## Included Agents

- `planner.md`: architecture design、trait/module planning、trade-off evaluation (`/track:plan` Phase 1.5 / Phase 2)
- `designer.md`: TDDD catalogue editing — `TypeDefinitionKind` variant 選択と kind-specific fields の author (`/track:design`)
- `review-fix-lead.md`: スコープ所有型の自律 fix+review ループ実行 (`/track:review`)

## Capability correspondence

`.harness/config/agent-profiles.json` の各 Claude-provider capability に対応する subagent:

| capability | agent file | invocation |
|---|---|---|
| planner | `planner.md` | `subagent_type: "planner"` |
| designer | `designer.md` | `subagent_type: "designer"` (subagent 化時、通常は main session inline) |
| orchestrator | — | Claude Code main session 自身が担う (subagent 不要) |
| implementer | — | main session or ad-hoc delegation |
| reviewer | — (provider は codex) | `review-fix-lead.md` がループを所有し Codex CLI を呼ぶ |
| researcher | — (provider は gemini) | Gemini CLI を main session が直接呼ぶ
