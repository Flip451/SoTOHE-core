<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# TDDD v2 — catalogue layer schema / rustdoc_types::Crate hybrid TypeGraph / 3-way diff 信号評価器の実装

## Tasks (17/17 resolved)

### S1 — 新 CatalogueDocument schema — domain 型 (newtype 系 + Role/Action/Pattern 軸分離)

> T001 は newtype 系 12 種 (Identifier base + 各 newtype) と Role/Action の primitive enum および Layer 軸に使用する LayerId newtype を実装する。T002 はこれを基盤として複合構造 (TypeKindV2 / CompositePattern / VariantPayload / エントリ群 / CatalogueDocument) を実装する。
> ADR 1 D1-D12 の型レベル制約 (TypeEntry に ContractRole を付けようとすると parse 段階で reject / members×kind の制約 schema 構造 encode / VariantPayload 3 variant) を CatalogueDocument 実装に折り込む。
> 旧型 (TypeDefinitionKind 等) はこのフェーズでは削除しない。T003 の codec 切り替え後に T008 で一括削除する。<500 行 / commit を維持するため newtype+primitive と複合構造を T001/T002 に 2 分割している。

- [x] **T001**: 新 catalogue schema domain 型 (newtype 系 + Role 3 enum + ItemAction + SelfReceiver + LayerId) を実装する。Identifier (共通 base validation) / TypeName / TraitName / FieldName / MethodName / ParamName / VariantName / CrateName / FunctionName / ModulePath / TypeRef / FunctionPath の 12 newtype に Display / Serialize / Deserialize / FromStr を実装する。DataRole (13 値) / ContractRole (3 値) / FunctionRole (2 値) / ItemAction (4 値) / SelfReceiver (3 値) を enum として実装する。Layer 軸は ADR `2026-04-17-1528-tddd-contract-map` §D1 で定義済みの `LayerId` newtype (architecture-rules.json 駆動) を使用する (固定 3 値 enum は導入しない)。libs/domain/src/tddd/catalogue.rs 内の新 tddd::catalogue_v2 (または tddd/catalogue/ サブモジュール) に配置する。ラウンドトリップ unit test (AC-03) を追加する。この時点では旧型 (TypeDefinitionKind 等) はまだ残す。 (`ba7d0eb2574049186b9478ffb62b0e62b07a3084`)
- [x] **T002**: 新 catalogue schema domain 型 (複合構造: TypeKindV2 / VariantPayload / VariantDecl / FieldDecl / TypestateMarker / MethodDeclaration / ParamDeclaration 修正 / TraitImplDeclV2 / TypeEntry / TraitEntry / FunctionEntry / CatalogueDocument + CatalogueDocumentError) を実装する。TypeKindV2 (UnitStruct / TupleStruct { fields: Vec<TypeRef>, has_stripped_fields } / PlainStruct { fields: Vec<FieldDecl>, has_stripped_fields, typestate: Option<TypestateMarker> } / Enum / TypeAlias の 5 flat variant payload-encoded) / VariantPayload (Unit/Tuple/Struct) / VariantDecl (name+payload, serde default=Unit) / FieldDecl / TraitImplDeclV2 (identity-only) を実装する。既存 ParamDeclaration / MethodDeclaration を newtype フィールド (ParamName/TypeRef/MethodName/SelfReceiver) に修正する。TypeEntry / TraitEntry / FunctionEntry / CatalogueDocument (3-BTreeMap + validation) / CatalogueDocumentError を実装する。crate_name とファイル名一致 validation を実装する。serde ラウンドトリップ unit test (AC-01 / AC-02 / AC-03 の新構造部分) を追加する。 (`1efb5b6bdd0d91e29e2136644fc291e60ddfc1e3`)

### S2 — 新 CatalogueDocument serde codec — infrastructure 層 (CatalogueLoader 更新含む)

> T003 は新 CatalogueDocument 専用 codec を infrastructure 層に実装し、旧 TypeCatalogueDocument 直接読み込み経路を削除する (no-backward-compat 原則)。
> FsCatalogueLoader の内部実装を v3 CatalogueDocument 読み込みに切り替える。CatalogueLoader port の返り値型は BTreeMap<LayerId, TypeCatalogueDocument> を維持し、infrastructure 側で v3_doc_to_stub 変換を行う (CatalogueLinter との互換性保持)。
> CatalogueLoader port は domain-types.json に宣言されている (既存の tddd-contract-map-phase1 catalogue から踏襲した domain 層配置)。FsCatalogueLoader adapter が infrastructure-types.json に宣言される。
> S1 (domain 型 T001/T002) の完了後に着手する。

- [x] **T003**: 新 catalogue schema の infrastructure 層 serde codec を実装する。CatalogueDocumentCodecError (Json / UnsupportedSchemaVersion / InvalidEntry / CrateNameMismatch) を infrastructure 層に実装する。TypeCatalogueDocument / TypeCatalogueEntry / TypeDefinitionKind を読む旧 serde 経路を削除し (no-backward-compat 原則)、新 CatalogueDocument 専用 codec を infrastructure/src/tddd/catalogue_document_codec.rs に実装する。FsCatalogueLoader (infrastructure) の内部実装を v3 CatalogueDocument 読み込みに切り替え、v3_doc_to_stub で CatalogueDocument → TypeCatalogueDocument stub 変換を行い CatalogueLoader port の返り値 BTreeMap<LayerId, TypeCatalogueDocument> に詰めて返す (CatalogueLoader port の返り値型は既存の BTreeMap<LayerId, TypeCatalogueDocument> のまま維持 — 呼び出し元 CatalogueLinter が TypeCatalogueDocument を要求するため互換性を保持)。TypeCatalogueCodecError は既存 pre-migration track ディレクトリ用の v3-first/v2-fallback 経路の reference として残存する (新規 v2 ファイルは作成しない)。codec unit test (AC-01 の serde round-trip / CrateNameMismatch / UnsupportedSchemaVersion) を追加する。 (`c7dbcbc152f045c8635c53d5c3069d5eb65879fd`)

### S3 — ExtendedCrate schema + Catalogue → TypeGraph A codec (domain port + infrastructure adapter)

> T004 は ExtendedCrate schema (domain 層) と BaselineRustdocCodec / CatalogueToExtendedCratePort を実装する。T005 は syn 依存の TypeRef parse + external_crates auto-build + inline→id 参照変換 + Catalogue→ExtendedCrate codec (infrastructure 層 adapter) を実装する。
> T004 と T005 は依存関係があるため逐次実施 (T004 完了 → T005 着手)。
> TypeRef parse は syn::parse_str::<syn::Type> を使い自前 tokenizer は書かない (CN-08)。未解決マーカーは open-world で保持し、closed-world 検証は T006 Phase 1 で実施 (CN-06)。
> S1/S2 の完了後に着手する。

- [x] **T004**: ExtendedCrate schema を domain 層に実装する。ExtendedCrate { krate: rustdoc_types::Crate, item_actions: BTreeMap<Id, ItemAction> } を libs/domain/src/tddd/ 内 (例: extended_crate.rs) に実装する。BaselineRustdocCodecError (Json/IoError/UnsupportedFormatVersion) を infrastructure 層に実装し、rustdoc_types::Crate JSON をロードする concrete deserializer を infrastructure internal implementation (catalogue に track する named adapter contract なし) として実装する。CatalogueToExtendedCratePort (domain 層 secondary_port) と NewTypeGraphCodecError (domain 層 error_type) を実装する。CatalogueToExtendedCrateCodecError (infrastructure 層 error_type) を実装する。AC-04 の ExtendedCrate unit test を追加する。 (`bad65779e39fef3e6a7a143c5492888367c8c2a0`)
- [x] **T005**: Catalogue → ExtendedCrate (TypeGraph A) codec の core 変換ロジックを実装する。syn crate を使用した TypeRef generics parse (syn::parse_str::<syn::Type> → rustdoc_types::Type 変換) を実装する。std prelude allowlist (Vec/Option/Result/String/Box 等) の自動解決を実装する。未解決マーカー表現 (未 declare 型の保持方式) を実装する。TraitImplDeclV2.origin_crate + TypeRef crate prefix からの external_crates 自動 build (per-graph incremental crate_id 発番, crate_id==0 は自 crate) を実装する。inline → id 参照変換 (FieldDecl/VariantDecl を別 Item として index に登録し Vec<Id> 参照) を実装する。1 type = 1 Inherent Impl block の grouping を実装する。Crate.paths の [crate_name, ...module_path, item_name] 形式生成を実装する。CatalogueToExtendedCrateCodec (infrastructure 層 secondary_adapter) として CatalogueToExtendedCratePort を実装する。AC-05 / AC-06 の codec unit test (inline→id-ref 変換 / generics parse / module_path 込み paths 生成 / std prelude 自動解決 / 未解決マーカー生成) を追加する。 (`c34b14e`)

### S4 — Signal evaluator Phase 1 (S/D 構築) + Phase 2 (3-way 評価)

> T006 は Phase 1 (S/D 構築 + declare 整合性検証 + closed-world 検証) を domain 層に実装する。T007 は Phase 2 (11 領域 × signal table) を infrastructure 層 SignalEvaluatorV2 として実装する。
> T006/T007 は依存関係があるため逐次実施 (T006 完了 → T007 着手)。
> S3 の ExtendedCrate + codec が前提となるため S3 完了後に着手する。
> identity 判定基準 (types/traits=short name / functions=FunctionPath、ADR 2 D3 / ADR 3 D2) を Phase 1/2 の全処理で統一する (CN-03)。

- [x] **T006**: Signal evaluator Phase 1 (S/D 構築) を domain 層に実装する。SignalRegion (12 variant) / ThreeWaySignalKind (Skip/Blue/Yellow/Red) / ThreeWaySignal / ThreeWayEvaluationReport を domain 層に実装する (T007 の SignalEvaluatorV2 が実装する SignalEvaluatorPort の戻り値型として T006 で定義する必要がある)。SignalEvaluatorPort を domain 層に実装する (evaluate(&self, a: ExtendedCrate, b: Crate, c: Crate) -> Result<ThreeWayEvaluationReport, Phase1Error>)。Phase1Error (ActionContradiction / UnresolvedTypeRef / DanglingId) を実装する。Phase 1 アルゴリズム: B 由来全要素を S に Reference で attach (types/traits=short name / functions=FunctionPath で identity、S 内 flat incremental Id 再発番)。A の各要素を action 別 (Add/Modify/Reference/Delete) に処理して S/D を構築する。Phase 1.5 (unresolved marker closed-world 検証: Delete 後の S を universe set とし、resolve 不能なら Phase1Error) を実装する。Phase 1.6 (dangling Id 検証) を実装する。external_crates の S/D 各スコープでの再発番を実装する。AC-07 の Phase 1 unit test (action 別処理 / ActionContradiction / UnresolvedTypeRef / DanglingId) を追加する。 (`55281f5103e1e30b4c1db8aa2593b109496be385`)
- [x] **T007**: Signal evaluator Phase 2 (S/D/C 3-way 評価) を infrastructure 層に実装する。SignalRegion / ThreeWaySignalKind / ThreeWaySignal / ThreeWayEvaluationReport は T006 で domain 層に定義済み。SignalEvaluatorV2 (infrastructure 層 secondary_adapter: implements SignalEvaluatorPort) を実装し、Phase 2 の 11 領域 × signal table (ADR 3 D3) を実装する: S∩C+構造一致+action別 / S∩C+構造不一致+action別 / S\C+action別 / D∩C / D\C / C\(S∪D) の各領域を網羅する。identity 判定は types/traits=short name / functions=FunctionPath で統一する。AC-08 の Phase 2 unit test (全 11 領域の signal 判定、境界ケース含む) を追加する。 (`cdb77d020122336ac1140077c138a7d53acd60d7`)

### S5 — 旧 TypeGraph 置換 + 旧 schema 削除 + 既存コード書き換え

> T008 は旧 TypeGraph (schema.rs の TypeNode/TraitNode/FunctionNode/TraitImplEntry HashMap 独自 schema) と旧 TypeBaseline 系を削除し、TypeGraph を読む全既存コードを新形式 (rustdoc_types::Crate / ExtendedCrate) に書き換える。
> 旧型の削除タイミングを S4 完了後まで遅らせることで、S1-S4 の各 task では旧コードを壊さずに新コードを追加できる。T008 で一括置換することでレビューサーフェースを明確化する。
> Contract Map renderer / Reality View renderer の新 TypeGraph 形式への完全対応は OS-06 でスコープ外。T008 では callers が compile error にならない最小限の修正を行う。
> S4 完了後に着手する。

- [x] **T008**: 既存 TypeGraph (libs/domain/src/schema.rs の TypeNode/TraitNode/FunctionNode/TraitImplEntry HashMap ベース独自 schema) を削除し、TypeGraph を読む既存コード (tddd/consistency.rs / tddd/signals.rs / contract_map_render.rs / infrastructure/tddd/type_signals_evaluator.rs / infrastructure/tddd/type_graph_cluster.rs 等) を新 TypeGraph 形式 (rustdoc_types::Crate / ExtendedCrate) に書き換える。旧 TypeBaseline / TypeBaselineEntry / TraitBaselineEntry / FunctionBaselineEntry / TraitImplBaselineEntry (libs/domain/src/tddd/baseline.rs) を削除し、baseline の保存・ロードを rustdoc_types::Crate JSON に変更する。SchemaExportCodecError / EvaluateSignalsError など既存 reference 型は signature 整合性のみ確認し必要に応じて修正する。FsContractMapWriter など reference adapter は TypeGraph 形式変更に合わせて最低限の呼び出し側修正を行う (renderer の詳細化は OS-06 でスコープ外)。cargo make ci-rust (fmt-check + clippy + nextest + deny + check-layers) が通るよう Rust CI を緑にする。verify-* catalogue 整合性ゲート (verify-catalogue-spec-refs / check-catalogue-spec-signals) は T003 以降の *-types.json 旧フォーマットに対して新 codec が読めないため T009 完了後に確認する。 (`c7dbcbc152f045c8635c53d5c3069d5eb65879fd`)

### S6 — bin/sotp rebuild + 現 track catalogue 書き換え + CI ゲート確認 (v3 schema migration)

> T009 は新 schema 実装完了後に bin/sotp を rebuild し、現 branch の track/items/tddd-v2-2026-05-08/ の *-types.json を新 CatalogueDocument 形式 (3-BTreeMap) に書き換える。
> 旧 TypeDefinitionKind / payload_types: Vec<String> 形式の catalogue JSON は新 schema codec で読めないため全 catalogue ファイルを書き換える (no-backward-compat: backward compat 移行 layer は導入しない、CN-09)。
> TypeKindV2 informal_grounds を spec_refs へ昇格させ、型カタログの信号機評価の yellow を解消する。
> S5 完了後に着手する。

- [x] **T009**: bin/sotp を rebuild し (cargo make build-sotp)、新 catalogue schema / signal evaluator の実装が完了した状態で現 branch の track item の TDDD 信号が整合することを確認する。track/items/tddd-v2-2026-05-08/ の既存 catalogue JSON (*-types.json) を新 CatalogueDocument schema (3-BTreeMap 形式) に書き換える。旧 payload_types: Vec<String> 形式 / TypeDefinitionKind ベース形式を新 CatalogueDocument 形式に変換する (後方互換なし: 新 codec 専用)。TypeKindV2 の informal_grounds を spec_refs へ昇格させる (IN-01 / AC-01 の spec element との対応を追加)。cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) が pass することを確認する (AC-10)。

### S7 — v3 catalogue spec-link 復元 — grounding フィールド追加 (domain 型拡張)

> T010 は v3 catalogue の全エントリ種 (TypeEntry / TraitEntry / FunctionEntry) に `spec_refs` と `informal_grounds` コレクションを追加する (ADR 2026-05-11-1257 D1)。
> 既存 v2 catalogue の SpecRef / InformalGroundRef 型を domain 層で再利用し、serde default で空 Vec を許容する。
> S6 (v3 catalogue JSON 書き換え完了) を前提とし、domain 型レベルの変更から着手する。T011 の codec 拡張より先に完了させる必要がある。

- [x] **T010**: v3 catalogue の全エントリ種 (TypeEntry / TraitEntry / FunctionEntry) に grounding コレクション (`spec_refs: Vec<SpecRef>` と `informal_grounds: Vec<InformalGroundRef>`) を追加する (ADR 2026-05-11-1257 D1)。既存 v2 catalogue の SpecRef / InformalGroundRef 型を domain 層の共有モジュールから再利用する。serde default で空 Vec を許容し、フィールド省略時は空 Vec として decode する (migration smoothness)。既存 domain unit test を更新し、grounding コレクションを含む ラウンドトリップ unit test を追加する。Layer: domain。

### S8 — v3 catalogue spec-link 復元 — codec 拡張 + D4 クロス crate 検証 (infrastructure codec)

> T011 は CatalogueDocumentCodec を拡張して spec_refs / informal_grounds の encode / decode を実装する (ADR 2026-05-11-1257 D1 / D3)。T012 は codec decode 時に cross-crate function path を silent drop ではなく明示的 error にする (ADR 2026-05-11-1257 D4)。
> T011 と T012 は同じ codec ファイル上の独立した変更であり、parallel 実施が可能 (ただし T010 domain 型拡張の完了後)。
> <500 行 / commit を維持するため grounding フィールド encode/decode (T011) と cross-crate key 検証 (T012) を分割している。
> T010 完了後に着手する。

- [x] **T011**: CatalogueDocumentCodec (infrastructure/src/tddd/catalogue_document_codec.rs) を拡張し、TypeEntry / TraitEntry / FunctionEntry の `spec_refs` および `informal_grounds` フィールドを encode / decode する (ADR 2026-05-11-1257 D1 / D3)。DTO 側に SpecRefDto / InformalGroundRefDto を追加する (v2 codec の既存 DTO を共通モジュールに移動して再利用する)。常に空でもフィールドを emit する (explicit visibility)。フィールドが存在しない場合は空 Vec として decode する (migration smoothness)。round-trip unit test (spec_refs + informal_grounds の全エントリ種) を追加する。Layer: infrastructure。
- [x] **T012**: CatalogueDocumentCodec::decode で `functions` map の各キー (FunctionPath) が catalogue 自身の crate_name prefix で始まることを検証し、他 crate prefix のキーを silent drop せず `CatalogueDocumentCodecError::CrossCrateFunctionPath { key, expected_crate }` として decode error にする (ADR 2026-05-11-1257 D4 / 2026-05-08-0248 §D11 amendment)。テスト: 自 crate prefix のみのキーは成功 / 他 crate prefix のキーは明示的 error / 空の functions map は成功。Layer: infrastructure。

### S9 — v3 catalogue spec-link 復元 — refresher/loader/evaluator の dead code 除去 + 実評価有効化

> T013 は catalogue_spec_signals_refresher の v3 branch を実際の信号評価に書き換え、一律 Blue fallback と cross-crate フィルタ dead code を除去する (ADR 2026-05-11-1257 D2 / D4)。T014 は v3_doc_to_stub の grounding 複写と cross-crate フィルタ除去を行う (ADR 2026-05-11-1257 D3 / D4)。T015 は type_signals_evaluator の cross-crate フィルタ dead code を除去する (ADR 2026-05-11-1257 D4)。
> T013 / T014 / T015 は T012 の D4 codec enforcement 完了を前提とする (codec reject により全 3 フィルタが dead code 化する)。3 task は異なるファイルへの変更であり parallel 実施が可能。
> T016 は render.rs の v3 fallback を修正して Cat-Spec 列を復元する (ADR 2026-05-11-1257 D2)。S8 完了後に着手する。
> S8 完了後に着手する。

- [x] **T013**: catalogue_spec_signals_refresher.rs (infrastructure) の v3 branch を書き換え、一律 Blue fallback を除去して実際の signal 評価を行う (ADR 2026-05-11-1257 D2)。各エントリの `spec_refs` と `informal_grounds` を domain 層の `evaluate_catalogue_entry_signal(spec_refs, informal_grounds)` に渡して信号色を決定する。v3 エントリに対する「一律 Blue」doc comment ブロックを削除する。T012 により codec decode 段階で cross-crate function path が reject されるため、refresher 内の cross-crate function フィルタ (lines ~168-178 付近) は dead code となる — 削除する。テスト: Blue / Yellow / Red が混在する grounding を持つエントリが正しい信号色になることを確認する。Layer: infrastructure。
- [x] **T014**: catalogue_bulk_loader.rs (infrastructure) の `v3_doc_to_stub` 関数を更新し、CatalogueDocument → TypeCatalogueDocument stub 変換時に `spec_refs` と `informal_grounds` を stub に複写する (ADR 2026-05-11-1257 D3)。T012 により codec decode 段階で cross-crate function path が reject されるため、`v3_doc_to_stub` 内の cross-crate function フィルタ (line ~493 付近) は dead code となる — 削除する。テスト: grounding コレクションが変換後 stub に保存されることを確認する。Layer: infrastructure。
- [x] **T015**: type_signals_evaluator.rs (infrastructure, line ~250 付近) の cross-crate function フィルタを削除する (ADR 2026-05-11-1257 D4)。T012 により codec decode 段階で cross-crate path が reject されるため、このフィルタは impossible-to-trigger な dead code となる。フィルタ削除後、type-signal 評価が全エントリで uniform に動作することを unit test で確認する。Layer: infrastructure。
- [x] **T016**: track/render.rs (infrastructure) の v3 fallback を修正し、catalogue-spec-signals doc を正しくロードして `render_type_catalogue` に渡す (ADR 2026-05-11-1257 D2 — 信号の可視性)。現状は v3 fallback で `None` を渡しているため Cat-Spec 列が非表示になっている。修正後に Cat-Spec 列が rendered view に表示されることを確認する。Layer: infrastructure。

### S10 — 統合ゲート確認 — signals 再生成 + CI pass (spec-link 復元完了確認)

> T017 は bin/sotp rebuild 後に全 3 layer の type-signals / catalogue-spec-signals を再生成し、rendered view を更新して cargo make ci を pass させる (AC-10 充足)。
> T013-T016 (S9) 全完了後に着手する。CI ゲートが全通過することで spec-link 復元スコープの完了を確認する。

- [x] **T017**: 統合ゲート確認: bin/sotp を rebuild し (cargo make build-sotp)、全 3 layer の type-signals と catalogue-spec-signals を再生成する (`bin/sotp track type-signals tddd-v2-2026-05-08` × 3 + `bin/sotp track catalogue-spec-signals tddd-v2-2026-05-08` × 3)。`cargo make track-sync-views` で rendered view を再生成する。`cargo make ci` が pass することを確認する (verify-catalogue-spec-refs / check-catalogue-spec-signals を含む全ゲート通過)。AC-10 を充足する最終統合タスク。Layer: integration。
