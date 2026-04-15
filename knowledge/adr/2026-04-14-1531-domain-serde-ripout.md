# Domain serde 依存除去 — hexagonal 純粋性回復 + infrastructure 層 TDDD partial dogfood

## Status

Accepted (track: `domain-serde-ripout-2026-04-15`, planning approval 2026-04-14T15:31Z UTC)

## Context

### §1 プロセス違反の発見

2026-04-07 の bridge01 commit `a5e4c6b` (track `bridge01-export-schema-2026-04-06` の T005) で、ADR を書かずに `libs/domain/Cargo.toml` に `serde = { version = "1", features = ["derive"] }` 依存が追加された。これは hexagonal architecture の domain purity 原則 (`knowledge/conventions/hexagonal-architecture.md`) に違反する:

- domain 層は wire format / serialization 形式を知るべきではない
- DTO は infrastructure or usecase 層に置く
- domain 型に `Serialize` / `Deserialize` を持たせると DTO/domain 境界が溶ける

該当コードの footprint (researcher capability で確認済み、`knowledge/research/2026-04-14-1510-researcher-domain-serde-ripout.md` §3):

- `libs/domain/src/schema.rs` の 6 型 (`SchemaExport`, `TypeKind`, `TypeInfo`, `FunctionInfo`, `TraitInfo`, `ImplInfo`) に `#[derive(Serialize)]`
- `libs/domain/src/tddd/catalogue.rs` の 3 型 (`ParamDeclaration`, `MethodDeclaration`, `MemberDeclaration`) に `#[derive(Serialize)]`
- 合計 9 derive site + 2 `use serde::Serialize;` 文 + 1 module doc 言及 = 12 箇所
- `Deserialize` derive は 0 件、`#[cfg(test)]` 内の serde 利用も 0 件

ADR `2026-04-14-0625-finding-taxonomy-cleanup.md` D6 でも「`libs/domain` には既に serde crate 依存があるが (catalogue.rs / schema.rs で使用)、validated newtype は serialization-free を維持する」と記載されており、本問題は明示的に「本 track (tddd-04) 範囲外」とされた。本 ADR が tddd-04 D6 の追補としてその回復を担当する。

### §2 直接の caller と影響範囲

researcher capability 調査 (`knowledge/research/2026-04-14-1510-researcher-domain-serde-ripout.md` §5) によれば、domain 型を直接 serialize する箇所は **唯一** `apps/cli/src/commands/domain.rs:52,54` の `serde_json::to_string[_pretty](&schema)` のみ:

```rust
fn export_schema(args: &ExportSchemaArgs) -> Result<ExitCode, CliError> {
    let workspace_root = discover_workspace_root()?;
    let exporter = RustdocSchemaExporter::new(workspace_root);
    let schema = exporter.export(&args.crate_name)
        .map_err(|e| CliError::Message(e.to_string()))?;

    let json = if args.pretty {
        serde_json::to_string_pretty(&schema)  // ← domain::SchemaExport を直接 serialize
    } else {
        serde_json::to_string(&schema)
    }
    .map_err(|e| CliError::Message(format!("JSON serialization failed: {e}")))?;
    // ...
}
```

これは `sotp domain export-schema --crate <name>` の CLI コマンドで、`cargo make export-schema` から呼ばれ、bridge01-export-schema トラック以来の bridge01 JSON 出力フォーマット。**public CLI API なので JSON 形式は本 track で変えない**。

副次的な caller として `libs/infrastructure/src/schema_export_tests.rs` の `serde_json::to_string(&schema)` (`#[ignore = "requires nightly toolchain"]` 付き roundtrip テスト) も存在する。

### §3 既存 DTO パターン資産

researcher capability 調査 (`knowledge/research/2026-04-14-1510-researcher-domain-serde-ripout.md` §1) によれば、infrastructure 層には既に **38 DTOs / 11 files** の成熟した DTO パターンが存在する。代表例:

- `libs/infrastructure/src/tddd/catalogue_codec.rs`: `TypeCatalogueDocDto` / `TypeCatalogueEntryDto` / `MethodDto` / `ParamDto` / `TypeSignalDto` 等
- `libs/infrastructure/src/tddd/baseline_codec.rs`: `BaselineDto` / `TypeEntryDto` / `MemberDto` / `MethodDto` / `ParamDto` 等
- `libs/infrastructure/src/spec/codec.rs`: `SpecDocumentDto` / `SpecRequirementDto` 等
- `libs/infrastructure/src/track/codec.rs`: `TrackDocumentV2` / `TrackTaskDocument` 等

全 codec が `pub fn encode(&domain) -> Result<String, _>` / `pub fn decode(&str) -> Result<DomainType, _>` という対称的なパターンで統一されている。本 ADR の DTO 設計はこのパターンに揃える。

### §4 rustdoc viability の前提

researcher capability 静的調査 (`knowledge/research/2026-04-14-1510-researcher-domain-serde-ripout.md` §7) によれば、infrastructure crate には `pub struct/enum/trait` レベルでの同名衝突は存在しない。`MethodDto` / `ParamDto` / `SignalCountsDto` の重複は全て `private` 可視性であり rustdoc collision に該当しない。したがって `cargo +nightly rustdoc -p infrastructure -- -Z unstable-options --output-format json` は static 調査上 success する可能性が高い。ただし rustdoc nightly 仕様の不安定性のため T001 で実機検証する。

### §5 nutype crate との関係

researcher capability 調査 (`knowledge/research/2026-04-14-1510-researcher-domain-serde-ripout.md` §6) によれば、nutype 0.6 は serde feature をオプトインする設計であり、現在の SoTOHE-core では `nutype = "0.6"` (features 指定なし) で利用されている。`libs/domain/src/ids.rs` の nutype derive (`Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, AsRef`) は全て serde なしで動作する。**nutype は domain から serde を除去しても影響を受けない**。

### §6 Two-track split (Track 1 of 2)

本 ADR は **Track 1 of 2** に対応する。本 track のスコープは hexagonal 純粋性回復 + infrastructure 層 TDDD の partial dogfood (5-10 DTO seed) のみに限定する。以下は **Track 2 (`tddd-05-infra-wiring-YYYY-MM-DD`)** で扱う:

- Adapter / SecondaryPortAdapter variant の `TypeDefinitionKind` 拡張 (taxonomy ADR `2026-04-13-1813-tddd-taxonomy-expansion.md` の延長)
- infrastructure 層全体の catalogue 充実 (verify modules / review_v2 adapters / git wrappers / hook handlers / 各種 store などの 40-80 entries)
- CI rustdoc cache 戦略 (ADR `2026-04-11-0002-tddd-multilayer-extension.md` §3.E で deferred とされた item)
- infrastructure 内同名衝突解消 (T002 baseline-capture で検出される collision の rename cascade — T001 の plain rustdoc では build_type_graph を実行しないため collision は検出不可)

理由: 本トラックを膨らませると review concern が混ざり (hexagonal compliance vs taxonomy 設計 vs CI 最適化)、各トラック <500 行の guideline (`.claude/rules/10-guardrails.md`) を守れなくなる。Track 1 完了後の verification.md に Track 2 への 5 引継ぎ事項を記載することで継続性を担保する。

## Decision

### D1: serde 依存は infrastructure DTO に逃がす

`libs/domain/Cargo.toml` から `serde = { version = "1", features = ["derive"] }` を削除する。`libs/domain/src/` 配下の `Serialize` derive を全削除し、対応する DTO 群を `libs/infrastructure/src/schema_export_codec.rs` に新設する。`From<&domain::T> for TDto` 変換を infrastructure 内に実装する。

**判断根拠**:

1. **hexagonal 原則との整合**: §1 で述べた通り domain 層は wire format 形式 (serde) を知るべきではない。`Serialize` derive は serde という具体的な serialization library への依存であり、domain 型がこれを持つと「validated newtype (domain)」と「wire format carrier (DTO)」の境界が溶ける。DTO と変換関数を外層 (infrastructure) に置くことで、domain 型を serialization library から完全に切り離す (`knowledge/conventions/hexagonal-architecture.md`)。

2. **依存方向の制約**: `architecture-rules.json` により domain → infrastructure の依存は **禁止** されている (一方 infrastructure → domain は許可)。したがって domain 型を変換する DTO を定義できるのは infrastructure 側のみで、`impl From<&domain::T> for TDto` も infrastructure 側に置くのが hexagonal の依存方向に自然に沿う。CLI 側で変換する代替案 (`impl From` を CLI 内で呼ぶ) は D5 で却下する (CLI が DTO の存在を知ることになり、DTO は infrastructure の実装詳細であるという原則に反する)。

3. **既存 codec パターンとの一貫性**: `tddd/catalogue_codec.rs` / `tddd/baseline_codec.rs` / `spec/codec.rs` / `track/codec.rs` はいずれも infrastructure 内に private DTO を持ち、`pub fn encode(&domain) -> Result<String, _>` / `pub fn decode(&str) -> Result<DomainType, _>` という対称的なパターンで domain 型を入出力する (`knowledge/research/2026-04-14-1510-researcher-domain-serde-ripout.md` §1, §2 で計 38 DTOs / 11 files を確認)。本 ADR の `schema_export_codec.rs` も同じパターンに揃えることで、infrastructure crate の codec 群を読む開発者に一貫した mental model を提供する。

4. **From 変換が infallible に保てる**: `SchemaExport` は validated newtype を内包せず `String` / `Vec<T>` / `Option<String>` / `bool` のプリミティブ組み合わせ (`libs/domain/src/schema.rs`)。したがって `From<&domain::T> for TDto` は `&str → String::to_owned()` と `iter().map()` のみで構成でき、`unwrap()` / `expect()` / `panic!()` を本体に持たずに済む (`.claude/rules/04-coding-principles.md` §No Panics in Library Code)。`Result` を返す必要がなく API が最小化される。

### D2: schema_export_codec.rs 新設 — `tddd/codec/dto/` ディレクトリ案は却下

新 DTO の配置場所として以下 3 案を比較した:

- A1: `libs/infrastructure/src/tddd/codec/dto/` 配下に新規 module
- A2: `libs/infrastructure/src/schema_export_codec.rs` (新規ファイル) ← **採用**
- A3: 既存の `libs/infrastructure/src/schema_export.rs` 内に DTO 追加

**採用根拠 (A2)**:

1. **責務の分離**: `schema_export.rs` は `RustdocSchemaExporter` adapter (domain の `SchemaExporter` trait 実装) を持ち、外部プロセス呼び出しと rustdoc JSON 解析を担う。codec の責務 (DTO 定義 + encode 関数) は adapter の責務 (rustdoc 実行) と異なるので別ファイルが自然。

2. **既存パターンとの対称性**: D1 §3 で述べた既存 codec 群 (`tddd/catalogue_codec.rs`, `tddd/baseline_codec.rs`, `spec/codec.rs`, `track/codec.rs`) はいずれも「`<concern>_codec.rs`」という命名で独立ファイルに配置されている。本 ADR の `schema_export_codec.rs` も同じ命名規則に揃えることで、読み手が既存の mental model で新ファイルを把握できる。

3. **`schema_export.rs` の肥大化を避ける (A3 却下)**: `schema_export.rs` は既に 725 行あり、`architecture-rules.json` の `module_limits.warn_lines: 400` を既に超過している (`max_lines: 700` も超過)。DTO + 変換ロジック (~150 行) を追加するとさらに肥大化し、module 分割が必須となる。A3 は最終的に A2 と同じ結論に到達するので最初から A2 を採用する方が一貫性がある。

4. **`dto/` サブディレクトリは over-engineering (A1 却下)**: 本トラックで追加される DTO は 8 個 (7 DTO + 1 error_type) で、サブディレクトリを作ると module ナビゲーションコストが上がる。`catalogue_codec.rs` / `baseline_codec.rs` もそれぞれ 6-7 DTO を 1 ファイルに収めており、この規模で階層を分ける必要はない。Track 2 で DTO 数が 40+ に増えた段階で再評価する (Reassess When)。

### D3: 命名規則 — `SchemaParamDto` 等の独自命名で catalogue_codec の private DTO と区別

`tddd/catalogue_codec.rs` には既に `MethodDto` / `ParamDto` という private DTO が存在する (`knowledge/research/2026-04-14-1510-researcher-domain-serde-ripout.md` §1 で確認)。これらを schema_export 側で共有する案と、schema_export 専用 DTO を別途定義する案を比較して後者を採用する。

**共有案を却下した根拠**:

1. **L1 enforcement の非対称性**: catalogue_codec の `MethodDto` は decode 時に `::` を含む型名を拒否する L1 enforcement を持ち、validation ロジックと強く結合している (`libs/infrastructure/src/tddd/catalogue_codec.rs` の `method_from_dto`)。schema_export 側は **encode-only** で validation 不要 (domain 側が既に validated)。共有すると一方の制約が他方に漏れる。

2. **visibility の問題**: catalogue_codec の `MethodDto` / `ParamDto` は module-private な struct で、外部から参照するには `pub(crate)` 以上への visibility 昇格が必要。これは catalogue_codec の「internal 実装詳細を隠蔽する」という設計を破壊する。

3. **blast radius**: 共有モジュールを新設する (共通 `dto/method.rs` 等に移す) 案は、本トラックの scope 外である catalogue_codec 内部の改変が必要になる。本 ADR の scope は「domain から serde を除去する」であり、catalogue_codec の refactor は含まない。

4. **DRY 違反は意味的に薄い**: `MethodDto` / `ParamDto` の struct 形状は似ているが、catalogue_codec 側は decode path (JSON → domain, validation あり) を担い、schema_export 側は encode path (domain → JSON, validation なし) を担う。同じ形状でも意味的に異なる型として別立てする方が、将来の仕様変更 (例: 片方だけ新フィールド追加) に柔軟。

**採用案**: schema_export 専用に `SchemaParamDto` を別途定義する (`Schema` prefix で catalogue_codec の `ParamDto` と区別)。`MethodDto` 相当は schema_export 側では不要 (§D6 で述べた通り `MethodDeclaration::Serialize` は dead code で、SchemaExport の transitive serialize chain には含まれないため)。コード上の DTO 型一覧 (8 types、うち catalogue 登録は 7 `dto` + 1 `error_type` = 8 entries — `TypeKindDto` は private enum のため catalogue から除外、§D8 参照):

```
SchemaExportDto, TypeInfoDto, FunctionInfoDto, TraitInfoDto, ImplInfoDto,
TypeKindDto (snake_case enum — catalogue 除外、private のため rustdoc 対象外),
MemberDeclarationDto (externally-tagged Variant/Field enum — BRIDGE-01 互換、see D7),
SchemaParamDto
```

`SchemaExportCodecError` を `Json(#[from] serde_json::Error)` variant 1 つで定義する。catalogue 登録エントリ: 上記 8 types から `TypeKindDto` を除いた 7 `dto` + `SchemaExportCodecError` (1 `error_type`) = 合計 8 entries (§D8)。

### D4: encode-only DTO — `Deserialize` derive は付与しない

`sotp domain export-schema` コマンドは domain → JSON の片方向のみで、decode 経路 (JSON → `SchemaExport`) は caller 側に存在しない (`apps/cli/src/commands/domain.rs::export_schema` は `exporter.export()` → `serde_json::to_string` という流れで、JSON を読み戻す code path はない)。したがって本 ADR の DTO 群は **encode-only** とする。

**判断根拠**:

1. **YAGNI 原則**: `Deserialize` derive と `TryFrom<SchemaExportDto> for SchemaExport` の両方を追加すると、validated newtype の再構築ロジック (fallible, validation 失敗を `Result` で報告) を書く必要がある。現状 caller が存在しない機能を先行実装するのはメンテナンスコストを増やすだけで価値が低い。

2. **Reassess When のタイミングが明確**: 将来 JSON キャッシュや CI 高速化のために「生成済み `export-schema.json` を読み直して `SchemaExport` に戻す」要求が出た場合、その時点で `Deserialize` + `TryFrom` を追加する。追加時は `From` (infallible) → `TryFrom` (fallible) への切り替えであり、破壊的変更ではないため後追いでも問題ない。

3. **テスト簡略化**: roundtrip テスト (encode → decode → 等価性検証) を要求しないため、unit test は「encode 出力が期待する JSON 構造を持つ」ことの確認のみで済む。`libs/infrastructure/src/schema_export_tests.rs` の既存 roundtrip テスト (`#[ignore = "requires nightly toolchain"]` 付き) も encode-only 検証に縮小する (T003 で実施)。

**実装規約**:

- DTO は全て `#[derive(Serialize)]` のみ (Deserialize なし)
- `From<&SchemaExport> for SchemaExportDto` (および各下位型の From 実装) のみ実装
- `TryFrom<SchemaExportDto> for SchemaExport` は本トラックでは実装しない
- `From` 実装は全て infallible / panic-free (D1 §4 で述べた通り `SchemaExport` が validated newtype を内包しないため構造的に保証される)

### D5: encode 関数を infrastructure に置き、CLI から委譲する

`apps/cli/src/commands/domain.rs::export_schema()` 内で DTO 変換を行う案 (C1) と、infrastructure 内に `encode` 関数を置いて CLI から呼ぶ案 (C2) を比較して C2 を採用する。

**C1 (CLI 内で From 呼び出し) を却下した根拠**:

1. **CLI が DTO の存在を知ることになる**: `impl From<&SchemaExport> for SchemaExportDto` を CLI 内で呼ぶには、CLI が `infrastructure::schema_export_codec::SchemaExportDto` 型を import する必要がある。DTO は infrastructure の実装詳細であり、CLI 側から見えるのは encode 関数の入力 (domain 型) と出力 (JSON 文字列) だけであるべき。

2. **`knowledge/conventions/hexagonal-architecture.md` の CLI 責務定義違反**: 同 convention では CLI の責務を「(1) clap で引数パース, (2) infrastructure adapter 構築, (3) usecase 関数呼び出し, (4) 結果出力 + ExitCode mapping」と定義している。`SchemaExport` → JSON の変換は「(4) 結果出力」の一部だが、serialization 形式の知識は infrastructure 層に閉じるべきで、CLI が serde_json や DTO 型を直接扱うのは原則違反。

3. **既存 codec パターンとの非対称性**: `catalogue_codec::encode(doc: &TypeCatalogueDocument) -> Result<String, _>` などの既存 API は CLI からは `codec::encode(&domain_value)` という 1 関数呼び出しで完結しており、CLI は DTO の存在を一切知らない。本 ADR で C1 を採用すると schema_export だけが例外になり一貫性が崩れる。

**C2 (encode 関数を infrastructure に置く) を採用する根拠**:

1. **既存パターンとの完全な対称性**: `infrastructure::schema_export_codec::encode(schema: &SchemaExport, pretty: bool) -> Result<String, SchemaExportCodecError>` は `catalogue_codec::encode(doc: &TypeCatalogueDocument) -> Result<String, _>` と同じ形。CLI 側から見ると「domain 型を渡すと JSON 文字列が返る」というシンプルな契約になる。

2. **CLI 書き換えが最小**: 現行の CLI コード (`serde_json::to_string[_pretty](&schema)`) を `infrastructure::schema_export_codec::encode(&schema, args.pretty)` に 1 行差し替えるだけで済む。DTO import 追加や変換ロジック記述は不要。

3. **`pretty` フラグを引数で渡す**: `encode(schema, pretty: bool)` は `pretty` / `compact` を 1 関数で扱う。CLI からは `args.pretty` を渡すだけで済み、infrastructure 側は `serde_json::to_string_pretty` / `serde_json::to_string` を内部で切り替える。代替案として `encode_pretty` / `encode_compact` の 2 関数に分ける案もあるが、`catalogue_codec` が `encode` 1 つのみ公開している既存パターンに合わせて 1 関数にする。

`encode` 関数のシグネチャ:

```rust
pub fn encode(schema: &SchemaExport, pretty: bool) -> Result<String, SchemaExportCodecError>
```

`pretty` フラグを引数に取ることで、`pretty` / `compact` 用の関数を 2 つ作る代わりにシンプルに保つ (catalogue_codec パターンとの一貫性)。

### D6: dead code の `MethodDeclaration::Serialize` を同コミットで削除

`libs/domain/src/tddd/catalogue.rs:93` の `MethodDeclaration` は `#[derive(Serialize)]` を持つが、実際には dead code であり transitive な serialize 経路が存在しないことを確認した。

**dead code である根拠**:

1. **schema.rs からの参照元**: `MethodDeclaration` を参照する場所は `libs/domain/src/schema.rs` で `TypeNode::methods: Vec<MethodDeclaration>` と `TraitNode::methods: Vec<MethodDeclaration>` のみ (grep で確認)。これらは内部 query interface 用で serialization 対象ではない。

2. **TypeNode / TraitNode は `Serialize` derive を持たない**: `schema.rs:404` (`TypeNode`) / `schema.rs:473` (`TraitNode`) の定義でいずれも `#[derive(Debug, Clone)]` のみで serde derive なし (`libs/domain/src/schema.rs` で確認済み)。したがって `MethodDeclaration` は `SchemaExport` の serialize chain には含まれない。

3. **infrastructure / CLI 側の直接 serialize も存在しない**: researcher capability の grep 調査 (`knowledge/research/2026-04-14-1510-researcher-domain-serde-ripout.md`) で `serde_json::to_string*(<MethodDeclaration>)` の呼び出しはゼロ件。catalogue_codec は独自の `MethodDto` を定義して変換しているため、`domain::MethodDeclaration` の Serialize derive は誰からも transitive に Serialize されない。

**同コミットで削除する根拠**:

1. **DoD 達成**: 本トラックの DoD には「`grep -rn 'derive.*Serialize' libs/domain/src/` がゼロ件」が含まれる。`MethodDeclaration::Serialize` を残すとこの DoD を満たせない。

2. **物理的な近接性**: `ParamDeclaration` / `MemberDeclaration` と同じ `catalogue.rs` 内にあり、同じ `use serde::Serialize;` import を共有している。別 follow-up に分離すると、同じファイルを 2 回触り、2 度目の commit で初めて `use serde::Serialize;` 行を削除する段取りになる。1 コミット内で完結させる方が diff が小さく rollback も容易。

3. **影響範囲がゼロ**: caller が存在しないため、derive を削除してもコンパイルエラーは発生しない。リスクなしで削除できる。

### D7: BRIDGE-01 JSON wire format を維持する

`cargo make export-schema` の出力 JSON フォーマットは変えない。DTO のフィールド名は domain 型のフィールド名と 1:1 完全一致させる (`crate_name` / `types` / `functions` / `traits` / `impls` / `name` / `kind` / `docs` / `members` / `module_path` / `params` / `returns` / `receiver` / `is_async` / `target_type` / `trait_name` / `methods`)。`#[serde(rename_all = ...)]` は使わない。`MemberDeclarationDto` は現行 BRIDGE-01 フォーマットを維持するため `#[derive(Serialize)]` の default (externally-tagged) を使用する — `Variant(String)` → `{"Variant": "name"}` / `Field { name, ty }` → `{"Field": {"name": ..., "ty": ...}}`。`baseline_codec` の `MemberDto` は internally-tagged でフォーマットが異なるため参照パターンとして採用しない。

T004 で `cargo make export-schema -- --crate domain --pretty` の出力を変更前と手動 diff し、structural に同一であることを確認する。

### D8: infrastructure 層 TDDD partial dogfood — 8 DTO seed のみ + domain/usecase は opt-out

副次目的として `architecture-rules.json` の infrastructure tddd を `enabled: false` → `enabled: true` に flip し、本トラックで新設した 8 DTO を `infrastructure-types.json` に seed する (`knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md` の `Dto` variant を活用)。

#### per-layer opt-in model の採用

TDDD の per-layer opt-in model (ADR `2026-04-12-1200-strict-spec-signal-gate-v2.md` §D2.1、`libs/infrastructure/src/verify/spec_states.rs::evaluate_layer_catalogue` 実装) に従い、本トラックは以下を採用する:

| Layer | 本 track での扱い | catalogue file | Stage 2 spec-states |
|---|---|---|---|
| domain | **opt-out** | `track/items/domain-serde-ripout-2026-04-15/domain-types.json` を作らない | NotFound → skip (PASS) |
| usecase | **opt-out** | 同上 `usecase-types.json` を作らない | NotFound → skip (PASS) |
| infrastructure | **opt-in** | `infrastructure-types.json` を 8 entries で作成 | 評価対象 (T003 実装後 blue=8) |

opt-out 可能な根拠:

1. **本トラックは domain / usecase の構造的変更を起こさない**: domain 層の変更は `#[derive(Serialize)]` 削除のみで、`TypeNode::members` / `TypeNode::methods` (inherent impl) / `TypeKind` のいずれも変化しない。`build_type_graph` (`libs/infrastructure/src/code_profile_builder.rs:21, 36`) は trait impl を明示的に `i.trait_name().is_none()` filter で除外するため、serde derive が生成する `impl Serialize for T` は TypeGraph に現れない。usecase 層は変更なし。

2. **`spec_states.rs` の NotFound → skip semantics**: `evaluate_layer_catalogue` (`libs/infrastructure/src/verify/spec_states.rs:187-232`) は catalogue file が track dir に存在しない場合、その layer 全体を `VerifyOutcome::pass()` で skip する。`strict` モードでも同じ挙動 (ADR `2026-04-12-1200-strict-spec-signal-gate-v2.md` §D2.1 で確定)。したがって domain-types.json / usecase-types.json を本 track で作らなければ、Stage 2 は該当 layer に対して何も評価せず PASS する。

3. **tddd-04 precedent の正しい解釈**: tddd-04 (`tddd-04-finding-taxonomy-cleanup-2026-04-14`) は domain の `Finding` 型 2 種類を rename する内容で、`TypeNode::members` / `TypeNode::methods` を実際に変更するため domain catalogue の維持が必須だった。tddd-04 は T006 で tddd-01 の `domain-types.json` を in-place 編集する選択をとり、これは **自動継承機構ではなく tddd-04 の scope 判断**。本トラックは domain の構造を触らないため、この precedent を踏襲する必要はない。

#### seed 範囲は本 track 追加分のみに限定

verify modules / review_v2 adapters / git wrappers / hook handlers などの 40-80 entries の充実は **Track 2** (`tddd-05-infra-wiring-YYYY-MM-DD`) で扱う (`.claude/rules/10-guardrails.md` §Small task commits 原則)。

`TypeKindDto` は `pub` ではない private enum のため rustdoc JSON の export 対象に含まれず、catalogue に declare しても Yellow / Red どちらの signal にもならない。したがって catalogue から除外する (8 entries 中に含めない)。残る 8 entries は `SchemaExportDto` / `TypeInfoDto` / `FunctionInfoDto` / `TraitInfoDto` / `ImplInfoDto` / `MemberDeclarationDto` / `SchemaParamDto` (= 7 `dto` variant) + `SchemaExportCodecError` (= 1 `error_type` variant with `expected_variants: ["Json"]`)。

### D9: タスク分割 — 5 commits (T001-T005)

本トラックを 5 commits に分割する:

| Commit | Tasks | 推定変更行数 | コンパイル状態 |
|---|---|---|---|
| Commit 1 | T001: rustdoc viability audit + prereq doc fix (6 箇所: 2 × `invalid_html_tags` + 4 × `private_intra_doc_links`) + `architecture-rules.json` infrastructure tddd 有効化 | ~42 行 | Pass |
| Commit 2 | T002: `/track:design --layer infrastructure` 実行 (`infrastructure-types.json` + `infrastructure-types-baseline.json` + rendered view 生成、yellow=8 初期状態) | ~150 行 (catalogue JSON + baseline JSON + rendered md) | Pass |
| Commit 3 | T003: `libs/infrastructure/src/schema_export_codec.rs` 新設 (DTO + encode 関数) — yellow=8 → blue=8 遷移 | ~150 行 | Pass (domain serde はまだある) |
| Commit 4 | T004: domain serde 除去 + CLI 書き換え + `schema_export_tests.rs` 更新 | ~36 行 | Pass (T003 完了が必須) |
| Commit 5 | T005: `knowledge/adr/README.md` 索引追加 (本 ADR + 未登録 2 ADR) + verification.md 完了 + Track 2 引継ぎ事項記載 | ~33 行 (README.md ~3 行 + verification.md ~30 行) | Pass |

**分割根拠**:

1. **`/track:design` を独立 task として分離**: TDDD workflow は `/track:plan` → `/track:design` → `/track:implement` の順で型宣言を実装より先行させる設計 (`knowledge/adr/2026-04-08-1800-reverse-signal-integration.md` §5)。T002 で `infrastructure-types.json` と `infrastructure-types-baseline.json` を先に作ることで、T003 の DTO 実装時に yellow=8 → blue=8 への signal 遷移を明示的に観測できる。これは TDDD の「先に型宣言、後に実装」原則そのもの。T002 と T003 を 1 commit にまとめると、catalogue 宣言と実装が同時に出現し、signal の段階遷移が観測できなくなる (常に blue=8 で catalogue が書かれる)。

2. **T001 は T002 の前提条件**: `/track:design --layer infrastructure` は `architecture-rules.json` で `infrastructure.tddd.enabled: true` になっていることを前提とする (`libs/infrastructure/src/verify/tddd_layers.rs::parse_tddd_layers` が enabled フラグをチェックし、disabled layer は `--layer` 引数で明示指定してもエラーとする)。T001 の flip を完了してからでないと T002 で `/track:design` を起動できない。したがって T001 → T002 は厳密な先行関係。

3. **中間状態で workspace を compile-clean に保つ**: T003 (DTO 新設) 完了時点で domain の `Serialize` derive はまだ残っており、T004 で初めて削除する。この段階的移行によって各 commit 境界で `cargo build --workspace` / `cargo test --workspace` が通る状態を保てる。T003 と T004 を 1 commit に統合すると、同一 commit 内に「domain Serialize 有り + DTO 追加」「domain Serialize 無し + DTO + CLI 書き換え」の 2 つの compile-clean 状態しか存在せず、中間状態での bisect や部分的 rollback ができない。

4. **Commit 4 を最小変更にして rollback コストを下げる**: T004 は serde 削除 + CLI 書き換え + test 更新のみで ~36 行。もし T004 後に想定外の問題 (BRIDGE-01 JSON フォーマット差異、infrastructure crate の予期せぬ依存など) が見つかった場合、`git revert <T004 commit>` で serde 削除だけを巻き戻せる (T003 の `schema_export_codec.rs` は残る)。1 commit に統合すると revert 範囲が ~200 行になり、部分巻き戻しが困難になる。

5. **T001 は gating ステップ**: `cargo +nightly rustdoc -p infrastructure -- -Z unstable-options --output-format json` が失敗した場合、本トラック全体を停止して別の prereq track (`tddd-05-infra-rustdoc-fix-prereq` 等) を立て直す判断を行う。T001 を独立 commit にすることで、この gating 結果が即座に track registry / verification.md に反映され、後続 task への進行可否が明確になる。T002 以降と統合すると、gating 失敗時に実装分も一緒に破棄することになり無駄が大きい。

6. **review 粒度の明確化**: 各 commit が単一の concern にスコープされ (T001=設定変更, T002=catalogue 宣言 + baseline capture, T003=新規コード追加, T004=既存コード改変, T005=ドキュメント整理)、reviewer が各 commit を独立した単位で読める。`.claude/rules/10-guardrails.md` §Small task commits 原則の「diff < 500 行 / commit」にも全 commit が収まる (最大は T002 と T003 の ~150 行)。

### D10: Track 2 への引継ぎ事項 — verification.md に 5 項目記載

本トラック完了時、verification.md に Track 2 (`tddd-05-infra-wiring-YYYY-MM-DD`) への引継ぎとして以下を記載:

1. rustdoc viability audit 結果 (success/failure, JSON サイズ, wall time)
2. infrastructure 内同名衝突 audit 結果 (見つかった場合は型名と location)
3. infrastructure-types.json に seed した DTO 一覧
4. CI rustdoc 実行時間の体感 (domain + usecase + infrastructure の 3 layer での wall time、許容範囲か否か)
5. Adapter variant が必要そうな infra type の暫定リスト (`CodexReviewer`, `FsReviewStore`, `GitDiffGetter`, `Sha256ReviewHasher` 等)

### D11: 既存 ADR 索引の補完

`knowledge/adr/README.md` の信号機アーキテクチャ section に本 ADR を追加する。同時に未登録の以下 2 ADR も併せて追加する:

- `knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md`
- `knowledge/adr/2026-04-14-0625-finding-taxonomy-cleanup.md`

これらは tddd-04 完了時に索引追加されておらず、本トラックの T005 で併せて補完する。

## Rejected Alternatives

### A1: serde を残して許容 (継続違反)

却下理由: bridge01 commit a5e4c6b は ADR なしで domain に serde を追加しており、これを許容すると hexagonal 純粋性原則 (`knowledge/conventions/hexagonal-architecture.md`) が将来の同種違反を防げなくなる。プロセス違反の回復が本トラックの主目的。

### A2: catalogue_codec の private DTO を共有

却下理由 (D3 と同じ):
- catalogue_codec の `MethodDto` / `ParamDto` は L1 enforcement (`::` チェック) と decode validation を持つ
- 共有すると catalogue_codec の internal DTO を external に晒す必要があり、別 codec の internal を改変するコストが高い
- DRY 違反への懸念は正当だが、本 track の blast radius を広げないことを優先

### A3: CLI 内で `From<&SchemaExport> for SchemaExportDto` を呼ぶ

却下理由 (D5 と同じ): CLI が DTO の存在を知ることになり、DTO は infrastructure の実装詳細であるという原則に反する。`knowledge/conventions/hexagonal-architecture.md` の CLI 責務定義に違反。

### A4: usecase 層に DTO を置く

却下理由: usecase 層は純粋なオーケストレーターであり (`knowledge/conventions/hexagonal-architecture.md` Usecase Layer Purity Rules)、wire format DTO は infrastructure 層が担うのが正しい配置。usecase はすでに別の serde DTO を保持しているが (Clean Architecture スタイル)、schema_export の DTO は infrastructure 配置が自然。

### A5: tddd/codec/dto/ サブディレクトリ案

却下理由 (D2 と同じ): over-engineering。本トラックで追加される DTO は 8 個に限定され、サブディレクトリを作ると module 構造が複雑化する。Track 2 で DTO 数が大幅に増えた段階で再評価する。

## Consequences

### Positive

- **hexagonal 純粋性回復**: domain crate が wire format に対する非依存に戻り、将来の同種違反を防ぐ structural incentive になる
- **infrastructure 層 TDDD の partial dogfood 開始**: `infrastructure-types.json` に 8 entries が seed され、`bin/sotp track type-signals --layer infrastructure` が `blue=8 yellow=0 red=0` を返す状態が達成される
- **ADR `2026-04-14-0625-finding-taxonomy-cleanup.md` D6 の追補**: tddd-04 で「本 track 範囲外」とされた domain serde 依存が正式に Resolved となる
- **Track 2 への足場確立**: T005 verification.md に 5 引継ぎ事項が記載され、`tddd-05-infra-wiring-YYYY-MM-DD` がスムーズに着手できる
- **既存 ADR 索引の補完**: handoff §F で指摘された未登録 ADR 2 件 (`2026-04-13-1813-tddd-taxonomy-expansion`, `2026-04-14-0625-finding-taxonomy-cleanup`) が併せて索引追加される
- **rollback コストが低い**: Commit 4 (T004) が最小変更 (~36 行) で、`git revert` で個別 rollback 可能

### Negative / Trade-offs

- **infrastructure crate のテスト時間がわずかに増加**: `schema_export_codec.rs` の unit test 追加分
- **DTO の維持コスト**: 将来 `SchemaExport` のフィールドを変更する際、infrastructure DTO 側も併せて更新する必要がある (1 箇所だけだった serde derive が 2 ファイル間の同期に変わる)
- **CI rustdoc 時間が線形に増加**: infrastructure tddd を有効化することで `cargo +nightly rustdoc` が 3 layer (domain + usecase + infrastructure) で走るようになる。許容範囲か Track 2 で再評価する (ADR `2026-04-11-0002-tddd-multilayer-extension.md` §3.E の deferred item に直結)
- **catalogue_codec の `MethodDto` / `ParamDto` と schema_export_codec の `SchemaParamDto` の DRY 違反**: 名前と structure が似ているが、L1 enforcement の有無で意味が異なる。Track 2 で共通化を再評価する余地あり

### Neutral

- BRIDGE-01 JSON wire format は完全に維持される (D7)
- `catalogue_codec` / `baseline_codec` の public API は変更なし (これらは既に DTO 経由で実装されている)

## Reassess When

- 将来 serde の代替 (postcard / bincode 等) を検討するとき
- DTO 群が肥大化したとき (本トラックで 8 entries → 将来 100+ になったら module 分割を検討)
- Track 2 で Adapter variant を追加するとき (catalogue 設計の追補が必要になる可能性)
- CI rustdoc 時間が許容できなくなったとき (cache 戦略の実装トリガー)
- infrastructure 内に新しい同名衝突が発生したとき (rename cascade の必要性を再評価)

## References

- bridge01 commit `a5e4c6b` (track `bridge01-export-schema-2026-04-06` の T005、2026-04-07): プロセス違反の元コミット
- ADR `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` §D1 / §3.E: TDDD multilayer の SSoT、infrastructure 設定と CI cache 戦略
- ADR `knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md`: `TypeDefinitionKind::Dto` variant の追加 ADR
- ADR `knowledge/adr/2026-04-14-0625-finding-taxonomy-cleanup.md` D6: domain serde 依存「本 track 範囲外」記述の追補対象
- Convention `knowledge/conventions/hexagonal-architecture.md`: hexagonal 純粋性原則の SSoT
- Convention `.claude/rules/04-coding-principles.md` §Enum-first / §No Panics: DTO 設計の原則
- Convention `.claude/rules/10-guardrails.md` §Small task commits: <500 行 per commit guideline
- Planner output `knowledge/research/2026-04-14-1510-planner-domain-serde-ripout.md`: Q1-Q7 設計判断と Canonical Blocks (DTO スケルトン, Mermaid graph, infrastructure-types.json seed)
- Researcher output `knowledge/research/2026-04-14-1510-researcher-domain-serde-ripout.md`: 既存 DTO inventory (38 DTOs / 11 files), domain serde footprint 完全リスト, nutype 解析, infrastructure 重複型名 audit
- Track artifacts `track/items/domain-serde-ripout-2026-04-15/{metadata,spec,verification}.{json,md}`: 本トラックの実行成果物
