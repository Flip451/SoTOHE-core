<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 25, yellow: 0, red: 0 }
---

# `#[doc(hidden)]` の禁止を独立 syn 走査ゲートで機械化する

## Goal

- [GO-01] `libs/infrastructure/src/verify/` に共通 syn 走査フレームワークを整備する。`.rs` ファイルの再帰的発見、`syn::parse_file` による AST 解析、`#[cfg(test)]` / `#[test]` によるテストコードの除外、および判定ロジックをコールバックで受け取る骨格を提供し、`#[doc(hidden)]` ゲートだけでなく後続の syn ベース lint でも再利用できるようにする。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D1]
- [GO-02] `sotp verify doc-hidden` サブコマンドを実装する。`architecture-rules.json` の `layers[]` に列挙された全クレートのプロダクションコードを走査し、`pub` アイテムに `#[doc(hidden)]` が付いている場合をエラーとして報告する。エラーメッセージはファイルパス・アイテム名・禁止理由（rustdoc paths 除外 → TDDD chain ③ DanglingId 発火 → track-active-gate ブロック）を含む。ゲートは `cargo make ci` に組み込まれ、コミット前に自動検出する。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2]
- [GO-03] 既存の verify ゲート（usecase-purity / domain-purity / canonical-modules / type-signals）は一切変更せず、それぞれの責務を維持する。新ゲートはこれらと責務が分離した独立サブコマンドとして実装し、既存機構への相乗りをしない。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D3]

## Scope

### In Scope
- [IN-01] `libs/infrastructure/src/verify/syn_helpers.rs` に、`.rs` ファイル再帰発見とコールバックベースの syn 走査フレームワークを追加する。既存の `has_cfg_test_attr` を活用してテストスコープ除外を共通化し、後続の syn ベース lint モジュールが同フレームワークを呼び出すことで、ファイル発見・AST 解析・テスト除外のロジックを重複なく共有できるようにする。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D1] [tasks: T001]
- [IN-02] `libs/infrastructure/src/verify/doc_hidden.rs` モジュールを新設し、`pub + #[doc(hidden)]` の検出ロジックを実装する。検出ロジックは IN-01 の走査フレームワークにコールバックとして渡す形とし、ファイル発見・テスト除外の重複実装を避ける。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2] [tasks: T001]
- [IN-03] `sotp verify doc-hidden` コマンドを `sotp verify` サブコマンド群に追加する。既存の `VerifyCommand` dispatch 機構に組み込み、`items_dir` / gate-name ラベル等の既存配線パターンに倣って実装する。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2] [tasks: T001]
- [IN-04] `Makefile.toml` に `verify-doc-hidden-local` タスクと Docker ラッパーを追加し、`ci-local` の dependency 配列に組み込む。`cargo make ci` の実行で `sotp verify doc-hidden` が自動的に呼ばれ、違反があれば CI がブロックされる。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2] [tasks: T001]
- [IN-05] ゲートのスキャン対象クレートは実行時に `architecture-rules.json` の `layers[]` から読み取る。クレートパスのハードコードを持たず、`layers[]` に新規クレートが追加された場合はコード変更なしに自動的にスキャン対象となる。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2] [tasks: T001]
- [IN-06] プロダクションコードのみをスキャン対象とする。`#[cfg(test)]` で囲まれたブロックおよび `#[test]` 属性を持つ関数は syn AST レベルで除外し、正規表現ではなく AST 構造により誤検知を防ぐ。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D1, knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2] [tasks: T001]

### Out of Scope
- [OS-01] per-layer または per-crate の有効化フラグ（`layers[]` エントリ個別の doc-hidden 無効化設定等）は実装しない。全 `layers[]` クレートを一律対象とし、除外したい場合は `pub` 修飾子または `#[doc(hidden)]` 属性を取り除くことで対処する。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2]
- [OS-02] 既存の verify ゲート（`usecase_purity` / `domain_purity` / `canonical_modules` / type-signals）を新共通走査フレームワークへ乗せ替える移行作業は本トラックのスコープ外とする。将来の任意改善として留保する。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D3]
- [OS-03] dylint によるカスタム rustc lint の実装は行わない。`#[doc(hidden)]` 属性の検出に型解決・HIR は不要であり、syn AST 走査で十分かつ stable toolchain のまま実装できる。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2]
- [OS-04] `#[doc(hidden)]` が付いた非 pub アイテム（`pub(crate)` / `pub(super)` / プライベートアイテム）の検出は本ゲートの対象外。禁止対象は `pub + #[doc(hidden)]` の組み合わせのみとし、非 pub への適用は rustdoc paths 除外を引き起こさないため問題にならない。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2]

## Constraints
- [CN-01] `doc_hidden.rs` モジュールおよびその走査フレームワークは、`canonical_modules` / `usecase_purity` / `domain_purity` / type-signals の各モジュールをインポートせず、これらのモジュールの実装を変更しない。新ゲートの追加が既存ゲートの動作に影響を与えてはならない。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D3] [tasks: T001]
- [CN-02] ゲートのスキャン対象は `architecture-rules.json` の `layers[]` を唯一のソースとして実行時に決定する。クレートパス・クレート名をコード内にハードコードしない。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2] [tasks: T001]
- [CN-03] 実装は stable Rust toolchain で完結する。nightly フィーチャー、`rustc_private` クレート、dylint は使用しない。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2] [tasks: T001]
- [CN-04] テストコードの除外は syn AST による構造的除外とする。行単位のテキスト走査や正規表現による `#[cfg(test)]` の heuristic 検出は使用しない。これにより comment や文字列リテラル中の `#[cfg(test)]` 記述、複数行属性、`cfg` ネストに対する誤検知を防ぐ。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D1, knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2] [tasks: T001]
- [CN-05] 違反時のエラーメッセージはファイルパスとアイテム名を特定し、禁止理由（`pub + #[doc(hidden)]` は rustdoc の paths 除外を引き起こし、TDDD chain ③ の DanglingId を誘発して track-active-gate（コミットゲート）を不正にブロックする）を含める。開発者がエラーメッセージのみで原因と修正方法を把握できるようにする。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] `sotp verify doc-hidden` を実行した結果、`layers[]` に属するプロダクションコードに `pub + #[doc(hidden)]` の組み合わせが存在する場合、非ゼロ exit code でエラーを返し、ファイルパス・アイテム名・禁止理由を含むメッセージを出力する。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2] [tasks: T001]
- [ ] [AC-02] `sotp verify doc-hidden` を実行した結果、`layers[]` に属する全プロダクションコードに `pub + #[doc(hidden)]` が存在しない場合、ゼロ exit code で正常終了する。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2] [tasks: T001]
- [ ] [AC-03] `architecture-rules.json` の `layers[]` に新規クレートを追加した後に `sotp verify doc-hidden` を実行すると、そのクレートも自動的にスキャン対象に含まれる。コード変更なしに対象クレートが拡張される。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2] [tasks: T001]
- [ ] [AC-04] `#[cfg(test)]` ブロック内または `#[test]` 属性を持つ関数内の `pub + #[doc(hidden)]` はエラーとして報告されない（テストコードは除外される）。syn AST による構造的除外のため、コメントや文字列リテラル中の記述を誤検知しない。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D1, knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2] [tasks: T001]
- [ ] [AC-05] `cargo make ci` の実行で `sotp verify doc-hidden` が呼ばれる。`Makefile.toml` の `ci-local` 依存配列に doc-hidden ゲートのタスクが含まれており、`pub + #[doc(hidden)]` 違反が存在する場合は CI がブロックされる。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D2] [tasks: T001]
- [ ] [AC-06] 本変更後も、`cargo make verify-usecase-purity` / `cargo make verify-domain-purity` / `cargo make verify-canonical-modules` の各タスクが変更なしに pass する。既存ゲートの実装・テスト・設定ファイルに変更が加えられていない。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D3] [tasks: T001]
- [ ] [AC-07] IN-01 の走査フレームワークを `doc_hidden.rs` が使用していることが単体テストで確認できる。フレームワークの API が doc-hidden 固有ロジックと分離されており、後続の syn ベース lint モジュールが同フレームワークを呼び出すことで同等の走査能力を獲得できる構造になっている。 [adr: knowledge/adr/2026-06-25-1344-forbid-doc-hidden.md#D1] [tasks: T001]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/responsibility-boundary.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 25  🟡 0  🔴 0

