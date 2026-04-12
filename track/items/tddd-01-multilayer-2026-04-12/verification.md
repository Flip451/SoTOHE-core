# Verification — TDDD-01: 型カタログ多層化 + L1 シグネチャ検証

## Scope Verified

- [ ] `libs/domain/src/tddd/catalogue.rs` が `catalogue.rs` / `signals.rs` / `consistency.rs` の 3 モジュールに分割されている (各 ≤400 行)
- [ ] `DomainType*` シンボルが `TypeDefinition*` / `TypeCatalogue*` / `TypeSignal` / `evaluate_type_signals` / `check_type_signals` に全置換されている。検証 grep は path scoped: `rg 'DomainType|domain_types|domain-type-signals' libs apps .claude Makefile.toml knowledge/adr knowledge/strategy` で残存ゼロ (履歴 track `track/items/**` と frozen research snapshot `knowledge/research/**` は対象外)
- [ ] C1 breaking change が適用されている: `rg 'signature: String|pub fn signature|fn signature\(|format_sig|FunctionInfo.*signature' libs/` がゼロ件を返し、`FunctionInfo::signature: String` フィールド・`pub fn signature()` アクセサ・`format_sig` 関数・`FunctionInfo` の `signature` 引数がすべて削除されている (struct field 残存もカバー)
- [ ] `MethodDeclaration` / `ParamDeclaration` / `MemberDeclaration` が domain 層で定義され、`FunctionInfo` / `TypeNode` / `TraitNode` / `TypeBaselineEntry` / `TraitBaselineEntry` の 5 箇所で共有されている
- [ ] `TypeDefinitionKind::TraitPort { expected_methods: Vec<MethodDeclaration> }` が L1 forward/reverse check を実装している
- [ ] `architecture-rules.json` の `layers[]` に `tddd` ブロックが追加され、`sotp track type-signals` が `--layer` flag を受け付ける
- [ ] `verify_from_spec_json` と `check_strict_merge_gate` が全 `tddd.enabled` 層を AND 集約する
- [ ] merge gate U19-U26 テスト (2-layer 組み合わせ) が pass する
- [ ] `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` Status が `Accepted` に更新されている

## Manual Verification Steps

```bash
# 1. CI 全項目の確認
cargo make ci

# 2. リネーム残存確認
rg 'DomainType|domain_types\b|domain-type-signals|check_domain_types_signals|evaluate_domain_type_signals' libs apps .claude Makefile.toml

# 3. 新規型の domain 層配置確認
rg -l 'struct MethodDeclaration|struct ParamDeclaration|enum MemberDeclaration' libs/domain/src/tddd/

# 4. 多層 CLI の動作確認 (track 内で実施)
#    4a. 単層: domain のみ
sotp track type-signals tddd-01-multilayer-2026-04-12 --layer domain

#    4b. 全層ループ (domain + usecase)
sotp track type-signals tddd-01-multilayer-2026-04-12

#    4c. baseline-capture の layer 対応
sotp track baseline-capture tddd-01-multilayer-2026-04-12 --layer domain
sotp track baseline-capture tddd-01-multilayer-2026-04-12 --layer usecase

# 5. verify spec-states が全層 catalogue を AND 集約
sotp verify spec-states track/items/tddd-01-multilayer-2026-04-12/spec.md

# 6. v1 baseline 検出時のエラー確認
echo '{"schema_version": 1, "captured_at": "2026-04-12T00:00:00Z", "types": {}, "traits": {}}' > /tmp/v1-baseline.json
cp /tmp/v1-baseline.json track/items/tddd-01-multilayer-2026-04-12/domain-types-baseline.json
sotp track baseline-capture tddd-01-multilayer-2026-04-12 --layer domain
# → UnsupportedSchemaVersion(1) エラーと再実行手順が表示されることを確認
# → 確認後、正規の baseline を再キャプチャ: sotp track baseline-capture tddd-01-multilayer-2026-04-12 --layer domain

# 7. L1 forward check の実機確認 (T006 後)
#    意図的に params の型を primitive に変えて Yellow になるか確認
```

## Result / Open Issues

- Phase 2 (L2 generics, cross-layer 参照検証) は本 track の対象外 — 将来の follow-up track で対応
- CI 時間が増加する可能性 (tddd.enabled 層ごとに `cargo +nightly rustdoc`) — キャッシュ戦略の見直しは follow-up として記録
- `async-trait` proc-macro で desugar されたメソッドは L1 上 `is_async=false` として扱われる既知の制約 (ADR Consequences で言及済み)
- `MemberDeclaration` を `#[serde(untagged)]` enum-first にしたため、baseline JSON では enum variant は plain string `"VariantName"`、struct field は `{"name": "field_name", "ty": "TypeName"}` オブジェクトとして直列化される (developer が手で読む際はバリアント = 文字列 / フィールド = オブジェクトで識別する)

## verified_at

- 未検証 (planned status)
