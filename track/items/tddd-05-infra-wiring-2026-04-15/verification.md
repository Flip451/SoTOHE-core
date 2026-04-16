# Verification — TDDD-05: Secondary Adapter variant の追加 — infrastructure 層における hexagonal port 実装の検証

## Scope Verified

- [ ] T001: `libs/domain/src/tddd/catalogue.rs` に `TraitImplDecl` 新型と `TypeDefinitionKind::SecondaryAdapter { implements: Vec<TraitImplDecl> }` variant が追加されている
- [ ] T001: `kind_tag()` が `"secondary_adapter"` を返す test が pass している
- [ ] T001: `consistency.rs` の `declared_type_names` フィルタで `SecondaryAdapter` が type 区分側に分類される test が pass している
- [ ] T002: `libs/infrastructure/src/tddd/catalogue_codec.rs` に `TypeDefinitionKindDto::SecondaryAdapter` と新 DTO `TraitImplDeclDto` が追加され、decode/encode round-trip test が pass している
- [ ] T002: `EXISTENCE_ONLY_KINDS` (line 228-249) と `is_method_bearing` (line 306-307) に `secondary_adapter` が含まれていないことを test が保証している
- [ ] T003: `libs/domain/src/schema.rs` に `TraitImplEntry` 新型と `TypeNode::trait_impls` フィールドと `TypeGraph::get_impl(type_name, trait_name)` アクセサが追加されている (`schema_version` は 2 のまま維持)
- [ ] T003: `TypeGraph::get_impl` の hit / miss / wrong-trait test が pass している
- [ ] T004: `libs/infrastructure/src/code_profile_builder.rs:36` の trait 実装フィルタが解除され、trait impls が別経路で `TypeNode::trait_impls` に格納されている
- [ ] T004: 既存テスト `test_build_type_graph_with_trait_impl_excludes_outgoing` が依然として pass している (outgoing 計算は inherent only を維持)
- [ ] T004: `is_negative` フィールドの存在を rustdoc-types 0.57.3 で確認し、必要であればフィルタに追加した
- [ ] T005: `libs/domain/src/tddd/signals.rs` に `evaluate_secondary_adapter` 関数と `evaluate_impl_methods` helper が追加され、`evaluate_single` の match arm に `SecondaryAdapter` variant が追加されている
- [ ] T005: 集約 signal の test が pass している (全 trait 確認済 → Blue / struct 自体不在 → Yellow / 1 つでも未確認 → Red / 空 implements → Blue / method mismatch → Red)
- [ ] T006: `track/items/tddd-05-infra-wiring-2026-04-15/infrastructure-types.json` が 11 エントリ以上で存在する (plan 段階の `/track:design --layer infrastructure --force` で生成)
- [ ] T006: `bin/sotp track type-signals tddd-05-infra-wiring-2026-04-15 --layer infrastructure` が `blue=N (N>=11) yellow=0 red=0` を返す
- [ ] T006: `knowledge/adr/README.md` の信号機アーキテクチャ section に `2026-04-15-1636-tddd-05-secondary-adapter.md` の索引行が追加されている
- [ ] T006: `libs/domain/Cargo.toml` に serde 依存が含まれていない (Track 1 §D1 不変条件の維持、`grep 'serde' libs/domain/Cargo.toml` がゼロマッチ)
- [ ] T006: `cargo make ci` (fmt-check + clippy -D warnings + nextest + deny + check-layers + verify-spec-states + verify-arch-docs) が全通過している
- [ ] T006: 「infrastructure TDDD full production 宣言」セクションが本ファイルに記載されている
- [ ] PR review (Codex Cloud `@codex review`) で zero findings を達成している

## Manual Verification Steps

### T001: domain catalogue + `SecondaryAdapter` variant

```bash
# variant 定義の存在確認
rg "SecondaryAdapter" libs/domain/src/tddd/catalogue.rs
# expect: enum variant 定義 (SecondaryAdapter { ... }) が見つかる

rg "pub struct TraitImplDecl" libs/domain/src/tddd/catalogue.rs
# expect: 1 件

# kind_tag test
cargo nextest run -p domain test_secondary_adapter_kind_tag
# expect: pass

# consistency partition test
cargo nextest run -p domain test_consistency_partitions_secondary_adapter_as_type
# expect: pass
```

### T002: infrastructure codec

```bash
# DTO 定義の存在確認
rg "SecondaryAdapter" libs/infrastructure/src/tddd/catalogue_codec.rs
rg "struct TraitImplDeclDto" libs/infrastructure/src/tddd/catalogue_codec.rs
# expect: 各 1 件以上

# EXISTENCE_ONLY_KINDS / is_method_bearing に secondary_adapter が含まれない確認
rg 'EXISTENCE_ONLY_KINDS' libs/infrastructure/src/tddd/catalogue_codec.rs
rg 'is_method_bearing' libs/infrastructure/src/tddd/catalogue_codec.rs
# expect: secondary_adapter は含まれていない

# round-trip test + invariant test
cargo nextest run -p infrastructure test_decode_secondary_adapter
cargo nextest run -p infrastructure test_encode_secondary_adapter_round_trip
cargo nextest run -p infrastructure test_existence_only_kinds_excludes_secondary_adapter
cargo nextest run -p infrastructure test_is_method_bearing_excludes_secondary_adapter
# expect: 全 pass
```

### T003: domain schema

```bash
rg "pub struct TraitImplEntry" libs/domain/src/schema.rs
rg "trait_impls: Vec<TraitImplEntry>" libs/domain/src/schema.rs
rg "pub fn get_impl" libs/domain/src/schema.rs
# expect: 各 1 件

cargo nextest run -p domain test_type_graph_get_impl
cargo nextest run -p domain test_trait_impl_entry_accessors
# expect: 全 pass
```

### T004: infrastructure builder

```bash
# filter の確認 (outgoing 計算用 filter のみ残る、trait impls は別経路で収集)
rg "i.trait_name\(\).is_none\(\)" libs/infrastructure/src/code_profile_builder.rs
rg "i.trait_name\(\).is_some\(\)" libs/infrastructure/src/code_profile_builder.rs
# expect: outgoing 計算用 filter (is_none) は残る、新たに trait impls 収集用 filter (is_some) が追加される

cargo nextest run -p infrastructure test_build_type_graph_trait_impl_populated
cargo nextest run -p infrastructure test_build_type_graph_with_trait_impl_excludes_outgoing
cargo nextest run -p infrastructure test_build_type_graph_multiple_trait_impls_on_same_type
# expect: 全 pass (新規 2 件 + 既存 1 件)

# is_negative フィールドの存在確認 (rustdoc-types 0.57.3)
# Impl 構造体に is_negative: bool が存在するかを確認する
rg "is_negative" ~/.cargo/registry/src/*/rustdoc-types-0.57.*/src/lib.rs
# expect: フィールド定義が見つかれば存在確認完了 (schema_export.rs のフィルタに追加が必要)
# expect: 見つからなければ存在しない (フィルタ追加不要)
```

### T005: domain evaluator

```bash
rg "fn evaluate_secondary_adapter" libs/domain/src/tddd/signals.rs
rg "fn evaluate_impl_methods" libs/domain/src/tddd/signals.rs
# expect: 各 1 件

# evaluate_single の match arm
rg "TypeDefinitionKind::SecondaryAdapter" libs/domain/src/tddd/signals.rs
# expect: 1 件以上 (match arm)

cargo nextest run -p domain evaluate_secondary_adapter
# expect: 全 pass (blue / yellow / red / empty implements / method mismatch)
```

### T006: track 完了化と最終 CI

```bash
# infrastructure-types.json の作成:
# Claude Code で `/track:design --layer infrastructure --force` を実行する (bash command ではなく Claude slash command)。
# T001-T002 完了後に .claude/commands/track/design.md に SecondaryAdapter variant 定義を追加してから実行すること。
# 代替: orchestrator が ADR を参照して infrastructure-types.json を手動作成しても良い。

# entry 数の確認
jq '.type_definitions | length' track/items/tddd-05-infra-wiring-2026-04-15/infrastructure-types.json
# expect: >= 11

# signal 確認
bin/sotp track type-signals tddd-05-infra-wiring-2026-04-15 --layer infrastructure
# expect: blue=N (N>=11) yellow=0 red=0

# ADR README 索引補完の確認
rg "2026-04-15-1636-tddd-05-secondary-adapter" knowledge/adr/README.md
# expect: 1 件

# Track 1 §D1 不変条件 (domain serde ゼロ)
rg "serde" libs/domain/Cargo.toml
# expect: ゼロマッチ

# 最終 CI
cargo make ci
# expect: fmt-check / clippy -D warnings / nextest / deny / check-layers / verify-spec-states / verify-arch-docs 全 pass
```

## Result

### T001 (実装後に記入)

(未着手)

### T002 (実装後に記入)

(未着手)

### T003 (実装後に記入)

(未着手)

### T004 (実装後に記入)

(未着手)

### T005 (実装後に記入)

(未着手)

### T006 (実装後に記入)

(未着手)

## Verified At

(track 完了時に UTC タイムスタンプを記入: `date -u +"%Y-%m-%dT%H:%M:%SZ"`)

---

## infrastructure TDDD full production 宣言

(T006 完了時に追記する。Track 2 完了をもって infrastructure 層の TDDD は full production 運用化が完成し、`SecondaryAdapter` variant により hexagonal secondary port 実装が catalogue ベースで検証可能となる。)

## 後続トラックへの引継ぎ事項

(T006 完了時に記入する。例:)

1. **`tddd-06-cli-wiring`**: cli 層の TDDD 拡張 + infrastructure 内部 trait (`GitRepository`, `GhClient`) の扱い
2. **`tddd-rustdoc-cache`**: Phase D で計測した rustdoc 時間が許容外なら別 sub-track を起こす
3. **`is_negative` フィールドの確認結果**: T004 実装中に rustdoc-types 0.57.3 で確認した結果を記録する
4. **`SecondaryAdapter` の variant 固有検証ルール強化**: 将来 reassess 時の検討事項
