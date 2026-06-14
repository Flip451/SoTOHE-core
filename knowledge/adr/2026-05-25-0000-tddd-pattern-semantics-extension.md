---
adr_id: 2026-05-25-0000-tddd-pattern-semantics-extension
decisions:
  - id: D1
    user_decision_ref: "chat_segment:user-direction:DDD-Clean-pattern-comprehensive:2026-05-25"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:user-direction:catalogue-foundation-before-gen-tests:2026-05-25"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:user-decision:implementation-order-schema-then-roles-then-linter:2026-06-08"
    status: proposed
  - id: D4
    review_finding_ref: "researcher-gap-analysis:2026-05-25"
    user_decision_ref: "chat_segment:user-decision:per-variant-enum-first-semantics:2026-05-26"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:user-decision:identity-accessor-schema-plus-equality-linter-optin:2026-05-27"
    status: proposed
  - id: D6
    review_finding_ref: "researcher-gap-analysis:2026-05-25"
    user_decision_ref: "chat_segment:user-decision:per-variant-enum-first-semantics:2026-05-26"
    status: proposed
  - id: D7
    review_finding_ref: "researcher-gap-analysis:2026-05-25"
    user_decision_ref: "chat_segment:user-decision:per-variant-enum-first-semantics:2026-05-26"
    status: proposed
  - id: D8
    review_finding_ref: "researcher-gap-analysis:2026-05-25"
    user_decision_ref: "chat_segment:user-decision:per-variant-enum-first-semantics:2026-05-26"
    status: proposed
  - id: D9
    review_finding_ref: "researcher-gap-analysis:2026-05-25"
    status: proposed
  - id: D10
    user_decision_ref: "chat_segment:user-decision:repository-independent-variant:2026-05-26"
    status: proposed
  - id: D11
    review_finding_ref: "researcher-gap-analysis:2026-05-25"
    status: proposed
  - id: D12
    review_finding_ref: "researcher-gap-analysis:2026-05-25"
    status: proposed
  - id: D13
    review_finding_ref: "researcher-gap-analysis:2026-05-25"
    status: proposed
  - id: D14
    user_decision_ref: "chat_segment:user-decision:make-illegal-states-unrepresentable-field-optionality:2026-05-25"
    status: proposed
  - id: D15
    user_decision_ref: "chat_segment:user-decision:linter-opt-in-strict-no-advisory:2026-05-27"
    status: proposed
  - id: D16
    user_decision_ref: "chat_segment:user-decision:event-policy-independent-role:2026-06-13"
    status: proposed
  - id: D17
    user_decision_ref: "chat_segment:user-decision:linter-eval-logic-in-core:2026-06-13"
    status: proposed
  - id: D18
    user_decision_ref: "chat_segment:user-decision:vo-equality-immutability-core:2026-06-13"
    status: proposed
---
# TDDD カタログ taxonomy の意味論拡張 — パターン固有の機械検査ルールを持たせる

## Context

### §1 現状のカタログロールは分類ラベルにすぎない

`DataRole`（13 variant）と `ContractRole`（3 variant）は、型に DDD / Clean Architecture のロール名を付与する。しかしこれらは **名前（ラベル）** であり、DDD パターンとしての意味論（述語・制約・境界ルール）は一切持っていない。

特に `Entity` / `AggregateRoot` / `ValueObject` の 3 値は、ADR `2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` の D2 が language / role / layer の 3 軸分離を実施した際に `DataRole` へラベルとして収録したものである。その ADR の目的は軸の整理であり、各ロールに DDD パターン固有の意味論や検査ルールを与えることは対象外だった。結果として、これら 3 variant は **現時点でも名前だけが存在し、パターンレベルの検査能力はゼロ** の状態にある:

- `AggregateRoot` という名前はカタログに書けるが、「どの型がこの集約の専属 Entity か」「共有 VO か」「一貫性境界の外から専属 Entity が直接参照されていないか」は宣言も検査も不可能。
- `Entity` という名前はカタログに書けるが、どの型が identity を担うかを宣言する手段がなく、identity-based equality の lint は存在しない。
- `ValueObject` という名前はカタログに書けるが、値の不変条件（例: Email は `@` を含む）を宣言する手段がなく、それを検証する lint・テスト生成も存在しない。
- `UseCase` も同様に「一貫性境界を形成するか」「単一トランザクションか複数集約にまたがるか」を表明する手段がない。

つまり、現行の TDDD カタログで `AggregateRoot` を宣言することは「これは集約ルートと名付けよう」という意思表示であり、DDD パターンとしての機械的な保証は何も与えない。

2026-05-25 に実施した Gemini による DDD / Clean Architecture パターン taxonomy ギャップ分析（`knowledge/research/2026-05-25-ddd-clean-pattern-taxonomy-gap.md`）で、以下の二層の不足が確認された。

- **Layer ① 意味論の欠如**: `Entity` / `AggregateRoot` / `ValueObject` 等のロールはラベルとして存在するが、パターン固有の述語・制約を宣言するフィールドを持たない。本 ADR が初めてこれらに意味論を与え、DDD パターンとして実体化させる。
- **Layer ② 新ロールの欠如**: Domain Event・Repository・Aggregate Boundary・EventPolicy は高い機械検査 ROI を持つが、現行の taxonomy に存在しない。

### §2 追加の原則

#### linter と signal 評価器の区別

本 ADR が定める機械検査ルールは、実装上2つの異なる機構を使う。用語を明確にする。

**linter（`CatalogueLinter`）**: 入力はカタログ宣言（A-side = `CatalogueDocument`）のみ。カタログ宣言の内部で完結する規則を静的にチェックし、違反があれば `CatalogueLintViolation`（`rule_kind` + `entry_name` + `message` のみ、severity フィールドなし）を返す。ルールは呼び出し側から `&[CatalogueLinterRule]` として外部供給されるため、設定ファイルでルールリストを組み替えることで採用者ごとの opt-in/opt-out が実現する。

**signal 評価器（`SignalEvaluatorPort`）**: 入力は A（`ExtendedCrate` = 宣言由来のアノテーション付き型グラフ）+ B（`rustdoc_types::Crate` = ベースライン）+ C（`rustdoc_types::Crate` = 現在実装）の3点。宣言（A）と実装（C）の整合性を 🔵🟡🔴 で評価する別機構であり、linter とは入力・責務・出力形式のすべてが異なる。

**本 ADR の機械検査は 3 手段を使い分ける**:

- **schema invariant**（型/codec で強制、opt-in/opt-out 不可）: `identity` 必須（`IdentityAccessor`）・`Repository.aggregate`（単一 `TypeRef`）・`EventPolicy.reacts_to` 非空（`NonEmptyVec`）がこれに相当する。decode 時に空/欠損を弾くため、採用者が意図せず無効化することはない。
- **opt-in linter ルール（D15）**: カタログ宣言（A-side = `CatalogueDocument`）内で完結する静的チェック。D4〜D11 および D16 / D18 の DomainEvent チェック・境界チェック等がこれに相当する。linter は signal 評価器（A+B+C の 3 点比較）とは入力・責務・出力形式のすべてが異なる。「宣言した型・メソッド・trait_impl が実装と整合するか」は既存 signal 評価器の責務であり、本 ADR はそれを再定義しない。既存の `CatalogueLinterRuleKind`（`libs/domain/src/tddd/catalogue_linter.rs`）は `FieldEmpty` / `FieldNonEmpty` / `KindLayerConstraint` の 3 variant を持つ。本 ADR が追加する最小コアルールはこの enum に新 variant を追加することで実装する（詳細は D15）。
- **テスト生成**: gen-tests がカタログ宣言を読んでテスト骨格を生成する。

本 ADR が採用する設計原則: **ロールや意味論フィールドは、それが独自の機械検査ルール（schema invariant / signal / lint / テスト生成）を持つ場合にのみ追加する。DDD 語彙を enum に追加するだけの拡張は行わない。**

本 ADR は検査ルールの**最小コア**を定義する。検査範囲の網羅・深化（全型スロット・全 entry 種別・method body 等）は利用者がカスタムルールとして追加する。網羅範囲は利用者のドメイン/プロジェクト構造に依存するため、ADR では決めない。最小コアはロールの意味論が壊れない範囲に限定する。

**最小コアとカスタムの線引き**: 最小コア = ロール名の意味が成立する最低限の surface check。カスタム = 検査対象の拡大・深い依存解析・プロジェクト固有の運用ルール。

ADR `2026-04-13-1813-tddd-taxonomy-expansion.md` の D3「YAGNI: 検証ルールのない variant は存在チェックのみ」とは意図が異なる点に注意する。D3 は「variant は追加するが検証は後回し」という判断であり、本 ADR は「検査ルールがあって初めて追加する」という、より厳しい基準を採る。両者は矛盾しない（既存の variant は D3 のもとで追加済み）。

### §3 現行コードの確認

`libs/domain/src/tddd/catalogue_v2/` を読み込んで確認した事実:

- `entries.rs` の `TypeEntry` は `role: DataRole` と `kind: TypeKindV2` を持つが、**パターン固有の述語フィールドを一切持たない**。`role` フィールドはラベルを格納するだけであり、`Entity` を宣言しても identity チェックは走らず、`AggregateRoot` を宣言しても境界 lint は走らない。`TraitEntry` も同様にパターン固有述語を持たない。
- `roles.rs` の `DataRole` は 13 variant を持ち、すべて unit variant（フィールドなし）で `Copy` + strum derive（文字列ラウンドトリップ）を持つ。`Entity` / `AggregateRoot` / `ValueObject` は ADR `2026-05-08-0248` D2 の軸分離作業でラベルとして収録されたが、それ以来意味論は追加されていない。`ContractRole` は 3 unit variant（`SpecificationPort` / `ApplicationService` / `SecondaryPort`）を持ち、`Repository` に対応する専用 variant は存在しない。実務上は `SecondaryPort` として宣言されている。
- `composite.rs` の `TypeKindV2` は現在 `Struct(StructKind)` / `Enum` / `TypeAlias` の 3 variant（ADR `2026-05-26-1002-typestate-struct-kind-orthogonal.md` D1 適用後）を持つ。`StructKind` は struct 形状（unit / tuple / plain）に直交して `typestate: Option<TypestateMarker>` を持つ。これは「意味論フィールドをそれが属する軸の enum に結合する」先行事例であり、本 ADR の方向性と一致する。typestate marker は `TypeKindV2` の struct グループという **kind 軸** に結合している点が重要である。
- 本 ADR が行うのは、DDD 意味論フィールドを **role 軸**（`DataRole` variant）に結合すること——`TypeEntry` 構造体への平置きではない。共通原則は「各意味論はそれが属する軸の enum に結合し、Entry 構造体へ平置きしない」。同 ADR との違い: typestate は全 struct 形状が共有するので kind 軸に1回置ける。一方 DDD 意味論は role ごとに異なる（`invariants` は ValueObject / Entity / AggregateRoot の 3 variant に有効、`emits` は AggregateRoot / DomainService の 2 variant に有効）。role をグルーピングできない理由は、AggregateRoot は複数のフィールド群（identity / invariants / exclusive_members / shared_value_objects / emits）すべてに属し、`emits` は2つの異なる variant グループにまたがるため、単一の階層には収まらない。よって各 variant が自分の有効フィールドを持つ per-variant 構造になる。
- カタログは public surface のみを列挙する（`has_stripped_fields` により private field は名前が出ない）。identity は型参照ではなく public getter メソッドの accessor を指す（public field identity は D5 で禁止。根拠と詳細は D5）。
- `TypeEntry` や `TraitEntry` にフィールドを追加することは、スキーマ変更として codec（infrastructure 層）の更新を伴う breaking change である。既存カタログとの後方互換性は保証しない——アクティブなトラックのカタログのみ移行し、非アクティブトラックのカタログはライトプロテクトして移行対象外とする。各フィールドを必須にするか任意にするかは「空または `None` が不正な状態と 1:1 対応するか」で判断する（D14）。

## Decision

### D1: DDD / Clean Architecture のパターンを機械処理可能な形で包括的にカバーする

> ユーザー指示: DDD / Clean Architecture のパターンを、機械処理可能な形で SoTOHE カタログ上で包括的に扱えるようにする。

本 ADR が定める拡張は、このユーザー方針を踏まえ、「独自の機械検査ルールを持つパターンのみを追加する」という絞り込み原則のもとで具体化したものである。

### D2: カタログ拡張の基本方針 — gen-tests の前にカタログを整備する

> ユーザー指示: カタログ駆動テスト生成（gen-tests）の設計を進める前に、カタログ taxonomy の意味論基盤を整備する。

テスト生成（gen-tests）が有意義な検査コードを生成するには、ロール + 意味論フィールドの組み合わせが揃っている必要がある。カタログ側の基盤なしに gen-tests を実装しても、生成されるテストは「型が存在する」程度の shallow な確認にとどまる。本 ADR の Layer ① + Layer ② を先に実装・安定化してから gen-tests の実装に移行する。

### D3: 実装は schema 強制 → 新ロール追加 → linter の順で進める

> ユーザー決定（2026-06-08）: 実装フェーズは「型定義による schema 強制の意味論 → 新ロール追加 → linter」の機構軸で分ける。ロールの種類（既存／新規）では分けない。

意味論の実装を、ロールの種類ではなく機構の軸（型で守る schema 強制 / linter の A-side 静的チェック）で 3 段に分ける:

1. **schema 強制の意味論**: `DataRole` / `ContractRole` を data-carrying enum 化し、payload を持つすべての variant の schema を 1 回の breaking migration で確定する。対象は既存ロールへの意味論フィールド（D4–D8）に加え、`ContractRole::Repository { aggregate: TypeRef }`（D10）と `DataRole::EventPolicy { reacts_to: NonEmptyVec<TypeRef> }`（D16）を含む。型で守れる制約（`identity` / `aggregate` / `reacts_to` の必須性、ロールに無効なフィールドの構造的排除）はこの段で確定する。
2. **新ロール追加**: `DomainEvent`（D9）を unit variant として追加し、段 1 で schema を確定した `ContractRole::Repository`（D10）および `DataRole::EventPolicy`（D16）を含む新ロール群を taxonomy / 表示 / fixture に反映する。この段では payload schema の breaking migration を追加しない。
3. **linter**: D4–D11 および D16 / D18 の機械検査ルール（A-side 静的チェック）を、確定した schema の上に opt-in ルール（D15）として実装する。

理由:

- `DataRole` / `ContractRole` の data-carrying enum 化は不可分な変更である。1 variant でも data を持つと `Copy` が外れ、strum・codec・signal evaluator・renderer・すべての `role ==` 比較を一斉に更新する必要がある（Negative 節参照）。よって schema 変更は 1 回の migration として先に確定させるしかない。
- linter は確定した schema にのみ依存し、追加的で opt-in（D15）である。schema を先に固めれば、基盤フィールドが無い状態で linter を仮実装する必要がなくなる。
- 大きい breaking migration を段 1 に閉じ込めることで各 commit を小さく保て、payload を持たない新ロールの admission と周辺整備は段 1 の完了後に進められる。

**移行対象の限定**: スキーマ変更は breaking change であり、既存カタログとの後方互換性は保証しない。移行はアクティブなトラックのカタログのみを対象とする。非アクティブ（archive 済み・completed）なトラックのカタログはライトプロテクトして移行対象外とする——遡及移行は行わない。

`DomainEvent` は unit variant で schema 強制の意味論を持たない——その規約（`&mut self` 禁止）はすべて linter ルールである。よって段 2 では variant を追加するだけで、意味論は段 3 で入る。一方 `Repository` は `aggregate` 必須が単一 `TypeRef` で、`EventPolicy` は `reacts_to` 非空が `NonEmptyVec<TypeRef>` で、いずれも型で守られるため、その payload schema は段 1 の不可分な migration に含める。

Layer ①（既存ロールへの意味論付与）/ Layer ②（新ロール追加）の見出しは説明上の分類として残す。実装順は本決定の機構軸に従う。

### D14: 意味論フィールドの必須 / 任意は make-illegal-states-unrepresentable で判定する

> ユーザー決定: `Option` や空 `Vec` が許容されるかは「空または `None` が不正な状態と 1:1 対応するか」で判断する。対応するなら必須（`Option` 不可）とし、ロールと構造的に結合する（enum-first）。空が正当な状態なら `Vec` / `Option` でよい。

各フィールドへの必須 / 任意の判定結果は各決定（D4–D8 および D10 / D16）の本文に記載する。

### D15: DDD パターンの機械検査ルールは設定で opt-in / opt-out できる linter ルールとして提供する

> ユーザー決定（2026-05-27）: 機械検査ルールは設定ファイルで opt-in / opt-out できる linter ルールとして提供する。テンプレートの各採用者が適用するルールを選ぶ。opt-in されたルールは厳格に適用し（違反を許さない）、advisory（無視可能な警告）という中間の重み付けは設けない。

D4〜D11 および D16 / D18 で定める各 **opt-in linter ルール**は、`CatalogueLinter` に対して外部から供給するルールリスト（`&[CatalogueLinterRule]`）として実現する。ルールの組み合わせを設定ファイルで管理することで、採用者ごとに適用ルールを選ぶことができる。opt-in したルールに違反があれば linter はそれを `CatalogueLintViolation` として報告し、違反を容認する「advisory」という扱いは設けない（報告されるが無視されうる警告は意味をなさないため）。なお、schema invariant（`identity` 必須・`Repository.aggregate`・`EventPolicy.reacts_to` 非空）は型/codec で常に強制されるため、opt-in/opt-out の対象外である。

本 ADR は「opt-in/厳格/advisory なし」という原則を定める。ADR は最小コアルールを定義し、利用者は最小コア + カスタムルールを `&[CatalogueLinterRule]` として組み上げる。検査範囲の網羅（全型スロット・全 entry 種別・method body 等）はカスタムに委ねる。

**既存 framework の刷新方針（canonical 決定事項）**: `CatalogueLinterRuleKind`（`libs/domain/src/tddd/catalogue_linter.rs`）は現在 3 variant を持つ unit enum + フラットフィールド構造。本 ADR はこの構造を以下の 2 方針で刷新する。rule list を core の評価関数（evaluator）に渡す形は維持する。既存 `CatalogueLinter` trait は D17 で core 評価関数へ移行する対象であり、評価ロジックの層配置は D17 が定める。

**方針 1（canonical）**: ルールは「**検査対象**（どのロールのエントリに適用するか）」と「**検査内容**（何を検査するか）」を分けて指定できる構造とする。`NoPublicField` / `ForbiddenMethodReceiver` のように検査内容のみでは対象ロールが特定できない kind があるため、この分離が必要である。具体的な型名・フィールド構造は実装フェーズの型定義で確定する（以下の参考スケッチを参照）。

**方針 2（canonical）**: 検査対象ロールの指定は `DataRole` / `ContractRole` を横断して行える必要がある。`DataRole` は data-carrying enum 化で payload を持つため discriminant としてロール種別を指定するには別型が必要であり、また `Repository` は `ContractRole` 側のロールなので `DataRole` 型では指定できない。実装では payload を持たないロール種別 discriminant（参考スケッチでは `RoleKind`）を設ける方向で実装する。

---

*以下は non-canonical な参考スケッチである。型シグネチャ・フィールド名・variant 名は実装フェーズで確定する。*

**参考スケッチ: `CatalogueLinterRule` の 2 部構成**:

- `RuleTarget`: ルールの適用対象を選択する selector（例: `target_roles: Vec<RoleKind>`）。どのロールを持つエントリにこのルールを適用するかを宣言的に指定する。
- `CatalogueLinterRuleKind`: 検査内容のパラメータのみを保持する。適用対象は `RuleTarget` に委ねる。

**参考スケッチ: `RoleKind` discriminant**:
`DataRole` / `ContractRole` / `FunctionRole` の各 variant に対応する payload なし discriminant（例: `RoleKind::Entity` / `RoleKind::AggregateRoot` / `RoleKind::ValueObject` / `RoleKind::DomainEvent` / `RoleKind::EventPolicy` / `RoleKind::Repository` / `RoleKind::UseCase` …）。rule kind のロール指定はこの discriminant で統一する方向で実装する。

**参考スケッチ: `CatalogueLinterRuleKind` の variant 候補**（正確な型シグネチャは実装フェーズで確定する）:

- `KindLayerConstraint { permitted_layers: Vec<LayerId> }` — 既存: entry の layer が許可リストに含まれるか
- `FieldEmpty { target_field: FieldName }` — 既存: 指定フィールドが空か
- `FieldNonEmpty { target_field: FieldName }` — 既存: 指定フィールドが非空か
- `ReferencedRoleConstraint { target_field: FieldName, expected_role: RoleKind }` — 新規: 指定フィールドに列挙された型が expected_role で宣言されているか
- `TraitImplRequired { required_traits: Vec<TraitRef> }` — 新規: 指定 trait の impl 宣言が `trait_impls` に存在するか
- `NoRoleInMethodSignature { forbidden_roles: Vec<RoleKind> }` — 新規: メソッドシグネチャに forbidden_roles のロールの型が出現しないか（`ContractRole` 由来の discriminant も指定できる）
- `MethodReferenceSignature { target_field: FieldName, receiver, params_empty, returns }` — 新規: target_field で参照されたメソッド名が public method 集合に存在し指定 signature を満たすか
- `AccessorSignatureRequired { receiver, params_empty, returns }` — 新規: identity getter が public method 集合に存在し指定 signature（`&self` / params 空 / 非 `()` 返却）を満たすか
- `FieldElementUniqueAcrossEntries { target_field: FieldName }` — 新規: target_field に列挙された型が他のエントリの同一フィールドと重複しないか
- `NoExternalReferenceInMethods { target_field: FieldName }` — 新規: target_field に列挙された型が同一集約外の TypeEntry のメソッドシグネチャに出現しないか
- `NoPublicField` — 新規: `StructShape::Plain` / `Tuple` の public field が存在すれば違反（適用対象は `RuleTarget` で指定）
- `ForbiddenMethodReceiver { forbidden_receiver }` — 新規: forbidden_receiver に一致する receiver を持つメソッド宣言があれば違反（適用対象は `RuleTarget` で指定）

`MethodReferenceSignature` と `AccessorSignatureRequired` は method 存在と signature を一括検査するものであり、ロール参照整合を担う `ReferencedRoleConstraint` とは責務が異なる。

**参考スケッチ: 最小コアルールと rule kind の対応**（以下は non-canonical。正確な型は実装フェーズで確定する）:

| ルール | 対応する rule kind（候補） | `target_roles`（候補） | `target_field` | 備考 |
| --- | --- | --- | --- | --- |
| D4: invariant SelfMethod 名前存在チェック + signature チェック | `MethodReferenceSignature` | `[Entity, AggregateRoot, ValueObject]` | `"invariants"` | 述語メソッド名の public method 宣言集合存在 + `&self` / params 空 / `bool` 返却 |
| D5: identity getter 存在チェック + getter signature チェック | `AccessorSignatureRequired` | `[Entity, AggregateRoot]` | `"identity"` | getter メソッド名存在 + `&self` / params 空 / 非`()`返却（public field identity は禁止） |
| D5: equality 宣言存在チェック | `TraitImplRequired` | `[Entity, AggregateRoot]` | — | `PartialEq` / `Eq` の trait_impl 宣言必須 |
| D6: exclusive_members → Entity ロール整合チェック | `ReferencedRoleConstraint` | `[AggregateRoot]` | `"exclusive_members"` | 列挙型が `Entity` ロールで宣言されているか（`AggregateRoot` は除く） |
| D6: shared_value_objects → ValueObject ロール整合チェック | `ReferencedRoleConstraint` | `[AggregateRoot]` | `"shared_value_objects"` | 列挙型が `ValueObject` ロールで宣言されているか |
| D6/D11: exclusive_members 重複所属チェック | `FieldElementUniqueAcrossEntries` | `[AggregateRoot]` | `"exclusive_members"` | 同一型が複数 AggregateRoot の exclusive_members に出現しない |
| D6/D11: exclusive_members 直接参照チェック（shallow） | `NoExternalReferenceInMethods` | `[AggregateRoot]` | `"exclusive_members"` | 集約外 TypeEntry のメソッドシグネチャに専属 Entity 型が出現しない |
| D6/D11: ValueObject 独立性チェック | `NoRoleInMethodSignature` | `[ValueObject]` | — | ValueObject ロールを持つ型のメソッドシグネチャに Entity / AggregateRoot ロールの型が出現しない（shared かどうかによらず VO 一般に適用） |
| D7: emits → DomainEvent ロール整合チェック | `ReferencedRoleConstraint` | `[AggregateRoot, DomainService]` | `"emits"` | 列挙型が `DomainEvent` ロールで宣言されているか |
| D8: UseCase.handles → DomainEvent ロール整合チェック | `ReferencedRoleConstraint` | `[UseCase]` | `"handles"` | UseCase が handles に列挙する型が `DomainEvent` ロールで宣言されているか |
| D16: EventPolicy.reacts_to → DomainEvent ロール整合チェック | `ReferencedRoleConstraint` | `[EventPolicy]` | `"reacts_to"` | EventPolicy が reacts_to に列挙する型が `DomainEvent` ロールで宣言されているか |
| D9: DomainEvent public mutation surface削減（`&mut self` 禁止） | `ForbiddenMethodReceiver` | `[DomainEvent]` | — | `&mut self` 受信メソッドを持つ `DomainEvent` は違反 |
| D9: DomainEvent struct public field 禁止（Plain/Tuple struct） | `NoPublicField` | `[DomainEvent]` | — | `StructShape::Plain` / `Tuple` の public field があれば違反。enum variant payload は対象外 |
| D10: Repository.aggregate → AggregateRoot 整合チェック | `ReferencedRoleConstraint` | `[Repository]` | `"aggregate"` | aggregate で参照する型が `AggregateRoot` ロールで宣言されているか |
| D16: EventPolicy は domain 層のみ許可 | `KindLayerConstraint` | `[EventPolicy]` | — | 既存 variant 流用 |
| D16: EventPolicy に `&mut self` 禁止 | `ForbiddenMethodReceiver` | `[EventPolicy]` | — | `&mut self` 受信禁止 |
| D16: EventPolicy に副作用ロール禁止 | `NoRoleInMethodSignature` | `[EventPolicy]` | — | メソッドシグネチャに Repository / UseCase ロールの型が出現しない（Repository は ContractRole 由来——RoleKind discriminant が必要な主要動機） |
| D18: ValueObject 値等価性チェック | `TraitImplRequired` | `[ValueObject]` | — | `PartialEq` / `Eq` の trait_impl 宣言必須（D5 Entity と同じ rule kind） |
| D18: ValueObject public mutation surface削減（`&mut self` 禁止） | `ForbiddenMethodReceiver` | `[ValueObject]` | — | `&mut self` 受信メソッドを持つ `ValueObject` は違反（D9 と同じ rule kind） |
| D18: ValueObject struct public field 禁止 | `NoPublicField` | `[ValueObject]` | — | `StructShape::Plain` / `Tuple` の public field があれば違反（D9 と同じ rule kind） |

**ddd-strict 既定プリセット**: 本 ADR の最小コアルール（D4〜D11 / D16 / D18 で定めた全最小コアルール）を一括で有効化するルールセットプリセットを `ddd-strict` として提供する。本 ADR が定める「機械処理可能な意味論」は `ddd-strict` プリセットの適用を前提とする。opt-in 方式は維持しつつ `ddd-strict` を既定プリセットとして提供することで、採用者が個別ルールを選ばずとも最小コアを一括有効化でき、ロールがラベルに戻ることを防ぐ。採用者は `ddd-strict` の全ルールをそのまま使うことも、一部を外して個別選択することも、独自カスタムルールを追加することもできる。

### カスタム拡張例

利用者がカスタムルールとして追加できる検査の例を示す。いずれも `&[CatalogueLinterRule]` に追加することで有効になる。

- **直接参照チェックの対象拡大**: D6 / D11 の直接参照チェック対象を `TypeEntry.methods` 以外に広げる場合: `TraitEntry.methods`（port / application service / repository のシグネチャ）/ `FunctionEntry` のシグネチャ / struct fields / constructor params / inherent impls / nested generic 型引数 / method body（rustdoc 依存グラフ経由）。
- **EventPolicy の field / constructor params 依存制約**: EventPolicy の field に Repository / UseCase 型が現れない（field dependency 禁止）/ constructor params に Repository / UseCase 型が現れない。method body 内の依存（rustdoc 依存グラフ経由で確認可能な範囲）の確認も含む。最小コアの `NoRoleInMethodSignature`（メソッドシグネチャのみ）を補完する深い依存チェック。
- **exclusive_members / shared_value_objects の重複確認**: `exclusive_members` と `shared_value_objects` の間で同一の型が重複して列挙されていないかの整合チェック。
- **Entity / AggregateRoot の Hash 宣言チェック**: HashMap キーとして使いたい場合に `Hash` の impl が `trait_impls` に宣言されているかを確認する。DDD の Entity 本質（identity-based equality）とは独立した都合であるため、最小コアには含めない。なお、`Hash` の impl 宣言が存在するかは A-side 宣言で確認できるが、「その実装が identity と整合した hash（accessor で得られる identity のみを使って hash しているか）」は型カタログからは検証できない（equality 宣言チェックと同じ構造的限界）。
- **DomainEvent の配信可能性**: `Clone + Send + Sync` を `trait_impls` に宣言しているかを確認する。`Send` / `Sync` は Rust の auto trait であり通常 `impl Send for T {}` を明示的に書かないため、A-side 宣言検査には限界がある（宣言の書き忘れで形骸化しうる）。sync のみのプロジェクトや配信機構を別手段で担保するプロジェクトは不要なため最小コアに含めない。
- **プロジェクト固有ルール**: 層・命名規約・イベント配送・永続化戦略など、プロジェクト固有の制約を検査するカスタムルール。

### D17: linter ルール評価ロジックを domain/application core に置く

`CatalogueLinterRule` を `CatalogueDocument` に適用し `CatalogueLintViolation` を生成する処理は、外部 I/O を伴わない純粋ロジックであり、検査対象はドメイン語彙の意味論そのものである。よってこの評価ロジックは domain/application core に置く。

**現行配置の見直し**: 現行コードは `CatalogueLinter` trait を domain 層の secondary port として定義し、評価ロジックを `InMemoryCatalogueLinter`（infrastructure 層）に実装している。この配置は Clean Architecture の依存方向（純粋ロジックは内側、外部 I/O 依存は外側）に反する——I/O を持たない純粋評価ロジックが外側の層（infrastructure）に置かれている。

**決定**: 評価ロジックを domain/application core に移す。port（trait）は外部 I/O が実際に必要な操作——設定ファイルからのルールリスト読込・codec・外部ストレージへの報告など——に限定する。これにより、linter ルールの評価は infrastructure に依存せずユニットテストできる。

**既存 `CatalogueLinter` trait と `InMemoryCatalogueLinter` の扱い**: 評価ロジックを core に移す再設計は、`CatalogueLinter` trait（secondary port の位置づけ）を廃止または再定義することを意味する。廃止か再定義かは実装で判断する。いずれにせよ、評価関数（`run` 相当）は domain/application core に直接置く。application/core のユースケースがルール設定を受け取り（secondary port 経由で rule リスト等を入力）、純粋評価関数（domain/application core）を呼ぶ。infrastructure は codec / config adapter として外側に留まり、その port を実装する。依存方向は内向き（infra → core）であり、core が infra を直接呼ぶことはない。

**D15 との分担**: D15 は「opt-in/厳格/advisory なし」という原則と rule kind 体系（`CatalogueLinterRule` の外部供給の仕組み）を定める。評価ロジックの層配置は本決定（D17）が定める。

---

## Layer ① — 意味論ゼロのロールラベルに初めて意味論を与える

### D4: `invariants` フィールド — ValueObject / Entity / AggregateRoot の各 variant に結合

> ユーザー決定（2026-05-26）: 意味論フィールドは `TypeEntry` に平置きするのでなく、有効ロールに対応する `DataRole` variant に結合する（per-variant enum-first）。

`DataRole::ValueObject` / `DataRole::Entity` / `DataRole::AggregateRoot` の各 variant が `invariants: Vec<InvariantDecl>` フィールドを持つ。

`InvariantDecl` は名前付きのドメインルールを宣言する struct である。invariant は「この型が満たすべき不変条件」という構造的な宣言であり、「どう名指すか（name）」と「どう検査するか（predicate）」は別概念である。これらを struct の 2 フィールドで分離する。

```rust
// <!-- illustrative, non-canonical -->
struct InvariantDecl {
    name: InvariantName,             // 不変条件の名前（必須）
    predicate: InvariantPredicate,   // 検査手段（必須）
}
enum InvariantPredicate {
    SelfMethod(MethodName),          // self（このエントリの型）の述語メソッド
    // 将来拡張の軸として enum を維持する（Specification predicate 等は今後の判断ポイントを参照）
}
```

```json
// <!-- illustrative, non-canonical -->
{ "role": "ValueObject", "invariants": [
    { "name": "ContainsAtSign", "predicate": { "self_method": "contains_at_sign" } }
]}
```

**設計の根拠（3 点）**:

1. **enum 二択から struct へ**: 1 つの invariant は「SelfMethod で検査される」と「何らかの根拠がある」の両方を同時に持てなかった。invariant はドメインルールの構造宣言であり、検査手段と根拠は別概念なので struct で分離する。
2. **SpecRef を predicate から削除**: 根拠（要求仕様へのトレース）は検査手段の代替ではなく traceability である。根拠の管理は既存の `TypeEntry.spec_refs`（型エントリ全体の SoT Chain ② spec trace）に委ね、invariant の predicate には含めない。
3. **owner なし（SelfMethod）**: invariant は self（このエントリの型）の述語である。型エントリは `CatalogueDocument::types` の `TypeName` キーで識別されるので、述語メソッドの owner は親エントリのキーと重複する。`SelfMethod(MethodName)` は owner を持たず、linter は親エントリ型の public method 集合を検索する。

`InvariantPredicate` を `SelfMethod` のみの enum として維持する理由は、将来 Specification 等の検査手段を追加する拡張軸を確保するためである（詳細は今後の判断ポイントを参照）。

**InvariantName / InvariantPredicate のフィールド必須性（D14 適用）**: 名前のない invariant は識別不能（不正状態と 1:1）なので `name` は必須。検査手段のない invariant は検査不能なので `predicate` も必須。

- **型**: `Vec<InvariantDecl>`
- **意味**: この型が満たすべき不変条件の宣言リスト（各条件に名前と検査手段を持つ）。
- **有効ロール**: `ValueObject` / `Entity` / `AggregateRoot`。他の variant には `invariants` フィールドが存在しないため、不正な組み合わせは構造的に表現不可能になる。
- **任意（D14）**: 空 Vec も正当な状態（バリデーションを持たないシンプルな型は存在する）なので宣言省略可。
- **invariant の意味と enforcement の範囲**: `invariants` は **observable predicate（観測可能な述語）の宣言**である。linter は「その述語メソッドがカタログの public surface に存在し、signature が述語の契約（`&self` + 引数なし + `bool` 返却）を満たすか」を検査する——「その不変条件が生成時・変更時に常に破られないこと（enforcement）」は linter の保証範囲外である。DDD invariant の本質は enforcement（型が常に条件を保持すること）にあるが、linter は A-side 宣言のみを見るため、enforcement の検証はできない。enforcement（constructor / factory / mutator が invariant を破らないこと）については、gen-tests が `InvariantDecl` を読んでテスト骨格を生成できる（実行レベルの意味確認は後続の gen-tests ADR が定める contract に依存する）。enforcement surface（constructor / factory / mutator のどこで invariant を保持するかの体系的宣言・lint）は本 ADR の最小コアには含めない——今後の判断ポイントを参照。
- **機械検査ルール（linter / opt-in 時に厳格適用）**:
  - `InvariantPredicate::SelfMethod(method)`: 対象メソッド名が親エントリ型の public method 宣言集合に存在し（名前存在チェック）、かつ対象 `MethodDeclaration` が `receiver: Some(SelfReceiver::SharedRef)`（`&self`）/ `params` が空 / `returns` が `bool` 型であることを確認する（signature チェック）。名前一致のみでは `fn foo(&mut self) -> String` が通ってしまうため、述語メソッドとして使う契約（`&self` + 引数なし + `bool` 返却）を名前存在と signature で一括して検査する。このルールはロール参照整合ではなく method の存在と signature を検査するため `MethodReferenceSignature`（`target_field: "invariants"`）として実装する（D15 参照）。
  - 根拠 spec の存在チェックは既存の spec-signal / `TypeEntry.spec_refs` 機構がカバーする（invariant 単位の spec 検査は設けない）。
  - 宣言した参照先の実装整合（メソッドが実際に存在するか）は既存 signal 評価器が別途カバーする。
  - テスト生成器（gen-tests）は `name` を識別子として、`SelfMethod` の述語メソッドが `true` を返すことを検証するテスト骨格を生成できる（実行レベルの意味確認は後続の gen-tests ADR が定める contract に依存する）。

```rust
// <!-- illustrative, non-canonical -->
// IdentityAccessor は getter 参照に統一（public field identity は禁止、D5 参照）
struct IdentityAccessor(MethodName);  // または単一 variant の enum — 型定義の粒度は実装に委ねる

enum DataRole {
    ValueObject { invariants: Vec<InvariantDecl> },
    Entity      { identity: IdentityAccessor, invariants: Vec<InvariantDecl> },
    AggregateRoot { identity: IdentityAccessor, invariants: Vec<InvariantDecl>,
                    exclusive_members: Vec<TypeRef>, shared_value_objects: Vec<TypeRef>,
                    emits: Vec<TypeRef> },
    // ...
}
```

### D18: ValueObject の値等価性と不変性を最小コアに（D3 段3: linter ルール）

`ValueObject` ロールの本質は「**値による等価性**」と「**値として扱える（内部状態が変わらない）**」の 2 点である。D4 で `invariants`（個別の不変条件宣言）を与えたが、VO ロールとしての基本的な性質（値として扱える・public mutation surface を持たない）を保証する最小コアルールが不足していた。D5 が Entity に identity-based equality チェックを与えたのと対称に、D18 では VO に値等価性と public mutation surface削減チェック（不変性の近似）の最小コアを与える。D18 が定めるのは A-side 静的チェック（linter ルール）であり、D3 段3（linter）の対象である。schema フィールドの追加（D3 段1）は行わない——`ValueObject` は D4 で `invariants` フィールドを持つが、D18 が追加するのはそのフィールド構造を前提としたカタログ宣言の内部チェックルールである。

**値等価性チェック（最小コア）**: `ValueObject` ロールを持つ型に対し、`PartialEq` / `Eq` の impl が catalogue の `trait_impls` に宣言されているかを確認する。**VO は値による等価（全フィールド比較）、Entity は identity による等価（accessor で識別）**という DDD の本質的な違いはあるが、linter が確認するのは「`PartialEq` / `Eq` の impl 宣言が存在するか」という点であり（D5 と同じ rule kind `TraitImplRequired`）、「その equality が真に値等価（全フィールド比較）かどうか」は静的に検証できない——`#[derive(PartialEq)]` が全フィールド比較になることは一般に期待通りだが、手動 impl で別の比較を書いても linter は通る。値等価の意味の確認については、gen-tests が「同じフィールド値を持つ 2 インスタンスが `eq` であること」を検証するテスト骨格を生成できる（実行レベルの意味確認は後続の gen-tests ADR が定める contract に依存する）。いずれかが欠ければ linter が違反を報告する。`TraitImplRequired`（Entity の equality 宣言チェックと同じ rule kind）として実装する（D15 参照）。

**public mutation surface削減チェック（最小コア・不変性の近似）**: `ValueObject` ロールを持つ型について、以下の 2 つを確認する。

- **`&mut self` 禁止（linter / opt-in 時に厳格適用）**: メソッドに `&mut self`（`SelfReceiver::ExclusiveRef`）が宣言されていれば linter が違反を報告する。VO は型定義上の public mutation surface を持つべきでない。D9 の DomainEvent `ForbiddenMethodReceiver` と同じ rule kind。
- **struct public field 禁止（linter / opt-in 時に厳格適用）**: `StructShape::Plain` の public field / `StructShape::Tuple` の public 要素がカタログ上に存在すれば linter が違反を報告する。外部から直接書き換えられる surface を持つ VO は値として扱えない。D9 の DomainEvent `NoPublicField` と同じ rule kind。

**不変性チェックの限界（注記）**: 不変性チェック（struct(Plain/Tuple) の public field 禁止 + `&mut self` 禁止）は、**型定義上の public mutation surface を減らす surface check の近似**である。完璧な不変性は型レベルでは保証できない——所有者が `&mut` で値を保持すれば enum payload を `&mut` pattern match で変更でき、interior mutability（`Cell` / `RefCell` 等）も静的には防げない。これらは静的宣言チェックの構造的限界として受け入れ、完全な不変性保証は gen-tests や利用者のレビューに委ねる。

**DomainEvent との関係**: D9 の DomainEvent も不変であるべきデータを要件とするが、DomainEvent は「ドメインで起きた事実」という時間的文脈と専用の `ForbiddenMethodReceiver` / `NoPublicField` 組み合わせを持つ。D18 の VO public mutation surface削減チェックは同じ rule kind を流用する——VO と DomainEvent はそれぞれ独立した意味論ロールとして宣言され、共通の検査機構を適用する設計である。

- **有効ロール**: `ValueObject` variant のみ。他の variant にはこの決定は適用しない。

### D5: `identity` フィールド — Entity / AggregateRoot variant に構造的に必須、equality 宣言の存在は linter で保証

> ユーザー決定（2026-05-27）: `identity` は `DataRole::Entity` / `DataRole::AggregateRoot` variant に **構造的に必須** とする。値型は public accessor 参照（public named field の存在 または public getter メソッドの存在）とし、field か getter かを区別する。`PartialEq` / `Eq` / `Hash` の catalogue 宣言（`trait_impls`）の存在は linter で保証する（スキーマと linter の役割分担）。

`DataRole::Entity` と `DataRole::AggregateRoot` の variant が `identity: IdentityAccessor` フィールドを持つ（`Option` 不可）。`IdentityAccessor` は getter メソッド名を保持する（`struct IdentityAccessor(MethodName)`、または単一 variant の enum——型定義の粒度は実装に委ねる）。public field identity は禁止（根拠は以下の注記を参照）。

```json
// <!-- illustrative, non-canonical -->
{ "role": "Entity", "identity": { "getter": "id" }, "invariants": [] }
```

**スキーマと linter の役割分担**: 型（スキーマ）で守れる存在性の制約は型で表現し、型だけでは構造的に結びつけられない制約は linter で保証する——これは「型システムで不正な状態を表現不可能にする」原則の型レベルでの限界に応じた役割分担である。

- **スキーマで保証（identity フィールドの必須性）**: `DataRole::Entity` / `DataRole::AggregateRoot` variant が `identity: IdentityAccessor` フィールドを必須として持つ（`Option` 不可）。「Entity なのに identity 未宣言」という状態がスキーマ上で構造的に表現不可能になる。
- **linter で保証（equality 宣言の存在）**: `PartialEq` / `Eq` の impl は、`identity` フィールドとは別データ（`CatalogueDocument.trait_impls`）に記録される。型レベルで「Entity variant を持つ型エントリに必ず trait_impls 内で PartialEq が宣言される」という不変条件を構造的に強制することはできないため、linter（カタログ宣言の A-side 静的チェック機構）で検査する。linter は宣言（A-side）のみを見る——「宣言した trait_impl の実装が実際に存在するか」は既存 signal 評価器が別途カバーする。
  - **注記（D5 冒頭のユーザー決定との差異 — Hash と field identity）**: D5 冒頭のユーザー決定（2026-05-27）の引用文では `PartialEq` / `Eq` / `Hash` を列挙し、accessor として「public named field の存在 または public getter メソッドの存在」を許容しているが、その後の検討で 2 点変更した。(1) `Hash` は HashMap 利用の都合（DDD の Entity 本質である identity-based equality とは独立した要件）として最小コアから外し、カスタム拡張例に分離した。最小コアの equality 宣言チェックは `PartialEq` / `Eq` のみである。(2) public field identity（`IdentityAccessor::Field`）は最小コアで禁止した。Rust の public field は `entity.id = ...` で外部から変更可能であり、Entity の identity 安定性（identity-based equality の前提）に反する。identity は public getter メソッド（`&self -> &Id`、private field を読み取り専用で公開）で参照することを標準とする。引用文は承認時の原文を保持する（getter 統一は 2026-06-14 のユーザー決定（`chat_segment:user-decision:identity-getter-only:2026-06-14`）による）。

- **値型**: public getter メソッド参照（`MethodName`）。
  - **public getter メソッドの存在**（例: `pub fn id(&self) -> &UserId` → method 名 `id`）
  - private field はカタログに出ないため getter で参照する（§3 参照）。
  - 根拠: カタログは public surface のみを名前で列挙する（§3 参照）。`TypeRef`（型参照）を指定しても「UserId 型が存在するか」しか確認できず、「この Entity が UserId で同一性を持つ」という accessor との関係は検証できない——型参照だけでは entity の identity 関係は原理的に辿れない。よって getter accessor 参照を採用する。public field identity を禁止する理由: Rust の public field は外部から直接書き換え可能（`entity.id = new_id`）であり、Entity の identity 安定性を型レベルで保証できない。getter（`&self` receiver、`returns` 非 `()`）は読み取り専用であり、private field を安全に公開する。
- **意味**: この Entity の同一性を担う public getter accessor。Entity の定義は identity-based equality であるため、identity を宣言していない Entity は概念として成立しない（`None` = 不正な状態と 1:1 対応——D14 適用）。
- **有効ロール**: `Entity` / `AggregateRoot`（AggregateRoot は常に Entity でもある）。variant に結合するため他のロールでは `identity` フィールドは存在しない。
- **機械検査ルール**:
  - **getter 存在 + signature チェック（linter / opt-in 時に厳格適用）**: `identity` で指定した getter メソッド名が、対象型のカタログ上の public method 宣言集合（`TypeEntry::methods: Vec<MethodDeclaration>`）に存在するかを確認する。存在しなければ linter が違反を報告する。さらに対象 `MethodDeclaration` が `receiver: Some(SelfReceiver::SharedRef)`（`&self`）/ `params` が空 / `returns` が `()` 以外であることを確認する。名前一致のみでは `fn id(&mut self)` や引数付きメソッドが getter として通ってしまうため、identity getter の契約（`&self` + 引数なし + 非 `()` 返却）を名前存在と signature で一括して検査する。このルールはロール参照整合ではなく getter accessor の存在と signature を検査するため `AccessorSignatureRequired`（`target_field: "identity"`）として実装する（D15 参照）。
  - **equality impl 宣言チェック（linter / opt-in 時に厳格適用）**: `Entity` / `AggregateRoot` ロールの型に対し、`PartialEq` / `Eq` の impl が catalogue の `trait_impls` に宣言されているかを確認する（A-side のみ）。このチェックは「`PartialEq` / `Eq` の impl 宣言が存在するか」を検査するものであり、「その equality が真に identity-based（accessor のみで比較）かどうか」は検証しない（D5 冒頭の equality チェックの限界を参照）。DDD の Entity 本質は identity-based equality であるため、impl 宣言の存在を最低限の surface として確認する。いずれかが欠ければ linter が違反を報告する。宣言した trait_impl の実装整合（C-side での実装の有無）は既存 signal 評価器が別途カバーする。テスト生成器（gen-tests）は `identity` 宣言を読み、「同一 identity を持つ 2 インスタンスが `eq`、異なる identity を持つ 2 インスタンスが `ne` であること」を検証するテスト骨格を生成できる（実行レベルの意味確認は後続の gen-tests ADR が定める contract に依存する）。
  - **equality チェックの限界（注記）**: 上記チェックは catalogue の `trait_impls` に対応する宣言が**存在するか**までしか検証しない。`TraitImplDeclV2` は `trait_ref` + `for_type` のみを持ち（`methods` フィールドは存在しない——impl body / signature alignment は Rust コンパイラに委譲）、rustdoc も impl body を出力しない。よって「その `PartialEq` が宣言した accessor だけを使って比較しているか（真に identity-based か）」は静的に検証不可能である。`#[derive(PartialEq)]`（全フィールド比較）でも linter は通る。accessor 必須（どれが identity か）と equality 宣言存在の二段で「静的にチェックできる最大限の近似」を押さえる、という位置づけとして受け入れる。
  - **テスト生成**: accessor 経由で identity を取り出し「異なる identity を持つ 2 インスタンスが `ne` であること」を検証するテスト骨格を生成できる（実行レベルの意味確認は後続の gen-tests ADR が定める contract に依存する）。

### D6: `exclusive_members` / `shared_value_objects` フィールド — AggregateRoot variant に結合

`DataRole::AggregateRoot` variant が `exclusive_members: Vec<TypeRef>` と `shared_value_objects: Vec<TypeRef>` の 2 フィールドを持つ。Value Object（Money / Email / Address 等）は複数集約で共有されるのが自然であるため、集約専属 Entity（`exclusive_members`）と共有可能 VO（`shared_value_objects`）を型レベルで分離する。

```json
// <!-- illustrative, non-canonical -->
{ "role": "AggregateRoot", "identity": { "getter": "id" },
  "exclusive_members": ["OrderLine"],
  "shared_value_objects": ["Money", "Address"],
  "invariants": [], "emits": [] }
```

- **`exclusive_members`**:
  - **型**: `Vec<TypeRef>`
  - **意味**: この集約に専属する Entity の型名リスト。重複所属禁止・集約外からの直接参照禁止の検査対象。
  - **任意（D14）**: 空 Vec 正当（1 型だけで完結する集約は存在する）。
- **`shared_value_objects`**:
  - **型**: `Vec<TypeRef>`
  - **意味**: この集約で使用する Value Object の型名リスト。複数集約で共有されうるため、重複所属・集約外参照禁止の**対象外**（共有が正当）。
  - **任意（D14）**: 空 Vec 正当。
- **有効ロール**: `AggregateRoot` variant のみ。他の variant にはこれらのフィールドが存在しないため不正な組み合わせは構造的に排除される。
- **機械検査ルール（Aggregate Boundary linter / opt-in 時に厳格適用）**:
  - **exclusive_members → Entity ロール整合チェック（opt-in 時に厳格適用）**: `exclusive_members` に列挙された型が、カタログ上で `Entity` ロールで宣言されているかを確認する（`AggregateRoot` は除く。集約内専属の Entity であることが要件であり、入れ子集約ルートの宣言を防ぐ）。未宣言であれば linter が違反を報告する。`ReferencedRoleConstraint`（`target_field: "exclusive_members"`）として実装する（D15 参照）。
  - **shared_value_objects → ValueObject ロール整合チェック（opt-in 時に厳格適用）**: `shared_value_objects` に列挙された型が、カタログ上で `ValueObject` ロールで宣言されているかを確認する。未宣言であれば linter が違反を報告する。`ReferencedRoleConstraint`（`target_field: "shared_value_objects"`）として実装する（D15 参照）。
  - **重複所属チェック（opt-in 時に厳格適用）**: `exclusive_members` に列挙された型が、他の `AggregateRoot` の `exclusive_members` に同時に列挙されていないかを確認する。重複があれば linter が違反を報告する。`shared_value_objects` は対象外（重複してよい）。`FieldElementUniqueAcrossEntries`（`target_field: "exclusive_members"`）として実装する（D15 参照）。
  - **直接参照チェック（shallow）（opt-in 時に厳格適用）**: `exclusive_members` に列挙された型が、その集約ルートおよび同一集約の `exclusive_members` / `shared_value_objects` に列挙された型以外の `TypeEntry` のメソッド（`TypeEntry::methods`）の引数型・戻り値型として直接参照されていないかを確認する（shallow lint; 検査対象拡大はカスタム拡張例参照）。同一集約内の参照は許容し、集約外からの専属 Entity の直接参照のみを違反とする。違反があれば linter が報告する。`NoExternalReferenceInMethods`（`target_field: "exclusive_members"`）として実装する（D15 参照）。`TypeEntry::methods` に限定するのは、AggregateRoot / Entity のロール意味論（集約境界の強制）に最も直接対応する surface だからである。`TraitEntry.methods`（port / application service のシグネチャ）/ `FunctionEntry` / struct fields / generic 引数まで検査対象に含めるかはプロジェクトの構造・移行コストに依存するため、最小コアには含めずカスタム拡張例に委ねる。
  - **ValueObject 独立性チェック（opt-in 時に厳格適用）**: `ValueObject` ロールを持つ型のメソッド（`TypeEntry::methods`）の引数型・戻り値型に `Entity` / `AggregateRoot` ロールの型が現れないかを確認する。VO は値であり、identity を持つ型（Entity / AggregateRoot）に依存しないのが DDD 原則である——これは shared かどうかに関わらず ValueObject ロール一般の性質であり、`shared_value_objects` に列挙された特定型に限定する必要はない。違反があれば linter が報告する。`target_roles: [ValueObject]` で適用する形で実装する（D15 参照）。
  - **Rust の型システムとの関係**: Rust の可視性（`pub(crate)` / `pub(super)` 等）でアクセス制御を実装することはできるが、カタログの boundary lint はモジュール境界ではなく **意図された集約境界** を確認する。モジュールが分かれていても同一集約内なら参照を許容し、モジュールが同一でも別集約の専属 Entity を直接受け取ることは禁じる。この区別は Rust の型システム単独では表現できない。よって lint の追加価値は実在する。

### D7: `emits` フィールド — AggregateRoot / DomainService variant に結合

`DataRole::AggregateRoot` と `DataRole::DomainService` の各 variant が `emits: Vec<TypeRef>` フィールドを持つ。

```json
// <!-- illustrative, non-canonical -->
{ "role": "AggregateRoot", "identity": { "getter": "id" }, "emits": ["OrderPlacedEvent", "OrderCancelledEvent"], "invariants": [], "exclusive_members": [], "shared_value_objects": [] }
```

```rust
// <!-- illustrative, non-canonical -->
DataRole::DomainService { emits: Vec<TypeRef> }
```

- **型**: `Vec<TypeRef>`
- **意味**: この variant のメソッドが発行する Domain Event の型名リスト。
- **任意（D14）**: 空 Vec も正当な状態（イベントを発行しない集約は存在する）なので宣言省略可。
- **有効ロール**: `AggregateRoot` / `DomainService`。両 variant が `emits` フィールドを持つ。他の variant にはこのフィールドが存在しない。DomainService は `reacts_to` / `handles` を持たない（D8 参照）。
- **機械検査ルール（linter / opt-in 時に厳格適用）**: `emits` に列挙された各型が、カタログ上で `DomainEvent` ロールを持つ `DataRole` として宣言されているかを確認する（D9 で追加する新ロール）。未宣言であれば linter が違反を報告する。これにより `emits` と `DomainEvent` の宣言の整合性がカタログ宣言（A-side）の内部で保証される。

### D8: `handles` フィールド — UseCase variant に結合（D3 段1: 既存ロールへの schema 強制）

`DataRole::UseCase` は `handles: Vec<TypeRef>` フィールドを持つ。UseCase は D3 段1（既存ロールへの schema 強制）の対象であり、同じ段 1 の不可分な schema migration に含める `EventPolicy.reacts_to` とは別決定（D16）で扱う。DomainService は `handles` を持たない（reactive orchestration は EventPolicy に集約し、DomainService を純粋ドメインロジック——emit はするが react しない——に保つ）。

```json
// <!-- illustrative, non-canonical -->
{ "role": "UseCase", "handles": ["OrderPlacedEvent"] }
```

```rust
// <!-- illustrative, non-canonical -->
DataRole::UseCase { handles: Vec<TypeRef> }
```

- **型**: `Vec<TypeRef>`（空可能）。
- **意味**: この UseCase が処理を起動する Domain Event の型名リスト。UseCase は application 層に置き、Repository / 外部 I/O port を呼び出す（UseCase は application 層で副作用を調停し、I/O は port 経由で外側に委譲する）。
- **任意（D14）**: コマンド駆動の UseCase は Domain Event をハンドルしない場合がある（空 Vec 正当）。
- **機械検査ルール（linter / opt-in 時に厳格適用）**: `handles` に列挙された各型が、カタログ上で `DomainEvent` ロールで宣言されているかを確認する。未宣言であれば linter が違反を報告する。`ReferencedRoleConstraint`（`target_field: "handles"`）として実装する（D15 参照）。

**EventPolicy.reacts_to との対比**: `EventPolicy` は D16 で `reacts_to: NonEmptyVec<TypeRef>` フィールドを持つ新 variant として定義し、その payload schema は D3 段1 の不可分な schema migration に含める。UseCase.handles（`Vec`、空可能）と EventPolicy.reacts_to（`NonEmptyVec`、非空必須）は型レベルで区別される。

**UseCase / 既存ロールの layer 配置チェックについて**: UseCase は application 層に置くべきロールだが、本 ADR が定める最小コアルールに UseCase の layer 配置チェック（`KindLayerConstraint`）は含めない。UseCase / Repository / ApplicationService 等の**既存ロール**の layer 配置は、既存の `KindLayerConstraint` 機構（本 ADR の対象外、別途設定）で担保される。本 ADR が新たに定義するのは**新ロール EventPolicy の domain 層配置チェック**のみである（D16）。これは非対称ではなく、新ロールの admission 要件として layer 配置を最小コアに含める方針による。

**`consistency` フィールドについて（今回は保留）**: 研究レポートは UseCase 向けに `consistency: Atomic | Eventual` の宣言を提案しているが、現時点では保留とする。`Eventual`（Saga / バックグラウンド補償）を lint するには Saga 対応が先に必要（後述の「Saga / Process Manager / Read Model」付記参照）。`Atomic` の lint（「単一 UseCase 内で複数 AggregateRoot を変更していないか」）は rustdoc 静的情報からの確認が困難。Saga 設計が固まったタイミングで再評価する。

---

## Layer ② — 新ロールの追加

### D9: `DomainEvent` を `DataRole` に追加する

`DataRole` enum に `DomainEvent` variant を追加する。

```rust
// <!-- illustrative, non-canonical -->
pub enum DataRole {
    // ...既存 13 variant...
    DomainEvent,  // payload なし — unit variant のまま
}
```

- **意味**: ドメイン内で起きた事実を表す**不変であるべきデータ**。DDD の本質として不変性が期待されるが、最小コアの検査は public mutation surface を持たないことを近似的に確認するにとどまる（以下の限界注記を参照）。
- §2 の admission 原則は以下の最小コアルールで満たす。
- **機械検査ルール**:
  - **public mutation surface削減チェック（linter / opt-in 時に厳格適用）**: `DomainEvent` ロールを持つ型に `&mut self`（`SelfReceiver::ExclusiveRef`）を受け取るメソッドが宣言されていれば linter が違反を報告する。DomainEvent は不変であるべきデータであり、public な mutation surface を持つべきでない。`ForbiddenMethodReceiver` として実装する（D15 参照）。
  - **struct public field 禁止チェック（linter / opt-in 時に厳格適用）**: `DomainEvent` ロールの型について、struct 形状の public field がカタログ上に存在すれば linter が違反を報告する——`StructShape::Plain` の `fields: Vec<FieldDecl>`（plain struct の public field）/ `StructShape::Tuple` の tuple 要素（tuple struct の public 要素）。**enum の variant payload（`VariantPayload::Struct` / `VariantPayload::Tuple`）は禁止しない**: `OrderEvent::Placed { order_id }` のように enum バリアントに payload を持つ DomainEvent は妥当な表現である。enum variant payload は `&mut` pattern match で変更可能であり、`&mut self` 禁止では防げない——この限界は以下の注記が明記する通り、static に防げない範囲として受け入れる。`NoPublicField` として実装する（D15 参照）。
  - **public mutation surface削減チェックの限界（注記）**: 最小コアは **public mutation surface を持たないことを近似検査する**。完璧な不変性は型レベルでは保証できない——所有者が `&mut` で値を保持すれば enum payload を `&mut` pattern match で変更でき、interior mutability（`Cell` / `RefCell` 等）も静的には防げない。これらは静的宣言チェックの構造的限界として受け入れ、完全な不変性保証は gen-tests や利用者のレビューに委ねる。
  - **D7 連動チェック（linter / opt-in 時に厳格適用）**: `emits` に参照されているが `DomainEvent` ロールで宣言されていない型は linter が違反を報告する（D7 との連動）。`ReferencedRoleConstraint`（`target_field: "emits"`）として実装する（D15 参照）。
- **なぜ既存 variant で代替できないか**: `ValueObject` は不変性を持つが「ドメインで起きた事実」という時間的文脈を持たない。`Command` は「これからやること」を表し `DomainEvent` とは逆方向。`Dto` は層を越えるデータ容器であり、ドメインロジックとは切り離された意味論を持つ。いずれも `DomainEvent` の lint ルール（可変メソッドの禁止）とは異なる検査対象であるため、専用 variant が必要。

### D10: `Repository` を独立した `ContractRole::Repository` variant に昇格する

> ユーザー決定（2026-05-26）: Repository を独立した `ContractRole` variant として追加する。`SecondaryPort` + `Option<aggregate>` での表現は「aggregate なし Repository」「aggregate あり 非Repository」の不正状態を許容するため採用しない。

`ContractRole::Repository { aggregate: TypeRef }` variant を追加する。既存の `ContractRole::SecondaryPort` は aggregate を持たない非 Repository の secondary port を表す。

```rust
// <!-- illustrative, non-canonical -->
pub enum ContractRole {
    SpecificationPort,
    ApplicationService,
    SecondaryPort,                            // aggregate を持たない secondary port
    Repository { aggregate: TypeRef },        // aggregate 必須
}
```

```json
// <!-- illustrative, non-canonical -->
{ "role": "Repository", "aggregate": "Order" }
```

- **`aggregate` フィールドの必須化（D14 適用）**: Repository は必ず 1 つの aggregate を持つ。`aggregate: Option<TypeRef>` は「aggregate なしの Repository」という不正な状態を表現可能にしてしまうため `Option` 不可。
- **機械検査ルール（linter / opt-in 時に厳格適用）**:
  - `aggregate` フィールドで参照された型が、カタログ上で `AggregateRoot` ロールで宣言されているかを確認する。未宣言であれば linter が違反を報告する。
  - テスト生成器: 「save して find すると同じ内容が返る」という永続化ラウンドトリップテストの骨格を生成できる（実行レベルの意味確認は後続の gen-tests ADR が定める contract に依存する）。
- **なぜ独立 variant か**: Repository は必ず 1 つの aggregate を持ち、非 Repository の SecondaryPort は持たない。独立 variant にすることで「aggregate なし Repository」「aggregate あり 非Repository」の両方の不正状態が構造的に排除される（make illegal states unrepresentable）。`SecondaryPort` + `Option<aggregate>` では両不正状態が表現可能になる。

### D11: Aggregate Boundary 最小コア lint — exclusive_members を基盤とする

D6 で `DataRole::AggregateRoot` variant に結合した `exclusive_members` / `shared_value_objects` フィールドを基盤として、Aggregate Boundary lint を linter の独立した検査ルールとして実装する。このルールは新しい `DataRole` variant ではなく、`AggregateRoot` variant のフィールドが宣言されたときに opt-in 設定で有効になる（D15）。

- **linter: exclusive_members → Entity ロール整合チェック（opt-in 時に厳格適用）**: D6 と同じ（`ReferencedRoleConstraint`、`target_field: "exclusive_members"`）。
- **linter: shared_value_objects → ValueObject ロール整合チェック（opt-in 時に厳格適用）**: D6 と同じ（`ReferencedRoleConstraint`、`target_field: "shared_value_objects"`）。
- **linter: 重複所属チェック（opt-in 時に厳格適用）**: 同一の型が複数の `AggregateRoot` の `exclusive_members` に列挙されていないか。違反があれば linter が報告する。`shared_value_objects` は対象外（重複してよい）。`FieldElementUniqueAcrossEntries`（`target_field: "exclusive_members"`）として実装する（D15 参照）。
- **linter: 直接参照チェック（shallow）（opt-in 時に厳格適用）**: `exclusive_members` の型が、その集約ルートおよび同一集約の `exclusive_members` / `shared_value_objects` に列挙された型以外の `TypeEntry` のメソッド（`TypeEntry::methods`）の引数型・戻り値型として直接受け取られていないか確認する（検査対象の拡大はカスタム拡張例参照）。同一集約内の参照は許可し、集約外からの専属 Entity の直接参照のみを違反とする。違反があれば linter が報告する。`NoExternalReferenceInMethods`（`target_field: "exclusive_members"`）として実装する（D15 参照）。`TypeEntry::methods` に限定するのは D6 と同じ理由（AggregateRoot / Entity ロール意味論に最も直接対応する surface; 拡大対象はプロジェクト構造に依存するためカスタムに委ねる）。
- **linter: ValueObject 独立性チェック（opt-in 時に厳格適用）**: `ValueObject` ロールを持つ型のメソッド（`TypeEntry::methods`）の引数型・戻り値型に `Entity` / `AggregateRoot` ロールの型が現れないかを確認する。D6 と同じ設計方針——VO は値であり、identity を持つ型に依存しないのが DDD 原則であり、これは shared かどうかによらず ValueObject ロール一般に適用する（D6 参照）。違反があれば linter が報告する。`target_roles: [ValueObject]` で適用する形で実装する（D15 参照）。
  - **Rust の可視性との関係**: D6 と同じ（意図された集約境界はモジュール可視性と別物で lint の価値がある。可視性で十分なら `exclusive_members` を空にして無効化できる）。

### D16: EventPolicy を独立した DataRole::EventPolicy variant として追加する

`DataRole` に `EventPolicy { reacts_to: NonEmptyVec<TypeRef> }` variant を追加する。

`EventPolicy` とするのは、DDD の一般的な Policy（価格計算・割引判定など event に反応しない純粋方針）と、event に反応して判断を返す専用ロールを混同しないためである。フィールド名を `reacts_to` とした理由は、UseCase（処理を起動する handler）と EventPolicy（イベントに反応して判断を返す宣言型リアクター）の責務の違いを名前で明示するためである——処理の実行は application 層の UseCase が担い、EventPolicy は副作用なしの判断のみを行う（D8 参照）。

```rust
// <!-- illustrative, non-canonical -->
pub enum DataRole {
    // ...
    EventPolicy { reacts_to: NonEmptyVec<TypeRef> },  // 宣言型リアクター — 型で非空保証
}
```

```json
// <!-- illustrative, non-canonical -->
{ "role": "EventPolicy", "reacts_to": ["OrderPlacedEvent", "PaymentFailedEvent"] }
```

**EventPolicy の設計意図は宣言型リアクター**（Domain Event に反応して判断を返す domain 層のロールであるべき）。副作用なしの pure な判断ロールを意図するが、最小コアの検査は method signature 上の依存を surface-level で確認するにとどまる（完全な purity 保証ではない——field / constructor params / body 依存はカスタム拡張例参照）。反応するイベントが 0 個の EventPolicy は概念として成立しない。`reacts_to` は `NonEmptyVec<TypeRef>` で**型による非空保証**とする。空の `reacts_to` は型レベルで表現不可能（make-illegal-states-unrepresentable）。Repository の `aggregate`（単一 `TypeRef` 必須）と並行する設計（D14 適用）。

`NonEmptyVec<TypeRef>` は domain 層の新型（serde-free）。infrastructure の codec で JSON 配列 ⇄ `NonEmptyVec` を変換し、空配列は decode エラーとして弾く（自作 newtype か既存 crate かは実装時に選択する）。

**§2 admission 原則の充足**: EventPolicy は Repository（D10）と並行する構造で admission を満たす。(1) `reacts_to` を `NonEmptyVec` にして「反応するイベントが必ず 1 つ以上」を **schema invariant**（型/codec で強制、opt-in 不能）で保証——`UseCase.handles` は `Vec`（空可能）であり、この型レベルの差が EventPolicy を UseCase と区別する構造的独自性となる。(2) `reacts_to` 参照先が `DomainEvent` ロールかを確認する opt-in linter ルール（`ReferencedRoleConstraint`）。(3) schema invariant（`reacts_to` 非空）と linter ルール（参照先 DomainEvent 整合）の組み合わせにより、カタログ宣言レベルで EventPolicy の意味論的整合を確認できる。実行可能な reactive テスト骨格の生成（どのメソッドが入力イベントを受けどう反応するか）は method-level 契約（今後の判断ポイント）の確定後に可能になる。この 3 点（schema invariant + linter + 宣言整合）で admission を満たす。

**旧 D12 が EventPolicy を却下した 2 理由の訂正**:

1. **「reactive エンジンがない」**: カタログ / linter は実行せず型契約を宣言・検査するだけである。EventPolicy ロール追加は reactive エンジンの実装を意味しない。実行エンジンの有無は型契約の宣言とは別レイヤーであり、却下理由にならない。
2. **「独自の機械検査ルールがない」**: `NonEmptyVec` による schema invariant（reacts_to 非空）・`reacts_to` 参照先 DomainEvent 宣言整合 linter の 2 点で覆る（実行可能な reactive テスト生成は method-level 契約確定後、今後の判断ポイントを参照）。

**`reacts_to` の有効ロールは EventPolicy（`NonEmptyVec`、型で非空保証）のみ**。UseCase の `handles`（`Vec`、任意）との対比は D8 を参照。

**`reacts_to` は surface 宣言である**: `reacts_to` は EventPolicy が反応する Domain Event の型名リスト（surface）を宣言する。どのメソッドが入力イベントを受け取り、何を判断・決定として返すか（handler method / input event / decision output の契約）は method-level の拡張であり、本 ADR の最小コアには含めない。最小コアが保証するのは「`reacts_to` に列挙された型が `DomainEvent` ロールで宣言されているか」という宣言整合にとどまる。「各 `reacts_to` イベントに対して EventPolicy が実際に反応するか」を検証する実行可能な reactive テスト骨格の生成は、method-level 契約（handler method / input event / decision output）が確定した後に可能になる。method-level 拡張は今後の判断ポイントで扱う。

**EventPolicy は emit しない**: emit は AggregateRoot / DomainService が担う。EventPolicy が後続イベントを emit するモデル（Process Manager / Saga）は状態機械を伴うため、今回は対象外とし「今後の判断ポイント」の Saga に委ねる。本 ADR の EventPolicy はステートレスな単純リアクター（イベント → 判断）に限定する。

**DomainService から reacts_to を外す根拠**: 宣言型リアクションを EventPolicy に集約し、DomainService を純粋ドメインロジック（emit はするが react しない）に保つ（D8 参照）。

**EventPolicy の配置と副作用制約**: EventPolicy は **domain 層**に置き、**純粋な判断を返すロールであるべき**。最小コアは method signature 上に Repository / UseCase ロールの型が出現しないことを surface-level で検査する（field / constructor params / body 依存はカスタム拡張例参照。完全な purity 保証ではない）。副作用の実行は application 層が担う（EventPolicy が判断を返し、application 層が実行するという設計意図）。

- **機械検査ルール（linter / opt-in 時に厳格適用）**:
  - **reacts_to 参照先 DomainEvent チェック**: `reacts_to` の各型がカタログ上で `DomainEvent` ロールで宣言されているかを確認する（D16 / D9 連動）。未宣言であれば linter が違反を報告する。`ReferencedRoleConstraint { expected_role: RoleKind::DomainEvent }`（`target_field: "reacts_to"`）、`target_roles: [EventPolicy]` として実装する（D15 参照）。
  - **domain 層配置チェック（linter / opt-in 時に厳格適用）**: EventPolicy エントリが domain 層以外で宣言されていれば linter が違反を報告する。`KindLayerConstraint { permitted_layers: ["domain"] }`、`target_roles: [EventPolicy]` として実装する（既存 variant 流用、D15 参照）。
  - **`&mut self` 禁止（linter / opt-in 時に厳格適用）**: EventPolicy ロールの型のメソッドに `&mut self`（`SelfReceiver::ExclusiveRef`）が宣言されていれば linter が違反を報告する。純粋な判定のみを行う型が内部状態を変更することはない。`ForbiddenMethodReceiver { forbidden_receiver: ExclusiveRef }`、`target_roles: [EventPolicy]` として実装する（D15 参照）。
  - **副作用ロール禁止（linter / opt-in 時に厳格適用）**: EventPolicy のメソッドシグネチャ（引数型・戻り値型）に Repository / UseCase ロールの型が出現する場合に linter が違反を報告する（shallow: `TypeEntry::methods` のシグネチャのみ確認）。I/O・永続化・UseCase 呼び出しを持たないことを surface level で確認する。`NoRoleInMethodSignature { forbidden_roles: [RoleKind::Repository, RoleKind::UseCase] }`、`target_roles: [EventPolicy]` として実装する（D15 参照）。ここで `RoleKind::Repository` は `ContractRole::Repository` に対応する discriminant であり、`Vec<DataRole>` では表現できない——これが `RoleKind` 導入の主要な動機である。

### D12: Presenter / Controller は追加しない

研究レポートが提案した残りのパターンについて判断する。

**Presenter / Controller（Clean Architecture の Primary Adapters）**:
追加しない。SoTOHE の TDDD は **内側の六角形**（Domain / UseCase 層）を主な対象とする。Primary Adapters（CLI / HTTP handler 等）は SoTOHE の `apps/` 層に相当し、ハーネスが提供する型レベルの検査の主対象ではない。Presenter / Controller に固有の機械検査ルールを定義しにくいことも理由（「プレゼンテーション層の型である」以上の述語を自動確認できない）。

Presenter / Controller を追加したい採用プロジェクトは、`informal_grounds` に rationale を記録した上で独自カタログエントリとして扱うことができる。

### D13: Saga / Process Manager および Read Model（CQRS）は今回追加しない — 具体的要求が来た時点で別 ADR で判断する

研究レポートが提案した残りのパターンのうち、Saga / Process Manager と Read Model（CQRS）は今回の拡張対象に含めない。

**Saga / Process Manager**:
追加しない。状態機械のステップ遷移を機械検査するには、`typestate` フィールド（struct 形状と直交配置、ADR `2026-05-26-1002-typestate-struct-kind-orthogonal.md`）を拡張した Saga 固有の状態遷移スキーマが必要であり、設計コストが高い。また採用先での具体的なユースケースが不明確であるため、今回は追加しない。EventPolicy が後続イベントを emit するモデル（Process Manager への拡張）も同じ条件で再評価する（D16）。採用プロジェクトから「Saga を TDDD で管理したい」という具体的要求が来た場合は、別 ADR で設計する。

**Read Model（CQRS）**:
追加しない。「ドメインロジックを持たない読み取り専用ビュー」という定義は `Dto` との区別が曖昧であり、`source_events: [EventRef]` を lint するためには Domain Event の配信機構が必要である。Event Sourcing を採用するプロジェクトからの具体的な要求が来た時点で、別 ADR で判断する。

---

## Rejected Alternatives

### A1: 全意味論フィールドを `TypeEntry.docs` に文字列で埋め込む

却下。自由テキストは機械検査不可能。TDDD のゴールは lint / signal が読める構造データの宣言。

### A2: `invariants` を `Vec<String>` または `Vec<TypeRef>` にする

`Vec<String>` は却下。`String` は空文字を許容するため不正な状態が生まれる。

`Vec<TypeRef>` も却下。`TypeRef` は型参照専用 newtype であり、`FieldDecl.ty` / `MethodDeclaration.returns` / `VariantPayload::Tuple` / `TraitEntry.supertrait_bounds` といった「型スロット」での使用を前提に設計されている。invariant は述語（メソッドや spec 要素）への参照であり、型スロットとは異なる。述語参照を型スロットに詰め込むと `methods.rs` の typed newtype 設計意図（identifier kind 間の type confusion をコンパイル時に検出する）に反し、linter がどの解決規則を適用すべきか曖昧になる。

採用した `Vec<InvariantDecl>`: `InvariantDecl` は名前付きドメインルールの構造宣言（struct）であり、検査手段を表す `InvariantPredicate` enum（現在は `SelfMethod` のみ、将来拡張の軸として enum を維持）を `predicate` フィールドに持つ。`SpecRef` を variant から外した理由は、根拠（仕様へのトレース）は検査手段の代替ではなく traceability であり、既存の `TypeEntry.spec_refs` で管理するためである。`InvariantName` / `InvariantPredicate` という専用型を設けることで、述語参照と型スロット（`TypeRef`）の種別混同は構造的に排除される——D5 の `IdentityAccessor`（getter メソッド名を保持する専用型）と同じ方針（各参照先を専用型で表現し、型スロットと混在させない）。

### A3: `DomainEvent` を `DataRole::ValueObject` で代替する

却下。`DomainEvent` は「ドメインで起きた事実」という時間的意味を持ち、`emits` / `reacts_to` の参照先としての役割を担う。`ValueObject`（値）・`Command`（これからやること）・`Dto`（層を越えるデータ容器）とは異なる語彙であり、専用 variant とする。なお、public mutation surface 削減チェック（`&mut self` 禁止 + struct public field 禁止）の rule kind は VO と共通のものを流用するが（D18 参照）、ロールとしての意味論——時間的文脈 / `emits` 参照 / `reacts_to` 参照先の役割——は `ValueObject` と異なるため、専用 variant が必要である。

### A4: `ContractRole::SecondaryPort` + `aggregate: Option<TypeRef>` フィールドで Repository を表現する（旧 D10 / 旧 D13 の暫定方針）

却下。`Option<aggregate>` で表現すると「aggregate なし Repository」「aggregate あり 非Repository」の2つの不正な状態が表現可能になる。独立 variant `ContractRole::Repository { aggregate: TypeRef }` にすることで両方が構造的に排除される（make illegal states unrepresentable）。D10 でこの判断を確定させたため、本案は採用しない。

### A5: `emits` / `handles` を `TraitEntry` にも追加する

保留。Domain Event を発行するのが `trait` レベルで宣言された Port の場合（例: Application Service が Event を返す）は有用かもしれない。しかし現行の SoTOHE dogfood では具体型（`AggregateRoot` / `DomainService`）の `emits` で十分なため、現時点では `DataRole` variant のみに結合する。

### A6: 意味論フィールドを `TypeEntry` / `TraitEntry` 構造体に平置きする

却下。`TypeEntry { role: DataRole, invariants: Vec<InvariantDecl>, identity: Option<TypeRef>, ... }` のような平置きでは、`role: Command` + `invariants` / `role: Dto` + `identity` といった「ロール的に意味をなさない組み合わせ」が構造的に表現可能になる。不正な状態を型システムで排除できない。`DataRole` を data-carrying enum にして各 variant が自分の有効フィールドのみを持つことで、不正な組み合わせがコンパイル時に排除される。ADR `knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md` が typestate を kind 軸の struct グループに結合した（`TypeEntry` のトップレベルに置かなかった）のと同じ原則による。

### A7: EventPolicy を DomainService / FreeFunction で代替する（旧 D12 の方針）

却下。reactive なイベント反応（reacts_to）を DomainService に持たせると、DomainService が宣言型リアクションの受け皿になり責務境界が曖昧になる。EventPolicy を独立ロールにすれば reacts_to 非空を schema invariant（NonEmptyVec）で型保証でき、admission 原則を満たしつつ DomainService を純粋ドメインロジックに保てる（D16）。よって代替案ではなく独立ロール（D16）を採用する。

## Consequences

### Positive

- **既存 lint の深化**: `ValueObject` / `Entity` / `AggregateRoot` が、存在チェックを超えた述語レベルの検査を受けられるようになる。
- **gen-tests の基盤**: テスト生成器（gen-tests）が `invariants` / `identity` / `aggregate` を読んでテスト骨格を生成できる基盤が整う。`identity` が public getter accessor 参照であるため、テスト生成器は accessor 経由で identity を取り出すコードを骨格として生成できる（例: `a.id() != b.id()` を前提とした `ne` 検証の骨格）。`invariants` の各 `InvariantDecl` については、`name` を識別子として `SelfMethod` の述語メソッドが `true` を返すことを検証するテスト骨格を生成できる。実行レベルの意味確認は後続の gen-tests ADR が定める contract に依存する。
- **Entity / AggregateRoot の equality impl 宣言チェック**: `PartialEq` / `Eq` の catalogue 宣言（`trait_impls`）の存在が linter で確認される（DDD の Entity 本質に対応する独立ルール）。`PartialEq` / `Eq` チェックを opt-in した場合は違反を許さず、identity accessor の宣言と合わせ、「同一性を持つ型が equality を catalogue に宣言しているか」を確認できる。ただし宣言が真に identity-based equality（accessor のみで比較）かどうかは静的に検証不可能であり、その意味の確認については gen-tests がテスト骨格を生成できる（実行レベルの意味確認は後続の gen-tests ADR が定める contract に依存する——D5 参照）。宣言の実装整合は既存 signal 評価器がカバーする。
- **Domain Event の first-class 化**: `DomainEvent` が独自 variant を得ることで、カタログ読者が「何がドメインイベントか」を即座に判断できる。
- **EventPolicy の first-class 化**: 宣言型リアクターが明示されることで、カタログ読者が「どの型が何のイベントに反応するか」を即座に判断できる。DomainService が pure domain logic（emit はするが react しない）に保たれ、責務境界が明確になる。
- **Aggregate Boundary の明示化**: `exclusive_members` / `shared_value_objects` / `emits` の宣言を通じて、集約境界の意図がカタログに記録される。専属 Entity（`exclusive_members`）と共有可能 VO（`shared_value_objects`）を型レベルで区別することで、Value Object を複数集約で共有する正当なユースケースを歪めずに、専属 Entity の境界侵食のみを lint で検出できる。
- **不正状態の構造的排除**: 意味論フィールドを `DataRole` variant に結合することで、「role に対して意味をなさないフィールドが設定される」不正な状態がコンパイル時に排除される。`ContractRole::Repository` の独立 variant 化により「aggregate なし Repository」「aggregate あり 非Repository」の不正状態も排除される。

### Negative / Trade-offs

- **`DataRole` / `ContractRole` の data-carrying 化による波及変更**: 現行 `DataRole` / `ContractRole` は unit variant で `Copy` + strum の `Display` / `EnumString` derive（variant 名のみのラウンドトリップ）を持つ。data-carrying enum になると `Copy` が外れ（data を持つ enum は `Copy` 不可）、strum の文字列変換も variant 名だけのシリアライズが機能しなくなる。codec / signal evaluator / renderer はすべて role 値を扱うため、以下の変更が必要になる:
  - codec（infrastructure 層）: role の JSON シリアライズ / デシリアライズを variant 名だけの文字列から payload を含む構造体形式に変更する。
  - signal evaluator: role の比較・分岐ロジックを `role == DataRole::Entity` のような unit variant 比較から pattern match に変更する。
  - renderer / contract map: role を表示・集計するコードを payload 付き enum に対応させる。
  - `Copy` を前提としているコード（`role` フィールドの単純コピー代入等）を `Clone` または参照に切り替える。
- **スキーマ変更（breaking）**: codec の更新が必要であり、既存カタログとの後方互換性は保証しない。アクティブなトラックのカタログのみ移行し、非アクティブトラックはライトプロテクトして移行対象外とする。
- **宣言の任意性（一部フィールドのみ）**: `invariants` / `exclusive_members` / `shared_value_objects` / `emits` および UseCase の `handles` は空 Vec が正当な状態であるため任意（D14）。宣言しなくても lint は発火しない。以下の 3 つは例外——`identity` は `Entity` / `AggregateRoot` variant に構造的に必須（D5 / D14）。`Repository.aggregate` は `ContractRole::Repository` variant に構造的に必須（D10 / D14）。EventPolicy の `reacts_to` は `NonEmptyVec<TypeRef>` で型による非空保証（空の reacts_to は型レベルで表現不可能、D16 / D14）。
- **`NonEmptyVec<TypeRef>` の導入コスト**: `NonEmptyVec<TypeRef>` は domain 層の新型（serde-free）。infrastructure codec で JSON 配列 ⇄ `NonEmptyVec` の変換を実装し、空配列は decode エラーとして弾く。自作 newtype か既存 crate かは実装時に選択する。
- **equality 宣言チェックの限界**: `PartialEq` / `Eq` の宣言チェックは catalogue の `trait_impls` に宣言が存在するかまでしか検証できない。`TraitImplDeclV2` は `trait_ref` + `for_type` のみを持ち（`methods` フィールドは存在しない）、rustdoc も impl body を出力しない。そのため「その `PartialEq` が宣言した accessor だけを使って比較しているか（真に identity-based か）」は静的に検証不可能である。`#[derive(PartialEq)]`（全フィールド比較）でも linter は通る——accessor 必須 + equality 宣言存在の二段による近似として受け入れる（D5）。
- **signature lint の限界（D4 / D5）**: D4 の SelfMethod signature チェックおよび D5 の getter signature チェックはカタログ上の `MethodDeclaration`（`receiver` / `params` / `returns`）を検査するが、`returns` の型の意味論（例: 型が実際に identity を表すか）までは確認できない。`fn id(&self) -> ()` は signature チェックが弾くが `fn id(&self) -> SomeUnrelatedType` は通る。静的宣言チェックの構造的限界として受け入れる。
- **linter ルールの opt-in 制**: D15 の原則として、採用者が opt-out したルールは検査されない。「ルールを有効にしていないから検出されない」という状況は設計上の意図であり、opt-out を選んだ採用者は該当するパターン規約を自己管理することになる。
- **linter rule 設計の刷新による波及**: linter rule は「検査対象（どのロール / エントリに適用するか）」と「検査内容」を分離する構造になり、`DataRole` / `ContractRole` を横断する対象指定が必要になる。具体的な型設計（2 部構成・discriminant 型の導入等）は実装フェーズで確定するが、その移行は `CatalogueLinterRule` を構築・パースするコード（codec / config adapter）・既存 linter 設定ファイルを参照するテスト・設定ファイルのスキーマに波及する（既存設定との後方互換性は保証しない）。
- **不変性チェックの限界（D9 / D18）**: struct の public field 禁止と `&mut self` 禁止は、型定義上の public mutation surface を減らす surface check の近似にとどまる。所有者が `&mut` で値を保持すれば enum payload を `&mut` pattern match で変更でき、interior mutability（`Cell` / `RefCell` 等）は static に防げない。完全な不変性保証は gen-tests や利用者のレビューに委ねる。
- **Aggregate Boundary lint の限界**: shallow lint（`TypeEntry::methods` の引数型・戻り値型に対する型名チェック）は実装コードの import を直接見ない。検査対象の拡大（TraitEntry / FunctionEntry / body 等）はカスタム拡張例参照。型のモジュールパスを使わずに同名型を別集約のメンバーとして宣言した場合に false negative が起きる可能性がある。method body の呼び出し依存は依然として不可視である。

### Neutral

- `DataRole` は 13 variant から 15 variant に増える（DomainEvent + EventPolicy 追加）。`roles.rs` のテストで `ALL_DATA_ROLES` 配列の更新（EventPolicy を含む）が必要。
- `ContractRole` は 3 variant から 4 variant に増える（Repository 追加）。`roles.rs` のテストで `ALL_CONTRACT_ROLES` 配列の更新が必要。

## 今後の判断ポイント（Reassess When）

- **Saga / Process Manager の要求**: 採用プロジェクトが状態機械を持つ長期プロセスを TDDD で管理したいと要求した場合、`TypeKindV2` の拡張と合わせて設計する。
- **Read Model の要求**: Event Sourcing / CQRS を採用するプロジェクトからの要求があった場合。
- **`consistency` フィールドの必要性**: Saga 設計が固まったタイミングで再評価する。
- **`emits` / `handles` の TraitEntry への拡張**: Application Service がイベントを返すパターンが広まった場合。
- **`emits` / `handles` の method-level 拡張**: 現行は型単位（この型のメソッドが発行 / 処理するイベント）の宣言で名前存在チェックには十分だが、gen-tests で「どのメソッドがどのイベントを発行 / 処理するか」を検証するには method 単位の `emits` / `handles` が必要になる。gen-tests の設計を具体化する段階で method-level への拡張を評価する。
- **Aggregate Boundary lint の深化**: shallow lint（カタログシグネチャ上の型名チェック）を超えて、rustdoc 由来の実コード依存グラフで直接参照を確認する lint に昇格させるかを評価する。
- **Specification predicate の追加**: invariant の検査手段として DDD Specification 型を使う場合、対象型への適用契約（`is_satisfied_by` 等のメソッド・引数・戻り値）込みで設計し、`InvariantPredicate` enum に variant を追加する。
- **EventPolicy の method / decision 契約（handler method・input event・decision output）**: `reacts_to` は反応する Domain Event の型名リスト（surface）を宣言するにとどまる。どのメソッドが入力イベントを受け取りどの型を決定として返すか（handler method / input event / decision output の三つ組み）を宣言・検査する method-level 拡張は、gen-tests の設計を具体化する段階で評価する。
- **invariant enforcement surface**: `invariants` は observable predicate の宣言にとどまり、constructor / factory / mutator が invariant を破らないことの体系的な検査（enforcement surface の宣言・lint）は本 ADR の最小コアには含めない。gen-tests がテストスケルトンを生成することで enforcement の一部をカバーするが、「どこで enforcement が行われるか（constructor / factory / mutator の選択）」をカタログで宣言し lint する仕組みの設計は、gen-tests の具体化と合わせて評価する。

## Related

- **ADR `2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md`**: language / role / layer の 3 軸分離を実施した ADR。D2 で `Entity` / `AggregateRoot` / `ValueObject` 等を `DataRole` の値として収録したが、これは軸整理のための収録であり、各ロールに DDD パターン固有の意味論・検査ルールを与えることは対象外だった。本 ADR はその積み残しを初めて埋める。
- **ADR `2026-04-13-1813-tddd-taxonomy-expansion.md`**: ロールを 5 variant から 12 variant に拡張した先行決定。本 ADR の Layer ② は D3「存在チェックのみ」ポリシーをより厳しい「検査ルール必須」原則に更新する。
- **ADR `2026-04-11-0003-type-action-declarations.md`**: `add` / `modify` / `reference` / `delete` action の定義。本 ADR の新フィールドはすべて同じ action 機構の下で動作する。
- **ADR `2026-04-26-0855-tddd-feature-extension-with-verification.md`**: カタログスキーマ拡張は TypeGraph / baseline スキーマの拡張を伴う不変条件を定義した。本 ADR の新フィールドはすべてこの不変条件に従い、対応する TypeGraph フィールドと baseline フィールドの拡張を伴う。
- **ADR `2026-05-18-1223-make-catalogue-schema-permissive.md`**: 未知フィールドを無視する寛大なスキーマ方針。本 ADR の新フィールドに対しても同方針が適用されるが、後方互換性の保証が目的ではなく、スキーマ進化時のパーサー堅牢性（旧パーサーが新フィールドを無視できること）を確保するためである。
- **研究レポート `knowledge/research/2026-05-25-ddd-clean-pattern-taxonomy-gap.md`**: 本 ADR の Layer ① / ② の追加根拠となった Gemini ギャップ分析。
- **ADR `2026-05-26-1002-typestate-struct-kind-orthogonal.md`**: typestate marker を struct 形状と直交して kind 軸に配置する決定。本 ADR が採用する「意味論は属する軸の enum に結合する」という原則の先行事例。同 ADR は kind 軸（struct グループ）への配置であり、本 ADR は role 軸（`DataRole` variant）への配置であるという対比を §3 で説明している。
