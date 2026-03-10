# Testing Rules (Rust)

## Core Principles

- **TDD推奨**: テストを先に書く（Red → Green → Refactor）
- **カバレッジ目標**: 新規コード 80% 以上
- **テスト速度**: ユニットテストは高速（< 50ms/test）

## Test Structure

### モジュール内テスト（Unit Tests）

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_with_valid_format_succeeds() {
        let result = Email::new("alice@example.com");
        assert!(result.is_ok());
    }

    #[test]
    fn test_email_with_missing_at_sign_returns_error() {
        let result = Email::new("not-an-email");
        assert!(matches!(result, Err(DomainError::InvalidEmail)));
    }
}
```

### 統合テスト（tests/ ディレクトリ）

```rust
// tests/user_integration.rs
#[test]
fn test_register_user_flow() {
    let repo = MockUserRepository::new();
    let use_case = RegisterUserUseCase::new(Arc::new(repo));
    let result = use_case.execute(RegisterUserCommand {
        email: "alice@example.com".to_string(),
    });
    assert!(result.is_ok());
}
```

> **Note**: `track/tech-stack.md` で async runtime を採用した場合は `#[tokio::test]` + `async fn` に切り替える。

### Naming Convention

```
test_{target}_{condition}_{expected_result}
例:
- test_email_with_valid_format_succeeds
- test_email_with_missing_at_sign_returns_invalid_email_error
```

## Mocking

`mockall` クレートを使う：

```rust
use mockall::automock;

#[automock]
pub trait UserRepository: Send + Sync {
    fn find_by_email(&self, email: &Email) -> Result<Option<User>, DomainError>;
}
```

> **Note**: `track/tech-stack.md` で async runtime を採用した場合は native `async fn in trait`
> （Rust 1.75+）または `async-trait` クレートと組み合わせる。
> `mockall` は `#[automock]` + `#[async_trait]` の順で属性を付与する。

## Commands

```bash
cargo make test                 # 標準の全体テスト
cargo make test-one-exec test_name  # 単一テストの高速確認
cargo make test-doc             # ドキュメントテスト（必要時のみ）
cargo make llvm-cov             # カバレッジ（HTML レポート）
cargo make test-nocapture       # テスト出力表示（必要時のみ）
```

## Checklist

- [ ] ハッピーパスのテストがある
- [ ] エラーケース（Err variant）のテストがある
- [ ] テストは独立している（実行順序に依存しない）
- [ ] 外部依存（DB, API）はモックされている
- [ ] `unwrap()` はテスト内でのみ使用
- [ ] テスト名が意図を明確に説明している
