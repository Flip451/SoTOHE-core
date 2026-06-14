# Agent Definitions

`.claude/agents/` holds the custom subagent definitions that Claude Code Orchestra invokes through `subagent_type`. Each agent pins Opus via `model: opus` frontmatter so that an unintended Sonnet fallback (driven by `CLAUDE_CODE_SUBAGENT_MODEL` in `.claude/settings.json`) never happens.

## Included Agents

- `spec-designer.md`: authors the behavioral contract (`spec.json`) (`/track:spec` = Phase 1)
- `impl-planner.md`: authors the implementation plan (`impl-plan.json` + `task-coverage.json`) (`/track:impl-plan` = Phase 3)
- `type-designer.md`: authors TDDD catalogue entries — picks `TypeDefinitionKind` variants and kind-specific fields (`/track:design` = Phase 2)
- `adr-editor.md`: edits existing ADRs in the working tree when a downstream SoT Chain signal turns 🔴 (back-and-forth escalation invoked by `/track:plan`; write scope limited to `knowledge/adr/`)
- `review-fix-lead.md`: runs the autonomous fix+review loop owned by a single review scope (`/track:review`)
- `dry-fix-lead.md`: runs the autonomous DFP (DRY fix phase) loop over the whole codebase — `sotp dry write` → fix DRY violations → `sotp dry check-approved` until the gate passes (`/track:dry-check`)

## Capability correspondence

Subagents aligned with the Claude-provider capabilities in `.harness/config/agent-profiles.json`:

| capability | agent file | invocation |
|---|---|---|
| spec-designer | `spec-designer.md` | `subagent_type: "spec-designer"` |
| impl-planner | `impl-planner.md` | `subagent_type: "impl-planner"` |
| type-designer | `type-designer.md` | `subagent_type: "type-designer"` (available as a subagent; typically runs inline in the main session) |
| adr-editor | `adr-editor.md` | `subagent_type: "adr-editor"` (auto-invoked by `/track:plan` on 🔴 escalation) |
| orchestrator | — | handled directly by the Claude Code main session (no subagent needed) |
| implementer | — | main session or ad-hoc delegation |
| reviewer | — (provider: codex) | dispatched internally by `review-fix-lead.md` via `bin/sotp review local`; not invoked directly as a subagent |
| review-fix-lead | `review-fix-lead.md` (provider: claude) | `review-fix-lead.md` owns the fix+review loop as a Claude subagent (`subagent_type: "review-fix-lead"`); invokes the reviewer via `bin/sotp review local` internally (`/track:review`) |
| dry-fix-lead | `dry-fix-lead.md` (provider: claude) | `dry-fix-lead.md` owns the DFP loop as a Claude subagent (`subagent_type: "dry-fix-lead"`); runs `sotp dry write` → fix → `cargo make ci-rust` → `sotp dry check-approved` internally (`/track:dry-check`) |
| dry-checker | — (provider: codex) | invoked by the `sotp dry` CLI (CodexDryChecker adapter), not as a subagent |
| researcher | — (provider: gemini) | Gemini CLI is invoked directly from the main session |
