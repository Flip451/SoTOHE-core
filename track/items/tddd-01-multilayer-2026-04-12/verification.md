# Verification — TDDD-01: 型カタログ多層化 + L1 シグネチャ検証

## Scope Verified

- [x] `libs/domain/src/tddd/catalogue.rs` が `catalogue.rs` / `signals.rs` / `consistency.rs` の 3 モジュールに分割されている (T001)
- [x] `DomainType*` シンボルが `TypeDefinition*` / `TypeCatalogue*` / `TypeSignal` / `evaluate_type_signals` / `check_type_signals` に全置換されている (T001-T003)
- [x] C1 breaking change が適用されている: `FunctionInfo::signature: String` / `pub fn signature()` / `format_sig` が全て削除されている (T004)
- [x] `MethodDeclaration` / `ParamDeclaration` / `MemberDeclaration` が domain 層で定義され、`FunctionInfo` / `TypeNode` / `TraitNode` / `TypeBaselineEntry` / `TraitBaselineEntry` の 5 箇所で共有されている (T004, T005)
- [x] `TypeDefinitionKind::TraitPort { expected_methods: Vec<MethodDeclaration> }` が L1 forward/reverse check を実装している (T006)
- [x] `architecture-rules.json` の `layers[]` に `tddd` ブロックが追加され、`sotp track type-signals` が `--layer` flag を受け付ける (T007)
- [x] `verify_from_spec_json` と `check_strict_merge_gate` が全 `tddd.enabled` 層を AND 集約する (T007)
- [x] merge gate U19-U26 テスト (2-layer 組み合わせ) が pass する (T007: 1841 tests total)
- [x] `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` Status が `Accepted` に更新されている (T007)

## Verification Results

### T001 — Domain rename + catalogue split (commit 6ab44f6)
- Review: fast + full model zero_findings on domain / infrastructure / other scopes
- CI: passed, 1825 tests

### T002 — Infrastructure rename (commit 6a67e77)
- Review: zero_findings
- CI: passed

### T003 — Usecase / CLI / alias removal (commit 4a18172)
- Review: zero_findings
- CI: passed, domain-type-signals → type-signals subcommand rename

### T004 — TypeGraph extension (commit 3df2ac7)
- Review: fast + full model zero_findings on 3 scopes (domain / infrastructure / other)
- CI: passed, 1825 tests
- `FunctionInfo::has_self_receiver` derived from `receiver` during review fix
- Generic type impl matching fix (`base_name` replaces `last_segment` for `Foo<T>` → `Foo`)
- `::` in array length / const-generic expressions normalized to `.`

### T005 — Baseline schema v2 (commit 15f56d0)
- Review: fast + full model zero_findings on 3 scopes
- CI: passed, 1823 tests
- `TypeNode::new` is now 4-arg (legacy `method_return_types` bridge removed)
- `TraitNode::method_names()` accessor removed
- v1 baseline rejection with rerun hint verified

### T006 — L1 signature check (commit 3fed54d)
- Review: fast + full model zero_findings on 4 scopes (domain / infrastructure / cli / other)
- CI: passed, 1823 tests
- Catalogue schema v2, top-level key `type_definitions`
- TraitPort L1 six-axis forward check + reverse check implemented
- `::` rejection enforced at both encode and decode boundaries

### T007 — Multilayer + ADR Accepted (commit 840541d)
- Review: fast + full model zero_findings on 5 scopes (usecase / infrastructure / cli / harness-policy / other)
- CI: passed, 1841 tests (8 new U19-U26 merge gate tests)
- `read_enabled_layers(branch)` port method wired through merge_gate_adapter
- Infrastructure adapter reads `architecture-rules.json` from PR branch blob
- Phase 1 scope: `domain` layer is wired end-to-end; `usecase` is declared enabled but
  `usecase-types.json` does not yet exist and is treated as NotFound opt-out per layer.
  Non-`domain` `--layer` values on `sotp track type-signals` are fail-closed rejected.
- ADR Status flipped to
  `Accepted (implemented in track tddd-01-multilayer-2026-04-12, 2026-04-13)`.

## Manual Verification Steps (executed during track loop)

- `cargo make ci` passes after every task commit.
- `bin/sotp track type-signals tddd-01-multilayer-2026-04-12` reports
  `blue=4 yellow=0 red=0` (4 declared reference entries: `ParamDeclaration`,
  `MethodDeclaration`, `MemberDeclaration`, `Finding`).
- `bin/sotp track baseline-capture tddd-01-multilayer-2026-04-12 --force`
  regenerates `domain-types-baseline.json` at schema v2.
- `cargo make track-check-approved -- --track-id tddd-01-multilayer-2026-04-12`
  reports Approved after every commit.

## Result / Open Issues

- **Phase 2 follow-ups**:
  - L2 generics / cross-layer 参照検証 — 本 track の対象外、将来の track で対応
  - `async-trait` proc-macro desugar の `is_async=false` 扱い — ADR Consequences で言及済み
  - Same-name collision (`domain::verify::Finding` / `domain::review_v2::types::Finding`) は
    reference entry で suppression。TypeGraph 構築時の非決定的 HashMap 順序に起因するため、
    将来 warning を Red signal に昇格する follow-up track が望ましい
  - `usecase-types.json` の作成 — Phase 1 では `tddd.enabled=true` だけ先行して宣言し、
    実カタログ作成は別 track に分離
  - `sotp track baseline-capture --layer <non-domain>` の実装 — Phase 1 は forward-compat
    stub、Phase 2 で per-layer ファイル名を導出して capture する

- **CI 時間**: `tddd.enabled` 層が増えると `cargo +nightly rustdoc` の呼び出し回数が
  線形に増える。キャッシュ戦略の見直しは follow-up。

- **MemberDeclaration JSON 直列化**: 現状は `serde(tag="kind")` による `{"kind":"variant","name":"X"}`
  / `{"kind":"field","name":"f","ty":"T"}` 形式。将来の外部ツール互換性のため format は
  凍結済みとみなす。

## verified_at

- 2026-04-13 (all 7 tasks committed, reviewed, CI passing; Phase 1 scope satisfied)
