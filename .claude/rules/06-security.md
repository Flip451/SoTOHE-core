# Security Rules (Rust)

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

## Input Validation

ドメイン型のコンストラクタで検証する：

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

```rust
// Bad: leaks internal info
Err(AppError::Database(format!("Connection to {}:{} failed", host, port)))

// Good: abstract to user, log details internally
tracing::error!("DB connection failed: host={} err={}", host, err);
Err(AppError::Internal("Service unavailable".to_string()))
```

## Dependencies

```bash
cargo audit          # セキュリティ脆弱性チェック
cargo make deny      # ライセンス・禁止クレートチェック
```

## Code Review Checklist

- [ ] シークレットのハードコードなし
- [ ] 外部入力はドメイン型で検証済み
- [ ] SQL クエリはパラメータバインド使用
- [ ] エラーメッセージは内部情報を漏らさない
- [ ] ログに機密情報が含まれていない
- [ ] `unsafe` コードは最小限かつコメント付き
- [ ] `cargo make deny` が通っている
