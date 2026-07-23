<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 16, yellow: 0, red: 0 }
---

# 型契約パイプラインの規範と機構を実挙動に整合させる

## Goal

- [GO-01] 型契約パイプラインについて、ValueObject の domain 配置規律、接触時に強制される catalogue lint、空層成果物の正規扱い、および contract-map の role style 完全性を、文書と機構の双方で一貫して適用できるようにする。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D1, knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D2, knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D3, knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D4]

## Scope

### In Scope
- [IN-01] ValueObject を domain 層に配置するには、同一 track catalogue の domain 層にある別 entry の型・trait・関数シグネチャから当該 ValueObject への参照があることを要求する。これを満たさないアプリケーション境界の値ラッパーは、usecase 層の Dto または Command の構成要素として扱う既定を規約に反映する。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D1] [conv: knowledge/conventions/type-designer-kind-selection.md#R1. Layer-Kind Compatibility (層 × kind 互換マトリクス)] [tasks: T001]
- [IN-02] domain 配置の ValueObject に domain 層内からの inbound 参照がないことを、catalogue lint の cross-entry 検査として検出する。add・reference・modify のいずれの action も検査対象とし、接触した既存の誤配置には移設または正当な domain 消費者の同一 track 内での宣言を要求する。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D2] [conv: knowledge/conventions/enforce-by-mechanism.md#Rules] [tasks: T002]
- [IN-03] 公開アイテムが 0 の層について、d1 が存在し生成コマンドが exit 0 で完了すれば type-designer の 12a を充足し、d2 は生成されないという扱いを capability 仕様、生成器の文書、および実装で一致させる。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D3] [tasks: T003]
- [IN-04] contract-map style 設定が DataRole・ContractRole・FunctionRole の全値に対応する classDef を持つことを必須にし、renderer が未定義 role を検出した場合は既定スタイルへの無言のフォールバックではなく警告を出す。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D4] [tasks: T004]

### Out of Scope
- [OS-01] usecase 境界型が domain 型を公開面に出すことの可否や公開面規律は扱わない。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D1] [tasks: T001]
- [OS-02] ValueObject 以外の role における同種の配置誤誘導への規律拡張は扱わない。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D1] [tasks: T001]
- [OS-03] 既存 baseline 全体を一斉に修正すること、または誤配置 ValueObject の grandfather・自由文による免除レーンを設けることは扱わない。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D2] [tasks: T002]

## Constraints
- [CN-01] D1 の domain 使用要件は、catalogue lint により機械的に強制し、散文根拠だけで違反を免除する経路を設けない。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D2] [conv: knowledge/conventions/enforce-by-mechanism.md#Rules] [tasks: T001, T002]
- [CN-02] D2 の判定は contract-map renderer が構築する参照グラフと同じ情報源に基づく cross-entry 検査として実現する。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D2] [tasks: T002]
- [CN-03] 空層に対する d1 の存在と生成コマンドの成功を、d2 の物理的な存在へ読み替えず、12a の正規の充足条件として扱う。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D3] [tasks: T003]
- [CN-04] role 集合が将来拡張された場合にも style 完全性検査で欠落を検出できるよう、role 値と classDef の対応を網羅的に検証する。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D4] [tasks: T004]

## Acceptance Criteria
- [ ] [AC-01] type-designer の R1 が、domain の ValueObject には同一 track catalogue 内の別 domain entry のシグネチャからの参照を要求し、この要件を満たさない境界値ラッパーの既定配置を usecase 層として示している。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D1] [tasks: T001]
- [ ] [AC-02] catalogue lint は domain 内 inbound 参照がゼロの domain ValueObject を action の種別にかかわらずエラーとして報告し、接触した誤配置に移設または同一 track での正当な domain 消費者の宣言を要求する。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D2] [tasks: T002]
- [ ] [AC-03] 公開アイテム 0 の層では d1 が生成され、d2 は生成されず、d1 の存在と生成コマンドの exit 0 によって type-designer の 12a が充足することが capability 仕様、生成器の文書、および実装で一致している。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D3] [tasks: T003]
- [ ] [AC-04] contract-map style 設定は全 DataRole・ContractRole・FunctionRole の classDef を備え、renderer は未定義 role を検出すると警告を出して利用者が無スタイル描画を認識できる。 [adr: knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md#D4] [tasks: T004]

## Related Conventions (Required Reading)
- knowledge/conventions/type-designer-kind-selection.md#R1. Layer-Kind Compatibility (層 × kind 互換マトリクス)
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/catalogue-schema-reference.md#Catalogue Lint Rule Kinds (reference)
- knowledge/conventions/no-upstream-restatement.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Decision Flow
- knowledge/conventions/coding-principles.md#No Panics in Library Code

## Signal Summary

### Stage 1: Spec Signals
🔵 16  🟡 0  🔴 0

