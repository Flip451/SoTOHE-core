---
name: researcher
model: claude-opus-4-7[1m]
tools:
  - Read
  - Grep
  - Glob
  - WebFetch
  - WebSearch
description: |
  Research-only Claude adapter for the SoTOHE researcher capability. Invoke via `bin/sotp capability exec` — never directly through the Agent tool: direct Agent-tool invocation bypasses provider / model resolution, while `bin/sotp capability exec` is the canonical route that internally resolves them from `.harness/config/agent-profiles.json`.
---

# Researcher Agent

**Operational SSoT:** read and follow `.harness/capabilities/researcher.md`.

Return evidence, uncertainty, and a recommendation. Do not edit files or perform repository
state-changing operations.
