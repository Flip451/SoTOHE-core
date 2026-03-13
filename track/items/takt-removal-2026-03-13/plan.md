<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# takt 廃止計画

`takt` を段階的縮退ではなく最終的に廃止し、公開操作面は `/track:*` のまま維持する。
実行オーケストレーション、pending scratch、traceability、CI/guardrail を Claude Code + Rust CLI 前提へ再設計する。
削除対象と代替経路を先に固定し、workflow・ドキュメント・検証を同期させながら `takt` 依存を repo から外す。

## Related Conventions (Required Reading)

project-docs/conventions/README.md
project-docs/conventions/security.md


## Inventory and public workflow rewrite

- [x] 現行の takt 依存面を棚卸しし、`.takt/` ディレクトリ、`cargo make takt-*` wrapper、`scripts/takt_profile.py`、`.claude/commands/**`、`.claude/rules/**`、`.claude/docs/WORKFLOW.md`、`START_HERE_HUMAN.md`、agent routing/profile 設定、traceability 文書、CI/guardrail の接点を inventory と cutover 原則として固定する
- [x] `/track:*` を公開インターフェースとして維持したまま、`.claude/commands/track/full-cycle.md`、`.claude/commands/track/setup.md`、`.claude/docs/WORKFLOW.md`、`START_HERE_HUMAN.md`、agent-router の案内文を含む workflow 文書・運用ガイド・agent profile 説明から takt を実行レイヤとして外し、Claude Code + Agent Teams + Rust CLI を前提にした新しい orchestration 記述へ置き換える

## Artifact and runtime removal design

- [x] takt 由来の pending artifact（`.takt/pending-add-paths.txt`, `.takt/pending-commit-message.txt`, `.takt/pending-note.md`）と handoff/debug scratch の扱いを再設計し、`libs/usecase/src/git_workflow.rs`、`scripts/git_ops.py`、関連テストの transient path 契約も含めて、通常 workflow と commit traceability が `tmp/` または別管理経路だけで完結するように移行計画を固める
- [x] `Makefile.toml` の `takt-*` / `takt-failure-report` wrapper、`.takt/config.yaml` / `pieces/` / `personas/`、`scripts/takt_profile.py` とそのテスト群を廃止または置換する差分を設計し、削除順序と互換境界を定義する

## Guardrails, docs, and rollout definition

- [ ] `.claude/settings.json`、`.claude/agent-profiles.json`、`.claude/hooks/_agent_profiles.py`、`.claude/hooks/agent-router.py`、`scripts/verify_orchestra_guardrails.py`、関連 selftest、`DEVELOPER_AI_WORKFLOW.md`、`LOCAL_DEVELOPMENT.md`、`track/workflow.md`、`TAKT_TRACK_TRACEABILITY.md`、`.claude/rules/07-dev-environment.md`、`.claude/commands/track/commit.md` を takt 非依存の整合した状態へ更新する実装計画を作り、必要なら後継文書へ再編する
- [ ] takt 廃止後の Definition of Done を固定し、`cargo make ci`、PR workflow、`/track:commit`、`/track:review`、archive/registry 更新が takt 無しで閉じることを検証する rollout 順と最終撤去条件を明文化する
