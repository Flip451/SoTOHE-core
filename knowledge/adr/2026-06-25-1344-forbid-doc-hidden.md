---
adr_id: 2026-06-25-1344-forbid-doc-hidden
decisions:
  - id: D1
    user_decision_ref: "chat:session_01HoKrcHzBhzCRU62NHLzXAC:2026-06-25"
    status: proposed
  - id: D2
    user_decision_ref: "chat:session_01HoKrcHzBhzCRU62NHLzXAC:2026-06-25"
    candidate_selection: "from:[clippy-disallowed-types,canonical-regex,type-signals-integration,dylint] chose:syn-independent-gate"
    status: proposed
  - id: D3
    user_decision_ref: "chat:session_01HoKrcHzBhzCRU62NHLzXAC:2026-06-25"
    status: proposed
---
# #[doc(hidden)] の禁止を独立 syn 走査ゲートで機械化する

## Context

`pub` + `#[doc(hidden)]` は、rustdoc の paths 除外を介して TDDD chain ③（type-signals / `calc-impl-catalog`）で DanglingId を発火させ、track-active-gate（コミットゲート）を不正に阻む。これを新規に書けないよう機械ゲートで禁止したい。

既存の機械ゲートを調査した結果、(a) `Cargo.toml` の `[workspace.lints.clippy]` が `unwrap` / `panic` / `indexing_slicing` 等を deny、(b) `sotp verify` に 2 系統のソース走査ゲート（syn AST ベースの purity = domain-purity / usecase-purity、regex 行ベースの canonical-modules）が存在することが分かった。`#[doc(hidden)]` の検出は属性の有無を見るだけなので syn AST 走査が最適である（clippy の `disallowed_types` は型パス用で属性を検出できない）。

「ソースを走査して違反を検出しエラーを吐く」型のゲートだが、既存ゲートはそれぞれ固有の責務（I/O 純粋性 / 再実装禁止 / 型シグネチャ追跡）を持つため、新ルールを相乗りさせると責務が混濁する。本 ADR で導入する syn ベースの共通走査ヘルパは、後続の同種 lint（例: `Result<_, String>` 禁止）でも再利用する。

## Decision

### D1: syn ベースの共通走査ヘルパを新設する

「`.rs` 再帰発見 → `syn::parse_file` → `#[cfg(test)]` / `#[test]` 除外」の走査骨格を共通モジュールに集約する。既存 `verify/syn_helpers.rs` の `has_cfg_test_attr` を流用し、判定ロジック（visitor）はコールバックで受ける。本ヘルパは `#[doc(hidden)]` ゲートの基盤であると同時に、後続の syn ベース lint でも再利用する。

### D2: #[doc(hidden)] 禁止を独立サブコマンドとして実装する

新サブコマンド（例 `sotp verify doc-hidden`）を追加する。`pub` item に `#[doc(hidden)]` が付くものを error とする。検出ロジックは type-signals から独立させ、禁止理由（rustdoc paths 除外 → DanglingId → track-active-gate ブロック）はエラーメッセージと規約 doc で説明する。有効化スコープは `architecture-rules.json` の `layers[]` に列挙された全 crate を一律対象とし（テストコードは syn の `#[cfg(test)]` / `#[test]` 除外により対象外）、per-layer フラグや層のハードコード選択は持たない。新 crate が `layers[]` に追加されれば自動的に対象になる。

### D3: 既存ゲート機構は不可侵とする

`usecase_purity`（I/O 純粋性）/ `canonical_modules`（再実装禁止）/ type-signals（型シグネチャ追跡）には手を入れない。新機能を既存の責務に相乗りさせない。既存ゲートを共通走査基盤へ乗せ替えるのは将来の任意改善とし、本決定のスコープ外とする。

## Rejected Alternatives

### A. clippy で禁止する

clippy には属性の有無で発火する汎用 lint 設定がなく、`disallowed_types` は型パス用で `#[doc(hidden)]` 属性を検出できない。

### B. canonical_modules の regex ルールに相乗りする

行単位 raw text regex のため comment / 文字列リテラルを誤検知し、複数行属性や `cfg` 絡みに脆い。スキーマも concern / owner / allowed_in 前提で、属性の禁止に意味的に不整合。

### C. type-signals に統合する

検出を type-signals の責務に混ぜる混濁。現状の DanglingId は副作用的検出でエラーメッセージが不明瞭（「`#[doc(hidden)]` を外せ」と示さない）。独立ゲート＋明示メッセージの方が原因を伝えられる。

### D. dylint でカスタム lint を書く

属性検出に rustc HIR / 型解決は不要で、nightly `rustc_private` 依存の dylint はオーバースペック。syn AST 走査で十分かつ stable のまま実装できる。

## Consequences

### Positive

- `#[doc(hidden)]` が commit gate（`ci-local`）で機械検出される。
- syn AST 走査により comment / 文字列 / test コードを誤検知しない。
- doc-hidden の不明瞭な DanglingId 副作用検出が、明示エラーに置き換わる。
- 共通走査ヘルパを導入し、後続の syn ベース lint の追加コストを下げる。stable toolchain を維持。
- 責務分離：独立サブコマンドで既存機構は無傷。

### Negative

- 新サブコマンド分の配線コスト（`VerifyCommand` enum / dispatch / `items_dir` / `CliApp` メソッド / gate-name ラベル / `verify/mod.rs` / Makefile `verify-*-local` / `ci-local` 依存配列）。

### Neutral

- doc-hidden の独立検出と type-signals の DanglingId が二重検出になり得る。実装着手時にメッセージ衝突の有無を確認する。
- 共通走査ヘルパの API は後続 lint（`Result<_, String>` 禁止等）の要件も見据える必要があるが、過剰一般化は避け、当面は doc-hidden の要件で確定する。

## Reassess When

- type-signals が `#[doc(hidden)]` を正式に扱うようになり、独立ゲートが冗長化したとき。
- 全層一律の方針が特定層で過剰と判明し、層別の有効化制御（per-layer 設定）が必要になったとき。
- 属性検出に型解決が必要な要件が生じ、syn では不足するとき。

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/coding-principles.md` — コーディング規約
