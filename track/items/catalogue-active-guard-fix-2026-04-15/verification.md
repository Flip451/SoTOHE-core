# Verification — catalogue-active-guard-fix: catalogue active-track guard + rendered view source-file-name fix + sync_rendered_views multi-layer rollout

## Scope Verified

- [x] T001: 破損差分 revert 済み (Track 1 infrastructure-types.md を git HEAD 状態に復元)、track artifacts (metadata.json / spec.json / verification.md) 作成済み、track branch `track/catalogue-active-guard-fix-2026-04-15` 作成 + switch 済み
- [x] T002: `knowledge/adr/2026-04-15-1012-catalogue-active-guard-fix.md` が Proposed 状態で起草され、user 承認後に Accepted に遷移した。Nygard 形式 + Decisions D1-D4 + Rejected Alternatives B1-B5 + SKILL.md 古記述の structural 経緯 D4/Context を含む (ADR-first gate は spec.json::constraints に記録、ADR には含めない)
- [x] T003: `apps/cli/src/commands/track/tddd/signals.rs::execute_type_signals` に exhaustive `match` 文で status guard が追加され、`TrackStatus` の 6 variants 全て (`Planned` / `InProgress` / `Done` / `Blocked` / `Cancelled` / `Archived`) が明示的に列挙されている。`Done | Archived` → `CliError::Message` で reject、`Planned | InProgress | Blocked | Cancelled` → proceed。`matches!` マクロは使用しない (非網羅的で新 variant silent pass の fail-open となるため)
- [x] T003: `test_execute_type_signals_rejects_done_track` 新規追加 + pass (+ `archived` variant)
- [x] T003: 将来 `TrackStatus` に新 variant が追加された場合、exhaustive `match` 文が compile error を発生させ、開発者に frozen/active 分類を強制する (fail-closed structural guarantee)
- [x] T004: `libs/infrastructure/src/type_catalogue_render.rs::render_type_catalogue` の signature が `(doc, source_file_name: &str)` に変更されている
- [x] T004: `libs/infrastructure/src/track/render.rs::sync_rendered_views` が `architecture-rules.json` の `tddd.enabled=true` 全 layer を iterate し、各 `<layer>-types.md` を生成するよう拡張されている (domain / usecase / infrastructure)
- [x] T004: `libs/infrastructure/src/track/render.rs::sync_rendered_views` が既存 `libs/infrastructure/src/verify/tddd_layers.rs::parse_tddd_layers` を `use crate::verify::tddd_layers::parse_tddd_layers;` で import して reuse している (新 helper は作成しない、apps/cli::resolve_layers と同じ resolver を共有)
- [x] T004: 呼び出し側 2 箇所が catalogue JSON ファイル名 (e.g. `infrastructure-types.json`) を `source_file_name` として渡している (rendered .md パスとは別)。`validate_and_write_catalogue` 側は `domain_types_path.file_name()` から catalogue ファイル名を導出 (`binding` はそのスコープに存在しない)、`sync_rendered_views` multi-layer loop 内では `binding.catalogue_file()` を使用
- [x] T004: 既存テスト `type_catalogue_render.rs:211` が signature 変更に追従し pass
- [x] T004: 既存テスト `sync_rendered_views_generates_domain_types_md_from_domain_types_json` が multi-layer loop 化後も pass (backward compat)
- [x] T004: 新規テスト `sync_rendered_views_generates_usecase_types_md_from_usecase_types_json` + `sync_rendered_views_generates_infrastructure_types_md_from_infrastructure_types_json` + `sync_rendered_views_generates_multiple_layer_types_md_independently` が追加され pass
- [x] T005: `track/items/tddd-02-usecase-wiring-2026-04-14/usecase-types.md` の 1 行目が `<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->` に復旧
- [x] T005: `track/items/domain-serde-ripout-2026-04-15/infrastructure-types.md` の 1 行目が `<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->` のまま維持 (T004 fix 後に dry re-run で drift なし)
- [x] T005: verification.md Result section に「done track の is_done_or_archived guard を bypass した cosmetic header 復旧である」旨のメモを記載
- [x] T006: `.claude/skills/track-plan/SKILL.md` line 165 classification table で feedback が Yellow 行に移動し、line 283-284 diff hearing update guidance で feedback → Blue 昇格 の記述が → Yellow のまま (Blue 昇格には ADR/convention 永続化が必要) に書き換えられている
- [x] T007: `knowledge/adr/README.md` の信号機アーキテクチャ section に本 ADR の索引が追加されている
- [x] T007: 最終 smoke test: `cargo make track-sync-views` を catalogue-active-guard-fix-2026-04-15 branch 上で実行し、エラーなし + not_file skip で無害終了を確認 (本 track は catalogue file を持たない structural track のため no-op; multi-layer 実機確認は次の active multi-layer track が出現した際に実施予定)
- [x] T007: `cargo make ci` 全通過 (fmt-check + clippy -D warnings + nextest + deny + check-layers + verify-spec-states + verify-arch-docs)

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

Track artifacts 作成完了。`track/items/catalogue-active-guard-fix-2026-04-15/{metadata.json, spec.json, verification.md}` 初版を main ブランチ merge 後の HEAD 状態から作成し、破損していた `track/items/domain-serde-ripout-2026-04-15/infrastructure-types.md` を git HEAD に revert。`track/catalogue-active-guard-fix-2026-04-15` branch を main から分岐して switch。

### T002

ADR `knowledge/adr/2026-04-15-1012-catalogue-active-guard-fix.md` 起草。Proposed → user レビュー → track-specific な self-reference を全削除する簡素化 (319→約 210 行) を経て Accepted へ遷移。ADR-first gate は process constraint のため spec.json::constraints に記録。Decisions D1 (active-track guard), D2 (render_type_catalogue signature), D3 (sync_rendered_views multi-layer), D4 (SKILL.md SSoT fix) + Rejected Alternatives B1-B5 を含む。

### T003

`apps/cli/src/commands/track/tddd/signals.rs` に `ensure_active_track` helper + exhaustive `match` 文を追加。`TrackStatus::{Done, Archived}` は reject、`Planned | InProgress | Blocked | Cancelled` は proceed。`DocumentMeta::original_status` を読んで archived を effective status として判定 (fast round P1 fix — `TrackMetadata::status()` は task state 派生で Archived を返さない)。`matches!` は意図的に不採用 (新 variant silent pass を防ぐ fail-closed 構造保証)。9 新規テスト (unit 6 + integration 3) + 4 既存テスト更新 + `minimal_active_metadata_json` helper 追加。commit `537e23f1`。

### T004

3 scope 並列レビューで fast + full の zero_findings を達成 (fast 3 round + full 9 round + 6 fix)。
- `render_type_catalogue(doc, source_file_name)` signature 変更 + `source_file_name` の `\n` / `\r` / `-->` サニタイズ (full round 2 P1 — attacker-controlled `catalogue_file` からの HTML コメントインジェクション防御)。
- `sync_rendered_views` が `parse_tddd_layers` 経由で全 `tddd.enabled` layer を iterate。`is_done_or_archived` guard で arch-rules 読込を囲い (round 1 fix)、`seen_rendered: HashSet<String>` で複数 layer の同一 rendered-path 衝突を防止 (round 3 fix)、opt-out の後に collision 登録するよう順序調整 (round 4 fix)。
- 新規テスト: `test_render_type_catalogue_header_reflects_source_file_name_argument` / `test_render_type_catalogue_header_sanitizes_comment_injection_sequences` / `test_validate_and_write_catalogue_rendered_header_uses_json_catalogue_filename` / `sync_rendered_views_generates_usecase_types_md_from_usecase_types_json` / `..._infrastructure_types_md_..._infrastructure_types_json` / `sync_rendered_views_generates_multiple_layer_types_md_independently` / `sync_rendered_views_malformed_layer_json_does_not_block_other_layers` (round 5 fix)。
- 既存 26 テスト call site を `replace_all` で bulk update し backward compat を確保。commit `eb2c7069`。

### T005

`track/items/tddd-02-usecase-wiring-2026-04-14/usecase-types.md` の 1 行目を `<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->` に復旧 (done track の `is_done_or_archived` guard を bypass した cosmetic header 復旧であり、signals / catalogue 本体は一切変更しない。git diff は 1 行のみ)。`track/items/domain-serde-ripout-2026-04-15/infrastructure-types.md` は既に正しいヘッダで変更不要。fast + full いずれも 1 round で zero_findings。commit `56f5c448`。

### T006

`.claude/skills/track-plan/SKILL.md` line 165 classification table で `feedback` を Blue 行から Yellow 行に移動、line 283-285 diff hearing update guidance を「feedback 追加後 → Blue 昇格」から「Yellow のまま (Blue 昇格には ADR/convention 永続化が必要)」に書き換え、`knowledge/conventions/source-attribution.md §Upgrading Yellow to Blue` へのポインタを追加。2026-04-12 strict-signal-gate-v2 ADR で feedback が Yellow に降格されて以降の SSoT ドリフトを修正。fast + full いずれも 1 round で zero_findings。commit `9e54dddb`。

### T007

`knowledge/adr/README.md` 信号機アーキテクチャ section に本 ADR 行を追加、verification.md の全 checkbox 確認 + Result 記入 + Verified At 記入、最終 `cargo make ci` 全通過、最終 smoke test (`cargo make track-sync-views`) で no-op + エラーなし終了を確認。

## Verified At

2026-04-15
