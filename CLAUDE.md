# CLAUDE.md

Minimal maintainer index for this repository. First-time user onboarding lives in `DEVELOPER_AI_WORKFLOW.md`.

Priority references:

- `track/tech-stack.md`
- `track/workflow.md`
- `track/registry.md`
- `track/items/<id>/metadata.json`
- `track/items/<id>/spec.md`
- `track/items/<id>/plan.md`
- `track/items/<id>/observations.md` (optional — manual observation log)
- `knowledge/DESIGN.md`
- `knowledge/WORKFLOW.md`
- `knowledge/adr/README.md`
- `knowledge/strategy/TODO-PLAN.md`
- `.harness/config/agent-profiles.json`
- `.claude/rules/`
- `knowledge/conventions/README.md`
- `knowledge/conventions/`
- `knowledge/conventions/pre-track-adr-authoring.md`  # ADR authored before /track:plan runs
- `knowledge/conventions/workflow-ceremony-minimization.md`  # post-hoc review + approved-state removal
- `architecture-rules.json`
- `knowledge/external/POLICY.md`
- `knowledge/external/guides.json`
- `TRACK_TRACEABILITY.md`

Operating notes:

- Public UI is `/track:*`
- Prefer `/track:*` over any legacy alias
- `plan.md` is a read-only view rendered from `metadata.json`
- `architecture-rules.json` is the SSoT for workspace structure and layer policy
- Workspace tree: `cargo make workspace-tree` / `cargo make workspace-tree-full`
- Implementation is blocked while `track/tech-stack.md` still has unresolved `TODO:` markers
- Keep references to `knowledge/conventions/` up to date
- Read `.claude/rules/08-orchestration.md`, `.claude/rules/09-maintainer-checklist.md`, and `.claude/rules/10-guardrails.md` before making changes
- The track workflow uses a **pre-track stage + 3-phase** structure:
  - **Pre-track stage**: ADRs are authored under `knowledge/adr/` outside the track (user-driven, see `knowledge/conventions/pre-track-adr-authoring.md`)
  - **Phase 0**: `/track:init` → `metadata.json` (identity-only)
  - **Phase 1**: `/track:spec-design` → `spec.json` (spec SSoT)
  - **Phase 2**: `/track:type-design` → `<layer>-types.json` (type-contract SSoT)
  - **Phase 3**: `/track:impl-plan` → `impl-plan.json` + `task-coverage.json` (progression markers + spec coverage)
  - `/track:plan` orchestrates these four commands in order as a thin state machine; back-and-forth escalation re-invokes the upstream writer on gate failure
- Artificial states such as `approved` / `Status` are removed; gates are built from SoT Chain signals (🔵🟡🔴) plus binary checks (see `knowledge/conventions/workflow-ceremony-minimization.md`)

Details:

- Orchestration / delegation: `.claude/rules/08-orchestration.md`
- Maintainer checklist: `.claude/rules/09-maintainer-checklist.md`
- Guardrails: `.claude/rules/10-guardrails.md`
- `knowledge/DESIGN.md`
- `.harness/config/agent-profiles.json`
- `.claude/rules/07-dev-environment.md`
- `.claude/skills/codex-system/SKILL.md`           # Codex delegation skill
- `.claude/skills/gemini-system/SKILL.md`          # Gemini delegation skill
- `.claude/commands/architecture-customizer.md`    # execution behavior: step-by-step workflow for architecture migration
- `.claude/skills/architecture-customizer/SKILL.md`  # backing skill definition registered in `skills:` metadata
