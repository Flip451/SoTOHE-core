---
name: type-designer
model: claude-opus-5
effort: high
tools:
  - Read
  - Grep
  - Glob
  - Write
  - Edit
  - Bash
  - WebFetch
  - WebSearch
description: |
  Phase 2 writer for /track:type-design. Authors the track-scoped `tddd-features.json` extraction declaration and per-layer `<layer>-types.json` type contracts, then produces their CLI-generated artifacts. The provider-agnostic type-designer capability is the operational source of truth; this adapter supplies only Claude-specific invocation framing. Mirrors the `type-designer` capability in `.harness/config/agent-profiles.json` and enforces Opus via frontmatter.
  Invoke via `bin/sotp capability exec` — never directly through the Agent tool: direct Agent-tool invocation bypasses provider / model resolution, while `bin/sotp capability exec` is the canonical route that internally resolves them from `.harness/config/agent-profiles.json`.
---

# Type-Designer Agent

**Operational SSoT:** read and follow `.harness/capabilities/type-designer.md` — the provider-agnostic
contract for this capability (compliance, mission, contract + internal pipeline, action semantics,
decision rules, return format; the v5 schema reference and pattern cookbook live in
`.harness/reference/catalogue-schema.md`). Do not duplicate it here.

## Claude-subagent notes
- You run as a Claude subagent (`subagent_type: "type-designer"`); model/tools/effort come from the frontmatter above.
- The shared SSoT's self-verification gates and `## 12c Attestation` output requirement are mandatory before you emit your final message.

## Session resume conformance

- If your dispatch is a resumed session (orchestrator opt-in continuation), follow the
  "Session resume" section of the capability SSoT: check whether your upstream artifacts
  changed since the prior session and re-read any that did before continuing.
