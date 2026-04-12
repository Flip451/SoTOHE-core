# TDDD Multilayer Extension — 型カタログの多層化とシグネチャ検証 (TDDD-01)

## Status

Proposed

## Context

SoTOHE の TDDD (Type-Definition-Driven Development) は `domain-types.json` で型を宣言し、
rustdoc JSON エクスポートと突き合わせて Blue/Yellow/Red のシグナルを出す仕組みである。

現状の制約:

1. **domain 層のみ対象**: `sotp track domain-type-signals` が `exporter.export("domain")` をハードコードしている (`apps/cli/src/commands/track/domain_state_signals.rs:62`)。usecase や infrastructure 層の型は TDDD の検証対象外
2. **メソッドシグネチャを検証しない**: `TraitPort` の `expected_methods` が `Vec<String>` (名前のみ)。引数型・戻り型を見ていないため、名前だけ一致すれば Blue になる。設計意図と異なるシグネチャ（primitive obsession 等）を検出できない
3. **層名がハードコード**: SoTOHE はテンプレートとして他プロジェクトに利用される。層名・crate 構成がプロジェクトごとに異なるため、`"domain"` のハードコードは汎用性を阻害する

これらは独立した問題ではなく連鎖している。多層化 (1) には層名の動的解決 (3) が前提となり、usecase 層で `TraitPort` を検証するにはシグネチャ検証 (2) が必要（名前だけでは port の設計意図を検証できない）。

## Decision

### D1: `architecture-rules.json` を TDDD 設定の SSoT にする

`architecture-rules.json` の `layers[]` に optional な `tddd` ブロックを追加する。
`architecture-rules.json` は v2 のまま (optional フィールド追加は破壊的変更ではない)。

```json
{
  "version": 2,
  "layers": [
    {
      "crate": "domain",
      "path": "libs/domain",
      "may_depend_on": [],
      "tddd": {
        "enabled": true,
        "catalogue_file": "domain-types.json",
        "schema_export": { "method": "rustdoc", "targets": ["domain"] }
      }
    },
    {
      "crate": "usecase",
      "path": "libs/usecase",
      "may_depend_on": ["domain"],
      "tddd": {
        "enabled": true,
        "catalogue_file": "usecase-types.json",
        "schema_export": { "method": "rustdoc", "targets": ["usecase"] }
      }
    },
    {
      "crate": "infrastructure",
      "path": "libs/infrastructure",
      "may_depend_on": ["domain", "usecase"],
      "tddd": { "enabled": false }
    }
  ]
}
```

設計原則:
- **層名のハードコード排除**: CLI は `architecture-rules.json` を読んで `tddd.enabled=true` の層を動的に発見する
- **`targets` は配列**: 1 層 = 複数 crate のプロジェクトに対応 (例: `["domain-core", "domain-events"]`)。単一 crate なら 1 要素配列
- **`catalogue_file` はデフォルト規約**: 省略時は `<crate>-types.json`
- **`method: "rustdoc"` を明示**: 評価器の実装方式を記録。Rust では rustdoc JSON が唯一の評価器基盤 (D6 参照)
- **オプトアウト可能**: `tddd` ブロックが無い / `enabled: false` の層は TDDD 対象外 (現状互換)

### D2: メソッドシグネチャを構造として宣言する (L1 解像度)

`expected_methods` を `Vec<String>` から `Vec<MethodDeclaration>` に拡張する。

#### syn の `Signature` を参考にした JSON 設計

JSON スキーマは syn クレートの `Signature` 型の構造を参考に設計する。syn の完全な AST は以下の通り:

```
syn::Signature
├── ident: Ident                           # 関数名
├── inputs: Punctuated<FnArg, Comma>       # パラメータ
│   ├── FnArg::Receiver  (&self 系)
│   └── FnArg::Typed     { pat, ty }
├── output: ReturnType                     # 戻り値
│   ├── Default           (-> () 省略)
│   └── Type(Box<Type>)
├── asyncness: Option<Async>               # async fn
├── generics: Generics                     # <T: Bound> + where
└── ...
```

TDDD はこの AST を再実装せず、**JSON スキーマ設計の参考** とする。実際の型情報取得は rustdoc JSON から行う (D6)。

#### L1 JSON スキーマ

```json
{
  "name": "UserRepository",
  "kind": "trait_port",
  "expected_methods": [
    {
      "name": "find_by_id",
      "receiver": "&self",
      "params": [
        { "name": "id", "ty": "UserId" }
      ],
      "returns": "Result<Option<User>, DomainError>",
      "async": false
    },
    {
      "name": "save",
      "receiver": "&self",
      "params": [
        { "name": "user", "ty": "User" }
      ],
      "returns": "Result<(), DomainError>",
      "async": false
    }
  ]
}
```

フィールドの対応:

| JSON フィールド | syn での対応概念 | 解像度 |
|---|---|---|
| `name` | `Signature::ident` | 完全一致 |
| `receiver` | `FnArg::Receiver` → `"&self"` / `"&mut self"` / `"self"` / `null` | 短縮文字列 |
| `params[].name` | `FnArg::Typed` → `PatType::pat` (Pat::Ident) | パターンは Ident のみ |
| `params[].ty` | `FnArg::Typed` → `PatType::ty` → 型表現文字列 | モジュールパスは最終セグメント、ジェネリクス構造は完全保持 |
| `returns` | `ReturnType::Type` → 型表現文字列。`Default` なら `"()"` | モジュールパスは最終セグメント、ジェネリクス構造は完全保持 |
| `async` | `Signature::asyncness` | bool |

#### L1 の検証ロジック

評価器は rustdoc JSON から取得した実コードのシグネチャ情報と L1 宣言を突き合わせる。
シグナルの意味は型レベルの TDDD シグナルと一致する:

- **Blue** = 宣言通りに実装済み
- **Yellow** = 宣言したがまだ途中 (WIP。interim commit では許容、`--strict` でブロック)
- **Red** = 根本的な問題 (常にブロック)

**Forward check** (カタログ宣言 → コード):

1. メソッド名が見つからない → **Yellow** (未着手 WIP)
2. `receiver` を比較 → 不一致なら **Yellow** (実装途中)
3. `params` の数が一致するか → 不一致なら **Yellow** (実装途中)
4. `params` の各 `ty` が実装の対応する引数型の型表現と **順序通りに完全一致** するか → 不一致なら **Yellow** (実装途中)
5. `returns` が実装の戻り値型の型表現と **完全一致** するか → 不一致なら **Yellow** (実装途中)
6. `async` が一致するか → 不一致なら **Yellow** (実装途中)
7. すべて一致 → **Blue**

**Reverse check** (コード → カタログ宣言):

7. trait にカタログで宣言されていないメソッドが存在 → **Red** (未宣言の実装)

#### 型表現の解像度

型表現は 2 つの次元を区別する:

| 次元 | 方針 | 例 |
|---|---|---|
| **モジュールパス** | 最終セグメント（短縮名） | `domain::user::UserId` → `UserId` |
| **ジェネリクス構造** | 完全保持 | `Result<Option<User>, DomainError>` をそのまま記述 |

モジュールパスの解決は cargo の責務であり TDDD では不要。一方、ジェネリクス構造（`Result`, `Option`, `Vec` 等のラッピング）は設計意図そのものであり、完全に記述する。

**既知の制約**: 異なるモジュールに同名の型がある場合（例: `domain::user::Id` と `domain::order::Id`）、短縮名が同一のため区別できない。これは TypeGraph のキーイングと同根の制約であり、TypeGraph が完全修飾名に移行する際に L1 も追従する。

完全マッチの利点:
- **実装が単純**: rustdoc JSON から型表現を文字列化し、宣言と比較するだけ。`Result<T, E>` のアンラップ等のファジーマッチルールが不要
- **情報量が多い**: エラー型 (`DomainError`)、`Option` の有無、コレクション型が明示される。カタログを読むだけでメソッドの契約がわかる
- **曖昧性がない**: `"returns": "User"` が `User` / `Result<User, _>` / `Option<User>` のどれにマッチするかという解釈問題が消滅する

#### L2 への拡張パス (将来)

L1 で不足する場合、`generics` フィールドを追加:

```json
{
  "name": "find_all",
  "receiver": "&self",
  "params": [{ "name": "filter", "ty": "F" }],
  "returns": "Vec<User>",
  "async": false,
  "generics": {
    "type_params": [{ "name": "F", "bounds": ["Fn(&User) -> bool"] }]
  }
}
```

syn の `Generics` → `GenericParam::Type` → `TypeParam { ident, bounds }` に直接対応。
L2 は L1 と加法的互換 (フィールド追加のみ) なので `schema_version` の破壊的更新は不要。

### D3: `DomainTypeKind` のリネーム

`DomainTypeKind` → `TypeDefinitionKind` (層中立な名前)。
同様に:
- `domain-types.json` のファイル名は `catalogue_file` で設定可能 (D1)。デフォルトは `<crate>-types.json`
- `DomainTypeEntry` → `TypeCatalogueEntry`
- `DomainTypesDocument` → `TypeCatalogueDocument`
- `evaluate_domain_type_signals()` → `evaluate_type_signals()`
- `/track:design` の `## Domain States` → `## Type Declarations`

後方互換性は対応しない。旧名のファイル・型は一括リネームし、v1 codec alias は設けない。

### D4: 層ごとの Kind 制限は TDDD コアに持たない

`TypeDefinitionKind` のどの Kind がどの層で使えるかは TDDD コアでは制限しない。
制約が必要なプロジェクトは `architecture-rules.json` に `forbidden_kinds` 等を追加して別 lint で対処する。

### D5: Cross-layer 型参照は Phase 1 では cargo に委ねる

#### 問題: 層をまたぐ型参照の検証を誰がやるか

多層 TDDD では、ある層のカタログが他層の型を参照する場面が出る。

例: domain 層に `User`, `UserId`, `DomainError` があり、usecase 層の port がこれらを使う:

```rust
// libs/usecase/src/ports.rs
pub trait UserRepository {
    fn find_by_id(&self, id: UserId) -> Result<Option<User>, DomainError>;
}
```

usecase カタログでこの port を宣言すると:

```json
{
  "name": "UserRepository",
  "kind": "trait_port",
  "expected_methods": [{
    "name": "find_by_id",
    "receiver": "&self",
    "params": [{ "name": "id", "ty": "UserId" }],
    "returns": "Result<Option<User>, DomainError>",
    "async": false
  }]
}
```

ここで `UserId`, `User`, `DomainError` は **usecase カタログに書かれているが、domain カタログで管理されている型**。これが cross-layer 参照。この参照の整合性を TDDD が検証すべきかが論点。

#### cargo が保証する領域

型の実在性とシグネチャの整合は cargo のコンパイルで保証される:

```rust
// typo → cargo がコンパイルエラーで止める
fn find_by_id(&self, id: UserId) -> Result<Option<Usr>, DomainError>;
//                                                 ^^^ 存在しない型
```

TDDD が改めて検証する必要はない。

#### TDDD L1 で検出可能な領域（cross-layer 参照なしで動作）

```rust
// カタログ宣言: params[0].ty = "UserId"
// 開発者の実装:
fn find_by_id(&self, id: i64) -> Result<Option<User>, DomainError>;
//                       ^^^ primitive obsession
```

cargo は **通る**（`i64` は有効な型）。しかし TDDD L1 の forward check で:
- カタログ: `params[0].ty = "UserId"`
- 実装: `params[0].ty = "i64"`
- → **Yellow**（シグネチャ不一致）

これは **cross-layer 参照の検証ではない**。usecase カタログ内で宣言と実装を比較しているだけ。`UserId` が domain カタログにも存在するかは見ていないが、primitive obsession は検出できている。

#### cargo に任せても実害が小さい領域

**宣言のみで未実装の参照:**

usecase カタログに `"ty": "UserId"` と書いたが、まだ `UserRepository` trait を実装していない（Yellow 状態）。このとき domain 側で `UserId` を削除しても:
- cargo は何も言わない（未実装の trait はコンパイル対象外）
- TDDD も何も言わない（cross-layer 参照を検証していない）
- 実害: 実装に着手したらコンパイルエラーで気づく。Yellow 期間だけの問題

**リネーム時の Yellow 段階での発見:**

domain で `UserId` → `UserIdentifier` にリネームした。usecase カタログにはまだ `"ty": "UserId"` と書いてある:
- trait が実装済みなら cargo がコンパイルエラーで止める
- trait が未実装なら cargo は何も言わないが、レビューで気づく範囲
- 実害: 実装済みのケースは cargo が守る。未実装のケースは Yellow 期間限定

#### 「Cross-layer 参照を catalogue に明示する」とは（Phase 2 検討事項）

TDDD 自体で cross-layer 参照を検証するには:

1. usecase カタログの `"ty": "UserId"` を読む
2. `UserId` が **どの層のカタログで宣言されているか** を全層カタログから横断検索する
3. domain カタログに `UserId` が存在し、Blue であることを確認する
4. 存在しなければ Red

これには全層カタログの横断検索が必要であり、実装コストが大きい。上記の通り cargo と L1 で実害の大部分はカバーできるため、Phase 1 では見送る。

#### cargo と TDDD の役割分担（まとめ）

| 検証対象 | cargo | TDDD L1 | Phase 2 cross-layer |
|---|---|---|---|
| 型の実在性（typo 等） | **検出可能** | 不要 | 不要 |
| 型の整合性（引数/戻り値の型エラー） | **検出可能** | 不要 | 不要 |
| primitive obsession（`i64` vs `UserId`） | 検出不可 | **検出可能** | 不要 |
| 未実装 trait の cross-layer 参照切れ | 検出不可 | 検出不可 | 検出可能（実害小） |

### D6: 評価器は rustdoc JSON を唯一の基盤とする

既存の rustdoc JSON ベースの評価器 (`RustdocSchemaExporter` + `build_type_graph`) を `targets` パラメタライズで多層化する。

syn によるソースコード直接パースは評価器の代替としない。rustdoc JSON は Rust コンパイラが既にパース・マクロ展開・型解決した結果であり、syn でこれを再実装するのは劣化コピーの再発明になる:
- マクロ展開後の型が見えない (derive マクロ等)
- ソースファイルの発見ロジックを自前で書く必要がある (cargo が解決済みの情報)
- 既存の `RustdocSchemaExporter` + `build_type_graph` の資産を捨てることになる
- nightly 依存は export ステップのみで影響範囲が限定的

syn の `Signature` 型は JSON スキーマ設計の参考 (D2) として活用するが、評価器の実装には使わない。

## Rejected Alternatives

### R1: 層ごとに別の `*-types.json` ファイルを固定命名する

`domain-types.json` / `usecase-types.json` / `infrastructure-types.json` を固定命名で並置する案。

却下理由: 層名がテンプレート利用プロジェクトごとに異なる。`core-types.json` / `app-types.json` / `adapters-types.json` のような名前に対応できない。`architecture-rules.json` の `catalogue_file` で動的に解決する D1 の方が汎用。

### R2: `expected_methods` にフルシグネチャ文字列を入れる

```json
"expected_methods": [
  "fn find_by_id(&self, id: UserId) -> Option<User>"
]
```

却下理由:
- JSON 内に Rust 構文が混在し、パースが必要になる (= syn を JSON 内で再実装)
- 空白・改行・属性の表記揺れで false negative が出る
- 構造化されていないので部分マッチ (「引数型だけ見たい」) ができない

### R3: TDDD コアで層ごとの Kind を制限する

「infra 層では Typestate を禁止」のような制約を `TypeDefinitionKind` の評価に組み込む案。

却下理由:
- `04-coding-principles.md` の推奨は禁止ではない
- プロジェクトごとに事情が異なる (infra 内部に小さな typestate を持ちたいケースもある)
- 制約が必要なら `architecture-rules.json` の拡張フィールドで別 lint として実装可能
- TDDD コアは Kind 中立であるべき

### R4: Cross-layer 参照を Phase 1 から catalogue に明示する

`params` の型名が他層 catalogue に存在するかを検証する仕組みを最初から入れる案。

却下理由:
- L1 シグネチャ検証 (D2) で primitive obsession は検出できる。これが最大の実害
- cross-layer 参照の評価には複数クレートの rustdoc JSON 相互参照が必要で実装コストが大きい
- Phase 1 スコープを膨らませると挫折リスク
- 未実装参照のリネーム不整合は Yellow 期間限定で実害小

### R5: syn ベース評価器（Phase 1 または将来）

rustdoc JSON の代わりに syn でソースコードを直接パースして評価する案。

却下理由:
- rustdoc JSON は Rust コンパイラが既にパース・マクロ展開・型解決した結果。syn でこれを再実装するのは劣化コピーの再発明
- マクロ展開後の型 (derive マクロ等) が syn 単独では見えない
- ソースファイルの発見ロジックを自前で書く必要がある (cargo / rustdoc が解決済みの情報)
- 既存の `RustdocSchemaExporter` + `build_type_graph` の資産を捨てることになる
- nightly 依存は export ステップのみで影響範囲が限定的であり、syn の唯一の利点（stable 動作）がこのコストに見合わない
- syn は JSON スキーマ設計の参考として活用する (D2) が、評価器の実装基盤としては不採用

## Consequences

### Good

- **汎用性**: SoTOHE をテンプレートとして使うプロジェクトが、任意の層構成で TDDD を利用できる
- **検証精度の向上**: L1 シグネチャ検証により、メソッド名だけでなく引数型・戻り型の設計意図を機械的に検証できる。primitive obsession を TDDD で検出可能になる
- **syn との整合**: JSON スキーマが syn の `Signature` の構造を参考に設計されており、型表現の概念モデルが Rust エコシステムと一貫している
- **段階的拡張**: L1 → L2 (generics/bounds)、単層 → cross-layer と加法的に拡張可能
- **既存資産の維持**: rustdoc JSON ベースの評価器を Phase 1 で継続し、移行リスクを抑える

### Bad

- **`architecture-rules.json` の責務増大**: 層定義 + 依存方向 + TDDD 設定を 1 ファイルに集約。ファイルが肥大化する可能性
- **リネーム作業**: `DomainTypeKind` → `TypeDefinitionKind` 等の rename が domain/infrastructure/cli の 3 crate に波及。後方互換性は対応しないため一括変更
- **nightly 依存の拡大**: rustdoc JSON エクスポートを複数 crate で実行するため CI 時間が増加。キャッシュ戦略の見直しが必要
- **L1 の限界**: ジェネリクス境界・ライフタイム・where clause は検証できない（L2 で対応可能）。ただし、これらの不整合は全てコンパイル時に cargo が検出するため、未検出のまま本番に出るリスクはない。L2 の価値は「trait 定義時点での早期発見」に限られ、必須ではない

### 補足: 実装上の判断と既知制約

Phase 1 計画時 (planner capability、2026-04-12) に確認された以下の項目を記録する。これらは D1–D6 の延長線上の実装判断であり、本 ADR の Status が `Accepted` に昇格した後は恒久的な契約として扱う。

#### C1: `FunctionInfo::signature: String` を削除する (planner Q2 再評価)

`FunctionInfo` に構造化フィールド (`params` / `returns` / `receiver` / `is_async`) を追加するのに合わせて、既存の `signature: String` フィールドは **削除する**。

削除の理由:

- 構造化フィールド (`params` / `returns` / `receiver` / `is_async`) が新しい source of truth であり、`signature` 文字列は完全な派生情報となる。冗長フィールドを残すと構造化フィールドとの不整合リスクが発生し、どちらが正なのか曖昧になる
- `signature: String` は現状 **読み出しコードが存在しない** (`libs/infrastructure/src/schema_export.rs::format_sig` で構築、`serde` derive で JSON に出るのみ)。BRIDGE-01 (`sotp domain export-schema`) の JSON 出力に現れるが、プロジェクト内に consumer は存在しない
- 構造化フィールドを常に含む前提であれば、`signature` 相当の文字列は表示時に `Display` impl / `signature_string(&self) -> String` 等で都度生成可能。レンダリング時に計算されれば十分で、永続化する意味がない

削除に伴う影響:

- **BRIDGE-01 JSON の breaking change**: `sotp domain export-schema` の JSON 出力から `signature` キーが消える。代わりに `params` / `returns` / `receiver` / `is_async` が出力される。consumer は構造化フィールドから signature 文字列を組み立てる
- **表示用途**: `domain-types.md` などの markdown render は `expected_methods` (`Vec<MethodDeclaration>`) を独自にフォーマットするため影響を受けない。`FunctionInfo` の表示が必要な場面では `MethodDeclaration::signature_string()` (後述) を使う
- **`MethodDeclaration` (catalogue / TypeGraph / baseline で共有)** には `signature` フィールドは **持たせない**。構造化フィールドのみを保持し、表示は都度生成する

`MethodDeclaration::signature_string(&self) -> String` はレンダリング専用のヘルパーメソッドとして提供する (`Display` impl でも可)。以下の形式で人間可読 signature を返す:

```text
[async ]fn name(receiver[, param1: ty1, param2: ty2]) -> returns
```

#### C2: `async-trait` 生成メソッドの `is_async` 検出は L1 の対象外 (planner EC-D)

Rust の `async fn in traits` は native 対応と `async-trait` proc-macro の 2 方式が存在する。後者は trait メソッドを `fn foo(&self) -> Pin<Box<dyn Future<Output = ...>>>` に desugar するため、rustdoc JSON には `Function.header.is_async = false` として現れる。

結果として:

- 開発者が catalogue に `"async": true` と宣言し、実装を `async-trait` で書いた場合、L1 forward check は「`is_async` 不一致」で Yellow を返す
- 運用ルール: `async-trait` 実装の trait port は catalogue 側でも `"async": false` と宣言する
- 将来 `native async fn in traits` が stable になり `async-trait` から移行した時点で、catalogue を `"async": true` に更新する

この制約は L1 (rustdoc JSON ベース) の構造的限界であり、syn ベース評価器を導入しない限り解消できない (D6 の「rustdoc JSON を唯一の基盤とする」方針に従う)。

## Reassess When

- **rustdoc JSON が stabilize された場合**: nightly 依存が解消され、CI 構成が簡素化できる
- **SoTOHE を Rust 以外の言語 (TypeScript 等) に展開する場合**: `schema_export.method` に言語固有の評価器を追加する設計が必要になる
- **Cross-layer 参照の検証漏れが実際にインシデントを起こした場合**: Phase 2 の cross-layer catalogue 参照を前倒し

## Implementation Phases

### Phase 1: 多層化 + L1 シグネチャ (MVP)

1. `architecture-rules.json` に `tddd` ブロック追加 (D1)
2. `DomainTypeKind` → `TypeDefinitionKind` 一括リネーム (D3)
3. `expected_methods` を `Vec<MethodDeclaration>` に拡張 (D2)
4. `TypeGraph` の拡張 (後述)。TDDD-02 (ADR `2026-04-11-0001`) で導入されたベースラインの比較解像度も自動的に向上する
5. `sotp track type-signals` を `--layer` パラメタライズ (D1)
6. `sotp verify spec-states` を全層 catalogue AND 集約に拡張
7. `/track:design` を多層対応 (`architecture-rules.json` から層を発見)

#### Phase 1-4: TypeGraph 拡張の方針

現状のデータフロー:

```text
rustdoc JSON
  ↓ RustdocSchemaExporter
SchemaExport (domain::schema)
  ├── TypeInfo      { name, kind, members: Vec<String> }       ← field名のみ、型なし
  ├── FunctionInfo  { name, signature: String,                 ← 文字列シグネチャ
  │                   return_type_names, has_self_receiver }
  ├── TraitInfo     { name, methods: Vec<FunctionInfo> }       ← FunctionInfo を持つ
  └── ImplInfo      { target_type, methods: Vec<FunctionInfo> }
  ↓ build_type_graph (infrastructure::code_profile_builder)
TypeGraph (domain::schema)
  ├── TypeNode   { kind, members: Vec<String>,                 ← field名のみ
  │               method_return_types: HashSet<String>,        ← 戻り型名のみ（メソッド名・引数は破棄）
  │               outgoing }
  └── TraitNode  { method_names: Vec<String> }                 ← メソッド名のみ（シグネチャは破棄）
```

`TraitInfo` は `Vec<FunctionInfo>` を持っているのに `TraitNode` では `Vec<String>` (名前のみ) になっている。
`ImplInfo` も同様。情報は rustdoc JSON にある。`build_type_graph` が捨てている。

拡張後:

```text
SchemaExport (domain::schema) — 拡張
  ├── TypeInfo      { name, kind,
  │                   members: Vec<MemberDeclaration> }        ← field名 + 型
  ├── FunctionInfo  { name,                                    ← signature: String は削除 (C1)
  │                   return_type_names, has_self_receiver,
  │                   params: Vec<ParamDeclaration>,           ← NEW: 構造化された引数
  │                   returns: String,                         ← NEW: 構造化された戻り型
  │                   receiver: Option<String>,                ← NEW: "&self" 等
  │                   is_async: bool }                         ← NEW
  ├── TraitInfo     { name, methods: Vec<FunctionInfo> }       ← 変更なし（FunctionInfo が拡張される）
  └── ImplInfo      { ... }                                    ← 変更なし

TypeGraph (domain::schema) — 拡張
  ├── TypeNode   { kind,
  │               members: Vec<MemberDeclaration>,             ← field名 + 型
  │               methods: Vec<MethodDeclaration>,             ← 完全なメソッドシグネチャ
  │               outgoing }                                   ← methods から導出可能
  └── TraitNode  { methods: Vec<MethodDeclaration> }           ← 完全なメソッドシグネチャ
```

変更箇所:

| 対象 | 変更 | 情報源 |
| --- | --- | --- |
| `TypeInfo::members` | `Vec<String>` → `Vec<MemberDeclaration>` | rustdoc JSON の field 型情報 (`RustdocSchemaExporter` で抽出) |
| `FunctionInfo` | `signature: String` を削除し、`params`, `returns`, `receiver`, `is_async` を追加 (C1) | rustdoc JSON から構造的に抽出。表示は `MethodDeclaration::signature_string()` で都度生成 |
| `TypeNode` | `method_return_types: HashSet<String>` → `methods: Vec<MethodDeclaration>` | `build_type_graph` が `FunctionInfo` → `MethodDeclaration` 変換 |
| `TypeNode::outgoing` | `FunctionInfo::return_type_names ∩ typestate_names` から引き続き derive (planner Q4 決定: `MethodDeclaration::returns` 文字列の再解析は不要) | `build_type_graph` が `FunctionInfo::return_type_names()` を使用 — セマンティクスは `methods` から導出可能だが実装は既存パスを維持 |
| `TraitNode` | `method_names: Vec<String>` → `methods: Vec<MethodDeclaration>` | `build_type_graph` が `FunctionInfo` → `MethodDeclaration` 変換 |

`MethodDeclaration` は D2 で定義した型と同一。カタログ宣言 / TypeGraph / ベースラインの 3 箇所で共有される:

```text
MethodDeclaration (domain 層の単一定義)
  ├── カタログ: expected_methods の各要素として（設計意図の宣言）
  ├── TypeGraph: TypeNode::methods / TraitNode::methods として（コードの実態）
  └── baseline: スナップショット時の TypeGraph から複製（design 時点の実態）
```

ベースラインは TypeGraph のスナップショットなので、TypeGraph が `MethodDeclaration` を持てばベースラインの比較解像度も自動的にシグネチャレベルに向上する。TDDD-02 (ADR `2026-04-11-0001`) の Bad に記載された「トレイト比較の解像度」制限が解消される。

**Baseline schema マイグレーション**: TypeGraph 拡張により `domain-types-baseline.json` の `schema_version: 1` は本 ADR のスキーマと非互換になる。後方互換性は対応しない方針のため、既存 baseline ファイルのマイグレーションは行わない。baseline は per-track の一時ファイルであり、完了した track の baseline は参照されないため放置で良い。TDDD-01 実装後に開始する新しい track では `baseline-capture` により新スキーマの baseline が生成される。

### Phase 2: L2 + cross-layer (拡張)

1. `MethodDeclaration` に `generics` フィールド追加 (L2)
2. `expected_variants` を `Vec<VariantDeclaration>` に拡張 (フィールド情報)
3. Cross-layer 参照の catalogue 明示と評価

### Phase 3: 高度な検証 (将来)

1. `impl Trait for Struct` のクロスクレート存在検証
2. 層ごとの Kind 制約 lint (`architecture-rules.json` 拡張)
3. 多言語対応 (`schema_export.method` の拡張)

## Related

- **ADR `2026-04-08-1800-reverse-signal-integration.md`**: TDDD reverse signal の導入元。本 ADR の多層化はその設計（単一ゲート、自動追加禁止）を全て維持する
- **ADR `2026-04-08-0045-spec-code-consistency-check-design.md`**: `check_consistency` 関数の設計元。L1 シグネチャ検証は `check_consistency` の入力データを拡張する
- **ADR `2026-04-11-0001-baseline-reverse-signals.md`** (TDDD-02): reverse signal のベースラインフィルタリング。0001 が先に実装され、本 ADR で baseline を per-layer 化 + `TypeGraph` 拡張によりベースライン比較の解像度を引き上げる
