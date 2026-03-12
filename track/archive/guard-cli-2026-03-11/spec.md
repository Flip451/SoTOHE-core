# Spec: Shell Command Guard CLI

## Goal

Python フック (`block-direct-git-ops.py`) の脆弱な regex/shlex コマンドパースを、決定論的な Rust ベースのシェルパースに置き換える。`sotp guard check` CLI サブコマンドとして実装し、Python フックはこれに委譲する薄いラッパーとなる。

## Scope

### In scope

- **Domain layer** (`libs/domain/src/guard/`):
  - `verdict.rs`: `Decision`（Allow/Block）、`GuardVerdict`、`ParseError` 型
  - `parser.rs`: conch-parser アダプタ（POSIX シェル AST パース）
    - conch-parser (vendored, patched) による構造的 AST 解析
    - パイプライン、リスト、サブシェル、コマンド置換の再帰的走査
    - AST → `SimpleCommand { argv }` への平坦化
    - ネスト深度制限（16段）
  - `policy.rs`: ガードポリシー（ブロック判定ロジック）
    - `VAR=val` 環境変数プレフィックススキップ
    - コマンドランチャースキップ（`nohup`, `nice`, `timeout`, `stdbuf`, `setsid` 等）
    - `env` コマンド一律ブロック
    - `$VAR`/`$(cmd)`/`` `cmd` `` が任意の位置（argv + redirect テキスト）に含まれていれば一律ブロック
    - git グローバルオプションスキップ（`-C`, `-c`, `--git-dir` 等）
    - 保護対象 git サブコマンド検出（`add`, `commit`, `push`）
    - `git branch -d/-D/--delete` 検出
    - argv レベル git 参照検出: 実効コマンドが `git` 以外で argv に "git" を含む場合は一律ブロック（shell -c, python -c, find -exec, xargs 等のネスト解析を単一チェックで代替）
- **CLI layer** (`apps/cli/src/commands/guard.rs`):
  - `guard check --command "..."` サブコマンド
  - JSON 出力: `{"decision": "allow"|"block", "reason": "..."}`
  - Exit code: 0 = allow, 1 = block（パースエラーも fail-closed で block 扱い）
- **Hook migration** (`.claude/hooks/block-direct-git-ops.py`):
  - ~907行 → ~30行の CLI 委譲ラッパーに書き換え
  - CLI バイナリ未検出時のフォールバック（既存 Python ロジック維持）

### Out of scope

- `log-cli-tools.py` のコマンド検出ロジック移行（将来対応）
- usecase / infrastructure 層の関与（純粋計算のため不要）

## Constraints

- Rust edition 2024, MSRV 1.85
- domain 層の外部依存は `thiserror` + `conch-parser`（vendored, patched）
- レイヤー依存ルール（`deny.toml`, `check_layers.py`）を違反しないこと
- パース失敗時は fail-closed（ブロック）
- ネスト深度制限: 16段（DoS 防止）
- **エッジケース排除方針**: テンプレートワークフローで不要なパターンは一律ブロック
  - `env` コマンド: チェーン・`-S` 再分割・オプション解析の曖昧性によるバイパスベクタを排除。`VAR=val command` シェル構文で代替可能
  - `$VAR` / `$(cmd)` / `` `cmd` `` が argv または redirect テキストの **いずれかの位置** に含まれていれば一律ブロック（コマンド位置に限定しない）
  - `.exe` サフィックス: `basename` で除去して検出（Linux/WSL 環境で不要）
  - **argv レベル git 参照検出**: 実効コマンドが `git` 以外の場合、argv 全トークンを走査し "git" を含むトークンがあれば一律ブロック。これにより `shell -c`, `python -c`, `find -exec`, `xargs` 等の個別ネスト解析（~200行）を単一の argv チェックで代替。"digit"/"legit" 等の偽陽性は許容（テンプレートワークフローでは git 参照をラッパー経由で渡す必要がない）
- **既知の限界**:
  - シェル AST 文字列レベルでの検出のため、Python インタプリタ内部での変数間接参照等は検出不可（ただし `$VAR` / `$(cmd)` / `` `cmd` `` はトークン全位置でブロック済み）
  - `git --help add` 等のヘルプ専用呼び出しも `git add` と同等にブロックされる。`--help` は git のトップレベルオプションとしてスキップされるため。テンプレートワークフローでは問題にならず、fail-closed ポリシーに合致するため許容
  - Heredoc 経由のインタプリタ呼び出し（`bash <<'SH'\ngit add .\nSH`）は `SimpleCommand.redirect_texts` を通じて検出可能（heredoc 本文を redirect テキストとして抽出しスキャン対象に含めている）

## Acceptance Criteria

1. `cargo make ci` が全チェック通過
2. `sotp guard check --command "git add ."` → block + 理由メッセージ
3. `sotp guard check --command "git status"` → allow
4. `sotp guard check --command "env VAR=val nohup git commit -m msg"` → block
5. `sotp guard check --command "bash -c 'git push origin main'"` → block
6. `sotp guard check --command '$CMD add'` → block（変数置換バイパス）
7. `sotp guard check --command "git branch -D feature"` → block
8. `sotp guard check --command "cargo make test"` → allow
9. `sotp guard check --command "find . -exec git add {} \\;"` → block
10. Python フックが CLI 委譲方式で動作し、既存の保護が維持される

## Resolves

- harness-issues-analysis.md 1.1: bashlex optional dependency
- harness-issues-analysis.md 1.4: KNOWN_SHELLS mismatch
- harness-issues-analysis.md 1.5: Subshell anchor gap
- harness-issues-analysis.md 4.2: extract_command_token quote bugs
- harness-issues-analysis.md 3.1/3.3/3.4: Python venv dependency / startup latency
