---
required_for:
  - type-designer
  - rollback-diagnoser
---

# Type-Designer Kind Selection Convention

## Purpose

`type-designer` agent が `<layer>-types.json` を起草する際、role 選定ミスや層配置違反を agent 自身で構造的に防ぐための拘束ルール集。

このハーネスにおける型設計は type-designer の専門領域であり、orchestrator / user が事後に role 選定ミスを指摘して redesign を迫る運用は逆転している。本 convention は type-designer が **自律的に正しい role を選び、誤った fallback を避ける** ための判断基準を SSoT として明示する。

過去のセッションで観察された type-designer の典型逸脱:

- 状態遷移ありの型に `role: ValueObject` + `kind: { "kind": "enum" }` (status field + Option<...>) を選び typestate pattern を回避
- application の性質以外の layer に `role: UseCase` / `role: ApplicationService` / `role: Interactor` を配置
- ゼロフィールド struct + 1 method の型を `role: ValueObject` で「validated value」の意味から大きく外して使用
- 他の role が fit しないときに `role: ValueObject` を catch-all として採用 (semantic stretch)

## Scope

- 適用対象:
  - `type-designer` agent
  - すべての TDDD 対応層 (`architecture-rules.json` の `tddd.enabled: true`) における `<layer>-types.json` の起草・更新
  - 各 entry の `role` 選定、`expected_*` フィールド設計、層配置判断
- 適用外:
  - `spec-designer` / `impl-planner` / `adr-editor` の owned artifact
  - role が確定済みで構造変更を伴わない `action: "modify"` 編集 (フィールド追加など) ※ role 変更を含む場合は本 convention 対象

> **強制先**: review 観点 — types scope

## Rules

### R1. Role-Layer Compatibility (role × layer 性質マトリクス)

`<layer>-types.json` の各 entry は、role と層の性質の組合せを以下の表に従う。Forbidden の組合せを起草してはならない。層の性質は crate 名ではなく、アーキテクチャ上の責務で判断する。

> **強制先**: 機械 lint — bin/sotp catalogue-lint check-active-track

#### 層の性質と layer id の解決

表の列は、特定の crate 名や固定された layer id ではなく、次の性質を表す。

- `innermost`: 外側の層へ依存せず、業務上の不変条件と domain model を所有する最内層
- `application`: application operation を組み立て、内側の model と port を使う層
- `driven adapter`: 内側が定義した port を実装し、ファイル・外部サービスなどの外部資源を扱う層
- `driving adapter`: 外部入力を受け、application operation を呼び出し、外向きの結果を表現する層
- `composition root`: adapter と port の object graph を組み立てる層。業務上の振る舞いは所有しない

実行可能 crate の薄い process entrypoint は、この五つの性質に追加する第六の列ではない。`architecture-rules.json` に宣言された binary crate が、引数を解析し、composition root または adapter を呼び出し、最終結果を process の終了コードへ変換するだけで、独自の application operation・adapter 実装・object graph を所有しない場合は process entrypoint として扱う。これは起動シェルの分類であり、R1 の五つの層の性質への割当ではない。process entrypoint の catalogue entry は、実際に委譲先へ属する責務を持つ場合にだけ、その委譲先の性質で R1 を適用する。

> **強制先**: review 観点 — types / harness-policy scope

実際の layer id と `<layer>-types.json` の対応は、`architecture-rules.json` の `layers[]` 宣言を参照して定める。`architecture-rules.json` は layer id・path・依存方向を宣言するが、層の性質を自動解決する機械写像ではない。`layers[].crate` の名前や path の語だけから性質を推測せず、依存方向と宣言された責務を根拠にして reviewer が性質を判定する。consumer が層を改名・分割・統合した場合は、該当する性質の列を選ぶだけでなく、literal な layer id を持つ lint config も別途更新・検証する。

> **強制先**: review 観点 — types / harness-policy scope

> **v5 schema (schema_version=5) の対応**: 現行 catalogue は `schema_version: 5` で、 **role 軸 × kind 軸** の 2 軸構造を採る。 type-designer は v5 format で `<layer>-types.json` を起草する。 本マトリクスの「role」列は **role フィールドの値** に対応する (`DataRole` / `ContractRole` / `FunctionRole` の variant 名)。 type-designer は role と layer の性質の組合せを本マトリクスで確認する。
>
> - v5 wire format: `schema_version: 5`, `crate_name`, `layer`, `types: {}` (TypeEntry), `traits: {}` (TraitEntry), `functions: {}` (FunctionEntry), `inherent_impls: []` / `trait_impls: []` の 2 つの top-level array。 `trait_impls` は `action` / `trait_ref` / `for_type` を持つ独立 entry (`TraitImplDeclV2`); `inherent_impls` は `action` を持たず `type_name` / `impl_generics` / `impl_where_predicates` / `methods` を持つ (`InherentImplDeclV2`、 target type への帰属で識別される)。 codec は v1–v4 を fail-closed で reject する (v4 は `SchemaVersionRequiresMigration` で migration prompt を返す)。
> - v5 roles: `types` エントリは `role: DataRole` (**17 値**: `ValueObject` / `Entity` / `AggregateRoot` / `DomainService` / `UseCase` / `EventPolicy` / `DomainEvent` / `Specification` / `Factory` / `Interactor` / `Command` / `Query` / `Dto` / `ErrorType` / `SecondaryAdapter` / `CompositionRoot` / `PrimaryAdapter`)、 `traits` エントリは `role: ContractRole` (**4 値**: `SpecificationPort` / `ApplicationService` / `SecondaryPort` / `Repository`)、 `functions` エントリは `role: FunctionRole` (2 値: `FreeFunction` / `UseCaseFunction`)。 `DataRole` / `ContractRole` のうち `ValueObject` / `Entity` / `AggregateRoot` / `DomainService` / `UseCase` / `EventPolicy` / `Repository` は **data-carrying variant** (payload field を持つ) で、 wire format は discriminated-object 形式 (例: `{ "EventPolicy": { "reacts_to": ["OrderPlaced"] } }`)。 旧 plain-string 形式 (`"role": "ValueObject"`) は codec が parse error として reject。
> - v5 kind (構造軸): `types` は `kind: { "kind": "struct" | "enum" | "type_alias", ... }` で記述する
> - 旧 v2 の `type_definitions` / `TypeDefinitionKind` は廃止済みで、codec は受け付けない。
>
> 本マトリクスは **層配置** の制約のみを規定する。各 role / entry に必要な具体的フィールド (`kind`, `methods` 等) および top-level の `trait_impls` / `inherent_impls` (impl block を独立 entry として持つ array) は `.harness/reference/catalogue-schema.md` を参照する。対応する symbol は `TypeEntry` / `TraitEntry` / `FunctionEntry` / `TraitImplDeclV2` / `InherentImplDeclV2` / `CatalogueDocument` である。

> **強制先**: 機械 lint — bin/sotp catalogue-lint check-active-track

| role (v5) | innermost | application | driven adapter | driving adapter | composition root | 配置根拠 |
|---|---|---|---|---|---|---|
| `ValueObject` (DataRole) | △ | △ | △ | ✗ | ✗ | 配置はユビキタス言語、不変条件、複数 operation を越えた意味の安定性、persistence / delivery / workflow 都合からの独立性で判断する。innermost 内部の inbound 参照は補助証拠であり必須条件ではない |
| `Entity` (DataRole) | ✓ | ✗ | ✗ | ✗ | ✗ | entity は domain 概念。他の性質の層での使用は domain leak |
| `AggregateRoot` (DataRole) | ✓ | ✗ | ✗ | ✗ | ✗ | aggregate root は domain 概念 |
| `DomainService` (DataRole) | ✓ | △ | ✗ | ✗ | ✗ | domain knowledge を集約する behavior 中心 struct。application は trans-domain な application logic で要根拠 |
| `EventPolicy` (DataRole) | **✓ ONLY** | ✗ | ✗ | ✗ | ✗ | event-driven policy。innermost のみ許可。payload に `reacts_to: NonEmptyVec<TypeRef>` を持ち、DomainEvent 役の型のみ参照可 |
| `DomainEvent` (DataRole) | **✓ ONLY** | ✗ | ✗ | ✗ | ✗ | aggregate が emit する事実。innermost のみ。enum 形 / unit struct どちらも可。mutation surface (`&mut self` / public field) は linter が禁止 |
| `Specification` (DataRole) | ✓ | ✗ | ✗ | ✗ | ✗ | domain predicate。他の性質の層は domain leak |
| `Factory` (DataRole) | ✓ | ✓ | △ | ✗ | ✗ | 集約 / entity factory。driven adapter に置くのは要根拠 |
| `UseCase` (DataRole) | ✗ | **✓ ONLY** | ✗ | ✗ | ✗ | name と意味が application を表す。他の性質の層は役割違反 |
| `Interactor` (DataRole) | ✗ | **✓ ONLY** | ✗ | ✗ | ✗ | ApplicationService trait の実装。application に置く |
| `Command` (DataRole) | ✗ | **✓ ONLY** | ✗ | ✗ | ✗ | CQRS command。application が受け取る入力 |
| `Query` (DataRole) | ✗ | **✓ ONLY** | ✗ | ✗ | ✗ | CQRS query。application が受け取る入力 |
| `Dto` (DataRole) | ✗ | △ | **✓** | ✓ | ✗ | serde 境界は driven adapter に置き、innermost は serde-free に保つ。application は要根拠。driven / driving adapter では入力 DTO・出力 DTO に使用 |
| `ErrorType` (DataRole) | ✓ | ✓ | ✓ | ✗ | ✓ | layer-flexible (各性質の層がそれぞれの責務に応じた error 型を持つ)。driving adapter は宣言した outcome 型で結果を返す場合、別の ErrorType を持たない |
| `SecondaryAdapter` (DataRole) | ✗ | ✗ | **✓ ONLY** | ✗ | ✗ | driven port の実装は driven adapter に置く |
| `CompositionRoot` (DataRole) | ✗ | ✗ | ✗ | ✗ | **✓ ONLY** | object graph を組む純 DI の住所。composition root のみ |
| `PrimaryAdapter` (DataRole) | ✗ | ✗ | ✗ | **✓ ONLY** | ✗ | driving adapter (invoke + render)。公開シグネチャは application の `Command` / `Query` / boundary `Dto` / application の `ValueObject` を参照してよい。innermost の `ValueObject` / `Entity` / `AggregateRoot` の直接露出、driven adapter / transport 型の漏出は不可。`ValueObject` の innermost / application 分類は R1 の semantic evidence で判定する review rule であり、role 名だけでは機械判定しない。したがって role-only catalogue lint は `ValueObject` を禁止しない |
| `SpecificationPort` (ContractRole) | ✓ | ✗ | ✗ | ✗ | ✗ | innermost の仕様を表す port |
| `SecondaryPort` (ContractRole) | ✓ | ✓ | ✗ | ✗ | ✗ | innermost / application のいずれにも置ける driven port |
| `ApplicationService` (ContractRole) | ✗ | **✓ ONLY** | ✗ | ✗ | ✗ | application interface |
| `Repository` (ContractRole) | ✓ | ✗ | ✗ | ✗ | ✗ | aggregate root の永続化 port (data-carrying: payload に `aggregate: TypeRef` を持ち、参照先は AggregateRoot 役で宣言)。aggregate の語彙で説明されるため innermost に置く |
| `FreeFunction` (FunctionRole) | ✓ | ✓ | ✓ | ✓ | ✗ | layer-flexible (top-level pub fn)。composition root は配線を CompositionRoot のメソッドとして書くため pub free function が生じない |
| `UseCaseFunction` (FunctionRole) | ✗ | **✓ ONLY** | ✗ | ✗ | ✗ | use-case entrypoint function。application に置く |

凡例: `✓` = OK, `△` = 要根拠 (default ではない、docs フィールドに根拠を記録。`ValueObject` は docs または review 可能な track 記録)、`✗` = forbidden, `**ONLY**` = この性質以外で使うことを禁止

薄い process entrypoint は、名前だけで driving adapter または composition root に分類してはならない。単に別の adapter / composition root を呼び出すだけの entrypoint は R1 の層の性質を自称せず、独立した application / adapter / wiring role を追加してはならない。entrypoint が入力変換、結果表現、または object graph 構築の責務を実際に所有する場合だけ、その責務を対応する driving adapter または composition root の catalogue entry として表現する。

> **強制先**: review 観点 — types / harness-policy scope

R1 の五つの層の性質に分類された layer の role には、出荷 catalogue-lint config の `KindLayerConstraint` がある。各 rule の `permitted_layers` は literal な layer id を列挙するだけで、`architecture-rules.json` から層の性質や許可 id を自動解決する resolver ではない。そのため、表の `✓` と `△` に対応する literal id と、その補集合である `✗` の拒否対象が R1 に一致することは、`architecture-rules.json` の宣言と突き合わせて reviewer が確認する。process entrypoint の layer について既存 lint が許可する `Dto` / `FreeFunction` / `ErrorType` は、起動シェルの境界表現に対する literal な例外であり、R1 の五列のいずれかへの対応を意味しない。reviewer はその例外について、process entrypoint が独自の application operation・adapter 実装・object graph を所有していないことも確認する。consumer が層を改名・分割・統合した場合、lint config は自動追随しないため、対応する literal id を更新・検証してから機械検査を enforcement として扱う。lint は `✓` と `△` の区別や根拠の妥当性を機械判定しない。

> **強制先**: review 観点 — types / harness-policy scope

`ValueObject` は innermost / application / driven adapter のいずれへ置く場合も、ユビキタス言語、不変条件の所有、複数 application operation を越えた意味の安定性、persistence・delivery・workflow 都合からの独立性を根拠として決める。same-track innermost 内部の inbound reference は model での利用を示す補助証拠として記録してよいが、その不在だけで拒否してはならない。application boundary にのみ意味を持つ値は application の `Dto` / `Command` / `Query` / `ValueObject` として置く。配置の semantic classification と根拠は catalogue の `docs` または track の review 記録に残し、reviewer が照合する。

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli_driver scope

五つの層の性質に分類された layer で `✗` または **ONLY** を破る role × layer 性質の選択は、`bin/sotp signal calc-impl-catalog` の signal 評価以前に **role 違反** として draft 段階で却下する。process entrypoint の literal な shell allowance はこの role × layer 判定ではなく、直前の process entrypoint 規則に従って判定する。

> **強制先**: 機械 lint — bin/sotp catalogue-lint check-active-track

#### Port placement tie-break

port が innermost の不変条件または aggregate の語彙で説明できるなら innermost に置く。application のオーケストレーションが必要とする技術的能力なら application に置く。たとえば aggregate の永続化は innermost の `Repository`、レビュー実行や差分取得の能力は application の `SecondaryPort` として分類する。

> **強制先**: review 観点 — types / domain / usecase scope

R7 (Cross-Track Port Reference) も参照すること: top-level `trait_impls` のうち `for_type` が `SecondaryAdapter` 型を指す entry の `trait_ref` が参照する port が当該 track の catalogue に未 declare の場合、`-.impl.->` edge が silently skip される。

> **強制先**: review 観点 — types scope

#### CQRS separation evidence

`Command` と `Query` を別 Interactor / ApplicationService に分離するのは、side effect、required collaborator、possible error、consistency boundary、または read/write model の少なくとも一つに操作固有の実質的な非対称性がある場合だけである。分離する catalogue は、該当次元、具体的な操作差、分離根拠を `docs` または review 可能な track 記録に残す。read と write の両方があることや role が利用可能なことだけでは分離理由にならない。

> **強制先**: review 観点 — types / usecase scope

#### Driver injection and facade prohibition

入力 port は 1 ユースケースにつき 1 trait とし、実行メソッドを 1 つだけ持つ。driver の注入粒度はこの port 粒度に合わせ、driver は自分が消費する複数の単能 port をそれぞれ直接受け取ってよい。「driver は 1 つの interactor だけを注入する」という制約は置かない。

> **強制先**: review 観点 — types / usecase / cli_driver / cli_composition scope

command と query を混載する `*Service` などの facade port を新設してはならない。この禁止は未移行の文脈にも適用する。既存の facade port や既存の単一 interactor 注入は、この規約だけを理由に遡及改修しない。

> **強制先**: review 観点 — types / usecase / cli_driver / cli_composition scope

#### Validated Command / Query boundary

- 新規コードの usecase 入力 boundary は、command usecase では検証済みの usecase 所有 `Command` 型を 1 個だけ、query usecase では検証済みの usecase 所有 `Query` 型を 1 個だけ受け取る。未検証の `String` などを入力 boundary の公開シグネチャに置いてはならない。

  > **強制先**: review 観点 — types / usecase / cli_driver scope
- `String` から対応する `Command` または `Query` へのパースと検証は usecase 所有の boundary 型が担う。CLI の driving path（規約上の `cli`）はそのパースを一度だけ呼び出してから対応する入力 boundary を呼び出し、得られた検証済み `Command` または `Query` を渡す。現行の層構成ではこの責務を `cli_driver` が担い、薄い `cli` bin は `cli_driver` を呼び出すだけで usecase crate に直接依存しない。

  > **強制先**: review 観点 — types / usecase / cli / cli_driver scope
- domain enum の鏡像を cli 側に定義してはならない。boundary の語彙は usecase 所有の boundary 型に統一し、`cli` と `cli_driver` は domain 型を知らないという原則を維持する。

  > **強制先**: review 観点 — types / usecase / cli / cli_driver scope
- 既存の境界実装は、この規約だけを理由に遡及改修しない。

  > **強制先**: review 観点 — types / usecase / cli / cli_driver scope

### R2. Free Function Preference (stateless behavior は FreeFunction)

以下の条件をすべて満たす型は `role: FreeFunction` (`functions` エントリ) で起草する。zero-field struct + 1 method を `role: ValueObject` / `role: UseCase` に matching するのは禁止する。

- top-level の pub fn (struct や trait の method ではない)
- またはゼロフィールド struct で、その「struct」が表す唯一の責務が 1 つの pub fn 呼び出しに帰着する
- 内部 state を持たない (struct field なし、または `()` のみ)
- 依存注入を必要としない (依存ありなら、まず structure-required port か任意の service-level 抽象かを分類する。structure-required port は D2 の必要性テストではなく支配するアーキテクチャ規則に従い、任意の service-level 抽象は D2 の条件に従う。application の任意の service-level 抽象は、条件成立時だけ `role: Interactor` + `role: ApplicationService` とし、その他は `role: UseCase` の具体型を既定とする。driven adapter は `role: SecondaryAdapter`)

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli_driver scope

判定例:

- `parse_yaml_frontmatter(input: &str) -> Result<AdrFrontMatter, AdrFrontMatterCodecError>` → `role: FreeFunction` (state なし、依存なし)
- `evaluate_adr_decision(entry: &AdrDecisionEntry) -> AdrSignal` → `role: FreeFunction`
- `EvaluateAdrDecision { /* zero fields */ } impl { fn evaluate(&self, ...) -> ... }` → 設計を `role: FreeFunction` に折り畳む。zero-field struct は wrapping だけで意味を加えない

#### 必要駆動の抽象

候補はまず、(1) アーキテクチャが要求する structure-required port とその実装の組か、(2) その組の上に層の内部で任意に重ねる service-level 抽象かを分類する。ユースケース自身の入力ポートは、1 ユースケース 1 trait・実行メソッド 1 つの `ApplicationService` inbound port と、その `Interactor` 実装からなる structure-required な組であり、複数実装や service 自体のテスト差し替えがなくても D2 の必要性テストを適用せず、R1 / D3 の配置・粒度規則に従って導入する。層を越える依存を表す `SecondaryPort` と aggregate の永続化を表す `Repository` も structure-required ports であり、必要性テストではなく支配するアーキテクチャ規則に従って導入する。

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli_driver scope

一方、structure-required な入力ポートの組の上に同じ service を共有するためだけに第二の trait と実装を重ねる場合、またはその他の層内 service-level 抽象を追加する場合は、D2 の必要性テストの対象である。(a) 複数の実装が現存する、または (b) service 自体をテスト境界で差し替える必要がある場合だけ導入し、共有所有だけなら `Arc<具象型>` を既定とする。条件が後から成立した時点で trait を切り出す。既存の抽象ペアは改訂後の規約に合わせて遡及解体しない。structure-required な `ApplicationService`、`SecondaryPort`、`Repository` の port 自体やその必要な実装を、単一実装だからという理由で省略してはならない。

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli_driver scope

### R3. ValueObject Semantic Restriction

`role: ValueObject` は値等価で識別される値を表す。自身の値から新しい値または述語を導出する side-effect-free な method は許容する。一方、依存または外部リソースを扱う behavior 中心の service 的 struct は ValueObject ではない。

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli_driver scope

| OK (ValueObject) | NG (ValueObject 違反) |
|---|---|
| `Email(String)` newtype + `new()` で形式検証 | `parse_*` のように外部表現を解釈する service 的 struct |
| `Money::add` / `DateRange::overlaps` のように値から値・述語を導出する method | `Codec` / `Validator` / `Resolver` のように依存または外部 resource を扱う struct |
| `AdrDecisionCommon { id, refs, ... }` 検証付き shared payload | `Codec` / `Validator` / `Resolver` のような behavior 中心の struct |
| 複合 primitive を集めた読み取り専用の record | trait 実装を意図する struct (→ `role: Interactor` / `role: SecondaryAdapter`) |

判定は構造条件より意味論を優先する。値等価で識別され、method がその値だけから値または述語を導出するなら ValueObject である。依存、外部 resource、または service の責務を中心にするなら ValueObject ではない。

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli_driver scope

behavior を持つ struct は以下のいずれかに振り分ける:

- 依存なし stateless → `role: FreeFunction` (R2)
- 依存あり (port を呼び出す) → application では、ユースケース自身の structure-required な inbound port の実装は `role: Interactor` とし、`role: ApplicationService` trait と組にする。structure-required な `SecondaryPort` / `Repository` も支配するアーキテクチャ規則に従う。structure-required な port とは別に任意の service-level 抽象を追加する場合は、共有所有だけなら `role: UseCase` の具体型を `Arc<具象型>` で扱い、複数実装または service 自体のテスト境界での差し替えが必要な場合だけ `role: ApplicationService` + `role: Interactor` の組を導入する。driven adapter では `role: SecondaryAdapter` (port 実装)
- 集約構築 → `role: Factory`
- 状態遷移あり → typestate cluster (`role: ValueObject` で各 state を typestate marker 付き `struct` として表現し、遷移メソッドを `methods` に宣言。wire format は `.harness/reference/catalogue-schema.md`「The `kind` field」節を参照)
- 値の同一性ではなく domain behavior を中心にする struct → `role: DomainService` (R6)

> **強制先**: review 観点 — types / domain / usecase / infrastructure scope

### R4. Kind Distribution Reconnaissance (起草前の偵察義務)

新規 catalogue の draft を書き始める前に、既存 track の catalogue から role 分布を調査して当 track の起草の参照基準にする。reconnaissance ステップ (baseline-capture → type-graph d1/d2 → Read) と並行して実施する。

調査内容:

- 既に完了済みの近接 track (同じ layer / 同じ ADR を参照) の `<layer>-types.json` を 1〜3 件 sample
- そこで採用されている role の分布 (どの role がどれだけ使われているか)
- naming convention (PascalCase struct / snake_case fn / `*Error` / `*Port` / `*Adapter` 等の suffix)
- `role: ValueObject` と `role: FreeFunction` の使い分け実例

この偵察により、特定 role を「思い出した順」で機械的に当てはめる代わりに、**プロジェクト全体の role 配分との整合** を保った起草が可能になる。偵察結果は internal preparation であり final report に出さなくてよい (orchestrator 出力には影響させない)。

例: ADR が「parse」「evaluate」のような stateless behavior を要求しているのに、過去 track で類似機能が `role: FreeFunction` で実装されている場合、当該 track でも `role: FreeFunction` を採用する。`role: UseCase` / `role: ValueObject` を選択した場合、その rationale を `docs` フィールドに記録する。

> **強制先**: review 観点 — types scope

### R5. No Fallback Rule (catch-all 禁止)

「他の role が完全に fit しない」という理由で `role: ValueObject` または `role: UseCase` を catch-all として採用してはならない。

> **強制先**: review 観点 — types scope

判断手順:

1. 候補 role を列挙し、R1 マトリクスで層と role の組合せを絞り込む
2. role が確定しない場合 → R2 (`FreeFunction`) と R3 (`ValueObject` 制限) を再確認
3. それでも確定しない場合 → R6 (`DomainService`) の判定基準で innermost の behavior の住所として fit するか確認
4. それでも確定しない場合 → 起草を止め、`## Open Questions` に「role が確定しない理由」と「検討した候補とその却下理由」を列挙して orchestrator に escalation
5. orchestrator は ADR / spec の補強 (adr-editor / spec-designer の re-invoke) または user 判断を仰ぐ

> **強制先**: review 観点 — types / harness-policy scope

`role: ValueObject` で迷ったときの最も多い真の答えは `role: FreeFunction` (R2) である。次に多いのは、依存を持つ application の任意 service の具体型としての `role: UseCase`、structure-required な inbound port の実装としての `role: Interactor` + `role: ApplicationService`、D2 の複数実装または service 自体のテスト差し替え条件が成立した任意 abstraction としての `role: Interactor` + `role: ApplicationService`、`role: SecondaryAdapter` (port 実装)、または `role: DomainService` (R6: field を持つ domain behavior) である。`role: ValueObject` を選ぶ前に、候補が structure-required な組か任意 abstraction かを分類する。

> **強制先**: review 観点 — types scope

### R6. DomainService Selection Criteria (domain behavior の住所)

値等価で識別され、side-effect-free な導出 method だけを持つ型は DomainService ではなく ValueObject (R3) である。`role: DomainService` は値ではなく domain behavior を中心にする struct の住所であり、structure-required な `ApplicationService` inbound port の実装、または D2 の条件を満たす任意 service-level abstraction の `role: Interactor` と混同しないため、以下の全条件を満たす場合に採用する。

採用条件 (AND):

- struct (enum / typestate cluster ではない)
- `kind.shape.fields` >= 1 field (state を保持する; ゼロフィールドは R2 の `FreeFunction` 候補)
- `methods` >= 1 entry (domain behavior を持つ; 導出 method だけなら R3 の `ValueObject` 候補)
- 状態遷移なし (ある場合は typestate pattern — R3 の振り分け)
- `ApplicationService` / `SecondaryPort` の実装ではない (structure-required な inbound port の実装は `role: Interactor`、任意 service-level abstraction が D2 の条件を満たして `ApplicationService` を実装する場合も `role: Interactor`、secondary port の実装は `role: SecondaryAdapter`)
- 配置層は innermost (default) / application (要根拠 — trans-domain な application logic で domain knowledge を集約する場合のみ、`docs` フィールドに根拠を記録) / driven adapter (forbidden)

> **強制先**: review 観点 — types / domain / usecase / infrastructure scope

判定例:

- `PolicyEvaluator { rules: Vec<Rule> }` + `evaluate(&self, ctx: &Context) -> Decision` → `role: DomainService` (state あり、behavior あり、依存なし)
- `Email(String)` + `new()` / `normalized()` → `role: ValueObject` (R3: 値からの side-effect-free な導出)
- `parse_yaml(input: &str) -> Result<...>` → `role: FreeFunction` (R2: state なし、依存なし)
- `RegisterUserApplicationService` + `RegisterUserInteractor { repo: Arc<dyn UserRepository> }` + `execute(&self, cmd) -> ...` → `role: ApplicationService` + `role: Interactor` (R1 / D3: ユースケース自身の structure-required な inbound port の組。repository port の複数実装や test seam の有無で省略しない)
- `RegisterUserUseCase { repo: Arc<dyn UserRepository> }` を `Arc<RegisterUserUseCase>` として共有し、`execute(&self, cmd) -> ...` を持たせる → `role: UseCase` (任意の service-level behavior で、共有所有だけなら service 自体は具体型を既定。`UserRepository` は required port のまま維持する)
- 任意の `RegisterUserService` trait + 実装を追加する → `role: ApplicationService` + `role: Interactor` (D2: service 自体に複数実装がある、または service 自体をテスト境界で差し替える必要がある場合だけ。required inbound port / `SecondaryPort` / `Repository` の組やその seam だけでは不十分)

### R7. Cross-Track Port Reference (SecondaryAdapter が参照する port は当該 track catalogue に declare する)

top-level `trait_impls[]` のうち `for_type` が `role: SecondaryAdapter` の型を指す entry の `trait_ref` で参照する trait (port) は、当該 track の `<layer>-types.json` のいずれかに `traits` エントリとして存在することが必須である。role は port の性質に応じて `SecondaryPort` (汎用 driven port) または `Repository` (aggregate root の永続化 port) のいずれかを選ぶ (R1 参照)。

> **強制先**: review 観点 — types scope

当該 track で改変しない baseline 由来の port は `action: "reference"` で declare する。declare 漏れは contract-map renderer のグローバル trait index (`build_trait_index`) の lookup が unmatched となり、`SecondaryAdapter -.impl.-> port` edge が silently skip される。

> **強制先**: review 観点 — types scope

#### declare 義務

- top-level `trait_impls[]` に `for_type: <SecondaryAdapter 型>` + `trait_ref: <port>` の entry を書いた以上、対応する `traits` entry (role は `SecondaryPort` または `Repository`) を当該 track の catalogue に作成する責任は type-designer に帰属する

  > **強制先**: review 観点 — types scope
- 当該 track で変更しない baseline 由来の port は `action: "reference"` で declare し catalogue への exposure を確保する

  > **強制先**: review 観点 — types scope

#### `action: "reference"` の semantics

- 当該 track では対象 port を変更しない (新規メソッド追加・既存メソッド変更なし)
- catalogue への exposure (contract-map / graph 描画) を成立させるための declare
- type-signal evaluator は `reference` action に対して「完全一致のみ Blue、不一致はすべて Red」として評価する (modify の Yellow 吸収は適用されない)
- baseline port の `methods` は baseline 当時の全 method を列挙する (method 型宣言の完全形規範 R8 は `reference` action でも同様に要求される)

  > **強制先**: review 観点 — types scope

#### declare 漏れの影響

contract-map renderer のグローバル trait index (`build_trait_index`) は当該 track の catalogue の `traits` エントリを role を問わず登録する (`action: delete` のみ除外)。当該 track の catalogue に対応する `traits` entry が存在しない trait 名は lookup で unmatched となり、`-.impl.->` edge が生成されない。graph 上の接合点が可視化されず、設計の空白が表面化しにくくなる。

### R8. Method Type Full Declaration (method / param 型フィールドは generic 引数を含む完全型文字列で宣言する)

以下のフィールドでは、generic 引数を省略した bare wrapper 名のみの宣言を禁止する:

- `methods[].returns` (TypeEntry / TraitEntry の inherent/trait method)
- `methods[].params[].ty` (同上)
- `params[].ty` (FunctionEntry の関数パラメータ)
- `returns` (FunctionEntry の戻り型)

> **強制先**: review 観点 — types scope

#### 禁止対象 wrapper 名 (generic 引数なし単独宣言)

`Result` / `Option` / `Vec` / `Box` / `Arc` / `Rc` / `Cow` / `BTreeMap` / `HashMap` / `HashSet` / `BTreeSet`

これらが具象型を伴わず単独で宣言された場合、contract-map renderer の `extract_type_names()` は wrapper 名 token しか返さず、内部具象型への edge が生まれない。

> **強制先**: review 観点 — types scope

#### lint ゲート

bare wrapper 名のみの宣言を catalogue の codec / verify CLI が schema validation で reject する lint を後続作業として組み込む。実装前は設計レビューで確認する (過渡期間)。

> **強制先**: review 観点 — types scope

### R9. No Primitive Obsession (制約ある概念を生 primitive で宣言しない)

catalogue の field / payload / param / returns / map キーで、検証可能な制約・有限値集合・ドメイン的意味を持つ概念を生 primitive (`String` / `i32` / `bool` 等) で宣言してはならない。値オブジェクト (`role: ValueObject` の `tuple` shape newtype、または有限値集合の `enum`) を定義して使う。

> **強制先**: review 観点 — types / domain / usecase / infrastructure scope

対象フィールド:

- `kind.shape.fields[].ty` (plain shape) / `kind.shape.fields[]` (tuple shape — each element is the bare type string)
- `kind.variants[].payload` の field 型 (enum payload)
- `methods[].params[].ty` / `methods[].returns` / FunctionEntry の `params[].ty` / `returns`
- `BTreeMap` / `HashMap` のキー型 (有限集合 → enum、識別子 → 検証付き newtype)

> **強制先**: review 観点 — types scope

判断手順:

1. その概念に「不正値」が存在するか判定する (空文字禁止 / 特定書式 / 有限集合 / 単位など)
2. 制約があるなら値オブジェクトを定義する: 有限集合 → `role: ValueObject` の `enum`; 検証付き識別子・値 → `role: ValueObject` の `tuple` shape (newtype) + constructor 検証
3. 生 primitive が正当なのは「真に制約のない不透明値」(検証も有限性もないフリーテキスト等) のみ。その場合は `docs` に生 primitive を選んだ根拠を記録する
4. **serde 境界 (`role: Dto`) も R9 の例外ではない**。`role: Dto` は wire format だが、概念に対応するフィールド・map キー・Vec 要素を生 String にしてよい免罪符ではない:
   - フィールド / Vec 要素が domain VO/enum を表す → driven adapter 側 `deserialize_with` で domain VO/enum へパースする (例: `include_function_roles: Vec<FunctionRole>` を文字列要素からパースする deserializer で受ける。`Vec<String>` 禁止)
   - **serde map キー** が serde-free な domain enum を表す → driven adapter 側に deserializable な **mirror enum** を定義し (`#[derive(Deserialize)]` + domain enum への `From` / `TryFrom`)、`BTreeMap<MirrorEnum, _>` で受ける。生 String キー + runtime 検証へ退避してはならない
   - **config キー / filter 値が domain 概念を名指すなら、それは概念への参照である**。`[role.<RoleName>]` の RoleName、`[edge.<EdgeKind>]` の EdgeKind、`include_function_roles` の各 FunctionRole 等、有限の domain 概念集合を名指すキー/値は「ただの設定文字列」ではなく、当該 domain enum (serde は driven adapter の mirror 経由) で型付ける。「open-ended だから String」「runtime で検証するから String」は R9 違反
   - 対応する enum が未だ無ければ、R1 の semantic evidence で配置を判定する。ユビキタス言語・不変条件・operation を越えた安定性・delivery/persistence/workflow からの独立性がある domain 概念なら R10 に従い domain enum を新設する。application boundary にのみ意味を持つ値なら、application の `Dto` / `Command` / `Query` / `ValueObject` として型付ける。いずれの場合も生 String へ退避してはならない。生 String 可は color / mermaid 構文のような domain 的意味を持たない提示専用値のみ

> **強制先**: review 観点 — types / domain / usecase / infrastructure scope

draft が本ルールに違反する (制約ある概念を生 primitive で宣言している) 場合、orchestrator レビュー前に self-reject して値オブジェクト化する。

> **強制先**: review 観点 — types scope

判定例:

- mermaid class 名 (空文字禁止の識別子) → `MermaidClassName(String)` newtype (生 `String` 禁止)
- 有限の edge 種別 → innermost の `EdgeKind` enum (R10: domain 概念。生 `String` キー禁止)。driven adapter の TOML map キーは deserializable な mirror enum (`EdgeKindKey` 等) で受け、innermost の `EdgeKind` へ変換する
- `include_function_roles = ["UseCaseFunction"]` (TOML) → `Vec<FunctionRole>` を `deserialize_with` で受ける (`Vec<String>` 禁止)
- 検証も有限性もない任意ラベル / color / mermaid 構文 → 生 `String` 許容 (`docs` に根拠記録)

**根拠**: `knowledge/conventions/prefer-type-safe-abstractions.md` § Make Illegal States Unrepresentable / Newtype。本ルールは当プロジェクト固有 convention であり、生 primitive を許容する方針のプロジェクトでは異なりうる (type-designer.md の横断性を保つため、本制約は agent 定義でなく本 convention に置く)。

### R10. Domain Concept → Domain Object in Innermost Layer (domain 概念は innermost にドメインオブジェクトとして定義する)

R10 を適用する前に R1 の semantic-first evidence で候補を分類する。ユビキタス言語に属し、不変条件を所有し、複数 application operation を越えて意味が安定し、persistence・delivery・workflow の都合から独立して存在するなら domain 概念である。same-track innermost 内部の inbound reference はその利用を示す補助証拠であり、欠如だけで innermost 配置を拒否しない。application boundary にのみ意味を持つ値は application の `Dto` / `Command` / `Query` / `ValueObject` として型付ける。

> **強制先**: review 観点 — types / domain / usecase scope

R1 で domain 概念と分類された概念 (ユビキタス言語に現れる名詞: 識別子・数量・分類・ポリシー・状態 等) は、必ず **ドメインオブジェクト** として R1 マトリクスで innermost に合法な role (`ValueObject` / `Entity` / `AggregateRoot` / `DomainService` / `Specification` / `Factory` / `ErrorType` — R1 マトリクスの innermost 列を参照) のいずれかでモデル化し、**innermost の性質に対応する layer catalogue に定義する**。どの層がそれを消費するかは問わない。R9 が「概念を生 primitive にしない」を、本 R10 が「domain 概念のドメインオブジェクト化 + innermost 配置 + カタログ宣言」を担う。role 選定は R1–R6 の判断木 (R3: ValueObject 制限 / R6: DomainService 選定基準 等) に従う。

> **強制先**: review 観点 — types / domain scope

論理連鎖 (なぜ概念が省略不能か):

1. 候補は R1 の semantic evidence で分類し、根拠を catalogue `docs` または review 記録に残す
2. domain 概念は **ドメインオブジェクト化** して innermost に配置する (生 primitive 化は R9 で禁止。`Entity` / `AggregateRoot` / `Specification` は innermost ONLY)
3. innermost の型は他層から参照されるため **`pub` 宣言が必須** (層 = 別クレート境界。`pub` + 公開パスがなければ application / driven adapter から名前で参照できずコンパイル不能)
4. `pub` 型は **カタログ宣言が必須** (カタログは public rustdoc API surface を写す。source に在る pub 型がカタログ未宣言なら signal evaluator の `CMinusSUnionD` = 🔴)
5. ∴ **各 domain 概念は省略不能で innermost catalogue に宣言される**。R1 で application 境界値と分類された候補は、その application catalogue に宣言される

> **強制先**: review 観点 — types / domain / usecase scope

ただし手順 4 の signal 裏打ち (`CMinusSUnionD` 🔴) は **実装後** にしか効かない (計画段階では概念が source に未在のため、カタログから省いても赤にならない)。よって R10 は **計画段階** で概念のモデル化・配置・宣言を保証する上流ルールであり、R9 / 12c と同じく **信号機評価とは別軸** (全緑でも R10 充足を意味しない)。

**serde / domain 純粋性を概念モデリング省略の口実にしてはならない**:

- innermost を serde-free に保つことは、R1 で domain 概念と分類された概念を innermost にモデル化しない理由には **ならない**。外部形式 (TOML / JSON 等) から読む必要がある domain 概念は、(a) innermost の性質に対応する layer に serde-free なドメインオブジェクトを定義し、(b) driven adapter に `role: Dto` の serde DTO を定義して相互変換する (R1: `Dto` は driven adapter)。purity は「innermost の domain model + driven adapter の DTO」の対で解決する。
- 「serde が要るから driven adapter の生 struct に留める」「R1 の分類をせずに概念をカタログから省略する」は **いずれも R10 違反**。R1 で application 境界値と分類された候補は、innermost ではなく application catalogue に型付けて宣言する。

> **強制先**: review 観点 — types / domain / usecase / infrastructure scope

判別 (R1 による分類):

- ドメイン的意味 (ドメインエキスパートとの会話に現れるか)、不変条件の所有、operation を越えた意味の安定性、delivery/persistence/workflow からの独立性を合わせて判断する。serde / 外部形式 / 表示の都合は判別に **関与しない** (それは配置ではなく DTO 変換の問題)。same-track inbound reference は補助証拠であり、意味分類を置き換えない。
- ドメイン的意味を一切持たない純粋な技術ノブ (adapter 内部のバッファサイズ・リトライ回数等) は innermost に置かない。R1〜R6 を適用しても role または配置が確定しない場合は、R5 に従い `## Open Questions` に escalation し、曖昧さだけを理由に innermost に配置してはならない。

> **強制先**: review 観点 — types / domain / usecase / infrastructure scope

判定例:

- innermost entry が参照する「許可された種別の有限集合」という概念 → semantic evidence が domain を示すなら `role: ValueObject` の `enum` を定義。外部設定から読むなら driven adapter に `role: Dto` を置いて変換。`Vec<String>` を driven adapter に持つのは R9 + R10 違反
- innermost の不変条件を表す検証付き識別子の概念 → innermost に `role: ValueObject` newtype。driven adapter DTO フィールドも生 `String` にはしない — `deserialize_with` カスタムデシリアライザで受けてフィールド型を innermost の VO にするか、serde-free な innermost enum を持つ場合は driven adapter 側に deserializable な mirror newtype を定義して変換する。application boundary にのみ意味を持つ値は application の `Dto` / `Command` / `Query` / `ValueObject` として型付ける

**根拠**: innermost は `architecture-rules.json` の `may_depend_on: []` で定義される最内層である。本ルールは当プロジェクト固有 convention であり、agent 定義でなく本 convention に置く (横断性のため)。

## Examples

### Good

- `parse_adr_frontmatter` を `role: FreeFunction` で driven adapter の `functions` エントリに置く (R2)
- `evaluate_adr_decision` を `role: FreeFunction` で innermost の `functions` エントリに置く (R2 + R1: `FreeFunction` は layer-flexible)
- `AdrDecisionCommon { id, user_decision_ref, ... }` を `role: ValueObject` で innermost の `types` エントリに置く (R3: 検証済み shared payload で behavior なし)
- `ProposedDecision` / `AcceptedDecision` / ... を `role: ValueObject` + typestate marker 付き `struct` で innermost に置き、`AdrDecisionEntry` を `role: ValueObject` + `kind: { "kind": "enum" }` の wrapper として並置 (decision tree: state machine + heterogeneous Vec。typestate の wire format は `.harness/reference/catalogue-schema.md`「The `kind` field」節を参照)
- `FsAdrFileAdapter` を `role: SecondaryAdapter` で driven adapter の `types` エントリに置く (R1: `SecondaryAdapter` は driven adapter ONLY)
- baseline 由来の `ReviewReader` port を当該 track の innermost catalogue に `action: "reference"` で `role: SecondaryPort` の `traits` エントリとして declare する (R7: declare により `FsReviewStore -.impl.-> ReviewReader` edge が contract-map に出る)
- `methods[].returns` フィールドに `"Result<AdrFrontMatter, AdrFrontMatterCodecError>"` と完全型文字列を書く (R8: `extract_type_names()` が `AdrFrontMatter` / `AdrFrontMatterCodecError` への edge を生成できる)

### Bad

- `AdrFrontMatterCodec` (parse method を持つ struct) を `role: ValueObject` で起草 (R3 違反: behavior を持つ)
  - 正しい修正: `parse_adr_frontmatter` を `role: FreeFunction` に分解 (R2)
- `AdrSignalsVerifyAdapter` を `role: UseCase` で driven adapter に起草 (R1 違反: `UseCase` は application ONLY)
  - 正しい修正: R1 の層配置違反を直し、driven adapter の port 実装なら `role: SecondaryAdapter` を置く。application のユースケース自身の inbound port なら `role: ApplicationService` + `role: Interactor` の structure-required な組を複数実装や service 自体のテスト差し替えなしでも置く。structure-required な組とは別の任意 service-level 抽象なら、D2 の必要性テストを適用し、共有所有だけなら `Arc<具象型>` を既定として不要な抽象を追加しない
- 状態遷移を持つ ADR decision を `role: ValueObject` + `kind: { "kind": "enum" }` (`DecisionStatus { Proposed, Accepted, ... }`) で起草し、別 entry に `role: ValueObject` + `kind: { "kind": "struct", "shape": { "kind": "plain", "fields": [...], "has_stripped_fields": false } }` (`status: DecisionStatus`, `implemented_in: Option<String>`) を置く (R3 違反 + 決定木違反)
  - 正しい修正: typestate cluster + enum wrapper (`role: ValueObject` + typestate marker 付き `struct` で各 state を起草し、heterogeneous Vec 用の enum wrapper を `role: ValueObject` + `kind: { "kind": "enum" }` で追加。typestate の wire format は `.harness/reference/catalogue-schema.md`「The `kind` field」節を参照)
- 「他の role が fit しないので」という理由で `role: ValueObject` を選ぶ (R5 違反)
  - 正しい修正: 決定木を再適用 → `role: FreeFunction` 候補を検討 → それでも確定しないなら `## Open Questions` に escalation
- `FsReviewStore` (baseline 由来の `ReviewReader` / `ReviewWriter` port を implement する adapter) を driven adapter に `role: SecondaryAdapter` で起草したが、当該 track の catalogue に `ReviewReader` / `ReviewWriter` の `role: SecondaryPort` entry を declare しない (R7 違反: declare 漏れによる `-.impl.->` edge の silently skip)
  - 正しい修正: `ReviewReader` / `ReviewWriter` を `action: "reference"` で innermost catalogue に `role: SecondaryPort` の `traits` エントリとして declare する
- `methods[].returns` / `methods[].params[].ty` / FunctionEntry の `returns` / `params[].ty` を bare wrapper 名のみで宣言する (R8 違反: edge 漏れの原因)
  - 悪い例: `returns: "Result"` / `ty: "Arc"` / `ty: "Vec"`
  - 正しい修正: `returns: "Result<AdrFrontMatter, AdrFrontMatterCodecError>"` / `ty: "Arc<dyn AdrFilePort>"` / `ty: "Vec<AdrDecisionEntry>"`

## Review Checklist

type-designer 自身および reviewer は draft 段階で以下を確認する:

> **強制先**: review 観点 — types scope

- [ ] 各 entry の `role` × layer 性質の組合せが R1 マトリクスで OK か (✗ / ONLY 違反がないか、`DomainService` は driven adapter の層に置かれていないか)

  > **強制先**: review 観点 — types scope
- [ ] zero-field struct + 1 method の entry がないか (あれば R2: `role: FreeFunction` に折り畳めないか確認)

  > **強制先**: review 観点 — types scope
- [ ] `role: ValueObject` の entry がすべて R3 を満たすか (値等価で識別され、`methods` が生成時に不変条件を確立する constructor / validation、または自身の値から値または述語を導出する side-effect-free なものに限られるか。依存や外部リソースを扱う service 的 behavior がないか。typestate state の entry は遷移メソッドを `methods` に持つ)

  > **強制先**: review 観点 — types scope
- [ ] R6 の採用条件を満たし、値等価で識別される ValueObject (R3) ではない service 中心の field + behavior を持つ domain struct が `role: DomainService` で起草されているか (`role: ValueObject` / `role: Interactor` への誤分類がないか)

  > **強制先**: review 観点 — types / domain scope
- [ ] role 起草前に偵察 (R4) を実施したか (近接 track の role 分布を確認したか)

  > **強制先**: review 観点 — types scope
- [ ] catch-all として `role: ValueObject` / `role: UseCase` を選んでいないか (R5)

  > **強制先**: review 観点 — types scope
- [ ] top-level `trait_impls[]` のうち `for_type` が `role: SecondaryAdapter` の型を指す entry の `trait_ref` で参照するすべての trait (port) が当該 track の catalogue に `traits` エントリ (role は `SecondaryPort` または `Repository`) として declare されているか (R7)。baseline 由来の port は `action: "reference"` で declare されているか

  > **強制先**: review 観点 — types scope
- [ ] `methods[].returns` / `methods[].params[].ty` (TypeEntry / TraitEntry) および FunctionEntry の `returns` / `params[].ty` に bare wrapper 名のみの宣言 (`Result` / `Option` / `Vec` / `Box` / `Arc` / `Rc` / `Cow` / `BTreeMap` / `HashMap` / `HashSet` / `BTreeSet`) がないか (R8)

  > **強制先**: review 観点 — types scope
- [ ] field / payload / param / returns / map キーで、制約ある概念を生 primitive (`String` 等) で宣言していないか (R9)。制約があれば値オブジェクト (newtype / enum) を定義しているか。**`role: Dto` / serde 境界も例外ではない** — 概念を名指す map キー・filter 値は innermost の domain enum (serde は driven adapter の mirror enum 経由) で型付けているか。生 primitive は color / 自由ラベル等の真に不透明な提示専用値のみで、その場合 `docs` に根拠が記録されているか

  > **強制先**: review 観点 — types / domain / usecase / infrastructure scope
- [ ] `ValueObject` 候補を R1 の semantic-first evidence (ユビキタス言語、不変条件、operation を越えた安定性、delivery/persistence/workflow からの独立性) で分類し、根拠を記録したか。domain 概念は R1 マトリクスで innermost に合法な role (ValueObject / Entity / AggregateRoot / DomainService / Specification / Factory / ErrorType) のいずれかで innermost に定義し、innermost catalogue に宣言しているか (R10)。same-track inbound reference は補助証拠としてのみ扱ったか。application boundary 値は application の `Dto` / `Command` / `Query` / `ValueObject` として型付け、application catalogue に宣言しているか。serde / 外部形式の都合を口実に、R1 の分類をせずに概念を driven adapter の生 struct に留めたりカタログから省略したりしていないか。外部形式が要る domain 概念は「innermost の domain object + driven adapter の `role: Dto`」の対で表現しているか

  > **強制先**: review 観点 — types / domain / usecase / infrastructure scope
- [ ] R1〜R10 のいずれかで判断不能な entry が `## Open Questions` に escalation されているか

  > **強制先**: review 観点 — types / harness-policy scope

## Enforcement

- 第一線: catalogue を起草する agent の定義で本 convention の reading + compliance を義務付ける

  > **強制先**: review 観点 — types scope
- 第二線: `bin/sotp catalogue-lint check-active-track` が、出荷 catalogue-lint config の `KindLayerConstraint` で active track の R1 forbidden role × layer 組合せを signal 評価より先に reject する。これは config を消費する lint gate であり、将来候補の codec validation とは別の機構である

  > **強制先**: 機械 lint — bin/sotp catalogue-lint check-active-track
- 第三線: track ごとの reviewer briefing (`tmp/reviewer-runtime/briefing-{scope}.md`) で本 convention を参照し、R1〜R10 の checklist を review 観点として明示する。`.harness/custom/review-prompts/<scope>.md` は利用者所有の severity policy であり、framework methodology の enforcement source にはしない

  > **強制先**: review 観点 — types / harness-policy scope
- 第四線: `bin/sotp signal calc-impl-catalog` の signal 評価 (catalogue → spec の trace integrity)。role 違反は第二線で signal 評価より先に draft 段階で却下するため、検証の網としては最終 backstop の位置づけ

  > **強制先**: review 観点 — harness-policy scope

将来の自動化候補: catalogue validation で R1 layer-role マトリクスを machine-readable に表現し、`bin/sotp` の validation で reject する (`forbidden` 組合せ → codec error)。

> **強制先**: 強制なし (明記) — 自動化は将来候補で現行機構なし

## Related Documents

- `knowledge/conventions/prefer-type-safe-abstractions.md` — enum-first / typestate / newtype の design principle (本 convention は role 選定への適用)
- `.harness/policies/pre-track-adr-authoring.md` — ADR 配置規則 (catalogue の上流 SSoT)
- `architecture-rules.json` — TDDD 対応層の SSoT (R1 layer 列挙の根拠)
- `.harness/reference/catalogue-schema.md` — v5 wire format と `DataRole` / `ContractRole` / `FunctionRole` / `TypeEntry` / `TraitEntry` / `FunctionEntry` / `TypeKindV2` / `StructKind` / `StructShape` / `TypestateMarker` / `TraitImplDeclV2` / `InherentImplDeclV2` の参照
- `knowledge/adr/README.md` — 設計判断の索引（履歴を確認する必要がある場合）
