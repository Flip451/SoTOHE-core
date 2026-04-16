<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: approved
approved_at: "2026-04-16T05:29:13Z"
version: "1.0"
signals: { blue: 41, yellow: 0, red: 0 }
---

# TDDD-05: Secondary Adapter variant の追加 — infrastructure 層における hexagonal port 実装の検証

## Goal

infrastructure 層の hexagonal secondary port 実装 (adapter) をカタログ化し、TDDD (Type-Definition-Driven Development) の本番運用化を完成させる
TypeDefinitionKind に新 variant SecondaryAdapter を追加し、複数の port を実装する adapter を 1 エントリで表現する (Vec<TraitImplDecl> 形式)
code_profile_builder.rs の trait 実装フィルタを緩和し、rustdoc JSON の trait 実装情報を schema 層に取り込む (Strategy S1)
evaluate_secondary_adapter 評価関数を追加し、各 adapter について implements で宣言した全 trait の実装が存在するかを検証する
infrastructure 内部 trait (GitRepository / GhClient) は本 track の対象外として後続 tddd-06 で扱う
ADR 起草と infrastructure-types.json の 11 エントリ作成は metadata.json::tasks[] に含めず、plan 段階および /track:design で扱う

## Scope

### In Scope
- domain catalogue: TraitImplDecl 新型を libs/domain/src/tddd/catalogue.rs に追加し、TypeDefinitionKind に SecondaryAdapter { implements: Vec<TraitImplDecl> } variant を追加する。kind_tag は "secondary_adapter"。consistency.rs の declared_type_names フィルタは既存の SecondaryPort | ApplicationService の補集合に SecondaryAdapter を自動的に含めるため、明示的な変更は最小限に留まる [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D1, knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D2, libs/domain/src/tddd/catalogue.rs:307, libs/domain/src/tddd/consistency.rs:170-195] [tasks: T001]
- infrastructure codec: libs/infrastructure/src/tddd/catalogue_codec.rs に TypeDefinitionKindDto::SecondaryAdapter と新 DTO TraitImplDeclDto を追加し、decode/encode と round-trip テストを実装する。EXISTENCE_ONLY_KINDS と is_method_bearing には secondary_adapter を追加しないことを test で保証する [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D2, libs/infrastructure/src/tddd/catalogue_codec.rs:228-249, libs/infrastructure/src/tddd/catalogue_codec.rs:306-307, convention — knowledge/conventions/typed-deserialization.md] [tasks: T002]
- domain schema: libs/domain/src/schema.rs に TraitImplEntry 新型 + TypeNode::trait_impls フィールド + TypeGraph::get_impl(type_name, trait_name) アクセサを追加する。schema_version は 2 のまま維持する (variant 追加は加法的) [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D3, libs/domain/src/schema.rs:403, libs/domain/src/schema.rs:353] [tasks: T003]
- infrastructure builder: libs/infrastructure/src/code_profile_builder.rs:36 の trait 実装フィルタ (i.trait_name().is_none()) を解除し、trait impls を別経路で TypeNode::trait_impls に格納する (Strategy S1)。outgoing 計算は引き続き inherent methods のみを使う設計を維持し、既存テスト test_build_type_graph_with_trait_impl_excludes_outgoing が pass し続けることを確認する。schema_export.rs:158-167 の trait_name 抽出パターンを流用する [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D4, libs/infrastructure/src/code_profile_builder.rs:36, libs/infrastructure/src/schema_export.rs:158-167, knowledge/research/2026-04-16-tddd-05-rustdoc-impl.md] [tasks: T004]
- domain evaluator: libs/domain/src/tddd/signals.rs に evaluate_secondary_adapter 関数 (Vec<TraitImplDecl> を loop で評価し集約 signal を返す) と evaluate_impl_methods 新 helper (method_structurally_matches を impl 側に流用) を追加し、evaluate_single の match arm に SecondaryAdapter variant を追加する。集約 signal の規則: 全 trait 確認済み → Blue、1 つでも未確認 → Red、struct 自体不在 → Yellow [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D2, libs/domain/src/tddd/signals.rs:301, libs/domain/src/tddd/signals.rs:334] [tasks: T005]
- infrastructure-types.json の 11 エントリ作成は plan 承認後の /track:design --layer infrastructure --force コマンド実行で行う。各 adapter の secondary_adapter エントリは Vec<TraitImplDecl> 形式で複数の port 実装を 1 エントリにまとめる。catalogue 作成自体は task ではないが、T005 完了後に signal を blue 化する手順として acceptance criteria に組み込む [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md Context §2, convention — .claude/rules/08-orchestration.md §Planner Gate] [tasks: T006]
- track 完了化: knowledge/adr/README.md の信号機アーキテクチャ section に本 ADR の索引行を追加し、verification.md の全 checkbox を埋め、Result セクションに各 task の実測結果を記入し、最終 cargo make ci の全通過を確認する [source: knowledge/adr/README.md §索引 信号機アーキテクチャ, track/items/domain-serde-ripout-2026-04-15/verification.md §5. ADR README index + verification 完了 (T005), convention — knowledge/conventions/source-attribution.md] [tasks: T006]

### Out of Scope
- infrastructure 内部 trait (GitRepository / GhClient) のカタログ化。これらは port owner と adapter owner が同じ infrastructure 層に存在し、hexagonal の secondary port 意味論と異なる。後続 tddd-06-cli-wiring で別 variant を検討する [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D5, knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §I (Rejected Alternatives), convention — knowledge/conventions/hexagonal-architecture.md]
- cli 層の TDDD 拡張 (→ tddd-06-cli-wiring) [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D5, knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §Reassess When 2]
- domain / usecase 層の catalogue 拡張。本 track は infrastructure 層のみを対象とする [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md Context §1, knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §6 Two-track split]
- domain crate に serde 依存を戻すこと。Track 1 §D1 で確立された hexagonal 純粋性の不変条件を維持する [source: knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §D1, convention — knowledge/conventions/hexagonal-architecture.md]
- rustdoc JSON parser の rewrite。既存 schema_export.rs:158-167 の trait_name 抽出パターンを流用するため、parser 自体への変更は不要 [source: libs/infrastructure/src/schema_export.rs:158-167, knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D4]
- review-system v3 や他の aspect 変更 [source: knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §6]
- SecondaryAdapter variant 固有の追加検証ルール強化 (例: adapter は対応 port の全 method を実装すること)。本 track では expected_methods が optional な存在チェック中心の設計を採用し、強化は将来 reassess 時に検討する [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §Reassess When, knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md §D3]
- Strategy S3 (TraitNode::implementors による逆引き) の追加。S1 で十分であり、双方向参照は tddd-06 以降で検討する [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §H (Rejected Alternatives)]
- rustdoc cache 戦略 (Phase D)。CI rustdoc の wall time が許容範囲内であれば不要、許容外であれば別 sub-track tddd-rustdoc-cache-YYYY-MM-DD に分離する [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md §3.E (deferred), knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §6 Two-track split]
- codec duplicate validation の compound key 拡張 (Option D-1)。TypeSignal::type_name の単一キー前提と矛盾するため ADR で却下済み [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §Rejected Alternatives D]

## Constraints
- domain layer に serde 依存を戻さない (Track 1 §D1 不変条件) [source: knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §D1, convention — knowledge/conventions/hexagonal-architecture.md]
- small task commits: 各 task の diff は 500 行未満を目標とする [source: convention — .claude/rules/10-guardrails.md §Small task commits]
- ADR-first gate: 設計判断は ADR を Accepted にしてから実装着手する。本 track では knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md が plan 段階で Accepted 済み [source: knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §D1, knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §Status]
- TypeAction (add / modify / delete / reference) との整合: SecondaryAdapter の delete action は struct 側 (get_type) で存在確認のみ行う。impl 単独削除の検出は本 track の対象外 [source: knowledge/adr/2026-04-11-0003-type-action-declarations.md, knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §Bad]
- 4 グループ評価との整合: SecondaryAdapter は forward check (グループ 1, 2) で実装の存在を確認し、reverse check (グループ 4) では未宣言の adapter を Red として検出する [source: knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md §3, knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D2]
- アクティブでない track のデータに影響を与えないこと。active-track guard (ADR 2026-04-15-1012 D1) により completed / archived track への type-signals 実行は拒否される。各 track は独自のカタログ / baseline / signal を持つため他 track との後方互換性は不要 [source: knowledge/adr/2026-04-15-1012-catalogue-active-guard-fix.md §D1, knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §Good]
- code_profile_builder の outgoing 計算は inherent methods のみを使う設計を維持する。trait impls を別経路で TypeNode::trait_impls に格納することで、既存 typestate 検出 (test_build_type_graph_with_trait_impl_excludes_outgoing) には影響しない [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D4, libs/infrastructure/src/code_profile_builder.rs:36]
- ADR 0002 D6 の layer-agnostic 不変条件: TraitImplDecl と SecondaryAdapter variant に層名 ("domain" / "usecase" 等) をハードコードしない。trait の所属層情報は description フィールドで運用する [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md §D6, knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D6]
- TDD red→green の順序を守る。各 task で test を先に追加し、red 状態を確認してから実装で green 化する [source: convention — .claude/rules/05-testing.md §Core Principles]
- 既存の MethodDeclaration / evaluate_trait_methods / method_structurally_matches ヘルパーの再利用を優先する。`evaluate_secondary_adapter` で必要な method 一致判定は impl 側のメソッドリストに対して同様のロジックを適用する [source: libs/domain/src/tddd/signals.rs:334, knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D2]

## Acceptance Criteria
- [ ] libs/domain/src/tddd/catalogue.rs に pub struct TraitImplDecl と TypeDefinitionKind::SecondaryAdapter { implements: Vec<TraitImplDecl> } variant が追加されている [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D1, knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D2] [tasks: T001]
- [ ] TypeDefinitionKind::SecondaryAdapter の kind_tag が "secondary_adapter" を返し、consistency.rs の type / trait 区分で type 側に分類されることを test で保証する [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D1, libs/domain/src/tddd/consistency.rs:170-195] [tasks: T001]
- [ ] libs/infrastructure/src/tddd/catalogue_codec.rs に TypeDefinitionKindDto::SecondaryAdapter と新 DTO TraitImplDeclDto が追加され、decode/encode の round-trip テストが pass する [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D2, convention — knowledge/conventions/typed-deserialization.md] [tasks: T002]
- [ ] catalogue_codec.rs:228-249 の EXISTENCE_ONLY_KINDS と line 306-307 の is_method_bearing の closure には secondary_adapter が追加されていないことを test で保証する [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §Neutral, libs/infrastructure/src/tddd/catalogue_codec.rs:228-249, libs/infrastructure/src/tddd/catalogue_codec.rs:306-307] [tasks: T002]
- [ ] libs/domain/src/schema.rs に pub struct TraitImplEntry 新型と TypeNode::trait_impls: Vec<TraitImplEntry> フィールドと TypeGraph::get_impl(type_name, trait_name) -> Option<&TraitImplEntry> アクセサが追加されている [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D3] [tasks: T003]
- [ ] libs/infrastructure/src/code_profile_builder.rs:36 の trait 実装フィルタが解除され、trait impls が TypeNode::trait_impls に格納されることが test で保証されている。同時に、既存テスト test_build_type_graph_with_trait_impl_excludes_outgoing が依然として pass する (outgoing 計算が trait impls の影響を受けないことを保証) [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D4, libs/infrastructure/src/code_profile_builder.rs:36] [tasks: T004]
- [ ] libs/domain/src/tddd/signals.rs に evaluate_secondary_adapter 関数と evaluate_impl_methods helper が追加され、evaluate_single の match arm が SecondaryAdapter variant を扱う。Blue / Yellow / Red の集約規則が test で保証されている (全 trait 確認済 → Blue、struct 自体不在 → Yellow、1 つでも未確認 → Red) [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D2] [tasks: T005]
- [ ] track/items/tddd-05-infra-wiring-2026-04-15/infrastructure-types.json が 11 エントリ以上で存在する。作成方法: T001-T002 完了後に .claude/commands/track/design.md に SecondaryAdapter variant を追記してから /track:design --layer infrastructure --force を実行するか、orchestrator が ADR を参照して手動作成する [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md Context §2 (17 trait impls 表), knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D5] [tasks: T006]
- [ ] bin/sotp track type-signals tddd-05-infra-wiring-2026-04-15 --layer infrastructure が blue=N (N>=11) yellow=0 red=0 を返す [source: knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md Context §2, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md] [tasks: T006]
- [ ] knowledge/adr/README.md の信号機アーキテクチャ section に knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md の索引行が追加されている [source: knowledge/adr/README.md §索引 信号機アーキテクチャ, track/items/domain-serde-ripout-2026-04-15/verification.md §T005] [tasks: T006]
- [ ] libs/domain/Cargo.toml に serde 依存が含まれていない (Track 1 §D1 不変条件の維持)。grep 'serde' libs/domain/Cargo.toml がゼロマッチであることを確認する [source: knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §D1] [tasks: T006]
- [ ] cargo make ci が全通過する (CI gate の具体的なチェック項目は Makefile.toml の ci-local dependencies を参照) [source: convention — .claude/rules/07-dev-environment.md §Pre-commit Checklist] [tasks: T006]
- [ ] track/items/tddd-05-infra-wiring-2026-04-15/verification.md に「infrastructure TDDD full production 宣言」セクションが記載され、後続トラック (tddd-06-cli-wiring 等) への引継ぎ事項が明記されている [source: track/items/domain-serde-ripout-2026-04-15/verification.md §Track 2 引継ぎ事項, knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md §D5] [tasks: T006]
- [ ] PR review (Codex Cloud @codex review) で zero findings を達成する [source: convention — knowledge/conventions/review-protocol.md] [tasks: T006]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md
- knowledge/conventions/source-attribution.md
- knowledge/conventions/typed-deserialization.md
- knowledge/conventions/prefer-type-safe-abstractions.md
- knowledge/conventions/nightly-dev-tool.md
- knowledge/conventions/review-protocol.md

## Signal Summary

### Stage 1: Spec Signals
🔵 41  🟡 0  🔴 0

