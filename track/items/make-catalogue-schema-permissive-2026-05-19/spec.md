<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 33, yellow: 0, red: 0 }
---

# catalogue スキーマを Rust 文法に対して寛容な設計にする

## Goal

- [GO-01] catalogue スキーマが Rust 文法の表現力に対して中立になるよう拡張し、catalogue 設計者が Rust ソースコードの構造を catalogue 上で正規に宣言できるようにする。これにより、構造的に解消できない false-positive (偽陽性) 信号をゼロにし、TDDD strict signal gate が迂回なく機能するようにする [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D1, knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D2, knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D3, knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D4]

## Scope

### In Scope
- [IN-01] WherePredicateDecl スキーマを lhs / rhs / operator の 3 フィールド構造 (single struct + BoundOp enum) に拡張する。lhs は HRTB バインダーを含む任意の Rust where 句の左辺、rhs は '+' 連結された複数の境界を格納する Vec<TypeRef>、operator は Bound (':') か Equal ('=') を示す BoundOp。WherePredicateDecl でカバーするのは Rust where 句として現れる制約 (ライフタイム境界 / HRTB on where 句 / where 句ライフタイム境界制約 / where 句等価制約 / トレイト境界以外の任意境界種類) である。HRTB on トレイト境界は where 句の lhs に HRTB バインダーを文字列として組み込む。精密捕捉 (use<'a, T>) は where 句でなく impl Trait 返却型の修飾子であり、WherePredicateDecl ではなく MethodGenericParam.bounds 文字列として保持し、IN-02 の codec 寛容化によって accept する [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D1] [tasks: T001, T003]
- [IN-02] A-codec の validate_supported_bound を撤廃し、境界の種類 (ライフタイム境界 / HRTB / 精密捕捉等) による reject を撤廃する。syn でパース可能な境界文字列はすべて受け入れる [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D1] [tasks: T002]
- [IN-03] 信号評価器の strip_outlives_from_index を撤廃し、両側 (A 側・C 側) でライフタイム境界を保持する。build_generics_fingerprint を全境界種類 (ライフタイム境界 / HRTB / 精密捕捉等) および BoundOp に対応させ、{lhs, rhs, operator} 単位でフィンガープリントを生成する。rhs の要素は集合として正規化 (順序非依存ソート) し、T: A + B と T: B + A を同一フィンガープリントにする [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D1] [tasks: T003]
- [IN-04] A 側・C 側のいずれかで syn のパースが失敗した境界文字列は、信号評価の対象外として error を返す。信号 (Blue / Yellow / Red) ではなく明示的なエラーとして catalogue 設計者に伝える [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D1] [tasks: T003]
- [IN-05] inherent impl ブロック用の新エントリ InherentImplDeclV2 を導入する。type_name (対象 struct 名)、impl_generics (MethodGenericParam の配列、型パラメータのみ)、impl_where_predicates (WherePredicateDecl の配列)、impl ブロック内のメソッド一覧などを保持する。1 struct に対して複数の inherent impl ブロックを複数エントリで表現できる。TypeEntry には impl 情報を持たせない [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D2] [tasks: T004]
- [IN-06] TraitImplDeclV2 に impl_generics フィールド (MethodGenericParam の配列、型パラメータのみ) と impl_where_predicates フィールド (WherePredicateDecl の配列) を追加する。各 trait impl ブロックが独自のジェネリックと where 句を持てるようにする [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D2] [tasks: T005]
- [IN-07] TraitEntry にトレイト宣言レベルのジェネリック宣言部分を保持するフィールドを追加する (generics: MethodGenericParam の配列 (型パラメータのみ)、where_predicates: WherePredicateDecl の配列)。旧 ADR 2026-05-13-1153-tddd-where-form-generics-normalization.md D2 で先送りされていたトレイトエントリの generics スキーマ化を完了する [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D2] [tasks: T006]
- [IN-08] D2 のスコープ限定: InherentImplDeclV2 / TraitImplDeclV2 / TraitEntry に追加する MethodGenericParam は型パラメータのみをサポートする。ライフタイムパラメータ ('a 等) および const パラメータ (const N: usize 等) は本トラックのスコープ外とし、将来の拡張で対応する [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D2] [tasks: T004, T005, T006]
- [IN-09] A-codec はエンコード時に rustdoc の impl ブロック・トレイト宣言レベルのジェネリックを正しいフィールド (InherentImplDeclV2.impl_generics / TraitImplDeclV2.impl_generics / TraitEntry.generics) に配置する。信号評価器は rustdoc 側の各レベルのジェネリックと catalogue 側の対応フィールドを対称な形で比較する [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D2] [tasks: T007]
- [IN-10] A 側にも B 側と同様の Pre-step を導入し、全 A Item に fresh S id を一括 pre-allocate する (a_id_remap の構築)。Step 4 & 5 の action 別処理 (Add / Modify / Reference / Delete) は a_id_remap を参照して id を解決する [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D3, knowledge/adr/2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md#D2] [tasks: T008]
- [IN-11] patch_impl_for_ids 関数および patch_impl_trait_ids 関数を削除する。rewrite_type_ref_ids_in_item のみで for_ の remap が完結するようにする。自クレート Item を指す id は a_id_remap / b_id_remap で新 S id に remap し、外部クレート Item を指す id は external_crates rebuild で新 crate_id に rebind する [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D3] [tasks: T009]
- [IN-12] 旧 ADR 2026-05-13-1153-tddd-where-form-generics-normalization.md の D2 (TraitEntry generics スキーマ化の deferred 項目) および D3 (fail-closed スコープ限定) が本トラックで supersede されることを反映し、旧 ADR の frontmatter を更新する [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D1, knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D2] [tasks: T010]
- [IN-13] 信号評価器 C 側の build_impl_identity_map 内にある for_is_external filter (for_ 型の crate_id をチェックして外部型を指す impl を c_impls から除外する処理) を撤廃し、cross-crate impl も c_impls に含める。B 側 orphan impl 挿入処理 (phase1/builder.rs) は現状維持 (filter なしで全 impl を S に挿入する既存の挙動)。結果として両側に cross-crate impl が含まれる状態になり、fingerprint が対称に生成されて Blue が得られる。また、cross-crate impl を catalogue で declare できるようにするため、TraitImplDeclV2 に for_ フィールド (Option<String>、serde default None) を追加する (CN-01 の後方互換設計)。for_ には self-type の fully-qualified path (例: domain::tddd::new_typegraph_codec_error::NewTypeGraphCodecError) を記述し、A-codec は fully-qualified path を parse して外部クレートを判定し、external_crates および paths に登録する。for_ が None の場合は従来通り親 TypeEntry の型が for_ として使用される [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D4] [tasks: T012]

### Out of Scope
- [OS-01] ライフタイムパラメータ ('a 等) および const パラメータ (const N: usize 等) の impl/trait ジェネリックへの追加。これらは将来の拡張トラックで対応する [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D2]
- [OS-02] Linter コンポーネントの変更。Linter は CatalogueDocument の role / pattern × 構造制約を enforcement する別コンポーネントであり、本トラックの対象外 [adr: knowledge/adr/2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md#D6]
- [OS-03] 信号評価器の Phase 1 (S 構築 + D 構築) における action 別処理の意味論変更。D3 は id 管理の Pre-step 追加のみを変更対象とし、Add / Modify / Reference / Delete の各 action がどう id を扱うかは旧 ADR 2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md の決定通りとする [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D3]
- [OS-04] catalogue スキーマの version 番号の bump。D1/D2/D3/D4 の変更はいずれも serde default での後方互換拡張とし、既存 catalogue ファイルは無修正でロードできる [adr: knowledge/adr/2026-05-13-1153-tddd-where-form-generics-normalization.md#D2]

## Constraints
- [CN-01] 本トラックで追加するすべての省略可能フィールド (Vec フィールド: impl_generics / impl_where_predicates / generics / where_predicates 等、および Option フィールド: TraitImplDeclV2.for_ 等) は serde default で空 Vec / None をデフォルト値とし、既存の catalogue ファイルが無修正でロードできる後方互換設計を維持する。type_name などの必須識別子フィールドは serde default の対象外とし、deserialize 時に常に存在が要求される [adr: knowledge/adr/2026-05-13-1153-tddd-where-form-generics-normalization.md#D2] [tasks: T001, T004, T005, T006, T012]
- [CN-02] 信号評価器の変更はヘキサゴナルアーキテクチャのレイヤー依存規則に従う。signal evaluator の orchestration は libs/usecase/ に置き、rustdoc 解析・codec は libs/infrastructure/ に置く。domain 層 (libs/domain/) は純粋なドメインロジックのみを保持する [adr: knowledge/adr/2026-05-11-2330-catalogue-impl-signals-command-layering.md#D1] [tasks: T002, T003, T007, T008, T009, T012]
- [CN-03] B 側と A 側の id pre-allocate Pre-step は対称な設計にする。id 管理の統一後、for_ の処理は rewrite_type_ref_ids_in_item + id_map の 1 経路に集約され、patch_impl_for_ids のような補完関数を追加しない [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D3, knowledge/adr/2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md#D2] [tasks: T008, T009]

## Acceptance Criteria
- [ ] [AC-01] WherePredicateDecl が lhs (TypeRef 型)、rhs (Vec<TypeRef> 型)、operator (BoundOp 型) の 3 フィールドを持ち、HRTB バインダー付き where 句 (例: for<'a> T::Item<'a>: Bound)、ライフタイム境界 (例: T: 'static)、等価制約 (例: T::Assoc = U) を catalogue で正規に宣言できる。なお精密捕捉 (use<'a, T>) は where 句構造ではなく impl Trait 返却型の修飾子であり、MethodGenericParam.bounds 文字列として保持して IN-02 の codec 寛容化によって accept する [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D1] [tasks: T001, T003]
- [ ] [AC-02] validate_supported_bound が撤廃され、<F: Fn(...) + Send + Sync + 'static> のようなライフタイム境界を含む宣言が A-codec でエラーなくエンコードされる [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D1] [tasks: T002]
- [ ] [AC-03] strip_outlives_from_index が撤廃され、ライフタイム境界を含む where 句を両側 (A 側・C 側) で保持したまま build_generics_fingerprint が {lhs, rhs, operator} 単位でフィンガープリントを生成する。T: A + B と T: B + A が同一フィンガープリントになる [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D1] [tasks: T003]
- [ ] [AC-04] syn でパース不能な境界文字列 (例: for<'a (未閉じ binder)) を catalogue に記述した場合、信号 (Blue/Yellow/Red) ではなく明示的なエラーが返される [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D1] [tasks: T003]
- [ ] [AC-05] InherentImplDeclV2 エントリが catalogue に宣言でき、1 struct に対して複数の inherent impl ブロックを複数エントリで表現できる。A-codec は rustdoc の impl ブロックレベルのジェネリックを InherentImplDeclV2.impl_generics / impl_where_predicates に正しく配置し、信号評価器はそれを対称に比較する [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D2] [tasks: T004, T007]
- [ ] [AC-06] TraitImplDeclV2 に impl_generics と impl_where_predicates フィールドが追加され、impl<L, R, W> Trait for Foo<L, R, W> where L: Send のような trait impl ブロック固有のジェネリックと where 句を catalogue で正規に宣言できる [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D2] [tasks: T005, T007]
- [ ] [AC-07] TraitEntry に generics と where_predicates フィールドが追加され、trait Foo<T> where T: Clone のようなトレイト宣言レベルのジェネリックを catalogue で正規に宣言できる [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D2] [tasks: T006, T007]
- [ ] [AC-08] 既存の catalogue ファイル (impl_generics / impl_where_predicates / TraitEntry.generics / TraitImplDeclV2.for_ フィールドが存在しないもの) が無修正でロードでき、信号評価の結果が変化しない [adr: knowledge/adr/2026-05-13-1153-tddd-where-form-generics-normalization.md#D2] [tasks: T001, T004, T005, T006, T007, T012]
- [ ] [AC-09] A 側に B 側と対称な Pre-step が追加され、全 A Item に fresh S id が一括 pre-allocate される (a_id_remap の構築)。action 別処理 (Add / Modify / Reference / Delete) は a_id_remap を参照して id を解決する [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D3] [tasks: T008]
- [ ] [AC-10] patch_impl_for_ids 関数および patch_impl_trait_ids 関数が削除され、for_ の remap が rewrite_type_ref_ids_in_item + a_id_remap / b_id_remap の 1 経路に統一される。外部クレート型を対象とする impl (例: impl From<MyError> for OtherCrateError) の for_ が誤って自クレート親型 id に上書きされなくなる (remap 経路の安定化保証。for_is_external filter の撤廃は D4 / AC-12 が担う) [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D3] [tasks: T009]
- [ ] [AC-11] 旧 ADR 2026-05-13-1153-tddd-where-form-generics-normalization.md の D2 と D3 の status が superseded に更新され、superseded_by フィールドが 2026-05-18-1223-make-catalogue-schema-permissive.md への参照を含む [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D1, knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D2] [tasks: T010]
- [ ] [AC-12] build_impl_identity_map の for_is_external filter が撤廃され、cross-crate impl (例: impl From<CatalogueToExtendedCrateCodecError> for NewTypeGraphCodecError) が c_impls に含まれる。B 側 (s_impls) と C 側 (c_impls) の両側に cross-crate impl が対称に含まれることで fingerprint が一致し、SMinusC_Reference 扱いで false-positive Red になるパターンが解消される。cross-crate impl を catalogue に declare する場合、TraitImplDeclV2.for_ に fully-qualified path を記述することで A-codec が外部クレートを正しく判定し external_crates および paths に登録する [adr: knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md#D4] [tasks: T012]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/no-backward-compat.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 33  🟡 0  🔴 0

