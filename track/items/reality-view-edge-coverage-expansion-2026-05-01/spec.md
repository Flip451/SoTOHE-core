<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 29, yellow: 0, red: 0 }
---

# Reality View renderer の edge カバレッジ拡張 — receiver-less method / trait-method incoming + 起源別視覚区別

## Goal

- [GO-01] Reality View (TypeGraph ベース mermaid 図) の collect_edges() が見落とす 3 種の edge 起源 — (1) receiver-less associated function の戻り値・引数型、(2) trait method の戻り値・引数型 — を新たに edge 化することで、これまで孤立していた error 型・factory 引数型などを関係グラフに接続し、実装状態の俯瞰精度を高める [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D1, knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D2]
- [GO-02] edge 起源 (引数由来 / 戻り値由来) を mermaid 上で形状と色によって視覚的に区別することで、edge 数増加後も「入力依存 (argument)」と「出力依存 (return)」を即座に読み分けられる一貫した表記体系を確立する [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D3]

## Scope

### In Scope
- [IN-01] collect_edges() の EdgeSet::Methods 分岐における receiver-less 判定ガード (method.receiver().is_none() による early-continue) の撤廃。associated function (constructor / factory / parser) も edge 起源として走査する [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D1] [tasks: T001]
- [IN-02] collect_edges() における method 引数型 (method.params() の各 ParamDeclaration::ty()) の走査追加。戻り値型 (method.returns()) と引数型の両方を extract_type_names にかけ、workspace 型名への edge を発生させる。self-loop 抑制 (target != source_name) は引数側にも適用する [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D1] [tasks: T001]
- [IN-03] edge_kind タグの分離: 現行の単一値 "method" を "method_return" (戻り値由来) と "method_param" (引数由来) の 2 値に分割する。render_edge_symbol の dispatch と snapshot テスト fixture を新 2 値に対応させる [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D1] [tasks: T001, T003, T004]
- [IN-04] collect_edges() への trait method 走査ブロック追加。EdgeSet::Methods | EdgeSet::All に対して graph.trait_names() をループし、各 trait の methods() の戻り値型・引数型から workspace 型への edge を発生させる。edge_kind は戻り値由来を "trait_method_return"、引数由来を "trait_method_param" とする [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D2] [tasks: T002]
- [IN-05] EdgeSet::Methods 単独で呼ばれた場合に D2 の trait edge source となる trait ノードがクラスタに存在しないケースの対処: trait ノードを edge target が属するクラスタかモジュールパス由来のクラスタへ明示的に追加する [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D2] [tasks: T002, T004]
- [IN-06] render_edge_symbol() の kind dispatch 拡張: "method_return" / "trait_method_return" を --> (実線・塗りつぶし矢印) に、"method_param" / "trait_method_param" を --o (実線・open circle 終端) にマップする新 match arm を追加する [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D3] [tasks: T003]
- [IN-07] 引数由来 edge のグレー着色: render_type_graph_clustered および render_type_graph_flat の各関数で、引数由来 edge ("method_param" / "trait_method_param") の出現順インデックスを収集し、mermaid ブロック末尾に linkStyle <i1>,<i2>,... stroke:#888; を 1 行で追記する [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D3] [tasks: T003]
- [IN-08] snapshot テスト fixture の更新: edge_kind 分離 (IN-03) および render_edge_symbol 拡張 (IN-06) に伴い、既存の snapshot テストを新しい 4 値 kind と --o / linkStyle 出力に対応するよう更新する [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D1, knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D3] [tasks: T004]

### Out of Scope
- [OS-01] enum variant payload 型の edge 化: MemberDeclaration::Variant が抱える workspace 型を edge にする変更。variant payload を edge 化するには catalogue / TypeGraph / baseline 3 輪の同時拡張 (ADR 2026-04-26-0855 Core invariant) が必要であり、本トラックのスコープ外として別 ADR に切り出す [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D1] [tasks: T001]
- [OS-02] trait impl の generic 引数 edge 化 (例: impl From<DomainError> for FooError の DomainError への edge): 本 ADR では「trait impl edge は trait short name のみ」という現行動作を維持し、generic 引数の edge 化は別 ADR 案件とする [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D1] [tasks: T001]
- [OS-03] render_type_graph_overview の linkStyle 着色: overview はクラスタ間 edge を集約出力するため個別 edge インデックスが存在せず、引数由来 / 戻り値由来の色分けは適用しない。overview の edge はすべて既定色のままとする [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D3] [tasks: T003]
- [OS-04] 凡例ブロック (legend) の追加: index.md 上部への凡例ブロックは本トラックでは導入しない。cluster ファイル先頭コメントで --o が引数由来であることを 1 行示すにとどめる。凡例の正式導入は別 ADR で扱う [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D3] [tasks: T003]
- [OS-05] Contract Map (catalogue 入力ベース) への同一スキームの移植: D3 の引数由来 / 戻り値由来の視覚スキームを Contract Map side に将来移植する余地を残すが、本トラックは Reality View のみを対象とする [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D3] [tasks: T003]

## Constraints
- [CN-01] 変更範囲は renderer 層 (libs/infrastructure/src/tddd/type_graph_render.rs) のみに限定する。catalogue schema / TypeGraph schema / baseline schema には一切変更を加えない。schema 変更を伴う拡張は別 ADR で扱う [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D1, knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D2] [tasks: T001, T002, T003]
- [CN-02] self-loop 抑制ロジック (target != source_name) は引数側・戻り値側ともに適用する。D1 の associated function 追加分と D2 の trait method 追加分の両方でこの抑制を適用する [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D1, knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D2] [tasks: T001, T002]
- [CN-03] edge dedup は現行の edges.sort(); edges.dedup(); で吸収する。同一 (source, label, target, kind) 組は自動マージされるため、専用の dedup ロジックを追加しない [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D1] [tasks: T001, T002]
- [CN-04] D2 の trait 走査対象は workspace に存在する trait のみとする。graph.trait_names() がそもそも workspace 限定であり、外部クレート trait を走査する処理は追加しない [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D2] [tasks: T002]
- [CN-05] D3 の視覚区別は形状を主軸 (--o vs -->) とし色 (linkStyle グレー) を補強として与える。色のみによる区別は採用しない。field edge (---) と impl edge (-.impl.->) は本変更の影響範囲外とし現行表記を維持する [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D3] [tasks: T003]
- [CN-06] Reality View は実装状態の検証ドリルダウン artifact として位置付けを維持する。本トラックの変更が ADR 2026-04-16-2200 §D10 が定める Reality View と Contract Map の役割分担 (Reality View = per-layer archaeology・Contract Map = 設計意図の俯瞰) を変更しない [adr: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md#D10] [tasks: T001, T002, T003]

## Acceptance Criteria
- [ ] [AC-01] 直前トラック (cli-via-usecase-only-2026-04-30) の domain-graph で孤立していた 6 ノード (CatalogueLinterError / CatalogueLinterRuleError / CatalogueLoaderError / ContractMapWriterError / ContractMapContent / TypestateTransitions) のうち、associated function または trait method からの edge が存在する型は、本変更後の graph で 1 本以上の edge を持つ (孤立から脱する) [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D1, knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D2] [tasks: T001, T002]
- [ ] [AC-02] fn try_new(rule_kind: CatalogueLinterRuleKind, ...) -> Result<Self, CatalogueLinterRuleError> のような receiver-less associated function から、CatalogueLinterRuleKind および CatalogueLinterRuleError の双方への edge が graph に出力される [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D1] [tasks: T001]
- [ ] [AC-03] CatalogueLinter::run(rules: &[CatalogueLinterRule], catalogue: &TypeCatalogueDocument, ...) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> のような trait method から、CatalogueLinterRule / TypeCatalogueDocument / CatalogueLintViolation / CatalogueLinterError への 4 本の edge が graph に出力される [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D2] [tasks: T002]
- [ ] [AC-04] 戻り値由来 edge が mermaid の --> (実線・塗りつぶし矢印) で出力される。引数由来 edge が mermaid の --o (実線・open circle 終端) で出力される。render_type_graph_clustered / render_type_graph_flat の出力において形状の区別が確認できる [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D3] [tasks: T003]
- [ ] [AC-05] 引数由来 edge が存在する場合、mermaid ブロック末尾に linkStyle <idx>,... stroke:#888; が追記される。linkStyle の index が edge の出現順序と一致している [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D3] [tasks: T003]
- [ ] [AC-06] render_type_graph_overview の出力には linkStyle 着色が追加されない (overview は個別 edge インデックスを持たないため、着色は clustered / flat に限定される) [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D3] [tasks: T003]
- [ ] [AC-07] field edge (---) と impl edge (-.impl.->) は変更前と同一の mermaid 表記で出力される (リグレッションなし) [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D3] [tasks: T003, T004]
- [ ] [AC-08] cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) が pass する。既存の snapshot テストが新しい edge_kind 4 値と --o / linkStyle 出力に対応するよう更新され、テストスイートが通る [adr: knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D1, knowledge/adr/2026-05-01-1226-reality-view-edge-coverage-expansion.md#D3] [tasks: T004]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/source-attribution.md#Source Tag Types
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 29  🟡 0  🔴 0

