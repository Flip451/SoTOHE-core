---
required_for:
  - spec-designer
  - impl-planner
---

# Coding Principles Convention

## Purpose

Rust コードベース全体に適用する実装規約。エラーハンドリング・命名規則・モジュールサイズ・ドキュメント・パニック禁止・`unsafe` の扱いを定める。

## Scope

- Applies to: `libs/`, `apps/` 配下の全 Rust プロダクションコード
- Does not apply to: `#[cfg(test)]` ブロック、`tests/` 統合テスト（パニック禁止ルールとモジュールサイズ上限のみ適用外）

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope

---

## Rules

### Error Handling: Result and ? Operator

`unwrap()` は本番コード禁止（テスト内のみ可）。

> **強制先**: 機械 lint — cargo make clippy

エラーは原則として `?` 演算子で伝搬し、境界では適切な `From` 変換を実装する。型付き変換や
エラーコンテキストの付与に明示的な `match` が必要な場合は、全分岐で `Result` を返し、エラーを
捨てない形にする。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope

### Naming Conventions

| 対象 | スタイル | 例 |
|---|---|---|
| Types / Traits | `PascalCase` | `UserRepository`, `RegisterUserCommand` |
| Functions / Methods | `snake_case` | `find_by_email`, `register_user` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_RETRY_COUNT` |
| Modules / Crates | `snake_case` | `user_domain`, `postgres_adapter` |
| Lifetimes | `'a` または意味のある名前 | `'input` |

> **強制先**: 機械 lint — cargo make clippy

### Module Size

- 1 モジュールに 1 つの責務。
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- 目安: 200–400 行（最大 700 行）。
- 行数の目安・上限は**プロダクションコードのみ**が対象。テストコード（`#[cfg(test)] mod tests` ブロック、`*_tests.rs` 等のテスト専用ファイル、`tests/` 統合テスト）はファイルサイズ判定の対象外。関連テストは 1 ファイルにまとめてよい。

> **強制先**: 機械 lint — bin/sotp verify module-size

### Documentation

公開 API には `///` コメントを書く。`# Errors` セクションは必須。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope

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

> **強制先**: 機械 lint — cargo make clippy

`assert!()` / `assert_eq!()` を本番コードのエラー処理に使わず、失敗を `Result` で返す。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope

### Unsafe Code

`unsafe` は最小限かつ Safety コメント必須。使用前に `reviewer` capability のレビューを受けること。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope

### Usecase Layer Purity

usecase 層は純粋なオーケストレーターであり、実行環境へ直接到達しない。I/O と実行時依存は境界で受け取り、必要な外部機能は domain / usecase の port を通じて扱う。

> **強制先**: review 観点 — usecase / cli_driver / cli_composition scope

| 禁止 | 正しい対処 |
|---|---|
| `std::fs::*` / `std::net::*` / `std::process::*` / `std::io::*` / `std::env::*` | CLI または infrastructure adapter が扱い、typed input / port 経由で渡す |
| `chrono::Utc::now()` / `std::time::SystemTime` / `std::time::Instant` | ユーザー指定の時刻は usecase entrypoint の typed input として受け取り、実行時刻は usecase 所有の Clock port から取得する |
| `println!` / `eprintln!` / `print!` / `eprint!` | `Result<T, E>` を返し、CLI が表示と exit code を担う |

> **強制先**: review 観点 — usecase / cli / cli_driver / cli_composition scope

`sotp verify usecase-purity` は syn AST により上記のパターンを検査する。現行の強制強度は CI blocking である — 違反は error finding として検出され (exit 1)、`cargo make ci` の依存 gate (`verify-usecase-purity-local`) を失敗させる。強制の緩和 (gate からの除外等) は採用者が ADR で判断する。async runtime の採用も ADR の決定事項である。

> **強制先**: 機械 lint — bin/sotp verify usecase-purity

async runtime の採用や強制の緩和は、ADR の決定事項として扱う。

> **強制先**: review 観点 — adr scope

### Port Injection and Facade Policy

入力 port は 1 ユースケースにつき 1 trait とし、実行メソッドを 1 つだけ持つ。driver の注入粒度は port の粒度に合わせ、driver は自分が消費する複数の単能 port をそれぞれ直接受け取ってよい。「driver は 1 つの interactor だけを注入する」という制約は置かない。

> **強制先**: review 観点 — types / usecase / cli_driver / cli_composition scope

command と query を混載する facade port を新設してはならない。この禁止は未移行の文脈にも適用する。既存の facade port や既存の単一 interactor 注入は、この規約だけを理由に遡及改修しない。

> **強制先**: review 観点 — types / usecase / cli_driver / cli_composition scope

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
    a.checked_div(b).ok_or(MathError::DivisionOverflow)
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
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- モジュールサイズ上限（700 行）はテスト専用ファイルには適用しない。
  > **強制先**: 機械 lint — bin/sotp verify module-size

## Review Checklist

- [ ] 本番コードに `unwrap()` / `expect()` / `panic!()` / `todo!()` / `unreachable!()` がないか
  > **強制先**: 機械 lint — cargo make clippy
- [ ] インデックスアクセス `slice[i]` / `str[range]` が `.get()` に置き換えられているか
  > **強制先**: 機械 lint — cargo make clippy
- [ ] 公開 API に `///` コメントと `# Errors` セクションがあるか
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- [ ] モジュールが 700 行以内か（プロダクションコードのみ）
  > **強制先**: 機械 lint — bin/sotp verify module-size
- [ ] 命名が PascalCase / snake_case 規則に従っているか
  > **強制先**: 機械 lint — cargo make clippy
- [ ] `unsafe` ブロックに Safety コメントがあるか
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- [ ] usecase が I/O、暗黙的な時刻、環境、プロセス、出力を直接扱っていないか
  > **強制先**: 機械 lint — bin/sotp verify usecase-purity

## Decision Reference

- `knowledge/conventions/prefer-type-safe-abstractions.md`: 型安全パターン（Newtype / Enum-first / Typestate）
- `architecture-rules.json`: crate 間依存の機械可読 SSoT
- `knowledge/conventions/type-designer-kind-selection.md` R1: role × layer 配置の SSoT
- `knowledge/conventions/security.md`: シークレット管理・SQL インジェクション対策
