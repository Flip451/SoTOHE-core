---
required_for:
  - type-designer
  - spec-designer
  - impl-planner
  - rollback-diagnoser
---

# Prefer Type-Safe Abstractions Convention

## この文書の所有権

この規約は **利用プロジェクトが所有する**。初期値としてテンプレートから供給されるが、以後の改稿・改名・削除はプロジェクトの裁量である。ハーネスが所有するのは、規約を capability へ届ける解決機構 (`required_for` frontmatter とその resolver) だけであり、**どの抽象を優先するか** という方針そのもの — 以下の Decision Flow と各パターン — はこの文書にあり、プロジェクトのものである。

この文書のファイル名にも節見出しにも、ハーネスの側から依存しているものはない。節の追加・削除・改名、パターンの入れ替え、文書ごとの破棄はいずれも安全な変更である。破棄する場合は `required_for` frontmatter ごと削除すれば、上記 capability への必読解決から外れる。

## Rule

バグパターンが発見されたとき、lint ルールや convention doc で「やってはいけない」を追加するのではなく、そのバグクラスを型システムや標準ライブラリで根本的に排除する方法を優先すること。

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

## 必要駆動の抽象

候補はまず、アーキテクチャが要求する structure-required port とその実装の組か、層の内部で任意に追加する service-level 抽象かを分類する。ユースケース自身の入力ポートは、1 ユースケース 1 trait・実行メソッド 1 つの `ApplicationService` inbound port と、その `Interactor` 実装からなる structure-required な組であり、複数の実装や service 自体のテスト境界での差し替えがなくても、必要性テストではなくアーキテクチャ規則に従って導入する。層を越える依存を表す `SecondaryPort` と aggregate の永続化を表す `Repository` も同じく structure-required ports として扱う。

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli_driver scope

structure-required な入力ポートの組の上に同じ service を共有するためだけに第二の trait と実装を重ねたり、その他の任意の service-level 抽象を追加したりしてはならない。これらの任意 abstraction は、複数の実装が現存するか、service 自体をテスト境界で差し替える必要がある場合だけ導入する。共有所有だけなら `Arc<具象型>` を既定とし、条件が後から成立したときに trait を切り出す。structure-required な `ApplicationService` + `Interactor` の組、`SecondaryPort`、`Repository` は、この禁止の対象ではなく、単一実装でも支配するアーキテクチャ規則に従う。

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli_driver scope

既存の抽象ペアは、改訂後の規約に合わせて遡及解体しない。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope

## Rationale

- **Lint ルールは破られる**: convention doc や記憶に残したルールは忘れられる。CI ルールも例外追加で形骸化する。
- **型は破れない**: コンパイラが強制する制約は、すべての開発者とすべての AI エージェントに等しく適用される。
- **AI エージェントの傾向**: AI は「手書きコード + ルールで防止」に走りがちだが、「標準ライブラリで問題クラスを排除」が正しい選択肢であることが多い。

## Decision Flow

バグパターン発見時:

次の順で、型または既存 crate による解決可能性を確認する。

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

1. **標準ライブラリ / 既存 crate で排除可能か?**
   - `serde` の typed deserialization → `serde_json::Value` の手動走査の排除
   - `syn` の AST パース → Rust ソースに対する行ベースヒューリスティックの排除
   - その領域で確立した parser crate → hand-rolled parser の逐次修正の排除
   - **可能なら採用** (最優先)

   > **強制先**: review 観点 — domain / usecase / infrastructure / cli scope

2. **型で表現可能か?**
   - Newtype パターンで不正値を構築不能にする
   - `enum` で有限の値集合や状態ごとに異なるデータを構造化する
   - 状態遷移がある場合は typestate + 遷移関数を第一候補にする
   - **可能なら採用**

   > **強制先**: review 観点 — types / domain / usecase / infrastructure / cli / cli_driver scope

3. **上記で対応不可能な場合のみ**:
   - CI lint (`architecture-rules.json`, clippy)
   - Convention doc
   - 記憶 / behavioral rule (最後の手段)

   > **強制先**: review 観点 — harness-policy scope

## Examples

| バグパターン | Bad (ルールで防止) | Good (型で排除) |
|---|---|---|
| JSON の不正データ | `filter_map` 禁止ルール | `#[derive(Deserialize)]` |
| Rust ソースの構文誤判定 | 行ベース heuristic の改善 | `syn` crate で AST パース |
| 外部フォーマットの解析漏れ | hand-rolled parser の修正 | 確立した parser crate の AST 走査 |
| 不正な Email 値 | バリデーション関数 | `Email` newtype |

---

## Make Illegal States Unrepresentable

型システムで不正な状態を表現不可能にする。

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

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

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli / cli_driver scope

```rust
// Bad: struct + runtime validation — 不正状態がメモリ上に存在しうる
struct PaymentResult {
    kind: PaymentResultKind,       // Approved or Declined
    reasons: Vec<DeclineReason>,   // Approved なのに reasons が入りうる
}
impl PaymentResult {
    fn new(kind: PaymentResultKind, reasons: Vec<DeclineReason>) -> Result<Self, Error> {
        if kind == PaymentResultKind::Approved && !reasons.is_empty() {
            return Err(Error::Inconsistent); // runtime でしか防げない
        }
        Ok(Self { kind, reasons })
    }
}

// Good: enum — 不正状態が構造的に不可能
enum PaymentResult {
    Approved,                        // reasons を持てない
    Declined(Vec<DeclineReason>),    // reasons が必ずある
}
```

**判断基準：**

| パターン | 対処 |
|---|---|
| 状態ごとに持つデータが違う | → enum の variant にデータを持たせる |
| struct + `Option<T>` で「この状態では None」 | → enum を検討 (Option の None が特定状態と 1:1 対応なら enum が適切) |
| struct + constructor validation で cross-field 制約 | → enum で構造的に排除できないか検討 |
| 型で表現できない制約 (例: Vec の non-empty) | → constructor validation は OK (型レベルの限界) |

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli / cli_driver scope

**プロジェクト内の良い例：**
採用した実例をここに追記していく。「この enum は何を構造的に不可能にしたか」を 1 行で添えると、後続の設計判断で参照できる基準になる。

### Typestate パターン：状態遷移をコンパイル時に強制する

状態遷移がある場合、**単一の型 + status フィールド + runtime 遷移チェック** ではなく、
**状態ごとに別の型** を定義して遷移メソッドの引数/戻り値で正しい遷移のみを許可する。

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli / cli_driver scope

```rust
// Bad: runtime で遷移を検証 — 不正遷移がコンパイルを通る
struct Order {
    status: OrderStatus,  // Draft, Placed, Shipped
}
impl Order {
    fn ship(&mut self) -> Result<(), Error> {
        if self.status != OrderStatus::Placed {
            return Err(Error::InvalidTransition); // runtime エラー
        }
        self.status = OrderStatus::Shipped;
        Ok(())
    }
}

// Good: typestate — 不正遷移がコンパイルエラーになる
struct Draft;
struct Placed { placed_at: Timestamp }
struct Shipped { placed_at: Timestamp, shipped_at: Timestamp }

struct Order<S> { state: S, /* 共通フィールド */ }

impl Order<Draft> {
    fn place(self, at: Timestamp) -> Order<Placed> {
        Order { state: Placed { placed_at: at }, /* ... */ }
    }
}
impl Order<Placed> {
    fn ship(self, at: Timestamp) -> Order<Shipped> {
        Order { state: Shipped { placed_at: self.state.placed_at, shipped_at: at }, /* ... */ }
    }
}
// Order<Draft> に ship() は存在しない → コンパイルエラー
```

**使い分け：enum vs typestate (基本原則)**

- **状態遷移がない** (有限の値の集合) → **enum**
- **状態遷移がある** (少しでも) → **typestate + 遷移関数を優先**
- typestate は「遷移の有無」で判断する。遷移が少しでもあれば typestate を第一候補にする。

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli / cli_driver scope

| 要件 | 推奨パターン |
|---|---|
| 有限の値の集合 (遷移なし) | → **enum-first** |
| 状態ごとにデータが違う (遷移なし) | → **enum-first** (variant にデータを持たせる) |
| 状態遷移がある (少しでも) | → **typestate** + 遷移関数 |
| 状態ごとにデータが違う + 遷移あり | → **typestate + 状態型を enum-first で設計** |
| 状態が永続化から復元される (serde 必要) | → domain 層は **typestate**、infrastructure 層で serde 対応 enum DTO に変換 (ヘキサゴナル分離) |
| 状態数が多く組み合わせ爆発する | → enum + runtime validation (typestate の型爆発を避けるエスケープハッチ) |

> **強制先**: review 観点 — types / domain / infrastructure scope

**typestate が適さないケース (エスケープハッチ)：**
- 状態数が多い (型の数が爆発する)
- 状態遷移がデータ駆動 (外部入力で遷移先が決まる)

これらの場合は enum + runtime validation が現実的。ただし「typestate で表現できないか」を最初に検討すること。

> **強制先**: review 観点 — types / domain / usecase / infrastructure scope

**永続化が必要な場合：**
domain 層では typestate を維持し、infrastructure 層で serde 対応 enum DTO と相互変換する。
- domain → DTO: `From<Order<Shipped>> for OrderDto`
- DTO → domain: `TryFrom<OrderDto> for Order<Placed>` (fallible — 不正な状態復元は `Result` で報告)

> **強制先**: review 観点 — types / domain / infrastructure scope

domain 層の型安全性を永続化の都合で妥協しない (ヘキサゴナルアーキテクチャの原則)。

> **強制先**: review 観点 — domain / infrastructure scope

## Review Checklist

- [ ] 不正状態が型レベルで排除されているか (struct + runtime validation より enum/typestate を優先)
  > **強制先**: review 観点 — types / domain / usecase / infrastructure / cli / cli_driver scope
- [ ] プリミティブ値の制約は Newtype パターンで表現されているか
  > **強制先**: review 観点 — types / domain / usecase / infrastructure / cli / cli_driver scope
- [ ] 状態遷移がある場合、typestate パターンを検討したか
  > **強制先**: review 観点 — types / domain / usecase / infrastructure / cli / cli_driver scope
- [ ] serde が必要な場合、domain 層の typestate は維持されているか (infrastructure 層で DTO 変換)
  > **強制先**: review 観点 — types / domain / infrastructure scope
- [ ] 外部データのデシリアライズは typed deserialization を使っているか (手動の動的値走査になっていないか)
  > **強制先**: review 観点 — infrastructure scope

## Related Documents

- `type-designer-kind-selection.md` — 型の role × layer 配置 (本 convention の設計原則を role 選定へ適用したもの)
- `coding-principles.md` — Error handling / naming / module size / no-panics の規約
- `architecture-rules.json` — 層 id と層 path の SSoT
