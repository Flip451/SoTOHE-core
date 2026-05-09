<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# TDDD v2 — catalogue layer schema / rustdoc_types::Crate hybrid TypeGraph / 3-way diff 信号評価器の実装

## Tasks (0/9 resolved)

### S1 — 新 CatalogueDocument schema — domain 型 (newtype 系 + Role/Action/Pattern 軸分離)

> T001 は newtype 系 12 種 (Identifier base + 各 newtype) と Role/Action/Layer の primitive enum を実装する。T002 はこれを基盤として複合構造 (TypeKindV2 / CompositePattern / VariantPayload / エントリ群 / CatalogueDocument) を実装する。
> ADR 1 D1-D12 の型レベル制約 (TypeEntry に ContractRole を付けようとすると parse 段階で reject / members×kind の制約 schema 構造 encode / VariantPayload 3 variant) を CatalogueDocument 実装に折り込む。
> 旧型 (TypeDefinitionKind 等) はこのフェーズでは削除しない。T003 の codec 切り替え後に T008 で一括削除する。<500 行 / commit を維持するため newtype+primitive と複合構造を T001/T002 に 2 分割している。

- [~] **T001**: 新 catalogue schema domain 型 (newtype 系 + Role 3 enum + ItemAction + SelfReceiver + Layer) を実装する。Identifier (共通 base validation) / TypeName / TraitName / FieldName / MethodName / ParamName / VariantName / CrateName / FunctionName / ModulePath / TypeRef / FunctionPath の 12 newtype に Display / Serialize / Deserialize / FromStr を実装する。DataRole (13 値) / ContractRole (3 値) / FunctionRole (2 値) / ItemAction (4 値) / SelfReceiver (3 値) / Layer (3 値) を enum として実装する。libs/domain/src/tddd/catalogue.rs 内の新 tddd::catalogue_v2 (または tddd/catalogue/ サブモジュール) に配置する。ラウンドトリップ unit test (AC-03) を追加する。この時点では旧型 (TypeDefinitionKind 等) はまだ残す。
- [ ] **T002**: 新 catalogue schema domain 型 (複合構造: TypeKindV2 / CompositePattern / VariantPayload / VariantDecl / FieldDecl / MethodDeclaration / ParamDeclaration 修正 / TraitImplDeclV2 / TypeEntry / TraitEntry / FunctionEntry / CatalogueDocument + CatalogueDocumentError) を実装する。TypeKindV2 (Struct/Enum/TypeAlias の 3 variant payload-encoded) / CompositePattern / VariantPayload (Unit/Tuple/Struct) / VariantDecl (name+payload, serde default=Unit) / FieldDecl / TraitImplDeclV2 (identity-only) を実装する。既存 ParamDeclaration / MethodDeclaration を newtype フィールド (ParamName/TypeRef/MethodName/SelfReceiver) に修正する。TypeEntry / TraitEntry / FunctionEntry / CatalogueDocument (3-BTreeMap + validation) / CatalogueDocumentError を実装する。crate_name とファイル名一致 validation を実装する。serde ラウンドトリップ unit test (AC-01 / AC-02 / AC-03 の新構造部分) を追加する。

### S2 — 新 CatalogueDocument serde codec — infrastructure 層 (CatalogueLoader 更新含む)

> T003 は新 CatalogueDocument 専用 codec を infrastructure 層に実装し、旧 TypeCatalogueDocument 経路を削除する (no-backward-compat 原則)。
> FsCatalogueLoader を CatalogueDocument を返すように修正し、CatalogueLoader port を Vec<(LayerId, CatalogueDocument)> 返り値に更新する。
> CatalogueLoader port が domain 層か infrastructure 層のどちらに属すべきかは open question として残す (hexagonal 上は usecase 内側 port が正だが、既存 catalogue に infrastructure 配置で declare されている)。
> S1 (domain 型 T001/T002) の完了後に着手する。

- [ ] **T003**: 新 catalogue schema の infrastructure 層 serde codec を実装する。CatalogueDocumentCodecError (Json / UnsupportedSchemaVersion / InvalidEntry / CrateNameMismatch) を infrastructure 層に実装する。旧 TypeCatalogueCodecError / TypeCatalogueDocument / TypeCatalogueEntry / TypeDefinitionKind を読む serde 経路を削除し (no-backward-compat 原則)、新 CatalogueDocument 専用 codec を infrastructure/src/tddd/catalogue_codec.rs に実装する。FsCatalogueLoader (infrastructure) を CatalogueDocument を返すように修正し、CatalogueLoader port (infrastructure-types.json に declare) の返り値型を Vec<(LayerId, CatalogueDocument)> に変更する。旧 TypeCatalogueCodecError を削除する。codec unit test (AC-01 の serde round-trip / CrateNameMismatch / UnsupportedSchemaVersion) を追加する。

### S3 — ExtendedCrate schema + Catalogue → TypeGraph A codec (domain port + infrastructure adapter)

> T004 は ExtendedCrate schema (domain 層) と BaselineRustdocCodec / CatalogueToExtendedCratePort を実装する。T005 は syn 依存の TypeRef parse + external_crates auto-build + inline→id 参照変換 + Catalogue→ExtendedCrate codec (infrastructure 層 adapter) を実装する。
> T004 と T005 は依存関係があるため逐次実施 (T004 完了 → T005 着手)。
> TypeRef parse は syn::parse_str::<syn::Type> を使い自前 tokenizer は書かない (CN-08)。未解決マーカーは open-world で保持し、closed-world 検証は T006 Phase 1 で実施 (CN-06)。
> S1/S2 の完了後に着手する。

- [ ] **T004**: ExtendedCrate schema を domain 層に実装する。ExtendedCrate { krate: rustdoc_types::Crate, item_actions: BTreeMap<Id, ItemAction> } を libs/domain/src/tddd/ 内 (例: extended_crate.rs) に実装する。BaselineRustdocCodecError (Json/IoError/UnsupportedFormatVersion) を infrastructure 層に実装し、rustdoc_types::Crate JSON をロードする concrete deserializer を infrastructure internal implementation (catalogue に track する named adapter contract なし) として実装する。CatalogueToExtendedCratePort (domain 層 secondary_port) と NewTypeGraphCodecError (domain 層 error_type) を実装する。CatalogueToExtendedCrateCodecError (infrastructure 層 error_type) を実装する。AC-04 の ExtendedCrate unit test を追加する。
- [ ] **T005**: Catalogue → ExtendedCrate (TypeGraph A) codec の core 変換ロジックを実装する。syn crate を使用した TypeRef generics parse (syn::parse_str::<syn::Type> → rustdoc_types::Type 変換) を実装する。std prelude allowlist (Vec/Option/Result/String/Box 等) の自動解決を実装する。未解決マーカー表現 (未 declare 型の保持方式) を実装する。TraitImplDeclV2.origin_crate + TypeRef crate prefix からの external_crates 自動 build (per-graph incremental crate_id 発番, crate_id==0 は自 crate) を実装する。inline → id 参照変換 (FieldDecl/VariantDecl を別 Item として index に登録し Vec<Id> 参照) を実装する。1 type = 1 Inherent Impl block の grouping を実装する。Crate.paths の [crate_name, ...module_path, item_name] 形式生成を実装する。CatalogueToExtendedCrateCodec (infrastructure 層 secondary_adapter) として CatalogueToExtendedCratePort を実装する。AC-05 / AC-06 の codec unit test (inline→id-ref 変換 / generics parse / module_path 込み paths 生成 / std prelude 自動解決 / 未解決マーカー生成) を追加する。

### S4 — Signal evaluator Phase 1 (S/D 構築) + Phase 2 (3-way 評価)

> T006 は Phase 1 (S/D 構築 + declare 整合性検証 + closed-world 検証) を domain 層に実装する。T007 は Phase 2 (11 領域 × signal table) を infrastructure 層 SignalEvaluatorV2 として実装する。
> T006/T007 は依存関係があるため逐次実施 (T006 完了 → T007 着手)。
> S3 の ExtendedCrate + codec が前提となるため S3 完了後に着手する。
> identity 判定基準 (types/traits=short name / functions=FunctionPath、ADR 2 D3 / ADR 3 D2) を Phase 1/2 の全処理で統一する (CN-03)。

- [ ] **T006**: Signal evaluator Phase 1 (S/D 構築) を domain 層に実装する。SignalRegion (12 variant) / ThreeWaySignalKind (Skip/Blue/Yellow/Red) / ThreeWaySignal / ThreeWayEvaluationReport を domain 層に実装する (T007 の SignalEvaluatorV2 が実装する SignalEvaluatorPort の戻り値型として T006 で定義する必要がある)。SignalEvaluatorPort を domain 層に実装する (evaluate(&self, a: ExtendedCrate, b: Crate, c: Crate) -> Result<ThreeWayEvaluationReport, Phase1Error>)。Phase1Error (ActionContradiction / UnresolvedTypeRef / DanglingId) を実装する。Phase 1 アルゴリズム: B 由来全要素を S に Reference で attach (types/traits=short name / functions=FunctionPath で identity、S 内 flat incremental Id 再発番)。A の各要素を action 別 (Add/Modify/Reference/Delete) に処理して S/D を構築する。Phase 1.5 (unresolved marker closed-world 検証: Delete 後の S を universe set とし、resolve 不能なら Phase1Error) を実装する。Phase 1.6 (dangling Id 検証) を実装する。external_crates の S/D 各スコープでの再発番を実装する。AC-07 の Phase 1 unit test (action 別処理 / ActionContradiction / UnresolvedTypeRef / DanglingId) を追加する。
- [ ] **T007**: Signal evaluator Phase 2 (S/D/C 3-way 評価) を infrastructure 層に実装する。SignalRegion / ThreeWaySignalKind / ThreeWaySignal / ThreeWayEvaluationReport は T006 で domain 層に定義済み。SignalEvaluatorV2 (infrastructure 層 secondary_adapter: implements SignalEvaluatorPort) を実装し、Phase 2 の 11 領域 × signal table (ADR 3 D3) を実装する: S∩C+構造一致+action別 / S∩C+構造不一致+action別 / S\C+action別 / D∩C / D\C / C\(S∪D) の各領域を網羅する。identity 判定は types/traits=short name / functions=FunctionPath で統一する。AC-08 の Phase 2 unit test (全 11 領域の signal 判定、境界ケース含む) を追加する。

### S5 — 旧 TypeGraph 置換 + 旧 schema 削除 + 既存コード書き換え

> T008 は旧 TypeGraph (schema.rs の TypeNode/TraitNode/FunctionNode/TraitImplEntry HashMap 独自 schema) と旧 TypeBaseline 系を削除し、TypeGraph を読む全既存コードを新形式 (rustdoc_types::Crate / ExtendedCrate) に書き換える。
> 旧型の削除タイミングを S4 完了後まで遅らせることで、S1-S4 の各 task では旧コードを壊さずに新コードを追加できる。T008 で一括置換することでレビューサーフェースを明確化する。
> Contract Map renderer / Reality View renderer の新 TypeGraph 形式への完全対応は OS-06 でスコープ外。T008 では callers が compile error にならない最小限の修正を行う。
> S4 完了後に着手する。

- [ ] **T008**: 既存 TypeGraph (libs/domain/src/schema.rs の TypeNode/TraitNode/FunctionNode/TraitImplEntry HashMap ベース独自 schema) を削除し、TypeGraph を読む既存コード (tddd/consistency.rs / tddd/signals.rs / contract_map_render.rs / infrastructure/tddd/type_signals_evaluator.rs / infrastructure/tddd/type_graph_cluster.rs 等) を新 TypeGraph 形式 (rustdoc_types::Crate / ExtendedCrate) に書き換える。旧 TypeBaseline / TypeBaselineEntry / TraitBaselineEntry / FunctionBaselineEntry / TraitImplBaselineEntry (libs/domain/src/tddd/baseline.rs) を削除し、baseline の保存・ロードを rustdoc_types::Crate JSON に変更する。SchemaExportCodecError / EvaluateSignalsError など既存 reference 型は signature 整合性のみ確認し必要に応じて修正する。FsContractMapWriter など reference adapter は TypeGraph 形式変更に合わせて最低限の呼び出し側修正を行う (renderer の詳細化は OS-06 でスコープ外)。cargo make ci-rust (fmt-check + clippy + nextest + deny + check-layers) が通るよう Rust CI を緑にする。verify-* catalogue 整合性ゲート (verify-catalogue-spec-refs / check-catalogue-spec-signals) は T003 以降の *-types.json 旧フォーマットに対して新 codec が読めないため T009 完了後に確認する。

### S6 — bin/sotp rebuild + 現 track catalogue 書き換え + CI ゲート確認

> T009 は新 schema 実装完了後に bin/sotp を rebuild し、現 branch の track/items/tddd-v2-2026-05-08/ の *-types.json を新 CatalogueDocument 形式 (3-BTreeMap) に書き換える。
> 旧 TypeDefinitionKind / payload_types: Vec<String> 形式の catalogue JSON は新 schema codec で読めないため全 catalogue ファイルを書き換える (no-backward-compat: backward compat 移行 layer は導入しない、CN-09)。
> TypeKindV2 informal_grounds を spec_refs へ昇格させ、型カタログの信号機評価の yellow を解消する。
> cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) が pass することで AC-10 を充足する。
> S5 完了後に着手する。

- [ ] **T009**: bin/sotp を rebuild し (cargo make build-sotp)、新 catalogue schema / signal evaluator の実装が完了した状態で現 branch の track item の TDDD 信号が整合することを確認する。track/items/tddd-v2-2026-05-08/ の既存 catalogue JSON (*-types.json) を新 CatalogueDocument schema (3-BTreeMap 形式) に書き換える。旧 payload_types: Vec<String> 形式 / TypeDefinitionKind ベース形式を新 CatalogueDocument 形式に変換する (後方互換なし: 新 codec 専用)。TypeKindV2 の informal_grounds を spec_refs へ昇格させる (IN-01 / AC-01 の spec element との対応を追加)。cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) が pass することを確認する (AC-10)。
