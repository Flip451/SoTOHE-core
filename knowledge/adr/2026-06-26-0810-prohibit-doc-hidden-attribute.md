---
adr_id: 2026-06-26-0810-prohibit-doc-hidden-attribute
decisions:
  - id: D1
    user_decision_ref: "chat:session_01LZzkoFBfPHtNXvhWmjBox3:2026-06-26"
    status: proposed
  - id: D2
    user_decision_ref: "chat:session_01LZzkoFBfPHtNXvhWmjBox3:2026-06-26"
    candidate_selection: "from:[clippy-disallowed-types,canonical-regex,type-signals-integration,dylint,pub-only-detection] chose:syn-uniform-ban"
    status: proposed
  - id: D3
    user_decision_ref: "chat:session_01LZzkoFBfPHtNXvhWmjBox3:2026-06-26"
    status: proposed
---
# `#[doc(hidden)]` 属性の宣言を syn AST 走査で一律禁止する

## Context

`#[doc(hidden)]` 属性は rustdoc が生成する paths テーブルから対象 item を除外する。TDDD chain ③（type-signals / `calc-impl-catalog`）は rustdoc paths を入力に契約検証を行うため、`#[doc(hidden)]` が付いた item は paths から消えて DanglingId を発火させ、commit gate（track-active-gate）を不正に阻む。

機械検出を入れる際の選択軸は (a) 検出ロジックの単純さ、(b) 偽検知の少なさ、(c) 後方互換性 — の 3 つだが、これらは「禁止スコープ」の決定で大きく分岐する。

既存の機械ゲートを調査すると、`Cargo.toml` の `[workspace.lints.clippy]` が `unwrap` / `panic` / `indexing_slicing` 等を deny し、`sotp verify` には 2 系統のソース走査ゲート（syn AST ベースの purity = domain-purity / usecase-purity、regex 行ベースの canonical-modules）が存在する。`#[doc(hidden)]` の検出は属性の有無を見るだけなので syn AST 走査が最適で、clippy の `disallowed_types` は型パス用で属性を検出できない。

「ソースを走査して違反を検出しエラーを吐く」型のゲートだが、既存ゲートはそれぞれ固有の責務（I/O 純粋性 / 再実装禁止 / 型シグネチャ追跡）を持つため、新ルールを相乗りさせると責務が混濁する。本 ADR で導入する syn ベースの共通走査ヘルパは、後続の同種 lint（例: `Result<_, String>` 禁止）でも再利用する。

禁止スコープを「`pub` item に限定」する自然な拡張は、有効可視性（effective visibility）の機械決定を要求する。すなわち:

- `#[doc(hidden)] impl Foo { pub fn bar() }` のような impl block 属性は内部 item に伝播するため別判定が必要。
- `pub mod foo;` + `foo.rs` 先頭の inner `#![doc(hidden)]` は宣言面と本体面で属性が分かれるため file-level 解析が必要。
- `#[doc(hidden)] mod gen; pub use gen::Item;` の非 pub mod + 再エクスポート経路は cross-file `pub use` 解析が必要。
- `#[path]` / inline mod nested / file-backed `mod foo;` を辿るモジュール解決と推移閉包が必要。

これらは検出ロジック側に大きな状態（pub-reachable 集合、impl block propagation、再エクスポートグラフ）を要求し、属性形式や module 構造の組み合わせごとに edge case を生む。一方で「非 pub item の `#[doc(hidden)]`」は rustdoc paths にもともと現れないため **装飾として意味を持たない** — 後に pub 化したときに silent escape の経路となるだけで、書く正当な理由がない。

## Decision

### D1: syn ベースの共通走査ヘルパを新設する

「`.rs` 再帰発見 → `syn::parse_file` → `#[cfg(test)]` / `#[test]` 除外」の走査骨格を共通モジュールに集約する。既存 `verify/syn_helpers.rs` の `has_cfg_test_attr` を流用し、判定ロジック（visitor）はコールバックで受ける。本ヘルパは本 ADR のゲートの基盤であると同時に、後続の syn ベース lint でも再利用する。

### D2: `#[doc(hidden)]` 相当の属性宣言を可視性不問で一律禁止する

新サブコマンド（例 `sotp verify doc-hidden`）を追加し、ソース上に出現する `#[doc(hidden)]` 相当の属性宣言を可視性に関わらず error とする。検出対象は次の形式すべて:

- 直接の outer attribute: `#[doc(hidden)]`
- 直接の inner attribute: `#![doc(hidden)]`
- 複合 doc 引数（順序不問）: `#[doc(hidden, alias = "x")]` / `#[doc(alias = "x", hidden)]`
- `cfg_attr` 包み込み: `#[cfg_attr(<pred>, doc(hidden))]` / `#[cfg_attr(<pred>, doc(hidden, ...))]`
- 上記の inner attribute 版

判定対象は `syn::File::attrs` に現れる crate/file-level inner attribute と、syn traversal で到達する各 item / impl associated item の attribute とし、`pub` / `pub(crate)` / 非 pub の区別を行わない。Rust の doc comment は syn AST 上 `#[doc = "..."]` の name-value attribute として現れるため、検出器は `doc(hidden)` を含む list attribute（および `cfg_attr` 内の同形）だけを match し、`#[doc = "..."]` は対象外とする。

有効化スコープは `architecture-rules.json` の `layers[]` に列挙された全 crate を一律対象とし、per-layer フラグや層のハードコード選択は持たない。新 crate が `layers[]` に追加されれば自動的に対象になる。テストコードは `#[cfg(test)]` / `#[test]` の syn AST 除外で対象外（テスト item は rustdoc paths に出ないため doc(hidden) を付ける意味も無く、対象にしても実害は無いが、規約の明確化のため除外を維持する）。

非 pub item を含めて一律禁止することで、(a) effective visibility 解析（impl block propagation / pub-reachable 集合 / cross-file `pub use` 等）が一切不要となり、検出ロジックは「item の attribute だけを見る」 syn 単純走査で完結する。(b) 非 pub item の doc(hidden) は装飾として無意味なので失われる正当用途は無い。(c) 将来 pub 化したときに silent escape する経路を事前に閉じられる。

### D3: 既存ゲート機構は不可侵とする

`usecase_purity`（I/O 純粋性）/ `canonical_modules`（再実装禁止）/ type-signals（型シグネチャ追跡）には手を入れない。新機能を既存の責務に相乗りさせない。既存ゲートを共通走査基盤へ乗せ替えるのは将来の任意改善とし、本決定のスコープ外とする。

## Rejected Alternatives

### A. clippy で禁止する

clippy には属性の有無で発火する汎用 lint 設定がなく、`disallowed_types` は型パス用で `#[doc(hidden)]` 属性を検出できない。

### B. `canonical_modules` の regex ルールに相乗りする

行単位 raw text regex のため comment / 文字列リテラルを誤検知し、複数行属性や `cfg` 絡みに脆い。スキーマも concern / owner / allowed_in 前提で、属性の禁止に意味的に不整合。

### C. type-signals に統合する

検出を type-signals の責務に混ぜる混濁。現状の DanglingId は副作用的検出でエラーメッセージが不明瞭（「`#[doc(hidden)]` を外せ」と示さない）。独立ゲート＋明示メッセージの方が原因を伝えられる。

### D. dylint でカスタム lint を書く

属性検出に rustc HIR / 型解決は不要で、nightly `rustc_private` 依存の dylint はオーバースペック。syn AST 走査で十分かつ stable のまま実装できる。

### E. `pub` item のみに限定する

検出の意味的核は「rustdoc paths から消える item」なので最初の発想は `pub` 限定。だが effective visibility の機械決定を要求し、impl block の属性伝播、`pub mod` + inner attribute、非 pub mod + `pub use` 再エクスポート、`#[path]` / inline-nested を辿るモジュール解決、推移閉包など多数のサブ問題を抱え込む。属性形式や module 構造の組合せごとに edge case が増え、検出ロジックの複雑度が攻撃面と保守コストの両方を押し上げる。可視性不問の一律禁止に変えると、これらすべてのサブ問題が消滅し、実装は「attribute だけを見る」単純な syn 走査になる。失われる正当用途も無い（非 pub item の doc(hidden) は無意味）。

## Consequences

### Positive

- `#[doc(hidden)]` が commit gate（`ci-local`）で機械検出される。
- syn AST 走査により comment / 文字列 / test コードを誤検知しない。
- doc-hidden の不明瞭な DanglingId 副作用検出が、明示エラーに置き換わる。
- 検出ロジックが可視性解析を伴わない単純な item-attribute 走査に閉じ、属性形式や module 構造の組合せから来る edge case 連鎖を構造的に回避できる。
- 共通走査ヘルパを導入し、後続の syn ベース lint の追加コストを下げる。stable toolchain を維持。
- 責務分離：独立サブコマンドで既存機構は無傷。

### Negative

- 新サブコマンド分の配線コスト（`VerifyCommand` enum / dispatch / `items_dir` / `CliApp` メソッド / gate-name ラベル / `verify/mod.rs` / Makefile `verify-*-local` / `ci-local` 依存配列）。

### Neutral

- 既存コードベースに非 pub `#[doc(hidden)]` の実例がある場合、cleanup が必要（attribute を外すか、対象 item ごと削除）。実装着手時に grep で実例の有無を確認する。
- doc-hidden の独立検出と type-signals の DanglingId が二重検出になり得る。実装着手時にメッセージ衝突の有無を確認する。
- 共通走査ヘルパの API は後続 lint（`Result<_, String>` 禁止等）の要件も見据える必要があるが、過剰一般化は避け、当面は doc-hidden の要件で確定する。

## Reassess When

- type-signals が `#[doc(hidden)]` を正式に扱うようになり、独立ゲートが冗長化したとき。
- 属性検出に型解決が必要な要件が生じ、syn では不足するとき。
- ライブラリ作者向けに「非公開だが意図的に hidden 化したい」明確な正当用途が出現し、可視性別の例外運用が必要になったとき。

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/coding-principles.md` — コーディング規約
- `knowledge/conventions/enforce-by-mechanism.md` — 機械検証層の選択
