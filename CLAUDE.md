# CLAUDE.md

このファイルは保守者向けの最小インデックス。初見ユーザー向け導線は `DEVELOPER_AI_WORKFLOW.md`。

優先参照:
- `track/tech-stack.md`
- `track/workflow.md`
- `track/registry.md`
- `track/items/<id>/metadata.json`
- `track/items/<id>/spec.md`
- `track/items/<id>/plan.md`
- `track/items/<id>/verification.md`
- `.claude/docs/DESIGN.md`
- `knowledge/WORKFLOW.md`
- `knowledge/adr/README.md`
- `knowledge/strategy/TODO-PLAN.md`
- `.claude/agent-profiles.json`
- `.claude/rules/`
- `knowledge/conventions/README.md`
- `knowledge/conventions/`
- `architecture-rules.json`
- `knowledge/external/POLICY.md`
- `knowledge/external/guides.json`
- `TRACK_TRACEABILITY.md`

運用要点:
- 公開 UI は `/track:*`
- 旧 alias より `/track:*` を優先
- `plan.md` は `metadata.json` から render される read-only view
- `architecture-rules.json` が workspace 構造と layer policy の SSoT
- workspace tree は `cargo make workspace-tree` / `cargo make workspace-tree-full`
- `track/tech-stack.md` に未解決 `TODO:` がある間は実装開始禁止
- `knowledge/conventions/` への参照を維持すること
- 変更前に `.claude/rules/08-orchestration.md` `09-maintainer-checklist.md` `10-guardrails.md` を読むこと

詳細:
- orchestration / delegation: `.claude/rules/08-orchestration.md`
- maintainer checklist: `.claude/rules/09-maintainer-checklist.md`
- guardrails: `.claude/rules/10-guardrails.md`
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
