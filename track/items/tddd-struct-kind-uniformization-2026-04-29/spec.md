<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 31, yellow: 0, red: 0 }
---

# TDDD struct kind 均質化と type catalogue linter framework の導入

## Goal

- [GO-01] TypeDefinitionKind の struct 系 9 kind すべてに expected_members と expected_methods を均質に持たせることで、catalogue schema の表現能力を Rust struct の実態 (field + method 両保持) に合わせ、validating constructor の error 型・補助 method などこれまで表現できなかった型契約を catalogue に宣言できるようにする [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#M1]
- [GO-02] field と behavior method を持つ domain 層の behavioral struct (DDD domain service パターン) の正しい住所として domain_service kind を新設し、value_object semantic restriction を守りながら kind 選択の歪みを解消する [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S1]
- [GO-03] contract-map renderer の methods_of() を全 struct kind 対応に拡張し、catalogue に宣言済みの全 struct 系 kind の method edge が contract-map に出力されるようにする。これにより catalogue 宣言済みの型に起因する contract-map orphan を自然解消する [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S2]
- [GO-04] type catalogue linter framework の 3 primitive (field-empty / field-non-empty / kind-layer constraint) を決定し、convention の kind 配置層マトリクス / value_object semantic restriction / kind 固有制約を機械的に enforce する経路の基盤を確立する [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S3]

## Scope

### In Scope
- [IN-01] libs/domain/src/tddd/catalogue.rs の TypeDefinitionKind 変更: struct 系 9 kind (Typestate / ValueObject / UseCase / Interactor / Dto / Command / Query / Factory / SecondaryAdapter) すべてに expected_methods: Vec<MethodDeclaration> フィールドを追加する。既存の expected_members は保持し、例外なく uniform にする [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#M1] [tasks: T001, T002]
- [IN-02] TypeDefinitionKind への domain_service variant 新設: expected_members と expected_methods を持つ新 kind として DomainService { expected_members: Vec<MemberDeclaration>, expected_methods: Vec<MethodDeclaration> } を追加し、domain / usecase 層への配置を許可する (infrastructure は禁止) [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S1] [tasks: T001, T002]
- [IN-03] libs/domain/src/tddd/contract_map_render.rs の methods_of() 拡張: SecondaryPort / ApplicationService / struct 系 8 kind (M1 で expected_methods が付く種) および DomainService (S1) を top-level expected_methods を uniform に返す 1 つの arm で処理する。SecondaryAdapter は top-level expected_methods と implements[].expected_methods の 2 source merge の意図的な専用 arm として残す [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S2] [tasks: T003]
- [IN-04] catalogue codec の更新: M1 + S1 によって変更された TypeDefinitionKind の serde serialize / deserialize を新 schema 専用として実装する。旧 schema を読む経路は持たない (no-backward-compat 方針に従い新規 track の catalogue から uniform schema を採用すれば足りる) [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#M1] [tasks: T002]
- [IN-05] type catalogue linter framework の 3 primitive 設計と実装決定: (1) field-empty enforcement (特定 kind の特定 field が空であることを強制)、(2) field-non-empty enforcement (特定 kind の特定 field が非空であることを強制)、(3) kind-layer constraint (ある kind が指定 layer でのみ宣言できることを強制) を bin/sotp の linter サブコマンドとして実装する framework の基盤を確立する。具体的な DSL / config schema / CLI 統合の詳細設計は別 ADR / 別 track の対象とする [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S3] [tasks: T004, T005, T006, T007]
- [IN-06] knowledge/conventions/type-designer-kind-selection.md の更新: R1 layer-kind 互換マトリクスへの domain_service 行追加 (domain ✓ / usecase △ / infrastructure ✗)、R3 value_object semantic restriction の linter 移行 (S3) への対応、R5 No Fallback ルールの domain_service 選択肢追加 [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S1, knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S3] [tasks: T008]
- [IN-07] TypeGraph / baseline schema への変更は不要であることの確認: 既存の TypeNode::members / TypeNode::methods および TypeBaselineEntry::members / TypeBaselineEntry::methods が uniform 化された catalogue field を受け取れるため、ADR 2026-04-26-0855 の Core invariant (catalog / TypeGraph / baseline 同時更新) は既存フィールドの再利用で満たされる [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#M1, knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S1] [tasks: T001]

### Out of Scope
- [OS-01] 他 track の既存 catalogue の一括変換: 本 ADR が適用される時点以降に authored される新規 track の catalogue で uniform schema を採用すれば足りる。他 track の既存 catalogue を retroactive に書き換える作業は行わない (project 方針として backward compat は持たない / track 跨ぎ整合は非推奨) [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#M1]
- [OS-02] linter framework の具体的な config schema / rule DSL / CLI 統合の設計と実装: S3 は本 ADR では framework の必要性と 3 primitive の方針を決定するのみ。config schema 設計・rule DSL・CLI フラグ仕様の詳細は別 ADR / 別 track の対象とする [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S3]
- [OS-03] catalogue 自体が未宣言の型 (ComplianceContext 等、別 track で catalogue 起草対象となっているもの) の orphan 解消: 本 track の contract-map orphan 解消範囲は catalogue 宣言済みの型に限定される。catalogue 未宣言型の orphan は別途 catalogue 起草が必要 [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S2]
- [OS-04] struct 系以外の kind (enum / error_type / trait 系 / free_function) の均質化に類する schema 変更: 本 ADR は struct 系 9 kind + domain_service 新設のみを扱う。enum variant 系や trait 系 kind の schema 拡張は別 ADR で扱う [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#M1]
- [OS-05] No Fallback ルール (どの kind も fit しないとき value_object 等に押し込まない) の linter primitive による直接表現: No Fallback ルールは 3 primitive では直接表現できないため、type-designer 側のロジックで引き続き担当する [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S3]

## Constraints
- [CN-01] M1 適用は struct 系 9 kind すべてに例外なく適用する。ValueObject を含め特定 kind を expected_methods のない非対称 schema に固定しない。schema は表現能力を担い、意味論ポリシーは linter (S3) で enforce する (関心分離の原則) [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#M1] [tasks: T001, T002]
- [CN-02] catalogue codec は新 schema 専用とし、旧 schema を読む経路は持たない。後方互換を維持する移行 layer は導入しない [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#M1] [tasks: T002]
- [CN-03] domain_service kind の配置層は domain (✓) および usecase (△ 要根拠) に限定する。infrastructure 層への配置は禁止する。usecase 配置は trans-domain な application logic の場合のみ許可し、informal_grounds[] に説明を記録する [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S1] [tasks: T001, T008]
- [CN-04] Interactor との排他的境界を守る: domain_service には declares_application_service がない (trait 実装を持たない)。application_service / secondary_port を実装するなら domain_service ではなく interactor / secondary_adapter を選ぶ [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S1] [tasks: T001]
- [CN-05] SecondaryAdapter の methods_of() arm は意図的な専用 arm として維持する: top-level expected_methods (M1 で新設) と implements[].expected_methods の 2 source merge が必要なため、struct 系 8 kind の uniform arm とは分けて処理する [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S2] [tasks: T003]
- [CN-06] linter framework の rule は project config で disable / 緩和 / 拡張できるよう customizability を確保する。opt-out 経路を設けることで false positive (例: newtype tuple struct の pub inner field 宣言) に対応する [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S3] [tasks: T004, T005]
- [CN-07] TypeGraph / baseline schema は変更しない。既存の TypeNode::members / TypeNode::methods および TypeBaselineEntry::members / TypeBaselineEntry::methods が uniform 化された catalogue を受け取れるため、ADR 2026-04-26-0855 の Core invariant は既存フィールドの再利用で満たす [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#M1, knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S1] [tasks: T001]
- [CN-08] renderer 出力 (contract-map.md / <layer>-types.md) の手編集禁止運用は引き続き必須。DO NOT EDIT DIRECTLY marker を維持する [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S2] [tasks: T003]

## Acceptance Criteria
- [ ] [AC-01] libs/domain/src/tddd/catalogue.rs の TypeDefinitionKind コンパイルが通り、struct 系 9 kind (Typestate / ValueObject / UseCase / Interactor / Dto / Command / Query / Factory / SecondaryAdapter) すべてが expected_methods: Vec<MethodDeclaration> フィールドを持つ。DomainService variant が追加されており、expected_members と expected_methods を持つ [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#M1, knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S1] [tasks: T001]
- [ ] [AC-02] contract_map_render.rs の methods_of() が全 struct kind 対応になっており、struct 系 8 kind (M1 適用後) および DomainService が top-level expected_methods を返す uniform arm で処理される。SecondaryAdapter が 2 source merge の専用 arm を持つ。既存の SecondaryPort / ApplicationService arm は uniform arm に統合されるか維持される [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S2] [tasks: T003]
- [ ] [AC-03] catalogue codec (serde serialize / deserialize) が新 schema の expected_methods フィールドを正しくエンコード / デコードする。DomainService を含む全 struct 系 kind のラウンドトリップが通る [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#M1] [tasks: T002]
- [ ] [AC-04] domain_service kind の layer-kind 制約が type-signal 評価または catalogue codec で確認できる。infrastructure 層への domain_service 配置が拒否される経路が存在する (codec validation または linter rule の 3 primitive の kind-layer constraint として実装) [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S1, knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S3] [tasks: T005, T006]
- [ ] [AC-05] linter framework の 3 primitive (field-empty / field-non-empty / kind-layer constraint) が bin/sotp のサブコマンドとして呼び出せる。value_object の expected_methods 空強制を表現するルールを 3 primitive で定義・実行できることを確認する [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S3] [tasks: T004, T005, T006, T007]
- [ ] [AC-06] knowledge/conventions/type-designer-kind-selection.md の R1 layer-kind 互換マトリクスに domain_service 行が追加されており (domain ✓ / usecase △ / infrastructure ✗)、R3 および R5 に domain_service への言及が含まれている [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S1] [tasks: T008]
- [ ] [AC-07] cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) が pass する。M1 / S1 / S2 の実装変更が既存テストを壊さず、新規ユニットテストが追加されている [adr: knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#M1, knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md#S2] [tasks: T009]

## Related Conventions (Required Reading)
- knowledge/conventions/type-designer-kind-selection.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/source-attribution.md#Source Tag Types
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 31  🟡 0  🔴 0

