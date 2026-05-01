<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 28, yellow: 0, red: 0 }
---

# enum variant の payload 型を schema レベルで宣言可能にする — catalogue / TypeGraph / baseline / serde codec の 4 点同時拡張

## Goal

- [GO-01] MemberDeclaration::Variant を名前文字列のみの保持から EnumVariantDeclaration 構造体 (name + payload_types) への変更によって、Enum / ErrorType の各 variant が保持する payload 型を catalogue schema で宣言できるようにし、enum 型と payload 型がグラフ上で孤立する問題を解消する [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1]
- [GO-02] ADR 2026-04-26-0855 の Core invariant (catalogue schema 拡張 / TypeGraph schema 拡張 / baseline schema 拡張 / serde codec は同時に決め、同時に実装する) を発動させ、MemberDeclaration::Variant の schema 変更を 4 点同時に完結させることで照合可能性を構造的に保証する [adr: knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md#Core invariant, knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1]
- [GO-03] Contract Map renderer と Reality View renderer の両方が enum variant の payload_types を参照して enum → payload type edge を描画できるようにし、設計意図の俯瞰 (Contract Map) と実装検証 (Reality View) の双方で variant 選択肢関係を可視化する [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1]

## Scope

### In Scope
- [IN-01] catalogue schema 変更: libs/domain/src/tddd/catalogue.rs の MemberDeclaration::Variant(String) を MemberDeclaration::Variant(EnumVariantDeclaration) に変更する。EnumVariantDeclaration は name: String と payload_types: Vec<String> を持つ。unit variant は payload_types: [] で表現する [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1] [tasks: T001]
- [IN-02] TypeGraph schema 変更: TypeNode::members の MemberDeclaration::Variant が payload_types を保持できるよう TypeGraph 側の schema を拡張する。IN-01 の catalogue 変更と 1:1 対応させる [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1, knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md#Core invariant] [tasks: T001]
- [IN-03] baseline schema 変更: TypeBaselineEntry::members が変更後の MemberDeclaration::Variant (payload_types 付き) を capture できるよう baseline schema を拡張する。modify / delete action での diff 比較が成立することを確保する [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1, knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md#Core invariant] [tasks: T001]
- [IN-04] serde codec 変更: MemberDeclaration::Variant の serde 表現を { "name": "...", "payload_types": [...] } 形式に変更する。新 schema 専用の codec とし、旧 schema (String 形式) を読む経路は持たない [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1] [tasks: T001]
- [IN-05] Contract Map renderer 拡張: catalogue の expected_variants[].payload_types の各 type token を type_index で resolve し、enum → payload type への edge を描画する。edge label は ::VariantName 形式とし、通常の field edge (.field_name) と区別する [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1] [tasks: T002]
- [IN-06] Reality View renderer 拡張: TypeNode::members の MemberDeclaration::Variant を走査し、payload_types に列挙された各 type token を resolve して enum → payload type への edge を描画する。ADR 2026-04-16-2200 D2 (b) の「variant 名のみ」制約はこの拡張によって上書きされる [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1, knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md#D2] [tasks: T003]
- [IN-07] rustdoc JSON 解析の拡張: schema_export.rs::extract_enum_variants() (infrastructure 層) が rustdoc JSON から enum variant の payload 型情報を抽出し、MemberDeclarationDto::Variant の payload_types に格納するよう拡張する。build_type_graph はこの拡張済み SchemaExport を受け取り TypeGraph に payload_types を伝播させる [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1] [tasks: T001]

### Out of Scope
- [OS-01] 過去 track の既存 catalogue の一括変換: 本 ADR 適用後に authored される新規 track の catalogue に新 schema を採用すれば足りる。旧 catalogue (expected_variants: ["Variant", ...] 形式) の retroactive 書き換えは行わない。プロジェクト方針として backward compat を持たない [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1] [tasks: T001]
- [OS-02] 旧 schema を読む移行 codec の導入: 新 schema serde codec は旧 schema (String 形式) を読む経路を持たない。旧 catalogue を参照する場合は手動での読み替えが必要 [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1] [tasks: T001]
- [OS-03] struct 系 kind の expected_members / expected_methods に関する schema 変更: 本 ADR は MemberDeclaration::Variant の payload_types 拡張に限定する。struct フィールド宣言の変更は別 track (tddd-struct-kind-uniformization) の対象 [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#M1] [tasks: T001, T002, T003, T004]
- [OS-04] TypeDefinitionKind taxonomy の新規 variant 追加: Enum / ErrorType の expected_variants 構造変更に留まる。新 kind variant の追加は別 ADR の対象 [adr: knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md#D1] [tasks: T001]
- [OS-05] struct variant (フィールド付き enum variant) の個別フィールド名宣言: payload_types は型文字列のリストのみを保持し、struct variant のフィールド名 (field_a: TypeA, field_b: TypeB) は本 ADR では宣言の対象外とする [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1] [tasks: T001]

## Constraints
- [CN-01] catalogue schema / TypeGraph schema / baseline schema / serde codec の 4 点は同時に変更し、同時に完結させる。段階分離 (catalogue のみ先行など) は採らない。Core invariant (ADR 2026-04-26-0855) に従う [adr: knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md#Core invariant] [tasks: T001]
- [CN-02] 新 schema serde codec は旧 schema (MemberDeclaration::Variant(String)) を読む経路を持たない。後方互換を維持する移行 layer は導入しない [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1] [tasks: T001, T004]
- [CN-03] payload_types は Vec<String> とし、generic 引数を含む完全な型文字列のリストで宣言する。unit variant は payload_types: [] で表現する。単一 payload のみに制限しない (tuple variant の複数 payload に対応するため) [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1] [tasks: T001]
- [CN-04] 両 renderer (Contract Map / Reality View) の edge label は ::VariantName 形式とし、通常の field edge (---|.field_name|) と意味論上区別できるようにする [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1] [tasks: T002, T003]
- [CN-05] Reality View renderer の variant payload edge 描画は ADR 2026-04-16-2200 D2 (b) の「variant 名のみ」制約を上書きする。両 renderer は同じ意味論 (enum → payload type edge) を生成する [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1, knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md#D2] [tasks: T003]
- [CN-06] rustdoc JSON から variant の payload 型情報を抽出する責務は infrastructure 層 (schema_export.rs::extract_enum_variants()) が持つ。domain 層は抽出ロジックを持たない (hexagonal architecture の layer 依存方向に従う)。build_type_graph は SchemaExport 経由でこの情報を受け取り TypeGraph に伝播させる [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] libs/domain/src/tddd/catalogue.rs の MemberDeclaration::Variant が EnumVariantDeclaration 型の値を保持し、Enum { expected_variants: Vec<EnumVariantDeclaration> } および ErrorType { expected_variants: Vec<EnumVariantDeclaration> } がコンパイルを通る [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1] [tasks: T001]
- [ ] [AC-02] TypeGraph schema (TypeNode::members) が変更後の MemberDeclaration::Variant (payload_types: Vec<String> を持つ) を保持できる。schema_export.rs::extract_enum_variants() が rustdoc JSON から variant payload 情報を抽出し、build_type_graph が SchemaExport 経由でそれを受け取り TypeGraph に格納する [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1, knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md#Core invariant] [tasks: T001]
- [ ] [AC-03] baseline schema (TypeBaselineEntry::members) が変更後の MemberDeclaration::Variant を capture できる。baseline capture コマンドが payload_types 情報を含む baseline JSON を書き出す [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1, knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md#Core invariant] [tasks: T001]
- [ ] [AC-04] serde codec が EnumVariantDeclaration を { "name": "...", "payload_types": [...] } 形式で serialize / deserialize するラウンドトリップテストが通る。旧形式 (String のみ) を読む経路が存在しない [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1] [tasks: T001, T004]
- [ ] [AC-05] Contract Map renderer が expected_variants[].payload_types を参照して enum → payload type edge を描画する。edge label が ::VariantName 形式で出力される [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1] [tasks: T002]
- [ ] [AC-06] Reality View renderer が TypeNode::members の MemberDeclaration::Variant の payload_types を走査して enum → payload type edge を描画する。edge label が ::VariantName 形式で出力される [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1, knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md#D2] [tasks: T003]
- [ ] [AC-07] cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) が pass する。4 点同時変更 (catalogue / TypeGraph / baseline / serde codec) による既存テストへのリグレッションが存在しない [adr: knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md#D1, knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md#Core invariant] [tasks: T004]

## Related Conventions (Required Reading)
- knowledge/conventions/source-attribution.md#Source Tag Types
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/type-designer-kind-selection.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 28  🟡 0  🔴 0

