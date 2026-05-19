---
adr_id: 2026-05-13-1153-tddd-where-form-generics-normalization
decisions:
  - id: D1
    user_decision_ref: "chat_segment:tddd-v2-gap3-where-form:2026-05-13"
    status: accepted
  - id: D2
    user_decision_ref: "chat_segment:tddd-v2-gap3-where-form:2026-05-13"
    status: superseded
    superseded_by: "2026-05-18-1223-make-catalogue-schema-permissive.md#D2"
  - id: D3
    user_decision_ref: "chat_segment:tddd-v2-gap3-where-form:2026-05-13"
    status: superseded
    superseded_by: "2026-05-18-1223-make-catalogue-schema-permissive.md#D3"
  - id: D4
    user_decision_ref: "chat_segment:tddd-v2-gap3-where-form:2026-05-13"
    status: accepted
  - id: D5
    user_decision_ref: "chat_segment:tddd-v2-gap3-where-form:2026-05-13"
    status: accepted
  - id: D6
    user_decision_ref: "chat_segment:tddd-v2-gap3-where-form:2026-05-13"
    status: accepted
---
# TDDD where 形式 generics 正規化による構造等価性評価

## Context

### §1 Gap 3 の発見経緯

ADR `2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` の D14 にて `FunctionEntry.generics: Vec<MethodGenericParam>` フィールドが追加され、free function の generic type parameter を A-codec でエンコードできるようになった。その後 T034 で実装を進めたところ、A-codec 出力 (A 側) と rustdoc 出力 (C 側) の間で `Function.generics` の構造が一致しない事例が確認された。

問題の本質は表現形式の違いにある。rustdoc は `where` 句を含む generics を次の 2 通りで出力する。

- **インライン形式**: `<T: Bound>` — `GenericParamDef.bounds` にバウンドを直接格納する
- **where 形式**: `<T> where T: Bound` — バウンドを `Generics.where_predicates` に格納する

両者は Rust の意味論として完全に等価だが、rustdoc が選択する表現は文脈に依存する。一方、型カタログの A-codec (`encode_function` / `encode_method_items`) は従来から常に `GenericParamDef.bounds` にバウンドを格納していた。この不一致が Signal evaluator の Phase 2 構造比較で誤った不一致検出 (false 🔴) を引き起こしていた。

さらに、型カタログの既存スキーマ (`MethodGenericParam`) は `name: ParamName` という単一の型パラメータ名を LHS として持つ設計であり、`where Vec<T>: Clone` や `where T::Item: Send` のような型式 (type expression) を LHS に持つ where predicate を表現できなかった。これらは Rust の表現力上インライン形式に書き換えることができず、カタログスキーマの表現力の欠如が Gap 3 となっていた。

### §2 既存決定との連続性

- **ADR 2 D9** (provenance 非依存のトレイト実装比較): 「異なる構文形式で表現された同一の制約は、構造等価として扱う」という設計思想の先例。本 ADR の D1 はその generics 版と位置づけられる。
- **ADR 2 D13** (外部クレート修飾 TypeRef の triple-structure): 評価時に形式を合わせることで A/C の比較を成立させるという正規化の先例。
- **ADR 1 D14** (FunctionEntry.generics): D14 はインライン形式のバウンド (`MethodGenericParam.bounds`) を追加した。本 ADR の D2 はその直交する拡張として型式 LHS を持つ where predicate を追加する。

### §3 対象スコープ

rustdoc の `WherePredicate` には 3 種ある。

1. `BoundPredicate`: `where T: Bound` 形 (型パラメータまたは型式への trait/lifetime バウンド)
2. `LifetimePredicate`: `where 'a: 'b` 形 (lifetime の include 関係)
3. `EqPredicate`: `where T::Assoc = U` 形 (associated type の等式)

2 と 3 は実用頻度が低く、対応には別途スキーマ変更と評価ロジック追加が必要であることから、本 ADR では **1 の `BoundPredicate` のみをスコープとする**。

## Decision

### D1: 最大限 where 形式に正規化してから fingerprint を作る

A 側 (カタログ由来の `ExtendedCrate`) と C 側 (rustdoc 由来の `ExtendedCrate`) の `Function.generics` を構造的に比較するとき、両側を **最大限 where 形式** に正規化した後で fingerprint を生成する。where 形式を正規表現として選ぶ根拠は次の通り。

- `<T: Bound>` は `<T> where T: Bound` の構文糖衣であり意味は同一。
- `where Vec<T>: Clone` のように型式が LHS に来るケースはインライン形式では書けず、where 形式でしか表現できない。where 形式はすべての `BoundPredicate` を網羅できる唯一の形式である。

正規化は次の 3 箇所で実施する。

**A-codec 側** (`encode_function` / `encode_method_items` 内):

カタログエントリの `MethodGenericParam.bounds` (インライン形式バウンド) と `WherePredicateDecl` (where 形式バウンド、D2 で追加) を読み込み、すべてのバウンドを `Generics.where_predicates` に格納する。`GenericParamDef.bounds` は常に空とし、`is_synthetic: false` にする。これにより A-codec の出力は常に完全 where 形式になる。

**Signal evaluator 側** (`generics_eq.rs` 内の `build_generics_fingerprint` / `build_trait_method_map`):

C 側 rustdoc データを **直接変換せず**、正規化済みビューを構築してから fingerprint を生成する。具体的には、`Generics.params` の各 `GenericParamDef` に設定された `bounds` を `where_predicates` へ移動した仮想ビューを構築し (rustdoc データは不変のまま保持する)、そのビューの `where_predicates` を `(type_str, sorted_bound_strs)` のタプルでソートしたものを fingerprint の素材とする。

**カタログスキーマ側**:

D2 で追加する `WherePredicateDecl` 型と `where_predicates` フィールドがカタログ上の where 形式バウンドを保持する。

この 3 点を揃えることで、A 側・C 側ともに同じ fingerprint アルゴリズムが同じ出力を返し、false 🔴 の誤検出が解消される。

正規化は Signal evaluator の Phase 2 (構造比較) 内で行われる。Phase 1 (S/D 構築) の処理には影響しない。

### D2: WherePredicateDecl 型の導入と where_predicates フィールドの追加

`MethodDeclaration` と `FunctionEntry` に `where_predicates: Vec<WherePredicateDecl>` フィールドを追加する (serde default = 空 Vec)。`MethodDeclaration` はトレイト内のメソッド・関連関数を対象とするため、`TraitEntry` 内のメソッドもこの拡張の恩恵を受ける。`TraitEntry` 自体 (トレイト宣言レベルの where 句) は、トレイト level の generic parameter 宣言 (`TraitEntry.generics`) が別途スキーマ化されるまで D2 のスコープ外とする。`WherePredicateDecl` は以下の新規 domain 型として定義する。

```rust
<!-- illustrative, non-canonical -->
pub struct WherePredicateDecl {
    /// 左辺の型式 (例: "T"、"Vec<T>"、"T::Item")
    pub type_: TypeRef,
    /// 右辺のバウンドリスト (例: ["Clone"]、["Send", "Sync"])
    pub bounds: Vec<TypeRef>,
}
```

`type_` は `TypeRef` (generics を含む任意の型式文字列) として保持するため、`Vec<T>` や `T::Item` のような型式 LHS を表現できる。これは `MethodGenericParam.name: ParamName` が単一の Rust identifier しか受け入れないのとは異なる。

この拡張で表現できる where predicate の例を示す。

```rust
<!-- illustrative, non-canonical -->
// Rust 記法
fn process<T>(items: Vec<T>) where Vec<T>: Clone { ... }
fn get_item<I: Iterator>(iter: I) where I::Item: Send { ... }

// カタログ記法 (JSON)
"where_predicates": [
  { "type_": "Vec<T>", "bounds": ["Clone"] },
  { "type_": "I::Item", "bounds": ["Send"] }
]
```

serde default = 空 Vec を採用することで forward-compat 拡張となり、既存カタログは無修正でロードできる。schema version の bump は不要 (`schema_version: 3` のまま)。

### D3: スコープ限定 — BoundPredicate のみ、lifetime/eq predicate は対象外

本 ADR のスコープは rustdoc の `WherePredicate::BoundPredicate` のうち、バウンドが **トレイト参照のみ** で構成されるケースに限定する。

- `WherePredicate::LifetimePredicate` (`where 'a: 'b` 形) は対象外とし、A-codec も Signal evaluator もこれを無効 (フェイルクローズド) として扱う。
- `WherePredicate::EqPredicate` (`where T::Assoc = U` 形) も同様に対象外とする。
- `WherePredicate::BoundPredicate` の `generic_params` が非空の HRTB binder (`for<'a> T: Fn(&'a ()) -> ()` 形) も対象外とし、フェイルクローズドとして扱う。
- `WherePredicate::BoundPredicate` の `bounds` 内に `GenericBound::Outlives` (`where T: 'a` 形 — 型パラメータへの lifetime バウンド) が含まれる場合も対象外とし、フェイルクローズドとして扱う。`WherePredicateDecl.bounds` は `Vec<TypeRef>` (トレイト参照のみ) であり、lifetime バウンドを表現できない。この unsupported bound が C 側に出現した場合も unconditional mismatch とする。
- `GenericParamDef::Type.bounds` 内 (インライン param bounds) に `GenericBound::Outlives` (`<T: 'a>` 形 — inline lifetime バウンド) が含まれる場合も同様に対象外とし、フェイルクローズドとして扱う。where_predicates 経路だけでなく inline param bounds でも `Outlives` を検査する必要がある。
- `GenericBound::TraitBound.generic_params` が非空 (HRTB binder を TraitBound に持つケース — 例: `F: for<'a> Fn(&'a ())`) も対象外とし、フェイルクローズドとして扱う。`WherePredicate::BoundPredicate.generic_params` でのHRTB チェックに加え、個別 `TraitBound` に設定される HRTB も検査する必要がある。
- `GenericBound::TraitBound` 以外の任意の bound variant (例: `GenericBound::Use` — precise-capturing bound) も対象外とし、フェイルクローズドとして扱う。サポート対象は `GenericBound::TraitBound { generic_params: [], .. }` のみである。

これらを将来必要とするカタログ作成者は、対応する別 ADR の確定を待つ必要がある。Signal evaluator は `build_generics_fingerprint` を `Result<String, FingerprintError>` として、C 側 (rustdoc 由来) の Generics に本 ADR のサポート外の predicate/bound 形式が出現した場合にエラーを返す（あるいは比較不能な sentinel 値を返す）ことで、常に mismatch として扱う (unconditional fail-closed)。サポート対象は「`WherePredicate::BoundPredicate { generic_params: [], bounds: [GenericBound::TraitBound { generic_params: [], .. }, ...], .. }` のみ」であり、それ以外の predicate/bound 形式（LifetimePredicate、EqPredicate、HRTB binder を持つ BoundPredicate、`GenericBound::Outlives`、`GenericBound::TraitBound.generic_params` 非空、`GenericBound::TraitBound` 以外の任意の variant）はすべて fail-closed となる。inline param bounds (`GenericParamDef::Type.bounds`) についても同一の検査規則を適用する。サイレントスキップや比較可能なトークンの追加は fail-open になるため採用しない。A-codec 側はカタログスキーマがこれらの predicate 種別を持たないため出力しない。この扱いにより、サポート外の predicate を持つ関数は 🟡 / 🔴 となり、フェイルクローズドを維持する。

### D4: 移行ガイダンスとカタログ作成者向けルール

既存カタログエントリで `MethodGenericParam.bounds` にバウンドを記述しているもの (例: `<S: Into<String>>` 形式の APIT 由来エントリ) は引き続き有効である。A-codec がエンコード時に `bounds` を `where_predicates` へ移し、Signal evaluator が C 側も同様に正規化するため、両者の fingerprint は一致する。

新規に作成または修正するカタログエントリでは次のルールを推奨する。

- where 句を持つ Rust コードを declare する場合は `where_predicates` に書く。
- インライン形式 (`<T: Bound>`) として declare する場合は `MethodGenericParam.bounds` に書く。
- どちらの形式で書いてもエンコード後の fingerprint は同一になるため、カタログ作成者は Rust ソースの記述形式に合わせて選択すればよい。

`MethodGenericParam.bounds` と `where_predicates` の両方に同じバウンドを重複して書くことは誤りである (fingerprint に二重カウントされる)。片方にのみ書くこと。

### D5: spec への反映方針

本 ADR の D1 (正規化) と D2 (スキーマ拡張) に対応する spec 要素 (IN-30) は、実装タスク (T035) の着手前に計画段階で spec.json に追加する。これは「仕様として合意された要件」を先行して記録し、T035 がその要件を実現するという計画フローに従う。spec.json の IN-30 は T035 の実装対象の仕様であり、T035 完了後に AC-13 が達成される。本 ADR は「何を決定したか」を記録し、対応する spec 要素 (IN-30) はこの計画バンドルで合わせて追加する。

### D6: signal traceability

D1〜D5 のすべての決定は、2026-05-13 のユーザー確認 (direction (a) 承認) に基づく。frontmatter の各 decision に `user_decision_ref: "chat_segment:tddd-v2-gap3-where-form:2026-05-13"` を付与することで 🔵 シグナルとして評価される。

## Rejected Alternatives

### A: C 側データを変換して where 形式に書き換える

rustdoc 出力 (`GenericParamDef.bounds`) を直接 `where_predicates` へ移す変換を C 側に対して永続的に行い、変換済みの `rustdoc_types::Crate` を Signal evaluator に渡す案。

却下理由: rustdoc データは B (baseline) として永続保存される不変のソースオブトゥルースであり (ADR 2 D2 / ADR 3 D4)、途中で書き換えるとデバッグ性が失われる。また B/C の「純粋な rustdoc_types::Crate」という不変条件 (ADR 2 D2) に反する。A-codec を常に where 形式で出力し、C 側は fingerprint 生成時にのみ仮想正規化ビューを構築する D1 の方式が、データの不変性と比較の正確性を両立する。

### B: C 側のインライン形式バウンドを where 形式バウンドと別々に fingerprint する

`GenericParamDef.bounds` と `where_predicates` を別々のフィールドとして fingerprint に組み込み、A-codec も同様に分離して出力する案。

却下理由: rustdoc が使う形式がコンパイル文脈や Rust バージョンによって変わりうる (現状でも両形式が混在して出現する)。分離 fingerprint は「同じ制約を異なる形式で書いた」ときに誤った不一致を報告し続ける。正規化によって形式の差を吸収することが D1 の目的であり、分離は D1 の目的と矛盾する。

### C: where 形式の型式 LHS を識別子に限定する (WherePredicateDecl.type\_ を ParamName にする)

`WherePredicateDecl.type_` を `TypeRef` ではなく `ParamName` として設計し、識別子のみを LHS に受け入れる案。

却下理由: `where Vec<T>: Clone` や `where T::Item: Send` のような型式 LHS を表現できなくなり、D2 の目的を達成できない。`TypeRef` を使うことで任意の Rust 型式を LHS に置けるようになる。識別子のみを LHS に受け入れる制約はすでに `MethodGenericParam.name: ParamName` で満たされており、`WherePredicateDecl` はその制約を外した形式として明確に位置づけられる。

## Consequences

### 良い影響

- where 句を持つ generic function / method のカタログ declare が `action: modify` で正確に Match_Modify と評価されるようになり、false 🔴 の誤検出が解消される。
- `where Vec<T>: Clone` / `where T::Item: Send` のような型式 LHS を持つ where predicate をカタログで表現できるようになる。
- A-codec が常に where 形式で出力するため、Signal evaluator の正規化ロジックが C 側のみを対象にすれば済み、実装が非対称にならない (A 側はすでに where 形式固定)。
- 既存カタログ (`MethodGenericParam.bounds` 形式) は無修正でロードできる (serde forward-compat)。A-codec がエンコード時に自動的に where 形式に変換するため、カタログ更新なしに評価結果が改善する。

### 悪い影響

- `WherePredicateDecl` 型と `where_predicates` フィールドの domain / infrastructure 両側への追加作業が発生する。
- Signal evaluator の `build_generics_fingerprint` と `build_trait_method_map` に正規化ビュー構築ロジックが追加され、コードの複雑度が若干上がる。
- `LifetimePredicate` / `EqPredicate` は D3 のスコープ外として除外されるため、それらを持つ generic function は現時点では正確な評価を受けられない。

## Reassess When

- Rust の構文拡張で `GenericParamDef.bounds` と `where_predicates` の使い分けに追加のルールが生じた場合: 正規化ロジックの再評価が必要になる。
- rustdoc-types crate のメジャーバージョンアップで `WherePredicate` の variant 構造が変わった場合: D1 の正規化ビュー構築ロジックを更新する必要がある。
- `WherePredicate::LifetimePredicate` または `WherePredicate::EqPredicate` に対する評価が必要になった場合: D3 のスコープ限定を解除する専用 ADR を作成する。
- `WherePredicateDecl.type_` の `TypeRef` 文字列では表現できない型式 LHS が現れた場合 (例: 高階多相 bound など): スキーマ拡張を再評価する。

## Related

- `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` — ADR 1 (Catalogue layer schema)。D13 (`has_default_impl`) は trait method の has_body 非対称を解決した。D14 (`FunctionEntry.generics`) はインライン形式のバウンドを `MethodGenericParam.bounds` として追加した先行拡張であり、本 ADR の D2 はその直交する拡張として型式 LHS を持つ where predicate (`where_predicates`) を追加する。
- `knowledge/adr/2026-05-08-0258-tddd-typegraph-hybrid-and-codec.md` — ADR 2 (TypeGraph hybrid + codec)。D9 (provenance 非依存のトレイト実装比較) は「構文形式の違いを吸収して構造等価として扱う」という設計思想の先例であり、本 ADR の D1 はその generics 版と位置づけられる。D13 (外部クレート修飾 TypeRef の triple-structure) は「評価時に A/C の形式を合わせる」という正規化の先例。
- `knowledge/adr/2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` — ADR 3 (Signal evaluator)。D1 の正規化は Phase 2 (D3 の 11 領域 × signal table) の「構造一致」比較の内部で行われる。Phase 1 (S/D 構築) の動作は変わらない。
- `knowledge/adr/README.md` — ADR 索引
