# AGENTS.md

Root agent entry point. Keep this file pointer-only so PR-review guidance is not injected into
the root orchestrator's always-applied context.

## Provider rules

- Codex root orchestrator: `.codex/instructions.md` and `.codex/rules/default.rules`
- Claude root orchestrator: `.claude/rules/orchestrator.md`

## Shared workflow references

- Workflow SSoT: `.harness/workflows/track/`
- Capability contracts: `.harness/capabilities/`
- Review scope and briefings: `.harness/config/review-scope.json` and
  `.harness/custom/review-prompts/`
- Track artifacts: `track/items/<id>/`

## Automated PR review

Automated PR reviewers (for example, Codex Cloud `@codex review`) must read
`.harness/custom/review-prompts/pr-review.md` and apply it as their review guideline. Root
orchestrators must not load that file as part of their standing instructions.

The review workflow supplies its scope-specific briefing; this entry point is not a PR-review
briefing.
