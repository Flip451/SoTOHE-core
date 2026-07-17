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

### Usecase Layer Purity

usecase 層は純粋なオーケストレーターであり、実行環境へ直接到達しない。I/O と実行時依存は境界で受け取り、必要な外部機能は domain / usecase の port を通じて扱う。

| 禁止 | 正しい対処 |
|---|---|
| `std::fs::*` / `std::net::*` / `std::process::*` / `std::io::*` / `std::env::*` | CLI または infrastructure adapter が扱い、typed input / port 経由で渡す |
| `chrono::Utc::now()` / `std::time::SystemTime` / `std::time::Instant` | 時刻を usecase entrypoint の引数として受け取る |
| `println!` / `eprintln!` / `print!` / `eprint!` | `Result<T, E>` を返し、CLI が表示と exit code を担う |

`sotp verify usecase-purity` は syn AST により上記のパターンを検査する。現時点の強制強度は warning-only であり、error への昇格は採用者が ADR で判断する。async runtime の採用も ADR の決定事項である。

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

### Usecase Purity

```rust
// Good: external capability is supplied through a port.
pub fn load<R: SpecDocumentLoaderPort>(reader: &R, path: &Path) -> Result<SpecDocument, LoadError> {
    reader.load(path).map_err(LoadError::from)
}

// Bad: usecase reaches directly into the runtime and presents output.
let content = std::fs::read_to_string(path)?;
println!("loaded");
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
- [ ] usecase が I/O、暗黙的な時刻、環境、プロセス、出力を直接扱っていないか

## Decision Reference

- `knowledge/conventions/prefer-type-safe-abstractions.md`: 型安全パターン（Newtype / Enum-first / Typestate）
- `architecture-rules.json`: crate 間依存の機械可読 SSoT
- `knowledge/conventions/type-designer-kind-selection.md` R1: role × layer 配置の SSoT
- `knowledge/conventions/security.md`: シークレット管理・SQL インジェクション対策
