<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 27, yellow: 0, red: 0 }
---

# contract-map renderer の dyn Trait edge 解決

## Goal

- [GO-01] contract-map renderer が TypeRef 位置に現れる `dyn Trait` 参照を宣言済み trait への依存 edge として描画し、hexagonal 頻出パターン (factory port の `Arc<dyn Port>` 返却、interactor への `Arc<dyn Port>` 注入等) の port-to-port 依存が rendered view から系統的に欠落しないようにする。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D1]
- [GO-02] `dyn Trait` 由来の trait 参照と `trait_impl` の `trait_ref` を同じ解決規則で扱い、renderer 内の trait 解決の二重実装と drift を防ぐ。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D2]

## Scope

### In Scope
- [IN-01] TypeRef 文字列から抽出される候補を「型候補」と「trait 候補」の 2 系統に分ける。`dyn Trait` (TraitObject) が現れた場合は bound の path 自身のみを trait 候補とし、lifetime bound は候補に加えない。既存の型候補抽出経路は不変とする。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D1] [tasks: T001]
- [IN-02] TraitObject の bound path 内の generic 引数および associated-type binding の値側は、通常の型候補として再帰走査する。bound path 自身は型候補に混ぜない。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D1] [tasks: T001]
- [IN-03] 複数 bound を持つ TraitObject (`dyn A + B` や `dyn Port + Send` 等) は、各 bound を独立に trait 候補として解決し、宣言済み trait すべてに edge を張る。未宣言の bound はスキップしても他の bound の解決結果に影響しない。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D1] [tasks: T001]
- [IN-04] trait 候補は以下の 4 段解決規則で解決する: (a) 非修飾名は自 crate の trait として `(current_crate, name)` で引く、(b) `crate::` / `self::` / `super::` prefix を持つ修飾付きパスは prefix を除いた末尾 segment を trait 名として `(current_crate, name)` で引く、(c) それ以外の修飾付きパスは先頭 segment を crate、末尾 segment を trait 名として `(crate, name)` で引く、(d) いずれでも引けなければ edge を張らずに黙ってスキップする。型候補は従来どおりこの trait fallback を受けない。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D2] [tasks: T001]
- [IN-05] `dyn Trait` の trait 名解決と `trait_impl` の `trait_ref` 解決は同じ 4 段解決規則で扱う。`trait_impl` の `trait_ref` に `crate::` / `self::` / `super::` prefix が付いた場合も (b) に従い末尾 segment を自 crate の trait 名として解決するよう既存意味論を拡張する。bare name / prefix を持たない他 crate 修飾パス / 外部 trait の silent skip という既存挙動は変えない。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D2] [tasks: T001]
- [IN-06] 解決済み trait target への edge は trait の代表ノード id (`trait_rep_node_id`) に張り、trait subgraph 全体を指す id には張らない。クラスタ境界のレイアウト破壊回避という既存方針を踏襲する。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D2] [tasks: T001]
- [IN-07] TypeRef 位置に現れる `dyn Trait` の解決は field / variant payload / type alias / fn param / fn return / method param / method return のすべての位置で一貫して行われる (returns / params 位置に限らない)。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D3] [tasks: T001]
- [IN-08] 解決済み trait target への edge は、その TypeRef が出現した位置の既存 edge style (method_returns / method_param / transition / field / variant_payload / alias) と同じ視覚的表現で描画される。新しい edge style key は導入しない。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D4] [tasks: T001]
- [IN-09] renderer の回帰テストとして ADR §D5 に列挙された 7 ケース (正常系 / 外部 trait / 同名衝突 / 複数 bound / generic + associated type / 自 crate prefix 3 種 / 共通 resolver 適用) をすべて追加する。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D5] [tasks: T002]

### Out of Scope
- [OUT-01] NodeIndex に TraitEntry を混載する解 (Rejected Alternative A) は採用しない。plain TypeRef が同名 trait に誤 link する退行を再導入しないため、型のみの NodeIndex を維持し、`dyn` 文脈で現れた名前のみが trait 解決を受ける形を保つ。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D1] [tasks: T001]
- [OUT-02] 未宣言 trait への edge を可視化する「ghost node」(Rejected Alternative B) は生成しない。「edge は宣言済み型・trait の間のみ」という既存原則を維持し、未解決 bound は silent skip で扱う。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D2] [tasks: T001]
- [OUT-03] `dyn Trait` edge 用の新しい edge style config key を追加しない。既存 style key が欠ける場合の fail-closed 挙動も不変とする。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D4] [tasks: T001]
- [OUT-04] `impl Trait` 戻り値、`<T as Trait>::Assoc` の関連型解決など、Rust の trait object 以外の trait 出現位置は本 track では対象外とする。Reassess When に列挙された将来の再検討対象として扱う。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D1] [tasks: T001]
- [OUT-05] catalogue schema、TypeRef 文字列表現、`architecture-rules.json`、spec.json など上流の SoT には手を入れない。改修は contract-map renderer 内部 (`libs/infrastructure/src/tddd/contract_map_renderer_adapter/render/`) に閉じる。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D3] [tasks: T001]

## Constraints
- [CN-01] 同名衝突する type `Foo` と trait `Foo` が併存する catalogue で、plain TypeRef `Foo` は型に link し、`dyn Foo` (TraitObject 文脈の `Foo`) だけが trait に link する。plain TypeRef の従来の解決結果は変えない。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D2] [tasks: T001, T002]
- [CN-02] 未宣言 trait bound、workspace 外部 trait、`Send` / `Sync` などの marker trait は edge を張らずに silent skip し、panic を起こさない。この silent skip 規則は `dyn Trait` 側と `trait_impl` 側で同一とする。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D2] [tasks: T001, T002]
- [CN-03] TypeRef 解決の戻り値は型経由と trait 経由で解決した代表ノード id の和集合とし、同じ id は 1 回だけ返す。同一 TypeRef から重複 edge が描画されないようにする。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D2] [tasks: T001, T002]
- [CN-04] `trait_impl` の `trait_ref` 側の既存挙動 (bare name → 自 crate の trait、prefix を持たない修飾付きパス → 他 crate の trait、workspace 外部 trait → silent skip) は、自 crate prefix 正規化 (IN-05) の追加以外は不変とする。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D2] [tasks: T001, T002]

## Acceptance Criteria
- [ ] [AC-01] `fn build(&self) -> Arc<dyn DeclaredPort>` を持つ trait / method と同 catalogue に `DeclaredPort` trait entry がある場合、rendered contract-map view に `build --> DeclaredPort_rep` の trait edge が描画されることを検証する。加えて、field / variant payload / type alias / fn param / fn return / method param / method return の各 TypeRef 位置に別個の宣言済み trait を指す `dyn` を置く 1 つの renderer fixture で、各出現位置から対応する trait の代表ノードへの edge がそれぞれ 1 本ずつ描画されることを検証する。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D5, knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D3] [tasks: T002]
- [ ] [AC-02] `Arc<dyn std::fmt::Debug>` など catalogue に宣言されていない外部 trait が TypeRef 位置に現れても、renderer は panic せず、その trait への edge を張らないことを検証する。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D5] [tasks: T002]
- [ ] [AC-03] 同名の type `Foo` と trait `Foo` が併存する catalogue で、plain TypeRef `Foo` は型ノードに、`Arc<dyn Foo>` は trait ノードに、それぞれ独立に link されることを検証する。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D5] [tasks: T002]
- [ ] [AC-04] `Arc<dyn DeclaredPort + DeclaredMarker>` で `DeclaredPort` と `DeclaredMarker` の両方の宣言済み trait に edge を張り、同じ位置に未宣言の `Send` を加えても追加 edge を出さないことを検証する。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D5] [tasks: T002]
- [ ] [AC-05] 同 catalogue に `GenericType` と `AssociatedType` の別々の type entry がある `Arc<dyn DeclaredPort<GenericType, Item = AssociatedType>>` で、`DeclaredPort` への trait edge、generic 引数側の `GenericType` への通常の type edge、associated-type binding の値側の `AssociatedType` への通常の type edge がそれぞれ 1 本ずつ張られることを検証する。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D5] [tasks: T002]
- [ ] [AC-06] `Arc<dyn crate::port::DeclaredPort>` / `Arc<dyn self::port::DeclaredPort>` / `Arc<dyn super::port::DeclaredPort>` のいずれの記法でも `(current_crate, DeclaredPort)` として解決し、`DeclaredPort_rep` への trait edge が張られることを検証する (中間 segment は解決に使わない)。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D5] [tasks: T002]
- [ ] [AC-07] `trait_impl` の `trait_ref` に `crate::` / `self::` / `super::` prefix を使っても `DeclaredPort_rep` への trait_impl edge を張り、bare name / prefix を持たない他 crate path / 外部 trait を持つ既存 trait_impl の解決結果は変わらないことを検証する。加えて、trait-index lookup と 4 段解決規則は単一の共有 resolver に集約され、TypeRef 由来の trait 候補経路と `trait_impl` の `trait_ref` 経路の双方がその resolver に委譲することを、renderer source の関数定義および呼び出し箇所の静的検査で検証する。 [adr: knowledge/adr/2026-07-13-0308-contract-map-dyn-trait-return-edge.md#D5] [tasks: T002]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/catalogue-schema-reference.md#TypeRef rules
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/no-upstream-restatement.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 27  🟡 0  🔴 0

