<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 10, yellow: 0, red: 0 }
---

# TDDD chain ③ の cargo rustdoc 呼び出しに --document-hidden-items を追加する

## Goal

- [GO-01] TDDD chain ③ の `cargo rustdoc` 呼び出しに `--document-hidden-items` を付与することで、`pub` かつ `#[doc(hidden)]` な要素が rustdoc paths に含まれるようになり、catalogue 突合での `DanglingId` Yellow/Red 発火を構造的に解消して track-active-gate のブロックを防ぐ。`#[doc(hidden)]` を semver hazard hiding / internal API marker / unstable API 非公開などの本来用途で引き続き使用できるようになる。 [adr: knowledge/adr/2026-06-27-0440-tddd-rustdoc-document-hidden-items.md#D1]

## Scope

### In Scope
- [IN-01] `libs/infrastructure/src/schema_export/bin_target.rs` の `run_rustdoc` 関数内 `args` クロージャが `cargo rustdoc` に渡すフラグ列 (`["--", "-Z", "unstable-options", "--output-format", "json"]`) に `"--document-hidden-items"` を追加する。`--lib` パスと `--bin` パスは同一の `args` クロージャを共有しているため、この 1 箇所の変更が baseline capture (TypeGraph B 取得) と actual capture (TypeGraph C 取得) の両経路に適用される。 [adr: knowledge/adr/2026-06-27-0440-tddd-rustdoc-document-hidden-items.md#D1, knowledge/adr/2026-06-27-0440-tddd-rustdoc-document-hidden-items.md#D2] [tasks: T001]

### Out of Scope
- [OS-01] user-facing `cargo doc` / `docs.rs` への `--document-hidden-items` 適用。本変更は chain ③ 内部の catalogue 生成 (`calc-impl-catalog`) に限定し、プロジェクト全体の rustdoc 設定や公開 doc 生成には適用しない。 [adr: knowledge/adr/2026-06-27-0440-tddd-rustdoc-document-hidden-items.md#D2]
- [OS-02] source レベルの `#[doc(hidden)]` 禁止 gate の導入。`syn` AST scanner、grep ベース scanner、コンパイラ lint いずれの形態の source-level 禁止機構も本 track では導入しない。 [adr: knowledge/adr/2026-06-27-0440-tddd-rustdoc-document-hidden-items.md#D3]
- [OS-03] 機械チェックなしの convention-only による `#[doc(hidden)]` 抑止。rustdoc invocation 側での構造的解消 (D1) で chain ③ の DanglingId は防げるため、convention-only 運用は採用しない。 [adr: knowledge/adr/2026-06-27-0440-tddd-rustdoc-document-hidden-items.md#D1, knowledge/adr/2026-06-27-0440-tddd-rustdoc-document-hidden-items.md#D3]

## Constraints
- [CN-01] flag の適用は `libs/infrastructure/src/schema_export/bin_target.rs` の `run_rustdoc` が構築する `args` クロージャ 1 箇所のみとし、`RustdocSchemaExporter` 呼び出し側 (`baseline_capture.rs`, `rustdoc_baseline_capture_adapter.rs`, `type_signals_evaluator.rs`) は変更しない。 [adr: knowledge/adr/2026-06-27-0440-tddd-rustdoc-document-hidden-items.md#D2] [tasks: T001]
- [CN-02] 新規 toolchain 依存を導入しない。`--document-hidden-items` は `-Z unstable-options` 傘下の nightly 限定 feature であり、chain ③ が既に要求する nightly + `-Z unstable-options` に追加 toolchain コストなく付与できる。安定 Rust での `cargo build` / `cargo test` には影響しない。 [adr: knowledge/adr/2026-06-27-0440-tddd-rustdoc-document-hidden-items.md#D1] [conv: knowledge/conventions/nightly-dev-tool.md#Rules] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] `run_rustdoc` の `args` クロージャが返すフラグ列に `"--document-hidden-items"` が含まれることをユニットテストで確認できる。 [adr: knowledge/adr/2026-06-27-0440-tddd-rustdoc-document-hidden-items.md#D1] [tasks: T001]
- [ ] [AC-02] `pub` かつ `#[doc(hidden)]` な要素を含む対象 layer crate に対して `bin/sotp signal calc-impl-catalog` を実行したとき、当該要素に起因する `DanglingId` Yellow/Red シグナルが出力されない。 [adr: knowledge/adr/2026-06-27-0440-tddd-rustdoc-document-hidden-items.md#D1] [tasks: T001]
- [ ] [AC-03] `cargo make ci` (fmt-check + clippy + nextest + deny + check-layers + verify-*) が pass し、既存テストへのリグレッションがない。 [adr: knowledge/adr/2026-06-27-0440-tddd-rustdoc-document-hidden-items.md#D1] [tasks: T001]

## Related Conventions (Required Reading)
- knowledge/conventions/nightly-dev-tool.md#Rules
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies

## Signal Summary

### Stage 1: Spec Signals
🔵 10  🟡 0  🔴 0

