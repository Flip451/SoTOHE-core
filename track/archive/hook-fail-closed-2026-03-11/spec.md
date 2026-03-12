# Spec: Hook Fail-Closed via Rust Dispatcher

## Goal

Python フックの fail-open 例外処理パターンを解消する。セキュリティクリティカルなフックを Rust `sotp hook dispatch` に移行し、型システムと Result 型で fail-closed を構造的に保証する。アドバイザリーフックは Python のまま warn-and-log に改善する。

## Scope

### In scope

- **Domain layer** (`libs/domain/src/`):
  - `decision.rs`: 共有 `Decision` enum（guard と hook で再利用、SRP）
  - `hook/types.rs`: `HookName`, `HookContext`, `HookInput`（framework-free、serde 不使用）
  - `hook/verdict.rs`: `HookVerdict`（`Decision` を使用）
  - `hook/error.rs`: `HookError`
  - `guard/verdict.rs`: `GuardVerdict` を共有 `Decision` に移行
- **UseCase layer** (`libs/usecase/src/`):
  - `hook.rs`: `HookHandler` trait（OCP: 新フック追加時に既存コード不変）
  - `GuardHookHandler`: `domain::guard::policy::check` に委譲
  - `LockAcquireHookHandler` / `LockReleaseHookHandler`: `domain::lock::FileLockManager` に委譲
- **CLI layer** (`apps/cli/src/commands/`):
  - `hook.rs`: `HookCommand` + `CliHookName`（`clap::ValueEnum` は CLI 層のみ、DIP）
  - stdin JSON パース → `HookEnvelope`（CLI層 serde 型）→ `HookInput`（domain 型）に変換 → `HookHandler::handle` → stdout + exit code
  - stdout 形式は hook ごとに異なる（既存 Python パターン準拠）:
    - guard (block-direct-git-ops): plain text reason + exit 2、または空 + exit 0
    - lock (file-lock-acquire): block JSON `{"hookSpecificOutput": {"decision": "block", "reason": "..."}}` + exit 2
    - lock (file-lock-release): PostToolUse JSON `{"hookSpecificOutput": {"hookEventName": "PostToolUse", ...}}` + exit 0
  - PreToolUse エラー時: guard は plain text + exit 2、lock-acquire は block JSON + exit 2
  - PostToolUse エラー時 (lock-release): stderr に warning + exit 0（PostToolUse は block 不可）
- **Python hooks** (`.claude/hooks/`):
  - `block-direct-git-ops.py`: `sotp hook dispatch block-direct-git-ops` の薄いランチャーに
  - `file-lock-acquire.py` / `file-lock-release.py`: 同様にランチャー化
  - 残り11フック: `_shared.py` の warn-and-log パターンに変換

### Out of scope

- アドバイザリーフックの Rust 移行（将来対応）
- `TrackStore` 実装（filelock-migration トラックの責務）

## Constraints

- Rust edition 2024, MSRV 1.85
- Domain 層に `clap` / `serde` / `serde_json` への直接依存を持ち込まない（DIP）
- `HookEnvelope`（serde 型）は CLI/infrastructure 層に配置。Domain は `HookInput`（framework-free）のみ定義
- セキュリティフィールド `tool_name` に `#[serde(default)]` を使わない — パース失敗 → PreToolUse は exit 2（fail-closed）、PostToolUse は warn + exit 0
- PreToolUse hooks（guard, lock-acquire）: Rust 側の全エラーパスは exit 2（block）で終了（fail-closed by design）
- PostToolUse hooks（lock-release）: Rust 側のエラーは stderr warning + exit 0（PostToolUse は block 不可）
- PreToolUse Python ランチャーはフォールバック無し — CLI 未検出/クラッシュ/タイムアウト時は `os._exit(2)` で block
- PostToolUse Python ランチャーは CLI 未検出/クラッシュ/タイムアウト時も `os._exit(0)` + stderr warning（block 不可）
- Python ランチャーは `except BaseException` + `os._exit()` パターン（`KeyboardInterrupt` / `SystemExit` / `TimeoutExpired` 対応）
- Python ランチャーの subprocess に `timeout=10` を設定（ハング防止、現行 `file-lock-acquire.py` パターン準拠）
- stdout/stderr を `os._exit()` 前に明示的に flush
- 各 `HookHandler` は hook 固有の必須フィールドを検証（`command` for guard, `file_path` for lock）→ 欠損時 `HookError::Input` → PreToolUse は exit 2、PostToolUse は warn + exit 0
- `HookCommand::Dispatch` は `--locks-dir`, `--agent`, `--pid` CLI args を持つ（lock hooks 用）。`--locks-dir` と `--agent` は env var からも取得可能。`--pid` は CLI arg のみ（env var 不可 — 安全なデフォルトが存在しないため明示的に渡す）
- lock-acquire の `--pid` と `--agent` は Python launcher が明示的に渡す（sotp 内にはどちらも安全なデフォルトが存在しない: Python launcher → sotp の親子関係のため sotp の `getppid()` は Python PID を返す）
- lock-release は `--agent` のみ必須（`FileLockManager::release` は `path` + `agent` で動作、`pid` 不要）。`--pid` が未設定でも release を実行する
- `--locks-dir` のデフォルトは `$CLAUDE_PROJECT_DIR/.locks`。`--locks-dir` / `$SOTP_LOCKS_DIR` / `$CLAUDE_PROJECT_DIR` のいずれも未設定 → exit 2（fail-closed、cwd フォールバック禁止）
- `file-lock-release` (PostToolUse) のエラーは block 不可 — warn + exit 0（PostToolUse はツール実行後に発火するため）
- lock hooks の `FileGuard` は `mem::forget` で drop 防止（PostToolUse で明示的に release）

## Acceptance Criteria

1. `sotp hook dispatch block-direct-git-ops` が stdin JSON を受けて正しく allow/block 判定
2. PreToolUse hooks: Rust 側の内部エラー（パース失敗等）は全て block (exit 2) で終了
2b. PostToolUse hooks (lock-release): Rust 側の内部エラーは stderr warning + exit 0（block 不可）
3. Python `block-direct-git-ops.py` が `sotp hook dispatch` に委譲（フォールバック無し、fail-closed）
4. PreToolUse Python ランチャーが `except BaseException` + `os._exit(2)` を使用し、全例外で block
4b. PostToolUse Python ランチャーが `except BaseException` + `os._exit(0)` + stderr warning（block 不可）
5. hook 固有の必須フィールド欠損（`command` / `file_path`）が `HookError::Input` → PreToolUse は exit 2、PostToolUse は warn + exit 0
6. CLI の hook stdout 出力が既存 Python hooks のパターンに準拠（guard: plain text, lock: hookSpecificOutput JSON）
7. アドバイザリーフックの例外時に警告メッセージが出力される
8. 既存のフック selftest + 新規テストが全パス
9. `cargo make ci` が全チェック通過
10. `domain::guard::verdict::GuardVerdict` が共有 `Decision` を使用

## Resolves

- TODO ERR-06: Fail-open 例外処理によるガードレール喪失

## Related Conventions (Required Reading)

- `project-docs/conventions/security.md`
