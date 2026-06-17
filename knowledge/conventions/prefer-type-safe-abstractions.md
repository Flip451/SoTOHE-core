# Prefer Type-Safe Abstractions Convention

## Rule

バグパターンが発見されたとき、lint ルールや convention doc で「やってはいけない」を追加するのではなく、そのバグクラスを型システムや標準ライブラリで根本的に排除する方法を優先すること。

## Rationale

- **Lint ルールは破られる**: convention doc やメモリに記録されたルールは忘れられる。CI ルールも例外追加で形骸化する。
- **型は破れない**: コンパイラが強制する制約は全開発者と全 AI エージェントに等しく適用される。
- **AI エージェントの傾向**: AI は「手書きコード + ルールで防止」に走りがちだが、「標準ライブラリで問題クラスを排除」が正しい選択肢であることが多い。

## Decision Flow

バグパターン発見時:

1. **標準ライブラリ / 既存 crate で排除可能か?**
   - `serde` typed deserialization → `serde_json::Value` 手動走査の排除
   - `syn` AST パース → 行ベースヒューリスティックの排除
   - `conch-parser` → hand-rolled shell tokenizer の排除
   - **可能なら採用** (最優先)

2. **型で表現可能か?**
   - Newtype pattern で不正値を構築不能にする
   - `enum` で有限の値集合や状態ごとに異なるデータを構造化する
   - 状態遷移がある場合は typestate + 遷移関数を第一候補にする
   - **可能なら採用**

3. **上記で対応不可能な場合のみ**:
   - CI lint (`architecture-rules.json`, clippy)
   - Convention doc
   - メモリ / behavioral rule (最後の手段)

## Examples

| バグパターン | Bad (ルールで防止) | Good (型で排除) |
|---|---|---|
| JSON の不正データ | `filter_map` 禁止ルール | `#[derive(Deserialize)]` |
| テストモジュール誤判定 | 行ベース heuristic 改善 | `syn` crate で AST パース |
| Shell コマンド解析漏れ | hand-rolled parser 修正 | `conch-parser` AST 走査 |
| 不正な Email 値 | バリデーション関数 | `Email` newtype |

---

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

**使い分け：enum vs typestate（基本原則）**

- **状態遷移がない**（有限の値の集合）→ **enum**
- **状態遷移がある**（少しでも）→ **typestate + 遷移関数を優先**
- typestate は「遷移の有無」で判断する。遷移が少しでもあれば typestate を第一候補にする。

| 要件 | 推奨パターン |
|---|---|
| 有限の値の集合（遷移なし） | → **enum-first** |
| 状態ごとにデータが違う（遷移なし） | → **enum-first**（variant にデータを持たせる） |
| 状態遷移がある（少しでも） | → **typestate** + 遷移関数 |
| 状態ごとにデータが違う + 遷移あり | → **typestate + 状態型を enum-first で設計** |
| 状態が永続化から復元される（serde 必要） | → domain 層は **typestate**、infrastructure 層で serde 対応 enum DTO に変換（ヘキサゴナル分離） |
| 状態数が多く組み合わせ爆発する | → enum + runtime validation（typestate の型爆発を避けるエスケープハッチ） |

**typestate が適さないケース（エスケープハッチ）：**
- 状態数が多い（型の数が爆発する）
- 状態遷移がデータ駆動（外部入力で遷移先が決まる）

これらの場合は enum + runtime validation が現実的。ただし「typestate で表現できないか」を最初に検討すること。

**永続化が必要な場合：**
domain 層では typestate を維持し、infrastructure 層で serde 対応 enum DTO と相互変換する。
- domain → DTO: `From<Review<State>> for ReviewStatusDto`
- DTO → domain: `TryFrom<ReviewStatusDto> for Review<State>`（fallible — 不正な状態復元は `Result` で報告）

domain 層の型安全性を永続化の都合で妥協しない（ヘキサゴナルアーキテクチャの原則）。

## Review Checklist

- [ ] 不正状態が型レベルで排除されているか（struct + runtime validation より enum/typestate を優先）
- [ ] プリミティブ値の制約は Newtype パターンで表現されているか
- [ ] 状態遷移がある場合、typestate パターンを検討したか
- [ ] serde が必要な場合、domain 層の typestate は維持されているか（infrastructure 層で DTO 変換）
- [ ] 外部データのデシリアライズは typed deserialization を使っているか

## Decision Reference

- `knowledge/conventions/typed-deserialization.md`: serde を使った型安全なデシリアライズ
- `knowledge/conventions/hexagonal-architecture.md`: ヘキサゴナルアーキテクチャにおける型の配置
- `knowledge/conventions/coding-principles.md`: Error handling / naming / module size / no-panics の規約
