# Coding Principles (Rust)

## Make Illegal States Unrepresentable

型システムで不正な状態を表現不可能にする：

```rust
// Bad: 空文字を許す
struct User { email: Option<String> }

// Good: 検証済み型
pub struct Email(String);
impl Email {
    pub fn new(s: impl Into<String>) -> Result<Self, DomainError> {
        let s = s.into();
        if s.contains('@') { Ok(Self(s)) } else { Err(DomainError::InvalidEmail) }
    }
}
```

## Error Handling: Result and ? Operator

`unwrap()` は本番コード禁止（テスト内のみ可）：

```rust
// Bad: panics in production
let user = find_user(id).unwrap();

// Good: ? operator
pub fn find_user(&self, id: UserId) -> Result<User, AppError> {
    let user = self.repo.find_by_id(id)?;
    Ok(user)
}
```

## Trait-Based Abstraction (Hexagonal Architecture)

インフラ依存を Trait で分離する：

```rust
// Port (domain layer) — sync baseline
pub trait UserRepository: Send + Sync {
    fn save(&self, user: &User) -> Result<(), DomainError>;
}

// Adapter (infrastructure layer)
pub struct PostgresUserRepository { pool: PgPool }
impl UserRepository for PostgresUserRepository {
    fn save(&self, user: &User) -> Result<(), DomainError> { ... }
}
```

> **Note**: async runtime（tokio 等）を `track/tech-stack.md` で採用した場合は、
> native `async fn in trait`（Rust 1.75+）を優先する。`async-trait` クレートは
> dyn dispatch（object safety）や明示的な future auto-trait bounds が必要な場合に使用する。

## Naming Conventions

- **Types/Traits**: `PascalCase`
- **Functions/Methods**: `snake_case`
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Modules/Crates**: `snake_case`
- **Lifetimes**: `'a` or meaningful `'input`

## Module Size

- 1モジュールに1つの責務
- 目安: 200–400行（最大700行）

## Documentation

公開 API には `///` コメントを書く（`# Errors` セクション必須）：

```rust
/// Creates a new user.
///
/// # Errors
/// Returns `DomainError::InvalidEmail` if the email format is invalid.
pub fn new(email: &str) -> Result<User, DomainError> { ... }
```

## No Panics in Library Code

```rust
// Bad: panics
pub fn divide(a: i32, b: i32) -> i32 { a / b }

// Good: Result
pub fn divide(a: i32, b: i32) -> Result<i32, MathError> {
    if b == 0 { return Err(MathError::DivisionByZero); }
    Ok(a / b)
}
```

## Unsafe Code

`unsafe` は最小限かつコメント必須：

```rust
// Safety: ptr was created by Box::into_raw and has not been freed.
unsafe { Box::from_raw(ptr) }
```

`unsafe` の使用前に Codex にレビューを依頼すること。
