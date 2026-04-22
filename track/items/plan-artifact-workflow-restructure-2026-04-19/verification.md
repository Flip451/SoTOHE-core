# Verification — 計画成果物ワークフローの再構築 (Scope D: T1 + T2 + T3)

> **Track**: `plan-artifact-workflow-restructure-2026-04-19`
> **ADR**: `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md`
> **Scope**: T1 (convention + SKILL 刷新) + T2 (schema 分割 + CI gate 再整理 + task-completion gate 読み替え) + T3 (plan-artifact-refs CLI 新設)

## 検証範囲

- convention 新設 (`workflow-ceremony-minimization.md` / `pre-track-adr-authoring.md`)
- CLAUDE.md / DEVELOPER_AI_WORKFLOW.md / track/workflow.md の track 前段階 + 3 フェーズ明記
- SKILL / command / agent (planner, designer) から approved 廃止 + ADR 事前確認 + D1.6 research 配置反映
- libs/domain/src/plan_ref/ 新モジュール導入 (ref 種別ごとに 1 ファイル)
- 値オブジェクト newtype 6 種 (SpecElementId / AdrAnchor / ConventionAnchor / ContentHash / InformalGroundKind / InformalGroundSummary) の導入
- 4 独立 ref 構造体 (AdrRef / ConventionRef / SpecRef / InformalGroundRef) の導入
- spec.json schema 刷新 (status / approved_at / content_hash / task_refs 削除、id 必須化、sources → adr_refs + convention_refs + informal_grounds の 3 分割)
- 型カタログに spec_refs + informal_grounds 追加
- metadata.json を identity-only に縮小
- impl-plan.json / task-coverage.json schema 新設
- verify-track-metadata-local / verify-latest-track-local の file 存在ベース改訂
- 空カタログ受け入れ
- check_tasks_resolved_from_git_ref の impl-plan.json 読み替え
- plan.md / spec.md renderer の集約化
- cargo make spec-approve 廃止
- sotp verify plan-artifact-refs CLI 新設 (ref schema / anchor / hash / canonical block 警告)
- 既存 spec_coverage::verify の新 CLI 統合

## 手動検証手順

### T001 (convention + SKILL 刷新)

1. `knowledge/conventions/workflow-ceremony-minimization.md` と `knowledge/conventions/pre-track-adr-authoring.md` が存在し、ADR §D4 / §D5 の内容を反映している
2. CLAUDE.md / DEVELOPER_AI_WORKFLOW.md / track/workflow.md に track 前段階 + 3 フェーズの明記がある
3. SKILL.md / track/plan.md / track/design.md / planner.md / designer.md から「approved」「spec-approve」への依存記述が消えている
4. D1.6 research 配置規約 (`track/items/<id>/research/<timestamp>-<capability>-*.md`) が明文化されている

### T002 (plan_ref モジュール + 値オブジェクト + ref 構造体)

1. `libs/domain/src/plan_ref/` が新設され、ref 種別ごとに 1 ファイル構成 (mod.rs + adr_ref.rs + convention_ref.rs + spec_ref.rs + informal_ground_ref.rs) である
2. `libs/domain/src/plan_ref/` に 6 newtype (SpecElementId / AdrAnchor / ConventionAnchor / ContentHash / InformalGroundKind / InformalGroundSummary) が存在する
3. 各 newtype の `try_new` / `new` コンストラクタで validation が閉じ込められている (SpecElementId: 非空 + ID 命名規則 / AdrAnchor / ConventionAnchor: 非空 loose / ContentHash: SHA-256 32 バイト / InformalGroundKind: 4 variant enum / InformalGroundSummary: 非空)
4. 4 独立 ref 構造体 (AdrRef / ConventionRef / SpecRef / InformalGroundRef) が独立構造体として定義され、共通 trait / enum 抽象化がない
5. 既存 `libs/domain/src/ids.rs` とは分離されており、NonEmptyString の流用がない
6. `cargo nextest run -p domain` で newtype / ref struct の unit tests が pass する

### T003 (spec.json schema 刷新)

1. 新 SpecDocument に `status` / `approved_at` / トップレベル `content_hash` / 各要素 `task_refs` が存在しない
2. 各 spec element (goal / in_scope / out_of_scope / constraints / acceptance_criteria) に `id: SpecElementId` が必須
3. `sources` field が `adr_refs: Vec<AdrRef>` + `convention_refs: Vec<ConventionRef>` + `informal_grounds: Vec<InformalGroundRef>` の 3 つに分割されている
4. `related_conventions` が `Vec<ConventionRef>` になっている
5. spec/codec の serde round-trip tests が新 schema で pass する
6. spec-signals ツールが adr_refs と informal_grounds を合成して総合 signal を算出する (informal_grounds 非空 → 🟡 発火)

### T004 (impl-plan.json / task-coverage.json schema 新設)

1. `libs/domain/` に ImplPlanDocument が schema_version + tasks + plan で定義されている
2. `libs/domain/` に TaskCoverageDocument が 4 セクション (in_scope / out_of_scope / constraints / acceptance_criteria) の task_refs 構造で定義されている
3. 両 document の codec が `libs/infrastructure/` に存在し、serde round-trip tests が pass する

### T005 (catalogue spec_refs + informal_grounds + metadata identity-only)

1. 型カタログドキュメントに `spec_refs: Vec<SpecRef>` + `informal_grounds: Vec<InformalGroundRef>` field が追加されている
2. TrackMetadata から `tasks` / `plan` が削除され、identity field のみ保持する
3. 既存 v3 metadata.json との並立戦略 (schema_version 対応 or 互換アダプタ) が実装されている
4. codec round-trip tests が pass する

### T006 (verify-*-local + 空カタログ受け入れ)

1. `verify-track-metadata-local` が identity field のみ検証する (tasks / plan を要求しない)
2. `verify-latest-track-local` が impl-plan.json の存在を条件として task 項目チェックを分岐する
3. `sotp track type-signals` / `baseline-capture` / 関連 verify が 0 件の空カタログで pass する
4. tests が新挙動を反映している

### T007 (check_tasks_resolved_from_git_ref の impl-plan.json 読み替え)

1. `libs/usecase/src/task_completion.rs` の `check_tasks_resolved_from_git_ref` が impl-plan.json を読んで task resolution を判定する
2. `TrackBlobReader` port が impl-plan.json 読み取り用に改修されている (read_impl_plan の追加 or read_track_metadata の置換)
3. `apps/cli/src/commands/pr.rs` の呼び出し側も改修されている
4. K1-K7 MockReader tests が新 port で pass する (plan/ branch skip / 全 resolved pass / 未解決 BLOCKED / NotFound / FetchError / 危険 branch / 空 branch)

### T008 (renderer 集約化)

1. plan.md が metadata.json + impl-plan.json を集約して render される
2. spec.md が spec.json + task-coverage.json を集約して render される
3. render tests が集約形式を確認する

### T009 (spec-approve 廃止)

1. `cargo make spec-approve` が Makefile.toml から削除されている
2. 関連 CLI コマンド (`bin/sotp spec approve` 等) が廃止されている
3. spec-signals ツールが status / approved_at / content_hash 依存なしで動作する
4. 既存 tests から spec-approve 依存の assertion が除去されている

### T010 (sotp verify plan-artifact-refs CLI - ref validation)

1. `apps/cli/src/commands/verify/plan_artifact_refs.rs` が新設されている
2. 各 ref field (adr_refs / convention_refs / spec_refs / related_conventions / informal_grounds) を走査する
3. schema validation / file 存在 (file ベース ref のみ) / SpecRef.anchor 解決 / SpecRef.hash 照合 / AdrAnchor / ConventionAnchor の loose validation / InformalGroundRef (kind variant + summary 非空) validation が機能する
4. unit + integration tests で各分岐が網羅されている

### T011 (CLI に task-coverage + canonical block + CI 統合)

1. task-coverage.json の coverage 強制 + referential integrity 検査が追加されている
2. canonical block 疑惑検出 (ADR 例示マーカー除外 / spec 内 10 行超コードブロック警告) が実装されている
3. `cargo make ci` / Makefile.toml に組み込まれ、違反時に fail-closed する

### T012 (spec_coverage::verify 統合)

1. 既存 `spec_coverage::verify` が新 CLI に統合されている
2. 旧呼び出し経路 (Makefile.toml / scripts/) が新 CLI 経由に切り替わっている
3. 重複コードが削除されている
4. 既存動作との互換性が tests で確認されている

### 共通

1. `cargo make ci` が全通過する (fmt-check + clippy + nextest + test-doc + deny + check-layers + verify-*)
2. 既存 track には遡及適用されず、既存 track 読み取りが壊れていない
3. `cargo make track-sync-views` で plan.md / spec.md の集約 render が正常に生成される

## 結果 / 未解決事項

(実装完了時に記録)

## verified_at

(実装完了時に記録)
