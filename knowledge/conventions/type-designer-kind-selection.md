# Type-Designer Kind Selection Convention

## Purpose

`type-designer` agent が `<layer>-types.json` を起草する際、`TypeDefinitionKind` の選定ミスや層配置違反を agent 自身で構造的に防ぐための拘束ルール集。

このハーネスにおける型設計は type-designer の専門領域であり、orchestrator / user が事後に kind 選定ミスを指摘して redesign を迫る運用は逆転している。本 convention は type-designer が **自律的に正しい kind を選び、誤った fallback を避ける** ための判断基準を SSoT として明示する。

過去のセッションで観察された type-designer の典型逸脱:

- 状態遷移ありの型に `kind: enum` (status field + Option<...>) を選び typestate cluster を回避
- usecase 層以外の layer (domain / infrastructure) に `kind: use_case` / `kind: application_service` / `kind: interactor` を配置
- ゼロフィールド struct + 1 method の型を `kind: value_object` で「validated value」の意味から大きく外して使用
- 他の kind が fit しないときに `kind: value_object` を catch-all として採用 (semantic stretch)

## Scope

- 適用対象:
  - `type-designer` agent
  - すべての TDDD 対応層 (`architecture-rules.json` の `tddd.enabled: true`) における `<layer>-types.json` の起草・更新
  - 各 entry の `kind` 選定、`expected_*` フィールド設計、層配置判断
- 適用外:
  - `spec-designer` / `impl-planner` / `adr-editor` の owned artifact
  - kind が確定済みで構造変更を伴わない `action: "modify"` 編集 (フィールド追加など) ※ kind 変更を含む場合は本 convention 対象

## Rules

### R1. Layer-Kind Compatibility (層 × kind 互換マトリクス)

`<layer>-types.json` の各 entry は、層と kind の組合せを以下の表に従う。Forbidden の組合せを起草してはならない。

> 本マトリクスは **層配置** の制約のみを規定する。各 kind が必要とする具体的フィールド (`expected_members`, `expected_methods`, `implements` 等) の定義は `libs/domain/src/tddd/catalogue.rs` の `TypeDefinitionKind` を正本とする。

| kind | domain | usecase | infrastructure | 配置根拠 |
|---|---|---|---|---|
| `typestate` | ✓ | △ | △ | 状態遷移は domain 概念。usecase / infra に置くのは domain leak の疑い |
| `enum` | ✓ | ✓ | ✓ | layer-flexible (有限の値集合を表現する変種型) |
| `value_object` | ✓ | △ | △ | "validated value" は domain 概念。layer-flexible だが domain 外配置は要根拠 |
| `domain_service` | ✓ | △ | ✗ | domain knowledge を集約する behavior 中心 struct (S1)。usecase は trans-domain な application logic で要根拠 |
| `error_type` | ✓ | ✓ | ✓ | layer-flexible (各層がそれぞれの責務に応じた error 型を持つ) |
| `secondary_port` | ✓ | ✓ | ✗ | hexagonal: driven port は domain または usecase に置く (CN-05; usecase port 例: `Reviewer`, `DiffGetter`) |
| `application_service` | ✗ | **✓ ONLY** | ✗ | hexagonal: driving port (use-case interface) は usecase layer |
| `use_case` | ✗ | **✓ ONLY** | ✗ | name と意味が usecase 層を表す。他層は kind 違反 |
| `interactor` | ✗ | **✓ ONLY** | ✗ | application_service trait の実装。usecase 層 |
| `dto` | ✗ | △ | **✓** | serde 境界 = infrastructure (CN-05: domain は serde-free)。usecase は要根拠 |
| `command` | ✗ | **✓ ONLY** | ✗ | CQRS command。usecase 層が受け取る入力 |
| `query` | ✗ | **✓ ONLY** | ✗ | CQRS query。usecase 層が受け取る入力 |
| `factory` | ✓ | ✓ | △ | 集約 / entity factory。infrastructure に置くのは要根拠 |
| `secondary_adapter` | ✗ | ✗ | **✓ ONLY** | secondary_port の実装 = infrastructure (CN-05) |
| `free_function` | ✓ | ✓ | ✓ | layer-flexible (top-level pub fn) |

凡例: `✓` = OK, `△` = 要根拠 (default ではない、informal_grounds[] で説明)、`✗` = forbidden, `**ONLY**` = この層以外で使うことを禁止

`✗` または **ONLY** を破る kind 選択は、`bin/sotp track type-signals` の signal 評価以前に **kind 違反** として draft 段階で却下する。

### R2. Free Function Preference (stateless behavior は free_function)

以下の条件をすべて満たす型は `kind: free_function` で起草する。zero-field struct + 1 method を `kind: value_object` / `kind: use_case` に matching するのは禁止する。

- top-level の pub fn (struct や trait の method ではない)
- またはゼロフィールド struct で、その「struct」が表す唯一の責務が 1 つの pub fn 呼び出しに帰着する
- 内部 state を持たない (struct field なし、または `()` のみ)
- 依存注入を必要としない (依存ありなら interactor / use_case / secondary_adapter)

判定例:

- `parse_yaml_frontmatter(input: &str) -> Result<AdrFrontMatter, AdrFrontMatterCodecError>` → `free_function` (state なし、依存なし)
- `evaluate_adr_decision(entry: &AdrDecisionEntry) -> AdrSignal` → `free_function`
- `EvaluateAdrDecision { /* zero fields */ } impl { fn evaluate(&self, ...) -> ... }` → 設計を `free_function` に折り畳む。zero-field struct は wrapping だけで意味を加えない

例外: トレイト境界に組み込む必要がある (`Arc<dyn Service>` で渡したい等) 場合は `interactor` + `application_service` ペアを使う。free_function は trait 境界に組み込めない。

### R3. value_object Semantic Restriction (validated value のみ)

`kind: value_object` は「validated value (検証済み値)」の意味に厳格に限定する。**behavior を持つ struct は value_object ではない**。

| OK (value_object) | NG (value_object 違反) |
|---|---|
| `Email(String)` newtype + `new()` で形式検証 | `parse_*` / `evaluate_*` / `compute_*` などの計算 method を持つ struct |
| `AdrDecisionCommon { id, refs, ... }` 検証付き shared payload | `Codec` / `Validator` / `Resolver` のような behavior 中心の struct |
| 複合 primitive を集めた読み取り専用の record | trait 実装を意図する struct (→ `interactor` / `secondary_adapter`) |

「validated value」の判定基準: `new()` (またはコンストラクタ) で field に格納される値の不変条件 (invariant) を確立し、その後は read-only として参照されるか。値そのものを返す getter / accessor は OK。**値以外の何かを計算して返す method は behavior** であり value_object 違反。

> M1 以降の schema 上は struct 系全 9 kind が `expected_methods` フィールドを uniform に持つが、`value_object` で behavior method を宣言することは依然として違反である。S3 linter の `FieldEmpty` rule (`target_kind=value_object`, `target_field=expected_methods`) で機械的に enforce する。

behavior を持つ struct は以下のいずれかに振り分ける:

- 依存なし stateless → `free_function` (R2)
- 依存あり (port を呼び出す) → `interactor` (usecase) または `secondary_adapter` (infrastructure)
- 集約構築 → `factory`
- 状態遷移あり → `typestate` cluster
- field を持つ domain behavior (状態遷移なし、依存なし) → `domain_service` (R6)

### R4. Kind Distribution Reconnaissance (起草前の偵察義務)

新規 catalogue の draft を書き始める前に、既存 track の catalogue から kind 分布を調査して当 track の起草の参照基準にする。reconnaissance ステップ (baseline-capture → type-graph d1/d2 → Read) と並行して実施する。

調査内容:

- 既に完了済みの近接 track (同じ layer / 同じ ADR を参照) の `<layer>-types.json` を 1〜3 件 sample
- そこで採用されている kind の分布 (どの kind がどれだけ使われているか)
- naming convention (PascalCase struct / snake_case fn / `*Error` / `*Port` / `*Adapter` 等の suffix)
- value_object と free_function の使い分け実例

この偵察により、特定 kind を「思い出した順」で機械的に当てはめる代わりに、**プロジェクト全体の kind 配分との整合** を保った起草が可能になる。偵察結果は internal preparation であり final report に出さなくてよい (orchestrator 出力には影響させない)。

例: ADR が「parse」「evaluate」のような stateless behavior を要求しているのに、過去 track で類似機能が `free_function` で実装されている場合、当該 track でも `free_function` を採用する。`use_case` / `value_object` を選択した場合、その rationale を `informal_grounds[]` に記録する。

### R5. No Fallback Rule (catch-all 禁止)

「他の kind が完全に fit しない」という理由で `value_object` または `use_case` を catch-all として採用してはならない。

判断手順:

1. 候補 kind を列挙し、R1 マトリクスで層と kind の組合せを絞り込む
2. kind が確定しない場合 → R2 (free_function) と R3 (value_object 制限) を再確認
3. それでも確定しない場合 → R6 (`domain_service`) の判定基準で domain 層 behavior の住所として fit するか確認
4. それでも確定しない場合 → 起草を止め、`## Open Questions` に「kind が確定しない理由」と「検討した候補とその却下理由」を列挙して orchestrator に escalation
5. orchestrator は ADR / spec の補強 (adr-editor / spec-designer の re-invoke) または user 判断を仰ぐ

`value_object` で迷ったときの最も多い真の答えは `free_function` (R2) である。次に多いのは `interactor` (依存あり) / `secondary_adapter` (port 実装) / `domain_service` (R6: field を持つ domain behavior)。`value_object` を選ぶ前に、これらの候補を必ず検討する。

### R6. domain_service Selection Criteria (S1: field を持つ domain behavior の住所)

`kind: domain_service` は **field を持ち behavior method を持つ domain struct** の正しい住所である。`value_object` (R3 違反) や `interactor` (依存ありの usecase 層) との混同を防ぐため、以下の全条件を満たす場合に採用する。

採用条件 (AND):

- struct (enum / typestate cluster ではない)
- `expected_members` >= 1 field (state を保持する; ゼロフィールドは R2 の free_function 候補)
- `expected_methods` >= 1 method (behavior を持つ; ゼロメソッドは R3 の value_object 候補)
- 状態遷移なし (ある場合は typestate cluster — R3 の振り分け)
- `application_service` / `secondary_port` の実装ではない (実装する場合は `interactor` / `secondary_adapter`)
- 配置層は domain (default) / usecase (要根拠 — trans-domain な application logic で domain knowledge を集約する場合のみ、`informal_grounds[]` に記録) / infrastructure (forbidden)

判定例:

- `PolicyEvaluator { rules: Vec<Rule> }` + `evaluate(&self, ctx: &Context) -> Decision` → `domain_service` (state あり、behavior あり、依存なし)
- `Email(String)` + `new()` のみ → `value_object` (R3: 検証済み値、behavior なし)
- `parse_yaml(input: &str) -> Result<...>` → `free_function` (R2: state なし、依存なし)
- `RegisterUserInteractor { repo: Arc<dyn UserRepository> }` + `execute(&self, cmd) -> ...` → `interactor` (R1: 依存あり、usecase 層)

## Examples

### Good

- `parse_adr_frontmatter` を `kind: free_function` で `infrastructure-types.json` に置く (R2)
- `evaluate_adr_decision` を `kind: free_function` で `domain-types.json` に置く (R2 + R1: free_function は layer-flexible)
- `AdrDecisionCommon { id, user_decision_ref, ... }` を `kind: value_object` で domain に置く (R3: 検証済み shared payload で behavior なし)
- `ProposedDecision` / `AcceptedDecision` / ... を `kind: typestate` で domain に置き、`AdrDecisionEntry` を `kind: enum` の wrapper として並置 (decision tree: state machine + heterogeneous Vec)
- `FsAdrFileAdapter` を `kind: secondary_adapter` で infrastructure に置く (R1: secondary_adapter は infrastructure ONLY)

### Bad

- `AdrFrontMatterCodec` (parse method を持つ struct) を `kind: value_object` で起草 (R3 違反: behavior を持つ)
  - 正しい修正: `parse_adr_frontmatter` を `kind: free_function` に分解 (R2)
- `AdrSignalsVerifyAdapter` を `kind: use_case` で `infrastructure-types.json` に起草 (R1 違反: use_case は usecase ONLY)
  - 正しい修正: usecase 層に `interactor` + `application_service` ペアを置き、infrastructure には `secondary_adapter` を置く
- 状態遷移を持つ ADR decision を `kind: enum` (`DecisionStatus { Proposed, Accepted, ... }`) + `value_object` (`status: DecisionStatus`, `implemented_in: Option<String>`) で起草 (R3 違反 + 決定木違反)
  - 正しい修正: typestate cluster + enum wrapper (各 state を `kind: typestate` で起草し、heterogeneous Vec 用の enum wrapper を `kind: enum` で追加)
- 「他の kind が fit しないので」という理由で `kind: value_object` を選ぶ (R5 違反)
  - 正しい修正: 決定木を再適用 → `free_function` 候補を検討 → それでも確定しないなら `## Open Questions` に escalation

## Review Checklist

type-designer 自身および reviewer は draft 段階で以下を確認する:

- [ ] 各 entry の `kind` × layer の組合せが R1 マトリクスで OK か (✗ / ONLY 違反がないか、`domain_service` は infrastructure 層に置かれていないか)
- [ ] zero-field struct + 1 method の entry がないか (あれば R2: free_function に折り畳めないか確認)
- [ ] `kind: value_object` の entry がすべて R3 を満たすか (validated value のみで behavior を持たないか、`expected_methods` が空か)
- [ ] field + behavior を持つ domain struct が `domain_service` (R6) で起草されているか (`value_object` / `interactor` への誤分類がないか)
- [ ] kind 起草前に偵察 (R4) を実施したか (近接 track の kind 分布を確認したか)
- [ ] catch-all として `value_object` / `use_case` を選んでいないか (R5)
- [ ] R1〜R6 のいずれかで判断不能な entry が `## Open Questions` に escalation されているか

## Enforcement

- 第一線: catalogue を起草する agent の定義で本 convention の reading + compliance を義務付ける
- 第二線: reviewer briefing template (将来 `track/review-prompts/<scope>.md` 配下に追加可能) に R1〜R6 の checklist を埋め込む
- 第三線: `bin/sotp track type-signals` の signal 評価 (catalogue → spec の trace integrity)。kind 違反は signal 評価より先に draft 段階で却下するため、検証の網としては最終 backstop の位置づけ

将来の自動化候補: catalogue codec (`libs/infrastructure/src/tddd/catalogue_codec.rs`) で R1 layer-kind マトリクスを machine-readable に表現し、`bin/sotp` の codec validation で reject する (`forbidden` 組合せ → codec error)。

## Related Documents

- `.claude/rules/04-coding-principles.md` — enum-first / typestate / newtype の design principle (本 convention は kind 選定への適用)
- `knowledge/conventions/hexagonal-architecture.md` — layer 境界と port placement (R1 の根拠)
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR 配置規則 (catalogue の上流 SSoT)
- `architecture-rules.json` — TDDD 対応層の SSoT (R1 layer 列挙の根拠)
- `libs/domain/src/tddd/catalogue.rs` — `TypeDefinitionKind` enum 定義 (kind variant の正本)
- `libs/infrastructure/src/tddd/catalogue_codec.rs` — catalogue serde codec (将来の R1 自動化候補)
