# Phase 1 Diff Plan: Security-Critical Hooks -> Rust Direct Invocation

作成日: 2026-03-13
対象: `STRAT-03` Phase 1

## スコープ

- `.claude/hooks/block-direct-git-ops.py`
- `.claude/hooks/file-lock-acquire.py`
- `.claude/hooks/file-lock-release.py`
- `.claude/settings.json`
- `apps/cli/src/commands/hook.rs`
- bootstrap / CI / selftest

## ゴール

security-critical hook 3本を Python launcher 経由ではなく Rust バイナリ `sotp` の直接呼び出しへ切り替える。

完了条件:

- `.venv` や `python3` が存在しなくても、対象 hook が fail-closed / warn+exit0 の契約どおり動く
- `.claude/settings.json` に対象 3 本の `python3 ...hooks/*.py` が残らない
- Python launcher は削除または非接続化され、必須経路から外れる

## 現状

### 現在の呼び出し経路

1. Claude Code hook event
2. `.claude/settings.json` が `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/*.py"` を実行
3. Python launcher が stdin JSON を受け取る
4. launcher が `sotp hook dispatch ...` を subprocess 実行
5. Rust が実処理

### 問題

- `python3` が前提
- `.venv` / PATH / ホスト環境差分の影響を受ける
- launcher が存在する限り、必須経路の Python 依存が消えない

## 変更方針

### 1. `.claude/settings.json` の command を Rust バイナリ直接呼び出しへ変更

変更後イメージ:

```json
"command": "\"${SOTP_CLI_BINARY:-sotp}\" hook dispatch block-direct-git-ops"
"command": "\"${SOTP_CLI_BINARY:-sotp}\" hook dispatch file-lock-acquire --agent \"${SOTP_AGENT_ID:-pid-$PPID}\" --pid \"$PPID\""
"command": "\"${SOTP_CLI_BINARY:-sotp}\" hook dispatch file-lock-release --agent \"${SOTP_AGENT_ID:-pid-$PPID}\""
```

注意:

- `command` は shell 展開される前提で `$PPID` と `${...:-...}` を使う
- `--locks-dir` は省略し、`hook.rs` の `CLAUDE_PROJECT_DIR/.locks` fallback を使う
- `SOTP_LOCKS_DIR` が設定されていれば clap env で優先される

### 2. `hook.rs` 側の契約は維持しつつ、launcher 依存の前提を Rust へ寄せる

必要な確認 / 追加:

- `block-direct-git-ops`
  - 現状の plain text stdout + exit 2 を維持する
- `file-lock-acquire`
  - 現状の block JSON `{"hookSpecificOutput": ...}` + exit 2 を維持する
- `file-lock-release`
  - 現状どおり warn to stderr + exit 0 を維持する

追加検討:

- `PPID` 非依存にするため、必要なら `--agent-from-parent-pid` / `--pid-from-parent` のような CLI shorthand を追加する
- `SOTP_CLI_BINARY` 未設定かつ `sotp` 未検出時の fail-closed 文言を明確にする

### 3. Python launcher の扱い

段階案:

- Step A: `.claude/settings.json` から参照を外す
- Step B: Python launcher は互換用に残すが deprecated 扱いにする
- Step C: hooks-selftest / docs / rules からの参照を消した時点で削除する

推奨:

- まず Step A+B
- 1トラック安定運用後に Step C

## 具体差分

### A. `.claude/settings.json`

変更対象:

- `PreToolUse` matcher=`Bash`
- `PreToolUse` matcher=`Edit|Write|Read`
- `PostToolUse` matcher=`Edit|Write|Read`

差分内容:

- `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/block-direct-git-ops.py"`
  - `-> "${SOTP_CLI_BINARY:-sotp}" hook dispatch block-direct-git-ops`
- `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/file-lock-acquire.py"`
  - `-> "${SOTP_CLI_BINARY:-sotp}" hook dispatch file-lock-acquire --agent "${SOTP_AGENT_ID:-pid-$PPID}" --pid "$PPID"`
- `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/file-lock-release.py"`
  - `-> "${SOTP_CLI_BINARY:-sotp}" hook dispatch file-lock-release --agent "${SOTP_AGENT_ID:-pid-$PPID}"`

### B. `apps/cli/src/commands/hook.rs`

確認済み:

- `locks_dir`: `--locks-dir` > `SOTP_LOCKS_DIR` > `CLAUDE_PROJECT_DIR/.locks`
- `agent`: `SOTP_AGENT_ID` または `--agent`
- `pid`: CLI 引数

必要差分候補:

- shell 直呼び前提の運用を docs に合わせてコメント更新
- `sotp` 未検出時のエラー文言をテストで固定
- 必要なら parent-pid shorthand を追加

### C. `.claude/hooks/*.py`

対象:

- `.claude/hooks/block-direct-git-ops.py`
- `.claude/hooks/file-lock-acquire.py`
- `.claude/hooks/file-lock-release.py`

差分方針:

- Step A 時点ではファイルは残すが deprecated コメントを先頭に追加
- Step C で削除

### D. `scripts/verify_orchestra_guardrails.py`

必要差分:

- hook path の存在前提チェックを見直す
- `settings.json` の command が `python3 ...hooks/*.py` であることを期待している箇所を `sotp hook dispatch ...` に置換
- `python3` allow/deny 前提の一部テスト期待値を更新

### E. テスト

更新対象:

- `.claude/hooks/test_policy_hooks.py`
- `.claude/hooks/test_post_tool_hooks.py`
- `scripts/test_verify_scripts.py`
- `scripts/test_make_wrappers.py`

新規に必要なテスト:

- `.claude/settings.json` が Rust direct command を指している
- `SOTP_CLI_BINARY` 未設定時に `sotp` を探す
- `SOTP_CLI_BINARY` を上書きできる
- `file-lock-acquire` が `$PPID` と `SOTP_AGENT_ID` で動く
- `CLAUDE_PROJECT_DIR` 未設定時:
  - guard は実行可能
  - lock-acquire は fail-closed
  - lock-release は warn + exit 0

## ロールアウト順

### Step 1

- `.claude/settings.json` を Rust direct command に変更
- verify script / tests を更新
- Python launcher は残す

### Step 2

- hooks-selftest を Rust direct command 前提に更新
- maintainer docs / rules / DESIGN.md の参照を更新

### Step 3

- Python launcher を deprecated 化
- 一定期間後に削除

## リスク

### 1. Hook command の shell 展開差異

リスク:

- Claude Code が `command` をどの shell でどう実行するかに依存して `$PPID` 展開が壊れる可能性

緩和:

- まず最小実験で `$PPID` が使えるか確認
- 使えない場合は `sotp hook dispatch ... --pid-from-parent` 形式の shorthand を追加する

### 2. `sotp` バイナリ配置

リスク:

- PATH に `sotp` が無い環境で即死する

緩和:

- bootstrap で `SOTP_CLI_BINARY` を設定
- あるいは repo-local binary path を `.claude/settings.json` に固定

### 3. hooks-selftest の前提崩れ

リスク:

- 既存テストは Python file の存在や挙動を前提にしている

緩和:

- 「Python launcher の単体テスト」から「hook command 契約テスト」へ移す

## 決めるべき事項

1. `.claude/settings.json` の direct command で `$PPID` を使うか
2. `sotp` バイナリは PATH 前提にするか、repo-local path を固定するか
3. Python launcher を即削除するか、1段階 deprecated にするか

## 推奨結論

- PID はまず `$PPID` で実装
- バイナリ解決は `${SOTP_CLI_BINARY:-sotp}` を使い、bootstrap で保証
- launcher は 1 段階 deprecated を挟んで削除
