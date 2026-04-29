---
adr_id: 2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter
decisions:
  - id: M1
    user_decision_ref: "chat_segment:tddd-struct-kind-uniformization-adr:2026-04-29"
    status: proposed
  - id: S1
    user_decision_ref: "chat_segment:tddd-struct-kind-uniformization-adr:2026-04-29"
    status: proposed
  - id: S2
    user_decision_ref: "chat_segment:tddd-struct-kind-uniformization-adr:2026-04-29"
    status: proposed
  - id: S3
    user_decision_ref: "chat_segment:tddd-struct-kind-uniformization-adr:2026-04-29"
    status: proposed
---
# TDDD struct kind taxonomy の field/method 均質化と type catalogue linter 機構の導入

## Context

### §1 根本問題: struct 系 9 kind の field/method 表現能力が均質でない

`TypeDefinitionKind` には struct ベースの 9 kind がある:
`Typestate` / `ValueObject` / `UseCase` / `Interactor` / `Dto` / `Command` / `Query` /
`Factory` / `SecondaryAdapter`。

これらのうち `expected_methods` を持つのは `SecondaryAdapter` だけだが、それも直接ではなく
`implements[].expected_methods` 経由に限られる。残り 8 kind は `expected_members`
のみを持ち、method 宣言の場所がない。

| kind | expected_members | expected_methods |
|---|---|---|
| `typestate` | ✓ | ✗ |
| `value_object` | ✓ | ✗ |
| `use_case` | ✓ | ✗ |
| `interactor` | ✓ | ✗ (declares_application_service のみ) |
| `dto` | ✓ | ✗ |
| `command` | ✓ | ✗ |
| `query` | ✓ | ✗ |
| `factory` | ✓ | ✗ |
| `secondary_adapter` | ✓ | △ (implements 内部経由のみ) |
| `secondary_port` | ✗ | ✓ |
| `application_service` | ✗ | ✓ |
| `free_function` | ✗ | ✓ (params/returns) |

Rust struct は field と method を両方持てる。しかし catalogue schema はこの事実を
表現できない。renderer (`libs/domain/src/tddd/contract_map_render.rs`) の `methods_of()`
(line 392 周辺) が 3 種特例分岐 (`SecondaryPort` / `ApplicationService` / `SecondaryAdapter`)
しか method を返さないのも、この schema 非均質性の下流影響そのものである。

### §2 派生問題 1: 表現できない型契約

schema の非均質性が直接引き起こす表現漏れ:

1. **validating constructor の error 型**: `fn new(...) -> Result<Self, FooError>` の
   `FooError` を catalog から指したくても、`value_object` には `expected_methods` がなく
   method の戻り値型として宣言できない。`AdrFrontMatterError` / `AdrDecisionCommonError`
   のような error 型がすべて graph 上で孤立する。

2. **interactor / factory / use_case の補助 method**: trait 実装外の helper / accessor
   method を catalog に書く場所がない。

3. **domain layer の behavioral struct**: `libs/domain/src/skill_compliance/mod.rs:17` の
   `ComplianceContext { skill_match }` + `render() -> Option<String>` が典型例。field を
   持ち、`render` という behavior method を持つ。これを受け止める kind が現状にはない。

### §3 派生問題 2: kind 選択の歪み

表現能力の差が原因で、type-designer が意味論的に正しくない kind を選ぶ選択を迫られる:

- behavior を持つ struct を `value_object` に押し込む (value_object semantic restriction 違反:
  validated immutable value のみを value_object に置くという意味論を破る)
- `use_case` を非 usecase 層に配置する (kind 配置層ルール違反: use_case は usecase 層 ONLY)
- どの kind も fit しないとき `value_object` を catch-all に使う (No Fallback ルール違反:
  fit しない型を value_object に逃がすことを convention は禁じている)

convention の各ルール (kind 配置層マトリクス / value_object semantic restriction / No Fallback 等) は
agent self-reject / reviewer / type-signal evaluator の多層防御でこの歪みを防ごうとしているが、
根本は schema 非均質性に由来するため、convention 側だけで完全には封じられない。

### §4 派生問題 3: contract-map の孤立ノード (副作用)

renderer の `methods_of()` が member-only kind に method edge を生成できないため、
catalog に declare した依存関係が graph に出ない。具体的には:

- value_object の `new()` 戻り値の error 型への edge が欠落する
- interactor の helper method が参照する型への edge が欠落する
- catalogue 宣言済みの struct (例えば validating constructor を持つ value_object) で
  `expected_methods` を declare できず、method edge が graph に出ない

これは §1 の schema 非均質性が引き起こす下流症状である。M1 + S2 で解消するのは
「catalogue 宣言済みの struct で `expected_methods` を declare できない / renderer が
method edge を render しない」問題であり、catalogue 宣言済みの型の orphan の大半が
自然に解消する。catalogue 自体が未宣言の型 (例: `ComplianceContext` のように別 track
で catalogue 起草対象となっているもの) は本 ADR の scope 外で、catalogue 起草が別途
必要になる。

2026-04-27 の adr-decision-traceability-lifecycle トラックで生成された `contract-map.md` の
orphan ノード群はこの副作用の具体例である。

---

以上の 3 つは独立した問題ではない。§1 の schema 非均質性が §2 → §3 → §4 の
順に影響を派生させている連鎖構造である。本 ADR はこの連鎖の根本 (schema 均質化 +
新 kind 追加 + linter framework 導入) を決定する。

## Decision

### M1: struct 系 9 kind に `expected_members` + `expected_methods` を均質化する

#### 変更内容

`TypeDefinitionKind` の struct 系 9 kind すべてに `expected_members` と
`expected_methods` を持たせる。例外なし — `ValueObject` も含めて uniform にする。

```rust
// <!-- illustrative, non-canonical -->
ValueObject {
    expected_members: Vec<MemberDeclaration>,
    expected_methods: Vec<MethodDeclaration>,  // 新設 (現行はなし)
},
UseCase {
    expected_members: Vec<MemberDeclaration>,
    expected_methods: Vec<MethodDeclaration>,  // 新設
},
Interactor {
    expected_members: Vec<MemberDeclaration>,
    expected_methods: Vec<MethodDeclaration>,  // 新設
    declares_application_service: Vec<String>,
},
// Dto / Command / Query / Factory / SecondaryAdapter も同様に expected_methods を追加
```

#### 関心分離の原則

「何を書けるか」は schema が担う。「何を書くべきか」は convention と linter (S3) が担う。
たとえば `value_object` に behavior method を書く表現能力を与えることと、
「`value_object` には behavior を書かない」という意味論ポリシー (value_object semantic restriction) は別の関心である。
schema で policy を内包するより、linter で policy を enforce する方が
opt-out 経路を確保しやすく、プロジェクト間の柔軟性も保てる。

#### TypeGraph / baseline schema は変更不要

既存の `TypeNode::members` / `TypeNode::methods` および
`TypeBaselineEntry::members` / `TypeBaselineEntry::methods` がすでに存在するため、
TypeGraph / baseline は現行構造のまま均質化した catalogue field を受け取れる。
ADR `2026-04-26-0855` の Core invariant (catalog / TypeGraph / baseline 同時更新) は
既存フィールドの再利用で満たされる。

#### 既存 catalogue の扱い

本 ADR が適用される時点以降に authored される **新規 track の catalogue** で
uniform schema を採用すれば足りる。他 track の既存 catalogue を一括変換する作業は
不要 (project 方針として backward compat は持たない / track 跨ぎ整合は非推奨)。
serde codec も新 schema 専用とし、旧 schema を読む経路は持たない。

### S1: `domain_service` kind を新設する

#### 動機

M1 で全 struct 系 kind に `expected_methods` が付くと、
`value_object` に behavior method を書くことが技術的には可能になる。
しかし value_object semantic restriction (validated immutable value のみを value_object に置く) を守るためには、
behavior を持つ domain struct の正しい住所が別途必要になる。

`domain_service` を新設することで次の問題を解消する:

- `ComplianceContext` 級の behavioral struct が意味論的に正しい kind を得る
- validating constructor の error 型宣言 (`fn new() -> Result<Self, FooError>`)
  を `expected_methods` に書ける場所が domain に生まれる
- field と behavior method を持つ domain struct の住所が domain 層に生まれる
  (DDD domain service パターンを含む。stateless なゼロフィールド struct は `free_function` を使う)

#### schema 定義

```rust
// <!-- illustrative, non-canonical -->
DomainService {
    expected_members: Vec<MemberDeclaration>,
    expected_methods: Vec<MethodDeclaration>,
}
```

#### 配置層

| kind | domain | usecase | infrastructure | 配置根拠 |
|---|---|---|---|---|
| `domain_service` | ✓ | △ | ✗ | struct + members + methods を持つ domain logic の入れ物。usecase 配置は trans-domain な application logic の場合のみ要根拠 |

`Interactor` との違い: `domain_service` には `declares_application_service` がない
(trait 実装を持たない)。これが `interactor` との排他的境界である。

#### kind 判定基準

以下をすべて満たす場合に `domain_service` を採用する:

- struct であり `expected_members` に 1 件以上の field を持つ
  (zero-field なら zero-field struct → free_function 移行ルールで `free_function`)
- `expected_methods` に 1 件以上の behavior method を持つ
  (method なしなら `value_object` か別 kind)
- 状態遷移を持たない (持つなら `typestate` cluster)
- `application_service` / `secondary_port` を実装しない
  (実装するなら `interactor` / `secondary_adapter`)
- 集約・entity 構築が主目的ではない (主目的なら `factory`)
- domain 層に配置 (usecase は trans-domain な application logic の場合のみ要根拠)

#### 現行 codebase の実例

`libs/domain/src/skill_compliance/mod.rs:17` の `ComplianceContext { skill_match }`
+ `render() -> Option<String>` が `domain_service` に分類されるべき型である。
現状は catalogue 宣言自体がなく、別 track で起草対象となる。

#### TypeGraph / baseline schema は変更不要

M1 と同じ理由。`TypeNode::members` / `TypeNode::methods` を再利用できるため、
Core invariant の「schema 4 点同時更新」の重さは発生しない。

### S2: contract-map renderer の `methods_of()` を全 struct kind 対応に拡張する

#### 現行の問題

`libs/domain/src/tddd/contract_map_render.rs` line 392 周辺の `methods_of()` は:

```rust
// <!-- illustrative, non-canonical -->
// 現行: 3 種だけ method を返す特例分岐
fn methods_of(kind: &TypeDefinitionKind) -> Vec<&MethodDeclaration> {
    match kind {
        TypeDefinitionKind::SecondaryPort { expected_methods }
        | TypeDefinitionKind::ApplicationService { expected_methods } => {
            expected_methods.iter().collect()
        }
        TypeDefinitionKind::SecondaryAdapter { implements, .. } => {
            implements.iter().flat_map(TraitImplDecl::expected_methods).collect()
        }
        _ => Vec::new(),  // 残り 10 種は空返し
    }
}
```

M1 で全 struct 系 kind が `expected_methods` を持つようになるため、
`SecondaryPort` / `ApplicationService` および struct 系 8 kind の arm は 1 つに統合できる。
`SecondaryAdapter` は top-level `expected_methods` と `implements[].expected_methods` の
2 source merge が必要なため、専用 arm を残す。

#### 変更方針

`SecondaryPort` / `ApplicationService` / struct 系 8 kind (M1 で `expected_methods` が
付く種) および S1 新設の `DomainService` は top-level `expected_methods` を uniform に
返す 1 つの arm で処理する。

`SecondaryAdapter` は意図的に別 arm として残す。理由は M1 で top-level
`expected_methods` が新設される一方、`implements[].expected_methods` (各 trait impl
のメソッド) も保持するため、struct 自身の inherent method と trait impl method の
両方を catalogue に書ける **2 source merge** が必要だからである。これは他の struct 系
kind にはない `SecondaryAdapter` 固有の設計で、特例分岐ではなく意図的な専用 arm である。

まとめると: **struct 系 8 kind は uniform / `SecondaryAdapter` のみ 2 source merge の
意図的な専用 arm を持つ**。

```rust
// <!-- illustrative, non-canonical -->
// 変更後: struct 系 8 kind は uniform arm / SecondaryAdapter は 2 source merge 専用 arm
fn methods_of(kind: &TypeDefinitionKind) -> Vec<&MethodDeclaration> {
    match kind {
        TypeDefinitionKind::SecondaryPort { expected_methods }
        | TypeDefinitionKind::ApplicationService { expected_methods }
        | TypeDefinitionKind::Typestate { expected_methods, .. }
        | TypeDefinitionKind::ValueObject { expected_methods, .. }
        | TypeDefinitionKind::UseCase { expected_methods, .. }
        | TypeDefinitionKind::Interactor { expected_methods, .. }
        | TypeDefinitionKind::Dto { expected_methods, .. }
        | TypeDefinitionKind::Command { expected_methods, .. }
        | TypeDefinitionKind::Query { expected_methods, .. }
        | TypeDefinitionKind::Factory { expected_methods, .. }
        | TypeDefinitionKind::DomainService { expected_methods, .. } => {
            expected_methods.iter().collect()
        }
        TypeDefinitionKind::SecondaryAdapter { expected_methods, implements, .. } => {
            // M1 で追加された top-level expected_methods と
            // implements[].expected_methods の両方を返す (2 source merge)
            expected_methods.iter()
                .chain(implements.iter().flat_map(TraitImplDecl::expected_methods))
                .collect()
        }
        _ => Vec::new(),  // Enum / ErrorType / FreeFunction は別経路
    }
}
```

mermaid の method edge 表現は既存の `-->|.method_name()|` をそのまま再利用する。
field edge (`-->|.field_name|`) は ADR `2026-04-26-0855` §D2 の自然な拡張として
`expected_members` から生成する (renderer 拡張は同じ実装 track 内の作業)。

### S3: type catalogue linter framework の導入を決定する

#### 本 ADR の扱い

本 ADR は framework の **必要性と default rule の方針** を決定する。
具体的な設計 (config schema / rule DSL / CLI 統合 / type-signals との関係) は
後続の専用 ADR および実装 track の対象とする。

#### 必要性

M1 で全 struct 系 kind に `expected_methods` が付くと、
type-designer が `value_object` に behavior method を宣言できるようになる。
これは value_object semantic restriction の機械的 enforce がないと
convention だけでは守れないことを意味する。

convention の各ルール (kind 配置層マトリクス / value_object semantic restriction /
No Fallback 等) は agent self-reject / reviewer / type-signal evaluator の
多層防御で支えられているが、「`value_object.expected_methods` に何を書いてはいけないか」
という意味論ポリシーを機械的に reject する経路が現状にない。

linter framework を導入することで、convention ルールを機械的に enforce する
経路を確立し、project ごとの custom rule 追加・rule 緩和も可能にする。

#### linter framework が提供する rule primitive

linter framework が default で提供する rule primitive を 3 種に整理する。具体的な default rule
(`value_object` の `expected_methods` 空強制 等) はこれらの primitive を組み合わせて表現する。
primitive の DSL / config schema 自体の設計は別 ADR / 別 track の対象とする。

##### 3 種の primitive

1. **field empty enforcement**: 特定 kind の特定 field が空であることを強制する
   - 例: `value_object` の `expected_methods` を空配列に強制 (behavior 禁止 / value_object semantic restriction の lint 化)

2. **field non-empty enforcement**: 特定 kind の特定 field が非空であることを強制する
   - 例: `interactor` の `declares_application_service` に少なくとも 1 件存在することを強制

3. **kind-layer constraint**: ある kind が指定 layer でのみ宣言できることを強制する
   - 例: `domain_service` は domain または usecase 層のみ (infrastructure は禁止)
   - 例: `secondary_port` は domain または usecase 層のみ
   - kind 配置層マトリクスの forbidden 組合せ全般をこれで一括表現する

##### 3 primitive で表現できること

これら 3 primitive を組み合わせれば、convention の kind 配置層マトリクス /
value_object semantic restriction / kind 固有制約 (例: interactor は application_service を必ず実装)
を機械的に enforce する経路に乗せられる。No Fallback ルール (どの kind も fit しないとき
value_object 等に押し込まない) は primitive では直接表現できないため、別途 type-designer 側の
ロジックで担当する。

##### 現行 codec が既に enforce しているもの

`interactor` の `declares_application_service` 非空制約は現行 codec が既に enforce している。
本 framework に取り込めば codec 側の特殊実装を linter rule に統一できる。

#### customizability の確保

rule は project config で disable / 緩和 / 拡張できるようにする。
たとえば `pub struct Email(pub String)` のような Rust idiom の newtype tuple struct で
`pub` な inner field を `expected_members` に宣言したい場合、
rule によっては false positive になり得る。opt-out 経路を設ける。

## Rejected Alternatives

### A1: schema レベルで `value_object` を `expected_methods` のない構造に固定する (M1 の代替)

`value_object` だけ `expected_methods` を持たせない非対称 schema を維持する案。

**却下理由**: schema が semantic policy を内包する形になる。「schema = 表現能力」
「linter = 意味論ポリシー」という関心分離の原則に反する。Rust idiom の newtype tuple
struct を許容したいプロジェクトが opt-out できなくなる。S3 の linter framework で
同じ default 効果を保ちながら customizability を確保できるため、不要な制約を schema に
持ち込む必要はない。

### A2: `Vec<MemberDeclaration>` に method 種別を埋め込む (M1 の代替)

`MemberDeclaration::Method { ... }` を既存の `Variant` / `Field` と同じ列に追加して
`expected_members` に method も混ぜる案。

**却下理由**: 既存の `MemberDeclaration` は「composite type の member (enum variant
または struct field)」という論理的に閉じた enum である。method はカテゴリが根本的に
異なる。混在させると schema の概念整合性が壊れ、renderer 側でも種別 dispatch が
複雑になる。`expected_members` と `expected_methods` を別フィールドとして並べる方が
意味論が明確で実装も素直である。

### A3: linter framework を導入せず convention の各ルール (kind 配置 / value_object semantic restriction / No Fallback 等) のみで運用する (S3 の代替)

M1 適用後も機械的 enforce を諦めて convention と人間レビューに任せる案。

**却下理由**: M1 によって `value_object` に behavior method を書けるようになると、
その表現力を持った上で「書かない」という運用を convention だけで維持するのは
難易度が上がる。agent self-reject / reviewer の防衛線は convention テキストを読む能力
に依存しており、テキスト読み取りミスが起きたとき機械的な backstop がない。
また project ごとに rule を調整する経路もない。linter framework は
custom rule per project の追加経路も兼ねる。

## Consequences

### 良い影響

- struct kind 均質化 (M1) により renderer の `methods_of()` で struct 系 8 kind は
  uniform arm 1 つに集約でき、コードが単純になる。`SecondaryAdapter` は 2 source merge
  (top-level `expected_methods` + `implements[].expected_methods`) の意図的な専用 arm を
  維持する。
- validating constructor の error 型 (`AdrFrontMatterError` 等) を全 struct 系 kind で
  `expected_methods` 経由で declare できるようになる。
- domain layer の behavioral struct (`ComplianceContext` 級) が `domain_service` (S1) で
  意味論的に正しい kind を得る。
- renderer の均質化 (S2) により、全 struct 系 kind の method edge が contract-map に出る。
  **副作用として catalogue 宣言済みの型に起因する contract-map orphan の大半が解消する**
  (§4 の自然解消)。catalogue 自体が未宣言の型の orphan は本 ADR の scope 外。
- linter framework (S3) により convention の各ルール (kind 配置層マトリクス / value_object semantic restriction / No Fallback 等) を機械的に enforce する経路が開ける。
- M1 + S2 は ADR `2026-04-26-0855` §D2 の「struct 系 9 種に field edge を発生させる」
  決定を完全に実現する前提条件でもある。

### 悪い影響・トレードオフ

- **linter 設計コスト**: S3 は本 ADR では decision のみ。config schema / rule DSL /
  CLI 統合の設計と実装は別 ADR / 別 track の subject になる。linter 未実装の期間は
  convention + 人間レビューの多層防御で継続する。
- **kind 数増加**: S1 で kind が 13 → 14 になる。type-designer の kind 判定で No Fallback ルールの
  候補が 1 つ増える。convention の kind 配置層マトリクス / value_object semantic restriction /
  No Fallback ルール / Examples / Review Checklist への `domain_service` 行追加が必要。
- renderer 出力 (`contract-map.md` / `<layer>-types.md`) の手編集禁止運用は引き続き
  必須 (DO NOT EDIT DIRECTLY marker を維持する)。

## Reassess When

- linter framework (S3) が未実装のまま長期化した場合: convention SSoT のみの運用が
  実際に機能しているか確認し、別経路を検討する。
- `domain_service` kind の使用例が極端に少ない場合: kind 削除を検討
  (`value_object` の linter rule customize で代替できるか確認)。
- M1 + S2 で均質化された全 struct 系 kind の method edge が contract-map を読みにくく
  するほど密になった場合: render filter の強化を検討する。
- `value_object` の default linter rule (expected_methods 空配列強制) を opt-out する
  project が多数出た場合: default rule の見直しを検討する。
- struct 系以外の kind (enum / error_type / trait 系 / free_function) でも均質化に類する
  schema 変更が必要になった場合: 本 ADR の scope 外として別 ADR で扱う。

## Related

- `knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md` —
  本 ADR の前駆。`expected_members` 必須化と Core invariant (catalog / TypeGraph /
  baseline 同時更新) を確立。M1 / S2 はそこで決定された §D2 の完全実現に必要な前提。
- `knowledge/adr/2026-04-17-1528-tddd-contract-map.md` —
  Contract Map 設計の元 ADR。本 ADR §4 の派生問題 3 (orphan) の発生コンテキスト。
- `knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md` —
  `TypeDefinitionKind` taxonomy の ADR。S1 の `domain_service` 追加はここで決定された
  taxonomy の延長。
- `knowledge/conventions/type-designer-kind-selection.md` —
  M1 / S1 / S3 が補完する convention。kind 配置層マトリクスへの `domain_service` 行追加、
  value_object semantic restriction の linter 移行 (S3)、No Fallback ルールの選択肢拡張は
  convention 側で別途更新が必要。
