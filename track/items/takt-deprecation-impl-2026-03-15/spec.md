# Spec: takt 廃止実装

## Goal

`takt-removal-2026-03-13` で固定した計画に基づき、takt の runtime / wrapper / テスト / ドキュメントをリポジトリから完全に除去する。最終状態は `/track:*` + Rust CLI + Claude Code / Agent Teams のみで全 workflow が閉じること。

## Scope

- `.takt/` ディレクトリ配下の全ファイル削除
- `Makefile.toml` の `takt-*` wrapper と `TAKT_PYTHON` env 変数の除去
- `scripts/takt_profile.py`、`scripts/takt_failure_report.py` と関連テスト群の削除
- `.claude/hooks/` の `TAKT_SESSION` silence guard と `takt_host_*` 互換エイリアスの除去
- `.claude/settings.json` の legacy pending permissions 除去
- `.claude/rules/`、`track/workflow.md`、`DEVELOPER_AI_WORKFLOW.md`、`TAKT_TRACK_TRACEABILITY.md` の takt 言及除去
- `libs/usecase/src/git_workflow.rs`、`scripts/git_ops.py` の `.takt/pending-*` transient path 除去
- `Makefile.toml` の `add-pending-paths`、`commit-pending-message`、`note-pending` legacy wrapper 除去
- CI selftest / verify script の takt 前提除去

## Non-Goals

- takt と同等の新しい自律キューシステムを作ること
- Python utility の全面削除（takt 関連以外は scope 外）
- 既存 track の git notes や履歴の書き換え

## Constraints

- 公開 UI `/track:*` は維持する
- `cargo make ci` を各タスク完了時に green に保つ
- `metadata.json` SSoT の track workflow は維持する
- security-critical hook と branch guard の fail-closed 契約は維持する
- 削除順序は `takt-runtime-removal-sequence.md` の Phase A→B→C→D に従う

## Canonical Design

- `track/items/takt-removal-2026-03-13/takt-runtime-removal-sequence.md`
- `track/items/takt-removal-2026-03-13/takt-removal-definition-of-done.md`
- `track/items/takt-removal-2026-03-13/pending-artifact-cutover.md`
- `track/items/takt-removal-2026-03-13/takt-touchpoint-inventory.md`

## Acceptance Criteria

- [ ] `.takt/` ディレクトリがリポジトリから完全に除去されている
- [ ] `Makefile.toml` に `takt-*` wrapper、`TAKT_PYTHON`、legacy `pending-*` wrapper が存在しない
- [ ] `scripts/takt_profile.py`、`scripts/takt_failure_report.py`、`test_takt_*` が削除されている
- [ ] `.claude/hooks/` の `TAKT_SESSION` guard と `takt_host_*` エイリアスが除去されている
- [ ] `.claude/settings.json` に legacy pending permissions が存在しない
- [ ] 全ドキュメントから takt を primary path として案内する記述が除去されている
- [ ] `git_workflow.rs` と `git_ops.py` から `.takt/pending-*` transient path が除去されている
- [ ] `cargo make ci` が takt 無しで全ゲート通過する
- [ ] M1〜M4 の exit criteria が verification.md に記録されている
