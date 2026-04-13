# Verification — TDDD-02: usecase 層 TDDD wiring + Type Catalogue Taxonomy 拡張

## Scope Verified

- [ ] `TypeDefinitionKind` に 7 新 variants (`ApplicationService`, `UseCase`, `Interactor`, `Dto`, `Command`, `Query`, `Factory`) が追加されている (T001)
- [ ] `TraitPort` → `SecondaryPort` rename が `libs/domain/src/tddd/` 全域で完了し、`kind_tag` が `"secondary_port"` に変更されている (T001)
- [ ] `catalogue_codec` が 12 kind_tag (`secondary_port` + 7 新 + 既存 4) を decode/encode round-trip し、`"trait_port"` は拒否する (T002)
- [ ] `type_catalogue_render.rs` のセクションヘッダが新 variants 用に更新されている (T002)
- [ ] `libs/domain/src/tddd/` が layer-agnostic であること grep 検証済み (T003)
- [ ] `RustdocSchemaExporter::export("usecase")` が正常動作することを確認済み (T004)
- [ ] `signals.rs` から Phase 1 reject block (`non_domain_enabled`, `skipped_enabled_layers`, `if filter != "domain"` arm) が完全削除されている (T005)
- [ ] `baseline.rs` から `enforce_domain_tddd_enabled` / `synthetic_domain_binding` / `--layer usecase` reject / partial-failure exit code が完全削除されている (T006)
- [ ] `execute_type_signals_single` → `execute_type_signals_for_layer` にリネーム済み、`execute_type_signals` が `bindings` loop になっている (T005)
- [ ] `apps/cli/src/commands/track/tddd/` 全域で `non_domain_enabled` / `skipped_enabled_layers` / `enforce_domain_tddd_enabled` / `synthetic_domain_binding` の grep がゼロ件 (T005, T006)。注: `resolve_layers` 内のレガシーフォールバック JSON (`"domain"` 文字列含む) はこのチェックの対象外
- [ ] `architecture-rules.json` で `usecase.tddd.enabled = true` + `catalogue_file` + `schema_export.targets` が設定されている (T007)
- [ ] `usecase-types.json` が 11 entries (1 application_service + 4 secondary_port + 4 error_type + 2 use_case) を持ち、`catalogue_codec::decode` で valid JSON として parse される (T008)
- [ ] `usecase-types-baseline.json` が `sotp track baseline-capture --layer usecase --force` で生成され commit されている (T009)
- [ ] `sotp track type-signals --layer usecase` が `blue=11 yellow=0 red=0 (total=11, undeclared=0)` を出力する (T010)
- [ ] `usecase-types.md` が track dir に生成されている (T010)
- [ ] `.claude/commands/track/design.md` で `Phase 1 only: domain` / `Phase 2 will wire` / `only domain is wired` の文字列 grep がゼロ件 (T011)
- [ ] merge gate U27-U30 新規テスト (usecase enablement 組み合わせ) が pass し、U1-U18 / U19-U26 は non-regression で pass (T012)
- [ ] ADR 0002 Phase 1 Completion Amendment セクションが 7 項目 (完遂宣言 / 7 新 variants 根拠 / rename 理由 / §3.B 分離 / §3.C defer / §3.E 分離 / Status 更新) を含む (T013)
- [ ] ADR 0002 Status が `Accepted (Phase 1 complete: tddd-01-multilayer-2026-04-12 + tddd-02-usecase-wiring-2026-04-14)` に更新されている (T013)
- [ ] `cargo make ci` (fmt-check + clippy -D warnings + test + deny + check-layers + verify-spec-states + verify-arch-docs) が全通過する (T014)

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
# 期待: "type-signals: blue=11 yellow=0 red=0 (total=11, undeclared=0, skipped=0)"

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

### 未着手 (planned)

全 14 tasks が `todo` 状態。実装開始前のベースライン。

### 実装中 (in_progress)

(未着手)

### 完了 (done)

(未着手)

### Open Issues / Known Risks

- **R1** (低): PR #95 の 11 ラウンド振動再発リスク — `skipped_enabled_layers` 削除で根本原因は絶たれるが、Codex reviewer が scope creep (B/C/E pull-in) を要求する可能性があるので T013 ADR amendment に明示的な deferral 根拠を書くこと
- **R2** (低): `HookError` は domain crate で定義されているが usecase の HookHandler の戻り値に現れる。`format_type` が last-segment 化するため catalogue string match は通るはずだが、T010 iterate で確認必要
- **R3** (低): `Finding` 型が transitive に `Verdict::FindingsRemain(NonEmptyFindings)` 経由で含まれるが、catalogue の expected_methods には `Verdict` としてのみ出現するため直接影響なし。tddd-04 finding-taxonomy-cleanup で本格的に整理する
- **R4** (中): T008 の seed で `expected_methods` が実際のソースコードと一字一句一致する必要がある。T010 iterate で Yellow/Red が出た場合は `usecase-types.json` を修正して再実行

## verified_at

(未完了)
