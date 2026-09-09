# Agent Definitions

`.claude/agents/` holds the custom subagent definitions that Claude Code invokes through `subagent_type`. Each file is a thin adapter that points back to a provider-agnostic capability contract under `.harness/capabilities/`; the contract owns the behaviour, the adapter owns only the Claude-side invocation surface.

## Why the frontmatter carries `model` and `effort`

`bin/sotp capability exec` resolves the capability's provider and model from
`.harness/config/agent-profiles.json`. When the resolved provider differs from the host, the
dispatcher runs that provider's subprocess. When the resolved provider is Claude and the host is
Claude, it returns `delegate-in-host`; that payload carries the capability, briefing path, and
discipline body, but not model or effort. The selected Claude adapter's frontmatter supplies those
in-host values. Keep them aligned with the profile when a capability is configured for this path.

## Available adapters

The following is an inventory of adapter files shipped in this checkout. It describes available
Claude invocation surfaces, not the current provider assignment or a fixed set of active workers;
`.harness/config/agent-profiles.json` remains the routing authority.

| agent file | capability | invoked by |
|---|---|---|
| `spec-designer.md` | spec-designer | `/track:spec-design` (Phase 1) — authors `spec.json` |
| `type-designer.md` | type-designer | `/track:type-design` (Phase 2) — authors `<layer>-types.json` |
| `impl-planner.md` | impl-planner | `/track:impl-plan` (Phase 3) — authors the Phase 3 plan artifacts |
| `adr-editor.md` | adr-editor | the ADR editing lanes — owns `knowledge/adr/*.md` |
| `adr-diagnoser.md` | adr-diagnoser | ADR baseline guardian lane — read-only verdicts |
| `rollback-diagnoser.md` | rollback-diagnoser | `/track:diagnose` — routes a finding to its owning phase |
| `implementer.md` | implementer | `/track:implement` and focused implementation corrections |
| `review-fix-lead.md` | review-fix-lead | `/track:review` — owns one scope's fix loop |
| `researcher.md` | researcher | research and codebase analysis lanes |
| `dry-fix-lead.md` | dry-fix-lead | the DRY-fix adapter surface when selected by its workflow |

The orchestrator has no adapter file because it is the host's main session. A capability without
an adapter in this directory is executed by the provider-resolving workflow or typed pipeline that
owns it; this README does not infer or pin that provider.

## Dispatch rule

Never invoke these agents directly through the Agent tool. Direct invocation bypasses provider and
model resolution. From a Claude host, the canonical route is
`bin/sotp capability exec <capability> --host claude --briefing-file <path>`. If the dispatcher
returns `CAPABILITY_EXEC_OUTCOME: delegate-in-host`, invoke the returned adapter with its briefing
path and discipline body. Otherwise, let the dispatcher run the resolved provider subprocess.

Phase writers still enter through their matching `bin/sotp phase enter` command. Review and DRY
fix lanes use their workflow-owned wrappers, which resolve the profile internally.

`.harness/config/agent-profiles.json` is the routing SSoT. `.harness/config/samples/agent-profiles.*.json` hold alternative provider mixes.
