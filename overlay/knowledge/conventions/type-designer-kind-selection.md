---
required_for:
  - type-designer
  - rollback-diagnoser
---

# Type-Designer Kind Selection Convention

## この文書の所有権

この規約は **利用プロジェクトが所有する**。初期値としてテンプレートから供給されるが、以後の改稿・改名・削除はプロジェクトの裁量である。ハーネスは role 語彙 (`DataRole` / `ContractRole` / `FunctionRole` の variant 集合)、catalogue の wire format、`KindLayerConstraint` という lint rule 種別とその検査意味論、および type-designer が lint gate を通す義務までを所有する。**どの role をどの層に置いてよいか** という方針そのもの — R1 のマトリクス本体 — はこの文書にあり、プロジェクトのものである。

したがって、role × layer 方針を変えたい場合にハーネス側を書き換える必要はない。R1 のマトリクスと、その機械写像である `.harness/catalogue-lint/config.json` の `permitted_layers` を書き換えればよい (手順は R1 の「層名を変更する場合」を参照)。この規約を全面的に破棄しても構わない — その場合 `required_for` frontmatter ごと削除すれば、type-designer への必読解決から外れる。

## Purpose

`type-designer` が `<layer>-types.json` を起草する際、role 選定ミスや層配置違反を agent 自身が構造的に防ぐための拘束ルール集である。

型設計は type-designer の専門領域であり、orchestrator や利用者が事後に role 選定ミスを指摘して redesign を迫る運用は逆転している。本 convention は type-designer が **自律的に正しい role を選び、誤った fallback を避ける** ための判断基準を SSoT として明示する。

このハーネスで繰り返し観測される type-designer の典型的な逸脱は次の 4 つである。本規約の各ルールは、これらを名指しで塞ぐために書かれている。

- 状態遷移を持つ型に `role: ValueObject` + `kind: { "kind": "enum" }` (status field + `Option<...>`) を選び、typestate pattern を回避する
- usecase 以外の層に `role: UseCase` / `role: ApplicationService` / `role: Interactor` を配置する
- ゼロフィールド struct + 1 method の型を `role: ValueObject` として、「検証済みの値」という意味から大きく外して使う
- 他の role が fit しないとき `role: ValueObject` を catch-all として採用する (semantic stretch)

## Scope

- 適用対象:
  - `type-designer`
  - すべての TDDD 対応層 (`architecture-rules.json` の `tddd.enabled: true`) における `<layer>-types.json` の起草・更新
  - 各 entry の `role` 選定、`expected_*` フィールド設計、層配置判断
- 適用外:
  - `spec-designer` / `impl-planner` / `adr-editor` が所有する artifact
  - role が確定済みで構造変更を伴わない `action: "modify"` 編集 (フィールド追加など)。ただし role 変更を含む場合は本 convention の対象

## Rules

### R1. Role-Layer Compatibility (role × layer 互換マトリクス)

`<layer>-types.json` の各 entry は、role と層の組合せを以下の表に従う。Forbidden の組合せを起草してはならない。

entry がどの role 軸に属するか、および各 entry に必要な具体的フィールド (`kind`, `methods`, top-level の `trait_impls` / `inherent_impls` など) は `.harness/reference/catalogue-schema.md` を参照する。本マトリクスは **層配置** の制約だけを規定し、schema を再述しない。要点は次の 3 つだけである。

- `types` エントリの role は `DataRole`、`traits` エントリの role は `ContractRole`、`functions` エントリの role は `FunctionRole` である
- 一部の role は payload を持つ data-carrying variant で、wire format は discriminated-object 形式になる
- 表の「role」列は role フィールドの値 (variant 名) に対応する

#### 層名の出所

以下の表の層名 (`domain` / `usecase` / `infrastructure` / `cli` / `cli_composition` / `cli_driver`) は、**このプロジェクトの `architecture-rules.json` が宣言する層**である。ハーネスが固定した値ではなく、テンプレートが供給した既定の層構成にすぎない。層構成はプロジェクトの選択であり、層を増やす・減らす・改名するのは正当な変更である。

このマトリクスは、その既定の層構成を前提とした初期値として書かれている。層構成を変えたなら、このマトリクスは変えた本人が更新する対象になる。

#### 層名を変更する場合

`architecture-rules.json` の層を追加・削除・改名したときは、次の 3 箇所を同じ変更として揃える。

1. `architecture-rules.json` の `layers[].crate` (層 id の SSoT)
2. 本 R1 のマトリクスの列見出しと各セル
3. `.harness/catalogue-lint/config.json` の各 `KindLayerConstraint` の `permitted_layers` — これはマトリクスの機械写像であり、`✓` と `△` の層の集合をそのまま列挙する

3 が 2 と食い違うと、lint gate が意図しない配置を通すか、正当な配置を拒否する。層名は `permitted_layers` に literal で書かれるため、改名は文字列一致で追随させる必要がある。`.harness/catalogue-lint/presets/` 配下の preset を使っている場合は、その preset も同じ構造に保つ。

| role | domain | usecase | infrastructure | cli | cli_composition | cli_driver | 配置根拠 |
|---|---|---|---|---|---|---|---|
| `ValueObject` (DataRole) | △ | △ | △ | ✗ | ✗ | ✗ | 配置はユビキタス言語、不変条件、複数 operation を越えた意味の安定性、persistence / delivery / workflow 都合からの独立性で判断する。domain-internal inbound 参照は補助証拠であり必須条件ではない |
| `Entity` (DataRole) | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ | entity は domain 概念。他層での使用は domain leak |
| `AggregateRoot` (DataRole) | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ | aggregate root は domain 概念 |
| `DomainService` (DataRole) | ✓ | △ | ✗ | ✗ | ✗ | ✗ | domain knowledge を集約する behavior 中心 struct。usecase は trans-domain な application logic の場合のみで要根拠 |
| `EventPolicy` (DataRole) | **✓ ONLY** | ✗ | ✗ | ✗ | ✗ | ✗ | event-driven policy。domain 層のみ許可。payload に反応対象を持ち、DomainEvent 役の型のみ参照可 |
| `DomainEvent` (DataRole) | **✓ ONLY** | ✗ | ✗ | ✗ | ✗ | ✗ | aggregate が emit する事実。enum 形 / unit struct どちらも可。mutation surface (`&mut self` / public field) は lint が禁止する |
| `Specification` (DataRole) | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ | domain predicate。他層は domain leak |
| `Factory` (DataRole) | ✓ | ✓ | △ | ✗ | ✗ | ✗ | 集約 / entity factory。infrastructure に置くのは要根拠 |
| `UseCase` (DataRole) | ✗ | **✓ ONLY** | ✗ | ✗ | ✗ | ✗ | 名前と意味が usecase 層を表す。他層は役割違反 |
| `Interactor` (DataRole) | ✗ | **✓ ONLY** | ✗ | ✗ | ✗ | ✗ | `ApplicationService` の実装。usecase 層 |
| `Command` (DataRole) | ✗ | **✓ ONLY** | ✗ | ✗ | ✗ | ✗ | CQRS command。usecase 層が受け取る入力 |
| `Query` (DataRole) | ✗ | **✓ ONLY** | ✗ | ✗ | ✗ | ✗ | CQRS query。usecase 層が受け取る入力 |
| `Dto` (DataRole) | ✗ | △ | **✓** | ✓ | ✗ | ✓ | serde 境界は infrastructure に置き、domain は serde-free に保つ。usecase は要根拠。delivery 層では引数パーサの args・入力 DTO・出力 DTO に使う |
| `ErrorType` (DataRole) | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ | layer-flexible (各層がそれぞれの責務に応じた error 型を持つ)。`cli_driver` は primary adapter が常に成功型の outcome を返すため error 型を持たない |
| `SecondaryAdapter` (DataRole) | ✗ | ✗ | **✓ ONLY** | ✗ | ✗ | ✗ | secondary port の実装は infrastructure に置く |
| `CompositionRoot` (DataRole) | ✗ | ✗ | ✗ | ✗ | **✓ ONLY** | ✗ | object graph を組む純 DI の住所。composition 層のみ |
| `PrimaryAdapter` (DataRole) | ✗ | ✗ | ✗ | ✗ | ✗ | **✓ ONLY** | driving adapter (invoke + render)。公開シグネチャは usecase の `Command` / `Query` / boundary `Dto` / usecase `ValueObject` を参照してよい。domain の `ValueObject` / `Entity` / `AggregateRoot` の直接露出、infrastructure / transport 型の漏出は不可。`ValueObject` の domain / usecase 分類は R1 の semantic evidence で判定する review rule であり、role 名だけでは機械判定できない。したがって role だけを見る lint は `ValueObject` の露出を禁止しない |
| `SpecificationPort` (ContractRole) | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ | domain の仕様を表す port |
| `SecondaryPort` (ContractRole) | ✓ | ✓ | ✗ | ✗ | ✗ | ✗ | domain / usecase のいずれにも置ける driven port |
| `ApplicationService` (ContractRole) | ✗ | **✓ ONLY** | ✗ | ✗ | ✗ | ✗ | usecase interface |
| `Repository` (ContractRole) | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ | aggregate root の永続化 port (data-carrying: payload の参照先は `AggregateRoot` 役で宣言する)。aggregate の語彙で説明されるため domain に置く |
| `FreeFunction` (FunctionRole) | ✓ | ✓ | ✓ | ✓ | ✗ | ✓ | layer-flexible (top-level pub fn)。composition 層は配線を `CompositionRoot` のメソッドとして書くため pub free function が生じない |
| `UseCaseFunction` (FunctionRole) | ✗ | **✓ ONLY** | ✗ | ✗ | ✗ | ✗ | use-case entrypoint function。usecase 層 |

凡例: `✓` = OK、`△` = 要根拠 (default ではない。根拠を `docs` フィールドに記録する。`ValueObject` は `docs` または review 可能な track 記録)、`✗` = forbidden、`**ONLY**` = この層以外で使うことを禁止

上表の全 role に対応する `KindLayerConstraint` が出荷 catalogue-lint config に存在する。各 rule の `permitted_layers` は表の `✓` と `△` の層を許可し、その補集合である `✗` を active track で拒否する。lint は `✓` と `△` の区別や根拠の妥当性を機械判定しない — それは reviewer の担当である。

`ValueObject` は domain / usecase / infrastructure のいずれへ置く場合も、ユビキタス言語、不変条件の所有、複数 application operation を越えた意味の安定性、persistence・delivery・workflow 都合からの独立性を根拠として決める。same-track の domain-internal inbound reference は domain model での利用を示す補助証拠として記録してよいが、その不在だけを理由に domain 配置を拒否してはならない。application boundary にのみ意味を持つ値は usecase の `Dto` / `Command` / `Query` / `ValueObject` として置く。配置の semantic classification とその根拠は catalogue の `docs` または track の review 記録に残し、reviewer が照合する。

`✗` または **ONLY** を破る role × layer 選択は、signal 評価以前に **role 違反** として draft 段階で却下する。

#### Port placement tie-break

port が domain の不変条件または aggregate の語彙で説明できるなら domain に置く。アプリケーションのオーケストレーションが必要とする技術的能力なら usecase に置く。たとえば aggregate の永続化は domain の `Repository`、外部サービス呼び出しや差分取得の能力は usecase の `SecondaryPort` として分類する。

#### CQRS separation evidence

`Command` と `Query` を別の `Interactor` / `ApplicationService` に分離するのは、side effect、required collaborator、possible error、consistency boundary、read/write model のうち少なくとも一つに操作固有の実質的な非対称性がある場合だけである。分離する catalogue は、該当次元、具体的な操作差、分離根拠を `docs` または review 可能な track 記録に残す。read と write の両方があることや、role が利用可能なことだけでは分離理由にならない。

#### Driver injection and facade prohibition

入力 port は 1 ユースケースにつき 1 trait とし、実行メソッドを 1 つだけ持つ。driver の注入粒度はこの port 粒度に合わせ、driver は自分が消費する複数の単能 port をそれぞれ直接受け取ってよい。「driver は 1 つの interactor だけを注入する」という制約は置かない。

> **強制先**: review 観点 — types / usecase / cli_driver / cli_composition scope

command と query を混載する `*Service` などの facade port を新設してはならない。この禁止は未移行の文脈にも適用する。既存の facade port や既存の単一 interactor 注入は、この規約だけを理由に遡及改修しない。

> **強制先**: review 観点 — types / usecase / cli_driver / cli_composition scope

### R2. Free Function Preference (stateless behavior は FreeFunction)

以下の条件をすべて満たす型は `role: FreeFunction` (`functions` エントリ) で起草する。ゼロフィールド struct + 1 method を `role: ValueObject` / `role: UseCase` に matching するのは禁止する。

- top-level の pub fn である (struct や trait の method ではない)
- またはゼロフィールド struct で、その「struct」が表す唯一の責務が 1 つの pub fn 呼び出しに帰着する
- 内部 state を持たない (struct field なし、または `()` のみ)
- 依存注入を必要としない (依存があるなら、まず structure-required port か任意の service-level 抽象かを分類する。structure-required port は D2 の必要性テストではなく支配するアーキテクチャ規則に従い、任意の service-level 抽象は D2 の条件に従う。usecase の任意の service-level 抽象は、条件成立時だけ `role: Interactor` + `role: ApplicationService` とし、その他は `role: UseCase` の具体型を既定とする。infrastructure は `role: SecondaryAdapter`)

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli_driver scope

判定例:

- `parse_config(input: &str) -> Result<Config, ConfigParseError>` → `role: FreeFunction` (state なし、依存なし)
- `evaluate_discount(order: &Order) -> Discount` → `role: FreeFunction`
- `EvaluateDiscount { /* zero fields */ } impl { fn evaluate(&self, ...) -> ... }` → 設計を `role: FreeFunction` に折り畳む。ゼロフィールド struct は wrapping だけで意味を加えない

#### 必要駆動の抽象

候補はまず、(1) アーキテクチャが要求する structure-required port とその実装の組か、(2) その組の上に層の内部で任意に重ねる service-level 抽象かを分類する。ユースケース自身の入力ポートは、1 ユースケース 1 trait・実行メソッド 1 つの `ApplicationService` inbound port と、その `Interactor` 実装からなる structure-required な組であり、複数実装や service 自体のテスト差し替えがなくても D2 の必要性テストを適用せず、R1 / D3 の配置・粒度規則に従って導入する。層を越える依存を表す `SecondaryPort` と aggregate の永続化を表す `Repository` も structure-required ports であり、必要性テストではなく支配するアーキテクチャ規則に従って導入する。

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli_driver scope

一方、structure-required な入力ポートの組の上に同じ service を共有するためだけに第二の trait と実装を重ねる場合、またはその他の層内 service-level 抽象を追加する場合は、D2 の必要性テストの対象である。(a) 複数の実装が現存する、または (b) service 自体をテスト境界で差し替える必要がある場合だけ導入し、共有所有だけなら `Arc<具象型>` を既定とする。条件が後から成立した時点で trait を切り出す。既存の抽象ペアは改訂後の規約に合わせて遡及解体しない。structure-required な `ApplicationService`、`SecondaryPort`、`Repository` の port 自体やその必要な実装を、単一実装だからという理由で省略してはならない。

> **強制先**: review 観点 — types / domain / usecase / infrastructure / cli_driver scope

### R3. ValueObject Semantic Restriction

`role: ValueObject` は値等価で識別される値を表す。自身の値から新しい値または述語を導出する side-effect-free な method は許容する。一方、依存または外部リソースを扱う behavior 中心の service 的 struct は ValueObject ではない。

| OK (ValueObject) | NG (ValueObject 違反) |
|---|---|
| `Email(String)` newtype + `new()` で形式検証 | `parse_*` のように外部表現を解釈する service 的 struct |
| `Money::add` / `DateRange::overlaps` のように値から値・述語を導出する method | `Codec` / `Validator` / `Resolver` のように依存または外部 resource を扱う struct |
| 検証付きの shared payload record | behavior を中心に据えた struct |
| 複合 primitive を集めた読み取り専用の record | trait 実装を意図する struct (→ `role: Interactor` / `role: SecondaryAdapter`) |

判定は構造条件より意味論を優先する。値等価で識別され、method がその値だけから値または述語を導出するなら ValueObject である。依存、外部 resource、または service の責務を中心にするなら ValueObject ではない。

behavior を持つ struct は以下のいずれかに振り分ける。

- 依存なし stateless → `role: FreeFunction` (R2)
- 依存あり (port を呼び出す) → usecase では、ユースケース自身の structure-required な inbound port の実装は `role: Interactor` とし、`role: ApplicationService` trait と組にする。structure-required な `SecondaryPort` / `Repository` も支配するアーキテクチャ規則に従う。structure-required な port とは別に任意の service-level 抽象を追加する場合は、共有所有だけなら `role: UseCase` の具体型を `Arc<具象型>` で扱い、複数実装または service 自体のテスト境界での差し替えが必要な場合だけ `role: ApplicationService` + `role: Interactor` の組を導入する。infrastructure では `role: SecondaryAdapter` (port 実装)
- 集約構築 → `role: Factory`
- 状態遷移あり → typestate cluster (`role: ValueObject` で各 state を typestate marker 付き `struct` として表現し、遷移メソッドを `methods` に宣言する。wire format は `.harness/reference/catalogue-schema.md` を参照)
- 値の同一性ではなく domain behavior を中心にする struct → `role: DomainService` (R6)

> **強制先**: review 観点 — types / domain / usecase / infrastructure scope

### R4. Role Distribution Reconnaissance (起草前の偵察義務)

新規 catalogue の draft を書き始める前に、既存 track の catalogue から role 分布を調査し、当 track の起草の参照基準にする。他の reconnaissance ステップと並行して実施してよい。

調査内容:

- 完了済みの近接 track (同じ層 / 同じ ADR を参照するもの) の `<layer>-types.json` を 1〜3 件 sample する
- そこで採用されている role の分布 (どの role がどれだけ使われているか)
- naming convention (PascalCase struct / snake_case fn / `*Error` / `*Port` / `*Adapter` 等の suffix)
- `role: ValueObject` と `role: FreeFunction` の使い分けの実例

この偵察により、特定 role を「思い出した順」で機械的に当てはめる代わりに、**プロジェクト全体の role 配分との整合** を保った起草ができる。偵察結果は internal preparation であり、final report に出さなくてよい。

まだ完了 track が存在しないプロジェクト (テンプレート導入直後など) では、参照先がないことを確認したうえで偵察を省略してよい。その場合は R1〜R3 と R5〜R6 の判断木だけで role を決める。

例: ADR が parse や evaluate のような stateless behavior を要求しているのに、過去 track で類似機能が `role: FreeFunction` で実装されているなら、当該 track でも `role: FreeFunction` を採用する。`role: UseCase` / `role: ValueObject` を選ぶ場合は、その rationale を `docs` フィールドに記録する。

### R5. No Fallback Rule (catch-all 禁止)

「他の role が完全に fit しない」という理由で `role: ValueObject` または `role: UseCase` を catch-all として採用してはならない。

判断手順:

1. 候補 role を列挙し、R1 マトリクスで層と role の組合せを絞り込む
2. role が確定しない場合 → R2 (`FreeFunction`) と R3 (`ValueObject` 制限) を再確認する
3. それでも確定しない場合 → R6 (`DomainService`) の判定基準で domain 層 behavior の住所として fit するか確認する
4. それでも確定しない場合 → 起草を止め、`## Open Questions` に「role が確定しない理由」と「検討した候補とその却下理由」を列挙して orchestrator に escalation する
5. orchestrator は ADR / spec の補強 (adr-editor / spec-designer の再実行) または利用者の判断を仰ぐ

`role: ValueObject` で迷ったときの最も多い真の答えは `role: FreeFunction` (R2) である。次に多いのは、依存を持つ application の任意 service の具体型としての `role: UseCase`、structure-required な inbound port の実装としての `role: Interactor` + `role: ApplicationService`、D2 の複数実装または service 自体のテスト差し替え条件が成立した任意 abstraction としての `role: Interactor` + `role: ApplicationService`、`role: SecondaryAdapter` (port 実装)、または `role: DomainService` (R6: field を持つ domain behavior) である。`role: ValueObject` を選ぶ前に、候補が structure-required な組か任意 abstraction かを分類する。

> **強制先**: review 観点 — types scope

### R6. DomainService Selection Criteria (domain behavior の住所)

値等価で識別され、side-effect-free な導出 method だけを持つ型は DomainService ではなく ValueObject (R3) である。`role: DomainService` は値ではなく domain behavior を中心にする struct の住所であり、structure-required な `ApplicationService` inbound port の実装、または D2 の条件を満たす任意 service-level abstraction の `role: Interactor` と混同しないため、以下の全条件を満たす場合に採用する。

採用条件 (AND):

- struct である (enum / typestate cluster ではない)
- `kind.shape.fields` >= 1 field (state を保持する。ゼロフィールドは R2 の `FreeFunction` 候補)
- `methods` >= 1 entry (domain behavior を持つ。導出 method だけなら R3 の `ValueObject` 候補)
- 状態遷移がない (ある場合は typestate pattern — R3 の振り分け)
- `ApplicationService` / `SecondaryPort` の実装ではない (structure-required な inbound port の実装は `role: Interactor`、任意 service-level abstraction が D2 の条件を満たして `ApplicationService` を実装する場合も `role: Interactor`、secondary port の実装は `role: SecondaryAdapter`)
- 配置層は domain (default) / usecase (要根拠 — trans-domain な application logic で domain knowledge を集約する場合のみ、`docs` フィールドに根拠を記録) / infrastructure (forbidden)

> **強制先**: review 観点 — types / domain / usecase / infrastructure scope

判定例:

- `PricingPolicy { rules: Vec<Rule> }` + `apply(&self, order: &Order) -> Price` → `role: DomainService` (state あり、behavior あり、依存なし)
- `Email(String)` + `new()` / `normalized()` → `role: ValueObject` (R3: 値からの side-effect-free な導出)
- `parse_config(input: &str) -> Result<Config, ConfigParseError>` → `role: FreeFunction` (R2: state なし、依存なし)
- `RegisterUserApplicationService` + `RegisterUserInteractor { repo: Arc<dyn UserRepository> }` + `execute(&self, cmd) -> ...` → `role: ApplicationService` + `role: Interactor` (R1 / D3: ユースケース自身の structure-required な inbound port の組。repository port の複数実装や test seam の有無で省略しない)
- `RegisterUserUseCase { repo: Arc<dyn UserRepository> }` を `Arc<RegisterUserUseCase>` として共有し、`execute(&self, cmd) -> ...` を持たせる → `role: UseCase` (任意の service-level behavior で、共有所有だけなら service 自体は具体型を既定。`UserRepository` は required port のまま維持する)
- 任意の `RegisterUserService` trait + 実装を追加する → `role: ApplicationService` + `role: Interactor` (D2: service 自体に複数実装がある、または service 自体をテスト境界で差し替える必要がある場合だけ。required inbound port / `SecondaryPort` / `Repository` の組やその seam だけでは不十分)

### R7. Cross-Track Port Reference (SecondaryAdapter が参照する port は当該 track catalogue に declare する)

top-level `trait_impls[]` のうち `for_type` が `role: SecondaryAdapter` の型を指す entry の `trait_ref` が参照する trait (port) は、当該 track のいずれかの `<layer>-types.json` に `traits` エントリとして存在することが必須である。role は port の性質に応じて `SecondaryPort` (汎用 driven port) または `Repository` (aggregate root の永続化 port) のいずれかを選ぶ (R1 参照)。

当該 track で改変しない baseline 由来の port は `action: "reference"` で declare する。declare 漏れは contract-map の trait 解決で unmatched となり、`SecondaryAdapter -.impl.-> port` の edge が黙って落ちる。

#### declare 義務

- top-level `trait_impls[]` に `for_type: <SecondaryAdapter 型>` + `trait_ref: <port>` の entry を書いた以上、対応する `traits` entry (role は `SecondaryPort` または `Repository`) を当該 track の catalogue に作成する責任は type-designer に帰属する
- 当該 track で変更しない baseline 由来の port は `action: "reference"` で declare し、catalogue への exposure を確保する

#### `action: "reference"` の semantics

- 当該 track では対象 port を変更しない (新規メソッド追加・既存メソッド変更なし)
- catalogue への exposure (contract-map / graph 描画) を成立させるための declare である
- type-signal evaluator は `reference` action を「完全一致のみ Blue、不一致はすべて Red」として評価する (`modify` の Yellow 吸収は適用されない)
- baseline port の `methods` は baseline 当時の全 method を列挙する (R8 の完全形規範は `reference` action でも同様に要求される)

#### declare 漏れの影響

contract-map の trait index は当該 track の catalogue の `traits` エントリを role を問わず登録する (`action: delete` のみ除外)。当該 track の catalogue に対応する `traits` entry が存在しない trait 名は lookup で unmatched となり、`-.impl.->` edge が生成されない。graph 上の接合点が可視化されず、設計の空白が表面化しにくくなる。

### R8. Method Type Full Declaration (method / param 型フィールドは generic 引数を含む完全型文字列で宣言する)

以下のフィールドでは、generic 引数を省略した bare wrapper 名のみの宣言を禁止する。

- `methods[].returns` (TypeEntry / TraitEntry の inherent / trait method)
- `methods[].params[].ty` (同上)
- `params[].ty` (FunctionEntry の関数パラメータ)
- `returns` (FunctionEntry の戻り型)

#### 禁止対象 wrapper 名 (generic 引数なし単独宣言)

`Result` / `Option` / `Vec` / `Box` / `Arc` / `Rc` / `Cow` / `BTreeMap` / `HashMap` / `HashSet` / `BTreeSet`

これらが具象型を伴わず単独で宣言された場合、contract-map の型名抽出は wrapper 名 token しか返さず、内部具象型への edge が生まれない。

### R9. No Primitive Obsession (制約ある概念を生 primitive で宣言しない)

catalogue の field / payload / param / returns / map キーで、検証可能な制約・有限値集合・ドメイン的意味を持つ概念を生 primitive (`String` / `i32` / `bool` 等) で宣言してはならない。値オブジェクト (`role: ValueObject` の `tuple` shape newtype、または有限値集合の `enum`) を定義して使う。

対象フィールド:

- `kind.shape.fields[].ty` (plain shape) / `kind.shape.fields[]` (tuple shape — 各要素が型文字列)
- `kind.variants[].payload` の field 型 (enum payload)
- `methods[].params[].ty` / `methods[].returns` / FunctionEntry の `params[].ty` / `returns`
- `BTreeMap` / `HashMap` のキー型 (有限集合 → enum、識別子 → 検証付き newtype)

判断手順:

1. その概念に「不正値」が存在するか判定する (空文字禁止 / 特定書式 / 有限集合 / 単位など)
2. 制約があるなら値オブジェクトを定義する。有限集合 → `role: ValueObject` の `enum`、検証付き識別子・値 → `role: ValueObject` の `tuple` shape (newtype) + constructor 検証
3. 生 primitive が正当なのは「真に制約のない不透明値」(検証も有限性もないフリーテキスト等) のみである。その場合は `docs` に生 primitive を選んだ根拠を記録する
4. **serde 境界 (`role: Dto`) も R9 の例外ではない**。`role: Dto` は wire format だが、概念に対応するフィールド・map キー・Vec 要素を生 `String` にしてよい免罪符ではない:
   - フィールド / Vec 要素が domain の VO / enum を表す → infrastructure 側の `deserialize_with` で domain VO / enum へパースする (`Vec<String>` で受けて後段で解釈するのは禁止)
   - **serde map キー** が serde-free な domain enum を表す → infrastructure 側に deserializable な **mirror enum** を定義し (`#[derive(Deserialize)]` + domain enum への `From` / `TryFrom`)、`BTreeMap<MirrorEnum, _>` で受ける。生 `String` キー + runtime 検証へ退避してはならない
   - **設定キーや filter 値が domain 概念を名指すなら、それは概念への参照である**。有限の domain 概念集合を名指すキー / 値は「ただの設定文字列」ではなく、当該 domain enum (serde は infrastructure の mirror 経由) で型付ける。「open-ended だから String」「runtime で検証するから String」は R9 違反である
   - 対応する enum がまだ無ければ、R1 の semantic evidence で配置を判定する。ユビキタス言語・不変条件・operation を越えた安定性・delivery / persistence / workflow からの独立性がある domain 概念なら R10 に従い domain enum を新設する。application boundary にのみ意味を持つ値なら、usecase の `Dto` / `Command` / `Query` / `ValueObject` として型付ける。いずれの場合も生 `String` へ退避してはならない。生 `String` が許されるのは、color やレンダリング構文のように domain 的意味を持たない提示専用値だけである

draft が本ルールに違反する (制約ある概念を生 primitive で宣言している) 場合、orchestrator のレビュー前に self-reject して値オブジェクト化する。

判定例:

- 空文字を禁じる表示用識別子 → `DiagramNodeName(String)` newtype (生 `String` 禁止)
- 有限の分類種別 → domain の `enum` (R10: domain 概念。生 `String` キー禁止)。infrastructure 側の設定 map キーは deserializable な mirror enum で受け、domain enum へ変換する
- 設定ファイルの `roles = ["UseCaseFunction"]` のような有限概念集合の列挙 → `Vec<FunctionRole>` を `deserialize_with` で受ける (`Vec<String>` 禁止)
- 検証も有限性もない任意ラベル / color / レンダリング構文 → 生 `String` 許容 (`docs` に根拠記録)

**根拠**: `prefer-type-safe-abstractions.md` の Make Illegal States Unrepresentable / Newtype。本ルールは利用プロジェクトが所有する方針であり、生 primitive を許容する方針のプロジェクトでは異なりうる。

### R10. Domain Concept → Domain Object in Domain Layer (domain 概念は domain 層にドメインオブジェクトとして定義する)

R10 を適用する前に R1 の semantic-first evidence で候補を分類する。ユビキタス言語に属し、不変条件を所有し、複数の application operation を越えて意味が安定し、persistence・delivery・workflow の都合から独立して存在するなら domain 概念である。same-track の domain-internal inbound reference はその利用を示す補助証拠であり、欠如だけを理由に domain 配置を拒否しない。application boundary にのみ意味を持つ値は usecase の `Dto` / `Command` / `Query` / `ValueObject` として型付ける。

R1 で domain 概念と分類された概念 (ユビキタス言語に現れる名詞: 識別子・数量・分類・ポリシー・状態など) は、必ず **ドメインオブジェクト** として R1 マトリクスで domain 層に合法な role (`ValueObject` / `Entity` / `AggregateRoot` / `DomainService` / `Specification` / `Factory` / `ErrorType`) のいずれかでモデル化し、**domain 層の catalogue に定義する**。どの層がそれを消費するかは問わない。R9 が「概念を生 primitive にしない」を、本 R10 が「domain 概念のドメインオブジェクト化 + domain 層配置 + カタログ宣言」を担う。role 選定は R1〜R6 の判断木に従う。

論理連鎖 (なぜ概念が省略不能か):

1. 候補は R1 の semantic evidence で分類し、根拠を catalogue の `docs` または review 記録に残す
2. domain 概念は **ドメインオブジェクト化** して domain 層に配置する (生 primitive 化は R9 で禁止。`Entity` / `AggregateRoot` / `Specification` は domain ONLY)
3. domain 層の型は他層から参照されるため **`pub` 宣言が必須** である (層 = 別クレート境界。`pub` と公開パスがなければ他層から名前で参照できずコンパイル不能)
4. `pub` 型は **カタログ宣言が必須** である (カタログは public な API surface を写す。source に在る pub 型がカタログ未宣言なら signal が 🔴 になる)
5. ∴ **各 domain 概念は省略不能で domain catalogue に宣言される**。R1 で usecase 境界値と分類された候補は、その usecase catalogue に宣言される

ただし手順 4 の signal 裏打ちは **実装後** にしか効かない (計画段階では概念が source に未在のため、カタログから省いても赤にならない)。よって R10 は **計画段階** で概念のモデル化・配置・宣言を保証する上流ルールであり、R9 と同じく **信号機評価とは別軸** である (全緑でも R10 充足を意味しない)。

**serde / domain 純粋性を概念モデリング省略の口実にしてはならない**:

- domain を serde-free に保つことは、R1 で domain 概念と分類された概念を domain にモデル化しない理由には **ならない**。外部形式 (TOML / JSON 等) から読む必要がある domain 概念は、(a) domain 層に serde-free なドメインオブジェクトを定義し、(b) infrastructure 層に `role: Dto` の serde DTO を定義して相互変換する (R1: `Dto` は infrastructure)。purity は「domain モデル + infrastructure DTO」の対で解決する
- 「serde が要るから infrastructure の生 struct に留める」「R1 の分類をせずに概念をカタログから省略する」は **いずれも R10 違反** である。R1 で usecase 境界値と分類された候補は、domain ではなく usecase catalogue に型付けて宣言する

判別 (R1 による分類):

- ドメイン的意味 (ドメインエキスパートとの会話に現れるか)、不変条件の所有、operation を越えた意味の安定性、delivery / persistence / workflow からの独立性を合わせて判断する。serde・外部形式・表示の都合は判別に **関与しない** (それは配置ではなく DTO 変換の問題である)。same-track inbound reference は補助証拠であり、意味分類を置き換えない
- ドメイン的意味を一切持たない純粋な技術ノブ (adapter 内部のバッファサイズ・リトライ回数など) は domain に置かない。R1〜R6 を適用しても role または配置が確定しない場合は、R5 に従い `## Open Questions` に escalation し、曖昧さだけを理由に domain に配置してはならない

判定例:

- domain entry が参照する「許可された種別の有限集合」という概念 → semantic evidence が domain を示すなら `role: ValueObject` の `enum` を定義する。外部設定から読むなら infrastructure に `role: Dto` を置いて変換する。`Vec<String>` を infrastructure に持つのは R9 + R10 違反
- domain の不変条件を表す検証付き識別子の概念 → domain に `role: ValueObject` newtype。infrastructure DTO のフィールドも生 `String` にはしない。`deserialize_with` で受けてフィールド型を domain VO にするか、serde-free な domain enum を持つ場合は infrastructure 側に deserializable な mirror newtype を定義して変換する。application boundary にのみ意味を持つ値は usecase の `Dto` / `Command` / `Query` / `ValueObject` として型付ける

**根拠**: domain は `architecture-rules.json` で `may_depend_on: []` として定義される最内層である。

## Examples

### Good

- `parse_config` を `role: FreeFunction` で `infrastructure-types.json` の `functions` エントリに置く (R2)
- `evaluate_discount` を `role: FreeFunction` で `domain-types.json` の `functions` エントリに置く (R2 + R1: `FreeFunction` は layer-flexible)
- 検証済みの shared payload record を `role: ValueObject` で domain の `types` エントリに置く (R3: behavior なし)
- 注文の各状態を `role: ValueObject` + typestate marker 付き `struct` で domain に置き、heterogeneous な集合用に `role: ValueObject` + `kind: { "kind": "enum" }` の wrapper を並置する (state machine + heterogeneous Vec の決定木)
- `PostgresUserRepository` を `role: SecondaryAdapter` で infrastructure の `types` エントリに置く (R1: `SecondaryAdapter` は infrastructure ONLY)
- baseline 由来の `UserRepository` port を当該 track の `domain-types.json` に `action: "reference"` + `role: Repository` の `traits` エントリとして declare する (R7: declare により `PostgresUserRepository -.impl.-> UserRepository` edge が contract-map に出る)
- `methods[].returns` に `"Result<Config, ConfigParseError>"` と完全型文字列を書く (R8: `Config` / `ConfigParseError` への edge が生成できる)

### Bad

- `ConfigCodec` (parse method を持つ struct) を `role: ValueObject` で起草する (R3 違反: behavior を持つ)
  - 正しい修正: `parse_config` を `role: FreeFunction` に分解する (R2)
- `PostgresUserRepository` を `role: UseCase` で `infrastructure-types.json` に起草する (R1 違反: `UseCase` は usecase ONLY)
  - 正しい修正: R1 の層配置違反を直し、driven adapter の port 実装なら `role: SecondaryAdapter` を置く。application のユースケース自身の inbound port なら `role: ApplicationService` + `role: Interactor` の structure-required な組を複数実装や service 自体のテスト差し替えなしでも置く。structure-required な組とは別の任意 service-level 抽象なら、D2 の必要性テストを適用し、共有所有だけなら `Arc<具象型>` を既定として不要な抽象を追加しない
- 状態遷移を持つ注文を `role: ValueObject` + `kind: { "kind": "enum" }` (`OrderStatus { Draft, Placed, ... }`) で起草し、別 entry に `status: OrderStatus` / `placed_at: Option<Timestamp>` を持つ struct を置く (R3 違反 + 決定木違反)
  - 正しい修正: typestate cluster + enum wrapper にする (各 state を `role: ValueObject` + typestate marker 付き `struct` で起草し、heterogeneous Vec 用の enum wrapper を追加する)
- 「他の role が fit しないので」という理由で `role: ValueObject` を選ぶ (R5 違反)
  - 正しい修正: 決定木を再適用し、`role: FreeFunction` 候補を検討する。それでも確定しないなら `## Open Questions` に escalation する
- baseline 由来の port を implement する adapter を `role: SecondaryAdapter` で起草したが、当該 track の catalogue にその port の `traits` entry を declare しない (R7 違反: declare 漏れによる `-.impl.->` edge の黙殺)
  - 正しい修正: 当該 port を `action: "reference"` で `role: SecondaryPort` または `role: Repository` の `traits` エントリとして declare する
- `methods[].returns` / `methods[].params[].ty` / FunctionEntry の `returns` / `params[].ty` を bare wrapper 名のみで宣言する (R8 違反: edge 漏れの原因)
  - 悪い例: `returns: "Result"` / `ty: "Arc"` / `ty: "Vec"`
  - 正しい修正: `returns: "Result<Config, ConfigParseError>"` / `ty: "Arc<dyn UserRepository>"` / `ty: "Vec<Order>"`

## Review Checklist

type-designer 自身および reviewer は draft 段階で以下を確認する。

- [ ] 各 entry の `role` × layer の組合せが R1 マトリクスで OK か (`✗` / ONLY 違反がないか、`DomainService` が infrastructure 層に置かれていないか)
- [ ] ゼロフィールド struct + 1 method の entry がないか (あれば R2: `role: FreeFunction` に折り畳めないか確認する)
- [ ] `role: ValueObject` の entry がすべて R3 を満たすか (値等価で識別され、`methods` が生成時に不変条件を確立する constructor / validation、または自身の値から値・述語を導出する side-effect-free なものに限られるか。依存や外部リソースを扱う service 的 behavior がないか。typestate state の entry は遷移メソッドを `methods` に持つか)
- [ ] R6 の採用条件を満たし、値等価で識別される ValueObject (R3) ではない、field + behavior を持つ domain struct が `role: DomainService` で起草されているか (`role: ValueObject` / `role: Interactor` への誤分類がないか)
- [ ] role 起草前に偵察 (R4) を実施したか (近接 track の role 分布を確認したか。参照先が存在しない場合はその確認をしたか)
- [ ] catch-all として `role: ValueObject` / `role: UseCase` を選んでいないか (R5)
- [ ] top-level `trait_impls[]` のうち `for_type` が `role: SecondaryAdapter` の型を指す entry の `trait_ref` が参照するすべての port が、当該 track の catalogue に `traits` エントリ (role は `SecondaryPort` または `Repository`) として declare されているか (R7)。baseline 由来の port は `action: "reference"` で declare されているか
- [ ] `methods[].returns` / `methods[].params[].ty` および FunctionEntry の `returns` / `params[].ty` に bare wrapper 名のみの宣言 (`Result` / `Option` / `Vec` / `Box` / `Arc` / `Rc` / `Cow` / `BTreeMap` / `HashMap` / `HashSet` / `BTreeSet`) がないか (R8)
- [ ] field / payload / param / returns / map キーで、制約ある概念を生 primitive (`String` 等) で宣言していないか (R9)。制約があれば値オブジェクト (newtype / enum) を定義しているか。**`role: Dto` / serde 境界も例外ではない** — 概念を名指す map キーや filter 値を domain enum (serde は infrastructure の mirror enum 経由) で型付けているか。生 primitive は color や自由ラベル等の真に不透明な提示専用値のみで、その場合 `docs` に根拠が記録されているか
- [ ] `ValueObject` 候補を R1 の semantic-first evidence (ユビキタス言語、不変条件、operation を越えた安定性、delivery / persistence / workflow からの独立性) で分類し、根拠を記録したか。domain 概念は R1 マトリクスで domain 層に合法な role で domain 層に定義し、domain catalogue に宣言しているか (R10)。same-track inbound reference は補助証拠としてのみ扱ったか。application boundary 値は usecase の `Dto` / `Command` / `Query` / `ValueObject` として型付け、usecase catalogue に宣言しているか。serde / 外部形式の都合を口実に、R1 の分類をせずに概念を infrastructure の生 struct に留めたりカタログから省略したりしていないか
- [ ] R1〜R10 のいずれでも判断できない entry が `## Open Questions` に escalation されているか

## Enforcement

- 第一線: catalogue を起草する capability が本 convention を必読として解決し、compliance を負う (`required_for` frontmatter による解決)
- 第二線: catalogue-lint が、出荷 catalogue-lint config の `KindLayerConstraint` で active track の R1 forbidden な role × layer 組合せを signal 評価より先に reject する。これは config を消費する lint gate である
- 第三線: track ごとの reviewer briefing で本 convention を参照し、R1〜R10 の checklist を review 観点として明示する。`.harness/custom/review-prompts/<scope>.md` は利用者所有の severity policy であり、方法論そのものの enforcement source にはしない
- 第四線: catalogue → spec の trace integrity を測る signal 評価。role 違反は第二線で draft 段階に却下されるため、signal は最終 backstop の位置づけである

## Related Documents

- `prefer-type-safe-abstractions.md` — enum-first / typestate / newtype の design principle (本 convention はその role 選定への適用)
- `architecture-rules.json` — 層 id と TDDD 対応層の SSoT (R1 の層列挙の根拠)
- `.harness/catalogue-lint/config.json` — R1 マトリクスの機械写像 (`KindLayerConstraint` の `permitted_layers`)
- `.harness/reference/catalogue-schema.md` — catalogue の wire format と role 語彙の参照
- `.harness/policies/pre-track-adr-authoring.md` — ADR 配置規則 (catalogue の上流 SSoT)
- `knowledge/adr/README.md` — 設計判断の索引
