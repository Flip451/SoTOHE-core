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

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope

---

## Rules

### Error Handling: Result and ? Operator

`unwrap()` は本番コード禁止 (テスト内のみ可)。

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
- 行数の目安と上限は `architecture-rules.json` の `module_limits` が SSoT である。`warn_lines` を超えると警告、`max_lines` を超えると error になる。テンプレートは既定値を入れて出荷するが、値を決めるのはプロジェクトであり、本文に数値を書き写さない。
- 行数の目安・上限は**プロダクションコードのみ**が対象。`bin/sotp verify module-size` は、ファイル先頭が `#![cfg(test)]` のファイル、`#[cfg(test)] mod` ブロックの行、および `*_tests.rs` / `tests/` のテスト専用ファイルを行数から除外する。関連テストは 1 ファイルにまとめてよい。

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

唯一の限定例外は、秘匿境界の静的リテラル正規表現である。`LazyLock<Regex>` として構築し、当該行に限定した allow 注釈つきの `expect` を使い、構築検証のテストを併設する。不正な静的パターンはプログラミングエラーであり、秘匿の無音停止より fail-stop が正しいという判断は ADR（`knowledge/adr/README.md` の索引から辿る sensitive-redaction の決定）に記録されている。この例外を他の `expect` / `unwrap` に広げてはならない。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope

`assert!()` / `assert_eq!()` を本番コードのエラー処理に使わず、失敗を `Result` で返す。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope

### Unsafe Code

`unsafe` は最小限かつ Safety コメント必須。使用前にコードレビューを受けること。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope

### Usecase Layer Purity

usecase 層は純粋なオーケストレーターであり、実行環境へ直接到達しない。I/O と実行時依存は境界で受け取り、必要な外部機能は domain / usecase の port を通じて扱う。

> **強制先**: review 観点 — usecase / cli_driver / cli_composition scope

| 禁止 | 正しい対処 |
|---|---|
| `std::fs::*` / `std::net::*` / `std::process::*` / `std::io::*` / `std::env::*` | delivery 層または infrastructure adapter が扱い、typed input / port 経由で渡す |
| `chrono::Utc::now()` / `std::time::SystemTime` / `std::time::Instant` | 利用者が指定した時刻は usecase entrypoint の typed input として受け取り、実行時刻は usecase 所有の Clock port から取得する |
| `println!` / `eprintln!` / `print!` / `eprint!` | `Result<T, E>` を返し、delivery 層が表示と exit code を担う |

> **強制先**: review 観点 — usecase / cli / cli_driver / cli_composition scope

このルールが適用される層は `architecture-rules.json` が決める。層 entry に `verify.usecase_purity: true` を宣言した層に対して、`bin/sotp verify usecase-purity` が syn AST で上記パターンを検査する。テンプレート既定では違反は error finding となり、`cargo make ci` を失敗させる。強制の緩和 (検査対象からの除外、警告への降格) と async runtime の採用は、いずれもプロジェクトが ADR で判断する事項である。

> **強制先**: 機械 lint — bin/sotp verify usecase-purity

async runtime の採用や強制の緩和は、ADR の決定事項として扱う。

> **強制先**: review 観点 — adr scope

### Port Injection and Facade Policy

入力 port は 1 ユースケースにつき 1 trait とし、実行メソッドを 1 つだけ持つ。driver の注入粒度は port の粒度に合わせ、driver は自分が消費する複数の単能 port をそれぞれ直接受け取ってよい。「driver は 1 つの interactor だけを注入する」という制約は置かない。

> **強制先**: review 観点 — types / usecase / cli_driver / cli_composition scope

入力 port の 1 trait 規則は、入力 port trait を置く場合の粒度を定める。R2 の stateless 判定であっても、R1 で application-only の user-facing use-case entrypoint と分類する top-level `pub fn` は `FreeFunction` 判定から除外し、`role: UseCaseFunction` としてモデル化する。その entrypoint は port trait も Interactor も持たず、driver はその関数を直接呼び出す。この形は 1 trait 規則の対象外である。後からその操作に入力 port trait を導入する時点で、1 ユースケース 1 trait・実行メソッド 1 つの規則に従う。

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
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- モジュールサイズ上限はテスト専用ファイルには適用しない。
  > **強制先**: 機械 lint — bin/sotp verify module-size

## Review Checklist

- [ ] 本番コードに `unwrap()` / `panic!()` / `todo!()` / `unreachable!()` がなく、`expect()` は秘匿境界の静的リテラル正規表現を `LazyLock<Regex>` として構築し、当該行に限定した allow 注釈と構築検証テストを併設する場合に限って使われているか
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- [ ] インデックスアクセス `slice[i]` / `str[range]` が `.get()` に置き換えられているか
  > **強制先**: 機械 lint — cargo make clippy
- [ ] 公開 API に `///` コメントと `# Errors` セクションがあるか
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- [ ] モジュールが `architecture-rules.json` の `module_limits.max_lines` 以内か (プロダクションコードのみ)
  > **強制先**: 機械 lint — bin/sotp verify module-size
- [ ] 命名が PascalCase / snake_case 規則に従っているか
  > **強制先**: 機械 lint — cargo make clippy
- [ ] `unsafe` ブロックに Safety コメントがあるか
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- [ ] usecase が I/O、暗黙的な時刻、環境、プロセス、出力を直接扱っていないか
  > **強制先**: 機械 lint — bin/sotp verify usecase-purity

## Related Documents

- `prefer-type-safe-abstractions.md` — 型安全パターン (Newtype / Enum-first / Typestate)
- `type-designer-kind-selection.md` — role × layer 配置の方針
- `security.md` — シークレット管理、入力検証、SQL インジェクション対策
- `testing.md` — TDD サイクルとテスト構造 (本規約のテスト例外の適用先)
- `architecture-rules.json` — 層 id、層 path、`module_limits`、層ごとの verify フラグの SSoT
