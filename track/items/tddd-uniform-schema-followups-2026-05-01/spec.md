<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 29, yellow: 0, red: 0 }
---

# TDDD uniform-schema 派生 ADR bundle (#1 method 完全型宣言 / #2 typestate transition edge / #4 cross-track port reference)

## Goal

- [GO-01] method / param の型フィールド (expected_methods[].returns, expected_methods[].params[].ty, FreeFunction.expected_params[].ty, FreeFunction.expected_returns) において、generic 引数を省略した bare wrapper 名のみの宣言を禁止し、完全な型文字列で宣言することを規範化する。これにより extract_type_names() が内部具象型への edge を生成でき、contract-map 上の method edge 漏れを解消する [adr: knowledge/adr/2026-04-29-0240-method-type-full-generic-declaration.md#D1]
- [GO-02] contract-map renderer の render_contract_map 関数の edge 生成経路を拡張し、Typestate variant の TypestateTransitions::To(names) を読んで transition edge を生成するようにする。これにより catalogue に宣言された typestate 遷移関係 (ProposedDecision -> AcceptedDecision 等) が contract-map 上に可視化される [adr: knowledge/adr/2026-04-29-0241-typestate-transition-edge-rendering.md#D1]
- [GO-03] secondary_adapter の implements[] が参照する port は当該 track の catalogue に必ず declare する規範を確立する。baseline 由来の port は action: "reference" で declare し、contract-map renderer の port_index lookup が unmatched となって impl edge が silently skip されるケースを解消する [adr: knowledge/adr/2026-04-29-0243-cross-track-port-reference.md#D1]
- [GO-04] knowledge/conventions/type-designer-kind-selection.md の R7 (Cross-Track Port Reference) を draft 段階から正式な運用ルールに昇格させ、type-designer agent が self-reject できる判断基準を SSoT として明示する [adr: knowledge/adr/2026-04-29-0243-cross-track-port-reference.md#D1] [conv: knowledge/conventions/type-designer-kind-selection.md#Rules]

## Scope

### In Scope
- [IN-01] method / param 型宣言の完全形規範の文書化: expected_methods[].returns / expected_methods[].params[].ty / FreeFunction.expected_params[].ty / FreeFunction.expected_returns において、Result / Option / Vec / Box / Arc / Rc / Cow / BTreeMap / HashMap / HashSet / BTreeSet を bare wrapper 名のみで宣言することを禁止するルールを catalogue authoring 規範として確立する [adr: knowledge/adr/2026-04-29-0240-method-type-full-generic-declaration.md#D1] [tasks: T001]
- [IN-02] bare wrapper 名 lint の後続作業の scope 定義: catalogue codec / verify CLI が schema validation で bare wrapper 名のみの宣言を reject する lint を実装することを後続作業と定義する。本 track では lint 実装は行わず、設計レビューで確認する過渡期間を規範として明示する [adr: knowledge/adr/2026-04-29-0240-method-type-full-generic-declaration.md#D1] [tasks: T001]
- [IN-03] contract-map renderer の typestate transition edge 生成拡張: libs/domain/src/tddd/contract_map_render.rs の methods_of() または同等の edge 生成経路を拡張し、TypestateTransitions::To(names) の各 name を type_index で resolve して transition edge (==>) を生成する。TypestateTransitions::Terminal は edge を生成しない。catalogue 未登録の遷移先名前は silently skip する。self-loop は抑制する [adr: knowledge/adr/2026-04-29-0241-typestate-transition-edge-rendering.md#D1] [tasks: T002]
- [IN-04] typestate transition edge の mermaid 表現: 既存の method-call edge (-->) および impl edge (-.impl.->) と視覚的に区別できる太線 (==>) を typestate transition edge の表記として採用する [adr: knowledge/adr/2026-04-29-0241-typestate-transition-edge-rendering.md#D1] [tasks: T002]
- [IN-05] cross-track port reference 規範の確立: secondary_adapter の implements[] で参照する trait は、当該 track の <layer>-types.json のいずれかに secondary_port として entry が必須であることを規範化する。当該 track で改変しない baseline 由来の port は action: "reference" で declare する [adr: knowledge/adr/2026-04-29-0243-cross-track-port-reference.md#D1] [tasks: T001]
- [IN-06] knowledge/conventions/type-designer-kind-selection.md への R7 (Cross-Track Port Reference) の正式追加: draft 段階の R7 を運用ルールとして finalize し、type-designer が secondary_adapter implements[] の参照先 port を当該 track catalogue に declare する義務 / action: "reference" の意味論 / declare 漏れの影響を判断基準として明示する [adr: knowledge/adr/2026-04-29-0243-cross-track-port-reference.md#D1] [conv: knowledge/conventions/type-designer-kind-selection.md#Rules] [tasks: T001]

### Out of Scope
- [OS-01] bare wrapper 名 lint の実装: catalogue codec / verify CLI での schema validation による機械的 reject は後続作業とする。本 track では lint 実装を行わない [adr: knowledge/adr/2026-04-29-0240-method-type-full-generic-declaration.md#D1]
- [OS-02] contract-map renderer の schema 拡張 / codec 追加 / baseline 拡張: typestate transition edge rendering は renderer のみの変更で解消できるため、schema 拡張・codec 追加・baseline 拡張は本 track では不要 [adr: knowledge/adr/2026-04-29-0241-typestate-transition-edge-rendering.md#D1]
- [OS-03] typestate 遷移 edge が密になった場合の表示制御強化: kind フィルタや edge 種別フィルタによる表示制御は本 track の scope 外とする。グラフが読めなくなった場合に別途検討する [adr: knowledge/adr/2026-04-29-0241-typestate-transition-edge-rendering.md#D1]
- [OS-04] contract-map renderer の baseline port 自動補完: secondary_adapter の implements[] のうち当該 track の port_index に存在しないものを workspace 全体の baseline から自動 resolve して edge を補完する案は却下されており、本 track の scope 外 [adr: knowledge/adr/2026-04-29-0243-cross-track-port-reference.md#D1]
- [OS-05] merge gate の integrity check 実装: implements[] の trait_name が当該 track または baseline に存在するかを verify する gate の実装は後続作業として位置づけられ、本 track では実施しない [adr: knowledge/adr/2026-04-29-0243-cross-track-port-reference.md#D1]
- [OS-06] 過去 track の既存 catalogue の retrofit: cross-track port reference 規範は新規 track から適用する。過去 track の catalogue で reference declare が漏れている entry の一括修正は本 track の scope 外 [adr: knowledge/adr/2026-04-29-0243-cross-track-port-reference.md#D1]

## Constraints
- [CN-01] bare wrapper 名のみの宣言禁止リスト: Result / Option / Vec / Box / Arc / Rc / Cow / BTreeMap / HashMap / HashSet / BTreeSet を generic 引数なしで単独宣言してはならない。これらが具象型を伴わず宣言された場合、extract_type_names() は wrapper 名 token のみを返し内部具象型への edge が生まれない [adr: knowledge/adr/2026-04-29-0240-method-type-full-generic-declaration.md#D1] [tasks: T001]
- [CN-02] typestate transition edge 生成は renderer 側のみの変更とする。schema 拡張・codec 追加・baseline 拡張は行わない。TypestateTransitions 型は既存 schema のまま利用する [adr: knowledge/adr/2026-04-29-0241-typestate-transition-edge-rendering.md#D1] [tasks: T002]
- [CN-03] typestate transition edge の表記は太線 (==>) を使い、method-call edge (-->) および impl edge (-.impl.->) と視覚的に区別する。edge ラベルは transitions_to とする [adr: knowledge/adr/2026-04-29-0241-typestate-transition-edge-rendering.md#D1] [tasks: T002, T003]
- [CN-04] catalogue 未登録の遷移先名前は silently skip する。TypestateTransitions::Terminal の場合は edge を描画しない。self-loop は抑制する (現行 method edge と同じ self-loop 抑制ロジックを適用する) [adr: knowledge/adr/2026-04-29-0241-typestate-transition-edge-rendering.md#D1] [tasks: T002, T003]
- [CN-05] secondary_adapter の implements[] で参照する trait は必ず当該 track の <layer>-types.json に secondary_port として entry を作成する。当該 track で変更しない baseline 由来の port は action: "reference" で declare する。declare 漏れは contract-map renderer の port_index lookup unmatched を引き起こし -.impl.-> edge が silently skip される [adr: knowledge/adr/2026-04-29-0243-cross-track-port-reference.md#D1] [tasks: T001]
- [CN-06] action: "reference" の type-signal evaluator の評価は「完全一致のみ Blue、不一致はすべて Red」とする。modify の Yellow 吸収は reference action には適用されない [adr: knowledge/adr/2026-04-29-0243-cross-track-port-reference.md#D1] [tasks: T001]
- [CN-07] reference action で declare する baseline port の expected_methods は baseline 当時の全 method を列挙する。method 型宣言の完全形規範 (CN-01) は reference action でも同様に要求される [adr: knowledge/adr/2026-04-29-0243-cross-track-port-reference.md#D1] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] knowledge/conventions/type-designer-kind-selection.md に method 型宣言の完全形規範が追記されており、bare wrapper 名のみ宣言の禁止リスト (Result / Option / Vec / Box / Arc / Rc / Cow / BTreeMap / HashMap / HashSet / BTreeSet) と良い例 / 悪い例が Examples セクションに示されている [adr: knowledge/adr/2026-04-29-0240-method-type-full-generic-declaration.md#D1] [tasks: T001]
- [ ] [AC-02] libs/domain/src/tddd/contract_map_render.rs の typestate 担当 edge 生成経路が TypestateTransitions::To(names) を読んで各 name を type_index で lookup し、resolve できた名前について ProposedDecision ==>|transitions_to| AcceptedDecision 形式の mermaid edge を生成する [adr: knowledge/adr/2026-04-29-0241-typestate-transition-edge-rendering.md#D1] [tasks: T002]
- [ ] [AC-03] TypestateTransitions::Terminal の typestate は transition edge を生成しない。catalogue 未登録の遷移先名前は edge 生成対象から除外される。TypestateTransitions::To(["Self"]) のような self-loop は edge 生成から除外される [adr: knowledge/adr/2026-04-29-0241-typestate-transition-edge-rendering.md#D1] [tasks: T002, T003]
- [ ] [AC-04] knowledge/conventions/type-designer-kind-selection.md に R7 (Cross-Track Port Reference) が正式ルールとして追加されており、secondary_adapter implements[] の参照先 port の declare 義務 / action: "reference" の意味論 / declare 漏れが -.impl.-> edge の silently skip を引き起こす旨が記述されている [adr: knowledge/adr/2026-04-29-0243-cross-track-port-reference.md#D1] [conv: knowledge/conventions/type-designer-kind-selection.md#Rules] [tasks: T001]
- [ ] [AC-05] action: "reference" で declare した secondary_port entry が contract-map renderer の port_index に登録され、対応する secondary_adapter から -.impl.-> edge が生成されることを、新規 track または新規テスト catalogue 上でユニットテストにより確認できる。過去 track の既存 catalogue (例: FsReviewStore / ReviewReader) の retrofit は本 track では行わない (OS-06) [adr: knowledge/adr/2026-04-29-0243-cross-track-port-reference.md#D1] [tasks: T003]
- [ ] [AC-06] cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) が pass する。renderer 拡張 (IN-03) の変更が既存テストを壊さず、typestate transition edge 生成の新規ユニットテストが追加されている [adr: knowledge/adr/2026-04-29-0241-typestate-transition-edge-rendering.md#D1] [tasks: T002, T003]

## Related Conventions (Required Reading)
- knowledge/conventions/type-designer-kind-selection.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 29  🟡 0  🔴 0

