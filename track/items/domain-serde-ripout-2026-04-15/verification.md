# Verification — domain-serde-ripout: hexagonal 純粋性回復 + infrastructure 層 TDDD partial dogfood (Track 1 of 2)

## Scope Verified

- [ ] T001: infrastructure 層 rustdoc viability audit が成功し、wall time が記録されている (collision warning 有無は plain rustdoc では検出不可、T002 の baseline-capture 結果を参照)
- [ ] T001: `architecture-rules.json` の infrastructure tddd が `enabled: true` に flip され、`catalogue_file` / `schema_export.targets` が設定されている
- [ ] T002: `/track:design --layer infrastructure` が成功し、`track/items/domain-serde-ripout-2026-04-15/infrastructure-types.json` (8 entries) + `infrastructure-types-baseline.json` + `infrastructure-types.md` rendered view が生成されている
- [ ] T002: `bin/sotp track type-signals domain-serde-ripout-2026-04-15 --layer infrastructure` が `blue=0 yellow=8 red=0` (初期状態、DTO 未実装) を返している
- [ ] T002: `track/items/domain-serde-ripout-2026-04-15/` 配下に `domain-types.json` / `usecase-types.json` が存在しない (per-layer opt-out)
- [ ] T003: `libs/infrastructure/src/schema_export_codec.rs` が新規作成され、8 DTO + `SchemaExportCodecError` + 8 `From` 実装 + `pub fn encode()` が定義されている
- [ ] T003: `libs/infrastructure/src/lib.rs` に `pub mod schema_export_codec;` が追加されている
- [ ] T003: unit test (空 schema / 1 type/function/trait/impl / pretty vs compact) が全通過している
- [ ] T003: `type-signals --layer infrastructure` が `blue=8 yellow=0 red=0` に遷移している (yellow=8 → blue=8)
- [ ] T004: `libs/domain/src/schema.rs` から `use serde::Serialize;` と 6 derive が削除されている
- [ ] T004: `libs/domain/src/tddd/catalogue.rs` から `use serde::Serialize;` と 3 derive (MethodDeclaration の dead code 含む) が削除されている
- [ ] T004: `libs/domain/Cargo.toml` から `serde` 依存が削除されている
- [ ] T004: `apps/cli/src/commands/domain.rs::export_schema()` が `infrastructure::schema_export_codec::encode()` 経由に書き換えられている
- [ ] T004: `libs/infrastructure/src/schema_export_tests.rs` が `schema_export_codec::encode` 経由のテストに更新されている
- [ ] T004: `cargo make export-schema -- --crate domain --pretty` の出力 JSON が変更前と structural に同一であることを手動 diff で確認済み
- [ ] T004: `type-signals --layer infrastructure` が依然として `blue=8` を維持している (serde 削除による回帰なし)
- [ ] T005: `knowledge/adr/README.md` の信号機アーキテクチャ section に本 ADR + 未登録 2 ADR が索引追加されている
- [ ] T005: Track 2 引継ぎ事項セクションの 5 項目が埋まっている
- [ ] `cargo make ci` 全通過 (fmt-check + clippy -D warnings + test + deny + check-layers + verify-spec-states + verify-arch-docs)

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
jq '.type_definitions | length' track/items/domain-serde-ripout-2026-04-15/infrastructure-types.json   # expect: 8
jq '.schema_version' track/items/domain-serde-ripout-2026-04-15/infrastructure-types.json              # expect: 2

# 全 entry が approved=true
jq '.type_definitions[] | select(.approved == false)' track/items/domain-serde-ripout-2026-04-15/infrastructure-types.json
# expect: empty

# 初期 signal (DTO 未実装時点)
bin/sotp track type-signals domain-serde-ripout-2026-04-15 --layer infrastructure
# expect: blue=0 yellow=8 red=0 (total=8)

# per-layer opt-out 確認
ls track/items/domain-serde-ripout-2026-04-15/domain-types.json        # expect: not found
ls track/items/domain-serde-ripout-2026-04-15/usecase-types.json       # expect: not found

# Stage 2 spec-states は per-layer で NotFound → skip なので PASS
cargo make ci    # expect: all pass (infrastructure が yellow=8 でも strict merge gate 前は interim allowed)
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

# signal 遷移: yellow=8 → blue=8
bin/sotp track type-signals domain-serde-ripout-2026-04-15 --layer infrastructure
# expect: blue=8 yellow=0 red=0
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
# expect: blue=8 yellow=0 red=0 (serde 削除による回帰なし — build_type_graph が trait impls を除外)
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

### T002 (2026-04-14)

- **`/track:design --layer infrastructure`** の Step 2-4 を順次実行:
  - **Step 2 (type design)**: 8 entries designed — 7 × `dto` kind (`SchemaExportDto`, `TypeInfoDto`, `FunctionInfoDto`, `TraitInfoDto`, `ImplInfoDto`, `MemberDeclarationDto`, `SchemaParamDto`) + 1 × `error_type` kind (`SchemaExportCodecError` with `expected_variants: ["Json"]`). All `approved: true`, `action` omitted (= `add` default). `TypeKindDto` intentionally excluded (private enum).
  - **Step 3 (write catalogue)**: `track/items/domain-serde-ripout-2026-04-15/infrastructure-types.json` created with schema_version 2 + 8 entries.
  - **Step 4.1 (baseline-capture)**: `bin/sotp track baseline-capture domain-serde-ripout-2026-04-15 --layer infrastructure --force` → wrote `infrastructure-types-baseline.json` containing **38 types + 2 traits**. No `same-name type collision for X` warning from `build_type_graph`.
  - **Step 4.2 (type-signals)**: `bin/sotp track type-signals domain-serde-ripout-2026-04-15 --layer infrastructure` → `blue=0 yellow=8 red=0 (total=8, undeclared=0, skipped=40)`. All 8 declared DTOs are Yellow because the code is not yet present (T003 will implement them). All 38 existing infrastructure types and 2 traits are in `B\A` and correctly classified as `skipped` (unchanged structure; 38 + 2 = 40 skipped total).
- **Collision audit (deferred from T001)**: baseline-capture emitted no `same-name type collision` warnings on stderr. Infrastructure crate has no rustdoc-visible same-name type collisions as of this track.
- **per-layer opt-out confirmed**: `track/items/domain-serde-ripout-2026-04-15/` contains no `domain-types.json` and no `usecase-types.json`. `spec_states.rs::evaluate_layer_catalogue` skips both layers at Stage 2. Stage 2 spec-states PASS.
- **`cargo make ci`**: PASS (Build Done in 11.86s). `verify-spec-states` emits the expected interim warning listing the 8 Yellow types and noting "merge gate will block these until upgraded to Blue" — this is the intended TDDD WIP signal state and will clear in T003.

(T003 以降は各 task 完了時に追記)

## Verified At

(完了時に ISO 8601 UTC で追記)

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
- **T002 で検出予定**: T002 の `/track:design --layer infrastructure` が内部で `baseline-capture` を走らせ、そこで `build_type_graph` が実行される。そのタイミングで `warning: same-name type collision for X` の有無を T002 の verification result に追記する。
- **対応方針**: 本トラックでは記録のみ、Track 2 で rename cascade

### 3. infrastructure-types.json に seed した DTO 一覧 (T002)

- 7 `dto` kind entry: `SchemaExportDto` / `TypeInfoDto` / `FunctionInfoDto` / `TraitInfoDto` / `ImplInfoDto` / `MemberDeclarationDto` / `SchemaParamDto`
- 1 `error_type` kind entry: `SchemaExportCodecError` (`expected_variants: ["Json"]`)
- `TypeKindDto` は private enum のため catalogue から除外

### 4. CI rustdoc 実行時間の体感 (T004 後)

- domain layer rustdoc: TBD
- usecase layer rustdoc: TBD
- infrastructure layer rustdoc: TBD
- 合計 wall time: TBD
- 許容範囲か: TBD (許容外なら ADR 0002 §3.E の cache 戦略を Track 2 で実装)

### 5. Adapter variant が必要そうな infra type の暫定リスト

Track 2 で TypeDefinitionKind::Adapter / SecondaryAdapter 等の新 variant 設計と併せて catalog する候補:

- `CodexReviewer` (usecase の `Reviewer` trait の impl)
- `FsReviewStore` (usecase の `ReviewWriter` / `ReviewReader` trait の impl)
- `GitDiffGetter` (usecase の `DiffGetter` trait の impl)
- `Sha256ReviewHasher` (usecase の `ReviewHasher` trait の impl)
- 各種 `verify` module (orchestration や validation)
- `RustdocSchemaExporter` (domain の `SchemaExporter` trait の impl)
- `GitShowTrackBlobReader` (usecase の `TrackBlobReader` trait の impl)
- (T001 audit 後に追加候補を記録)
