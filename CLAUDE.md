# CLAUDE.md

このファイルは、このテンプレートを扱う Claude Code 向けの保守者リファレンスです。
初見ユーザー向けの操作導線は `DEVELOPER_AI_WORKFLOW.md` を参照すること。

## 1. Primary Role

Claude Code is the orchestrator.

- User-facing interface: `/track:*`
- Context management: `track/`
- Execution workflows: `takt`
- Capability routing: `.claude/agent-profiles.json`
- Default specialist profile:
  - `planner` / `reviewer` / `debugger`: Codex CLI
  - `researcher` / `multimodal_reader`: Gemini CLI
  - `implementer`: Claude Code
- Parallel execution: Agent Teams

Host orchestration stays in Claude Code.
Specialist capabilities may switch as models evolve, but the public `/track:*` interface should remain stable.

用語:

- `track`: `metadata.json`（SSoT） / `spec.md` / `plan.md`（読み取り専用ビュー） / `verification.md` / 進捗を管理する文脈管理レイヤー
- `takt`: 実装やレビューを進める実行ワークフロー

## 2. Source Of Truth

Read these first before planning or implementation:

- `track/tech-stack.md`
- `track/workflow.md`
- `track/registry.md`
- `project-docs/conventions/README.md`
- `track/items/<id>/metadata.json` (SSoT for task state)
- `track/items/<id>/spec.md`
- `track/items/<id>/plan.md` (read-only view rendered from metadata.json)
- `track/items/<id>/verification.md`
- `TAKT_TRACK_TRACEABILITY.md`
- `.claude/docs/DESIGN.md`
- `.claude/rules/`
- `docs/EXTERNAL_GUIDES.md`
- `docs/external-guides.json`
- `docs/architecture-rules.json`

Operational split:

- `DEVELOPER_AI_WORKFLOW.md`: user-facing operating guide
- `CLAUDE.md`: maintainer/reference guide
- `track/workflow.md`: day-to-day workflow rules
- `project-docs/conventions/`: project-specific engineering rules and implementation policies
- `TAKT_TRACK_TRACEABILITY.md`: `plan.md` state transitions and registry update rules
- `docs/external-guides.json`: registry for long-form external guides cached outside git
- `docs/EXTERNAL_GUIDES.md`: operating policy for external long-form guides
- `docs/architecture-rules.json`: machine-readable layer dependency source of truth for `deny.toml` and `scripts/check_layers.py`
- `.claude/agent-profiles.json`: capability-to-provider mapping source of truth

## 3. Canonical User Interface

Public interface is `/track:*`.
Do not prefer older aliases or direct internal names in user-facing guidance.

Primary commands:

- `/track:setup`
- `/track:catchup`
- `/track:plan <feature>`
- `/track:full-cycle <task>`
- `/track:implement`
- `/track:review`
- `/track:pr-review`
- `/track:revert`
- `/track:ci`
- `/track:commit <message>`
- `/track:archive <id>`
- `/track:status`

Detailed command semantics live in:

- `track/workflow.md`
- `.claude/docs/WORKFLOW.md`
- `.claude/commands/track/*.md`

Note: For workspace architecture migration (crate map, dependency direction, enforcement rules),
use `/architecture-customizer` — a separate, dedicated entry point outside the `/track:*` namespace.

For adding project-specific convention docs under `project-docs/conventions/`,
use `/conventions:add <name>` — a separate formal entry point outside the `/track:*` namespace for project-specific implementation rules.

## 4. Delegation Rules

Use the minimum capable capability first, then resolve it via `.claude/agent-profiles.json`.

- Claude Code (`orchestrator` host):
  - normal edits
  - workflow control
  - file synchronization
  - user interaction
- specialist capabilities:
  - `planner`: architecture design, trait/module planning, trade-off evaluation
  - `researcher`: crate research, codebase-wide analysis, external research
  - `implementer`: difficult Rust implementation, refactoring, performance-oriented edits
  - `reviewer`: code review, correctness analysis, idiomatic Rust checks
  - `debugger`: compile-error diagnosis, failing test analysis
  - `multimodal_reader`: PDF / image / audio / video understanding
- provider resolution:
  - default profile maps `planner` / `reviewer` / `debugger` to Codex CLI
  - default profile maps `researcher` / `multimodal_reader` to Gemini CLI
  - default profile maps `implementer` to Claude Code
- Agent Teams:
  - `/track:implement`
  - `/track:review`
- takt:
  - autonomous implementation / review workflows driven by `.takt/pieces/`

If unsure:

1. Workflow control or user interaction → Claude Code
2. Research or multimodal need → `researcher` / `multimodal_reader`
3. Design, review, or debugging need → `planner` / `reviewer` / `debugger`
4. Deterministic workflow execution → takt
5. Implementation work → `implementer`

## 5. Workflow Reference

Maintainer summary:

- day-to-day workflow and gates: `track/workflow.md`
- project-specific coding rules: `project-docs/conventions/README.md`
- Claude-side execution model: `.claude/docs/WORKFLOW.md`
- state transitions and registry rules: `TAKT_TRACK_TRACEABILITY.md`
- implementation must not start while `track/tech-stack.md` still has unresolved `TODO:` entries
- strict tech-stack guardrails are on by default; template maintainers may disable them locally only for template work

## 6. Maintainer Checklist

When changing workflow or architecture, update all affected layers together.

Host prerequisite:

- `python3` is required for `.claude/hooks/*.py` and `scripts/external_guides.py`

Always consider:

- user-facing docs:
  - `DEVELOPER_AI_WORKFLOW.md`
  - `.claude/docs/WORKFLOW.md`
- track docs:
  - `track/workflow.md`
  - `track/tech-stack.md`
  - `track/registry.md`
  - `TAKT_TRACK_TRACEABILITY.md`
- enforcement:
  - `Makefile.toml`
  - `scripts/verify_*.py`
  - `scripts/track_schema.py` / `track_state_machine.py` / `track_markdown.py`
  - `.claude/settings.json`
  - `.claude/hooks/`
  - `scripts/external_guides.py`
- takt definitions:
  - `.takt/config.yaml`
  - `.takt/pieces/`
  - `.takt/personas/`

After such changes, run `cargo make ci`.

## 7. Workspace Map

This tree should mirror the current workspace member paths from `docs/architecture-rules.json`.
The default template uses `apps/` and `libs/`, but other roots are allowed when this map stays in sync.

```text
Cargo.toml                  # workspace definition
apps/
└── cli/                    # CLI entry point + composition root
libs/
├── domain/                 # domain layer
├── usecase/                # use case layer
└── infrastructure/         # infrastructure layer
project-docs/
└── conventions/
    ├── README.md           # project-specific rule index
    └── *.md                # project-specific rules chosen by the project
track/
├── product.md              # product goals
├── product-guidelines.md   # product constraints
├── tech-stack.md           # technology decisions
├── workflow.md             # workflow rules
├── registry.md             # track registry
└── items/<id>/
    ├── spec.md             # what to build
    ├── plan.md             # read-only view (rendered from metadata.json)
    ├── verification.md     # manual verification record
    └── metadata.json       # SSoT: task state, plan structure, track status
```

## 8. Guardrails

Core guardrails:

- Prefer `/track:*` in user-facing guidance
- Do not use direct `git add` / `git commit`
- Do not tell users to run `*-local` tasks directly
- Keep `track/tech-stack.md` free of blocking `TODO:` before implementation
- Keep `track/registry.md`, `spec.md`, `plan.md`, and `verification.md` synchronized
- Keep `cargo make ci`, `cargo make deny`, and `cargo make verify-*` as reproducible final gates (`run --rm`)
- Before committing code changes, run the `reviewer` capability review cycle
  (review → fix → review → ... → no findings). Do not commit until the reviewer
  reports zero findings. The reviewer provider is resolved via `.claude/agent-profiles.json`.

Operational details live in:

- `track/workflow.md`
- `.claude/docs/WORKFLOW.md`
- `.claude/settings.json`
- `.claude/hooks/`

## 9. Key References

- `.claude/docs/WORKFLOW.md`
- `.claude/docs/DESIGN.md`
- `.claude/agent-profiles.json`
- `.claude/rules/02-codex-delegation.md`
- `.claude/rules/03-gemini-delegation.md`
- `.claude/rules/07-dev-environment.md`
- `.claude/skills/track-plan/SKILL.md`
- `.claude/skills/codex-system/SKILL.md`           # Codex delegation skill
- `.claude/skills/gemini-system/SKILL.md`          # Gemini delegation skill
- `.claude/commands/architecture-customizer.md`    # execution behavior: step-by-step workflow for architecture migration
- `.claude/skills/architecture-customizer/SKILL.md`  # backing skill definition registered in `skills:` metadata
