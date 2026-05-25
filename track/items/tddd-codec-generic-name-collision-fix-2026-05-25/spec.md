<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 18, yellow: 0, red: 0 }
---

# 型シグネチャ codec の generic param 名前衝突の恒久対策

## Goal

- [GO-01] `build_generic_canon_map` および関連する canon map 構築において、同名の合成 generic param を位置（index）で識別するよう変更し、`impl Trait` 引数が複数ある場合の HashMap 上の名前衝突を解消する。あわせて `format_type_with_canon` で `Type::ImplTrait` を位置 placeholder として描画することで、C 側（rustdoc 由来）の `Type::Generic` 表現と一致させ、`impl Trait` 引数を持つ signature を明示ジェネリクスへ書き換えることなく catalogue 宣言できるようにする [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1]

## Scope

### In Scope
- [IN-01] `libs/infrastructure/src/tddd/signal_evaluator_v2/format.rs` の `build_generic_canon_map` 関数を変更する: 同名の合成 generic param（rustdoc が `impl Trait` 引数を desugar して生成する param）が複数存在する場合に、名前ではなく宣言順の位置（0-based index）で識別し、`"#0"`, `"#1"`, … の placeholder を割り当てる。これにより `fn(a: impl Into<String>, b: impl Into<String>)` のような signature で生じる `"impl Into<String>"` キーの衝突が解消される [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1] [tasks: T001]
- [IN-02] `libs/infrastructure/src/tddd/signal_evaluator_v2/format.rs` の `format_type_with_canon`（および `Type::ImplTrait` を扱う関連ヘルパー）を変更する: A 側（catalogue codec）が `Type::ImplTrait` を描画する際に、bound 文字列のリテラル (`"impl Into<String>"`) ではなく、canon map 経由の位置 placeholder（`"#0"`, `"#1"`, …）として出力する。これにより C 側が生成する `Type::Generic("impl Into<String>")` の位置 placeholder 表現と対称になる [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1] [tasks: T002]
- [IN-03] `libs/infrastructure/src/tddd/signal_evaluator_v2/generics_eq.rs` の `build_generic_canon_map` 利用箇所（`fn_sigs_structurally_equal`, `build_combined_canon_map`, `build_where_form_view` 等）を、変更後の `build_generic_canon_map` に対応させる: 位置ベースの canon map が正しく伝播し、`generics_structurally_equal` および `fn_sigs_structurally_equal` の比較が `impl Trait` 引数について対称になることを確認する [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1] [tasks: T003]
- [IN-04] `impl Trait` 引数を持つ signature に対するユニットテストを追加する: 単一の `impl Trait` 引数（`fn(a: impl Into<String>)`）と、同名複数の `impl Trait` 引数（`fn(a: impl Into<String>, b: impl Into<String>)`）の両パターンで、A 側 codec 出力と C 側 rustdoc 出力が構造等値と判定されることを確認する [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1] [tasks: T004]

### Out of Scope
- [OS-01] adopter 側の source 変更（明示ジェネリクス化）の差し戻し: reviewer-claude-option-2026-05-24 track（PR #138）で `impl Into<String>` を `<M: Into<String>, B: Into<String>>` に書き換えた workaround の revert は本 track のスコープ外とする。codec 修正後に adopter 側の source をどう扱うかは別途判断する [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1]
- [OS-02] signal_evaluator_v2 の canon map 構築の全面再設計: 本 track は `build_generic_canon_map` の名前衝突を位置ベース識別で解消する最小限の修正に留める。canon map 全体の struct 化や公開 API の変更を伴う再設計は別 ADR・別 track の対象とする [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1]
- [OS-03] catalogue schema の変更: `MethodDeclaration` や `FunctionEntry` の schema フィールドへの追加・変更は行わない。本 track は codec の内部描画ロジックのみを修正する [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1]
- [OS-04] rustdoc が `impl Trait` 合成 param の命名規則を変更した場合の対応: rustdoc 側の変更による名前衝突のそもそもの解消は本 track のスコープ外とする（ADR の Reassess When に記載） [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1]

## Constraints
- [CN-01] 変更は `libs/infrastructure/src/tddd/signal_evaluator_v2/` 配下の `format.rs` と `generics_eq.rs`（および必要に応じてそれらを利用する同ディレクトリ内のファイル）に閉じる。他レイヤー（domain / usecase / apps）の型や公開 API を変更しない [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1] [tasks: T001, T002, T003, T004]
- [CN-02] 位置ベースの識別導入により param の順序に依存する新たな偽陰性・偽陽性が生じないよう、テストで param 順序パターンを網羅する。ADR Consequences §Negative に記載の通り、順序依存バグの混入リスクに対してテストで対処する [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1] [tasks: T003, T004]
- [CN-03] 単一の `impl Trait` 引数（名前衝突が起きないケース）においても、A 側描画と C 側表現の非対称（ImplTrait リテラル vs 位置 placeholder）を解消する。名前衝突の有無に関わらず対称性を保証する [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1] [tasks: T002]
- [CN-04] per-adopter の明示ジェネリクス回避（rejected alternative A）および phantom-generic encode（rejected alternative B）はいずれも採用しない。codec の修正によって catalogue が言語仕様を忠実に表現できる状態を実現する [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1] [tasks: T001, T002]

## Acceptance Criteria
- [ ] [AC-01] 同名の `impl Trait` 引数を複数持つ signature（例: `fn(a: impl Into<String>, b: impl Into<String>)`）を catalogue で忠実に宣言した場合に、signal evaluator v2 が 🔵 Blue と評価する。修正前は HashMap 衝突により 🟡/🔴 になっていたことをテストで確認する [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1] [tasks: T004]
- [ ] [AC-02] 単一の `impl Trait` 引数を持つ signature（例: `fn(a: impl Into<String>)`）を catalogue で忠実に宣言した場合に、signal evaluator v2 が 🔵 Blue と評価する。修正前は ImplTrait リテラル vs 位置 placeholder の非対称により 🟡/🔴 になっていたことをテストで確認する [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1] [tasks: T004]
- [ ] [AC-03] `impl Trait` 引数の順序が異なる signature（例: `fn(a: impl Display, b: impl Into<String>)` と `fn(a: impl Into<String>, b: impl Display)`）は構造不一致（🔴）と評価される。位置ベース識別が param 順序を正しく反映することをテストで確認する [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1] [tasks: T004]
- [ ] [AC-04] 既存の generic param 比較テスト（`generics_eq.rs` のテスト群）がすべて pass する。位置ベース識別の導入によって、`impl Trait` 以外の通常の generic param（`T`, `U` 等）の比較挙動にリグレッションが生じない [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1] [tasks: T004]
- [ ] [AC-05] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する [adr: knowledge/adr/2026-05-25-0423-tddd-codec-generic-name-collision-fix.md#D1] [tasks: T004]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/source-attribution.md#Source Tag Types
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 18  🟡 0  🔴 0

