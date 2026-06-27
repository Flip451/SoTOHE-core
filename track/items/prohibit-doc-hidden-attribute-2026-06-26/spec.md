<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 24, yellow: 0, red: 0 }
---

# `#[doc(hidden)]` 属性の宣言を syn AST 走査で一律禁止する

## Goal

- [GO-01] 新サブコマンド `sotp verify doc-hidden` を導入し、`architecture-rules.json` の `layers[]` 対象 crate 配下の全 `.rs` ソースから `#[doc(hidden)]` 相当の属性宣言を syn AST 走査で検出して CI (`ci-local`) でブロックする。スコープは `layers[]` を実行時に読み取ることで自動的に全層に及ぶ。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2, knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D1]
- [GO-02] `#[doc(hidden)]` に起因する TDDD chain ③ の DanglingId 副作用検出（エラーメッセージが不明瞭で根本原因を伝えられない）を、属性名を明示する専用ゲートエラーに置き換え、開発者がメッセージを見た時点で対処方針（属性を外すか item を削除する）を判断できるようにする。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2]

## Scope

### In Scope
- [IN-01] 新 `sotp verify doc-hidden` サブコマンドの追加と、`Makefile.toml` の `ci-local` dependencies への組み込み。`VerifyCommand` enum バリアント / dispatch / gate-name ラベル / `verify/mod.rs` への公開 / `verify-doc-hidden-local` Makefile タスク配線を含む。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T002, T003]
- [IN-02] 以下の全形式の `#[doc(hidden)]` 相当属性の syn AST 検出: 直接の outer `#[doc(hidden)]`、直接の inner `#![doc(hidden)]`、複合 doc 引数（順序不問）`#[doc(hidden, alias = "x")]` / `#[doc(alias = "x", hidden)]`、`cfg_attr` 包み outer `#[cfg_attr(pred, doc(hidden))]` / `#[cfg_attr(pred, doc(hidden, ...))]`、`cfg_attr` 包み inner `#![cfg_attr(pred, doc(hidden))]`。`#[doc = "..."]` name-value 形式は対象外とする。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T001]
- [IN-04] `libs/infrastructure/src/verify/` 配下に共通 syn 走査ヘルパ（各 layer crate ディレクトリ配下の `.rs` ファイル再帰発見 → `syn::parse_file` → コールバック visitor による判定）を新設する。判定ロジックはコールバックで受ける設計とする。後続の syn ベース lint（例: `Result<_, String>` 禁止）でも再利用できる粒度とする。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D1] [tasks: T001]
- [IN-05] ゲートの有効化スコープを `architecture-rules.json` の `layers[]` から実行時に取得する。per-layer フラグや特定 crate パスのハードコードは持たず、`layers[]` に新 crate が追加されれば自動的に対象に含まれる。走査対象は各 layer crate ディレクトリ配下の `.rs` ファイル全体とし、`tests/` / `examples/` / `benches/` も除外しない。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T001]

### Out of Scope
- [OS-01] 既存 verify ゲート（`usecase-purity` / `domain-purity` / `canonical-modules` / type-signals）への変更・相乗り。本トラックは独立サブコマンドの新設にとどまり、既存ゲートのロジック・インターフェースには触れない。既存ゲートを共通走査基盤へ乗せ替える将来的な改善も対象外とする。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D3] [tasks: T001, T002]
- [OS-02] per-layer（per-crate）ごとのゲート有効・無効フラグ。スコープは `layers[]` で一律に決まり、個別ハードコードや設定による除外機構は提供しない。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T001]
- [OS-03] 有効可視性（effective visibility）解析に基づく `pub` item 限定検出。impl block 属性伝播 / pub-reachable 集合 / cross-file `pub use` 再エクスポートグラフ / `#[path]` を辿るモジュール解決などのサブ問題を伴う。可視性不問の一律禁止によりこれらのサブ問題をすべて回避する。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T001]
- [OS-04] 既存コードベースに存在する `#[doc(hidden)]` の cleanup（attribute 除去 / item 削除）。cleanup の要否確認（grep による実例の有無の確認）と実施は実装着手時の別作業であり、本 spec のゲート導入とは切り離す。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D3] [tasks: T003]
- [OS-05] dylint カスタム lint による実装。`#[doc(hidden)]` 検出に rustc HIR / 型解決は不要であり、stable syn AST 走査で十分。dylint は nightly `rustc_private` 依存でオーバースペックとなる。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T001]

## Constraints
- [CN-01] 検出ロジックは syn AST 走査で実装する。行ベースの raw text regex は使わない。syn パースにより comment / 文字列リテラル / トークン境界を正確に扱い、複数行属性や `cfg_attr` 絡みの偽陽性・偽陰性を構造的に排除する。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D1, knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T001]
- [CN-02] 検出は可視性（`pub` / `pub(crate)` / 非 pub）を問わず全 item に適用する。テストコード、integration tests、examples、benches も除外しない。有効可視性の機械解析は行わず、属性の有無のみで判定する。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T001]
- [CN-03] ゲートのスコープは `architecture-rules.json` の `layers[]` を実行時に参照して決定する。特定 crate のパスや名前をコードにハードコードしない。`layers[]` に新 crate が追加されれば自動的にスコープに含まれる（per-layer フラグなし）。走査ディレクトリは各 layer crate ディレクトリとし、その配下の `.rs` ファイルを対象とする。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T001, T002]
- [CN-04] 既存の verify ゲート（`usecase_purity` / `canonical_modules` / type-signals 等）のロジックおよびインターフェースに変更を加えない。共通走査ヘルパを新設するが、既存ゲートを共通基盤へ乗せ替えることは本トラックのスコープ外とする。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D3] [tasks: T001, T002, T003]
- [CN-05] ゲートのエラー出力は `#[doc(hidden)]` 属性の存在をファイルパス・行情報とともに明示するメッセージを含む。DanglingId のように根本原因を隠すメッセージは許容しない。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T001]
- [CN-06] 実装全体は stable Rust toolchain のみで動作する。nightly や dylint の nightly `rustc_private` 依存は使わない。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] `sotp verify doc-hidden` が `VerifyCommand` enum のバリアントとして登録されており、`#[doc(hidden)]` のないクリーンなソースに対して `[OK] All checks passed.` を出力して exit 0 を返す。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T001, T002, T003]
- [ ] [AC-02] `layers[]` のいずれかの crate の任意の Rust ソース（テストコード、integration tests、examples、benches を含む）に `#[doc(hidden)]` 相当の属性が存在する場合、ゲートが exit non-zero を返し、`#[doc(hidden)]` 属性の存在をファイルパス・行情報とともに示す finding を出力する。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T001]
- [ ] [AC-04] IN-02 で列挙した全属性形式（outer / inner / 複合 doc 引数（順序不問）/ cfg_attr 包み outer / cfg_attr 包み inner）が AC-02 の検出対象に含まれ、いずれの形式でも finding が出力される。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T001]
- [ ] [AC-05] `verify-doc-hidden-local` タスクが `Makefile.toml` の `ci-local` dependencies 配列に含まれ、`cargo make ci` 実行時にゲートが自動的に走る。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T003]
- [ ] [AC-06] ゲートの走査対象 crate リストが `architecture-rules.json` の `layers[]` を実行時に読み取って決定される。特定 crate パスがコードにハードコードされていない。`layers[]` に新 crate を追加するだけでゲートのスコープが自動的に広がる。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D2] [tasks: T001]
- [ ] [AC-07] IN-04 で導入した共通走査ヘルパが、後続の syn ベース lint（例: `Result<_, String>` 禁止）から再利用できる設計になっている。具体的には、各 layer crate ディレクトリ配下のファイル再帰発見・`syn::parse_file` を共通基盤とし、判定ロジックのみをコールバックで差し替えられること。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D1] [tasks: T001]
- [ ] [AC-08] 既存の verify ゲート（`usecase-purity` / `domain-purity` / `canonical-modules`）の実装・インターフェースが変更されておらず、`cargo make ci` でこれらのゲートが引き続き pass する。 [adr: knowledge/adr/2026-06-26-0810-prohibit-doc-hidden-attribute.md#D3] [tasks: T001, T002, T003]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/responsibility-boundary.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Decision Flow

## Signal Summary

### Stage 1: Spec Signals
🔵 24  🟡 0  🔴 0

