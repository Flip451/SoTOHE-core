<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Codex を Claude と同等の SoTOHE オーケストレーターにする設定追加

## Summary

Goal coverage note: task-coverage.json schema_version 1 maps in_scope / out_of_scope / constraints / acceptance_criteria only. GO-01 is covered by T003 and T005; GO-02 is covered by T001, T002, T004, T005, T006, and T007.

## Tasks (0/7 resolved)

### S1 — Codex Project Surface

> Add tracked project-local Codex config, repo-persistent execpolicy rules, instructions, and hook adapter.
> Keep config keys safe for repository sharing and route hooks through existing `bin/sotp hook dispatch` policy.

- [ ] **T001**: Codex project surface を repository root に追加する。対象は `.codex/config.toml`、`.codex/instructions.md`、`.codex/rules/default.rules`、`.codex/hooks/sotp-hook.sh`。config は project-local に安全な key だけを使い、provider auth/base URL/profile/telemetry/notify は入れない。rules は SoTOHE canonical `cargo make` wrapper と read-only Git inspection の prefix を least-privilege で allow し、direct git mutation、destructive shell、broad shell wrapper は allow しない。hook adapter は policy を再実装せず `bin/sotp hook dispatch <hook-id>` へ委譲する。

### S2 — Agents And Skills

> Add thin Codex custom agent TOMLs and promote the missing specialist workflows into root `.agents/skills`.
> Reuse the existing review-fix and dry-fix skills instead of duplicating them.

- [ ] **T002**: Codex custom agents と specialist skills を追加する。`.codex/agents/{orchestrator,spec-designer,impl-planner,type-designer,adr-editor,review-fix-lead,dry-fix-lead}.toml` は薄い role definition と実行境界だけを持たせ、詳細 workflow は `.agents/skills/{spec-designer,impl-planner,type-designer,adr-editor}/SKILL.md` に置く。既存 `.agents/skills/review-fix-lead` と `.agents/skills/dry-fix-lead` は再利用し、agent TOML だけが存在して workflow が空になる状態を避ける。

### S3 — Provider Selection

> Represent Codex root orchestration through the existing `capabilities.orchestrator.provider` field.
> Avoid introducing a second profile concept or Codex-only routing schema.

- [ ] **T003**: Codex root orchestrator の provider assignment を既存 `capabilities.orchestrator.provider` で表現できるように tracked sample config を追加または更新する。新しい profile layer、root-host 専用 schema、Codex 専用 parallel config map は導入しない。default template は Claude/Codex の両 root orchestrator 選択肢を恒久的に保持し、specialist provider assignment は root provider 選択とは別問題として扱う。

### S4 — Verifier Coverage

> Extend `verify-orchestra` to validate the Codex project surface, command rules, custom agents, skills, and hooks.
> Add negative tests for missing artifacts and dangerous repo-persistent rule entries.

- [ ] **T004**: `verify-orchestra` を拡張し、Claude surface に加えて Codex project config、project-local rules、custom agent TOMLs、specialist skill coverage、hook adapter を fail-closed に検証する。rules 検証では canonical wrapper/read-only Git prefix が存在し、direct git mutation/destructive/broad shell allow が存在しないことを確認する。agent と skill の対応、hook adapter の `bin/sotp hook dispatch` 接続、必須 artifact 欠落、危険な rules entry に対する unit test を追加する。

### S5 — Operational Docs

> Document permanent Claude/Codex root orchestrator support and the trusted-project prerequisite for project-local `.codex` rules.
> Keep `/track:*` unchanged and avoid durable references to scratch/cache paths.

- [ ] **T005**: 運用文書を Claude 固定から Claude/Codex の恒久的な root orchestrator 選択に更新する。対象は `track/workflow.md`、`.codex/instructions.md`、`.claude/rules/08-orchestration.md` 相当の root orchestration 説明。`/track:*` user-facing command surface は変更しない。project-local `.codex` config/rules/hooks が trusted project でのみ読み込まれる前提を記述し、git history に残らない scratch/cache path を durable reference として書かない。

### S6 — Verification

> Run rule/verifier checks, CI, review, DRY, and PR workflow gates.
> Fix any failures within the scoped implementation.

- [ ] **T006**: Codex rules と verifier の動作をローカルで確認する。`.codex/rules/default.rules` は `codex execpolicy check` または `verify-orchestra` unit tests で allow/forbid の代表例を検証し、`cargo make verify-orchestra` と `cargo make ci` を通す。必要な clippy/test/doc-link 修正は同タスク内で行う。
- [ ] **T007**: track の最終 gate を通す。`bin/sotp review check-approved`、`bin/sotp dry check-approved`、plan artifact refs、catalogue-spec refs/signals、view freshness を確認し、review/DRY/PR workflow を既存 SoTOHE gate で完了させる。ADR/spec/docs/config/agents/skills に scratch/cache path の durable reference が残っていないことも最終確認する。
