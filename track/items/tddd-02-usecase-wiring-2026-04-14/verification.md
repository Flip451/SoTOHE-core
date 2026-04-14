# Verification — TDDD-02: usecase 層 TDDD wiring + Type Catalogue Taxonomy 拡張

## Scope Verified

- [x] `TypeDefinitionKind` に 7 新 variants (`ApplicationService`, `UseCase`, `Interactor`, `Dto`, `Command`, `Query`, `Factory`) が追加されている (T001)
- [x] `TraitPort` → `SecondaryPort` rename が `libs/domain/src/tddd/` 全域で完了し、`kind_tag` が `"secondary_port"` に変更されている (T001)
- [x] `catalogue_codec` が 12 kind_tag (`secondary_port` + 7 新 + 既存 4) を decode/encode round-trip し、`"trait_port"` は拒否する (T002)
- [x] `type_catalogue_render.rs` のセクションヘッダが新 variants 用に更新されている (T002)
- [x] `libs/domain/src/tddd/` が layer-agnostic であること grep 検証済み (T003)
- [x] `RustdocSchemaExporter::export("usecase")` が正常動作することを確認済み (T004)
- [x] `signals.rs` から Phase 1 reject block (`non_domain_enabled`, `skipped_enabled_layers`, `if filter != "domain"` arm) が完全削除されている (T005)
- [x] `baseline.rs` から `enforce_domain_tddd_enabled` / `synthetic_domain_binding` / `--layer usecase` reject / partial-failure exit code が完全削除されている (T006)
- [x] `execute_type_signals_single` → `execute_type_signals_for_layer` にリネーム済み、`execute_type_signals` が `bindings` loop になっている (T005)
- [x] `apps/cli/src/commands/track/tddd/` 全域で `non_domain_enabled` / `skipped_enabled_layers` / `enforce_domain_tddd_enabled` / `synthetic_domain_binding` の grep がゼロ件 (T005, T006)。注: `resolve_layers` 内のレガシーフォールバック JSON (`"domain"` 文字列含む) はこのチェックの対象外
- [x] `architecture-rules.json` で `usecase.tddd.enabled = true` + `catalogue_file` + `schema_export.targets` が設定されている (T007)
- [x] `usecase-types.json` が 11 entries (1 application_service + 4 secondary_port + 4 error_type + 2 use_case) を持ち、`catalogue_codec::decode` で valid JSON として parse される (T008)
- [x] `usecase-types-baseline.json` が `sotp track baseline-capture --layer usecase --force` で生成され commit されている (T009)
- [x] `sotp track type-signals --layer usecase` が `blue=11 yellow=0 red=0 (total=11, undeclared=0)` を出力する (T010)
- [x] `usecase-types.md` が track dir に生成されている (T010)
- [x] `.claude/commands/track/design.md` で `Phase 1 only: domain` / `Phase 2 will wire` / `only domain is wired` の文字列 grep がゼロ件 (T011)
- [x] merge gate U27-U30 新規テスト (usecase enablement 組み合わせ) が pass し、U1-U18 / U19-U26 は non-regression で pass (T012)
- [x] ADR 0002 Phase 1 Completion Amendment セクションが 7 項目 (完遂宣言 / 7 新 variants 根拠 / rename 理由 / §3.B 分離 / §3.C defer / §3.E 分離 / Status 更新) を含む (T013)
- [x] ADR 0002 Status が `Accepted (Phase 1 complete: tddd-01-multilayer-2026-04-12 + tddd-02-usecase-wiring-2026-04-14)` に更新されている (T013)
- [x] `cargo make ci` (fmt-check + clippy -D warnings + test + deny + check-layers + verify-spec-states + verify-arch-docs) が全通過する (T014)

## Manual Verification Steps

### 1. `TypeDefinitionKind` variants 検証

```bash
rg '^    (ApplicationService|UseCase|Interactor|Dto|Command|Query|Factory|SecondaryPort|Typestate|Enum|ValueObject|ErrorType)' libs/domain/src/tddd/catalogue.rs
# 期待: 全 12 variant が enum 定義ブロックに出現 (行頭 4 スペースインデント)

rg 'TraitPort|trait_port' libs apps .claude Makefile.toml -- --glob '!track/items/tddd-01-multilayer-2026-04-12/**'
# 期待: ゼロ件 (tddd-01 アーカイブを除く全域で完全置換)
```

### 2. CLI 一般化検証

```bash
rg 'non_domain_enabled|skipped_enabled_layers|enforce_domain_tddd_enabled|synthetic_domain_binding' apps/cli/src/commands/track/tddd/
# 期待: ゼロ件
# 注: resolve_layers 内の "domain" フォールバック JSON はこのチェックの対象外

rg 'execute_type_signals_single' apps/cli/src/commands/track/tddd/signals.rs
# 期待: ゼロ件 (execute_type_signals_for_layer に完全 rename)
```

### 3. usecase TDDD dogfood 検証

```bash
cargo run -p cli -- track baseline-capture tddd-02-usecase-wiring-2026-04-14 --layer usecase --force
# 期待: "baseline-capture: wrote usecase-types-baseline.json (N types, M traits)"

cargo run -p cli -- track type-signals tddd-02-usecase-wiring-2026-04-14 --layer usecase
# 期待: "type-signals: blue=11 yellow=0 red=0 (total=11, undeclared=0, skipped=29)"

ls track/items/tddd-02-usecase-wiring-2026-04-14/usecase-types*
# 期待: usecase-types.json / usecase-types.md / usecase-types-baseline.json が揃っている
```

### 3b. multi-layer loop (no --layer arg) 動作確認

```bash
# --layer 未指定で全 enabled 層 (domain + usecase) を処理することを確認
cargo run -p cli -- track type-signals tddd-02-usecase-wiring-2026-04-14
# 期待: domain 層と usecase 層の両方のサマリが出力される

cargo run -p cli -- track baseline-capture tddd-02-usecase-wiring-2026-04-14
# 期待: domain と usecase の baseline-capture が順次実行される (既存 baseline がある場合は skip)
```

### 4. 2-layer merge gate 検証

```bash
cargo test -p usecase merge_gate::tests::test_u27 merge_gate::tests::test_u28 merge_gate::tests::test_u29 merge_gate::tests::test_u30
# 期待: 全 pass

cargo test -p usecase merge_gate::tests
# 期待: U1-U30 全 pass (U1-U26 は non-regression)
```

### 5. CI 全通過

```bash
cargo make ci
# 期待: 全チェック通過、exit 0
```

### 6. Self-review チェックリスト (`/track:review` 前)

- [ ] CLI subcommand 名が旧名を参照していないか grep
- [ ] cross-doc 参照 (`.claude/commands/`, `knowledge/`) が新 kind_tag / 新 variant 名に追従しているか
- [ ] Timestamps が全て UTC (`Z` suffix) であるか
- [ ] spec.json / plan.md / spec.md の signals 再計算済みか
- [ ] Dependencies 非循環 (domain → usecase → infrastructure → cli のみ)

## Result

### 完了 (done) — 全 14 tasks

| Task | Commit | 概要 |
|---|---|---|
| T001 | `b1ff108` | domain crate の `TypeDefinitionKind` に 7 新 variants + `TraitPort` → `SecondaryPort` rename + `evaluate_trait_methods` 共有ヘルパー抽出 + 31 新 tests |
| T002 | `b1ff108` | infra crate の `catalogue_codec` decode/encode 8 新 kind_tag + `"trait_port"` 拒否 + `type_catalogue_render` sectioned renderer (D7 canonical order) + 30+ 新 tests |
| T003 | `49cf070` | domain/tddd/ の layer-agnostic 性 grep 検証 (zero hits, 変更不要) |
| T004 | `49cf070` | `RustdocSchemaExporter::export("usecase")` parametric 動作確認 (変更不要) |
| T005 | `49cf070` | `signals.rs` 一般化 (Phase 1 reject block 削除 + `execute_type_signals_for_layer` rename + `binding.targets().first()?`) |
| T006 | `49cf070` | `baseline.rs` 一般化 (`enforce_domain_tddd_enabled` 削除 + `synthetic_domain_binding` 削除 + `capture_baseline_for_layer` 新設) |
| T007 | `49cf070` | `architecture-rules.json` で `usecase.tddd.enabled = true` + `catalogue_file: "usecase-types.json"` + `schema_export.targets: ["usecase"]` |
| T008 | `cba52fe` | `usecase-types.json` 11 entries seed (1 application_service + 4 secondary_port + 4 error_type + 2 use_case) |
| T009 | `cba52fe` | `sotp track baseline-capture --layer usecase --force` → `usecase-types-baseline.json` (35 types, 5 traits) |
| T010 | `cba52fe` | `sotp track type-signals --layer usecase` → **blue=11 yellow=0 red=0 (全 Blue, first pass)** |
| T011 | `15431ba` | `.claude/commands/track/design.md` multi-layer loop 書き換え (162 → ~225 行、12 variants + `--layer` optional + `<catalogue_file>`/`<baseline_file>`/`<rendered_file>` derivation + spec.json-gated Stage 2) |
| T012 | `15431ba` | merge gate U27-U30 新規テスト (usecase-enablement 2-layer 組み合わせ、empty enabled_layers fail-closed、35/35 tests pass) |
| T013 | `15431ba` | ADR 0002 "Phase 1 Completion Amendment" 追加 + Status `Accepted (Phase 1 complete)` 更新 + async→is_async 統一 + child ADR `2026-04-13-1813` Status `Accepted` 更新 |
| T014 | `15431ba` | `cargo make ci` 全通過 (fmt + clippy -D warnings + test + deny + check-layers + verify-spec-states + verify-spec-coverage + verify-view-freshness + verify-arch-docs + verify-domain-purity + verify-usecase-purity) |

### Review cycles per commit

- `b1ff108` (T001+T002): 3 scopes parallel review (domain 2 rounds, infrastructure 4 rounds 2 P1 fix, other 4 rounds), 全 zero_findings
- `49cf070` (T003-T007): 2 scopes parallel review (cli 8 rounds 11 P1 fixes, other 3 rounds), 全 zero_findings
- `cba52fe` (T008-T010): 1 scope review (other 4 rounds), zero_findings
- `15431ba` (T011-T014): 3 scopes parallel review (harness-policy fast 6 + full 8 rounds 多数の P1 fixes, usecase 2 rounds, other 5 rounds), 全 zero_findings

### Dogfooding 成果

```
[OK] type-signals: blue=11 yellow=0 red=0 (total=11, undeclared=0, skipped=29)
```

全 11 entries が first-pass Blue (L1 forward check 完全一致)。usecase 層 TDDD は実運用可能な状態に到達。新 variants (`ApplicationService` + `SecondaryPort` + `UseCase`) の実利用経路が end-to-end で動作することを確認。

### Open Issues / Known Risks (resolved)

- **R1** (低 → 解消): PR #95 の 11 ラウンド振動再発リスク — harness-policy scope で fast 6 + full 8 rounds と近い規模に達したが、review-fix-lead agent が根本原因ベースの fixes (spec.json existence 条件) で着地。`skipped_enabled_layers` 削除は根本原因を絶ち、Codex reviewer が scope creep (B/C/E pull-in) を要求することはなかった
- **R2** (低 → 解消): `HookError` cross-crate 参照は format_type last-segment 化で forward check 通過確認 (T010 で blue=11)
- **R3** (延期): `Finding` 同名衝突は tddd-04 `finding-taxonomy-cleanup` follow-up track に分離 (ADR 0002 Phase 1 Completion Amendment 参照)
- **R4** (低 → 解消): T008 seed の expected_methods は initial iteration で blue=11 を達成、追加修正不要

### Follow-up tracks (from ADR 0002 Phase 1 Completion Amendment)

- **tddd-04 finding-taxonomy-cleanup**: 4 種 Finding 系型 (`domain::review_v2::Finding`, `domain::verify::Finding`, `usecase::review_workflow::ReviewFinding`, `usecase::pr_review::PrReviewFinding`) の taxonomy 再設計
- **async-trait `is_async` detection**: 実 async trait 出現時の個別対応
- **tddd-future-ci-cache**: rustdoc JSON のキャッシュ戦略

## verified_at

2026-04-14 (all 14 tasks committed: b1ff108 → 49cf070 → cba52fe → 15431ba)
