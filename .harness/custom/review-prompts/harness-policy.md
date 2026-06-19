# Harness Policy Review: Severity Policy

The reviewer's role is **convention / wiring / responsibility-boundary
consistency review** for the `.claude/` / `.harness/` / `.codex/` /
`knowledge/conventions/` / `README.md` / `CLAUDE.md` / `Makefile.toml`
surface (the loader config `.harness/config/review-scope.json` is part of
this surface via the `.harness/**` pattern). These files define **how the
harness operates**: which command does what, which capability resolves to
which provider, which permission is allowed, what the responsibility boundary is.
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
  `.harness/config/review-scope.json` change that adds a scope without
  declaring its `briefing_file`, OR a `briefing_file` value pointing
  at a path that does not match the actual file's location.
- **template-distribution reference leak**: files under the harness-policy
  scope (`.claude/**`, `.harness/**`, `.codex/**`, `.agents/**`,
  `knowledge/conventions/**`, `README.md`, `CLAUDE.md`, `AGENTS.md`,
  `Makefile.toml`) are template distribution targets — they ship to
  every consumer of this template. A reference from a distribution target
  to a path **not present in a fresh consumer checkout** (e.g.,
  `track/items/<some-id>/...`, `tmp/...`, `target/...`,
  `.semantic_index/...`, a gitignored artifact, or a deleted file) will
  resolve to a non-existent path in the consumer's environment and break
  the harness. Examples: a `.claude/commands/**` step that cites
  `track/items/<a-specific-track>/spec.md`; a `.harness/custom/**`
  briefing that mentions `tmp/reviewer-runtime/...` as if it were a
  durable reference; a `knowledge/conventions/**` rule that points at a
  removed file. Cite `knowledge/conventions/responsibility-boundary.md`
  for the distribution surface boundary.

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
- **Legitimate references to non-distributed paths** — the
  `template-distribution reference leak` bullet has the following
  exceptions; do NOT flag these:
  - **Placeholder paths**: `track/items/<track-id>/spec.md`,
    `.harness/custom/review-prompts/<scope>.md`, and similar `<...>`
    placeholders. Readers understand these are templates to be
    instantiated, not concrete references.
  - **Per-run ephemeral paths created by workflow commands**:
    `tmp/reviewer-runtime/briefing-{scope}.md`,
    `tmp/track-commit/commit-message.txt`, `tmp/track-commit/note.md`,
    `tmp/research/...`, and similar. A workflow command in
    `.claude/commands/**` legitimately instructs the user to create or
    write these at runtime; the path does not need to exist in the
    distribution.
  - **Build artifacts and runtime tools**: `bin/sotp`,
    `target/release/...`, `target-w1/...`, `.semantic_index/...`,
    `.fastembed_cache/...`. These are produced by `cargo make
    build-sotp` / `cargo make ci` / similar, and `README.md` /
    `CLAUDE.md` / convention docs may reference them as the canonical
    runtime path even though they are gitignored.
  - **Consumer-runtime data under a runtime-created tree**:
    `track/items/<consumer-actual-track-id>/...`,
    `track/items/<id>/review.json`, `track/registry.md`, and similar.
    These come into existence after the consumer runs `/track:init`;
    the distribution ships an empty `track/items/` tree.
  - **References to a path that matches a harness-policy pattern but
    is gitignored**: `.claude/settings.local.json` or similar
    consumer-local files. The matching pattern includes them, but git
    excludes them from the distribution; references to them in
    convention docs (e.g., "your local overrides go in
    `.claude/settings.local.json`") are not a distribution leak.
