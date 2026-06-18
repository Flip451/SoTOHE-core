# Harness Policy Review: Severity Policy

The reviewer's role is **convention / wiring / responsibility-boundary
consistency review** for the `.claude/` / `.harness/` / `.codex/` /
`knowledge/conventions/` / `README.md` / `CLAUDE.md` / `Makefile.toml` /
`track/review-scope.json` surface. These files define **how the harness
operates**: which command does what, which capability resolves to which
provider, which permission is allowed, what the responsibility boundary is.
The reviewer focuses on consistency drift — wiring that is internally
plausible but breaks an established contract with the rest of the
harness.

## What to report

Report findings ONLY for the following categories:

- **command wiring breakage**: a `.claude/commands/<x>.md` step that
  references another command (`/track:y`) whose surface has changed in
  a way that makes the step incoherent (e.g., the referenced command
  no longer takes the cited argument). Reviewers should not re-do
  `verify-doc-links`-style existence checks — focus on semantic drift.
- **capability routing mismatch**: a `.harness/config/agent-profiles.json`
  capability entry whose declared model is incompatible with the
  declared provider (e.g., a Claude-named model assigned to
  `provider: codex`). Cite the agent-profiles loader's documented
  contract.
- **responsibility-boundary cross**: harness code / docs that move a
  framework-owned concern into a consumer-owned slot (or vice-versa)
  in a way that conflicts with
  `knowledge/conventions/responsibility-boundary.md`. Specifically:
  framework methodology (review process, gate enforcement) must not
  live under `.harness/custom/`; consumer-customizable policies
  (severity preferences) must not live under `.harness/briefings/`
  / `.harness/capabilities/`.
- **convention contradiction**: a `knowledge/conventions/<x>.md` rule
  that contradicts another convention or contradicts a guardrail in
  `.claude/rules/`. Convention changes must propagate consistently
  across the `.claude/rules/` / `knowledge/conventions/` / CLAUDE.md
  surface.
- **permission posture drift**: an addition to `.claude/settings.json`
  `permissions.allow` (or `.claude/permission-extensions.json`) that
  matches a pattern in `.claude/rules/10-guardrails.md` §Dangerous
  to allow without a documented exception. Cite the specific
  dangerous-allow entry.
- **hook coverage gap**: a Bash command / agent flow that the
  `block-direct-git-ops` hook is supposed to intercept but the change
  routes around (e.g., wrapping git ops inside a Codex subprocess
  with `workspace-write`). Cite `.claude/rules/10-guardrails.md`
  §Sandbox and Hook Coverage Warning.
- **review-scope or briefing wiring inconsistency**: a
  `track/review-scope.json` change (or its successor
  `.harness/config/review-scope.json`) that adds a scope without
  declaring its `briefing_file`, OR a `briefing_file` value pointing
  at a path that does not match the actual file's location.

## What NOT to report

- Wording / tone of convention text (factual error / contradiction
  only)
- Re-ordering of `permissions.allow` entries
- Adding "(optional)" / "(recommended)" labels to convention rules
- Suggested CI gates for things D6 / similar decisions have
  explicitly deferred — those are decided out-of-scope
- Adding cross-links between conventions when the existing structure
  already covers the rule
- Re-organizing `.claude/commands/` directory layout
- Stylistic markdown nits (heading depth, bullet style, code-fence
  language tags)
