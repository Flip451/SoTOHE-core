---
adr_id: 2026-06-21-1420-cli-layers-tddd-and-role-placement-lint
decisions:
  - id: D1
    user_decision_ref: "chat_segment:adr-add-cli-layers-tddd-role-lint:2026-06-21"
    candidate_selection: "from:[two-layers,three-layers-with-driver,four-layers-with-presentation] chose:three-layers-with-driver"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:adr-add-cli-layers-tddd-role-lint:2026-06-21"
    candidate_selection: "from:[single-primaryadapter,split-compositionroot-and-primaryadapter] chose:split-compositionroot-and-primaryadapter"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:adr-add-cli-layers-tddd-role-lint:2026-06-21"
    candidate_selection: "from:[compose-existing-kindlayerconstraint,new-layerroleallowlist-primitive] chose:compose-existing-kindlayerconstraint"
    status: proposed
---
# cli 系 3 層への TDDD 適用と既存 linter によるロール配置制約の設定

## Context

本 ADR は `2026-06-21-1328-cli-composition-split-presentation-layer.md`（CLI delivery 側の責務分離）の **下流**であり、その分解完了を前提とする。分解後、CLI の delivery 側は **3 層**になる。

- `cli`（`apps/cli`、bin）: clap parse → driver.handle → emit
- `cli_composition`（`apps/cli-composition`、composition root）: 純 DI。secondary adapter + interactor + driver を構築・注入する（invoke しない）
- `cli_driver`（`apps/cli-driver`、primary adapter）: 注入された use case を保持し、handle で use case を呼び（invoke）結果を整形して（render）`CommandOutcome` を返す。invoke と render は同一層（render は層内 module）

これら 3 層はいずれも TDDD カタログ検査の対象外であり、delivery 側全体（入力パース・DI 配線・use case invoke + render）が型カタログ・ロール配置制約の死角になっている。

型カタログ linter（`CatalogueLinter` / `CatalogueLinterRuleKind`）は既に実装済みで、ロールの層配置を制約する `KindLayerConstraint { permitted_layers }` primitive を含む。したがって 3 層のロール配置制約は **新たな機構を足さず、この既存機能を適用する**ことで設定できる。役割-層マトリクス（`type-designer-kind-selection.md` R1）は現状 domain / usecase / infrastructure の 3 層のみを規定し、cli 系の列を持たない。

## Decision

### D1: cli / cli_composition / cli_driver の 3 層を TDDD 有効化する

`architecture-rules.json` の `cli` / `cli_composition` / `cli_driver` レイヤーの `tddd.enabled` を `true` にし、3 層を型カタログ検査の対象に含める。各層とも `catalogue_spec_signal`（SoT Chain ②）と `schema_export`（SoT Chain ③）を有効化した完全 TDDD 層とする。`cli` は workspace で初めて `[[bin]]` crate を TDDD 対象にする事例である。

本 ADR は分解 ADR（`1328`）の完了を前提とするため、`tddd.enabled` のフリップと catalogue 起草は分解後の 3 層構造（純 DI composition root / primary adapter / typed `CompositionError`）に対して行う。

### D2: `CompositionRoot` ロールと `PrimaryAdapter` ロールを新設する

分解後、wire（DI）と invoke+render は別の層に分離される。それぞれに canonical な役割を与える。

- **`CompositionRoot`**（DataRole 新設）: object graph を組む純 DI の住所。`cli_composition` の wiring 構造（driver を構築し use case を注入する）が取る。**invoke しない**。
- **`PrimaryAdapter`**（DataRole 新設）: driving adapter（controller）の住所。`cli_driver` の、注入された use case を保持し handle で呼んで整形する構造が取る。**DI しない**（注入される側）。

両者を分けるのは、`1328` が wire と invoke を層レベルで分離したことの型レベルの反映である（wire と invoke は別責務）。一方 invoke と render は primary adapter の双方向変換の表裏なので分けず、`PrimaryAdapter` が両方を担う。`PrimaryAdapter` 単独で wire まで兼ねると composition root と driving adapter の責務混在になり canonical でない（web の DI コンテナ＝composition root と Controller＝primary adapter の分離に対応）。`CompositionRoot` は `KindLayerConstraint`（cli_composition のみ）、`PrimaryAdapter` は `KindLayerConstraint`（cli_driver のみ）+ `NoRoleInMethodSignature`（domain/usecase 固有ロール型を公開 signature に出さない）という独自の機械検査を持つため、「ロールは独自検査を持つ場合のみ追加」原則を満たす。

### D3: cli 系 3 層の per-layer 使用可能ロールを既存の `KindLayerConstraint` で定義する

per-layer allowlist を直接表現する新 primitive は追加せず、既存の `KindLayerConstraint`（ロール → 許可層）で表現する。各層の使用可能ロールは本 ADR で次のように **定義する**（先送りしない）。allowlist は分解後の各層の責務に基づく。

| 層 | 使用可能ロール（allowlist） | 主な型 |
|---|---|---|
| `cli`（bin） | `Dto` / `ErrorType` / `FreeFunction` | clap `*Args`/`*Command`(Dto) / `CliError`(ErrorType) / dispatch・clap→driver入力変換(FreeFunction) |
| `cli_composition`（wire） | `CompositionRoot` / `ErrorType` | per-context CompositionRoot(共有 context を保持し driver を構築・注入する struct) / `CompositionError`(ErrorType)。配線は CompositionRoot の build **メソッド**として書かれ catalogue にその struct のメソッドとして載る（pub free function が生じない）ため `FreeFunction` は不要 |
| `cli_driver`（invoke+render） | `PrimaryAdapter` / `Dto` / `FreeFunction` | per-context driver(PrimaryAdapter, use case 保持+handle) / 入力 DTO・`CommandOutcome`(Dto) / `render_<command>`・helper(FreeFunction) |

- domain / usecase / infrastructure 固有のロール（`Entity` / `AggregateRoot` / `UseCase` / `Interactor` / `Command` / `Query` / `SecondaryAdapter` / `SecondaryPort` / `ApplicationService` / `Repository` 等）は cli 系 3 層のいずれでも forbidden とする。
- `CompositionRoot` は `cli_composition` 限定、`PrimaryAdapter` は `cli_driver` 限定（互いの層・他層には置かない）。
- `cli_driver`（primary adapter）は handle が常に `CommandOutcome` を返す（use case エラーも整形して返す）ため `ErrorType` を持たない。
- `cli_composition` の配線は CompositionRoot struct の build **メソッド**として書かれ、catalogue にはその struct のメソッドとして載る（隠れず管理される）。pub な *free function* が生じないため `FreeFunction` は allowlist に不要。一方 `cli_driver` の `render_*` と `cli` の dispatch は stateless な **pub free function** であり、隠さず catalogue で追跡する方が TDDD の趣旨に沿うため `FreeFunction` を許可する（`fn main()` は private なので catalogue 非対象で、`FreeFunction` を要求しない）。

この定義を `KindLayerConstraint` ルール群として lint config（`.harness/catalogue-lint/`）に展開する。

```json
// <!-- illustrative, non-canonical -->
[
  { "target_roles": ["CompositionRoot"], "kind": { "KindLayerConstraint": { "permitted_layers": ["cli_composition"] } } },
  { "target_roles": ["PrimaryAdapter"], "kind": { "KindLayerConstraint": { "permitted_layers": ["cli_driver"] } } },
  { "target_roles": ["Dto"], "kind": { "KindLayerConstraint": { "permitted_layers": ["infrastructure", "cli", "cli_driver"] } } },
  { "target_roles": ["FreeFunction"], "kind": { "KindLayerConstraint": { "permitted_layers": ["domain", "usecase", "infrastructure", "cli", "cli_driver"] } } },
  { "target_roles": ["PrimaryAdapter"], "kind": { "NoRoleInMethodSignature": { "forbidden_roles": ["Entity", "AggregateRoot", "ValueObject", "Repository", "SecondaryPort", "UseCase", "Interactor", "Command", "Query", "ApplicationService"] } } }
]
```

`type-designer-kind-selection.md` R1 マトリクスは **本決定を反映**して cli / cli_composition / cli_driver の 3 列と `CompositionRoot` / `PrimaryAdapter` 行を追加・同期する（マトリクスは決定の写しであり、決定の場ではない）。先送りするのは個々の型のロール割り当て（per-type 作業）のみで、上記 allowlist の枠内で実装 track の type-designer が確定する。enforcement は既存の `sotp track lint` + `.harness/catalogue-lint/` に乗る。

## Rejected Alternatives

### A. per-layer allowlist 専用 primitive の追加（D3 の代替）

「層 → 許可ロール」を直接表現する新 primitive を `CatalogueLinterRuleKind` に追加する案。

却下理由: 既存の `KindLayerConstraint` で 3 層の配置制約は表現でき、新 primitive はコード変更や SSoT 二重化を招く。既存機能の活用で足りる。

### B. 単一 `PrimaryAdapter` ロールで wire と invoke を兼ねる（D2 の代替）

`CompositionRoot` を新設せず、`PrimaryAdapter` 1 つに wire（DI）と invoke を兼ねさせる案。

却下理由: composition root（組むだけ）と driving adapter（呼ぶだけ）は canonical には別概念で、`1328` が層レベルで分離した。ロールも分離しないと「PrimaryAdapter が DI を担う」という非 canonical な混在を型カタログ上で追認することになる。

### C. render を `cli_driver` から独立層に分け 4 層にする（D1 の代替）

render を `cli_presentation` 層として分離し cli 系を 4 層にする案。

却下理由: invoke と render は primary adapter の双方向変換の表裏で一体（`1328` Rejected Alternative A）。`cli_driver` の中で render を担い、cli 系は 3 層に保つ。

### D. cli_driver を TDDD 対象から外す（D1 の代替）

分解後も cli / cli_composition の 2 層のみを TDDD 対象にし、primary adapter を対象外にする案。

却下理由: 分解で invoke + render（cli_driver）が独立 delivery 層になった以上、対象外にすると delivery 側の invoke / 出力面に死角が残る。3 層を揃えて検査・制約する。

## Consequences

### Positive

- delivery 側 3 層（入力 bin / wire composition / invoke+render driver）が型カタログ検査の網に載る。
- 分解後の clean な構造の上にロールが乗るため、各層の allowlist が意味を持つ。
- `CompositionRoot`（wire）と `PrimaryAdapter`（invoke+render）を分離したことで、両ロールが canonical な意味を得る（PrimaryAdapter が DI を担う混在を回避）。
- 新 primitive を足さず、既存 `KindLayerConstraint` と lint config だけで 3 層のロール配置制約が成立する。

### Negative

- `CompositionRoot` / `PrimaryAdapter` の 2 variant 追加で `DataRole` の breaking migration（codec / signal evaluator / renderer / 全 `role ==` 比較の更新）を伴う。
- `type-designer-kind-selection.md` R1 マトリクスへの cli / cli_composition / cli_driver の 3 列および 2 行の追加が必要（maintainer checklist）。
- `cli` は初の `[[bin]]` crate を `schema_export`（rustdoc JSON）対象にする事例であり、bin target での rustdoc 挙動に実装上の留意が要る。
- 3 層それぞれの catalogue を起草する負荷（特に cli の clap `*Args` 群を `Dto` 宣言する負荷、`pub(crate)` 降格で縮小可）。

### Neutral

- enforcement は既存の linter framework + `.harness/catalogue-lint/` で成立し、新しい lint 機構は増えない。
- 本 ADR は分解 ADR（`1328`）の下流であり、その完了後に `tddd.enabled` フリップと catalogue 起草を行う。

## Reassess When

- 既存 `KindLayerConstraint` では cli 系の配置制約を十分に表現できない事例が出た場合: 専用機構（per-layer allowlist primitive）の導入を再検討する（Rejected Alternative A の再検討）。
- `CompositionRoot` / `PrimaryAdapter` の使用例が cli_composition / cli_driver にしか現れない状態が続く場合: ロール独立の価値を再評価する。
- `1328` の層構成が見直された（render を別層に分離 / 層を併合）場合: 本 ADR の層数・ロール・allowlist を追従して再定義する。
- web / gRPC など 2 つ目の entry point が追加された場合: cli 系のロール配置制約と新ロールの配置を再評価する。

## Related

- `knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md` — 本 ADR の上流・前提。CLI delivery 側の責務分離（composition root / cli-driver）を決定。本 ADR はその構造にロールと配置制約を載せる。
- `knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md` — `cli_composition` crate 新設の元 ADR（一部は上流 ADR `1328` が supersede）。
- `knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md` — usecase 出力 DTO（cli_driver が扱う）と cli→usecase 境界の前提。
- `knowledge/adr/2026-05-25-0000-tddd-pattern-semantics-extension.md` — 型カタログ linter framework の確立（`KindLayerConstraint` を含む opt-in ルール機構 / config 解決）。本 ADR はその既存機能を cli 系 3 層に適用する。
- `knowledge/conventions/type-designer-kind-selection.md` — R1 役割-層マトリクス。本 ADR 確定後、cli 系 3 列と `CompositionRoot` / `PrimaryAdapter` 行を追加して同期する。
- `knowledge/conventions/hexagonal-architecture.md` — composition root（wire）と primary adapter（invoke+render）の区別、`SecondaryAdapter`（driven）との対比の根拠。
- `architecture-rules.json` — 層定義と `tddd` ブロックの SSoT。D1 はここの `tddd.enabled` を直接書き換える形で実体化される。
