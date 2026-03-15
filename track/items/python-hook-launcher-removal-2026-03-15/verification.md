# Verification: Python security hook launcher 除去

## 自動検証

- [ ] `cargo make ci` 通過
- [ ] `cargo make hooks-selftest` 通過（残存 Python hook テストが削除対象を参照していないこと）
- [ ] `cargo make scripts-selftest` 通過（verify_orchestra_guardrails の更新が正しいこと）
- [ ] `cargo make verify-orchestra` 通過

## 手動検証

- [ ] `.claude/hooks/block-direct-git-ops.py` が存在しないこと
- [ ] `.claude/hooks/file-lock-acquire.py` が存在しないこと
- [ ] `.claude/hooks/file-lock-release.py` が存在しないこと
- [ ] `.claude/hooks/test_policy_hooks.py` が存在しないこと
- [ ] Bash コマンド実行時にフックが正常に動作すること（sotp 直接呼び出し）
- [ ] Edit/Write 実行時に file-lock フックが正常に動作すること（SOTP_LOCK_ENABLED=1 時）
- [ ] `.claude/rules/10-guardrails.md` に `block-direct-git-ops.py` への参照が残っていないこと
- [ ] `.claude/rules/02-codex-delegation.md` に Python launcher 前提の記述が残っていないこと
- [ ] `.claude/docs/DESIGN.md` の Python launcher 前提コメントが sotp 直接呼び出し前提に更新されていること
- [ ] `libs/domain/src/hook/types.rs` の doc comment が Python launcher 前提でなくなっていること

## 結果

- 検証日時: (未実施)
- 結果: (未実施)
