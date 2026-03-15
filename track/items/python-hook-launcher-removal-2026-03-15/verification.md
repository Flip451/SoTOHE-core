# Verification: Python security hook launcher 除去

## 自動検証

- [x] `cargo make ci` 通過
- [x] `cargo make hooks-selftest` 通過（残存 Python hook テストが削除対象を参照していないこと）
- [x] `cargo make scripts-selftest` 通過（verify_orchestra_guardrails の更新が正しいこと）
- [x] `cargo make verify-orchestra` 通過

## 手動検証

- [x] `.claude/hooks/block-direct-git-ops.py` が存在しないこと
- [x] `.claude/hooks/file-lock-acquire.py` が存在しないこと
- [x] `.claude/hooks/file-lock-release.py` が存在しないこと
- [x] `.claude/hooks/test_policy_hooks.py` が存在しないこと
- [x] Bash コマンド実行時にフックが正常に動作すること（sotp 直接呼び出し）
- [x] Edit/Write 実行時に file-lock フックが正常に動作すること（SOTP_LOCK_ENABLED=1 時）
- [x] `.claude/rules/10-guardrails.md` に `block-direct-git-ops.py` への参照が残っていないこと
- [x] `.claude/rules/02-codex-delegation.md` に Python launcher 前提の記述が残っていないこと
- [x] `.claude/docs/DESIGN.md` の Python launcher 前提コメントが sotp 直接呼び出し前提に更新されていること
- [x] `libs/domain/src/hook/types.rs` の doc comment が Python launcher 前提でなくなっていること

## 結果

- verified_at: 2026-03-15
- 結果: 全項目パス。Python launcher 4ファイル削除済み、verify_orchestra_guardrails.py から verify_block_hook() 除去済み、ドキュメント4箇所を sotp 直接呼び出し前提に更新済み。`cargo make ci` 全通過。
