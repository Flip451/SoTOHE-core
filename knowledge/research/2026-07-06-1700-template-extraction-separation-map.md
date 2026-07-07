# Template Extraction Separation Map — sotp-dev vs Generic Template

Date: 2026-07-07
Status: survey complete (4 parallel Explore agents + inline verification)
Purpose: ground the pre-track ADR that reassesses the deferred sotp extraction decision
(`knowledge/adr/2026-03-23-2110-sotp-extraction-deferred.md`) and defines how to carve a
generic Rust-project template out of this repository.

## 1. Problem statement

This repository has a dual identity:

1. **sotp development workspace** — the 6 workspace crates (`libs/domain`, `libs/usecase`,
   `libs/infrastructure`, `apps/cli`, `apps/cli-composition`, `apps/cli-driver`) implement the
   sotp CLI itself, plus ~141 ADRs, 164 track items, ~18 archived tracks, and research notes
   that are sotp's own development history.
2. **Generic SDD template** — the track workflow, agent harness (`.claude/`, `.codex/`,
   `.gemini/`, `.agents/`, `.harness/`), git hooks, CI harness, and conventions that should
   host ANY Rust project.

The 2026-03-23 ADR deferred physical separation (SPLIT-03/04/05) as YAGNI; logical boundary
documentation (SPLIT-01) and `bin/sotp` path abstraction (SPLIT-02) were allowed anytime.
The original overfitting analysis artifacts were deleted by the knowledge-strategy cleanup;
recorded overfitting rates at that time: Makefile 50%, hooks 68%, scripts 73% (scripts since
removed entirely by the Python-runtime full removal).

`knowledge/conventions/responsibility-boundary.md` already defines the conceptual boundary
(framework enforces SoT Chain integrity / workflow mechanism / architecture rules / framework
code quality; the consumer owns provider config, signal-gate strictness, and domain code).
This survey turns that conceptual boundary into a concrete per-file separation map.

## 2. Buckets

- **GENERIC** — ships in the template unchanged.
- **SOTP-DEV** — sotp source / dev history; excluded from the template.
- **PARAMETERIZED** — the template needs the file, but its content embeds sotp-specific
  values (crate names, `libs/*`+`apps/*` paths, this-repo ADR slugs, sotp deps) that must be
  derived from config or reset to placeholders.

## 3. Cross-cutting findings

1. **The dominant coupling is that sotp lives in-workspace as `apps/cli` and is built from
   source.** ~20 Makefile tasks invoke `cargo run -p cli -- <subcommand>`. The template must
   keep the *invocations* (`bin/sotp ...`) and drop the *source build* (`build-sotp`,
   bootstrap step 3). `bin/sotp` is already `.gitignore`d — the binary boundary exists.
2. **sotp is already mostly config-driven at runtime.** All convention paths are joined onto
   a resolved repo root; the layer/crate graph, TDDD catalogue filenames, and schema-export
   targets all come from `architecture-rules.json`. sotp can already operate on an arbitrary
   repo tree that adopts the convention layout.
3. **Exactly three verifiers are hard-coupled to SoTOHE's own layout**, bypassing config and
   dispatched unconditionally:
   - `libs/infrastructure/src/verify/domain_purity.rs:10` (`libs/domain/src`)
   - `libs/infrastructure/src/verify/domain_strings.rs:10` (`libs/domain/src`)
   - `libs/infrastructure/src/verify/usecase_purity.rs:16` (`libs/usecase/src`)
   Making these arch-rules-driven (or opt-in per layer role) removes the single hard blocker
   in sotp source.
4. **Heavy build-time deps are confined to `libs/infrastructure`**: vendored `conch-parser`
   patch (shell guard) and `lancedb` (semantic dup/DRY index → protoc at build, MSRV 1.91).
   They constrain *building* sotp, not *running* the prebuilt binary. Prebuilt distribution
   (or feature-gating) neutralizes both for consumers.
5. **The agent harness contains no pure sotp-dev files** — only concentrated parameterization
   points (machine-readable layer maps) plus provenance leaks (this-repo ADR slugs, sotp
   source paths) in a handful of capability/workflow docs.
6. **TDDD's inherent hosting constraint**: the signal pipeline shells out
   `cargo +nightly rustdoc -p <crate>` against the target workspace, so the consumer repo
   must be a compilable cargo workspace with nightly rustdoc JSON available. This is a
   methodology constraint, not sotp overfitting.

## 4. Per-surface classification

### 4.1 Build/CI surfaces

GENERIC unchanged: `clippy.toml`, `rustfmt.toml`, `.repomixignore`, `.githooks/*`
(thin `bin/sotp hook dispatch` wrappers), docker/compose lifecycle tasks, standard Rust
quality tasks (fmt/clippy/test/check/deny/machete/llvm-cov), workspace lint policy
(`unwrap_used`/`panic` deny etc.), cargo-chef/sccache build layering, git/harness wrapper
tasks (`add`, `sync`, `track-branch-*`, `track-pr-*`, `track-local-review*`).

SOTP-DEV: `build-sotp` task; `vendor/` + `[patch.crates-io] conch-parser`; workspace
`[workspace.dependencies]` (sotp's dep set); `protobuf-compiler` in Dockerfile (lancedb);
`deny.toml` lancedb/fastembed/ort license+advisory exceptions; `.gitignore` entries
`.fastembed_cache`, `.semantic_index*`, `sotp-dry-index-*`; stale Python leftovers
(`.tool-versions` python 3.12.8, `__pycache__/`, `*.pyc`, `.cache/pytest/`).

PARAMETERIZED:
- `Cargo.toml` members (6 concrete crates) — becomes the skeleton workspace.
- `deny.toml` `[bans].deny` layer list — mirror of `architecture-rules.json`.
- Layer-hardcoded verify tasks: `verify-domain-strings`, `verify-domain-purity`,
  `verify-usecase-purity`, `export-schema -- --crate domain`.
- ~20 `cargo run -p cli --` → `bin/sotp` swaps across verify/signal/task-contract tasks.
- CI aggregates (`ci-local`, `ci-rust-local`, `ci-track-local`) — gate lists mix generic and
  harness gates.
- compose image names `rust-sdd-template-tools/-dev`; ci.yml branch triggers
  `[main, develop]`; tmpfs `private/` + `config/secrets` dirs.
- Dockerfile `APP_BIN=server` / `EXPOSE 8080` / distroless runtime stage — HTTP-server
  assumption; must be optional for CLI/lib consumers.
- `bootstrap` — generic except step 3 (`build-sotp`); becomes "install sotp binary" step.
- ci.yml `track/` branch recreate step + `ci-track-container` — template workflow feature
  (keep).

### 4.2 sotp source (genericity audit)

- Convention paths (`architecture-rules.json`, `track/items`, `knowledge/adr`,
  `knowledge/conventions`, `.harness/config/*.json`, `tmp/*-runtime`) are hardcoded consts
  but root-relative → portable when the consumer adopts the convention layout.
- Layer/crate model: genuinely config-driven (`arch.rs` `ArchRules`/`LayerEntry` parses
  arbitrary layers; `signal_layer_reader.rs` re-enumerates TDDD layers + per-layer catalogue
  files; catalogue filename derived `format!("{layer_id}-types.json")`).
- Hard blocker: the 3 purity/strings verifiers (§3.3).
- Self-reference: none to sotp's own crates — rustdoc runs against target-workspace crates
  from config; respects `CARGO_TARGET_DIR` / cargo metadata.
- Test fixtures embed `libs/domain` etc. pervasively but only under `#[cfg(test)]` — stays
  with sotp source, no runtime impact.

### 4.3 Agent harness

Highest-priority parameterization (machine-readable; silently mis-routes on other layouts):
- `.harness/config/review-scope.json` — 6 layer groups hardcode `libs/*`+`apps/*` patterns +
  briefing paths (SoT-artifact groups adr/spec/types/impl-plan/harness-policy are GENERIC).
- `.harness/catalogue-lint/config.json` + `presets/ddd-strict.json` — `permitted_layers` and
  `ForbidPrimitiveInTypes.layers` hardcode the 6 crate names (DDD role vocabulary GENERIC).

Per-layer review prompts `.harness/custom/review-prompts/{domain,usecase,infrastructure,cli,
cli_driver,cli_composition}.md` — hardcode crate paths, this-repo ADR filenames and decision
IDs → scaffold per layer from `architecture-rules.json`; `spec/adr/impl-plan/harness-policy/
types` prompts GENERIC.

Capability contracts, Flavor A (consumer layout): architecture-guard blocks in
`dry-fix-lead.md` (L133-135), `review-fix-lead.md` (L142-144),
`review-fix-lead-codex.md` (L176-178), plus `rollback-diagnoser.md`, `type-designer.md`
(layer gates + worked examples), `workflows/track/init.md` (hardcodes `domain-types.json`).
Inconsistency found while surveying: `dry-fix-lead.md` names the composition layer
`apps/cli-composition/` while `review-fix-lead.md`(+codex twin) say `apps/cli/` — one is
stale; reconcile during parameterization.

Flavor B (provenance references to sotp's own ADRs/source — replace with generic phrasing):
`workflows/track/done.md`, `capabilities/adr-editor.md`, `commands/track/diagnose.md`,
`rules/07-dev-environment.md` (lancedb/protoc note, `--crate domain`),
`.codex/config.toml` (one comment).

GENERIC: all 22 `/track:*` command adapters (except diagnose provenance), rules
01/08/09/10, all 8 `.claude/agents`, all skills, all 19 `.agents/skills`, remaining
workflows, `spec-designer`/`impl-planner` capabilities, ref-verifier prompts, `.codex/*`
(minus one comment), `.gemini/GEMINI.md`, `.claude/settings.json` (hook wiring generic via
`bin/sotp hook dispatch`; allowlist generic), `signal-gates`/`branch-strategy`/`dry-check`/
`agent-profiles` configs + samples, style TOMLs.

### 4.4 Docs / knowledge / track

- `knowledge/conventions/` (27+README): GENERIC 7 (`testing`, `coding-principles`,
  `git-notes`, `pre-track-adr-authoring`, `review-protocol`, `track-lifecycle`,
  `workflow-ceremony-minimization`); SOTP-DEV 3 (`shell-parsing`, `nightly-dev-tool`,
  `tddd-product-correctness`; `dry-check-workflow` borderline); PARAMETERIZED 18 — keep the
  rule, reset sotp specifics (crate paths, adapter/type names, dated ADR slugs, history
  codenames). Discriminator: `bin/sotp` / `cargo make` / `.harness/config` / artifact
  filenames = generic tooling vocabulary; `libs/apps` crate paths + concrete sotp types +
  dated ADR slugs = reset.
- `knowledge/adr/`: 141 ADRs ≈ 90% sotp dev history → exclude wholesale; keep `README.md`
  (authoring process + template) with an empty index.
- `knowledge/research/`: keep 5 generic seeds (`README.md`, `version-baseline-template.md`,
  one dated baseline sample, `harness-engineering-landscape`, `agent-agnostic-vcs-guardrail`);
  exclude ~23 sotp notes.
- `track/`: `product.md`, `product-guidelines.md`, `tech-stack.md` → reset to
  placeholder/TODO-seeded state (TODO-seeding IS the designed fresh state — implementation is
  blocked while `TODO:` markers remain). `items/` (164 dirs) + `archive/` (~18 + one stray
  loose review JSON) → ship empty. `registry.md` is a generated, gitignored view →
  regenerate via `bin/sotp track views sync` on first setup.
- `README.md`: already a template onboarding doc; reset product name only.
  `CLAUDE.md` / `AGENTS.md`: GENERIC as-is.
- `architecture-rules.json`: `canonical_modules` shell-parsing entry → SOTP-DEV (remove);
  `layers` = the template's default architecture (PARAMETERIZED; genericize the
  T006/`bin_target.rs` note); `module_limits` + `extra_dirs` GENERIC.

## 5. Design implications (candidates for the ADR)

- **D1 separation mode** — options: (a) in-repo boundary manifest + export tooling that
  emits the generic template tree (preserves the self-hosting dev loop; template is a build
  artifact), (b) immediate physical repo split (sotp repo + template repo), (c) root
  workspace swap with sotp as excluded subtree.
- **D2 bin/sotp distribution** — `cargo install --git <this repo> --tag <pinned>` at
  bootstrap vs prebuilt release binaries vs vendored source. Prebuilt/`cargo install`
  neutralizes the conch-parser/lancedb build constraints for consumers (protoc + MSRV only
  bind the build host).
- **D3 skeleton workspace** — template ships minimal compiling placeholder crates in the
  standard 6-crate layout matching `architecture-rules.json`; consumers reshape via the
  existing `architecture-customizer` skill.
- **D4 boundary SSoT** — machine-readable manifest (path → bucket (+ transform)) consumed by
  the export tooling; realizes SPLIT-01/02 from the deferred ADR.
- **Prerequisite fixes independent of D1**: make the 3 layout-hardcoded verifiers
  arch-rules-driven; derive `review-scope.json` / catalogue-lint `permitted_layers` /
  fix-lead guard blocks / `init.md` catalogue reference from `architecture-rules.json`;
  reconcile the `apps/cli-composition` vs `apps/cli` guard-block contradiction; strip
  Flavor-B provenance references; remove stale Python leftovers.

## 6. Sources

- Survey agents: build/CI surfaces; agent-harness surfaces; sotp source genericity audit;
  docs/knowledge/track surfaces (2026-07-07, this session).
- `knowledge/adr/2026-03-23-2110-sotp-extraction-deferred.md` — deferred decision + SPLIT
  item names.
- `knowledge/conventions/responsibility-boundary.md` — conceptual framework/consumer
  boundary.
