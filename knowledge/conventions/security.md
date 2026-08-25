# Security Convention

## Sensitive Directories

This project defines two project-specific sensitive directories. Files in these directories must
not be committed to version control and must not be read by Claude Code.

> **強制先**: review 観点 — harness-policy scope

> **Scope of enforcement**: The `Read` / `Grep` deny rules in `.claude/settings.json` apply only
> to Claude Code's own tool calls. They do **not** apply inside a Codex subprocess
> (`workspace-write` sandbox) or when Gemini CLI accesses the filesystem directly — see
> `.claude/rules/guardrails.md` §Sandbox and Hook Coverage Warning for details. When using Codex with `workspace-write`,
> instruct it explicitly not to read files under `private/` or `config/secrets/`.

> **強制先**: review 観点 — harness-policy scope
>
> **Optional container-level enforcement**: A project that selects the Docker environment can
> additionally use Docker Compose services to enforce these rules at OS level:
> - `.git` is mounted read-only (`:ro`), preventing `git add/commit` from containers (EROFS).
>   Note: `git push` may still succeed as it primarily reads `.git`; network-level controls
>   or hook-based blocking should be used if push prevention is required.
> - `private/` and `config/secrets/` are masked by empty tmpfs overlays, making them appear empty
>   inside containers regardless of host contents

> **強制先**: 強制なし (明記) — Docker 環境の選択は consumer 所有
>
> This container isolation does not cover host-runner commands or Codex `workspace-write`
> subprocesses. Those paths must follow the permission and guarded-workflow rules above.

> **強制先**: review 観点 — harness-policy scope

### `private/`

Purpose: Local certificates, TLS credentials, SSH keys, and other host-specific secrets that never
leave the developer's machine.

- **Git**: Must not be committed. Add `private/` to `.gitignore`.
  > **強制先**: review 観点 — harness-policy scope
- **AI read**: Prohibited. `Read(./private/**)` and `Grep(./private/**)` are in
  `.claude/settings.json` deny.
  > **強制先**: review 観点 — harness-policy scope
- **Typical contents**: `dev-cert.crt`, `dev-key.pem`, host-specific config with embedded credentials.

### `config/secrets/`

Purpose: Application-level secrets for local development (OAuth client IDs, API keys, database
passwords, and other credential files).

- **Git**: Must not be committed. Add `config/secrets/` to `.gitignore`.
  > **強制先**: review 観点 — harness-policy scope
- **AI read**: Prohibited. `Read(./config/secrets/**)` and `Grep(./config/secrets/**)` are in
  `.claude/settings.json` deny.
  > **強制先**: review 観点 — harness-policy scope
- **Typical contents**: `local.toml`, `oauth/client.json`, environment-specific credential files.

## Symlink Rejection in Infrastructure Adapters

Infrastructure 層のファイル I/O アダプターは、対象ファイルとその親ディレクトリの symlink を事前に拒絶する。

### ルール

| 対象 | チェック |
|---|---|
| 読み書き対象ファイル（leaf） | `symlink_metadata()` で symlink なら fail-closed エラー |
| 親ディレクトリ（track dir 等） | `symlink_metadata()` で symlink なら fail-closed エラー |
| root ディレクトリ | CLI composition root から渡されるため信頼する |

> **強制先**: review 観点 — infrastructure / cli_composition scope

### 理由

- symlink 経由のファイル差し替えにより、review state や metadata が外部パスに redirect される可能性がある
- `std::fs::read_to_string` / `atomic_write_file` は symlink を透過的に follow する
- tamper-proof 対策として、ファイルアクセス前に symlink を検出して拒絶する

### 適用例

- `FsReviewStore` (review_v2): `reject_symlinks_below()` + `WriteGuard` で read/write の前に symlink / 外部書き込みを拒絶

### 新規アダプター追加時

1. ファイル I/O の前に `symlink_metadata()` で symlink チェックを追加する
2. symlink の場合は fail-closed でエラーを返す（silent skip 禁止）
3. テストで symlink 拒絶を検証する（プラットフォーム対応に注意）

> **強制先**: review 観点 — infrastructure scope

## Security Boundary Failure Handling

秘匿・入力検証・権限判定のセキュリティ境界では、エラー時の無音の機能縮退を許さない。
構築または初期化に失敗した場合は、警告のみの通知、無効値への縮退、保護なしでの処理継続ではなく、
処理を停止してエラーを返すか fail-stop とする。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition / harness-policy scope

静的な秘匿パターンなど、構築がプログラミングエラーを示す場合も、同じ構築保証を適用する。
外部入力を使う動的な値は、検証に失敗した時点でエラーとして伝播させる。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition / harness-policy scope

## Enforcement

When adding a new sensitive directory to this project:

1. Add the directory to `.gitignore`.
2. Add `Read(./new-dir/**)` and `Grep(./new-dir/**)` deny rules to `.claude/settings.json`.
3. Document the directory purpose in this file.

> **強制先**: review 観点 — harness-policy scope

> **Consumer's responsibility**: CI enforcement of sensitive-directory deny rules (verifying that
> `.claude/settings.json` contains the expected `Read`/`Grep` deny entries) is the **consumer's
> responsibility**, not SoTOHE's. SoTOHE ships recommended deny entries as defaults and documents
> the intent here, but does not hard-fail CI against them. See
> `.harness/policies/consumer-ownership.md` for the provide-not-enforce principle.

> **強制先**: 強制なし (明記) — sensitive-directory CI は consumer 所有

## Secrets Management

```rust
// Bad: hardcoded secrets
let api_key = "sk-1234abcd";

// Good: from environment — propagate error instead of panicking
// Implement From<std::env::VarError> for ConfigError to preserve the error kind.
fn init_config() -> Result<Config, ConfigError> {
    let api_key = std::env::var("API_KEY")?; // ? uses From<VarError> for ConfigError
    Ok(Config { api_key })
}
```

`.env` はコミットしない。`.env.example` のみコミットする。

> **強制先**: review 観点 — harness-policy scope

## Input Validation

ドメイン型のコンストラクタで検証する：

> **強制先**: review 観点 — domain scope

```rust
pub struct Email(String);
impl Email {
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        if !is_valid_email(s) { return Err(ValidationError::InvalidEmail); }
        Ok(Self(s.to_string()))
    }
}
```

## SQL Injection Prevention

SQLx のパラメータバインドを必ず使う：

> **強制先**: review 観点 — infrastructure scope

```rust
// Bad
let query = format!("SELECT * FROM users WHERE id = {id}");

// Good
let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
    .bind(user_id)
    .fetch_optional(&pool)
    .await?;
```

## Error Messages

内部詳細をユーザーに漏らさない：

> **強制先**: review 観点 — infrastructure scope

```rust
// Bad: leaks internal info
Err(AppError::Database(format!("Connection to {}:{} failed", host, port)))

// Good: abstract to user, log details internally
tracing::error!("DB connection failed: host={} err={}", host, err);
Err(AppError::Internal("Service unavailable".to_string()))
```

## Dependencies

```bash
cargo make deny      # 脆弱性・ライセンス・禁止クレートチェック
```

> **強制先**: 機械 lint — cargo make deny

## Code Review Checklist

- [ ] シークレットのハードコードなし
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition / harness-policy scope
- [ ] 外部入力はドメイン型で検証済み
  > **強制先**: review 観点 — domain scope
- [ ] SQL クエリはパラメータバインド使用
  > **強制先**: review 観点 — infrastructure scope
- [ ] エラーメッセージは内部情報を漏らさない
  > **強制先**: review 観点 — infrastructure scope
- [ ] ログに機密情報が含まれていない
  > **強制先**: review 観点 — infrastructure scope
- [ ] セキュリティ境界（秘匿・検証・権限判定）で無音の機能縮退がなく、構築・初期化の失敗が停止として扱われている
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition / harness-policy scope
- [ ] `unsafe` コードは最小限かつコメント付き
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition / harness-policy scope
- [ ] `cargo make deny` が通っている
  > **強制先**: 機械 lint — cargo make deny
