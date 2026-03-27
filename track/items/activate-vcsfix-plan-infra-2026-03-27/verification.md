# Verification: activate-vcsfix-plan-infra-2026-03-27

## Scope Verified

- [x] Phase 1: activation gitignore 修正（T001-T005）
- [x] Phase 2: planner 移譲インフラ（T006-T009）

## Manual Verification Steps

### Phase 1: activation gitignore 修正
- [x] `GITIGNORED_RENDERED_VIEWS` 定数が `activate.rs` に存在する
- [x] `persist_activation_commit()` が gitignored パスを staging しない（ユニットテスト `persist_activation_commit_skips_gitignored_registry_view` で確認）
- [x] `activation_artifact_paths()` が `registry.md` を含まず `spec.md` を含む（ユニットテスト `activation_artifact_paths_includes_spec_md_not_registry` で確認）
- [x] `sync_rendered_views()` のドキュメントに VCS フィルタ責任が明示されている
- [x] `track:activate` end-to-end 検証: gitignored `track/registry.md` を含む rendered_paths で activation commit が成功する（回帰テスト `persist_activation_commit_skips_gitignored_registry_view`）

### Phase 2: planner 移譲インフラ
- [x] `sotp plan codex-local --briefing-file <path>` が動作する（ブリーフィングファイルモード）
- [x] `sotp plan codex-local --prompt "..."` が動作する（インラインプロンプトモード）
- [x] `cargo make track-local-plan -- --briefing-file <path>` で Codex planner が hook ブロックなしに呼び出せる（end-to-end 確認、`--` セパレータ必須）
- [x] `cargo make track-local-plan` が Makefile.toml に定義されている
- [x] `Bash(cargo make track-local-plan:*)` が `.claude/settings.json` の `permissions.allow` に登録されている
- [x] `.claude/rules/02-codex-delegation.md` が briefing file パターンを文書化している
- [x] `.claude/rules/10-guardrails.md` の `--briefing-file` 記載が正しい方式に修正されている

### Wrapper regression test
- [x] `scripts/test_make_wrappers.py` に `track-local-plan` ラッパーのテストケースを追加（既存パターンに準拠）

### CI
- [x] `cargo make ci` が通る（1526 Rust tests + 245 Python selftest + all verify checks）

## Result / Open Issues

全タスク完了。問題なし。
- `permission-extensions.json` に `track-local-plan` エントリ追加済み（orchestra guardrails 通過）

## verified_at

2026-03-27
