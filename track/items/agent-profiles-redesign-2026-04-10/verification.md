# Verification: RV2-Phase2 agent-profiles redesign

## Scope Verified

- [ ] `.harness/config/agent-profiles.json` が存在し、新スキーマ (schema_version=1, providers は label のみ, capabilities 6 個) を満たす (T01)
- [ ] `.harness/config/samples/agent-profiles.{default,claude-heavy,codex-heavy}.json` の 3 サンプルが存在する (T01)
- [ ] `libs/infrastructure/src/agent_profiles.rs` に新 API (AgentProfiles / CapabilityConfig / ResolvedExecution / RoundType / resolve_execution) が実装されている (T02)
- [ ] resolve_execution のユニットテスト 6 件以上が PASS (Final / Fast fast_model only / Fast cross-provider / 不明 capability / 不正 JSON / missing file) (T02)
- [ ] `resolve_full_auto_from_profiles` / `ModelProfile` / `resolve_full_auto` のデッドコードが workspace 全体から削除されている (T03)
- [ ] `libs/usecase/src/pr_review.rs::resolve_reviewer_provider` が新 API を使用し、active_profile 参照が削除されている (T04)
- [ ] `apps/cli/src/commands/pr.rs` のパス参照 2 箇所が `.harness/config/agent-profiles.json` に更新されている (T05)
- [ ] `apps/cli/src/commands/review/tests.rs` の fixture が新パス + 新スキーマを使用している (T06)
- [ ] `libs/infrastructure/src/verify/orchestra.rs` の REVIEW_WRAPPER_TARGETS / MODEL_RESOLUTION_TARGETS が新パス基準に更新され、削除対象ファイル 02/03 のエントリが除去されている (T07)
- [ ] `.claude/rules/02-codex-delegation.md` が存在しない (T08)
- [ ] `.claude/rules/03-gemini-delegation.md` が存在しない (T09)
- [ ] `.claude/rules/10-guardrails.md` に Sandbox/Hook Coverage Warning が移設されている (T08, T11)
- [ ] `.claude/rules/08-orchestration.md` / `11-subagent-model.md` が新 capability 6 個 + 新パスに更新されている (T10, T11)
- [ ] `.claude/commands/track/{review,design,pr-review}.md` が更新されている (T12)
- [ ] `.claude/skills/{track-plan,codex-system,gemini-system}/SKILL.md` が更新されている (T13)
- [ ] top-level docs 9 ファイルが更新されている (CLAUDE.md / DEVELOPER_AI_WORKFLOW.md / knowledge/WORKFLOW.md / knowledge/DESIGN.md / LOCAL_DEVELOPMENT.md / START_HERE_HUMAN.md / track/workflow.md / .codex/instructions.md / .gemini/GEMINI.md) (T14)
- [ ] 旧 `.claude/agent-profiles.json` が削除されている (T15)
- [ ] ADR 2026-04-09-2235 の Status が Accepted に更新されている (T16)
- [ ] `knowledge/strategy/TODO.md` の該当エントリが done 状態 (T16)
- [ ] `cargo make ci` 全チェック通過 (T17)

## Manual Verification Steps

1. `ls .harness/config/` で `agent-profiles.json` と `samples/` ディレクトリの存在を確認
2. `cargo make test-one-exec resolve_execution` で新 loader のユニットテスト通過を確認
3. `cargo make test-one-exec resolve_reviewer_provider` で pr_review 側の統合テスト通過を確認
4. `rg -l '.claude/agent-profiles.json' apps/ libs/ .claude/ .codex/ .gemini/ CLAUDE.md DEVELOPER_AI_WORKFLOW.md LOCAL_DEVELOPMENT.md START_HERE_HUMAN.md knowledge/WORKFLOW.md knowledge/DESIGN.md knowledge/conventions/` で参照ゼロを確認 (track/ は歴史的 plan.md 等を含むため除外。knowledge/adr/ knowledge/research/ も除外)
5. `rg -l 'workflow_host|multimodal_reader' apps/ libs/ .claude/ .codex/ .gemini/ CLAUDE.md DEVELOPER_AI_WORKFLOW.md LOCAL_DEVELOPMENT.md START_HERE_HUMAN.md knowledge/WORKFLOW.md knowledge/DESIGN.md knowledge/conventions/` で参照ゼロを確認 (track/ は除外。knowledge/adr/ knowledge/research/ も除外)。加えて `.claude/rules/08-orchestration.md` / `.claude/commands/track/{review,design,pr-review}.md` / `.claude/skills/{track-plan,codex-system,gemini-system,repomix-snapshot}/SKILL.md` / `DEVELOPER_AI_WORKFLOW.md` / `track/workflow.md` に debugger への言及が残存していないことを個別確認する
6. `rg -l 'resolve_full_auto_from_profiles|ModelProfile|resolve_full_auto' apps/ libs/` で参照ゼロを確認 (T03 のデッドコード削除確認 — 3 シンボル全て)
7. `cargo make ci` を実行し全ゲート通過を確認 (特に verify-orchestra で新パスの整合性確認)
8. `ls .claude/rules/` で 02-codex-delegation.md と 03-gemini-delegation.md が存在しないことを確認
9. `ls .claude/agent-profiles.json` がエラーになる (ファイルが存在しない) ことを確認
10. Claude Code を再起動し、新 `.harness/config/agent-profiles.json` が正しく読み込まれること、主要 capability (planner / reviewer / researcher) が期待通り routing されることを確認

## Result

（実装完了後に記録）

## Verified At

（実装完了後に記録）

## Open Issues

（実装中に発見された問題を記録）

### 既知の Out-of-Scope (本トラックでは未対応)

- RV2-16 (Phase 3: planning review phase separation) の実装 — 別トラック (ADR 2026-04-09-2047)
- review escalation threshold の v2 再実装 — 将来 review 機能側で
- scripts/ 配下 Python ファイルの削除 — 別トラック (ADR 2026-04-09-2323 §Decision/5 段階的除去)
- `.harness/config/` への他設定ファイル (review-scope.json / architecture-rules.json / planning-artifacts.json) の移行 — 本 ADR では agent-profiles のみ対象
- `knowledge/research/` 配下 legacy 文書の capability 名更新 — ADR §5 末尾方針により修正対象外
- `.claude/rules/` 配下の他ファイル (04/05/06/07/09/01) のスリム化審査 — ユーザー指定でスコープ外
