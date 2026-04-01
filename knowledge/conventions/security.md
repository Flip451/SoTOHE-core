# Security Convention

## Sensitive Directories

This project defines two project-specific sensitive directories. Files in these directories must
not be committed to version control and must not be read by Claude Code.

> **Scope of enforcement**: The `Read` / `Grep` deny rules in `.claude/settings.json` apply only
> to Claude Code's own tool calls. They do **not** apply inside a Codex subprocess
> (`workspace-write` sandbox) or when Gemini CLI accesses the filesystem directly — see
> `.claude/rules/02-codex-delegation.md` for details. When using Codex with `workspace-write`,
> instruct it explicitly not to read files under `private/` or `config/secrets/`.
>
> **Container-level enforcement**: Docker Compose services enforce these rules at OS level:
> - `.git` is mounted read-only (`:ro`), preventing `git add/commit` from containers (EROFS).
>   Note: `git push` may still succeed as it primarily reads `.git`; network-level controls
>   or hook-based blocking should be used if push prevention is required.
> - `private/` and `config/secrets/` are masked by empty tmpfs overlays, making them appear empty
>   inside containers regardless of host contents
>
> This covers Codex `workspace-write` subprocesses and `cargo make shell` sessions that bypass
> Claude Code hooks. See `compose.yml` and `compose.dev.yml` for the mount configuration.

### `private/`

Purpose: Local certificates, TLS credentials, SSH keys, and other host-specific secrets that never
leave the developer's machine.

- **Git**: Must not be committed. Add `private/` to `.gitignore`.
- **AI read**: Prohibited. `Read(./private/**)` and `Grep(./private/**)` are in
  `.claude/settings.json` deny.
- **Typical contents**: `dev-cert.crt`, `dev-key.pem`, host-specific config with embedded credentials.

### `config/secrets/`

Purpose: Application-level secrets for local development (OAuth client IDs, API keys, database
passwords, and other credential files).

- **Git**: Must not be committed. Add `config/secrets/` to `.gitignore`.
- **AI read**: Prohibited. `Read(./config/secrets/**)` and `Grep(./config/secrets/**)` are in
  `.claude/settings.json` deny.
- **Typical contents**: `local.toml`, `oauth/client.json`, environment-specific credential files.

## Symlink Rejection in Infrastructure Adapters

Infrastructure 層のファイル I/O アダプターは、対象ファイルとその親ディレクトリの symlink を事前に拒絶する。

### ルール

| 対象 | チェック |
|---|---|
| 読み書き対象ファイル（leaf） | `symlink_metadata()` で symlink なら fail-closed エラー |
| 親ディレクトリ（track dir 等） | `symlink_metadata()` で symlink なら fail-closed エラー |
| root ディレクトリ | CLI composition root から渡されるため信頼する |

### 理由

- symlink 経由のファイル差し替えにより、review state や metadata が外部パスに redirect される可能性がある
- `std::fs::read_to_string` / `atomic_write_file` は symlink を透過的に follow する
- tamper-proof 対策として、ファイルアクセス前に symlink を検出して拒絶する

### 適用例

- `FsReviewJsonStore`: `reject_symlink()` を read/write の前に呼び出し
- `review_adapters.rs`: `open_regular_file_nofollow()` で no-follow open

### 新規アダプター追加時

1. ファイル I/O の前に `symlink_metadata()` で symlink チェックを追加する
2. symlink の場合は fail-closed でエラーを返す（silent skip 禁止）
3. テストで symlink 拒絶を検証する（プラットフォーム対応に注意）

## Enforcement

When adding a new sensitive directory to this project:

1. Add the directory to `.gitignore`.
2. Add `Read(./new-dir/**)` and `Grep(./new-dir/**)` deny rules to `.claude/settings.json`.
3. Add corresponding entries to `EXPECTED_DENY` in `scripts/verify_orchestra_guardrails.py` so the
   verifier requires the rules in CI.
4. Add regression tests in `scripts/test_verify_scripts.py` to guard against accidental removal.
5. Document the directory purpose in this file.
