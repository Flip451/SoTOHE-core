# Testing Convention

## Purpose

Rust コードベース全体に適用するテスト規約。TDD サイクル・テスト構造・命名規則・モック・実行コマンドを定める。

## Scope

- Applies to: `libs/`, `apps/` 配下の全 Rust コード（プロダクションコードおよびテストコード）
- Does not apply to: `knowledge/`, `track/`, `.harness/` など非 Rust ドキュメント

---

## Rules

### TDD Cycle

テストを先に書く（Red → Green → Refactor）。実装コードを書く前に失敗するテストを用意する。

### Coverage Goal

新規コードのカバレッジ目標は 80% 以上。

### Test Speed

ユニットテストは高速に保つ（目安: 1 テスト 50ms 未満）。

### Test Structure: Unit Tests

モジュール内に `#[cfg(test)] mod tests { ... }` ブロックを置く。

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

### Test Structure: Integration Tests

`tests/` ディレクトリに配置する。

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

テスト関数名は `test_{target}_{condition}_{expected_result}` の形式にする。

```
test_email_with_valid_format_succeeds
test_email_with_missing_at_sign_returns_invalid_email_error
```

### Mocking

外部依存（リポジトリ、外部 API など）のモックには `mockall` クレートを使う。

```rust
use mockall::automock;

#[automock]
pub trait UserRepository: Send + Sync {
    fn find_by_email(&self, email: &Email) -> Result<Option<User>, DomainError>;
}
```

> **Note**: `track/tech-stack.md` で async runtime を採用した場合は native `async fn in trait`（Rust 1.75+）または `async-trait` クレートと組み合わせる。`mockall` は `#[automock]` + `#[async_trait]` の順で属性を付与する。

---

## Commands

```bash
cargo make test                 # 標準の全体テスト
cargo make test-nocapture       # テスト出力（stdout/stderr）を表示しながら実行
cargo make test-doc             # ドキュメントテスト（必要時のみ）
cargo make llvm-cov             # カバレッジ（HTML レポート）
```

特定のテストだけを実行したい場合は、コンテナシェル内で nextest のフィルタ構文を使う。

```bash
cargo make shell
# コンテナ内で:
cargo nextest run -E 'test(test_email_with_valid_format)'
```

---

## Exceptions

- テストコード（`#[cfg(test)]`）では `unwrap()` / `expect()` / `assert!()` を使ってよい。
- モジュールサイズ上限（700 行）はテスト専用ファイルには適用しない（`coding-principles.md` §Module Size 参照）。

## Review Checklist

- [ ] ハッピーパスのテストがある
- [ ] エラーケース（`Err` variant）のテストがある
- [ ] テストは独立している（実行順序に依存しない）
- [ ] 外部依存（DB, API）はモックされている
- [ ] `unwrap()` はテスト内でのみ使用
- [ ] テスト名が `test_{target}_{condition}_{expected_result}` 形式に従っている

## Decision Reference

- `knowledge/conventions/coding-principles.md`: エラーハンドリング・パニック禁止ルール（テスト例外を含む）
- `knowledge/conventions/hexagonal-architecture.md`: ポート（Trait）定義と mockall を使ったアダプタのテスト戦略
