<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# domain-types.json — typed domain type registry (spec.json から分離)

spec.json から domain_states を切り出し、独立ファイル domain-types.json として新設する。
DomainTypeKind enum で型カテゴリ (typestate/enum/value_object/error_type/trait_port) を表現。
各カテゴリは固有の検証データを持ち、信号評価は Blue/Red 2値。
spec は「何を作るか」(要件)、domaintypes は「何が実装されているか」(型宣言) — ライフサイクルが異なるため分離。
SchemaExport (BRIDGE-01) を optional 入力として受け取り、enum variant / trait メソッドの検証を可能にする。

## Domain 層: 新型定義 + 評価ロジック

DomainTypeKind enum: Typestate{transitions_to}, Enum{expected_variants}, ValueObject, ErrorType{expected_variants}, TraitPort{expected_methods}
DomainTypeEntry: name + description + kind
DomainTypeSignal: type_name + kind_tag + signal (Blue/Red) + found_type + found_items + missing_items + extra_items
DomainTypesDocument: schema_version + entries + signals (domain-types.json の domain 表現)
evaluate_domain_type_signals(): kind ごとの Blue/Red 2値判定。CodeScanResult + Optional SchemaExport
SpecDocument から domain_states / domain_state_signals を削除
信号ルール — Blue: spec と code が完全一致。Red: それ以外全て

- [ ] Domain: DomainTypeKind enum + DomainTypeEntry + DomainTypeSignal 型定義。DomainStateEntry/DomainStateSignal を削除し置換
- [ ] Domain: DomainTypesDocument 型定義 (domain-types.json の domain 表現)。schema_version + entries + signals を保持
- [ ] Domain: evaluate_domain_type_signals() — kind ごとの Blue/Red 2値評価ロジック。CodeScanResult + Optional SchemaExport
- [ ] Domain: SpecDocument から domain_states / domain_state_signals フィールドを削除。関連アクセサ削除

## Infrastructure 層: 新 codec/renderer + 既存削除

domain-types.json 用 codec 新設: DomainTypeKindDto (serde tag = kind), schema_version 1
domain-types.md renderer 新設: Domain Types テーブル + kind ごとの Details 列
spec.json codec から domain_states / domain_state_signals を削除
render_spec() から Domain States テーブルを削除
verify: domain-types.json 読み込み + Blue/Red ゲート

- [ ] Infrastructure: domain-types.json 用 codec 新設 (encode/decode)。DomainTypeKindDto (serde tag = kind)。schema_version 1。transitions_to 参照整合性チェックは Typestate kind のみ
- [ ] Infrastructure: domain-types.md renderer 新設。Domain Types テーブル + kind ごとの Details 列 (遷移/variant/メソッド) + 信号表示
- [ ] Infrastructure: spec.json codec から domain_states / domain_state_signals を削除。spec.json content hash から domain_states を除去
- [ ] Infrastructure: render_spec() から Domain States テーブル表示を削除
- [ ] Infrastructure: verify spec_states.rs を domain-types.json 読み込みに切替。Blue/Red 2値ゲート

## CLI + マイグレーション + ドキュメント

CLI: domain-type-signals コマンド新設 (domain-types.json 読み書き)
CLI: sotp track views sync に domain-types.md 生成を追加
既存 spec.json から domain_states を抽出し domain-types.json を生成
DESIGN.md + ADR 更新

- [ ] CLI: domain-type-signals コマンド新設 (domain-types.json を読み書き)。旧 domain-state-signals 廃止
- [ ] CLI: sotp track views sync に domain-types.md 生成を追加。track-sync-views で自動レンダリング
- [ ] 既存 track の spec.json から domain_states を抽出し domain-types.json を生成。spec.json から domain_states/domain_state_signals を削除
- [ ] DESIGN.md + ADR 更新: domain-types.json 分離 + DomainTypeKind の設計決定を記録
