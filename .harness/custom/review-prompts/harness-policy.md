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

## Priority categories

Violations of the role statement above are always reportable. The following priority categories focus the review and guide severity assessment; they are not an exhaustive list of reportable design deviations. The exclusions in **What NOT to report** still apply:

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
  `.harness/policies/consumer-ownership.md`. Specifically:
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
  matches a pattern in `.claude/rules/guardrails.md` §Dangerous
  to allow without a documented exception. Cite the specific
  dangerous-allow entry.
- **hook coverage gap**: a Bash command / agent flow that the
  `block-direct-git-ops` hook is supposed to intercept but the change
  routes around (e.g., wrapping git ops inside a Codex subprocess
  with `workspace-write`). Cite `.claude/rules/guardrails.md`
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
  removed file. Cite `.harness/policies/consumer-ownership.md`
  for the distribution surface boundary.
- **adapter-SSoT 同期 check**: `harness-policy` review scope が trigger されたとき、
  adapter と workflow SSoT の整合について以下の 3 要件を adapter / SSoT 両方について検査する。
  - adapter (`.claude/commands/track/*.md` および `.agents/skills/track-*/SKILL.md`)
    が冒頭で明示する `.harness/workflows/<path>.md` が実在すること。
  - adapter 本文に workflow logic (step 番号、gate 条件、状態遷移、失敗復旧手順) が
    長文複製されていないこと。
  - workflow SSoT (`.harness/workflows/**/*.md`) に provider 固有の起動細部
    (subagent / skill 起動方法、provider 固有の sandbox / 権限 flag 等) が
    漏れ込んでいないこと。

## What NOT to report

- Wording / tone of convention text (factual error / contradiction
  only)
- Re-ordering of `permissions.allow` entries
- Adding "(optional)" / "(recommended)" labels to convention rules
- Suggested CI gates for concerns whose decision explicitly deferred them — those
  are decided out-of-scope
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
    These come into existence after the consumer runs `/track:init`; the
    distribution need not contain the runtime-created `track/items/` tree.
  - **References to a path that matches a harness-policy pattern but
    is gitignored**: `.claude/settings.local.json` or similar
    consumer-local files. The matching pattern includes them, but git
    excludes them from the distribution; references to them in
    convention docs (e.g., "your local overrides go in
    `.claude/settings.local.json`") are not a distribution leak.

## Known Accepted Deviations

- **Conditional allowance — apply only when the review run's resolved Track ID is
  exactly `scope-conditional-pre-review-gates-2026-07-31`; otherwise, do not
  apply this allowance and review PhaseCommandService fulfillment bindings
  normally.**
  **PhaseCommandService fulfillment bindings are cross-populated** (each per-method
  derived obligation cites the shared validate/explain/enter test set). Factual
  background: the derive mechanism attaches every entry-level spec anchor to each
  per-method trait obligation and the current catalogue schema has no method-level
  spec_refs, so method-scoped fulfillment sets cannot satisfy whole-entry anchors and
  edge-level voluntary bindings are structurally rejected
  (VoluntaryBindingOwnsDerivedObligation). A rollback-diagnoser verdict (2026-08-03)
  routed the root cause to the test-obligation ADR's undecided obligation granularity;
  the user adjudicated on 2026-08-03 to accept the cross-populated binding shape in this
  track and to resolve the granularity (method-level spec_refs + derive changes) in a
  separate track. It is not an accepted deviation for any other track or consumer.
- **Conditional allowance — apply only when the review run's resolved Track ID is
  exactly `scope-conditional-pre-review-gates-2026-07-31`; otherwise ignore.**
  **Staged `check-zero-findings` registration (resolved)**: the staged-registration
  suppression previously recorded here has EXPIRED — the review CLI enum registration
  and execution path landed with task T042, and the driver dispatch / aggregate wiring
  landed earlier; the former cli.md suppression note has been removed. The follow-on
  attribution-semantics note has also EXPIRED — the T043 implementation landed, the
  attributed cli_driver review entries converged with their declarations, and the
  corresponding cli_driver.md note has been removed. No suppression remains under this
  entry for any track or consumer.
- **Conditional allowance — apply only when the review run's resolved Track ID is
  exactly `scope-conditional-pre-review-gates-2026-07-31`; otherwise ignore.**
  **Unit-variant match premise error (cli_driver)**: a rollback-diagnoser adjudication
  recorded that the review-results renderer's `match scope.state` /
  `matches!(round.round_type, ...)` on borrowed places with unit-variant patterns binds
  no payload and performs no move; the workspace passes `cargo make clippy`
  (`-D warnings`) and `cargo make ci-rust` with this code. The cli_driver.md briefing
  carries the matching track-limited note dismissing the repeated compile-error claim.
  It is factual only and expires with this track.
- **Conditional allowance — apply only when the review run's resolved Track ID is
  exactly `scope-conditional-pre-review-gates-2026-07-31`; otherwise ignore.**
  **Pre-existing review_v2 composition surface**: a user adjudication (2026-07-22)
  deferred the review_v2 composition wire-only/render remediation to a separate track.
  A rollback-diagnoser adjudication on this track additionally recorded that the
  T043 warning-transport rerouting (`eprintln!` → `RunReviewOutput.diagnostics`) is
  transport-only and does not void the deferral; the run-lane findings
  (`render_verdict_payload` rendering, composition-built `[WARN]` text / typed
  diagnostic condition, `FindingsCountReviewer` placement) transfer to that deferred
  remediation track. The cli_composition.md note carries the matching record.
  The cli_composition briefing carries a matching track-limited note: invoke-leak
  findings on the PRE-EXISTING review_v2 gated-entry surface
  (`ReviewCompositionRoot::review_run_local`, the interim `ReviewServiceImpl` shim) are
  not reported when this track's diff makes no semantic change to that surface;
  NEW invoke paths added by this track remain fully reportable. The allowance expires
  when the deferred remediation track lands. It is not an accepted deviation for any
  other track or consumer.
- **Conditional allowance — apply only when the review run's resolved Track ID is
  exactly `scope-conditional-pre-review-gates-2026-07-31`; otherwise ignore.**
  **Pre-existing CommitHashReader / CommitHashWriter domain placement**: a
  rollback-diagnoser verdict (2026-08-09) adjudicated the layer relocation of these
  pre-existing ports (`libs/domain/src/review_v2/ports.rs`, introduced by an earlier
  track under the review-system-v2 redesign ADR; see the ADR index) as out-of-diff for
  this track; the
  placement-rule conflict is to be resolved by a dedicated ADR-routed track (adr-editor
  reconciling the ADR with the current Port-placement tie-break, then a migration track).
  The domain.md briefing carries a matching track-limited note. The allowance expires
  when that ADR track lands. It is not an accepted deviation for any other track or
  consumer.

- **Conditional allowance — apply only when the review run's resolved Track ID is
  exactly `scope-conditional-pre-review-gates-2026-07-31`; otherwise, do not apply
  this allowance.**
  **Staged completion of the phase convergence matrix and workflow adoption**: the
  shipped `.harness/config/phase-commands.json` does not yet declare the per-phase
  direct-upstream convergence pre-entry sequences, and canonical workflows do not yet
  invoke `bin/sotp phase enter`. Factual background: the admitted delta ADR
  phase-enter adoption and check-commands ADR and spec elements
  IN-12/IN-13/CN-06/AC-14/AC-15/AC-16 decide both obligations, and the in-track tasks
  T038 (check-zero-findings command), T039 (ref-verify chain selector), T040 (config
  convergence matrix), and T041 (workflow/adapter migration) are planned in batches
  B6/B7 to implement them. The user adjudicated on 2026-08-04 to accept intermediate
  commits while those planned tasks land within this track; this allowance expires when
  T040 and T041 are complete and is not an accepted deviation for any other track or
  consumer.
