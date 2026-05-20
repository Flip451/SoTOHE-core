# Type-Designer Kind Selection Convention

## Purpose

`type-designer` agent が `<layer>-types.json` を起草する際、role 選定ミスや層配置違反を agent 自身で構造的に防ぐための拘束ルール集。

このハーネスにおける型設計は type-designer の専門領域であり、orchestrator / user が事後に role 選定ミスを指摘して redesign を迫る運用は逆転している。本 convention は type-designer が **自律的に正しい role を選び、誤った fallback を避ける** ための判断基準を SSoT として明示する。

過去のセッションで観察された type-designer の典型逸脱:

- 状態遷移ありの型に `role: ValueObject` + `kind: { "kind": "enum" }` (status field + Option<...>) を選び typestate pattern を回避
- usecase 層以外の layer (domain / infrastructure) に `role: UseCase` / `role: ApplicationService` / `role: Interactor` を配置
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

## Rules

### R1. Layer-Kind Compatibility (層 × kind 互換マトリクス)

`<layer>-types.json` の各 entry は、層と kind の組合せを以下の表に従う。Forbidden の組合せを起草してはならない。

> **v3 schema (schema_version=3) の対応**: v3 catalogue では `TypeDefinitionKind` 単一 enum が廃止され、**role 軸 × kind 軸** の 2 軸構造に変わった。type-designer は v3 format で `<layer>-types.json` を起草する。本マトリクスの「kind」列は v3 における **role フィールドの値** に対応する (`DataRole` / `ContractRole` / `FunctionRole` の variant 名)。type-designer は role と layer の組合せを本マトリクスで確認する。
>
> - v3 wire format: `schema_version: 3`, `crate_name`, `layer`, `types: {}` (TypeEntry), `traits: {}` (TraitEntry), `functions: {}` (FunctionEntry), `inherent_impls: []` / `trait_impls: []` の 2 つの top-level array のトップレベル構造。`trait_impls` は `action` / `trait_ref` / `for_type` を持つ独立 entry (`TraitImplDeclV2`); `inherent_impls` は `action` を持たず `type_name` / `impl_generics` / `impl_where_predicates` / `methods` を持つ (`InherentImplDeclV2`。action はなく target type への帰属で識別される)。JSON 上の top-level 配置は共通だが、`action` の有無・フィールド構造は異なる
> - v3 roles: `types` エントリは `role: DataRole` (13 値: `ValueObject` / `Entity` / `AggregateRoot` / `DomainService` / `Specification` / `Factory` / `UseCase` / `Interactor` / `Command` / `Query` / `Dto` / `ErrorType` / `SecondaryAdapter`)、`traits` エントリは `role: ContractRole` (3 値: `SpecificationPort` / `ApplicationService` / `SecondaryPort`)、`functions` エントリは `role: FunctionRole` (2 値: `FreeFunction` / `UseCaseFunction`)
> - v3 kind (構造軸): `types` は `kind: { "kind": "struct" | "enum" | "type_alias", ... }` で記述する
> - 旧 v2 の `type_definitions` / `TypeDefinitionKind` は廃止済み (ADR `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md`)
>
> 本マトリクスは **層配置** の制約のみを規定する。各 role / entry に必要な具体的フィールド (`kind`, `methods` 等) および top-level の `trait_impls` / `inherent_impls` (impl block を独立 entry として持つ array) の定義は `libs/domain/src/tddd/catalogue_v2/` の `TypeEntry` / `TraitEntry` / `FunctionEntry` / `TraitImplDeclV2` / `InherentImplDeclV2` / `CatalogueDocument` を正本とする。

| role (v3) | domain | usecase | infrastructure | 配置根拠 |
|---|---|---|---|---|
| `ValueObject` (DataRole) | ✓ | △ | △ | "validated value" は domain 概念。layer-flexible だが domain 外配置は要根拠 |
| `Entity` (DataRole) | ✓ | ✗ | ✗ | entity は domain 概念。他層での使用は domain leak |
| `AggregateRoot` (DataRole) | ✓ | ✗ | ✗ | aggregate root は domain 概念 |
| `DomainService` (DataRole) | ✓ | △ | ✗ | domain knowledge を集約する behavior 中心 struct。usecase は trans-domain な application logic で要根拠 |
| `Specification` (DataRole) | ✓ | ✗ | ✗ | domain predicate。他層は domain leak |
| `Factory` (DataRole) | ✓ | ✓ | △ | 集約 / entity factory。infrastructure に置くのは要根拠 |
| `UseCase` (DataRole) | ✗ | **✓ ONLY** | ✗ | name と意味が usecase 層を表す。他層は役割違反 |
| `Interactor` (DataRole) | ✗ | **✓ ONLY** | ✗ | ApplicationService trait の実装。usecase 層 |
| `Command` (DataRole) | ✗ | **✓ ONLY** | ✗ | CQRS command。usecase 層が受け取る入力 |
| `Query` (DataRole) | ✗ | **✓ ONLY** | ✗ | CQRS query。usecase 層が受け取る入力 |
| `Dto` (DataRole) | ✗ | △ | **✓** | serde 境界 = infrastructure (CN-05: domain は serde-free)。usecase は要根拠 |
| `ErrorType` (DataRole) | ✓ | ✓ | ✓ | layer-flexible (各層がそれぞれの責務に応じた error 型を持つ) |
| `SecondaryAdapter` (DataRole) | ✗ | ✗ | **✓ ONLY** | secondary port の実装 = infrastructure (CN-05) |
| `SpecificationPort` (ContractRole) | ✓ | ✗ | ✗ | driven port は domain (hexagonal) |
| `SecondaryPort` (ContractRole) | ✓ | ✓ | ✗ | hexagonal: driven port は domain または usecase (CN-05; usecase port 例: `Reviewer`, `DiffGetter`) |
| `ApplicationService` (ContractRole) | ✗ | **✓ ONLY** | ✗ | hexagonal: driving port (use-case interface) は usecase layer |
| `FreeFunction` (FunctionRole) | ✓ | ✓ | ✓ | layer-flexible (top-level pub fn) |
| `UseCaseFunction` (FunctionRole) | ✗ | **✓ ONLY** | ✗ | use-case entrypoint function。usecase 層 |

凡例: `✓` = OK, `△` = 要根拠 (default ではない、docs フィールドに根拠を記録)、`✗` = forbidden, `**ONLY**` = この層以外で使うことを禁止

`✗` または **ONLY** を破る role × layer 選択は、`bin/sotp track type-signals` の signal 評価以前に **role 違反** として draft 段階で却下する。

R7 (Cross-Track Port Reference) も参照すること: top-level `trait_impls` のうち `for_type` が `SecondaryAdapter` 型を指す entry の `trait_ref` が参照する port が当該 track の catalogue に未 declare の場合、`-.impl.->` edge が silently skip される。

### R2. Free Function Preference (stateless behavior は FreeFunction)

以下の条件をすべて満たす型は `role: FreeFunction` (`functions` エントリ) で起草する。zero-field struct + 1 method を `role: ValueObject` / `role: UseCase` に matching するのは禁止する。

- top-level の pub fn (struct や trait の method ではない)
- またはゼロフィールド struct で、その「struct」が表す唯一の責務が 1 つの pub fn 呼び出しに帰着する
- 内部 state を持たない (struct field なし、または `()` のみ)
- 依存注入を必要としない (依存ありなら `role: Interactor` / `role: UseCase` / `role: SecondaryAdapter`)

判定例:

- `parse_yaml_frontmatter(input: &str) -> Result<AdrFrontMatter, AdrFrontMatterCodecError>` → `role: FreeFunction` (state なし、依存なし)
- `evaluate_adr_decision(entry: &AdrDecisionEntry) -> AdrSignal` → `role: FreeFunction`
- `EvaluateAdrDecision { /* zero fields */ } impl { fn evaluate(&self, ...) -> ... }` → 設計を `role: FreeFunction` に折り畳む。zero-field struct は wrapping だけで意味を加えない

例外: トレイト境界に組み込む必要がある (`Arc<dyn Service>` で渡したい等) 場合は `role: Interactor` + `role: ApplicationService` ペアを使う。`FreeFunction` は trait 境界に組み込めない。

### R3. ValueObject Semantic Restriction (validated value のみ)

`role: ValueObject` は「validated value (検証済み値)」の意味に厳格に限定する。**behavior を持つ struct は ValueObject ではない**。

| OK (ValueObject) | NG (ValueObject 違反) |
|---|---|
| `Email(String)` newtype + `new()` で形式検証 | `parse_*` / `evaluate_*` / `compute_*` などの計算 method を持つ struct |
| `AdrDecisionCommon { id, refs, ... }` 検証付き shared payload | `Codec` / `Validator` / `Resolver` のような behavior 中心の struct |
| 複合 primitive を集めた読み取り専用の record | trait 実装を意図する struct (→ `role: Interactor` / `role: SecondaryAdapter`) |

「validated value」の判定基準: `new()` (またはコンストラクタ) で field に格納される値の不変条件 (invariant) を確立し、その後は read-only として参照されるか。値そのものを返す getter / accessor は OK。**値以外の何かを計算して返す method は behavior** であり `ValueObject` 違反。

behavior を持つ struct は以下のいずれかに振り分ける:

- 依存なし stateless → `role: FreeFunction` (R2)
- 依存あり (port を呼び出す) → `role: Interactor` (usecase) または `role: SecondaryAdapter` (infrastructure)
- 集約構築 → `role: Factory`
- 状態遷移あり → `kind: { "kind": "struct", "pattern": { "pattern": "typestate_state", ... } }` cluster (`role: ValueObject` + typestate pattern で各 state を表現し、遷移メソッドを `methods` に宣言)
- field を持つ domain behavior (状態遷移なし、依存なし) → `role: DomainService` (R6)

### R4. Kind Distribution Reconnaissance (起草前の偵察義務)

新規 catalogue の draft を書き始める前に、既存 track の catalogue から role 分布を調査して当 track の起草の参照基準にする。reconnaissance ステップ (baseline-capture → type-graph d1/d2 → Read) と並行して実施する。

調査内容:

- 既に完了済みの近接 track (同じ layer / 同じ ADR を参照) の `<layer>-types.json` を 1〜3 件 sample
- そこで採用されている role の分布 (どの role がどれだけ使われているか)
- naming convention (PascalCase struct / snake_case fn / `*Error` / `*Port` / `*Adapter` 等の suffix)
- `role: ValueObject` と `role: FreeFunction` の使い分け実例

この偵察により、特定 role を「思い出した順」で機械的に当てはめる代わりに、**プロジェクト全体の role 配分との整合** を保った起草が可能になる。偵察結果は internal preparation であり final report に出さなくてよい (orchestrator 出力には影響させない)。

例: ADR が「parse」「evaluate」のような stateless behavior を要求しているのに、過去 track で類似機能が `role: FreeFunction` で実装されている場合、当該 track でも `role: FreeFunction` を採用する。`role: UseCase` / `role: ValueObject` を選択した場合、その rationale を `docs` フィールドに記録する。

### R5. No Fallback Rule (catch-all 禁止)

「他の role が完全に fit しない」という理由で `role: ValueObject` または `role: UseCase` を catch-all として採用してはならない。

判断手順:

1. 候補 role を列挙し、R1 マトリクスで層と role の組合せを絞り込む
2. role が確定しない場合 → R2 (`FreeFunction`) と R3 (`ValueObject` 制限) を再確認
3. それでも確定しない場合 → R6 (`DomainService`) の判定基準で domain 層 behavior の住所として fit するか確認
4. それでも確定しない場合 → 起草を止め、`## Open Questions` に「role が確定しない理由」と「検討した候補とその却下理由」を列挙して orchestrator に escalation
5. orchestrator は ADR / spec の補強 (adr-editor / spec-designer の re-invoke) または user 判断を仰ぐ

`role: ValueObject` で迷ったときの最も多い真の答えは `role: FreeFunction` (R2) である。次に多いのは `role: Interactor` (依存あり) / `role: SecondaryAdapter` (port 実装) / `role: DomainService` (R6: field を持つ domain behavior)。`role: ValueObject` を選ぶ前に、これらの候補を必ず検討する。

### R6. DomainService Selection Criteria (S1: field を持つ domain behavior の住所)

`role: DomainService` は **field を持ち behavior method を持つ domain struct** の正しい住所である。`role: ValueObject` (R3 違反) や `role: Interactor` (依存ありの usecase 層) との混同を防ぐため、以下の全条件を満たす場合に採用する。

採用条件 (AND):

- struct (enum / typestate cluster ではない)
- `kind.fields` >= 1 field (state を保持する; ゼロフィールドは R2 の `FreeFunction` 候補)
- `methods` >= 1 entry (behavior を持つ; ゼロメソッドは R3 の `ValueObject` 候補)
- 状態遷移なし (ある場合は typestate pattern — R3 の振り分け)
- `ApplicationService` / `SecondaryPort` の実装ではない (実装する場合は `role: Interactor` / `role: SecondaryAdapter`)
- 配置層は domain (default) / usecase (要根拠 — trans-domain な application logic で domain knowledge を集約する場合のみ、`docs` フィールドに根拠を記録) / infrastructure (forbidden)

判定例:

- `PolicyEvaluator { rules: Vec<Rule> }` + `evaluate(&self, ctx: &Context) -> Decision` → `role: DomainService` (state あり、behavior あり、依存なし)
- `Email(String)` + `new()` のみ → `role: ValueObject` (R3: 検証済み値、behavior なし)
- `parse_yaml(input: &str) -> Result<...>` → `role: FreeFunction` (R2: state なし、依存なし)
- `RegisterUserInteractor { repo: Arc<dyn UserRepository> }` + `execute(&self, cmd) -> ...` → `role: Interactor` (R1: 依存あり、usecase 層)

### R7. Cross-Track Port Reference (SecondaryAdapter が参照する port は当該 track catalogue に declare する)

top-level `trait_impls[]` のうち `for_type` が `role: SecondaryAdapter` の型を指す entry の `trait_ref` で参照する trait (port) は、当該 track の `<layer>-types.json` のいずれかに `role: SecondaryPort` の `traits` エントリとして存在することが必須である。

当該 track で改変しない baseline 由来の port は `action: "reference"` で declare する。declare 漏れは contract-map renderer の `port_index` lookup が unmatched となり、`SecondaryAdapter -.impl.-> port` edge が silently skip される。

#### declare 義務

- top-level `trait_impls[]` に `for_type: <SecondaryAdapter 型>` + `trait_ref: <port>` の entry を書いた以上、対応する `role: SecondaryPort` entry を当該 track の catalogue に作成する責任は type-designer に帰属する
- 当該 track で変更しない baseline 由来の port は `action: "reference"` で declare し catalogue への exposure を確保する

#### `action: "reference"` の semantics

- 当該 track では対象 port を変更しない (新規メソッド追加・既存メソッド変更なし)
- catalogue への exposure (contract-map / graph 描画) を成立させるための declare
- type-signal evaluator は `reference` action に対して「完全一致のみ Blue、不一致はすべて Red」として評価する (modify の Yellow 吸収は適用されない)
- baseline port の `methods` は baseline 当時の全 method を列挙する (method 型宣言の完全形規範 R8 は `reference` action でも同様に要求される)

#### declare 漏れの影響

`port_index: BTreeMap<String, Vec<String>>` は当該 track の `role: SecondaryPort` entry のみを登録する。当該 track の catalogue に `role: SecondaryPort` entry が存在しない trait 名は lookup で unmatched となり、`-.impl.->` edge が生成されない。graph 上の接合点が可視化されず、設計の空白が表面化しにくくなる。

**関連 ADR**: `knowledge/adr/2026-04-29-0243-cross-track-port-reference.md#D1`

### R8. Method Type Full Declaration (method / param 型フィールドは generic 引数を含む完全型文字列で宣言する)

以下のフィールドでは、generic 引数を省略した bare wrapper 名のみの宣言を禁止する:

- `methods[].returns` (TypeEntry / TraitEntry の inherent/trait method)
- `methods[].params[].ty` (同上)
- `params[].ty` (FunctionEntry の関数パラメータ)
- `returns` (FunctionEntry の戻り型)

#### 禁止対象 wrapper 名 (generic 引数なし単独宣言)

`Result` / `Option` / `Vec` / `Box` / `Arc` / `Rc` / `Cow` / `BTreeMap` / `HashMap` / `HashSet` / `BTreeSet`

これらが具象型を伴わず単独で宣言された場合、contract-map renderer の `extract_type_names()` は wrapper 名 token しか返さず、内部具象型への edge が生まれない。

#### lint ゲート

bare wrapper 名のみの宣言を catalogue の codec / verify CLI が schema validation で reject する lint を後続作業として組み込む。実装前は設計レビューで確認する (過渡期間)。

**関連 ADR**: `knowledge/adr/2026-04-29-0240-method-type-full-generic-declaration.md#D1`

## Examples

### Good

- `parse_adr_frontmatter` を `role: FreeFunction` で `infrastructure-types.json` の `functions` エントリに置く (R2)
- `evaluate_adr_decision` を `role: FreeFunction` で `domain-types.json` の `functions` エントリに置く (R2 + R1: `FreeFunction` は layer-flexible)
- `AdrDecisionCommon { id, user_decision_ref, ... }` を `role: ValueObject` で domain の `types` エントリに置く (R3: 検証済み shared payload で behavior なし)
- `ProposedDecision` / `AcceptedDecision` / ... を `role: ValueObject` + `kind: { "kind": "struct", "pattern": { "pattern": "typestate_state", "of": "AdrDecision", "transition_methods": [...] } }` で domain に置き、`AdrDecisionEntry` を `role: ValueObject` + `kind: { "kind": "enum" }` の wrapper として並置 (decision tree: state machine + heterogeneous Vec)
- `FsAdrFileAdapter` を `role: SecondaryAdapter` で infrastructure の `types` エントリに置く (R1: `SecondaryAdapter` は infrastructure ONLY)
- baseline 由来の `ReviewReader` port を当該 track の `domain-types.json` に `action: "reference"` で `role: SecondaryPort` の `traits` エントリとして declare する (R7: declare により `FsReviewStore -.impl.-> ReviewReader` edge が contract-map に出る)
- `methods[].returns` フィールドに `"Result<AdrFrontMatter, AdrFrontMatterCodecError>"` と完全型文字列を書く (R8: `extract_type_names()` が `AdrFrontMatter` / `AdrFrontMatterCodecError` への edge を生成できる)

### Bad

- `AdrFrontMatterCodec` (parse method を持つ struct) を `role: ValueObject` で起草 (R3 違反: behavior を持つ)
  - 正しい修正: `parse_adr_frontmatter` を `role: FreeFunction` に分解 (R2)
- `AdrSignalsVerifyAdapter` を `role: UseCase` で `infrastructure-types.json` に起草 (R1 違反: `UseCase` は usecase ONLY)
  - 正しい修正: usecase 層に `role: Interactor` + `role: ApplicationService` ペアを置き、infrastructure には `role: SecondaryAdapter` を置く
- 状態遷移を持つ ADR decision を `role: ValueObject` + `kind: { "kind": "enum" }` (`DecisionStatus { Proposed, Accepted, ... }`) で起草し、別 entry に `role: ValueObject` + `kind: { "kind": "struct" }` (`status: DecisionStatus`, `implemented_in: Option<String>`) を置く (R3 違反 + 決定木違反)
  - 正しい修正: typestate cluster + enum wrapper (`role: ValueObject` + `kind: { "kind": "struct", "pattern": { "pattern": "typestate_state", "of": "<machine_name>", "transition_methods": [...] } }` で各 state を起草し、heterogeneous Vec 用の enum wrapper を `role: ValueObject` + `kind: { "kind": "enum" }` で追加)
- 「他の role が fit しないので」という理由で `role: ValueObject` を選ぶ (R5 違反)
  - 正しい修正: 決定木を再適用 → `role: FreeFunction` 候補を検討 → それでも確定しないなら `## Open Questions` に escalation
- `FsReviewStore` (baseline 由来の `ReviewReader` / `ReviewWriter` port を implement する adapter) を `infrastructure-types.json` に `role: SecondaryAdapter` で起草したが、当該 track の catalogue に `ReviewReader` / `ReviewWriter` の `role: SecondaryPort` entry を declare しない (R7 違反: declare 漏れによる `-.impl.->` edge の silently skip)
  - 正しい修正: `ReviewReader` / `ReviewWriter` を `action: "reference"` で `domain-types.json` に `role: SecondaryPort` の `traits` エントリとして declare する
- `methods[].returns` / `methods[].params[].ty` / FunctionEntry の `returns` / `params[].ty` を bare wrapper 名のみで宣言する (R8 違反: edge 漏れの原因)
  - 悪い例: `returns: "Result"` / `ty: "Arc"` / `ty: "Vec"`
  - 正しい修正: `returns: "Result<AdrFrontMatter, AdrFrontMatterCodecError>"` / `ty: "Arc<dyn AdrFilePort>"` / `ty: "Vec<AdrDecisionEntry>"`

## Review Checklist

type-designer 自身および reviewer は draft 段階で以下を確認する:

- [ ] 各 entry の `role` × layer の組合せが R1 マトリクスで OK か (✗ / ONLY 違反がないか、`DomainService` は infrastructure 層に置かれていないか)
- [ ] zero-field struct + 1 method の entry がないか (あれば R2: `role: FreeFunction` に折り畳めないか確認)
- [ ] `role: ValueObject` の entry がすべて R3 を満たすか (validated value のみで behavior を持たないか、`methods` が空か — ただし typestate state の entry は遷移メソッドを `methods` に持つため例外)
- [ ] field + behavior を持つ domain struct が `role: DomainService` (R6) で起草されているか (`role: ValueObject` / `role: Interactor` への誤分類がないか)
- [ ] role 起草前に偵察 (R4) を実施したか (近接 track の role 分布を確認したか)
- [ ] catch-all として `role: ValueObject` / `role: UseCase` を選んでいないか (R5)
- [ ] top-level `trait_impls[]` のうち `for_type` が `role: SecondaryAdapter` の型を指す entry の `trait_ref` で参照するすべての trait (port) が当該 track の catalogue に `role: SecondaryPort` の `traits` エントリとして declare されているか (R7)。baseline 由来の port は `action: "reference"` で declare されているか
- [ ] `methods[].returns` / `methods[].params[].ty` (TypeEntry / TraitEntry) および FunctionEntry の `returns` / `params[].ty` に bare wrapper 名のみの宣言 (`Result` / `Option` / `Vec` / `Box` / `Arc` / `Rc` / `Cow` / `BTreeMap` / `HashMap` / `HashSet` / `BTreeSet`) がないか (R8)
- [ ] R1〜R8 のいずれかで判断不能な entry が `## Open Questions` に escalation されているか

## Enforcement

- 第一線: catalogue を起草する agent の定義で本 convention の reading + compliance を義務付ける
- 第二線: reviewer briefing template (将来 `track/review-prompts/<scope>.md` 配下に追加可能) に R1〜R8 の checklist を埋め込む
- 第三線: `bin/sotp track type-signals` の signal 評価 (catalogue → spec の trace integrity)。role 違反は signal 評価より先に draft 段階で却下するため、検証の網としては最終 backstop の位置づけ

将来の自動化候補: catalogue codec (`libs/infrastructure/src/tddd/catalogue_document_codec.rs`) で R1 layer-role マトリクスを machine-readable に表現し、`bin/sotp` の codec validation で reject する (`forbidden` 組合せ → codec error)。

## Related Documents

- `.claude/rules/04-coding-principles.md` — enum-first / typestate / newtype の design principle (本 convention は role 選定への適用)
- `knowledge/conventions/hexagonal-architecture.md` — layer 境界と port placement (R1 の根拠)
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR 配置規則 (catalogue の上流 SSoT)
- `architecture-rules.json` — TDDD 対応層の SSoT (R1 layer 列挙の根拠)
- `libs/domain/src/tddd/catalogue_v2/roles.rs` — `DataRole` / `ContractRole` / `FunctionRole` enum 定義 (v3 schema の role 正本; v2 の `TypeDefinitionKind` に相当)
- `libs/domain/src/tddd/catalogue_v2/entries.rs` — `TypeEntry` / `TraitEntry` / `FunctionEntry` + `TypeKindV2` / `CompositePattern` 定義 (v3 schema の型正本)
- `libs/infrastructure/src/tddd/catalogue_document_codec.rs` — v3 catalogue serde codec (将来の R1 自動化候補; TypeKindDto / PatternDto 等の wire format 定義)
- `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` — v3 schema 設計 ADR (DataRole / ContractRole / FunctionRole 導入の決定記録)
- `knowledge/adr/2026-04-29-0243-cross-track-port-reference.md` — R7 の決定記録 (cross-track port reference の意味論・declare 義務)
- `knowledge/adr/2026-04-29-0240-method-type-full-generic-declaration.md` — R8 の決定記録 (method / param 型フィールドの完全型宣言規範)
