<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Python security hook launcher 除去

STRAT-03 Phase 1 完了: deprecated Python security hook launcher 3本を削除し、関連テスト・検証・ドキュメントを更新する。
settings.json は既に sotp 直接呼び出しに移行済みのため、Python launcher の削除のみで必須経路から Python 依存が消える。

## Python launcher ファイル削除

- [x] .claude/hooks/block-direct-git-ops.py, file-lock-acquire.py, file-lock-release.py を削除する
- [x] block-direct-git-ops.py の Python 関数に依存するテストファイル .claude/hooks/test_policy_hooks.py を削除する

## 検証スクリプト・ドキュメント更新

- [x] BLOCK_HOOK_PATH, BLOCK_HOOK_MARKERS, verify_block_hook() を削除する。Rust テストが権威的実装
- [x] 10-guardrails.md, 02-codex-delegation.md, .claude/docs/DESIGN.md, libs/domain/src/hook/types.rs の Python launcher 参照を sotp 直接呼び出し前提に更新する

## CI 検証

- [x] cargo make ci が通ることを確認する
