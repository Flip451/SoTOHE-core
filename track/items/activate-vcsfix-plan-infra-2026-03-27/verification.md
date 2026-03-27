# Verification: activate-vcsfix-plan-infra-2026-03-27

## Scope Verified

- [ ] Phase 1: activation gitignore 修正（T001-T005）
- [ ] Phase 2: planner 移譲インフラ（T006-T008）

## Manual Verification Steps

### Phase 1: activation gitignore 修正
- [ ] `GITIGNORED_RENDERED_VIEWS` 定数が `activate.rs` に存在する
- [ ] `persist_activation_commit()` が gitignored パスを staging しない（ユニットテストで確認）
- [ ] `activation_artifact_paths()` が `registry.md` を含まず `spec.md` を含む（ユニットテストで確認）
- [ ] `sync_rendered_views()` のドキュメントに VCS フィルタ責任が明示されている
- [ ] `track:activate` end-to-end 検証: gitignored `track/registry.md` を含む rendered_paths で activation commit が成功する（回帰テスト `persist_activation_commit_skips_gitignored_registry_view`）

### Phase 2: planner 移譲インフラ
- [ ] `sotp plan codex-local --briefing-file <path>` が動作する（ブリーフィングファイルモード）
- [ ] `sotp plan codex-local --prompt "..."` が動作する（インラインプロンプトモード）
- [ ] `cargo make track-local-plan -- --briefing-file <path>` で Codex planner が hook ブロックなしに呼び出せる（end-to-end 確認、`--` セパレータ必須）
- [ ] `cargo make track-local-plan` が Makefile.toml に定義されている
- [ ] `Bash(cargo make track-local-plan:*)` が `.claude/settings.json` の `permissions.allow` に登録されている
- [ ] `.claude/rules/02-codex-delegation.md` が briefing file パターンを文書化している
- [ ] `.claude/rules/10-guardrails.md` の `--briefing-file` 記載が正しい方式に修正されている

### Wrapper regression test
- [ ] `scripts/test_make_wrappers.py` に `track-local-plan` ラッパーのテストケースを追加（既存パターンに準拠）

### CI
- [ ] `cargo make ci` が通る

## Result / Open Issues

(実装後に記入)

## verified_at

(実装完了後に記入)
