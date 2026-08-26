# CLAUDE.md

Claude Code root entry point for SoTOHE-core. Keep this file pointer-only; the always-applied
orchestrator rules live in `.claude/rules/orchestrator.md`.

## Read for orchestration

- Root orchestrator rules: `.claude/rules/orchestrator.md`
- Detailed orchestration reference: `.claude/rules/orchestration.md`
- Guardrails and workflow-command boundaries: `.claude/rules/guardrails.md`
- Toolchain and wrapper catalogue: `.claude/rules/dev-environment.md`
- Language guidance: `.claude/rules/language.md`

## Shared sources of truth

- Workflow SSoT: `.harness/workflows/track/`
- Capability contracts: `.harness/capabilities/`
- Capability provider routing: `.harness/config/agent-profiles.json`
- Harness policies: `.harness/policies/`
- Project conventions (read by delegated capabilities from their briefings): `knowledge/conventions/`
- Track artifacts: `track/items/<id>/`
- User onboarding: `README.md`

PR-review briefings are loaded by the review workflow from
`.harness/config/review-scope.json`; they are not part of the root orchestrator's standing
instructions.
