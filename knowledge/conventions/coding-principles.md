# Coding Principles Convention

## Purpose

Rust コードベース全体に適用する実装規約。エラーハンドリング・命名規則・モジュールサイズ・ドキュメント・パニック禁止・`unsafe` の扱いを定める。

## Scope

- Applies to: `libs/`, `apps/` 配下の全 Rust プロダクションコード
- Does not apply to: `#[cfg(test)]` ブロック、`tests/` 統合テスト（パニック禁止ルールとモジュールサイズ上限のみ適用外）

---

## Rules

### Error Handling: Result and ? Operator

`unwrap()` は本番コード禁止（テスト内のみ可）。`?` 演算子で伝搬し、境界では適切な `From` 変換を実装する。

### Naming Conventions

| 対象 | スタイル | 例 |
|---|---|---|
| Types / Traits | `PascalCase` | `UserRepository`, `RegisterUserCommand` |
| Functions / Methods | `snake_case` | `find_by_email`, `register_user` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_RETRY_COUNT` |
| Modules / Crates | `snake_case` | `user_domain`, `postgres_adapter` |
| Lifetimes | `'a` または意味のある名前 | `'input` |

### Module Size

- 1 モジュールに 1 つの責務。
- 目安: 200–400 行（最大 700 行）。
- 行数の目安・上限は**プロダクションコードのみ**が対象。テストコード（`#[cfg(test)] mod tests` ブロック、`*_tests.rs` 等のテスト専用ファイル、`tests/` 統合テスト）はファイルサイズ判定の対象外。関連テストは 1 ファイルにまとめてよい。

### Documentation

公開 API には `///` コメントを書く。`# Errors` セクションは必須。

### No Panics in Library Code

`#[cfg(test)]` 以外のコードでパニックを起こしうる構文は**禁止**。

| 禁止パターン | 安全な代替 |
|---|---|
| `slice[i]` / `str[range]` | `.get(i)` / `.get(range)` |
| `.unwrap()` | `?` / `.unwrap_or()` / `if let` |
| `.expect("...")` | `?` / `.unwrap_or()` / `if let` |
| `assert!()` / `assert_eq!()` | `if !cond { return Err(...) }` |
| `panic!()` / `unreachable!()` | `return Err(...)` |
| `todo!()` / `unimplemented!()` | コンパイルエラーにするか `return Err(...)` |

### Unsafe Code

`unsafe` は最小限かつ Safety コメント必須。使用前に `reviewer` capability のレビューを受けること。

---

## Examples

### Error Propagation

```rust
// Bad: panics in production
let user = find_user(id).unwrap();

// Good: ? operator
pub fn find_user(&self, id: UserId) -> Result<User, AppError> {
    let user = self.repo.find_by_id(id)?;
    Ok(user)
}
```

### Public API Documentation

```rust
/// Creates a new user.
///
/// # Errors
/// Returns `DomainError::InvalidEmail` if the email format is invalid.
pub fn new(email: &str) -> Result<User, DomainError> { ... }
```

### Panic-Free Access

```rust
// Bad: panics on multi-byte UTF-8 or out-of-range
let suffix = &name[name.len() - 4..];

// Good: safe byte-level check
if name.as_bytes().get(name.len().wrapping_sub(4)..).map_or(false, |b| b.eq_ignore_ascii_case(b".exe")) {
    // strip .exe
}

// Bad: panics
pub fn divide(a: i32, b: i32) -> i32 { a / b }

// Good: Result
pub fn divide(a: i32, b: i32) -> Result<i32, MathError> {
    if b == 0 { return Err(MathError::DivisionByZero); }
    Ok(a / b)
}
```

### Unsafe Justification

```rust
// Safety: ptr was created by Box::into_raw and has not been freed.
unsafe { Box::from_raw(ptr) }
```

---

## Exceptions

- テストコード（`#[cfg(test)]`）では `unwrap()` / `expect()` / `assert!()` を使ってよい。
- モジュールサイズ上限（700 行）はテスト専用ファイルには適用しない。

## Review Checklist

- [ ] 本番コードに `unwrap()` / `expect()` / `panic!()` / `todo!()` / `unreachable!()` がないか
- [ ] インデックスアクセス `slice[i]` / `str[range]` が `.get()` に置き換えられているか
- [ ] 公開 API に `///` コメントと `# Errors` セクションがあるか
- [ ] モジュールが 700 行以内か（プロダクションコードのみ）
- [ ] 命名が PascalCase / snake_case 規則に従っているか
- [ ] `unsafe` ブロックに Safety コメントがあるか

## Decision Reference

- `knowledge/conventions/prefer-type-safe-abstractions.md`: 型安全パターン（Newtype / Enum-first / Typestate）
- `knowledge/conventions/hexagonal-architecture.md`: ヘキサゴナルアーキテクチャ（Trait-Based Abstraction を含む）
- `knowledge/conventions/security.md`: シークレット管理・SQL インジェクション対策
