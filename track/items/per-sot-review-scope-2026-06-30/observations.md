# Observations: per-sot-review-scope-2026-06-30

## AC-13 移行網羅性検証メモ（T005）

`2026-06-30T18:50:13Z` 時点の未コミット worktree snapshot として、T001-T004 の変更対象面に残存する `plan-artifacts` / `plan_artifacts` スコープ名参照がないこと、および本 track の Git 管理対象 artifact と新規 `observations.md` が新 4 scope (`adr` / `spec` / `types` / `impl-plan`) または `review_operational` のいずれかに分類されることを記録する。これは T005 の完了判定ではない。T005 は `impl-plan.json` 上で `todo` のままであり、T001-T004 の status transition 後にこの手順を再実行して T005 の evidence として確定する。

### 残存 `plan-artifacts` 文字列の分類

T005 が指定する T001-T004 の変更対象面だけを対象にした scoped grep 手順は以下。T005 の pass / fail 判定では、この同じ scoped surface を再実行対象にする。

```sh
rg -n 'plan-artifacts|plan_artifacts' \
  .harness/config/review-scope.json \
  .harness/custom/review-prompts \
  .harness/workflows/track/full-cycle.md \
  .harness/capabilities/rollback-diagnoser.md \
  .claude/commands/track/diagnose.md \
  .claude/skills/diagnose/SKILL.md \
  .claude/agents/rollback-diagnoser.md \
  .agents/skills/rollback-diagnoser/SKILL.md \
  .codex/agents/rollback-diagnoser.toml \
  .codex/instructions.md \
  knowledge/conventions/enforce-by-mechanism.md \
  libs/domain/src/track_phase.rs \
  apps/cli-composition/src/track/fixpoint_resolve.rs \
  libs/usecase/src/fixpoint_resolve.rs \
  libs/infrastructure/src/review_v2/scope_config_loader.rs \
  apps/cli/src/commands/review/tests.rs \
  libs/domain/src/review_v2/tests.rs
```

snapshot 結果: no matches（`rg` exit code 1）。したがって、現時点の未コミット worktree では T001-T004 の変更対象面にスコープ名としての旧 `plan-artifacts` 参照は残存していない。

全 worktree grep は判定根拠にしない。`target/doc/**` の rustdoc JSON、過去 track の `review.json` / `dry-check.json`、および本 track 自身の planning artifact には歴史的記録や計画文として `plan-artifacts` が残り得るため、AC-13 の scoped surface から除外する。

### track artifact の scope 分類結果

Git 管理対象の本 track artifact は `git ls-files -z track/items/per-sot-review-scope-2026-06-30 | xargs -0 bin/sotp review classify` で分類した。新規 `observations.md` は未追跡ファイルなので、別途 `bin/sotp review classify track/items/per-sot-review-scope-2026-06-30/observations.md` で分類した。T005 の完了判定時には同じ分類手順を再実行する。

| ファイル群 | 分類先 |
|---|---|
| `spec.json` / `spec.md` | `spec` |
| `*-types.json` / `contract-map.md` | `types` |
| `impl-plan.json` / `task-coverage.json` / `task-contract.json` / `plan.md` / `observations.md` | `impl-plan` |
| `metadata.json` / `*-types.md` / `*-type-signals.json` / `*-catalogue-spec-signals.json` / `spec-adr-verify-cache.json` / `review.json` | `<excluded>` (review_operational) |

snapshot 結果: Git 管理対象 artifact と `observations.md` は、いずれも暗黙の `other` スコープには落ちていない。`*-graph-d1` / `*-graph-d2` / `logs` / `*.lock` は CI・review 実行時の gitignored runtime output であり、AC-13 の track artifact 分類判定から除外する。

### T005 実装中の発見と T001 への追記

本 batch の未コミット worktree 検証中に、以下 2 件の `other` scope 落ちを発見した。いずれも本 ADR が掲げる SoT 別スコープ化の整合性を保つため、**同一 batch 内で T001 (`review-scope.json`) を追加修正**した:

1. **`*-types-baseline.json` (6 layer)** が `review_operational` に未列挙だった（Phase 2 で type-designer が生成する rustdoc baseline。SSoT ではなく機械生成 cache）。→ `review_operational` に `track/items/<track-id>/*-types-baseline.json` を追加（D2 の意図に整合）。
2. **`.claude/skills/**`** が `harness-policy` の patterns に未列挙だった（既存の `harness-policy` 漏れ。T003 で `.claude/skills/diagnose/SKILL.md` を編集した結果として顕在化）。→ `harness-policy` patterns に `.claude/skills/**` を追加（既存の `.claude/commands/**` / `.claude/rules/**` / `.claude/agents/**` 列挙と並列）。

修正後の snapshot 再分類で、Git 管理対象 artifact と `observations.md`、および T003 の編集対象がいずれも `other` スコープに落ちないことを確認した。T005 の status transition 時には、この確認を再実行して完了 evidence とする。

## 検証手順

- 残存検証コマンド: 上記の T001-T004 scoped `rg -n 'plan-artifacts|plan_artifacts' ...`
- 分類検証コマンド: `git ls-files -z track/items/per-sot-review-scope-2026-06-30 | xargs -0 bin/sotp review classify` および `bin/sotp review classify track/items/per-sot-review-scope-2026-06-30/observations.md`
- 実施日時: 2026-06-30T18:50:13Z
