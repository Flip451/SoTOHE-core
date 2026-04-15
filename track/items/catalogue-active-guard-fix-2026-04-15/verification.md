# Verification — catalogue-active-guard-fix: catalogue active-track guard + rendered view source-file-name fix + sync_rendered_views multi-layer rollout

## Scope Verified

- [ ] T001: 破損差分 revert 済み (Track 1 infrastructure-types.md を git HEAD 状態に復元)、track artifacts (metadata.json / spec.json / verification.md) 作成済み、track branch `track/catalogue-active-guard-fix-2026-04-15` 作成 + switch 済み
- [ ] T002: `knowledge/adr/2026-04-15-1012-catalogue-active-guard-fix.md` が Proposed 状態で起草され、user 承認後に Accepted に遷移した。Nygard 形式 + Decisions D1-D4 + Rejected Alternatives B1-B5 + SKILL.md 古記述の structural 経緯 D4/Context を含む (ADR-first gate は spec.json::constraints に記録、ADR には含めない)
- [ ] T003: `apps/cli/src/commands/track/tddd/signals.rs::execute_type_signals` に exhaustive `match` 文で status guard が追加され、`TrackStatus` の 6 variants 全て (`Planned` / `InProgress` / `Done` / `Blocked` / `Cancelled` / `Archived`) が明示的に列挙されている。`Done | Archived` → `CliError::Message` で reject、`Planned | InProgress | Blocked | Cancelled` → proceed。`matches!` マクロは使用しない (非網羅的で新 variant silent pass の fail-open となるため)
- [ ] T003: `test_execute_type_signals_rejects_done_track` 新規追加 + pass (+ `archived` variant)
- [ ] T003: 将来 `TrackStatus` に新 variant が追加された場合、exhaustive `match` 文が compile error を発生させ、開発者に frozen/active 分類を強制する (fail-closed structural guarantee)
- [ ] T004: `libs/infrastructure/src/type_catalogue_render.rs::render_type_catalogue` の signature が `(doc, source_file_name: &str)` に変更されている
- [ ] T004: `libs/infrastructure/src/track/render.rs::sync_rendered_views` が `architecture-rules.json` の `tddd.enabled=true` 全 layer を iterate し、各 `<layer>-types.md` を生成するよう拡張されている (domain / usecase / infrastructure)
- [ ] T004: `libs/infrastructure/src/track/render.rs::sync_rendered_views` が既存 `libs/infrastructure/src/verify/tddd_layers.rs::parse_tddd_layers` を `use crate::verify::tddd_layers::parse_tddd_layers;` で import して reuse している (新 helper は作成しない、apps/cli::resolve_layers と同じ resolver を共有)
- [ ] T004: 呼び出し側 2 箇所が catalogue JSON ファイル名 (e.g. `infrastructure-types.json`) を `source_file_name` として渡している (rendered .md パスとは別)。`validate_and_write_catalogue` 側は `domain_types_path.file_name()` から catalogue ファイル名を導出 (`binding` はそのスコープに存在しない)、`sync_rendered_views` multi-layer loop 内では `binding.catalogue_file()` を使用
- [ ] T004: 既存テスト `type_catalogue_render.rs:211` が signature 変更に追従し pass
- [ ] T004: 既存テスト `sync_rendered_views_generates_domain_types_md_from_domain_types_json` が multi-layer loop 化後も pass (backward compat)
- [ ] T004: 新規テスト `sync_rendered_views_generates_usecase_types_md_from_usecase_types_json` + `sync_rendered_views_generates_infrastructure_types_md_from_infrastructure_types_json` + `sync_rendered_views_generates_multiple_layer_types_md_independently` が追加され pass
- [ ] T005: `track/items/tddd-02-usecase-wiring-2026-04-14/usecase-types.md` の 1 行目が `<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->` に復旧
- [ ] T005: `track/items/domain-serde-ripout-2026-04-15/infrastructure-types.md` の 1 行目が `<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->` のまま維持 (T004 fix 後に dry re-run で drift なし)
- [ ] T005: verification.md Result section に「done track の is_done_or_archived guard を bypass した cosmetic header 復旧である」旨のメモを記載
- [ ] T006: `.claude/skills/track-plan/SKILL.md` line 165 classification table で feedback が Yellow 行に移動し、line 283-284 diff hearing update guidance で feedback → Blue 昇格 の記述が → Yellow のまま (Blue 昇格には ADR/convention 永続化が必要) に書き換えられている
- [ ] T007: `knowledge/adr/README.md` の信号機アーキテクチャ section に本 ADR の索引が追加されている
- [ ] T007: 最終 smoke test: `cargo make track-sync-views` を catalogue-active-guard-fix-2026-04-15 branch 上で実行し、全 active tracks の各 layer の rendered view が一括生成される動作を手動確認
- [ ] T007: `cargo make ci` 全通過 (fmt-check + clippy -D warnings + nextest + deny + check-layers + verify-spec-states + verify-arch-docs)

## Manual Verification Steps

### 1. Active-track guard 再現シナリオ (T003)

```bash
# バグ再現: main branch 上で merged track (status=done) を指定
git branch --show-current                                    # expect: track/catalogue-active-guard-fix-2026-04-15 (or main after cleanup)
jq '.status' track/items/domain-serde-ripout-2026-04-15/metadata.json  # expect: "done"

bin/sotp track type-signals domain-serde-ripout-2026-04-15 --layer infrastructure
# expect: exit code non-zero
# expect: stderr message like "cannot run type-signals on 'domain-serde-ripout-2026-04-15' (status=done). Completed tracks are frozen — run on an active track instead."
# expect: track/items/domain-serde-ripout-2026-04-15/infrastructure-types.json は変更なし
# expect: track/items/domain-serde-ripout-2026-04-15/infrastructure-types.md は変更なし

git status --short  # expect: no diff on the above files
```

### 2. `render_type_catalogue` signature 変更 + sync_rendered_views multi-layer (T004)

```bash
# signature 確認
rg 'fn render_type_catalogue' libs/infrastructure/src/type_catalogue_render.rs
# expect: pub fn render_type_catalogue(doc: &TypeCatalogueDocument, source_file_name: &str) -> String

# sync_rendered_views が multi-layer loop 化されている (parse_tddd_layers を直接 reuse、新 helper は作成しない)
rg -n 'parse_tddd_layers|binding\.catalogue_file|binding\.rendered_file' libs/infrastructure/src/track/render.rs
# expect: multiple matches (use crate::verify::tddd_layers::parse_tddd_layers import + loop 内での使用)

# 呼び出し側 2 箇所が source_file_name を渡している (production call sites のみ確認)
rg -n 'render_type_catalogue\(' libs/infrastructure/src/track/render.rs apps/cli/src/commands/track/tddd/signals.rs
# expect: 少なくとも 2 行が 2 引数形式 (sync_rendered_views loop 内 + validate_and_write_catalogue 内)
# Note: 関数定義は type_catalogue_render.rs に存在するためこのコマンドでは含まれない

# テスト: signature 変更 + 新規テストの確認 (特定テスト名を指定して6件確認)
cargo nextest run -p infrastructure test_render_type_catalogue
cargo nextest run -p infrastructure sync_rendered_views_generates_domain_types_md
cargo nextest run -p infrastructure sync_rendered_views_generates_usecase_types_md
cargo nextest run -p infrastructure sync_rendered_views_generates_infrastructure_types_md
cargo nextest run -p infrastructure sync_rendered_views_generates_multiple_layer_types_md
# expect: 各テストが pass
```

### 3. データ復旧確認 (T005)

```bash
head -1 track/items/tddd-01-multilayer-2026-04-12/domain-types.md
# expect: <!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

head -1 track/items/tddd-02-usecase-wiring-2026-04-14/usecase-types.md
# expect: <!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

head -1 track/items/domain-serde-ripout-2026-04-15/infrastructure-types.md
# expect: <!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->
```

### 4. SKILL.md SSoT 整合確認 (T006)

```bash
# classification table: feedback が Yellow 行に移動
rg -n 'feedback.*Blue' .claude/skills/track-plan/SKILL.md
# expect: no match (古記述が除去されている)

# Blue 行は document / convention のみ
rg -n '🔵 確定済み.*document.*convention' .claude/skills/track-plan/SKILL.md
# expect: 1 match without "feedback"

# diff hearing update guidance: feedback 追加で Blue 昇格しない旨
rg -n 'feedback.*Yellow のまま|feedback.*ADR.*convention.*必要' .claude/skills/track-plan/SKILL.md
# expect: at least 1 match
```

### 5. ADR index 登録 (T002 + T007)

```bash
rg 'catalogue-active-guard-fix' knowledge/adr/README.md
# expect: 1 match (the ADR index row)

rg '^## Status' knowledge/adr/2026-04-15-1012-catalogue-active-guard-fix.md -A 2
# expect: ## Status
#         (blank or next line)
#         Accepted
```

### 6. 最終 smoke test — track-sync-views multi-layer (T007)

```bash
# catalogue-active-guard-fix-2026-04-15 は現在 catalogue file を持たない (domain-only structural track)
# しかし loop は全 tddd-enabled layer を check するので no-op が 3 件走る
cargo make track-sync-views
# expect: エラーなし、not_file skip で無害終了

# 他 active track (もしあれば) で各 layer の md が生成される動作を確認
# 現状 active な multi-layer track が存在しないため、次のトラック作業時に実機で確認
```

### 7. 最終 CI

```bash
cargo make ci
# expect: fmt-check / clippy -D warnings / nextest / deny / check-layers / verify-spec-states / verify-arch-docs 全通過
```

## Result

### T001

TBD — track 作成完了時に記録する。

### T002

TBD

### T003

TBD

### T004

TBD

### T005

TBD — done track (tddd-02 / domain-serde-ripout) の header 書き換えは是否 `is_done_or_archived` guard を bypass する cosmetic 復旧であり、signals / catalogue 本体は一切変更しない (git diff は 1 行のみ)。

### T006

TBD

### T007

TBD

## Verified At

TBD
