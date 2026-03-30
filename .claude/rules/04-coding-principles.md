# Coding Principles (Rust)

## Make Illegal States Unrepresentable

型システムで不正な状態を表現不可能にする。

### Newtype パターン：プリミティブ値の制約

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

### Enum-first パターン：バリアント依存データは enum で表現する

状態ごとに持つべきデータが異なる場合、**struct + runtime validation ではなく enum の variant にデータを持たせる**。
これにより不正な組み合わせがコンパイル時に排除される。

```rust
// Bad: struct + runtime validation — 不正状態がメモリ上に存在しうる
struct Verdict {
    kind: VerdictKind,           // ZeroFindings or FindingsRemain
    findings: Vec<Finding>,      // ZeroFindings なのに findings が入りうる
}
impl Verdict {
    fn new(kind: VerdictKind, findings: Vec<Finding>) -> Result<Self, Error> {
        if kind == VerdictKind::ZeroFindings && !findings.is_empty() {
            return Err(Error::Inconsistent); // runtime でしか防げない
        }
        Ok(Self { kind, findings })
    }
}

// Good: enum — 不正状態が構造的に不可能
enum Verdict {
    ZeroFindings,                       // findings を持てない
    FindingsRemain(Vec<Finding>),       // findings が必ずある
}
```

**判断基準：**

| パターン | 対処 |
|---|---|
| 状態ごとに持つデータが違う | → enum の variant にデータを持たせる |
| struct + `Option<T>` で「この状態では None」 | → enum を検討（Option の None が特定状態と 1:1 対応なら enum が適切） |
| struct + constructor validation で cross-field 制約 | → enum で構造的に排除できないか検討 |
| 型で表現できない制約（例: Vec の non-empty） | → constructor validation は OK（型レベルの限界） |

**プロジェクト内の良い例：**
- `CodeHash`: `NotRecorded` | `Pending` | `Computed(String)` — 3 状態を struct + Option で表現せず enum
- `ReviewGroupState`: `NoRounds` | `FastOnly(R)` | `FinalOnly(R)` | `BothRounds { fast, final }` — 組み合わせごとに variant
- `GroupRoundVerdict`: `ZeroFindings` | `FindingsRemain(Vec<StoredFinding>)` — verdict と findings の不整合を構造的に排除

### Typestate パターン：状態遷移をコンパイル時に強制する

状態遷移がある場合、**単一の型 + status フィールド + runtime 遷移チェック** ではなく、
**状態ごとに別の型** を定義して遷移メソッドの引数/戻り値で正しい遷移のみを許可する。

```rust
// Bad: runtime で遷移を検証 — 不正遷移がコンパイルを通る
struct Review {
    status: ReviewStatus,  // NotStarted, FastPassed, Approved
}
impl Review {
    fn record_final(&mut self) -> Result<(), Error> {
        if self.status != ReviewStatus::FastPassed {
            return Err(Error::InvalidTransition); // runtime エラー
        }
        self.status = ReviewStatus::Approved;
        Ok(())
    }
}

// Good: typestate — 不正遷移がコンパイルエラーになる
struct NotStarted;
struct FastPassed { fast_hash: String }
struct Approved { fast_hash: String, final_hash: String }

struct Review<S> { state: S, /* 共通フィールド */ }

impl Review<NotStarted> {
    fn record_fast(self, hash: String) -> Review<FastPassed> {
        Review { state: FastPassed { fast_hash: hash }, /* ... */ }
    }
}
impl Review<FastPassed> {
    fn record_final(self, hash: String) -> Review<Approved> {
        Review { state: Approved { fast_hash: self.state.fast_hash, final_hash: hash }, /* ... */ }
    }
}
// Review<NotStarted> に record_final() は存在しない → コンパイルエラー
```

**使い分け：enum-first vs typestate**

| 要件 | 推奨パターン |
|---|---|
| 状態ごとにデータが違う（表現の問題） | → **enum-first** |
| 状態遷移に制約がある（遷移の問題） | → **typestate** |
| 両方 | → **typestate + 状態型を enum-first で設計** |
| 状態が永続化から復元される（serde 必要） | → enum（typestate は永続化と相性が悪い） |
| 状態数が多く組み合わせ爆発する | → enum + runtime validation（typestate の型爆発を避ける） |

**typestate が適さないケース：**
- 状態を JSON/DB から復元する必要がある（serde との統合が複雑）
- 状態数が多い（型の数が爆発する）
- 状態遷移がデータ駆動（外部入力で遷移先が決まる）

これらの場合は enum + runtime validation が現実的。ただし「typestate で表現できないか」を最初に検討すること。

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

テスト（`#[cfg(test)]`）以外のコードでパニックを起こしうる構文は**禁止**:

| 禁止パターン | 安全な代替 |
|---|---|
| `slice[i]` / `str[range]` | `.get(i)` / `.get(range)` |
| `.unwrap()` | `?` / `.unwrap_or()` / `if let` |
| `.expect("...")` | `?` / `.unwrap_or()` / `if let` |
| `assert!()` / `assert_eq!()` | `if !cond { return Err(...) }` |
| `panic!()` / `unreachable!()` | `return Err(...)` |
| `todo!()` / `unimplemented!()` | コンパイルエラーにするか `return Err(...)` |

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

## Unsafe Code

`unsafe` は最小限かつコメント必須：

```rust
// Safety: ptr was created by Box::into_raw and has not been freed.
unsafe { Box::from_raw(ptr) }
```

`unsafe` の使用前に Codex にレビューを依頼すること。
