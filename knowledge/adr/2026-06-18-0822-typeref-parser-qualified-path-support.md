---
adr_id: 2026-06-18-0822-typeref-parser-qualified-path-support
decisions:
  - id: D1
    user_decision_ref: "chat_segment:2026-06-18:typeref-qualified-path-fix-direction"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:2026-06-18:typeref-qualified-path-fix-direction"
    status: proposed
---
# TypeRef パーサに QualifiedPath / impl_generics サポートを追加 — GAT projection + blanket impl の Blue 化を解禁

## Context

### §1 SoTOHE TDDD における catalogue ↔ rustdoc 比較の仕組み

SoTOHE の TDDD (Type-Driven Development Discipline) は、型カタログ (`<layer>-types.json`) に宣言した型シグネチャと、`rustdoc_types::Crate` が出力する実装側の型表現を文字列比較することで signal を決定する。catalogue の `params[].ty` フィールドに書かれた型文字列が rustdoc 側の format 結果と一致すれば 🔵 Blue、宣言が存在するが一致しなければ 🟡 Yellow、宣言がなければ 🔴 Red という 3 段階評価を行う。この比較は `libs/domain/src/tddd/baseline.rs::methods_structurally_equal` が文字列等価で実施するため、catalogue 側と rustdoc 出力側の文字列形式が 1 文字でも異なると一致しない。

### §2 GAT projection 型をめぐる非対称（D1 起因）

GAT (Generic Associated Types) を使ったメソッドシグネチャ、例えば

```rust
// <!-- illustrative, non-canonical -->
fn check(input: &Self::Input<'_>, strict: bool) -> VerifyOutcome;
```

において、rustdoc は `Self::Input<'_>` 部分を `rustdoc_types::Type::QualifiedPath { name: "Input", self_type: Box(Type::Generic("Self")), trait_: ..., args: ... }` として出力する。format layer の `format_qualified_path_with` (`libs/infrastructure/src/tddd/signal_evaluator_v2/format/ty_base.rs`) はこれを `<Self as ChainIdentity>::Input<'_>` 形式の文字列に変換する実装を既に持っている。

一方、TypeRef パーサ (`libs/infrastructure/src/tddd/type_ref_parser/parse_ctx.rs`) は catalogue に書かれた型文字列を `syn::TypePath` として受け取り、以下の実装で処理する:

```rust
// <!-- illustrative, non-canonical -->
// line 60-63
if type_path.qself.is_some() {
    return unresolved_type("<qualified_path>");
}
```

この分岐は `syn::TypePath.qself` が存在する、すなわち `<X as Trait>::Assoc` 形式のすべての qualified path を無条件に unresolved marker `"<qualified_path>"` で返す。結果として signal evaluator は catalogue 宣言側を `"<qualified_path>"` として比較しようとするが、rustdoc 出力側は `<Self as ChainIdentity>::Input<'_>` であるため文字列が一致せず 🟡 Yellow になる。

format layer が rustdoc 側の出力形式を正しく扱える実装を持つにもかかわらず、parser 側が catalogue 宣言を解析できていないという非対称な状態が 🟡 の直接原因である。

### §3 blanket impl をめぐる過剰拒絶（D2 起因）

GAT 付き trait（例: `SoTChain`）に対し、別 trait を境界にした blanket impl、例えば

```rust
// <!-- illustrative, non-canonical -->
impl<T: PersistedSoTChain> SoTChain for T { ... }
```

を catalogue で表現する際、`TraitImplDeclV2` は `for_type: "T"` のように記述する。catalogue schema v5 は `TraitImplDeclV2` に `impl_generics: Vec<MethodGenericParam>` と `impl_where_predicates: Vec<WherePredicateDecl>` フィールドを既に持っており（`libs/domain/src/tddd/catalogue_v2/traits.rs` line 99 / 105）、schema 拡張なしで impl-block の型パラメータと where 節を表現できる。

しかし TypeRef パーサが `for_type: "T"` を parse する際、`T` は単一セグメントの識別子であり、`convert_type_path` のレジストリ chain（Primitive → Known type → Local → Unresolved）のいずれにも一致しない。結果として `Type::ResolvedPath { path: "T", id: UNRESOLVED_CRATE_ID, args: None }` が返り、signal evaluator が `UnresolvedTypeRef("T")` として扱う。一方 rustdoc は `T` を `Type::Generic("T")` として報告するため、型の分類が食い違い 🔴 Red または 🟡 Yellow になる。

`structural_eq` は `Type::Generic("F")` の照合ロジックを既に持っている（`libs/infrastructure/src/tddd/signal_evaluator_v2/structural_eq.rs:1127`）。不足しているのは parser が `"T"` を impl_generics コンテキストと照合して `Type::Generic("T")` を返す経路のみである。

現状のワークアラウンドとして blanket impl に `#[doc(hidden)]` を付けて rustdoc の走査から除外しているが、これは問題を隠蔽するものであり恒久運用には適さない。

### §4 TDDD 哲学上の 🟡 Yellow の位置づけ

TDDD における 🟡 Yellow は「catalogue に宣言があるが実装がまだ Blue になっていない実装進行中」を表す一時的マーカーであり、設計上の恒久受容状態ではない。parser overreach に起因して構造的に Blue 到達が不可能な Yellow を「accepted deviation」として運用することは、設計バグを永続化することと等しい。本 ADR はこのような Yellow / Red の排除を目的とする。

## Decision

### D1: TypeRef パーサに `Type::QualifiedPath` encoding を追加する

`libs/infrastructure/src/tddd/type_ref_parser/parse_ctx.rs::convert_type_path` において、`type_path.qself.is_some()` 時に unresolved_type marker を返している現行実装（line 60-63）を、`rustdoc_types::Type::QualifiedPath { name, self_type, trait_, args }` を構築する処理に置き換える。

具体的な実装方針:

- `qself.ty`（`syn::QSelf` の self_type フィールド）を `convert_type` で再帰的に変換し、`Box<Type>` として `self_type` に格納する
- `type_path.path.segments` のうち `qself.position` より前のセグメント群を trait path として `resolve_trait_bound_path` で解決し、`trait_` に格納する
- `qself.position` 以降の最初のセグメントを `name`（associated item 名）として抽出する
- そのセグメントの generic arguments を `convert_generic_args` で変換し、`args` に格納する
- 出力を `rustdoc_types::Type::QualifiedPath { name, self_type, trait_, args }` とする

この encoding により、catalogue 側で `<Self as ChainIdentity>::Input<'_>` のような canonical qualified-path 宣言を書いた場合に `Type::QualifiedPath` として正しく内部表現され、format layer の既存実装 `format_qualified_path_with` がそれを同じ `<Self as ChainIdentity>::Input<'_>` 文字列にレンダーする。rustdoc 出力も同じ文字列形式を持つため、文字列等価比較で 🔵 Blue が立つ。

変更は `convert_type_path` 関数 1 箇所と関連テスト 1-2 件に留まる。format layer・比較ロジック・catalogue schema のいずれも変更しない。

### D2: TypeRef パーサに impl_generics 文脈を渡し `Type::Generic(name)` encoding を追加する

`TraitImplDeclV2.for_type` を parse する際に、その entry の `impl_generics: Vec<MethodGenericParam>` から導出した generic-param 名集合（`["T"]` 等）を parser に渡し、`convert_type_path` で single-segment path の識別子がその集合に含まれていれば `Type::Generic(name)` を返すように拡張する。

具体的な実装方針:

- `parse_type_ref_str`（または同等の entry point）に `generic_params: &[&str]`（または `&BTreeSet<&str>`）引数を追加し、既存の callsite を網羅的に更新する
- `convert_type_path`（`parse_ctx.rs:53`）のレジストリ chain（step 1: Primitive → step 6: Unresolved）のうち、step 3（`resolve_local`）の直後に新ステップ「local impl_generics に含まれる generic param 名か?」を挿入し、含まれていれば `Type::Generic(name)` を即返す
- method 本体内の他フィールド（例: `methods[].params[].ty`、`methods[].returns`）の解釈にも同一の `generic_params` 文脈（= trait の `traits[].generics` と method の `generics` を組み合わせたもの）を渡せるよう、infrastructure 側 codec の伝播経路を整える
- 結果: catalogue 側で `for_type: "T"` + `impl_generics: [{name: "T", ...}]` + `impl_where_predicates: [{lhs: "T", rhs: ["PersistedSoTChain"]}]` と宣言された blanket impl が、rustdoc が報告する `impl<T: PersistedSoTChain> SoTChain for T`（`for_: Type::Generic("T")`、`generics.params: [{name: "T", kind: Type, bounds: [PersistedSoTChain]}]`）と `structural_eq` で一致し 🔵 Blue が立つ

この変更により、現行の `#[doc(hidden)]` ワークアラウンドが不要になる。`#[doc(hidden)]` の除去は本 ADR の実装後に別タスクで行う。

## Rejected Alternatives

### A. 🟡 Yellow / 🔴 Red を accepted deviation として運用する

GAT projection や blanket impl に由来する Yellow / Red を「例外的に許容された偏差」として記録し、CI gate を素通りさせる仕組みを整備するという案。

却下理由: TDDD 哲学上 Yellow は実装途中の一時状態であり、構造的に Blue 到達が不可能な状態を「accepted」と呼んで固定化することは設計バグの永続化に等しい。また、accepted deviation 記録機構を新たに設けることは、設計劣化を隠蔽する抜け穴になりうる。本 ADR の動機そのものがこの選択肢の否定である。

### B. rustdoc 出力側の format を変更し `"Self"` 文字列で統一する（catalogue 側に合わせる）

`format_qualified_path_with` を変更して `<Self as Trait>::Assoc<args>` 形式ではなく `Self::Assoc<args>` 形式で出力し、catalogue の `ty: "Self::Input<'_>"` 表記と一致させる案。

却下理由: rustdoc 側は upstream `rustdoc_types::Crate` データを忠実に反映しており、`Type::QualifiedPath` という明示的な情報を単なる `"Self"` プレフィックスに潰すと GAT projection の trait 情報が失われる。複数の異なる trait に由来する GAT projection が同一の `"Self::Foo"` 表現に潰れ、カタログ評価の精度が下がる。情報量を落とす方向ではなく、catalogue 宣言を rustdoc 側の形式に合わせる方向（D1）を採る。

### C. catalogue の `ty` フィールドを文字列から構造化型に変更する（schema v6 化）

`params[].ty` を文字列ではなく `{ kind: "qualified_path", name: ..., self_type: ..., trait_: ... }` のような構造化オブジェクトにすることで、文字列レベルの形式依存を排除する案。

却下理由: schema v6 への大規模 migration が必要で、既存の全カタログファイルの再生成と全トラックへの retroactive 修正を伴う。本 ADR の対象範囲を大幅に超える。本 ADR は既存 schema を維持したままパーサのみを最小限に拡張する方針とし、構造化型案は将来の必要性が実証されたときに別 ADR で扱う。

### D. blanket impl 向けに catalogue schema v5 を v6 へ migrate する（TraitImplDeclV2 拡張）

`TraitImplDeclV2` に `generic_params` や `where_predicates` のフィールドが不足していると仮定し、schema migration を行う案。

却下理由: `TraitImplDeclV2` は既に `impl_generics: Vec<MethodGenericParam>` と `impl_where_predicates: Vec<WherePredicateDecl>` フィールドを持っており（schema v5）、不足しているのはパーサが `impl_generics` の情報を使わずに `for_type` を処理していることに起因するパーサの問題である。schema 拡張は不要であり、D2 の parser 拡張で解決できる。

### E. `#[doc(hidden)]` を blanket impl への恒久的ワークアラウンドとして維持する

blanket impl を rustdoc の走査から除外し続けることで TDDD 評価を回避する案。

却下理由: `#[doc(hidden)]` による除外は問題を隠蔽するものであり、catalogue に宣言した blanket impl が実装と整合しているかの検証を永久に無効化する。TDDD の目的（catalogue 宣言と実装の対応を継続的に確認する）と相反する。D2 によって parser を正しく拡張することで、この workaround は不要になる。

## Consequences

### Positive

- GAT projection を使うメソッド（source では `fn check(input: &Self::Input<'_>, strict: bool) -> VerifyOutcome`、catalogue では `<Self as ChainIdentity>::Input<'_>` などの canonical qualified-path 宣言）が catalogue 宣言と rustdoc 出力で文字列一致するようになり、🟡 Yellow が 🔵 Blue に転じる（D1）
- blanket impl（`impl<T: PersistedSoTChain> SoTChain for T` 形式）が catalogue 宣言と rustdoc 出力で一致するようになり、🔴 Red / 🟡 Yellow が 🔵 Blue に転じる（D2）
- 将来 GAT を使う trait を追加した場合も、catalogue 宣言が同じ仕組みで Blue 化できる（catalogue 側に特別対応を加える必要がない）
- 将来 blanket impl を追加した場合も、`impl_generics` + `impl_where_predicates` を宣言するだけで Blue 化できる
- format layer は既存の `format_qualified_path_with` をそのまま使用するため、D1 の変更影響範囲がパーサ 1 関数と関連テストに限定される
- blanket impl に付与していた `#[doc(hidden)]` ワークアラウンドが不要になり、後続の cleanup タスクで除去できる
- 「Yellow / Red accepted deviation」として設計バグを記録・維持するコストが不要になる

### Negative / トレードオフ

- `convert_type_path` に新しい分岐が追加される（D1: line 60-63 の 4 行が約 20-30 行の `QualifiedPath` builder に置き換わる）
- パーサの entry point に `generic_params` パラメータが増えるため、既存の callsite を網羅的に更新するコストが発生する（D2）
- パーサテストカバレッジを `QualifiedPath` ケースと `Type::Generic` ケースで拡張する保守コストが追加される
- `impl_generics` に登場しない名前を `impl_where_predicates.lhs` で使う場合（catalogue 不整合）はランタイムエラーにならず silent mismatch になりうる。これは別タスクで catalogue linter による静的検査として対応することが望ましい
- lifetime param や const param は現在 catalogue で表現できない（`impl_generics` は type param のみを扱う）。それらへの拡張需要が出た場合は別途対応が必要になる（Reassess When 参照）

## Reassess When

- `rustdoc_types` crate の major update により `Type::QualifiedPath` の構造（フィールド名・型）が変わったとき
- lifetime param や const param を blanket impl の catalogue に表現する需要が出たとき（現状は type param のみ対応、D2 の範囲外）
- catalogue schema 全体を構造化型に置き換える議論が浮上したとき（Rejected Alternatives C 参照）
- `impl_generics` / `impl_where_predicates` 整合を catalogue linter で静的検査する必要が生じたとき

## Related

- `knowledge/adr/2026-06-16-1030-signal-gate-strictness-config.md` — D7 で GAT 付き trait taxonomy を導入。本 ADR はその実装フットプリントの一部（パーサ側拡張）を扱う
- `libs/infrastructure/src/tddd/type_ref_parser/parse_ctx.rs`（line 60-63）— D1 修正対象コード
- `libs/infrastructure/src/tddd/signal_evaluator_v2/format/ty_base.rs`（`format_qualified_path_with`）— format 側の既存対応（修正不要）
- `libs/domain/src/tddd/baseline.rs::methods_structurally_equal` — 文字列等価比較ロジック（修正不要）
- `libs/domain/src/tddd/catalogue_v2/traits.rs`（line 99 / 105）— `TraitImplDeclV2.impl_generics` / `impl_where_predicates` フィールド（D2 活用対象、schema 変更不要）
- `libs/infrastructure/src/tddd/signal_evaluator_v2/structural_eq.rs`（line 1127）— `Type::Generic` 照合ロジック（D2 活用対象、修正不要）
