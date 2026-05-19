---
adr_id: 2026-05-18-1223-make-catalogue-schema-permissive
decisions:
  - id: D1
    user_decision_ref: "chat_segment:make-catalogue-schema-permissive:2026-05-18"
    candidate_selection: "from:[sum-type,vec-typeref-permissive-single-struct-operator-enum,both-strip,one-side-strip,two-variant-enum,hrtb-desugar,eqpredicate-yellow-accept,scope-limited-d3,generic-params-field,rhs-typeref-with-decomposition] chose:vec-typeref-permissive-single-struct-operator-enum"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:make-catalogue-schema-permissive:2026-05-18"
    candidate_selection: "from:[impl-block-plus-trait-decl-schema-extension,evaluator-aggregation,catalogue-re-attribution] chose:impl-block-plus-trait-decl-schema-extension"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:make-catalogue-schema-permissive:2026-05-18"
    review_finding_ref: "contract-map-v3-2026-05-15:workaround-4-entries"
    status: proposed
---
# catalogue スキーマを Rust 文法に対して寛容な設計にする

## Context

catalogue v3 スキーマは Rust のジェネリック / where 句を完全には反映できておらず、以下の表現を宣言できない。

- **ライフタイム境界** (`'static`、`'a` 等): A-codec の `validate_supported_bound` がライフタイム境界を fail-closed で拒否するため、`<F: Fn(...) + Send + Sync + 'static>` のような Rust ソースを宣言できない (旧 ADR `2026-05-13-1153-tddd-where-form-generics-normalization.md` D3)。

- **HRTB on where 句トレイト境界制約**: `where for<'a> T: Fn(&'a ())` のような形を fail-closed で拒否する。トレイト境界レベル HRTB への脱糖 (`where T: for<'a> Fn(&'a ())`) は単純なケースでのみ成立し、GAT + HRTB (`where for<'a> T::Item<'a>: Bound`) のように左辺でバインダーを使うケースや、バインダー内に Outlives 関係を持つケース (`where for<'a, 'b: 'a> T: Trait<'a, 'b>`) では脱糖できない。

- **HRTB on トレイト境界**: `F: for<'a> Fn(&'a ())` のような形も fail-closed で拒否する。

- **where 句のライフタイム境界制約**: `where 'a: 'b` のような制約を宣言できない (旧 ADR D3 でスコープ外)。

- **where 句の等価制約**: `where T::Assoc = U` のような等価制約を宣言できない (旧 ADR D3 でスコープ外)。

- **精密捕捉**: `use<'a, T>` 形式の精密捕捉を fail-closed で拒否する。

- **トレイト境界以外の任意の境界の種類** (開放型): 将来 rustdoc に追加される境界も含め、現状は fail-closed で拒否する。

- **impl ブロックレベルのジェネリックパラメータと where 句**: 型エントリ (`TypeEntry`) およびトレイト impl 宣言エントリ (`TraitImplDeclV2`) に impl ブロックの `<L, R, W>` を宣言するフィールドがなく、メソッドレベルに「間借り」するか宣言を諦めるしかない。

- **トレイト宣言レベルのジェネリック**: トレイトエントリ (`TraitEntry`) にトレイト宣言レベルのジェネリック宣言がなく、`trait Foo<T> where T: ...` のようなトレイト宣言を catalogue で正規に表現できない。旧 ADR `2026-05-13-1153-tddd-where-form-generics-normalization.md` D2 で先送りされていた。

これらの制限により、catalogue 設計者が Rust コードを正しく宣言できない場合がある。S/C の構造比較に非対称性が生まれ、本来 Blue であるべき宣言が Yellow / Red と判定される false-positive (偽陽性) の根本原因となっている。

なお、HRTB / where 句ライフタイム境界制約 / where 句等価制約 / 精密捕捉 / トレイトエントリのジェネリックについては現時点で実証済みの false-positive が特定されているわけではないが、旧 ADR D3 のスコープ限定が同じ false-positive を生む構造的欠陥であるため、予防的に一括解消する。

## Decision

### D1: catalogue を Rust 文法に対して寛容な設計とし、旧 ADR D3 全項目を置き換える

catalogue を **Rust 文法に対して寛容な** 設計とする。境界 (トレイト境界 / ライフタイム境界 / HRTB / 精密捕捉等) はメソッドのジェネリックパラメータ宣言 (`MethodGenericParam`) の `bounds` フィールド (`Vec<TypeRef>` 型、`TypeRef` は catalogue 側の type alias = `String`) に文字列として保持する。where 句の制約構造は、where 句宣言 (`WherePredicateDecl`) を以下の単一 struct + 演算子 enum (3 フィールド構造) で表現する。

<!-- illustrative, non-canonical -->
```rust
pub struct WherePredicateDecl {
    pub lhs: TypeRef,        // 任意の Rust where 句の左辺 (HRTB バインダー含む)
    pub rhs: Vec<TypeRef>,   // 任意の Rust where 句の右辺。`+` 連結された複数の境界は 1 Vec にまとめる
    pub operator: BoundOp,
}
pub enum BoundOp {
    Bound,  // `:` (T: Bound1 + Bound2)
    Equal,  // `=` (T::Assoc = U) — rhs Vec は長さ 1
}
```

Rust where 句の本質的な構造は「`lhs Operator rhs` の繰り返しリスト」であり、この構造を直接 catalogue に対応させる。HRTB バインダー (`for<'a>`) は `lhs` 文字列の先頭に組み込む (例: `lhs: "for<'a> T::Item<'a>"`、`lhs: "for<'a, 'b: 'a> T"`)。`+` 連結された複数の境界 (`T: A + B + C`) は `rhs` Vec に `["A", "B", "C"]` としてまとめ、1 エントリ = 1 where 句とする。

旧 ADR `2026-05-13-1153-tddd-where-form-generics-normalization.md` D3 で fail-closed 拒否されていた 7 項目を本 D1 で一括して置き換え、catalogue 側で正規に宣言できるようにする (ライフタイム境界 / HRTB on where 句 / HRTB on トレイト境界 / where 句ライフタイム境界制約 / where 句等価制約 / 精密捕捉 / トレイト境界以外の任意境界種類)。これにより信号評価上、構造的に Blue 化できない項目が生まれなくなる (TDDD 適合)。

具体的な修正:

1. **スキーマ最小拡張**: `WherePredicateDecl` を上記 3 フィールド構造に拡張する。`MethodGenericParam.bounds: Vec<TypeRef>` は維持する。
2. **A-codec `validate_supported_bound` 撤廃**: 境界の種類 (ライフタイム境界 / HRTB / 精密捕捉等) による reject を撤廃し、種類を問わずエンコードする。`syn` でパース可能な境界文字列はすべて受け入れる。
3. **信号評価器 `strip_outlives_from_index` 撤廃 + 両側保持**: 片側ストリップを撤廃し、両側でライフタイム境界を保持する。`build_generics_fingerprint` を全境界種類 (ライフタイム境界 / HRTB / 精密捕捉等) および `BoundOp` に対応させ、`{ lhs, rhs, operator }` 単位でフィンガープリントを生成する。`rhs` の要素は集合として正規化 (順序非依存ソート) し、`T: A + B` と `T: B + A` を同一フィンガープリントにする。
4. **パース失敗のエラー処理**: A 側 / C 側のいずれかで `syn` のパースが失敗した境界文字列は、信号評価の対象外として error を返す。catalogue 側のパース失敗は catalogue 設計者の宣言ミスを示し、rustdoc 側のパース失敗は信号評価以前のシステム異常を示すため、いずれも信号 (Blue / Yellow / Red) ではなく明示的なエラーとして catalogue 設計者に伝える。

### D2: impl ブロックレベル + トレイト宣言レベルのジェネリックを catalogue スキーマに追加する

Rust では 1 つの struct に対して複数の impl ブロックを書け、各 impl ブロックが独自のジェネリック宣言と where 句を持つ (例: `impl<L> Foo<L> { ... }` と `impl<L: Display> Foo<L> { ... }` を併記)。catalogue でもこの 1 struct - N impl block の関係を表現するために、以下のスキーマ拡張を行う:

- **inherent impl ブロック用の新エントリ (`InherentImplDeclV2`)** を導入する。impl ブロックレベルのジェネリック宣言と where 句、対象 struct 名を保持する。1 struct に対して複数の inherent impl がある場合は複数エントリで表現する。`TypeEntry` には impl ブロック情報を持たせない。
- **トレイト impl 宣言エントリ (`TraitImplDeclV2`)** に、impl ブロックレベルのジェネリック宣言と where 句を追加する。`TraitImplDeclV2` は既に各 impl ブロックを独立エントリとして扱っているため、各 impl ブロック固有のジェネリックを直接保持する。
- **トレイトエントリ (`TraitEntry`)** に、トレイト宣言レベルのジェネリック宣言部分を追加する。トレイト宣言は trait ごとに 1 つなので、`TraitEntry` 内に直接保持する。

これは D1 と同じ方針 (catalogue を Rust 表現力に対して中立にする) の延長であり、旧 ADR `2026-05-13-1153-tddd-where-form-generics-normalization.md` D2 で先送りされていたトレイトエントリの generics スキーマ化を本 D2 で完了する。

**スコープ限定 — 型パラメータのみ**: 本 D2 で使用する `MethodGenericParam` は現在の実装で**型パラメータ**のみをサポートする (`name: ParamName` + `bounds: Vec<TypeRef>`)。ライフタイムパラメータ (`'a` 等) および const パラメータ (`const N: usize` 等) は本 D2 の初期スコープ外とし、将来の拡張で対応する。実際に catalogue 設計が必要な impl / trait ジェネリックの大部分は型パラメータで構成されており (`impl<L, R, W> Foo<L, R, W>` 等)、ライフタイム / const を含む宣言は別途 Phase 3 の impl-plan で正確なフィールド型を確定する。

具体的な修正方針:

1. inherent impl ブロック用の新エントリ (`InherentImplDeclV2`) を導入する。フィールドは対象 struct 名 (`type_name`)、impl ブロックレベルのジェネリック宣言 (`impl_generics` — `MethodGenericParam` の配列、型パラメータのみ)、where 句 (`impl_where_predicates` — `WherePredicateDecl` の配列)、impl ブロック内のメソッド一覧などを保持する。catalogue 設計者は 1 struct に対して複数の inherent impl ブロックを、それぞれ独立したエントリとして宣言できる (例: `impl<L> Foo<L> { fn a() }` と `impl<L: Display> Foo<L> { fn b() }` を 2 エントリで宣言)。`TypeEntry` には impl 情報を持たせない。
2. `TraitImplDeclV2` に `impl_generics` フィールド (`MethodGenericParam` の配列、型パラメータのみ) と `impl_where_predicates` フィールド (`WherePredicateDecl` の配列) を追加する。各 trait impl ブロックが独自のジェネリックと where 句を持てるようになる。
3. `TraitEntry` にトレイト宣言レベルのジェネリック宣言部分を保持するフィールドを追加する (例: `generics` フィールド — `MethodGenericParam` の配列 (型パラメータのみ)、`where_predicates` フィールド — `WherePredicateDecl` の配列)。catalogue 設計者は `trait Foo<T> where T: ...` のようなトレイト宣言を正規の場所に宣言できるようになる。
4. A-codec はエンコード時に rustdoc の impl ブロック / トレイト宣言レベルのジェネリックを正しいフィールドに配置する。catalogue 設計者が各レベルに宣言したジェネリックは rustdoc の対応フィールドと同じ位置に格納され、メソッドレベルとの混在が起きない。
5. 信号評価器は rustdoc 側の impl ブロック / トレイト宣言レベルのジェネリックと catalogue 側の各フィールドを対称な形で比較する。

<!-- illustrative, non-canonical -->
```rust
// inherent impl ブロック用の新エントリ (1 struct に対して複数 inherent impl がある場合は複数エントリで表現)
// NOTE: MethodGenericParam は型パラメータのみサポート。ライフタイム/const パラメータは将来の拡張。
pub struct InherentImplDeclV2 {
    pub type_name: String,                              // 対象 struct 名 (例: "Foo")
    pub impl_generics: Vec<MethodGenericParam>,         // impl<L, R, W> の型パラメータ <L, R, W> (ライフタイム/const は将来対応)
    pub impl_where_predicates: Vec<WherePredicateDecl>, // impl ブロックの where 句
    // ... impl ブロック内のメソッド一覧など ...
}

// 既存の TraitImplDeclV2 / TraitEntry に以下のフィールドを追加する (1 impl block - 1 エントリ、各ブロックが独自のジェネリックを持つ)
// NOTE: MethodGenericParam は型パラメータのみサポート。ライフタイム/const パラメータは将来の拡張。
pub struct TraitImplDeclV2 {
    // ... 既存フィールド ...
    pub impl_generics: Vec<MethodGenericParam>,         // impl<L, R, W> Trait for Foo<L, R, W> の型パラメータ <L, R, W> (ライフタイム/const は将来対応)
    pub impl_where_predicates: Vec<WherePredicateDecl>, // impl ブロックの where 句
}

pub struct TraitEntry {
    // ... 既存フィールド ...
    pub generics: Vec<MethodGenericParam>,         // trait Foo<T> の型パラメータ <T> (ライフタイム/const は将来対応)
    pub where_predicates: Vec<WherePredicateDecl>, // trait 宣言の where 句
}

// TypeEntry には impl ブロック情報を持たせない (1 struct - N impl block を独立エントリで表現する方針)
pub struct TypeEntry {
    // ... 既存フィールド (struct 自体の宣言: 名前、フィールド等) ...
}
```

<!-- illustrative, non-canonical -->
```json
// `impl<L: Left, R: Right, W: Write> Foo<L, R, W> where L: Send, W: Sync` の InherentImplDeclV2 宣言例 (1 inherent impl block = 1 エントリ)
{
  "type_name": "Foo",
  "impl_generics": [
    { "name": "L", "bounds": ["Left"] },
    { "name": "R", "bounds": ["Right"] },
    { "name": "W", "bounds": ["Write"] }
  ],
  "impl_where_predicates": [
    { "lhs": "L", "rhs": ["Send"], "operator": "Bound" },
    { "lhs": "W", "rhs": ["Sync"], "operator": "Bound" }
  ]
}

// 同じ struct (`Foo`) に対する別の inherent impl block (例: `impl<L: Display> Foo<L> { ... }`) は別エントリとして宣言
{
  "type_name": "Foo",
  "impl_generics": [
    { "name": "L", "bounds": ["Display"] }
  ],
  "impl_where_predicates": []
}

// `trait Converter<T: Clone>` の TraitEntry 宣言例 (T がトレイトの型パラメータ)
{
  "name": "Converter",
  "generics": [
    { "name": "T", "bounds": ["Clone"] }
  ],
  "where_predicates": []
}
```

### D3: `patch_impl_for_ids` を廃止し、A 側にも Pre-step で全 Item の id を一括 pre-allocate する

信号評価器は比較前処理として、catalogue (A) と baseline (B、rustdoc 出力) を統合スコープ (S = ExtendedCrate) に取り込む際、両者の独立した id 空間の衝突を避けるため、各 Item の id を flat incremental に振り直す (旧 ADR `2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` D2)。impl block がぶら下がる親型エントリ (struct または enum) の id が変わると、配下の impl エントリが持つ `for_` フィールド (impl の対象型を示す id) も新しい親型 id に合わせて更新しなければならない。

現状、B 側は Pre-step で全 B Item に fresh S id を一括 pre-allocate (`b_id_remap` の構築) するシンプルな設計に統一されている (T037)。一方 A 側は action 別処理 (Add / Modify / Reference / Delete) の中でローカルに id 再発番が分散しており、`id_map` に親型の変換エントリ `(旧親型 id → 新親型 id)` が入っていない call site が複数存在する。`patch_impl_for_ids` はこれらの call site で `for_.id` を新しい親型 id (`new_parent_id`) で強制上書きすることで補完する役割を果たしてきた。

しかしこの強制上書きは、外部クレート Item を指す `for_` (例: `impl From<MyError> for OtherCrateError` のような別クレートが所有する型を対象とする impl の `for_`) に対しても無条件に自クレート親型の id を書き込んでしまう。その結果、信号評価器は当該 impl を「自クレート型を対象とする impl」と誤認識する。`for_is_external` フラグ (外部クレート対象であることを示す) が誤判定され、false-positive の信号が出る。

本 D3 では A 側にも B 側と同様の Pre-step を導入し、全 A Item に fresh S id を一括 pre-allocate する (`a_id_remap` の構築)。Step 4 & 5 の action 別処理 (Add / Modify / Reference / Delete) は `a_id_remap` を参照して id を解決する (action の意味論に従い、各 action がどう id を扱うかは旧 ADR `2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` の決定通り — 本 D3 では変更しない)。これにより `id_map` に親型 mapping が必ず含まれる状態になり、`rewrite_type_ref_ids_in_item` のみで `for_` の remap が完結する。補完用の `patch_impl_for_ids` 関数 (および類似の `patch_impl_trait_ids`) は不要になるため削除する。

すべての `Type::ResolvedPath.id` は同じ仕組みで remap される。区別は **id_map での remap** か **external_crates rebuild での rebind** かのみで、impl block の `for_` を特別扱いする必要はなくなる:

- 自クレート Item を指す id (`crate_id == 0`): id_map (`a_id_remap` または `b_id_remap`) で新 S id に remap される。
- 外部クレート Item を指す id (`crate_id != 0`、例えば `impl From<X> for Foreign` の Foreign): external_crates rebuild で新 crate_id に rebind される (旧 ADR `2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` D2)。

impl の `for_.id` も他の `Type::ResolvedPath.id` (field の type、generic の bound 等) と同じ処理経路に統一される。`patch_impl_for_ids` の強制上書きが介入しなくなることで、外部クレート Item を指す `for_.id` が誤って自クレート親型の id に潰されなくなり、`for_is_external` フラグが正しく判定される。

これにより信号評価器の id 管理が「Pre-step で全 Item に fresh id を一括 pre-allocate → action 別処理は id_map を参照」というシンプルな構造に統一され、外部クレート Item を指す `for_` の誤判定 (信号 false-positive) が根本から解消される。

## Rejected Alternatives

### A. 境界を sum type 化する案 (D1 の代替)

`MethodGenericParam.bounds` および `WherePredicateDecl.rhs` を `Vec<TypeRef>` から `Vec<Bound>` に変更し、`Bound = TraitBound | LifetimeBound | HRTBBound { for_lifetimes, inner } | PreciseCapture { lifetimes }` のような sum type で境界の種類を catalogue スキーマレベルで区別する案。

却下理由: 全境界が `syn` でパース可能な文字列として表現できる。catalogue スキーマレベルで variant 化する必要はなく、A-codec がエンコード時に各境界の種類に振り分ければ十分。sum type 化すると catalogue 設計者が境界の種類を明示的に選択する必要が生じ、`<F: Fn(...) + Send + Sync + 'static>` のような自然な Rust ソースをそのまま反映した宣言を阻害する。

### B. 両側ストリップ (D1 の代替)

スキーマは変えず、S 側 (`a_index`) にも `strip_outlives_from_index` を適用して S/C 両側でライフタイム境界を除去する案。

却下理由: ライフタイム境界という Rust 表現力の一部を信号評価器が恒久的に捨てる方向であり、catalogue 設計者は `'static` 等を宣言できないままになる。catalogue が Rust 表現力を制限する状況を肯定することになり、本 ADR の方針 (catalogue を Rust 表現力に対して中立にする) と反する。

### C. 片側ストリップ継続 (現状維持)

`b_index` への `strip_outlives_from_index` 適用のみを維持し、S 側 (`a_index`) には適用しない案。

却下理由: S 側にライフタイム境界が含まれる経路 (baseline-seeded children 等) で false-positive が継続する。修正ではなく問題の温存であり却下。

### D. 信号評価器集約 (D2 の代替)

スキーマは変えず、信号評価器側でメソッドの構造比較を行う際に外側の impl ブロックのジェネリックをメソッドのジェネリックに集約する案。catalogue 側は引き続きメソッドレベルに impl ブロックジェネリックを「間借り」して宣言する。

却下理由: catalogue スキーマの制限を信号評価器の複雑化で隠す対処にすぎない。catalogue 設計者は正規の場所にジェネリックを宣言できず、誤りやすい「間借り」記法を続けることになる。

### E. catalogue side re-attribution (D2 の代替)

スキーマは変えず、A-codec のエンコード時にメソッドレベルに宣言されたジェネリックのうち実質 impl ブロックレベルであるものを rustdoc の impl ブロック側に自動移動する案。

却下理由: catalogue スキーマの制限をコーデック側の複雑化で隠す方向であり、自動判定の誤判定リスクも伴う。catalogue 設計者が意図した宣言をコーデックが無言で書き換えることは宣言の意図を損なう。

### F. where 句宣言を 2 種別の enum とする案 (D1 の代替)

`WherePredicateDecl` を `BoundPredicate { type_, bounds, generic_params }` と `EqPredicate { lhs, rhs }` の 2 バリアント enum として定義する案。

却下理由: バリアントごとにフィールド構造が分かれることでコーデック / 信号評価器の種別分岐が増える。`rhs: Vec<TypeRef>` の単一 struct + 演算子 enum なら 1 型で均一に表現でき、振り分けは `operator` フィールドの switch のみに集約できる。

### G. HRTB on where 句トレイト境界制約をトレイト境界レベル HRTB に脱糖する案

`WherePredicateDecl` に `generic_params` フィールドを設けず、HRTB on where 句トレイト境界制約をトレイト境界レベル HRTB に脱糖することで `{ lhs: "T", rhs: ["for<'a> Fn(&'a ())"], operator: Bound }` 形式のみで表現する案。

却下理由: GAT + HRTB (`where for<'a> T::Item<'a>: Bound`) のように左辺でバインダーを使うケースは脱糖できない (脱糖すると `T::Item<'a>` の `'a` が自由変数となり構文エラーになる)。バインダー内 Outlives 関係 (`where for<'a, 'b: 'a> T: Trait<'a, 'b>`) も脱糖では保持できない。これらのパターンを catalogue 設計者が宣言しようとすると正規宣言が不可能となり、構造的に解消できない Yellow が生じる。

### H. where 句等価制約を Yellow 受容 (スキーマ変更ゼロで等価制約を宣言不可のままにする案)

`WherePredicateDecl` を維持し、等価制約 (`where T::Assoc = U`) は catalogue で宣言できない状態を温存する案。等価制約を含むエントリが現れた場合はフィンガープリント不一致 (Yellow) を受け入れる。

却下理由: TDDD の Yellow は「設計者が宣言を精緻化すれば Blue 化できる」状態を表す。設計者がいかに修正しても Blue にできない Yellow は構造的に解消できない Yellow であり、strict signal gate のもとではマージ不能な詰み状態を生む。等価制約エントリが 1 件でも現れた時点で当該 spec エントリがマージ不能となるため、TDDD の運用原則に反する。

### I. 旧 ADR D3 / D2 残項目を別 ADR で個別対応 (本 ADR スコープ限定)

本 ADR D1 を `validate_supported_bound` のライフタイム境界拒否のみの supersede に限定し、HRTB / where 句ライフタイム / where 句等価制約 / 精密捕捉 / トレイトエントリ generic は将来別 ADR で対応する案。旧 ADR D3 / D2 の「スコープ限定」路線を継続する形になる。

却下理由: これらの制限は同一の根本原因 (catalogue が Rust 表現力を制限している) から生じており、対応を分割する技術的根拠がない。先送りは将来同型の false-positive を生み続ける構造的欠陥を温存する。一括 supersede の方が catalogue 設計者に対する制約の全体像を明確にし、将来の追加 false-positive を予防できる。

### J. `WherePredicateDecl` に `generic_params` フィールドを別途設ける案 (D1 の代替)

HRTB バインダーを `lhs` 文字列に組み込まず、`generic_params: Vec<TypeRef>` フィールドを独立させて binder の lifetime params を構造化宣言する案。例: `{ lhs: "T::Item<'a>", rhs: ["Bound"], generic_params: ["'a"], operator: Bound }`。

却下理由: `lhs` / `rhs` は既に任意の Rust 文字列として寛容な設計のため、バインダーを `lhs` 先頭に組み込む (`lhs: "for<'a> T::Item<'a>"`) ことで全パターンを表現できる。フィールド分離は catalogue 設計者に「バインダーと本体を別々に書く」負担を生むだけで、Rust ソースの構文をそのまま反映するという方針と整合しない。A-codec は `syn` で `lhs` から自動分離できるため、フィールド分離はコーデック実装を複雑化するだけで表現力の向上はない。

### K. `WherePredicateDecl.rhs: TypeRef` (単一) + 分解慣習 (D1 の代替)

`rhs` を単一 `TypeRef` のままとし、`T: A + B + C` のような複数境界は catalogue 設計者の側で `{lhs:T, rhs:A, op:Bound}`、`{lhs:T, rhs:B, op:Bound}`、`{lhs:T, rhs:C, op:Bound}` の 3 エントリに分解宣言する慣習を確立する案。

却下理由: rustdoc の where 句構造は元々境界を Vec で保持しており、catalogue 側だけエントリを分解させると Rust ソースと catalogue 表現の間に不要な変換が挟まる。catalogue 設計者は頭で分解してから宣言する必要があり、書きにくく読みにくい。`rhs: Vec<TypeRef>` で均一に持てば 1 エントリ = 1 where 句で rustdoc 構造に 1 対 1 対応でき、設計者は Rust ソースをそのまま反映できる。等価制約の `rhs` が Vec 長さ 1 になる軽微な非対称性は許容範囲。

### L. `patch_impl_for_ids` を残したまま id 比較で cross-crate target をスキップする案 (D3 の代替)

`patch_impl_for_ids` 関数を残し、`for_.id` が親 struct の id と異なる場合 (= 既にクレート横断ターゲットを指している場合) はパッチをスキップする条件分岐を追加する案。

却下理由: `for_` の正規 remap は既に `rewrite_type_ref_ids_in_item` + `id_map` で行われており、`patch_impl_for_ids` は本来「id_map に root mapping が欠けている call site の補完」のために導入された処理である。条件分岐で症状を回避するのは対症療法であり、`for_` の処理を 2 種類の経路 (`id_map` remap + 強制上書き条件付き) に分岐させる複雑さを残す。本 D3 では A 側にも B 側と同様の Pre-step で全 Item に fresh id を一括 pre-allocate することで補完を不要にし、`for_` の処理経路を `id_map` remap 1 種類に統一する。

## Consequences

### Positive

- 旧 ADR `2026-05-13-1153-tddd-where-form-generics-normalization.md` D3 の全 7 項目 (ライフタイム境界 / HRTB on where 句トレイト境界制約 / HRTB on トレイト境界 / where 句ライフタイム境界制約 / where 句等価制約 / 精密捕捉 / トレイト境界以外の任意境界種類) と D2 の deferred 項目 (トレイトエントリ generic) が一括解消され、catalogue が Rust 表現力に対して中立になる。
- false-positive 3 パターン (ライフタイム境界の非対称性 / impl ブロックジェネリック欠落 / `patch_impl_for_ids` のクレート横断ターゲット誤動作) が根本から解消する。
- 構造的に Blue 化できない項目 (構造的に解消できない Yellow) が生まれず、TDDD strict signal gate を迂回する必要がない。
- `WherePredicateDecl` が `lhs` / `rhs` / `operator` の 3 フィールドのみで覚えるべき要素が最小限、Rust where 句の本質的な構造 (`lhs Operator rhs` の繰り返しリスト) を直接対応させた素直な設計。
- rustdoc 側の where 句構造 (左辺 1 つ + 境界複数を Vec で保持) を 1 対 1 で対応させられるため、A-codec / 信号評価器の振り分けロジックが自然 (1 エントリ → 1 rustdoc where 句)。
- 将来 rustdoc に新しい境界の種類が追加されても文字列 + A-codec の振り分けロジック拡張で吸収できる (追加のスキーマ変更不要)。
- 境界の種類による reject (`validate_supported_bound` の旧挙動) は撤廃され、catalogue 設計者は任意の種類の境界を文字列として宣言できるようになる。一方、`syn` でパース不能な文字列を書いた場合は A-codec / 信号評価器が明示的な error を返すため、catalogue 設計者は宣言ミスを信号評価以前のエラーで早期に検出できる。

### Negative

- `lhs` / `rhs` が任意の文字列なので、不正なバインダー構造 (`for<'a` のような未閉じ) は A-codec の `syn` パース時まで検出されない (型レベル早期検出の犠牲)。
- A-codec の `validate_supported_bound` 撤廃 / 信号評価器の `strip_outlives_from_index` 削除 / `build_generics_fingerprint` の全境界種類対応 / 比較ロジック対称化が必要で、修正範囲は残る (主に `structural_eq.rs` と `child_items.rs`)。
- 旧 ADR `2026-05-13-1153-tddd-where-form-generics-normalization.md` の D3 / D2 を supersede するための frontmatter 更新が必要。

## Reassess When

- rustdoc が impl ブロックレベルのジェネリックをメソッドのジェネリックに統合するようになった場合 (Rust toolchain 更新): D2 の比較ロジックが不要になる可能性があり、スキーマ設計を再評価する。
- rustdoc が新しい境界の種類を追加した場合: D1 の方針 (任意の境界種類は文字列で表現) で原則として吸収できる。種類の意味論が文字列で表現できない場合はスキーマ拡張を検討する。
- rustdoc-types crate のメジャーバージョンアップで where 句 / 境界の構造が変わった場合: コーデックと信号評価器ロジックを更新する必要がある。

## Related

- `knowledge/adr/2026-05-13-1153-tddd-where-form-generics-normalization.md` — where 形式 generics 正規化。**本 ADR D1 は同 ADR の D3 を、D2 は同 ADR の D2 を supersede する**。D1 (BoundPredicate 正規化) は引き続き有効。
- `knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md` — TDDD-02 (4 グループ評価)。信号評価器 v2 の評価基盤。
- `knowledge/adr/2026-04-11-0003-type-action-declarations.md` — TDDD-03 (action 値別評価ロジック)。`action: modify` / `action: reference` の信号条件。
- `knowledge/adr/2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` — 3-way diff 評価 (信号評価器の Phase 1 / Phase 2 設計)。本 ADR の修正はこの評価器の内部非対称性を直す。
- `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` — catalogue スキーマの軸分離設計。D2 のフィールド追加は本 ADR の軸設計と整合する形で配置する。
