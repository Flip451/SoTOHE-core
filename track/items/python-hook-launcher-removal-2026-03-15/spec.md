# Spec: Python security hook launcher 除去

## 概要

STRAT-03 Phase 1 の完了として、deprecated Python security hook launcher 3本を削除し、関連するテスト・検証スクリプト・ドキュメントを更新する。

## 背景

- `.claude/settings.json` は既に Rust バイナリ (`sotp hook dispatch`) の直接呼び出しに移行済み
- Python launcher は "deprecated launcher retained for rollback" として残存
- ロールバック期間が十分に経過し、安定運用を確認済み
- Python launcher の存在自体が `.venv` / `python3` への暗黙の依存を残し、`INF-09` (bootstrapping paradox) の温床となっている

## ゴール

- security-critical hook 3本の Python launcher ファイルを削除する
- 関連するテスト・検証スクリプトを更新する
- ドキュメントの参照を Rust 実装に合わせる
- `cargo make ci` が通る状態を維持する

## スコープ

### 削除対象

| ファイル | 理由 |
|---|---|
| `.claude/hooks/block-direct-git-ops.py` | deprecated launcher + policy logic |
| `.claude/hooks/file-lock-acquire.py` | deprecated launcher |
| `.claude/hooks/file-lock-release.py` | deprecated launcher |
| `.claude/hooks/test_policy_hooks.py` | 削除対象の Python module に依存 |

### 更新対象

| ファイル | 変更内容 |
|---|---|
| `scripts/verify_orchestra_guardrails.py` | `verify_block_hook()` 関連コード削除 |
| `.claude/rules/10-guardrails.md` | `block-direct-git-ops.py` 参照を更新 |
| `.claude/rules/02-codex-delegation.md` | Python hook 参照を更新 |
| `.claude/docs/DESIGN.md` | Python launcher 前提のコメント・設計ノートを sotp 直接呼び出し前提に更新 |
| `libs/domain/src/hook/types.rs` | Python launcher 前提の doc comment を sotp 直接呼び出し前提に更新 |

### 対象外

- Non-critical advisory hooks (`agent-router.py`, `check-codex-before-write.py` 等)
- Hook 共通ライブラリ (`_shared.py`, `_agent_profiles.py`)
- `test_verify_scripts.py` の退行検知テスト（sotp → python3 置換を検知するテストとして有効）

## 完了条件

- [ ] Python launcher 3本が削除されている
- [ ] `test_policy_hooks.py` が削除されている
- [ ] `verify_orchestra_guardrails.py` が Python ファイル存在に依存しない
- [ ] ドキュメントが Rust 実装を反映している
- [ ] `cargo make ci` が通る
