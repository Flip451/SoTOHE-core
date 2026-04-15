# Verification — domain-serde-ripout: hexagonal 純粋性回復 + infrastructure 層 TDDD partial dogfood (Track 1 of 2)

## Scope Verified

- [x] T001: infrastructure 層 rustdoc viability audit が成功し、wall time が記録されている (collision warning 有無は plain rustdoc では検出不可、T002 の baseline-capture 結果を参照)
- [x] T001: `architecture-rules.json` の infrastructure tddd が `enabled: true` に flip され、`catalogue_file` / `schema_export.targets` が設定されている
- [x] T002: `/track:design --layer infrastructure` が成功し、`track/items/domain-serde-ripout-2026-04-15/infrastructure-types.json` (9 entries after T003 correction — 6 dto + 2 enum + 1 error_type) + `infrastructure-types-baseline.json` + `infrastructure-types.md` rendered view が生成されている
- [x] T002: `bin/sotp track type-signals domain-serde-ripout-2026-04-15 --layer infrastructure` が `blue=0 yellow=9 red=0` (初期状態、DTO 未実装、T003 で 9 entries に補正後) を返している
- [x] T002: `track/items/domain-serde-ripout-2026-04-15/` 配下に `domain-types.json` / `usecase-types.json` が存在しない (per-layer opt-out)
- [x] T003: `libs/infrastructure/src/schema_export_codec.rs` が新規作成され、9 pub types (6 structs + 2 enums + 1 error_enum) + 8 `From` 実装 + `pub fn encode()` が定義されている
- [x] T003: `libs/infrastructure/src/lib.rs` に `pub mod schema_export_codec;` が追加されている
- [x] T003: unit test (5 ケース: empty schema / 1 type-function-trait-impl / pretty vs compact / externally-tagged MemberDeclaration / PascalCase TypeKind) が全通過している
- [x] T003: `type-signals --layer infrastructure` が `blue=9 yellow=0 red=0` に遷移している (yellow=9 → blue=9)
- [x] T004: `libs/domain/src/schema.rs` から `use serde::Serialize;` と 6 derive が削除されている
- [x] T004: `libs/domain/src/tddd/catalogue.rs` から `use serde::Serialize;` と 3 derive (MethodDeclaration の dead code 含む) が削除されている
- [x] T004: `libs/domain/Cargo.toml` から `serde` 依存が削除されている
- [x] T004: `apps/cli/src/commands/domain.rs::export_schema()` が `infrastructure::schema_export_codec::encode()` 経由に書き換えられている
- [x] T004: `libs/infrastructure/src/schema_export_tests.rs` が `schema_export_codec::encode` 経由のテストに更新されている
- [x] T004: `cargo make export-schema -- --crate domain --pretty` の出力 JSON が変更前と structural に同一であることを手動 diff で確認済み
- [x] T004: `type-signals --layer infrastructure` が依然として `blue=9` を維持している (serde 削除による回帰なし)
- [x] T005: `knowledge/adr/README.md` の信号機アーキテクチャ section に本 ADR + 未登録 2 ADR が索引追加されている
- [x] T005: Track 2 引継ぎ事項セクションの 5 項目が埋まっている
- [x] `cargo make ci` 全通過 (fmt-check + clippy -D warnings + test + deny + check-layers + verify-spec-states + verify-arch-docs)

## Manual Verification Steps

### 1. rustdoc viability audit + architecture-rules.json flip (T001)

```bash
# infrastructure rustdoc が成功すること
cargo +nightly rustdoc -p infrastructure --target-dir target/rustdoc-audit -- -Z unstable-options --output-format json
# expect: success, JSON 生成
# 計測: wall time を verification.md の「Track 2 引継ぎ事項」に記録

# architecture-rules.json infrastructure tddd 設定
# 変更前: "tddd": { "enabled": false }
# 変更後: "tddd": { "enabled": true, "catalogue_file": "infrastructure-types.json", "schema_export": {...} }
# verify spec-states が infrastructure layer を認識するが catalogue 不在のため skip PASS
cargo make ci    # expect: all pass (stage 2 skips infrastructure because catalogue not created yet)
```

### 2. /track:design --layer infrastructure (T002)

```bash
# /track:design 起動 (claude subagent で designer capability が実行)
/track:design --layer infrastructure

# 自動生成ファイルの存在確認
ls track/items/domain-serde-ripout-2026-04-15/infrastructure-types.json
ls track/items/domain-serde-ripout-2026-04-15/infrastructure-types-baseline.json
ls track/items/domain-serde-ripout-2026-04-15/infrastructure-types.md

# catalogue の entry 数
jq '.type_definitions | length' track/items/domain-serde-ripout-2026-04-15/infrastructure-types.json   # expect: 9 (after T003 catalogue correction from 8 → 9 entries)
jq '.schema_version' track/items/domain-serde-ripout-2026-04-15/infrastructure-types.json              # expect: 2

# 全 entry が approved=true
jq '.type_definitions[] | select(.approved == false)' track/items/domain-serde-ripout-2026-04-15/infrastructure-types.json
# expect: empty

# 初期 signal (DTO 未実装時点)
bin/sotp track type-signals domain-serde-ripout-2026-04-15 --layer infrastructure
# expect: blue=0 yellow=9 red=0 (total=9) — after T003 catalogue correction from 8 → 9 entries

# per-layer opt-out 確認
ls track/items/domain-serde-ripout-2026-04-15/domain-types.json        # expect: not found
ls track/items/domain-serde-ripout-2026-04-15/usecase-types.json       # expect: not found

# Stage 2 spec-states は per-layer で NotFound → skip なので PASS
cargo make ci    # expect: all pass (infrastructure が yellow=9 でも strict merge gate 前は interim allowed)
```

### 3. schema_export_codec 新設 (T003)

```bash
# DTO + encode 関数の存在
rg 'pub fn encode\(schema: &SchemaExport' libs/infrastructure/src/schema_export_codec.rs   # expect: 1 match
rg 'struct SchemaExportDto' libs/infrastructure/src/schema_export_codec.rs                  # expect: 1 match
rg 'enum TypeKindDto' libs/infrastructure/src/schema_export_codec.rs                        # expect: 1 match
rg 'enum MemberDeclarationDto' libs/infrastructure/src/schema_export_codec.rs               # expect: 1 match
rg 'impl From<&SchemaExport> for SchemaExportDto' libs/infrastructure/src/schema_export_codec.rs  # expect: 1 match

# lib.rs 公開
rg 'pub mod schema_export_codec' libs/infrastructure/src/lib.rs                             # expect: 1 match

# unit test
cargo test -p infrastructure --lib schema_export_codec                                       # expect: all pass

# signal 遷移: yellow=9 → blue=9 (after T003 catalogue correction 8 → 9 entries)
bin/sotp track type-signals domain-serde-ripout-2026-04-15 --layer infrastructure
# expect: blue=9 yellow=0 red=0
```

### 4. domain serde 除去 (T004)

```bash
# 旧 derive がゼロ件
rg 'use serde' libs/domain/src/                                # expect: zero matches
rg 'derive.*Serialize' libs/domain/src/                        # expect: zero matches
rg 'derive.*Deserialize' libs/domain/src/                      # expect: zero matches

# Cargo.toml から serde 削除
rg 'serde' libs/domain/Cargo.toml                              # expect: zero matches

# CLI 書き換え
rg 'serde_json::to_string.*&schema' apps/cli/src/commands/domain.rs   # expect: zero matches
rg 'schema_export_codec::encode' apps/cli/src/commands/domain.rs       # expect: 1 match

# schema_export_tests.rs 更新
rg 'serde_json::to_string\(&schema\)' libs/infrastructure/src/schema_export_tests.rs   # expect: zero matches
rg 'schema_export_codec::encode' libs/infrastructure/src/schema_export_tests.rs        # expect: at least 1 match

# JSON 出力の互換性 (手動 diff)
cargo make export-schema -- --crate domain --pretty > /tmp/after.json
# 変更前出力 (T003 直後の状態) と diff 取って structural に一致することを確認

# 回帰確認: infrastructure signal 維持
bin/sotp track type-signals domain-serde-ripout-2026-04-15 --layer infrastructure
# expect: blue=9 yellow=0 red=0 (serde 削除による回帰なし — build_type_graph が trait impls を除外)
```

### 5. ADR README index + verification 完了 (T005)

```bash
# README 索引の補完確認
rg 'domain-serde-ripout' knowledge/adr/README.md              # expect: 1 match (本 ADR index 追加)
rg 'tddd-taxonomy-expansion' knowledge/adr/README.md          # expect: 1 match (未登録 ADR 追加)
rg 'finding-taxonomy-cleanup' knowledge/adr/README.md         # expect: 1 match (未登録 ADR 追加)

# ADR の存在と Status
ls knowledge/adr/2026-04-14-1531-domain-serde-ripout.md       # expect: 1 file
rg 'Status\s*\n\nAccepted' knowledge/adr/2026-04-14-1531-domain-serde-ripout.md  # expect: match

# verification.md Track 2 引継ぎ事項が埋まっている
grep -A 2 "Track 2 引継ぎ事項" track/items/domain-serde-ripout-2026-04-15/verification.md
# expect: 5 sub-items filled with actual values
```

### 6. 最終 CI 全通過

```bash
cargo make ci
# expect: fmt-check / clippy -D warnings / test / deny / check-layers / verify-spec-states / verify-arch-docs 全通過
```

## Result

### T001 (2026-04-14)

- **rustdoc viability audit**: success after 6 prereq doc fixes. `target/rustdoc-audit/doc/infrastructure.json` produced, size 1,360,827 bytes (1.3 MB).
- **wall time**: initial failed audit took ~6.0s (errored out on first warning). Warm re-runs complete in ~0.7s once cached. Clean-slate cold rustdoc for infrastructure is expected to run in single-digit seconds (exact cold timing not measured in this audit because the docs target was warm from prior cargo runs).
- **prereq doc fixes applied** (6 files, 1 line each):
  1. `libs/infrastructure/src/review_v2/hasher.rs:15` — wrapped `"rvw1:sha256:<hex>"` in backticks (HTML tag false-positive fix).
  2. `libs/infrastructure/src/tddd/baseline_codec.rs:9` — wrapped `Vec<String>` in backticks.
  3. `libs/infrastructure/src/verify/merge_gate_adapter.rs:6` — `[`crate::git_cli::show`]` → `` `crate::git_cli::show` `` (private intra doc link → inline code).
  4. `libs/infrastructure/src/verify/trusted_root.rs:42` — `[`ensure_not_symlink_root`]` → `` `ensure_not_symlink_root` ``.
  5. `libs/infrastructure/src/verify/domain_purity.rs:4` — `[`super::usecase_purity::check_layer_purity`]` → `` `super::usecase_purity::check_layer_purity` ``.
  6. `libs/infrastructure/src/shell/flatten.rs:3` — `[`super::conch`]` → `` `super::conch` ``.
- **collision warning**: not checked in T001 (rustdoc JSON itself does not invoke `build_type_graph`). Collision detection is deferred to T002 where `/track:design --layer infrastructure` internally runs `baseline-capture` → `build_type_graph`.
- **architecture-rules.json flip**: `infrastructure.tddd` changed from `{"enabled": false}` to `{"enabled": true, "catalogue_file": "infrastructure-types.json", "schema_export": {"method": "rustdoc", "targets": ["infrastructure"]}}`.
- **`cargo make ci` after flip**: PASS (Build Done in 16.73s). fmt-check / clippy -D warnings / nextest (1940 tests) / deny / check-layers / verify-spec-states / verify-* all PASSED. Warnings are pre-existing (module size, pub String fields) and not introduced by this task.

### T002 (2026-04-14, commit 800bddf — initial; corrected during T003)

- **`/track:design --layer infrastructure`** の Step 2-4 を順次実行。初期カタログは 8 entries で commit したが、T003 実装時点で 9 entries に補正した (下記「T002 scope 補正ノート」参照)。
  - **Step 2 (type design, as committed in 800bddf)**: 8 entries designed — 7 × `dto` kind (`SchemaExportDto`, `TypeInfoDto`, `FunctionInfoDto`, `TraitInfoDto`, `ImplInfoDto`, `MemberDeclarationDto`, `SchemaParamDto`) + 1 × `error_type` kind (`SchemaExportCodecError` with `expected_variants: ["Json"]`). All `approved: true`, `action` omitted (= `add` default). `TypeKindDto` intentionally excluded (planned to be private).
  - **Step 3 (write catalogue)**: `track/items/domain-serde-ripout-2026-04-15/infrastructure-types.json` created with schema_version 2 + 8 entries (initial).
  - **Step 4.1 (baseline-capture)**: `bin/sotp track baseline-capture domain-serde-ripout-2026-04-15 --layer infrastructure --force` → wrote `infrastructure-types-baseline.json` containing **38 types + 2 traits**. No `same-name type collision for X` warning from `build_type_graph`.
  - **Step 4.2 (type-signals, initial 8-entry state)**: `bin/sotp track type-signals domain-serde-ripout-2026-04-15 --layer infrastructure` → `blue=0 yellow=8 red=0 (total=8, undeclared=0, skipped=40)`. All 8 declared DTOs are Yellow because the code is not yet present (T003 will implement them). All 38 existing infrastructure types and 2 traits are in `B\A` and correctly classified as `skipped` (unchanged structure; 38 + 2 = 40 skipped total).
- **Collision audit (deferred from T001)**: baseline-capture emitted no `same-name type collision` warnings on stderr. Infrastructure crate has no rustdoc-visible same-name type collisions as of this track.
- **per-layer opt-out confirmed**: `track/items/domain-serde-ripout-2026-04-15/` contains no `domain-types.json` and no `usecase-types.json`. `spec_states.rs::evaluate_layer_catalogue` skips both layers at Stage 2. Stage 2 spec-states PASS.
- **`cargo make ci` (T002 commit時)**: PASS (Build Done in 11.86s). `verify-spec-states` emits the expected interim warning listing the 8 Yellow types.

#### T002 scope 補正ノート (T003 commit 時に判明)

T003 実装時点で上記 8 entries のうち 2 点が catalogue の kind と不一致であることが type-signals の出力から判明した:

1. **`MemberDeclarationDto` was Red (kind mismatch)**: T002 catalogue declared it as `kind: "dto"`, but the actual Rust type is an `enum` with two variants (`Variant(String)` and `Field { name, ty }`). The TDDD `dto` kind expects a Rust `struct` and emits Red for enum-shaped declarations. Fix: change to `kind: "enum"` with `expected_variants: ["Variant", "Field"]`.
2. **`TypeKindDto` was undeclared Red**: T002 catalogue intentionally excluded `TypeKindDto` on the assumption it could stay private. However, `TypeKindDto` is used as the type of the public field `TypeInfoDto::kind`, and Rust E0446 (private type in public interface) forces it to be `pub`. As a result, it shows up in the rustdoc public surface as an undeclared type → Red. Fix: add a 9th catalogue entry with `kind: "enum"` and `expected_variants: ["Struct", "Enum", "TypeAlias"]`.

Net result: catalogue expanded from 8 → 9 entries (6 × `dto` + 2 × `enum` + 1 × `error_type`). All 9 entries are declared `pub` in the Rust implementation, and `type-signals --layer infrastructure` transitions `yellow=9 → blue=9` after T003's implementation. The correction is persisted in `infrastructure-types.json`, metadata.json T002 plan description, spec.json, and ADR §D3 / §D8 in the T003 commit. The T002 commit itself (800bddf) is left unchanged — it represents the 8-entry state at that point in history.

### T003 (2026-04-14)

- **`libs/infrastructure/src/schema_export_codec.rs`** 新規作成 (~330 行 including unit tests).
  - 9 pub types defined: 6 structs (`SchemaExportDto`, `TypeInfoDto`, `FunctionInfoDto`, `TraitInfoDto`, `ImplInfoDto`, `SchemaParamDto`) + 2 enums (`TypeKindDto`, `MemberDeclarationDto`) + 1 error enum (`SchemaExportCodecError`).
  - 8 `impl From<&domain::T> for TDto` implementations (all infallible — `clone`/`iter().map()` only, no unwrap/expect/panic).
  - `pub fn encode(schema: &SchemaExport, pretty: bool) -> Result<String, SchemaExportCodecError>`.
  - All Option fields keep their domain-equivalent serialization (no `#[serde(skip_serializing_if)]` to preserve BRIDGE-01 wire format).
  - `MemberDeclarationDto` uses serde default (externally-tagged) to match the current domain `MemberDeclaration` wire format: `{"Variant": "name"}` / `{"Field": {"name": "...", "ty": "..."}}`.
  - `TypeKindDto` uses serde default (externally-tagged, PascalCase variant names): `"Struct"` / `"Enum"` / `"TypeAlias"`.
- **`libs/infrastructure/src/lib.rs`**: added `pub mod schema_export_codec;`.
- **Unit tests (5 cases)**:
  1. `encode_empty_schema_produces_valid_json_with_crate_name` — empty schema encodes to valid JSON.
  2. `encode_single_entries_each_category` — 1 type + 1 function + 1 trait + 1 impl assertions.
  3. `encode_pretty_vs_compact` — pretty contains `\n`, compact does not.
  4. `encode_member_declaration_variant_uses_externally_tagged_form` — externally-tagged verification.
  5. `encode_type_kind_uses_pascal_case_variants` — `Struct`/`Enum`/`TypeAlias` verification.
- **`cargo nextest run`**: 1945 tests pass (5 new schema_export_codec tests + 1940 existing).
- **Signal transition (after catalogue correction from 8 → 9 entries)**: `bin/sotp track type-signals domain-serde-ripout-2026-04-15 --layer infrastructure` → `blue=9 yellow=0 red=0 (total=9, undeclared=0, skipped=40)`. All 9 catalogue entries transitioned from Yellow to Blue.
- **`cargo make ci`**: PASS (subsequent to signal re-evaluation).

### T004 (2026-04-15, commit ad77e03)

- **`libs/domain/Cargo.toml`**: `serde = { version = "1", features = ["derive"] }` を `[dependencies]` から削除。`[dev-dependencies]` 側には元々 serde が無かったため変更なし。
- **`libs/domain/src/schema.rs`**: `use serde::Serialize;` を削除。6 つの `#[derive(... Serialize)]` (`SchemaExport` / `TypeKind` / `TypeInfo` / `FunctionInfo` / `TraitInfo` / `ImplInfo`) から `Serialize` を削除。module doc comment は BRIDGE-01 JSON wire format が `infrastructure::schema_export_codec` に移管された旨に更新。
- **`libs/domain/src/tddd/catalogue.rs`**: `use serde::Serialize;` を削除。3 つの `#[derive(... Serialize)]` (`ParamDeclaration` / `MethodDeclaration` / `MemberDeclaration`) から `Serialize` を削除。`MethodDeclaration::Serialize` は dead code (`TypeNode` / `TraitNode` が `Serialize` を持たないため transitive 経路なし) で、削除してもコンパイルエラー発生せず。
- **`apps/cli/src/commands/domain.rs`**: `export_schema()` が `infrastructure::schema_export_codec::encode(&schema, args.pretty)` 経由に書き換え済み。旧 `if args.pretty { serde_json::to_string_pretty } else { serde_json::to_string }` 分岐は削除。エラー mapping (`CliError::Message(format!("JSON serialization failed: {e}"))`) と `--pretty` semantics は保持。
- **`libs/infrastructure/src/schema_export_tests.rs`**: `export_schema_json_roundtrip` test を `export_schema_encode_produces_parseable_json` に rename し、body を `schema_export_codec::encode(&schema, false)` 経由に書き換え。`parsed["crate_name"] == "domain"` assertion を追加。test module の `#[allow]` に `clippy::indexing_slicing` を追加 (`serde_json::Value` indexing のため)。
- **`Cargo.lock`**: `domain` crate graph から serde 依存が削除された旨を反映。
- **`cargo make ci`**: PASS. fmt-check / clippy -D warnings / nextest / deny / check-layers / verify-spec-states / verify-arch-docs 全通過。
- **`grep 'use serde\|derive.*Serialize\|derive.*Deserialize' libs/domain/src/`**: 全 matches が doc comment / コメント内の historical marker のみでコード依存は zero (ADR §D1 検証条件 satisfied)。
- **`grep 'serde' libs/domain/Cargo.toml`**: zero matches.
- **`type-signals --layer infrastructure`**: `blue=9 yellow=0 red=0 (total=9, undeclared=0, skipped=40)` 維持 — `build_type_graph` が trait impls を `i.trait_name().is_none()` で除外するため、serde derive 削除は `TypeNode::methods` に影響しない (ADR §D1 / §D7 検証条件 satisfied、回帰なし)。
- **Review approvals (full-model gpt-5.4)**: `domain` / `cli` / `infrastructure` / `other` 全 4 scope で `zero_findings`。fast round は `domain` / `cli` / `infrastructure` の 3 scope が 1 round 収束、`other` scope のみ `infrastructure-types.md` の source attribution 誤記を round 1 で検出・修正し、round 2 で `zero_findings`。

### T005 (2026-04-15)

- **`knowledge/adr/README.md`** の「信号機アーキテクチャ」section に 3 ADR を索引追加:
  1. `2026-04-13-1813-tddd-taxonomy-expansion.md` — TDDD 型カタログ Taxonomy 拡張 (Accepted 2026-04-13)
  2. `2026-04-14-0625-finding-taxonomy-cleanup.md` — Finding 型 Taxonomy クリーンアップ (Accepted 2026-04-14)
  3. `2026-04-14-1531-domain-serde-ripout.md` — Domain serde 依存除去 (本 ADR、Accepted 2026-04-14)
- **verification.md**: T004 の Result を追記、Scope Verified の全 checkbox を `[x]` に更新、Track 2 引継ぎ事項 #4 (CI rustdoc 実行時間の体感) の実測値を埋める (下記「Track 2 引継ぎ事項」参照)。
- **本 ADR `2026-04-14-1531-domain-serde-ripout.md`**: `/track:plan` Phase 4 で事前作成済みで、T005 では content 変更を加えていない (D1-D11 セクションは T001-T004 実装内容と整合確認済み)。

## Verified At

2026-04-15T07:00:00Z

---

## Track 2 引継ぎ事項

本トラック完了時に T005 で記載する 5 項目。

### 1. rustdoc viability audit 結果 (T001)

- **success / failure**: success (after 6 prereq doc fixes)
- **JSON サイズ**: 1,360,827 bytes (1.3 MB) — `target/rustdoc-audit/doc/infrastructure.json`
- **wall time**: ~6.0s on failed first attempt (errored out early on warnings), ~0.7s on warm re-runs. Clean-slate cold timing not separately measured.
- **prereq doc fixes**: 6 files (2 × `invalid_html_tags` + 4 × `private_intra_doc_links`) — see Result section T001 for details.
- **備考**: Infrastructure rustdoc JSON has now been generated for the first time in the project's history (previously blocked by workspace `warnings = "deny"` lint policy catching pre-existing doc comment cruft). Track 2 should expect this to remain stable once these 6 fixes are preserved.

### 2. infrastructure 内同名衝突 audit 結果 (T001)

- **T001 時点では未検出**: rustdoc JSON 生成自体は success。ただし `build_type_graph` (`code_profile_builder.rs`) の collision warning は rustdoc JSON **を読んだ後** に出るため、T001 (rustdoc のみ実行) では検出機会がない。
- **T002 で実施・結果確定**: T002 の `baseline-capture` で `build_type_graph` が実行された。`warning: same-name type collision for X` は一件も出力されなかった — infrastructure crate に同名型衝突は存在しない。
- **対応方針**: 衝突なし確認済み。rename cascade は不要。Track 2 での follow-up 不要。

### 3. infrastructure-types.json に seed した DTO 一覧 (T002 + T003 補正後)

- 6 `dto` kind entry: `SchemaExportDto` / `TypeInfoDto` / `FunctionInfoDto` / `TraitInfoDto` / `ImplInfoDto` / `SchemaParamDto`
- 2 `enum` kind entry: `MemberDeclarationDto` (`expected_variants: ["Variant", "Field"]`) / `TypeKindDto` (`expected_variants: ["Struct", "Enum", "TypeAlias"]`)
- 1 `error_type` kind entry: `SchemaExportCodecError` (`expected_variants: ["Json"]`)
- 合計 9 entries (6 dto + 2 enum + 1 error_type)
- **T003 補正ノート**: T002 commit 時点は 8 entries (MemberDeclarationDto を `dto` kind、TypeKindDto を private 除外) だったが、T003 実装時点で補正。`MemberDeclarationDto` は Rust `enum` のため `enum` kind に変更、`TypeKindDto` は Rust E0446 により `pub` 必須のため 9th entry として追加。

### 4. CI rustdoc 実行時間の体感 (T004 後)

- **infrastructure layer rustdoc** (T001 実測): cold-ish failed first attempt ~6.0s (early exit on lint error), warm re-run ~0.7s. Clean-slate cold timing not separately measured. 生成 JSON サイズ 1,360,827 bytes (1.3 MB)。
- **domain / usecase layer rustdoc**: T001-T004 の範囲では個別計測していない (T001 audit は infrastructure のみ)。Track 2 で計測予定。現時点の推定 (workspace size からの類推): domain ~1-2s warm / ~数秒 cold, usecase ~1s warm / ~数秒 cold。
- **`cargo make ci` 全体 wall time**: T002 commit 時 (800bddf): 11.86s。T001 commit 時 (d51694e, initial audit 後): 16.73s。T003 commit 時 (ad3aeae): "PASS (subsequent to signal re-evaluation)" のみ記録 (明示的な秒数なし)。T004 commit 時 (ad77e03): "PASS" のみ記録 (明示的な秒数なし)。いずれも rustdoc は CI pipeline に組み込まれていない状態での数値。
- **許容範囲か**: CI rustdoc を **全 3 layer 同時** に組み込むと、単純合算で +5-10s 程度 warm + より大きな cold 時間増加が見込まれる。現在の `cargo make ci` は 30s 以内に収まっており、+10s は体感許容範囲内。ただし CI 環境では cold hit の頻度次第で体感が変わるため、Track 2 で ADR 0002 §3.E の cache 戦略 (rustdoc JSON の incremental 再利用) を検討・実装する価値が残る。
- **Track 2 での対応**: (a) CI pipeline に `cargo +nightly rustdoc -p domain / usecase / infrastructure --output-format json` を組み込み、実測 cold / warm 時間を verification で記録する、(b) 実測値が体感許容範囲 (総計 20s を目安) を超える場合は ADR 0002 §3.E の cache 戦略実装に進む。

### 5. Adapter variant が必要そうな infra type の暫定リスト

Track 2 で TypeDefinitionKind::Adapter / SecondaryAdapter 等の新 variant 設計と併せて catalog する候補:

- `CodexReviewer` (usecase の `Reviewer` trait の impl)
- `FsReviewStore` (usecase の `ReviewWriter` / `ReviewReader` trait の impl)
- `GitDiffGetter` (usecase の `DiffGetter` trait の impl)
- `Sha256ReviewHasher` (usecase の `ReviewHasher` trait の impl)
- 各種 `verify` module (orchestration や validation)
- `RustdocSchemaExporter` (domain の `SchemaExporter` trait の impl)
- `GitShowTrackBlobReader` (usecase の `TrackBlobReader` trait の impl)
