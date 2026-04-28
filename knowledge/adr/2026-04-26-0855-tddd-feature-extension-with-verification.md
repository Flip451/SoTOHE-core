---
adr_id: 2026-04-26-0855-tddd-feature-extension-with-verification
decisions:
  - id: 2026-04-26-0855-tddd-feature-extension-with-verification_grandfathered
    status: accepted
    grandfathered: true
---
# TDDD カタログ機能拡張と照合可能性の不変条件

## Context

### §1 照合不能な状態が表現できてしまう問題

TDDD では、カタログ (`<catalogue_file>`) に宣言された型・宣言内容と、実装コード側 (`TypeGraph`、rustdoc JSON 由来) の実態を突き合わせ、Blue / Yellow / Red の信号を出す。

この照合が成り立つのは、「カタログ側で宣言できる情報」と「TypeGraph 側に存在するデータ」が 1:1 対応している場合だけである。両者の対応が欠けると、照合自体が不可能になる。

カタログ schema を拡張する際に TypeGraph 側の対応するデータ構造の拡張を省いた場合、「カタログで宣言できるが、TypeGraph に該当データがないため照合できない」という状態が生じる。この状態は `.claude/rules/04-coding-principles.md` § Make Illegal States Unrepresentable の原則に反する。型レベルで排除できる不正状態が表現可能になる。

照合不能な状態をランタイムで扱おうとすると、操作の方向 (追加 / 削除) によって信号の扱いを変えるといった非対称な処理が生まれる。こうした特別扱いは不正状態の存在を前提とした設計であり、型レベルの排除という原則と矛盾する。

さらに、TDDD の照合機構には **baseline** (`<catalogue-stem>-baseline.json`) も参加している。baseline は `/track:type-design` 時点で TypeGraph のスナップショットを capture したファイルであり、`modify` action と `delete` action の signal 判定で使われる。具体的には:

- `modify` action では「baseline 当時の signature」と「現在の実装 signature」の diff で変更を検出する
- `delete` action では「baseline に存在したが現在のコードに存在しない」ことで削除の完了を判定する

カタログ schema を拡張した場合に TypeGraph schema だけを同時拡張し、baseline schema の拡張を省くと、「宣言できる・TypeGraph で照合可能・しかし baseline でその情報を capture できない」という状態が生じる。この状態では `modify` や `delete` の diff 比較が正しく成立しない。例えば `expected_members` を新たに宣言できるようにした場合、baseline にその members 情報が capture されていなければ、「baseline 時点の members」と「現在の members」を比較できず、members の変更を `modify` として正しく検出できない。

### §2 孤立ノードの真因

Contract Map (ADR 2026-04-17-1528-tddd-contract-map.md) の初期実装を経たドッグフーディングで、以下の知見が得られた。

- カタログ上に宣言された型が Contract Map 上で孤立する (edge を持たない) 主な原因は、カタログ設計者が「その型を使う側」を宣言していないことである
- 孤立したノードを視覚的に隠す手段 (破線マスキング、`unused_reference` / `declaration_only` といった特別扱いの classDef など) は、真因を隠してしまう。この対策は却下された

「孤立している型があれば、その型を使う宣言を追加する」という運用ルールが正しい対処である。

### §3 TypeGraph と baseline の現状と拡張が必要な宣言の種別

ADR 2026-04-16-2200-tddd-type-graph-view.md が示すとおり、`TypeGraph` は以下を保持する。

```rust
// <!-- illustrative, non-canonical -->
pub struct TypeGraph {
    types: HashMap<String, TypeNode>,
    traits: HashMap<String, TraitNode>,
}

pub struct TypeNode {
    kind: TypeKind,
    members: Vec<MemberDeclaration>,   // struct フィールド / enum バリアント
    methods: Vec<MethodDeclaration>,   // 固有 impl のメソッド
    trait_impls: Vec<TraitImplEntry>,  // impl Trait for Struct
    outgoing: HashSet<String>,         // typestate 遷移先
    module_path: Option<String>,
}
```

現時点の `TraitImplEntry` は trait の short name のみを保持しており、その trait が workspace crate 由来か std / 外部クレート由来かを区別する情報を持たない。D3 の workspace 由来限定版 reverse check (workspace 由来 trait のみを対象とする方式) では `TraitImplEntry` に `origin_crate: String` フィールドを追加し、`build_type_graph` (infrastructure 層) が rustdoc JSON から origin crate 情報を抽出して格納する。この拡張は Core invariant に従い D3 と同じ ADR で決定する。

**baseline schema の現状**: ADR 2026-04-11-0001-baseline-reverse-signals.md が示す設計に基づき、baseline は以下の構造を持つ (`libs/domain/src/tddd/baseline.rs`)。

```rust
// <!-- illustrative, non-canonical -->
pub struct TypeBaseline {
    schema_version: u32,
    captured_at: Timestamp,
    types: HashMap<String, TypeBaselineEntry>,
    traits: HashMap<String, TraitBaselineEntry>,
}

pub struct TypeBaselineEntry {
    kind: TypeKind,
    members: Vec<MemberDeclaration>,  // フィールド / バリアント
    methods: Vec<MethodDeclaration>,  // 固有 impl のメソッド
}

pub struct TraitBaselineEntry {
    methods: Vec<MethodDeclaration>,  // trait が定義するメソッド
}
```

現時点の baseline には以下が存在しない:

- **`functions` マップ** (free function のスナップショット): D4 で TypeGraph に `functions: HashMap<(String, Option<String>), FunctionNode>` を追加するのと同時に、baseline にも `HashMap<String, FunctionBaselineEntry>` が必要。TypeGraph 側の key は in-memory のみであるため tuple `(short_name, module_path)` を使えるが、baseline は `<catalogue-stem>-baseline.json` として JSON に書き出されるため serde JSON が map key として受け付ける文字列を使う必要がある。このため baseline の key は完全修飾名文字列 (`module_path::name`、`module_path` が `None` の場合は `name` 単独) とし、`FunctionBaselineEntry` 内に `module_path: Option<String>` を保持することで照合と diff 比較を成立させる
- **`TypeBaselineEntry::trait_impls`** (Interactor / SecondaryAdapter の trait impl スナップショット): D3 で `TraitImplEntry::origin_crate` を追加するのと同時に、baseline にも同等の `Vec<TraitImplBaselineEntry>` が必要 (modify action での trait impl 残骸評価のため)

本 ADR では各 bundle として上記の不足をそれぞれ解消する。D3 の bundle として `TypeBaselineEntry` に `trait_impls: Vec<TraitImplBaselineEntry>` を追加することで trait impl スナップショット不足が解消され、D4 の bundle として `TypeBaseline` に `functions: HashMap<String, FunctionBaselineEntry>` (key は完全修飾名文字列) を追加することで free function スナップショット不足が解消される (各詳細は D3 / D4 を参照)。

これを前提に、カタログ機能拡張の候補ごとに照合可能性を評価する。

**フィールド宣言 (`expected_members`)**:  
`TypeNode::members` が既存であり、struct のフィールド名と型、enum のバリアント名を照合できる。TypeGraph 拡張は不要。

`MemberDeclaration::Field` の `name` フィールドには、named field の場合は公開フィールド名 (`"id"`, `"email"` 等)、tuple struct の場合は公開フィールドの位置インデックスの文字列 (`"0"`, `"1"`, `"2"`, ...) を使う。rustdoc JSON も tuple struct の公開フィールドを `0`, `1`, `2` というインデックス名で出力するため、TypeGraph 側と 1:1 対応する。TypeGraph に公開メンバーがゼロ個と現れる struct (unit-like struct `struct Marker;`、または全フィールドが private な struct) のみが `expected_members: []` を正当に使えるケースとなる。

**interactor の ApplicationService 実装宣言 (`declares_application_service`)**:  
`TypeNode::trait_impls` の構造は既存であり、`impl ApplicationServiceTrait for Interactor` の照合自体はできる。ただし reverse check を workspace 由来 trait に限定するために `TraitImplEntry` に `origin_crate: String` フィールドを追加する必要があり、この追加が D3 の TypeGraph schema 拡張として必須となる。TypeGraph 拡張は `origin_crate` の追加という形で必要である。

**free function 宣言 (`FreeFunction` kind)**:  
`TypeGraph` は関数の名前空間を持たない。`functions` マップに相当するフィールドが存在しない。この宣言をカタログに追加しても照合できない。カタログ schema 拡張と TypeGraph schema 拡張を同時に行わなければならない。

## Decision

### §S 信号機評価の原則

信号は「**forward miss**（宣言した要素が実装にない）」と「**reverse extra**（宣言にない要素が実装にある）」という 2 つの構造事実と、action（add / modify / delete / reference）の組み合わせで決まる。

| 信号 | 対応する構造事実 (action によって異なる) | 基本的な意味 (action 中立) |
| --- | --- | --- |
| **Blue** | forward miss なし / reverse extra なし (全 action 共通) | 契約の正しい履行 |
| **Yellow** | action によって異なる (下表参照) | 実装中 |
| **Red** | action によって異なる (下表参照) | 契約違反 |

**action 別の信号適用**:

| action | 完全一致 | forward miss あり | reverse extra あり |
| --- | --- | --- | --- |
| **add** (新規追加) | Blue | Yellow — 実装中の通常状態 | Red — catalogue 宣言外の契約違反 |
| **modify** (既存変更中) | Blue | Yellow — 変更途中の通常状態 | Yellow — 変更途中の残骸として吸収 |
| **delete** (削除中) | Blue (実装なし) | — (照合軸なし) | Yellow — まだ実装が残っている |
| **reference** (既存参照) | Blue | Red — 契約違反 (発生タイミングで原因の見当がつく) | Red — 契約違反 (発生タイミングで原因の見当がつく) |

**各 action の説明**:

- **add**: 「これから実装する全契約を宣言する」という action。実装は宣言の範囲に収まるべきであり、宣言の外側に実装が出た場合は catalogue 宣言外として Red。
- **modify**: 「宣言通りに既存型を修正する途中」という action。変更が完全に終わるまで forward miss も reverse extra も WIP の一部として Yellow に吸収される。変更途中の残骸（まだ実装が宣言より多い状態）は Red ではなく Yellow として扱う。**baseline との関係**: `modify` action の signal 判定は「baseline に記録された設計時点の signature」と「現在の実装 signature」の diff に基づく。forward miss（宣言した変更がコードに反映されていない）と reverse extra（変更前の実装がまだ残っている）の両方が Yellow に吸収されるのは、この diff 比較が成立しているという前提のもとで機能する。
- **delete**: 「削除予定の型」という action。実装なし → Blue（削除完了）、実装あり → Yellow（まだ残っている）。**baseline との関係**: `delete` action の Blue 判定は「baseline に存在した型が現在のコードに存在しない」ことで確認する。baseline に当該型がなければ宣言の誤りとして扱う。
- **reference**: 「既存の完成した型を参照するだけで変更しない」という action。完成を前提とするため WIP（Yellow）は出ない。不一致はすべて Red となるが、原因は 2 つあり得る — (a) catalogue が誤った型名やシグネチャを宣言している、または (b) 実装者が誤って既存型を書き換えた。Red が出たタイミングでおおよその見当がつく — カタログ作成直後 (実装にまだ手をつけていない段階) であれば catalogue 側の誤り (型名の typo やシグネチャの間違い等) が原因である可能性が高く、実装作業中 (catalogue は固定済みでコードを書いている段階) であれば実装者が既存型を誤って書き換えた可能性が高い。

function signature 照合（D4）では forward miss と reverse extra の扱いに非対称性がある。forward miss は「同一 `module_path` に同名関数がない」と「同 `(name, module_path)` でも signature (引数の型と個数 / 戻り値の型 / async 有無) が一致しない」の両方を含む。reverse extra は「カタログに何らかの FreeFunction 宣言がある `module_path` を対象として、その `module_path` 内に宣言された名前の集合に含まれない名前の関数が実装にある」ことを指す (reverse-check のスコープの詳細は D4 を参照)。この定義により、カタログ上で宣言がない `module_path` のモジュールに存在する関数は reverse-check の対象外となり、別モジュールの同名関数が誤検出されない。この非対称性は `evaluate_trait_methods` の forward / reverse check semantics と一致する。各構造事実に対応する信号は §S の action 別 mapping に従う。

### Core invariant: カタログ schema 拡張 / TypeGraph schema 拡張 / baseline schema 拡張は同時に決め、同時に実装する

カタログ schema で宣言可能な宣言の種別 / フィールドは、TypeGraph schema で照合可能なデータ型 / 情報と 1:1 対応する。この対応が欠けた状態は、「宣言できるが照合できない」という不正状態が表現できる状態であり、Make Illegal States Unrepresentable の原則に反する。

さらに、TypeGraph schema で照合可能なデータは、baseline schema でも capture できる必要がある。baseline は `modify` action と `delete` action の signal 判定で「設計時点のスナップショット」として参照されるため、TypeGraph に存在するが baseline に capture されていない情報が生じると、「照合可能だが baseline との diff 比較が成立しない」という不正状態になる。

schema を拡張するときは、**カタログ側・TypeGraph 側・baseline 側の 3 者を同じ ADR で決め、同じ実装作業の中で完了させる**。どれか 1 つだけ先に進める「段階分離」は採らない。

この原則の具体例として: D4 は `FreeFunction` 宣言 (カタログ側 `module_path` / `expected_params` / `expected_returns` / `expected_is_async`) / TypeGraph `functions` マップ (key: in-memory の tuple `(String, Option<String>)` = `(short_name, module_path)`, value: `FunctionNode { params, returns, is_async, module_path }`) / baseline `functions` マップ (key: JSON serializable な完全修飾名文字列 `module_path::name`、value: `FunctionBaselineEntry { params, returns, is_async, module_path }`) の 3 者を bundle で決定する。TypeGraph の in-memory tuple key と baseline の JSON 文字列 key が異なるのは、serde JSON が map key として文字列しか受け付けないためであり、`FunctionBaselineEntry::module_path` フィールドによって key から完全な照合情報を復元できる。D2 は `expected_members` 宣言 / TypeGraph `TypeNode::members` (既存) / baseline `TypeBaselineEntry::members` (既存) の 3 者が揃っていることを確認したうえで決定する。**D3 は `declares_application_service` 宣言 / TypeGraph `TypeNode::trait_impls` + `TraitImplEntry::origin_crate` / baseline `TypeBaselineEntry::trait_impls: Vec<TraitImplBaselineEntry>` の 3 者を bundle で決定する**。D3 での精度限界 (modify action での trait impl 残骸評価) は `TypeBaselineEntry::trait_impls` の追加によって解消し、`TraitImplEntry::origin_crate` を用いた workspace crate 判別が D3 の workspace 由来限定版 reverse check の前提となる。

**なぜ段階分離を採らないか**: 段階分離の期間中は「宣言できるが照合できない」または「照合可能だが baseline で capture できない」という不正状態が存在する。これに対処するためにランタイムで特別扱いを入れると、不正状態を型レベルではなくランタイムで誤魔化すことになる。こうした誤魔化しは複数の場所に広がりやすく、後から取り除くコストが高い。

**内部宣言 field の省略禁止 (invariant の追加)**: TypeGraph 側に照合対象データが存在するにもかかわらず、カタログ側の内部宣言 field を省略可能にすると、「照合可能なのに照合を放棄する」状態が schema レベルで許容されてしまう。これは「宣言が契約になる」という TDDD の原則と矛盾する。type-designer が内部宣言 field を省略すれば、コードがどれだけ変化しても信号が動かない (silent stale)。

このため、TypeGraph に対応する照合軸を持つ内部宣言 field はすべて必須フィールドとする。省略可能 (`Option` 型) にしない。空の `Vec` を明示的に書くことで「実装側もメンバーがゼロ個 (marker struct / marker trait 等)」を宣言できるが、省略という抜け穴は残さない。

**tuple struct のフィールド宣言**: `MemberDeclaration::Field` の `name` として、tuple struct の公開フィールドは位置インデックスの文字列 `"0"` / `"1"` / `"2"` ... を使う (rustdoc JSON の出力規則と一致)。named field を持つ struct は公開フィールド名をそのまま使う。空の `Vec` が許容されるのは TypeGraph に公開メンバーがゼロ個と現れる構造 (unit-like struct `struct Marker;`、または全フィールドが private な tuple struct / named field struct) のみ。tuple struct に公開フィールドがある場合 (例: `struct Email(pub String)`) は位置インデックスで宣言する。**根拠**: `extract_struct_fields` (infrastructure 層 `schema_export.rs`) は `StructKind::Plain` では `Visibility::Public` でフィルタし、`StructKind::Tuple` では private field が `None` として表現されることを利用して公開フィールドのみを抽出する。`StructKind::Unit` は空 Vec を返す。全フィールドが private な tuple struct は TypeGraph 上でメンバーがゼロ個として現れる。

**「空の Vec」の扱い**: kind の意味論によって異なる。

- **禁止** (空の Vec を許容しない): `Enum` / `ErrorType` の `expected_variants`、および `Interactor` の `declares_application_service`。バリアントがゼロの enum は型として意味をなさず、空の `expected_variants` は「宣言不足を意図的に選んだ」とはみなせない。同様に、`Interactor` は hexagonal の usecase 実装であり `ApplicationService` trait を impl することが本義である。trait impl なしの Interactor は Interactor ではなく別の kind (Factory / Dto 等) として扱うべき設計上の誤りであるため、空の `declares_application_service` を許容しない。
- **許容** (空の Vec を明示的意思として受け入れる): 残り 11 の kind。ただし「許容」は「照合を省略してよい」という意味ではない。空の Vec は「実装側もメンバー / メソッドがゼロ個であることを宣言する」という意味に統一する。forward check (宣言 → 実装) と reverse check (実装 → 宣言) を両方適用し、実装側に 1 件でも要素があれば reverse extra が発生する (信号化は §S の action 別 mapping に従う)。marker struct (フィールドをゼロ個持つ struct)、marker trait (メソッドをゼロ個持つ trait)、typestate の terminal 状態型など、ゼロ個であることが正当な設計上の選択である場合にのみ空の Vec を使う。

### D1: 孤立型への対処は「使う側の宣言を追加する」運用ルールとする

Contract Map 上で孤立している型があれば、その型を使う側 (引数・戻り値・フィールドで参照する型) をカタログに宣言することで edge を発生させる。孤立自体を隠す仕組み (破線マスキングなど) は導入しない。

この運用ルールは、カタログが「設計者が契約として宣言した型の集合」であるという性質と整合する。孤立している型は「まだ使う側が宣言されていない」という設計上の情報であり、隠す必要はない。

### D2: フィールド宣言 (`expected_members`) を struct ベースの全 9 種に追加し、必須化する

`Typestate` / `ValueObject` / `UseCase` / `Interactor` / `Dto` / `Command` / `Query` / `Factory` / `SecondaryAdapter` の 9 kind に `expected_members` フィールドを追加する。このフィールドは省略不可 (必須) とし、空の `Vec` で「実装側もメンバーがゼロ個 (marker struct 等)」を宣言する。

**対象外**: `Enum` / `ErrorType` は既存の `expected_variants` が同等の役割を担う (D2-2 で別途扱う)。`SecondaryPort` / `ApplicationService` は `expected_methods` が同等の役割を担う (D2-2 で別途扱う)。

**照合の根拠**: TypeGraph の `TypeNode::members` が既存であり、struct のフィールド名・型、enum のバリアント名を照合できる。TypeGraph 拡張は不要。

**baseline schema との関係**: `TypeBaselineEntry` は既存の `members: Vec<MemberDeclaration>` フィールドを持つ。`expected_members` の照合に使う `MemberDeclaration` (フィールド名 + 型文字列) は、baseline がすでに capture しているデータ構造と 1:1 対応しており、baseline schema の新規拡張は不要である。`modify` action での members 変更は「baseline の members」と「現在のコードの members」を diff することで、変更途中の残骸 (reverse extra) が Yellow として正しく評価される。

**信号の評価**: forward miss / reverse extra と action の組み合わせによる Blue / Yellow / Red の決定は §S に従う。`expected_members` が空の Vec の場合、forward check は自動通過し reverse check のみが実質的に機能する — コードにメンバーが 1 個でもあれば reverse extra が発生し (信号化は §S に従う)、コードも 0 個なら完全一致 (§S で Blue)。

**各 kind ごとの運用上の期待**:

tuple struct の公開フィールドはすべて位置インデックス (`"0"`, `"1"`, ...) を name として宣言する。named field を持つ struct は公開フィールド名をそのまま使う。空の Vec を使えるのは TypeGraph に公開メンバーがゼロ個と現れる struct のみ (unit-like struct はその典型例。全フィールドが private な struct も含む)。

- `ValueObject`: newtype / tuple struct が多く公開フィールドを持つのが通常。例: `struct Email(pub String)` → `[{name: "0", ty: "String"}]`、`struct Range(pub usize, pub usize)` → `[{name: "0", ty: "usize"}, {name: "1", ty: "usize"}]`。named field がある場合は公開フィールド名で宣言する。空の Vec は TypeGraph に公開メンバーがゼロ個の struct のみ。
- `Dto` / `Command` / `Query`: データ転送が目的の struct であり、公開フィールドを持つのが通常の設計。named field の場合はフィールド名で、tuple struct の場合はインデックス名 (`"0"`, `"1"`, ...) で宣言する。空の Vec は公開フィールドが真にゼロ個の構造 (marker command 等) のみ。
- `Typestate`: 状態型であり、フィールドを持つ場合もゼロ個の場合 (marker typestate) もある。named field はフィールド名で、tuple struct のフィールドはインデックス名で宣言する。空の Vec = 実装側も field がゼロ個 (marker typestate) の宣言。
- `UseCase` / `Interactor` / `Factory`: 依存性注入を constructor で受け取る struct が多く、public field をほぼ持たない。public field がある場合は named field ならフィールド名、tuple struct ならインデックス名で宣言する。public field がない場合は空の Vec を書く。
- `SecondaryAdapter`: 既存の `implements` フィールドで trait 実装を宣言している。`expected_members` は struct の public field (例: DB pool を公開するアクセサ) を照合するために使う。named field はフィールド名で、tuple struct のフィールドはインデックス名で宣言する。public field がない場合は空の Vec を書く。

**`SecondaryAdapter` の既存 `implements` との関係**: `implements` フィールドはそのまま維持する。`expected_members` は struct field の照合軸として独立して機能する。

### D2-2: enum ベース / trait ベース kind の内部宣言 field を必須化し、`Enum` / `ErrorType` では空の Vec を禁止する

`Enum` / `ErrorType` の `expected_variants` と、`SecondaryPort` / `ApplicationService` の `expected_methods` は既存フィールドであるが、空の Vec を許容している。D2 と同じ invariant により、これらも省略不可とする。加えて `Enum` / `ErrorType` については空の Vec も禁止する。

**`Enum` / `ErrorType` の空 Vec 禁止の理由**: バリアントがゼロ個の enum は Rust として有効なコードではあるが、カタログに宣言する型として実用上意味をなさない。空の `expected_variants` は「宣言不足を意図的に選んだ」とはみなせず、単なる宣言漏れと区別できない。schema レベルで空の Vec を禁止することで、type-designer は少なくとも 1 件以上のバリアントを宣言する義務を負う。

**`SecondaryPort` / `ApplicationService` の空 Vec 許容**: メソッドがゼロ個の trait は Rust では marker trait として有効な設計である (例: ある性質を持つことを示すだけで、メソッドを要求しない)。空の `expected_methods` = 「実装側も trait メソッドがゼロ個 (marker trait)」を宣言する意味に統一する。forward check + reverse check を両方適用し、コードの trait にメソッドが 1 個でもあれば reverse extra が発生する (信号化は §S に従う)。禁止しないが省略は不可。

**`SecondaryPort` / `ApplicationService` の信号の評価**: forward miss / reverse extra と action の組み合わせによる Blue / Yellow / Red の決定は §S に従う。ただし forward check 失敗の定義はこの照合軸特有で、メソッドがコードにない場合だけでなく、同名でも signature (receiver / 引数の型と個数 / 戻り値の型 / async 有無のいずれか) が一致しない場合も forward miss として扱う。`expected_methods` が空の Vec の場合、forward check は自動通過し reverse check のみが実質的に機能する — コードの trait にメソッドが 1 個でもあれば reverse extra が発生し (信号化は §S に従う)、コードも 0 個 (marker trait) なら完全一致 (§S で Blue)。

**`expected_variants` と `expected_methods` の既存挙動との整合**: `evaluate_trait_methods` (signals.rs) は既に forward check + reverse check の両方を走らせる実装であり、空 Vec の場合も reverse check でコード側の余分なメソッドを extra として検出する。この挙動は本 ADR の「空 Vec = 実装側も 0 個」semantics と §S 原則に一致している。`Enum` / `ErrorType` については「空の Vec は schema validation で拒否する」制約を追加する。

### D3: interactor の ApplicationService 実装宣言 (`declares_application_service`) を `Interactor` に追加する

カタログの `Interactor` 宣言に `declares_application_service: Vec<String>` フィールドを追加し、どの `ApplicationService` trait を実装しているかを宣言できるようにする。このフィールドは省略不可 (必須) かつ空の Vec 禁止とする。

**型の設計**:

- `Vec<String>`: 1 件以上の trait 名を必須で列挙する。1 つの Interactor が複数の `ApplicationService` trait を impl するケースも表現できる
- 省略不可 (フィールドが必ず存在する): Core invariant の「内部宣言 field の省略禁止」原則に従う
- 空の Vec 禁止: `Interactor` の本義は `ApplicationService` trait の impl であり、trait impl なしの Interactor は設計上の誤り (kind を Factory / Dto 等に変えるべき)。schema レベルで空の Vec を拒否することで type-designer はいずれかの trait 名を明示する義務を負う

**照合の根拠**: TypeGraph の `TypeNode::trait_impls` が既存であり、`impl TraitName for Struct` の実態が照合できる。ただし reverse check の対象を workspace 由来 trait に絞り込むために、`TraitImplEntry` に `origin_crate: String` フィールドを追加する (下記 TypeGraph schema 拡張を参照)。

**baseline schema との関係 (D3 bundle)**: `TypeBaselineEntry` に `trait_impls: Vec<TraitImplBaselineEntry>` フィールドを追加する (Core invariant の 3 輪整合を維持するための D3 bundle 決定)。`declares_application_service` の `modify` action (既存 Interactor が実装する ApplicationService trait を変更する場合) では、「baseline 時点に impl されていた trait のリスト」と「現在 impl されているリストの diff」が必要になる。この diff を成立させるために `TypeBaselineEntry::trait_impls` が必要である。baseline builder (infrastructure 層) が `TypeNode::trait_impls` を `TraitImplBaselineEntry` の Vec に変換して格納する責務を持つ。

`TraitImplBaselineEntry` の構造:

```rust
// <!-- illustrative, non-canonical -->
pub struct TraitImplBaselineEntry {
    pub trait_name: String,    // trait の short name (TraitImplEntry::trait_name と対称)
    pub origin_crate: String,  // trait が定義された crate 名 (TraitImplEntry::origin_crate と対称)
}
```

この追加により、`modify` action での reverse extra (変更前の trait impl が baseline には存在するが宣言内容が更新されたことで「残骸」となっている状態) も baseline diff 経由で正しく Yellow として評価できる。D3 の精度限界は `TypeBaselineEntry::trait_impls` の追加によって完全に解消される。また `origin_crate` を baseline 側でも保持することで、workspace crate 判別が baseline diff を通じても成立する。

**TypeGraph schema 拡張 (Core invariant に従い D3 bundle で決定)**:

reverse check を workspace 由来 trait に限定するために必要なため、`TraitImplEntry` に `origin_crate` フィールドを追加する。

```rust
// <!-- illustrative, non-canonical -->
pub struct TraitImplEntry {
    pub trait_name: String,       // 既存: trait の short name
    pub origin_crate: String,     // 追加: trait が定義された crate 名
}
```

`build_type_graph` (infrastructure 層) が rustdoc JSON を解析する際に、trait の origin crate 情報を `TraitImplEntry::origin_crate` に格納する責務を持つ。rustdoc JSON の `Crate.paths` / `external_crates` から trait の定義 crate を特定する。workspace crate の集合は `architecture-rules.json` の `crates` 一覧または `Cargo.toml` の workspace メンバーから取得する。

**信号の評価**: forward miss / reverse extra と action の組み合わせによる Blue / Yellow / Red の決定は §S に従う。

**reverse check の範囲**: Interactor はコード上で `Debug` / `Clone` など ApplicationService 以外の trait も impl する。reverse check は「`TypeNode::trait_impls` に現れる各 trait のうち、`TraitImplEntry::origin_crate` が workspace crate のいずれかに一致するもの全て」を対象とする。std / 外部クレート由来の trait は `origin_crate` の判別で自動的にスコープ外となるため、derive macro 由来の周辺 trait が誤検出されない。workspace 由来の trait が `declares_application_service` の Vec に含まれていない場合は reverse extra として §S の action 別 mapping に従い信号を出す。

**注意 (infrastructure 内部 trait の扱い)**: `origin_crate` が workspace crate に一致する trait の中には、ADR `2026-04-15-1636-tddd-05-secondary-adapter.md` D5 で「infrastructure 内部の trait」として意図的にカタログ対象外とした trait (`GitRepository`, `GhClient` 等) が含まれる。これらは port の所有者と adapter の所有者が同一 infrastructure crate に属するため、hexagonal secondary port とは意味論が異なる。現時点の D3 では workspace 由来 trait を一律に reverse check 対象とするため、カタログに未記載の infrastructure 内部 trait が reverse extra として検出される可能性がある。この false positive については Reassess When を参照。将来的には `origin_crate` に加えて「trait の所属 crate が同一の infrastructure crate か否か」の判別を加え、infrastructure 内部 trait を reverse check から除外する仕組みを検討する。

**SecondaryAdapter の reverse check も同等に workspace 由来限定化**: `SecondaryAdapter` の `implements` に対する reverse check においても、同じ方針を適用する。`TypeNode::trait_impls` の各 trait について `TraitImplEntry::origin_crate` が workspace crate に一致するものを対象とし、`implements` Vec に含まれていない workspace 由来 trait があれば reverse extra として扱う。std / 外部クレート由来の trait は判別で自動除外する。これにより `Interactor` と `SecondaryAdapter` の両方で、workspace 由来 trait に関する宣言漏れを reverse check が直接検出できる。上記の infrastructure 内部 trait の false positive 問題は SecondaryAdapter にも同様に存在する。

Contract Map (ADR 2026-04-17-1528) の Known Limitations §L3 に記録された「interactor → application_service の trait-impl edge が描画されない」問題の照合基盤として機能する。Contract Map の edge 描画ロジックと整合させること。

### D4: free function 宣言 (`FreeFunction` kind) はカタログ schema 拡張 / TypeGraph schema 拡張 / baseline schema 拡張を同時に決める

`TypeDefinitionKind` に `FreeFunction` variant を追加し、モジュール公開の free function をカタログに宣言できるようにする。同時に、TypeGraph に関数の名前空間 (`functions` マップ) を追加し、照合を可能にする。さらに、baseline にも同等の `functions` スナップショットを追加し、`modify` / `delete` action の diff 比較を成立させる。

カタログ schema・TypeGraph schema・baseline schema の 3 者が揃って初めて宣言と照合と baseline 比較が成立する。いずれか 1 つだけ先に進めることはしない。

**カタログ側の宣言形式**:

```rust
// <!-- illustrative, non-canonical -->
FreeFunction {
    module_path: Option<String>,            // 宣言する関数が属するモジュールパス
    expected_params: Vec<ParamDeclaration>, // 引数の型リスト
    expected_returns: String,               // 戻り値の型文字列
    expected_is_async: bool,                // async fn かどうか
}
```

`module_path` が必要な理由と `None` 時の semantics:

- **なぜ必要か**: 照合の軸は `(name, module_path)` の tuple であり (`functions` map の key 設計を参照)、type-designer が「どのモジュールの関数を宣言しているか」を表現できなければ同名関数が複数モジュールに存在する場合に一意の関数を特定できない。`FunctionNode::module_path` / `FunctionBaselineEntry::module_path` が `Option<String>` であるのと同型のフィールドをカタログ側にも置くことで、catalog / TypeGraph / baseline の 3 者で一貫した照合軸が成立する。
- **`None` の semantics**: カタログ側で `module_path: None` を宣言した場合、TypeGraph 上で `module_path: None` の関数 (rustdoc JSON からモジュールパスを取得できなかった関数、典型的には crate root 直下の関数) と照合する。`Some(...)` を宣言した場合はそのパスが完全一致する関数のみが照合対象となる。`module_path: None` の宣言は「モジュールパスを問わずどの関数とも一致する」という意味ではなく、「TypeGraph 上でも `module_path: None` として記録されている関数」と照合することを宣言する。これにより、型宣言と同様に crate root 直下の関数を正確に特定できる。

**TypeGraph 側の拡張形式**:

```rust
// <!-- illustrative, non-canonical -->
pub struct TypeGraph {
    types: HashMap<String, TypeNode>,
    traits: HashMap<String, TraitNode>,
    functions: HashMap<(String, Option<String>), FunctionNode>,  // 追加: key は (short_name, module_path)
}

pub struct FunctionNode {
    params: Vec<ParamDeclaration>,
    returns: String,
    is_async: bool,
    module_path: Option<String>,  // 追加: "domain::review_v2" など。TypeNode::module_path と同じ pattern
}
```

**baseline 側の拡張形式**:

```rust
// <!-- illustrative, non-canonical -->
pub struct TypeBaseline {
    // ... 既存フィールド ...
    functions: HashMap<String, FunctionBaselineEntry>,  // 追加: key は完全修飾名文字列 (module_path::name / name)
}

pub struct FunctionBaselineEntry {
    params: Vec<ParamDeclaration>,
    returns: String,
    is_async: bool,
    module_path: Option<String>,  // FunctionNode::module_path と対称。key の復元にも使う
}
```

**TypeGraph と baseline の key 方式の違い**: TypeGraph の `functions` map は in-memory のみで使用されるため、tuple key `(String, Option<String>)` = `(short_name, module_path)` を使える。一方 baseline は `<catalogue-stem>-baseline.json` として JSON に書き出されるため、serde JSON が map key として文字列しか受け付けないという制約がある。このため baseline の key を完全修飾名文字列 (`module_path + "::" + name`、`module_path` が `None` の場合は `name` 単独) とする。`FunctionBaselineEntry` 内に `module_path: Option<String>` を保持することで、key を分解せずとも照合と diff 比較が成立する。

baseline capture 時、baseline builder は `TypeGraph::functions` の各 `(short_name, module_path)` エントリを `FunctionBaselineEntry` に変換し、key を完全修飾名文字列に変換して `TypeBaseline::functions` に格納する。`modify` action (free function の signature を変更中) では「baseline の signature」と「現在の signature」の diff で変更途中の残骸を Yellow として正しく評価するために、baseline の `functions` スナップショットが必要である。

`build_type_graph` (infrastructure 層) が rustdoc JSON の free function 情報を `FunctionNode` に変換し、`TypeGraph::functions` に格納する責任を持つ。`FunctionNode::module_path` は rustdoc JSON の `Crate.paths` から関数が属するモジュールパスを抽出して格納する (`TypeNode::module_path` と同じ抽出経路)。baseline builder (infrastructure 層) が `FunctionNode` を `FunctionBaselineEntry` に変換し、`TypeBaseline::functions` に格納する責任を持つ。

**`TypeGraph::functions` の key 設計 (in-memory)**: key は `(short_name, module_path)` の tuple `(String, Option<String>)` とする。`TypeGraph::types` / `traits` の key が last-segment short name (`String`) である (ADR `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` §D2) のに対し、free function は snake_case 慣習上 `new`, `build`, `parse` などの同名が複数モジュールに存在しやすいため、short name 単独では HashMap 挿入時点で上書き衝突が発生し `module_path` による後段の識別が成立しない。tuple key とすることで、同じ short name を持つ関数であっても `module_path` が異なれば別エントリとして格納される。`module_path: None` はモジュールパスを取得できない関数 (crate root 直下など) を表し、`Some("domain::review_v2")` のように取得できた場合はそのパスを格納する (`TypeNode::module_path` と同じ pattern)。照合の軸は `(name, module_path)` の tuple であり、カタログで宣言した FreeFunction のエントリ名と `module_path` の組み合わせで唯一の関数を特定する。baseline 側は JSON serialization の制約から別途文字列 key を使う (上記「TypeGraph と baseline の key 方式の違い」を参照)。

**信号の評価**: forward miss / reverse extra と action の組み合わせによる Blue / Yellow / Red の決定は §S に従う。この照合軸固有の定義:

- **forward miss**: 宣言した signature と完全一致 (関数名 + `module_path` + 引数の型と個数 + 戻り値の型 + `expected_is_async` の async 有無がすべて一致) する実装が存在しない場合。関数がない場合だけでなく、同名かつ同 `module_path` でも signature が一致しない場合も forward miss として扱う
- **reverse extra**: カタログに何らかの FreeFunction 宣言がある `module_path` を対象として、その `module_path` 内に宣言された名前の集合に含まれない名前の関数が実装にある場合。カタログで宣言された FreeFunction が存在しない `module_path` は reverse-check の対象外であり、その `module_path` に存在する同名関数は reverse extra として誤検出されない

**D3 との線引き**: D4 の reverse extra は「カタログに宣言した FreeFunction 集合と同じ `module_path` 内に、宣言にない名前の関数が存在する」ケースを検出する。D3 は `Interactor` / `SecondaryAdapter` の trait_impls で workspace 由来の trait がカタログに宣言されているかを確認するものであり、対象が異なる。FreeFunction の reverse extra と D3 は重複しない。

**Contract Map との統合**: `FreeFunction` kind のノードは Contract Map (ADR 2026-04-17-1528) 上で `expected_params` の型と `expected_returns` の型への edge を描画できる。`TypeGraph::functions` の存在により、照合と edge 描画の両方が成立する。

**ADR 2026-04-17-1528 Known Limitations §L4 との関係**: 「free function が返すエラー型への edge がない」という問題は、本 ADR の `FreeFunction` kind と TypeGraph 拡張により、`expected_returns` に該当エラー型を記述することで edge が発生するようになる。§L2 の `ContractMapRenderOptions` 相当の型 (free function の引数型) も同様に `expected_params` での宣言で対応できる。

## Rejected Alternatives

### A1: カタログ schema を先に拡張し、TypeGraph schema の拡張は後の段階とする

**却下理由**: §1 で述べた通り、この段階分離は「宣言できるが照合できない」という不正状態を生む。この状態に対処するため「照合不能なら操作の方向で処理を分ける」という特別扱いを入れると、Make Illegal States Unrepresentable の原則に違反する。コードの複雑さが増え、後から取り除くコストが高い。

### A2: 照合不能な場合、操作の方向 (追加 / 削除) で信号の扱いを変える

**却下理由**: 照合不能という不正状態をランタイムで誤魔化す設計である。追加方向では合格扱い、削除方向では止めるという非対称な処理は、「宣言できるが照合できない」という状態が存在することを前提にしている。型レベルで不正状態を排除する設計原則と矛盾する。

### A3: 孤立ノードを破線マスキングや特別扱いの classDef で隠す

**却下理由**: §2 で述べた通り、孤立の真因は「使う側が宣言されていない」ことである。隠す仕組みは真因を覆い隠し、設計者が状況を把握する機会を奪う。前の調査でこの対策が却下されており、本 ADR もその判断を引き継ぐ。

### A4: `Interactor` の ApplicationService 実装確認を catalog-consistency check に委ねて D3 を省略する

**却下理由**: 宣言がなければ照合対象にもならない。カタログに `declares_application_service` を宣言することで、設計者の意図 (「この interactor はこの ApplicationService を実装する」) が明示され、信号評価の対象になる。宣言なしに「なんとなく整合している」ことを期待する設計は、TDDD の「宣言が契約になる」という原則に反する。

### A5: `FreeFunction` の TypeGraph 拡張を省略し、名前の存在チェックのみにとどめる

**却下理由**: 名前の存在チェックだけでは引数や戻り値の型契約を検証できない。TDDD の信号評価は「宣言した契約がコードに実現されているか」を確認するものであり、パラメータ照合を省くと宣言の価値が大幅に下がる。また、TypeGraph への関数 namespace 追加を省略すると「宣言した関数の引数型・戻り値型への edge」が Contract Map に発生しなくなり、可視化の価値が失われる。

### A7: 空の Vec は「存在チェックのみ」の意味として扱い、メンバー照合を省略可能にする

**却下理由**: ユーザーフィードバックによる明示的却下。`expected_members: []` を宣言した場合、実装側がフィールドを 1 個以上持っていても照合がパスするという silent stale を許容する設計となる。これは「宣言が契約になる」という TDDD の原則と矛盾する。「marker struct を宣言するつもり」と「実装と宣言の乖離 (宣言忘れ)」が信号で区別できなくなり、信号の意味が損なわれる。同じ問題構造が `SecondaryPort` / `ApplicationService` の `expected_methods` でも生じる。空の Vec は「メンバー / メソッドが実装側でも真にゼロ個」という宣言として統一することで、宣言漏れは reverse check が検出し、marker 用途の正当性はコード側の実態で担保される。

### A8: `declares_application_service` を `Option<String>` のままにし、省略時は存在チェックのみとする

**却下理由**: 省略可能な `Option<String>` では、type-designer が省略した時点でコードがどれだけ変化しても信号が存在チェックのままになる (silent stale)。A6 と同じ「運用ルールに頼る」構造であり、schema 制約として機能しない。また単一 `String` では 1 つの Interactor が複数の `ApplicationService` trait を impl するケースが表現できない。Interactor は `ApplicationService` trait の impl が本義であるため、trait 名を省略できる設計は Interactor の意味論と矛盾する。`Vec<String>` 必須化によって型レベルで制約を実現できるため、`Option` 維持の理由はない。

### A6: 内部宣言 field (expected_members / expected_variants / expected_methods) を省略可能のままにし、運用ルールで「必ず書くこと」と定める

**却下理由**: 運用ルールは schema 制約の代替にならない。「省略可能」な field を「必ず書く」という合意は、検証機構のない状態では誰かが省略した時点で信号評価が静かに劣化する。省略された field がある場合、コードがどれだけ変化しても信号は存在チェックのままになり、type-designer の意図しない silent stale が生まれる。この状態は、前 track で却下された「破線マスキング」と同じ構造、すなわち「真因 (宣言不足) を許容する設計」である。Core invariant (「宣言できるが照合できない」状態の排除) は schema 制約として実現する必要がある。

### A9: D3 / SecondaryAdapter の reverse check 範囲を実装全体に広げ、ヒューリスティックで非 hexagonal trait を除外する

reverse check の範囲を「カタログで宣言された trait」に限定するのではなく「Interactor / SecondaryAdapter が impl している全 trait」に広げ、`Debug` / `Clone` / `PartialEq` など標準・外部 trait を除外リストで除く方式。

**却下理由**:

- 除外リストは運用負荷が高く、新しいサードパーティ trait が増えるたびに更新が必要になる
- 「どの trait が TDDD scope 外か」の判別基準が曖昧になる。workspace crate の trait でも hexagonal でないもの (例: 内部ユーティリティ trait) を除外するかの判断が都度必要になる
- 除外リストに漏れがあれば誤検出 (false positive) が発生し、信号の信頼性が下がる
- 正しく運用するには事実上「TDDD scope 内の hexagonal trait の集合」を別途管理することになる。これは除外リストを維持しながら workspace crate の trait 集合を動的に列挙するという形で、D3 が `TraitImplEntry::origin_crate` で直接判別している情報を別角度から再構築することになり、二重管理になる
- D3 は `TraitImplEntry::origin_crate` を用いた workspace crate 判別を採用することで除外リストを不要にし、std / 外部クレート trait を自動除外する

### A11: 信号の 3 段階を「一致 / 不一致」の 2 段階として定義し、Yellow を省略する

**却下理由**: 「宣言したが未実装」という状態は宣言駆動開発の通常の途中状態である。この状態を Red として扱うと、type-designer がカタログを先行して宣言した直後から Red が出続け、実装が追いつくまで信号が赤のままになる。これは「宣言が契約になる」という TDDD の原則を維持しながら段階的に実装を進める開発スタイルと相容れない。Yellow は「宣言が先行した正当な未実装」を表す必要な信号であり、省略すると「宣言と実装の同時更新」を強制する設計になる。

### A13: action を問わず 3 段階原則を一律に適用し、modify / reference の特別扱いをしない

全 action で同じルールを使う方式。modify でも reverse extra = Red、reference でも forward miss = Yellow とする。

**却下理由**:

- **modify**: 変更中の型は宣言と実装が一時的にずれる期間が必ず生じる。この期間中に reverse extra を Red として扱うと、「変更前の実装がまだ残っている」という通常の WIP 状態が契約違反と誤報される。type-designer は信号を無視するか、宣言を変更のたびに 2 段階に分けて更新するかを強いられる。これは開発の流れを不必要に複雑にする。
- **reference**: 既存の完成した型を参照するだけの宣言では「まだ実装中」という状態は成立しない。forward miss = Yellow を許容すると、catalogue が実装に存在しない型名を誤って宣言していても WIP として黙認される。reference は「すでに存在する型との契約」であり、不一致はすべて catalogue の誤りとして Red で検出するのが正しい。

action 中立の一律ルールは、action の意味論 (「新規追加中」「変更中」「参照のみ」) を信号に反映できない。§S で action 別に semantics を明示することで、一律ルールよりも信号の意味が明確になる。

### A12: Red を「宣言と実装の単純な不一致」として定義し、「宣言に含まれない要素が実装にある」という方向性を廃棄する

**却下理由**: 「不一致」という定義は方向を持たないため、「宣言が宣言のサブセットしか実装されていない (実装中)」と「宣言を超える実装がある (契約違反)」を区別できなくなる。両者の信号が同じ Red になると、「実装が足りない」と「実装が宣言を超えている」という性質の異なる問題が混在し、対処が曖昧になる。§S の原則は「宣言に含まれない要素が実装にある」という方向を持つ定義により Yellow (実装中) と Red (契約違反) を明確に分離する。また「宣言が一致しない」という表現では、方向性 (forward check / reverse check) を持つ実装 (`evaluate_trait_methods` 等) の semantics を ADR レベルで正しく説明できない。

### A14: catalogue completeness check を別 Decision (D5) として追加し、D3 reverse check の範囲制限を維持する

D3 の reverse check 範囲を「カタログで `ApplicationService` kind として宣言された trait に限定」のまま維持し、「workspace 由来 trait がカタログに宣言されているか」の確認を別軸 (D5) として追加する方式。

**却下理由**:

- `TraitImplEntry::origin_crate` による workspace crate origin 判別を D3 で導入した時点で、D3 の reverse check 対象を workspace 由来 trait に直接限定できる。「カタログに宣言されているかの確認」は D3 の reverse check そのものであり、別軸 (D5) は不要になる
- D5 が検出する「workspace 由来 trait がカタログに宣言されていない」ケースは、D3 workspace 由来限定版の reverse extra と同一のギャップである。D5 を残すと同じギャップを 2 つの Decision が重複して担当することになり、type-designer が「どちらの信号がどの問題を検出しているか」を把握しにくくなる
- Decision が増えると評価ロジックの実装箇所も増え、将来の変更時に両方を一貫して修正する必要が生まれる。D3 に統合することでロジックの場所が 1 か所に集約される

### A10: catalogue index 逆引き (a') を判別手段として使い、type-designer の宣言漏れは運用ルールで補う

「カタログに宣言された trait のみ」を reverse check 対象とし (catalogue index 逆引き)、カタログに宣言されていない workspace 由来の hexagonal trait については「type-designer が必ずカタログに宣言する」という運用ルールで補う方式。この方式では `TraitImplEntry` への schema 変更が不要となる。

**却下理由**:

- 「型レベルで排除できる不正状態は型レベルで排除する」という Make Illegal States Unrepresentable の原則に対して不徹底である。type-designer が宣言し忘れた workspace 由来の hexagonal trait は reverse check の対象外となり、silent fail-open が発生する
- この状態は A6 (運用ルールで宣言を強制する) と同じ構造であり、検証機構のない合意に頼っている。カタログに宣言されていない trait がある場合、コードがどれだけ変化しても信号が動かない
- D3 の workspace 由来限定版 reverse check (workspace crate origin 判別による対象絞り込み) と A10 の catalogue index 逆引きでは、「type-designer がカタログに宣言し忘れた workspace 由来 trait を検出できるか」という点で機能差がある。A10 はその検出を運用ルールに委ねるため、silent fail-open を防げない
- `TraitImplEntry` への `origin_crate` フィールド追加は TypeGraph の内部照合情報の強化であり、カタログ側 schema を変えない。この強化を D3 と同じ ADR で決定することで Core invariant を満たせる。schema 変更を避けるための妥協として a' を残す理由はない

## Consequences

### 利点

1. **不正状態の構造的排除**: 「宣言できるが照合できない」「照合可能だが baseline で capture できない」という状態がなくなる。カタログ schema・TypeGraph schema・baseline schema が常に整合しているため、照合不能や baseline diff 失敗によるランタイム特別扱いが不要になる
2. **信号の意味が明確になる**: Blue / Yellow / Red のどれかが必ず確定する。「照合不能なので扱いを変える」という例外ロジックが存在しない
3. **設計規律の統一**: schema を拡張するたびに「TypeGraph 側の対応は？baseline 側の対応は？」を問う習慣が生まれる。この問いが拡張コストに組み込まれることで、軽率な schema 拡張を抑制する効果がある
4. **Contract Map の edge 密度が上がる**: フィールド宣言・interactor 実装宣言・free function 宣言が揃うことで、Contract Map 上で従来は孤立していたノードに edge が発生する
5. **カタログ ↔ TypeGraph ↔ baseline の 3 輪整合が保証される (D3)**: Core invariant が「カタログで宣言可能 ↔ TypeGraph で照合可能」の対応を保証し、D3 の workspace 由来限定版 reverse check が「TypeGraph 上の workspace 由来 trait impl がカタログの宣言に含まれているか」を直接確認する。さらに D3 bundle として追加した `TypeBaselineEntry::trait_impls` により baseline でも trait impl の origin crate 情報を capture でき、modify action での残骸評価が成立する。3 輪が揃うことで、カタログと実装の間に silent stale が発生しない構造が成立し、type-designer の宣言漏れも reverse extra として検出されるため (信号化は §S の action 別 mapping に従い、add action では Red、modify action では Yellow として扱われる)、silent fail-open が完全に排除される

### コスト / リスク

1. **TypeGraph 拡張の実装コスト**: `FreeFunction` kind の追加には TypeGraph への `functions` マップ追加と `build_type_graph` 側の対応が必要。catalog schema の拡張だけでは完結しない
2. **`TraitImplEntry::origin_crate` 抽出コストと baseline 側 capture コスト (D3)**: rustdoc JSON の `Crate.paths` / `external_crates` から trait の定義 crate を特定する処理が `build_type_graph` に加わる。workspace crate の集合を正確に取得する仕組み (Cargo.toml workspace メンバー一覧など) を infrastructure 層で用意する必要がある。さらに baseline builder が `TypeNode::trait_impls` を `Vec<TraitImplBaselineEntry>` に変換して `TypeBaselineEntry::trait_impls` に格納する処理も加わる
3. **schema 拡張の手数が増える**: 新しい宣言種別を追加するたびに「TypeGraph 側も baseline 側も同時に対応する」という制約がある。この制約は設計規律として正当だが、実装作業量の見積もりに反映する必要がある (カタログ schema 1 件の拡張が TypeGraph + baseline の 2 者を同時に引き連れる。D3 bundle では `TypeBaselineEntry::trait_impls` の追加がこの手数の具体例である)
4. **rustdoc JSON の free function 情報の精度**: rustdoc が公開 free function の引数型・戻り値型をどの程度の解像度で出力するかは、実装前に実測して確認する

## Reassess When

- **rustdoc JSON が free function の引数型・戻り値型を照合に必要な精度で提供できないことが実測で判明した場合**: `FreeFunction` kind の D4 の決定を見直す。`TypeGraph::functions` の設計を変更するか、存在チェックのみに後退するかを再評価する
- **`expected_members` の照合が struct フィールドと enum バリアントで意味論上の問題が生じた場合**: 例えば「enum バリアントの宣言」と「struct フィールドの宣言」を同一フィールドで扱うことが混乱を生むようなら、variant 別に宣言フィールドを分ける設計を再検討する
- **Contract Map の edge 描画ロジックと信号評価ロジックが乖離した場合**: 両者は同じ TypeGraph を参照するが、edge 描画の有無と信号の Blue/Red が一致しない状況が生じたら、評価ロジックの分担を再検討する
- **新しい宣言種別の追加で「既存 TypeGraph で照合可能か」の判断コストが問題になった場合**: 照合可能性の判定を schema 変更前に機械的にチェックする仕組みの導入を検討する
- **`TraitImplEntry::origin_crate` の抽出で rustdoc JSON の情報が十分でないことが実測で判明した場合**: origin crate の特定方法を再検討する。例えば rustdoc JSON の `external_crates` では workspace crate を正確に区別できないケースがあれば、Cargo metadata (`cargo metadata --format-version 1`) を補助情報として使う方法を検討する
- **workspace crate 由来だが hexagonal でない trait の false positive が実運用上問題になった場合**: D3 の reverse check 範囲をさらに絞り込む仕組み (例: カタログ側に「reverse check 対象外」を明示する opt-out 宣言) を検討する。その際は Core invariant に従い、カタログ schema 変更と TypeGraph 変更を同じ ADR で決定する
- **Interactor / SecondaryAdapter 以外の kind が workspace 由来 trait を impl するケースが設計上正当化された場合**: D3 / SecondaryAdapter reverse check の対象 kind リストを拡張する

## Related

- `knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md` — TypeGraph の構造と Reality View
- `knowledge/adr/2026-04-17-1528-tddd-contract-map.md` — Contract Map (Known Limitations §L2 / §L3 / §L4 が本 ADR の拡張動機)
- `knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md` — SecondaryAdapter variant の追加、D5 で `GitRepository` / `GhClient` 等の infrastructure 内部 trait をカタログ対象外と決定 (D3 の workspace 由来限定版 reverse check との関係に注意)
- `knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md` — TypeDefinitionKind の現行 taxonomy
- `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` — TDDD 多層化と layer-agnostic 不変条件
- `knowledge/adr/2026-04-11-0003-type-action-declarations.md` — 型アクション宣言 (add / modify / delete) と WIP Yellow 規則
- `knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md` — baseline (`<catalogue-stem>-baseline.json`) の設計: TypeBaseline / TypeBaselineEntry / TraitBaselineEntry の構造、baseline capture コマンド、4 グループ評価 (A\B / A∩B / B\A / ∁(A∪B)∩C)
