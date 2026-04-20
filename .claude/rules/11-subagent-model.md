# Subagent Model

Default Claude Code subagent model is `claude-sonnet-4-6`.

Override guidance:

- Keep the default for normal review and routine implementation support (Explore / general-purpose etc.).
- **Custom agent files with `model: opus` frontmatter** bypass the default automatically. These are the preferred path when a capability needs Opus guaranteed:
  - `.claude/agents/spec-designer.md` — `subagent_type: "spec-designer"` for `/track:spec` (Phase 1, spec.json writer)
  - `.claude/agents/impl-planner.md` — `subagent_type: "impl-planner"` for `/track:impl-plan` (Phase 3, impl-plan.json + task-coverage.json writer)
  - `.claude/agents/type-designer.md` — `subagent_type: "type-designer"` for TDDD catalogue authoring (typically inline on main session; available as subagent when orchestrator delegates)
  - `.claude/agents/adr-editor.md` — `subagent_type: "adr-editor"` for ADR back-and-forth escalation (auto-invoked by `/track:plan` when a downstream SoT Chain signal turns 🔴)
  - `.claude/agents/review-fix-lead.md` — `subagent_type: "review-fix-lead"` for `/track:review` scope-owned fix+review loops
  These correspond to the `spec-designer` / `impl-planner` / `type-designer` / `adr-editor` / `reviewer` capabilities in `.harness/config/agent-profiles.json`.
- **Codex-heavy profile**: when `capabilities.spec-designer.provider = codex` or `capabilities.impl-planner.provider = codex`, the subagent is invoked via the `cargo make track-local-plan -- --model {model} --briefing-file ...` wrapper (out-of-process; the `claude --bare -p` path does not apply). The briefing content distinguishes the role (spec vs impl-plan).
- Override to `claude-opus-4-7` on the calling side (Agent tool `model: "opus"`) only when the built-in `subagent_type: "Plan"` / `general-purpose` is used without a custom agent file. Prefer custom agent files for anything recurring.
- Do not downgrade to Haiku for normal track work. `claude-haiku-4-5-20251001` remains allowlisted only as an escape hatch for narrowly scoped, low-risk automation.

When documentation or prompts mention a subagent model, prefer describing the default plus override criteria (or point at the relevant custom agent file) rather than hardcoding Opus as the default.

**Spec-designer / impl-planner model tier rule**: Use the highest available Claude model tier (currently `claude-opus-4-7`) for spec authoring and implementation planning tasks. Behavioral-contract mistakes (spec-designer) and task-decomposition mistakes (impl-planner) produce expensive review loops downstream, so default to the top tier rather than falling back to Sonnet.

## Codex Model Tiers

| Tier | Model | Usage |
|---|---|---|
| full | `gpt-5.4` | Final review verdict (`capabilities.reviewer.model`) |
| fast | `gpt-5.4-mini` | Reviewer's first pass (`capabilities.reviewer.fast_model`); parallel subtasks |
| nano | `gpt-5.4-nano` | API-direct use only (classification / extraction / ranking-style light batch work; not yet supported by Codex CLI) |

- `fast_model` is used for the initial pass in the review sequential-escalation ladder.
- The nano tier becomes usable via the `nano_model` field once Codex CLI adds support.
