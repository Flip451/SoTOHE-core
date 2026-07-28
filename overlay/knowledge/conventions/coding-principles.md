---
required_for:
  - spec-designer
  - impl-planner
---

# Coding Principles Convention

## この文書の所有権

この規約は **利用プロジェクトが所有する**。初期値としてテンプレートから供給されるが、以後の改稿・改名・削除はプロジェクトの裁量である。ハーネスが所有するのは、規約を capability へ届ける解決機構 (`required_for` frontmatter とその resolver) と、本文が参照する検査コマンド (`bin/sotp verify module-size` / `bin/sotp verify usecase-purity`) の実装だけである。**どう書くか** という方針そのもの — 以下の各ルール — はこの文書にあり、プロジェクトのものである。

機械が読む値 (行数閾値、適用層) はこの文書ではなく `architecture-rules.json` が持つ。本文はその値を書き写さず、どこが SSoT かだけを示す。文書に数値を複製すると、閾値を変えたときに文書と gate が黙って食い違う。

この規約を全面的に破棄しても構わない。その場合 `required_for` frontmatter ごと削除すれば、spec-designer / impl-planner への必読解決から外れる。ファイル名も節見出しもハーネスの参照対象ではないため、改名や再構成で壊れるものはない。

## Purpose

Rust コードベース全体に適用する実装規約。エラーハンドリング・命名規則・モジュールサイズ・ドキュメント・パニック禁止・`unsafe` の扱いを定める。

## Scope

- 適用対象: `architecture-rules.json` の `layers[].path` が指すクレート配下の Rust プロダクションコード
- 適用外: `#[cfg(test)]` ブロック、`tests/` 統合テスト (パニック禁止ルールとモジュールサイズ上限のみ適用外)

---

## Rules

### Error Handling: Result and ? Operator

`unwrap()` は本番コード禁止 (テスト内のみ可)。`?` 演算子で伝搬し、境界では適切な `From` 変換を実装する。

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
- 行数の目安と上限は `architecture-rules.json` の `module_limits` が SSoT である。`warn_lines` を超えると警告、`max_lines` を超えると error になる。テンプレートは既定値を入れて出荷するが、値を決めるのはプロジェクトであり、本文に数値を書き写さない。
- 行数の目安・上限は**プロダクションコードのみ**が対象。`bin/sotp verify module-size` は、ファイル先頭が `#![cfg(test)]` のファイル、`#[cfg(test)] mod` ブロックの行、および `*_tests.rs` / `tests/` のテスト専用ファイルを行数から除外する。関連テストは 1 ファイルにまとめてよい。

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

`unsafe` は最小限かつ Safety コメント必須。使用前にコードレビューを受けること。

### Usecase Layer Purity

usecase 層は純粋なオーケストレーターであり、実行環境へ直接到達しない。I/O と実行時依存は境界で受け取り、必要な外部機能は domain / usecase の port を通じて扱う。

| 禁止 | 正しい対処 |
|---|---|
| `std::fs::*` / `std::net::*` / `std::process::*` / `std::io::*` / `std::env::*` | delivery 層または infrastructure adapter が扱い、typed input / port 経由で渡す |
| `chrono::Utc::now()` / `std::time::SystemTime` / `std::time::Instant` | 利用者が指定した時刻は usecase entrypoint の typed input として受け取り、実行時刻は usecase 所有の Clock port から取得する |
| `println!` / `eprintln!` / `print!` / `eprint!` | `Result<T, E>` を返し、delivery 層が表示と exit code を担う |

このルールが適用される層は `architecture-rules.json` が決める。層 entry に `verify.usecase_purity: true` を宣言した層に対して、`bin/sotp verify usecase-purity` が syn AST で上記パターンを検査する。テンプレート既定では違反は error finding となり、`cargo make ci` を失敗させる。強制の緩和 (検査対象からの除外、警告への降格) と async runtime の採用は、いずれもプロジェクトが ADR で判断する事項である。

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
    match a.checked_div(b) {
        Some(result) => Ok(result),
        None if b == 0 => Err(MathError::DivisionByZero),
        None => Err(MathError::Overflow),
    }
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
pub fn place_order<R: OrderRepositoryPort>(repo: &R, order: Order) -> Result<OrderId, PlaceOrderError> {
    repo.save(order).map_err(PlaceOrderError::from)
}

// Bad: usecase reaches directly into the runtime and presents output.
let content = std::fs::read_to_string(path)?;
println!("saved");
```

---

## Exceptions

- テストコード (`#[cfg(test)]`) では `unwrap()` / `expect()` / `assert!()` を使ってよい。
- モジュールサイズ上限はテスト専用ファイルには適用しない。

## Review Checklist

- [ ] 本番コードに `unwrap()` / `expect()` / `panic!()` / `todo!()` / `unreachable!()` がないか
- [ ] インデックスアクセス `slice[i]` / `str[range]` が `.get()` に置き換えられているか
- [ ] 公開 API に `///` コメントと `# Errors` セクションがあるか
- [ ] モジュールが `architecture-rules.json` の `module_limits.max_lines` 以内か (プロダクションコードのみ)
- [ ] 命名が PascalCase / snake_case 規則に従っているか
- [ ] `unsafe` ブロックに Safety コメントがあるか
- [ ] usecase が I/O、暗黙的な時刻、環境、プロセス、出力を直接扱っていないか

## Related Documents

- `prefer-type-safe-abstractions.md` — 型安全パターン (Newtype / Enum-first / Typestate)
- `type-designer-kind-selection.md` — role × layer 配置の方針
- `security.md` — シークレット管理、入力検証、SQL インジェクション対策
- `testing.md` — TDD サイクルとテスト構造 (本規約のテスト例外の適用先)
- `architecture-rules.json` — 層 id、層 path、`module_limits`、層ごとの verify フラグの SSoT
